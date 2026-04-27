//! 全局调度器
//!
//! 实现基于 CFS（Completely Fair Scheduler）风格的全局调度器，
//! 支持多优先级类、虚拟运行时间（vruntime）公平调度、
//! 任务创建/退出/睡眠/唤醒等完整的调度生命周期管理。
//!
//! 全局实例使用 `spin::Lazy<Mutex<Scheduler>>` 提供，
//! 通过便捷函数进行访问。

use alloc::collections::BTreeMap;
use spin::{Lazy, Mutex};

use crate::scheduler::error::SchedulerError;
use crate::scheduler::priority::PriorityClass;
use crate::scheduler::run_queue::RunQueue;
use crate::scheduler::task::{TaskControlBlock, TaskId, TaskFlags, TaskState};

/// 全局调度器实例
///
/// 使用 spin::Lazy 延迟初始化，spin::Mutex 提供无 std 依赖的互斥锁。
pub static SCHEDULER: Lazy<Mutex<Scheduler>> = Lazy::new(|| {
    Mutex::new(Scheduler::new())
});

/// 调度器统计信息
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    /// 总调度次数
    pub total_schedules: u64,
    /// 上下文切换次数
    pub context_switches: u64,
    /// 抢占次数
    pub preemptions: u64,
    /// 任务总数
    pub task_count: usize,
    /// 就绪队列中的任务数
    pub ready_count: u32,
}

/// 全局调度器
///
/// 管理所有任务的生命周期，维护运行队列，执行调度决策。
pub struct Scheduler {
    /// 运行队列
    run_queue: RunQueue,
    /// 任务表：TaskId -> TCB
    tasks: BTreeMap<u64, TaskControlBlock>,
    /// 下一个可用的任务 ID
    next_task_id: u64,
    /// 空闲任务 ID
    idle_task_id: Option<TaskId>,
    /// 总调度次数
    total_schedules: u64,
    /// 上下文切换次数
    context_switches: u64,
    /// 抢占次数
    preemptions: u64,
}

impl Scheduler {
    /// 创建新的调度器
    pub fn new() -> Self {
        Scheduler {
            run_queue: RunQueue::new(),
            tasks: BTreeMap::new(),
            next_task_id: 1,
            idle_task_id: None,
            total_schedules: 0,
            context_switches: 0,
            preemptions: 0,
        }
    }

    /// 初始化调度器
    ///
    /// 创建空闲任务（优先级 Idle），在没有其他可运行任务时调度。
    pub fn init(&mut self) {
        let idle_id = self.next_task_id;
        self.next_task_id += 1;

        let mut idle_tcb = TaskControlBlock::new(
            TaskId(idle_id),
            0, // 空闲任务入口点（占位）
            0, // 空闲任务栈顶（占位）
            PriorityClass::Idle,
            false,
            None,
        );
        idle_tcb.flags.insert(TaskFlags::IS_IDLE);
        idle_tcb.state = TaskState::Ready;

        self.tasks.insert(idle_id, idle_tcb);
        self.idle_task_id = Some(TaskId(idle_id));
    }

    /// 创建新任务
    ///
    /// 分配任务 ID，创建 TCB，初始状态为 Created。
    /// 调用者需要手动调用 `enqueue` 将任务加入运行队列。
    pub fn create_task(
        &mut self,
        entry: u64,
        stack_top: u64,
        priority: PriorityClass,
        is_user: bool,
        agent_handle: Option<u64>,
    ) -> Result<TaskId, SchedulerError> {
        let id = self.next_task_id;
        self.next_task_id += 1;

        let tcb = TaskControlBlock::new(
            TaskId(id),
            entry,
            stack_top,
            priority,
            is_user,
            agent_handle,
        );

        self.tasks.insert(id, tcb);
        Ok(TaskId(id))
    }

    /// 将任务加入运行队列
    ///
    /// 任务状态必须为 Created 或 Ready，加入后状态变为 Ready。
    pub fn enqueue(&mut self, task_id: TaskId) -> Result<(), SchedulerError> {
        let task = self.tasks.get_mut(&task_id.0)
            .ok_or(SchedulerError::TaskNotFound(task_id.0))?;

        // 验证状态转换
        if !task.state.can_transition(TaskState::Ready) {
            return Err(SchedulerError::InvalidStateTransition {
                from: task.state,
                to: TaskState::Ready,
            });
        }

        task.state = TaskState::Ready;
        let task_clone = task.clone();
        self.run_queue.enqueue(&task_clone);
        Ok(())
    }

    /// 从运行队列中移除任务
    pub fn dequeue(&mut self, task_id: TaskId) -> Result<(), SchedulerError> {
        let task = self.tasks.get(&task_id.0)
            .ok_or(SchedulerError::TaskNotFound(task_id.0))?;

        if task.state != TaskState::Ready && task.state != TaskState::Running {
            return Err(SchedulerError::NotEnqueued { task_id: task_id.0 });
        }

        self.run_queue.dequeue(task_id);
        if let Some(t) = self.tasks.get_mut(&task_id.0) {
            t.state = TaskState::Ready;
        }
        Ok(())
    }

    /// 选择下一个要运行的任务
    ///
    /// 从运行队列中取出 vruntime 最小的任务（优先考虑高优先级类）。
    /// 将选中任务的状态设为 Running。
    pub fn pick_next(&mut self) -> Option<TaskId> {
        let task_id = self.run_queue.pick_next_task()?;
        if let Some(task) = self.tasks.get_mut(&task_id.0) {
            task.state = TaskState::Running;
        }
        Some(task_id)
    }

    /// 当前任务主动让出 CPU
    ///
    /// 将当前任务重新加入运行队列，选择下一个任务运行。
    pub fn yield_now(&mut self) {
        if let Some(current_id) = self.run_queue.current() {
            if let Some(task) = self.tasks.get_mut(&current_id.0) {
                if task.state == TaskState::Running {
                    task.state = TaskState::Ready;
                    let task_clone = task.clone();
                    self.run_queue.enqueue(&task_clone);
                    self.run_queue.set_current(None);
                }
            }
        }
        // 选择下一个任务
        if let Some(next_id) = self.run_queue.pick_next_task() {
            if let Some(task) = self.tasks.get_mut(&next_id.0) {
                task.state = TaskState::Running;
                self.context_switches += 1;
            }
        }
    }

    /// 使任务进入睡眠状态
    ///
    /// 任务从运行队列中移除，状态变为 Blocked。
    /// `channel` 用于标识等待的事件/资源。
    pub fn sleep(&mut self, task_id: TaskId, channel: u64) {
        if let Some(task) = self.tasks.get_mut(&task_id.0) {
            if task.state == TaskState::Running || task.state == TaskState::Ready {
                task.state = TaskState::Blocked;
                task.wait_channel = channel;
                self.run_queue.dequeue(task_id);
            }
        }
    }

    /// 唤醒睡眠中的任务
    ///
    /// 将 Blocked 状态的任务重新加入运行队列。
    pub fn wake_up(&mut self, task_id: TaskId) {
        if let Some(task) = self.tasks.get_mut(&task_id.0) {
            if task.state == TaskState::Blocked {
                task.state = TaskState::Ready;
                task.wait_channel = 0;
                let task_clone = task.clone();
                self.run_queue.enqueue(&task_clone);
            }
        }
    }

    /// 任务退出
    ///
    /// 将任务状态设为 Zombie，从运行队列中移除。
    pub fn exit(&mut self, task_id: TaskId, exit_code: i32) {
        if let Some(task) = self.tasks.get_mut(&task_id.0) {
            task.state = TaskState::Zombie;
            task.exit_code = exit_code;
            task.flags.insert(TaskFlags::EXITED);
            self.run_queue.dequeue(task_id);
        }
    }

    /// 执行调度决策
    ///
    /// 如果当前任务需要重新调度（NEED_RESCHED 标志）或没有当前任务，
    /// 则从运行队列中选择下一个任务运行。
    pub fn schedule(&mut self) {
        self.total_schedules += 1;

        let current_id = self.run_queue.current();

        // 检查当前任务是否需要让出
        let need_resched = if let Some(id) = current_id {
            if let Some(task) = self.tasks.get(&id.0) {
                task.flags.contains(TaskFlags::NEED_RESCHED)
            } else {
                true
            }
        } else {
            true
        };

        if need_resched {
            // 如果有当前任务且它还在运行，将其放回队列
            if let Some(id) = current_id {
                if let Some(task) = self.tasks.get_mut(&id.0) {
                    if task.state == TaskState::Running {
                        task.state = TaskState::Ready;
                        task.flags.remove(TaskFlags::NEED_RESCHED);
                        let task_clone = task.clone();
                        self.run_queue.enqueue(&task_clone);
                        self.preemptions += 1;
                    }
                }
                self.run_queue.set_current(None);
            }

            // 选择下一个任务
            if let Some(next_id) = self.run_queue.pick_next_task() {
                if let Some(task) = self.tasks.get_mut(&next_id.0) {
                    task.state = TaskState::Running;
                    self.context_switches += 1;
                }
            }
        }
    }

    /// 获取当前运行任务的不可变引用
    pub fn current_task(&self) -> Option<&TaskControlBlock> {
        let current_id = self.run_queue.current()?;
        self.tasks.get(&current_id.0)
    }

    /// 获取当前运行任务的可变引用
    pub fn current_task_mut(&mut self) -> Option<&mut TaskControlBlock> {
        let current_id = self.run_queue.current()?;
        self.tasks.get_mut(&current_id.0)
    }

    /// 获取指定任务的不可变引用
    pub fn get_task(&self, id: TaskId) -> Option<&TaskControlBlock> {
        self.tasks.get(&id.0)
    }

    /// 定时器 tick 处理
    ///
    /// 更新当前任务的 vruntime，检查是否需要抢占。
    /// 每次调用模拟一个 tick（默认 1ms）。
    pub fn timer_tick(&mut self) {
        if let Some(current_id) = self.run_queue.current() {
            if let Some(task) = self.tasks.get_mut(&current_id.0) {
                if task.state == TaskState::Running {
                    // 模拟 1ms 的运行时间
                    let delta_ns: u64 = 1_000_000;
                    task.sched_info.update_vruntime(delta_ns);

                    // 减少剩余时间片
                    if task.sched_info.time_slice_remain > delta_ns {
                        task.sched_info.time_slice_remain -= delta_ns;
                    } else {
                        // 时间片用完，设置需要重新调度标志
                        task.sched_info.time_slice_remain = 0;
                        task.flags.insert(TaskFlags::NEED_RESCHED);
                    }
                }
            }
        }
    }

    /// 获取任务总数
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// 获取调度器统计信息
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            total_schedules: self.total_schedules,
            context_switches: self.context_switches,
            preemptions: self.preemptions,
            task_count: self.tasks.len(),
            ready_count: self.run_queue.nr_running(),
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 全局便捷函数
// ============================================================================

/// 初始化全局调度器
pub fn init() {
    SCHEDULER.lock().init();
}

/// 创建新任务
pub fn create_task(
    entry: u64,
    stack_top: u64,
    priority: PriorityClass,
    is_user: bool,
    agent_handle: Option<u64>,
) -> Result<TaskId, SchedulerError> {
    SCHEDULER.lock().create_task(entry, stack_top, priority, is_user, agent_handle)
}

/// 执行调度
pub fn schedule() {
    SCHEDULER.lock().schedule();
}

/// 当前任务主动让出 CPU
pub fn yield_now() {
    SCHEDULER.lock().yield_now();
}

/// 获取当前运行任务的 ID
pub fn current_task_id() -> Option<TaskId> {
    SCHEDULER.lock().run_queue.current()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用调度器的辅助函数
    fn make_scheduler() -> Scheduler {
        Scheduler::new()
    }

    /// 测试 20：创建任务
    #[test]
    fn test_scheduler_create_task() {
        let mut sched = make_scheduler();

        let id = sched.create_task(
            0x1000,
            0x8000,
            PriorityClass::Normal,
            true,
            Some(42),
        ).unwrap();

        assert_eq!(id, TaskId(1));
        assert_eq!(sched.task_count(), 1);

        // 验证任务属性
        let task = sched.get_task(id).unwrap();
        assert_eq!(task.id, TaskId(1));
        assert_eq!(task.state, TaskState::Created);
        assert_eq!(task.agent_handle, Some(42));
        assert_eq!(task.context.rip, 0x1000);
        assert_eq!(task.context.rsp, 0x8000);

        // 创建第二个任务，ID 应递增
        let id2 = sched.create_task(0x2000, 0x9000, PriorityClass::High, false, None).unwrap();
        assert_eq!(id2, TaskId(2));
        assert_eq!(sched.task_count(), 2);
    }

    /// 测试 21：入队并选择任务
    #[test]
    fn test_scheduler_enqueue_pick() {
        let mut sched = make_scheduler();

        // 创建并入队任务
        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id1).unwrap();

        let id2 = sched.create_task(0x2000, 0x9000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id2).unwrap();

        // 选择下一个任务
        let next = sched.pick_next().unwrap();
        assert_eq!(next, id1); // vruntime 相同，先入队的先出

        // 验证任务状态变为 Running
        let task = sched.get_task(next).unwrap();
        assert_eq!(task.state, TaskState::Running);

        // 再选一个
        let next2 = sched.pick_next().unwrap();
        assert_eq!(next2, id2);
    }

    /// 测试 22：主动让出 CPU
    #[test]
    fn test_scheduler_yield() {
        let mut sched = make_scheduler();

        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id1).unwrap();

        let id2 = sched.create_task(0x2000, 0x9000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id2).unwrap();

        // 选择第一个任务
        let next = sched.pick_next().unwrap();
        assert_eq!(next, id1);

        // 给当前任务一些运行时间以增加 vruntime
        sched.timer_tick();
        sched.timer_tick();

        // 当前任务让出
        sched.yield_now();

        // 让出后应调度另一个任务
        let current = sched.run_queue.current();
        assert!(current.is_some());
        // 当前任务应该是 id2（因为 id1 的 vruntime 已增加）
        assert_ne!(current.unwrap(), id1);
    }

    /// 测试 23：睡眠与唤醒
    #[test]
    fn test_scheduler_sleep_wakeup() {
        let mut sched = make_scheduler();

        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id1).unwrap();

        // 选择任务使其运行
        sched.pick_next().unwrap();
        assert_eq!(sched.get_task(id1).unwrap().state, TaskState::Running);

        // 睡眠
        sched.sleep(id1, 42);
        assert_eq!(sched.get_task(id1).unwrap().state, TaskState::Blocked);
        assert_eq!(sched.get_task(id1).unwrap().wait_channel, 42);

        // 运行队列中不应有任务
        assert_eq!(sched.run_queue.nr_running(), 0);

        // 唤醒
        sched.wake_up(id1);
        assert_eq!(sched.get_task(id1).unwrap().state, TaskState::Ready);
        assert_eq!(sched.get_task(id1).unwrap().wait_channel, 0);
        assert_eq!(sched.run_queue.nr_running(), 1);
    }

    /// 测试 24：任务退出
    #[test]
    fn test_scheduler_exit() {
        let mut sched = make_scheduler();

        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id1).unwrap();
        sched.pick_next().unwrap();

        // 任务退出
        sched.exit(id1, 0);
        let task = sched.get_task(id1).unwrap();
        assert_eq!(task.state, TaskState::Zombie);
        assert_eq!(task.exit_code, 0);
        assert!(task.flags.contains(TaskFlags::EXITED));

        // 任务仍在任务表中
        assert_eq!(sched.task_count(), 1);
    }

    /// 测试 25：定时器 tick
    #[test]
    fn test_scheduler_timer_tick() {
        let mut sched = make_scheduler();

        let id = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id).unwrap();
        sched.pick_next().unwrap();

        // 初始状态
        let task = sched.get_task(id).unwrap();
        assert_eq!(task.sched_info.vruntime, 0);
        assert_eq!(task.sched_info.runtime, 0);

        // 模拟多个 tick
        for _ in 0..3 {
            sched.timer_tick();
        }

        // 验证 vruntime 和 runtime 更新
        let task = sched.get_task(id).unwrap();
        // Normal 权重 1024，每个 tick 1ms = 1_000_000ns
        // delta_vruntime = 1_000_000 * 1024 / 1024 = 1_000_000
        // 3 ticks: vruntime = 3_000_000, runtime = 3_000_000
        assert_eq!(task.sched_info.vruntime, 3_000_000);
        assert_eq!(task.sched_info.runtime, 3_000_000);

        // 时间片应减少
        assert!(task.sched_info.time_slice_remain < PriorityClass::Normal.base_time_slice_ns());
    }

    /// 测试 26：多任务调度
    #[test]
    fn test_scheduler_multiple_tasks() {
        let mut sched = make_scheduler();

        // 创建多个不同优先级的任务
        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        let id2 = sched.create_task(0x2000, 0x9000, PriorityClass::High, true, None).unwrap();
        let id3 = sched.create_task(0x3000, 0xA000, PriorityClass::Agent, true, None).unwrap();

        sched.enqueue(id1).unwrap();
        sched.enqueue(id2).unwrap();
        sched.enqueue(id3).unwrap();

        // 应优先选择高优先级任务
        let next = sched.pick_next().unwrap();
        assert_eq!(next, id2); // High 优先级最高

        let next = sched.pick_next().unwrap();
        assert_eq!(next, id3); // Agent 次之

        let next = sched.pick_next().unwrap();
        assert_eq!(next, id1); // Normal 最后
    }

    /// 测试 27：调度器统计信息
    #[test]
    fn test_scheduler_stats() {
        let mut sched = make_scheduler();

        // 初始统计
        let stats = sched.stats();
        assert_eq!(stats.total_schedules, 0);
        assert_eq!(stats.context_switches, 0);
        assert_eq!(stats.preemptions, 0);
        assert_eq!(stats.task_count, 0);
        assert_eq!(stats.ready_count, 0);

        // 创建并入队任务
        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id1).unwrap();

        // 调度
        sched.schedule();
        let stats = sched.stats();
        assert_eq!(stats.total_schedules, 1);
        assert_eq!(stats.context_switches, 1);
        assert_eq!(stats.task_count, 1);
    }

    /// 测试 28：调度器初始化（创建空闲任务）
    #[test]
    fn test_scheduler_init() {
        let mut sched = make_scheduler();

        // 初始化前没有空闲任务
        assert!(sched.idle_task_id.is_none());

        // 初始化
        sched.init();

        // 应创建空闲任务
        assert!(sched.idle_task_id.is_some());
        let idle_id = sched.idle_task_id.unwrap();
        assert_eq!(idle_id, TaskId(1));

        // 验证空闲任务属性
        let idle_task = sched.get_task(idle_id).unwrap();
        assert_eq!(idle_task.state, TaskState::Ready);
        assert!(idle_task.flags.contains(TaskFlags::IS_IDLE));
        assert_eq!(idle_task.sched_info.priority, PriorityClass::Idle);
    }

    /// 测试 29：当前任务查询
    #[test]
    fn test_scheduler_current_task() {
        let mut sched = make_scheduler();

        // 初始没有当前任务
        assert!(sched.current_task().is_none());

        // 创建并入队任务
        let id = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        sched.enqueue(id).unwrap();
        sched.pick_next().unwrap();

        // 现在应有当前任务
        let current = sched.current_task().unwrap();
        assert_eq!(current.id, id);
        assert_eq!(current.state, TaskState::Running);
    }

    /// 测试 30：多任务公平性（vruntime 增长验证）
    #[test]
    fn test_scheduler_fairness() {
        let mut sched = make_scheduler();

        // 创建两个 Normal 优先级的任务
        let id1 = sched.create_task(0x1000, 0x8000, PriorityClass::Normal, true, None).unwrap();
        let id2 = sched.create_task(0x2000, 0x9000, PriorityClass::Normal, true, None).unwrap();

        sched.enqueue(id1).unwrap();
        sched.enqueue(id2).unwrap();

        // 选择第一个任务并运行 5 个 tick
        sched.pick_next().unwrap();
        for _ in 0..5 {
            sched.timer_tick();
        }

        let task1_vruntime = sched.get_task(id1).unwrap().sched_info.vruntime;

        // 让出并选择第二个任务（yield_now 内部会自动选择下一个任务）
        sched.yield_now();

        // 第二个任务也运行 5 个 tick
        for _ in 0..5 {
            sched.timer_tick();
        }

        let task2_vruntime = sched.get_task(id2).unwrap().sched_info.vruntime;

        // 两个任务的 vruntime 应该相同（相同优先级、相同运行时间）
        assert_eq!(task1_vruntime, task2_vruntime,
            "相同优先级和运行时间的任务 vruntime 应相等: {} vs {}",
            task1_vruntime, task2_vruntime);

        // 验证具体数值：5ms * 1024 / 1024 = 5_000_000
        assert_eq!(task1_vruntime, 5_000_000);
        assert_eq!(task2_vruntime, 5_000_000);
    }
}
