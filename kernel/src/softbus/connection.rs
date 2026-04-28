//! 连接管理模块
//! 模仿鸿蒙 DSoftBus 的连接管理机制，支持多种连接类型、状态管理和最优连接选择

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use crate::softbus::error::SoftBusError;

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 已断开
    Disconnected = 0,
    /// 正在连接
    Connecting = 1,
    /// 已连接
    Connected = 2,
    /// 正在认证
    Authenticating = 3,
    /// 正在断开
    Disconnecting = 4,
}

/// 连接类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    /// 进程内连接
    Local = 0,
    /// TCP 连接
    Tcp = 1,
    /// UDP 连接
    Udp = 2,
    /// 共享内存
    SharedMemory = 3,
    /// 蓝牙连接
    Bluetooth = 4,
    /// Wi-Fi 直连
    Wifi = 5,
}

/// 连接信息
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// 连接唯一标识
    pub connection_id: u64,
    /// 对端设备 ID
    pub peer_device_id: u64,
    /// 连接类型
    pub conn_type: ConnectionType,
    /// 连接状态
    pub state: ConnectionState,
    /// 延迟（微秒）
    pub latency_us: u32,
    /// 带宽（Kbps）
    pub bandwidth: u32,
    /// 创建时间戳
    pub created_at: u64,
    /// 最后活跃时间戳
    pub last_active: u64,
}

/// 连接管理器
/// 负责管理设备间的多种连接，支持连接建立、断开和最优连接选择
pub struct ConnectionManager {
    /// 连接表
    connections: Mutex<BTreeMap<u64, ConnectionInfo>>,
    /// 下一个连接 ID
    next_conn_id: AtomicU64,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(BTreeMap::new()),
            next_conn_id: AtomicU64::new(1),
        }
    }

    /// 建立到指定设备的连接
    ///
    /// # 参数
    /// - `peer_id`: 对端设备 ID
    /// - `conn_type`: 连接类型
    ///
    /// # 返回
    /// 成功返回连接 ID
    pub fn connect(&self, peer_id: u64, conn_type: ConnectionType) -> Result<u64, SoftBusError> {
        let conn_id = self.next_conn_id.fetch_add(1, Ordering::SeqCst);
        let info = ConnectionInfo {
            connection_id: conn_id,
            peer_device_id: peer_id,
            conn_type,
            state: ConnectionState::Connecting,
            latency_us: 0,
            bandwidth: 0,
            created_at: 1000,
            last_active: 1000,
        };

        let mut connections = self.connections.lock();
        connections.insert(conn_id, info);
        Ok(conn_id)
    }

    /// 断开指定连接
    ///
    /// # 参数
    /// - `conn_id`: 连接 ID
    pub fn disconnect(&self, conn_id: u64) -> Result<(), SoftBusError> {
        let mut connections = self.connections.lock();
        if let Some(conn) = connections.get_mut(&conn_id) {
            conn.state = ConnectionState::Disconnecting;
            connections.remove(&conn_id);
            Ok(())
        } else {
            Err(SoftBusError::NotConnected)
        }
    }

    /// 获取连接信息
    ///
    /// # 参数
    /// - `conn_id`: 连接 ID
    pub fn get_connection(&self, conn_id: u64) -> Option<ConnectionInfo> {
        let connections = self.connections.lock();
        connections.get(&conn_id).cloned()
    }

    /// 列出所有连接
    pub fn list_connections(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.lock();
        connections.values().cloned().collect()
    }

    /// 获取到指定设备的连接
    ///
    /// # 参数
    /// - `peer_id`: 对端设备 ID
    pub fn get_connection_to(&self, peer_id: u64) -> Option<ConnectionInfo> {
        let connections = self.connections.lock();
        connections
            .values()
            .find(|c| c.peer_device_id == peer_id)
            .cloned()
    }

    /// 更新连接状态
    ///
    /// # 参数
    /// - `conn_id`: 连接 ID
    /// - `state`: 新状态
    pub fn update_state(&self, conn_id: u64, state: ConnectionState) {
        let mut connections = self.connections.lock();
        if let Some(conn) = connections.get_mut(&conn_id) {
            conn.state = state;
        }
    }

    /// 更新连接延迟
    ///
    /// # 参数
    /// - `conn_id`: 连接 ID
    /// - `latency_us`: 延迟（微秒）
    pub fn update_latency(&self, conn_id: u64, latency_us: u32) {
        let mut connections = self.connections.lock();
        if let Some(conn) = connections.get_mut(&conn_id) {
            conn.latency_us = latency_us;
        }
    }

    /// 获取连接总数
    pub fn connection_count(&self) -> usize {
        let connections = self.connections.lock();
        connections.len()
    }

    /// 选择到指定设备的最佳连接
    /// 选择标准：延迟最低的已连接连接
    ///
    /// # 参数
    /// - `peer_id`: 对端设备 ID
    pub fn select_best_connection(&self, peer_id: u64) -> Option<ConnectionInfo> {
        let connections = self.connections.lock();
        connections
            .values()
            .filter(|c| c.peer_device_id == peer_id && c.state == ConnectionState::Connected)
            .min_by_key(|c| c.latency_us)
            .cloned()
    }
}

/// 全局连接管理器实例
pub static CONNECTION_MANAGER: spin::Lazy<Mutex<ConnectionManager>> = spin::Lazy::new(|| {
    Mutex::new(ConnectionManager::new())
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(100, ConnectionType::Tcp).unwrap();
        assert_eq!(conn_id, 1);

        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.peer_device_id, 100);
        assert_eq!(conn.conn_type, ConnectionType::Tcp);
        assert_eq!(conn.state, ConnectionState::Connecting);
    }

    #[test]
    fn test_disconnect() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(200, ConnectionType::Wifi).unwrap();
        assert_eq!(manager.connection_count(), 1);

        assert!(manager.disconnect(conn_id).is_ok());
        assert_eq!(manager.connection_count(), 0);

        // 断开不存在的连接应返回错误
        let result = manager.disconnect(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_connection() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(300, ConnectionType::Bluetooth).unwrap();

        let conn = manager.get_connection(conn_id);
        assert!(conn.is_some());
        assert_eq!(conn.unwrap().peer_device_id, 300);

        // 获取不存在的连接
        let not_found = manager.get_connection(999);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_list_connections() {
        let manager = ConnectionManager::new();
        manager.connect(1, ConnectionType::Tcp).unwrap();
        manager.connect(2, ConnectionType::Udp).unwrap();
        manager.connect(3, ConnectionType::Local).unwrap();

        let list = manager.list_connections();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_update_state() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(100, ConnectionType::Tcp).unwrap();

        manager.update_state(conn_id, ConnectionState::Connected);
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.state, ConnectionState::Connected);

        manager.update_state(conn_id, ConnectionState::Authenticating);
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.state, ConnectionState::Authenticating);
    }

    #[test]
    fn test_update_latency() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(100, ConnectionType::Tcp).unwrap();

        manager.update_latency(conn_id, 500);
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.latency_us, 500);

        manager.update_latency(conn_id, 100);
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.latency_us, 100);
    }

    #[test]
    fn test_select_best() {
        let manager = ConnectionManager::new();

        // 建立到同一设备的两条连接
        let conn1 = manager.connect(100, ConnectionType::Tcp).unwrap();
        let conn2 = manager.connect(100, ConnectionType::Wifi).unwrap();

        // 设置为已连接状态并更新延迟
        manager.update_state(conn1, ConnectionState::Connected);
        manager.update_latency(conn1, 1000);
        manager.update_state(conn2, ConnectionState::Connected);
        manager.update_latency(conn2, 100);

        // 应选择延迟最低的连接
        let best = manager.select_best_connection(100).unwrap();
        assert_eq!(best.connection_id, conn2);
        assert_eq!(best.latency_us, 100);
    }

    #[test]
    fn test_connection_count() {
        let manager = ConnectionManager::new();
        assert_eq!(manager.connection_count(), 0);

        manager.connect(1, ConnectionType::Tcp).unwrap();
        manager.connect(2, ConnectionType::Udp).unwrap();
        assert_eq!(manager.connection_count(), 2);

        manager.disconnect(1).unwrap();
        assert_eq!(manager.connection_count(), 1);
    }

    /// 测试：连接状态转换完整流程
    #[test]
    fn test_connection_state_transitions() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(100, ConnectionType::Tcp).unwrap();

        // 初始状态为 Connecting
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.state, ConnectionState::Connecting);

        // Connecting -> Authenticating
        manager.update_state(conn_id, ConnectionState::Authenticating);
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.state, ConnectionState::Authenticating);

        // Authenticating -> Connected
        manager.update_state(conn_id, ConnectionState::Connected);
        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.state, ConnectionState::Connected);

        // Connected -> Disconnecting -> 移除
        manager.update_state(conn_id, ConnectionState::Disconnecting);
        manager.disconnect(conn_id).unwrap();
        assert_eq!(manager.connection_count(), 0);
    }

    /// 测试：同一设备建立多条连接
    #[test]
    fn test_multiple_connections_same_peer() {
        let manager = ConnectionManager::new();

        let conn1 = manager.connect(100, ConnectionType::Tcp).unwrap();
        let conn2 = manager.connect(100, ConnectionType::Wifi).unwrap();
        let conn3 = manager.connect(100, ConnectionType::Bluetooth).unwrap();

        assert_eq!(manager.connection_count(), 3);

        // 查询到该设备的连接
        let conn = manager.get_connection_to(100);
        assert!(conn.is_some());

        // 断开一条不影响其他
        manager.disconnect(conn1).unwrap();
        assert_eq!(manager.connection_count(), 2);
        assert!(manager.get_connection_to(100).is_some());

        manager.disconnect(conn2).unwrap();
        assert_eq!(manager.connection_count(), 1);

        manager.disconnect(conn3).unwrap();
        assert_eq!(manager.connection_count(), 0);
        assert!(manager.get_connection_to(100).is_none());
    }

    /// 测试：不同连接类型的连接
    #[test]
    fn test_all_connection_types() {
        let manager = ConnectionManager::new();

        let types = [
            ConnectionType::Local,
            ConnectionType::Tcp,
            ConnectionType::Udp,
            ConnectionType::SharedMemory,
            ConnectionType::Bluetooth,
            ConnectionType::Wifi,
        ];

        for conn_type in &types {
            let conn_id = manager.connect(1, *conn_type).unwrap();
            let conn = manager.get_connection(conn_id).unwrap();
            assert_eq!(conn.conn_type, *conn_type);
        }

        assert_eq!(manager.connection_count(), types.len());
    }

    /// 测试：select_best_connection 只选择 Connected 状态的连接
    #[test]
    fn test_select_best_only_connected() {
        let manager = ConnectionManager::new();

        let conn1 = manager.connect(100, ConnectionType::Tcp).unwrap();
        let conn2 = manager.connect(100, ConnectionType::Wifi).unwrap();

        // 两条连接都不是 Connected 状态
        manager.update_latency(conn1, 50);
        manager.update_latency(conn2, 10);

        // 没有已连接的连接，应返回 None
        let best = manager.select_best_connection(100);
        assert!(best.is_none());

        // 将 conn2 设为 Connected
        manager.update_state(conn2, ConnectionState::Connected);
        let best = manager.select_best_connection(100).unwrap();
        assert_eq!(best.connection_id, conn2);
    }

    /// 测试：连接 ID 严格递增
    #[test]
    fn test_connection_id_increments() {
        let manager = ConnectionManager::new();

        let id1 = manager.connect(1, ConnectionType::Tcp).unwrap();
        let id2 = manager.connect(2, ConnectionType::Udp).unwrap();
        let id3 = manager.connect(3, ConnectionType::Wifi).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    /// 测试：断开不存在的连接应返回错误
    #[test]
    fn test_disconnect_nonexistent() {
        let manager = ConnectionManager::new();

        let result = manager.disconnect(999);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SoftBusError::NotConnected);
    }

    /// 测试：更新不存在连接的状态不应 panic
    #[test]
    fn test_update_state_nonexistent() {
        let manager = ConnectionManager::new();

        // 对不存在的连接更新状态不应 panic
        manager.update_state(999, ConnectionState::Connected);
        manager.update_latency(999, 100);
    }

    /// 测试：查询不存在设备的连接应返回 None
    #[test]
    fn test_get_connection_to_nonexistent() {
        let manager = ConnectionManager::new();

        let result = manager.get_connection_to(999);
        assert!(result.is_none());
    }

    /// 测试：连接信息字段验证
    #[test]
    fn test_connection_info_fields() {
        let manager = ConnectionManager::new();
        let conn_id = manager.connect(42, ConnectionType::Bluetooth).unwrap();

        let conn = manager.get_connection(conn_id).unwrap();
        assert_eq!(conn.connection_id, conn_id);
        assert_eq!(conn.peer_device_id, 42);
        assert_eq!(conn.conn_type, ConnectionType::Bluetooth);
        assert_eq!(conn.state, ConnectionState::Connecting);
        assert_eq!(conn.latency_us, 0);
        assert_eq!(conn.bandwidth, 0);
        assert_eq!(conn.created_at, 1000);
        assert_eq!(conn.last_active, 1000);
    }
}
