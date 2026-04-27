//! 分布式软总线错误类型
//! 模仿鸿蒙 DSoftBus 的错误处理设计

use core::fmt;

/// 软总线错误类型
#[derive(Debug, Clone)]
pub enum SoftBusError {
    /// 设备未找到
    DeviceNotFound(u64),
    /// 连接失败
    ConnectionFailed { reason: &'static str },
    /// 操作超时
    Timeout,
    /// 协议不支持
    ProtocolNotSupported,
    /// 缓冲区过小
    BufferTooSmall,
    /// 设备已连接
    AlreadyConnected(u64),
    /// 未连接
    NotConnected,
    /// 认证失败
    AuthenticationFailed,
    /// 无效消息
    InvalidMessage,
    /// 设备/资源忙
    Busy,
}

impl fmt::Display for SoftBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoftBusError::DeviceNotFound(id) => {
                write!(f, "设备未找到: {}", id)
            }
            SoftBusError::ConnectionFailed { reason } => {
                write!(f, "连接失败: {}", reason)
            }
            SoftBusError::Timeout => {
                write!(f, "操作超时")
            }
            SoftBusError::ProtocolNotSupported => {
                write!(f, "协议不支持")
            }
            SoftBusError::BufferTooSmall => {
                write!(f, "缓冲区过小")
            }
            SoftBusError::AlreadyConnected(id) => {
                write!(f, "设备已连接: {}", id)
            }
            SoftBusError::NotConnected => {
                write!(f, "未连接")
            }
            SoftBusError::AuthenticationFailed => {
                write!(f, "认证失败")
            }
            SoftBusError::InvalidMessage => {
                write!(f, "无效消息")
            }
            SoftBusError::Busy => {
                write!(f, "设备或资源忙")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SoftBusError::DeviceNotFound(42);
        assert_eq!(format!("{}", err), "设备未找到: 42");

        let err = SoftBusError::ConnectionFailed { reason: "网络不可达" };
        assert_eq!(format!("{}", err), "连接失败: 网络不可达");

        let err = SoftBusError::Timeout;
        assert_eq!(format!("{}", err), "操作超时");

        let err = SoftBusError::ProtocolNotSupported;
        assert_eq!(format!("{}", err), "协议不支持");

        let err = SoftBusError::BufferTooSmall;
        assert_eq!(format!("{}", err), "缓冲区过小");

        let err = SoftBusError::AlreadyConnected(100);
        assert_eq!(format!("{}", err), "设备已连接: 100");

        let err = SoftBusError::NotConnected;
        assert_eq!(format!("{}", err), "未连接");

        let err = SoftBusError::AuthenticationFailed;
        assert_eq!(format!("{}", err), "认证失败");

        let err = SoftBusError::InvalidMessage;
        assert_eq!(format!("{}", err), "无效消息");

        let err = SoftBusError::Busy;
        assert_eq!(format!("{}", err), "设备或资源忙");
    }

    #[test]
    fn test_error_clone() {
        let err = SoftBusError::DeviceNotFound(1);
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));
    }
}
