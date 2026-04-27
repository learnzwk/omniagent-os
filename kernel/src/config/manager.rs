//! 配置管理器
//!
//! 提供全局配置管理功能，包括多命名空间配置存储、
//! 热重载支持、配置变更回调和快照/回滚。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use spin::Mutex;

use crate::config::error::ConfigError;
use crate::config::store::{ConfigCallback, ConfigStore, ConfigValue};

// ============================================================================
// ConfigManager
// ============================================================================

/// 配置管理器
///
/// 管理多个命名配置存储，提供全局配置访问接口。
pub struct ConfigManager {
    /// 命名配置存储集合
    stores: BTreeMap<String, ConfigStore>,
    /// 全局配置变更回调
    global_callbacks: Vec<ConfigCallback>,
    /// 热重载标记
    reload_pending: bool,
}

impl ConfigManager {
    /// 创建新的配置管理器
    pub fn new() -> Self {
        let mut mgr = ConfigManager {
            stores: BTreeMap::new(),
            global_callbacks: Vec::new(),
            reload_pending: false,
        };
        mgr.init_default_stores();
        mgr
    }

    /// 初始化默认配置存储
    fn init_default_stores(&mut self) {
        // system 命名空间
        let system = ConfigStore::new("system");
        self.stores.insert("system".to_string(), system);

        // network 命名空间
        let network = ConfigStore::new("network");
        self.stores.insert("network".to_string(), network);

        // security 命名空间
        let security = ConfigStore::new("security");
        self.stores.insert("security".to_string(), security);

        // agent 命名空间
        let agent = ConfigStore::new("agent");
        self.stores.insert("agent".to_string(), agent);
    }

    /// 获取指定命名空间的配置存储
    pub fn get_store(&self, namespace: &str) -> Result<&ConfigStore, ConfigError> {
        self.stores
            .get(namespace)
            .ok_or_else(|| ConfigError::NamespaceNotFound(namespace.to_string()))
    }

    /// 获取指定命名空间的配置存储（可变引用）
    pub fn get_store_mut(&mut self, namespace: &str) -> Result<&mut ConfigStore, ConfigError> {
        self.stores
            .get_mut(namespace)
            .ok_or_else(|| ConfigError::NamespaceNotFound(namespace.to_string()))
    }

    /// 创建新的命名空间
    pub fn create_namespace(&mut self, name: &str) -> Result<(), ConfigError> {
        if self.stores.contains_key(name) {
            return Err(ConfigError::NamespaceAlreadyExists(name.to_string()));
        }
        self.stores.insert(name.to_string(), ConfigStore::new(name));
        Ok(())
    }

    /// 删除命名空间
    pub fn remove_namespace(&mut self, name: &str) -> Result<ConfigStore, ConfigError> {
        self.stores
            .remove(name)
            .ok_or_else(|| ConfigError::NamespaceNotFound(name.to_string()))
    }

    /// 检查命名空间是否存在
    pub fn has_namespace(&self, name: &str) -> bool {
        self.stores.contains_key(name)
    }

    /// 获取所有命名空间名称
    pub fn namespaces(&self) -> Vec<String> {
        self.stores.keys().cloned().collect()
    }

    /// 在指定命名空间中设置配置值
    pub fn set(
        &self,
        namespace: &str,
        key: &str,
        value: ConfigValue,
    ) -> Result<(), ConfigError> {
        let store = self
            .stores
            .get(namespace)
            .ok_or_else(|| ConfigError::NamespaceNotFound(namespace.to_string()))?;
        store.set(key, value)
    }

    /// 从指定命名空间获取配置值
    pub fn get(&self, namespace: &str, key: &str) -> Result<ConfigValue, ConfigError> {
        let store = self
            .stores
            .get(namespace)
            .ok_or_else(|| ConfigError::NamespaceNotFound(namespace.to_string()))?;
        store.get(key)
    }

    /// 标记需要热重载
    pub fn mark_reload(&mut self) {
        self.reload_pending = true;
    }

    /// 检查是否需要热重载
    pub fn is_reload_pending(&self) -> bool {
        self.reload_pending
    }

    /// 执行热重载（清除 reload 标记）
    pub fn reload(&mut self) {
        self.reload_pending = false;
    }

    /// 注册全局配置变更回调
    pub fn register_callback(&mut self, callback: ConfigCallback) {
        self.global_callbacks.push(callback);
    }

    /// 获取全局回调数量
    pub fn callback_count(&self) -> usize {
        self.global_callbacks.len()
    }

    /// 创建指定命名空间的快照
    pub fn snapshot(&self, namespace: &str) -> Result<BTreeMap<String, ConfigValue>, ConfigError> {
        let store = self
            .stores
            .get(namespace)
            .ok_or_else(|| ConfigError::NamespaceNotFound(namespace.to_string()))?;
        Ok(store.snapshot())
    }

    /// 从快照恢复指定命名空间
    pub fn restore(
        &self,
        namespace: &str,
        snapshot: BTreeMap<String, ConfigValue>,
    ) -> Result<(), ConfigError> {
        let store = self
            .stores
            .get(namespace)
            .ok_or_else(|| ConfigError::NamespaceNotFound(namespace.to_string()))?;
        store.restore(snapshot);
        Ok(())
    }

    /// 获取所有命名空间的快照
    pub fn snapshot_all(&self) -> BTreeMap<String, BTreeMap<String, ConfigValue>> {
        let mut result = BTreeMap::new();
        for (name, store) in &self.stores {
            result.insert(name.clone(), store.snapshot());
        }
        result
    }

    /// 初始化默认配置值
    pub fn init_defaults(&self) {
        // system 默认配置
        if let Some(store) = self.stores.get("system") {
            let _ = store.set("kernel_version", ConfigValue::String("0.2.0".to_string()));
            let _ = store.set("max_tasks", ConfigValue::Integer(256));
            let _ = store.set("debug_enabled", ConfigValue::Boolean(false));
        }

        // network 默认配置
        if let Some(store) = self.stores.get("network") {
            let _ = store.set("tcp.max_connections", ConfigValue::Integer(1024));
            let _ = store.set("udp.buffer_size", ConfigValue::Integer(8192));
            let _ = store.set("hostname", ConfigValue::String("omniagent".to_string()));
        }

        // security 默认配置
        if let Some(store) = self.stores.get("security") {
            let _ = store.set("audit_enabled", ConfigValue::Boolean(true));
            let _ = store.set("max_login_attempts", ConfigValue::Integer(3));
        }

        // agent 默认配置
        if let Some(store) = self.stores.get("agent") {
            let _ = store.set("max_agents", ConfigValue::Integer(64));
            let _ = store.set("default_timeout", ConfigValue::Integer(30000));
        }
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 全局配置管理器
// ============================================================================

/// 全局配置管理器实例
pub static CONFIG_MANAGER: spin::Lazy<Mutex<ConfigManager>> =
    spin::Lazy::new(|| Mutex::new(ConfigManager::new()));

/// 获取配置值的便捷函数
pub fn get_config(namespace: &str, key: &str) -> Result<ConfigValue, ConfigError> {
    CONFIG_MANAGER.lock().get(namespace, key)
}

/// 设置配置值的便捷函数
pub fn set_config(
    namespace: &str,
    key: &str,
    value: ConfigValue,
) -> Result<(), ConfigError> {
    CONFIG_MANAGER.lock().set(namespace, key, value)
}

/// 初始化默认配置的便捷函数
pub fn init_default_config() {
    CONFIG_MANAGER.lock().init_defaults();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_global_manager() {
        let mut mgr = CONFIG_MANAGER.lock();
        *mgr = ConfigManager::new();
    }

    #[test]
    fn test_config_manager_new() {
        let mgr = ConfigManager::new();
        assert!(mgr.has_namespace("system"));
        assert!(mgr.has_namespace("network"));
        assert!(mgr.has_namespace("security"));
        assert!(mgr.has_namespace("agent"));
        assert_eq!(mgr.namespaces().len(), 4);
    }

    #[test]
    fn test_get_store() {
        let mgr = ConfigManager::new();
        let store = mgr.get_store("system").unwrap();
        assert_eq!(store.name(), "system");
    }

    #[test]
    fn test_get_store_not_found() {
        let mgr = ConfigManager::new();
        let result = mgr.get_store("nonexistent");
        match result {
            Err(ConfigError::NamespaceNotFound(ns)) => assert_eq!(ns, "nonexistent"),
            _ => panic!("expected NamespaceNotFound error"),
        }
    }

    #[test]
    fn test_create_and_remove_namespace() {
        let mut mgr = ConfigManager::new();
        assert!(mgr.create_namespace("custom").is_ok());
        assert!(mgr.has_namespace("custom"));
        assert_eq!(mgr.namespaces().len(), 5);

        let result = mgr.remove_namespace("custom");
        assert!(result.is_ok());
        assert!(!mgr.has_namespace("custom"));
        assert_eq!(mgr.namespaces().len(), 4);
    }

    #[test]
    fn test_create_duplicate_namespace() {
        let mut mgr = ConfigManager::new();
        let result = mgr.create_namespace("system");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::NamespaceAlreadyExists(ns) => assert_eq!(ns, "system"),
            _ => panic!("expected NamespaceAlreadyExists"),
        }
    }

    #[test]
    fn test_set_and_get() {
        let mgr = ConfigManager::new();
        mgr.set("system", "name", ConfigValue::String("test".to_string()))
            .unwrap();
        let val = mgr.get("system", "name").unwrap();
        assert_eq!(val.as_str(), Some("test"));
    }

    #[test]
    fn test_set_missing_namespace() {
        let mgr = ConfigManager::new();
        let result = mgr.set("missing", "key", ConfigValue::Null);
        assert!(result.is_err());
    }

    #[test]
    fn test_reload_flag() {
        let mut mgr = ConfigManager::new();
        assert!(!mgr.is_reload_pending());
        mgr.mark_reload();
        assert!(mgr.is_reload_pending());
        mgr.reload();
        assert!(!mgr.is_reload_pending());
    }

    #[test]
    fn test_register_callback() {
        let mut mgr = ConfigManager::new();
        assert_eq!(mgr.callback_count(), 0);
        mgr.register_callback(|_key, _value| {});
        assert_eq!(mgr.callback_count(), 1);
    }

    #[test]
    fn test_snapshot_and_restore() {
        let mgr = ConfigManager::new();
        mgr.set("system", "key1", ConfigValue::Integer(100)).unwrap();

        let snapshot = mgr.snapshot("system").unwrap();
        assert_eq!(snapshot.len(), 1);

        mgr.set("system", "key2", ConfigValue::Integer(200)).unwrap();
        assert_eq!(mgr.get_store("system").unwrap().len(), 2);

        mgr.restore("system", snapshot).unwrap();
        assert_eq!(mgr.get_store("system").unwrap().len(), 1);
    }

    #[test]
    fn test_snapshot_all() {
        let mgr = ConfigManager::new();
        mgr.set("system", "a", ConfigValue::Integer(1)).unwrap();
        mgr.set("network", "b", ConfigValue::Integer(2)).unwrap();

        let all = mgr.snapshot_all();
        assert_eq!(all.len(), 4); // 4 default namespaces
        assert!(all.contains_key("system"));
        assert!(all.contains_key("network"));
    }

    #[test]
    fn test_init_defaults() {
        let mgr = ConfigManager::new();
        mgr.init_defaults();

        let version = mgr.get("system", "kernel_version").unwrap();
        assert_eq!(version.as_str(), Some("0.2.0"));

        let max_tasks = mgr.get("system", "max_tasks").unwrap();
        assert_eq!(max_tasks.as_i64(), Some(256));

        let hostname = mgr.get("network", "hostname").unwrap();
        assert_eq!(hostname.as_str(), Some("omniagent"));

        let audit = mgr.get("security", "audit_enabled").unwrap();
        assert_eq!(audit.as_bool(), Some(true));

        let max_agents = mgr.get("agent", "max_agents").unwrap();
        assert_eq!(max_agents.as_i64(), Some(64));
    }

    #[test]
    fn test_default() {
        let mgr = ConfigManager::default();
        assert!(mgr.has_namespace("system"));
    }

    #[test]
    fn test_global_convenience_functions() {
        reset_global_manager();
        init_default_config();

        let version = get_config("system", "kernel_version").unwrap();
        assert_eq!(version.as_str(), Some("0.2.0"));

        set_config("system", "custom_key", ConfigValue::Boolean(true)).unwrap();
        let val = get_config("system", "custom_key").unwrap();
        assert_eq!(val.as_bool(), Some(true));
    }
}
