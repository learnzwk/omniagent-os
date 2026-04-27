//! libagent - OmniAgent OS 用户态 Agent Runtime 库
//!
//! 提供 Agent 生命周期管理、消息通信、能力控制、资源配额等用户态 API。
//! 作为 Agent 与内核之间的桥梁，通过 syscall ABI 与内核交互。
//!
//! # 架构
//!
//! ```text
//! 用户态 Agent 代码
//!       │
//!       ▼
//!   libagent (本 crate)
//!   ├── AgentConfig    - 配置构建器
//!   ├── AgentRuntime   - 运行时 API (spawn/kill/query/send/receive/...)
//!   ├── AgentHandle    - Agent 句柄
//!   ├── AgentMessage   - 消息类型
//!   ├── MsgFlags       - 消息标志
//!   ├── Signal         - 终止信号
//!   ├── EventMask      - 事件掩码
//!   ├── ResourceQuota  - 资源配额
//!   └── 错误类型        - RuntimeError / ConfigError
//!       │
//!       ▼
//!   extern "C" syscall 接口
//!       │
//!       ▼
//!   内核 Agent 子系统
//! ```

use std::collections::HashMap;
use std::fmt;

// ============================================================================
// 内核 ABI 类型重导出 (与 kernel/src/syscall/abi.rs 完全一致)
// ============================================================================

// === Agent 类型 (与内核 ABI 一致的 7 种类型) ===
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AgentType {
    Generic = 0,
    AIInference = 1,
    DataProcessing = 2,
    Network = 3,
    System = 4,
    Sandbox = 5,
    Virtualization = 6,
}

impl AgentType {
    /// 从 u32 值创建 AgentType
    ///
    /// 无效值返回 Generic 作为安全默认值。
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => AgentType::Generic,
            1 => AgentType::AIInference,
            2 => AgentType::DataProcessing,
            3 => AgentType::Network,
            4 => AgentType::System,
            5 => AgentType::Sandbox,
            6 => AgentType::Virtualization,
            _ => AgentType::Generic,
        }
    }
}

impl Default for AgentType {
    fn default() -> Self {
        AgentType::Generic
    }
}

impl fmt::Display for AgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentType::Generic => write!(f, "Generic"),
            AgentType::AIInference => write!(f, "AIInference"),
            AgentType::DataProcessing => write!(f, "DataProcessing"),
            AgentType::Network => write!(f, "Network"),
            AgentType::System => write!(f, "System"),
            AgentType::Sandbox => write!(f, "Sandbox"),
            AgentType::Virtualization => write!(f, "Virtualization"),
        }
    }
}

// === Agent 状态 (与内核 ABI 一致的 9 种状态) ===
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AgentState {
    Creating = 0,
    Ready = 1,
    Running = 2,
    Waiting = 3,
    Frozen = 4,
    Migrating = 5,
    Terminating = 6,
    Terminated = 7,
    Failed = 8,
}

impl AgentState {
    /// 从 u32 值创建 AgentState
    ///
    /// 无效值返回 Creating 作为安全默认值。
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => AgentState::Creating,
            1 => AgentState::Ready,
            2 => AgentState::Running,
            3 => AgentState::Waiting,
            4 => AgentState::Frozen,
            5 => AgentState::Migrating,
            6 => AgentState::Terminating,
            7 => AgentState::Terminated,
            8 => AgentState::Failed,
            _ => AgentState::Creating,
        }
    }
}

impl Default for AgentState {
    fn default() -> Self {
        AgentState::Creating
    }
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentState::Creating => write!(f, "Creating"),
            AgentState::Ready => write!(f, "Ready"),
            AgentState::Running => write!(f, "Running"),
            AgentState::Waiting => write!(f, "Waiting"),
            AgentState::Frozen => write!(f, "Frozen"),
            AgentState::Migrating => write!(f, "Migrating"),
            AgentState::Terminating => write!(f, "Terminating"),
            AgentState::Terminated => write!(f, "Terminated"),
            AgentState::Failed => write!(f, "Failed"),
        }
    }
}

// === Agent 优先级 (与内核 ABI 一致的 u8 范围 0-255) ===
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AgentPriority {
    Idle = 0,
    Low = 64,
    Normal = 128,
    High = 192,
    Realtime = 255,
}

impl AgentPriority {
    /// 从 u8 值创建 AgentPriority
    ///
    /// 根据值范围映射到最接近的优先级。
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => AgentPriority::Idle,
            1..=63 => AgentPriority::Low,
            64..=127 => AgentPriority::Low,
            128..=191 => AgentPriority::Normal,
            192..=254 => AgentPriority::High,
            255 => AgentPriority::Realtime,
        }
    }
}

impl Default for AgentPriority {
    fn default() -> Self {
        AgentPriority::Normal
    }
}

impl fmt::Display for AgentPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentPriority::Idle => write!(f, "Idle"),
            AgentPriority::Low => write!(f, "Low"),
            AgentPriority::Normal => write!(f, "Normal"),
            AgentPriority::High => write!(f, "High"),
            AgentPriority::Realtime => write!(f, "Realtime"),
        }
    }
}

// === Agent 唯一标识符 ===
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentId(pub u64);

impl AgentId {
    /// 无效 AgentId 常量
    pub const INVALID: AgentId = AgentId(0);
    /// 系统 AgentId 常量
    pub const SYSTEM: AgentId = AgentId(1);

    /// 创建新的 AgentId
    pub fn new(id: u64) -> Self {
        AgentId(id)
    }

    /// 检查 AgentId 是否有效 (非零)
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }

    /// 检查是否为系统 AgentId
    pub fn is_system(&self) -> bool {
        *self == Self::SYSTEM
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::INVALID
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agent({})", self.0)
    }
}

// === Agent 句柄 (不透明，与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct KernelAgentHandle(pub u64);

impl KernelAgentHandle {
    /// 无效句柄常量
    pub const INVALID: KernelAgentHandle = KernelAgentHandle(0);

    /// 检查句柄是否有效 (非零)
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }

    /// 获取句柄的内部索引 (handle - 1)
    ///
    /// 用于数组索引访问。仅对有效句柄有意义。
    pub fn index(&self) -> usize {
        (self.0 - 1) as usize
    }
}

impl Default for KernelAgentHandle {
    fn default() -> Self {
        Self::INVALID
    }
}

// === 能力位图 (128 位，与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CapBitmap {
    pub bits: [u64; 2],
}

impl CapBitmap {
    /// 创建一个全零的能力位图
    pub const fn new() -> Self {
        Self { bits: [0, 0] }
    }

    /// 设置指定位置的能力位
    pub fn set(&mut self, cap: usize) {
        let word = cap / 64;
        let bit = cap % 64;
        if word < 2 {
            self.bits[word] |= 1 << bit;
        }
    }

    /// 清除指定位置的能力位
    pub fn clear(&mut self, cap: usize) {
        let word = cap / 64;
        let bit = cap % 64;
        if word < 2 {
            self.bits[word] &= !(1 << bit);
        }
    }

    /// 检查指定位置的能力位是否已设置
    pub fn is_set(&self, cap: usize) -> bool {
        let word = cap / 64;
        let bit = cap % 64;
        if word < 2 {
            (self.bits[word] & (1 << bit)) != 0
        } else {
            false
        }
    }

    /// 检查指定位置的能力位是否已设置 (is_set 的别名)
    pub fn test(&self, cap: usize) -> bool {
        self.is_set(cap)
    }
}

impl Default for CapBitmap {
    fn default() -> Self {
        Self::new()
    }
}

// === 资源配额 (与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ResourceQuota {
    pub max_memory_bytes: u64,
    pub max_shm_bytes: u64,
    pub max_cpu_percent: u32,
    pub max_fds: u32,
    pub max_threads: u32,
    pub max_msg_per_sec: u32,
}

impl ResourceQuota {
    /// 创建默认资源配额（无限制）
    pub const fn new() -> Self {
        Self {
            max_memory_bytes: 0,
            max_shm_bytes: 0,
            max_cpu_percent: 0,
            max_fds: 0,
            max_threads: 0,
            max_msg_per_sec: 0,
        }
    }
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self::new()
    }
}

// === Agent 标志常量 (与内核 ABI 一致) ===
pub const AGENT_FLAG_NONE: u32 = 0;
pub const AGENT_FLAG_AUTO_START: u32 = 1 << 0;
pub const AGENT_FLAG_PERSISTENT: u32 = 1 << 1;
pub const AGENT_FLAG_ISOLATED: u32 = 1 << 2;
pub const AGENT_FLAG_PRIVILEGED: u32 = 1 << 3;
pub const AGENT_FLAG_ENCLAVED: u32 = 1 << 4;

// === AgentSpec (256 字节，与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AgentSpec {
    pub version: u32,
    pub agent_type: AgentType,
    pub name: [u8; 64],
    pub entry_point: u64,
    pub code_size: u64,
    pub heap_size: u64,
    pub stack_size: u64,
    pub cpu_affinity: u64,
    pub priority: u8,
    pub sched_policy: u8,
    pub _pad0: [u8; 6],
    pub memory_limit: u64,
    pub max_fds: u32,
    pub _pad1: u32,
    pub capabilities: CapBitmap,
    pub port_count: u16,
    pub _pad2: u16,
    pub flags: u32,
    pub quota: ResourceQuota,
    pub security_label: [u8; 32],
    pub init_param: [u8; 32],
}

impl Default for AgentSpec {
    fn default() -> Self {
        Self {
            version: 1,
            agent_type: AgentType::Generic,
            name: [0u8; 64],
            entry_point: 0,
            code_size: 0,
            heap_size: 0,
            stack_size: 0,
            cpu_affinity: 0,
            priority: 128,
            sched_policy: 0,
            _pad0: [0u8; 6],
            memory_limit: 0,
            max_fds: 0,
            _pad1: 0,
            capabilities: CapBitmap::new(),
            port_count: 0,
            _pad2: 0,
            flags: 0,
            quota: ResourceQuota::new(),
            security_label: [0u8; 32],
            init_param: [0u8; 32],
        }
    }
}

// === AgentInfo (264 字节，与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct AgentInfo {
    pub handle: KernelAgentHandle,
    pub state: AgentState,
    pub agent_type: AgentType,
    pub name: [u8; 64],
    pub creator_pid: u64,
    pub create_time_ns: u64,
    pub cpu_time_ns: u64,
    pub memory_used: u64,
    pub memory_peak: u64,
    pub thread_count: u32,
    pub connection_count: u32,
    pub msg_sent: u64,
    pub msg_received: u64,
    pub last_active_ns: u64,
    pub security_label: [u8; 32],
    pub current_cpu: u32,
    pub _pad: [u8; 4],
    pub _reserved: [u8; 72],
}

impl Default for AgentInfo {
    fn default() -> Self {
        Self {
            handle: KernelAgentHandle::INVALID,
            state: AgentState::Creating,
            agent_type: AgentType::Generic,
            name: [0u8; 64],
            creator_pid: 0,
            create_time_ns: 0,
            cpu_time_ns: 0,
            memory_used: 0,
            memory_peak: 0,
            thread_count: 0,
            connection_count: 0,
            msg_sent: 0,
            msg_received: 0,
            last_active_ns: 0,
            security_label: [0u8; 32],
            current_cpu: 0,
            _pad: [0u8; 4],
            _reserved: [0u8; 72],
        }
    }
}

// === AgentMsgHeader (48 字节，与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AgentMsgHeader {
    pub msg_type: u32,
    pub flags: u32,
    pub msg_id: u64,
    pub timestamp_ns: u64,
    pub payload_size: u64,
    pub shm_region_id: u32,
    pub priority: u8,
    pub reserved: [u8; 7],
    pub _pad: [u8; 4],
}

impl Default for AgentMsgHeader {
    fn default() -> Self {
        Self {
            msg_type: 0,
            flags: 0,
            msg_id: 0,
            timestamp_ns: 0,
            payload_size: 0,
            shm_region_id: 0,
            priority: 0,
            reserved: [0u8; 7],
            _pad: [0u8; 4],
        }
    }
}

// === 消息标志常量 (与内核 ABI 一致) ===
pub const MSG_SYNC: u32 = 1 << 0;
pub const MSG_ASYNC: u32 = 1 << 1;
pub const MSG_URGENT: u32 = 1 << 2;
pub const MSG_NOCOPY: u32 = 1 << 3;
pub const MSG_BROADCAST: u32 = 1 << 4;
pub const MSG_RELIABLE: u32 = 1 << 5;

// === 事件掩码 (与内核 ABI 一致) ===
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EventMask {
    pub bits: [u64; 4],
}

impl EventMask {
    /// 创建一个全零的事件掩码
    pub const fn new() -> Self {
        Self { bits: [0, 0, 0, 0] }
    }

    /// 设置指定事件位
    pub fn set(&mut self, event_bit: u64) {
        let word = (event_bit / 64) as usize;
        let bit = event_bit % 64;
        if word < 4 {
            self.bits[word] |= 1 << bit;
        }
    }

    /// 清除指定事件位
    pub fn clear(&mut self, event_bit: u64) {
        let word = (event_bit / 64) as usize;
        let bit = event_bit % 64;
        if word < 4 {
            self.bits[word] &= !(1 << bit);
        }
    }

    /// 检查指定事件位是否已设置
    pub fn is_set(&self, event_bit: u64) -> bool {
        let word = (event_bit / 64) as usize;
        let bit = event_bit % 64;
        if word < 4 {
            (self.bits[word] & (1 << bit)) != 0
        } else {
            false
        }
    }
}

impl Default for EventMask {
    fn default() -> Self {
        Self::new()
    }
}

impl EventMask {
    /// 全部事件掩码
    pub const ALL: EventMask = EventMask {
        bits: [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
    };

    /// 检查是否包含另一个掩码中的所有事件位
    pub fn contains(&self, other: &EventMask) -> bool {
        for i in 0..4 {
            if (self.bits[i] & other.bits[i]) != other.bits[i] {
                return false;
            }
        }
        true
    }

    /// 检查是否包含指定的事件位
    pub fn has(&self, event_bit: u64) -> bool {
        self.is_set(event_bit)
    }
}

// === 事件类型常量 (与内核 ABI 一致) ===
pub const EVT_STATE_CHANGED: u64 = 1 << 0;
pub const EVT_MSG_RECEIVED: u64 = 1 << 1;
pub const EVT_CAP_REGISTERED: u64 = 1 << 2;
pub const EVT_CAP_REVOKED: u64 = 1 << 3;
pub const EVT_RESOURCE_LOW: u64 = 1 << 4;
pub const EVT_ERROR: u64 = 1 << 5;
pub const EVT_MIGRATION: u64 = 1 << 6;
pub const EVT_SNAPSHOT: u64 = 1 << 7;
pub const EVT_HEARTBEAT: u64 = 1 << 8;
pub const EVT_CUSTOM_START: u64 = 1 << 16;

// === 终止信号常量 (与内核 ABI 一致) ===
pub const SIG_KILL: u32 = 0;
pub const SIG_TERM: u32 = 1;
pub const SIG_COREDUMP: u32 = 2;
pub const SIG_FREEZE: u32 = 3;
pub const SIG_THAW: u32 = 4;

// === 能力定义常量 (与内核 ABI 一致) ===
pub const CAP_SPAWN_AGENT: usize = 0;
pub const CAP_SPAWN_SYSTEM_AGENT: usize = 1;
pub const CAP_NETWORK: usize = 2;
pub const CAP_DEVICE_ACCESS: usize = 3;
pub const CAP_IPC: usize = 4;
pub const CAP_SHARED_MEMORY: usize = 5;
pub const CAP_FILESYSTEM: usize = 6;
pub const CAP_ADMIN: usize = 7;
pub const CAP_VIRTUALIZATION: usize = 8;
pub const CAP_ENCLAVE: usize = 9;
pub const CAP_SPAWN_ENCLAVED: usize = 10;

// === Syscall 错误码 (与内核 ABI 一致) ===
pub const E_OK: isize = 0;
pub const E_INVAL: isize = -22;
pub const E_PERM: isize = -1;
pub const E_NOENT: isize = -2;
pub const E_NOMEM: isize = -12;
pub const E_BUSY: isize = -16;
pub const E_EXIST: isize = -17;
pub const E_NOTSUP: isize = -95;
pub const E_AGAIN: isize = -11;
pub const E_SRCH: isize = -3;
pub const E_ACCES: isize = -13;
pub const E_FAULT: isize = -14;
pub const E_TIMEOUT: isize = -110;
pub const E_QUOTA: isize = -200;
pub const E_CAP: isize = -201;

// === Syscall 编号 (与内核 ABI 一致) ===
pub const SYS_AGENT_SPAWN: u64 = 512;
pub const SYS_AGENT_KILL: u64 = 513;
pub const SYS_AGENT_QUERY: u64 = 514;
pub const SYS_AGENT_MSG: u64 = 515;
pub const SYS_AGENT_REGISTER: u64 = 516;
pub const SYS_AGENT_SUBSCRIBE: u64 = 517;
pub const SYS_AGENT_MIGRATE: u64 = 518;
pub const SYS_AGENT_MEMORY_SHARE: u64 = 519;
pub const SYS_AGENT_CAP_GRANT: u64 = 520;
pub const SYS_AGENT_CAP_REVOKE: u64 = 521;
pub const SYS_AGENT_BIND_PORT: u64 = 522;
pub const SYS_AGENT_EXPORT: u64 = 523;
pub const SYS_AGENT_IMPORT: u64 = 524;
pub const SYS_AGENT_SET_QUOTA: u64 = 525;
pub const SYS_AGENT_GET_QUOTA: u64 = 526;
pub const SYS_AGENT_SNAPSHOT: u64 = 527;
pub const SYS_AGENT_RESTORE: u64 = 528;

// ============================================================================
// 用户态高层 API 类型
// ============================================================================

// === 信号类型 ===
/// Agent 终止信号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// 强制终止
    Kill,
    /// 优雅终止
    Terminate,
    /// 冻结
    Freeze,
    /// 解冻
    Thaw,
}

impl Signal {
    /// 转换为内核 ABI 信号编号
    pub fn to_u32(&self) -> u32 {
        match self {
            Signal::Kill => SIG_KILL,
            Signal::Terminate => SIG_TERM,
            Signal::Freeze => SIG_FREEZE,
            Signal::Thaw => SIG_THAW,
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Signal::Kill => write!(f, "SIGKILL"),
            Signal::Terminate => write!(f, "SIGTERM"),
            Signal::Freeze => write!(f, "SIGFREEZE"),
            Signal::Thaw => write!(f, "SIGTHAW"),
        }
    }
}

// === 消息标志 ===
/// Agent 消息标志位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgFlags(pub u32);

impl MsgFlags {
    /// 同步消息
    pub const SYNC: MsgFlags = MsgFlags(MSG_SYNC);
    /// 异步消息
    pub const ASYNC: MsgFlags = MsgFlags(MSG_ASYNC);
    /// 紧急消息
    pub const URGENT: MsgFlags = MsgFlags(MSG_URGENT);
    /// 零拷贝消息
    pub const NOCOPY: MsgFlags = MsgFlags(MSG_NOCOPY);
    /// 广播消息
    pub const BROADCAST: MsgFlags = MsgFlags(MSG_BROADCAST);
    /// 可靠消息
    pub const RELIABLE: MsgFlags = MsgFlags(MSG_RELIABLE);

    /// 创建空消息标志
    pub fn empty() -> Self {
        MsgFlags(0)
    }

    /// 获取原始位值
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// 检查是否为同步消息
    pub fn is_sync(&self) -> bool {
        (self.0 & MSG_SYNC) != 0
    }

    /// 检查是否为异步消息
    pub fn is_async(&self) -> bool {
        (self.0 & MSG_ASYNC) != 0
    }

    /// 检查是否为紧急消息
    pub fn is_urgent(&self) -> bool {
        (self.0 & MSG_URGENT) != 0
    }

    /// 检查是否为零拷贝消息
    pub fn is_nocopy(&self) -> bool {
        (self.0 & MSG_NOCOPY) != 0
    }

    /// 检查是否为广播消息
    pub fn is_broadcast(&self) -> bool {
        (self.0 & MSG_BROADCAST) != 0
    }

    /// 检查是否为可靠消息
    pub fn is_reliable(&self) -> bool {
        (self.0 & MSG_RELIABLE) != 0
    }

    /// 检查是否包含指定的标志位
    pub fn contains(&self, other: MsgFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    /// 合并两个消息标志
    pub fn union(&self, other: MsgFlags) -> MsgFlags {
        MsgFlags(self.0 | other.0)
    }
}

impl Default for MsgFlags {
    fn default() -> Self {
        MsgFlags::empty()
    }
}

impl std::ops::BitOr for MsgFlags {
    type Output = MsgFlags;
    fn bitor(self, rhs: MsgFlags) -> MsgFlags {
        MsgFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for MsgFlags {
    fn bitor_assign(&mut self, rhs: MsgFlags) {
        self.0 |= rhs.0;
    }
}

// === Agent 句柄 (用户态高层封装) ===
/// Agent 运行时句柄
///
/// 封装内核返回的句柄值，提供类型安全的 Agent 引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentHandle {
    /// 内部句柄值
    inner: u64,
}

impl AgentHandle {
    /// 无效句柄常量
    pub const INVALID: AgentHandle = AgentHandle { inner: 0 };

    /// 创建新的 AgentHandle
    pub fn new(inner: u64) -> Self {
        AgentHandle { inner }
    }

    /// 从内核句柄创建
    pub fn from_kernel(handle: KernelAgentHandle) -> Self {
        AgentHandle { inner: handle.0 }
    }

    /// 转换为内核句柄
    pub fn to_kernel(&self) -> KernelAgentHandle {
        KernelAgentHandle(self.inner)
    }

    /// 检查句柄是否有效 (非零)
    pub fn is_valid(&self) -> bool {
        self.inner != 0
    }

    /// 获取内部句柄值
    pub fn as_u64(&self) -> u64 {
        self.inner
    }

    /// 获取对应的 AgentId
    pub fn agent_id(&self) -> AgentId {
        AgentId(self.inner)
    }
}

impl Default for AgentHandle {
    fn default() -> Self {
        Self::INVALID
    }
}

impl fmt::Display for AgentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AgentHandle({})", self.inner)
    }
}

// === Agent 消息 ===
/// Agent 消息
///
/// 表示 Agent 间通信的消息，包含消息 ID、来源、负载等。
#[derive(Debug, Clone)]
pub struct AgentMessage {
    /// 消息唯一标识
    pub msg_id: u64,
    /// 发送者句柄
    pub src: AgentHandle,
    /// 消息负载
    pub payload: Vec<u8>,
    /// 时间戳 (纳秒)
    pub timestamp: u64,
    /// 消息标志
    pub flags: u32,
}

impl AgentMessage {
    /// 创建新的 Agent 消息
    pub fn new(msg_id: u64, src: AgentHandle, payload: Vec<u8>) -> Self {
        Self {
            msg_id,
            src,
            payload,
            timestamp: 0,
            flags: 0,
        }
    }

    /// 创建带时间戳的消息
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    /// 创建带标志的消息
    pub fn with_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    /// 获取负载长度
    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }
}

// === Agent 信息 (用户态高层封装) ===
/// Agent 运行时信息
///
/// 从内核 AgentInfo 提取的用户态可读信息。
#[derive(Debug, Clone)]
pub struct AgentInfoView {
    /// Agent 句柄
    pub handle: AgentHandle,
    /// Agent 状态
    pub state: AgentState,
    /// Agent 类型
    pub agent_type: AgentType,
    /// Agent 名称
    pub name: String,
    /// 创建者 PID
    pub creator_pid: u64,
    /// 创建时间 (纳秒)
    pub create_time_ns: u64,
    /// CPU 时间 (纳秒)
    pub cpu_time_ns: u64,
    /// 已用内存
    pub memory_used: u64,
    /// 内存峰值
    pub memory_peak: u64,
    /// 线程数
    pub thread_count: u32,
    /// 连接数
    pub connection_count: u32,
    /// 已发送消息数
    pub msg_sent: u64,
    /// 已接收消息数
    pub msg_received: u64,
}

impl AgentInfoView {
    /// 从内核 AgentInfo 创建
    pub fn from_kernel(info: &AgentInfo) -> Self {
        // 从 name 字节数组中提取名称 (以第一个零字节为终止)
        let name = info.name
            .iter()
            .copied()
            .take_while(|&b| b != 0)
            .map(|b| b as char)
            .collect::<String>();

        Self {
            handle: AgentHandle::new(info.handle.0),
            state: info.state,
            agent_type: info.agent_type,
            name,
            creator_pid: info.creator_pid,
            create_time_ns: info.create_time_ns,
            cpu_time_ns: info.cpu_time_ns,
            memory_used: info.memory_used,
            memory_peak: info.memory_peak,
            thread_count: info.thread_count,
            connection_count: info.connection_count,
            msg_sent: info.msg_sent,
            msg_received: info.msg_received,
        }
    }
}

// ============================================================================
// 错误类型
// ============================================================================

// === 运行时错误 ===
/// Agent 运行时错误
#[derive(Debug)]
pub enum RuntimeError {
    /// 无效句柄
    InvalidHandle,
    /// Agent 未找到
    AgentNotFound,
    /// 权限不足
    PermissionDenied,
    /// 无效配置
    InvalidConfig(String),
    /// 配额超限
    QuotaExceeded,
    /// 操作不支持
    NotSupported,
    /// 内部错误 (附带内核错误码)
    InternalError(i64),
    /// 操作超时
    Timeout,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidHandle => write!(f, "无效的 Agent 句柄"),
            RuntimeError::AgentNotFound => write!(f, "Agent 未找到"),
            RuntimeError::PermissionDenied => write!(f, "权限不足"),
            RuntimeError::InvalidConfig(msg) => write!(f, "无效配置: {}", msg),
            RuntimeError::QuotaExceeded => write!(f, "资源配额超限"),
            RuntimeError::NotSupported => write!(f, "操作不支持"),
            RuntimeError::InternalError(code) => write!(f, "内部错误 (错误码: {})", code),
            RuntimeError::Timeout => write!(f, "操作超时"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl RuntimeError {
    /// 从内核返回的 i64 值创建 RuntimeError
    pub fn from_syscall_result(ret: i64) -> Result<i64, RuntimeError> {
        if ret >= 0 {
            Ok(ret)
        } else {
            Err(RuntimeError::from_error_code(ret))
        }
    }

    /// 从内核错误码创建 RuntimeError
    pub fn from_error_code(code: i64) -> RuntimeError {
        match code {
            x if x == E_INVAL as i64 => RuntimeError::InvalidConfig(format!("内核错误码: {}", code)),
            x if x == E_PERM as i64 => RuntimeError::PermissionDenied,
            x if x == E_NOENT as i64 => RuntimeError::AgentNotFound,
            x if x == E_SRCH as i64 => RuntimeError::AgentNotFound,
            x if x == E_ACCES as i64 => RuntimeError::PermissionDenied,
            x if x == E_NOMEM as i64 => RuntimeError::QuotaExceeded,
            x if x == E_QUOTA as i64 => RuntimeError::QuotaExceeded,
            x if x == E_CAP as i64 => RuntimeError::PermissionDenied,
            x if x == E_NOTSUP as i64 => RuntimeError::NotSupported,
            x if x == E_TIMEOUT as i64 => RuntimeError::Timeout,
            x if x == E_FAULT as i64 => RuntimeError::InvalidConfig(format!("内核错误码: {}", code)),
            _ => RuntimeError::InternalError(code),
        }
    }
}

// === 配置错误 ===
/// Agent 配置错误
#[derive(Debug)]
pub enum ConfigError {
    /// 无效名称 (空字符串)
    InvalidName,
    /// 名称过长 (超过 63 字节)
    NameTooLong,
    /// 无效类型
    InvalidType,
    /// 无效优先级
    InvalidPriority,
    /// 无效大小
    InvalidSize(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidName => write!(f, "无效的 Agent 名称: 名称不能为空"),
            ConfigError::NameTooLong => write!(f, "Agent 名称过长: 最大 63 字节"),
            ConfigError::InvalidType => write!(f, "无效的 Agent 类型"),
            ConfigError::InvalidPriority => write!(f, "无效的 Agent 优先级"),
            ConfigError::InvalidSize(msg) => write!(f, "无效的大小配置: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

// ============================================================================
// Agent 配置构建器
// ============================================================================

// === 能力名称到索引的映射 ===
/// 已知能力名称到能力索引的映射
fn capability_to_index(name: &str) -> Option<usize> {
    match name {
        "spawn_agent" => Some(CAP_SPAWN_AGENT),
        "spawn_system_agent" => Some(CAP_SPAWN_SYSTEM_AGENT),
        "network" => Some(CAP_NETWORK),
        "device_access" => Some(CAP_DEVICE_ACCESS),
        "ipc" => Some(CAP_IPC),
        "shared_memory" => Some(CAP_SHARED_MEMORY),
        "filesystem" => Some(CAP_FILESYSTEM),
        "admin" => Some(CAP_ADMIN),
        "virtualization" => Some(CAP_VIRTUALIZATION),
        "enclave" => Some(CAP_ENCLAVE),
        "spawn_enclaved" => Some(CAP_SPAWN_ENCLAVED),
        _ => None,
    }
}

/// Agent 配置构建器
///
/// 使用链式调用风格构建 Agent 配置，最终通过 `build()` 转换为内核 AgentSpec。
///
/// # 示例
///
/// ```ignore
/// let spec = AgentConfig::new("my-agent")
///     .agent_type(AgentType::AIInference)
///     .priority(AgentPriority::High)
///     .heap_size(64 * 1024 * 1024)
///     .stack_size(8 * 1024 * 1024)
///     .capability("network")
///     .capability("ipc")
///     .auto_start(true)
///     .build()?;
/// ```
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Agent 名称
    pub name: String,
    /// Agent 类型
    pub agent_type: AgentType,
    /// Agent 优先级
    pub priority: AgentPriority,
    /// 堆大小 (字节)
    pub heap_size: u64,
    /// 栈大小 (字节)
    pub stack_size: u64,
    /// 能力列表
    pub capabilities: Vec<String>,
    /// 是否自动启动
    pub auto_start: bool,
    /// 入口点 (可选)
    pub entry_point: Option<String>,
    /// 自定义参数
    pub params: HashMap<String, String>,
}

impl AgentConfig {
    /// 名称最大长度 (63 字节，因为内核 name 字段为 64 字节含终止符)
    pub const MAX_NAME_LEN: usize = 63;

    /// 创建新的 Agent 配置
    ///
    /// 默认值:
    /// - 类型: Generic
    /// - 优先级: Normal
    /// - 堆大小: 0 (使用系统默认)
    /// - 栈大小: 0 (使用系统默认)
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agent_type: AgentType::Generic,
            priority: AgentPriority::Normal,
            heap_size: 0,
            stack_size: 0,
            capabilities: Vec::new(),
            auto_start: false,
            entry_point: None,
            params: HashMap::new(),
        }
    }

    /// 设置 Agent 类型
    pub fn agent_type(mut self, t: AgentType) -> Self {
        self.agent_type = t;
        self
    }

    /// 设置优先级
    pub fn priority(mut self, p: AgentPriority) -> Self {
        self.priority = p;
        self
    }

    /// 设置堆大小 (字节)
    pub fn heap_size(mut self, size: u64) -> Self {
        self.heap_size = size;
        self
    }

    /// 设置栈大小 (字节)
    pub fn stack_size(mut self, size: u64) -> Self {
        self.stack_size = size;
        self
    }

    /// 添加能力
    pub fn capability(mut self, cap: &str) -> Self {
        self.capabilities.push(cap.to_string());
        self
    }

    /// 设置是否自动启动
    pub fn auto_start(mut self, auto: bool) -> Self {
        self.auto_start = auto;
        self
    }

    /// 设置入口点
    pub fn entry_point(mut self, ep: &str) -> Self {
        self.entry_point = Some(ep.to_string());
        self
    }

    /// 添加自定义参数
    pub fn param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    /// 构建为内核 AgentSpec
    ///
    /// 验证配置有效性，并将高层配置转换为内核 ABI 格式。
    pub fn build(&self) -> Result<AgentSpec, ConfigError> {
        // 验证名称
        if self.name.is_empty() {
            return Err(ConfigError::InvalidName);
        }
        if self.name.len() > Self::MAX_NAME_LEN {
            return Err(ConfigError::NameTooLong);
        }

        // 构建内核 AgentSpec
        let mut spec = AgentSpec::default();
        spec.version = 1;
        spec.agent_type = self.agent_type;
        spec.priority = self.priority as u8;
        spec.heap_size = self.heap_size;
        spec.stack_size = self.stack_size;

        // 写入名称 (以零终止)
        let name_bytes = self.name.as_bytes();
        let len = name_bytes.len().min(63);
        spec.name[..len].copy_from_slice(&name_bytes[..len]);
        // 剩余字节保持为零

        // 设置自动启动标志
        if self.auto_start {
            spec.flags |= AGENT_FLAG_AUTO_START;
        }

        // 设置能力位图
        for cap_name in &self.capabilities {
            if let Some(idx) = capability_to_index(cap_name) {
                spec.capabilities.set(idx);
            }
        }

        // 设置入口点 (将字符串哈希为 u64 作为入口点地址)
        if let Some(ref ep) = self.entry_point {
            spec.entry_point = simple_hash(ep);
        }

        // 将自定义参数序列化到 init_param (简单拼接，键值对以 \0 分隔)
        if !self.params.is_empty() {
            let mut param_buf = [0u8; 32];
            let mut offset = 0;
            for (key, value) in &self.params {
                let entry = format!("{}={}\0", key, value);
                let entry_bytes = entry.as_bytes();
                let remaining = 32 - offset;
                let copy_len = entry_bytes.len().min(remaining);
                if copy_len == 0 {
                    break;
                }
                param_buf[offset..offset + copy_len].copy_from_slice(&entry_bytes[..copy_len]);
                offset += copy_len;
            }
            spec.init_param = param_buf;
        }

        Ok(spec)
    }
}

/// 简单字符串哈希函数 (FNV-1a 变体)
///
/// 用于将入口点字符串转换为 u64 地址。
/// 注意: 这仅用于用户态库的入口点标识，不是加密哈希。
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    hash
}

// ============================================================================
// Syscall 接口 (extern "C" 声明)
// ============================================================================

/// 原始 syscall 调用接口
///
/// 通过 x86_64 syscall 指令调用内核。
/// 在实际内核环境中，这些函数会通过内联汇编实现。
/// 当前实现为桩函数，返回 E_NOTSUP，供用户态编译和测试使用。
mod syscall {
    use super::*;

    /// 原始 syscall 调用
    ///
    /// # Safety
    /// 调用者必须确保参数与目标 syscall 的 ABI 要求一致。
    pub unsafe fn raw_syscall(
        number: u64,
        arg1: u64,
        arg2: u64,
        arg3: u64,
        arg4: u64,
        arg5: u64,
        arg6: u64,
    ) -> i64 {
        // 在真实内核环境中，这里会使用内联汇编:
        // llvm_asm!("syscall" : "={rax}"(ret) : "{rax}"(number), ...);
        //
        // 当前为用户态桩实现，返回 E_NOTSUP
        let _ = (number, arg1, arg2, arg3, arg4, arg5, arg6);
        E_NOTSUP as i64
    }

    /// Agent 创建 syscall
    pub unsafe fn agent_spawn(spec: &AgentSpec, spec_len: usize) -> i64 {
        raw_syscall(
            SYS_AGENT_SPAWN,
            spec as *const AgentSpec as u64,
            spec_len as u64,
            0, // cap_slot
            0, 0, 0,
        )
    }

    /// Agent 终止 syscall
    pub unsafe fn agent_kill(handle: u64, signal: u32) -> i64 {
        raw_syscall(
            SYS_AGENT_KILL,
            handle,
            signal as u64,
            0, 0, 0, 0,
        )
    }

    /// Agent 查询 syscall
    pub unsafe fn agent_query(handle: u64, info: &mut AgentInfo, info_len: usize) -> i64 {
        raw_syscall(
            SYS_AGENT_QUERY,
            handle,
            info as *mut AgentInfo as u64,
            info_len as u64,
            0, 0, 0,
        )
    }

    /// Agent 消息发送 syscall
    pub unsafe fn agent_msg(
        src: u64,
        dst: u64,
        header: &AgentMsgHeader,
        flags: u32,
    ) -> i64 {
        raw_syscall(
            SYS_AGENT_MSG,
            src,
            dst,
            header as *const AgentMsgHeader as u64,
            flags as u64,
            0, 0,
        )
    }

    /// Agent 注册 syscall
    pub unsafe fn agent_register(handle: u64, cap_id: u64, data: *const u8, data_len: usize) -> i64 {
        raw_syscall(
            SYS_AGENT_REGISTER,
            handle,
            cap_id,
            data as u64,
            data_len as u64,
            0, 0,
        )
    }

    /// Agent 事件订阅 syscall
    pub unsafe fn agent_subscribe(
        subscriber: u64,
        target: u64,
        mask: &EventMask,
    ) -> i64 {
        raw_syscall(
            SYS_AGENT_SUBSCRIBE,
            subscriber,
            target,
            mask as *const EventMask as u64,
            0, 0, 0,
        )
    }

    /// Agent 能力授予 syscall
    pub unsafe fn agent_cap_grant(handle: u64, cap: usize, grant: u64) -> i64 {
        raw_syscall(
            SYS_AGENT_CAP_GRANT,
            handle,
            cap as u64,
            grant,
            0, 0, 0,
        )
    }

    /// Agent 资源配额设置 syscall
    pub unsafe fn agent_set_quota(handle: u64, quota: &ResourceQuota) -> i64 {
        raw_syscall(
            SYS_AGENT_SET_QUOTA,
            handle,
            quota as *const ResourceQuota as u64,
            0, 0, 0, 0,
        )
    }

    /// Agent 资源配额获取 syscall
    pub unsafe fn agent_get_quota(handle: u64, quota: &mut ResourceQuota) -> i64 {
        raw_syscall(
            SYS_AGENT_GET_QUOTA,
            handle,
            quota as *mut ResourceQuota as u64,
            0, 0, 0, 0,
        )
    }
}

// ============================================================================
// Agent 运行时
// ============================================================================

/// Agent 运行时
///
/// 提供用户态 Agent 管理的静态 API，所有方法通过 syscall 与内核交互。
///
/// # 示例
///
/// ```ignore
/// // 创建 Agent
/// let config = AgentConfig::new("my-agent")
///     .agent_type(AgentType::AIInference)
///     .priority(AgentPriority::High)
///     .build()?;
/// let handle = AgentRuntime::spawn(&config)?;
///
/// // 查询状态
/// let info = AgentRuntime::query(handle)?;
///
/// // 终止 Agent
/// AgentRuntime::kill(handle, Signal::Terminate)?;
/// ```
pub struct AgentRuntime;

impl AgentRuntime {
    /// 创建新 Agent
    ///
    /// 通过 SYS_AGENT_SPAWN syscall 向内核提交 AgentSpec，
    /// 成功时返回 AgentHandle。
    ///
    /// # 错误
    ///
    /// - `InvalidConfig`: 配置无效
    /// - `PermissionDenied`: 没有创建 Agent 的权限
    /// - `QuotaExceeded`: 资源配额不足
    /// - `NotSupported`: 内核不支持此操作 (用户态桩)
    pub fn spawn(config: &AgentConfig) -> Result<AgentHandle, RuntimeError> {
        let spec = config
            .build()
            .map_err(|e| RuntimeError::InvalidConfig(e.to_string()))?;

        let spec_len = std::mem::size_of::<AgentSpec>();

        // SAFETY: spec 是有效的引用，spec_len 是正确的结构体大小
        let ret = unsafe { syscall::agent_spawn(&spec, spec_len) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(handle_val) => Ok(AgentHandle::new(handle_val as u64)),
            Err(e) => Err(e),
        }
    }

    /// 终止 Agent
    ///
    /// 通过 SYS_AGENT_KILL syscall 向内核发送终止信号。
    ///
    /// # 错误
    ///
    /// - `InvalidHandle`: 句柄无效
    /// - `AgentNotFound`: Agent 不存在
    pub fn kill(handle: AgentHandle, signal: Signal) -> Result<(), RuntimeError> {
        if !handle.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        // SAFETY: handle 和 signal 都是有效值
        let ret = unsafe { syscall::agent_kill(handle.as_u64(), signal.to_u32()) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// 查询 Agent 状态
    ///
    /// 通过 SYS_AGENT_QUERY syscall 获取 Agent 信息。
    ///
    /// # 错误
    ///
    /// - `InvalidHandle`: 句柄无效
    /// - `AgentNotFound`: Agent 不存在
    pub fn query(handle: AgentHandle) -> Result<AgentInfoView, RuntimeError> {
        if !handle.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        let mut info = AgentInfo::default();
        let info_len = std::mem::size_of::<AgentInfo>();

        // SAFETY: info 是有效的可变引用，info_len 是正确的结构体大小
        let ret = unsafe { syscall::agent_query(handle.as_u64(), &mut info, info_len) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(_) => Ok(AgentInfoView::from_kernel(&info)),
            Err(e) => Err(e),
        }
    }

    /// 列出所有 Agent
    ///
    /// 注意: 当前内核 ABI 没有专门的 "list" syscall。
    /// 此方法通过遍历可能的句柄值来模拟，实际实现应使用专用的枚举接口。
    pub fn list() -> Result<Vec<AgentHandle>, RuntimeError> {
        // 在真实实现中，这里应该有一个 SYS_AGENT_LIST syscall
        // 当前返回空列表，因为内核尚未提供枚举接口
        Ok(Vec::new())
    }

    /// 发送消息
    ///
    /// 通过 SYS_AGENT_MSG syscall 向目标 Agent 发送消息。
    ///
    /// # 参数
    ///
    /// - `src`: 发送者 Agent 句柄
    /// - `dst`: 接收者 Agent 句柄
    /// - `payload`: 消息负载
    /// - `flags`: 消息标志
    ///
    /// # 返回
    ///
    /// 成功时返回消息 ID。
    pub fn send(
        src: AgentHandle,
        dst: AgentHandle,
        payload: &[u8],
        flags: MsgFlags,
    ) -> Result<u64, RuntimeError> {
        if !src.is_valid() || !dst.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        let header = AgentMsgHeader {
            msg_type: 1, // 普通消息
            flags: flags.bits(),
            payload_size: payload.len() as u64,
            ..AgentMsgHeader::default()
        };

        // SAFETY: header 是有效的引用，src 和 dst 是有效句柄
        let ret = unsafe { syscall::agent_msg(src.as_u64(), dst.as_u64(), &header, flags.bits()) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(msg_id) => Ok(msg_id as u64),
            Err(e) => Err(e),
        }
    }

    /// 接收消息
    ///
    /// 注意: 当前内核 ABI 没有专门的 "receive" syscall。
    /// 消息接收通过事件订阅机制实现。
    pub fn receive(_handle: AgentHandle) -> Result<AgentMessage, RuntimeError> {
        // 在真实实现中，这里应该有一个 SYS_AGENT_RECV syscall
        // 或者通过事件队列轮询实现
        Err(RuntimeError::NotSupported)
    }

    /// 订阅事件
    ///
    /// 通过 SYS_AGENT_SUBSCRIBE syscall 订阅目标 Agent 的事件。
    ///
    /// # 参数
    ///
    /// - `handle`: 订阅者 Agent 句柄
    /// - `events`: 事件掩码
    pub fn subscribe(handle: AgentHandle, events: EventMask) -> Result<(), RuntimeError> {
        if !handle.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        // SAFETY: handle 是有效句柄，events 是有效引用
        let ret = unsafe { syscall::agent_subscribe(handle.as_u64(), 0, &events) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// 注册能力
    ///
    /// 通过 SYS_AGENT_CAP_GRANT syscall 为 Agent 注册能力。
    ///
    /// # 参数
    ///
    /// - `handle`: Agent 句柄
    /// - `cap`: 能力名称 (如 "network", "ipc" 等)
    pub fn register_capability(handle: AgentHandle, cap: &str) -> Result<(), RuntimeError> {
        if !handle.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        let cap_idx = match capability_to_index(cap) {
            Some(idx) => idx,
            None => return Err(RuntimeError::InvalidConfig(format!("未知能力: {}", cap))),
        };

        // SAFETY: handle 是有效句柄，cap_idx 是有效索引
        let ret = unsafe { syscall::agent_cap_grant(handle.as_u64(), cap_idx, 1) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// 设置资源配额
    ///
    /// 通过 SYS_AGENT_SET_QUOTA syscall 设置 Agent 的资源配额。
    ///
    /// # 参数
    ///
    /// - `handle`: Agent 句柄
    /// - `quota`: 资源配额
    pub fn set_quota(handle: AgentHandle, quota: &ResourceQuota) -> Result<(), RuntimeError> {
        if !handle.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        // SAFETY: handle 是有效句柄，quota 是有效引用
        let ret = unsafe { syscall::agent_set_quota(handle.as_u64(), quota) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// 获取资源配额
    ///
    /// 通过 SYS_AGENT_GET_QUOTA syscall 获取 Agent 的当前资源配额。
    ///
    /// # 参数
    ///
    /// - `handle`: Agent 句柄
    ///
    /// # 返回
    ///
    /// 成功时返回当前的 ResourceQuota。
    pub fn get_quota(handle: AgentHandle) -> Result<ResourceQuota, RuntimeError> {
        if !handle.is_valid() {
            return Err(RuntimeError::InvalidHandle);
        }

        let mut quota = ResourceQuota::default();

        // SAFETY: handle 是有效句柄，quota 是有效的可变引用
        let ret = unsafe { syscall::agent_get_quota(handle.as_u64(), &mut quota) };

        match RuntimeError::from_syscall_result(ret) {
            Ok(_) => Ok(quota),
            Err(e) => Err(e),
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === 测试: 配置构建器链式调用 ===
    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new("test-agent")
            .agent_type(AgentType::AIInference)
            .priority(AgentPriority::High)
            .heap_size(64 * 1024 * 1024)
            .stack_size(8 * 1024 * 1024)
            .capability("network")
            .capability("ipc")
            .auto_start(true)
            .entry_point("main")
            .param("model", "llama3")
            .param("device", "gpu0");

        assert_eq!(config.name, "test-agent");
        assert_eq!(config.agent_type, AgentType::AIInference);
        assert_eq!(config.priority, AgentPriority::High);
        assert_eq!(config.heap_size, 64 * 1024 * 1024);
        assert_eq!(config.stack_size, 8 * 1024 * 1024);
        assert_eq!(config.capabilities, vec!["network", "ipc"]);
        assert!(config.auto_start);
        assert_eq!(config.entry_point, Some("main".to_string()));
        assert_eq!(config.params.get("model"), Some(&"llama3".to_string()));
        assert_eq!(config.params.get("device"), Some(&"gpu0".to_string()));
    }

    // === 测试: 构建为内核 AgentSpec ===
    #[test]
    fn test_agent_config_build_to_spec() {
        let config = AgentConfig::new("my-agent")
            .agent_type(AgentType::Network)
            .priority(AgentPriority::Realtime)
            .heap_size(128 * 1024 * 1024)
            .stack_size(16 * 1024 * 1024)
            .capability("network")
            .capability("ipc")
            .auto_start(true)
            .build()
            .unwrap();

        // 验证基本字段
        assert_eq!(config.version, 1);
        assert_eq!(config.agent_type, AgentType::Network);
        assert_eq!(config.priority, AgentPriority::Realtime as u8);
        assert_eq!(config.heap_size, 128 * 1024 * 1024);
        assert_eq!(config.stack_size, 16 * 1024 * 1024);

        // 验证名称已写入
        let name_str = std::str::from_utf8(&config.name).unwrap_or("");
        assert!(name_str.starts_with("my-agent"));

        // 验证自动启动标志
        assert_eq!(config.flags & AGENT_FLAG_AUTO_START, AGENT_FLAG_AUTO_START);

        // 验证能力位图
        assert!(config.capabilities.is_set(CAP_NETWORK));
        assert!(config.capabilities.is_set(CAP_IPC));
        assert!(!config.capabilities.is_set(CAP_ADMIN));

        // 验证 AgentSpec 大小为 256 字节
        assert_eq!(std::mem::size_of::<AgentSpec>(), 256);
    }

    // === 测试: 无效名称 (空字符串) ===
    #[test]
    fn test_agent_config_invalid_name() {
        let config = AgentConfig::new("");
        let result = config.build();
        assert!(matches!(result, Err(ConfigError::InvalidName)));
    }

    // === 测试: 名称过长 ===
    #[test]
    fn test_agent_config_name_too_long() {
        let long_name = "a".repeat(64); // 超过 63 字节限制
        let config = AgentConfig::new(&long_name);
        let result = config.build();
        assert!(matches!(result, Err(ConfigError::NameTooLong)));
    }

    // === 测试: 名称恰好 63 字节 (边界值) ===
    #[test]
    fn test_agent_config_name_boundary() {
        let name = "a".repeat(63); // 恰好 63 字节
        let config = AgentConfig::new(&name);
        let result = config.build();
        assert!(result.is_ok());
    }

    // === 测试: AgentType 枚举值 ===
    #[test]
    fn test_agent_type_values() {
        assert_eq!(AgentType::Generic as u32, 0);
        assert_eq!(AgentType::AIInference as u32, 1);
        assert_eq!(AgentType::DataProcessing as u32, 2);
        assert_eq!(AgentType::Network as u32, 3);
        assert_eq!(AgentType::System as u32, 4);
        assert_eq!(AgentType::Sandbox as u32, 5);
        assert_eq!(AgentType::Virtualization as u32, 6);
    }

    // === 测试: AgentType from_u32 ===
    #[test]
    fn test_agent_type_from_u32() {
        assert_eq!(AgentType::from_u32(0), AgentType::Generic);
        assert_eq!(AgentType::from_u32(1), AgentType::AIInference);
        assert_eq!(AgentType::from_u32(6), AgentType::Virtualization);
        // 无效值返回 Generic
        assert_eq!(AgentType::from_u32(99), AgentType::Generic);
    }

    // === 测试: AgentType Display ===
    #[test]
    fn test_agent_type_display() {
        assert_eq!(format!("{}", AgentType::Generic), "Generic");
        assert_eq!(format!("{}", AgentType::AIInference), "AIInference");
        assert_eq!(format!("{}", AgentType::Network), "Network");
    }

    // === 测试: AgentState 枚举值 ===
    #[test]
    fn test_agent_state_values() {
        assert_eq!(AgentState::Creating as u32, 0);
        assert_eq!(AgentState::Ready as u32, 1);
        assert_eq!(AgentState::Running as u32, 2);
        assert_eq!(AgentState::Waiting as u32, 3);
        assert_eq!(AgentState::Frozen as u32, 4);
        assert_eq!(AgentState::Migrating as u32, 5);
        assert_eq!(AgentState::Terminating as u32, 6);
        assert_eq!(AgentState::Terminated as u32, 7);
        assert_eq!(AgentState::Failed as u32, 8);
    }

    // === 测试: AgentState from_u32 ===
    #[test]
    fn test_agent_state_from_u32() {
        assert_eq!(AgentState::from_u32(0), AgentState::Creating);
        assert_eq!(AgentState::from_u32(2), AgentState::Running);
        assert_eq!(AgentState::from_u32(8), AgentState::Failed);
        // 无效值返回 Creating
        assert_eq!(AgentState::from_u32(99), AgentState::Creating);
    }

    // === 测试: AgentState Display ===
    #[test]
    fn test_agent_state_display() {
        assert_eq!(format!("{}", AgentState::Running), "Running");
        assert_eq!(format!("{}", AgentState::Failed), "Failed");
    }

    // === 测试: AgentPriority 排序 ===
    #[test]
    fn test_agent_priority_ordering() {
        assert!(AgentPriority::Realtime > AgentPriority::High);
        assert!(AgentPriority::High > AgentPriority::Normal);
        assert!(AgentPriority::Normal > AgentPriority::Low);
        assert!(AgentPriority::Low > AgentPriority::Idle);
    }

    // === 测试: AgentPriority 数值 ===
    #[test]
    fn test_agent_priority_values() {
        assert_eq!(AgentPriority::Idle as u8, 0);
        assert_eq!(AgentPriority::Low as u8, 64);
        assert_eq!(AgentPriority::Normal as u8, 128);
        assert_eq!(AgentPriority::High as u8, 192);
        assert_eq!(AgentPriority::Realtime as u8, 255);
    }

    // === 测试: AgentPriority Display ===
    #[test]
    fn test_agent_priority_display() {
        assert_eq!(format!("{}", AgentPriority::Idle), "Idle");
        assert_eq!(format!("{}", AgentPriority::Normal), "Normal");
        assert_eq!(format!("{}", AgentPriority::Realtime), "Realtime");
    }

    // === 测试: 消息标志 ===
    #[test]
    fn test_msg_flags() {
        // 验证各标志位值
        assert_eq!(MsgFlags::SYNC.bits(), 1);
        assert_eq!(MsgFlags::ASYNC.bits(), 2);
        assert_eq!(MsgFlags::URGENT.bits(), 4);
        assert_eq!(MsgFlags::NOCOPY.bits(), 8);
        assert_eq!(MsgFlags::BROADCAST.bits(), 16);
        assert_eq!(MsgFlags::RELIABLE.bits(), 32);

        // 验证标志位互不重叠
        let all = MsgFlags::SYNC | MsgFlags::ASYNC | MsgFlags::URGENT
            | MsgFlags::NOCOPY | MsgFlags::BROADCAST | MsgFlags::RELIABLE;
        assert_eq!(all.bits(), 63); // 1+2+4+8+16+32

        // 验证 is_sync / is_async / is_urgent
        let sync_flags = MsgFlags::SYNC;
        assert!(sync_flags.is_sync());
        assert!(!sync_flags.is_async());
        assert!(!sync_flags.is_urgent());

        // 验证 contains
        let combined = MsgFlags::SYNC | MsgFlags::URGENT;
        assert!(combined.contains(MsgFlags::SYNC));
        assert!(combined.contains(MsgFlags::URGENT));
        assert!(!combined.contains(MsgFlags::ASYNC));

        // 验证 union
        let a = MsgFlags::SYNC;
        let b = MsgFlags::RELIABLE;
        let c = a.union(b);
        assert!(c.is_sync());
        assert!(c.is_reliable());

        // 验证 empty
        let empty = MsgFlags::empty();
        assert!(!empty.is_sync());
        assert!(!empty.is_async());
        assert_eq!(empty.bits(), 0);

        // 验证 BitOrAssign
        let mut flags = MsgFlags::empty();
        flags |= MsgFlags::SYNC;
        assert!(flags.is_sync());
    }

    // === 测试: 信号类型 ===
    #[test]
    fn test_signal_types() {
        // 验证信号到内核编号的映射
        assert_eq!(Signal::Kill.to_u32(), SIG_KILL);
        assert_eq!(Signal::Terminate.to_u32(), SIG_TERM);
        assert_eq!(Signal::Freeze.to_u32(), SIG_FREEZE);
        assert_eq!(Signal::Thaw.to_u32(), SIG_THAW);

        // 验证 Display
        assert_eq!(format!("{}", Signal::Kill), "SIGKILL");
        assert_eq!(format!("{}", Signal::Terminate), "SIGTERM");
        assert_eq!(format!("{}", Signal::Freeze), "SIGFREEZE");
        assert_eq!(format!("{}", Signal::Thaw), "SIGTHAW");
    }

    // === 测试: RuntimeError Display ===
    #[test]
    fn test_runtime_error_display() {
        assert_eq!(
            format!("{}", RuntimeError::InvalidHandle),
            "无效的 Agent 句柄"
        );
        assert_eq!(
            format!("{}", RuntimeError::AgentNotFound),
            "Agent 未找到"
        );
        assert_eq!(
            format!("{}", RuntimeError::PermissionDenied),
            "权限不足"
        );
        assert_eq!(
            format!("{}", RuntimeError::QuotaExceeded),
            "资源配额超限"
        );
        assert_eq!(
            format!("{}", RuntimeError::NotSupported),
            "操作不支持"
        );
        assert_eq!(
            format!("{}", RuntimeError::Timeout),
            "操作超时"
        );
        assert!(!format!("{}", RuntimeError::InternalError(-42)).is_empty());
        assert!(!format!("{}", RuntimeError::InvalidConfig("test".to_string())).is_empty());
    }

    // === 测试: RuntimeError from_error_code ===
    #[test]
    fn test_runtime_error_from_error_code() {
        assert!(matches!(
            RuntimeError::from_error_code(E_PERM as i64),
            RuntimeError::PermissionDenied
        ));
        assert!(matches!(
            RuntimeError::from_error_code(E_SRCH as i64),
            RuntimeError::AgentNotFound
        ));
        assert!(matches!(
            RuntimeError::from_error_code(E_NOMEM as i64),
            RuntimeError::QuotaExceeded
        ));
        assert!(matches!(
            RuntimeError::from_error_code(E_NOTSUP as i64),
            RuntimeError::NotSupported
        ));
        assert!(matches!(
            RuntimeError::from_error_code(E_TIMEOUT as i64),
            RuntimeError::Timeout
        ));
        assert!(matches!(
            RuntimeError::from_error_code(-999),
            RuntimeError::InternalError(_)
        ));
    }

    // === 测试: RuntimeError from_syscall_result ===
    #[test]
    fn test_runtime_error_from_syscall_result() {
        // 成功返回值
        let result = RuntimeError::from_syscall_result(42);
        assert_eq!(result.unwrap(), 42);

        // 错误返回值
        let result = RuntimeError::from_syscall_result(E_PERM as i64);
        assert!(result.is_err());
    }

    // === 测试: ConfigError Display ===
    #[test]
    fn test_config_error_display() {
        assert!(!format!("{}", ConfigError::InvalidName).is_empty());
        assert!(!format!("{}", ConfigError::NameTooLong).is_empty());
        assert!(!format!("{}", ConfigError::InvalidType).is_empty());
        assert!(!format!("{}", ConfigError::InvalidPriority).is_empty());
        assert!(!format!("{}", ConfigError::InvalidSize("heap".to_string())).is_empty());
    }

    // === 测试: AgentHandle 有效性 ===
    #[test]
    fn test_agent_handle_validity() {
        // 无效句柄
        let invalid = AgentHandle::INVALID;
        assert!(!invalid.is_valid());
        assert_eq!(invalid.as_u64(), 0);

        // 有效句柄
        let valid = AgentHandle::new(42);
        assert!(valid.is_valid());
        assert_eq!(valid.as_u64(), 42);

        // 与内核句柄的转换
        let kernel_handle = KernelAgentHandle(42);
        let user_handle = AgentHandle::from_kernel(kernel_handle);
        assert_eq!(user_handle.as_u64(), 42);
        assert_eq!(user_handle.to_kernel().0, 42);

        // AgentId 转换
        assert_eq!(valid.agent_id(), AgentId(42));

        // Display
        assert_eq!(format!("{}", valid), "AgentHandle(42)");

        // Default
        assert_eq!(AgentHandle::default(), AgentHandle::INVALID);
    }

    // === 测试: AgentMessage 创建 ===
    #[test]
    fn test_agent_message_creation() {
        let handle = AgentHandle::new(1);
        let msg = AgentMessage::new(100, handle, vec![1, 2, 3, 4]);

        assert_eq!(msg.msg_id, 100);
        assert_eq!(msg.src, handle);
        assert_eq!(msg.payload, vec![1, 2, 3, 4]);
        assert_eq!(msg.timestamp, 0);
        assert_eq!(msg.flags, 0);
        assert_eq!(msg.payload_len(), 4);

        // with_timestamp
        let msg = msg.with_timestamp(12345);
        assert_eq!(msg.timestamp, 12345);

        // with_flags
        let msg = msg.with_flags(MSG_SYNC | MSG_URGENT);
        assert_eq!(msg.flags, MSG_SYNC | MSG_URGENT);
    }

    // === 测试: AgentId ===
    #[test]
    fn test_agent_id() {
        assert!(!AgentId::INVALID.is_valid());
        assert!(AgentId::new(42).is_valid());
        assert!(AgentId::SYSTEM.is_system());
        assert!(!AgentId::new(42).is_system());
        assert_eq!(format!("{}", AgentId::new(42)), "Agent(42)");
        assert_eq!(AgentId::default(), AgentId::INVALID);
    }

    // === 测试: KernelAgentHandle ===
    #[test]
    fn test_kernel_agent_handle() {
        assert!(!KernelAgentHandle::INVALID.is_valid());
        let h = KernelAgentHandle(5);
        assert!(h.is_valid());
        assert_eq!(h.index(), 4);
    }

    // === 测试: CapBitmap 操作 ===
    #[test]
    fn test_cap_bitmap_operations() {
        let mut caps = CapBitmap::new();
        assert!(!caps.is_set(CAP_NETWORK));
        assert!(!caps.is_set(CAP_ADMIN));

        caps.set(CAP_NETWORK);
        assert!(caps.is_set(CAP_NETWORK));
        assert!(!caps.is_set(CAP_ADMIN));

        caps.set(CAP_ADMIN);
        assert!(caps.is_set(CAP_NETWORK));
        assert!(caps.is_set(CAP_ADMIN));

        caps.clear(CAP_NETWORK);
        assert!(!caps.is_set(CAP_NETWORK));
        assert!(caps.is_set(CAP_ADMIN));

        // 超出范围不应 panic
        caps.set(128);
        assert!(!caps.is_set(128));
        caps.clear(200);
    }

    // === 测试: ResourceQuota ===
    #[test]
    fn test_resource_quota() {
        let quota = ResourceQuota::new();
        assert_eq!(quota.max_memory_bytes, 0);
        assert_eq!(quota.max_shm_bytes, 0);
        assert_eq!(quota.max_cpu_percent, 0);
        assert_eq!(quota.max_fds, 0);
        assert_eq!(quota.max_threads, 0);
        assert_eq!(quota.max_msg_per_sec, 0);

        let quota2 = ResourceQuota::default();
        assert_eq!(quota.max_memory_bytes, quota2.max_memory_bytes);
    }

    // === 测试: EventMask ===
    #[test]
    fn test_event_mask() {
        let mut mask = EventMask::new();
        assert!(!mask.is_set(0));
        assert!(!mask.is_set(1));

        mask.set(0);
        assert!(mask.is_set(0));
        assert!(!mask.is_set(1));

        mask.set(1);
        mask.set(5);
        assert!(mask.is_set(0));
        assert!(mask.is_set(1));
        assert!(mask.is_set(5));

        mask.clear(0);
        assert!(!mask.is_set(0));
        assert!(mask.is_set(1));

        // ALL 掩码
        assert!(EventMask::ALL.contains(&mask));

        // has 方法
        assert!(EventMask::ALL.has(0));
        assert!(EventMask::ALL.has(255));

        // 超出范围不应 panic
        mask.set(256);
        assert!(!mask.is_set(256));
    }

    // === 测试: AgentInfoView ===
    #[test]
    fn test_agent_info_view() {
        let mut info = AgentInfo::default();
        info.handle = KernelAgentHandle(42);
        info.state = AgentState::Running;
        info.agent_type = AgentType::AIInference;
        info.creator_pid = 100;
        info.memory_used = 1024 * 1024;
        info.msg_sent = 10;
        info.msg_received = 20;

        // 写入名称
        let name = b"test-agent";
        info.name[..name.len()].copy_from_slice(name);

        let view = AgentInfoView::from_kernel(&info);
        assert_eq!(view.handle.as_u64(), 42);
        assert_eq!(view.state, AgentState::Running);
        assert_eq!(view.agent_type, AgentType::AIInference);
        assert_eq!(view.name, "test-agent");
        assert_eq!(view.creator_pid, 100);
        assert_eq!(view.memory_used, 1024 * 1024);
        assert_eq!(view.msg_sent, 10);
        assert_eq!(view.msg_received, 20);
    }

    // === 测试: AgentSpec 大小 ===
    #[test]
    fn test_agent_spec_size() {
        assert_eq!(
            std::mem::size_of::<AgentSpec>(),
            256,
            "AgentSpec 大小必须为 256 字节"
        );
    }

    // === 测试: AgentInfo 大小 ===
    #[test]
    fn test_agent_info_size() {
        assert_eq!(
            std::mem::size_of::<AgentInfo>(),
            264,
            "AgentInfo 大小必须为 264 字节"
        );
    }

    // === 测试: AgentMsgHeader 大小 ===
    #[test]
    fn test_agent_msg_header_size() {
        assert_eq!(
            std::mem::size_of::<AgentMsgHeader>(),
            48,
            "AgentMsgHeader 大小必须为 48 字节"
        );
    }

    // === 测试: Agent 标志位 ===
    #[test]
    fn test_agent_flags() {
        assert_eq!(AGENT_FLAG_NONE, 0);
        assert_eq!(AGENT_FLAG_AUTO_START & AGENT_FLAG_PERSISTENT, 0);
        assert_eq!(AGENT_FLAG_ISOLATED & AGENT_FLAG_PRIVILEGED, 0);
        assert_eq!(AGENT_FLAG_ENCLAVED & AGENT_FLAG_AUTO_START, 0);

        let flags = AGENT_FLAG_AUTO_START | AGENT_FLAG_ISOLATED;
        assert_eq!(flags & AGENT_FLAG_AUTO_START, AGENT_FLAG_AUTO_START);
        assert_eq!(flags & AGENT_FLAG_ISOLATED, AGENT_FLAG_ISOLATED);
        assert_eq!(flags & AGENT_FLAG_PERSISTENT, 0);
    }

    // === 测试: 错误码负值 ===
    #[test]
    fn test_error_codes_negative() {
        assert!(E_OK >= 0);
        let error_codes = [
            E_INVAL, E_PERM, E_NOENT, E_NOMEM, E_BUSY, E_EXIST,
            E_NOTSUP, E_AGAIN, E_SRCH, E_ACCES, E_FAULT, E_TIMEOUT,
            E_QUOTA, E_CAP,
        ];
        for &code in &error_codes {
            assert!(code < 0, "错误码 {} 应为负值", code);
        }
        assert!(E_QUOTA < -100);
        assert!(E_CAP < -100);
    }

    // === 测试: Syscall 编号连续性 ===
    #[test]
    fn test_syscall_numbers() {
        assert_eq!(SYS_AGENT_SPAWN, 512);
        assert_eq!(SYS_AGENT_KILL, 513);
        assert_eq!(SYS_AGENT_QUERY, 514);
        assert_eq!(SYS_AGENT_MSG, 515);
        assert_eq!(SYS_AGENT_RESTORE, 528);
    }

    // === 测试: AgentRuntime::spawn 配置验证 ===
    #[test]
    fn test_runtime_spawn_invalid_config() {
        // 空名称应返回 InvalidConfig 错误
        let config = AgentConfig::new("");
        let result = AgentRuntime::spawn(&config);
        assert!(matches!(result, Err(RuntimeError::InvalidConfig(_))));
    }

    // === 测试: AgentRuntime::kill 无效句柄 ===
    #[test]
    fn test_runtime_kill_invalid_handle() {
        let result = AgentRuntime::kill(AgentHandle::INVALID, Signal::Kill);
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: AgentRuntime::query 无效句柄 ===
    #[test]
    fn test_runtime_query_invalid_handle() {
        let result = AgentRuntime::query(AgentHandle::INVALID);
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: AgentRuntime::list ===
    #[test]
    fn test_runtime_list() {
        let result = AgentRuntime::list();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // === 测试: AgentRuntime::send 无效句柄 ===
    #[test]
    fn test_runtime_send_invalid_handle() {
        let result = AgentRuntime::send(
            AgentHandle::INVALID,
            AgentHandle::new(1),
            &[1, 2, 3],
            MsgFlags::SYNC,
        );
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));

        let result = AgentRuntime::send(
            AgentHandle::new(1),
            AgentHandle::INVALID,
            &[1, 2, 3],
            MsgFlags::SYNC,
        );
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: AgentRuntime::receive ===
    #[test]
    fn test_runtime_receive() {
        let result = AgentRuntime::receive(AgentHandle::new(1));
        assert!(matches!(result, Err(RuntimeError::NotSupported)));
    }

    // === 测试: AgentRuntime::subscribe 无效句柄 ===
    #[test]
    fn test_runtime_subscribe_invalid_handle() {
        let result = AgentRuntime::subscribe(AgentHandle::INVALID, EventMask::ALL);
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: AgentRuntime::register_capability 无效句柄 ===
    #[test]
    fn test_runtime_register_capability_invalid_handle() {
        let result = AgentRuntime::register_capability(AgentHandle::INVALID, "network");
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: AgentRuntime::register_capability 未知能力 ===
    #[test]
    fn test_runtime_register_capability_unknown() {
        let result = AgentRuntime::register_capability(AgentHandle::new(1), "unknown_cap");
        assert!(matches!(result, Err(RuntimeError::InvalidConfig(_))));
    }

    // === 测试: AgentRuntime::set_quota 无效句柄 ===
    #[test]
    fn test_runtime_set_quota_invalid_handle() {
        let quota = ResourceQuota::new();
        let result = AgentRuntime::set_quota(AgentHandle::INVALID, &quota);
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: AgentRuntime::get_quota 无效句柄 ===
    #[test]
    fn test_runtime_get_quota_invalid_handle() {
        let result = AgentRuntime::get_quota(AgentHandle::INVALID);
        assert!(matches!(result, Err(RuntimeError::InvalidHandle)));
    }

    // === 测试: simple_hash ===
    #[test]
    fn test_simple_hash() {
        // 相同输入应产生相同输出
        let h1 = simple_hash("main");
        let h2 = simple_hash("main");
        assert_eq!(h1, h2);

        // 不同输入应产生不同输出
        let h3 = simple_hash("other");
        assert_ne!(h1, h3);

        // 哈希值不应为零
        assert_ne!(h1, 0);
    }

    // === 测试: capability_to_index ===
    #[test]
    fn test_capability_to_index() {
        assert_eq!(capability_to_index("network"), Some(CAP_NETWORK));
        assert_eq!(capability_to_index("ipc"), Some(CAP_IPC));
        assert_eq!(capability_to_index("admin"), Some(CAP_ADMIN));
        assert_eq!(capability_to_index("unknown"), None);
    }

    // === 测试: ConfigError 实现 std::error::Error ===
    #[test]
    fn test_config_error_is_error() {
        let err: Box<dyn std::error::Error> = Box::new(ConfigError::InvalidName);
        assert!(!err.to_string().is_empty());
    }

    // === 测试: RuntimeError 实现 std::error::Error ===
    #[test]
    fn test_runtime_error_is_error() {
        let err: Box<dyn std::error::Error> = Box::new(RuntimeError::AgentNotFound);
        assert!(!err.to_string().is_empty());
    }
}
