//! 服务注册表
//!
//! 实现鸿蒙风格的原子化服务注册、发现和查询功能。
//! 支持按名称、类型、能力进行服务查找。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use crate::service::error::ServiceError;

// ============================================================================
// 服务 ID
// ============================================================================

/// 服务唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceId(pub u64);

// ============================================================================
// 服务类型
// ============================================================================

/// 服务类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceType {
    /// 系统服务
    System = 0,
    /// Agent 服务
    Agent = 1,
    /// 驱动服务
    Driver = 2,
    /// 网络服务
    Network = 3,
    /// 文件系统服务
    FileSystem = 4,
    /// 安全服务
    Security = 5,
    /// UI 服务
    Ui = 6,
    /// AI 服务
    Ai = 7,
    /// 自定义服务
    Custom(u8),
}

// ============================================================================
// 服务状态
// ============================================================================

/// 服务生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceStateEnum {
    /// 已注册
    Registered = 0,
    /// 初始化中
    Initializing = 1,
    /// 运行中
    Running = 2,
    /// 停止中
    Stopping = 3,
    /// 已停止
    Stopped = 4,
    /// 失败
    Failed = 5,
}

// ============================================================================
// 服务信息
// ============================================================================

/// 服务信息结构体
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// 服务 ID
    pub id: ServiceId,
    /// 服务名称
    pub name: String,
    /// 版本号
    pub version: u32,
    /// 服务类型
    pub service_type: ServiceType,
    /// 提供者 Agent/任务 ID
    pub provider: u64,
    /// 能力列表
    pub capabilities: Vec<String>,
    /// 依赖列表
    pub dependencies: Vec<String>,
    /// 当前状态
    pub state: ServiceStateEnum,
    /// 优先级
    pub priority: u8,
    /// 已重启次数
    pub restart_count: u32,
    /// 最大重启次数
    pub max_restart: u32,
}

// ============================================================================
// 服务注册表
// ============================================================================

/// 服务注册表
///
/// 提供服务的注册、注销、查询等功能。
/// 使用 BTreeMap 存储服务信息，支持按 ID、名称、类型、能力进行查找。
pub struct ServiceRegistry {
    /// 服务映射表（ID -> 服务信息）
    services: Mutex<BTreeMap<u64, ServiceInfo>>,
    /// 名称索引（名称 -> ID）
    name_index: Mutex<BTreeMap<String, u64>>,
    /// 下一个可用 ID
    next_id: AtomicU64,
}

impl ServiceRegistry {
    /// 创建新的服务注册表
    pub fn new() -> Self {
        ServiceRegistry {
            services: Mutex::new(BTreeMap::new()),
            name_index: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 注册服务
    ///
    /// 将服务信息注册到注册表中，自动分配服务 ID。
    pub fn register(&self, mut info: ServiceInfo) -> Result<ServiceId, ServiceError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        info.id = ServiceId(id);

        // 检查名称是否已存在
        {
            let name_idx = self.name_index.lock();
            if name_idx.contains_key(&info.name) {
                return Err(ServiceError::ServiceAlreadyExists(id));
            }
        }

        // 注册服务
        {
            let mut services = self.services.lock();
            services.insert(id, info.clone());
        }

        // 更新名称索引
        {
            let mut name_idx = self.name_index.lock();
            name_idx.insert(info.name.clone(), id);
        }

        Ok(ServiceId(id))
    }

    /// 注销服务
    ///
    /// 从注册表中移除指定 ID 的服务。
    pub fn unregister(&self, id: ServiceId) -> Result<(), ServiceError> {
        let name = {
            let mut services = self.services.lock();
            let info = services
                .remove(&id.0)
                .ok_or(ServiceError::ServiceNotFound(id.0))?;
            info.name.clone()
        };

        // 移除名称索引
        {
            let mut name_idx = self.name_index.lock();
            name_idx.remove(&name);
        }

        Ok(())
    }

    /// 获取服务信息
    ///
    /// 根据服务 ID 获取服务信息的副本。
    pub fn get(&self, id: ServiceId) -> Option<ServiceInfo> {
        let services = self.services.lock();
        services.get(&id.0).cloned()
    }

    /// 按名称查找服务
    pub fn find_by_name(&self, name: &str) -> Option<ServiceInfo> {
        let name_idx = self.name_index.lock();
        let id = name_idx.get(name).copied()?;
        drop(name_idx);

        self.get(ServiceId(id))
    }

    /// 按类型查找服务
    pub fn find_by_type(&self, service_type: ServiceType) -> Vec<ServiceInfo> {
        let services = self.services.lock();
        services
            .values()
            .filter(|s| s.service_type == service_type)
            .cloned()
            .collect()
    }

    /// 按能力查找服务
    pub fn find_by_capability(&self, cap: &str) -> Vec<ServiceInfo> {
        let services = self.services.lock();
        services
            .values()
            .filter(|s| s.capabilities.iter().any(|c| c == cap))
            .cloned()
            .collect()
    }

    /// 更新服务状态
    pub fn update_state(&self, id: ServiceId, state: ServiceStateEnum) -> Result<(), ServiceError> {
        let mut services = self.services.lock();
        let info = services
            .get_mut(&id.0)
            .ok_or(ServiceError::ServiceNotFound(id.0))?;
        info.state = state;
        Ok(())
    }

    /// 列出所有服务
    pub fn list_services(&self) -> Vec<ServiceInfo> {
        let services = self.services.lock();
        services.values().cloned().collect()
    }

    /// 获取运行中的服务数量
    pub fn running_count(&self) -> usize {
        let services = self.services.lock();
        services
            .values()
            .filter(|s| s.state == ServiceStateEnum::Running)
            .count()
    }

    /// 获取服务总数
    pub fn total_count(&self) -> usize {
        let services = self.services.lock();
        services.len()
    }

    /// 增加服务重启计数
    pub fn increment_restart(&self, id: ServiceId) {
        let mut services = self.services.lock();
        if let Some(info) = services.get_mut(&id.0) {
            info.restart_count += 1;
        }
    }
}

/// 全局服务注册表
pub static SERVICE_REGISTRY: spin::Lazy<Mutex<ServiceRegistry>> = spin::Lazy::new(|| {
    Mutex::new(ServiceRegistry::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的服务信息
    fn make_service_info(name: &str, service_type: ServiceType) -> ServiceInfo {
        ServiceInfo {
            id: ServiceId(0), // 将由 register 分配
            name: String::from(name),
            version: 1,
            service_type,
            provider: 100,
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            state: ServiceStateEnum::Registered,
            priority: 10,
            restart_count: 0,
            max_restart: 3,
        }
    }

    /// 创建带能力的服务信息
    fn make_service_with_caps(
        name: &str,
        service_type: ServiceType,
        caps: &[&str],
    ) -> ServiceInfo {
        ServiceInfo {
            id: ServiceId(0),
            name: String::from(name),
            version: 1,
            service_type,
            provider: 100,
            capabilities: caps.iter().map(|c| String::from(*c)).collect(),
            dependencies: Vec::new(),
            state: ServiceStateEnum::Registered,
            priority: 10,
            restart_count: 0,
            max_restart: 3,
        }
    }

    // === 测试: 注册服务 ===
    #[test]
    fn test_register_service() {
        let registry = ServiceRegistry::new();
        let info = make_service_info("test_svc", ServiceType::System);
        let result = registry.register(info);
        assert!(result.is_ok());
        let id = result.unwrap();
        assert_eq!(id.0, 1);

        // 验证可以通过 ID 获取
        let fetched = registry.get(id);
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.name, "test_svc");
        assert_eq!(fetched.service_type, ServiceType::System);
    }

    // === 测试: 注销服务 ===
    #[test]
    fn test_unregister() {
        let registry = ServiceRegistry::new();
        let info = make_service_info("to_remove", ServiceType::Agent);
        let id = registry.register(info).unwrap();

        assert!(registry.unregister(id).is_ok());
        assert!(registry.get(id).is_none());
        assert!(registry.find_by_name("to_remove").is_none());
    }

    // === 测试: 获取服务 ===
    #[test]
    fn test_get_service() {
        let registry = ServiceRegistry::new();
        let info = make_service_info("get_test", ServiceType::Driver);
        let id = registry.register(info).unwrap();

        let fetched = registry.get(id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "get_test");

        // 不存在的 ID
        assert!(registry.get(ServiceId(999)).is_none());
    }

    // === 测试: 按名称查找 ===
    #[test]
    fn test_find_by_name() {
        let registry = ServiceRegistry::new();
        let info = make_service_info("name_svc", ServiceType::Network);
        registry.register(info).unwrap();

        let found = registry.find_by_name("name_svc");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "name_svc");

        // 不存在的名称
        assert!(registry.find_by_name("nonexistent").is_none());
    }

    // === 测试: 按类型查找 ===
    #[test]
    fn test_find_by_type() {
        let registry = ServiceRegistry::new();
        registry
            .register(make_service_info("sys1", ServiceType::System))
            .unwrap();
        registry
            .register(make_service_info("sys2", ServiceType::System))
            .unwrap();
        registry
            .register(make_service_info("agent1", ServiceType::Agent))
            .unwrap();

        let system_svcs = registry.find_by_type(ServiceType::System);
        assert_eq!(system_svcs.len(), 2);

        let agent_svcs = registry.find_by_type(ServiceType::Agent);
        assert_eq!(agent_svcs.len(), 1);

        let ai_svcs = registry.find_by_type(ServiceType::Ai);
        assert_eq!(ai_svcs.len(), 0);
    }

    // === 测试: 按能力查找 ===
    #[test]
    fn test_find_by_capability() {
        let registry = ServiceRegistry::new();
        registry
            .register(make_service_with_caps(
                "file_svc",
                ServiceType::FileSystem,
                &["read", "write"],
            ))
            .unwrap();
        registry
            .register(make_service_with_caps(
                "net_svc",
                ServiceType::Network,
                &["read", "network"],
            ))
            .unwrap();

        let read_svcs = registry.find_by_capability("read");
        assert_eq!(read_svcs.len(), 2);

        let write_svcs = registry.find_by_capability("write");
        assert_eq!(write_svcs.len(), 1);

        let missing_svcs = registry.find_by_capability("nonexistent_cap");
        assert_eq!(missing_svcs.len(), 0);
    }

    // === 测试: 更新状态 ===
    #[test]
    fn test_update_state() {
        let registry = ServiceRegistry::new();
        let info = make_service_info("state_svc", ServiceType::System);
        let id = registry.register(info).unwrap();

        // 初始状态应为 Registered
        let svc = registry.get(id).unwrap();
        assert_eq!(svc.state, ServiceStateEnum::Registered);

        // 更新为 Running
        assert!(registry.update_state(id, ServiceStateEnum::Running).is_ok());
        let svc = registry.get(id).unwrap();
        assert_eq!(svc.state, ServiceStateEnum::Running);

        // 更新不存在的服务
        assert!(registry
            .update_state(ServiceId(999), ServiceStateEnum::Stopped)
            .is_err());
    }

    // === 测试: 列出所有服务 ===
    #[test]
    fn test_list_services() {
        let registry = ServiceRegistry::new();
        registry
            .register(make_service_info("svc1", ServiceType::System))
            .unwrap();
        registry
            .register(make_service_info("svc2", ServiceType::Agent))
            .unwrap();
        registry
            .register(make_service_info("svc3", ServiceType::Driver))
            .unwrap();

        let all = registry.list_services();
        assert_eq!(all.len(), 3);
    }

    // === 测试: 运行中服务计数 ===
    #[test]
    fn test_running_count() {
        let registry = ServiceRegistry::new();
        let id1 = registry
            .register(make_service_info("run1", ServiceType::System))
            .unwrap();
        let id2 = registry
            .register(make_service_info("run2", ServiceType::System))
            .unwrap();
        let id3 = registry
            .register(make_service_info("run3", ServiceType::System))
            .unwrap();

        assert_eq!(registry.running_count(), 0);

        registry.update_state(id1, ServiceStateEnum::Running).unwrap();
        assert_eq!(registry.running_count(), 1);

        registry.update_state(id2, ServiceStateEnum::Running).unwrap();
        assert_eq!(registry.running_count(), 2);

        // 停止一个
        registry.update_state(id1, ServiceStateEnum::Stopped).unwrap();
        assert_eq!(registry.running_count(), 1);

        // id3 仍然是 Registered，不算运行中
        assert_eq!(registry.running_count(), 1);
    }

    // === 测试: 重复注册 ===
    #[test]
    fn test_duplicate_register() {
        let registry = ServiceRegistry::new();
        let info1 = make_service_info("dup_svc", ServiceType::System);
        let info2 = make_service_info("dup_svc", ServiceType::Agent);

        let result1 = registry.register(info1);
        assert!(result1.is_ok());

        // 同名服务应失败
        let result2 = registry.register(info2);
        assert!(result2.is_err());
    }

    // === 测试: 增加重启计数 ===
    #[test]
    fn test_increment_restart() {
        let registry = ServiceRegistry::new();
        let info = make_service_info("restart_svc", ServiceType::System);
        let id = registry.register(info).unwrap();

        assert_eq!(registry.get(id).unwrap().restart_count, 0);

        registry.increment_restart(id);
        assert_eq!(registry.get(id).unwrap().restart_count, 1);

        registry.increment_restart(id);
        registry.increment_restart(id);
        assert_eq!(registry.get(id).unwrap().restart_count, 3);

        // 对不存在的服务调用不应 panic
        registry.increment_restart(ServiceId(999));
    }

    // === 测试: 服务总数 ===
    #[test]
    fn test_total_count() {
        let registry = ServiceRegistry::new();
        assert_eq!(registry.total_count(), 0);

        registry
            .register(make_service_info("t1", ServiceType::System))
            .unwrap();
        assert_eq!(registry.total_count(), 1);

        registry
            .register(make_service_info("t2", ServiceType::Agent))
            .unwrap();
        assert_eq!(registry.total_count(), 2);

        let id = registry
            .register(make_service_info("t3", ServiceType::Driver))
            .unwrap();
        assert_eq!(registry.total_count(), 3);

        // 注销后应减少
        registry.unregister(id).unwrap();
        assert_eq!(registry.total_count(), 2);
    }
}
