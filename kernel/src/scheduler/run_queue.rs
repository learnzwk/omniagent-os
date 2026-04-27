//! 运行队列
//!
//! 实现基于优先级的运行队列，每个优先级类维护一棵基于 vruntime 的
//! BTreeMap 红黑树。调度器从最高优先级的非空树中选择 vruntime 最小的任务。
//!
//! 数据结构：
//! - `PriorityTree`：单个优先级类的 vruntime 红黑树
//! - `RunQueue`：包含 5 棵优先级树（对应 5 个优先级类）的运行队列

use alloc::collections::BTreeMap;

use crate::scheduler::task::{TaskControlBlock, TaskId};

/// 优先级树
///
/// 使用 BTreeMap 以 vruntime 为键组织任务，支持 O(log n) 的
/// 入队、出队和查找操作。
///
/// 键：vruntime（虚拟运行时间）
/// 值：TaskId（任务 ID）
pub struct PriorityTree {
    /// vruntime -> TaskId 的映射
    tree: BTreeMap<u64, u64>,
    /// 树中的任务数量
    nr_tasks: u32,
    /// 树中所有任务的总权重
    total_weight: u64,
}

impl PriorityTree {
    /// 创建空的优先级树
    pub fn new() -> Self {
        PriorityTree {
            tree: BTreeMap::new(),
            nr_tasks: 0,
            total_weight: 0,
        }
    }

    /// 将任务加入优先级树
    ///
    /// 如果存在相同 vruntime 的任务，会在 vruntime 后附加任务 ID 的低位
    /// 以确保唯一性。
    pub fn enqueue(&mut self, vruntime: u64, task_id: u64, weight: u32) {
        // 处理 vruntime 冲突：将 task_id 的低 32 位合并到 vruntime 中
        let mut key = vruntime;
        while self.tree.contains_key(&key) {
            // 在 vruntime 的低位添加偏移以避免冲突
            key = vruntime.wrapping_add(task_id & 0xFFFF);
        }
        self.tree.insert(key, task_id);
        self.nr_tasks += 1;
        self.total_weight += weight as u64;
    }

    /// 取出 vruntime 最小的任务
    ///
    /// 返回 (vruntime_key, task_id)，如果树为空则返回 None。
    pub fn dequeue_min(&mut self) -> Option<(u64, u64)> {
        let (&key, &task_id) = self.tree.first_key_value()?;
        self.tree.remove(&key);
        self.nr_tasks -= 1;
        Some((key, task_id))
    }

    /// 从树中移除指定任务
    ///
    /// 返回被移除任务的 vruntime key，如果任务不存在则返回 None。
    pub fn remove(&mut self, task_id: u64) -> Option<u64> {
        // 查找并移除指定 task_id
        let key = self.tree.iter()
            .find(|(_, &id)| id == task_id)
            .map(|(&k, _)| k)?;

        self.tree.remove(&key);
        self.nr_tasks -= 1;
        Some(key)
    }

    /// 检查树是否为空
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// 获取树中的任务数量
    pub fn len(&self) -> usize {
        self.nr_tasks as usize
    }

    /// 获取树中所有任务的总权重
    pub fn total_weight(&self) -> u64 {
        self.total_weight
    }
}

impl Default for PriorityTree {
    fn default() -> Self {
        Self::new()
    }
}

/// 运行队列
///
/// 管理所有可运行任务的队列，包含 5 棵优先级树（对应 5 个优先级类）。
/// 调度器从最高优先级的非空树中选择 vruntime 最小的任务执行。
pub struct RunQueue {
    /// 当前正在运行的任务
    current: Option<TaskId>,
    /// 每个优先级类一棵树（索引 0=Idle, 1=Normal, 2=Agent, 3=High, 4=Realtime）
    trees: [PriorityTree; 5],
    /// 全局最小 vruntime（用于新任务初始 vruntime 的计算）
    min_vruntime: u64,
    /// 运行队列中的任务总数
    nr_running: u32,
}

impl RunQueue {
    /// 创建空的运行队列
    pub fn new() -> Self {
        RunQueue {
            current: None,
            trees: [
                PriorityTree::new(),
                PriorityTree::new(),
                PriorityTree::new(),
                PriorityTree::new(),
                PriorityTree::new(),
            ],
            min_vruntime: 0,
            nr_running: 0,
        }
    }

    /// 将任务加入运行队列
    ///
    /// 根据任务的优先级类将其放入对应的优先级树中。
    /// 新创建的任务（vruntime=0）的 vruntime 至少设为 min_vruntime，防止饥饿。
    /// 已有 vruntime 的任务保持原值。
    pub fn enqueue(&mut self, task: &TaskControlBlock) {
        let priority_idx = task.sched_info.priority as usize;
        // 只有 vruntime 为 0 的新任务才使用 min_vruntime 归一化
        let vruntime = if task.sched_info.vruntime == 0 && self.min_vruntime > 0 {
            self.min_vruntime
        } else {
            task.sched_info.vruntime
        };
        let weight = task.sched_info.weight;

        self.trees[priority_idx].enqueue(vruntime, task.id.0, weight);
        self.nr_running += 1;
        self.update_min_vruntime();
    }

    /// 从运行队列中移除指定任务
    ///
    /// 注意：此方法仅从队列中移除任务，不管理 TCB 本身。
    /// 如果移除的是当前任务，则清空 current。
    pub fn dequeue(&mut self, task_id: TaskId) -> Option<TaskControlBlock> {
        // 在所有优先级树中查找并移除
        for tree in &mut self.trees {
            if tree.remove(task_id.0).is_some() {
                self.nr_running = self.nr_running.saturating_sub(1);
                if self.current == Some(task_id) {
                    self.current = None;
                }
                self.update_min_vruntime();
                // 注意：TCB 在 Scheduler 中管理，这里返回 None
                return None;
            }
        }
        None
    }

    /// 选择下一个要运行的任务
    ///
    /// 从最高优先级的非空树中选择 vruntime 最小的任务。
    /// 优先级顺序：Realtime > High > Agent > Normal > Idle
    pub fn pick_next_task(&mut self) -> Option<TaskId> {
        // 从最高优先级（Realtime=4）到最低（Idle=0）搜索
        for i in (0..5).rev() {
            if !self.trees[i].is_empty() {
                let (_, task_id) = self.trees[i].dequeue_min()?;
                self.nr_running = self.nr_running.saturating_sub(1);
                self.current = Some(TaskId(task_id));
                self.update_min_vruntime();
                return Some(TaskId(task_id));
            }
        }
        None
    }

    /// 获取当前正在运行的任务
    pub fn current(&self) -> Option<TaskId> {
        self.current
    }

    /// 设置当前正在运行的任务
    pub fn set_current(&mut self, task_id: Option<TaskId>) {
        self.current = task_id;
    }

    /// 获取全局最小 vruntime
    pub fn min_vruntime(&self) -> u64 {
        self.min_vruntime
    }

    /// 更新全局最小 vruntime
    ///
    /// 遍历所有优先级树，取所有树中最小的 vruntime 作为全局最小值。
    pub fn update_min_vruntime(&mut self) {
        let mut min = u64::MAX;
        for tree in &self.trees {
            if let Some((&key, _)) = tree.tree.first_key_value() {
                if key < min {
                    min = key;
                }
            }
        }
        if min != u64::MAX {
            self.min_vruntime = min;
        }
    }

    /// 获取运行队列中的任务总数
    pub fn nr_running(&self) -> u32 {
        self.nr_running
    }

    /// 获取运行队列中所有任务的总权重
    pub fn total_weight(&self) -> u64 {
        let mut total = 0u64;
        for tree in &self.trees {
            total += tree.total_weight();
        }
        total
    }
}

impl Default for RunQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::priority::PriorityClass;

    /// 创建测试用任务的辅助函数
    fn make_task(id: u64, priority: PriorityClass, vruntime: u64) -> TaskControlBlock {
        let mut tcb = TaskControlBlock::new(
            TaskId(id),
            0x1000 + id,
            0x8000 + id * 0x1000,
            priority,
            false,
            None,
        );
        tcb.sched_info.vruntime = vruntime;
        tcb.state = crate::scheduler::task::TaskState::Ready;
        tcb
    }

    /// 测试 13：优先级树入队出队
    #[test]
    fn test_priority_tree_enqueue_dequeue() {
        let mut tree = PriorityTree::new();

        // 空树出队应返回 None
        assert!(tree.dequeue_min().is_none());
        assert!(tree.is_empty());

        // 入队三个任务
        tree.enqueue(100, 1, 1024);
        tree.enqueue(50, 2, 1024);
        tree.enqueue(200, 3, 1024);

        assert_eq!(tree.len(), 3);
        assert!(!tree.is_empty());

        // 出队应按 vruntime 从小到大
        let (vruntime, task_id) = tree.dequeue_min().unwrap();
        assert_eq!(vruntime, 50);
        assert_eq!(task_id, 2);

        let (vruntime, task_id) = tree.dequeue_min().unwrap();
        assert_eq!(vruntime, 100);
        assert_eq!(task_id, 1);

        let (vruntime, task_id) = tree.dequeue_min().unwrap();
        assert_eq!(vruntime, 200);
        assert_eq!(task_id, 3);

        // 树应为空
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    /// 测试 14：空优先级树操作
    #[test]
    fn test_priority_tree_empty() {
        let mut tree = PriorityTree::new();

        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.total_weight(), 0);
        assert!(tree.dequeue_min().is_none());
        assert!(tree.remove(999).is_none());
    }

    /// 测试 15：运行队列入队出队
    #[test]
    fn test_run_queue_enqueue_dequeue() {
        let mut rq = RunQueue::new();

        // 空队列
        assert!(rq.pick_next_task().is_none());
        assert_eq!(rq.nr_running(), 0);

        // 入队任务
        let task1 = make_task(1, PriorityClass::Normal, 100);
        let task2 = make_task(2, PriorityClass::Normal, 200);
        let task3 = make_task(3, PriorityClass::Normal, 50);

        rq.enqueue(&task1);
        rq.enqueue(&task2);
        rq.enqueue(&task3);

        assert_eq!(rq.nr_running(), 3);

        // 出队应按 vruntime 排序
        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(3)); // vruntime=50 最小

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(1)); // vruntime=100

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(2)); // vruntime=200

        // 队列应为空
        assert!(rq.pick_next_task().is_none());
        assert_eq!(rq.nr_running(), 0);
    }

    /// 测试 16：按优先级选择任务
    #[test]
    fn test_run_queue_priority_ordering() {
        let mut rq = RunQueue::new();

        // 在不同优先级树中各放一个任务
        let idle_task = make_task(1, PriorityClass::Idle, 10);
        let normal_task = make_task(2, PriorityClass::Normal, 20);
        let agent_task = make_task(3, PriorityClass::Agent, 30);
        let high_task = make_task(4, PriorityClass::High, 40);
        let rt_task = make_task(5, PriorityClass::Realtime, 50);

        rq.enqueue(&idle_task);
        rq.enqueue(&normal_task);
        rq.enqueue(&agent_task);
        rq.enqueue(&high_task);
        rq.enqueue(&rt_task);

        assert_eq!(rq.nr_running(), 5);

        // 应优先选择最高优先级的任务（Realtime）
        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(5)); // Realtime

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(4)); // High

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(3)); // Agent

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(2)); // Normal

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(1)); // Idle

        assert!(rq.pick_next_task().is_none());
    }

    /// 测试 17：同优先级按 vruntime 排序
    #[test]
    fn test_run_queue_vruntime_ordering() {
        let mut rq = RunQueue::new();

        // 同一优先级（Normal）的多个任务
        let task1 = make_task(1, PriorityClass::Normal, 300);
        let task2 = make_task(2, PriorityClass::Normal, 100);
        let task3 = make_task(3, PriorityClass::Normal, 200);

        rq.enqueue(&task1);
        rq.enqueue(&task2);
        rq.enqueue(&task3);

        // 应按 vruntime 从小到大选择
        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(2)); // vruntime=100

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(3)); // vruntime=200

        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(1)); // vruntime=300
    }

    /// 测试 18：当前任务管理
    #[test]
    fn test_run_queue_current() {
        let mut rq = RunQueue::new();

        // 初始没有当前任务
        assert!(rq.current().is_none());

        // 设置当前任务
        rq.set_current(Some(TaskId(42)));
        assert_eq!(rq.current(), Some(TaskId(42)));

        // 清除当前任务
        rq.set_current(None);
        assert!(rq.current().is_none());

        // pick_next_task 会设置 current
        let task = make_task(1, PriorityClass::Normal, 100);
        rq.enqueue(&task);
        let next = rq.pick_next_task().unwrap();
        assert_eq!(next, TaskId(1));
        assert_eq!(rq.current(), Some(TaskId(1)));
    }

    /// 测试 19：最小 vruntime 追踪
    #[test]
    fn test_run_queue_min_vruntime() {
        let mut rq = RunQueue::new();

        // 初始 min_vruntime 应为 0
        assert_eq!(rq.min_vruntime(), 0);

        // 入队任务
        let task1 = make_task(1, PriorityClass::Normal, 500);
        let task2 = make_task(2, PriorityClass::High, 300);
        let task3 = make_task(3, PriorityClass::Agent, 200);

        rq.enqueue(&task1);
        rq.enqueue(&task2);
        rq.enqueue(&task3);

        // min_vruntime 应为所有树中最小的 vruntime
        assert_eq!(rq.min_vruntime(), 200);

        // 出队最高优先级的任务（High, vruntime=300）
        let picked = rq.pick_next_task().unwrap();
        assert_eq!(picked, TaskId(2)); // High 优先级最高

        // min_vruntime 应更新为剩余任务中的最小值
        assert_eq!(rq.min_vruntime(), 200); // Agent(vruntime=200) 仍在队列中

        // 再出队 Agent 任务
        let picked2 = rq.pick_next_task().unwrap();
        assert_eq!(picked2, TaskId(3)); // Agent 优先级次之

        // min_vruntime 应更新为 Normal 任务的 vruntime
        assert_eq!(rq.min_vruntime(), 500);
    }
}
