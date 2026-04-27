//! Agent 系统调用模块
//!
//! 本模块定义了 OmniAgent OS 内核侧的 Agent 系统调用 ABI，
//! 包含类型定义和系统调用编号，作为内核与用户态之间的契约。

pub mod abi;
pub mod numbers;
pub mod dispatch;

// 导出核心类型，方便其他内核模块直接使用
pub use abi::{
    AgentType, AgentState, SchedPolicy, CapBitmap, ResourceQuota,
    AgentHandle, AgentSpec, AgentInfo, AgentMsgHeader, EventMask,
    MigrationToken, ShmSpec, SyscallError,
    // Agent 标志常量
    AGENT_FLAG_NONE, AGENT_FLAG_AUTO_START, AGENT_FLAG_PERSISTENT,
    AGENT_FLAG_ISOLATED, AGENT_FLAG_PRIVILEGED, AGENT_FLAG_ENCLAVED,
    // 消息标志常量
    MSG_SYNC, MSG_ASYNC, MSG_URGENT, MSG_NOCOPY, MSG_BROADCAST, MSG_RELIABLE,
    // 事件类型常量
    EVT_STATE_CHANGED, EVT_MSG_RECEIVED, EVT_CAP_REGISTERED, EVT_CAP_REVOKED,
    EVT_RESOURCE_LOW, EVT_ERROR, EVT_MIGRATION, EVT_SNAPSHOT, EVT_HEARTBEAT,
    EVT_CUSTOM_START,
    // 迁移标志常量
    MIGRATE_LIVE, MIGRATE_COLD, MIGRATE_FORCE, MIGRATE_VERIFY,
    MIGRATE_COMPRESS, MIGRATE_ENCRYPTED, MIGRATE_CHECKPOINT,
    // 能力定义常量
    CAP_SPAWN_AGENT, CAP_SPAWN_SYSTEM_AGENT, CAP_NETWORK, CAP_DEVICE_ACCESS,
    CAP_IPC, CAP_SHARED_MEMORY, CAP_FILESYSTEM, CAP_ADMIN, CAP_VIRTUALIZATION,
    CAP_ENCLAVE, CAP_SPAWN_ENCLAVED,
    // 终止信号常量
    SIG_KILL, SIG_TERM, SIG_COREDUMP, SIG_FREEZE, SIG_THAW,
    // 错误码常量
    E_OK, E_INVAL, E_PERM, E_NOENT, E_NOMEM, E_BUSY, E_EXIST,
    E_NOTSUP, E_AGAIN, E_SRCH, E_ACCES, E_FAULT, E_TIMEOUT,
    E_QUOTA, E_CAP,
};

pub use numbers::*;
