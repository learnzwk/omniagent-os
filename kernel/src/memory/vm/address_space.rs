//! 地址空间管理
//!
//! 提供进程/内核地址空间的抽象，管理虚拟内存区域（VMA）和页表映射。

use bitflags::bitflags;
use core::fmt;

#[cfg(test)]
use std::vec::Vec;
#[cfg(not(test))]
use alloc::vec::Vec;

use super::addr::{PhysAddr, VirtAddr, PAGE_SIZE};
use super::page_table::{MapError, PageTable, UnmapError};

/// 虚拟内存区域标志位
bitflags! {
    /// 虚拟内存区域标志位
    pub struct VmFlags: u32 {
        /// 可读
        const READ    = 1 << 0;
        /// 可写
        const WRITE   = 1 << 1;
        /// 可执行
        const EXECUTE = 1 << 2;
        /// 用户空间
        const USER    = 1 << 3;
        /// 写时复制
        const COW     = 1 << 4;
        /// 保护页（不可访问）
        const GUARD   = 1 << 5;
    }
}

/// 虚拟内存区域
///
/// 描述一段连续的虚拟地址范围及其属性。
pub struct VmArea {
    /// 区域起始地址（包含）
    pub start: VirtAddr,
    /// 区域结束地址（不包含）
    pub end: VirtAddr,
    /// 区域标志位
    pub flags: VmFlags,
    /// 区域名称（用于调试）
    pub name: &'static str,
}

/// 地址空间类型
#[derive(Debug, Clone, PartialEq)]
pub enum AddressSpaceKind {
    /// 内核地址空间
    Kernel,
    /// 用户地址空间
    User { agent_handle: u64 },
}

impl fmt::Display for AddressSpaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressSpaceKind::Kernel => write!(f, "内核地址空间"),
            AddressSpaceKind::User { agent_handle } => {
                write!(f, "用户地址空间 (agent={})", agent_handle)
            }
        }
    }
}

/// 地址空间
///
/// 管理一个页表和一组虚拟内存区域。
pub struct AddressSpace {
    /// 页表
    pub page_table: PageTable,
    /// 虚拟内存区域列表
    pub areas: Vec<VmArea>,
    /// 地址空间类型
    pub kind: AddressSpaceKind,
}

impl AddressSpace {
    /// 创建内核地址空间
    pub fn new_kernel() -> Self {
        AddressSpace {
            page_table: PageTable::new(),
            areas: Vec::new(),
            kind: AddressSpaceKind::Kernel,
        }
    }

    /// 创建用户地址空间
    pub fn new_user(agent_handle: u64) -> Self {
        AddressSpace {
            page_table: PageTable::new(),
            areas: Vec::new(),
            kind: AddressSpaceKind::User { agent_handle },
        }
    }

    /// 映射一个虚拟内存区域
    ///
    /// 在地址空间中创建一个新的 VMA，并建立页表映射。
    /// size 会被向上对齐到页大小。
    pub fn map_area(
        &mut self,
        start: VirtAddr,
        size: u64,
        flags: VmFlags,
        name: &'static str,
    ) -> Result<(), MapError> {
        if !start.is_canonical() || !start.is_aligned() {
            return Err(MapError::InvalidAddress);
        }

        let aligned_size = ((size + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1)).max(PAGE_SIZE as u64);
        let end = VirtAddr::new(start.as_u64() + aligned_size);

        // 检查是否与已有区域重叠
        if self.is_overlap(start, end) {
            return Err(MapError::AlreadyMapped);
        }

        // 将 VmFlags 转换为 PageTableFlags
        let pt_flags = vm_flags_to_pt_flags(flags);

        // 建立页表映射
        let mut addr = start.as_u64();
        let end_addr = end.as_u64();
        while addr < end_addr {
            let virt = VirtAddr::new(addr);
            // 使用虚拟地址作为物理地址的模拟（在真实内核中会分配物理帧）
            let phys = PhysAddr::new(addr);
            self.page_table.map(virt, phys, pt_flags)?;
            addr += PAGE_SIZE as u64;
        }

        // 添加 VMA
        self.areas.push(VmArea {
            start,
            end,
            flags,
            name,
        });

        Ok(())
    }

    /// 取消映射一个虚拟内存区域
    ///
    /// 移除指定起始地址对应的 VMA，并取消所有页表映射。
    pub fn unmap_area(&mut self, start: VirtAddr) -> Result<(), UnmapError> {
        if !start.is_canonical() || !start.is_aligned() {
            return Err(UnmapError::InvalidAddress);
        }

        // 查找匹配的 VMA
        let area_idx = self
            .areas
            .iter()
            .position(|area| area.start == start)
            .ok_or(UnmapError::NotMapped)?;

        let area = &self.areas[area_idx];

        // 取消页表映射
        let mut addr = area.start.as_u64();
        let end_addr = area.end.as_u64();
        while addr < end_addr {
            let virt = VirtAddr::new(addr);
            let _ = self.page_table.unmap(virt);
            addr += PAGE_SIZE as u64;
        }

        // 移除 VMA
        self.areas.remove(area_idx);

        Ok(())
    }

    /// 查找空闲区域
    ///
    /// 在地址空间中查找一块足够大的连续空闲区域。
    /// 返回空闲区域的起始地址。
    pub fn find_free_area(&self, size: u64) -> Option<VirtAddr> {
        let aligned_size = ((size + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1)).max(PAGE_SIZE as u64);

        // 对区域按起始地址排序
        let mut sorted_areas: Vec<&VmArea> = self.areas.iter().collect();
        sorted_areas.sort_by_key(|a| a.start.as_u64());

        // 搜索起始地址（内核空间从较高地址开始，用户空间从较低地址开始）
        let search_start = match self.kind {
            AddressSpaceKind::Kernel => 0xFFFF_8000_0000_0000u64,
            AddressSpaceKind::User { .. } => 0x1000u64, // 跳过空指针页
        };

        let mut prev_end = search_start;

        for area in &sorted_areas {
            let gap = area.start.as_u64().saturating_sub(prev_end);
            if gap >= aligned_size {
                return Some(VirtAddr::new(prev_end));
            }
            prev_end = area.end.as_u64();
        }

        // 检查最后一个区域之后的空间
        // 确保不超出规范地址范围
        let max_addr = match self.kind {
            AddressSpaceKind::Kernel => 0xFFFF_FFFF_FFFF_FFFFu64,
            AddressSpaceKind::User { .. } => 0x0000_7FFF_FFFF_FFFFu64,
        };

        if max_addr.saturating_sub(prev_end) >= aligned_size {
            return Some(VirtAddr::new(prev_end));
        }

        None
    }

    /// 检查地址范围是否与已有区域重叠
    pub fn is_overlap(&self, start: VirtAddr, end: VirtAddr) -> bool {
        for area in &self.areas {
            // 两个区间 [start, end) 和 [area.start, area.end) 重叠的条件
            if start.as_u64() < area.end.as_u64() && end.as_u64() > area.start.as_u64() {
                return true;
            }
        }
        false
    }

    /// 翻译虚拟地址到物理地址
    pub fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.page_table.translate(addr)
    }
}

/// 将 VmFlags 转换为 PageTableFlags
fn vm_flags_to_pt_flags(vm_flags: VmFlags) -> super::pte::PageTableFlags {
    use super::pte::PageTableFlags;

    let mut pt_flags = PageTableFlags::PRESENT;

    if vm_flags.contains(VmFlags::WRITE) {
        pt_flags |= PageTableFlags::WRITABLE;
    }
    if vm_flags.contains(VmFlags::USER) {
        pt_flags |= PageTableFlags::USER_ACCESSIBLE;
    }
    if !vm_flags.contains(VmFlags::EXECUTE) {
        pt_flags |= PageTableFlags::NO_EXECUTE;
    }

    pt_flags
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 22：创建内核地址空间
    #[test]
    fn test_address_space_new_kernel() {
        let aspace = AddressSpace::new_kernel();
        assert_eq!(aspace.kind, AddressSpaceKind::Kernel);
        assert!(aspace.areas.is_empty());
        assert_eq!(aspace.page_table.mapping_count(), 0);
    }

    /// 测试 23：创建用户地址空间
    #[test]
    fn test_address_space_new_user() {
        let aspace = AddressSpace::new_user(42);
        assert_eq!(aspace.kind, AddressSpaceKind::User { agent_handle: 42 });
        assert!(aspace.areas.is_empty());
        assert_eq!(aspace.page_table.mapping_count(), 0);
    }

    /// 测试 24：映射区域
    #[test]
    fn test_address_space_map_area() {
        let mut aspace = AddressSpace::new_kernel();

        // 映射一个 4KB 的区域
        let start = VirtAddr::new(0xFFFF_8000_0000_0000);
        let flags = VmFlags::READ | VmFlags::WRITE;
        let result = aspace.map_area(start, 4096, flags, "test_area");
        assert!(result.is_ok());

        // 验证 VMA
        assert_eq!(aspace.areas.len(), 1);
        assert_eq!(aspace.areas[0].start, start);
        assert_eq!(aspace.areas[0].end, VirtAddr::new(0xFFFF_8000_0000_1000));
        assert_eq!(aspace.areas[0].flags, flags);
        assert_eq!(aspace.areas[0].name, "test_area");

        // 验证页表映射
        assert_eq!(aspace.page_table.mapping_count(), 1);
        assert_eq!(
            aspace.translate(start),
            Some(PhysAddr::new(0xFFFF_8000_0000_0000))
        );

        // 映射更大的区域（8KB = 2 页）
        let start2 = VirtAddr::new(0xFFFF_8000_0000_1000);
        let result2 = aspace.map_area(start2, 8192, flags, "test_area2");
        assert!(result2.is_ok());
        assert_eq!(aspace.areas.len(), 2);
        assert_eq!(aspace.page_table.mapping_count(), 3);

        // 重复映射应失败
        let result3 = aspace.map_area(start, 4096, flags, "duplicate");
        assert_eq!(result3, Err(MapError::AlreadyMapped));
    }

    /// 测试 25：查找空闲区域
    #[test]
    fn test_address_space_find_free_area() {
        let mut aspace = AddressSpace::new_user(1);

        // 空地址空间应能找到空闲区域
        let free = aspace.find_free_area(4096);
        assert!(free.is_some());
        assert_eq!(free.unwrap(), VirtAddr::new(0x1000));

        // 映射一个区域
        let start = VirtAddr::new(0x1000);
        aspace
            .map_area(start, 4096, VmFlags::READ | VmFlags::WRITE, "area1")
            .unwrap();

        // 下一个空闲区域应在 0x2000
        let free = aspace.find_free_area(4096);
        assert!(free.is_some());
        assert_eq!(free.unwrap(), VirtAddr::new(0x2000));

        // 映射第二个区域
        let start2 = VirtAddr::new(0x2000);
        aspace
            .map_area(start2, 4096, VmFlags::READ, "area2")
            .unwrap();

        // 下一个空闲区域应在 0x3000
        let free = aspace.find_free_area(4096);
        assert!(free.is_some());
        assert_eq!(free.unwrap(), VirtAddr::new(0x3000));
    }

    /// 测试 26：重叠检测
    #[test]
    fn test_address_space_is_overlap() {
        let mut aspace = AddressSpace::new_user(1);

        // 映射区域 [0x1000, 0x3000)
        aspace
            .map_area(
                VirtAddr::new(0x1000),
                8192,
                VmFlags::READ,
                "area1",
            )
            .unwrap();

        // 完全重叠
        assert!(aspace.is_overlap(VirtAddr::new(0x1000), VirtAddr::new(0x3000)));

        // 部分重叠（左侧）
        assert!(aspace.is_overlap(VirtAddr::new(0x500), VirtAddr::new(0x1500)));

        // 部分重叠（右侧）
        assert!(aspace.is_overlap(VirtAddr::new(0x2500), VirtAddr::new(0x3500)));

        // 完全包含
        assert!(aspace.is_overlap(VirtAddr::new(0x1500), VirtAddr::new(0x2500)));

        // 不重叠（左侧）
        assert!(!aspace.is_overlap(VirtAddr::new(0x3000), VirtAddr::new(0x4000)));

        // 不重叠（右侧）
        assert!(!aspace.is_overlap(VirtAddr::new(0x0), VirtAddr::new(0x1000)));

        // 相邻但不重叠
        assert!(!aspace.is_overlap(VirtAddr::new(0x3000), VirtAddr::new(0x3000)));
    }

    /// 测试 27：地址翻译
    #[test]
    fn test_address_space_translate() {
        let mut aspace = AddressSpace::new_kernel();

        // 未映射的地址应返回 None
        assert_eq!(
            aspace.translate(VirtAddr::new(0xFFFF_8000_0000_0000)),
            None
        );

        // 映射后翻译
        let start = VirtAddr::new(0xFFFF_8000_0000_0000);
        aspace
            .map_area(start, 4096, VmFlags::READ | VmFlags::WRITE, "test")
            .unwrap();

        assert_eq!(
            aspace.translate(start),
            Some(PhysAddr::new(0xFFFF_8000_0000_0000))
        );

        // 带偏移的翻译
        assert_eq!(
            aspace.translate(VirtAddr::new(0xFFFF_8000_0000_0ABC)),
            Some(PhysAddr::new(0xFFFF_8000_0000_0ABC))
        );
    }
}
