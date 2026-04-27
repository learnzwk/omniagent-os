//! 页表管理
//!
//! 实现 4 级页表（PML4 → PDPT → PD → PT）的映射、取消映射和地址翻译。
//! 使用 Vec<u64> 模拟每级页表，中间页表按需分配。
//!
//! 中间页表条目使用特殊编码存储中间页表的索引（而非真实物理地址）。
//! 最终页表条目（PT）存储完整的物理地址和标志位。

use core::fmt;

#[cfg(test)]
use std::vec::Vec;
#[cfg(not(test))]
use alloc::vec::Vec;
#[cfg(not(test))]
use alloc::vec;

use super::addr::{PhysAddr, VirtAddr, PAGE_SIZE};
use super::pte::PageTableFlags;

/// 每级页表的条目数
const ENTRY_COUNT: usize = 512;

/// 标志位掩码：低 12 位
const FLAGS_MASK: u64 = 0xFFF;

/// 中间页表条目使用的地址掩码（用于提取中间页表索引）
/// 中间页表索引编码在位 12-51 中（与真实硬件 PTE 格式兼容）
const INTERMEDIATE_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// 映射错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// 无效地址（非规范地址或未对齐）
    InvalidAddress,
    /// 该虚拟地址已被映射
    AlreadyMapped,
    /// 页表分配失败
    AllocationFailed,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::InvalidAddress => write!(f, "无效的虚拟地址"),
            MapError::AlreadyMapped => write!(f, "该虚拟地址已被映射"),
            MapError::AllocationFailed => write!(f, "页表分配失败"),
        }
    }
}

/// 取消映射错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum UnmapError {
    /// 该虚拟地址未被映射
    NotMapped,
    /// 无效地址
    InvalidAddress,
}

impl fmt::Display for UnmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnmapError::NotMapped => write!(f, "该虚拟地址未被映射"),
            UnmapError::InvalidAddress => write!(f, "无效的虚拟地址"),
        }
    }
}

/// 4 级页表
///
/// 使用 Vec<u64> 模拟页表结构，每个 Vec 有 512 个 u64 条目。
/// 中间页表（PDPT、PD、PT）按需分配。
pub struct PageTable {
    /// PML4（第4级页表），始终存在
    pml4: Vec<u64>,
    /// 所有已分配的中间页表（PDPT、PD、PT）
    /// 使用 Vec<u64> 存储，每个 Vec 有 512 个条目
    intermediate_tables: Vec<Vec<u64>>,
}

impl PageTable {
    /// 创建空页表（全零）
    pub fn new() -> Self {
        PageTable {
            pml4: vec![0u64; ENTRY_COUNT],
            intermediate_tables: Vec::new(),
        }
    }

    /// 从虚拟地址提取指定级别的索引
    ///
    /// - level 0: PML4 索引（位 39-47）
    /// - level 1: PDPT 索引（位 30-38）
    /// - level 2: PD 索引（位 21-29）
    /// - level 3: PT 索引（位 12-20）
    fn index_from_addr(addr: u64, level: usize) -> usize {
        let shift = 12 + 9 * (3 - level);
        ((addr >> shift) & 0x1FF) as usize
    }

    /// 从中间页表条目中提取下一级页表的索引
    ///
    /// 中间页表条目编码：((index + 1) << 12) | PRESENT
    fn extract_table_index(entry: u64) -> usize {
        (((entry & INTERMEDIATE_ADDR_MASK) >> 12) - 1) as usize
    }

    /// 创建中间页表条目
    ///
    /// 将页表索引编码为中间页表条目格式
    fn make_table_entry(index: usize) -> u64 {
        ((index as u64) + 1) << 12 | PageTableFlags::PRESENT.bits()
    }

    /// 分配新的中间页表，返回其索引
    fn alloc_intermediate_table(&mut self) -> usize {
        let idx = self.intermediate_tables.len();
        self.intermediate_tables.push(vec![0u64; ENTRY_COUNT]);
        idx
    }

    /// 获取或创建下一级页表
    ///
    /// 如果当前级条目为 0，则分配新的中间页表。
    /// 返回下一级页表在 intermediate_tables 中的索引。
    fn get_or_create_next_table(
        &mut self,
        table: &[u64],
        entry_idx: usize,
    ) -> usize {
        if table[entry_idx] == 0 {
            let new_idx = self.alloc_intermediate_table();
            // 注意：此处无法直接修改 table，由调用者负责写入
            new_idx
        } else {
            Self::extract_table_index(table[entry_idx])
        }
    }

    /// 映射一个 4KB 页
    ///
    /// 将虚拟地址映射到物理地址，设置指定的标志位。
    /// 如果中间页表不存在，则按需分配。
    pub fn map(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: PageTableFlags,
    ) -> Result<(), MapError> {
        // 验证虚拟地址
        if !virt.is_canonical() || !virt.is_aligned() {
            return Err(MapError::InvalidAddress);
        }

        let addr = virt.as_u64();

        // 提取 4 级索引
        let pml4_idx = Self::index_from_addr(addr, 0);
        let pdpt_idx = Self::index_from_addr(addr, 1);
        let pd_idx = Self::index_from_addr(addr, 2);
        let pt_idx = Self::index_from_addr(addr, 3);

        // PML4 条目指向 PDPT 表
        let pdpt_table_idx = if self.pml4[pml4_idx] == 0 {
            let new_idx = self.alloc_intermediate_table();
            self.pml4[pml4_idx] = Self::make_table_entry(new_idx);
            new_idx
        } else {
            Self::extract_table_index(self.pml4[pml4_idx])
        };

        // PDPT 条目指向 PD 表
        let pd_table_idx = if self.intermediate_tables[pdpt_table_idx][pdpt_idx] == 0 {
            let new_idx = self.alloc_intermediate_table();
            self.intermediate_tables[pdpt_table_idx][pdpt_idx] = Self::make_table_entry(new_idx);
            new_idx
        } else {
            Self::extract_table_index(self.intermediate_tables[pdpt_table_idx][pdpt_idx])
        };

        // PD 条目指向 PT 表
        let pt_table_idx = if self.intermediate_tables[pd_table_idx][pd_idx] == 0 {
            let new_idx = self.alloc_intermediate_table();
            self.intermediate_tables[pd_table_idx][pd_idx] = Self::make_table_entry(new_idx);
            new_idx
        } else {
            Self::extract_table_index(self.intermediate_tables[pd_table_idx][pd_idx])
        };

        // PT 条目：最终的页面映射
        if self.intermediate_tables[pt_table_idx][pt_idx] != 0 {
            return Err(MapError::AlreadyMapped);
        }

        // 设置最终的页面映射（使用完整 64 位物理地址 + 标志位）
        let entry = (phys.as_u64() & !FLAGS_MASK) | (flags.bits() & FLAGS_MASK);
        self.intermediate_tables[pt_table_idx][pt_idx] = entry;

        Ok(())
    }

    /// 取消映射
    ///
    /// 移除指定虚拟地址的映射，返回之前映射的物理地址。
    pub fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, UnmapError> {
        if !virt.is_canonical() || !virt.is_aligned() {
            return Err(UnmapError::InvalidAddress);
        }

        let addr = virt.as_u64();

        let pml4_idx = Self::index_from_addr(addr, 0);
        let pdpt_idx = Self::index_from_addr(addr, 1);
        let pd_idx = Self::index_from_addr(addr, 2);
        let pt_idx = Self::index_from_addr(addr, 3);

        // 检查 PML4 条目
        if self.pml4[pml4_idx] == 0 {
            return Err(UnmapError::NotMapped);
        }

        let pdpt_table_idx = Self::extract_table_index(self.pml4[pml4_idx]);

        // 检查 PDPT 条目
        if self.intermediate_tables[pdpt_table_idx][pdpt_idx] == 0 {
            return Err(UnmapError::NotMapped);
        }

        let pd_table_idx = Self::extract_table_index(
            self.intermediate_tables[pdpt_table_idx][pdpt_idx],
        );

        // 检查 PD 条目
        if self.intermediate_tables[pd_table_idx][pd_idx] == 0 {
            return Err(UnmapError::NotMapped);
        }

        let pt_table_idx = Self::extract_table_index(
            self.intermediate_tables[pd_table_idx][pd_idx],
        );

        // 检查 PT 条目
        let entry = self.intermediate_tables[pt_table_idx][pt_idx];
        if entry == 0 {
            return Err(UnmapError::NotMapped);
        }

        // 提取物理地址（清除标志位）
        let phys = PhysAddr::new(entry & !FLAGS_MASK);

        // 清除映射
        self.intermediate_tables[pt_table_idx][pt_idx] = 0;

        Ok(phys)
    }

    /// 翻译虚拟地址到物理地址
    ///
    /// 如果映射存在，返回对应的物理地址；否则返回 None。
    pub fn translate(&self, virt: VirtAddr) -> Option<PhysAddr> {
        if !virt.is_canonical() {
            return None;
        }

        let addr = virt.as_u64();

        let pml4_idx = Self::index_from_addr(addr, 0);
        let pdpt_idx = Self::index_from_addr(addr, 1);
        let pd_idx = Self::index_from_addr(addr, 2);
        let pt_idx = Self::index_from_addr(addr, 3);
        let page_offset = addr & (PAGE_SIZE as u64 - 1);

        // 检查 PML4 条目
        let pml4_entry = self.pml4[pml4_idx];
        if pml4_entry == 0 {
            return None;
        }

        let pdpt_table_idx = Self::extract_table_index(pml4_entry);

        // 检查 PDPT 条目
        let pdpt_entry = self.intermediate_tables[pdpt_table_idx][pdpt_idx];
        if pdpt_entry == 0 {
            return None;
        }

        let pd_table_idx = Self::extract_table_index(pdpt_entry);

        // 检查 PD 条目
        let pd_entry = self.intermediate_tables[pd_table_idx][pd_idx];
        if pd_entry == 0 {
            return None;
        }

        let pt_table_idx = Self::extract_table_index(pd_entry);

        // 检查 PT 条目
        let pt_entry = self.intermediate_tables[pt_table_idx][pt_idx];
        if pt_entry == 0 {
            return None;
        }

        // 计算物理地址 = 页面基址（清除标志位） + 页内偏移
        let phys_base = pt_entry & !FLAGS_MASK;
        Some(PhysAddr::new(phys_base + page_offset))
    }

    /// 更改映射标志
    ///
    /// 更新指定虚拟地址映射的标志位，保留物理地址不变。
    pub fn update_flags(
        &mut self,
        virt: VirtAddr,
        flags: PageTableFlags,
    ) -> Result<(), MapError> {
        if !virt.is_canonical() || !virt.is_aligned() {
            return Err(MapError::InvalidAddress);
        }

        let addr = virt.as_u64();

        let pml4_idx = Self::index_from_addr(addr, 0);
        let pdpt_idx = Self::index_from_addr(addr, 1);
        let pd_idx = Self::index_from_addr(addr, 2);
        let pt_idx = Self::index_from_addr(addr, 3);

        // 检查 PML4 条目
        if self.pml4[pml4_idx] == 0 {
            return Err(MapError::InvalidAddress);
        }

        let pdpt_table_idx = Self::extract_table_index(self.pml4[pml4_idx]);

        if self.intermediate_tables[pdpt_table_idx][pdpt_idx] == 0 {
            return Err(MapError::InvalidAddress);
        }

        let pd_table_idx = Self::extract_table_index(
            self.intermediate_tables[pdpt_table_idx][pdpt_idx],
        );

        if self.intermediate_tables[pd_table_idx][pd_idx] == 0 {
            return Err(MapError::InvalidAddress);
        }

        let pt_table_idx = Self::extract_table_index(
            self.intermediate_tables[pd_table_idx][pd_idx],
        );

        let entry = self.intermediate_tables[pt_table_idx][pt_idx];
        if entry == 0 {
            return Err(MapError::InvalidAddress);
        }

        // 保留物理地址，更新标志位
        let phys_addr = entry & !FLAGS_MASK;
        self.intermediate_tables[pt_table_idx][pt_idx] =
            phys_addr | (flags.bits() & FLAGS_MASK);

        Ok(())
    }

    /// 获取映射数量
    ///
    /// 遍历所有 PT 表，统计非零条目的数量。
    pub fn mapping_count(&self) -> usize {
        let mut count = 0;

        // 遍历 PML4，找到所有有效的 PDPT 表
        for &pml4_entry in &self.pml4 {
            if pml4_entry == 0 {
                continue;
            }

            let pdpt_table_idx = Self::extract_table_index(pml4_entry);

            // 遍历 PDPT，找到所有有效的 PD 表
            for &pdpt_entry in &self.intermediate_tables[pdpt_table_idx] {
                if pdpt_entry == 0 {
                    continue;
                }

                let pd_table_idx = Self::extract_table_index(pdpt_entry);

                // 遍历 PD，找到所有有效的 PT 表
                for &pd_entry in &self.intermediate_tables[pd_table_idx] {
                    if pd_entry == 0 {
                        continue;
                    }

                    let pt_table_idx = Self::extract_table_index(pd_entry);

                    // 统计 PT 中的非零条目
                    for &pt_entry in &self.intermediate_tables[pt_table_idx] {
                        if pt_entry != 0 {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 14：创建空页表
    #[test]
    fn test_page_table_new() {
        let pt = PageTable::new();
        assert_eq!(pt.pml4.len(), 512);
        // 所有条目应为 0
        for &entry in &pt.pml4 {
            assert_eq!(entry, 0);
        }
        // 中间页表列表应为空
        assert!(pt.intermediate_tables.is_empty());
        // 映射数量应为 0
        assert_eq!(pt.mapping_count(), 0);
    }

    /// 测试 15：映射单个页
    #[test]
    fn test_page_table_map() {
        let mut pt = PageTable::new();

        let virt = VirtAddr::new(0x1000);
        let phys = PhysAddr::new(0x2000);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // 映射应成功
        let result = pt.map(virt, phys, flags);
        assert!(result.is_ok());

        // 映射数量应为 1
        assert_eq!(pt.mapping_count(), 1);

        // 重复映射应失败
        let result2 = pt.map(virt, phys, flags);
        assert_eq!(result2, Err(MapError::AlreadyMapped));
    }

    /// 测试 16：映射后翻译
    #[test]
    fn test_page_table_map_translate() {
        let mut pt = PageTable::new();

        let virt = VirtAddr::new(0x1000);
        let phys = PhysAddr::new(0x2000);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        pt.map(virt, phys, flags).unwrap();

        // 翻译对齐地址
        let translated = pt.translate(virt);
        assert_eq!(translated, Some(phys));

        // 翻译带偏移的地址
        let virt_with_offset = VirtAddr::new(0x1234);
        let translated = pt.translate(virt_with_offset);
        assert_eq!(translated, Some(PhysAddr::new(0x2234)));

        // 翻译未映射的地址
        let unmapped = pt.translate(VirtAddr::new(0x2000));
        assert_eq!(unmapped, None);
    }

    /// 测试 17：取消映射
    #[test]
    fn test_page_table_unmap() {
        let mut pt = PageTable::new();

        let virt = VirtAddr::new(0x1000);
        let phys = PhysAddr::new(0x2000);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        pt.map(virt, phys, flags).unwrap();
        assert_eq!(pt.mapping_count(), 1);

        // 取消映射应返回之前的物理地址
        let unmapped_phys = pt.unmap(virt).unwrap();
        assert_eq!(unmapped_phys, phys);
        assert_eq!(pt.mapping_count(), 0);

        // 翻译应返回 None
        assert_eq!(pt.translate(virt), None);

        // 重复取消映射应失败
        let result = pt.unmap(virt);
        assert_eq!(result, Err(UnmapError::NotMapped));
    }

    /// 测试 18：映射多个页
    #[test]
    fn test_page_table_map_multiple() {
        let mut pt = PageTable::new();

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // 映射多个页（同一 PML4 索引下的不同 PT 条目）
        for i in 0..10u64 {
            let virt = VirtAddr::new(0x1000 * (i + 1));
            let phys = PhysAddr::new(0x100000 + 0x1000 * i);
            pt.map(virt, phys, flags).unwrap();
        }

        assert_eq!(pt.mapping_count(), 10);

        // 验证所有映射
        for i in 0..10u64 {
            let virt = VirtAddr::new(0x1000 * (i + 1));
            let expected = PhysAddr::new(0x100000 + 0x1000 * i);
            assert_eq!(pt.translate(virt), Some(expected));
        }
    }

    /// 测试 19：映射到不同 PML4 索引
    #[test]
    fn test_page_table_map_different_levels() {
        let mut pt = PageTable::new();

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // 在规范地址范围内映射到不同的 PML4 索引：
        // PML4[0]: 低地址空间 0x0 - 0x7F_FFFF_FFFF
        // PML4[256-511]: 高地址空间 0xFFFF_8000_0000_0000 - 0xFFFF_FFFF_FFFF_FFFF

        // 映射到 PML4[0]（低地址空间）
        let virt1 = VirtAddr::new(0x1000);
        let phys1 = PhysAddr::new(0x1000);
        pt.map(virt1, phys1, flags).unwrap();

        // 映射到 PML4[256]（内核空间，0xFFFF_8000_0000_0000 开始）
        let virt2 = VirtAddr::new(0xFFFF_8000_0000_0000);
        let phys2 = PhysAddr::new(0x2000);
        pt.map(virt2, phys2, flags).unwrap();

        // 映射到 PML4[511]（最高地址空间）
        let virt3 = VirtAddr::new(0xFFFF_FFFF_FFFF_F000);
        let phys3 = PhysAddr::new(0x3000);
        pt.map(virt3, phys3, flags).unwrap();

        assert_eq!(pt.mapping_count(), 3);

        // 验证翻译
        assert_eq!(pt.translate(virt1), Some(phys1));
        assert_eq!(pt.translate(virt2), Some(phys2));
        assert_eq!(pt.translate(virt3), Some(phys3));
    }

    /// 测试 20：更新标志
    #[test]
    fn test_page_table_update_flags() {
        let mut pt = PageTable::new();

        let virt = VirtAddr::new(0x1000);
        let phys = PhysAddr::new(0x2000);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        pt.map(virt, phys, flags).unwrap();

        // 更新标志为只读
        let new_flags = PageTableFlags::PRESENT;
        pt.update_flags(virt, new_flags).unwrap();

        // 翻译应仍然工作（物理地址不变）
        assert_eq!(pt.translate(virt), Some(phys));

        // 更新未映射地址的标志应失败
        let result = pt.update_flags(VirtAddr::new(0x2000), new_flags);
        assert_eq!(result, Err(MapError::InvalidAddress));
    }

    /// 测试 21：取消未映射的页
    #[test]
    fn test_page_table_unmap_not_mapped() {
        let mut pt = PageTable::new();

        // 取消未映射的地址应失败
        let result = pt.unmap(VirtAddr::new(0x1000));
        assert_eq!(result, Err(UnmapError::NotMapped));

        // 取消无效地址应失败
        let result = pt.unmap(VirtAddr::new(0x8000_0000_0000)); // 非规范地址
        assert_eq!(result, Err(UnmapError::InvalidAddress));

        // 取消非对齐地址应失败
        let result = pt.unmap(VirtAddr::new(0x1001));
        assert_eq!(result, Err(UnmapError::InvalidAddress));
    }
}
