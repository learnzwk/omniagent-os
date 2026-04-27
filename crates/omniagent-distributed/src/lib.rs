//! OmniAgent 分布式服务
//!
//! 本 crate 实现了分布式 Agent 通信和状态同步功能，包括：
//! - 节点管理（NodeId, NodeInfo, ClusterMembership）
//! - CRDT 状态同步（VectorClock, CrdtCounter, CrdtSet, CrdtRegister）
//! - 分布式消息传递（DistributedMessage, MessageBus）

pub mod error;
pub mod node;
pub mod crdt;
pub mod message;
pub mod membership;

// 重新导出核心类型
pub use error::DistributedError;
pub use node::{NodeId, NodeState, NodeInfo};
pub use crdt::{VectorClock, CausalOrder, CrdtCounter, CrdtSet, CrdtRegister};
pub use message::{DistributedMessageType, DistributedMessage, MessageBus};
pub use membership::ClusterMembership;
