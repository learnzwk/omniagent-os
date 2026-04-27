//! CPU 异常处理函数
//!
//! 注意: 理想情况下应使用 `extern "x86-interrupt"` ABI，
//! 但该 ABI 需要 nightly Rust。在 stable 上使用 `extern "C"` 作为替代。

use core::fmt::Write;

use crate::arch::x86_64::idt::{exception_name, has_error_code, InterruptStackFrame};
use crate::drivers::serial::SERIAL;

/// 默认异常处理函数
pub extern "C" fn default_exception_handler(
    frame: *const InterruptStackFrame,
) {
    let vector: u8 = 0; // Will be set by actual handler
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nEXCEPTION: {} at {:#018X}\n",
            exception_name(vector),
            frame_ref.instruction_pointer,
        );
    }
}

/// 除零错误 (Vector 0)
pub extern "C" fn division_error_handler(frame: *const InterruptStackFrame) {
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nEXCEPTION: Division Error at {:#018X}\n",
            frame_ref.instruction_pointer,
        );
    }
}

/// 断点 (Vector 3) -- 可恢复
pub extern "C" fn breakpoint_handler(frame: *const InterruptStackFrame) {
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nBREAKPOINT at {:#018X}\n",
            frame_ref.instruction_pointer,
        );
    }
}

/// 无效操作码 (Vector 6)
pub extern "C" fn invalid_opcode_handler(frame: *const InterruptStackFrame) {
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nEXCEPTION: Invalid Opcode at {:#018X}\n",
            frame_ref.instruction_pointer,
        );
    }
}

/// General Protection Fault (Vector 13)
pub extern "C" fn gp_fault_handler(
    frame: *const InterruptStackFrame,
    error_code: u64,
) {
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nEXCEPTION: General Protection Fault (error={:#X}) at {:#018X}\n",
            error_code,
            frame_ref.instruction_pointer,
        );
    }
}

/// Page Fault (Vector 14)
pub extern "C" fn page_fault_handler(
    frame: *const InterruptStackFrame,
    error_code: u64,
) {
    let cr2: u64;
    unsafe { core::arch::asm!("mov {}, cr2", out(reg) cr2) };
    let present = (error_code & 0x1) != 0;
    let write = (error_code & 0x2) != 0;
    let user = (error_code & 0x4) != 0;
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nEXCEPTION: Page Fault at {:#018X}\n  CR2={:#018X} present={} write={} user={}\n",
            frame_ref.instruction_pointer, cr2, present, write, user,
        );
    }
}

/// Double Fault (Vector 8)
pub extern "C" fn double_fault_handler(
    frame: *const InterruptStackFrame,
    error_code: u64,
) -> ! {
    unsafe {
        let frame_ref = &*frame;
        let mut serial = SERIAL.lock();
        let _ = write!(
            serial,
            "\nFATAL: Double Fault (error={:#X}) at {:#018X}\n",
            error_code,
            frame_ref.instruction_pointer,
        );
    }
    loop {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::x86_64::idt::exception_name;

    #[test]
    fn test_exception_name_division_error() {
        assert_eq!(exception_name(0), "Division Error");
    }

    #[test]
    fn test_exception_name_page_fault() {
        assert_eq!(exception_name(14), "Page Fault");
    }

    #[test]
    fn test_exception_name_gpf() {
        assert_eq!(exception_name(13), "General Protection Fault");
    }

    #[test]
    fn test_has_error_code_page_fault() {
        assert!(has_error_code(14));
    }

    #[test]
    fn test_has_error_code_double_fault() {
        assert!(has_error_code(8));
    }

    #[test]
    fn test_has_error_code_division_error() {
        assert!(!has_error_code(0));
    }

    #[test]
    fn test_interrupt_stack_frame_size() {
        use core::mem::size_of;
        assert_eq!(size_of::<InterruptStackFrame>(), 40);
    }
}
