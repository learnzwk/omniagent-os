//! 页表条目定义
//!
//! 定义页表条目（PTE）和页表标志位，用于描述内存映射的属性。
//! 在模拟环境中，使用完整的 64 位地址空间（非真实硬件的 52 位限制）。
//!
//! 编码格式：
//! - 位 0-11: 标志位（含 NO_EXECUTE 在位 11）
//! - 位 12-63: 物理地址

use bitflags::bitflags;
use core::fmt;

use super::addr::PhysAddr;

/// 页表标志位
///
/// 对应 x86_64 架构的页表条目标志。
/// 在模拟环境中，NO_EXECUTE 使用位 11（而非真实硬件的位 63），
/// 以避免与高位物理地址冲突。
bitflags! {
    /// 页表标志位
    pub struct PageTableFlags: u64 {
        /// 页面存在（位 0）
        const PRESENT         = 1 << 0;
        /// 可写（位 1）
        const WRITABLE        = 1 << 1;
        /// 用户态可访问（位 2）
        const USER_ACCESSIBLE = 1 << 2;
        /// 写穿透（位 3）
        const WRITE_THROUGH   = 1 << 3;
        /// 禁用缓存（位 4）
        const NO_CACHE        = 1 << 4;
        /// 已访问（位 5）
        const ACCESSED        = 1 << 5;
        /// 已修改（脏页，位 6）
        const DIRTY           = 1 << 6;
        /// 大页面（位 7）
        const HUGE_PAGE       = 1 << 7;
        /// 全局页面（位 8）
        const GLOBAL          = 1 << 8;
        /// 禁止执行（模拟环境使用位 11，真实硬件为位 63）
        const NO_EXECUTE      = 1 << 11;
    }
}

/// 标志位掩码：低 12 位
const FLAGS_MASK: u64 = 0xFFF;

/// 页表条目
///
/// 每个 PTE 占 8 字节。
/// 在模拟环境中，使用低 12 位存储标志位，高位存储完整物理地址。
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// 创建空的页表条目（未使用）
    pub fn new() -> Self {
        PageTableEntry(0)
    }

    /// 检查条目是否未使用（全零）
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    /// 读取标志位（低 12 位）
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.0 & FLAGS_MASK)
    }

    /// 读取物理地址（清除标志位后的完整地址）
    pub fn phys_addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 & !FLAGS_MASK)
    }

    /// 设置物理地址（保留已有标志位）
    pub fn set_addr(&mut self, addr: PhysAddr) {
        let flags = self.0 & FLAGS_MASK;
        self.0 = flags | (addr.as_u64() & !FLAGS_MASK);
    }

    /// 设置标志位（保留已有地址）
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        let addr = self.0 & !FLAGS_MASK;
        self.0 = addr | (flags.bits() & FLAGS_MASK);
    }

    /// 同时设置物理地址和标志位
    pub fn set(&mut self, addr: PhysAddr, flags: PageTableFlags) {
        self.0 = (addr.as_u64() & !FLAGS_MASK) | (flags.bits() & FLAGS_MASK);
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageTableEntry")
            .field("addr", &self.phys_addr())
            .field("flags", &self.flags())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 9：空条目
    #[test]
    fn test_pte_new_empty() {
        let pte = PageTableEntry::new();
        assert!(pte.is_unused());
        assert_eq!(pte.0, 0);
    }

    /// 测试 10：设置地址和标志
    #[test]
    fn test_pte_set_addr_flags() {
        let mut pte = PageTableEntry::new();

        // 设置地址和标志
        let addr = PhysAddr::new(0x1000);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        pte.set(addr, flags);

        // 验证不再为空
        assert!(!pte.is_unused());

        // 验证地址和标志
        assert_eq!(pte.phys_addr(), addr);
        assert_eq!(pte.flags(), flags);
    }

    /// 测试 11：读取标志
    #[test]
    fn test_pte_flags() {
        let mut pte = PageTableEntry::new();

        // 空条目没有标志
        assert!(pte.flags().is_empty());

        // 设置标志后读取
        let flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_EXECUTE;
        pte.set(PhysAddr::new(0x2000), flags);
        assert_eq!(pte.flags(), flags);

        // 验证单个标志位
        assert!(pte.flags().contains(PageTableFlags::PRESENT));
        assert!(pte.flags().contains(PageTableFlags::USER_ACCESSIBLE));
        assert!(pte.flags().contains(PageTableFlags::NO_EXECUTE));
        assert!(!pte.flags().contains(PageTableFlags::WRITABLE));
    }

    /// 测试 12：读取物理地址
    #[test]
    fn test_pte_phys_addr() {
        let mut pte = PageTableEntry::new();

        // 设置地址
        pte.set(PhysAddr::new(0x3F000), PageTableFlags::PRESENT);
        assert_eq!(pte.phys_addr(), PhysAddr::new(0x3F000));

        // 设置另一个地址
        pte.set(PhysAddr::new(0x100000), PageTableFlags::WRITABLE);
        assert_eq!(pte.phys_addr(), PhysAddr::new(0x100000));

        // 高位地址（模拟环境中支持完整 64 位）
        pte.set(PhysAddr::new(0xFFFF_8000_0000_0000), PageTableFlags::PRESENT);
        assert_eq!(pte.phys_addr(), PhysAddr::new(0xFFFF_8000_0000_0000));
    }

    /// 测试 13：未使用检查
    #[test]
    fn test_pte_is_unused() {
        let pte = PageTableEntry::new();
        assert!(pte.is_unused());

        let mut pte = PageTableEntry::new();
        pte.set(PhysAddr::new(0x1000), PageTableFlags::PRESENT);
        assert!(!pte.is_unused());

        // 仅设置标志位（无地址）也不算未使用
        let mut pte2 = PageTableEntry::new();
        pte2.set_flags(PageTableFlags::PRESENT);
        assert!(!pte2.is_unused());
    }
}
