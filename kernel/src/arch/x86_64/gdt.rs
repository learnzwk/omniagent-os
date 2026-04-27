//! GDT (Global Descriptor Table) 初始化
//!
//! 建立内核代码段/数据段和用户代码段/数据段

use core::mem::size_of;

/// GDT 表项数量
pub const GDT_ENTRY_COUNT: usize = 6;

/// 段选择子索引
pub const KERNEL_CODE_SELECTOR: u16 = 1 << 3;
pub const KERNEL_DATA_SELECTOR: u16 = 2 << 3;
pub const USER_CODE_SELECTOR: u16 = 3 << 3;
pub const USER_DATA_SELECTOR: u16 = 4 << 3;

/// GDT 表项 (8 字节)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GdtEntry {
    pub limit_low: u16,
    pub base_low: u16,
    pub base_middle: u8,
    pub access_byte: u8,
    pub flags_and_limit_high: u8,
    pub base_high: u8,
}

impl GdtEntry {
    pub const fn new(base: u32, limit: u32, access: u8, flags: u8) -> Self {
        Self {
            base_low: (base & 0xFFFF) as u16,
            base_middle: ((base >> 16) & 0xFF) as u8,
            base_high: ((base >> 24) & 0xFF) as u8,
            limit_low: (limit & 0xFFFF) as u16,
            access_byte: access,
            flags_and_limit_high: ((limit >> 16) & 0x0F) as u8 | ((flags & 0x0F) << 4),
        }
    }

    /// 内核代码段: Ring 0, 可执行可读
    pub const fn kernel_code() -> Self {
        Self::new(0, 0xFFFF, 0x9A, 0x0A) // G=1, D/B=1, L=0, AVL=0
    }

    /// 内核数据段: Ring 0, 可读写
    pub const fn kernel_data() -> Self {
        Self::new(0, 0xFFFF, 0x92, 0x0C) // G=1, D/B=1
    }

    /// 用户代码段: Ring 3, 可执行可读
    pub const fn user_code() -> Self {
        Self::new(0, 0xFFFF, 0xFA, 0x0A)
    }

    /// 用户数据段: Ring 3, 可读写
    pub const fn user_data() -> Self {
        Self::new(0, 0xFFFF, 0xF2, 0x0C)
    }
}

/// GDT 结构
#[repr(C)]
pub struct Gdt {
    entries: [GdtEntry; GDT_ENTRY_COUNT],
}

impl Gdt {
    pub const fn new() -> Self {
        Self {
            entries: [
                GdtEntry::new(0, 0, 0, 0), // Null descriptor
                GdtEntry::kernel_code(),
                GdtEntry::kernel_data(),
                GdtEntry::user_code(),
                GdtEntry::user_data(),
                GdtEntry::new(0, 0, 0, 0), // Reserved for TSS
            ],
        }
    }

    pub fn entries(&self) -> &[GdtEntry] {
        &self.entries
    }
}

/// 全局 GDT 实例
static GDT: Gdt = Gdt::new();

/// GDT 指针 (用于 lgdt 指令)
#[repr(C)]
pub struct GdtPointer {
    pub limit: u16,
    pub base: u64,
}

impl GdtPointer {
    pub fn new() -> Self {
        Self {
            limit: (GDT_ENTRY_COUNT * size_of::<GdtEntry>() - 1) as u16,
            base: &GDT as *const Gdt as u64,
        }
    }
}

/// 加载 GDT
#[cfg(not(test))]
pub unsafe fn load_gdt() {
    let pointer = GdtPointer::new();
    core::arch::asm!(
        "lgdt [{}]",
        "push {kernel_sel}",
        "lea {1:f}, [rip + 2]",
        "push QWORD PTR [rsp]",
        "retfq",
        in(reg) &pointer,
        kernel_sel = const KERNEL_CODE_SELECTOR,
        options(nostack),
    );
}

/// 加载 GDT (test stub - no-op)
#[cfg(test)]
pub unsafe fn load_gdt() {
    // 在测试环境中不需要实际加载 GDT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gdt_entry_count() {
        assert_eq!(GDT_ENTRY_COUNT, 6);
    }

    #[test]
    fn test_gdt_entry_size() {
        assert_eq!(size_of::<GdtEntry>(), 8);
    }

    #[test]
    fn test_null_descriptor() {
        let null = GdtEntry::new(0, 0, 0, 0);
        let limit_low = null.limit_low;
        let access_byte = null.access_byte;
        assert_eq!(limit_low, 0);
        assert_eq!(access_byte, 0);
    }

    #[test]
    fn test_kernel_code_segment() {
        let kc = GdtEntry::kernel_code();
        let access_byte = kc.access_byte;
        assert_eq!(access_byte, 0x9A);
    }

    #[test]
    fn test_kernel_data_segment() {
        let kd = GdtEntry::kernel_data();
        let access_byte = kd.access_byte;
        assert_eq!(access_byte, 0x92);
    }

    #[test]
    fn test_user_segments_dpl() {
        let uc = GdtEntry::user_code();
        let ud = GdtEntry::user_data();
        // DPL = 3 means bits 5-6 = 11 = 0x60
        let uc_access = uc.access_byte;
        let ud_access = ud.access_byte;
        assert_eq!(uc_access & 0x60, 0x60);
        assert_eq!(ud_access & 0x60, 0x60);
    }

    #[test]
    fn test_gdt_pointer_limit() {
        let ptr = GdtPointer::new();
        assert_eq!(ptr.limit, (GDT_ENTRY_COUNT * 8 - 1) as u16);
    }

    #[test]
    fn test_selector_values() {
        assert_eq!(KERNEL_CODE_SELECTOR, 0x08);
        assert_eq!(KERNEL_DATA_SELECTOR, 0x10);
        assert_eq!(USER_CODE_SELECTOR, 0x18);
        assert_eq!(USER_DATA_SELECTOR, 0x20);
    }
}
