# OmniAgent OS 授权设计规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: 正式发布
> **责任团队**: 安全工程与架构组

---

## 1. 授权架构概述

### 1.1 设计目标

OmniAgent OS 的授权系统基于以下核心原则：

| 原则 | 描述 |
|------|------|
| **最小权限** | 每个实体仅获得完成其任务所需的最小权限集 |
| **默认拒绝** | 未明确授权的操作默认被拒绝 |
| **权限可撤销** | 所有授权都可以被撤销，撤销即时生效 |
| **权限可委托** | 权限可以在受控条件下从一个实体委托给另一个 |
| **权限可审计** | 所有授权决策和操作都有完整的审计记录 |
| **意图驱动** | 授权基于操作的目的 (Purpose)，而非仅基于角色 |

### 1.2 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                      授权决策流程                             │
│                                                             │
│  ┌──────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │ 请求者    │ →  │  授权检查     │ →  │  策略引擎        │  │
│  │ (Agent/  │    │  (内核 +     │    │  (PBAC + RBAC)   │  │
│  │  Service)│    │   服务层)     │    │                  │  │
│  └──────────┘    └──────┬───────┘    └────────┬─────────┘  │
│                         │                      │            │
│                  ┌──────┴───────┐    ┌────────┴─────────┐  │
│                  │ 能力令牌验证  │    │ 策略存储          │  │
│                  │ (Capability) │    │ (Policy Store)   │  │
│                  └──────────────┘    └──────────────────┘  │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ 一次性授权    │  │ 永久授权      │  │ 审计日志          │  │
│  │ (One-Time)   │  │ (Permanent)  │  │ (Audit Log)     │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 授权模型层次

```
┌─────────────────────────────────────────┐
│          L4: 审计与合规                   │  ← 操作记录、合规报告
├─────────────────────────────────────────┤
│          L3: 策略引擎 (PBAC + RBAC)      │  ← 策略评估、决策
├─────────────────────────────────────────┤
│          L2: 能力系统 (Capability)        │  ← 令牌管理、委托
├─────────────────────────────────────────┤
│          L1: 内核强制 (Kernel Enforce)    │  ← 硬件级权限检查
└─────────────────────────────────────────┘
```

---

## 2. 能力令牌系统 (Capability-Based Access Control)

### 2.1 能力令牌格式

```rust
/// 能力令牌 - 不可伪造的权限证明
#[derive(Debug, Clone)]
pub struct CapabilityToken {
    /// 令牌唯一标识
    pub id: TokenId,
    /// 令牌持有者
    pub holder: EntityId,
    /// 令牌签发者
    pub issuer: EntityId,
    /// 授权的能力集合
    pub capabilities: CapSet,
    /// 令牌创建时间
    pub created_at: Timestamp,
    /// 令牌过期时间
    pub expires_at: Option<Timestamp>,
    /// 委托深度限制 (0 = 不可委托)
    pub delegation_depth: u8,
    /// 使用次数限制 (None = 无限制)
    pub usage_limit: Option<u64>,
    /// 签名 (防止伪造)
    pub signature: Vec<u8>,
}

/// 能力集合
#[derive(Debug, Clone)]
pub struct CapSet {
    bits: u128,  // 最多支持 128 种能力
}

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

    pub fn contains(&self, cap: &CapSet) -> bool {
        (self.bits & cap.bits) == cap.bits
    }

    pub fn union(&self, other: &CapSet) -> CapSet {
        CapSet { bits: self.bits | other.bits }
    }

    pub fn intersect(&self, other: &CapSet) -> CapSet {
        CapSet { bits: self.bits & other.bits }
    }
}
```

### 2.2 能力委托

```rust
/// 委托请求
pub struct DelegationRequest {
    /// 原始令牌
    pub source_token: CapabilityToken,
    /// 目标实体
    pub target: EntityId,
    /// 委托的能力子集 (不能超过原始令牌)
    pub delegated_caps: CapSet,
    /// 委托用途说明
    pub purpose: String,
    /// 委托有效期
    pub validity: Duration,
}

/// 能力管理器
pub struct CapabilityManager {
    tokens: HashMap<TokenId, CapabilityToken>,
    delegation_log: Vec<DelegationRecord>,
    revocation_list: HashSet<TokenId>,
}

impl CapabilityManager {
    /// 委托能力给另一个实体
    pub fn delegate(&mut self, request: DelegationRequest) -> Result<CapabilityToken, AuthError> {
        // 验证源令牌有效性
        let source = self.tokens.get(&request.source_token.id)
            .ok_or(AuthError::TokenNotFound)?;

        // 验证源令牌未过期
        if let Some(expires) = source.expires_at {
            if Timestamp::now() > expires {
                return Err(AuthError::TokenExpired);
            }
        }

        // 验证委托深度
        if source.delegation_depth == 0 {
            return Err(AuthError::DelegationNotAllowed);
        }

        // 验证委托的能力不超过源令牌
        if !source.capabilities.contains(&request.delegated_caps) {
            return Err(AuthError::InsufficientCapability);
        }

        // 创建新令牌
        let new_token = CapabilityToken {
            id: TokenId::generate(),
            holder: request.target,
            issuer: source.holder.clone(),
            capabilities: request.delegated_caps,
            created_at: Timestamp::now(),
            expires_at: Some(Timestamp::now() + request.validity),
            delegation_depth: source.delegation_depth - 1,
            usage_limit: None,
            signature: Vec::new(), // 由签发者签名
        };

        // 记录委托操作
        self.delegation_log.push(DelegationRecord {
            source_token: source.id.clone(),
            target_token: new_token.id.clone(),
            from: source.holder.clone(),
            to: request.target,
            capabilities: request.delegated_caps,
            purpose: request.purpose,
            timestamp: Timestamp::now(),
        });

        self.tokens.insert(new_token.id.clone(), new_token.clone());
        Ok(new_token)
    }

    /// 撤销令牌
    pub fn revoke(&mut self, token_id: &TokenId) -> Result<(), AuthError> {
        // 验证撤销权限 (只有签发者或管理员可以撤销)
        let token = self.tokens.get(token_id)
            .ok_or(AuthError::TokenNotFound)?;

        // 添加到撤销列表
        self.revocation_list.insert(token_id.clone());

        // 记录撤销操作到审计日志
        audit_log::record(AuditEvent::TokenRevoked {
            token_id: token_id.clone(),
            holder: token.holder.clone(),
            timestamp: Timestamp::now(),
        });

        Ok(())
    }
}
```

### 2.3 能力撤销

```rust
impl CapabilityManager {
    /// 检查令牌是否有效 (未被撤销)
    pub fn is_valid(&self, token_id: &TokenId) -> bool {
        if self.revocation_list.contains(token_id) {
            return false;
        }
        if let Some(token) = self.tokens.get(token_id) {
            if let Some(expires) = token.expires_at {
                if Timestamp::now() > expires {
                    return false;
                }
            }
            return true;
        }
        false
    }

    /// 批量撤销某实体持有的所有令牌
    pub fn revoke_all_for_entity(&mut self, entity: &EntityId) {
        let to_revoke: Vec<TokenId> = self.tokens.iter()
            .filter(|(_, t)| &t.holder == entity)
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_revoke {
            self.revocation_list.insert(id);
        }
    }
}
```

---

## 3. 一次性授权 (One-Time Auth)

### 3.1 令牌格式

```rust
/// 一次性授权令牌
pub struct OneTimeAuthToken {
    /// 令牌 ID
    pub id: TokenId,
    /// 授权的操作
    pub operation: AuthorizedOperation,
    /// 操作目标
    pub target: ResourceId,
    /// 授权持有者
    pub holder: EntityId,
    /// 令牌创建时间
    pub created_at: Timestamp,
    /// 令牌过期时间
    pub expires_at: Timestamp,
    /// 使用状态
    pub state: OneTimeState,
    /// HMAC 签名
    pub hmac: [u8; 32],
}

#[derive(Debug, Clone, PartialEq)]
pub enum OneTimeState {
    /// 未使用
    Unused,
    /// 已使用
    Used { used_at: Timestamp },
    /// 已过期
    Expired,
    /// 已撤销
    Revoked { reason: String },
}

/// 授权操作定义
#[derive(Debug, Clone)]
pub enum AuthorizedOperation {
    /// 文件操作
    FileRead { path: String },
    FileWrite { path: String },
    /// 网络操作
    NetworkConnect { host: String, port: u16 },
    /// IPC 操作
    IpcSend { service: String, method: String },
    /// Agent 操作
    AgentSpawn { config_hash: String },
    /// 云 API 操作
    CloudApiCall { provider: String, model: String },
}
```

### 3.2 令牌生命周期

```
创建 → [未使用] → 验证 → [已使用] → 归档
  │                  │
  │                  └→ 验证失败 → [未使用] (重试)
  │
  └→ 过期 → [已过期] → 清理
  │
  └→ 撤销 → [已撤销] → 审计记录
```

### 3.3 安全属性

| 属性 | 保证 | 实现方式 |
|------|------|---------|
| **一次性使用** | 令牌使用后立即失效 | 原子状态转换 + HMAC |
| **不可伪造** | 攻击者无法创建有效令牌 | HMAC 签名 + 密钥隔离 |
| **不可重放** | 已使用的令牌无法再次使用 | 状态记录 + 时序检查 |
| **时效性** | 过期令牌自动失效 | 时间戳验证 |
| **可追溯** | 每次使用都有审计记录 | 审计日志集成 |

### 3.4 一次性授权实现

```rust
pub struct OneTimeAuthManager {
    tokens: HashMap<TokenId, OneTimeAuthToken>,
    signing_key: HmacKey, // 存储在安全飞地中
}

impl OneTimeAuthManager {
    /// 创建一次性授权令牌
    pub fn create_token(
        &self,
        operation: AuthorizedOperation,
        target: ResourceId,
        holder: EntityId,
        validity: Duration,
    ) -> Result<OneTimeAuthToken, AuthError> {
        let token = OneTimeAuthToken {
            id: TokenId::generate(),
            operation,
            target,
            holder,
            created_at: Timestamp::now(),
            expires_at: Timestamp::now() + validity,
            state: OneTimeState::Unused,
            hmac: [0u8; 32], // 稍后计算
        };

        // 在安全飞地中签名
        let hmac = self.sign_in_enclave(&token)?;
        let token = OneTimeAuthToken { hmac, ..token };

        audit_log::record(AuditEvent::OneTimeTokenCreated {
            token_id: token.id.clone(),
            holder: token.holder.clone(),
            operation: format!("{:?}", token.operation),
        });

        Ok(token)
    }

    /// 验证并消费一次性令牌
    pub fn verify_and_consume(
        &mut self,
        token: &OneTimeAuthToken,
    ) -> Result<AuthorizationDecision, AuthError> {
        // 验证签名
        if !self.verify_signature(token)? {
            return Err(AuthError::InvalidSignature);
        }

        // 检查过期
        if Timestamp::now() > token.expires_at {
            return Err(AuthError::TokenExpired);
        }

        // 检查是否已使用
        let stored = self.tokens.get(&token.id)
            .ok_or(AuthError::TokenNotFound)?;

        if stored.state != OneTimeState::Unused {
            return Err(AuthError::TokenAlreadyUsed);
        }

        // 原子性状态转换
        let now = Timestamp::now();
        self.tokens.get_mut(&token.id).unwrap().state =
            OneTimeState::Used { used_at: now };

        audit_log::record(AuditEvent::OneTimeTokenUsed {
            token_id: token.id.clone(),
            holder: token.holder.clone(),
            used_at: now,
        });

        Ok(AuthorizationDecision::Allow)
    }
}
```

---

## 4. 永久授权 (Permanent Auth)

### 4.1 授权格式

```rust
/// 永久授权记录
pub struct PermanentGrant {
    /// 授权 ID
    pub id: GrantId,
    /// 被授权实体
    pub grantee: EntityId,
    /// 授权者
    pub grantor: EntityId,
    /// 授权的能力
    pub capabilities: CapSet,
    /// 授权目的
    pub purpose: String,
    /// 授权条件
    pub conditions: Vec<AuthCondition>,
    /// 创建时间
    pub created_at: Timestamp,
    /// 最后修改时间
    pub modified_at: Timestamp,
    /// 状态
    pub status: GrantStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GrantStatus {
    Active,
    Suspended { reason: String },
    Revoked { reason: String, at: Timestamp },
}

/// 授权条件
#[derive(Debug, Clone)]
pub enum AuthCondition {
    /// 时间窗口限制
    TimeWindow { start: Timestamp, end: Timestamp },
    /// 网络位置限制
    NetworkLocation { allowed_subnets: Vec<IpNetwork> },
    /// 使用频率限制
    RateLimit { max_per_minute: u32 },
    /// 资源配额
    ResourceQuota { max_memory_mb: u32, max_cpu_percent: u8 },
    /// 需要用户确认
    RequiresUserConsent { consent_prompt: String },
}
```

### 4.2 授权撤销机制

```rust
pub struct PermanentAuthManager {
    grants: HashMap<GrantId, PermanentGrant>,
    revocation_log: Vec<RevocationRecord>,
}

impl PermanentAuthManager {
    /// 撤销永久授权
    pub fn revoke(
        &mut self,
        grant_id: &GrantId,
        reason: &str,
        revoker: &EntityId,
    ) -> Result<(), AuthError> {
        let grant = self.grants.get_mut(grant_id)
            .ok_or(AuthError::GrantNotFound)?;

        // 验证撤销权限 (只有授权者或管理员可以撤销)
        if grant.grantor != *revoker {
            return Err(AuthError::NotAuthorized);
        }

        let now = Timestamp::now();
        grant.status = GrantStatus::Revoked {
            reason: reason.to_string(),
            at: now,
        };
        grant.modified_at = now;

        // 记录撤销
        self.revocation_log.push(RevocationRecord {
            grant_id: grant_id.clone(),
            reason: reason.to_string(),
            revoked_by: revoker.clone(),
            revoked_at: now,
        });

        // 通知相关服务
        self.notify_revocation(grant_id);

        // 审计记录
        audit_log::record(AuditEvent::GrantRevoked {
            grant_id: grant_id.clone(),
            grantee: grant.grantee.clone(),
            reason: reason.to_string(),
            revoked_by: revoker.clone(),
        });

        Ok(())
    }

    /// 检查授权是否有效
    pub fn is_grant_active(&self, grant_id: &GrantId) -> bool {
        if let Some(grant) = self.grants.get(grant_id) {
            if grant.status != GrantStatus::Active {
                return false;
            }
            // 检查条件是否满足
            for condition in &grant.conditions {
                if !self.evaluate_condition(condition) {
                    return false;
                }
            }
            return true;
        }
        false
    }
}
```

---

## 5. 策略引擎

### 5.1 PBAC (Purpose-Based Access Control)

```rust
/// 基于目的的访问控制策略
pub struct PurposePolicy {
    /// 策略 ID
    pub id: PolicyId,
    /// 策略名称
    pub name: String,
    /// 适用目的
    pub purpose: String,
    /// 允许的操作
    pub allow_rules: Vec<PolicyRule>,
    /// 拒绝的操作
    pub deny_rules: Vec<PolicyRule>,
    /// 策略优先级 (数值越大优先级越高)
    pub priority: u32,
    /// 策略状态
    pub enabled: bool,
}

/// 策略规则
#[derive(Debug, Clone)]
pub struct PolicyRule {
    /// 规则 ID
    pub id: RuleId,
    /// 匹配条件
    pub conditions: Vec<RuleCondition>,
    /// 操作 (允许/拒绝)
    pub effect: Effect,
    /// 规则描述
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Allow,
    Deny,
}

/// 规则条件
#[derive(Debug, Clone)]
pub enum RuleCondition {
    /// 匹配实体类型
    EntityType(EntityType),
    /// 匹配特定实体
    EntityEquals(EntityId),
    /// 匹配能力
    HasCapability(CapSet),
    /// 匹配资源类型
    ResourceType(String),
    /// 匹配资源 ID
    ResourceEquals(ResourceId),
    /// 匹配操作类型
    OperationEquals(String),
    /// 时间条件
    TimeBetween(Timestamp, Timestamp),
    /// 自定义条件
    Custom { name: String, params: HashMap<String, String> },
}
```

### 5.2 RBAC (Role-Based Access Control)

```rust
/// 角色定义
pub struct Role {
    pub id: RoleId,
    pub name: String,
    pub description: String,
    /// 角色继承的父角色
    pub parent: Option<RoleId>,
    /// 角色包含的能力
    pub capabilities: CapSet,
    /// 角色包含的策略
    pub policies: Vec<PolicyId>,
}

/// 角色层次结构
pub struct RoleHierarchy {
    roles: HashMap<RoleId, Role>,
}

impl RoleHierarchy {
    /// 获取角色的所有能力 (包括继承的)
    pub fn get_effective_capabilities(&self, role_id: &RoleId) -> CapSet {
        let mut caps = CapSet::empty();
        let mut current = role_id.clone();

        while let Some(role) = self.roles.get(&current) {
            caps = caps.union(&role.capabilities);
            if let Some(parent) = &role.parent {
                current = parent.clone();
            } else {
                break;
            }
        }

        caps
    }

    /// 预定义角色
    pub fn system_roles() -> Self {
        let mut hierarchy = Self { roles: HashMap::new() };

        // 超级管理员
        hierarchy.roles.insert(RoleId::new("superadmin"), Role {
            id: RoleId::new("superadmin"),
            name: "超级管理员",
            description: "拥有系统所有权限",
            parent: None,
            capabilities: CapSet::all(),
            policies: vec![],
        });

        // Agent 管理员
        hierarchy.roles.insert(RoleId::new("agent_admin"), Role {
            id: RoleId::new("agent_admin"),
            name: "Agent 管理员",
            description: "管理 Agent 生命周期和配置",
            parent: None,
            capabilities: CapSet::AGENT_MANAGE
                | CapSet::PROCESS_SPAWN
                | CapSet::IPC,
            policies: vec![],
        });

        // 普通用户
        hierarchy.roles.insert(RoleId::new("user"), Role {
            id: RoleId::new("user"),
            name: "普通用户",
            description: "标准用户权限",
            parent: None,
            capabilities: CapSet::FILE_READ
                | CapSet::FILE_WRITE
                | CapSet::NETWORK
                | CapSet::GPU_ACCESS
                | CapSet::AUDIO_ACCESS,
            policies: vec![],
        });

        // 受限 Agent
        hierarchy.roles.insert(RoleId::new("restricted_agent"), Role {
            id: RoleId::new("restricted_agent"),
            name: "受限 Agent",
            description: "最小权限 Agent",
            parent: None,
            capabilities: CapSet::IPC,
            policies: vec![],
        });

        hierarchy
    }
}
```

### 5.3 策略评估算法

```rust
pub struct PolicyEngine {
    policies: Vec<PurposePolicy>,
    role_hierarchy: RoleHierarchy,
    default_decision: AuthorizationDecision,
}

impl PolicyEngine {
    /// 评估授权请求
    pub fn evaluate(&self, request: &AuthRequest) -> AuthorizationDecision {
        // 第一步: 检查能力令牌
        if let Some(token) = &request.capability_token {
            if !self.validate_token(token) {
                audit_log::record(AuditEvent::AuthDenied {
                    reason: "无效的能力令牌",
                    request: request.clone(),
                });
                return AuthorizationDecision::Deny {
                    reason: "无效的能力令牌".to_string(),
                };
            }
        }

        // 第二步: 收集所有适用的策略 (按优先级排序)
        let mut applicable_policies: Vec<&PurposePolicy> = self.policies.iter()
            .filter(|p| p.enabled && self.matches_purpose(p, &request.purpose))
            .collect();
        applicable_policies.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 第三步: 评估策略规则
        let mut final_decision = self.default_decision.clone();

        for policy in &applicable_policies {
            // 先检查拒绝规则 (拒绝优先)
            for rule in &policy.deny_rules {
                if self.matches_rule(rule, request) {
                    audit_log::record(AuditEvent::AuthDenied {
                        reason: format!("策略 {} 拒绝", policy.id),
                        request: request.clone(),
                    });
                    return AuthorizationDecision::Deny {
                        reason: format!("被策略 {} 拒绝", policy.name),
                    };
                }
            }

            // 再检查允许规则
            for rule in &policy.allow_rules {
                if self.matches_rule(rule, request) {
                    final_decision = AuthorizationDecision::Allow;
                }
            }
        }

        // 第四步: 默认拒绝
        if matches!(final_decision, AuthorizationDecision::Deny { .. }) {
            audit_log::record(AuditEvent::AuthDenied {
                reason: "默认拒绝",
                request: request.clone(),
            });
        } else {
            audit_log::record(AuditEvent::AuthAllowed {
                request: request.clone(),
            });
        }

        final_decision
    }

    /// 匹配策略规则
    fn matches_rule(&self, rule: &PolicyRule, request: &AuthRequest) -> bool {
        rule.conditions.iter().all(|cond| {
            match cond {
                RuleCondition::EntityType(et) => request.actor.entity_type() == *et,
                RuleCondition::HasCapability(caps) => {
                    request.actor_capabilities().contains(caps)
                }
                RuleCondition::ResourceType(rt) => request.resource_type() == rt,
                RuleCondition::OperationEquals(op) => request.operation() == op,
                RuleCondition::TimeBetween(start, end) => {
                    let now = Timestamp::now();
                    now >= *start && now <= *end
                }
                _ => true, // 其他条件默认匹配
            }
        })
    }
}
```

---

## 6. 授权流程示例

### 6.1 Agent 访问文件

```
┌────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌────────┐
│ Agent  │     │ 内核     │     │ 授权服务  │     │ 策略引擎  │     │ 文件   │
│        │     │ (IPC)    │     │          │     │          │     │ 服务   │
└───┬────┘     └────┬─────┘     └────┬─────┘     └────┬─────┘     └───┬────┘
    │               │                │                │               │
    │ 1. 请求读取文件 │               │                │               │
    │ (带能力令牌)   │               │                │               │
    │──────────────→│               │                │               │
    │               │ 2. 转发授权请求 │                │               │
    │               │──────────────→│                │               │
    │               │               │ 3. 评估策略    │               │
    │               │               │──────────────→│               │
    │               │               │                │               │
    │               │               │ 4. 决策: 允许  │               │
    │               │               │←──────────────│               │
    │               │ 5. 授权通过    │                │               │
    │               │←──────────────│                │               │
    │               │ 6. 转发文件请求 │                │               │
    │               │──────────────────────────────────────────────→│
    │               │               │                │ 7. 文件内容   │
    │               │←──────────────────────────────────────────────│
    │ 8. 返回文件内容│               │                │               │
    │←──────────────│               │                │               │
    │               │               │                │               │
```

### 6.2 用户授权 Agent 操作

```
用户请求 Agent 执行需要额外权限的操作:

1. Agent 检测到需要 "文件写入" 能力
2. Agent 通过授权服务请求一次性授权
3. 系统向用户展示同意 UI:
   ┌─────────────────────────────────────┐
   │  Agent "助手" 请求以下权限:           │
   │                                     │
   │  [!] 写入文件: ~/documents/report.md │
   │                                     │
   │  目的: 保存分析报告                   │
   │  有效期: 本次操作                     │
   │                                     │
   │  [拒绝]              [允许]          │
   └─────────────────────────────────────┘
4. 用户点击 "允许"
5. 授权服务创建一次性令牌
6. Agent 使用令牌执行操作
7. 令牌自动失效
```

---

## 7. 内核能力集成

### 7.1 内核级能力检查

```rust
// kernel/src/auth/capability.rs

/// 内核能力检查 - 在系统调用处理路径中
pub fn syscall_check_capability(
    process: &Process,
    required_cap: CapSet,
) -> Result<(), SyscallError> {
    let caps = process.effective_capabilities();

    if !caps.contains(&required_cap) {
        audit_log_kernel(AuditEvent::CapabilityCheckFailed {
            process_id: process.pid(),
            required: required_cap,
            actual: caps,
        });
        return Err(SyscallError::CapabilityMissing(required_cap));
    }

    Ok(())
}

/// 系统调用分发中的能力检查
pub fn handle_syscall(process: &Process, call: Syscall) -> SyscallResult {
    match call {
        Syscall::Open(path, flags) => {
            let required = if flags.contains(OpenFlags::WRITE) {
                CapSet::FILE_WRITE
            } else {
                CapSet::FILE_READ
            };
            syscall_check_capability(process, required)?;
            // ... 执行文件打开
        }
        Syscall::Socket(domain, sock_type) => {
            syscall_check_capability(process, CapSet::NETWORK)?;
            // ... 执行 socket 创建
        }
        Syscall::SpawnProcess(config) => {
            syscall_check_capability(process, CapSet::PROCESS_SPAWN)?;
            // ... 执行进程创建
        }
        // ... 其他系统调用
    }
}
```

---

## 8. 授权缓存与失效

### 8.1 缓存策略

```rust
/// 授权决策缓存
pub struct AuthCache {
    /// 缓存条目: (请求指纹, 决策, 过期时间)
    entries: HashMap<AuthRequestFingerprint, CacheEntry>,
    /// 最大缓存条目数
    max_entries: usize,
    /// 默认缓存时间
    default_ttl: Duration,
}

#[derive(Debug)]
struct CacheEntry {
    decision: AuthorizationDecision,
    expires_at: Timestamp,
    hit_count: u64,
}

impl AuthCache {
    /// 查询缓存
    pub fn get(&self, request: &AuthRequest) -> Option<&AuthorizationDecision> {
        let fingerprint = AuthRequestFingerprint::from(request);
        if let Some(entry) = self.entries.get(&fingerprint) {
            if Timestamp::now() < entry.expires_at {
                return Some(&entry.decision);
            }
        }
        None
    }

    /// 缓存授权决策
    pub fn insert(&mut self, request: &AuthRequest, decision: &AuthorizationDecision) {
        let fingerprint = AuthRequestFingerprint::from(request);
        let entry = CacheEntry {
            decision: decision.clone(),
            expires_at: Timestamp::now() + self.default_ttl,
            hit_count: 0,
        };

        // LRU 淘汰
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        self.entries.insert(fingerprint, entry);
    }

    /// 令牌撤销时使缓存失效
    pub fn invalidate_for_token(&mut self, token_id: &TokenId) {
        self.entries.retain(|_, entry| {
            !matches!(entry.decision, AuthorizationDecision::Allow { token: Some(t) } if t == *token_id)
        });
    }
}
```

### 8.2 缓存失效策略

| 事件 | 失效范围 | 延迟 |
|------|---------|------|
| 令牌撤销 | 该令牌相关的所有缓存条目 | 即时 |
| 策略更新 | 该策略相关的所有缓存条目 | 即时 |
| 角色变更 | 该角色相关的所有缓存条目 | 即时 |
| 授权撤销 | 该授权相关的所有缓存条目 | 即时 |
| TTL 过期 | 单个缓存条目 | 自动 |

---

## 9. 同意 UI 安全

### 9.1 防点击劫持

```rust
/// 安全同意对话框
pub struct ConsentDialog {
    /// 请求来源
    pub requester: EntityId,
    /// 请求的权限
    pub requested_caps: CapSet,
    /// 请求目的
    pub purpose: String,
    /// 安全验证标志
    pub security_flags: SecurityFlags,
}

impl ConsentDialog {
    /// 渲染安全的同意对话框
    pub fn render_secure(&self, compositor: &Compositor) -> Window {
        let mut window = Window::new(WindowType::SystemConsent);

        // 设置安全属性
        window.set_security_attributes(WindowSecurityAttributes {
            // 标记为系统窗口，不可被其他窗口覆盖
            always_on_top: true,
            // 禁止截图
            screenshot_blocked: true,
            // 禁止输入注入
            input_injection_blocked: true,
            // 显示来源标识
            show_requester_identity: true,
            // 强制用户交互 (不可通过 API 自动关闭)
            requires_user_interaction: true,
        });

        // 渲染对话框内容
        window.set_content(self.build_dialog_content());

        window
    }

    /// 验证用户响应的真实性
    pub fn verify_user_response(&self, response: &UserResponse) -> ConsentResult {
        // 验证响应来自真实用户交互
        if !response.is_from_physical_input() {
            return ConsentResult::Rejected {
                reason: "响应非物理输入".to_string(),
            };
        }

        // 验证对话框未被篡改
        if !self.verify_dialog_integrity() {
            return ConsentResult::Rejected {
                reason: "对话框完整性验证失败".to_string(),
            };
        }

        // 验证响应时间合理 (防止自动化)
        if response.response_time() < Duration::from_millis(200) {
            return ConsentResult::Rejected {
                reason: "响应时间异常".to_string(),
            };
        }

        match response.choice() {
            Choice::Allow => ConsentResult::Allowed,
            Choice::Deny => ConsentResult::Denied,
        }
    }
}
```

---

## 10. 性能优化

### 10.1 批量评估

```rust
impl PolicyEngine {
    /// 批量评估授权请求
    pub fn evaluate_batch(&self, requests: &[AuthRequest]) -> Vec<AuthorizationDecision> {
        // 并行评估
        requests.par_iter().map(|req| self.evaluate(req)).collect()
    }
}

/// 预取策略: 在 Agent 启动时预加载常用策略
pub struct PolicyPrefetcher {
    cache: AuthCache,
}

impl PolicyPrefetcher {
    /// 为 Agent 预取策略评估结果
    pub fn prefetch_for_agent(&mut self, agent: &Agent, engine: &PolicyEngine) {
        let common_operations = agent.common_operations();
        for op in common_operations {
            let request = AuthRequest::from_operation(&agent.id(), &op);
            let decision = engine.evaluate(&request);
            self.cache.insert(&request, &decision);
        }
    }
}
```

### 10.2 性能目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 单次授权评估 | < 100 us | 缓存命中 |
| 单次授权评估 | < 1 ms | 缓存未命中 |
| 批量评估 (100 项) | < 5 ms | 并行评估 |
| 缓存命中率 | > 95% | 正常运行 |
| 缓存失效传播 | < 10 ms | 跨服务 |

---

## 11. 授权测试方法

### 11.1 测试策略

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_token_validation() {
        let manager = CapabilityManager::new();
        let token = manager.create_token(
            EntityId::new("agent-1"),
            CapSet::FILE_READ | CapSet::FILE_WRITE,
            Duration::from_secs(3600),
        ).unwrap();

        assert!(manager.is_valid(&token.id));
    }

    #[test]
    fn test_capability_delegation_restriction() {
        let manager = CapabilityManager::new();
        let token = manager.create_token(
            EntityId::new("agent-1"),
            CapSet::FILE_READ,
            Duration::from_secs(3600),
        ).unwrap();

        // 尝试委托超出源令牌的能力
        let result = manager.delegate(DelegationRequest {
            source_token: token,
            target: EntityId::new("agent-2"),
            delegated_caps: CapSet::FILE_READ | CapSet::NETWORK, // 超出范围
            purpose: "test".to_string(),
            validity: Duration::from_secs(60),
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_policy_default_deny() {
        let engine = PolicyEngine::new_with_default_deny();
        let request = AuthRequest::new(
            EntityId::new("unknown-agent"),
            "file:read".to_string(),
            ResourceId::new("/etc/passwd"),
        );

        let decision = engine.evaluate(&request);
        assert!(matches!(decision, AuthorizationDecision::Deny { .. }));
    }

    #[test]
    fn test_one_time_token_single_use() {
        let manager = OneTimeAuthManager::new();
        let token = manager.create_token(
            AuthorizedOperation::FileRead { path: "/tmp/test".to_string() },
            ResourceId::new("/tmp/test"),
            EntityId::new("agent-1"),
            Duration::from_secs(60),
        ).unwrap();

        // 第一次使用应成功
        assert!(manager.verify_and_consume(&token).is_ok());

        // 第二次使用应失败
        assert!(manager.verify_and_consume(&token).is_err());
    }

    #[test]
    fn test_role_capability_inheritance() {
        let hierarchy = RoleHierarchy::system_roles();
        let caps = hierarchy.get_effective_capabilities(&RoleId::new("agent_admin"));

        assert!(caps.contains(&CapSet::AGENT_MANAGE));
        assert!(caps.contains(&CapSet::PROCESS_SPAWN));
        assert!(!caps.contains(&CapSet::ADMIN));
    }
}
```
