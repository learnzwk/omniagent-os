//! 中断子系统
//!
//! CPU 异常处理和中断描述符表 (IDT) 管理

pub mod exceptions;

use crate::arch::x86_64::gdt::KERNEL_CODE_SELECTOR;
use crate::arch::x86_64::idt::{Idt, IdtEntry, load_idt};
use crate::interrupts::exceptions::{
    division_error_handler,
    breakpoint_handler,
    invalid_opcode_handler,
    double_fault_handler,
    gp_fault_handler,
    page_fault_handler,
};

/// 初始化 IDT (Interrupt Descriptor Table)
///
/// 设置 CPU 异常处理器 (向量 0, 3, 6, 8, 13, 14)
/// 并加载 IDT 到 CPU。此函数必须在 GDT 加载之后调用。
pub unsafe fn init_idt() {
    let mut idt = Idt::new();

    // 向量 0: 除零错误 (Division Error)
    idt.set_handler(0, IdtEntry::trap_gate(
        KERNEL_CODE_SELECTOR,
        division_error_handler as u64,
        0,
    ));

    // 向量 3: 断点 (Breakpoint) -- 可恢复
    idt.set_handler(3, IdtEntry::trap_gate(
        KERNEL_CODE_SELECTOR,
        breakpoint_handler as u64,
        0,
    ));

    // 向量 6: 无效操作码 (Invalid Opcode)
    idt.set_handler(6, IdtEntry::trap_gate(
        KERNEL_CODE_SELECTOR,
        invalid_opcode_handler as u64,
        0,
    ));

    // 向量 8: 双重错误 (Double Fault) -- 致命，带错误码
    idt.set_handler(8, IdtEntry::trap_gate(
        KERNEL_CODE_SELECTOR,
        double_fault_handler as u64,
        0,
    ));

    // 向量 13: 一般保护错误 (General Protection Fault) -- 带错误码
    idt.set_handler(13, IdtEntry::trap_gate(
        KERNEL_CODE_SELECTOR,
        gp_fault_handler as u64,
        0,
    ));

    // 向量 14: 页错误 (Page Fault) -- 带错误码
    idt.set_handler(14, IdtEntry::trap_gate(
        KERNEL_CODE_SELECTOR,
        page_fault_handler as u64,
        0,
    ));

    // 加载 IDT 到 CPU
    load_idt(&idt);
}
