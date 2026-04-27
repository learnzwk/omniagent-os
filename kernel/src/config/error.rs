//! 配置错误类型
//!
//! 定义配置管理中所有可能的错误类型。

use core::fmt;

/// 配置错误枚举
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// 键不存在
    KeyNotFound(alloc::string::String),
    /// 类型不匹配
    TypeMismatch {
        /// 期望的类型
        expected: &'static str,
        /// 实际的类型
        actual: &'static str,
    },
    /// 键已存在
    KeyAlreadyExists(alloc::string::String),
    /// 无效的键路径
    InvalidPath(alloc::string::String),
    /// 存储已满
    StoreFull,
    /// 解析错误
    ParseError(alloc::string::String),
    /// 命名空间不存在
    NamespaceNotFound(alloc::string::String),
    /// 命名空间已存在
    NamespaceAlreadyExists(alloc::string::String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::KeyNotFound(key) => write!(f, "键不存在: {}", key),
            ConfigError::TypeMismatch { expected, actual } => {
                write!(f, "类型不匹配: 期望 '{}', 实际 '{}'", expected, actual)
            }
            ConfigError::KeyAlreadyExists(key) => write!(f, "键已存在: {}", key),
            ConfigError::InvalidPath(path) => write!(f, "无效的键路径: {}", path),
            ConfigError::StoreFull => write!(f, "配置存储已满"),
            ConfigError::ParseError(msg) => write!(f, "解析错误: {}", msg),
            ConfigError::NamespaceNotFound(ns) => write!(f, "命名空间不存在: {}", ns),
            ConfigError::NamespaceAlreadyExists(ns) => write!(f, "命名空间已存在: {}", ns),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_key_not_found_display() {
        let err = ConfigError::KeyNotFound("foo.bar".to_string());
        assert_eq!(err.to_string(), "键不存在: foo.bar");
    }

    #[test]
    fn test_type_mismatch_display() {
        let err = ConfigError::TypeMismatch {
            expected: "Integer",
            actual: "String",
        };
        assert_eq!(err.to_string(), "类型不匹配: 期望 'Integer', 实际 'String'");
    }

    #[test]
    fn test_key_already_exists_display() {
        let err = ConfigError::KeyAlreadyExists("name".to_string());
        assert_eq!(err.to_string(), "键已存在: name");
    }

    #[test]
    fn test_invalid_path_display() {
        let err = ConfigError::InvalidPath("".to_string());
        assert_eq!(err.to_string(), "无效的键路径: ");
    }

    #[test]
    fn test_store_full_display() {
        let err = ConfigError::StoreFull;
        assert_eq!(err.to_string(), "配置存储已满");
    }

    #[test]
    fn test_parse_error_display() {
        let err = ConfigError::ParseError("bad syntax".to_string());
        assert_eq!(err.to_string(), "解析错误: bad syntax");
    }

    #[test]
    fn test_namespace_not_found_display() {
        let err = ConfigError::NamespaceNotFound("missing".to_string());
        assert_eq!(err.to_string(), "命名空间不存在: missing");
    }

    #[test]
    fn test_namespace_already_exists_display() {
        let err = ConfigError::NamespaceAlreadyExists("dup".to_string());
        assert_eq!(err.to_string(), "命名空间已存在: dup");
    }

    #[test]
    fn test_error_clone() {
        let err = ConfigError::StoreFull;
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_error_debug() {
        let err = ConfigError::KeyNotFound("x".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("KeyNotFound"));
    }
}
