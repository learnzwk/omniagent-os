//! 网络层错误类型定义

use core::fmt;

/// 网络层错误枚举
#[derive(Debug, Clone, PartialEq)]
pub enum NetError {
    /// 无效地址
    InvalidAddress,
    /// 地址已被占用
    AddressInUse,
    /// 连接被拒绝
    ConnectionRefused,
    /// 连接被重置
    ConnectionReset,
    /// 操作超时
    TimedOut,
    /// 操作会阻塞（非阻塞模式下）
    WouldBlock,
    /// Socket 已经连接
    AlreadyConnected,
    /// Socket 未连接
    NotConnected,
    /// 无效的 Socket 文件描述符
    InvalidSocket(i32),
    /// Socket 表已满
    SocketTableFull,
    /// 协议错误
    ProtocolError { reason: alloc::string::String },
    /// 缓冲区太小
    BufferTooSmall,
    /// 没有路由
    NoRoute,
    /// 主机不可达
    HostUnreachable,
    /// 网络已断开
    NetworkDown,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::InvalidAddress => write!(f, "无效地址"),
            NetError::AddressInUse => write!(f, "地址已被占用"),
            NetError::ConnectionRefused => write!(f, "连接被拒绝"),
            NetError::ConnectionReset => write!(f, "连接被重置"),
            NetError::TimedOut => write!(f, "操作超时"),
            NetError::WouldBlock => write!(f, "操作会阻塞"),
            NetError::AlreadyConnected => write!(f, "Socket 已经连接"),
            NetError::NotConnected => write!(f, "Socket 未连接"),
            NetError::InvalidSocket(fd) => write!(f, "无效的 Socket 文件描述符: {}", fd),
            NetError::SocketTableFull => write!(f, "Socket 表已满"),
            NetError::ProtocolError { reason } => write!(f, "协议错误: {}", reason),
            NetError::BufferTooSmall => write!(f, "缓冲区太小"),
            NetError::NoRoute => write!(f, "没有路由"),
            NetError::HostUnreachable => write!(f, "主机不可达"),
            NetError::NetworkDown => write!(f, "网络已断开"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = NetError::InvalidAddress;
        assert_eq!(format!("{}", err), "无效地址");

        let err = NetError::InvalidSocket(42);
        assert_eq!(format!("{}", err), "无效的 Socket 文件描述符: 42");

        let err = NetError::ProtocolError { reason: alloc::string::String::from("无效包头") };
        assert_eq!(format!("{}", err), "协议错误: 无效包头");
    }
}
