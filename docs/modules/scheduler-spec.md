# OmniAgent OS — 调度器模块规格说明

> **模块名称**: `omniagent-scheduler`
> **版本**: v0.1.0-draft
> **状态**: 设计阶段
> **依赖**: `arceos-fairsched`, `x86_64`, `spin`, `log`

---

## 1. 概述

### 1.1 目的

调度器模块是 OmniAgent OS 的核心子系统之一，负责在多个 CPU 核心上高效地分配 CPU 时间片给就绪任务。本调度器基于完全公平调度器（Completely Fair Scheduler, CFS）变体设计，引入了 Agent 优先级类，为 AI Agent 任务提供提升的调度优先级，确保 Agent 响应的实时性和系统整体公平性。

### 1.2 设计目标

| 目标 | 指标 |
|------|------|
| 调度延迟 | < 10μs（从就绪到获得 CPU） |
| 上下文切换时间 | < 1μs（纯寄存器保存/恢复） |
| 支持优先级类 | 5 类（Realtime, High, Normal, Idle, Agent） |
| 最大并发任务数 | 每核 4096 |
| CPU 核心数 | 1–256（SMP 支持） |
| 负载均衡开销 | < 5μs（跨核迁移） |

### 1.3 与 arceos-fairsched 的集成

本模块基于 `arceos-fairsched` 进行扩展和定制。`arceos-fairsched` 提供了基础的 CFS 调度框架，我们在此基础上：

1. **扩展优先级模型**：从 3 级优先级扩展到 5 级，新增 Agent 优先级类
2. **优化红黑树实现**：替换为支持批量操作的自定义红黑树
3. **添加负载均衡器**：实现跨核任务迁移
4. **集成抢占点**：在系统调用返回、IPC 完成等位置插入抢占检查

---

## 2. 优先级类设计

### 2.1 优先级类定义

```rust
/// 优先级类枚举
///
/// 数值越大，优先级越高。调度器按优先级类从高到低扫描就绪队列。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PriorityClass {
    /// 空闲优先级 — 仅在没有其他就绪任务时运行
    Idle     = 0,
    /// 普通优先级 — 默认用户任务
    Normal   = 1,
    /// Agent 优先级 — AI Agent 任务，高于普通任务
    Agent    = 2,
    /// 高优先级 — 关键系统服务
    High     = 3,
    /// 实时优先级 — 硬实时任务，使用 FIFO/RR 策略
    Realtime = 4,
}

impl PriorityClass {
    /// 获取优先级类对应的时间片权重
    pub const fn weight(&self) -> u32 {
        match self {
            PriorityClass::Idle     => 3,
            PriorityClass::Normal   => 1024,
            PriorityClass::Agent    => 1536,  // Agent 权重为 Normal 的 1.5 倍
            PriorityClass::High     => 2048,
            PriorityClass::Realtime => 4096,
        }
    }

    /// 获取优先级类对应的基础时间片（纳秒）
    pub const fn base_time_slice_ns(&self) -> u64 {
        match self {
            PriorityClass::Idle     => 1_000_000,    // 1ms
            PriorityClass::Normal   => 6_000_000,    // 6ms
            PriorityClass::Agent    => 8_000_000,    // 8ms（Agent 获得更长初始时间片）
            PriorityClass::High     => 10_000_000,   // 10ms
            PriorityClass::Realtime => 20_000_000,   // 20ms 或无限（FIFO）
        }
    }

    /// 是否为实时优先级类
    pub const fn is_realtime(&self) -> bool {
        matches!(self, PriorityClass::Realtime)
    }
}
```

### 2.2 Agent 优先级策略

Agent 任务在 OmniAgent OS 中享有提升的调度优先级，具体策略如下：

- **权重提升**：Agent 优先级的权重为 Normal 的 1.5 倍（1536 vs 1024），这意味着在相同虚拟运行时间下，Agent 任务获得更多实际 CPU 时间
- **时间片延长**：Agent 任务的基础时间片为 8ms，比 Normal 的 6ms 多 33%
- **抢占保护**：Agent 任务可被 High 和 Realtime 任务抢占，但不会被 Normal 任务抢占
- **动态调整**：当系统 Agent 负载过高时（> 70% CPU），Agent 权重自动降低至 Normal 水平，防止饥饿

```rust
/// Agent 优先级动态调整策略
pub struct AgentPriorityPolicy {
    /// CPU 使用率阈值（超过此值时降低 Agent 权重）
    pub cpu_threshold_high: f64,   // 默认 0.70
    /// CPU 使用率恢复阈值（低于此值时恢复 Agent 权重）
    pub cpu_threshold_low: f64,    // 默认 0.50
    /// 降级后的权重
    pub degraded_weight: u32,      // 默认 1024（等同于 Normal）
    /// 采样窗口（毫秒）
    pub sample_window_ms: u64,     // 默认 100ms
}
```

---

## 3. 虚拟运行时间（vruntime）

### 3.1 vruntime 计算模型

CFS 的核心思想是通过虚拟运行时间（vruntime）来衡量任务的"公平份额"。vruntime 的增长速度与任务优先级权重成反比：权重越高，vruntime 增长越慢，任务获得越多的 CPU 时间。

```rust
/// 任务控制块中的调度信息
#[derive(Debug)]
pub struct SchedInfo {
    /// 虚拟运行时间（纳秒），是 CFS 红黑树排序的键
    pub vruntime: u64,
    /// 实际运行时间（纳秒）
    pub runtime: u64,
    /// 优先级类
    pub priority: PriorityClass,
    /// 优先级权重
    pub weight: u32,
    /// 当前时间片剩余（纳秒）
    pub time_slice_remain: u64,
    /// 上次调度的时间戳
    pub last_sched_tick: u64,
}

impl SchedInfo {
    /// 更新 vruntime
    ///
    /// 公式: delta_vruntime = delta_runtime * (NICE_0_WEIGHT / weight)
    /// 其中 NICE_0_WEIGHT = 1024
    pub fn update_vruntime(&mut self, delta_runtime_ns: u64) {
        const NICE_0_WEIGHT: u64 = 1024;
        let delta_vruntime = delta_runtime_ns
            .wrapping_mul(NICE_0_WEIGHT)
            / self.weight as u64;
        self.vruntime = self.vruntime.wrapping_add(delta_vruntime);
        self.runtime = self.runtime.wrapping_add(delta_runtime_ns);
    }

    /// 计算任务应获得的时间片
    pub fn calc_time_slice(&self, total_weight: u64, sched_period_ns: u64) -> u64 {
        let slice = sched_period_ns * self.weight as u64 / total_weight;
        slice.max(self.priority.base_time_slice_ns() / 4)
            .min(self.priority.base_time_slice_ns() * 2)
    }
}
```

### 3.2 vruntime 归一化

当新任务入队时，需要将其 vruntime 设置为当前就绪队列的最小 vruntime，防止新任务获得过多的初始 CPU 时间。

```rust
/// 归一化新任务的 vruntime
fn normalize_new_task_vruntime(task: &mut SchedInfo, rq: &RunQueue) {
    let min_vruntime = rq.min_vruntime();
    // 新任务获得至少一个时间片的启动补偿
    let startup_granularity = task.priority.base_time_slice_ns();
    task.vruntime = min_vruntime.saturating_sub(startup_granularity);
}
```

---

## 4. 运行队列数据结构

### 4.1 红黑树运行队列

每个 CPU 核心维护一组运行队列，每个优先级类对应一棵红黑树。红黑树以 vruntime 为键，保证 O(log n) 的插入、删除和最小值查找。

```rust
use alloc::collections::BTreeMap;
use spin::RwLock;

/// 单个 CPU 核心的运行队列
pub struct RunQueue {
    /// 当前运行在此核心上的任务
    pub current: Option<TaskId>,
    /// 各优先级类的就绪队列（红黑树，键为 vruntime）
    pub trees: [PriorityTree; PRIORITY_CLASS_COUNT],
    /// 全局最小 vruntime，用于归一化
    pub min_vruntime: u64,
    /// 运行队列中的任务总数
    pub nr_running: u32,
    /// 该核心的 CPU 负载（用于负载均衡）
    pub load: AtomicU64,
    /// 运行队列自旋锁
    pub lock: SpinLock<()>,
    /// 核心编号
    pub cpu_id: usize,
}

/// 优先级树：以 vruntime 为键的红黑树
pub struct PriorityTree {
    /// 红黑树：vruntime -> TaskId
    pub tree: BTreeMap<u64, TaskId>,
    /// 该优先级类中的任务数量
    pub nr_tasks: u32,
    /// 该优先级类的总权重
    pub total_weight: u64,
}

const PRIORITY_CLASS_COUNT: usize = 5;

impl RunQueue {
    /// 创建新的运行队列
    pub fn new(cpu_id: usize) -> Self {
        Self {
            current: None,
            trees: core::array::from_fn(|_| PriorityTree::new()),
            min_vruntime: 0,
            nr_running: 0,
            load: AtomicU64::new(0),
            lock: SpinLock::new(()),
            cpu_id,
        }
    }

    /// 将任务加入就绪队列
    pub fn enqueue(&mut self, task: &TaskControlBlock) {
        let idx = task.sched_info.priority as usize;
        let tree = &mut self.trees[idx];
        tree.tree.insert(task.sched_info.vruntime, task.id);
        tree.nr_tasks += 1;
        tree.total_weight += task.sched_info.weight as u64;
        self.nr_running += 1;
        self.load.fetch_add(task.sched_info.weight as u64, Ordering::Relaxed);
    }

    /// 从就绪队列中选取下一个任务
    ///
    /// 策略：从最高优先级类开始扫描，选择 vruntime 最小的任务
    pub fn pick_next_task(&mut self) -> Option<TaskId> {
        for i in (0..PRIORITY_CLASS_COUNT).rev() {
            let tree = &mut self.trees[i];
            if let Some((&vruntime, &task_id)) = tree.tree.first_key_value() {
                tree.tree.remove(&vruntime);
                tree.nr_tasks -= 1;
                tree.total_weight -= 0; // 需要从 TCB 获取权重
                self.nr_running -= 1;
                return Some(task_id);
            }
        }
        None
    }

    /// 获取当前最小 vruntime
    pub fn min_vruntime(&self) -> u64 {
        self.min_vruntime
    }
}

impl PriorityTree {
    pub fn new() -> Self {
        Self {
            tree: BTreeMap::new(),
            nr_tasks: 0,
            total_weight: 0,
        }
    }
}
```

### 4.2 多核运行队列管理

```rust
/// 全局调度器，管理所有 CPU 核心的运行队列
pub struct GlobalScheduler {
    /// 每个核心的运行队列
    pub run_queues: [RunQueue; MAX_CPU_NUM],
    /// 负载均衡器
    pub balancer: LoadBalancer,
    /// 调度周期（纳秒）
    pub sched_period_ns: u64,
    /// 调度器统计信息
    pub stats: SchedulerStats,
}

const MAX_CPU_NUM: usize = 256;

impl GlobalScheduler {
    /// 初始化全局调度器
    pub fn init(cpu_num: usize) -> Self {
        let run_queues = core::array::from_fn(|i| RunQueue::new(i));
        Self {
            run_queues,
            balancer: LoadBalancer::new(cpu_num),
            sched_period_ns: 6_000_000, // 默认 6ms 调度周期
            stats: SchedulerStats::new(),
        }
    }

    /// 将任务唤醒并加入就绪队列
    pub fn wake_up(&self, task: &mut TaskControlBlock) {
        let cpu = self.balancer.select_cpu(task);
        let rq = &self.run_queues[cpu];
        let _guard = rq.lock.lock();
        // 将任务分配到选定的 CPU
        task.cpu = cpu;
        rq.enqueue(task);
    }

    /// 当前核心的调度入口
    pub fn schedule(&self) {
        let cpu = current_cpu_id();
        let rq = &self.run_queues[cpu];
        let _guard = rq.lock.lock();
        let prev = rq.current;
        let next = rq.pick_next_task();

        if prev != next {
            if let Some(next_id) = next {
                self.context_switch(prev, next_id);
            }
        }
    }
}
```

---

## 5. 负载均衡

### 5.1 负载均衡策略

```rust
/// 负载均衡器
pub struct LoadBalancer {
    /// CPU 核心数量
    pub cpu_num: usize,
    /// 每个 CPU 的负载历史记录
    pub load_history: [CircularBuffer<f64, 32>; MAX_CPU_NUM],
    /// 负载均衡间隔（纳秒）
    pub balance_interval_ns: u64,
    /// 上次均衡时间
    pub last_balance_ns: u64,
}

impl LoadBalancer {
    pub fn new(cpu_num: usize) -> Self {
        Self {
            cpu_num,
            load_history: core::array::from_fn(|_| CircularBuffer::new()),
            balance_interval_ns: 1_000_000, // 1ms
            last_balance_ns: 0,
        }
    }

    /// 为新任务选择最佳 CPU
    ///
    /// 策略：
    /// 1. 优先选择空闲 CPU
    /// 2. 其次选择负载最低的 CPU
    /// 3. 考虑 NUMA 亲和性
    /// 4. Agent 任务优先分散到不同核心
    pub fn select_cpu(&self, task: &TaskControlBlock) -> usize {
        let current_cpu = current_cpu_id();

        // Agent 任务：优先分散到不同核心
        if task.sched_info.priority == PriorityClass::Agent {
            return self.select_spread_cpu(task);
        }

        // 查找空闲核心
        for i in 0..self.cpu_num {
            if self.is_idle(i) {
                return i;
            }
        }

        // 选择负载最低的核心
        self.least_loaded_cpu()
    }

    /// 执行跨核负载均衡
    pub fn balance(&mut self, rqs: &mut [RunQueue]) {
        let now = current_time_ns();
        if now - self.last_balance_ns < self.balance_interval_ns {
            return;
        }
        self.last_balance_ns = now;

        // 计算平均负载
        let total_load: u64 = rqs[..self.cpu_num]
            .iter()
            .map(|rq| rq.load.load(Ordering::Relaxed))
            .sum();
        let avg_load = total_load / self.cpu_num as u64;

        // 从高负载核心迁移任务到低负载核心
        for i in 0..self.cpu_num {
            let load = rqs[i].load.load(Ordering::Relaxed);
            if load > avg_load * 12 / 10 { // 负载超过平均值 20%
                self.migrate_tasks(&mut rqs[i], rqs, avg_load);
            }
        }
    }

    /// 从源核心迁移任务到目标核心
    fn migrate_tasks(&self, src: &mut RunQueue, dsts: &mut [RunQueue], target_load: u64) {
        while src.load.load(Ordering::Relaxed) > target_load {
            // 从最高优先级类中找 vruntime 最大的任务进行迁移
            let task_id = src.pick_migrate_candidate();
            if let Some(tid) = task_id {
                let target = self.least_loaded_cpu_from(dsts);
                // 执行任务迁移（涉及 TCB 状态更新）
                self.migrate_task(tid, src.cpu_id, target);
            } else {
                break;
            }
        }
    }
}
```

---

## 6. 上下文切换机制

### 6.1 上下文切换流程

上下文切换是调度器最关键的操作之一，必须在 1μs 内完成。我们采用汇编桩（assembly stub）加寄存器保存/恢复的方式实现。

```rust
/// 上下文帧：保存在任务内核栈上
///
/// 内存布局（栈从高地址向低地址增长）：
/// ┌─────────────────────┐  ← 高地址
/// │  SS                 │
/// │  RSP                │
/// │  RFLAGS             │
/// │  CS                 │
/// │  RIP                │
/// ├─────────────────────┤
/// │  RAX                │
/// │  RBX                │
/// │  RCX                │
/// │  RDX                │
/// │  RSI                │
/// │  RDI                │
/// │  RBP                │
/// │  R8  - R15          │
/// │  FPU/SSE 状态       │
/// └─────────────────────┘  ← 低地址（RSP 指向此处）
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ContextFrame {
    // 通用寄存器
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8:  u64,
    pub r9:  u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // 控制寄存器
    pub rip: u64,
    pub cs:  u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss:  u64,
}

impl ContextFrame {
    /// 创建新的上下文帧（用于新任务初始化）
    pub fn new(entry_point: u64, stack_top: u64, is_user: bool) -> Self {
        let (cs, ss) = if is_user {
            (0x23, 0x1b) // 用户态代码段和数据段
        } else {
            (0x08, 0x10) // 内核态代码段和数据段
        };
        Self {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: entry_point,
            cs,
            rflags: 0x200, // IF 位设置（允许中断）
            rsp: stack_top,
            ss,
        }
    }
}
```

### 6.2 汇编上下文切换桩

```asm
# switch_to: 从当前任务切换到目标任务
# 参数：
#   RDI = prev_task_context_frame_ptr
#   RSI = next_task_context_frame_ptr
#
# 保存 ABI 规定的被调用者保存寄存器
switch_to:
    # 保存当前任务的 callee-saved 寄存器
    pushq %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    # 保存 RDI（prev 指针）到 RAX
    movq %rdi, %rax

    # 保存当前 RSP 到 prev->rsp
    movq %rsp, (%rax)

    # 恢复 next_task 的 RSP
    movq %rsi, %rsp

    # 恢复 next_task 的 callee-saved 寄存器
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp

    # 设置当前任务指针
    movq %rsi, %rdi
    callq set_current_task

    ret
```

### 6.3 Rust 侧上下文切换接口

```rust
/// 执行上下文切换
///
/// # Safety
/// - prev_frame 必须指向有效的、已分配的内核栈空间
/// - next_frame 必须指向有效的、已初始化的上下文帧
#[no_mangle]
pub unsafe extern "sysv64" fn switch_to(
    prev_frame: *mut ContextFrame,
    next_frame: *const ContextFrame,
) {
    // 调用汇编桩完成实际的寄存器保存/恢复
    core::arch::asm!(
        "call switch_to",
        in("rdi") prev_frame,
        in("rsi") next_frame,
        clobber_abi("sysv64"),
    );
}

/// 上下文切换 Trait
pub trait ContextSwitch {
    /// 保存当前上下文并切换到目标任务
    fn switch_to(&mut self, next_task: &TaskControlBlock);
    /// 初始化新任务的上下文
    fn init_context(&mut self, entry: u64, stack_top: u64, is_user: bool);
}

impl ContextSwitch for TaskControlBlock {
    fn switch_to(&mut self, next_task: &TaskControlBlock) {
        unsafe {
            switch_to(
                &mut self.context_frame as *mut ContextFrame,
                &next_task.context_frame as *const ContextFrame,
            );
        }
    }

    fn init_context(&mut self, entry: u64, stack_top: u64, is_user: bool) {
        self.context_frame = ContextFrame::new(entry, stack_top, is_user);
    }
}
```

---

## 7. 抢占点

### 7.1 抢占时机

调度器在以下位置检查是否需要抢占当前任务：

```rust
/// 抢占检查 Trait
pub trait PreemptCheck {
    /// 检查是否需要抢占
    fn should_preempt(&self, current: &TaskControlBlock) -> bool;
}

/// 基于 TIF_NEED_RESCHED 标志的抢占检查
pub struct ReschedFlagChecker;

impl PreemptCheck for ReschedFlagChecker {
    fn should_preempt(&self, current: &TaskControlBlock) -> bool {
        current.flags.contains(TaskFlags::NEED_RESCHED)
    }
}

/// 抢占点位置枚举
#[derive(Debug, Clone, Copy)]
pub enum PreemptPoint {
    /// 定时器中断处理程序中
    TimerInterrupt,
    /// 系统调用返回用户态之前
    SyscallReturn,
    /// IPC 操作完成时
    IpcCompletion,
    /// 中断返回时
    IrqReturn,
    /// 自愿让出 CPU（yield）
    VoluntaryYield,
}

/// 在抢占点执行抢占检查
///
/// 此函数在每个抢占点被调用，检查当前任务是否应被抢占
#[inline(always)]
pub fn check_preempt(point: PreemptPoint) {
    let current = current_task();
    if current.flags.contains(TaskFlags::NEED_RESCHED) {
        // 清除抢占标志
        current.flags.remove(TaskFlags::NEED_RESCHED);
        // 执行调度
        scheduler().schedule();
    }
}
```

### 7.2 定时器中断抢占

```rust
/// 定时器中断处理程序中的调度逻辑
pub fn timer_tick_handler() {
    let current = current_task();
    let cpu = current_cpu_id();
    let rq = &scheduler().run_queues[cpu];

    // 更新当前任务的运行时间
    let now = current_time_ns();
    let delta = now - current.sched_info.last_sched_tick;
    current.sched_info.last_sched_tick = now;
    current.sched_info.update_vruntime(delta);
    current.sched_info.time_slice_remain =
        current.sched_info.time_slice_remain.saturating_sub(delta);

    // 检查时间片是否用完
    if current.sched_info.time_slice_remain == 0 {
        // 重新计算时间片
        let total_weight = rq.total_weight();
        let new_slice = current.sched_info.calc_time_slice(
            total_weight,
            scheduler().sched_period_ns,
        );
        current.sched_info.time_slice_remain = new_slice;

        // 标记需要重新调度
        current.flags.insert(TaskFlags::NEED_RESCHED);

        // 更新运行队列的最小 vruntime
        rq.update_min_vruntime();
    }

    // 触发抢占检查
    check_preempt(PreemptPoint::TimerInterrupt);
}
```

---

## 8. 状态机

### 8.1 任务调度状态机

```
                    ┌──────────┐
                    │  CREATED │ (任务已创建，未就绪)
                    └────┬─────┘
                         │ wake_up()
                         ▼
                    ┌──────────┐
              ┌────▶│  READY   │◀────┐
              │     └────┬─────┘     │
              │          │ schedule()│ 时间片用完 / 抢占
              │          ▼           │
              │     ┌──────────┐     │
              │     │ RUNNING  │─────┘
              │     └────┬─────┘
              │          │ sleep() / wait()
              │          ▼
              │     ┌──────────┐
              │     │ BLOCKED  │
              │     └────┬─────┘
              │          │ wake_up()
              │          └──────────┘
              │
              │          │ exit()
              │          ▼
              │     ┌──────────┐
              └─────│  ZOMBIE  │ (等待父任务回收)
                    └──────────┘
```

```rust
/// 任务调度状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    /// 已创建，尚未就绪
    Created = 0,
    /// 就绪，等待调度
    Ready   = 1,
    /// 正在运行
    Running = 2,
    /// 阻塞（等待 I/O、锁、IPC 等）
    Blocked = 3,
    /// 已退出，等待回收
    Zombie  = 4,
}

/// 任务状态转换验证
impl TaskState {
    /// 验证状态转换是否合法
    pub fn can_transition(self, to: TaskState) -> bool {
        matches!(
            (self, to),
            (TaskState::Created, TaskState::Ready)
                | (TaskState::Ready, TaskState::Running)
                | (TaskState::Running, TaskState::Ready)
                | (TaskState::Running, TaskState::Blocked)
                | (TaskState::Running, TaskState::Zombie)
                | (TaskState::Blocked, TaskState::Ready)
                | (TaskState::Blocked, TaskState::Zombie)
        )
    }
}
```

---

## 9. 错误处理

### 9.1 调度器错误类型

```rust
use core::fmt;

/// 调度器错误类型
#[derive(Debug, Clone)]
pub enum SchedulerError {
    /// 无效的任务 ID
    InvalidTaskId(TaskId),
    /// 任务状态转换非法
    InvalidStateTransition {
        from: TaskState,
        to: TaskState,
    },
    /// 运行队列已满
    RunQueueFull { cpu: usize },
    /// 任务已在指定运行队列中
    AlreadyEnqueued { task_id: TaskId, cpu: usize },
    /// 任务不在运行队列中
    NotEnqueued { task_id: TaskId },
    /// 上下文切换失败
    ContextSwitchFailed { reason: &'static str },
    /// CPU 核心编号越界
    InvalidCpuId(usize),
    /// 优先级类不合法
    InvalidPriorityClass(u8),
    /// 负载均衡失败
    LoadBalanceFailed { reason: &'static str },
}

#[cfg(feature = "std")]
impl std::error::Error for SchedulerError {}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTaskId(id) => write!(f, "无效的任务 ID: {:?}", id),
            Self::InvalidStateTransition { from, to } => {
                write!(f, "非法状态转换: {:?} -> {:?}", from, to)
            }
            Self::RunQueueFull { cpu } => write!(f, "CPU {} 运行队列已满", cpu),
            Self::AlreadyEnqueued { task_id, cpu } => {
                write!(f, "任务 {:?} 已在 CPU {} 的运行队列中", task_id, cpu)
            }
            Self::NotEnqueued { task_id } => {
                write!(f, "任务 {:?} 不在任何运行队列中", task_id)
            }
            Self::ContextSwitchFailed { reason } => {
                write!(f, "上下文切换失败: {}", reason)
            }
            Self::InvalidCpuId(id) => write!(f, "无效的 CPU 核心编号: {}", id),
            Self::InvalidPriorityClass(p) => write!(f, "无效的优先级类: {}", p),
            Self::LoadBalanceFailed { reason } => {
                write!(f, "负载均衡失败: {}", reason)
            }
        }
    }
}

/// 调度器 Result 类型
pub type SchedulerResult<T> = Result<T, SchedulerError>;
```

---

## 10. 性能约束与监控

### 10.1 性能指标

| 指标 | 目标值 | 测量方法 |
|------|--------|----------|
| 调度延迟 | < 10μs | 从任务就绪到获得 CPU 的时间 |
| 上下文切换 | < 1μs | 寄存器保存/恢复时间（不含 FPU） |
| FPU 上下文切换 | < 3μs | 含 SSE/AVX 状态保存/恢复 |
| 红黑树插入 | < 100ns | 单次 enqueue 操作 |
| 红黑树查找最小 | < 50ns | pick_next_task 操作 |
| 负载均衡 | < 5μs | 单次跨核迁移 |

### 10.2 调度器统计

```rust
/// 调度器统计信息
#[derive(Debug, Default)]
pub struct SchedulerStats {
    /// 总调度次数
    pub total_schedules: AtomicU64,
    /// 上下文切换次数
    pub context_switches: AtomicU64,
    /// 抢占次数
    pub preemptions: AtomicU64,
    /// 跨核迁移次数
    pub migrations: AtomicU64,
    /// 平均调度延迟（纳秒）
    pub avg_sched_latency_ns: AtomicU64,
    /// 最大调度延迟（纳秒）
    pub max_sched_latency_ns: AtomicU64,
    /// 各优先级类的 CPU 使用率
    pub cpu_usage_per_class: [AtomicU64; PRIORITY_CLASS_COUNT],
    /// 运行队列长度统计
    pub rq_len_samples: AtomicU64,
}

impl SchedulerStats {
    /// 记录一次调度事件
    pub fn record_schedule(&self, latency_ns: u64) {
        self.total_schedules.fetch_add(1, Ordering::Relaxed);
        self.context_switches.fetch_add(1, Ordering::Relaxed);
        // 更新最大延迟
        let mut current_max = self.max_sched_latency_ns.load(Ordering::Relaxed);
        while latency_ns > current_max {
            match self.max_sched_latency_ns.compare_exchange_weak(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current_max = v,
            }
        }
    }
}
```

---

## 11. 安全考虑

### 11.1 安全机制

1. **优先级继承**：当高优先级任务等待低优先级任务持有的锁时，低优先级任务临时继承高优先级，防止优先级反转
2. **实时任务隔离**：Realtime 任务有独立的 CPU 亲和性掩码，不会与普通任务竞争
3. **Agent 任务沙箱**：Agent 任务虽然享有提升的优先级，但其资源使用受到严格限制（CPU 配额、内存上限）
4. **时间片窃取检测**：检测任务是否通过恶意行为（如禁用中断）窃取额外 CPU 时间
5. **运行队列保护**：所有运行队列操作都通过自旋锁保护，防止竞态条件

```rust
/// 优先级继承协议
pub struct PriorityInheritance {
    /// 等待链：task -> lock -> holder
    wait_chain: SpinLock<Vec<(TaskId, LockId, TaskId)>>,
}

impl PriorityInheritance {
    /// 当任务等待锁时，执行优先级提升
    pub fn boost_holder(&self, waiter: &TaskControlBlock, lock_id: LockId) {
        let holder_id = self.find_lock_holder(lock_id);
        if let Some(holder) = get_task(holder_id) {
            if waiter.sched_info.priority > holder.sched_info.priority {
                // 提升持有者的优先级
                holder.sched_info.priority = waiter.sched_info.priority;
                holder.sched_info.weight = waiter.sched_info.weight;
            }
        }
    }

    /// 当锁释放时，恢复原始优先级
    pub fn restore_priority(&self, holder_id: TaskId, lock_id: LockId) {
        if let Some(holder) = get_task(holder_id) {
            let original = self.get_original_priority(holder_id);
            holder.sched_info.priority = original;
            holder.sched_info.weight = original.weight();
        }
    }
}
```

---

## 12. 测试用例

### 12.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_class_ordering() {
        assert!(PriorityClass::Realtime > PriorityClass::High);
        assert!(PriorityClass::High > PriorityClass::Agent);
        assert!(PriorityClass::Agent > PriorityClass::Normal);
        assert!(PriorityClass::Normal > PriorityClass::Idle);
    }

    #[test]
    fn test_agent_weight_higher_than_normal() {
        assert!(PriorityClass::Agent.weight() > PriorityClass::Normal.weight());
        assert_eq!(PriorityClass::Agent.weight(), 1536);
        assert_eq!(PriorityClass::Normal.weight(), 1024);
    }

    #[test]
    fn test_vruntime_update() {
        let mut info = SchedInfo {
            vruntime: 1000,
            runtime: 5000,
            priority: PriorityClass::Normal,
            weight: 1024,
            time_slice_remain: 6_000_000,
            last_sched_tick: 0,
        };
        info.update_vruntime(1_000_000); // 运行 1ms
        // weight=1024, NICE_0_WEIGHT=1024, 所以 delta_vruntime = 1ms
        assert_eq!(info.vruntime, 1_001_000);
        assert_eq!(info.runtime, 6_000_000);
    }

    #[test]
    fn test_vruntime_agent_slower_growth() {
        let mut normal = SchedInfo {
            vruntime: 0, runtime: 0,
            priority: PriorityClass::Normal, weight: 1024,
            time_slice_remain: 0, last_sched_tick: 0,
        };
        let mut agent = SchedInfo {
            vruntime: 0, runtime: 0,
            priority: PriorityClass::Agent, weight: 1536,
            time_slice_remain: 0, last_sched_tick: 0,
        };
        // 相同实际运行时间
        normal.update_vruntime(1_000_000);
        agent.update_vruntime(1_000_000);
        // Agent 的 vruntime 增长更慢
        assert!(agent.vruntime < normal.vruntime);
    }

    #[test]
    fn test_task_state_transitions() {
        assert!(TaskState::Created.can_transition(TaskState::Ready));
        assert!(TaskState::Ready.can_transition(TaskState::Running));
        assert!(TaskState::Running.can_transition(TaskState::Blocked));
        assert!(TaskState::Blocked.can_transition(TaskState::Ready));
        assert!(TaskState::Running.can_transition(TaskState::Zombie));
        // 非法转换
        assert!(!TaskState::Created.can_transition(TaskState::Running));
        assert!(!TaskState::Zombie.can_transition(TaskState::Ready));
    }

    #[test]
    fn test_run_queue_enqueue_dequeue() {
        let mut rq = RunQueue::new(0);
        let task = create_test_task(1, PriorityClass::Normal, 1000);
        rq.enqueue(&task);
        assert_eq!(rq.nr_running, 1);
        let next = rq.pick_next_task();
        assert_eq!(next, Some(task.id));
        assert_eq!(rq.nr_running, 0);
    }

    #[test]
    fn test_priority_ordering_in_rq() {
        let mut rq = RunQueue::new(0);
        let normal = create_test_task(1, PriorityClass::Normal, 5000);
        let agent = create_test_task(2, PriorityClass::Agent, 3000);
        let high = create_test_task(3, PriorityClass::High, 1000);

        rq.enqueue(&normal);
        rq.enqueue(&agent);
        rq.enqueue(&high);

        // High 优先级最高，应先被选中
        assert_eq!(rq.pick_next_task(), Some(high.id));
        // Agent 次之
        assert_eq!(rq.pick_next_task(), Some(agent.id));
        // Normal 最后
        assert_eq!(rq.pick_next_task(), Some(normal.id));
    }

    #[test]
    fn test_context_frame_new() {
        let frame = ContextFrame::new(0x400000, 0x800000, true);
        assert_eq!(frame.rip, 0x400000);
        assert_eq!(frame.rsp, 0x800000);
        assert_eq!(frame.cs, 0x23); // 用户态
        assert!(frame.rflags & 0x200 != 0); // IF 位
    }

    #[test]
    fn test_time_slice_calculation() {
        let info = SchedInfo {
            vruntime: 0, runtime: 0,
            priority: PriorityClass::Agent, weight: 1536,
            time_slice_remain: 0, last_sched_tick: 0,
        };
        let total_weight = 1024 + 1536; // Normal + Agent
        let period = 6_000_000u64;
        let slice = info.calc_time_slice(total_weight, period);
        // slice = 6ms * 1536 / 2560 ≈ 3.6ms
        assert!(slice > 3_000_000 && slice < 4_000_000);
    }

    #[test]
    fn test_scheduler_error_display() {
        let err = SchedulerError::InvalidStateTransition {
            from: TaskState::Zombie,
            to: TaskState::Running,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("非法状态转换"));
    }
}
```

### 12.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 测试：多任务 CFS 调度的公平性
    ///
    /// 验证相同优先级的任务获得大致相等的 CPU 时间
    #[test]
    fn test_cfs_fairness() {
        // 创建 10 个相同优先级的任务
        // 运行 100ms 后检查各任务的运行时间
        // 期望每个任务的运行时间在 10ms ± 20% 范围内
    }

    /// 测试：Agent 任务获得更多 CPU 时间
    #[test]
    fn test_agent_priority_boost() {
        // 创建 5 个 Normal 任务和 5 个 Agent 任务
        // 运行 100ms 后检查
        // 期望 Agent 任务的平均运行时间 > Normal 任务
    }

    /// 测试：负载均衡跨核迁移
    #[test]
    fn test_load_balancing() {
        // 在 4 核系统上创建 100 个任务
        // 验证任务大致均匀分布在各核心上
        // 期望每核任务数在 25 ± 5 范围内
    }

    /// 测试：上下文切换性能
    #[test]
    fn test_context_switch_performance() {
        // 使用 TSC 测量上下文切换时间
        // 期望 < 1μs
    }

    /// 测试：实时任务抢占
    #[test]
    fn test_realtime_preemption() {
        // 创建一个长时间运行的 Normal 任务
        // 在运行过程中唤醒一个 Realtime 任务
        // 验证 Realtime 任务立即获得 CPU
    }
}
```

---

## 13. 配置参数

```rust
/// 调度器配置
pub struct SchedulerConfig {
    /// 调度周期（纳秒）
    pub sched_period_ns: u64,
    /// 最小时间片（纳秒）
    pub min_granularity_ns: u64,
    /// 休眠 vruntime 衰减因子
    pub sleep_decay_factor: u32,
    /// 负载均衡间隔（纳秒）
    pub balance_interval_ns: u64,
    /// 负载均衡迁移阈值（百分比）
    pub balance_threshold_pct: u8,
    /// 是否启用 FPU 惰性切换
    pub lazy_fpu_switch: bool,
    /// Agent 优先级策略
    pub agent_policy: AgentPriorityPolicy,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            sched_period_ns: 6_000_000,
            min_granularity_ns: 750_000,
            sleep_decay_factor: 3,
            balance_interval_ns: 1_000_000,
            balance_threshold_pct: 25,
            lazy_fpu_switch: true,
            agent_policy: AgentPriorityPolicy::default(),
        }
    }
}
```

---

## 14. 附录

### 14.1 与 Linux CFS 的对比

| 特性 | Linux CFS | OmniAgent Scheduler |
|------|-----------|---------------------|
| 优先级类 | 3 类（RT, Normal, Idle） | 5 类（RT, High, Agent, Normal, Idle） |
| 调度延迟 | ~20μs | < 10μs |
| 上下文切换 | ~2μs | < 1μs |
| Agent 支持 | 无 | 原生支持 |
| 负载均衡 | 周期性 + NO_HZ | 周期性 + 事件驱动 |
| 实现语言 | C | Rust（内存安全） |

### 14.2 参考资料

- Linux CFS 调度器设计文档
- arceos-fairsched 源码
- `x86_64` crate 文档
- OSTEP 第 7 章：CPU 调度
