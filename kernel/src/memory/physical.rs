//! 物理内存管理
//!
//! 基于引导信息构建物理内存映射

use crate::boot::multiboot2::{BootInfoSummary, MemoryRegion};

/// 物理地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr(pub u64);

impl PhysAddr {
    pub const ZERO: PhysAddr = PhysAddr(0);
    pub const fn new(addr: u64) -> Self { Self(addr) }
    pub fn is_page_aligned(&self) -> bool { self.0 % 4096 == 0 }
    pub fn page_align_up(&self) -> PhysAddr { PhysAddr((self.0 + 4095) & !4095) }
    pub fn page_align_down(&self) -> PhysAddr { PhysAddr(self.0 & !4095) }
    pub fn offset(&self, offset: u64) -> PhysAddr { PhysAddr(self.0 + offset) }
}

/// 物理帧
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysFrame {
    pub number: usize,
}

impl PhysFrame {
    pub const fn new(number: usize) -> Self { Self { number } }
    pub fn start_address(&self) -> PhysAddr { PhysAddr::new(self.number as u64 * 4096) }
    pub fn containing_address(addr: PhysAddr) -> Self {
        Self { number: (addr.0 / 4096) as usize }
    }
}

/// 物理内存管理器
pub struct PhysicalMemoryManager {
    boot_info: BootInfoSummary,
    next_frame: usize,
}

impl PhysicalMemoryManager {
    pub fn new(boot_info: BootInfoSummary) -> Self {
        Self { boot_info, next_frame: 0 }
    }

    pub fn total_usable_bytes(&self) -> u64 { self.boot_info.total_usable_bytes }
    pub fn total_usable_frames(&self) -> u64 { self.boot_info.total_usable_frames }

    pub fn usable_regions(&self) -> impl Iterator<Item = &MemoryRegion> {
        self.boot_info.usable_regions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phys_addr_page_alignment() {
        let aligned = PhysAddr::new(0x100000);
        assert!(aligned.is_page_aligned());
        let unaligned = PhysAddr::new(0x100500);
        assert!(!unaligned.is_page_aligned());
    }

    #[test]
    fn test_phys_addr_align_up() {
        let addr = PhysAddr::new(0x100500);
        assert_eq!(addr.page_align_up(), PhysAddr::new(0x101000));
    }

    #[test]
    fn test_phys_addr_align_down() {
        let addr = PhysAddr::new(0x100500);
        assert_eq!(addr.page_align_down(), PhysAddr::new(0x100000));
    }

    #[test]
    fn test_phys_frame_start_address() {
        let frame = PhysFrame::new(256);
        assert_eq!(frame.start_address(), PhysAddr::new(256 * 4096));
    }

    #[test]
    fn test_phys_frame_containing_address() {
        let frame = PhysFrame::containing_address(PhysAddr::new(0x100500));
        assert_eq!(frame.number, 256);
    }

    #[test]
    fn test_phys_addr_zero() {
        assert_eq!(PhysAddr::ZERO, PhysAddr::new(0));
    }

    #[test]
    fn test_phys_addr_offset() {
        let base = PhysAddr::new(0x1000);
        assert_eq!(base.offset(0x500), PhysAddr::new(0x1500));
    }

    #[test]
    fn test_phys_frame_eq() {
        assert_eq!(PhysFrame::new(42), PhysFrame::new(42));
        assert_ne!(PhysFrame::new(42), PhysFrame::new(43));
    }
}
