//! 地址类型定义
//!
//! 提供虚拟地址、物理地址和页号的封装类型，用于虚拟内存管理。

/// 页大小（4KB）
pub const PAGE_SIZE: usize = 4096;
/// 页大小的位数（2^12 = 4096）
pub const PAGE_SIZE_BITS: u32 = 12;
/// x86_64 符号扩展位（第47位）
const SIGN_EXTEND_BIT: u64 = 1 << 47;

/// 虚拟地址（48位有效）
///
/// x86_64 架构使用 48 位虚拟地址，高 16 位必须进行符号扩展。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct VirtAddr(pub u64);

impl VirtAddr {
    /// 创建新的虚拟地址
    pub fn new(addr: u64) -> Self {
        VirtAddr(addr)
    }

    /// 获取页内偏移（0-4095）
    pub fn page_offset(&self) -> u64 {
        self.0 & (PAGE_SIZE as u64 - 1)
    }

    /// 获取页号
    pub fn page_number(&self) -> PageNum {
        PageNum(self.0 >> PAGE_SIZE_BITS)
    }

    /// 向上页对齐
    pub fn align_up(&self) -> Self {
        VirtAddr((self.0 + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1))
    }

    /// 向下页对齐
    pub fn align_down(&self) -> Self {
        VirtAddr(self.0 & !(PAGE_SIZE as u64 - 1))
    }

    /// 检查地址是否页对齐
    pub fn is_aligned(&self) -> bool {
        self.0 & (PAGE_SIZE as u64 - 1) == 0
    }

    /// 检查地址是否在规范地址范围内
    ///
    /// x86_64 的规范地址范围是 0x0000_0000_0000_0000 ~ 0x0000_7FFF_FFFF_FFFF
    /// 和 0xFFFF_8000_0000_0000 ~ 0xFFFF_FFFF_FFFF_FFFF
    pub fn is_canonical(&self) -> bool {
        let addr = self.0;
        // 检查高16位是否全部为符号扩展
        if addr & SIGN_EXTEND_BIT == 0 {
            // 正半部分：高16位必须全为0
            addr & 0xFFFF_0000_0000_0000 == 0
        } else {
            // 负半部分：高16位必须全为1
            addr & 0xFFFF_0000_0000_0000 == 0xFFFF_0000_0000_0000
        }
    }

    /// 获取原始 u64 值
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// 转换为裸指针
    pub fn as_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }

    /// 从裸指针创建虚拟地址
    pub fn from_ptr(ptr: *mut u8) -> Self {
        VirtAddr(ptr as u64)
    }
}

/// 物理地址
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct PhysAddr(pub u64);

impl PhysAddr {
    /// 创建新的物理地址
    pub fn new(addr: u64) -> Self {
        PhysAddr(addr)
    }

    /// 获取页内偏移（0-4095）
    pub fn page_offset(&self) -> u64 {
        self.0 & (PAGE_SIZE as u64 - 1)
    }

    /// 获取页号
    pub fn page_number(&self) -> PageNum {
        PageNum(self.0 >> PAGE_SIZE_BITS)
    }

    /// 向上页对齐
    pub fn align_up(&self) -> Self {
        PhysAddr((self.0 + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1))
    }

    /// 向下页对齐
    pub fn align_down(&self) -> Self {
        PhysAddr(self.0 & !(PAGE_SIZE as u64 - 1))
    }

    /// 获取原始 u64 值
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// 转换为裸指针
    pub fn as_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }
}

/// 页号
///
/// 表示一个 4KB 页的编号，可用于索引页表。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct PageNum(pub u64);

impl PageNum {
    /// 创建新的页号
    pub fn new(num: u64) -> Self {
        PageNum(num)
    }

    /// 获取该页的起始物理地址
    pub fn start_address(&self) -> PhysAddr {
        PhysAddr(self.0 << PAGE_SIZE_BITS)
    }

    /// 获取偏移后的页号
    pub fn offset(&self, offset: u64) -> PageNum {
        PageNum(self.0 + offset)
    }

    /// 获取原始 u64 值
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 1：页内偏移计算
    #[test]
    fn test_virt_addr_page_offset() {
        // 对齐地址的偏移应为 0
        let addr = VirtAddr::new(0x1000);
        assert_eq!(addr.page_offset(), 0);

        // 非对齐地址的偏移
        let addr = VirtAddr::new(0x1234);
        assert_eq!(addr.page_offset(), 0x234);

        // 最大偏移
        let addr = VirtAddr::new(0x1FFF);
        assert_eq!(addr.page_offset(), 0xFFF);

        // 零地址的偏移
        let addr = VirtAddr::new(0);
        assert_eq!(addr.page_offset(), 0);
    }

    /// 测试 2：页号计算
    #[test]
    fn test_virt_addr_page_number() {
        // 0 地址的页号为 0
        let addr = VirtAddr::new(0);
        assert_eq!(addr.page_number(), PageNum(0));

        // 0x1000 的页号为 1
        let addr = VirtAddr::new(0x1000);
        assert_eq!(addr.page_number(), PageNum(1));

        // 0x1234 的页号为 1（向下取整）
        let addr = VirtAddr::new(0x1234);
        assert_eq!(addr.page_number(), PageNum(1));

        // 0x2000 的页号为 2
        let addr = VirtAddr::new(0x2000);
        assert_eq!(addr.page_number(), PageNum(2));
    }

    /// 测试 3：向上页对齐
    #[test]
    fn test_virt_addr_align_up() {
        // 已对齐的地址不变
        let addr = VirtAddr::new(0x1000);
        assert_eq!(addr.align_up(), VirtAddr::new(0x1000));

        // 未对齐的地址向上对齐
        let addr = VirtAddr::new(0x1001);
        assert_eq!(addr.align_up(), VirtAddr::new(0x2000));

        // 0 地址不变
        let addr = VirtAddr::new(0);
        assert_eq!(addr.align_up(), VirtAddr::new(0));

        // 边界情况：0xFFF 向上对齐到 0x1000
        let addr = VirtAddr::new(0xFFF);
        assert_eq!(addr.align_up(), VirtAddr::new(0x1000));
    }

    /// 测试 4：向下页对齐
    #[test]
    fn test_virt_addr_align_down() {
        // 已对齐的地址不变
        let addr = VirtAddr::new(0x1000);
        assert_eq!(addr.align_down(), VirtAddr::new(0x1000));

        // 未对齐的地址向下对齐
        let addr = VirtAddr::new(0x1234);
        assert_eq!(addr.align_down(), VirtAddr::new(0x1000));

        // 0 地址不变
        let addr = VirtAddr::new(0);
        assert_eq!(addr.align_down(), VirtAddr::new(0));

        // 0xFFF 向下对齐到 0
        let addr = VirtAddr::new(0xFFF);
        assert_eq!(addr.align_down(), VirtAddr::new(0));
    }

    /// 测试 5：对齐检查
    #[test]
    fn test_virt_addr_is_aligned() {
        // 对齐的地址
        assert!(VirtAddr::new(0).is_aligned());
        assert!(VirtAddr::new(0x1000).is_aligned());
        assert!(VirtAddr::new(0x2000).is_aligned());
        assert!(VirtAddr::new(0x100000).is_aligned());

        // 未对齐的地址
        assert!(!VirtAddr::new(1).is_aligned());
        assert!(!VirtAddr::new(0x100).is_aligned());
        assert!(!VirtAddr::new(0x1234).is_aligned());
        assert!(!VirtAddr::new(0xFFF).is_aligned());
    }

    /// 测试 6：规范地址检查
    #[test]
    fn test_virt_addr_is_canonical() {
        // 低半部分有效地址
        assert!(VirtAddr::new(0).is_canonical());
        assert!(VirtAddr::new(0x1000).is_canonical());
        assert!(VirtAddr::new(0x7FFF_FFFF_FFFF).is_canonical());

        // 高半部分有效地址（内核空间）
        assert!(VirtAddr::new(0xFFFF_8000_0000_0000).is_canonical());
        assert!(VirtAddr::new(0xFFFF_FFFF_FFFF_FFFF).is_canonical());

        // 非规范地址
        assert!(!VirtAddr::new(0x8000_0000_0000).is_canonical());
        assert!(!VirtAddr::new(0x0001_0000_0000_0000).is_canonical());
        assert!(!VirtAddr::new(0xFFFF_7FFF_FFFF_FFFF).is_canonical());
    }

    /// 测试 7：物理地址转换
    #[test]
    fn test_phys_addr_conversions() {
        // as_u64
        let addr = PhysAddr::new(0x1000);
        assert_eq!(addr.as_u64(), 0x1000);

        // page_offset
        let addr = PhysAddr::new(0x1234);
        assert_eq!(addr.page_offset(), 0x234);

        // page_number
        let addr = PhysAddr::new(0x1234);
        assert_eq!(addr.page_number(), PageNum(1));

        // align_up
        let addr = PhysAddr::new(0x1001);
        assert_eq!(addr.align_up(), PhysAddr::new(0x2000));

        // align_down
        let addr = PhysAddr::new(0x1234);
        assert_eq!(addr.align_down(), PhysAddr::new(0x1000));

        // as_ptr
        let addr = PhysAddr::new(0x1000);
        assert_eq!(addr.as_ptr() as u64, 0x1000);
    }

    /// 测试 8：页号到起始地址
    #[test]
    fn test_page_num_start_address() {
        // 页号 0 的起始地址为 0
        assert_eq!(PageNum(0).start_address(), PhysAddr::new(0));

        // 页号 1 的起始地址为 0x1000
        assert_eq!(PageNum(1).start_address(), PhysAddr::new(0x1000));

        // 页号 256 的起始地址为 0x100000
        assert_eq!(PageNum(256).start_address(), PhysAddr::new(0x100000));

        // offset
        assert_eq!(PageNum(10).offset(5), PageNum(15));

        // as_u64
        assert_eq!(PageNum(42).as_u64(), 42);
    }
}
