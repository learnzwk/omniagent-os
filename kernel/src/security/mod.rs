//! 安全子系统模块
//!
//! 提供内核安全能力管理，包括：
//! - 能力位图、能力枚举和能力桥接功能
//! - 地址令牌访问控制
//! - 审计链防篡改日志

pub mod capability_bridge;
pub mod access_token;
pub mod audit_chain;

pub use capability_bridge::*;
pub use access_token::*;
pub use audit_chain::*;
