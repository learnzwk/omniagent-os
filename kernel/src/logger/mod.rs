//! 系统日志服务模块
//!
//! 提供 OmniAgent OS 内核的日志记录功能，包括多级别日志、
//! 多输出目标、日志过滤和全局日志管理。

pub mod error;
pub mod sink;
pub mod record;
pub mod manager;

pub use manager::LogManager;
pub use record::{LogLevel, LogRecord};
pub use sink::{LogSink, RingBufferSink, VgaSink, MultiSink, SinkFilter};
