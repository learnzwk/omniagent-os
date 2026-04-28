//! 配置管理服务模块
//!
//! 提供 OmniAgent OS 内核的配置管理功能，包括键值存储、
//! 嵌套路径访问、类型安全访问、配置变更通知和快照/回滚。

pub mod error;
pub mod store;
pub mod manager;

pub use store::{ConfigStore, ConfigValue};
pub use manager::ConfigManager;
