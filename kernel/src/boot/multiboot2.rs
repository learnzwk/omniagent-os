//! Multiboot2 引导信息解析
//!
//! 封装 bootloader crate 的 BootInfo，提供内存映射等接口

/// 内存区域类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MemoryRegionType {
    Usable = 1,
    Reserved = 2,
    AcpiReclaimable = 3,
    AcpiNvs = 4,
    BadMemory = 5,
    BootloaderReclaimable = 6,
    KernelAndModules = 7,
    Unknown = 0,
}

impl From<u32> for MemoryRegionType {
    fn from(val: u32) -> Self {
        match val {
            1 => Self::Usable,
            2 => Self::Reserved,
            3 => Self::AcpiReclaimable,
            4 => Self::AcpiNvs,
            5 => Self::BadMemory,
            6 => Self::BootloaderReclaimable,
            7 => Self::KernelAndModules,
            _ => Self::Unknown,
        }
    }
}

/// 内存区域
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base_addr: u64,
    pub length: u64,
    pub region_type: MemoryRegionType,
}

impl MemoryRegion {
    pub fn new(base_addr: u64, length: u64, region_type: MemoryRegionType) -> Self {
        Self { base_addr, length, region_type }
    }

    pub fn end_addr(&self) -> u64 {
        self.base_addr + self.length
    }

    pub fn is_usable(&self) -> bool {
        self.region_type == MemoryRegionType::Usable
    }

    /// 转换为帧范围 (4KB 对齐)
    pub fn start_frame(&self) -> u64 {
        (self.base_addr + 4095) & !4095
    }

    pub fn end_frame(&self) -> u64 {
        self.end_addr() & !4095
    }

    pub fn frame_count(&self) -> u64 {
        let start = self.start_frame();
        let end = self.end_frame();
        if end > start { (end - start) / 4096 } else { 0 }
    }
}

/// 引导信息摘要
#[derive(Debug)]
pub struct BootInfoSummary {
    pub memory_regions: [MemoryRegion; 32],
    pub memory_region_count: usize,
    pub total_usable_bytes: u64,
    pub total_usable_frames: u64,
}

impl BootInfoSummary {
    /// 从模拟数据创建 (用于测试)
    pub fn from_test_data(regions: &[MemoryRegion]) -> Self {
        let mut summary = Self {
            memory_regions: [MemoryRegion::new(0, 0, MemoryRegionType::Unknown); 32],
            memory_region_count: 0,
            total_usable_bytes: 0,
            total_usable_frames: 0,
        };
        for (i, region) in regions.iter().enumerate() {
            if i >= 32 { break; }
            summary.memory_regions[i] = *region;
            if region.is_usable() {
                summary.total_usable_bytes += region.length;
                summary.total_usable_frames += region.frame_count();
            }
            summary.memory_region_count = i + 1;
        }
        summary
    }

    pub fn usable_regions(&self) -> impl Iterator<Item = &MemoryRegion> {
        self.memory_regions[..self.memory_region_count]
            .iter()
            .filter(|r| r.is_usable())
    }

    pub fn total_usable_kb(&self) -> u64 {
        self.total_usable_bytes / 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_region(base: u64, len: u64, ty: MemoryRegionType) -> MemoryRegion {
        MemoryRegion::new(base, len, ty)
    }

    #[test]
    fn test_memory_region_type_from_u32() {
        assert_eq!(MemoryRegionType::from(1), MemoryRegionType::Usable);
        assert_eq!(MemoryRegionType::from(2), MemoryRegionType::Reserved);
        assert_eq!(MemoryRegionType::from(99), MemoryRegionType::Unknown);
    }

    #[test]
    fn test_memory_region_is_usable() {
        let usable = make_region(0x100000, 0x100000, MemoryRegionType::Usable);
        assert!(usable.is_usable());
        let reserved = make_region(0, 0x100000, MemoryRegionType::Reserved);
        assert!(!reserved.is_usable());
    }

    #[test]
    fn test_memory_region_frame_alignment() {
        let region = make_region(0x100000, 0x20000, MemoryRegionType::Usable);
        assert_eq!(region.start_frame(), 0x100000);
        assert_eq!(region.end_frame(), 0x120000);
        assert_eq!(region.frame_count(), 32); // 0x20000 / 0x1000
    }

    #[test]
    fn test_memory_region_unaligned() {
        let region = make_region(0x100500, 0x1000, MemoryRegionType::Usable);
        // start_frame should round up to 0x101000
        assert_eq!(region.start_frame(), 0x101000);
        assert_eq!(region.frame_count(), 0); // end_frame = 0x101500, start = 0x101000, only 0x500 bytes
    }

    #[test]
    fn test_boot_info_summary_from_test_data() {
        let regions = [
            make_region(0x0, 0x100000, MemoryRegionType::Reserved),
            make_region(0x100000, 0x100000, MemoryRegionType::Usable),
            make_region(0x200000, 0x400000, MemoryRegionType::Usable),
        ];
        let summary = BootInfoSummary::from_test_data(&regions);
        assert_eq!(summary.memory_region_count, 3);
        assert_eq!(summary.total_usable_bytes, 0x500000);
        assert_eq!(summary.total_usable_kb(), 0x500000 / 1024);
    }

    #[test]
    fn test_boot_info_usable_regions_iterator() {
        let regions = [
            make_region(0x0, 0x100000, MemoryRegionType::Reserved),
            make_region(0x100000, 0x100000, MemoryRegionType::Usable),
            make_region(0x200000, 0x100000, MemoryRegionType::AcpiReclaimable),
            make_region(0x300000, 0x100000, MemoryRegionType::Usable),
        ];
        let summary = BootInfoSummary::from_test_data(&regions);
        let usable: Vec<_> = summary.usable_regions().collect();
        assert_eq!(usable.len(), 2);
    }

    #[test]
    fn test_boot_info_max_32_regions() {
        let regions: Vec<_> = (0..40)
            .map(|i| make_region((i as u64) * 0x100000, 0x100000, MemoryRegionType::Usable))
            .collect();
        let summary = BootInfoSummary::from_test_data(&regions);
        assert_eq!(summary.memory_region_count, 32);
    }
}
