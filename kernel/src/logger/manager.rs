//! 日志管理器
//!
//! 提供全局日志管理功能，包括 sink 注册/注销、全局日志级别控制、
//! 按模块过滤和日志统计。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use crate::logger::error::LoggerError;
use crate::logger::record::{LogRecord, LogLevel};
use crate::logger::sink::{LogSink, RingBufferSink};

// ============================================================================
// 日志统计
// ============================================================================

/// 日志统计信息
#[derive(Debug, Clone)]
pub struct LogStats {
    pub trace_count: u64,
    pub debug_count: u64,
    pub info_count: u64,
    pub warn_count: u64,
    pub error_count: u64,
    pub fatal_count: u64,
    pub total_count: u64,
    pub dropped_count: u64,
}

impl LogStats {
    pub fn new() -> Self {
        LogStats {
            trace_count: 0,
            debug_count: 0,
            info_count: 0,
            warn_count: 0,
            error_count: 0,
            fatal_count: 0,
            total_count: 0,
            dropped_count: 0,
        }
    }
}

impl Default for LogStats {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// LogManager
// ============================================================================

/// 日志管理器
///
/// 管理所有日志 sink、全局日志级别和日志统计。
pub struct LogManager {
    /// 已注册的 sink
    sinks: BTreeMap<String, Box<dyn LogSink + Send + Sync>>,
    /// 全局最低日志级别
    global_level: Mutex<LogLevel>,
    /// 模块级别覆盖
    module_levels: Mutex<BTreeMap<String, LogLevel>>,
    /// 日志统计
    stats: Mutex<LogStats>,
    /// 日志序列号
    sequence: AtomicU64,
    /// 内置环形缓冲区
    ring_buffer: RingBufferSink,
}

impl LogManager {
    /// 创建新的日志管理器
    pub fn new() -> Self {
        LogManager {
            sinks: BTreeMap::new(),
            global_level: Mutex::new(LogLevel::Trace),
            module_levels: Mutex::new(BTreeMap::new()),
            stats: Mutex::new(LogStats::new()),
            sequence: AtomicU64::new(0),
            ring_buffer: RingBufferSink::with_default_capacity(),
        }
    }

    /// 注册一个 sink
    pub fn register_sink(
        &mut self,
        name: &str,
        sink: Box<dyn LogSink + Send + Sync>,
    ) -> Result<(), LoggerError> {
        if self.sinks.contains_key(name) {
            return Err(LoggerError::SinkAlreadyExists(name.to_string()));
        }
        self.sinks.insert(name.to_string(), sink);
        Ok(())
    }

    /// 注销一个 sink
    pub fn unregister_sink(&mut self, name: &str) -> Result<(), LoggerError> {
        if self.sinks.remove(name).is_none() {
            return Err(LoggerError::SinkNotRegistered(name.to_string()));
        }
        Ok(())
    }

    /// 设置全局日志级别
    pub fn set_global_level(&self, level: LogLevel) {
        *self.global_level.lock() = level;
    }

    /// 获取全局日志级别
    pub fn global_level(&self) -> LogLevel {
        *self.global_level.lock()
    }

    /// 设置模块日志级别
    pub fn set_module_level(&self, module: &str, level: LogLevel) {
        self.module_levels.lock().insert(module.to_string(), level);
    }

    /// 获取模块日志级别
    pub fn get_module_level(&self, module: &str) -> Option<LogLevel> {
        self.module_levels.lock().get(module).copied()
    }

    /// 移除模块日志级别设置
    pub fn remove_module_level(&self, module: &str) -> bool {
        self.module_levels.lock().remove(module).is_some()
    }

    /// 获取已注册的 sink 数量
    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }

    /// 检查 sink 是否已注册
    pub fn has_sink(&self, name: &str) -> bool {
        self.sinks.contains_key(name)
    }

    /// 记录一条日志
    pub fn log_record(&self, mut record: LogRecord) -> Result<(), LoggerError> {
        // 检查日志级别
        let global_level = *self.global_level.lock();
        if record.level < global_level {
            self.stats.lock().dropped_count += 1;
            return Ok(());
        }

        // 检查模块级别
        if !record.module_path.is_empty() {
            if let Some(module_level) = self.get_module_level(&record.module_path) {
                if record.level < module_level {
                    self.stats.lock().dropped_count += 1;
                    return Ok(());
                }
            }
        }

        // 分配序列号作为时间戳（如果没有设置）
        if record.timestamp == 0 {
            record.timestamp = self.sequence.fetch_add(1, Ordering::SeqCst);
        }

        // 更新统计
        {
            let mut stats = self.stats.lock();
            match record.level {
                LogLevel::Trace => stats.trace_count += 1,
                LogLevel::Debug => stats.debug_count += 1,
                LogLevel::Info => stats.info_count += 1,
                LogLevel::Warn => stats.warn_count += 1,
                LogLevel::Error => stats.error_count += 1,
                LogLevel::Fatal => stats.fatal_count += 1,
            }
            stats.total_count += 1;
        }

        // 写入环形缓冲区
        let _ = self.ring_buffer.write(&record);

        // 写入所有注册的 sink
        let mut last_error = None;
        for sink in self.sinks.values() {
            if let Err(e) = sink.write(&record) {
                last_error = Some(e);
            }
        }

        if let Some(e) = last_error {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// 获取日志统计信息
    pub fn stats(&self) -> LogStats {
        self.stats.lock().clone()
    }

    /// 重置日志统计
    pub fn reset_stats(&self) {
        *self.stats.lock() = LogStats::new();
    }

    /// 从环形缓冲区读取所有日志
    pub fn read_history(&self) -> Vec<LogRecord> {
        self.ring_buffer.read_all()
    }

    /// 刷新所有 sink
    pub fn flush_all(&self) -> Result<(), LoggerError> {
        let mut last_error = None;
        for sink in self.sinks.values() {
            if let Err(e) = sink.flush() {
                last_error = Some(e);
            }
        }
        if let Some(e) = last_error {
            Err(e)
        } else {
            Ok(())
        }
    }
}

impl Default for LogManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 全局日志管理器
// ============================================================================

/// 全局日志管理器实例
pub static LOG_MANAGER: spin::Lazy<Mutex<LogManager>> =
    spin::Lazy::new(|| Mutex::new(LogManager::new()));

/// 记录一条日志的便捷函数
pub fn log_record(record: LogRecord) -> Result<(), LoggerError> {
    LOG_MANAGER.lock().log_record(record)
}

/// 设置全局日志级别的便捷函数
pub fn set_global_level(level: LogLevel) {
    LOG_MANAGER.lock().set_global_level(level);
}

/// 获取全局日志级别的便捷函数
pub fn get_global_level() -> LogLevel {
    LOG_MANAGER.lock().global_level()
}

/// 获取日志统计的便捷函数
pub fn get_log_stats() -> LogStats {
    LOG_MANAGER.lock().stats()
}

/// 重置日志统计的便捷函数
pub fn reset_log_stats() {
    LOG_MANAGER.lock().reset_stats();
}

/// 读取日志历史的便捷函数
pub fn read_log_history() -> Vec<LogRecord> {
    LOG_MANAGER.lock().read_history()
}

/// 刷新所有 sink 的便捷函数
pub fn flush_all_sinks() -> Result<(), LoggerError> {
    LOG_MANAGER.lock().flush_all()
}

/// 注册 sink 的便捷函数
pub fn register_sink(name: &str, sink: Box<dyn LogSink + Send + Sync>) -> Result<(), LoggerError> {
    LOG_MANAGER.lock().register_sink(name, sink)
}

/// 注销 sink 的便捷函数
pub fn unregister_sink(name: &str) -> Result<(), LoggerError> {
    LOG_MANAGER.lock().unregister_sink(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::sink::VgaSink;

    fn reset_global_manager() {
        // 替换全局管理器以获得干净状态
        let mut mgr = LOG_MANAGER.lock();
        *mgr = LogManager::new();
    }

    #[test]
    fn test_log_manager_new() {
        let mgr = LogManager::new();
        assert_eq!(mgr.sink_count(), 0);
        assert_eq!(mgr.global_level(), LogLevel::Trace);
    }

    #[test]
    fn test_register_and_unregister_sink() {
        let mut mgr = LogManager::new();
        let vga = Box::new(VgaSink::new());
        assert!(mgr.register_sink("vga", vga).is_ok());
        assert_eq!(mgr.sink_count(), 1);
        assert!(mgr.has_sink("vga"));
        assert!(mgr.unregister_sink("vga").is_ok());
        assert_eq!(mgr.sink_count(), 0);
    }

    #[test]
    fn test_register_duplicate_sink() {
        let mut mgr = LogManager::new();
        mgr.register_sink("vga", Box::new(VgaSink::new())).unwrap();
        let result = mgr.register_sink("vga", Box::new(VgaSink::new()));
        assert!(result.is_err());
        match result.unwrap_err() {
            LoggerError::SinkAlreadyExists(name) => assert_eq!(name, "vga"),
            _ => panic!("expected SinkAlreadyExists error"),
        }
    }

    #[test]
    fn test_unregister_nonexistent_sink() {
        let mut mgr = LogManager::new();
        let result = mgr.unregister_sink("nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            LoggerError::SinkNotRegistered(name) => assert_eq!(name, "nonexistent"),
            _ => panic!("expected SinkNotRegistered error"),
        }
    }

    #[test]
    fn test_set_and_get_global_level() {
        let mgr = LogManager::new();
        mgr.set_global_level(LogLevel::Warn);
        assert_eq!(mgr.global_level(), LogLevel::Warn);
        mgr.set_global_level(LogLevel::Error);
        assert_eq!(mgr.global_level(), LogLevel::Error);
    }

    #[test]
    fn test_module_level() {
        let mgr = LogManager::new();
        assert!(mgr.get_module_level("net").is_none());
        mgr.set_module_level("net", LogLevel::Error);
        assert_eq!(mgr.get_module_level("net"), Some(LogLevel::Error));
        assert!(mgr.remove_module_level("net"));
        assert!(mgr.get_module_level("net").is_none());
    }

    #[test]
    fn test_log_record_passes() {
        let mut mgr = LogManager::new();
        mgr.register_sink("vga", Box::new(VgaSink::new())).unwrap();
        let record = LogRecord::simple(LogLevel::Info, "test log");
        assert!(mgr.log_record(record).is_ok());
        let stats = mgr.stats();
        assert_eq!(stats.info_count, 1);
        assert_eq!(stats.total_count, 1);
    }

    #[test]
    fn test_log_record_filtered_by_global_level() {
        let mgr = LogManager::new();
        mgr.set_global_level(LogLevel::Warn);
        let record = LogRecord::simple(LogLevel::Debug, "should be filtered");
        assert!(mgr.log_record(record).is_ok());
        let stats = mgr.stats();
        assert_eq!(stats.total_count, 0);
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_log_record_filtered_by_module_level() {
        let mut mgr = LogManager::new();
        mgr.register_sink("vga", Box::new(VgaSink::new())).unwrap();
        mgr.set_module_level("net", LogLevel::Error);
        let record = LogRecord::new(
            0,
            LogLevel::Info,
            "net".to_string(),
            "should be filtered".to_string(),
            "net".to_string(),
            1,
            "net.rs".to_string(),
        );
        assert!(mgr.log_record(record).is_ok());
        let stats = mgr.stats();
        assert_eq!(stats.total_count, 0);
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_log_stats() {
        let mut mgr = LogManager::new();
        mgr.register_sink("vga", Box::new(VgaSink::new())).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Trace, "t")).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Debug, "d")).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Info, "i")).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Warn, "w")).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Error, "e")).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Fatal, "f")).unwrap();

        let stats = mgr.stats();
        assert_eq!(stats.trace_count, 1);
        assert_eq!(stats.debug_count, 1);
        assert_eq!(stats.info_count, 1);
        assert_eq!(stats.warn_count, 1);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.fatal_count, 1);
        assert_eq!(stats.total_count, 6);
    }

    #[test]
    fn test_reset_stats() {
        let mut mgr = LogManager::new();
        mgr.register_sink("vga", Box::new(VgaSink::new())).unwrap();
        mgr.log_record(LogRecord::simple(LogLevel::Info, "msg")).unwrap();
        assert_eq!(mgr.stats().total_count, 1);
        mgr.reset_stats();
        assert_eq!(mgr.stats().total_count, 0);
    }

    #[test]
    fn test_read_history() {
        let mgr = LogManager::new();
        mgr.log_record(LogRecord::simple(LogLevel::Info, "history msg")).unwrap();
        let history = mgr.read_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "history msg");
    }

    #[test]
    fn test_flush_all() {
        let mut mgr = LogManager::new();
        mgr.register_sink("vga", Box::new(VgaSink::new())).unwrap();
        assert!(mgr.flush_all().is_ok());
    }

    #[test]
    fn test_default() {
        let mgr = LogManager::default();
        assert_eq!(mgr.global_level(), LogLevel::Trace);
    }

    #[test]
    fn test_log_stats_default() {
        let stats = LogStats::new();
        assert_eq!(stats.total_count, 0);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_global_convenience_functions() {
        reset_global_manager();
        set_global_level(LogLevel::Error);
        assert_eq!(get_global_level(), LogLevel::Error);

        let record = LogRecord::simple(LogLevel::Info, "filtered");
        assert!(log_record(record).is_ok());

        let stats = get_log_stats();
        assert_eq!(stats.dropped_count, 1);

        reset_log_stats();
        let stats = get_log_stats();
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_global_register_sink() {
        reset_global_manager();
        assert!(register_sink("test_vga", Box::new(VgaSink::new())).is_ok());
        assert!(flush_all_sinks().is_ok());
        assert!(unregister_sink("test_vga").is_ok());
    }

    #[test]
    fn test_global_read_history() {
        reset_global_manager();
        log_record(LogRecord::simple(LogLevel::Info, "global history")).unwrap();
        let history = read_log_history();
        assert_eq!(history.len(), 1);
    }
}
