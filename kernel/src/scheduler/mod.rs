//! 调度器模块
//!
//! 提供 OmniAgent OS 内核的 CFS 风格调度器实现，包括：
//! - 错误类型定义
//! - 优先级系统和调度信息
//! - 任务控制块（TCB）
//! - 运行队列管理
//! - 全局调度器

pub mod error;
pub mod priority;
pub mod task;
pub mod run_queue;
pub mod scheduler;

// 重新导出常用类型
pub use error::SchedulerError;
pub use priority::{PriorityClass, SchedInfo};
pub use task::{TaskId, TaskState, TaskFlags, ContextFrame, TaskControlBlock};
pub use run_queue::RunQueue;
pub use scheduler::{Scheduler, SchedulerStats, SCHEDULER, init, create_task, schedule, yield_now, current_task_id};
