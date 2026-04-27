//! 调度器错误类型
//!
//! 定义调度器模块中使用的所有错误类型，包括无效任务 ID、
//! 非法状态转换、运行队列溢出等场景。

use core::fmt;

/// 调度器错误类型
///
/// 涵盖任务管理、状态转换、运行队列操作和上下文切换等场景中
/// 可能出现的所有错误情况。
#[derive(Debug, Clone)]
pub enum SchedulerError {
    /// 无效的任务 ID
    InvalidTaskId(u64),
    /// 非法的任务状态转换
    InvalidStateTransition {
        /// 转换前的状态
        from: crate::scheduler::task::TaskState,
        /// 目标状态
        to: crate::scheduler::task::TaskState,
    },
    /// 运行队列已满
    RunQueueFull,
    /// 任务已在运行队列中
    AlreadyEnqueued {
        /// 任务 ID
        task_id: u64,
    },
    /// 任务不在运行队列中
    NotEnqueued {
        /// 任务 ID
        task_id: u64,
    },
    /// 上下文切换失败
    ContextSwitchFailed {
        /// 失败原因
        reason: &'static str,
    },
    /// 无效的优先级类
    InvalidPriorityClass(u8),
    /// 任务未找到
    TaskNotFound(u64),
    /// 没有当前运行的任务
    NoCurrentTask,
    /// 队列为空
    QueueEmpty,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulerError::InvalidTaskId(id) => {
                write!(f, "无效的任务 ID: {}", id)
            }
            SchedulerError::InvalidStateTransition { from, to } => {
                write!(f, "非法的状态转换: {:?} -> {:?}", from, to)
            }
            SchedulerError::RunQueueFull => {
                write!(f, "运行队列已满")
            }
            SchedulerError::AlreadyEnqueued { task_id } => {
                write!(f, "任务 {} 已在运行队列中", task_id)
            }
            SchedulerError::NotEnqueued { task_id } => {
                write!(f, "任务 {} 不在运行队列中", task_id)
            }
            SchedulerError::ContextSwitchFailed { reason } => {
                write!(f, "上下文切换失败: {}", reason)
            }
            SchedulerError::InvalidPriorityClass(val) => {
                write!(f, "无效的优先级类: {}", val)
            }
            SchedulerError::TaskNotFound(id) => {
                write!(f, "任务 {} 未找到", id)
            }
            SchedulerError::NoCurrentTask => {
                write!(f, "没有当前运行的任务")
            }
            SchedulerError::QueueEmpty => {
                write!(f, "队列为空")
            }
        }
    }
}

#[cfg(test)]
impl std::error::Error for SchedulerError {}
