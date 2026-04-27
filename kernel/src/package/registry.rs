//! 包注册表
//!
//! 实现包的注册、注销、查询和状态管理功能。
//! 使用全局单例管理所有已安装的包。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use crate::package::error::PackageError;
use crate::package::manifest::PackageManifest;

// ============================================================================
// 包状态
// ============================================================================

/// 包安装状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PackageState {
    /// 已安装
    Installed = 0,
    /// 安装中
    Pending = 1,
    /// 卸载中
    Removing = 2,
    /// 错误状态
    Error = 3,
}

impl core::fmt::Display for PackageState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PackageState::Installed => write!(f, "Installed"),
            PackageState::Pending => write!(f, "Pending"),
            PackageState::Removing => write!(f, "Removing"),
            PackageState::Error => write!(f, "Error"),
        }
    }
}

// ============================================================================
// 已注册包信息
// ============================================================================

/// 已注册包的完整信息
#[derive(Debug, Clone)]
pub struct RegisteredPackage {
    /// 包清单
    pub manifest: PackageManifest,
    /// 当前状态
    pub state: PackageState,
    /// 安装时间戳
    pub install_time: u64,
    /// 注册表分配的唯一 ID
    pub registry_id: u64,
}

// ============================================================================
// 包注册表
// ============================================================================

/// 包注册表
///
/// 管理所有已注册的包，支持按名称、能力、Agent 类型查询。
pub struct PackageRegistry {
    /// 包映射表（registry_id -> RegisteredPackage）
    packages: Mutex<BTreeMap<u64, RegisteredPackage>>,
    /// 名称索引（name -> registry_id）
    name_index: Mutex<BTreeMap<String, u64>>,
    /// 能力索引（capability -> Vec<registry_id>）
    capability_index: Mutex<BTreeMap<String, Vec<u64>>>,
    /// Agent 类型索引（agent_type -> Vec<registry_id>）
    agent_type_index: Mutex<BTreeMap<String, Vec<u64>>>,
    /// 安装记录（按时间顺序）
    install_log: Mutex<Vec<String>>,
    /// 卸载记录
    uninstall_log: Mutex<Vec<String>>,
    /// 下一个可用 ID
    next_id: AtomicU64,
}

impl PackageRegistry {
    /// 创建新的包注册表
    pub fn new() -> Self {
        PackageRegistry {
            packages: Mutex::new(BTreeMap::new()),
            name_index: Mutex::new(BTreeMap::new()),
            capability_index: Mutex::new(BTreeMap::new()),
            agent_type_index: Mutex::new(BTreeMap::new()),
            install_log: Mutex::new(Vec::new()),
            uninstall_log: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 注册包
    ///
    /// 将包清单注册到注册表中，初始状态为 Pending。
    pub fn register(&self, manifest: PackageManifest) -> Result<u64, PackageError> {
        let name = manifest.id.name.clone();

        // 检查名称是否已存在
        {
            let name_idx = self.name_index.lock();
            if name_idx.contains_key(&name) {
                return Err(PackageError::PackageAlreadyExists(name));
            }
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // 注册能力索引
        {
            let mut cap_idx = self.capability_index.lock();
            for cap in &manifest.capabilities {
                cap_idx
                    .entry(cap.clone())
                    .or_insert_with(|| Vec::new())
                    .push(id);
            }
        }

        // 注册 Agent 类型索引
        {
            let mut at_idx = self.agent_type_index.lock();
            for at in &manifest.agent_types {
                at_idx
                    .entry(at.clone())
                    .or_insert_with(|| Vec::new())
                    .push(id);
            }
        }

        // 注册包
        {
            let mut packages = self.packages.lock();
            packages.insert(
                id,
                RegisteredPackage {
                    manifest,
                    state: PackageState::Pending,
                    install_time: 0,
                    registry_id: id,
                },
            );
        }

        // 更新名称索引
        {
            let mut name_idx = self.name_index.lock();
            name_idx.insert(name, id);
        }

        Ok(id)
    }

    /// 注销包
    ///
    /// 从注册表中移除指定 registry_id 的包。
    pub fn unregister(&self, registry_id: u64) -> Result<(), PackageError> {
        let (name, capabilities, agent_types) = {
            let mut packages = self.packages.lock();
            let mut pkg = packages
                .remove(&registry_id)
                .ok_or_else(|| PackageError::PackageNotFound(format!("{}", registry_id)))?;
            pkg.state = PackageState::Removing;
            (
                pkg.manifest.id.name.clone(),
                pkg.manifest.capabilities.clone(),
                pkg.manifest.agent_types.clone(),
            )
        };

        // 移除名称索引
        {
            let mut name_idx = self.name_index.lock();
            name_idx.remove(&name);
        }

        // 移除能力索引
        {
            let mut cap_idx = self.capability_index.lock();
            for cap in &capabilities {
                if let Some(ids) = cap_idx.get_mut(cap) {
                    ids.retain(|&x| x != registry_id);
                    if ids.is_empty() {
                        cap_idx.remove(cap);
                    }
                }
            }
        }

        // 移除 Agent 类型索引
        {
            let mut at_idx = self.agent_type_index.lock();
            for at in &agent_types {
                if let Some(ids) = at_idx.get_mut(at) {
                    ids.retain(|&x| x != registry_id);
                    if ids.is_empty() {
                        at_idx.remove(at);
                    }
                }
            }
        }

        // 记录卸载日志
        {
            let mut log = self.uninstall_log.lock();
            log.push(name);
        }

        Ok(())
    }

    /// 获取包信息
    pub fn get(&self, registry_id: u64) -> Option<RegisteredPackage> {
        let packages = self.packages.lock();
        packages.get(&registry_id).cloned()
    }

    /// 按名称查找包
    pub fn find_by_name(&self, name: &str) -> Option<RegisteredPackage> {
        let name_idx = self.name_index.lock();
        let id = name_idx.get(name).copied()?;
        drop(name_idx);
        self.get(id)
    }

    /// 按能力查找包
    pub fn find_by_capability(&self, cap: &str) -> Vec<RegisteredPackage> {
        let cap_idx = self.capability_index.lock();
        let ids = match cap_idx.get(cap) {
            Some(ids) => ids.clone(),
            None => return Vec::new(),
        };
        drop(cap_idx);

        let mut result = Vec::new();
        for id in ids {
            if let Some(pkg) = self.get(id) {
                result.push(pkg);
            }
        }
        result
    }

    /// 按 Agent 类型查找包
    pub fn find_by_agent_type(&self, agent_type: &str) -> Vec<RegisteredPackage> {
        let at_idx = self.agent_type_index.lock();
        let ids = match at_idx.get(agent_type) {
            Some(ids) => ids.clone(),
            None => return Vec::new(),
        };
        drop(at_idx);

        let mut result = Vec::new();
        for id in ids {
            if let Some(pkg) = self.get(id) {
                result.push(pkg);
            }
        }
        result
    }

    /// 更新包状态
    pub fn update_state(&self, registry_id: u64, state: PackageState) -> Result<(), PackageError> {
        let mut packages = self.packages.lock();
        let pkg = packages
            .get_mut(&registry_id)
            .ok_or_else(|| PackageError::PackageNotFound(format!("{}", registry_id)))?;
        pkg.state = state;
        Ok(())
    }

    /// 标记包为已安装
    pub fn mark_installed(&self, registry_id: u64) -> Result<(), PackageError> {
        let mut packages = self.packages.lock();
        let pkg = packages
            .get_mut(&registry_id)
            .ok_or_else(|| PackageError::PackageNotFound(format!("{}", registry_id)))?;
        pkg.state = PackageState::Installed;
        pkg.install_time = 1; // 简化的时间戳
        Ok(())
    }

    /// 列出所有包
    pub fn list_packages(&self) -> Vec<RegisteredPackage> {
        let packages = self.packages.lock();
        packages.values().cloned().collect()
    }

    /// 获取包总数
    pub fn total_count(&self) -> usize {
        let packages = self.packages.lock();
        packages.len()
    }

    /// 获取安装记录
    pub fn install_log(&self) -> Vec<String> {
        let log = self.install_log.lock();
        log.clone()
    }

    /// 获取卸载记录
    pub fn uninstall_log(&self) -> Vec<String> {
        let log = self.uninstall_log.lock();
        log.clone()
    }

    /// 记录安装
    pub fn log_install(&self, name: &str) {
        let mut log = self.install_log.lock();
        log.push(String::from(name));
    }

    /// 清空所有数据（用于测试）
    pub fn clear(&self) {
        self.packages.lock().clear();
        self.name_index.lock().clear();
        self.capability_index.lock().clear();
        self.agent_type_index.lock().clear();
        self.install_log.lock().clear();
        self.uninstall_log.lock().clear();
    }
}

/// 全局包注册表
pub static PACKAGE_REGISTRY: spin::Lazy<Mutex<PackageRegistry>> = spin::Lazy::new(|| {
    Mutex::new(PackageRegistry::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::{Dependency, PackageId};

    /// 创建测试用的包清单
    fn make_manifest(name: &str, version: &str) -> PackageManifest {
        let id = PackageId::new(name, version, "x86_64");
        PackageManifest {
            id,
            description: String::from("test package"),
            author: String::from("test"),
            license: String::from("MIT"),
            dependencies: Vec::new(),
            capabilities: Vec::new(),
            agent_types: Vec::new(),
            checksum: String::from("abc123"),
            size: 1024,
        }
    }

    /// 创建带能力的包清单
    fn make_manifest_with_caps(name: &str, version: &str, caps: &[&str]) -> PackageManifest {
        let mut manifest = make_manifest(name, version);
        manifest.capabilities = caps.iter().map(|c| String::from(*c)).collect();
        manifest
    }

    /// 创建带 Agent 类型的包清单
    fn make_manifest_with_agents(name: &str, version: &str, agents: &[&str]) -> PackageManifest {
        let mut manifest = make_manifest(name, version);
        manifest.agent_types = agents.iter().map(|a| String::from(*a)).collect();
        manifest
    }

    #[test]
    fn test_register_package() {
        let registry = PackageRegistry::new();
        let manifest = make_manifest("test-pkg", "1.0.0");
        let result = registry.register(manifest);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_register_duplicate_package() {
        let registry = PackageRegistry::new();
        let m1 = make_manifest("dup-pkg", "1.0.0");
        let m2 = make_manifest("dup-pkg", "2.0.0");
        assert!(registry.register(m1).is_ok());
        assert!(registry.register(m2).is_err());
    }

    #[test]
    fn test_unregister_package() {
        let registry = PackageRegistry::new();
        let manifest = make_manifest("remove-me", "1.0.0");
        let id = registry.register(manifest).unwrap();
        assert!(registry.unregister(id).is_ok());
        assert!(registry.get(id).is_none());
    }

    #[test]
    fn test_unregister_nonexistent() {
        let registry = PackageRegistry::new();
        assert!(registry.unregister(999).is_err());
    }

    #[test]
    fn test_get_package() {
        let registry = PackageRegistry::new();
        let manifest = make_manifest("get-pkg", "1.0.0");
        let id = registry.register(manifest).unwrap();
        let pkg = registry.get(id).unwrap();
        assert_eq!(pkg.manifest.id.name, "get-pkg");
        assert_eq!(pkg.state, PackageState::Pending);
    }

    #[test]
    fn test_find_by_name() {
        let registry = PackageRegistry::new();
        let manifest = make_manifest("name-pkg", "1.0.0");
        registry.register(manifest).unwrap();
        let found = registry.find_by_name("name-pkg");
        assert!(found.is_some());
        assert_eq!(found.unwrap().manifest.id.name, "name-pkg");
        assert!(registry.find_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_find_by_capability() {
        let registry = PackageRegistry::new();
        registry
            .register(make_manifest_with_caps("pkg-a", "1.0.0", &["network", "io"]))
            .unwrap();
        registry
            .register(make_manifest_with_caps("pkg-b", "1.0.0", &["network", "crypto"]))
            .unwrap();
        registry
            .register(make_manifest_with_caps("pkg-c", "1.0.0", &["storage"]))
            .unwrap();

        let net_pkgs = registry.find_by_capability("network");
        assert_eq!(net_pkgs.len(), 2);

        let storage_pkgs = registry.find_by_capability("storage");
        assert_eq!(storage_pkgs.len(), 1);

        let missing = registry.find_by_capability("nonexistent");
        assert_eq!(missing.len(), 0);
    }

    #[test]
    fn test_find_by_agent_type() {
        let registry = PackageRegistry::new();
        registry
            .register(make_manifest_with_agents("agent-a", "1.0.0", &["chat", "assistant"]))
            .unwrap();
        registry
            .register(make_manifest_with_agents("agent-b", "1.0.0", &["chat"]))
            .unwrap();

        let chat_agents = registry.find_by_agent_type("chat");
        assert_eq!(chat_agents.len(), 2);

        let assistant_agents = registry.find_by_agent_type("assistant");
        assert_eq!(assistant_agents.len(), 1);
    }

    #[test]
    fn test_update_state() {
        let registry = PackageRegistry::new();
        let manifest = make_manifest("state-pkg", "1.0.0");
        let id = registry.register(manifest).unwrap();

        assert_eq!(registry.get(id).unwrap().state, PackageState::Pending);
        registry.update_state(id, PackageState::Installed).unwrap();
        assert_eq!(registry.get(id).unwrap().state, PackageState::Installed);
        registry.update_state(id, PackageState::Error).unwrap();
        assert_eq!(registry.get(id).unwrap().state, PackageState::Error);
    }

    #[test]
    fn test_mark_installed() {
        let registry = PackageRegistry::new();
        let manifest = make_manifest("install-pkg", "1.0.0");
        let id = registry.register(manifest).unwrap();

        registry.mark_installed(id).unwrap();
        let pkg = registry.get(id).unwrap();
        assert_eq!(pkg.state, PackageState::Installed);
        assert_eq!(pkg.install_time, 1);
    }

    #[test]
    fn test_list_packages() {
        let registry = PackageRegistry::new();
        registry.register(make_manifest("pkg1", "1.0.0")).unwrap();
        registry.register(make_manifest("pkg2", "2.0.0")).unwrap();
        registry.register(make_manifest("pkg3", "3.0.0")).unwrap();

        let all = registry.list_packages();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_total_count() {
        let registry = PackageRegistry::new();
        assert_eq!(registry.total_count(), 0);
        registry.register(make_manifest("c1", "1.0.0")).unwrap();
        assert_eq!(registry.total_count(), 1);
        let id = registry.register(make_manifest("c2", "1.0.0")).unwrap();
        assert_eq!(registry.total_count(), 2);
        registry.unregister(id).unwrap();
        assert_eq!(registry.total_count(), 1);
    }

    #[test]
    fn test_install_uninstall_log() {
        let registry = PackageRegistry::new();
        registry.log_install("pkg-a");
        registry.log_install("pkg-b");

        let install_log = registry.install_log();
        assert_eq!(install_log.len(), 2);
        assert_eq!(install_log[0], "pkg-a");
        assert_eq!(install_log[1], "pkg-b");

        // 卸载也会记录
        let id = registry.register(make_manifest("log-pkg", "1.0.0")).unwrap();
        registry.unregister(id).unwrap();
        let uninstall_log = registry.uninstall_log();
        assert_eq!(uninstall_log.len(), 1);
        assert_eq!(uninstall_log[0], "log-pkg");
    }

    #[test]
    fn test_clear() {
        let registry = PackageRegistry::new();
        registry.register(make_manifest("clear-pkg", "1.0.0")).unwrap();
        registry.log_install("x");
        assert_eq!(registry.total_count(), 1);

        registry.clear();
        assert_eq!(registry.total_count(), 0);
        assert!(registry.install_log().is_empty());
    }

    #[test]
    fn test_package_state_display() {
        assert_eq!(format!("{}", PackageState::Installed), "Installed");
        assert_eq!(format!("{}", PackageState::Pending), "Pending");
        assert_eq!(format!("{}", PackageState::Removing), "Removing");
        assert_eq!(format!("{}", PackageState::Error), "Error");
    }

    #[test]
    fn test_update_state_nonexistent() {
        let registry = PackageRegistry::new();
        assert!(registry.update_state(999, PackageState::Installed).is_err());
    }
}
