//! OmniAgent 安全子系统
//!
//! 本 crate 实现了 OmniAgent OS 的安全模块，包括：
//! - 能力系统 (Capability)：基于能力的访问控制
//! - 访问控制引擎 (AccessControl)：策略驱动的访问控制
//! - 安全审计日志 (AuditLog)：安全事件的记录与查询
//! - 密码学原语 (Hash)：简化的哈希函数
//! - 安全令牌 (SecurityToken)：基于能力的授权令牌

mod error;
mod capability;
mod access_control;
mod audit;
mod crypto;
mod token;

pub use error::SecurityError;
pub use capability::{Capability, CapabilitySet};
pub use access_control::{
    AccessDecision, AccessRequest, AccessRule, AccessPolicy, AccessControlEngine, AuditEntry,
};
pub use audit::{AuditLevel, AuditEventType, SecurityAuditEntry, SecurityAuditLog, AuditFilter};
pub use crypto::Hash;
pub use token::SecurityToken;
