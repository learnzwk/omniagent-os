//! 零拷贝 IPC 错误类型
//! 模仿鸿蒙零拷贝 IPC 的错误处理设计

use core::fmt;

/// IPC 错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum IpcError {
    /// 内存不足
    OutOfMemory,
    /// 无效句柄
    InvalidHandle(u64),
    /// 通道未找到
    ChannelNotFound(u64),
    /// 通道已满
    ChannelFull,
    /// 通道已关闭
    ChannelClosed,
    /// 权限不足
    PermissionDenied,
    /// 无效大小
    InvalidSize(usize),
    /// 已存在
    AlreadyExists(u64),
    /// 操作超时
    Timeout,
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcError::OutOfMemory => {
                write!(f, "内存不足")
            }
            IpcError::InvalidHandle(id) => {
                write!(f, "无效句柄: {}", id)
            }
            IpcError::ChannelNotFound(id) => {
                write!(f, "通道未找到: {}", id)
            }
            IpcError::ChannelFull => {
                write!(f, "通道已满")
            }
            IpcError::ChannelClosed => {
                write!(f, "通道已关闭")
            }
            IpcError::PermissionDenied => {
                write!(f, "权限不足")
            }
            IpcError::InvalidSize(size) => {
                write!(f, "无效大小: {}", size)
            }
            IpcError::AlreadyExists(id) => {
                write!(f, "已存在: {}", id)
            }
            IpcError::Timeout => {
                write!(f, "操作超时")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = IpcError::OutOfMemory;
        assert_eq!(format!("{}", err), "内存不足");

        let err = IpcError::InvalidHandle(42);
        assert_eq!(format!("{}", err), "无效句柄: 42");

        let err = IpcError::ChannelNotFound(100);
        assert_eq!(format!("{}", err), "通道未找到: 100");

        let err = IpcError::ChannelFull;
        assert_eq!(format!("{}", err), "通道已满");

        let err = IpcError::ChannelClosed;
        assert_eq!(format!("{}", err), "通道已关闭");

        let err = IpcError::PermissionDenied;
        assert_eq!(format!("{}", err), "权限不足");

        let err = IpcError::InvalidSize(1024);
        assert_eq!(format!("{}", err), "无效大小: 1024");

        let err = IpcError::AlreadyExists(50);
        assert_eq!(format!("{}", err), "已存在: 50");

        let err = IpcError::Timeout;
        assert_eq!(format!("{}", err), "操作超时");
    }

    #[test]
    fn test_error_clone() {
        let err = IpcError::ChannelNotFound(1);
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));
    }
}
