//! 位图帧分配器
//!
//! 每一位代表一个 4KB 物理帧

use crate::memory::physical::PhysFrame;

const BITS_PER_U64: usize = 64;

/// 位图帧分配器
pub struct BitmapFrameAllocator {
    bitmap: &'static mut [u64],
    frame_count: usize,
    next_free: usize,
}

impl BitmapFrameAllocator {
    pub fn new(bitmap: &'static mut [u64], frame_count: usize) -> Self {
        // Clear all bits (all frames free)
        for word in bitmap.iter_mut() {
            *word = 0;
        }
        Self { bitmap, frame_count, next_free: 0 }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        for i in self.next_free..self.frame_count {
            let word_idx = i / BITS_PER_U64;
            let bit_idx = i % BITS_PER_U64;
            if word_idx >= self.bitmap.len() { return None; }
            if self.bitmap[word_idx] & (1 << bit_idx) == 0 {
                self.bitmap[word_idx] |= 1 << bit_idx;
                self.next_free = i + 1;
                return Some(PhysFrame::new(i));
            }
        }
        // Wrap around
        for i in 0..self.next_free {
            let word_idx = i / BITS_PER_U64;
            let bit_idx = i % BITS_PER_U64;
            if word_idx >= self.bitmap.len() { return None; }
            if self.bitmap[word_idx] & (1 << bit_idx) == 0 {
                self.bitmap[word_idx] |= 1 << bit_idx;
                self.next_free = i + 1;
                return Some(PhysFrame::new(i));
            }
        }
        None
    }

    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        let i = frame.number;
        if i >= self.frame_count { return; }
        let word_idx = i / BITS_PER_U64;
        let bit_idx = i % BITS_PER_U64;
        if word_idx < self.bitmap.len() {
            self.bitmap[word_idx] &= !(1 << bit_idx);
            if i < self.next_free { self.next_free = i; }
        }
    }

    pub fn is_allocated(&self, frame: PhysFrame) -> bool {
        let i = frame.number;
        if i >= self.frame_count { return false; }
        let word_idx = i / BITS_PER_U64;
        let bit_idx = i % BITS_PER_U64;
        if word_idx >= self.bitmap.len() { return false; }
        (self.bitmap[word_idx] & (1 << bit_idx)) != 0
    }

    pub fn usable_frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn allocated_count(&self) -> usize {
        let mut count = 0;
        for word in self.bitmap.iter() {
            count += word.count_ones() as usize;
        }
        count.min(self.frame_count)
    }

    pub fn free_count(&self) -> usize {
        self.frame_count - self.allocated_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn make_allocator(frame_count: usize) -> BitmapFrameAllocator {
        let word_count = (frame_count + BITS_PER_U64 - 1) / BITS_PER_U64;
        let bitmap_storage: Box<[u64; 1024]> = Box::new([0u64; 1024]); // Support up to 65536 frames
        let bitmap: &'static mut [u64] = Box::leak(bitmap_storage);
        BitmapFrameAllocator::new(&mut bitmap[..word_count], frame_count)
    }

    #[test]
    fn test_allocate_single_frame() {
        let mut alloc = make_allocator(100);
        let frame = alloc.allocate_frame().expect("should allocate");
        assert_eq!(frame.number, 0);
        assert!(alloc.is_allocated(frame));
    }

    #[test]
    fn test_allocate_deallocate_cycle() {
        let mut alloc = make_allocator(100);
        let frame = alloc.allocate_frame().unwrap();
        assert!(alloc.is_allocated(frame));
        alloc.deallocate_frame(frame);
        assert!(!alloc.is_allocated(frame));
        let frame2 = alloc.allocate_frame().unwrap();
        assert_eq!(frame2.number, frame.number);
    }

    #[test]
    fn test_allocate_multiple() {
        let mut alloc = make_allocator(100);
        let f0 = alloc.allocate_frame().unwrap();
        let f1 = alloc.allocate_frame().unwrap();
        let f2 = alloc.allocate_frame().unwrap();
        assert_eq!(f0.number, 0);
        assert_eq!(f1.number, 1);
        assert_eq!(f2.number, 2);
    }

    #[test]
    fn test_exhaust_all_frames() {
        let mut alloc = make_allocator(10);
        for _ in 0..10 {
            assert!(alloc.allocate_frame().is_some());
        }
        assert!(alloc.allocate_frame().is_none());
    }

    #[test]
    fn test_allocated_free_count() {
        let mut alloc = make_allocator(100);
        assert_eq!(alloc.free_count(), 100);
        assert_eq!(alloc.allocated_count(), 0);
        alloc.allocate_frame().unwrap();
        alloc.allocate_frame().unwrap();
        assert_eq!(alloc.allocated_count(), 2);
        assert_eq!(alloc.free_count(), 98);
    }

    #[test]
    fn test_deallocate_nonexistent() {
        let mut alloc = make_allocator(100);
        alloc.deallocate_frame(PhysFrame::new(999)); // Should not panic
    }

    #[test]
    fn test_is_allocated_unallocated() {
        let alloc = make_allocator(100);
        assert!(!alloc.is_allocated(PhysFrame::new(50)));
    }

    /// 测试：释放后重分配应返回相同帧号
    #[test]
    fn test_deallocate_and_reallocate() {
        let mut alloc = make_allocator(20);

        // 分配多个帧
        let f0 = alloc.allocate_frame().unwrap();
        let f1 = alloc.allocate_frame().unwrap();
        let f2 = alloc.allocate_frame().unwrap();

        // 释放中间的帧
        alloc.deallocate_frame(f1);
        assert!(!alloc.is_allocated(f1));

        // 重分配应优先使用刚释放的帧（next_free 回退）
        let f_re = alloc.allocate_frame().unwrap();
        assert_eq!(f_re.number, f1.number);
    }

    /// 测试：连续分配所有帧后释放再分配
    #[test]
    fn test_exhaust_deallocate_reallocate() {
        let mut alloc = make_allocator(5);

        // 分配所有帧
        let frames: Vec<_> = (0..5).map(|_| alloc.allocate_frame().unwrap()).collect();
        assert!(alloc.allocate_frame().is_none());

        // 释放所有帧
        for f in &frames {
            alloc.deallocate_frame(*f);
        }
        assert_eq!(alloc.allocated_count(), 0);
        assert_eq!(alloc.free_count(), 5);

        // 重新分配所有帧
        for _ in 0..5 {
            assert!(alloc.allocate_frame().is_some());
        }
        assert!(alloc.allocate_frame().is_none());
    }

    /// 测试：帧号跨越多个 u64 字
    #[test]
    fn test_cross_word_boundary() {
        let mut alloc = make_allocator(130); // 超过 2 个 u64 字（128 位）

        // 分配第 64 帧和第 65 帧（跨越字边界）
        let mut frames = Vec::new();
        for _ in 0..65 {
            frames.push(alloc.allocate_frame().unwrap());
        }

        // 验证帧号正确
        assert_eq!(frames[63].number, 63);
        assert_eq!(frames[64].number, 64);

        // 释放第 64 帧
        alloc.deallocate_frame(frames[64]);
        assert!(!alloc.is_allocated(frames[64]));

        // 重分配应返回第 64 帧
        let re = alloc.allocate_frame().unwrap();
        assert_eq!(re.number, 64);
    }

    /// 测试：usable_frame_count 返回正确值
    #[test]
    fn test_usable_frame_count() {
        let alloc = make_allocator(256);
        assert_eq!(alloc.usable_frame_count(), 256);
    }

    /// 测试：释放超出范围的帧不应 panic
    #[test]
    fn test_deallocate_out_of_range() {
        let mut alloc = make_allocator(10);

        // 释放超出 frame_count 的帧
        alloc.deallocate_frame(PhysFrame::new(10));
        alloc.deallocate_frame(PhysFrame::new(100));
        alloc.deallocate_frame(PhysFrame::new(65535));

        // 分配器应仍然正常工作
        let frame = alloc.allocate_frame().unwrap();
        assert_eq!(frame.number, 0);
    }

    /// 测试：is_allocated 对超出范围的帧返回 false
    #[test]
    fn test_is_allocated_out_of_range() {
        let alloc = make_allocator(10);

        assert!(!alloc.is_allocated(PhysFrame::new(10)));
        assert!(!alloc.is_allocated(PhysFrame::new(100)));
        assert!(!alloc.is_allocated(PhysFrame::new(65535)));
    }

    /// 测试：分配和释放交替操作
    #[test]
    fn test_interleaved_alloc_dealloc() {
        let mut alloc = make_allocator(10);

        let f1 = alloc.allocate_frame().unwrap();
        let f2 = alloc.allocate_frame().unwrap();
        alloc.deallocate_frame(f1);
        let f3 = alloc.allocate_frame().unwrap();
        assert_eq!(f3.number, f1.number);

        alloc.deallocate_frame(f2);
        let f4 = alloc.allocate_frame().unwrap();
        assert_eq!(f4.number, f2.number);

        assert_eq!(alloc.allocated_count(), 2);
        assert_eq!(alloc.free_count(), 8);
    }
}
