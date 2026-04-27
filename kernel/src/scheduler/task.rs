//! 任务控制块
//!
//! 定义任务 ID、任务状态、任务标志位、上下文帧和任务控制块（TCB）。
//! TCB 是调度器管理的核心数据结构，包含任务的完整运行时状态。

use alloc::vec::Vec;
use bitflags::bitflags;

use crate::scheduler::priority::{PriorityClass, SchedInfo};

/// 任务 ID
///
/// 全局唯一的任务标识符，使用 u64 表示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TaskId(pub u64);

/// 任务状态
///
/// 任务在其生命周期中的五种状态。
/// 状态转换必须遵循 `can_transition` 定义的有效路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    /// 已创建，尚未就绪
    Created = 0,
    /// 就绪，等待调度
    Ready   = 1,
    /// 正在运行
    Running = 2,
    /// 阻塞（等待 I/O、锁、信号等）
    Blocked = 3,
    /// 已终止，等待回收
    Zombie  = 4,
}

impl TaskState {
    /// 检查状态转换是否合法
    ///
    /// 有效转换路径：
    /// - Created -> Ready
    /// - Ready -> Running
    /// - Running -> Ready（被抢占或主动让出）
    /// - Running -> Blocked（等待资源）
    /// - Running -> Zombie（任务退出）
    /// - Blocked -> Ready（等待完成被唤醒）
    /// - Zombie -> Created（回收后复用，可选）
    pub fn can_transition(self, to: TaskState) -> bool {
        match (self, to) {
            // Created -> Ready：初始就绪
            (TaskState::Created, TaskState::Ready) => true,
            // Ready -> Running：被调度器选中
            (TaskState::Ready, TaskState::Running) => true,
            // Running -> Ready：被抢占或 yield
            (TaskState::Running, TaskState::Ready) => true,
            // Running -> Blocked：等待资源
            (TaskState::Running, TaskState::Blocked) => true,
            // Running -> Zombie：任务退出
            (TaskState::Running, TaskState::Zombie) => true,
            // Blocked -> Ready：被唤醒
            (TaskState::Blocked, TaskState::Ready) => true,
            // 相同状态转换（幂等）
            (a, b) if a == b => true,
            // 其他所有转换非法
            _ => false,
        }
    }
}

bitflags! {
    /// 任务标志位
    ///
    /// 用于标记任务的属性和状态标志。
    pub struct TaskFlags: u64 {
        /// 需要重新调度
        const NEED_RESCHED = 1 << 0;
        /// 任务已退出
        const EXITED       = 1 << 1;
        /// 内核任务
        const IS_KERNEL    = 1 << 2;
        /// Agent 任务
        const IS_AGENT     = 1 << 3;
        /// 空闲任务
        const IS_IDLE      = 1 << 4;
    }
}

/// 上下文帧
///
/// 保存 x86_64 架构的完整寄存器上下文，用于任务切换时
/// 保存和恢复 CPU 状态。
///
/// 寄存器布局遵循 System V AMD64 ABI 的调用约定。
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ContextFrame {
    // 通用寄存器（调用者保存）
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    // 参数/数据寄存器
    pub rsi: u64,
    pub rdi: u64,
    // 栈帧指针
    pub rbp: u64,
    // 通用寄存器（调用者保存）
    pub r8:  u64,
    pub r9:  u64,
    pub r10: u64,
    pub r11: u64,
    // 被调用者保存寄存器
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // 指令指针和段寄存器
    pub rip:    u64,
    pub cs:     u64,
    pub rflags: u64,
    // 栈指针和栈段
    pub rsp:    u64,
    pub ss:     u64,
}

impl ContextFrame {
    /// 创建新的上下文帧
    ///
    /// 设置入口点和栈顶，并根据是否为用户态任务配置 CS/SS 段寄存器。
    ///
    /// # 参数
    /// - `entry_point`: 任务入口地址
    /// - `stack_top`: 栈顶地址
    /// - `is_user`: 是否为用户态任务
    pub fn new(entry_point: u64, stack_top: u64, is_user: bool) -> Self {
        if is_user {
            // 用户态段选择子（RPL=3）
            ContextFrame {
                rax: 0,
                rbx: 0,
                rcx: 0,
                rdx: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                r8: 0,
                r9: 0,
                r10: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rip: entry_point,
                cs: 0x23,       // 用户态代码段 (GDT 索引 4, RPL=3)
                rflags: 0x202,  // IF 标志置位
                rsp: stack_top,
                ss: 0x2B,       // 用户态数据段 (GDT 索引 5, RPL=3)
            }
        } else {
            // 内核态段选择子（RPL=0）
            ContextFrame {
                rax: 0,
                rbx: 0,
                rcx: 0,
                rdx: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                r8: 0,
                r9: 0,
                r10: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rip: entry_point,
                cs: 0x08,       // 内核态代码段 (GDT 索引 1, RPL=0)
                rflags: 0x202,  // IF 标志置位
                rsp: stack_top,
                ss: 0x10,       // 内核态数据段 (GDT 索引 2, RPL=0)
            }
        }
    }
}

/// 任务控制块
///
/// 内核中每个任务的完整运行时表示，包含调度信息、
/// 上下文帧、Agent 关联和父子关系等。
#[derive(Clone)]
pub struct TaskControlBlock {
    /// 任务 ID
    pub id: TaskId,
    /// 当前任务状态
    pub state: TaskState,
    /// 任务标志位
    pub flags: TaskFlags,
    /// 调度信息
    pub sched_info: SchedInfo,
    /// CPU 上下文帧
    pub context: ContextFrame,
    /// 关联的 Agent 句柄（可选）
    pub agent_handle: Option<u64>,
    /// 退出码
    pub exit_code: i32,
    /// 父任务 ID（可选）
    pub parent_id: Option<TaskId>,
    /// 子任务 ID 列表
    pub children: Vec<TaskId>,
    /// 等待通道（用于 sleep/wakeup）
    pub wait_channel: u64,
    /// 创建时间（tick 计数）
    pub create_time: u64,
}

impl TaskControlBlock {
    /// 创建新的任务控制块
    ///
    /// 初始化任务 ID、入口点、栈、优先级和调度信息。
    /// 初始状态为 Created。
    ///
    /// # 参数
    /// - `id`: 任务 ID
    /// - `entry`: 入口地址
    /// - `stack_top`: 栈顶地址
    /// - `priority`: 优先级类
    /// - `is_user`: 是否为用户态任务
    /// - `agent_handle`: 关联的 Agent 句柄
    pub fn new(
        id: TaskId,
        entry: u64,
        stack_top: u64,
        priority: PriorityClass,
        is_user: bool,
        agent_handle: Option<u64>,
    ) -> Self {
        let mut flags = TaskFlags::empty();
        if is_user {
            // 用户态任务默认无特殊标志
        } else {
            flags.insert(TaskFlags::IS_KERNEL);
        }

        TaskControlBlock {
            id,
            state: TaskState::Created,
            flags,
            sched_info: SchedInfo::new(priority),
            context: ContextFrame::new(entry, stack_top, is_user),
            agent_handle,
            exit_code: 0,
            parent_id: None,
            children: Vec::new(),
            wait_channel: 0,
            create_time: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 7：任务状态转换验证
    #[test]
    fn test_task_state_transitions() {
        // Created -> Ready
        assert!(TaskState::Created.can_transition(TaskState::Ready));

        // Ready -> Running
        assert!(TaskState::Ready.can_transition(TaskState::Running));

        // Running -> Ready（yield/抢占）
        assert!(TaskState::Running.can_transition(TaskState::Ready));

        // Running -> Blocked（等待）
        assert!(TaskState::Running.can_transition(TaskState::Blocked));

        // Running -> Zombie（退出）
        assert!(TaskState::Running.can_transition(TaskState::Zombie));

        // Blocked -> Ready（唤醒）
        assert!(TaskState::Blocked.can_transition(TaskState::Ready));

        // 幂等转换（相同状态）
        assert!(TaskState::Created.can_transition(TaskState::Created));
        assert!(TaskState::Ready.can_transition(TaskState::Ready));
        assert!(TaskState::Running.can_transition(TaskState::Running));
        assert!(TaskState::Blocked.can_transition(TaskState::Blocked));
        assert!(TaskState::Zombie.can_transition(TaskState::Zombie));
    }

    /// 测试 8：非法状态转换
    #[test]
    fn test_task_state_invalid_transitions() {
        // Created -> Running（必须先 Ready）
        assert!(!TaskState::Created.can_transition(TaskState::Running));

        // Created -> Blocked
        assert!(!TaskState::Created.can_transition(TaskState::Blocked));

        // Created -> Zombie
        assert!(!TaskState::Created.can_transition(TaskState::Zombie));

        // Ready -> Blocked（必须先 Running）
        assert!(!TaskState::Ready.can_transition(TaskState::Blocked));

        // Ready -> Zombie
        assert!(!TaskState::Ready.can_transition(TaskState::Zombie));

        // Blocked -> Running（必须先 Ready）
        assert!(!TaskState::Blocked.can_transition(TaskState::Running));

        // Blocked -> Zombie
        assert!(!TaskState::Blocked.can_transition(TaskState::Zombie));

        // Zombie -> Ready
        assert!(!TaskState::Zombie.can_transition(TaskState::Ready));

        // Zombie -> Running
        assert!(!TaskState::Zombie.can_transition(TaskState::Running));

        // Zombie -> Blocked
        assert!(!TaskState::Zombie.can_transition(TaskState::Blocked));
    }

    /// 测试 9：内核上下文帧创建
    #[test]
    fn test_context_frame_new_kernel() {
        let frame = ContextFrame::new(0x1000, 0x8000, false);

        // 验证入口点和栈
        assert_eq!(frame.rip, 0x1000);
        assert_eq!(frame.rsp, 0x8000);

        // 验证内核态段选择子
        assert_eq!(frame.cs, 0x08);
        assert_eq!(frame.ss, 0x10);

        // 验证 RFLAGS（IF 标志置位）
        assert_eq!(frame.rflags, 0x202);

        // 验证通用寄存器初始化为 0
        assert_eq!(frame.rax, 0);
        assert_eq!(frame.rbx, 0);
        assert_eq!(frame.rcx, 0);
        assert_eq!(frame.rdx, 0);
        assert_eq!(frame.rdi, 0);
        assert_eq!(frame.rsi, 0);
    }

    /// 测试 10：用户上下文帧创建
    #[test]
    fn test_context_frame_new_user() {
        let frame = ContextFrame::new(0x400000, 0x7FFFFFFFFFF0, true);

        // 验证入口点和栈
        assert_eq!(frame.rip, 0x400000);
        assert_eq!(frame.rsp, 0x7FFFFFFFFFF0);

        // 验证用户态段选择子
        assert_eq!(frame.cs, 0x23);
        assert_eq!(frame.ss, 0x2B);

        // 验证 RFLAGS
        assert_eq!(frame.rflags, 0x202);
    }

    /// 测试 11：创建任务控制块
    #[test]
    fn test_tcb_new() {
        let tcb = TaskControlBlock::new(
            TaskId(1),
            0x1000,
            0x8000,
            PriorityClass::Normal,
            true,
            Some(42),
        );

        // 验证基本字段
        assert_eq!(tcb.id, TaskId(1));
        assert_eq!(tcb.state, TaskState::Created);
        assert_eq!(tcb.agent_handle, Some(42));
        assert_eq!(tcb.exit_code, 0);
        assert_eq!(tcb.parent_id, None);
        assert!(tcb.children.is_empty());
        assert_eq!(tcb.wait_channel, 0);

        // 验证上下文帧
        assert_eq!(tcb.context.rip, 0x1000);
        assert_eq!(tcb.context.rsp, 0x8000);

        // 验证调度信息
        assert_eq!(tcb.sched_info.priority, PriorityClass::Normal);
        assert_eq!(tcb.sched_info.weight, 1024);
        assert_eq!(tcb.sched_info.vruntime, 0);
        assert_eq!(tcb.sched_info.runtime, 0);

        // 用户态任务不应有 IS_KERNEL 标志
        assert!(!tcb.flags.contains(TaskFlags::IS_KERNEL));

        // 内核态任务应有 IS_KERNEL 标志
        let kernel_tcb = TaskControlBlock::new(
            TaskId(2),
            0x2000,
            0x9000,
            PriorityClass::High,
            false,
            None,
        );
        assert!(kernel_tcb.flags.contains(TaskFlags::IS_KERNEL));
        assert_eq!(kernel_tcb.agent_handle, None);
    }

    /// 测试 12：任务标志位操作
    #[test]
    fn test_task_flags() {
        let mut flags = TaskFlags::empty();
        assert!(flags.is_empty());

        // 设置标志
        flags.insert(TaskFlags::NEED_RESCHED);
        assert!(flags.contains(TaskFlags::NEED_RESCHED));
        assert!(!flags.contains(TaskFlags::IS_KERNEL));

        // 设置多个标志
        flags.insert(TaskFlags::IS_AGENT);
        flags.insert(TaskFlags::IS_IDLE);
        assert!(flags.contains(TaskFlags::NEED_RESCHED));
        assert!(flags.contains(TaskFlags::IS_AGENT));
        assert!(flags.contains(TaskFlags::IS_IDLE));

        // 清除标志
        flags.remove(TaskFlags::NEED_RESCHED);
        assert!(!flags.contains(TaskFlags::NEED_RESCHED));
        assert!(flags.contains(TaskFlags::IS_AGENT));

        // 位运算
        let combined = TaskFlags::IS_KERNEL | TaskFlags::IS_AGENT;
        assert!(combined.contains(TaskFlags::IS_KERNEL));
        assert!(combined.contains(TaskFlags::IS_AGENT));
        assert!(!combined.contains(TaskFlags::IS_IDLE));

        // 交集检查
        assert!(combined.intersects(TaskFlags::IS_KERNEL));
        assert!(!combined.intersects(TaskFlags::IS_IDLE));
    }
}
