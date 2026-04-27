//! 虚拟内存管理模块
//!
//! 提供虚拟地址、物理地址、页表条目、页表管理和地址空间等核心抽象，
//! 用于 OmniAgent OS 内核的虚拟内存管理。

pub mod addr;
pub mod pte;
pub mod page_table;
pub mod address_space;

pub use addr::{VirtAddr, PhysAddr, PageNum};
pub use pte::{PageTableEntry, PageTableFlags};
pub use page_table::{PageTable, MapError, UnmapError};
pub use address_space::{AddressSpace, VmArea, VmFlags, AddressSpaceKind};

/// 页大小（4KB）
pub const PAGE_SIZE: usize = 4096;
/// 页大小的位数（2^12 = 4096）
pub const PAGE_SIZE_BITS: u32 = 12;
