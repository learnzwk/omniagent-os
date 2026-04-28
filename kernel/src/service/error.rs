//! 服务错误类型
//!
//! 定义原子化服务框架中所有可能的错误类型。

use core::fmt;

/// 服务状态（前向声明，由 registry 模块定义）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceStateRaw(pub u8);

/// 服务错误枚举
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceError {
    /// 服务未找到
    ServiceNotFound(u64),
    /// 服务已存在
    ServiceAlreadyExists(u64),
    /// 无效状态转换
    InvalidState { current: ServiceStateRaw, expected: ServiceStateRaw },
    /// 依赖服务未找到
    DependencyNotFound(alloc::string::String),
    /// 循环依赖
    CircularDependency,
    /// 操作超时
    Timeout,
    /// 权限不足
    PermissionDenied,
    /// 未初始化
    NotInitialized,
    /// 服务已在运行
    AlreadyRunning(u64),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::ServiceNotFound(id) => write!(f, "服务未找到: {}", id),
            ServiceError::ServiceAlreadyExists(id) => write!(f, "服务已存在: {}", id),
            ServiceError::InvalidState { current, expected } => {
                write!(f, "无效状态转换: 当前={:?}, 期望={:?}", current, expected)
            }
            ServiceError::DependencyNotFound(name) => {
                write!(f, "依赖服务未找到: {}", name)
            }
            ServiceError::CircularDependency => write!(f, "检测到循环依赖"),
            ServiceError::Timeout => write!(f, "操作超时"),
            ServiceError::PermissionDenied => write!(f, "权限不足"),
            ServiceError::NotInitialized => write!(f, "服务未初始化"),
            ServiceError::AlreadyRunning(id) => write!(f, "服务已在运行: {}", id),
        }
    }
}
