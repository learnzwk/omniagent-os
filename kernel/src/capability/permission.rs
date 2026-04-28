//! 权限检查器
//!
//! 提供对能力令牌的权限检查功能，支持单个能力检查、
//! 全部能力检查和任一能力检查。

use crate::capability::token::CapabilityToken;

// ============================================================================
// 权限检查器
// ============================================================================

/// 权限检查器
///
/// 提供静态方法检查令牌是否拥有指定能力。
pub struct PermissionChecker;

impl PermissionChecker {
    /// 检查令牌是否拥有指定能力
    ///
    /// # 参数
    /// - `token`: 能力令牌
    /// - `capability_name`: 能力名称
    ///
    /// # 返回
    /// 如果令牌有效且拥有该能力返回 true
    pub fn check(token: &CapabilityToken, capability_name: &str) -> bool {
        // 检查是否已撤销
        if token.is_revoked.load(core::sync::atomic::Ordering::SeqCst) {
            return false;
        }

        token
            .capabilities
            .iter()
            .any(|c| c.name == capability_name)
    }

    /// 检查令牌是否拥有所有指定能力
    ///
    /// # 参数
    /// - `token`: 能力令牌
    /// - `capabilities`: 能力名称列表
    ///
    /// # 返回
    /// 如果令牌拥有所有指定能力返回 true
    pub fn check_all(token: &CapabilityToken, capabilities: &[&str]) -> bool {
        // 检查是否已撤销
        if token.is_revoked.load(core::sync::atomic::Ordering::SeqCst) {
            return false;
        }

        capabilities
            .iter()
            .all(|cap_name| token.capabilities.iter().any(|c| c.name == *cap_name))
    }

    /// 检查令牌是否拥有任一指定能力
    ///
    /// # 参数
    /// - `token`: 能力令牌
    /// - `capabilities`: 能力名称列表
    ///
    /// # 返回
    /// 如果令牌拥有任一指定能力返回 true
    pub fn check_any(token: &CapabilityToken, capabilities: &[&str]) -> bool {
        // 检查是否已撤销
        if token.is_revoked.load(core::sync::atomic::Ordering::SeqCst) {
            return false;
        }

        capabilities
            .iter()
            .any(|cap_name| token.capabilities.iter().any(|c| c.name == *cap_name))
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::token::{predefined, CapabilityEntry};
    use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    /// 创建测试用令牌
    fn make_test_token(caps: Vec<CapabilityEntry>) -> CapabilityToken {
        CapabilityToken {
            id: 1,
            owner: 1,
            capabilities: caps,
            issuer: 0,
            issued_at: 0,
            expires_at: 0,
            delegate_count: AtomicU32::new(0),
            max_delegates: 10,
            is_revoked: AtomicBool::new(false),
        }
    }

    // === 测试: 检查单个能力 ===
    #[test]
    fn test_check_single() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES, predefined::WRITE_FILES]);

        assert!(PermissionChecker::check(&token, "read_files"));
        assert!(PermissionChecker::check(&token, "write_files"));
        assert!(!PermissionChecker::check(&token, "network"));
        assert!(!PermissionChecker::check(&token, "execute"));
    }

    // === 测试: 检查所有能力 ===
    #[test]
    fn test_check_all() {
        let token = make_test_token(alloc::vec![
            predefined::READ_FILES,
            predefined::WRITE_FILES,
            predefined::EXECUTE,
        ]);

        assert!(PermissionChecker::check_all(
            &token,
            &["read_files", "write_files"]
        ));
        assert!(PermissionChecker::check_all(
            &token,
            &["read_files", "write_files", "execute"]
        ));

        // 缺少一个能力
        assert!(!PermissionChecker::check_all(
            &token,
            &["read_files", "network"]
        ));

        // 空列表应返回 true
        assert!(PermissionChecker::check_all(&token, &[]));
    }

    // === 测试: 检查任一能力 ===
    #[test]
    fn test_check_any() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES, predefined::WRITE_FILES]);

        assert!(PermissionChecker::check_any(
            &token,
            &["read_files", "network"]
        ));
        assert!(PermissionChecker::check_any(
            &token,
            &["network", "read_files"]
        ));
        assert!(PermissionChecker::check_any(
            &token,
            &["read_files", "write_files"]
        ));

        // 全部不匹配
        assert!(!PermissionChecker::check_any(
            &token,
            &["network", "execute"]
        ));

        // 空列表应返回 false
        assert!(!PermissionChecker::check_any(&token, &[]));
    }

    // === 测试: 检查缺失能力 ===
    #[test]
    fn test_check_missing() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES]);

        assert!(!PermissionChecker::check(&token, "write_files"));
        assert!(!PermissionChecker::check(&token, "network"));
        assert!(!PermissionChecker::check(&token, ""));
        assert!(!PermissionChecker::check_all(&token, &["write_files", "network"]));
        assert!(!PermissionChecker::check_any(&token, &["write_files", "network"]));
    }

    // === 测试: 检查已撤销令牌 ===
    #[test]
    fn test_check_revoked() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES, predefined::NETWORK]);

        // 撤销前
        assert!(PermissionChecker::check(&token, "read_files"));
        assert!(PermissionChecker::check_all(&token, &["read_files", "network"]));
        assert!(PermissionChecker::check_any(&token, &["read_files", "network"]));

        // 撤销后
        token.is_revoked.store(true, Ordering::SeqCst);
        assert!(!PermissionChecker::check(&token, "read_files"));
        assert!(!PermissionChecker::check_all(&token, &["read_files"]));
        assert!(!PermissionChecker::check_any(&token, &["read_files"]));
    }

    /// 测试：空能力令牌的所有检查都应返回 false
    #[test]
    fn test_check_empty_token() {
        let token = make_test_token(alloc::vec![]);

        assert!(!PermissionChecker::check(&token, "read_files"));
        assert!(!PermissionChecker::check(&token, ""));
        assert!(!PermissionChecker::check_all(&token, &["read_files"]));
        assert!(PermissionChecker::check_all(&token, &[])); // 空列表应返回 true
        assert!(!PermissionChecker::check_any(&token, &["read_files"]));
        assert!(!PermissionChecker::check_any(&token, &[])); // 空列表应返回 false
    }

    /// 测试：无效能力名称检查
    #[test]
    fn test_check_invalid_capability_names() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES]);

        // 各种无效名称
        assert!(!PermissionChecker::check(&token, ""));
        assert!(!PermissionChecker::check(&token, "READ_FILES")); // 大小写敏感
        assert!(!PermissionChecker::check(&token, "read_files "));
        assert!(!PermissionChecker::check(&token, " read_files"));
        assert!(!PermissionChecker::check(&token, "read_file")); // 部分匹配
        assert!(!PermissionChecker::check(&token, "read_files_extra"));
    }

    /// 测试：多能力检查 - 拥有全部预定义能力
    #[test]
    fn test_check_all_predefined_capabilities() {
        let token = make_test_token(predefined::all());

        // 检查所有预定义能力
        assert!(PermissionChecker::check(&token, "read_files"));
        assert!(PermissionChecker::check(&token, "write_files"));
        assert!(PermissionChecker::check(&token, "execute"));
        assert!(PermissionChecker::check(&token, "network"));
        assert!(PermissionChecker::check(&token, "create_agent"));
        assert!(PermissionChecker::check(&token, "kill_agent"));
        assert!(PermissionChecker::check(&token, "send_message"));
        assert!(PermissionChecker::check(&token, "receive_message"));
        assert!(PermissionChecker::check(&token, "manage_system"));
        assert!(PermissionChecker::check(&token, "access_hardware"));
        assert!(PermissionChecker::check(&token, "manage_security"));
    }

    /// 测试：部分能力匹配 - check_all 部分匹配应失败
    #[test]
    fn test_check_all_partial_match() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES, predefined::WRITE_FILES]);

        // 全部匹配应成功
        assert!(PermissionChecker::check_all(&token, &["read_files", "write_files"]));

        // 部分匹配应失败
        assert!(!PermissionChecker::check_all(&token, &["read_files", "network"]));
        assert!(!PermissionChecker::check_all(&token, &["write_files", "execute"]));
        assert!(!PermissionChecker::check_all(&token, &["network", "execute"]));
    }

    /// 测试：check_any 部分匹配应成功
    #[test]
    fn test_check_any_partial_match() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES]);

        // 只要有一个匹配就应成功
        assert!(PermissionChecker::check_any(&token, &["read_files", "network"]));
        assert!(PermissionChecker::check_any(&token, &["network", "read_files", "execute"]));
        assert!(PermissionChecker::check_any(&token, &["execute", "read_files"]));

        // 全部不匹配应失败
        assert!(!PermissionChecker::check_any(&token, &["network", "execute", "manage_system"]));
    }

    /// 测试：check_all 和 check_any 对空列表的行为
    #[test]
    fn test_check_empty_list() {
        let token = make_test_token(alloc::vec![predefined::READ_FILES]);

        // check_all 对空列表应返回 true（vacuous truth）
        assert!(PermissionChecker::check_all(&token, &[]));

        // check_any 对空列表应返回 false
        assert!(!PermissionChecker::check_any(&token, &[]));
    }

    /// 测试：撤销后所有检查方法都应返回 false
    #[test]
    fn test_revoked_all_methods() {
        let token = make_test_token(alloc::vec![
            predefined::READ_FILES,
            predefined::WRITE_FILES,
            predefined::NETWORK,
        ]);

        // 撤销令牌
        token.is_revoked.store(true, Ordering::SeqCst);

        // check 应返回 false
        assert!(!PermissionChecker::check(&token, "read_files"));
        assert!(!PermissionChecker::check(&token, "write_files"));
        assert!(!PermissionChecker::check(&token, "network"));

        // check_all 应返回 false
        assert!(!PermissionChecker::check_all(&token, &["read_files"]));
        assert!(!PermissionChecker::check_all(&token, &["read_files", "write_files"]));
        assert!(!PermissionChecker::check_all(&token, &[])); // 撤销后即使空列表也应返回 false

        // check_any 应返回 false
        assert!(!PermissionChecker::check_any(&token, &["read_files"]));
        assert!(!PermissionChecker::check_any(&token, &["read_files", "network"]));
        assert!(!PermissionChecker::check_any(&token, &[]));
    }

    /// 测试：重复能力条目
    #[test]
    fn test_check_duplicate_capabilities() {
        // 创建包含重复能力的令牌
        let token = make_test_token(alloc::vec![
            predefined::READ_FILES,
            predefined::READ_FILES, // 重复
        ]);

        // 重复能力不应影响检查结果
        assert!(PermissionChecker::check(&token, "read_files"));
        assert!(!PermissionChecker::check(&token, "network"));
        assert!(PermissionChecker::check_all(&token, &["read_files"]));
        assert!(PermissionChecker::check_any(&token, &["read_files", "network"]));
    }
}
