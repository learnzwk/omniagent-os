//! 系统定时器
//!
//! 使用 PIT (8254) 作为初始定时器，后续切换到 APIC Timer

#[cfg(not(test))]
use alloc::format;

use crate::arch::x86_64::port_io::outb;
use crate::drivers::serial::SERIAL;
use core::sync::atomic::{AtomicU64, Ordering};

/// PIT 端口
const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

/// 系统滴答计数
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 定时器频率 (Hz)
const TIMER_FREQUENCY_HZ: u32 = 100;

/// 获取当前滴答计数
pub fn tick_count() -> u64 {
    TICK_COUNT.load(Ordering::Relaxed)
}

/// 增加滴答计数
pub fn increment_tick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// 获取运行时间 (毫秒)
pub fn uptime_millis() -> u64 {
    tick_count() * 1000 / TIMER_FREQUENCY_HZ as u64
}

/// 初始化 PIT 定时器
pub unsafe fn init_pit_timer() {
    let divisor: u16 = (1193182 / TIMER_FREQUENCY_HZ as u32) as u16;
    // Channel 0, Rate Generator (mode 2), lobyte/hibyte
    outb(PIT_COMMAND, 0x36);
    outb(PIT_CHANNEL0, divisor as u8);
    outb(PIT_CHANNEL0, (divisor >> 8) as u8);
    SERIAL.lock().write_str(&format!(
        "[TIMER] PIT initialized at {}Hz\n", TIMER_FREQUENCY_HZ
    ));
}

/// 定时器中断处理函数
pub extern "C" fn timer_interrupt_handler() {
    increment_tick();
    unsafe { crate::arch::x86_64::apic::send_eoi(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_count_initial() {
        assert_eq!(tick_count(), 0);
    }

    #[test]
    fn test_increment_tick() {
        TICK_COUNT.store(0, Ordering::Relaxed);
        increment_tick();
        assert_eq!(tick_count(), 1);
        increment_tick();
        increment_tick();
        assert_eq!(tick_count(), 3);
        TICK_COUNT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_uptime_millis() {
        TICK_COUNT.store(100, Ordering::Relaxed);
        let millis = uptime_millis();
        assert_eq!(millis, 100 * 1000 / TIMER_FREQUENCY_HZ as u64);
        TICK_COUNT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_timer_frequency() {
        assert_eq!(TIMER_FREQUENCY_HZ, 100);
    }

    #[test]
    fn test_pit_divisor() {
        let divisor = (1193182 / TIMER_FREQUENCY_HZ as u32) as u16;
        assert!(divisor > 0);
        // At 100Hz, divisor should be ~11931
        assert!(divisor > 10000 && divisor < 12000);
    }

    #[test]
    fn test_pit_port() {
        assert_eq!(PIT_CHANNEL0, 0x40);
        assert_eq!(PIT_COMMAND, 0x43);
    }
}
