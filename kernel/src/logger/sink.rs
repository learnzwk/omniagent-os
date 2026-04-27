//! 日志输出目标
//!
//! 定义日志输出目标 trait 及其各种实现：环形缓冲区、VGA 文本模式、
//! 多目标组合和带过滤功能的 sink。

use alloc::string::String;
use alloc::vec::Vec;

use crate::logger::error::LoggerError;
use crate::logger::record::{LogRecord, LogLevel};

// ============================================================================
// LogSink Trait
// ============================================================================

/// 日志输出目标 trait
///
/// 所有日志输出目标都需要实现此 trait。
pub trait LogSink {
    /// 写入一条日志记录
    fn write(&self, record: &LogRecord) -> Result<(), LoggerError>;

    /// 刷新输出缓冲区
    fn flush(&self) -> Result<(), LoggerError>;
}

// ============================================================================
// RingBufferSink
// ============================================================================

/// 环形缓冲区日志输出
///
/// 使用固定大小的环形缓冲区存储日志记录，支持读取历史日志。
pub struct RingBufferSink {
    buffer: spin::Mutex<RingBufferInner>,
}

struct RingBufferInner {
    records: Vec<Option<LogRecord>>,
    write_pos: usize,
    count: usize,
    capacity: usize,
}

impl RingBufferSink {
    /// 创建新的环形缓冲区 sink
    pub fn new(capacity: usize) -> Self {
        let mut records = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            records.push(None);
        }
        RingBufferSink {
            buffer: spin::Mutex::new(RingBufferInner {
                records,
                write_pos: 0,
                count: 0,
                capacity,
            }),
        }
    }

    /// 创建默认大小（4096 条）的环形缓冲区 sink
    pub fn with_default_capacity() -> Self {
        Self::new(4096)
    }

    /// 读取所有存储的日志记录
    pub fn read_all(&self) -> Vec<LogRecord> {
        let inner = self.buffer.lock();
        let mut result = Vec::with_capacity(inner.count);
        if inner.count < inner.capacity {
            for i in 0..inner.count {
                if let Some(ref record) = inner.records[i] {
                    result.push(record.clone());
                }
            }
        } else {
            for i in 0..inner.capacity {
                let idx = (inner.write_pos + i) % inner.capacity;
                if let Some(ref record) = inner.records[idx] {
                    result.push(record.clone());
                }
            }
        }
        result
    }

    /// 读取指定级别的日志记录
    pub fn read_by_level(&self, level: LogLevel) -> Vec<LogRecord> {
        let all = self.read_all();
        all.into_iter().filter(|r| r.level == level).collect()
    }

    /// 获取当前存储的日志数量
    pub fn len(&self) -> usize {
        self.buffer.lock().count
    }

    /// 检查缓冲区是否为空
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().count == 0
    }

    /// 清空缓冲区
    pub fn clear(&self) {
        let mut inner = self.buffer.lock();
        for record in inner.records.iter_mut() {
            *record = None;
        }
        inner.write_pos = 0;
        inner.count = 0;
    }
}

impl LogSink for RingBufferSink {
    fn write(&self, record: &LogRecord) -> Result<(), LoggerError> {
        let mut inner = self.buffer.lock();
        let pos = inner.write_pos;
        inner.records[pos] = Some(record.clone());
        inner.write_pos = (inner.write_pos + 1) % inner.capacity;
        if inner.count < inner.capacity {
            inner.count += 1;
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), LoggerError> {
        // 环形缓冲区不需要刷新
        Ok(())
    }
}

// ============================================================================
// VgaSink
// ============================================================================

/// VGA 文本模式日志输出（模拟实现）
///
/// 在测试环境中将输出收集到 Vec 中，在真实内核中可替换为实际 VGA 输出。
pub struct VgaSink {
    output: spin::Mutex<Vec<String>>,
}

impl VgaSink {
    /// 创建新的 VGA sink
    pub fn new() -> Self {
        VgaSink {
            output: spin::Mutex::new(Vec::new()),
        }
    }

    /// 获取所有已输出的行
    pub fn get_output(&self) -> Vec<String> {
        self.output.lock().clone()
    }

    /// 获取输出行数
    pub fn len(&self) -> usize {
        self.output.lock().len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.output.lock().is_empty()
    }

    /// 清空输出
    pub fn clear(&self) {
        self.output.lock().clear();
    }
}

impl LogSink for VgaSink {
    fn write(&self, record: &LogRecord) -> Result<(), LoggerError> {
        let line = format!("{}", record);
        self.output.lock().push(line);
        Ok(())
    }

    fn flush(&self) -> Result<(), LoggerError> {
        // 模拟实现，不需要刷新
        Ok(())
    }
}

// ============================================================================
// MultiSink
// ============================================================================

/// 多目标组合 sink
///
/// 将日志同时输出到多个目标 sink。
pub struct MultiSink {
    sinks: spin::Mutex<Vec<Box<dyn LogSink + Send + Sync>>>,
}

impl MultiSink {
    /// 创建新的多目标 sink
    pub fn new() -> Self {
        MultiSink {
            sinks: spin::Mutex::new(Vec::new()),
        }
    }

    /// 添加一个 sink
    pub fn add_sink(&self, sink: Box<dyn LogSink + Send + Sync>) {
        self.sinks.lock().push(sink);
    }

    /// 获取 sink 数量
    pub fn sink_count(&self) -> usize {
        self.sinks.lock().len()
    }

    /// 移除所有 sink
    pub fn clear(&self) {
        self.sinks.lock().clear();
    }
}

impl LogSink for MultiSink {
    fn write(&self, record: &LogRecord) -> Result<(), LoggerError> {
        let sinks = self.sinks.lock();
        let mut last_error = None;
        for sink in sinks.iter() {
            if let Err(e) = sink.write(record) {
                last_error = Some(e);
            }
        }
        if let Some(e) = last_error {
            Err(e)
        } else {
            Ok(())
        }
    }

    fn flush(&self) -> Result<(), LoggerError> {
        let sinks = self.sinks.lock();
        let mut last_error = None;
        for sink in sinks.iter() {
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

// ============================================================================
// SinkFilter
// ============================================================================

/// 带日志级别过滤的 sink 包装器
///
/// 只允许指定级别及以上的日志通过。
pub struct SinkFilter<S: LogSink> {
    inner: S,
    min_level: LogLevel,
}

impl<S: LogSink> SinkFilter<S> {
    /// 创建新的过滤 sink
    pub fn new(inner: S, min_level: LogLevel) -> Self {
        SinkFilter { inner, min_level }
    }

    /// 获取当前最低日志级别
    pub fn min_level(&self) -> LogLevel {
        self.min_level
    }

    /// 设置最低日志级别
    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    /// 获取内部 sink 的引用
    pub fn inner(&self) -> &S {
        &self.inner
    }
}

impl<S: LogSink> LogSink for SinkFilter<S> {
    fn write(&self, record: &LogRecord) -> Result<(), LoggerError> {
        if record.level >= self.min_level {
            self.inner.write(record)
        } else {
            Ok(())
        }
    }

    fn flush(&self) -> Result<(), LoggerError> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- RingBufferSink tests ----

    #[test]
    fn test_ring_buffer_write_and_read() {
        let sink = RingBufferSink::new(10);
        let record = LogRecord::simple(LogLevel::Info, "test message");
        sink.write(&record).unwrap();
        let records = sink.read_all();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].message, "test message");
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let sink = RingBufferSink::new(3);
        for i in 0..5 {
            let record = LogRecord::simple(LogLevel::Info, &format!("msg {}", i));
            sink.write(&record).unwrap();
        }
        let records = sink.read_all();
        assert_eq!(records.len(), 3);
        // 最旧的记录应该被覆盖
        assert_eq!(records[0].message, "msg 2");
        assert_eq!(records[1].message, "msg 3");
        assert_eq!(records[2].message, "msg 4");
    }

    #[test]
    fn test_ring_buffer_read_by_level() {
        let sink = RingBufferSink::new(10);
        sink.write(&LogRecord::simple(LogLevel::Info, "info msg")).unwrap();
        sink.write(&LogRecord::simple(LogLevel::Error, "error msg")).unwrap();
        sink.write(&LogRecord::simple(LogLevel::Info, "info msg 2")).unwrap();
        let errors = sink.read_by_level(LogLevel::Error);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "error msg");
    }

    #[test]
    fn test_ring_buffer_len_and_empty() {
        let sink = RingBufferSink::new(10);
        assert!(sink.is_empty());
        assert_eq!(sink.len(), 0);
        sink.write(&LogRecord::simple(LogLevel::Info, "msg")).unwrap();
        assert!(!sink.is_empty());
        assert_eq!(sink.len(), 1);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let sink = RingBufferSink::new(10);
        sink.write(&LogRecord::simple(LogLevel::Info, "msg")).unwrap();
        sink.clear();
        assert!(sink.is_empty());
        assert_eq!(sink.len(), 0);
    }

    #[test]
    fn test_ring_buffer_flush() {
        let sink = RingBufferSink::new(10);
        assert!(sink.flush().is_ok());
    }

    #[test]
    fn test_ring_buffer_default_capacity() {
        let sink = RingBufferSink::with_default_capacity();
        assert_eq!(sink.len(), 0);
        // 写入一条记录应该成功
        sink.write(&LogRecord::simple(LogLevel::Trace, "first")).unwrap();
        assert_eq!(sink.len(), 1);
    }

    // ---- VgaSink tests ----

    #[test]
    fn test_vga_sink_write() {
        let sink = VgaSink::new();
        let record = LogRecord::simple(LogLevel::Info, "vga output");
        sink.write(&record).unwrap();
        let output = sink.get_output();
        assert_eq!(output.len(), 1);
        assert!(output[0].contains("vga output"));
    }

    #[test]
    fn test_vga_sink_multiple_writes() {
        let sink = VgaSink::new();
        sink.write(&LogRecord::simple(LogLevel::Info, "first")).unwrap();
        sink.write(&LogRecord::simple(LogLevel::Error, "second")).unwrap();
        assert_eq!(sink.len(), 2);
    }

    #[test]
    fn test_vga_sink_clear() {
        let sink = VgaSink::new();
        sink.write(&LogRecord::simple(LogLevel::Info, "msg")).unwrap();
        sink.clear();
        assert!(sink.is_empty());
    }

    #[test]
    fn test_vga_sink_flush() {
        let sink = VgaSink::new();
        assert!(sink.flush().is_ok());
    }

    // ---- MultiSink tests ----

    #[test]
    fn test_multi_sink_write_to_multiple() {
        let multi = MultiSink::new();
        let vga1 = VgaSink::new();
        let vga2 = VgaSink::new();
        multi.add_sink(Box::new(vga1));
        multi.add_sink(Box::new(vga2));
        assert_eq!(multi.sink_count(), 2);

        let record = LogRecord::simple(LogLevel::Info, "multi msg");
        multi.write(&record).unwrap();
        assert_eq!(multi.sink_count(), 2);
    }

    #[test]
    fn test_multi_sink_flush() {
        let multi = MultiSink::new();
        multi.add_sink(Box::new(VgaSink::new()));
        assert!(multi.flush().is_ok());
    }

    #[test]
    fn test_multi_sink_clear() {
        let multi = MultiSink::new();
        multi.add_sink(Box::new(VgaSink::new()));
        assert_eq!(multi.sink_count(), 1);
        multi.clear();
        assert_eq!(multi.sink_count(), 0);
    }

    // ---- SinkFilter tests ----

    #[test]
    fn test_sink_filter_passes_matching_level() {
        let vga = VgaSink::new();
        let filter = SinkFilter::new(vga, LogLevel::Warn);
        let record = LogRecord::simple(LogLevel::Error, "should pass");
        filter.write(&record).unwrap();
        assert_eq!(filter.inner().len(), 1);
    }

    #[test]
    fn test_sink_filter_blocks_lower_level() {
        let vga = VgaSink::new();
        let filter = SinkFilter::new(vga, LogLevel::Warn);
        let record = LogRecord::simple(LogLevel::Debug, "should be blocked");
        filter.write(&record).unwrap();
        assert!(filter.inner().is_empty());
    }

    #[test]
    fn test_sink_filter_exact_level() {
        let vga = VgaSink::new();
        let filter = SinkFilter::new(vga, LogLevel::Info);
        filter.write(&LogRecord::simple(LogLevel::Info, "exact")).unwrap();
        filter.write(&LogRecord::simple(LogLevel::Warn, "above")).unwrap();
        filter.write(&LogRecord::simple(LogLevel::Debug, "below")).unwrap();
        assert_eq!(filter.inner().len(), 2);
    }

    #[test]
    fn test_sink_filter_set_min_level() {
        let vga = VgaSink::new();
        let mut filter = SinkFilter::new(vga, LogLevel::Error);
        assert_eq!(filter.min_level(), LogLevel::Error);
        filter.set_min_level(LogLevel::Trace);
        assert_eq!(filter.min_level(), LogLevel::Trace);
        filter.write(&LogRecord::simple(LogLevel::Debug, "now passes")).unwrap();
        assert_eq!(filter.inner().len(), 1);
    }

    #[test]
    fn test_sink_filter_flush() {
        let vga = VgaSink::new();
        let filter = SinkFilter::new(vga, LogLevel::Info);
        assert!(filter.flush().is_ok());
    }
}
