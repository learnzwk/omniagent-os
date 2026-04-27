//! 分布式错误类型定义

use std::fmt;

/// 分布式系统错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DistributedError {
    /// 节点未找到
    NodeNotFound,
    /// 节点已存在
    NodeAlreadyExists,
    /// 未连接
    NotConnected,
    /// 超时（超时时间，单位毫秒）
    Timeout(u64),
    /// 消息过大（消息大小，单位字节）
    MessageTooLarge(usize),
    /// 序列化错误
    SerializationError(String),
    /// RPC 错误
    RpcError(String),
    /// 集群已满
    ClusterFull,
    /// 无效的节点 ID
    InvalidNodeId,
    /// 时钟不同步
    ClockDesync,
}

impl fmt::Display for DistributedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistributedError::NodeNotFound => write!(f, "节点未找到"),
            DistributedError::NodeAlreadyExists => write!(f, "节点已存在"),
            DistributedError::NotConnected => write!(f, "未连接"),
            DistributedError::Timeout(ms) => write!(f, "超时 ({}ms)", ms),
            DistributedError::MessageTooLarge(size) => write!(f, "消息过大 ({} 字节)", size),
            DistributedError::SerializationError(msg) => write!(f, "序列化错误: {}", msg),
            DistributedError::RpcError(msg) => write!(f, "RPC 错误: {}", msg),
            DistributedError::ClusterFull => write!(f, "集群已满"),
            DistributedError::InvalidNodeId => write!(f, "无效的节点 ID"),
            DistributedError::ClockDesync => write!(f, "时钟不同步"),
        }
    }
}

impl std::error::Error for DistributedError {}
