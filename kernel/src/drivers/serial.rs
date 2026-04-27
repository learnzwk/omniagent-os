//! COM1 串口驱动 + log trait 实现

use core::fmt;
use log::{Level, LevelFilter, Metadata, Record};
use spin::Mutex;

/// COM1 端口基地址
const COM1: u16 = 0x3F8;

/// 串口寄存器偏移
const DATA_REG: u16 = 0;
const INT_ENABLE_REG: u16 = 1;
const FIFO_CTRL_REG: u16 = 2;
const LINE_CTRL_REG: u16 = 3;
const MODEM_CTRL_REG: u16 = 4;
const LINE_STATUS_REG: u16 = 5;

/// 串口驱动
pub struct SerialPort {
    base: u16,
}

impl SerialPort {
    pub const fn new(base: u16) -> Self {
        Self { base }
    }

    /// 初始化串口: 115200 波特率, 8N1, 无中断
    pub fn init(&mut self) {
        unsafe {
            // 禁用所有中断
            self.outb(INT_ENABLE_REG, 0x00);
            // 启用 DLAB (设置波特率)
            self.outb(LINE_CTRL_REG, 0x80);
            // 设置除数: 115200 baud (115200 / 1 = 115200, divisor = 1)
            self.outb(DATA_REG, 0x01); // 低字节
            self.outb(INT_ENABLE_REG, 0x00); // 高字节
            // 8 bits, no parity, one stop bit (8N1)
            self.outb(LINE_CTRL_REG, 0x03);
            // 启用 FIFO, 清空, 14 字节阈值
            self.outb(FIFO_CTRL_REG, 0xC7);
            // IRQs enabled, RTS/DSR set
            self.outb(MODEM_CTRL_REG, 0x0B);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            // 等待发送缓冲区为空
            while self.inb(LINE_STATUS_REG) & 0x20 == 0 {}
            self.outb(DATA_REG, byte);
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    unsafe fn outb(&self, offset: u16, value: u8) {
        let port = self.base + offset;
        core::arch::asm!("outb %al, %dx", in("al") value, in("dx") port, options(nostack, nomem, preserves_flags));
    }

    unsafe fn inb(&self, offset: u16) -> u8 {
        let port = self.base + offset;
        let result: u8;
        core::arch::asm!("inb %dx, %al", out("al") result, in("dx") port, options(nostack, nomem, preserves_flags));
        result
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// 全局串口实例
pub static SERIAL: Mutex<SerialPort> = Mutex::new(SerialPort::new(COM1));

/// 初始化全局串口
pub fn init_serial() {
    SERIAL.lock().init();
}

/// 串口日志记录器
pub struct SerialLogger;

impl SerialLogger {
    pub const fn new() -> Self { Self }
}

impl log::Log for SerialLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let color = match record.level() {
            Level::Error => "\x1b[31m",
            Level::Warn => "\x1b[33m",
            Level::Info => "\x1b[32m",
            Level::Debug => "\x1b[34m",
            Level::Trace => "\x1b[90m",
        };
        let reset = "\x1b[0m";
        use core::fmt::Write;
        let mut serial = SERIAL.lock();
        let _ = serial.write_str(color);
        let _ = write!(serial, "[{:>5}] ", record.level());
        let _ = write!(serial, "{}", record.args());
        let _ = serial.write_str(reset);
        let _ = serial.write_str("\n");
    }

    fn flush(&self) {}
}

/// 全局日志记录器
pub static LOGGER: SerialLogger = SerialLogger::new();

/// 初始化日志系统
pub fn init_logger() {
    log::set_logger(&LOGGER).expect("logger already set");
    log::set_max_level(LevelFilter::Info);
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;

    #[test]
    fn test_serial_port_new() {
        let port = SerialPort::new(COM1);
        assert_eq!(port.base, COM1);
    }

    #[test]
    fn test_serial_port_const() {
        const PORT: SerialPort = SerialPort::new(COM1);
        assert_eq!(PORT.base, COM1);
    }

    #[test]
    fn test_logger_enabled_info() {
        let logger = SerialLogger::new();
        let meta = log::Metadata::builder()
            .target("test_module")
            .level(Level::Info)
            .build();
        assert!(logger.enabled(&meta));
    }

    #[test]
    fn test_logger_disabled_trace() {
        let logger = SerialLogger::new();
        let meta = log::Metadata::builder()
            .target("test_module")
            .level(Level::Trace)
            .build();
        assert!(!logger.enabled(&meta));
    }

    #[test]
    fn test_logger_enabled_error() {
        let logger = SerialLogger::new();
        let meta = log::Metadata::builder()
            .target("test_module")
            .level(Level::Error)
            .build();
        assert!(logger.enabled(&meta));
    }

    #[test]
    fn test_com1_address() {
        assert_eq!(COM1, 0x3F8);
    }
}
