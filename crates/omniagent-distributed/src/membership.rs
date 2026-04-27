//! 集群成员管理
//!
//! 管理集群中所有节点的注册、心跳检测和状态追踪。

use std::collections::HashMap;

use crate::error::DistributedError;
use crate::node::{NodeId, NodeInfo, NodeState};

/// 集群成员管理器
///
/// 维护集群中所有节点的信息，包括节点状态、心跳检测等。
pub struct ClusterMembership {
    /// 所有已知节点
    nodes: HashMap<NodeId, NodeInfo>,
    /// 本地节点 ID
    local_node: NodeId,
    /// 心跳间隔（毫秒）
    heartbeat_interval_ms: u64,
    /// 心跳超时时间（毫秒）
    heartbeat_timeout_ms: u64,
}

impl ClusterMembership {
    /// 创建新的集群成员管理器
    ///
    /// # 参数
    /// - `local_node`: 本地节点的 ID
    pub fn new(local_node: NodeId) -> Self {
        ClusterMembership {
            nodes: HashMap::new(),
            local_node,
            heartbeat_interval_ms: 5000,   // 默认 5 秒心跳间隔
            heartbeat_timeout_ms: 15000,   // 默认 15 秒超时
        }
    }

    /// 创建带自定义心跳配置的集群成员管理器
    ///
    /// # 参数
    /// - `local_node`: 本地节点的 ID
    /// - `heartbeat_interval_ms`: 心跳间隔（毫秒）
    /// - `heartbeat_timeout_ms`: 心跳超时时间（毫秒）
    pub fn with_config(
        local_node: NodeId,
        heartbeat_interval_ms: u64,
        heartbeat_timeout_ms: u64,
    ) -> Self {
        ClusterMembership {
            nodes: HashMap::new(),
            local_node,
            heartbeat_interval_ms,
            heartbeat_timeout_ms,
        }
    }

    /// 加入新节点
    ///
    /// 将节点信息添加到集群中。如果节点已存在则返回错误。
    pub fn add_node(&mut self, info: NodeInfo) -> Result<(), DistributedError> {
        if self.nodes.contains_key(&info.id) {
            return Err(DistributedError::NodeAlreadyExists);
        }
        self.nodes.insert(info.id.clone(), info);
        Ok(())
    }

    /// 移除节点
    ///
    /// 从集群中移除指定节点。如果节点不存在则返回错误。
    pub fn remove_node(&mut self, id: &NodeId) -> Result<(), DistributedError> {
        if self.nodes.remove(id).is_none() {
            return Err(DistributedError::NodeNotFound);
        }
        Ok(())
    }

    /// 更新节点心跳
    ///
    /// 更新指定节点的心跳时间戳。如果节点不存在则返回错误。
    pub fn update_heartbeat(&mut self, id: &NodeId) -> Result<(), DistributedError> {
        let node = self.nodes.get_mut(id).ok_or(DistributedError::NodeNotFound)?;
        node.last_heartbeat = 0; // 由调用者设置实际时间
        Ok(())
    }

    /// 更新节点心跳（带时间戳）
    pub fn update_heartbeat_with_time(
        &mut self,
        id: &NodeId,
        current_time: u64,
    ) -> Result<(), DistributedError> {
        let node = self.nodes.get_mut(id).ok_or(DistributedError::NodeNotFound)?;
        node.last_heartbeat = current_time;
        Ok(())
    }

    /// 获取所有活跃节点
    ///
    /// 返回状态为 Connected 或 Syncing 的节点列表
    pub fn active_nodes(&self) -> Vec<&NodeInfo> {
        self.nodes
            .values()
            .filter(|node| node.state.is_active())
            .collect()
    }

    /// 检查超时节点
    ///
    /// 返回心跳超时的节点 ID 列表，并将这些节点标记为 Offline
    pub fn check_timeouts(&mut self, current_time: u64) -> Vec<NodeId> {
        let mut timed_out = Vec::new();

        for (id, node) in &mut self.nodes {
            // 跳过本地节点
            if *id == self.local_node {
                continue;
            }

            // 跳过已经离线的节点
            if node.state == NodeState::Offline {
                continue;
            }

            // 检查心跳是否超时
            if current_time.saturating_sub(node.last_heartbeat) > self.heartbeat_timeout_ms {
                node.state = NodeState::Offline;
                timed_out.push(id.clone());
            }
        }

        timed_out
    }

    /// 获取节点信息
    pub fn get_node(&self, id: &NodeId) -> Option<&NodeInfo> {
        self.nodes.get(id)
    }

    /// 获取可修改的节点信息
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut NodeInfo> {
        self.nodes.get_mut(id)
    }

    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取所有节点 ID
    pub fn node_ids(&self) -> Vec<&NodeId> {
        self.nodes.keys().collect()
    }

    /// 获取本地节点 ID
    pub fn local_node(&self) -> &NodeId {
        &self.local_node
    }

    /// 设置心跳间隔
    pub fn set_heartbeat_interval(&mut self, interval_ms: u64) {
        self.heartbeat_interval_ms = interval_ms;
    }

    /// 设置心跳超时时间
    pub fn set_heartbeat_timeout(&mut self, timeout_ms: u64) {
        self.heartbeat_timeout_ms = timeout_ms;
    }

    /// 获取心跳间隔
    pub fn heartbeat_interval(&self) -> u64 {
        self.heartbeat_interval_ms
    }

    /// 获取心跳超时时间
    pub fn heartbeat_timeout(&self) -> u64 {
        self.heartbeat_timeout_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(id: NodeId, port: u16, current_time: u64) -> NodeInfo {
        NodeInfo::new_connected(id, "127.0.0.1".to_string(), port, current_time)
    }

    #[test]
    fn test_membership_add_node() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let node = NodeId::new();
        let info = create_test_node(node.clone(), 8080, 1000);

        assert!(membership.add_node(info).is_ok());
        assert_eq!(membership.node_count(), 1);
    }

    #[test]
    fn test_membership_add_duplicate_node() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let node = NodeId::new();
        let info1 = create_test_node(node.clone(), 8080, 1000);
        let info2 = create_test_node(node.clone(), 8081, 2000);

        assert!(membership.add_node(info1).is_ok());
        assert_eq!(
            membership.add_node(info2),
            Err(DistributedError::NodeAlreadyExists)
        );
    }

    #[test]
    fn test_membership_remove_node() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let node = NodeId::new();
        let info = create_test_node(node.clone(), 8080, 1000);
        membership.add_node(info).unwrap();

        assert!(membership.remove_node(&node).is_ok());
        assert_eq!(membership.node_count(), 0);
    }

    #[test]
    fn test_membership_remove_nonexistent_node() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let ghost = NodeId::new();
        assert_eq!(
            membership.remove_node(&ghost),
            Err(DistributedError::NodeNotFound)
        );
    }

    #[test]
    fn test_membership_update_heartbeat() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let node = NodeId::new();
        let info = create_test_node(node.clone(), 8080, 1000);
        membership.add_node(info).unwrap();

        assert!(membership.update_heartbeat_with_time(&node, 5000).is_ok());

        let updated = membership.get_node(&node).unwrap();
        assert_eq!(updated.last_heartbeat, 5000);
    }

    #[test]
    fn test_membership_update_heartbeat_nonexistent() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let ghost = NodeId::new();
        assert_eq!(
            membership.update_heartbeat(&ghost),
            Err(DistributedError::NodeNotFound)
        );
    }

    #[test]
    fn test_membership_active_nodes() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let node_a = NodeId::new();
        let node_b = NodeId::new();
        let node_c = NodeId::new();

        let mut info_a = create_test_node(node_a, 8080, 1000);
        let info_b = create_test_node(node_b, 8081, 1000);
        let mut info_c = create_test_node(node_c, 8082, 1000);

        // 设置不同的状态
        info_a.state = NodeState::Connected;
        info_c.state = NodeState::Offline;

        membership.add_node(info_a).unwrap();
        membership.add_node(info_b).unwrap();
        membership.add_node(info_c).unwrap();

        let active = membership.active_nodes();
        assert_eq!(active.len(), 2); // Connected + Syncing
    }

    #[test]
    fn test_membership_check_timeouts() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::with_config(
            local,
            5000,
            15000, // 15 秒超时
        );

        let node_a = NodeId::new();
        let node_b = NodeId::new();

        // node_a: 最后心跳在 1000，当前时间 20000（超时）
        let info_a = create_test_node(node_a.clone(), 8080, 1000);
        membership.add_node(info_a).unwrap();

        // node_b: 最后心跳在 18000，当前时间 20000（未超时）
        let info_b = create_test_node(node_b.clone(), 8081, 18000);
        membership.add_node(info_b).unwrap();

        let timed_out = membership.check_timeouts(20000);
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], node_a);

        // 超时节点应被标记为 Offline
        let node_info = membership.get_node(&node_a).unwrap();
        assert_eq!(node_info.state, NodeState::Offline);

        // 未超时节点应保持 Connected
        let node_b_info = membership.get_node(&node_b).unwrap();
        assert_eq!(node_b_info.state, NodeState::Connected);
    }

    #[test]
    fn test_membership_get_node() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        let node = NodeId::new();
        let info = create_test_node(node.clone(), 8080, 1000);
        membership.add_node(info).unwrap();

        let retrieved = membership.get_node(&node);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().port, 8080);

        let ghost = NodeId::new();
        assert!(membership.get_node(&ghost).is_none());
    }

    #[test]
    fn test_membership_node_count() {
        let local = NodeId::new();
        let mut membership = ClusterMembership::new(local);

        assert_eq!(membership.node_count(), 0);

        let node = NodeId::new();
        let info = create_test_node(node, 8080, 1000);
        membership.add_node(info).unwrap();

        assert_eq!(membership.node_count(), 1);
    }

    #[test]
    fn test_membership_local_node() {
        let local = NodeId::new();
        let membership = ClusterMembership::new(local.clone());

        assert_eq!(membership.local_node(), &local);
    }

    #[test]
    fn test_membership_with_config() {
        let local = NodeId::new();
        let membership = ClusterMembership::with_config(local, 3000, 9000);

        assert_eq!(membership.heartbeat_interval(), 3000);
        assert_eq!(membership.heartbeat_timeout(), 9000);
    }
}
