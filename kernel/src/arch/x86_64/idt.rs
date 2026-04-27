//! IDT (Interrupt Descriptor Table) 初始化
//!
//! 256 项中断描述符表，CPU 异常处理

use core::mem::size_of;

/// IDT 表项数量
pub const IDT_ENTRY_COUNT: usize = 256;

/// CPU 异常向量号
pub mod exception_vectors {
    pub const DIVISION_ERROR: u8 = 0;
    pub const DEBUG: u8 = 1;
    pub const NON_MASKABLE_INTERRUPT: u8 = 2;
    pub const BREAKPOINT: u8 = 3;
    pub const OVERFLOW: u8 = 4;
    pub const BOUND_RANGE_EXCEEDED: u8 = 5;
    pub const INVALID_OPCODE: u8 = 6;
    pub const DEVICE_NOT_AVAILABLE: u8 = 7;
    // 8: Double Fault (has error code)
    // 10: Invalid TSS
    pub const SEGMENT_NOT_PRESENT: u8 = 11;
    pub const STACK_SEGMENT_FAULT: u8 = 12;
    pub const GENERAL_PROTECTION_FAULT: u8 = 13;
    pub const PAGE_FAULT: u8 = 14;
    // 15: Reserved
    pub const X87_FPU_ERROR: u8 = 16;
    pub const ALIGNMENT_CHECK: u8 = 17;
    pub const MACHINE_CHECK: u8 = 18;
    pub const SIMD_EXCEPTION: u8 = 19;
    pub const VIRTUALIZATION_EXCEPTION: u8 = 20;
    pub const SECURITY_EXCEPTION: u8 = 30;
}

/// 中断栈帧 (由 CPU 在中断时自动压入)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InterruptStackFrame {
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: u64,
    pub stack_segment: u64,
}

/// IDT 表项 (16 字节)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    /// 缺省: 不存在
    pub const fn absent() -> Self {
        Self {
            offset_low: 0, selector: 0, ist: 0, type_attr: 0,
            offset_mid: 0, offset_high: 0, reserved: 0,
        }
    }

    /// 中断门 (用于硬件中断)
    pub const fn interrupt_gate(selector: u16, handler_addr: u64, ist: u8) -> Self {
        Self::new(selector, handler_addr, ist, 0x8E) // P=1, DPL=0, Type=0xE
    }

    /// 陷阱门 (用于 CPU 异常, 不自动禁用中断)
    pub const fn trap_gate(selector: u16, handler_addr: u64, ist: u8) -> Self {
        Self::new(selector, handler_addr, ist, 0x8F) // P=1, DPL=0, Type=0xF
    }

    const fn new(selector: u16, handler_addr: u64, ist: u8, type_attr: u8) -> Self {
        Self {
            offset_low: (handler_addr & 0xFFFF) as u16,
            offset_mid: ((handler_addr >> 16) & 0xFFFF) as u16,
            offset_high: ((handler_addr >> 32) & 0xFFFFFFFF) as u32,
            selector,
            ist: ist & 0x7,
            type_attr,
            reserved: 0,
        }
    }

    pub fn set_handler_addr(&mut self, addr: u64) {
        self.offset_low = (addr & 0xFFFF) as u16;
        self.offset_mid = ((addr >> 16) & 0xFFFF) as u16;
        self.offset_high = ((addr >> 32) & 0xFFFFFFFF) as u32;
    }

    pub fn is_present(&self) -> bool {
        (self.type_attr & 0x80) != 0
    }
}

/// IDT 结构
pub struct Idt {
    entries: [IdtEntry; IDT_ENTRY_COUNT],
}

impl Idt {
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::absent(); IDT_ENTRY_COUNT],
        }
    }

    pub fn set_handler(&mut self, vector: u8, handler: IdtEntry) {
        if (vector as usize) < IDT_ENTRY_COUNT {
            self.entries[vector as usize] = handler;
        }
    }

    pub fn get(&self, vector: u8) -> &IdtEntry {
        &self.entries[vector as usize]
    }
}

/// IDT 指针
#[repr(C)]
pub struct IdtPointer {
    pub limit: u16,
    pub base: u64,
}

impl IdtPointer {
    pub fn new(idt: &Idt) -> Self {
        Self {
            limit: (IDT_ENTRY_COUNT * size_of::<IdtEntry>() - 1) as u16,
            base: idt as *const Idt as u64,
        }
    }
}

/// 加载 IDT
#[cfg(not(test))]
pub unsafe fn load_idt(idt: &Idt) {
    let pointer = IdtPointer::new(idt);
    core::arch::asm!("lidt [{}]", in(reg) &pointer);
}

/// 加载 IDT (test stub - no-op)
#[cfg(test)]
pub unsafe fn load_idt(_idt: &Idt) {
    // 在测试环境中不需要实际加载 IDT
}

/// 异常名称映射
pub fn exception_name(vector: u8) -> &'static str {
    match vector {
        0 => "Division Error",
        1 => "Debug",
        2 => "NMI",
        3 => "Breakpoint",
        4 => "Overflow",
        5 => "Bound Range Exceeded",
        6 => "Invalid Opcode",
        7 => "Device Not Available",
        8 => "Double Fault",
        10 => "Invalid TSS",
        11 => "Segment Not Present",
        12 => "Stack-Segment Fault",
        13 => "General Protection Fault",
        14 => "Page Fault",
        16 => "x87 FPU Error",
        17 => "Alignment Check",
        18 => "Machine Check",
        19 => "SIMD Exception",
        20 => "Virtualization Exception",
        30 => "Security Exception",
        _ => "Unknown Exception",
    }
}

/// 带错误码的异常列表
pub fn has_error_code(vector: u8) -> bool {
    matches!(vector, 8 | 10 | 11 | 12 | 13 | 14 | 17 | 21 | 28 | 29 | 30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idt_entry_size() {
        assert_eq!(size_of::<IdtEntry>(), 16);
    }

    #[test]
    fn test_idt_entry_count() {
        assert_eq!(IDT_ENTRY_COUNT, 256);
    }

    #[test]
    fn test_absent_entry_not_present() {
        let entry = IdtEntry::absent();
        assert!(!entry.is_present());
    }

    #[test]
    fn test_trap_gate_is_present() {
        let entry = IdtEntry::trap_gate(0x08, 0x12345678, 0);
        assert!(entry.is_present());
    }

    #[test]
    fn test_interrupt_gate_type_attr() {
        let entry = IdtEntry::interrupt_gate(0x08, 0x12345678, 0);
        assert_eq!(entry.type_attr & 0x0F, 0x0E);
    }

    #[test]
    fn test_trap_gate_type_attr() {
        let entry = IdtEntry::trap_gate(0x08, 0x12345678, 0);
        assert_eq!(entry.type_attr & 0x0F, 0x0F);
    }

    #[test]
    fn test_idt_new_all_absent() {
        let idt = Idt::new();
        for i in 0..=255u8 {
            assert!(!idt.get(i).is_present());
        }
    }

    #[test]
    fn test_set_handler() {
        let mut idt = Idt::new();
        let handler = IdtEntry::trap_gate(0x08, 0xDEADBEEF, 0);
        idt.set_handler(14, handler);
        assert!(idt.get(14).is_present());
        assert!(!idt.get(13).is_present());
    }

    #[test]
    fn test_exception_name() {
        assert_eq!(exception_name(0), "Division Error");
        assert_eq!(exception_name(13), "General Protection Fault");
        assert_eq!(exception_name(14), "Page Fault");
        assert_eq!(exception_name(255), "Unknown Exception");
    }

    #[test]
    fn test_has_error_code() {
        assert!(has_error_code(8));   // Double Fault
        assert!(has_error_code(14));  // Page Fault
        assert!(has_error_code(13));  // GPF
        assert!(!has_error_code(0));  // Division Error
        assert!(!has_error_code(3));  // Breakpoint
    }

    #[test]
    fn test_interrupt_stack_frame_size() {
        assert_eq!(size_of::<InterruptStackFrame>(), 40);
    }

    #[test]
    fn test_idt_pointer_limit() {
        let idt = Idt::new();
        let ptr = IdtPointer::new(&idt);
        assert_eq!(ptr.limit, (256 * 16 - 1) as u16);
    }

    #[test]
    fn test_set_handler_addr() {
        let mut entry = IdtEntry::absent();
        entry.set_handler_addr(0xFEDCBA9876543210);
        let offset_low = entry.offset_low;
        let offset_mid = entry.offset_mid;
        let offset_high = entry.offset_high;
        assert_eq!(offset_low, 0x3210);
        assert_eq!(offset_mid, 0x7654);
        assert_eq!(offset_high, 0xFEDCBA98);
    }
}
