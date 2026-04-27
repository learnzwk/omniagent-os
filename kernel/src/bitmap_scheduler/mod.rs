//! 优先级位图调度器模块
//!
//! 模仿鸿蒙内核的优先级位图快速调度机制。
//! 使用 32 位位图实现 O(1) 的最高优先级任务选择。

pub mod scheduler;

pub use scheduler::{BitmapScheduler, SchedulerError, BITMAP_SCHEDULER};
