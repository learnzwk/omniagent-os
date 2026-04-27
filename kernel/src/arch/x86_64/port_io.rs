//! x86_64 端口 I/O 操作

/// 向 I/O 端口写入一个字节
#[inline(always)]
pub unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("outb %al, %dx", in("al") value, in("dx") port, options(nostack, nomem, preserves_flags));
}

/// 从 I/O 端口读取一个字节
#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let result: u8;
    core::arch::asm!("inb %dx, %al", out("al") result, in("dx") port, options(nostack, nomem, preserves_flags));
    result
}
