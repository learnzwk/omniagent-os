//! 安全错误类型

/// 安全子系统错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// 访问被拒绝
    AccessDenied {
        /// 请求者标识
        requester: String,
        /// 被访问的资源
        resource: String,
    },
    /// 无效的令牌
    InvalidToken(String),
    /// 令牌已过期
    TokenExpired,
    /// 能力不足
    InsufficientCapabilities(String),
    /// 策略未找到
    PolicyNotFound(String),
    /// 无效的策略
    InvalidPolicy(String),
    /// 审计日志已满
    AuditLogFull,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::AccessDenied { requester, resource } => {
                write!(f, "访问被拒绝: {} -> {}", requester, resource)
            }
            SecurityError::InvalidToken(msg) => write!(f, "无效令牌: {}", msg),
            SecurityError::TokenExpired => write!(f, "令牌已过期"),
            SecurityError::InsufficientCapabilities(msg) => {
                write!(f, "能力不足: {}", msg)
            }
            SecurityError::PolicyNotFound(id) => write!(f, "策略未找到: {}", id),
            SecurityError::InvalidPolicy(msg) => write!(f, "无效策略: {}", msg),
            SecurityError::AuditLogFull => write!(f, "审计日志已满"),
        }
    }
}

impl std::error::Error for SecurityError {}
