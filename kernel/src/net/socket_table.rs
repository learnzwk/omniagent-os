//! Socket 表管理

use alloc::boxed::Box;

use crate::net::error::NetError;
use crate::net::protocol::{
    ProtocolSocket, SocketDomain, SocketState, SocketType, TcpSocket, UdpSocket,
};

/// Socket 表条目（不含 protocol，用于查询）
#[derive(Debug, Clone)]
pub struct SocketEntry {
    /// 文件描述符
    pub fd: u32,
    /// Socket 域
    pub domain: SocketDomain,
    /// Socket 类型
    pub socket_type: SocketType,
    /// Socket 状态
    pub state: SocketState,
}

/// 最大 Socket 数量
const MAX_SOCKETS: usize = 256;

/// Socket 内部元数据
struct SocketMeta {
    /// 文件描述符
    fd: u32,
    /// Socket 域
    domain: SocketDomain,
    /// Socket 类型
    pub socket_type: SocketType,
    /// Socket 状态
    state: SocketState,
}

/// Socket 表，管理所有活跃的 Socket
///
/// 使用两个独立的数组分别存储元数据和协议 Socket，
/// 以避免嵌套锁的问题。
pub struct SocketTable {
    /// Socket 元数据数组
    metas: spin::Mutex<[Option<SocketMeta>; MAX_SOCKETS]>,
    /// 协议 Socket 数组（每个 Socket 使用独立的 Mutex）
    /// 与 metas 数组一一对应，当 metas[i] 为 Some 时 protocols[i] 有效
    protocols: [spin::Mutex<Option<Box<dyn ProtocolSocket>>>; MAX_SOCKETS],
    /// 下一个可用的文件描述符
    next_fd: spin::Mutex<u32>,
}

impl SocketTable {
    /// 创建新的 Socket 表
    pub fn new() -> Self {
        SocketTable {
            metas: spin::Mutex::new([const { None }; MAX_SOCKETS]),
            protocols: [const { spin::Mutex::new(None) }; MAX_SOCKETS],
            next_fd: spin::Mutex::new(0),
        }
    }

    /// 创建新的 Socket，返回文件描述符
    pub fn create(&self, domain: SocketDomain, socket_type: SocketType) -> Result<u32, NetError> {
        let protocol: Box<dyn ProtocolSocket> = match socket_type {
            SocketType::Stream => Box::new(TcpSocket::new()),
            SocketType::Datagram => Box::new(UdpSocket::new()),
            SocketType::Raw => {
                return Err(NetError::ProtocolError {
                    reason: "Raw Socket 暂不支持",
                })
            }
        };

        let mut metas = self.metas.lock();
        let mut next_fd = self.next_fd.lock();

        // 查找空闲槽位
        let mut found_slot = None;
        for (i, slot) in metas.iter_mut().enumerate() {
            if slot.is_none() {
                found_slot = Some(i);
                break;
            }
        }

        let slot = found_slot.ok_or(NetError::SocketTableFull)?;
        let fd = *next_fd;
        *next_fd += 1;

        let state = protocol.state();
        metas[slot] = Some(SocketMeta {
            fd,
            domain,
            socket_type,
            state,
        });

        // 将 protocol 存入独立的 Mutex
        *self.protocols[slot].lock() = Some(protocol);

        Ok(fd)
    }

    /// 获取 Socket 元信息（不含 protocol）
    pub fn get(&self, fd: u32) -> Result<SocketEntry, NetError> {
        let metas = self.metas.lock();
        for slot in metas.iter() {
            if let Some(meta) = slot {
                if meta.fd == fd {
                    return Ok(SocketEntry {
                        fd: meta.fd,
                        domain: meta.domain,
                        socket_type: meta.socket_type,
                        state: meta.state,
                    });
                }
            }
        }
        Err(NetError::InvalidSocket(fd as i32))
    }

    /// 获取协议 Socket 的可变引用
    ///
    /// 返回 `MutexGuard` 持有对 `Option<Box<dyn ProtocolSocket>>` 的引用。
    /// 调用者需要自行处理 `None` 的情况。
    pub fn get_protocol(
        &self,
        fd: u32,
    ) -> Result<spin::MutexGuard<'_, Option<Box<dyn ProtocolSocket>>>, NetError> {
        // 先在 metas 锁内找到槽位索引
        let slot_index = {
            let metas = self.metas.lock();
            let mut found = None;
            for (i, slot) in metas.iter().enumerate() {
                if let Some(meta) = slot {
                    if meta.fd == fd {
                        found = Some(i);
                        break;
                    }
                }
            }
            found.ok_or(NetError::InvalidSocket(fd as i32))?
        };
        // 释放 metas 锁后，获取独立的 protocol 锁
        Ok(self.protocols[slot_index].lock())
    }

    /// 关闭指定文件描述符的 Socket
    pub fn close(&self, fd: u32) -> Result<(), NetError> {
        let mut metas = self.metas.lock();
        for (i, slot) in metas.iter_mut().enumerate() {
            if let Some(meta) = slot {
                if meta.fd == fd {
                    *slot = None;
                    // 同时清除 protocol
                    *self.protocols[i].lock() = None;
                    return Ok(());
                }
            }
        }
        Err(NetError::InvalidSocket(fd as i32))
    }

    /// 获取当前活跃的 Socket 数量
    pub fn count(&self) -> usize {
        let metas = self.metas.lock();
        metas.iter().filter(|s| s.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_table_create() {
        let table = SocketTable::new();
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        assert_eq!(fd, 0);
        assert_eq!(table.count(), 1);

        let fd2 = table.create(SocketDomain::Inet, SocketType::Datagram).unwrap();
        assert_eq!(fd2, 1);
        assert_eq!(table.count(), 2);
    }

    #[test]
    fn test_socket_table_close() {
        let table = SocketTable::new();
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        assert_eq!(table.count(), 1);

        table.close(fd).unwrap();
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn test_socket_table_get() {
        let table = SocketTable::new();
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        let entry = table.get(fd).unwrap();
        assert_eq!(entry.fd, fd);
        assert_eq!(entry.domain, SocketDomain::Inet);
        assert_eq!(entry.socket_type, SocketType::Stream);
        assert_eq!(entry.state, SocketState::Created);
    }

    #[test]
    fn test_socket_table_invalid() {
        let table = SocketTable::new();
        let result = table.get(999);
        assert!(result.is_err());
        match result.unwrap_err() {
            NetError::InvalidSocket(fd) => assert_eq!(fd, 999),
            _ => panic!("期望 InvalidSocket 错误"),
        }

        let close_result = table.close(999);
        assert!(close_result.is_err());
    }

    #[test]
    fn test_socket_table_max() {
        let table = SocketTable::new();

        // 创建 MAX_SOCKETS 个 Socket
        for _ in 0..MAX_SOCKETS {
            table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        }
        assert_eq!(table.count(), MAX_SOCKETS);

        // 第 MAX_SOCKETS + 1 个应该失败
        let result = table.create(SocketDomain::Inet, SocketType::Stream);
        assert!(result.is_err());
        match result.unwrap_err() {
            NetError::SocketTableFull => {}
            _ => panic!("期望 SocketTableFull 错误"),
        }
    }
}
