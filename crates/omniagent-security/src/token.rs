//! 安全令牌模块
//!
//! 安全令牌是 Agent 的授权凭证，包含一组能力和有效期信息。
//! 令牌由授权方签发，用于在 Agent 间传递访问权限。

use crate::capability::{Capability, CapabilitySet};
use crate::crypto::Hash;

/// 安全令牌
///
/// 代表一个 Agent 的授权凭证，包含其能力集和有效期。
#[derive(Debug, Clone)]
pub struct SecurityToken {
    /// 令牌唯一标识
    pub token_id: String,
    /// 令牌持有的能力集
    pub capabilities: CapabilitySet,
    /// 关联的 Agent ID
    pub agent_id: String,
    /// 签发时间（时间戳）
    pub issued_at: u64,
    /// 过期时间（时间戳）
    pub expires_at: u64,
    /// 签发者标识
    pub issuer: String,
}

impl SecurityToken {
    /// 创建一个新的安全令牌
    ///
    /// # 参数
    /// - `agent_id`: 关联的 Agent ID
    /// - `capabilities`: 令牌持有的能力集
    /// - `ttl_ms`: 令牌有效期（毫秒）
    ///
    /// # 说明
    /// 令牌 ID 通过对 Agent ID 和签发时间进行哈希生成。
    /// 当前时间戳使用 0 作为默认值（简化版）。
    pub fn new(agent_id: &str, capabilities: CapabilitySet, ttl_ms: u64) -> Self {
        let current_time: u64 = 0; // 简化版，使用固定时间戳

        // 生成令牌 ID：对 agent_id + timestamp 进行哈希
        let mut hash_input = Vec::new();
        hash_input.extend_from_slice(agent_id.as_bytes());
        hash_input.extend_from_slice(&current_time.to_be_bytes());
        let hash = Hash::hash(&hash_input);
        let token_id = format!("{:08x}{:08x}{:08x}", hash[0] as u32, hash[4] as u32, hash[8] as u32);

        SecurityToken {
            token_id,
            capabilities,
            agent_id: agent_id.to_string(),
            issued_at: current_time,
            expires_at: current_time + ttl_ms,
            issuer: "system".to_string(),
        }
    }

    /// 使用指定的签发者创建安全令牌
    pub fn with_issuer(mut self, issuer: &str) -> Self {
        self.issuer = issuer.to_string();
        self
    }

    /// 使用指定的签发时间和当前时间创建安全令牌
    pub fn with_timestamps(mut self, issued_at: u64, current_time: u64) -> Self {
        self.issued_at = issued_at;
        self.expires_at = issued_at + (self.expires_at - current_time);
        self
    }

    /// 检查令牌是否已过期
    ///
    /// # 参数
    /// - `current_time`: 当前时间戳
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time >= self.expires_at
    }

    /// 检查令牌是否包含指定能力
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.has(cap)
    }

    /// 检查令牌是否包含所有指定能力
    pub fn has_all_capabilities(&self, caps: &CapabilitySet) -> bool {
        self.capabilities.contains_all(caps)
    }

    /// 获取令牌的剩余有效时间
    ///
    /// # 参数
    /// - `current_time`: 当前时间戳
    ///
    /// # 返回
    /// 剩余有效时间（毫秒），如果已过期则返回 0。
    pub fn remaining_ttl(&self, current_time: u64) -> u64 {
        if current_time >= self.expires_at {
            0
        } else {
            self.expires_at - current_time
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_new() {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Network);
        caps.add(Capability::Filesystem);

        let token = SecurityToken::new("agent-1", caps.clone(), 60000);

        assert_eq!(token.agent_id, "agent-1");
        assert_eq!(token.issuer, "system");
        assert!(!token.token_id.is_empty());
        assert!(token.has_capability(Capability::Network));
        assert!(token.has_capability(Capability::Filesystem));
        assert!(!token.has_capability(Capability::Admin));
    }

    #[test]
    fn test_token_not_expired() {
        let caps = CapabilitySet::new();
        let token = SecurityToken::new("agent-1", caps, 60000);

        // 当前时间为 0，过期时间为 60000
        assert!(!token.is_expired(0));
        assert!(!token.is_expired(30000));
        assert!(!token.is_expired(59999));
    }

    #[test]
    fn test_token_expired() {
        let caps = CapabilitySet::new();
        let token = SecurityToken::new("agent-1", caps, 60000);

        assert!(token.is_expired(60000));
        assert!(token.is_expired(100000));
    }

    #[test]
    fn test_token_has_capability() {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Network);
        caps.add(Capability::Admin);

        let token = SecurityToken::new("agent-1", caps, 60000);

        assert!(token.has_capability(Capability::Network));
        assert!(token.has_capability(Capability::Admin));
        assert!(!token.has_capability(Capability::CameraAccess));
    }

    #[test]
    fn test_token_has_all_capabilities() {
        let mut full_caps = CapabilitySet::new();
        full_caps.add(Capability::Network);
        full_caps.add(Capability::Filesystem);
        full_caps.add(Capability::Ipc);

        let mut partial_caps = CapabilitySet::new();
        partial_caps.add(Capability::Network);
        partial_caps.add(Capability::Filesystem);

        let token = SecurityToken::new("agent-1", full_caps, 60000);

        assert!(token.has_all_capabilities(&partial_caps));
    }

    #[test]
    fn test_token_with_issuer() {
        let caps = CapabilitySet::new();
        let token = SecurityToken::new("agent-1", caps, 60000)
            .with_issuer("admin-service");

        assert_eq!(token.issuer, "admin-service");
    }

    #[test]
    fn test_token_remaining_ttl() {
        let caps = CapabilitySet::new();
        let token = SecurityToken::new("agent-1", caps, 60000);

        assert_eq!(token.remaining_ttl(0), 60000);
        assert_eq!(token.remaining_ttl(30000), 30000);
        assert_eq!(token.remaining_ttl(60000), 0);
        assert_eq!(token.remaining_ttl(100000), 0);
    }

    #[test]
    fn test_token_deterministic_id() {
        let caps = CapabilitySet::new();
        // 相同参数应该生成相同的令牌 ID
        let token1 = SecurityToken::new("agent-1", caps.clone(), 60000);
        let token2 = SecurityToken::new("agent-1", caps, 60000);
        assert_eq!(token1.token_id, token2.token_id);
    }

    #[test]
    fn test_token_different_agents_different_ids() {
        let caps = CapabilitySet::new();
        let token1 = SecurityToken::new("agent-1", caps.clone(), 60000);
        let token2 = SecurityToken::new("agent-2", caps, 60000);
        assert_ne!(token1.token_id, token2.token_id);
    }
}
