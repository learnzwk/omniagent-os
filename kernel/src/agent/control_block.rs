//! Agent 控制块 (ACB)
//!
//! 内核中每个 Agent 的运行时状态表示。
//! ACB 包含 Agent 的标识信息、运行时状态、资源统计、
//! 安全属性和调度参数等核心数据。

use crate::syscall::abi::*;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Agent 控制块
///
/// 每个 Agent 在内核中都有一个对应的 ACB 实例，
/// 用于跟踪 Agent 的完整生命周期状态。
pub struct AgentControlBlock {
    // === 基本标识 ===
    /// Agent 句柄 (内核分配的唯一标识)
    pub handle: AgentHandle,
    /// Agent 类型
    pub agent_type: AgentType,
    /// Agent 名称 (UTF-8, 最大 64 字节含 NUL)
    pub name: [u8; 64],
    /// 创建者进程 ID
    pub creator_pid: u64,

    // === 运行时状态 (原子操作) ===
    /// 当前状态 (AgentState as u32)
    pub state: AtomicU32,
    /// 调度优先级 (0-255)
    pub priority: u8,
    /// 调度策略
    pub sched_policy: SchedPolicy,

    // === 资源统计 ===
    /// 创建时间 (纳秒, 从系统启动)
    pub create_time_ns: AtomicU64,
    /// CPU 使用时间 (纳秒)
    pub cpu_time_ns: AtomicU64,
    /// 当前内存使用量 (字节)
    pub memory_used: AtomicU64,
    /// 峰值内存使用量 (字节)
    pub memory_peak: AtomicU64,
    /// 已发送消息计数
    pub msg_sent: AtomicU64,
    /// 已接收消息计数
    pub msg_received: AtomicU64,
    /// 最后活跃时间 (纳秒)
    pub last_active_ns: AtomicU64,

    // === 资源限制 ===
    /// 内存使用上限 (字节, 0 表示无限制)
    pub memory_limit: u64,
    /// 最大文件描述符数
    pub max_fds: u32,
    /// 最大线程数
    pub max_threads: u32,

    // === 安全 ===
    /// 能力位图
    pub capabilities: CapBitmap,
    /// 安全标签 (用于强制访问控制)
    pub security_label: [u8; 32],

    // === 连接 ===
    /// 当前线程数
    pub thread_count: AtomicU32,
    /// 当前连接数
    pub connection_count: AtomicU32,

    // === 配置 ===
    /// 入口点地址
    pub entry_point: u64,
    /// 代码段大小 (字节)
    pub code_size: u64,
    /// 堆大小 (字节)
    pub heap_size: u64,
    /// 栈大小 (字节)
    pub stack_size: u64,
    /// CPU 亲和性掩码
    pub cpu_affinity: u64,
    /// 标志位
    pub flags: u32,

    // === 调度 ===
    /// 当前运行所在 CPU 核心
    pub current_cpu: AtomicU32,
    /// 通信端口数量
    pub port_count: u16,
}

impl Clone for AgentControlBlock {
    fn clone(&self) -> Self {
        AgentControlBlock {
            handle: self.handle,
            agent_type: self.agent_type,
            name: self.name,
            creator_pid: self.creator_pid,
            state: AtomicU32::new(self.state.load(Ordering::Relaxed)),
            priority: self.priority,
            sched_policy: self.sched_policy,
            create_time_ns: AtomicU64::new(self.create_time_ns.load(Ordering::Relaxed)),
            cpu_time_ns: AtomicU64::new(self.cpu_time_ns.load(Ordering::Relaxed)),
            memory_used: AtomicU64::new(self.memory_used.load(Ordering::Relaxed)),
            memory_peak: AtomicU64::new(self.memory_peak.load(Ordering::Relaxed)),
            msg_sent: AtomicU64::new(self.msg_sent.load(Ordering::Relaxed)),
            msg_received: AtomicU64::new(self.msg_received.load(Ordering::Relaxed)),
            last_active_ns: AtomicU64::new(self.last_active_ns.load(Ordering::Relaxed)),
            memory_limit: self.memory_limit,
            max_fds: self.max_fds,
            max_threads: self.max_threads,
            capabilities: self.capabilities,
            security_label: self.security_label,
            thread_count: AtomicU32::new(self.thread_count.load(Ordering::Relaxed)),
            connection_count: AtomicU32::new(self.connection_count.load(Ordering::Relaxed)),
            entry_point: self.entry_point,
            code_size: self.code_size,
            heap_size: self.heap_size,
            stack_size: self.stack_size,
            cpu_affinity: self.cpu_affinity,
            flags: self.flags,
            current_cpu: AtomicU32::new(self.current_cpu.load(Ordering::Relaxed)),
            port_count: self.port_count,
        }
    }
}

impl AgentControlBlock {
    /// 从 AgentSpec 创建新的 ACB
    ///
    /// 初始化所有字段，状态设为 Creating。
    pub fn new(handle: AgentHandle, spec: &AgentSpec, creator_pid: u64) -> Self {
        AgentControlBlock {
            // 基本标识
            handle,
            agent_type: spec.agent_type,
            name: spec.name,
            creator_pid,

            // 运行时状态
            state: AtomicU32::new(AgentState::Creating as u32),
            priority: spec.priority,
            sched_policy: spec.sched_policy,

            // 资源统计
            create_time_ns: AtomicU64::new(0), // 由调用者设置
            cpu_time_ns: AtomicU64::new(0),
            memory_used: AtomicU64::new(0),
            memory_peak: AtomicU64::new(0),
            msg_sent: AtomicU64::new(0),
            msg_received: AtomicU64::new(0),
            last_active_ns: AtomicU64::new(0),

            // 资源限制
            memory_limit: spec.memory_limit,
            max_fds: spec.max_fds,
            max_threads: spec.quota.max_threads,

            // 安全
            capabilities: spec.capabilities,
            security_label: spec.security_label,

            // 连接
            thread_count: AtomicU32::new(0),
            connection_count: AtomicU32::new(0),

            // 配置
            entry_point: spec.entry_point,
            code_size: spec.code_size,
            heap_size: spec.heap_size,
            stack_size: spec.stack_size,
            cpu_affinity: spec.cpu_affinity,
            flags: spec.flags,

            // 调度
            current_cpu: AtomicU32::new(0),
            port_count: spec.port_count,
        }
    }

    /// 获取当前状态
    pub fn state(&self) -> AgentState {
        AgentState::from_u32(self.state.load(Ordering::Acquire))
    }

    /// 设置状态 (带状态转换验证)
    ///
    /// 合法状态转换:
    /// - Creating -> Ready
    /// - Ready -> Running
    /// - Running -> Waiting
    /// - Running -> Frozen
    /// - Running -> Terminating
    /// - Waiting -> Running
    /// - Waiting -> Terminating
    /// - Frozen -> Running
    /// - Frozen -> Terminating
    /// - Terminating -> Terminated
    /// - 任何 -> Failed
    pub fn set_state(&self, new_state: AgentState) -> Result<(), SyscallError> {
        let current = self.state();
        let is_valid = match (current, new_state) {
            // Creating -> Ready (初始创建完成)
            (AgentState::Creating, AgentState::Ready) => true,
            // Ready -> Running (调度器启动)
            (AgentState::Ready, AgentState::Running) => true,
            // Running -> Waiting (I/O 等待)
            (AgentState::Running, AgentState::Waiting) => true,
            // Running -> Frozen (冻结)
            (AgentState::Running, AgentState::Frozen) => true,
            // Running -> Terminating (终止)
            (AgentState::Running, AgentState::Terminating) => true,
            // Waiting -> Running (I/O 完成)
            (AgentState::Waiting, AgentState::Running) => true,
            // Waiting -> Terminating (终止)
            (AgentState::Waiting, AgentState::Terminating) => true,
            // Frozen -> Running (解冻)
            (AgentState::Frozen, AgentState::Running) => true,
            // Frozen -> Terminating (终止)
            (AgentState::Frozen, AgentState::Terminating) => true,
            // Terminating -> Terminated (清理完成)
            (AgentState::Terminating, AgentState::Terminated) => true,
            // 任何状态 -> Failed (错误)
            (_, AgentState::Failed) => true,
            // 相同状态 (幂等操作)
            (a, b) if a == b => true,
            // 其他所有转换都是非法的
            _ => false,
        };

        if is_valid {
            self.state.store(new_state as u32, Ordering::Release);
            Ok(())
        } else {
            Err(SyscallError::EAGENT_INVALID_STATE)
        }
    }

    /// 增加已发送消息计数
    pub fn inc_msg_sent(&self) {
        self.msg_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加已接收消息计数
    pub fn inc_msg_received(&self) {
        self.msg_received.fetch_add(1, Ordering::Relaxed);
    }

    /// 更新内存使用量
    ///
    /// 同时更新峰值内存记录。
    pub fn update_memory(&self, used: u64) {
        self.memory_used.store(used, Ordering::Relaxed);
        // 原子地更新峰值: 如果当前峰值小于新值，则更新
        let mut current_peak = self.memory_peak.load(Ordering::Relaxed);
        loop {
            if used <= current_peak {
                break;
            }
            match self.memory_peak.compare_exchange_weak(
                current_peak,
                used,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
    }

    /// 增加 CPU 时间
    pub fn add_cpu_time(&self, ns: u64) {
        self.cpu_time_ns.fetch_add(ns, Ordering::Relaxed);
    }

    /// 检查是否具有指定能力
    pub fn has_capability(&self, cap: usize) -> bool {
        self.capabilities.test(cap)
    }

    /// 填充 AgentInfo 结构体
    ///
    /// 将 ACB 的当前状态快照到 AgentInfo 中。
    pub fn to_info(&self) -> AgentInfo {
        AgentInfo {
            handle: self.handle,
            state: self.state(),
            agent_type: self.agent_type,
            name: self.name,
            creator_pid: self.creator_pid,
            create_time_ns: self.create_time_ns.load(Ordering::Relaxed),
            cpu_time_ns: self.cpu_time_ns.load(Ordering::Relaxed),
            memory_used: self.memory_used.load(Ordering::Relaxed),
            memory_peak: self.memory_peak.load(Ordering::Relaxed),
            thread_count: self.thread_count.load(Ordering::Relaxed),
            connection_count: self.connection_count.load(Ordering::Relaxed),
            msg_sent: self.msg_sent.load(Ordering::Relaxed),
            msg_received: self.msg_received.load(Ordering::Relaxed),
            last_active_ns: self.last_active_ns.load(Ordering::Relaxed),
            security_label: self.security_label,
            current_cpu: self.current_cpu.load(Ordering::Relaxed),
            _pad: [0; 4],
            _reserved: [0; 72],
        }
    }

    /// 检查资源配额
    ///
    /// 返回 true 表示资源使用在配额范围内。
    pub fn check_quota(&self) -> bool {
        // 检查内存限制
        if self.memory_limit > 0 {
            let used = self.memory_used.load(Ordering::Relaxed);
            if used > self.memory_limit {
                return false;
            }
        }
        // 检查线程数限制
        let threads = self.thread_count.load(Ordering::Relaxed);
        if self.max_threads > 0 && threads > self.max_threads {
            return false;
        }
        true
    }

    /// 设置创建时间
    pub fn set_create_time(&self, ns: u64) {
        self.create_time_ns.store(ns, Ordering::Relaxed);
    }

    /// 更新最后活跃时间
    pub fn touch(&self, ns: u64) {
        self.last_active_ns.store(ns, Ordering::Relaxed);
    }

    /// 增加线程计数
    pub fn inc_thread_count(&self) {
        self.thread_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 减少线程计数
    pub fn dec_thread_count(&self) {
        self.thread_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// 增加连接计数
    pub fn inc_connection_count(&self) {
        self.connection_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 减少连接计数
    pub fn dec_connection_count(&self) {
        self.connection_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// 设置当前 CPU
    pub fn set_current_cpu(&self, cpu: u32) {
        self.current_cpu.store(cpu, Ordering::Relaxed);
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个测试用的 AgentSpec
    fn make_test_spec() -> AgentSpec {
        let mut spec = AgentSpec::default();
        // 设置名称 "test_agent\0"
        let name = b"test_agent";
        spec.name[..name.len()].copy_from_slice(name);
        spec.agent_type = AgentType::Generic;
        spec.priority = 128;
        spec.entry_point = 0x1000;
        spec.code_size = 0x2000;
        spec.heap_size = 0x100000;
        spec.stack_size = 0x80000;
        spec.memory_limit = 0x4000000;
        spec.max_fds = 64;
        spec.port_count = 4;
        spec.flags = 0;
        spec
    }

    #[test]
    fn test_acb_new_from_spec() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 42);

        assert_eq!(acb.handle, handle);
        assert_eq!(acb.agent_type, AgentType::Generic);
        assert_eq!(acb.creator_pid, 42);
        assert_eq!(acb.priority, 128);
        assert_eq!(acb.entry_point, 0x1000);
        assert_eq!(acb.code_size, 0x2000);
        assert_eq!(acb.heap_size, 0x100000);
        assert_eq!(acb.stack_size, 0x80000);
        assert_eq!(acb.memory_limit, 0x4000000);
        assert_eq!(acb.max_fds, 64);
        assert_eq!(acb.port_count, 4);
        assert_eq!(acb.flags, 0);

        // 验证名称复制
        let name_str = core::str::from_utf8(&acb.name).unwrap_or("");
        assert!(name_str.starts_with("test_agent"));
    }

    #[test]
    fn test_acb_initial_state() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // 初始状态应为 Creating
        assert_eq!(acb.state(), AgentState::Creating);

        // 初始计数器应为零
        assert_eq!(acb.cpu_time_ns.load(Ordering::Relaxed), 0);
        assert_eq!(acb.memory_used.load(Ordering::Relaxed), 0);
        assert_eq!(acb.memory_peak.load(Ordering::Relaxed), 0);
        assert_eq!(acb.msg_sent.load(Ordering::Relaxed), 0);
        assert_eq!(acb.msg_received.load(Ordering::Relaxed), 0);
        assert_eq!(acb.thread_count.load(Ordering::Relaxed), 0);
        assert_eq!(acb.connection_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_acb_state_transitions() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // Creating -> Ready
        assert!(acb.set_state(AgentState::Ready).is_ok());
        assert_eq!(acb.state(), AgentState::Ready);

        // Ready -> Running
        assert!(acb.set_state(AgentState::Running).is_ok());
        assert_eq!(acb.state(), AgentState::Running);

        // Running -> Waiting
        assert!(acb.set_state(AgentState::Waiting).is_ok());
        assert_eq!(acb.state(), AgentState::Waiting);

        // Waiting -> Running
        assert!(acb.set_state(AgentState::Running).is_ok());
        assert_eq!(acb.state(), AgentState::Running);

        // Running -> Frozen
        assert!(acb.set_state(AgentState::Frozen).is_ok());
        assert_eq!(acb.state(), AgentState::Frozen);

        // Frozen -> Running
        assert!(acb.set_state(AgentState::Running).is_ok());
        assert_eq!(acb.state(), AgentState::Running);

        // Running -> Terminating
        assert!(acb.set_state(AgentState::Terminating).is_ok());
        assert_eq!(acb.state(), AgentState::Terminating);

        // Terminating -> Terminated
        assert!(acb.set_state(AgentState::Terminated).is_ok());
        assert_eq!(acb.state(), AgentState::Terminated);
    }

    #[test]
    fn test_acb_invalid_state_transition() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // Creating -> Running (非法, 必须先 Ready)
        assert_eq!(
            acb.set_state(AgentState::Running),
            Err(SyscallError::EAGENT_INVALID_STATE)
        );
        assert_eq!(acb.state(), AgentState::Creating);

        // Creating -> Terminating (非法)
        assert_eq!(
            acb.set_state(AgentState::Terminating),
            Err(SyscallError::EAGENT_INVALID_STATE)
        );

        // Creating -> Terminated (非法)
        assert_eq!(
            acb.set_state(AgentState::Terminated),
            Err(SyscallError::EAGENT_INVALID_STATE)
        );

        // 先进入 Ready 再测试
        acb.set_state(AgentState::Ready).unwrap();

        // Ready -> Waiting (非法, 必须先 Running)
        assert_eq!(
            acb.set_state(AgentState::Waiting),
            Err(SyscallError::EAGENT_INVALID_STATE)
        );

        // Ready -> Frozen (非法)
        assert_eq!(
            acb.set_state(AgentState::Frozen),
            Err(SyscallError::EAGENT_INVALID_STATE)
        );
    }

    #[test]
    fn test_acb_any_to_failed() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // Creating -> Failed
        assert!(acb.set_state(AgentState::Failed).is_ok());
        assert_eq!(acb.state(), AgentState::Failed);
    }

    #[test]
    fn test_acb_idempotent_state() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // 相同状态设置应该是幂等的
        assert!(acb.set_state(AgentState::Creating).is_ok());
        assert_eq!(acb.state(), AgentState::Creating);
    }

    #[test]
    fn test_acb_capabilities() {
        let handle = AgentHandle(1);
        let mut spec = make_test_spec();
        spec.capabilities.set(0);
        spec.capabilities.set(5);
        spec.capabilities.set(127);

        let acb = AgentControlBlock::new(handle, &spec, 0);

        assert!(acb.has_capability(0));
        assert!(acb.has_capability(5));
        assert!(acb.has_capability(127));
        assert!(!acb.has_capability(1));
        assert!(!acb.has_capability(128)); // 超出范围
    }

    #[test]
    fn test_acb_message_counters() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // 初始计数为零
        assert_eq!(acb.msg_sent.load(Ordering::Relaxed), 0);
        assert_eq!(acb.msg_received.load(Ordering::Relaxed), 0);

        // 增加发送计数
        acb.inc_msg_sent();
        acb.inc_msg_sent();
        acb.inc_msg_sent();
        assert_eq!(acb.msg_sent.load(Ordering::Relaxed), 3);

        // 增加接收计数
        acb.inc_msg_received();
        acb.inc_msg_received();
        assert_eq!(acb.msg_received.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_acb_memory_tracking() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // 初始内存为零
        assert_eq!(acb.memory_used.load(Ordering::Relaxed), 0);
        assert_eq!(acb.memory_peak.load(Ordering::Relaxed), 0);

        // 更新内存使用
        acb.update_memory(1024);
        assert_eq!(acb.memory_used.load(Ordering::Relaxed), 1024);
        assert_eq!(acb.memory_peak.load(Ordering::Relaxed), 1024);

        // 增加内存使用
        acb.update_memory(2048);
        assert_eq!(acb.memory_used.load(Ordering::Relaxed), 2048);
        assert_eq!(acb.memory_peak.load(Ordering::Relaxed), 2048);

        // 减少内存使用 (峰值不应减少)
        acb.update_memory(512);
        assert_eq!(acb.memory_used.load(Ordering::Relaxed), 512);
        assert_eq!(acb.memory_peak.load(Ordering::Relaxed), 2048);
    }

    #[test]
    fn test_acb_cpu_time() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        acb.add_cpu_time(1000);
        acb.add_cpu_time(2000);
        assert_eq!(acb.cpu_time_ns.load(Ordering::Relaxed), 3000);
    }

    #[test]
    fn test_acb_to_info() {
        let handle = AgentHandle(42);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 100);

        // 设置一些运行时数据
        acb.set_create_time(12345);
        acb.add_cpu_time(999);
        acb.update_memory(4096);
        acb.inc_msg_sent();
        acb.inc_msg_received();
        acb.inc_thread_count();
        acb.inc_connection_count();
        acb.set_current_cpu(3);
        acb.touch(99999);

        let info = acb.to_info();

        assert_eq!(info.handle, handle);
        assert_eq!(info.state, AgentState::Creating);
        assert_eq!(info.agent_type, AgentType::Generic);
        assert_eq!(info.creator_pid, 100);
        assert_eq!(info.create_time_ns, 12345);
        assert_eq!(info.cpu_time_ns, 999);
        assert_eq!(info.memory_used, 4096);
        assert_eq!(info.memory_peak, 4096);
        assert_eq!(info.thread_count, 1);
        assert_eq!(info.connection_count, 1);
        assert_eq!(info.msg_sent, 1);
        assert_eq!(info.msg_received, 1);
        assert_eq!(info.last_active_ns, 99999);
        assert_eq!(info.current_cpu, 3);
    }

    #[test]
    fn test_acb_check_quota_ok() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // 内存使用在限制内
        acb.update_memory(1024);
        assert!(acb.check_quota());
    }

    #[test]
    fn test_acb_check_quota_exceeded() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        // 内存使用超过限制
        acb.update_memory(spec.memory_limit + 1);
        assert!(!acb.check_quota());
    }

    #[test]
    fn test_acb_thread_connection_counts() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        acb.inc_thread_count();
        acb.inc_thread_count();
        assert_eq!(acb.thread_count.load(Ordering::Relaxed), 2);

        acb.dec_thread_count();
        assert_eq!(acb.thread_count.load(Ordering::Relaxed), 1);

        acb.inc_connection_count();
        acb.inc_connection_count();
        acb.inc_connection_count();
        assert_eq!(acb.connection_count.load(Ordering::Relaxed), 3);

        acb.dec_connection_count();
        assert_eq!(acb.connection_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_acb_waiting_to_terminating() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        acb.set_state(AgentState::Ready).unwrap();
        acb.set_state(AgentState::Running).unwrap();
        acb.set_state(AgentState::Waiting).unwrap();

        // Waiting -> Terminating (合法)
        assert!(acb.set_state(AgentState::Terminating).is_ok());
        assert_eq!(acb.state(), AgentState::Terminating);
    }

    #[test]
    fn test_acb_frozen_to_terminating() {
        let handle = AgentHandle(1);
        let spec = make_test_spec();
        let acb = AgentControlBlock::new(handle, &spec, 0);

        acb.set_state(AgentState::Ready).unwrap();
        acb.set_state(AgentState::Running).unwrap();
        acb.set_state(AgentState::Frozen).unwrap();

        // Frozen -> Terminating (合法)
        assert!(acb.set_state(AgentState::Terminating).is_ok());
        assert_eq!(acb.state(), AgentState::Terminating);
    }
}
