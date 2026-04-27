//! 能力令牌
//!
//! 实现鸿蒙风格的能力令牌系统，支持令牌的颁发、撤销、委托。
//! 令牌具有不可伪造、可委托、可撤销的特性。

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bitflags::bitflags;
use spin::Mutex;

use crate::capability::error::CapabilityError;

// ============================================================================
// 能力标志位
// ============================================================================

bitflags! {
    /// 能力标志位
    ///
    /// 控制能力的授予、委托、撤销等行为。
    pub struct CapabilityFlags: u32 {
        /// 可授予他人
        const GRANTABLE   = 1 << 0;
        /// 可委托
        const DELEGATABLE = 1 << 1;
        /// 可被撤销
        const REVOCABLE   = 1 << 2;
        /// 有时限
        const TIMED       = 1 << 3;
        /// 持久化
        const PERSISTENT  = 1 << 4;
    }
}

// ============================================================================
// 能力条目
// ============================================================================

/// 能力条目
///
/// 描述单个能力的信息，包括能力 ID、名称和标志位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityEntry {
    /// 能力 ID
    pub capability_id: u32,
    /// 能力名称（静态字符串）
    pub name: &'static str,
    /// 能力标志位
    pub flags: CapabilityFlags,
}

// ============================================================================
// 能力令牌
// ============================================================================

/// 能力令牌
///
/// 鸿蒙风格的令牌系统：不可伪造、可委托、可撤销。
/// 每个令牌关联一个拥有者和一组能力。
#[derive(Debug)]
pub struct CapabilityToken {
    /// 令牌 ID
    pub id: u64,
    /// 拥有者 Agent/任务 ID
    pub owner: u64,
    /// 能力列表
    pub capabilities: Vec<CapabilityEntry>,
    /// 颁发者
    pub issuer: u64,
    /// 颁发时间
    pub issued_at: u64,
    /// 过期时间（0 = 永不过期）
    pub expires_at: u64,
    /// 已委托次数
    pub delegate_count: AtomicU32,
    /// 最大委托次数
    pub max_delegates: u32,
    /// 是否已撤销
    pub is_revoked: AtomicBool,
}

impl Clone for CapabilityToken {
    fn clone(&self) -> Self {
        CapabilityToken {
            id: self.id,
            owner: self.owner,
            capabilities: self.capabilities.clone(),
            issuer: self.issuer,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            delegate_count: AtomicU32::new(self.delegate_count.load(Ordering::SeqCst)),
            max_delegates: self.max_delegates,
            is_revoked: AtomicBool::new(self.is_revoked.load(Ordering::SeqCst)),
        }
    }
}

// ============================================================================
// 预定义能力
// ============================================================================

/// 预定义能力常量
pub mod predefined {
    use super::*;

    /// 读取文件
    pub const READ_FILES: CapabilityEntry = CapabilityEntry {
        capability_id: 0,
        name: "read_files",
        flags: CapabilityFlags::from_bits_truncate(1 << 0 | 1 << 1), // GRANTABLE | DELEGATABLE
    };

    /// 写入文件
    pub const WRITE_FILES: CapabilityEntry = CapabilityEntry {
        capability_id: 1,
        name: "write_files",
        flags: CapabilityFlags::from_bits_truncate(1 << 0), // GRANTABLE
    };

    /// 执行程序
    pub const EXECUTE: CapabilityEntry = CapabilityEntry {
        capability_id: 2,
        name: "execute",
        flags: CapabilityFlags::from_bits_truncate(1 << 0), // GRANTABLE
    };

    /// 网络访问
    pub const NETWORK: CapabilityEntry = CapabilityEntry {
        capability_id: 3,
        name: "network",
        flags: CapabilityFlags::from_bits_truncate(1 << 0 | 1 << 1), // GRANTABLE | DELEGATABLE
    };

    /// 创建 Agent
    pub const CREATE_AGENT: CapabilityEntry = CapabilityEntry {
        capability_id: 4,
        name: "create_agent",
        flags: CapabilityFlags::empty(),
    };

    /// 终止 Agent
    pub const KILL_AGENT: CapabilityEntry = CapabilityEntry {
        capability_id: 5,
        name: "kill_agent",
        flags: CapabilityFlags::empty(),
    };

    /// 发送消息
    pub const SEND_MESSAGE: CapabilityEntry = CapabilityEntry {
        capability_id: 6,
        name: "send_message",
        flags: CapabilityFlags::from_bits_truncate(1 << 0 | 1 << 1), // GRANTABLE | DELEGATABLE
    };

    /// 接收消息
    pub const RECEIVE_MESSAGE: CapabilityEntry = CapabilityEntry {
        capability_id: 7,
        name: "receive_message",
        flags: CapabilityFlags::from_bits_truncate(1 << 0 | 1 << 1), // GRANTABLE | DELEGATABLE
    };

    /// 管理系统
    pub const MANAGE_SYSTEM: CapabilityEntry = CapabilityEntry {
        capability_id: 8,
        name: "manage_system",
        flags: CapabilityFlags::empty(),
    };

    /// 访问硬件
    pub const ACCESS_HARDWARE: CapabilityEntry = CapabilityEntry {
        capability_id: 9,
        name: "access_hardware",
        flags: CapabilityFlags::empty(),
    };

    /// 管理安全
    pub const MANAGE_SECURITY: CapabilityEntry = CapabilityEntry {
        capability_id: 10,
        name: "manage_security",
        flags: CapabilityFlags::empty(),
    };

    /// 获取所有预定义能力
    pub fn all() -> Vec<CapabilityEntry> {
        alloc::vec![
            READ_FILES,
            WRITE_FILES,
            EXECUTE,
            NETWORK,
            CREATE_AGENT,
            KILL_AGENT,
            SEND_MESSAGE,
            RECEIVE_MESSAGE,
            MANAGE_SYSTEM,
            ACCESS_HARDWARE,
            MANAGE_SECURITY,
        ]
    }
}

// ============================================================================
// 令牌管理器
// ============================================================================

/// 令牌管理器
///
/// 管理所有能力令牌的颁发、撤销、委托和查询。
pub struct TokenManager {
    /// 令牌映射表（ID -> 令牌）
    tokens: Mutex<BTreeMap<u64, CapabilityToken>>,
    /// 下一个可用令牌 ID
    next_id: AtomicU64,
}

impl TokenManager {
    /// 创建新的令牌管理器
    pub fn new() -> Self {
        TokenManager {
            tokens: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 颁发令牌
    ///
    /// 为指定拥有者颁发包含给定能力列表的令牌。
    ///
    /// # 参数
    /// - `owner`: 拥有者 Agent/任务 ID
    /// - `capabilities`: 能力列表
    /// - `issuer`: 颁发者 ID
    /// - `ttl`: 生存时间（0 = 永不过期）
    ///
    /// # 返回
    /// 新颁发的令牌 ID
    pub fn issue_token(
        &self,
        owner: u64,
        capabilities: Vec<CapabilityEntry>,
        issuer: u64,
        ttl: u64,
    ) -> Result<u64, CapabilityError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let now = 0u64; // 在实际内核中会使用系统时钟
        let expires_at = if ttl == 0 { 0 } else { now + ttl };

        let token = CapabilityToken {
            id,
            owner,
            capabilities,
            issuer,
            issued_at: now,
            expires_at,
            delegate_count: AtomicU32::new(0),
            max_delegates: 10,
            is_revoked: AtomicBool::new(false),
        };

        let mut tokens = self.tokens.lock();
        tokens.insert(id, token);

        Ok(id)
    }

    /// 撤销令牌
    ///
    /// 撤销指定 ID 的令牌，使其失效。
    pub fn revoke_token(&self, token_id: u64) -> Result<(), CapabilityError> {
        let tokens = self.tokens.lock();
        let token = tokens
            .get(&token_id)
            .ok_or(CapabilityError::InvalidToken(token_id))?;
        token.is_revoked.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// 获取令牌信息
    ///
    /// 返回指定 ID 令牌的副本。
    pub fn get_token(&self, token_id: u64) -> Option<CapabilityToken> {
        let tokens = self.tokens.lock();
        tokens.get(&token_id).cloned()
    }

    /// 委托令牌
    ///
    /// 将指定令牌的部分能力委托给另一个实体。
    ///
    /// # 参数
    /// - `token_id`: 源令牌 ID
    /// - `to`: 被委托者 ID
    /// - `capabilities`: 要委托的能力列表
    ///
    /// # 返回
    /// 新创建的委托令牌 ID
    pub fn delegate(
        &self,
        token_id: u64,
        to: u64,
        capabilities: Vec<CapabilityEntry>,
    ) -> Result<u64, CapabilityError> {
        let mut tokens = self.tokens.lock();
        let source = tokens
            .get(&token_id)
            .ok_or(CapabilityError::InvalidToken(token_id))?;

        // 检查源令牌是否已撤销
        if source.is_revoked.load(Ordering::SeqCst) {
            return Err(CapabilityError::InvalidToken(token_id));
        }

        // 检查委托次数限制
        let current_delegates = source.delegate_count.load(Ordering::SeqCst);
        if current_delegates >= source.max_delegates {
            return Err(CapabilityError::DelegationNotAllowed {
                capability: "已达到最大委托次数",
            });
        }

        // 检查每个能力是否可委托
        for cap in &capabilities {
            if !cap.flags.contains(CapabilityFlags::DELEGATABLE) {
                return Err(CapabilityError::DelegationNotAllowed {
                    capability: cap.name,
                });
            }
        }

        // 增加委托计数
        source.delegate_count.fetch_add(1, Ordering::SeqCst);

        // 获取源令牌信息
        let source_owner = source.owner;
        let source_expires = source.expires_at;
        let source_max = source.max_delegates;

        // 创建新的委托令牌
        let new_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let new_token = CapabilityToken {
            id: new_id,
            owner: to,
            capabilities,
            issuer: source_owner,
            issued_at: 0,
            expires_at: source_expires,
            delegate_count: AtomicU32::new(0),
            max_delegates: source_max.saturating_sub(1),
            is_revoked: AtomicBool::new(false),
        };

        tokens.insert(new_id, new_token);
        Ok(new_id)
    }

    /// 检查权限
    ///
    /// 检查指定令牌是否拥有某个能力。
    ///
    /// # 参数
    /// - `token_id`: 令牌 ID
    /// - `capability`: 能力名称
    ///
    /// # 返回
    /// 如果令牌有效且拥有该能力返回 true
    pub fn check_permission(&self, token_id: u64, capability: &str) -> Result<bool, CapabilityError> {
        let tokens = self.tokens.lock();
        let token = tokens
            .get(&token_id)
            .ok_or(CapabilityError::InvalidToken(token_id))?;

        // 检查是否已撤销
        if token.is_revoked.load(Ordering::SeqCst) {
            return Err(CapabilityError::InvalidToken(token_id));
        }

        // 检查能力
        let has = token.capabilities.iter().any(|c| c.name == capability);
        Ok(has)
    }

    /// 列出指定拥有者的所有令牌
    pub fn list_tokens(&self, owner: u64) -> Vec<CapabilityToken> {
        let tokens = self.tokens.lock();
        tokens
            .values()
            .filter(|t| t.owner == owner)
            .cloned()
            .collect()
    }

    /// 撤销指定拥有者的所有令牌
    pub fn revoke_all(&self, owner: u64) {
        let tokens = self.tokens.lock();
        for token in tokens.values() {
            if token.owner == owner {
                token.is_revoked.store(true, Ordering::SeqCst);
            }
        }
    }

    /// 获取令牌总数
    pub fn token_count(&self) -> usize {
        let tokens = self.tokens.lock();
        tokens.len()
    }

    /// 检查令牌是否有效（未撤销）
    pub fn is_valid(&self, token_id: u64) -> bool {
        let tokens = self.tokens.lock();
        if let Some(token) = tokens.get(&token_id) {
            !token.is_revoked.load(Ordering::SeqCst)
        } else {
            false
        }
    }
}

/// 全局令牌管理器
pub static TOKEN_MANAGER: spin::Lazy<Mutex<TokenManager>> = spin::Lazy::new(|| {
    Mutex::new(TokenManager::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === 测试: 颁发令牌 ===
    #[test]
    fn test_issue_token() {
        let manager = TokenManager::new();
        let caps = alloc::vec![predefined::READ_FILES, predefined::WRITE_FILES];
        let result = manager.issue_token(1, caps, 0, 0);
        assert!(result.is_ok());
        let token_id = result.unwrap();
        assert_eq!(token_id, 1);

        // 验证令牌信息
        let token = manager.get_token(token_id).unwrap();
        assert_eq!(token.owner, 1);
        assert_eq!(token.issuer, 0);
        assert_eq!(token.capabilities.len(), 2);
        assert!(!token.is_revoked.load(Ordering::SeqCst));
    }

    // === 测试: 撤销令牌 ===
    #[test]
    fn test_revoke_token() {
        let manager = TokenManager::new();
        let caps = alloc::vec![predefined::READ_FILES];
        let token_id = manager.issue_token(1, caps, 0, 0).unwrap();

        assert!(manager.revoke_token(token_id).is_ok());
        assert!(!manager.is_valid(token_id));

        // 撤销不存在的令牌
        assert!(manager.revoke_token(999).is_err());
    }

    // === 测试: 获取令牌 ===
    #[test]
    fn test_get_token() {
        let manager = TokenManager::new();
        let caps = alloc::vec![predefined::NETWORK];
        let token_id = manager.issue_token(1, caps, 0, 0).unwrap();

        let token = manager.get_token(token_id);
        assert!(token.is_some());
        assert_eq!(token.unwrap().owner, 1);

        // 不存在的令牌
        assert!(manager.get_token(999).is_none());
    }

    // === 测试: 委托令牌 ===
    #[test]
    fn test_delegate() {
        let manager = TokenManager::new();
        // 使用可委托的能力
        let caps = alloc::vec![predefined::READ_FILES, predefined::NETWORK];
        let token_id = manager.issue_token(1, caps, 0, 0).unwrap();

        // 委托给另一个实体
        let delegate_caps = alloc::vec![predefined::READ_FILES];
        let result = manager.delegate(token_id, 2, delegate_caps);
        assert!(result.is_ok());
        let new_token_id = result.unwrap();
        assert_eq!(new_token_id, 2);

        // 验证新令牌
        let new_token = manager.get_token(new_token_id).unwrap();
        assert_eq!(new_token.owner, 2);
        assert_eq!(new_token.issuer, 1);
        assert_eq!(new_token.capabilities.len(), 1);
        assert_eq!(new_token.capabilities[0].name, "read_files");
    }

    // === 测试: 检查权限 ===
    #[test]
    fn test_check_permission() {
        let manager = TokenManager::new();
        let caps = alloc::vec![predefined::READ_FILES, predefined::EXECUTE];
        let token_id = manager.issue_token(1, caps, 0, 0).unwrap();

        assert!(manager.check_permission(token_id, "read_files").unwrap());
        assert!(manager.check_permission(token_id, "execute").unwrap());
        assert!(!manager.check_permission(token_id, "network").unwrap());

        // 不存在的令牌
        assert!(manager.check_permission(999, "read_files").is_err());
    }

    // === 测试: 列出令牌 ===
    #[test]
    fn test_list_tokens() {
        let manager = TokenManager::new();
        manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 0)
            .unwrap();
        manager
            .issue_token(1, alloc::vec![predefined::WRITE_FILES], 0, 0)
            .unwrap();
        manager
            .issue_token(2, alloc::vec![predefined::NETWORK], 0, 0)
            .unwrap();

        let owner1_tokens = manager.list_tokens(1);
        assert_eq!(owner1_tokens.len(), 2);

        let owner2_tokens = manager.list_tokens(2);
        assert_eq!(owner2_tokens.len(), 1);

        let owner3_tokens = manager.list_tokens(3);
        assert_eq!(owner3_tokens.len(), 0);
    }

    // === 测试: 撤销所有令牌 ===
    #[test]
    fn test_revoke_all() {
        let manager = TokenManager::new();
        let id1 = manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 0)
            .unwrap();
        let id2 = manager
            .issue_token(1, alloc::vec![predefined::WRITE_FILES], 0, 0)
            .unwrap();
        let id3 = manager
            .issue_token(2, alloc::vec![predefined::NETWORK], 0, 0)
            .unwrap();

        manager.revoke_all(1);

        assert!(!manager.is_valid(id1));
        assert!(!manager.is_valid(id2));
        // owner 2 的令牌不受影响
        assert!(manager.is_valid(id3));
    }

    // === 测试: 令牌计数 ===
    #[test]
    fn test_token_count() {
        let manager = TokenManager::new();
        assert_eq!(manager.token_count(), 0);

        manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 0)
            .unwrap();
        assert_eq!(manager.token_count(), 1);

        manager
            .issue_token(2, alloc::vec![predefined::WRITE_FILES], 0, 0)
            .unwrap();
        assert_eq!(manager.token_count(), 2);
    }

    // === 测试: 令牌有效性 ===
    #[test]
    fn test_is_valid() {
        let manager = TokenManager::new();
        let token_id = manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 0)
            .unwrap();

        assert!(manager.is_valid(token_id));
        assert!(!manager.is_valid(999));

        manager.revoke_token(token_id).unwrap();
        assert!(!manager.is_valid(token_id));
    }

    // === 测试: 过期令牌 ===
    #[test]
    fn test_expired_token() {
        let manager = TokenManager::new();
        // TTL = 100，过期时间 = 100
        let token_id = manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 100)
            .unwrap();

        let token = manager.get_token(token_id).unwrap();
        // 验证过期时间被正确设置
        assert_eq!(token.expires_at, 100);

        // TTL = 0 表示永不过期
        let forever_id = manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 0)
            .unwrap();
        let forever_token = manager.get_token(forever_id).unwrap();
        assert_eq!(forever_token.expires_at, 0);
    }

    // === 测试: 已撤销令牌 ===
    #[test]
    fn test_revoked_token() {
        let manager = TokenManager::new();
        let token_id = manager
            .issue_token(1, alloc::vec![predefined::READ_FILES], 0, 0)
            .unwrap();

        // 撤销后检查权限应失败
        manager.revoke_token(token_id).unwrap();
        let result = manager.check_permission(token_id, "read_files");
        assert!(result.is_err());
    }

    // === 测试: 最大委托次数 ===
    #[test]
    fn test_max_delegates() {
        let manager = TokenManager::new();
        let caps = alloc::vec![predefined::READ_FILES];
        let token_id = manager.issue_token(1, caps, 0, 0).unwrap();

        // 委托 10 次（默认 max_delegates = 10）
        for _ in 0..10 {
            let result = manager.delegate(
                token_id,
                2,
                alloc::vec![predefined::READ_FILES],
            );
            assert!(result.is_ok());
        }

        // 第 11 次应该失败
        let result = manager.delegate(
            token_id,
            3,
            alloc::vec![predefined::READ_FILES],
        );
        assert!(result.is_err());

        // 委托不可委托的能力应失败
        let undelegatable_id = manager
            .issue_token(
                1,
                alloc::vec![predefined::CREATE_AGENT],
                0,
                0,
            )
            .unwrap();
        let result = manager.delegate(
            undelegatable_id,
            2,
            alloc::vec![predefined::CREATE_AGENT],
        );
        assert!(result.is_err());
    }
}
