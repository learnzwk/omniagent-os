//! 日志错误类型
//!
//! 定义日志系统中所有可能的错误类型。

use core::fmt;

/// 日志错误枚举
#[derive(Debug, Clone)]
pub enum LoggerError {
    /// 缓冲区已满
    BufferFull,
    /// 无效的日志级别
    InvalidLevel(u8),
    /// Sink 未注册
    SinkNotRegistered(alloc::string::String),
    /// Sink 已存在
    SinkAlreadyExists(alloc::string::String),
    /// 写入失败
    WriteFailed(alloc::string::String),
    /// 刷新失败
    FlushFailed(alloc::string::String),
    /// 日志记录格式化错误
    FormatError(alloc::string::String),
}

impl fmt::Display for LoggerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoggerError::BufferFull => write!(f, "日志缓冲区已满"),
            LoggerError::InvalidLevel(level) => write!(f, "无效的日志级别: {}", level),
            LoggerError::SinkNotRegistered(name) => write!(f, "Sink 未注册: {}", name),
            LoggerError::SinkAlreadyExists(name) => write!(f, "Sink 已存在: {}", name),
            LoggerError::WriteFailed(reason) => write!(f, "写入失败: {}", reason),
            LoggerError::FlushFailed(reason) => write!(f, "刷新失败: {}", reason),
            LoggerError::FormatError(reason) => write!(f, "格式化错误: {}", reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_buffer_full_display() {
        let err = LoggerError::BufferFull;
        assert_eq!(err.to_string(), "日志缓冲区已满");
    }

    #[test]
    fn test_invalid_level_display() {
        let err = LoggerError::InvalidLevel(99);
        assert_eq!(err.to_string(), "无效的日志级别: 99");
    }

    #[test]
    fn test_sink_not_registered_display() {
        let err = LoggerError::SinkNotRegistered("vga".to_string());
        assert_eq!(err.to_string(), "Sink 未注册: vga");
    }

    #[test]
    fn test_sink_already_exists_display() {
        let err = LoggerError::SinkAlreadyExists("ring".to_string());
        assert_eq!(err.to_string(), "Sink 已存在: ring");
    }

    #[test]
    fn test_write_failed_display() {
        let err = LoggerError::WriteFailed("IO error".to_string());
        assert_eq!(err.to_string(), "写入失败: IO error");
    }

    #[test]
    fn test_flush_failed_display() {
        let err = LoggerError::FlushFailed("device busy".to_string());
        assert_eq!(err.to_string(), "刷新失败: device busy");
    }

    #[test]
    fn test_format_error_display() {
        let err = LoggerError::FormatError("bad format".to_string());
        assert_eq!(err.to_string(), "格式化错误: bad format");
    }

    #[test]
    fn test_error_clone() {
        let err = LoggerError::BufferFull;
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_error_debug() {
        let err = LoggerError::InvalidLevel(42);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidLevel"));
        assert!(debug_str.contains("42"));
    }
}
