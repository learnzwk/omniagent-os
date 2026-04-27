//! 8259 PIC 驱动 (禁用，为 APIC 让路)

use crate::arch::x86_64::port_io::outb;

/// PIC 端口地址
const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

/// 禁用 8259 PIC (所有中断被屏蔽)
pub fn disable_pic() {
    unsafe {
        // ICW1: 初始化 + ICW4 needed
        outb(PIC1_COMMAND, 0x11);
        outb(PIC2_COMMAND, 0x11);
        // ICW2: 主片偏移 0x20, 从片偏移 0x28
        outb(PIC1_DATA, 0x20);
        outb(PIC2_DATA, 0x28);
        // ICW3: 从片连接到主片 IRQ2
        outb(PIC1_DATA, 0x04);
        outb(PIC2_DATA, 0x02);
        // ICW4: 8086 mode
        outb(PIC1_DATA, 0x01);
        outb(PIC2_DATA, 0x01);
        // 屏蔽所有中断
        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);
    }
}

/// PIC IRQ 到向量号映射 (ISA IRQ 0-15 -> 向量 32-47)
pub const fn irq_to_vector(irq: u8) -> u8 {
    32 + irq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irq_to_vector() {
        assert_eq!(irq_to_vector(0), 32);
        assert_eq!(irq_to_vector(7), 39);
        assert_eq!(irq_to_vector(15), 47);
    }

    #[test]
    fn test_pic_port_addresses() {
        assert_eq!(PIC1_COMMAND, 0x20);
        assert_eq!(PIC2_COMMAND, 0xA0);
    }
}
