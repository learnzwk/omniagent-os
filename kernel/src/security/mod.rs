//! 安全子系统模块
//!
//! 提供内核安全能力管理，包括能力位图、能力枚举和能力桥接功能。

pub mod capability_bridge;
pub use capability_bridge::*;
