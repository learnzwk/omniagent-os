//! 地址令牌访问控制模块
//!
//! 实现鸿蒙风格的地址令牌访问控制机制。
//! 每个进程/Agent 持有地址令牌，内核通过令牌验证资源访问权限。

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

// ============================================================================
// 资源类型
// ============================================================================

/// 资源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// 内存
    Memory = 0,
    /// 设备
    Device = 1,
    /// 文件
    File = 2,
    /// 网络
    Network = 3,
    /// 进程间通信
    Ipc = 4,
    /// 服务
    Service = 5,
}

// ============================================================================
// 访问权限
// ============================================================================

/// 访问权限
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRight {
    /// 无权限
    None = 0,
    /// 读权限
    Read = 1,
    /// 写权限
    Write = 2,
    /// 执行权限
    Execute = 4,
    /// 全部权限
    All = 7,
}

impl AccessRight {
    /// 检查是否包含指定权限
    pub fn contains(&self, other: AccessRight) -> bool {
        (*self as u32) & (other as u32) != 0
    }
}

// ============================================================================
// 权限条目
// ============================================================================

/// 权限条目
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionEntry {
    /// 资源类型
    pub resource_type: ResourceType,
    /// 访问权限
    pub access_right: AccessRight,
    /// 资源 ID
    pub resource_id: u64,
}

// ============================================================================
// 地址令牌
// ============================================================================

/// 地址令牌
///
/// 每个进程/Agent 持有一个地址令牌，用于资源访问权限验证。
#[derive(Debug, Clone)]
pub struct AddressToken {
    /// 令牌 ID
    pub token_id: u64,
    /// 所有者 ID
    pub owner_id: u64,
    /// 权限列表
    pub permissions: Vec<PermissionEntry>,
    /// 是否激活
    pub is_active: bool,
    /// 创建时间
    pub created_at: u64,
}

// ============================================================================
// 地址令牌管理器
// ============================================================================

/// 地址令牌管理器
///
/// 管理所有地址令牌的创建、销毁和权限验证。
pub struct AddressTokenManager {
    /// 令牌映射表
    tokens: Mutex<BTreeMap<u64, AddressToken>>,
    /// 下一个可用令牌 ID
    next_id: AtomicU64,
}

impl AddressTokenManager {
    /// 创建新的地址令牌管理器
    pub fn new() -> Self {
        AddressTokenManager {
            tokens: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 创建地址令牌
    ///
    /// 为指定所有者创建一个新的地址令牌，初始状态为激活。
    pub fn create_token(&self, owner_id: u64, permissions: Vec<PermissionEntry>) -> u64 {
        let token_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let token = AddressToken {
            token_id,
            owner_id,
            permissions,
            is_active: true,
            created_at: 0, // 在实际内核中会使用系统时钟
        };

        let mut tokens = self.tokens.lock();
        tokens.insert(token_id, token);

        token_id
    }

    /// 销毁地址令牌
    pub fn destroy_token(&self, token_id: u64) {
        let mut tokens = self.tokens.lock();
        tokens.remove(&token_id);
    }

    /// 获取地址令牌
    pub fn get_token(&self, token_id: u64) -> Option<AddressToken> {
        let tokens = self.tokens.lock();
        tokens.get(&token_id).cloned()
    }

    /// 检查访问权限
    ///
    /// 验证令牌是否拥有对指定资源类型的指定权限。
    pub fn check_access(&self, token_id: u64, resource: ResourceType, right: AccessRight) -> bool {
        let tokens = self.tokens.lock();
        let token = match tokens.get(&token_id) {
            Some(t) => t,
            None => return false,
        };

        if !token.is_active {
            return false;
        }

        token
            .permissions
            .iter()
            .any(|p| p.resource_type == resource && p.access_right.contains(right))
    }

    /// 授予权限
    ///
    /// 向令牌添加新的权限条目。如果令牌不存在返回 false。
    pub fn grant_permission(&self, token_id: u64, entry: PermissionEntry) -> bool {
        let mut tokens = self.tokens.lock();
        if let Some(token) = tokens.get_mut(&token_id) {
            // 检查是否已存在相同权限
            let exists = token
                .permissions
                .iter()
                .any(|p| p.resource_type == entry.resource_type && p.resource_id == entry.resource_id);
            if !exists {
                token.permissions.push(entry);
            }
            true
        } else {
            false
        }
    }

    /// 撤销权限
    ///
    /// 撤销令牌对指定资源类型的所有权限。如果令牌不存在返回 false。
    pub fn revoke_permission(&self, token_id: u64, resource: ResourceType) -> bool {
        let mut tokens = self.tokens.lock();
        if let Some(token) = tokens.get_mut(&token_id) {
            token
                .permissions
                .retain(|p| p.resource_type != resource);
            true
        } else {
            false
        }
    }

    /// 列出指定所有者的所有令牌
    pub fn list_tokens(&self, owner_id: u64) -> Vec<AddressToken> {
        let tokens = self.tokens.lock();
        tokens
            .values()
            .filter(|t| t.owner_id == owner_id)
            .cloned()
            .collect()
    }

    /// 获取令牌总数
    pub fn token_count(&self) -> usize {
        let tokens = self.tokens.lock();
        tokens.len()
    }
}

/// 全局地址令牌管理器实例
pub static ADDRESS_TOKEN_MANAGER: spin::Lazy<Mutex<AddressTokenManager>> = spin::Lazy::new(|| {
    Mutex::new(AddressTokenManager::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用权限条目
    fn make_permission(resource_type: ResourceType, access_right: AccessRight, resource_id: u64) -> PermissionEntry {
        PermissionEntry {
            resource_type,
            access_right,
            resource_id,
        }
    }

    // === 测试: 创建令牌管理器 ===
    #[test]
    fn test_new() {
        let mgr = AddressTokenManager::new();
        assert_eq!(mgr.token_count(), 0);
    }

    // === 测试: 创建令牌 ===
    #[test]
    fn test_create_token() {
        let mgr = AddressTokenManager::new();
        let id = mgr.create_token(1, alloc::vec![]);
        assert_eq!(id, 1);
        assert_eq!(mgr.token_count(), 1);

        let token = mgr.get_token(id).unwrap();
        assert_eq!(token.owner_id, 1);
        assert!(token.is_active);
        assert!(token.permissions.is_empty());

        // 创建第二个令牌
        let id2 = mgr.create_token(2, alloc::vec![]);
        assert_eq!(id2, 2);
        assert_eq!(mgr.token_count(), 2);
    }

    // === 测试: 销毁令牌 ===
    #[test]
    fn test_destroy_token() {
        let mgr = AddressTokenManager::new();
        let id = mgr.create_token(1, alloc::vec![]);
        assert_eq!(mgr.token_count(), 1);

        mgr.destroy_token(id);
        assert_eq!(mgr.token_count(), 0);
        assert!(mgr.get_token(id).is_none());

        // 销毁不存在的令牌不应 panic
        mgr.destroy_token(999);
    }

    // === 测试: 检查访问权限 ===
    #[test]
    fn test_check_access() {
        let mgr = AddressTokenManager::new();
        let perms = alloc::vec![
            make_permission(ResourceType::Memory, AccessRight::Read, 0),
            make_permission(ResourceType::File, AccessRight::All, 0),
        ];
        let id = mgr.create_token(1, perms);

        assert!(mgr.check_access(id, ResourceType::Memory, AccessRight::Read));
        assert!(mgr.check_access(id, ResourceType::File, AccessRight::Read));
        assert!(mgr.check_access(id, ResourceType::File, AccessRight::Write));
        assert!(!mgr.check_access(id, ResourceType::Memory, AccessRight::Write));
        assert!(!mgr.check_access(id, ResourceType::Network, AccessRight::Read));

        // 不存在的令牌
        assert!(!mgr.check_access(999, ResourceType::Memory, AccessRight::Read));
    }

    // === 测试: 授予权限 ===
    #[test]
    fn test_grant_permission() {
        let mgr = AddressTokenManager::new();
        let id = mgr.create_token(1, alloc::vec![]);

        assert!(mgr.grant_permission(id, make_permission(ResourceType::Network, AccessRight::Read, 0)));
        assert!(mgr.check_access(id, ResourceType::Network, AccessRight::Read));

        // 对不存在的令牌授权应返回 false
        assert!(!mgr.grant_permission(999, make_permission(ResourceType::Memory, AccessRight::Read, 0)));
    }

    // === 测试: 撤销权限 ===
    #[test]
    fn test_revoke_permission() {
        let mgr = AddressTokenManager::new();
        let perms = alloc::vec![
            make_permission(ResourceType::Memory, AccessRight::Read, 0),
            make_permission(ResourceType::Memory, AccessRight::Write, 0),
            make_permission(ResourceType::File, AccessRight::Read, 0),
        ];
        let id = mgr.create_token(1, perms);

        assert!(mgr.check_access(id, ResourceType::Memory, AccessRight::Read));
        assert!(mgr.revoke_permission(id, ResourceType::Memory));
        assert!(!mgr.check_access(id, ResourceType::Memory, AccessRight::Read));
        assert!(!mgr.check_access(id, ResourceType::Memory, AccessRight::Write));
        // 文件权限不受影响
        assert!(mgr.check_access(id, ResourceType::File, AccessRight::Read));

        // 对不存在的令牌撤销应返回 false
        assert!(!mgr.revoke_permission(999, ResourceType::Memory));
    }

    // === 测试: 列出所有者的令牌 ===
    #[test]
    fn test_list_tokens() {
        let mgr = AddressTokenManager::new();
        mgr.create_token(1, alloc::vec![]);
        mgr.create_token(1, alloc::vec![]);
        mgr.create_token(2, alloc::vec![]);
        mgr.create_token(1, alloc::vec![]);

        let tokens = mgr.list_tokens(1);
        assert_eq!(tokens.len(), 3);

        let tokens2 = mgr.list_tokens(2);
        assert_eq!(tokens2.len(), 1);

        let tokens3 = mgr.list_tokens(99);
        assert_eq!(tokens3.len(), 0);
    }

    // === 测试: 未激活令牌的访问检查 ===
    #[test]
    fn test_inactive_token() {
        let mgr = AddressTokenManager::new();
        let perms = alloc::vec![
            make_permission(ResourceType::Memory, AccessRight::Read, 0),
        ];
        let id = mgr.create_token(1, perms);

        // 激活状态应有权限
        assert!(mgr.check_access(id, ResourceType::Memory, AccessRight::Read));

        // 手动获取并修改令牌为未激活
        {
            let mut tokens = mgr.tokens.lock();
            if let Some(token) = tokens.get_mut(&id) {
                token.is_active = false;
            }
        }

        // 未激活状态应无权限
        assert!(!mgr.check_access(id, ResourceType::Memory, AccessRight::Read));
    }

    // === 测试: 权限包含检查 ===
    #[test]
    fn test_access_right_contains() {
        assert!(AccessRight::All.contains(AccessRight::Read));
        assert!(AccessRight::All.contains(AccessRight::Write));
        assert!(AccessRight::All.contains(AccessRight::Execute));
        assert!(!AccessRight::Read.contains(AccessRight::Write));
        assert!(!AccessRight::None.contains(AccessRight::Read));
    }

    // === 测试: 令牌计数 ===
    #[test]
    fn test_token_count() {
        let mgr = AddressTokenManager::new();
        assert_eq!(mgr.token_count(), 0);

        mgr.create_token(1, alloc::vec![]);
        assert_eq!(mgr.token_count(), 1);

        mgr.create_token(2, alloc::vec![]);
        assert_eq!(mgr.token_count(), 2);

        mgr.destroy_token(1);
        assert_eq!(mgr.token_count(), 1);
    }
}
