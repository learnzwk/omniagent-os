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
            metas: spin::Mutex::new(core::array::from_fn(|_| None)),
            protocols: core::array::from_fn(|_| spin::Mutex::new(None)),
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
                    reason: alloc::string::String::from("Raw Socket 暂不支持"),
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

    /// 测试：关闭后重新分配应复用槽位
    #[test]
    fn test_socket_table_close_and_realloc() {
        let table = SocketTable::new();

        // 创建并关闭一个 Socket
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        table.close(fd).unwrap();
        assert_eq!(table.count(), 0);

        // 重新创建应成功（槽位被复用）
        let fd2 = table.create(SocketDomain::Inet, SocketType::Datagram).unwrap();
        assert!(table.get(fd2).is_ok());
        assert_eq!(table.count(), 1);
    }

    /// 测试：查询不存在的 fd 应返回错误
    #[test]
    fn test_socket_table_query_nonexistent_fd() {
        let table = SocketTable::new();

        // 查询从未分配过的 fd
        let result = table.get(0);
        assert!(result.is_err());
        match result.unwrap_err() {
            NetError::InvalidSocket(fd) => assert_eq!(fd, 0),
            _ => panic!("期望 InvalidSocket 错误"),
        }

        // 查询 fd = 0
        let result2 = table.get(1);
        assert!(result2.is_err());
    }

    /// 测试：创建 Raw 类型 Socket 应失败
    #[test]
    fn test_socket_table_create_raw_unsupported() {
        let table = SocketTable::new();

        let result = table.create(SocketDomain::Inet, SocketType::Raw);
        assert!(result.is_err());
        match result.unwrap_err() {
            NetError::ProtocolError { reason } => {
                assert!(reason.contains("Raw"), "错误信息应包含 'Raw'");
            }
            _ => panic!("期望 ProtocolError 错误"),
        }
    }

    /// 测试：创建多种域的 Socket
    #[test]
    fn test_socket_table_multiple_domains() {
        let table = SocketTable::new();

        let fd_inet = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        let fd_inet6 = table.create(SocketDomain::Inet6, SocketType::Stream).unwrap();
        let fd_unix = table.create(SocketDomain::Unix, SocketType::Stream).unwrap();

        let entry_inet = table.get(fd_inet).unwrap();
        assert_eq!(entry_inet.domain, SocketDomain::Inet);

        let entry_inet6 = table.get(fd_inet6).unwrap();
        assert_eq!(entry_inet6.domain, SocketDomain::Inet6);

        let entry_unix = table.get(fd_unix).unwrap();
        assert_eq!(entry_unix.domain, SocketDomain::Unix);

        assert_eq!(table.count(), 3);
    }

    /// 测试：重复关闭同一个 fd 应第二次失败
    #[test]
    fn test_socket_table_double_close() {
        let table = SocketTable::new();
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();

        // 第一次关闭应成功
        assert!(table.close(fd).is_ok());
        // 第二次关闭应失败
        let result = table.close(fd);
        assert!(result.is_err());
        match result.unwrap_err() {
            NetError::InvalidSocket(id) => assert_eq!(id, fd as i32),
            _ => panic!("期望 InvalidSocket 错误"),
        }
    }

    /// 测试：fd 应严格递增
    #[test]
    fn test_socket_table_fd_increments() {
        let table = SocketTable::new();

        let fd0 = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        let fd1 = table.create(SocketDomain::Inet, SocketType::Datagram).unwrap();
        let fd2 = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();

        assert_eq!(fd0, 0);
        assert_eq!(fd1, 1);
        assert_eq!(fd2, 2);
    }

    /// 测试：获取协议 Socket 应成功
    #[test]
    fn test_socket_table_get_protocol() {
        let table = SocketTable::new();
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();

        let protocol = table.get_protocol(fd);
        assert!(protocol.is_ok());
        let guard = protocol.unwrap();
        assert!(guard.is_some(), "协议 Socket 应存在");
    }

    /// 测试：获取不存在 fd 的协议 Socket 应失败
    #[test]
    fn test_socket_table_get_protocol_invalid() {
        let table = SocketTable::new();

        let result = table.get_protocol(999);
        assert!(result.is_err());
        match result.err().unwrap() {
            NetError::InvalidSocket(fd) => assert_eq!(fd, 999),
            _ => panic!("期望 InvalidSocket 错误"),
        }
    }

    /// 测试：关闭后协议 Socket 也应被清除
    #[test]
    fn test_socket_table_close_clears_protocol() {
        let table = SocketTable::new();
        let fd = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();

        // 关闭前协议 Socket 存在
        {
            let protocol = table.get_protocol(fd).unwrap();
            assert!(protocol.is_some());
        } // 显式释放 MutexGuard

        // 关闭后
        table.close(fd).unwrap();
        let result = table.get_protocol(fd);
        assert!(result.is_err(), "关闭后获取协议 Socket 应失败");
    }

    /// 测试：创建和关闭交替操作
    #[test]
    fn test_socket_table_create_close_interleaved() {
        let table = SocketTable::new();

        let fd1 = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        let fd2 = table.create(SocketDomain::Inet, SocketType::Datagram).unwrap();
        assert_eq!(table.count(), 2);

        table.close(fd1).unwrap();
        assert_eq!(table.count(), 1);

        let fd3 = table.create(SocketDomain::Inet, SocketType::Stream).unwrap();
        assert_eq!(table.count(), 2);

        table.close(fd2).unwrap();
        table.close(fd3).unwrap();
        assert_eq!(table.count(), 0);
    }
}
