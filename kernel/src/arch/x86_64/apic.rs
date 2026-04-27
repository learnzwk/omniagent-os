//! Local APIC 驱动

use crate::drivers::serial::SERIAL;

/// Local APIC MSR 寄存器
const APIC_MSR: u32 = 0x1B;

/// Local APIC MMIO 偏移
const APIC_ID: usize = 0x020;
const APIC_VERSION: usize = 0x030;
const APIC_EOI: usize = 0x0B0;
const APIC_SVR: usize = 0x0F0;
const APIC_TIMER_LVT: usize = 0x320;
const APIC_TIMER_INITIAL: usize = 0x380;
const APIC_TIMER_CURRENT: usize = 0x390;
const APIC_TIMER_DIV: usize = 0x3E0;

/// Local APIC 基地址 (默认)
const APIC_DEFAULT_BASE: u64 = 0xFEE00000;

/// Spurious Interrupt Vector
const SPURIOUS_VECTOR: u8 = 0xFF;

/// 全局 APIC 基地址
static mut APIC_BASE: u64 = APIC_DEFAULT_BASE;

/// 初始化 Local APIC
pub unsafe fn init_local_apic() {
    // 读取 MSR 获取 APIC 基地址
    let msr_value: u64;
    core::arch::asm!("rdmsr", out("eax") msr_value, options(nomem, nostack, preserves_flags));
    let apic_enabled = (msr_value & (1 << 11)) != 0;
    let base = msr_value & 0xFFFF_F000;
    APIC_BASE = base;

    if !apic_enabled {
        // 启用 APIC
        let new_msr = msr_value | (1 << 11);
        core::arch::asm!("wrmsr", in("eax") new_msr, options(nomem, nostack, preserves_flags));
    }

    // 设置 Spurious Interrupt Vector
    write_apic(APIC_SVR, 0x100 | SPURIOUS_VECTOR as u32);

    SERIAL.lock().write_str("[APIC] Local APIC initialized\n");
}

/// 发送 EOI (End of Interrupt)
pub unsafe fn send_eoi() {
    write_apic(APIC_EOI, 0);
}

/// 读取 APIC 寄存器
unsafe fn read_apic(offset: usize) -> u32 {
    let addr = APIC_BASE + offset as u64;
    core::ptr::read_volatile(addr as *const u32)
}

/// 写入 APIC 寄存器
unsafe fn write_apic(offset: usize, value: u32) {
    let addr = APIC_BASE + offset as u64;
    core::ptr::write_volatile(addr as *mut u32, value);
}

/// 获取 APIC ID
pub fn apic_id() -> u8 {
    unsafe { (read_apic(APIC_ID) >> 24) as u8 }
}

/// 获取 APIC 版本
pub fn apic_version() -> u8 {
    unsafe { (read_apic(APIC_VERSION) & 0xFF) as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apic_msr() {
        assert_eq!(APIC_MSR, 0x1B);
    }

    #[test]
    fn test_apic_default_base() {
        assert_eq!(APIC_DEFAULT_BASE, 0xFEE00000);
    }

    #[test]
    fn test_spurious_vector() {
        assert_eq!(SPURIOUS_VECTOR, 0xFF);
    }

    #[test]
    fn test_apic_register_offsets() {
        assert_eq!(APIC_EOI, 0x0B0);
        assert_eq!(APIC_SVR, 0x0F0);
        assert_eq!(APIC_TIMER_LVT, 0x320);
    }

    #[test]
    fn test_irq_vector_range() {
        for irq in 0..16u8 {
            let vec = crate::arch::x86_64::pic::irq_to_vector(irq);
            assert!(vec >= 32 && vec <= 47);
        }
    }
}
