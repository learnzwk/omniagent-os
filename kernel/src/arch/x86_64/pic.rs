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

    /// 测试：PIC 数据端口地址验证
    #[test]
    fn test_pic_data_port_addresses() {
        assert_eq!(PIC1_DATA, 0x21);
        assert_eq!(PIC2_DATA, 0xA1);
    }

    /// 测试：IRQ 到向量号映射 - 边界值
    #[test]
    fn test_irq_to_vector_boundary() {
        // 最小 IRQ
        assert_eq!(irq_to_vector(0), 32);
        // 最大 IRQ
        assert_eq!(irq_to_vector(15), 47);
        // 中间值
        assert_eq!(irq_to_vector(8), 40);
    }

    /// 测试：IRQ 到向量号映射 - 连续递增
    #[test]
    fn test_irq_to_vector_sequential() {
        for irq in 0..15u8 {
            assert_eq!(irq_to_vector(irq + 1), irq_to_vector(irq) + 1);
        }
    }

    /// 测试：所有 IRQ 向量号在有效范围内
    #[test]
    fn test_irq_vectors_in_range() {
        for irq in 0..=15u8 {
            let vec = irq_to_vector(irq);
            assert!(vec >= 32 && vec <= 47, "IRQ {} 映射到向量 {}，不在 32-47 范围内", irq, vec);
        }
    }

    /// 测试：PIC 端口地址关系 - 数据端口 = 命令端口 + 1
    #[test]
    fn test_pic_port_relationships() {
        assert_eq!(PIC1_DATA, PIC1_COMMAND + 1);
        assert_eq!(PIC2_DATA, PIC2_COMMAND + 1);
    }

    /// 测试：主片和从片端口地址不重叠
    #[test]
    fn test_pic_ports_no_overlap() {
        assert_ne!(PIC1_COMMAND, PIC2_COMMAND);
        assert_ne!(PIC1_DATA, PIC2_DATA);
        assert_ne!(PIC1_COMMAND, PIC2_DATA);
        assert_ne!(PIC1_DATA, PIC2_COMMAND);
    }

    /// 测试：irq_to_vector 是纯函数（const fn）
    #[test]
    fn test_irq_to_vector_is_pure() {
        // 多次调用相同输入应返回相同结果
        for _ in 0..10 {
            assert_eq!(irq_to_vector(7), 39);
            assert_eq!(irq_to_vector(3), 35);
        }
    }
}
