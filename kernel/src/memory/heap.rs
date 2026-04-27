//! 内核堆管理
//!
//! 使用 linked_list_allocator 提供全局堆

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, Ordering};

/// 堆是否已初始化
static HEAP_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 堆起始地址 (将在 init 时设置)
static mut HEAP_START: usize = 0;
/// 堆大小
static mut HEAP_SIZE: usize = 0;
/// 已用字节数
static mut HEAP_USED: usize = 0;

/// 堆对齐
const HEAP_ALIGN: usize = 16;

/// 简单的 bump 分配器 (Phase 1 初始版本)
pub struct BumpAllocator {
    next: AtomicBumpPtr,
}

use core::sync::atomic::AtomicUsize;

struct AtomicBumpPtr(AtomicUsize);

impl AtomicBumpPtr {
    const fn new() -> Self { Self(AtomicUsize::new(0)) }
    fn get(&self) -> usize { self.0.load(Ordering::Relaxed) }
    fn set(&self, val: usize) { self.0.store(val, Ordering::Relaxed); }
    fn fetch_add(&self, val: usize) -> usize { self.0.fetch_add(val, Ordering::Relaxed) }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !HEAP_INITIALIZED.load(Ordering::Relaxed) {
            return core::ptr::null_mut();
        }
        let align = layout.align().max(HEAP_ALIGN);
        let size = layout.size();
        let heap_start = unsafe { HEAP_START };
        let heap_end = unsafe { HEAP_START + HEAP_SIZE };

        let current = self.next.fetch_add(size);
        let current_abs = heap_start + current;
        let aligned = (current_abs + align - 1) & !(align - 1);
        let end = aligned + size;

        if end > heap_end {
            return core::ptr::null_mut();
        }
        unsafe {
            HEAP_USED = self.next.get();
        }
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator does not deallocate (sufficient for Phase 1)
    }
}

/// 全局堆分配器实例 (仅在 no_std / 非测试环境下使用)
#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator { next: AtomicBumpPtr::new() };

/// 测试环境下也提供分配器实例供直接调用测试
#[cfg(test)]
static ALLOCATOR: BumpAllocator = BumpAllocator { next: AtomicBumpPtr::new() };

/// 初始化内核堆
pub fn init_heap(start_addr: usize, size: usize) {
    unsafe {
        HEAP_START = start_addr;
        HEAP_SIZE = size;
        HEAP_USED = 0;
    }
    // Reset the bump pointer too
    ALLOCATOR.next.set(0);
    HEAP_INITIALIZED.store(true, Ordering::Relaxed);
}

/// 获取堆使用统计
pub fn heap_stats() -> (usize, usize) {
    unsafe { (HEAP_SIZE, HEAP_USED) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 所有堆测试合并到一个函数中以避免并行测试导致的全局状态竞争
    #[test]
    fn test_heap_all() {
        // Test 1: heap not initialized
        assert!(!HEAP_INITIALIZED.load(Ordering::Relaxed));

        // Test 2: init_heap
        let mut backing = [0u8; 4096];
        init_heap(backing.as_mut_ptr() as usize, 4096);
        assert!(HEAP_INITIALIZED.load(Ordering::Relaxed));
        let (size, _used) = heap_stats();
        assert_eq!(size, 4096);

        // Test 3: bump allocator basic allocation
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!ptr.is_null());
        let (size, used) = heap_stats();
        assert!(used > 0);
        assert!(used <= size);

        // Test 4: bump allocator multiple allocations
        // Re-init to get clean state
        init_heap(backing.as_mut_ptr() as usize, 4096);
        let layout1 = Layout::from_size_align(32, 8).unwrap();
        let layout2 = Layout::from_size_align(64, 16).unwrap();
        let p1 = unsafe { ALLOCATOR.alloc(layout1) };
        let p2 = unsafe { ALLOCATOR.alloc(layout2) };
        assert!(!p1.is_null());
        assert!(!p2.is_null());
        assert_ne!(p1, p2);

        // Test 5: bump allocator exhaust
        let mut small_backing = [0u8; 64];
        init_heap(small_backing.as_mut_ptr() as usize, 64);
        let big_layout = Layout::from_size_align(128, 8).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(big_layout) };
        assert!(ptr.is_null()); // Too large

        // Cleanup
        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }
}
