//! 能力令牌系统
//!
//! 扩展现有 CapBitmap 为完整的鸿蒙风格能力令牌系统，
//! 支持令牌的颁发、撤销、委托和权限检查。

pub mod error;
pub mod permission;
pub mod token;

pub use error::CapabilityError;
pub use permission::PermissionChecker;
pub use token::{
    predefined, CapabilityEntry, CapabilityFlags, CapabilityToken, TokenManager, TOKEN_MANAGER,
};
