//! 日志记录结构
//!
//! 定义日志级别和日志记录数据结构。

use alloc::format;
use alloc::string::String;
use core::fmt;

// ============================================================================
// 日志级别
// ============================================================================

/// 日志级别枚举
///
/// 从最低到最高：Trace < Debug < Info < Warn < Error < Fatal
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// 追踪级别 - 最详细的日志信息
    Trace = 0,
    /// 调试级别 - 调试信息
    Debug = 1,
    /// 信息级别 - 一般信息
    Info = 2,
    /// 警告级别 - 潜在问题
    Warn = 3,
    /// 错误级别 - 错误但不致命
    Error = 4,
    /// 致命级别 - 严重错误，系统可能无法继续
    Fatal = 5,
}

impl LogLevel {
    /// 从 u8 值创建日志级别
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(LogLevel::Trace),
            1 => Some(LogLevel::Debug),
            2 => Some(LogLevel::Info),
            3 => Some(LogLevel::Warn),
            4 => Some(LogLevel::Error),
            5 => Some(LogLevel::Fatal),
            _ => None,
        }
    }

    /// 返回日志级别的数值表示
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// 返回日志级别的短标签
    pub fn short_tag(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRC",
            LogLevel::Debug => "DBG",
            LogLevel::Info => "INF",
            LogLevel::Warn => "WRN",
            LogLevel::Error => "ERR",
            LogLevel::Fatal => "FTL",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

// ============================================================================
// 日志记录
// ============================================================================

/// 日志记录结构
///
/// 包含一条日志的所有信息：时间戳、级别、目标、消息等。
#[derive(Debug, Clone)]
pub struct LogRecord {
    /// 时间戳（毫秒）
    pub timestamp: u64,
    /// 日志级别
    pub level: LogLevel,
    /// 日志目标（模块名）
    pub target: String,
    /// 日志消息
    pub message: String,
    /// 源代码模块路径
    pub module_path: String,
    /// 源代码行号
    pub line: u32,
    /// 源代码文件名
    pub file: String,
}

impl LogRecord {
    /// 创建新的日志记录
    pub fn new(
        timestamp: u64,
        level: LogLevel,
        target: String,
        message: String,
        module_path: String,
        line: u32,
        file: String,
    ) -> Self {
        LogRecord {
            timestamp,
            level,
            target,
            message,
            module_path,
            line,
            file,
        }
    }

    /// 创建一个简单的日志记录（仅级别和消息）
    pub fn simple(level: LogLevel, message: &str) -> Self {
        LogRecord {
            timestamp: 0,
            level,
            target: String::new(),
            message: String::from(message),
            module_path: String::new(),
            line: 0,
            file: String::new(),
        }
    }

    /// 格式化为标准日志字符串
    pub fn format_record(&self) -> String {
        format!(
            "[{}] [{}] [{}] {} ({}:{})",
            self.timestamp, self.level, self.target, self.message, self.file, self.line
        )
    }
}

impl fmt::Display for LogRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] [{}] [{}] {} ({}:{})",
            self.timestamp, self.level, self.target, self.message, self.file, self.line
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Fatal);
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Error);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(format!("{}", LogLevel::Trace), "TRACE");
        assert_eq!(format!("{}", LogLevel::Debug), "DEBUG");
        assert_eq!(format!("{}", LogLevel::Info), "INFO");
        assert_eq!(format!("{}", LogLevel::Warn), "WARN");
        assert_eq!(format!("{}", LogLevel::Error), "ERROR");
        assert_eq!(format!("{}", LogLevel::Fatal), "FATAL");
    }

    #[test]
    fn test_log_level_from_u8() {
        assert_eq!(LogLevel::from_u8(0), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_u8(1), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_u8(2), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_u8(3), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_u8(4), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_u8(5), Some(LogLevel::Fatal));
        assert_eq!(LogLevel::from_u8(6), None);
        assert_eq!(LogLevel::from_u8(255), None);
    }

    #[test]
    fn test_log_level_as_u8() {
        assert_eq!(LogLevel::Trace.as_u8(), 0);
        assert_eq!(LogLevel::Fatal.as_u8(), 5);
    }

    #[test]
    fn test_log_level_short_tag() {
        assert_eq!(LogLevel::Trace.short_tag(), "TRC");
        assert_eq!(LogLevel::Debug.short_tag(), "DBG");
        assert_eq!(LogLevel::Info.short_tag(), "INF");
        assert_eq!(LogLevel::Warn.short_tag(), "WRN");
        assert_eq!(LogLevel::Error.short_tag(), "ERR");
        assert_eq!(LogLevel::Fatal.short_tag(), "FTL");
    }

    #[test]
    fn test_log_record_new() {
        let record = LogRecord::new(
            1000,
            LogLevel::Info,
            "test_module".to_string(),
            "hello world".to_string(),
            "my::module".to_string(),
            42,
            "main.rs".to_string(),
        );
        assert_eq!(record.timestamp, 1000);
        assert_eq!(record.level, LogLevel::Info);
        assert_eq!(record.target, "test_module");
        assert_eq!(record.message, "hello world");
        assert_eq!(record.module_path, "my::module");
        assert_eq!(record.line, 42);
        assert_eq!(record.file, "main.rs");
    }

    #[test]
    fn test_log_record_simple() {
        let record = LogRecord::simple(LogLevel::Warn, "simple message");
        assert_eq!(record.level, LogLevel::Warn);
        assert_eq!(record.message, "simple message");
        assert_eq!(record.timestamp, 0);
        assert_eq!(record.target, "");
    }

    #[test]
    fn test_log_record_display() {
        let record = LogRecord::new(
            500,
            LogLevel::Error,
            "net".to_string(),
            "connection failed".to_string(),
            "net::tcp".to_string(),
            10,
            "tcp.rs".to_string(),
        );
        let s = format!("{}", record);
        assert!(s.contains("[500]"));
        assert!(s.contains("[ERROR]"));
        assert!(s.contains("[net]"));
        assert!(s.contains("connection failed"));
        assert!(s.contains("tcp.rs:10"));
    }

    #[test]
    fn test_log_record_format_record() {
        let record = LogRecord::new(
            100,
            LogLevel::Debug,
            "scheduler".to_string(),
            "task switched".to_string(),
            "kernel::sched".to_string(),
            5,
            "sched.rs".to_string(),
        );
        let s = record.format_record();
        assert!(s.contains("[100]"));
        assert!(s.contains("[DEBUG]"));
    }

    #[test]
    fn test_log_record_clone() {
        let record = LogRecord::simple(LogLevel::Info, "clone me");
        let cloned = record.clone();
        assert_eq!(record.message, cloned.message);
        assert_eq!(record.level, cloned.level);
    }
}
