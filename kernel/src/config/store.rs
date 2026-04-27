//! 配置存储
//!
//! 提供基于 BTreeMap 的键值配置存储，支持嵌套路径访问、
//! 类型安全访问、配置变更通知和导入/导出。

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::config::error::ConfigError;

// ============================================================================
// ConfigValue
// ============================================================================

/// 配置值枚举
///
/// 支持多种数据类型的配置值。
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// 字符串值
    String(String),
    /// 整数值
    Integer(i64),
    /// 浮点数值
    Float(f64),
    /// 布尔值
    Boolean(bool),
    /// 数组值
    Array(Vec<ConfigValue>),
    /// 对象值（嵌套键值对）
    Object(BTreeMap<String, ConfigValue>),
    /// 空值
    Null,
}

impl ConfigValue {
    /// 获取值的类型名称
    pub fn type_name(&self) -> &'static str {
        match self {
            ConfigValue::String(_) => "String",
            ConfigValue::Integer(_) => "Integer",
            ConfigValue::Float(_) => "Float",
            ConfigValue::Boolean(_) => "Boolean",
            ConfigValue::Array(_) => "Array",
            ConfigValue::Object(_) => "Object",
            ConfigValue::Null => "Null",
        }
    }

    /// 尝试获取为字符串引用
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// 尝试获取为整数
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// 尝试获取为布尔值
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// 尝试获取为浮点数
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// 尝试获取为数组引用
    pub fn as_array(&self) -> Option<&Vec<ConfigValue>> {
        match self {
            ConfigValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// 尝试获取为对象引用
    pub fn as_object(&self) -> Option<&BTreeMap<String, ConfigValue>> {
        match self {
            ConfigValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// 导出为字符串表示
    pub fn to_config_string(&self, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let inner_spaces = " ".repeat(indent + 2);
        match self {
            ConfigValue::String(s) => format!("{}\"{}\"", spaces, s),
            ConfigValue::Integer(i) => format!("{}{}", spaces, i),
            ConfigValue::Float(f) => format!("{}{}", spaces, f),
            ConfigValue::Boolean(b) => format!("{}{}", spaces, b),
            ConfigValue::Null => format!("{}null", spaces),
            ConfigValue::Array(arr) => {
                if arr.is_empty() {
                    return format!("{}[]", spaces);
                }
                let mut result = format!("{}[\n", spaces);
                for item in arr {
                    result.push_str(&item.to_config_string(indent + 2));
                    result.push_str(",\n");
                }
                result.push_str(&format!("{}]", spaces));
                result
            }
            ConfigValue::Object(obj) => {
                if obj.is_empty() {
                    return format!("{}{{}}", spaces);
                }
                let mut result = format!("{}{{\n", spaces);
                for (key, value) in obj {
                    result.push_str(&format!("{}\"{}\": ", inner_spaces, key));
                    result.push_str(&value.to_config_string(0));
                    result.push_str(",\n");
                }
                result.push_str(&format!("{}}}", spaces));
                result
            }
        }
    }
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigValue::String(s) => write!(f, "{}", s),
            ConfigValue::Integer(i) => write!(f, "{}", i),
            ConfigValue::Float(fl) => write!(f, "{}", fl),
            ConfigValue::Boolean(b) => write!(f, "{}", b),
            ConfigValue::Null => write!(f, "null"),
            ConfigValue::Array(arr) => {
                write!(f, "[")?;
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            ConfigValue::Object(obj) => {
                write!(f, "{{")?;
                for (i, (key, value)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", key, value)?;
                }
                write!(f, "}}")
            }
        }
    }
}

// ============================================================================
// ConfigStore
// ============================================================================

/// 配置变更回调类型
pub type ConfigCallback = fn(&str, &ConfigValue);

#[derive(Debug)]
/// 配置存储
///
/// 基于 BTreeMap 的键值配置存储，支持嵌套路径访问。
pub struct ConfigStore {
    /// 存储名称
    name: String,
    /// 键值数据
    data: spin::Mutex<BTreeMap<String, ConfigValue>>,
    /// 变更回调
    callbacks: spin::Mutex<Vec<(String, ConfigCallback)>>,
}

impl ConfigStore {
    /// 创建新的配置存储
    pub fn new(name: &str) -> Self {
        ConfigStore {
            name: name.to_string(),
            data: spin::Mutex::new(BTreeMap::new()),
            callbacks: spin::Mutex::new(Vec::new()),
        }
    }

    /// 获取存储名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 设置一个配置值
    pub fn set(&self, key: &str, value: ConfigValue) -> Result<(), ConfigError> {
        if key.is_empty() {
            return Err(ConfigError::InvalidPath(key.to_string()));
        }
        let old_value = self.data.lock().insert(key.to_string(), value.clone());
        // 触发回调
        if old_value.is_some() || true {
            self.notify_callbacks(key, &value);
        }
        Ok(())
    }

    /// 获取一个配置值
    pub fn get(&self, key: &str) -> Result<ConfigValue, ConfigError> {
        self.data
            .lock()
            .get(key)
            .cloned()
            .ok_or_else(|| ConfigError::KeyNotFound(key.to_string()))
    }

    /// 获取字符串值
    pub fn get_string(&self, key: &str) -> Result<String, ConfigError> {
        let value = self.get(key)?;
        value
            .as_str()
            .map(|s| s.to_string())
            .ok_or(ConfigError::TypeMismatch {
                expected: "String",
                actual: value.type_name(),
            })
    }

    /// 获取整数值
    pub fn get_int(&self, key: &str) -> Result<i64, ConfigError> {
        let value = self.get(key)?;
        value
            .as_i64()
            .ok_or(ConfigError::TypeMismatch {
                expected: "Integer",
                actual: value.type_name(),
            })
    }

    /// 获取布尔值
    pub fn get_bool(&self, key: &str) -> Result<bool, ConfigError> {
        let value = self.get(key)?;
        value
            .as_bool()
            .ok_or(ConfigError::TypeMismatch {
                expected: "Boolean",
                actual: value.type_name(),
            })
    }

    /// 获取浮点数值
    pub fn get_float(&self, key: &str) -> Result<f64, ConfigError> {
        let value = self.get(key)?;
        value
            .as_f64()
            .ok_or(ConfigError::TypeMismatch {
                expected: "Float",
                actual: value.type_name(),
            })
    }

    /// 通过嵌套路径获取配置值
    ///
    /// 例如 "network.tcp.max_connections" 会依次查找 network -> tcp -> max_connections
    pub fn get_nested(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        if path.is_empty() {
            return Err(ConfigError::InvalidPath(path.to_string()));
        }
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Err(ConfigError::InvalidPath(path.to_string()));
        }

        // 先尝试直接查找完整路径
        if let Some(value) = self.data.lock().get(path) {
            return Ok(value.clone());
        }

        // 然后尝试嵌套查找
        let data = self.data.lock();
        let mut current: Option<&ConfigValue> = None;
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                current = data.get(*part);
            } else if let Some(ref val) = current {
                if let Some(obj) = val.as_object() {
                    current = obj.get(*part);
                } else {
                    return Err(ConfigError::InvalidPath(format!(
                        "路径 '{}' 处不是对象类型",
                        &path[..path.find(*part).unwrap_or(0)]
                    )));
                }
            } else {
                return Err(ConfigError::KeyNotFound(path.to_string()));
            }
        }
        current
            .cloned()
            .ok_or_else(|| ConfigError::KeyNotFound(path.to_string()))
    }

    /// 删除一个配置值
    pub fn delete(&self, key: &str) -> Result<ConfigValue, ConfigError> {
        self.data
            .lock()
            .remove(key)
            .ok_or_else(|| ConfigError::KeyNotFound(key.to_string()))
    }

    /// 检查键是否存在
    pub fn exists(&self, key: &str) -> bool {
        self.data.lock().contains_key(key)
    }

    /// 获取所有键
    pub fn keys(&self) -> Vec<String> {
        self.data.lock().keys().cloned().collect()
    }

    /// 获取配置项数量
    pub fn len(&self) -> usize {
        self.data.lock().len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.data.lock().is_empty()
    }

    /// 清空所有配置
    pub fn clear(&self) {
        self.data.lock().clear();
    }

    /// 注册配置变更回调
    pub fn watch(&self, key_pattern: &str, callback: ConfigCallback) {
        self.callbacks
            .lock()
            .push((key_pattern.to_string(), callback));
    }

    /// 移除指定 key_pattern 的所有回调
    pub fn unwatch(&self, key_pattern: &str) {
        self.callbacks
            .lock()
            .retain(|(pattern, _)| pattern != key_pattern);
    }

    /// 通知回调
    fn notify_callbacks(&self, key: &str, value: &ConfigValue) {
        let callbacks = self.callbacks.lock();
        for (pattern, callback) in callbacks.iter() {
            if key.starts_with(pattern) || pattern == "*" {
                callback(key, value);
            }
        }
    }

    /// 导出为字符串表示
    pub fn export(&self) -> String {
        let data = self.data.lock();
        let mut result = format!("// ConfigStore: {}\n", self.name);
        for (key, value) in data.iter() {
            result.push_str(&format!("{} = {}\n", key, value));
        }
        result
    }

    /// 创建快照
    pub fn snapshot(&self) -> BTreeMap<String, ConfigValue> {
        self.data.lock().clone()
    }

    /// 从快照恢复
    pub fn restore(&self, snapshot: BTreeMap<String, ConfigValue>) {
        *self.data.lock() = snapshot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ConfigValue tests ----

    #[test]
    fn test_config_value_type_name() {
        assert_eq!(ConfigValue::String("s".to_string()).type_name(), "String");
        assert_eq!(ConfigValue::Integer(42).type_name(), "Integer");
        assert_eq!(ConfigValue::Float(3.14).type_name(), "Float");
        assert_eq!(ConfigValue::Boolean(true).type_name(), "Boolean");
        assert_eq!(ConfigValue::Array(Vec::new()).type_name(), "Array");
        assert_eq!(ConfigValue::Object(BTreeMap::new()).type_name(), "Object");
        assert_eq!(ConfigValue::Null.type_name(), "Null");
    }

    #[test]
    fn test_config_value_accessors() {
        let s = ConfigValue::String("hello".to_string());
        assert_eq!(s.as_str(), Some("hello"));
        assert!(s.as_i64().is_none());

        let i = ConfigValue::Integer(-42);
        assert_eq!(i.as_i64(), Some(-42));
        assert!(i.as_str().is_none());

        let b = ConfigValue::Boolean(true);
        assert_eq!(b.as_bool(), Some(true));

        let f = ConfigValue::Float(2.5);
        assert_eq!(f.as_f64(), Some(2.5));

        let arr = ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        assert_eq!(arr.as_array().unwrap().len(), 2);

        let mut obj = BTreeMap::new();
        obj.insert("key".to_string(), ConfigValue::Null);
        let o = ConfigValue::Object(obj);
        assert!(o.as_object().unwrap().contains_key("key"));
    }

    #[test]
    fn test_config_value_display() {
        assert_eq!(format!("{}", ConfigValue::String("abc".to_string())), "abc");
        assert_eq!(format!("{}", ConfigValue::Integer(42)), "42");
        assert_eq!(format!("{}", ConfigValue::Boolean(true)), "true");
        assert_eq!(format!("{}", ConfigValue::Null), "null");
    }

    #[test]
    fn test_config_value_equality() {
        assert_eq!(ConfigValue::Integer(1), ConfigValue::Integer(1));
        assert_ne!(ConfigValue::Integer(1), ConfigValue::Integer(2));
        assert_eq!(
            ConfigValue::String("a".to_string()),
            ConfigValue::String("a".to_string())
        );
    }

    #[test]
    fn test_config_value_to_config_string() {
        let val = ConfigValue::Integer(42);
        let s = val.to_config_string(0);
        assert!(s.contains("42"));

        let val = ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        let s = val.to_config_string(0);
        assert!(s.contains("["));
        assert!(s.contains("]"));
    }

    // ---- ConfigStore tests ----

    #[test]
    fn test_store_new() {
        let store = ConfigStore::new("test");
        assert_eq!(store.name(), "test");
        assert!(store.is_empty());
    }

    #[test]
    fn test_store_set_and_get() {
        let store = ConfigStore::new("test");
        store.set("name", ConfigValue::String("omniagent".to_string())).unwrap();
        let val = store.get("name").unwrap();
        assert_eq!(val.as_str(), Some("omniagent"));
    }

    #[test]
    fn test_store_get_missing_key() {
        let store = ConfigStore::new("test");
        let result = store.get("missing");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::KeyNotFound(key) => assert_eq!(key, "missing"),
            _ => panic!("expected KeyNotFound"),
        }
    }

    #[test]
    fn test_store_type_safe_accessors() {
        let store = ConfigStore::new("test");
        store.set("port", ConfigValue::Integer(8080)).unwrap();
        store.set("debug", ConfigValue::Boolean(true)).unwrap();
        store.set("rate", ConfigValue::Float(1.5)).unwrap();
        store.set("host", ConfigValue::String("localhost".to_string())).unwrap();

        assert_eq!(store.get_int("port").unwrap(), 8080);
        assert_eq!(store.get_bool("debug").unwrap(), true);
        assert_eq!(store.get_float("rate").unwrap(), 1.5);
        assert_eq!(store.get_string("host").unwrap(), "localhost");
    }

    #[test]
    fn test_store_type_mismatch() {
        let store = ConfigStore::new("test");
        store.set("name", ConfigValue::String("test".to_string())).unwrap();
        let result = store.get_int("name");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::TypeMismatch { expected, actual } => {
                assert_eq!(expected, "Integer");
                assert_eq!(actual, "String");
            }
            _ => panic!("expected TypeMismatch"),
        }
    }

    #[test]
    fn test_store_delete() {
        let store = ConfigStore::new("test");
        store.set("key", ConfigValue::Integer(1)).unwrap();
        assert!(store.exists("key"));
        let deleted = store.delete("key").unwrap();
        assert_eq!(deleted, ConfigValue::Integer(1));
        assert!(!store.exists("key"));
    }

    #[test]
    fn test_store_exists() {
        let store = ConfigStore::new("test");
        assert!(!store.exists("key"));
        store.set("key", ConfigValue::Null).unwrap();
        assert!(store.exists("key"));
    }

    #[test]
    fn test_store_keys_and_len() {
        let store = ConfigStore::new("test");
        store.set("a", ConfigValue::Integer(1)).unwrap();
        store.set("b", ConfigValue::Integer(2)).unwrap();
        assert_eq!(store.len(), 2);
        let mut keys = store.keys();
        keys.sort();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn test_store_clear() {
        let store = ConfigStore::new("test");
        store.set("a", ConfigValue::Integer(1)).unwrap();
        store.set("b", ConfigValue::Integer(2)).unwrap();
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_store_set_empty_key() {
        let store = ConfigStore::new("test");
        let result = store.set("", ConfigValue::Null);
        assert!(result.is_err());
    }

    #[test]
    fn test_store_get_nested() {
        let store = ConfigStore::new("test");
        // 直接设置嵌套路径
        store.set("network.tcp.max_connections", ConfigValue::Integer(100)).unwrap();
        let val = store.get("network.tcp.max_connections").unwrap();
        assert_eq!(val.as_i64(), Some(100));
    }

    #[test]
    fn test_store_nested_object_access() {
        let store = ConfigStore::new("test");
        let mut tcp_obj = BTreeMap::new();
        tcp_obj.insert("max_connections".to_string(), ConfigValue::Integer(100));
        let mut network_obj = BTreeMap::new();
        network_obj.insert("tcp".to_string(), ConfigValue::Object(tcp_obj));
        store.set("network", ConfigValue::Object(network_obj)).unwrap();

        let val = store.get_nested("network.tcp.max_connections").unwrap();
        assert_eq!(val.as_i64(), Some(100));
    }

    #[test]
    fn test_store_watch_callback() {
        let store = ConfigStore::new("test");
        static mut CALLBACK_CALLED: bool = false;

        fn on_change(_key: &str, _value: &ConfigValue) {
            unsafe { CALLBACK_CALLED = true; }
        }

        store.watch("*", on_change);
        store.set("key", ConfigValue::Integer(42)).unwrap();
        unsafe {
            assert!(CALLBACK_CALLED);
            CALLBACK_CALLED = false;
        }
    }

    #[test]
    fn test_store_unwatch() {
        let store = ConfigStore::new("test");
        static mut CALLED: bool = false;

        fn cb(_key: &str, _value: &ConfigValue) {
            unsafe { CALLED = true; }
        }

        store.watch("prefix", cb);
        store.unwatch("prefix");
        store.set("prefix.key", ConfigValue::Integer(1)).unwrap();
        unsafe {
            assert!(!CALLED);
        }
    }

    #[test]
    fn test_store_export() {
        let store = ConfigStore::new("test");
        store.set("version", ConfigValue::Integer(1)).unwrap();
        let exported = store.export();
        assert!(exported.contains("ConfigStore: test"));
        assert!(exported.contains("version"));
    }

    #[test]
    fn test_store_snapshot_and_restore() {
        let store = ConfigStore::new("test");
        store.set("a", ConfigValue::Integer(1)).unwrap();
        store.set("b", ConfigValue::String("hello".to_string())).unwrap();

        let snapshot = store.snapshot();
        store.set("c", ConfigValue::Boolean(true)).unwrap();
        assert_eq!(store.len(), 3);

        store.restore(snapshot);
        assert_eq!(store.len(), 2);
        assert!(store.exists("a"));
        assert!(store.exists("b"));
        assert!(!store.exists("c"));
    }
}
