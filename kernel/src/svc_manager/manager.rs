//! 服务管理器
//!
//! 实现用户态服务的启动、停止、重启和状态管理。
//! 支持依赖检查、自动启动、启动顺序排序等功能。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use spin::Mutex;

// ============================================================================
// 服务状态
// ============================================================================

/// 服务运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// 已注册但未启动
    Stopped = 0,
    /// 正在启动
    Starting = 1,
    /// 正在运行
    Running = 2,
    /// 正在停止
    Stopping = 3,
    /// 已失败
    Failed = 4,
}

// ============================================================================
// 错误类型
// ============================================================================

/// 服务管理器错误类型
#[derive(Debug, Clone)]
pub enum SvcManagerError {
    /// 服务未找到
    ServiceNotFound(u64),
    /// 服务已在运行
    AlreadyRunning(u64),
    /// 启动失败
    StartFailed { reason: &'static str },
    /// 停止失败
    StopFailed { reason: &'static str },
    /// 配置无效
    InvalidConfig,
    /// 依赖未满足
    DependencyUnmet(String),
}

impl fmt::Display for SvcManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvcManagerError::ServiceNotFound(id) => {
                write!(f, "服务未找到: {}", id)
            }
            SvcManagerError::AlreadyRunning(id) => {
                write!(f, "服务已在运行: {}", id)
            }
            SvcManagerError::StartFailed { reason } => {
                write!(f, "启动失败: {}", reason)
            }
            SvcManagerError::StopFailed { reason } => {
                write!(f, "停止失败: {}", reason)
            }
            SvcManagerError::InvalidConfig => {
                write!(f, "配置无效")
            }
            SvcManagerError::DependencyUnmet(dep) => {
                write!(f, "依赖未满足: {}", dep)
            }
        }
    }
}

// ============================================================================
// 服务配置
// ============================================================================

/// 服务配置
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// 服务唯一标识
    pub service_id: u64,
    /// 是否自动启动
    pub auto_start: bool,
    /// 失败时是否自动重启
    pub restart_on_failure: bool,
    /// 最大重启次数
    pub max_restart_count: u32,
    /// 重启延迟（毫秒）
    pub restart_delay_ms: u32,
    /// 启动顺序（数值越小越先启动）
    pub start_order: u32,
    /// 依赖的服务 ID 列表
    pub dependencies: Vec<u64>,
}

// ============================================================================
// 服务状态快照
// ============================================================================

/// 服务状态快照
#[derive(Debug, Clone)]
pub struct ServiceSnapshot {
    /// 服务 ID
    pub service_id: u64,
    /// 当前状态
    pub state: ServiceState,
    /// 进程 ID
    pub pid: u64,
    /// 运行时间（毫秒）
    pub uptime_ms: u64,
    /// 已重启次数
    pub restart_count: u32,
    /// 已使用内存
    pub memory_used: u64,
    /// CPU 使用率
    pub cpu_usage: f32,
}

// ============================================================================
// 服务管理器
// ============================================================================

/// 服务管理器
///
/// 管理用户态服务的完整生命周期，包括注册配置、启动、停止、重启等操作。
pub struct ServiceManager {
    /// 服务配置映射表
    configs: Mutex<BTreeMap<u64, ServiceConfig>>,
    /// 服务状态快照映射表
    snapshots: Mutex<BTreeMap<u64, ServiceSnapshot>>,
    /// 按启动顺序排列的服务 ID 列表
    start_order: Mutex<Vec<u64>>,
}

impl ServiceManager {
    /// 创建新的服务管理器
    pub fn new() -> Self {
        ServiceManager {
            configs: Mutex::new(BTreeMap::new()),
            snapshots: Mutex::new(BTreeMap::new()),
            start_order: Mutex::new(Vec::new()),
        }
    }

    /// 注册服务配置
    ///
    /// 将服务配置注册到管理器中，服务初始状态为 Stopped。
    pub fn register_config(&self, config: ServiceConfig) -> Result<(), SvcManagerError> {
        let service_id = config.service_id;

        // 创建初始快照
        let snapshot = ServiceSnapshot {
            service_id,
            state: ServiceState::Stopped,
            pid: 0,
            uptime_ms: 0,
            restart_count: 0,
            memory_used: 0,
            cpu_usage: 0.0,
        };

        // 注册配置
        {
            let mut configs = self.configs.lock();
            configs.insert(service_id, config);
        }

        // 注册快照
        {
            let mut snapshots = self.snapshots.lock();
            snapshots.insert(service_id, snapshot);
        }

        // 按启动顺序插入排序列表
        {
            let mut order = self.start_order.lock();
            let start_ord = {
                let configs = self.configs.lock();
                configs.get(&service_id).map(|c| c.start_order).unwrap_or(u32::MAX)
            };
            // 找到正确的插入位置
            let pos = order
                .iter()
                .position(|&id| {
                    let configs = self.configs.lock();
                    configs.get(&id).map(|c| c.start_order).unwrap_or(u32::MAX) >= start_ord
                })
                .unwrap_or(order.len());
            order.insert(pos, service_id);
        }

        Ok(())
    }

    /// 启动服务
    ///
    /// 检查依赖是否满足，然后将服务状态设置为 Running。
    pub fn start_service(&self, service_id: u64) -> Result<(), SvcManagerError> {
        // 检查服务是否已注册
        let config = {
            let configs = self.configs.lock();
            configs
                .get(&service_id)
                .cloned()
                .ok_or(SvcManagerError::ServiceNotFound(service_id))?
        };

        // 检查是否已在运行
        {
            let snapshots = self.snapshots.lock();
            if let Some(snap) = snapshots.get(&service_id) {
                if snap.state == ServiceState::Running {
                    return Err(SvcManagerError::AlreadyRunning(service_id));
                }
            }
        }

        // 检查依赖是否满足
        for &dep_id in &config.dependencies {
            let snapshots = self.snapshots.lock();
            let dep_running = snapshots
                .get(&dep_id)
                .map(|s| s.state == ServiceState::Running)
                .unwrap_or(false);
            if !dep_running {
                // 在 no_std 环境下使用简单的字符串拼接
                let mut msg = alloc::string::String::from("依赖服务 ");
                // 将数字转换为字符串
                if dep_id == 0 {
                    msg.push_str("0");
                } else {
                    let mut n = dep_id;
                    let mut digits = alloc::vec::Vec::new();
                    while n > 0 {
                        digits.push((n % 10) as u8 + b'0');
                        n /= 10;
                    }
                    digits.reverse();
                    for d in digits {
                        msg.push(d as char);
                    }
                }
                msg.push_str(" 未运行");
                return Err(SvcManagerError::DependencyUnmet(msg));
            }
        }

        // 启动服务（模拟：分配 PID，设置状态为 Running）
        {
            let mut snapshots = self.snapshots.lock();
            if let Some(snap) = snapshots.get_mut(&service_id) {
                snap.state = ServiceState::Running;
                // 模拟分配 PID
                snap.pid = service_id * 1000 + 1;
                snap.uptime_ms = 0;
                snap.memory_used = 4096;
                snap.cpu_usage = 0.0;
            }
        }

        Ok(())
    }

    /// 停止服务
    ///
    /// 将服务状态设置为 Stopped，释放资源。
    pub fn stop_service(&self, service_id: u64) -> Result<(), SvcManagerError> {
        // 检查服务是否已注册
        {
            let configs = self.configs.lock();
            if !configs.contains_key(&service_id) {
                return Err(SvcManagerError::ServiceNotFound(service_id));
            }
        }

        // 停止服务
        {
            let mut snapshots = self.snapshots.lock();
            if let Some(snap) = snapshots.get_mut(&service_id) {
                snap.state = ServiceState::Stopped;
                snap.pid = 0;
                snap.uptime_ms = 0;
                snap.cpu_usage = 0.0;
            }
        }

        Ok(())
    }

    /// 重启服务
    ///
    /// 先停止再启动服务，增加重启计数。
    pub fn restart_service(&self, service_id: u64) -> Result<(), SvcManagerError> {
        // 检查服务是否已注册
        {
            let configs = self.configs.lock();
            if !configs.contains_key(&service_id) {
                return Err(SvcManagerError::ServiceNotFound(service_id));
            }
        }

        // 检查重启次数限制
        let max_restarts = {
            let configs = self.configs.lock();
            configs
                .get(&service_id)
                .map(|c| c.max_restart_count)
                .unwrap_or(0)
        };

        let current_restarts = {
            let snapshots = self.snapshots.lock();
            snapshots
                .get(&service_id)
                .map(|s| s.restart_count)
                .unwrap_or(0)
        };

        if current_restarts >= max_restarts {
            return Err(SvcManagerError::StartFailed {
                reason: "已达到最大重启次数",
            });
        }

        // 停止服务
        self.stop_service(service_id)?;

        // 增加重启计数
        {
            let mut snapshots = self.snapshots.lock();
            if let Some(snap) = snapshots.get_mut(&service_id) {
                snap.restart_count += 1;
            }
        }

        // 启动服务
        self.start_service(service_id)?;

        Ok(())
    }

    /// 获取服务状态快照
    pub fn get_snapshot(&self, service_id: u64) -> Option<ServiceSnapshot> {
        let snapshots = self.snapshots.lock();
        snapshots.get(&service_id).cloned()
    }

    /// 列出所有服务状态快照
    pub fn list_snapshots(&self) -> Vec<ServiceSnapshot> {
        let snapshots = self.snapshots.lock();
        snapshots.values().cloned().collect()
    }

    /// 按启动顺序启动所有标记为自动启动的服务
    pub fn start_all(&self) -> Result<(), SvcManagerError> {
        let order = {
            let order = self.start_order.lock();
            order.clone()
        };

        let auto_start_ids: Vec<u64> = {
            let configs = self.configs.lock();
            order
                .iter()
                .filter(|&&id| {
                    configs
                        .get(&id)
                        .map(|c| c.auto_start)
                        .unwrap_or(false)
                })
                .copied()
                .collect()
        };

        for id in auto_start_ids {
            self.start_service(id)?;
        }

        Ok(())
    }

    /// 停止所有正在运行的服务
    pub fn stop_all(&self) {
        let order = {
            let order = self.start_order.lock();
            order.clone()
        };

        // 逆序停止
        for id in order.iter().rev() {
            let _ = self.stop_service(*id);
        }
    }

    /// 获取已注册的服务总数
    pub fn service_count(&self) -> usize {
        let configs = self.configs.lock();
        configs.len()
    }

    /// 获取正在运行的服务数量
    pub fn running_count(&self) -> usize {
        let snapshots = self.snapshots.lock();
        snapshots
            .values()
            .filter(|s| s.state == ServiceState::Running)
            .count()
    }
}

/// 全局服务管理器实例
pub static SVC_MANAGER: spin::Lazy<Mutex<ServiceManager>> = spin::Lazy::new(|| {
    Mutex::new(ServiceManager::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用服务配置
    fn make_config(service_id: u64, start_order: u32) -> ServiceConfig {
        ServiceConfig {
            service_id,
            auto_start: false,
            restart_on_failure: true,
            max_restart_count: 3,
            restart_delay_ms: 1000,
            start_order,
            dependencies: Vec::new(),
        }
    }

    /// 创建带依赖的服务配置
    fn make_config_with_deps(service_id: u64, start_order: u32, deps: Vec<u64>) -> ServiceConfig {
        ServiceConfig {
            service_id,
            auto_start: false,
            restart_on_failure: true,
            max_restart_count: 3,
            restart_delay_ms: 1000,
            start_order,
            dependencies: deps,
        }
    }

    // === 测试: 创建服务管理器 ===
    #[test]
    fn test_new() {
        let mgr = ServiceManager::new();
        assert_eq!(mgr.service_count(), 0);
        assert_eq!(mgr.running_count(), 0);
    }

    // === 测试: 注册服务配置 ===
    #[test]
    fn test_register_config() {
        let mgr = ServiceManager::new();
        let config = make_config(1, 10);
        assert!(mgr.register_config(config).is_ok());
        assert_eq!(mgr.service_count(), 1);

        // 验证初始快照
        let snap = mgr.get_snapshot(1).unwrap();
        assert_eq!(snap.state, ServiceState::Stopped);
        assert_eq!(snap.restart_count, 0);
    }

    // === 测试: 启动服务 ===
    #[test]
    fn test_start_service() {
        let mgr = ServiceManager::new();
        mgr.register_config(make_config(1, 10)).unwrap();
        assert!(mgr.start_service(1).is_ok());

        let snap = mgr.get_snapshot(1).unwrap();
        assert_eq!(snap.state, ServiceState::Running);
        assert_ne!(snap.pid, 0);
        assert_eq!(mgr.running_count(), 1);
    }

    // === 测试: 启动未注册的服务 ===
    #[test]
    fn test_start_unregistered() {
        let mgr = ServiceManager::new();
        let result = mgr.start_service(999);
        assert!(result.is_err());
        match result.unwrap_err() {
            SvcManagerError::ServiceNotFound(id) => assert_eq!(id, 999),
            _ => panic!("期望 ServiceNotFound 错误"),
        }
    }

    // === 测试: 重复启动服务 ===
    #[test]
    fn test_start_already_running() {
        let mgr = ServiceManager::new();
        mgr.register_config(make_config(1, 10)).unwrap();
        mgr.start_service(1).unwrap();

        let result = mgr.start_service(1);
        assert!(result.is_err());
        match result.unwrap_err() {
            SvcManagerError::AlreadyRunning(id) => assert_eq!(id, 1),
            _ => panic!("期望 AlreadyRunning 错误"),
        }
    }

    // === 测试: 停止服务 ===
    #[test]
    fn test_stop_service() {
        let mgr = ServiceManager::new();
        mgr.register_config(make_config(1, 10)).unwrap();
        mgr.start_service(1).unwrap();
        assert_eq!(mgr.running_count(), 1);

        assert!(mgr.stop_service(1).is_ok());
        let snap = mgr.get_snapshot(1).unwrap();
        assert_eq!(snap.state, ServiceState::Stopped);
        assert_eq!(snap.pid, 0);
        assert_eq!(mgr.running_count(), 0);
    }

    // === 测试: 停止所有服务 ===
    #[test]
    fn test_stop_all() {
        let mgr = ServiceManager::new();
        mgr.register_config(make_config(1, 10)).unwrap();
        mgr.register_config(make_config(2, 20)).unwrap();
        mgr.start_service(1).unwrap();
        mgr.start_service(2).unwrap();
        assert_eq!(mgr.running_count(), 2);

        mgr.stop_all();
        assert_eq!(mgr.running_count(), 0);
    }

    // === 测试: 重启服务 ===
    #[test]
    fn test_restart_service() {
        let mgr = ServiceManager::new();
        mgr.register_config(make_config(1, 10)).unwrap();
        mgr.start_service(1).unwrap();

        assert!(mgr.restart_service(1).is_ok());
        let snap = mgr.get_snapshot(1).unwrap();
        assert_eq!(snap.state, ServiceState::Running);
        assert_eq!(snap.restart_count, 1);
    }

    // === 测试: 依赖检查 ===
    #[test]
    fn test_dependency_check() {
        let mgr = ServiceManager::new();
        // 服务 2 依赖服务 1
        mgr.register_config(make_config(1, 10)).unwrap();
        mgr.register_config(make_config_with_deps(2, 20, alloc::vec![1])).unwrap();

        // 依赖未满足时应失败
        let result = mgr.start_service(2);
        assert!(result.is_err());
        match result.unwrap_err() {
            SvcManagerError::DependencyUnmet(_) => {}
            _ => panic!("期望 DependencyUnmet 错误"),
        }

        // 启动依赖后应成功
        mgr.start_service(1).unwrap();
        assert!(mgr.start_service(2).is_ok());
    }

    // === 测试: 列出所有快照 ===
    #[test]
    fn test_list_snapshots() {
        let mgr = ServiceManager::new();
        mgr.register_config(make_config(1, 10)).unwrap();
        mgr.register_config(make_config(2, 20)).unwrap();
        mgr.register_config(make_config(3, 30)).unwrap();

        let snapshots = mgr.list_snapshots();
        assert_eq!(snapshots.len(), 3);
    }

    // === 测试: 启动所有自动启动的服务 ===
    #[test]
    fn test_start_all() {
        let mgr = ServiceManager::new();

        // 服务 1: 自动启动，启动顺序 10
        let mut config1 = make_config(1, 10);
        config1.auto_start = true;
        mgr.register_config(config1).unwrap();

        // 服务 2: 不自动启动
        let config2 = make_config(2, 20);
        mgr.register_config(config2).unwrap();

        // 服务 3: 自动启动，启动顺序 5（比服务 1 先启动）
        let mut config3 = make_config(3, 5);
        config3.auto_start = true;
        mgr.register_config(config3).unwrap();

        mgr.start_all().unwrap();
        assert_eq!(mgr.running_count(), 2);

        // 验证服务 1 和 3 在运行，服务 2 未运行
        assert_eq!(mgr.get_snapshot(1).unwrap().state, ServiceState::Running);
        assert_eq!(mgr.get_snapshot(2).unwrap().state, ServiceState::Stopped);
        assert_eq!(mgr.get_snapshot(3).unwrap().state, ServiceState::Running);
    }
}
