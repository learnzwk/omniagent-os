//! 原子化服务框架
//!
//! 模仿鸿蒙原子化服务思想，实现服务注册/发现/生命周期管理。
//! 提供统一的服务管理接口，支持服务的注册、注销、查询和生命周期跟踪。

pub mod error;
pub mod lifecycle;
pub mod registry;

pub use error::ServiceError;
pub use lifecycle::{LifecycleEvent, LifecycleEventRecord, LifecycleManager, LIFECYCLE_MANAGER};
pub use registry::{
    ServiceId, ServiceInfo, ServiceRegistry, ServiceStateEnum, ServiceType, SERVICE_REGISTRY,
};
