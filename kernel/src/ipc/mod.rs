//! 零拷贝 IPC 模块
//!
//! 模仿鸿蒙零拷贝 IPC 思想，实现：
//! - 共享内存池管理
//! - IPC 通道通信
//! - 消息头零拷贝传输
//!
//! 零拷贝 IPC 是鸿蒙高性能通信的核心机制，通过共享内存
//! 避免数据在内核空间和用户空间之间的冗余拷贝。

pub mod error;
pub mod shared_memory;
pub mod channel;

pub use error::IpcError;
pub use shared_memory::{SharedMemoryRegion, SharedMemoryPool, SHM_POOL};
pub use channel::{IpcMessageHeader, IpcChannel, IpcChannelManager, IPC_MANAGER};
