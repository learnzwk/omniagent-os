# OmniAgent OS 授权管理器规范 (Authorization Manager Specification)

> **模块编号**: `omniagent-auth` | **版本**: v0.3.0-draft | **状态**: 设计阶段

---

## 1. 概述 (Purpose)

授权管理器是 OmniAgent OS 的核心安全组件，管理所有 Agent 对资源的访问控制。采用三层授权架构：**一次性授权 (One-Time Auth)**、**永久授权 (Permanent Auth)** 和 **策略引擎 (Policy Engine, PBAC+RBAC)**，提供细粒度、可审计的访问控制。

### 1.1 设计目标

| 目标 | 描述 |
|------|------|
| 最小权限原则 | Agent 仅获得完成任务所需的最小权限集 |
| 全链路审计 | 所有授权操作可追溯，审计日志防篡改 |
| 多模态确认 | 桌面弹窗 / CLI 确认 / 语音确认 |
| 内核集成 | 基于能力 (Capability) 的访问控制与微内核深度集成 |
| 高性能 | 策略评估延迟 < 100μs |

### 1.2 架构总览

```
┌─────────────────────────────────────────────────┐
│              Agent / Application                  │
└──────────┬──────────────────┬────────────────────┘
     ┌──────▼──────┐  ┌───────▼──────┐
     │  One-Time   │  │  Permanent   │
     │  Auth       │  │  Auth        │
     └──────┬──────┘  └───────┬──────┘
     ┌──────▼──────────────────▼──────┐
     │    Policy Engine (PBAC+RBAC)   │
     └───────────────┬───────────────┘
     ┌───────────────▼───────────────┐
     │  Auth Store (Persistent+Log)  │
     └───────────────┬───────────────┘
     ┌───────────────▼───────────────┐
     │  Kernel Capability Layer      │
     └───────────────────────────────┘
```

---

## 2. 接口定义 (Interfaces)

### 2.1 核心特征

```rust
/// 授权提供者特征
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// 请求一次性授权令牌
    async fn request_once(
        &self, agent_id: AgentId, resource: ResourceRef, scope: AccessScope,
    ) -> Result<AuthToken, AuthError>;

    /// 授予永久授权
    async fn grant_permanent(
        &self, agent_id: AgentId, resource: ResourceRef, level: AccessLevel,
    ) -> Result<GrantId, AuthError>;

    /// 撤销授权
    async fn revoke(&self, grant_id: GrantId) -> Result<(), AuthError>;

    /// 验证授权有效性
    async fn validate(&self, token: &AuthToken) -> Result<AuthDecision, AuthError>;
}

/// 策略评估器特征 (PBAC + RBAC)
#[async_trait]
pub trait PolicyEvaluator: Send + Sync {
    async fn evaluate(&self, request: &AccessRequest, context: &PolicyContext)
        -> Result<PolicyDecision, AuthError>;
    async fn add_policy(&self, policy: Policy) -> Result<PolicyId, AuthError>;
    async fn remove_policy(&self, policy_id: PolicyId) -> Result<(), AuthError>;
}

/// 授权存储特征（持久化 + 审计日志）
#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn store_grant(&self, grant: GrantRecord) -> Result<(), AuthError>;
    async fn query_grant(&self, grant_id: GrantId) -> Result<Option<GrantRecord>, AuthError>;
    async fn append_audit(&self, entry: AuditEntry) -> Result<AuditHash, AuthError>;
    async fn verify_audit_chain(&self) -> Result<AuditIntegrity, AuthError>;
}
```

### 2.2 授权管理器主接口

```rust
pub struct AuthorizationManager {
    one_time: Box<dyn AuthProvider>,
    permanent: Box<dyn AuthProvider>,
    policy_engine: Arc<dyn PolicyEvaluator>,
    store: Arc<dyn AuthStore>,
    consent_ui: Arc<dyn ConsentUi>,
}

impl AuthorizationManager {
    /// 请求一次性授权（含用户确认流程）
    pub async fn request_once(
        &self, agent_id: AgentId, resource: ResourceRef, scope: AccessScope,
    ) -> Result<AuthToken, AuthError> {
        let request = AccessRequest::new(agent_id.clone(), resource.clone(), scope.clone());
        let decision = self.policy_engine.evaluate(&request, &PolicyContext::default()).await?;
        match decision {
            PolicyDecision::Allow | PolicyDecision::RequireConsent => {
                let consent = self.consent_ui.request_consent(&agent_id, &resource, &scope).await?;
                if !consent.granted { return Err(AuthError::Denied(DenyReason::UserRejected)); }
                let token = self.one_time.request_once(agent_id, resource, scope).await?;
                self.store.append_audit(AuditEntry::token_issued(&token)).await?;
                Ok(token)
            }
            PolicyDecision::Deny(reason) => Err(AuthError::Denied(reason)),
        }
    }

    /// 授予永久授权
    pub async fn grant_permanent(
        &self, agent_id: AgentId, resource: ResourceRef, level: AccessLevel,
    ) -> Result<GrantId, AuthError> {
        let consent = self.consent_ui.request_consent(
            &agent_id, &resource, &AccessScope::from(level.clone()),
        ).await?;
        if !consent.granted { return Err(AuthError::Denied(DenyReason::UserRejected)); }
        let grant_id = self.permanent.grant_permanent(agent_id, resource, level).await?;
        self.store.append_audit(AuditEntry::grant_created(&grant_id)).await?;
        Ok(grant_id)
    }

    /// 验证令牌并转换为内核能力
    pub async fn validate_to_capability(&self, token: &AuthToken) -> Result<KernelCapability, AuthError> {
        let decision = self.one_time.validate(token).await?;
        match decision {
            AuthDecision::Granted { resource, scope } => {
                let cap = self.issue_capability(&resource, &scope)?;
                self.store.append_audit(AuditEntry::capability_issued(&cap)).await?;
                Ok(cap)
            }
            AuthDecision::Expired => Err(AuthError::Expired),
            AuthDecision::Revoked => Err(AuthError::Revoked),
            AuthDecision::Denied(reason) => Err(AuthError::Denied(reason)),
        }
    }
}
```

### 2.3 同意 UI 特征

```rust
#[async_trait]
pub trait ConsentUi: Send + Sync {
    async fn request_consent(
        &self, agent_id: &AgentId, resource: &ResourceRef, scope: &AccessScope,
    ) -> Result<ConsentResult, AuthError>;
}

#[derive(Debug, Clone)]
pub struct ConsentResult {
    pub granted: bool,
    pub method: ConsentMethod,
    pub timestamp: SystemTime,
    pub user_id: Option<UserId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentMethod {
    DesktopPopup, CliConfirmation, VoiceConfirmation, Biometric, AutoApproved,
}
```

---

## 3. 数据结构 (Data Structures)

```rust
/// Agent 唯一标识
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentId { pub uuid: Uuid, pub name: String, pub public_key: [u8; 32] }

/// 资源引用
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRef {
    pub resource_type: ResourceType, pub path: String, pub namespace: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType { FileSystem, Network, Device, Memory, Process, SystemCall, Custom(u16) }

/// 访问范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessScope {
    pub permissions: Vec<Permission>,
    pub constraints: Vec<AccessConstraint>,
    pub expires_at: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission { Read, Write, Execute, Admin, Delegate }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessConstraint {
    MaxDuration(Duration), MaxUses(u32),
    TimeWindow { start: SystemTime, end: SystemTime },
}

/// 访问级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AccessLevel { None=0, Read=1, Write=2, Execute=3, Admin=4, FullControl=5 }

/// 一次性授权令牌
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token_id: Uuid, pub issuer: AgentId, pub holder: AgentId,
    pub resource: ResourceRef, pub scope: AccessScope,
    pub issued_at: SystemTime, pub expires_at: SystemTime,
    pub max_uses: u32, pub used_count: AtomicU32,
    pub signature: [u8; 32], pub nonce: [u8; 16],
}

/// 授权记录（永久授权）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantRecord {
    pub grant_id: GrantId, pub agent_id: AgentId, pub resource: ResourceRef,
    pub level: AccessLevel, pub granted_at: SystemTime, pub granted_by: UserId,
    pub revoked_at: Option<SystemTime>, pub conditions: Vec<PolicyCondition>,
}

/// 授权决策
#[derive(Debug, Clone)]
pub enum AuthDecision {
    Granted { resource: ResourceRef, scope: AccessScope },
    Denied(DenyReason), Expired, Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    PolicyViolation, InsufficientLevel, UserRejected,
    ResourceLocked, ScopeExceeded, AgentSuspended,
}
```

---

## 4. 策略引擎 (Policy Engine)

### 4.1 策略定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub id: PolicyId, pub name: String, pub effect: PolicyEffect,
    pub subjects: SubjectMatcher, pub resources: ResourceMatcher,
    pub conditions: Vec<PolicyCondition>, pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyEffect { Allow, Deny, RequireConsent }

/// 主体匹配器 (RBAC 角色 + Agent ID)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectMatcher {
    pub agent_ids: Vec<AgentId>, pub roles: Vec<String>, pub match_all: bool,
}

/// 策略条件 (PBAC 用途绑定)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyCondition {
    TimeWindow { start: NaiveTime, end: NaiveTime, timezone: String },
    Purpose { allowed_purposes: Vec<String>, purpose_evidence_required: bool },
    TrustLevel { minimum: TrustLevel },
    Context { key: String, operator: ConditionOperator, value: serde_json::Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel { Untrusted=0, Low=1, Medium=2, High=3, Critical=4 }

#[derive(Debug, Clone)]
pub enum PolicyDecision { Allow, Deny(DenyReason), RequireConsent }
```

### 4.2 混合评估流程

```
AccessRequest → RBAC Phase (角色匹配) → PBAC Phase (用途匹配)
    → Condition Evaluation (上下文评估) → Priority Merge → PolicyDecision
```

```rust
impl HybridPolicyEvaluator {
    pub async fn evaluate(&self, request: &AccessRequest, context: &PolicyContext)
        -> Result<PolicyDecision, AuthError> {
        let policies = self.policies.read().await;
        let mut matched: Vec<&Policy> = policies.iter()
            .filter(|p| self.matches_subject(p, &request.agent_id, context))
            .filter(|p| self.matches_resource(p, &request.resource))
            .collect();
        matched.sort_by_key(|p| std::cmp::Reverse(p.priority));

        let mut final_decision = PolicyDecision::Deny(DenyReason::PolicyViolation);
        let mut has_explicit_deny = false;
        for policy in &matched {
            if self.evaluate_conditions(&policy.conditions, request, context).await? {
                match policy.effect {
                    PolicyEffect::Deny => { has_explicit_deny = true; break; }
                    PolicyEffect::Allow => { final_decision = PolicyDecision::Allow; }
                    PolicyEffect::RequireConsent => {
                        if !matches!(final_decision, PolicyDecision::Allow) {
                            final_decision = PolicyDecision::RequireConsent;
                        }
                    }
                }
            }
        }
        if has_explicit_deny { return Ok(PolicyDecision::Deny(DenyReason::PolicyViolation)); }
        Ok(final_decision)
    }
}
```

---

## 5. 状态机 (State Machines)

### 5.1 一次性令牌生命周期

```
request_once() → Issued → validate() → Valid → consume() → Consumed → Destroyed
                              ↓                                    ↑
                           timeout                              auto
                              ↓                                    ↓
                          Expired ◄──────────────────────── Destroyed
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenState { Issued, Valid, Consumed, Expired, Destroyed, Revoked }

pub struct TokenStateMachine { state: AtomicU8, token: AuthToken }

impl TokenStateMachine {
    pub fn validate(&self) -> Result<(), AuthError> {
        loop {
            let current = self.state.load(Ordering::Acquire);
            match TokenState::from_u8(current) {
                Some(TokenState::Issued) | Some(TokenState::Valid) => {
                    match self.state.compare_exchange_weak(
                        current, TokenState::Valid as u8, Ordering::AcqRel, Ordering::Acquire,
                    ) { Ok(_) => return Ok(()), Err(_) => continue, }
                }
                Some(TokenState::Expired) => return Err(AuthError::Expired),
                Some(TokenState::Revoked) => return Err(AuthError::Revoked),
                _ => return Err(AuthError::Denied(DenyReason::ScopeExceeded)),
            }
        }
    }

    pub fn consume(&self) -> Result<(), AuthError> {
        loop {
            let current = self.state.load(Ordering::Acquire);
            if matches!(TokenState::from_u8(current), Some(TokenState::Valid)) {
                match self.state.compare_exchange_weak(
                    current, TokenState::Consumed as u8, Ordering::AcqRel, Ordering::Acquire,
                ) { Ok(_) => { self.transition_to(TokenState::Destroyed); return Ok(()); } Err(_) => continue, }
            }
            return Err(AuthError::Denied(DenyReason::ScopeExceeded));
        }
    }
}
```

### 5.2 永久授权状态机

```
grant_permanent() → Active → revoke() → Revoked
                       ↓
                   suspend() → Suspended → resume() → Active
                       ↓
                    expire() → Expired
```

---

## 6. 审计日志 (Audit Log)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub sequence: u64, pub timestamp: SystemTime, pub event: AuditEvent,
    pub actor: AuditActor, pub target: String, pub decision: Option<bool>,
    pub prev_hash: [u8; 32], pub hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    TokenIssued { token_id: Uuid }, TokenUsed { token_id: Uuid },
    GrantCreated { grant_id: Uuid }, GrantRevoked { grant_id: Uuid },
    ConsentGranted { method: String }, ConsentDenied { method: String },
    CapabilityIssued { cap_id: Uuid },
}
```

---

## 7. 错误处理 (Error Handling)

```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("授权被拒绝: {0}")] Denied(DenyReason),
    #[error("授权已过期")] Expired,
    #[error("授权已被撤销")] Revoked,
    #[error("权限级别不足: 需要 {required:?}, 实际 {actual:?}")]
    InsufficientLevel { required: AccessLevel, actual: AccessLevel },
    #[error("令牌无效: {0}")] InvalidToken(String),
    #[error("签名验证失败")] SignatureMismatch,
    #[error("重放攻击检测")] ReplayDetected,
    #[error("审计链完整性被破坏")] AuditChainBroken,
    #[error("存储错误: {0}")] Storage(#[from] StorageError),
    #[error("用户确认超时")] ConsentTimeout,
}
```

| 错误类型 | 处理策略 | 审计级别 |
|----------|----------|----------|
| `Denied` | 返回拒绝原因 | WARNING |
| `Expired` | 提示重新申请 | INFO |
| `Revoked` | 拒绝并记录 | WARNING |
| `ReplayDetected` | 拒绝并告警 | CRITICAL |
| `AuditChainBroken` | 系统告警 | CRITICAL |

---

## 8. 安全设计 (Security)

### 8.1 常量时间比较 + 防重放

```rust
/// 常量时间令牌比较（防止时序攻击）
pub fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    if a.len() != b.len() {
        let _ = a.iter().zip(b.iter().cycle()).fold(0u8, |acc, (x, y)| acc ^ x ^ y);
        return false;
    }
    a.ct_eq(b).into()
}

/// Nonce 缓存（滑动窗口防重放）
pub struct NonceCache { cache: DashMap<[u8; 16], Instant>, window: Duration }
```

### 8.2 审计链防篡改

```rust
impl AuditChainValidator {
    pub fn compute_hash(entry: &AuditEntry) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&entry.sequence.to_le_bytes());
        hasher.update(&serde_json::to_vec(&entry.event).unwrap());
        hasher.update(&entry.prev_hash);
        hasher.finalize().into()
    }

    pub async fn verify_chain(&self, entries: &[AuditEntry]) -> Result<AuditIntegrity, AuthError> {
        let mut prev_hash = [0u8; 32];
        for (i, entry) in entries.iter().enumerate() {
            if entry.prev_hash != prev_hash || entry.hash != Self::compute_hash(entry) {
                return Ok(AuditIntegrity { valid: false, first_violation: Some(i as u64), .. });
            }
            prev_hash = entry.hash;
        }
        Ok(AuditIntegrity { valid: true, total_entries: entries.len() as u64, first_violation: None, .. })
    }
}
```

---

## 9. 性能约束 (Performance Constraints)

| 操作 | 目标延迟 | 最大延迟 | 吞吐量 |
|------|---------|---------|--------|
| 策略评估 (evaluate) | < 50μs | < 100μs | > 10,000 ops/s |
| 令牌签发 (request_once) | < 200μs | < 500μs | > 5,000 ops/s |
| 令牌验证 (validate) | < 10μs | < 50μs | > 50,000 ops/s |
| 审计日志追加 | < 50μs | < 100μs | > 10,000 ops/s |

---

## 10. 测试用例 (Test Cases)

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_one_time_auth_lifecycle() {
        let provider = OneTimeAuthProvider::new(InMemoryAuthStore::new());
        let agent = AgentId::test_new("test-agent");
        let resource = ResourceRef::file("/tmp/test.dat");

        let token = provider.request_once(agent.clone(), resource.clone(), AccessScope::read_only()).await.unwrap();
        assert_eq!(token.used_count.load(Ordering::Relaxed), 0);

        let decision = provider.validate(&token).await.unwrap();
        assert!(matches!(decision, AuthDecision::Granted { .. }));

        provider.consume(&token.token_id).await.unwrap();
        assert!(matches!(provider.validate(&token).await, Err(AuthError::Denied(_))));
    }

    #[tokio::test]
    async fn test_expired_token_rejected() {
        let provider = OneTimeAuthProvider::new(InMemoryAuthStore::new());
        let mut token = provider.request_once(
            AgentId::test_new("agent"), ResourceRef::file("/tmp/f"), AccessScope::read_only(),
        ).await.unwrap();
        token.expires_at = SystemTime::now() - Duration::from_secs(1);
        assert!(matches!(provider.validate(&token).await, Err(AuthError::Expired)));
    }

    #[tokio::test]
    async fn test_replay_attack_detection() {
        let provider = OneTimeAuthProvider::with_nonce_cache(InMemoryAuthStore::new(), NonceCache::default());
        let token = provider.request_once(
            AgentId::test_new("attacker"), ResourceRef::file("/etc/shadow"), AccessScope::read_only(),
        ).await.unwrap();
        assert!(provider.validate(&token).await.is_ok());
        assert!(matches!(provider.validate(&token).await, Err(AuthError::Denied(_))));
    }

    #[tokio::test]
    async fn test_permanent_grant_revoke() {
        let provider = PermanentAuthProvider::new(InMemoryAuthStore::new());
        let grant_id = provider.grant_permanent(
            AgentId::test_new("agent"), ResourceRef::network("tcp://api:443"), AccessLevel::Write,
        ).await.unwrap();
        provider.revoke(grant_id.clone()).await.unwrap();
        let record = provider.store().query_grant(grant_id).await.unwrap();
        assert!(record.unwrap().revoked_at.is_some());
    }

    #[tokio::test]
    async fn test_rbac_role_matching() {
        let evaluator = HybridPolicyEvaluator::new();
        evaluator.add_policy(Policy {
            effect: PolicyEffect::Allow,
            subjects: SubjectMatcher { roles: vec!["admin".into()], ..Default::default() },
            resources: ResourceMatcher { resource_types: vec![ResourceType::FileSystem], ..Default::default() },
            priority: 100, ..Default::default()
        }).await.unwrap();
        let ctx = PolicyContext { agent_roles: vec!["admin".into()], ..Default::default() };
        let decision = evaluator.evaluate(&AccessRequest::test_default(), &ctx).await.unwrap();
        assert!(matches!(decision, PolicyDecision::Allow));
    }

    #[tokio::test]
    async fn test_pbac_purpose_matching() {
        let evaluator = HybridPolicyEvaluator::new();
        evaluator.add_policy(Policy {
            effect: PolicyEffect::Allow,
            conditions: vec![PolicyCondition::Purpose {
                allowed_purposes: vec!["debug".into()], purpose_evidence_required: false,
            }],
            ..Default::default()
        }).await.unwrap();
        let mut req = AccessRequest::test_default();
        req.purpose = Some("debug".into());
        assert!(matches!(evaluator.evaluate(&req, &PolicyContext::default()).await.unwrap(), PolicyDecision::Allow));
        req.purpose = Some("exfiltrate".into());
        assert!(matches!(evaluator.evaluate(&req, &PolicyContext::default()).await.unwrap(), PolicyDecision::Deny(_)));
    }

    #[tokio::test]
    async fn test_audit_chain_integrity() {
        let store = InMemoryAuthStore::new();
        for i in 0..100 { store.append_audit(AuditEntry::new_test(i)).await.unwrap(); }
        let entries = store.get_all_audit_entries().await.unwrap();
        let result = AuditChainValidator::verify_chain(&entries).await.unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn test_tampered_audit_detection() {
        let store = InMemoryAuthStore::new();
        for i in 0..50 { store.append_audit(AuditEntry::new_test(i)).await.unwrap(); }
        let mut entries = store.get_all_audit_entries().await.unwrap();
        entries[25].target = "TAMPERED".into();
        let result = AuditChainValidator::verify_chain(&entries).await.unwrap();
        assert!(!result.valid);
        assert_eq!(result.first_violation, Some(25));
    }

    #[test]
    fn test_constant_time_comparison() {
        assert!(constant_time_compare(b"secret_token_12345", b"secret_token_12345"));
        assert!(!constant_time_compare(b"secret_token_12345", b"secret_token_12346"));
        assert!(!constant_time_compare(b"short", b"much_longer_value"));
    }
}
```

---

## 11. 配置参考

```toml
[auth]
token_default_ttl = "5m"
token_max_ttl = "1h"
nonce_cache_window = "5m"
audit_max_entries = 1_000_000

[auth.consent]
default_method = "desktop_popup"
timeout = "60s"
auto_approve_trust_level = "high"

[auth.policy]
eval_cache_size = 10_000
eval_cache_ttl = "30s"
default_effect = "deny"
```

---

> **文档版本**: v0.3.0-draft | **最后更新**: 2026-04-25 | **作者**: OmniAgent OS 安全团队
