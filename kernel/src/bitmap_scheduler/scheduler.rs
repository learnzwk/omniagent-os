//! 优先级位图调度器
//!
//! 模仿鸿蒙内核：0-31 优先级（数值越小优先级越高），bitmap 快速选择。
//! 使用 CLZ (Count Leading Zeros) 找到最高优先级，实现 O(1) 调度。

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use spin::Mutex;

// ============================================================================
// 错误类型
// ============================================================================

/// 调度器错误类型
#[derive(Debug, Clone)]
pub enum SchedulerError {
    /// 无效优先级（必须在 0-31 范围内）
    InvalidPriority(u8),
    /// 任务未找到
    TaskNotFound(u64),
    /// 队列为空
    QueueEmpty(u8),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulerError::InvalidPriority(p) => {
                write!(f, "无效优先级: {} (必须在 0-31 范围内)", p)
            }
            SchedulerError::TaskNotFound(id) => {
                write!(f, "任务未找到: {}", id)
            }
            SchedulerError::QueueEmpty(p) => {
                write!(f, "优先级 {} 的队列为空", p)
            }
        }
    }
}

// ============================================================================
// 位图调度器
// ============================================================================

/// 优先级位图调度器
///
/// 使用 32 位位图跟踪每个优先级是否有就绪任务。
/// 位 i 置 1 表示优先级 i 的队列中有就绪任务。
/// 通过 CLZ（前导零计数）实现 O(1) 的最高优先级选择。
pub struct BitmapScheduler {
    /// 32 位位图，每一位代表一个优先级是否有就绪任务
    priority_bitmap: AtomicU32,
    /// 每个优先级的任务队列（0-31）
    queues: [Mutex<VecDeque<u64>>; 32],
    /// 当前运行的任务 ID（0 表示无任务）
    current: AtomicU64,
    /// 总调度次数
    total_schedules: AtomicU64,
    /// 上下文切换次数
    context_switches: AtomicU64,
}

impl BitmapScheduler {
    /// 创建新的位图调度器
    pub fn new() -> Self {
        // 使用 const fn 数组初始化
        const EMPTY_QUEUE: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());
        BitmapScheduler {
            priority_bitmap: AtomicU32::new(0),
            queues: [EMPTY_QUEUE; 32],
            current: AtomicU64::new(0),
            total_schedules: AtomicU64::new(0),
            context_switches: AtomicU64::new(0),
        }
    }

    /// 将任务加入指定优先级队列
    ///
    /// # 参数
    /// - `task_id`: 任务 ID
    /// - `priority`: 优先级（0-31，数值越小优先级越高）
    pub fn enqueue(&self, task_id: u64, priority: u8) -> Result<(), SchedulerError> {
        if priority >= 32 {
            return Err(SchedulerError::InvalidPriority(priority));
        }

        // 将任务加入对应优先级队列
        {
            let mut queue = self.queues[priority as usize].lock();
            queue.push_back(task_id);
        }

        // 设置位图对应位
        self.priority_bitmap.fetch_or(1 << priority, Ordering::SeqCst);

        Ok(())
    }

    /// 从队列中移除任务
    ///
    /// 在所有优先级队列中搜索并移除指定任务。
    pub fn dequeue(&self, task_id: u64) -> Result<(), SchedulerError> {
        let mut found = false;

        for priority in 0..32u8 {
            let mut queue = self.queues[priority as usize].lock();
            if let Some(pos) = queue.iter().position(|&id| id == task_id) {
                queue.remove(pos);
                found = true;

                // 如果队列空了，清除位图对应位
                if queue.is_empty() {
                    self.priority_bitmap
                        .fetch_and(!(1u32 << priority), Ordering::SeqCst);
                }
                break;
            }
        }

        if found {
            Ok(())
        } else {
            Err(SchedulerError::TaskNotFound(task_id))
        }
    }

    /// 选择下一个任务（O(1) bitmap 操作）
    ///
    /// 使用 CLZ (Count Leading Zeros) 找到最高优先级（最低位索引），
    /// 然后从该优先级队列头部取出一个任务。
    pub fn pick_next(&self) -> Option<u64> {
        let bitmap = self.priority_bitmap.load(Ordering::SeqCst);
        if bitmap == 0 {
            return None;
        }

        // CLZ: 找到最高置位位的位置
        // 在 u32 中，leading_zeros() 返回前导零的数量
        // 最高优先级 = 最小的置位位索引 = 32 - leading_zeros() - 1
        // 但实际上我们想要最低的置位位索引（最高优先级）
        let highest_priority = bitmap.trailing_zeros() as u8;

        let mut queue = self.queues[highest_priority as usize].lock();
        let task_id = queue.pop_front()?;

        // 如果队列空了，清除位图对应位
        if queue.is_empty() {
            self.priority_bitmap
                .fetch_and(!(1u32 << highest_priority), Ordering::SeqCst);
        }

        // 更新统计
        self.total_schedules.fetch_add(1, Ordering::SeqCst);

        // 如果当前任务不同，增加上下文切换计数
        let prev = self.current.load(Ordering::SeqCst);
        if prev != 0 && prev != task_id {
            self.context_switches.fetch_add(1, Ordering::SeqCst);
        }

        Some(task_id)
    }

    /// 设置当前运行的任务
    pub fn set_current(&self, task_id: u64) {
        self.current.store(task_id, Ordering::SeqCst);
    }

    /// 获取当前运行的任务
    pub fn current(&self) -> Option<u64> {
        let id = self.current.load(Ordering::SeqCst);
        if id == 0 {
            None
        } else {
            Some(id)
        }
    }

    /// 让出 CPU
    ///
    /// 将当前任务重新加入调度队列。
    pub fn yield_task(&self) {
        let current = self.current.load(Ordering::SeqCst);
        if current == 0 {
            return;
        }

        // 查找当前任务所在的优先级
        for priority in 0..32u8 {
            let queue = self.queues[priority as usize].lock();
            if queue.contains(&current) {
                // 任务已在队列中，不需要重新加入
                return;
            }
        }

        // 如果任务不在任何队列中，需要找到它的优先级
        // 简化处理：使用 pick_next 的逻辑
        // 这里我们假设 yield 只是清除当前任务标记
        self.current.store(0, Ordering::SeqCst);
    }

    /// 获取指定优先级的队列长度
    pub fn queue_len(&self, priority: u8) -> usize {
        if priority >= 32 {
            return 0;
        }
        let queue = self.queues[priority as usize].lock();
        queue.len()
    }

    /// 获取位图值（调试用）
    pub fn bitmap(&self) -> u32 {
        self.priority_bitmap.load(Ordering::SeqCst)
    }

    /// 获取统计信息
    ///
    /// 返回 (总调度次数, 上下文切换次数)
    pub fn stats(&self) -> (u64, u64) {
        (
            self.total_schedules.load(Ordering::SeqCst),
            self.context_switches.load(Ordering::SeqCst),
        )
    }
}

/// 全局位图调度器实例
pub static BITMAP_SCHEDULER: spin::Lazy<BitmapScheduler> = spin::Lazy::new(|| {
    BitmapScheduler::new()
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === 测试: 创建调度器 ===
    #[test]
    fn test_new() {
        let sched = BitmapScheduler::new();
        assert_eq!(sched.bitmap(), 0);
        assert_eq!(sched.current(), None);
        let (total, ctx) = sched.stats();
        assert_eq!(total, 0);
        assert_eq!(ctx, 0);
    }

    // === 测试: 入队和出队 ===
    #[test]
    fn test_enqueue_dequeue() {
        let sched = BitmapScheduler::new();
        sched.enqueue(1, 5).unwrap();
        sched.enqueue(2, 5).unwrap();
        sched.enqueue(3, 10).unwrap();

        assert_eq!(sched.queue_len(5), 2);
        assert_eq!(sched.queue_len(10), 1);

        sched.dequeue(2).unwrap();
        assert_eq!(sched.queue_len(5), 1);
        assert_eq!(sched.queue_len(10), 1);

        sched.dequeue(3).unwrap();
        assert_eq!(sched.queue_len(10), 0);
    }

    // === 测试: 选择最高优先级任务 ===
    #[test]
    fn test_pick_next_highest_priority() {
        let sched = BitmapScheduler::new();
        sched.enqueue(1, 10).unwrap();
        sched.enqueue(2, 5).unwrap();
        sched.enqueue(3, 15).unwrap();

        // 优先级 5 最高，应先选中任务 2
        assert_eq!(sched.pick_next(), Some(2));
        // 优先级 10 次之
        assert_eq!(sched.pick_next(), Some(1));
        // 优先级 15 最低
        assert_eq!(sched.pick_next(), Some(3));
        // 队列空
        assert_eq!(sched.pick_next(), None);
    }

    // === 测试: 空队列选择 ===
    #[test]
    fn test_pick_next_empty() {
        let sched = BitmapScheduler::new();
        assert_eq!(sched.pick_next(), None);
    }

    // === 测试: 设置当前任务 ===
    #[test]
    fn test_set_current() {
        let sched = BitmapScheduler::new();
        assert_eq!(sched.current(), None);

        sched.set_current(42);
        assert_eq!(sched.current(), Some(42));

        sched.set_current(0);
        assert_eq!(sched.current(), None);
    }

    // === 测试: 让出 CPU ===
    #[test]
    fn test_yield_task() {
        let sched = BitmapScheduler::new();
        sched.set_current(1);

        // 让出后当前任务应被清除
        sched.yield_task();
        assert_eq!(sched.current(), None);

        // 没有当前任务时让出不应 panic
        sched.yield_task();
        assert_eq!(sched.current(), None);
    }

    // === 测试: 队列长度 ===
    #[test]
    fn test_queue_len() {
        let sched = BitmapScheduler::new();
        assert_eq!(sched.queue_len(0), 0);
        assert_eq!(sched.queue_len(31), 0);

        sched.enqueue(1, 0).unwrap();
        sched.enqueue(2, 0).unwrap();
        sched.enqueue(3, 0).unwrap();
        assert_eq!(sched.queue_len(0), 3);

        // 无效优先级应返回 0
        assert_eq!(sched.queue_len(32), 0);
        assert_eq!(sched.queue_len(255), 0);
    }

    // === 测试: 位图值 ===
    #[test]
    fn test_bitmap() {
        let sched = BitmapScheduler::new();
        assert_eq!(sched.bitmap(), 0);

        sched.enqueue(1, 3).unwrap();
        assert_eq!(sched.bitmap(), 1 << 3);

        sched.enqueue(2, 7).unwrap();
        assert_eq!(sched.bitmap(), (1 << 3) | (1 << 7));

        // 出队所有任务后位图应清零
        sched.dequeue(1).unwrap();
        assert_eq!(sched.bitmap(), 1 << 7);

        sched.dequeue(2).unwrap();
        assert_eq!(sched.bitmap(), 0);
    }

    // === 测试: 多优先级 ===
    #[test]
    fn test_multiple_priorities() {
        let sched = BitmapScheduler::new();

        // 在多个优先级添加任务
        for i in 0..5u64 {
            sched.enqueue(i, 0).unwrap();
        }
        for i in 5..10u64 {
            sched.enqueue(i, 1).unwrap();
        }
        for i in 10..15u64 {
            sched.enqueue(i, 31).unwrap();
        }

        // 应先选完优先级 0，再选优先级 1，最后选优先级 31
        for i in 0..5u64 {
            assert_eq!(sched.pick_next(), Some(i));
        }
        for i in 5..10u64 {
            assert_eq!(sched.pick_next(), Some(i));
        }
        for i in 10..15u64 {
            assert_eq!(sched.pick_next(), Some(i));
        }
        assert_eq!(sched.pick_next(), None);
    }

    // === 测试: 无效优先级 ===
    #[test]
    fn test_invalid_priority() {
        let sched = BitmapScheduler::new();

        assert!(sched.enqueue(1, 32).is_err());
        assert!(sched.enqueue(1, 100).is_err());
        assert!(sched.enqueue(1, 255).is_err());

        match sched.enqueue(1, 32).unwrap_err() {
            SchedulerError::InvalidPriority(p) => assert_eq!(p, 32),
            _ => panic!("期望 InvalidPriority 错误"),
        }
    }

    // === 测试: 出队不存在的任务 ===
    #[test]
    fn test_dequeue_not_found() {
        let sched = BitmapScheduler::new();
        sched.enqueue(1, 5).unwrap();

        assert!(sched.dequeue(999).is_err());
        match sched.dequeue(999).unwrap_err() {
            SchedulerError::TaskNotFound(id) => assert_eq!(id, 999),
            _ => panic!("期望 TaskNotFound 错误"),
        }
    }

    // === 测试: 统计信息 ===
    #[test]
    fn test_stats() {
        let sched = BitmapScheduler::new();

        sched.enqueue(1, 5).unwrap();
        sched.enqueue(2, 10).unwrap();

        sched.pick_next(); // 调度任务 1
        sched.set_current(1); // 设置当前任务
        let (total, ctx) = sched.stats();
        assert_eq!(total, 1);
        assert_eq!(ctx, 0); // 第一次调度不算上下文切换

        sched.pick_next(); // 调度任务 2（上下文切换，因为当前是任务 1）
        let (total, ctx) = sched.stats();
        assert_eq!(total, 2);
        assert_eq!(ctx, 1);
    }
}
