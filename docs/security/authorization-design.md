# OmniAgent OS 授权设计规范

> **文档版本**: v1.0.0 | **最后更新**: 2026-04-25 | **责任团队**: 安全工程与架构组

---

## 1. 授权架构概述

### 1.1 设计原则

| 原则 | 描述 |
|------|------|
| **最小权限** | 每个实体仅获得完成其任务所需的最小权限集 |
| **默认拒绝** | 未明确授权的操作默认被拒绝 |
| **权限可撤销** | 所有授权都可以被撤销，撤销即时生效 |
| **权限可委托** | 权限可以在受控条件下从一个实体委托给另一个 |
| **权限可审计** | 所有授权决策和操作都有完整的审计记录 |
| **意图驱动** | 授权基于操作的目的 (Purpose)，而非仅基于角色 |

### 1.2 架构层次

```
L4: 审计与合规  ← 操作记录、合规报告
L3: 策略引擎 (PBAC + RBAC)  ← 策略评估、决策
L2: 能力系统 (Capability)  ← 令牌管理、委托
L1: 内核强制 (Kernel Enforce)  ← 硬件级权限检查
```

授权流程：请求者 → 内核能力检查 → 授权服务 → 策略引擎 (PBAC+RBAC) → 决策 → 审计日志

---

## 2. 能力令牌系统 (Capability-Based Access Control)

### 2.1 能力令牌格式

```rust
pub struct CapabilityToken {
    pub id: TokenId,
    pub holder: EntityId,
    pub issuer: EntityId,
    pub capabilities: CapSet,
    pub created_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub delegation_depth: u8,     // 0 = 不可委托
    pub usage_limit: Option<u64>,
    pub signature: Vec<u8>,       // 防止伪造
}

/// 能力集合 (u128 位掩码，最多 128 种能力)
pub struct CapSet { bits: u128 }

impl CapSet {
    pub const FILE_READ: CapSet = CapSet { bits: 1 << 0 };
    pub const FILE_WRITE: CapSet = CapSet { bits: 1 << 1 };
    pub const NETWORK: CapSet = CapSet { bits: 1 << 2 };
    pub const IPC: CapSet = CapSet { bits: 1 << 3 };
    pub const PROCESS_SPAWN: CapSet = CapSet { bits: 1 << 4 };
    pub const ADMIN: CapSet = CapSet { bits: 1 << 5 };
    pub const GPU_ACCESS: CapSet = CapSet { bits: 1 << 6 };
    pub const AUDIO_ACCESS: CapSet = CapSet { bits: 1 << 7 };
    pub const CLIPBOARD_READ: CapSet = CapSet { bits: 1 << 8 };
    pub const CLIPBOARD_WRITE: CapSet = CapSet { bits: 1 << 9 };
    pub const SCREEN_CAPTURE: CapSet = CapSet { bits: 1 << 10 };
    pub const CLOUD_API: CapSet = CapSet { bits: 1 << 11 };
    pub const AGENT_MANAGE: CapSet = CapSet { bits: 1 << 12 };

    pub fn contains(&self, cap: &CapSet) -> bool { (self.bits & cap.bits) == cap.bits }
    pub fn union(&self, other: &CapSet) -> CapSet { CapSet { bits: self.bits | other.bits } }
}
```

### 2.2 能力委托与撤销

```rust
pub struct DelegationRequest {
    pub source_token: CapabilityToken,
    pub target: EntityId,
    pub delegated_caps: CapSet,    // 不能超过原始令牌
    pub purpose: String,
    pub validity: Duration,
}

impl CapabilityManager {
    pub fn delegate(&mut self, req: DelegationRequest) -> Result<CapabilityToken, AuthError> {
        let source = self.tokens.get(&req.source_token.id).ok_or(AuthError::TokenNotFound)?;
        if source.delegation_depth == 0 { return Err(AuthError::DelegationNotAllowed); }
        if !source.capabilities.contains(&req.delegated_caps) { return Err(AuthError::InsufficientCapability); }
        let new_token = CapabilityToken {
            id: TokenId::generate(), holder: req.target, issuer: source.holder.clone(),
            capabilities: req.delegated_caps, created_at: Timestamp::now(),
            expires_at: Some(Timestamp::now() + req.validity),
            delegation_depth: source.delegation_depth - 1, usage_limit: None, signature: Vec::new(),
        };
        self.delegation_log.push(DelegationRecord { /* ... */ });
        self.tokens.insert(new_token.id.clone(), new_token.clone());
        Ok(new_token)
    }

    pub fn revoke(&mut self, token_id: &TokenId) -> Result<(), AuthError> {
        let token = self.tokens.get(token_id).ok_or(AuthError::TokenNotFound)?;
        self.revocation_list.insert(token_id.clone());
        audit_log::record(AuditEvent::TokenRevoked { token_id: token_id.clone(),
            holder: token.holder.clone(), timestamp: Timestamp::now() });
        Ok(())
    }
}
```

---

## 3. 一次性授权 (One-Time Auth)

### 3.1 令牌格式与生命周期

```rust
pub struct OneTimeAuthToken {
    pub id: TokenId,
    pub operation: AuthorizedOperation,
    pub target: ResourceId,
    pub holder: EntityId,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub state: OneTimeState,       // Unused → Used / Expired / Revoked
    pub hmac: [u8; 32],
}

pub enum AuthorizedOperation {
    FileRead { path: String }, FileWrite { path: String },
    NetworkConnect { host: String, port: u16 },
    IpcSend { service: String, method: String },
    AgentSpawn { config_hash: String },
    CloudApiCall { provider: String, model: String },
}
```

生命周期：创建 → [未使用] → 验证 → [已使用] → 归档 / 过期 → [已过期] → 清理 / 撤销 → [已撤销]

### 3.2 安全属性与实现

| 属性 | 保证 | 实现方式 |
|------|------|---------|
| **一次性使用** | 令牌使用后立即失效 | 原子状态转换 + HMAC |
| **不可伪造** | 攻击者无法创建有效令牌 | HMAC 签名 + 密钥隔离 |
| **不可重放** | 已使用的令牌无法再次使用 | 状态记录 + 时序检查 |
| **时效性** | 过期令牌自动失效 | 时间戳验证 |

```rust
impl OneTimeAuthManager {
    pub fn verify_and_consume(&mut self, token: &OneTimeAuthToken) -> Result<AuthorizationDecision, AuthError> {
        if !self.verify_signature(token)? { return Err(AuthError::InvalidSignature); }
        if Timestamp::now() > token.expires_at { return Err(AuthError::TokenExpired); }
        let stored = self.tokens.get(&token.id).ok_or(AuthError::TokenNotFound)?;
        if stored.state != OneTimeState::Unused { return Err(AuthError::TokenAlreadyUsed); }
        let now = Timestamp::now();
        self.tokens.get_mut(&token.id).unwrap().state = OneTimeState::Used { used_at: now };
        audit_log::record(AuditEvent::OneTimeTokenUsed { /* ... */ });
        Ok(AuthorizationDecision::Allow)
    }
}
```

---

## 4. 永久授权 (Permanent Auth)

```rust
pub struct PermanentGrant {
    pub id: GrantId, pub grantee: EntityId, pub grantor: EntityId,
    pub capabilities: CapSet, pub purpose: String,
    pub conditions: Vec<AuthCondition>,
    pub created_at: Timestamp, pub modified_at: Timestamp,
    pub status: GrantStatus,  // Active / Suspended / Revoked
}

pub enum AuthCondition {
    TimeWindow { start: Timestamp, end: Timestamp },
    NetworkLocation { allowed_subnets: Vec<IpNetwork> },
    RateLimit { max_per_minute: u32 },
    ResourceQuota { max_memory_mb: u32, max_cpu_percent: u8 },
    RequiresUserConsent { consent_prompt: String },
}

impl PermanentAuthManager {
    pub fn revoke(&mut self, grant_id: &GrantId, reason: &str, revoker: &EntityId) -> Result<(), AuthError> {
        let grant = self.grants.get_mut(grant_id).ok_or(AuthError::GrantNotFound)?;
        if grant.grantor != *revoker { return Err(AuthError::NotAuthorized); }
        grant.status = GrantStatus::Revoked { reason: reason.to_string(), at: Timestamp::now() };
        self.notify_revocation(grant_id);
        audit_log::record(AuditEvent::GrantRevoked { /* ... */ });
        Ok(())
    }
}
```

---

## 5. 策略引擎

### 5.1 PBAC + RBAC

```rust
pub struct PurposePolicy {
    pub id: PolicyId, pub name: String, pub purpose: String,
    pub allow_rules: Vec<PolicyRule>, pub deny_rules: Vec<PolicyRule>,
    pub priority: u32, pub enabled: bool,
}

pub struct PolicyRule {
    pub id: RuleId, pub conditions: Vec<RuleCondition>,
    pub effect: Effect,  // Allow / Deny
}

pub enum RuleCondition {
    EntityType(EntityType), EntityEquals(EntityId),
    HasCapability(CapSet), ResourceType(String), ResourceEquals(ResourceId),
    OperationEquals(String), TimeBetween(Timestamp, Timestamp),
    Custom { name: String, params: HashMap<String, String> },
}

pub struct Role {
    pub id: RoleId, pub name: String, pub parent: Option<RoleId>,
    pub capabilities: CapSet, pub policies: Vec<PolicyId>,
}

impl RoleHierarchy {
    pub fn system_roles() -> Self {
        // superadmin: CapSet::all()
        // agent_admin: AGENT_MANAGE | PROCESS_SPAWN | IPC
        // user: FILE_READ | FILE_WRITE | NETWORK | GPU_ACCESS | AUDIO_ACCESS
        // restricted_agent: IPC
        /* ... */
    }
}
```

### 5.2 策略评估算法

```rust
impl PolicyEngine {
    pub fn evaluate(&self, request: &AuthRequest) -> AuthorizationDecision {
        // 第一步: 检查能力令牌有效性
        if let Some(token) = &request.capability_token {
            if !self.validate_token(token) { return AuthorizationDecision::Deny { reason: "无效令牌".into() }; }
        }
        // 第二步: 收集适用策略 (按优先级排序)
        let mut policies: Vec<_> = self.policies.iter()
            .filter(|p| p.enabled && self.matches_purpose(p, &request.purpose))
            .collect();
        policies.sort_by(|a, b| b.priority.cmp(&a.priority));
        // 第三步: 拒绝优先评估
        for policy in &policies {
            for rule in &policy.deny_rules {
                if self.matches_rule(rule, request) { return AuthorizationDecision::Deny { /* ... */ }; }
            }
            for rule in &policy.allow_rules {
                if self.matches_rule(rule, request) { return AuthorizationDecision::Allow; }
            }
        }
        // 第四步: 默认拒绝
        AuthorizationDecision::Deny { reason: "默认拒绝".into() }
    }
}
```

---

## 6. 授权流程示例

### 6.1 Agent 访问文件

```
Agent → [IPC + 能力令牌] → 内核 → 授权服务 → 策略引擎 → 允许 → 文件服务 → 返回数据
```

### 6.2 用户授权 Agent 操作

```
1. Agent 检测到需要 "文件写入" 能力
2. Agent 通过授权服务请求一次性授权
3. 系统向用户展示同意 UI (系统窗口, always_on_top, 禁止截图)
4. 用户点击 "允许" (验证物理输入, 响应时间 > 200ms)
5. 授权服务创建一次性令牌
6. Agent 使用令牌执行操作
7. 令牌自动失效
```

---

## 7. 内核能力集成

```rust
pub fn syscall_check_capability(process: &Process, required_cap: CapSet) -> Result<(), SyscallError> {
    let caps = process.effective_capabilities();
    if !caps.contains(&required_cap) {
        audit_log_kernel(AuditEvent::CapabilityCheckFailed { /* ... */ });
        return Err(SyscallError::CapabilityMissing(required_cap));
    }
    Ok(())
}

pub fn handle_syscall(process: &Process, call: Syscall) -> SyscallResult {
    match call {
        Syscall::Open(path, flags) => {
            let required = if flags.contains(OpenFlags::WRITE) { CapSet::FILE_WRITE } else { CapSet::FILE_READ };
            syscall_check_capability(process, required)?;
        }
        Syscall::Socket(..) => syscall_check_capability(process, CapSet::NETWORK)?,
        Syscall::SpawnProcess(..) => syscall_check_capability(process, CapSet::PROCESS_SPAWN)?,
        // ...
    }
}
```

---

## 8. 授权缓存与失效

```rust
pub struct AuthCache {
    entries: HashMap<AuthRequestFingerprint, CacheEntry>,
    max_entries: usize, default_ttl: Duration,
}

impl AuthCache {
    pub fn get(&self, request: &AuthRequest) -> Option<&AuthorizationDecision> {
        let fp = AuthRequestFingerprint::from(request);
        self.entries.get(&fp).filter(|e| Timestamp::now() < e.expires_at).map(|e| &e.decision)
    }

    pub fn invalidate_for_token(&mut self, token_id: &TokenId) {
        self.entries.retain(|_, e| !matches!(&e.decision, AuthorizationDecision::Allow { token: Some(t) } if t == *token_id));
    }
}
```

| 事件 | 失效范围 | 延迟 |
|------|---------|------|
| 令牌撤销 | 该令牌相关缓存 | 即时 |
| 策略更新 | 该策略相关缓存 | 即时 |
| 角色变更 | 该角色相关缓存 | 即时 |
| TTL 过期 | 单个缓存条目 | 自动 |

---

## 9. 同意 UI 安全

```rust
pub struct ConsentDialog {
    pub requester: EntityId, pub requested_caps: CapSet,
    pub purpose: String, pub security_flags: SecurityFlags,
}

impl ConsentDialog {
    pub fn render_secure(&self, compositor: &Compositor) -> Window {
        let mut window = Window::new(WindowType::SystemConsent);
        window.set_security_attributes(WindowSecurityAttributes {
            always_on_top: true,           // 不可被其他窗口覆盖
            screenshot_blocked: true,      // 禁止截图
            input_injection_blocked: true, // 禁止输入注入
            show_requester_identity: true, // 显示来源标识
            requires_user_interaction: true, // 强制用户交互
        });
        window
    }

    pub fn verify_user_response(&self, response: &UserResponse) -> ConsentResult {
        if !response.is_from_physical_input() { return ConsentResult::Rejected { reason: "非物理输入".into() }; }
        if !self.verify_dialog_integrity() { return ConsentResult::Rejected { reason: "完整性验证失败".into() }; }
        if response.response_time() < Duration::from_millis(200) { return ConsentResult::Rejected { reason: "响应时间异常".into() }; }
        match response.choice() { Choice::Allow => ConsentResult::Allowed, Choice::Deny => ConsentResult::Denied }
    }
}
```

---

## 10. 性能优化

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 单次授权评估 | < 100 us | 缓存命中 |
| 单次授权评估 | < 1 ms | 缓存未命中 |
| 批量评估 (100 项) | < 5 ms | 并行评估 |
| 缓存命中率 | > 95% | 正常运行 |
| 缓存失效传播 | < 10 ms | 跨服务 |

---

## 11. 授权测试方法

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_capability_delegation_restriction() {
        let manager = CapabilityManager::new();
        let token = manager.create_token(EntityId::new("agent-1"), CapSet::FILE_READ, Duration::from_secs(3600)).unwrap();
        let result = manager.delegate(DelegationRequest {
            source_token: token, target: EntityId::new("agent-2"),
            delegated_caps: CapSet::FILE_READ | CapSet::NETWORK, // 超出范围
            purpose: "test".into(), validity: Duration::from_secs(60),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_policy_default_deny() {
        let engine = PolicyEngine::new_with_default_deny();
        let decision = engine.evaluate(&AuthRequest::new(EntityId::new("unknown"), "file:read".into(), ResourceId::new("/etc/passwd")));
        assert!(matches!(decision, AuthorizationDecision::Deny { .. }));
    }

    #[test]
    fn test_one_time_token_single_use() {
        let manager = OneTimeAuthManager::new();
        let token = manager.create_token(AuthorizedOperation::FileRead { path: "/tmp/test".into() },
            ResourceId::new("/tmp/test"), EntityId::new("agent-1"), Duration::from_secs(60)).unwrap();
        assert!(manager.verify_and_consume(&token).is_ok());
        assert!(manager.verify_and_consume(&token).is_err()); // 第二次失败
    }

    #[test]
    fn test_role_capability_inheritance() {
        let hierarchy = RoleHierarchy::system_roles();
        let caps = hierarchy.get_effective_capabilities(&RoleId::new("agent_admin"));
        assert!(caps.contains(&CapSet::AGENT_MANAGE));
        assert!(!caps.contains(&CapSet::ADMIN));
    }
}
```
