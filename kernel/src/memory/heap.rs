//! 内核堆管理
//!
//! 使用 BumpAllocator 提供全局堆

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// 堆是否已初始化
static HEAP_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 堆起始地址 (将在 init 时设置)
static HEAP_START: AtomicUsize = AtomicUsize::new(0);
/// 堆大小
static HEAP_SIZE: AtomicUsize = AtomicUsize::new(0);
/// 已用字节数
static HEAP_USED: AtomicUsize = AtomicUsize::new(0);

/// 堆对齐
const HEAP_ALIGN: usize = 16;

/// 简单的 bump 分配器 (Phase 1 初始版本)
pub struct BumpAllocator {
    next: AtomicBumpPtr,
}

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
        let heap_start = HEAP_START.load(Ordering::SeqCst);
        let heap_end = HEAP_START.load(Ordering::SeqCst) + HEAP_SIZE.load(Ordering::SeqCst);

        let current = self.next.fetch_add(size);
        let current_abs = heap_start + current;
        let aligned = (current_abs + align - 1) & !(align - 1);
        let end = aligned + size;

        if end > heap_end {
            return core::ptr::null_mut();
        }
        HEAP_USED.store(self.next.get(), Ordering::SeqCst);
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
    HEAP_START.store(start_addr, Ordering::SeqCst);
    HEAP_SIZE.store(size, Ordering::SeqCst);
    HEAP_USED.store(0, Ordering::SeqCst);
    // Reset the bump pointer too
    ALLOCATOR.next.set(0);
    HEAP_INITIALIZED.store(true, Ordering::Relaxed);
}

/// 获取堆使用统计
pub fn heap_stats() -> (usize, usize) {
    (HEAP_SIZE.load(Ordering::SeqCst), HEAP_USED.load(Ordering::SeqCst))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

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

    /// 测试：未初始化时分配应返回空指针
    #[test]
    fn test_alloc_before_init() {
        // 确保堆未初始化
        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
        ALLOCATOR.next.set(0);

        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(ptr.is_null(), "未初始化时分配应返回空指针");
    }

    /// 测试：初始化后统计信息应正确
    #[test]
    fn test_init_stats() {
        let backing = [0u8; 8192];
        init_heap(backing.as_ptr() as usize, 8192);

        let (size, used) = heap_stats();
        assert_eq!(size, 8192);
        assert_eq!(used, 0, "初始化后已用字节数应为 0");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：多次连续分配
    #[test]
    fn test_multiple_allocations() {
        let backing = [0u8; 4096];
        init_heap(backing.as_ptr() as usize, 4096);

        let mut ptrs = Vec::new();
        for i in 0..10 {
            let layout = Layout::from_size_align(32, 8).unwrap();
            let ptr = unsafe { ALLOCATOR.alloc(layout) };
            assert!(!ptr.is_null(), "第 {} 次分配不应返回空指针", i);
            ptrs.push(ptr);
        }

        // 所有分配的指针应该互不相同
        for i in 0..ptrs.len() {
            for j in (i + 1)..ptrs.len() {
                assert_ne!(ptrs[i], ptrs[j], "不同分配应返回不同指针");
            }
        }

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：高对齐分配
    #[test]
    fn test_aligned_allocation() {
        let backing = [0u8; 4096];
        init_heap(backing.as_ptr() as usize, 4096);

        // 请求 64 字节对齐
        let layout = Layout::from_size_align(128, 64).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!ptr.is_null());
        let addr = ptr as usize;
        assert_eq!(addr % 64, 0, "分配的地址应满足 64 字节对齐");

        // 请求 128 字节对齐
        let layout2 = Layout::from_size_align(256, 128).unwrap();
        let ptr2 = unsafe { ALLOCATOR.alloc(layout2) };
        assert!(!ptr2.is_null());
        let addr2 = ptr2 as usize;
        assert_eq!(addr2 % 128, 0, "分配的地址应满足 128 字节对齐");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：分配后统计验证
    #[test]
    fn test_alloc_updates_stats() {
        let backing = [0u8; 4096];
        init_heap(backing.as_ptr() as usize, 4096);

        let (size, used_before) = heap_stats();
        assert_eq!(used_before, 0);

        let layout = Layout::from_size_align(100, 8).unwrap();
        unsafe { ALLOCATOR.alloc(layout); };

        let (_, used_after) = heap_stats();
        assert!(used_after >= 100, "已用字节数应至少为分配的大小");
        assert!(used_after <= size, "已用字节数不应超过总大小");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：空间不足时分配应返回空指针
    #[test]
    fn test_alloc_exhaust_space() {
        let backing = [0u8; 128];
        init_heap(backing.as_ptr() as usize, 128);

        // 分配接近全部空间
        let layout = Layout::from_size_align(100, 8).unwrap();
        let ptr1 = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!ptr1.is_null());

        // 再次分配应失败（剩余空间不足）
        let layout2 = Layout::from_size_align(100, 8).unwrap();
        let ptr2 = unsafe { ALLOCATOR.alloc(layout2) };
        assert!(ptr2.is_null(), "空间不足时分配应返回空指针");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：初始化边界 - 零大小堆
    #[test]
    fn test_init_zero_size() {
        let backing = [0u8; 1];
        init_heap(backing.as_ptr() as usize, 0);

        let (size, used) = heap_stats();
        assert_eq!(size, 0);

        // 任何分配都应失败
        let layout = Layout::from_size_align(1, 1).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(ptr.is_null(), "零大小堆上分配应返回空指针");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：重新初始化堆应重置状态
    #[test]
    fn test_reinit_resets_state() {
        let backing = [0u8; 4096];
        init_heap(backing.as_ptr() as usize, 4096);

        // 分配一些内存
        let layout = Layout::from_size_align(64, 8).unwrap();
        unsafe { ALLOCATOR.alloc(layout); };
        let (_, used) = heap_stats();
        assert!(used > 0);

        // 重新初始化
        init_heap(backing.as_ptr() as usize, 4096);
        let (_, used_after) = heap_stats();
        assert_eq!(used_after, 0, "重新初始化后已用字节数应为 0");

        // 重新初始化后应能正常分配
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!ptr.is_null(), "重新初始化后应能正常分配");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：释放操作不应导致 panic（bump allocator 不支持释放）
    #[test]
    fn test_dealloc_noop() {
        let backing = [0u8; 4096];
        init_heap(backing.as_ptr() as usize, 4096);

        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!ptr.is_null());

        // 释放不应 panic
        unsafe { ALLOCATOR.dealloc(ptr, layout); }

        // 释放后统计不应减少
        let (_, used) = heap_stats();
        assert!(used > 0, "bump allocator 释放后已用字节数不应减少");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：分配零大小内存
    #[test]
    fn test_alloc_zero_size() {
        let backing = [0u8; 4096];
        init_heap(backing.as_ptr() as usize, 4096);

        let layout = Layout::from_size_align(0, 1).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        // 零大小分配可能返回非空指针或空指针，但不应 panic
        // 不做严格断言，只确保不崩溃
        let _ = ptr;

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }

    /// 测试：分配后指针在堆范围内
    #[test]
    fn test_alloc_ptr_in_range() {
        let backing = [0u8; 4096];
        let heap_start = backing.as_ptr() as usize;
        let heap_end = heap_start + 4096;
        init_heap(heap_start, 4096);

        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!ptr.is_null());

        let addr = ptr as usize;
        assert!(addr >= heap_start && addr < heap_end, "分配的指针应在堆范围内");

        HEAP_INITIALIZED.store(false, Ordering::Relaxed);
    }
}
