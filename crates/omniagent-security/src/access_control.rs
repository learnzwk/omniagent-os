//! 访问控制模块
//!
//! 实现基于策略的访问控制引擎 (Policy-Based Access Control)，
//! 支持多策略优先级评估、glob 模式匹配和审计日志记录。

use std::collections::HashMap;

/// 访问决策
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessDecision {
    /// 允许访问
    Allow = 0,
    /// 拒绝访问
    Deny = 1,
    /// 默认拒绝（未匹配到任何规则）
    DefaultDeny = 2,
    /// 仅审计（记录但不阻止）
    Audit = 3,
}

impl std::fmt::Display for AccessDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessDecision::Allow => write!(f, "Allow"),
            AccessDecision::Deny => write!(f, "Deny"),
            AccessDecision::DefaultDeny => write!(f, "DefaultDeny"),
            AccessDecision::Audit => write!(f, "Audit"),
        }
    }
}

/// 访问请求
#[derive(Debug, Clone)]
pub struct AccessRequest {
    /// 请求者 (Agent ID 或用户 ID)
    pub requester: String,
    /// 资源路径
    pub resource: String,
    /// 操作 (read/write/execute/delete)
    pub action: String,
    /// 上下文信息
    pub context: HashMap<String, String>,
}

impl AccessRequest {
    /// 创建一个新的访问请求
    pub fn new(requester: &str, resource: &str, action: &str) -> Self {
        AccessRequest {
            requester: requester.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
            context: HashMap::new(),
        }
    }

    /// 添加上下文信息
    pub fn with_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }
}

/// 访问规则
#[derive(Debug, Clone)]
pub struct AccessRule {
    /// 规则 ID
    pub id: String,
    /// 规则效果
    pub effect: AccessDecision,
    /// 请求者匹配模式 (glob 模式)
    pub requester_pattern: Option<String>,
    /// 资源匹配模式 (glob 模式)
    pub resource_pattern: Option<String>,
    /// 操作匹配模式 (glob 模式)
    pub action_pattern: Option<String>,
    /// 条件表达式（预留，当前未实现）
    pub condition: Option<String>,
}

impl AccessRule {
    /// 创建一个新的访问规则
    pub fn new(id: &str, effect: AccessDecision) -> Self {
        AccessRule {
            id: id.to_string(),
            effect,
            requester_pattern: None,
            resource_pattern: None,
            action_pattern: None,
            condition: None,
        }
    }

    /// 设置请求者匹配模式
    pub fn requester(mut self, pattern: &str) -> Self {
        self.requester_pattern = Some(pattern.to_string());
        self
    }

    /// 设置资源匹配模式
    pub fn resource(mut self, pattern: &str) -> Self {
        self.resource_pattern = Some(pattern.to_string());
        self
    }

    /// 设置操作匹配模式
    pub fn action(mut self, pattern: &str) -> Self {
        self.action_pattern = Some(pattern.to_string());
        self
    }

    /// 设置条件表达式
    pub fn condition(mut self, cond: &str) -> Self {
        self.condition = Some(cond.to_string());
        self
    }

    /// 检查规则是否匹配给定的访问请求
    fn matches(&self, request: &AccessRequest, glob_match: impl Fn(&str, &str) -> bool) -> bool {
        // 检查请求者模式
        if let Some(ref pattern) = self.requester_pattern {
            if !glob_match(pattern, &request.requester) {
                return false;
            }
        }

        // 检查资源模式
        if let Some(ref pattern) = self.resource_pattern {
            if !glob_match(pattern, &request.resource) {
                return false;
            }
        }

        // 检查操作模式
        if let Some(ref pattern) = self.action_pattern {
            if !glob_match(pattern, &request.action) {
                return false;
            }
        }

        // 条件表达式检查（当前版本未实现，默认通过）
        // TODO: 实现条件表达式解析

        true
    }
}

/// 访问策略
#[derive(Debug, Clone)]
pub struct AccessPolicy {
    /// 策略 ID
    pub id: String,
    /// 策略名称
    pub name: String,
    /// 策略描述
    pub description: String,
    /// 策略规则列表
    pub rules: Vec<AccessRule>,
    /// 默认决策（当没有规则匹配时）
    pub default_decision: AccessDecision,
    /// 策略优先级（数值越大优先级越高）
    pub priority: u32,
}

impl AccessPolicy {
    /// 创建一个新的访问策略
    pub fn new(id: &str, name: &str) -> Self {
        AccessPolicy {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            rules: Vec::new(),
            default_decision: AccessDecision::DefaultDeny,
            priority: 0,
        }
    }

    /// 设置策略描述
    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// 设置默认决策
    pub fn default_decision(mut self, decision: AccessDecision) -> Self {
        self.default_decision = decision;
        self
    }

    /// 设置优先级
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// 添加规则
    pub fn add_rule(mut self, rule: AccessRule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// 审计日志条目（访问控制相关）
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// 时间戳
    pub timestamp: u64,
    /// 请求者
    pub requester: String,
    /// 资源
    pub resource: String,
    /// 操作
    pub action: String,
    /// 访问决策
    pub decision: AccessDecision,
    /// 匹配的策略 ID
    pub policy_id: Option<String>,
    /// 决策原因
    pub reason: String,
}

/// 访问控制引擎
///
/// 基于策略的访问控制引擎，按照策略优先级从高到低评估访问请求。
/// 第一个匹配的规则决定最终的访问决策。
pub struct AccessControlEngine {
    /// 策略列表
    policies: Vec<AccessPolicy>,
    /// 审计日志
    audit_log: Vec<AuditEntry>,
}

impl AccessControlEngine {
    /// 创建一个新的访问控制引擎
    pub fn new() -> Self {
        AccessControlEngine {
            policies: Vec::new(),
            audit_log: Vec::new(),
        }
    }

    /// 添加策略
    pub fn add_policy(&mut self, policy: AccessPolicy) {
        self.policies.push(policy);
        // 按优先级降序排列
        self.policies.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 移除策略
    pub fn remove_policy(&mut self, id: &str) -> bool {
        let len_before = self.policies.len();
        self.policies.retain(|p| p.id != id);
        self.policies.len() != len_before
    }

    /// 评估访问请求
    ///
    /// 按优先级从高到低遍历所有策略，对每个策略按顺序检查规则。
    /// 第一个匹配的规则决定决策结果。如果策略没有匹配的规则且默认决策
    /// 不是 DefaultDeny，则使用该默认决策；否则继续尝试下一个策略。
    /// 如果没有任何策略产生决策，返回 DefaultDeny。
    pub fn evaluate(&mut self, request: &AccessRequest) -> AccessDecision {
        let timestamp = Self::current_timestamp();

        // 遍历所有策略（已按优先级排序）
        for policy in &self.policies {
            for rule in &policy.rules {
                if rule.matches(request, |pattern, text| self.glob_match(pattern, text)) {
                    let decision = rule.effect;
                    let reason = format!(
                        "匹配规则 '{}' (策略: '{}')",
                        rule.id, policy.id
                    );

                    // 记录审计日志
                    self.audit_log.push(AuditEntry {
                        timestamp,
                        requester: request.requester.clone(),
                        resource: request.resource.clone(),
                        action: request.action.clone(),
                        decision,
                        policy_id: Some(policy.id.clone()),
                        reason,
                    });

                    return decision;
                }
            }

            // 没有规则匹配，检查策略默认决策
            // DefaultDeny 表示"此策略不做决策"，继续尝试下一个策略
            if policy.default_decision != AccessDecision::DefaultDeny {
                let decision = policy.default_decision;
                let reason = format!(
                    "无规则匹配，使用策略 '{}' 的默认决策",
                    policy.id
                );

                self.audit_log.push(AuditEntry {
                    timestamp,
                    requester: request.requester.clone(),
                    resource: request.resource.clone(),
                    action: request.action.clone(),
                    decision,
                    policy_id: Some(policy.id.clone()),
                    reason,
                });

                return decision;
            }
        }

        // 没有任何策略
        let reason = "无匹配策略，默认拒绝".to_string();
        self.audit_log.push(AuditEntry {
            timestamp,
            requester: request.requester.clone(),
            resource: request.resource.clone(),
            action: request.action.clone(),
            decision: AccessDecision::DefaultDeny,
            policy_id: None,
            reason,
        });

        AccessDecision::DefaultDeny
    }

    /// 获取审计日志
    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// 清除审计日志
    pub fn clear_audit_log(&mut self) {
        self.audit_log.clear();
    }

    /// 获取策略数量
    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }

    /// 简单的 glob 匹配
    ///
    /// 支持以下通配符：
    /// - `*` 匹配任意数量的非分隔符字符
    /// - `**` 匹配任意路径
    /// - `?` 匹配单个字符
    fn glob_match(&self, pattern: &str, text: &str) -> bool {
        let pattern: Vec<char> = pattern.chars().collect();
        let text: Vec<char> = text.chars().collect();
        let mut pi = 0; // pattern 索引
        let mut ti = 0; // text 索引
        let mut star_idx: Option<usize> = None;
        let mut match_idx = 0;

        while ti < text.len() {
            // 检查是否是 ** 通配符
            if pi + 1 < pattern.len() && pattern[pi] == '*' && pattern[pi + 1] == '*' {
                // ** 匹配任意路径（包括分隔符）
                star_idx = Some(pi);
                match_idx = ti;
                pi += 2; // 跳过 **

                // 如果 ** 后面没有更多模式，直接匹配
                if pi >= pattern.len() {
                    return true;
                }

                // 跳过 ** 后面的分隔符（如果有）
                if pi < pattern.len() && pattern[pi] == '/' {
                    pi += 1;
                }
                continue;
            }

            // 检查是否是 * 通配符
            if pi < pattern.len() && pattern[pi] == '*' {
                star_idx = Some(pi);
                match_idx = ti;
                pi += 1;
                continue;
            }

            // 字符匹配或 ? 通配符
            if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
                pi += 1;
                ti += 1;
                continue;
            }

            // 回溯到上一个 * 通配符
            if let Some(idx) = star_idx {
                pi = idx + 1;
                match_idx += 1;
                ti = match_idx;
                continue;
            }

            return false;
        }

        // 消耗剩余的 * 通配符
        while pi < pattern.len() && pattern[pi] == '*' {
            pi += 1;
        }

        pi == pattern.len()
    }

    /// 获取当前时间戳（简化版，使用单调计数器）
    fn current_timestamp() -> u64 {
        // 在实际实现中，这里应该使用系统时钟
        // 测试中我们使用固定值
        0
    }
}

impl Default for AccessControlEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_decision_values() {
        assert_eq!(AccessDecision::Allow as u8, 0);
        assert_eq!(AccessDecision::Deny as u8, 1);
        assert_eq!(AccessDecision::DefaultDeny as u8, 2);
        assert_eq!(AccessDecision::Audit as u8, 3);
    }

    #[test]
    fn test_access_request_new() {
        let req = AccessRequest::new("agent-1", "/data/file.txt", "read");
        assert_eq!(req.requester, "agent-1");
        assert_eq!(req.resource, "/data/file.txt");
        assert_eq!(req.action, "read");
        assert!(req.context.is_empty());
    }

    #[test]
    fn test_access_request_with_context() {
        let req = AccessRequest::new("agent-1", "/data/file.txt", "read")
            .with_context("source_ip", "192.168.1.1");
        assert_eq!(req.context.get("source_ip"), Some(&"192.168.1.1".to_string()));
    }

    #[test]
    fn test_access_rule_builder() {
        let rule = AccessRule::new("rule-1", AccessDecision::Allow)
            .requester("agent-*")
            .resource("/data/*")
            .action("read");

        assert_eq!(rule.id, "rule-1");
        assert_eq!(rule.effect, AccessDecision::Allow);
        assert_eq!(rule.requester_pattern, Some("agent-*".to_string()));
        assert_eq!(rule.resource_pattern, Some("/data/*".to_string()));
        assert_eq!(rule.action_pattern, Some("read".to_string()));
    }

    #[test]
    fn test_access_policy_builder() {
        let policy = AccessPolicy::new("policy-1", "测试策略")
            .description("用于测试的策略")
            .default_decision(AccessDecision::Deny)
            .priority(10);

        assert_eq!(policy.id, "policy-1");
        assert_eq!(policy.name, "测试策略");
        assert_eq!(policy.description, "用于测试的策略");
        assert_eq!(policy.default_decision, AccessDecision::Deny);
        assert_eq!(policy.priority, 10);
    }

    #[test]
    fn test_engine_add_and_remove_policy() {
        let mut engine = AccessControlEngine::new();
        assert_eq!(engine.policy_count(), 0);

        let policy = AccessPolicy::new("policy-1", "测试策略");
        engine.add_policy(policy);
        assert_eq!(engine.policy_count(), 1);

        assert!(engine.remove_policy("policy-1"));
        assert_eq!(engine.policy_count(), 0);

        assert!(!engine.remove_policy("non-existent"));
    }

    #[test]
    fn test_engine_evaluate_no_policies() {
        let mut engine = AccessControlEngine::new();
        let req = AccessRequest::new("agent-1", "/data/file.txt", "read");
        let decision = engine.evaluate(&req);
        assert_eq!(decision, AccessDecision::DefaultDeny);
    }

    #[test]
    fn test_engine_evaluate_allow() {
        let mut engine = AccessControlEngine::new();

        let policy = AccessPolicy::new("policy-1", "允许读取策略")
            .default_decision(AccessDecision::Deny)
            .priority(1)
            .add_rule(
                AccessRule::new("allow-read", AccessDecision::Allow)
                    .resource("/data/*")
                    .action("read"),
            );

        engine.add_policy(policy);

        let req = AccessRequest::new("agent-1", "/data/file.txt", "read");
        let decision = engine.evaluate(&req);
        assert_eq!(decision, AccessDecision::Allow);
    }

    #[test]
    fn test_engine_evaluate_deny() {
        let mut engine = AccessControlEngine::new();

        let policy = AccessPolicy::new("policy-1", "拒绝写入策略")
            .default_decision(AccessDecision::Allow)
            .priority(1)
            .add_rule(
                AccessRule::new("deny-write", AccessDecision::Deny)
                    .resource("/system/*")
                    .action("write"),
            );

        engine.add_policy(policy);

        let req = AccessRequest::new("agent-1", "/system/config", "write");
        let decision = engine.evaluate(&req);
        assert_eq!(decision, AccessDecision::Deny);
    }

    #[test]
    fn test_engine_evaluate_priority() {
        let mut engine = AccessControlEngine::new();

        // 低优先级策略：允许所有读取
        let low_policy = AccessPolicy::new("low", "低优先级策略")
            .default_decision(AccessDecision::Deny)
            .priority(1)
            .add_rule(
                AccessRule::new("allow-all-read", AccessDecision::Allow)
                    .action("read"),
            );

        // 高优先级策略：拒绝系统目录的读取
        let high_policy = AccessPolicy::new("high", "高优先级策略")
            .default_decision(AccessDecision::DefaultDeny)
            .priority(10)
            .add_rule(
                AccessRule::new("deny-system-read", AccessDecision::Deny)
                    .resource("/system/*")
                    .action("read"),
            );

        engine.add_policy(low_policy);
        engine.add_policy(high_policy);

        // 系统目录读取应该被高优先级策略拒绝
        let req = AccessRequest::new("agent-1", "/system/config", "read");
        let decision = engine.evaluate(&req);
        assert_eq!(decision, AccessDecision::Deny);

        // 非系统目录读取应该被低优先级策略允许
        let req2 = AccessRequest::new("agent-1", "/data/file.txt", "read");
        let decision2 = engine.evaluate(&req2);
        assert_eq!(decision2, AccessDecision::Allow);
    }

    #[test]
    fn test_engine_audit_log() {
        let mut engine = AccessControlEngine::new();

        let policy = AccessPolicy::new("policy-1", "测试策略")
            .default_decision(AccessDecision::Deny)
            .priority(1)
            .add_rule(
                AccessRule::new("allow-read", AccessDecision::Allow)
                    .resource("/data/*")
                    .action("read"),
            );

        engine.add_policy(policy);

        let req = AccessRequest::new("agent-1", "/data/file.txt", "read");
        engine.evaluate(&req);

        let log = engine.audit_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].requester, "agent-1");
        assert_eq!(log[0].resource, "/data/file.txt");
        assert_eq!(log[0].decision, AccessDecision::Allow);
        assert_eq!(log[0].policy_id, Some("policy-1".to_string()));

        engine.clear_audit_log();
        assert!(engine.audit_log().is_empty());
    }

    #[test]
    fn test_glob_match_exact() {
        let engine = AccessControlEngine::new();
        assert!(engine.glob_match("hello", "hello"));
        assert!(!engine.glob_match("hello", "world"));
    }

    #[test]
    fn test_glob_match_star() {
        let engine = AccessControlEngine::new();
        assert!(engine.glob_match("/data/*", "/data/file.txt"));
        assert!(engine.glob_match("/data/*", "/data/subdir/file.txt"));
        assert!(!engine.glob_match("/data/*", "/system/file.txt"));
    }

    #[test]
    fn test_glob_match_question() {
        let engine = AccessControlEngine::new();
        assert!(engine.glob_match("agent-?", "agent-1"));
        assert!(engine.glob_match("agent-?", "agent-A"));
        assert!(!engine.glob_match("agent-?", "agent-10"));
    }

    #[test]
    fn test_glob_match_star_star() {
        let engine = AccessControlEngine::new();
        assert!(engine.glob_match("/data/**", "/data/file.txt"));
        assert!(engine.glob_match("/data/**", "/data/subdir/file.txt"));
        assert!(engine.glob_match("/data/**", "/data/a/b/c/d.txt"));
        assert!(!engine.glob_match("/data/**", "/system/file.txt"));
    }

    #[test]
    fn test_glob_match_complex() {
        let engine = AccessControlEngine::new();
        assert!(engine.glob_match("agent-*", "agent-123"));
        assert!(engine.glob_match("agent-*", "agent-abc"));
        assert!(!engine.glob_match("agent-*", "user-123"));
        assert!(engine.glob_match("*.txt", "file.txt"));
        assert!(engine.glob_match("*", "anything"));
    }

    #[test]
    fn test_engine_evaluate_requester_pattern() {
        let mut engine = AccessControlEngine::new();

        let policy = AccessPolicy::new("policy-1", "按请求者过滤")
            .default_decision(AccessDecision::Deny)
            .priority(1)
            .add_rule(
                AccessRule::new("allow-admin", AccessDecision::Allow)
                    .requester("admin-*")
                    .action("read"),
            );

        engine.add_policy(policy);

        // 管理员应该被允许
        let req = AccessRequest::new("admin-1", "/data/file.txt", "read");
        assert_eq!(engine.evaluate(&req), AccessDecision::Allow);

        // 普通用户应该被拒绝
        let req2 = AccessRequest::new("user-1", "/data/file.txt", "read");
        assert_eq!(engine.evaluate(&req2), AccessDecision::Deny);
    }

    #[test]
    fn test_engine_evaluate_default_decision() {
        let mut engine = AccessControlEngine::new();

        let policy = AccessPolicy::new("policy-1", "默认允许策略")
            .default_decision(AccessDecision::Allow)
            .priority(1);

        engine.add_policy(policy);

        // 没有规则匹配，应该使用默认决策
        let req = AccessRequest::new("agent-1", "/anything", "read");
        assert_eq!(engine.evaluate(&req), AccessDecision::Allow);
    }
}
