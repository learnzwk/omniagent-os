//! Agent 系统调用 ABI 类型定义
//!
//! 与 agent-syscall-abi.md 规范完全对齐。
//! 所有结构体使用 #[repr(C)] 保证内存布局稳定，
//! 作为内核与用户态之间的 ABI 契约。

// === Agent 类型 ===
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

// === Agent 状态 ===
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

// === 调度策略 ===
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SchedPolicy {
    CFS = 0,
    RTFifo = 1,
    RTRR = 2,
    Idle = 3,
    Batch = 4,
}

// === 能力位图 (128 位) ===
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

// === 资源配额 ===
// 字段按对齐要求排列: u64 在前，u32 在后，消除内部 padding
// 自然大小: 32 字节
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ResourceQuota {
    pub max_memory_bytes: u64,  // 最大内存
    pub max_shm_bytes: u64,     // 最大共享内存
    pub max_cpu_percent: u32,   // CPU 使用率上限 (百分比 * 100)
    pub max_fds: u32,           // 最大文件描述符
    pub max_threads: u32,       // 最大线程数
    pub max_msg_per_sec: u32,   // 每秒最大消息数
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

// === Agent 标志 ===
pub const AGENT_FLAG_NONE: u32 = 0;
pub const AGENT_FLAG_AUTO_START: u32 = 1 << 0;
pub const AGENT_FLAG_PERSISTENT: u32 = 1 << 1;
pub const AGENT_FLAG_ISOLATED: u32 = 1 << 2;
pub const AGENT_FLAG_PRIVILEGED: u32 = 1 << 3;
pub const AGENT_FLAG_ENCLAVED: u32 = 1 << 4;

// === Agent 句柄 (不透明) ===
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AgentHandle(pub u64);

impl AgentHandle {
    /// 无效句柄常量
    pub const INVALID: AgentHandle = AgentHandle(0);

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

// === AgentSpec (256 字节) ===
//
// 内存布局验证:
//   version:          u32       =  4 字节  (偏移 0)
//   agent_type:       u32       =  4 字节  (偏移 4)
//   name:             [u8; 64]  = 64 字节  (偏移 8)
//   entry_point:      u64       =  8 字节  (偏移 72)
//   code_size:        u64       =  8 字节  (偏移 80)
//   heap_size:        u64       =  8 字节  (偏移 88)
//   stack_size:       u64       =  8 字节  (偏移 96)
//   cpu_affinity:     u64       =  8 字节  (偏移 104)
//   priority:         u8        =  1 字节  (偏移 112)
//   sched_policy:     u8        =  1 字节  (偏移 113)
//   _pad0:            [u8; 6]   =  6 字节  (偏移 114, 对齐 memory_limit)
//   memory_limit:     u64       =  8 字节  (偏移 120)
//   max_fds:          u32       =  4 字节  (偏移 128)
//   _pad1:            u32       =  4 字节  (偏移 132, 对齐 capabilities)
//   capabilities:     CapBitmap = 16 字节  (偏移 136)
//   port_count:       u16       =  2 字节  (偏移 152)
//   _pad2:            u16       =  2 字节  (偏移 154, 对齐 flags)
//   flags:            u32       =  4 字节  (偏移 156)
//   quota:            ResourceQuota = 32 字节 (偏移 160)
//   security_label:   [u8; 32]  = 32 字节 (偏移 192)
//   init_param:       [u8; 32]  = 32 字节 (偏移 224)
//   总计: 256 字节
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
    pub sched_policy: SchedPolicy,
    _pad0: [u8; 6],
    pub memory_limit: u64,
    pub max_fds: u32,
    _pad1: u32,
    pub capabilities: CapBitmap,
    pub port_count: u16,
    _pad2: u16,
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
            sched_policy: SchedPolicy::CFS,
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

// === AgentInfo (264 字节) ===
//
// 内存布局验证:
//   handle:           AgentHandle =  8 字节 (偏移 0)
//   state:            u32         =  4 字节 (偏移 8)
//   agent_type:       u32         =  4 字节 (偏移 12)
//   name:             [u8; 64]    = 64 字节 (偏移 16)
//   creator_pid:      u64         =  8 字节 (偏移 80)
//   create_time_ns:   u64         =  8 字节 (偏移 88)
//   cpu_time_ns:      u64         =  8 字节 (偏移 96)
//   memory_used:      u64         =  8 字节 (偏移 104)
//   memory_peak:      u64         =  8 字节 (偏移 112)
//   thread_count:     u32         =  4 字节 (偏移 120)
//   connection_count: u32         =  4 字节 (偏移 124)
//   msg_sent:         u64         =  8 字节 (偏移 128)
//   msg_received:     u64         =  8 字节 (偏移 136)
//   last_active_ns:   u64         =  8 字节 (偏移 144)
//   security_label:   [u8; 32]   = 32 字节 (偏移 152)
//   current_cpu:      u32         =  4 字节 (偏移 184)
//   _pad:             [u8; 4]    =  4 字节 (偏移 188)
//   _reserved:        [u8; 72]   = 72 字节 (偏移 192)
//   总计: 264 字节
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct AgentInfo {
    pub handle: AgentHandle,
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
            handle: AgentHandle::INVALID,
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

// === AgentMsgHeader (48 字节) ===
//
// 内存布局验证:
//   msg_type:      u32       =  4 字节 (偏移 0)
//   flags:         u32       =  4 字节 (偏移 4)
//   msg_id:        u64       =  8 字节 (偏移 8)
//   timestamp_ns:  u64       =  8 字节 (偏移 16)
//   payload_size:  u64       =  8 字节 (偏移 24)
//   shm_region_id: u32       =  4 字节 (偏移 32)
//   priority:      u8        =  1 字节 (偏移 36)
//   reserved:      [u8; 7]   =  7 字节 (偏移 37)
//   _pad:          [u8; 4]   =  4 字节 (偏移 44)
//   总计: 48 字节
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

// === 消息标志 ===
pub const MSG_SYNC: u32 = 1 << 0;
pub const MSG_ASYNC: u32 = 1 << 1;
pub const MSG_URGENT: u32 = 1 << 2;
pub const MSG_NOCOPY: u32 = 1 << 3;
pub const MSG_BROADCAST: u32 = 1 << 4;
pub const MSG_RELIABLE: u32 = 1 << 5;

// === 事件掩码 ===
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

// === 事件类型常量 ===
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

// === 迁移令牌 ===
//
// 内存布局验证:
//   token_id:      u64       =  8 字节 (偏移 0)
//   src_node_id:   [u8; 16]  = 16 字节 (偏移 8)
//   dest_node_id:  [u8; 16]  = 16 字节 (偏移 24)
//   timestamp_ns:  u64       =  8 字节 (偏移 40)
//   checksum:      u32       =  4 字节 (偏移 48)
//   flags:         u32       =  4 字节 (偏移 52)
//   总计: 56 字节
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MigrationToken {
    pub token_id: u64,
    pub src_node_id: [u8; 16],
    pub dest_node_id: [u8; 16],
    pub timestamp_ns: u64,
    pub checksum: u32,
    pub flags: u32,
}

// === 迁移标志 ===
pub const MIGRATE_LIVE: u32 = 1 << 0;
pub const MIGRATE_COLD: u32 = 1 << 1;
pub const MIGRATE_FORCE: u32 = 1 << 2;
pub const MIGRATE_VERIFY: u32 = 1 << 3;
pub const MIGRATE_COMPRESS: u32 = 1 << 4;
pub const MIGRATE_ENCRYPTED: u32 = 1 << 5;
pub const MIGRATE_CHECKPOINT: u32 = 1 << 6;

// === 共享内存规格 ===
//
// 内存布局验证:
//   size:     u64 = 8 字节 (偏移 0)
//   src_addr: u64 = 8 字节 (偏移 8)
//   dst_addr: u64 = 8 字节 (偏移 16)
//   prot:     u32 = 4 字节 (偏移 24)
//   flags:    u32 = 4 字节 (偏移 28)
//   总计: 32 字节
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ShmSpec {
    pub size: u64,
    pub src_addr: u64,
    pub dst_addr: u64,
    pub prot: u32,
    pub flags: u32,
}

// === Agent 能力定义 ===
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

// === 终止信号 ===
pub const SIG_KILL: u32 = 0;
pub const SIG_TERM: u32 = 1;
pub const SIG_COREDUMP: u32 = 2;
pub const SIG_FREEZE: u32 = 3;
pub const SIG_THAW: u32 = 4;

// === Syscall 错误码 ===
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
pub const E_QUOTA: isize = -200;  // 自定义: 配额超限
pub const E_CAP: isize = -201;    // 自定义: 能力不足

// === Syscall 错误类型 ===
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SyscallError {
    // 通用错误
    EPERM = 1,
    ENOENT = 2,
    ESRCH = 3,
    EINTR = 4,
    EIO = 5,
    ENXIO = 6,
    EAGAIN = 11,
    ENOMEM = 12,
    EACCES = 13,
    EFAULT = 14,
    EBUSY = 16,
    EEXIST = 17,
    EINVAL = 22,
    ENOSPC = 28,
    EOVERFLOW = 75,
    ENOTSUP = 95,
    ENOTCONN = 107,
    ETIMEDOUT = 110,
    EALREADY = 114,
    EHOSTUNREACH = 113,
    // Agent 专用错误
    EAGENT_INVALID_STATE = 1000,
    EAGENT_NOT_FOUND = 1001,
    EAGENT_BUSY = 1002,
    EAGENT_DEAD = 1003,
    EAGENT_FROZEN = 1004,
    EAGENT_MIGRATING = 1005,
    EAGENT_QUOTA_EXCEEDED = 1006,
    EAGENT_CAP_MISSING = 1007,
    EAGENT_AUTH_FAILED = 1008,
    EAGENT_SANDBOX_VIOLATION = 1009,
    EAGENT_MSG_TOO_LARGE = 1010,
    EAGENT_QUEUE_FULL = 1011,
    EAGENT_SHM_INVALID = 1012,
    EAGENT_SNAPSHOT_FAILED = 1013,
    EAGENT_RESTORE_FAILED = 1014,
}

#[cfg(test)]
mod tests {
    use super::*;

    // === AgentType 枚举值测试 ===
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

    // === AgentState 枚举值测试 ===
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

    // === SchedPolicy 枚举值测试 ===
    #[test]
    fn test_sched_policy_values() {
        assert_eq!(SchedPolicy::CFS as u8, 0);
        assert_eq!(SchedPolicy::RTFifo as u8, 1);
        assert_eq!(SchedPolicy::RTRR as u8, 2);
        assert_eq!(SchedPolicy::Idle as u8, 3);
        assert_eq!(SchedPolicy::Batch as u8, 4);
    }

    // === AgentSpec 大小测试 (必须恰好 256 字节) ===
    #[test]
    fn test_agentspec_size() {
        assert_eq!(
            core::mem::size_of::<AgentSpec>(),
            256,
            "AgentSpec 大小必须为 256 字节，实际为 {} 字节",
            core::mem::size_of::<AgentSpec>()
        );
    }

    // === AgentInfo 大小测试 (必须恰好 264 字节) ===
    #[test]
    fn test_agentinfo_size() {
        assert_eq!(
            core::mem::size_of::<AgentInfo>(),
            264,
            "AgentInfo 大小必须为 264 字节，实际为 {} 字节",
            core::mem::size_of::<AgentInfo>()
        );
    }

    // === AgentMsgHeader 大小测试 (必须恰好 48 字节) ===
    #[test]
    fn test_agent_msg_header_size() {
        assert_eq!(
            core::mem::size_of::<AgentMsgHeader>(),
            48,
            "AgentMsgHeader 大小必须为 48 字节，实际为 {} 字节",
            core::mem::size_of::<AgentMsgHeader>()
        );
    }

    // === 能力位图操作测试 ===
    #[test]
    fn test_cap_bitmap_operations() {
        let mut caps = CapBitmap::new();

        // 初始状态全部为零
        assert!(!caps.is_set(CAP_SPAWN_AGENT));
        assert!(!caps.is_set(CAP_NETWORK));
        assert!(!caps.is_set(CAP_ADMIN));

        // 设置能力位
        caps.set(CAP_SPAWN_AGENT);
        assert!(caps.is_set(CAP_SPAWN_AGENT));
        assert!(!caps.is_set(CAP_NETWORK));

        // 设置另一个能力位
        caps.set(CAP_NETWORK);
        assert!(caps.is_set(CAP_SPAWN_AGENT));
        assert!(caps.is_set(CAP_NETWORK));

        // 清除能力位
        caps.clear(CAP_SPAWN_AGENT);
        assert!(!caps.is_set(CAP_SPAWN_AGENT));
        assert!(caps.is_set(CAP_NETWORK));

        // 测试高位能力位 (跨越两个 u64 边界)
        caps.set(64); // 第二个 u64 的第 0 位
        assert!(caps.is_set(64));
        assert!(!caps.is_set(65));

        // 测试超出范围的能力位 (不应 panic)
        caps.set(128); // 超出 128 位范围
        assert!(!caps.is_set(128));

        // 清除不存在的位 (不应 panic)
        caps.clear(200);
    }

    // === AgentHandle 比较操作测试 ===
    #[test]
    fn test_agent_handle_equality() {
        let h1 = AgentHandle(1);
        let h2 = AgentHandle(1);
        let h3 = AgentHandle(2);

        // 相等性
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);

        // 克隆
        let h4 = h1.clone();
        assert_eq!(h1, h4);

        // 拷贝
        let h5 = h1;
        assert_eq!(h1, h5);

        // Hash 一致性
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        h1.hash(&mut hasher1);
        h2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    // === Agent 标志位组合测试 ===
    #[test]
    fn test_agent_flags() {
        // 无标志
        assert_eq!(AGENT_FLAG_NONE, 0);

        // 各标志位互不重叠
        assert_eq!(AGENT_FLAG_AUTO_START & AGENT_FLAG_PERSISTENT, 0);
        assert_eq!(AGENT_FLAG_ISOLATED & AGENT_FLAG_PRIVILEGED, 0);
        assert_eq!(AGENT_FLAG_ENCLAVED & AGENT_FLAG_AUTO_START, 0);

        // 标志组合
        let flags = AGENT_FLAG_AUTO_START | AGENT_FLAG_ISOLATED;
        assert_eq!(flags & AGENT_FLAG_AUTO_START, AGENT_FLAG_AUTO_START);
        assert_eq!(flags & AGENT_FLAG_ISOLATED, AGENT_FLAG_ISOLATED);
        assert_eq!(flags & AGENT_FLAG_PERSISTENT, 0);

        // 验证各标志位值
        assert_eq!(AGENT_FLAG_AUTO_START, 1);
        assert_eq!(AGENT_FLAG_PERSISTENT, 2);
        assert_eq!(AGENT_FLAG_ISOLATED, 4);
        assert_eq!(AGENT_FLAG_PRIVILEGED, 8);
        assert_eq!(AGENT_FLAG_ENCLAVED, 16);
    }

    // === 消息标志组合测试 ===
    #[test]
    fn test_msg_flags() {
        // 各消息标志位互不重叠
        assert_eq!(MSG_SYNC & MSG_ASYNC, 0);
        assert_eq!(MSG_URGENT & MSG_NOCOPY, 0);
        assert_eq!(MSG_BROADCAST & MSG_RELIABLE, 0);

        // 消息标志组合
        let flags = MSG_SYNC | MSG_URGENT;
        assert_eq!(flags & MSG_SYNC, MSG_SYNC);
        assert_eq!(flags & MSG_URGENT, MSG_URGENT);
        assert_eq!(flags & MSG_ASYNC, 0);

        // 验证各消息标志位值
        assert_eq!(MSG_SYNC, 1);
        assert_eq!(MSG_ASYNC, 2);
        assert_eq!(MSG_URGENT, 4);
        assert_eq!(MSG_NOCOPY, 8);
        assert_eq!(MSG_BROADCAST, 16);
        assert_eq!(MSG_RELIABLE, 32);
    }

    // === 事件掩码操作测试 ===
    #[test]
    fn test_event_mask_operations() {
        let mut mask = EventMask::new();

        // 初始状态全部为零
        assert!(!mask.is_set(0)); // EVT_STATE_CHANGED 位索引 0
        assert!(!mask.is_set(1)); // EVT_MSG_RECEIVED 位索引 1
        assert!(!mask.is_set(16)); // EVT_CUSTOM_START 位索引 16

        // 设置事件位 (使用位索引)
        mask.set(0); // EVT_STATE_CHANGED
        assert!(mask.is_set(0));
        assert!(!mask.is_set(1));

        // 设置多个事件位
        mask.set(1); // EVT_MSG_RECEIVED
        mask.set(5); // EVT_ERROR
        assert!(mask.is_set(0));
        assert!(mask.is_set(1));
        assert!(mask.is_set(5));

        // 清除事件位
        mask.clear(0); // EVT_STATE_CHANGED
        assert!(!mask.is_set(0));
        assert!(mask.is_set(1));

        // 测试高位事件 (EVT_CUSTOM_START 位索引 16, 在第一个 u64 中)
        mask.set(16);
        assert!(mask.is_set(16));

        // 测试跨 u64 边界 (位索引 64 在第二个 u64 中)
        mask.set(64);
        assert!(mask.is_set(64));
        assert!(!mask.is_set(65));

        // 测试超出范围 (不应 panic)
        mask.set(256);
        assert!(!mask.is_set(256));
    }

    // === 错误码负值测试 ===
    #[test]
    fn test_error_codes_negative() {
        // E_OK 是唯一非负错误码
        assert!(E_OK >= 0, "E_OK 应为非负值");

        // 所有其他错误码必须为负值
        let error_codes = [
            E_INVAL, E_PERM, E_NOENT, E_NOMEM, E_BUSY, E_EXIST,
            E_NOTSUP, E_AGAIN, E_SRCH, E_ACCES, E_FAULT, E_TIMEOUT,
            E_QUOTA, E_CAP,
        ];
        for &code in &error_codes {
            assert!(code < 0, "错误码 {} 应为负值", code);
        }

        // 验证自定义错误码范围
        assert!(E_QUOTA < -100, "E_QUOTA 应在自定义范围内");
        assert!(E_CAP < -100, "E_CAP 应在自定义范围内");
        assert!(E_QUOTA > E_CAP, "E_QUOTA 应大于 E_CAP");
    }

    // === 资源配额默认值测试 ===
    #[test]
    fn test_resource_quota_default() {
        let quota = ResourceQuota::new();
        assert_eq!(quota.max_cpu_percent, 0);
        assert_eq!(quota.max_memory_bytes, 0);
        assert_eq!(quota.max_fds, 0);
        assert_eq!(quota.max_threads, 0);
        assert_eq!(quota.max_msg_per_sec, 0);
        assert_eq!(quota.max_shm_bytes, 0);

        // Default trait 也应产生相同结果
        let quota2 = ResourceQuota::default();
        assert_eq!(quota.max_cpu_percent, quota2.max_cpu_percent);
        assert_eq!(quota.max_memory_bytes, quota2.max_memory_bytes);
    }

    // === 迁移令牌大小测试 ===
    #[test]
    fn test_migration_token_size() {
        assert_eq!(
            core::mem::size_of::<MigrationToken>(),
            56,
            "MigrationToken 大小必须为 56 字节，实际为 {} 字节",
            core::mem::size_of::<MigrationToken>()
        );
    }

    // === 共享内存规格大小测试 ===
    #[test]
    fn test_shm_spec_size() {
        assert_eq!(
            core::mem::size_of::<ShmSpec>(),
            32,
            "ShmSpec 大小必须为 32 字节，实际为 {} 字节",
            core::mem::size_of::<ShmSpec>()
        );
    }
}
