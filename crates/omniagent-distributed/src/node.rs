//! 节点管理：NodeId、NodeState、NodeInfo

use std::collections::HashMap;
use std::fmt;

/// 节点 ID（16 字节 UUID）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub [u8; 16]);

impl NodeId {
    /// 生成随机 UUID v4 节点 ID
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        // 使用简单的伪随机生成器（无外部依赖）
        // 基于静态计数器混合地址空间生成
        use std::cell::Cell;

        thread_local! {
            static COUNTER: Cell<u64> = Cell::new(0);
        }

        COUNTER.with(|c| {
            let mut state = c.get();
            // 简单的 xorshift64 伪随机数生成器
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            c.set(state.wrapping_add(1));

            // 将状态填入字节
            let state_bytes = state.to_le_bytes();
            // 混入栈地址以增加熵
            let addr = &state as *const u64 as usize;
            let addr_bytes = addr.to_le_bytes();

            for i in 0..8 {
                bytes[i] = state_bytes[i] ^ addr_bytes[i];
                bytes[i + 8] = state_bytes[(i + 4) % 8] ^ addr_bytes[(i + 2) % 8];
            }

            // 设置 UUID v4 版本号（第 6 字节高 4 位 = 0100）
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            // 设置变体号（第 8 字节高 2 位 = 10）
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
        });

        NodeId(bytes)
    }

    /// 从 16 字节数组创建节点 ID
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        NodeId(bytes)
    }

    /// 获取节点 ID 的字节引用
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// 将节点 ID 格式化为 UUID 字符串
    pub fn to_string(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5],
            self.0[6], self.0[7],
            self.0[8], self.0[9],
            self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }

    /// 判断是否为本地节点（全零节点 ID 视为本地）
    pub fn is_local(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }

    /// 创建本地节点 ID（全零）
    pub fn local() -> Self {
        NodeId([0u8; 16])
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// 节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeState {
    /// 已断开连接
    Disconnected = 0,
    /// 正在连接中
    Connecting = 1,
    /// 已连接
    Connected = 2,
    /// 正在同步数据
    Syncing = 3,
    /// 正在排空（优雅关闭中）
    Draining = 4,
    /// 离线
    Offline = 5,
}

impl NodeState {
    /// 判断节点是否处于活跃状态（已连接或正在同步）
    pub fn is_active(&self) -> bool {
        matches!(self, NodeState::Connected | NodeState::Syncing)
    }

    /// 从数值创建节点状态
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(NodeState::Disconnected),
            1 => Some(NodeState::Connecting),
            2 => Some(NodeState::Connected),
            3 => Some(NodeState::Syncing),
            4 => Some(NodeState::Draining),
            5 => Some(NodeState::Offline),
            _ => None,
        }
    }
}

impl fmt::Display for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeState::Disconnected => write!(f, "已断开"),
            NodeState::Connecting => write!(f, "连接中"),
            NodeState::Connected => write!(f, "已连接"),
            NodeState::Syncing => write!(f, "同步中"),
            NodeState::Draining => write!(f, "排空中"),
            NodeState::Offline => write!(f, "离线"),
        }
    }
}

/// 节点信息
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// 节点 ID
    pub id: NodeId,
    /// 节点地址
    pub address: String,
    /// 节点端口
    pub port: u16,
    /// 节点状态
    pub state: NodeState,
    /// Agent 数量
    pub agent_count: u32,
    /// 节点能力列表
    pub capabilities: Vec<String>,
    /// 最后心跳时间戳
    pub last_heartbeat: u64,
    /// 加入集群时间戳
    pub joined_at: u64,
    /// 自定义元数据
    pub metadata: HashMap<String, String>,
}

impl NodeInfo {
    /// 创建新的节点信息
    pub fn new(id: NodeId, address: String, port: u16) -> Self {
        NodeInfo {
            id,
            address,
            port,
            state: NodeState::Disconnected,
            agent_count: 0,
            capabilities: Vec::new(),
            last_heartbeat: 0,
            joined_at: 0,
            metadata: HashMap::new(),
        }
    }

    /// 创建新的已连接节点信息
    pub fn new_connected(id: NodeId, address: String, port: u16, current_time: u64) -> Self {
        NodeInfo {
            id,
            address,
            port,
            state: NodeState::Connected,
            agent_count: 0,
            capabilities: Vec::new(),
            last_heartbeat: current_time,
            joined_at: current_time,
            metadata: HashMap::new(),
        }
    }

    /// 设置节点状态
    pub fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }

    /// 添加能力
    pub fn add_capability(&mut self, capability: String) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// 获取元数据
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_new() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        // 两次生成的 ID 应该不同
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_node_id_from_bytes() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let id = NodeId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn test_node_id_to_string() {
        let bytes = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
                     0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10];
        let id = NodeId::from_bytes(bytes);
        let s = id.to_string();
        assert_eq!(s, "01234567-89ab-cdef-fedc-ba9876543210");
    }

    #[test]
    fn test_node_id_is_local() {
        let local = NodeId::local();
        assert!(local.is_local());

        let non_local = NodeId::new();
        assert!(!non_local.is_local());
    }

    #[test]
    fn test_node_state_is_active() {
        assert!(NodeState::Connected.is_active());
        assert!(NodeState::Syncing.is_active());
        assert!(!NodeState::Disconnected.is_active());
        assert!(!NodeState::Connecting.is_active());
        assert!(!NodeState::Draining.is_active());
        assert!(!NodeState::Offline.is_active());
    }

    #[test]
    fn test_node_state_from_u8() {
        assert_eq!(NodeState::from_u8(0), Some(NodeState::Disconnected));
        assert_eq!(NodeState::from_u8(2), Some(NodeState::Connected));
        assert_eq!(NodeState::from_u8(5), Some(NodeState::Offline));
        assert_eq!(NodeState::from_u8(99), None);
    }

    #[test]
    fn test_node_info_new() {
        let id = NodeId::new();
        let info = NodeInfo::new(id.clone(), "127.0.0.1".to_string(), 8080);
        assert_eq!(info.state, NodeState::Disconnected);
        assert_eq!(info.agent_count, 0);
        assert!(info.capabilities.is_empty());
    }

    #[test]
    fn test_node_info_capabilities() {
        let id = NodeId::new();
        let mut info = NodeInfo::new(id, "127.0.0.1".to_string(), 8080);
        info.add_capability("compute".to_string());
        info.add_capability("storage".to_string());
        // 重复添加不应产生重复
        info.add_capability("compute".to_string());
        assert_eq!(info.capabilities.len(), 2);
    }

    #[test]
    fn test_node_info_metadata() {
        let id = NodeId::new();
        let mut info = NodeInfo::new(id, "127.0.0.1".to_string(), 8080);
        info.set_metadata("region".to_string(), "us-east".to_string());
        assert_eq!(info.get_metadata("region"), Some(&"us-east".to_string()));
        assert_eq!(info.get_metadata("nonexistent"), None);
    }
}
