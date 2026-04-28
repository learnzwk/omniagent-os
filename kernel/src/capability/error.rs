//! 能力令牌系统错误类型
//!
//! 定义能力令牌管理中所有可能的错误类型。

use core::fmt;

/// 能力错误枚举
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityError {
    /// 无效的令牌
    InvalidToken(u64),
    /// 令牌已过期
    TokenExpired(u64),
    /// 权限不足
    PermissionDenied {
        /// 所需权限
        required: &'static str,
        /// 拒绝原因
        reason: &'static str,
    },
    /// 令牌不属于当前操作者
    TokenNotOwned(u64),
    /// 不允许委托
    DelegationNotAllowed {
        /// 能力名称
        capability: &'static str,
    },
    /// 无效的能力
    InvalidCapability(&'static str),
    /// 已授予
    AlreadyGranted,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapabilityError::InvalidToken(id) => write!(f, "无效的令牌: {}", id),
            CapabilityError::TokenExpired(id) => write!(f, "令牌已过期: {}", id),
            CapabilityError::PermissionDenied { required, reason } => {
                write!(f, "权限不足: 需要 '{}', 原因: {}", required, reason)
            }
            CapabilityError::TokenNotOwned(id) => write!(f, "令牌不属于当前操作者: {}", id),
            CapabilityError::DelegationNotAllowed { capability } => {
                write!(f, "不允许委托能力: {}", capability)
            }
            CapabilityError::InvalidCapability(cap) => {
                write!(f, "无效的能力: {}", cap)
            }
            CapabilityError::AlreadyGranted => write!(f, "能力已授予"),
        }
    }
}
