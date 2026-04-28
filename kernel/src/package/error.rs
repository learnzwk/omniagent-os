//! 包管理器错误类型
//!
//! 定义包管理过程中所有可能的错误类型。

use core::fmt;

/// 包管理器错误枚举
#[derive(Debug, Clone, PartialEq)]
pub enum PackageError {
    /// 包未找到
    PackageNotFound(alloc::string::String),
    /// 包已存在
    PackageAlreadyExists(alloc::string::String),
    /// 无效的版本格式
    InvalidVersion(alloc::string::String),
    /// 无效的版本要求
    InvalidVersionReq(alloc::string::String),
    /// 依赖未找到
    DependencyNotFound(alloc::string::String),
    /// 循环依赖
    CircularDependency(alloc::string::String),
    /// 版本冲突
    VersionConflict {
        package: alloc::string::String,
        required: alloc::string::String,
        found: alloc::string::String,
    },
    /// 校验和不匹配
    ChecksumMismatch {
        expected: alloc::string::String,
        actual: alloc::string::String,
    },
    /// 无效的包状态转换
    InvalidStateTransition {
        current: alloc::string::String,
        target: alloc::string::String,
    },
    /// 包已损坏
    CorruptedPackage(alloc::string::String),
    /// 存储空间不足
    InsufficientSpace {
        required: u64,
        available: u64,
    },
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::PackageNotFound(name) => write!(f, "包未找到: {}", name),
            PackageError::PackageAlreadyExists(name) => write!(f, "包已存在: {}", name),
            PackageError::InvalidVersion(ver) => write!(f, "无效的版本格式: {}", ver),
            PackageError::InvalidVersionReq(req) => write!(f, "无效的版本要求: {}", req),
            PackageError::DependencyNotFound(name) => write!(f, "依赖未找到: {}", name),
            PackageError::CircularDependency(name) => write!(f, "检测到循环依赖: {}", name),
            PackageError::VersionConflict {
                package,
                required,
                found,
            } => write!(
                f,
                "版本冲突: 包 '{}' 要求 '{}' 但找到 '{}'",
                package, required, found
            ),
            PackageError::ChecksumMismatch { expected, actual } => {
                write!(f, "校验和不匹配: 期望 '{}' 实际 '{}'", expected, actual)
            }
            PackageError::InvalidStateTransition { current, target } => {
                write!(f, "无效的状态转换: '{}' -> '{}'", current, target)
            }
            PackageError::CorruptedPackage(name) => write!(f, "包已损坏: {}", name),
            PackageError::InsufficientSpace { required, available } => {
                write!(f, "存储空间不足: 需要 {} 字节, 可用 {} 字节", required, available)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_package_not_found_display() {
        let err = PackageError::PackageNotFound(alloc::string::String::from("test-pkg"));
        assert_eq!(format!("{}", err), "包未找到: test-pkg");
    }

    #[test]
    fn test_package_already_exists_display() {
        let err = PackageError::PackageAlreadyExists(alloc::string::String::from("dup-pkg"));
        assert_eq!(format!("{}", err), "包已存在: dup-pkg");
    }

    #[test]
    fn test_invalid_version_display() {
        let err = PackageError::InvalidVersion(alloc::string::String::from("abc"));
        assert_eq!(format!("{}", err), "无效的版本格式: abc");
    }

    #[test]
    fn test_invalid_version_req_display() {
        let err = PackageError::InvalidVersionReq(alloc::string::String::from(">>1.0"));
        assert_eq!(format!("{}", err), "无效的版本要求: >>1.0");
    }

    #[test]
    fn test_dependency_not_found_display() {
        let err = PackageError::DependencyNotFound(alloc::string::String::from("missing"));
        assert_eq!(format!("{}", err), "依赖未找到: missing");
    }

    #[test]
    fn test_circular_dependency_display() {
        let err = PackageError::CircularDependency(alloc::string::String::from("A->B->A"));
        assert_eq!(format!("{}", err), "检测到循环依赖: A->B->A");
    }

    #[test]
    fn test_version_conflict_display() {
        let err = PackageError::VersionConflict {
            package: alloc::string::String::from("foo"),
            required: alloc::string::String::from("^2.0.0"),
            found: alloc::string::String::from("1.5.0"),
        };
        assert_eq!(
            format!("{}", err),
            "版本冲突: 包 'foo' 要求 '^2.0.0' 但找到 '1.5.0'"
        );
    }

    #[test]
    fn test_checksum_mismatch_display() {
        let err = PackageError::ChecksumMismatch {
            expected: alloc::string::String::from("abc123"),
            actual: alloc::string::String::from("def456"),
        };
        assert_eq!(
            format!("{}", err),
            "校验和不匹配: 期望 'abc123' 实际 'def456'"
        );
    }

    #[test]
    fn test_invalid_state_transition_display() {
        let err = PackageError::InvalidStateTransition {
            current: alloc::string::String::from("Installed"),
            target: alloc::string::String::from("Pending"),
        };
        assert_eq!(format!("{}", err), "无效的状态转换: 'Installed' -> 'Pending'");
    }

    #[test]
    fn test_corrupted_package_display() {
        let err = PackageError::CorruptedPackage(alloc::string::String::from("bad-pkg"));
        assert_eq!(format!("{}", err), "包已损坏: bad-pkg");
    }

    #[test]
    fn test_insufficient_space_display() {
        let err = PackageError::InsufficientSpace {
            required: 1024,
            available: 512,
        };
        assert_eq!(
            format!("{}", err),
            "存储空间不足: 需要 1024 字节, 可用 512 字节"
        );
    }

    #[test]
    fn test_error_clone() {
        let err = PackageError::PackageNotFound(alloc::string::String::from("clone-test"));
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));
    }

    #[test]
    fn test_error_debug() {
        let err = PackageError::CircularDependency(alloc::string::String::from("loop"));
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("CircularDependency"));
    }
}
