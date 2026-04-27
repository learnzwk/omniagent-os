//! 服务管理器模块
//!
//! 提供用户态服务启动、监控、重启的完整管理框架。
//! 包含服务管理器和健康监控两个子模块。

pub mod manager;
pub mod monitor;

pub use manager::{SvcManagerError, ServiceConfig, ServiceSnapshot, ServiceManager, SVC_MANAGER};
pub use monitor::{HealthStatus, HealthCheckConfig, HealthMonitor, HEALTH_MONITOR};
