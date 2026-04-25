# OmniAgent OS 安全模型规范

> **文档编号**: OA-ARCH-SEC-001
> **版本**: 1.0.0
> **状态**: 草案
> **日期**: 2026-04-25
> **分类**: L1 架构文档

---

## 目录

1. [概述](#1-概述)
2. [威胁模型](#2-威胁模型)
3. [能力安全模型](#3-能力安全模型)
4. [Agent 隔离模型](#4-agent-隔离模型)
5. [授权框架](#5-授权框架)
6. [安全飞地](#6-安全飞地)
7. [IPC 安全](#7-ipc-安全)
8. [内存安全](#8-内存安全)
9. [虚拟化安全](#9-虚拟化安全)
10. [云端 AI 模型安全](#10-云端-ai-模型安全)
11. [审计日志](#11-审计日志)
12. [安全启动链](#12-安全启动链)
13. [威胁缓解措施表](#13-威胁缓解措施表)
14. [性能约束](#14-性能约束)
15. [测试用例](#15-测试用例)

---

## 1. 概述

### 1.1 文档目的

本文档定义 OmniAgent OS 的完整安全架构，涵盖从硬件启动到运行时 Agent 交互的全生命周期安全保障。作为微内核架构的 Agent 原生操作系统，OmniAgent OS 将 Agent 视为一等公民，因此安全模型必须同时保护传统进程和自主 Agent 实体。

### 1.2 设计原则

| 原则 | 描述 |
|------|------|
| 最小权限 | 每个组件仅持有完成任务所需的最小权限集合 |
| 纵深防御 | 多层安全机制叠加，任一层被突破不导致整体沦陷 |
| 默认拒绝 | 所有访问默认拒绝，仅通过显式授权放行 |
| 能力驱动 | 基于不可伪造的能力令牌（Capability）进行权限控制 |
| 零信任 | 不信任任何组件，包括内核自身（通过验证机制保障） |
| Agent 感知 | 安全策略理解 Agent 的自主性特征，支持动态权限调整 |

### 1.3 术语定义

| 术语 | 定义 |
|------|------|
| **Principal** | 安全主体，可以是用户、Agent 或系统服务 |
| **Capability (Cap)** | 不可伪造的权限令牌，证明持有者具有特定访问权限 |
| **Enclave** | 软件可信执行环境，提供隔离的安全计算区域 |
| **Security Label** | 安全标签，用于强制访问控制（MAC）分类 |
| **Port** | IPC 通信端点，具有关联的能力约束 |

---

## 2. 威胁模型

### 2.1 攻击面分析

OmniAgent OS 的攻击面分为以下层级：

```
┌─────────────────────────────────────────────────┐
│              外部攻击面                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ 网络接口  │ │ 设备接口  │ │ 云端 AI API 通道  │ │
│  └──────────┘ └──────────┘ └──────────────────┘ │
├─────────────────────────────────────────────────┤
│              边界攻击面                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ 系统调用  │ │ IPC 通道  │ │ 设备直通 (IOMMU) │ │
│  └──────────┘ └──────────┘ └──────────────────┘ │
├─────────────────────────────────────────────────┤
│              内部攻击面                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ Agent 间  │ │ Agent→内核│ │ 虚拟机逃逸       │ │
│  │ 攻击      │ │ 提权      │ │                  │ │
│  └──────────┘ └──────────┘ └──────────────────┘ │
└─────────────────────────────────────────────────┘
```

### 2.2 威胁分类

#### 2.2.1 远程威胁

| 威胁编号 | 威胁描述 | 严重程度 | 可能性 |
|----------|----------|----------|--------|
| T-REMOTE-001 | 网络协议栈漏洞导致远程代码执行 | 严重 | 中 |
| T-REMOTE-002 | 云端 AI API 响应注入恶意指令 | 高 | 中 |
| T-REMOTE-003 | DNS 欺骗导致 Agent 通信劫持 | 高 | 低 |
| T-REMOTE-004 | TLS 终止攻击窃取 API 密钥 | 严重 | 低 |

#### 2.2.2 本地威胁

| 威胁编号 | 威胁描述 | 严重程度 | 可能性 |
|----------|----------|----------|--------|
| T-LOCAL-001 | 恶意 Agent 利用 IPC 漏洞攻击其他 Agent | 高 | 中 |
| T-LOCAL-002 | Agent 逃逸至内核空间 | 严重 | 低 |
| T-LOCAL-003 | 能力令牌伪造或窃取 | 严重 | 低 |
| T-LOCAL-004 | 内存破坏漏洞（缓冲区溢出、UAF） | 高 | 中 |
| T-LOCAL-005 | 侧信道攻击（时序、缓存） | 中 | 中 |
| T-LOCAL-006 | Agent 资源耗尽攻击（DoS） | 中 | 高 |

#### 2.2.3 物理威胁

| 威胁编号 | 威胁描述 | 严重程度 | 可能性 |
|----------|----------|----------|--------|
| T-PHYS-001 | 冷启动攻击提取内存中的密钥 | 严重 | 低 |
| T-PHYS-002 | DMA 攻击通过恶意设备读取内存 | 高 | 低 |
| T-PHYS-003 | 固件级后门 | 严重 | 极低 |

### 2.3 对手模型

| 对手类型 | 能力 | 目标 |
|----------|------|------|
| **脚本小子** | 利用已知漏洞，无定制能力 | 随机攻击 |
| **有组织攻击者** | 可发现 0-day，具备逆向能力 | 特定目标数据窃取 |
| **国家级攻击者** | 具备固件级攻击能力，供应链攻击 | 长期潜伏与控制 |
| **恶意内部人员** | 拥有合法访问权限 | 数据泄露或破坏 |

---

## 3. 能力安全模型

### 3.1 能力令牌设计

能力（Capability）是 OmniAgent OS 安全模型的核心原语。每个能力令牌是一个不可伪造的 128 位标识符，由内核在授权时生成并绑定到特定 Principal。

```rust
/// 能力令牌 - 128 位不可伪造标识符
#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct Capability {
    /// 令牌唯一标识符（内核生成，用户空间不可写）
    token_id: u64,
    /// 令牌类型与权限标志
    flags: u64,
}

/// 能力类型
#[repr(u64)]
pub enum CapabilityType {
    /// 端口发送能力
    PortSend    = 0x0001,
    /// 端口接收能力
    PortRecv    = 0x0002,
    /// 内存映射能力
    MemoryMap   = 0x0003,
    /// 设备访问能力
    DeviceAccess = 0x0004,
    /// Agent 管理能力
    AgentManage = 0x0005,
    /// 安全飞地访问能力
    EnclaveAccess = 0x0006,
    /// 虚拟化管理能力
    VirtManage  = 0x0007,
    /// 审计日志读取能力
    AuditRead   = 0x0008,
    /// 系统配置能力
    SysConfig   = 0x0009,
}

/// 能力标志位
pub const CAP_FLAG_TRANSFERABLE: u64 = 1 << 0;  // 可转让
pub const CAP_FLAG_REVOCABLE: u64   = 1 << 1;  // 可撤销
pub const CAP_FLAG_TIME_LIMITED: u64 = 1 << 2;  // 有时限
pub const CAP_FLAG_DELEGATABLE: u64 = 1 << 3;  // 可委托
pub const CAP_FLAG_ONE_TIME: u64    = 1 << 4;  // 一次性使用
```

### 3.2 能力存储与管理

```rust
/// 能力空间 - 每个 Agent 拥有独立的能力空间
#[repr(C)]
pub struct CapabilitySpace {
    /// 能力槽数量（最大 256）
    slot_count: u32,
    /// 已使用槽位数
    used_slots: u32,
    /// 能力槽数组
    slots: [CapabilitySlot; 256],
}

/// 能力槽
#[repr(C)]
pub struct CapabilitySlot {
    /// 槽位状态
    state: CapSlotState,
    /// 能力令牌
    cap: Capability,
    /// 关联对象 ID
    object_id: u64,
    /// 过期时间戳（0 表示永不过期）
    expires_at: u64,
    /// 创建时间戳
    created_at: u64,
}

#[repr(u8)]
pub enum CapSlotState {
    Empty   = 0,
    Active  = 1,
    Revoked = 2,
    Expired = 3,
}
```

### 3.3 能力生命周期状态机

```
                    ┌──────────┐
                    │  Empty   │
                    └────┬─────┘
                         │ install_cap()
                         ▼
                    ┌──────────┐
              ┌────│  Active  │────┐
              │    └────┬─────┘    │
              │         │          │
     revoke()│    expires    delegate()
              │         │          │
              │         ▼          ▼
              │    ┌──────────┐ ┌──────────┐
              │    │ Expired  │ │  Active  │
              │    └──────────┘ │ (新槽位)  │
              │                 └──────────┘
              ▼
         ┌──────────┐
         │ Revoked  │
         └──────────┘
```

### 3.4 能力传递规则

| 操作 | 条件 | 行为 |
|------|------|------|
| **安装** | 内核验证授权 | 在调用者 CapSpace 中创建新槽位 |
| **转让** | `CAP_FLAG_TRANSFERABLE` 置位 | 从源槽位移除，在目标槽位安装 |
| **委托** | `CAP_FLAG_DELEGATABLE` 置位 | 创建受限副本（可缩减权限） |
| **撤销** | `CAP_FLAG_REVOCABLE` 置位 | 标记为 Revoked，所有衍生能力级联撤销 |
| **一次性使用** | `CAP_FLAG_ONE_TIME` 置位 | 使用后立即标记为 Revoked |

---

## 4. Agent 隔离模型

### 4.1 隔离层级

OmniAgent OS 实现三层 Agent 隔离：

```
┌────────────────────────────────────────────────┐
│              第 1 层：地址空间隔离               │
│  每个 Agent 拥有独立的页表 (CR3)                │
│  硬件强制执行，无法绕过                         │
├────────────────────────────────────────────────┤
│              第 2 层：IPC 隔离                  │
│  仅通过能力令牌控制的端口通信                    │
│  消息经过内核验证和过滤                          │
├────────────────────────────────────────────────┤
│              第 3 层：资源隔离                  │
│  独立的资源配额（CPU、内存、文件描述符）         │
│  防止资源耗尽攻击                               │
└────────────────────────────────────────────────┘
```

### 4.2 地址空间布局

```rust
/// Agent 地址空间布局
pub const AGENT_USER_SPACE_START: usize = 0x0000_0000_0000_0000;
pub const AGENT_USER_SPACE_END:   usize = 0x0000_7FFF_FFFF_FFFF;
pub const AGENT_STACK_TOP:        usize = 0x0000_7FFF_FFFF_F000;
pub const AGENT_STACK_SIZE:       usize = 0x0000_0001_0000_0000;  // 4 GB

/// 内核空间（所有 Agent 页表中映射，但用户态不可访问）
pub const KERNEL_SPACE_START:     usize = 0xFFFF_8000_0000_0000;
pub const KERNEL_SPACE_END:       usize = 0xFFFF_FFFF_FFFF_FFFF;

/// Agent 共享内存区域
pub const SHM_REGION_START:       usize = 0x0000_8000_0000_0000;
pub const SHM_REGION_END:         usize = 0x0000_BFFF_FFFF_FFFF;
```

### 4.3 隔离配置

```rust
/// Agent 隔离配置
#[repr(C)]
pub struct IsolationConfig {
    /// 地址空间隔离级别
    pub address_space_level: IsolationLevel,
    /// IPC 限制
    pub ipc_restriction: IpcRestriction,
    /// 资源配额
    pub resource_quota: ResourceQuota,
    /// 安全标签
    pub security_label: SecurityLabel,
    /// 是否允许设备访问
    pub device_access_allowed: bool,
    /// 是否允许网络访问
    pub network_access_allowed: bool,
    /// 是否允许虚拟化
    pub virtualization_allowed: bool,
}

#[repr(u8)]
pub enum IsolationLevel {
    /// 标准隔离：独立地址空间 + IPC 控制
    Standard = 0,
    /// 增强隔离：标准 + 额外沙箱限制
    Enhanced = 1,
    /// 最高隔离：独立于主系统的安全飞地
    Maximum  = 2,
}

#[repr(u8)]
pub enum IpcRestriction {
    /// 无限制（受能力令牌约束）
    Unrestricted = 0,
    /// 仅允许与特定 Agent 通信
    Whitelist    = 1,
    /// 完全禁止 IPC
    Disabled     = 2,
}
```

### 4.4 Agent 间攻击缓解

| 攻击类型 | 缓解机制 | 实现层级 |
|----------|----------|----------|
| 缓冲区溢出 | 独立页表 + NX 位 + Stack Canary | 硬件 + 编译器 |
| 返回导向编程 (ROP) | Shadow Stack (CET) + ASLR | 硬件 |
| 侧信道攻击 | 常量时间操作 + 缓存分区 | 软件 |
| 竞态条件 | 能力令牌原子操作 + RCU 同步 | 内核 |
| 资源耗尽 | 硬性资源配额 + OOM Kill | 内核 |
| 权限提升 | 最小能力原则 + 能力不可伪造 | 架构 |

---

## 5. 授权框架

### 5.1 框架概述

OmniAgent OS 的授权框架采用三层架构：

```
┌──────────────────────────────────────────────────┐
│                策略引擎 (Policy Engine)            │
│         PBAC (基于策略) + RBAC (基于角色)          │
├──────────────────────────────────────────────────┤
│         ┌──────────────┐  ┌──────────────────┐   │
│         │  一次性授权   │  │  永久授权         │   │
│         │ (One-Time)   │  │ (Permanent)      │   │
│         └──────────────┘  └──────────────────┘   │
├──────────────────────────────────────────────────┤
│              能力令牌层 (Capability Layer)         │
│         不可伪造令牌 + 权限验证                    │
└──────────────────────────────────────────────────┘
```

### 5.2 一次性授权 (One-Time Auth)

一次性授权用于高风险操作的临时权限授予：

```rust
/// 一次性授权令牌
#[repr(C)]
pub struct OneTimeAuth {
    /// 授权 ID（随机生成，128 位）
    auth_id: u128,
    /// 授权的操作类型
    operation: AuthOperation,
    /// 授权的目标对象
    target_id: u64,
    /// 授权的发起者
    grantor_id: u64,
    /// 授权的接收者
    grantee_id: u64,
    /// 授权创建时间
    created_at: u64,
    /// 授权过期时间
    expires_at: u64,
    /// 使用次数（0 表示未使用，1 表示已使用）
    use_count: AtomicU8,
    /// 授权签名（HMAC-SHA256）
    signature: [u8; 32],
}

#[repr(u32)]
pub enum AuthOperation {
    AgentSpawn     = 1,
    AgentKill      = 2,
    MemoryShare    = 3,
    DeviceAccess   = 4,
    EnclaveAccess  = 5,
    VirtControl    = 6,
    SystemConfig   = 7,
    AuditAccess    = 8,
}
```

### 5.3 永久授权 (Permanent Auth)

```rust
/// 永久授权记录
#[repr(C)]
pub struct PermanentAuth {
    /// 授权 ID
    auth_id: u64,
    /// 授权持有者
    principal_id: u64,
    /// 授权角色
    role: Role,
    /// 授权的能力集合
    capabilities: CapBitmap,
    /// 授权的安全标签
    security_label: SecurityLabel,
    /// 授权策略约束
    constraints: PolicyConstraints,
    /// 是否可撤销
    revocable: bool,
    /// 授权签名
    signature: [u8; 64],
}

/// 系统预定义角色
#[repr(u32)]
pub enum Role {
    /// 超级管理员
    SuperAdmin    = 0,
    /// 系统服务
    SystemService = 1,
    /// Agent 管理器
    AgentManager  = 2,
    /// 普通 Agent
    Agent         = 3,
    /// 沙箱 Agent
    Sandboxed     = 4,
    /// 审计员（只读）
    Auditor       = 5,
    /// 访客
    Guest         = 6,
}
```

### 5.4 策略引擎 (PBAC + RBAC)

```rust
/// 策略引擎配置
#[repr(C)]
pub struct PolicyEngine {
    /// RBAC 角色定义
    roles: Vec<RoleDefinition>,
    /// PBAC 策略规则
    policies: Vec<PolicyRule>,
    /// 策略决策缓存
    decision_cache: LruCache<PolicyQuery, PolicyDecision>,
}

/// 策略规则
#[repr(C)]
pub struct PolicyRule {
    /// 规则 ID
    rule_id: u64,
    /// 规则优先级（数值越小优先级越高）
    priority: u32,
    /// 匹配条件
    condition: PolicyCondition,
    /// 决策结果
    decision: PolicyDecision,
    /// 规则状态
    enabled: bool,
}

#[repr(u8)]
pub enum PolicyDecision {
    /// 允许
    Allow = 0,
    /// 拒绝
    Deny  = 1,
    /// 需要额外认证
    RequireAuth = 2,
    /// 委托给上级决策
    Delegate = 3,
}

/// 策略条件
#[repr(C)]
pub struct PolicyCondition {
    /// 源安全标签
    src_label: SecurityLabel,
    /// 目标安全标签
    dst_label: SecurityLabel,
    /// 所需能力类型
    required_cap: CapabilityType,
    /// 时间窗口约束
    time_window: Option<TimeWindow>,
    /// 资源使用阈值
    resource_threshold: Option<ResourceThreshold>,
}
```

### 5.5 授权决策流程

```
请求到达
    │
    ▼
┌─────────────┐     否
│ 检查能力令牌 ├──────────→ 拒绝
└──────┬──────┘
       │ 是
       ▼
┌─────────────┐     命中
│ 查询策略缓存 ├──────────→ 返回缓存决策
└──────┬──────┘
       │ 未命中
       ▼
┌─────────────┐
│ RBAC 角色检查│
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ PBAC 策略匹配│
└──────┬──────┘
       │
       ▼
┌─────────────┐     否
│ 一次性授权？ ├──────────→ 缓存并返回决策
└──────┬──────┘
       │ 是
       ▼
┌─────────────┐
│ 验证 OTA    │
└──────┬──────┘
       │
       ▼
  缓存并返回决策
```

---

## 6. 安全飞地

### 6.1 软件可信执行环境 (Software TEE)

OmniAgent OS 实现了纯软件的 TEE 方案，不依赖特定硬件（如 Intel SGX），通过内核级隔离机制提供可信执行环境。

### 6.2 架构设计

```
┌─────────────────────────────────────────────────┐
│                 普通用户空间                      │
│  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │ Agent A │  │ Agent B │  │ Cloud AI Client │ │
│  └─────────┘  └─────────┘  └─────────────────┘ │
├─────────────────────────────────────────────────┤
│                  内核空间                         │
│  ┌──────────────────────────────────────────┐   │
│  │           安全飞地管理器                   │   │
│  │  ┌────────────┐  ┌────────────────────┐  │   │
│  │  │ 密钥隔离区  │  │ 密封存储引擎       │  │   │
│  │  │ (Key Vault) │  │ (Sealed Storage)   │  │   │
│  │  └────────────┘  └────────────────────┘  │   │
│  └──────────────────────────────────────────┘   │
├─────────────────────────────────────────────────┤
│                  硬件层                           │
│  ┌──────────────────────────────────────────┐   │
│  │  页表保护 │ NX 位 │ SMAP/SMEP │ CET      │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

### 6.3 密钥隔离

```rust
/// 密钥隔离区
pub struct KeyVault {
    /// 主密钥（启动时生成，永不导出）
    master_key: [u8; 32],
    /// 密钥派生函数上下文
    kdf_context: HkdfCtx,
    /// 已注册密钥
    keys: BTreeMap<KeyId, SealedKey>,
    /// 访问控制列表
    acl: Vec<KeyAccessRule>,
    /// 密钥使用计数器（防重放）
    usage_counters: BTreeMap<KeyId, AtomicU64>,
}

/// 密钥访问规则
#[repr(C)]
pub struct KeyAccessRule {
    /// 允许访问的 Principal ID
    principal_id: u64,
    /// 允许的操作
    operations: KeyOpFlags,
    /// 每日使用上限
    daily_limit: u64,
    /// 单次使用限额
    per_use_limit: u64,
}

pub const KEY_OP_ENCRYPT:   u32 = 1 << 0;
pub const KEY_OP_DECRYPT:   u32 = 1 << 1;
pub const KEY_OP_SIGN:      u32 = 1 << 2;
pub const KEY_OP_VERIFY:    u32 = 1 << 3;
pub const KEY_OP_DERIVE:    u32 = 1 << 4;
pub const KEY_OP_WRAP:      u32 = 1 << 5;
pub const KEY_OP_UNWRAP:    u32 = 1 << 6;
```

### 6.4 密封存储

```rust
/// 密封存储条目
#[repr(C)]
pub struct SealedData {
    /// 数据 ID
    data_id: u64,
    /// 创建者 Agent ID
    creator_id: u64,
    /// 加密算法
    algorithm: CipherAlgorithm,
    /// 初始化向量
    iv: [u8; 16],
    /// 认证标签（AEAD）
    auth_tag: [u8; 16],
    /// 加密数据（存储在内核专用内存区域）
    ciphertext: Vec<u8>,
    /// 策略约束（解密条件）
    policy: SealPolicy,
}

/// 密封策略
#[repr(C)]
pub struct SealPolicy {
    /// 绑定的 Agent ID（仅该 Agent 可解封）
    bound_agent: Option<u64>,
    /// 绑定的安全标签
    bound_label: Option<SecurityLabel>,
    /// 绑定的启动测量值（PCR 等价物）
    bound_boot_hash: Option<[u8; 32]>,
    /// 最大解封次数
    max_unseal_count: Option<u64>,
    /// 有效期
    validity: Option<TimeWindow>,
}

#[repr(u8)]
pub enum CipherAlgorithm {
    Aes256Gcm    = 0,
    ChaCha20Poly = 1,
    XChaCha20Poly = 2,
}
```

### 6.5 飞地 API

| API | 功能 | 安全约束 |
|-----|------|----------|
| `enclave_create()` | 创建安全飞地实例 | 需要 `CAP_ENCLAVE_ACCESS` |
| `enclave_enter()` | 进入飞地执行代码 | 验证调用者身份和权限 |
| `enclave_seal()` | 密封数据到持久存储 | 数据在飞地内加密 |
| `enclave_unseal()` | 从持久存储解封数据 | 验证密封策略 |
| `enclave_destroy()` | 销毁飞地实例 | 安全擦除所有密钥材料 |
| `enclave_attest()` | 远程证明 | 生成飞地状态证明报告 |

---

## 7. IPC 安全

### 7.1 端口能力模型

所有 IPC 通信通过端口（Port）进行，每个端口关联一组能力约束：

```rust
/// IPC 端口
#[repr(C)]
pub struct Port {
    /// 端口 ID
    port_id: u64,
    /// 端口所有者
    owner_id: u64,
    /// 端口类型
    port_type: PortType,
    /// 最大消息大小
    max_msg_size: u32,
    /// 最大队列深度
    max_queue_depth: u32,
    /// 消息速率限制
    rate_limit: RateLimit,
    /// 安全约束
    security: PortSecurity,
}

#[repr(u8)]
pub enum PortType {
    /// 双向端口
    Bidirectional = 0,
    /// 单向发送端口
    SendOnly     = 1,
    /// 单向接收端口
    RecvOnly     = 2,
    /// 多播端口
    Multicast    = 3,
    /// 事件通知端口
    Notification = 4,
}

/// 端口安全约束
#[repr(C)]
pub struct PortSecurity {
    /// 允许的发送者列表
    allowed_senders: Vec<u64>,
    /// 允许的接收者列表
    allowed_receivers: Vec<u64>,
    /// 消息完整性要求
    integrity: IntegrityLevel,
    /// 消息加密要求
    encryption: EncryptionLevel,
    /// 是否需要审计
    audit_required: bool,
}

#[repr(u8)]
pub enum IntegrityLevel {
    None    = 0,  // 无完整性检查
    Checksum = 1, // CRC32 校验和
    HMAC    = 2,  // HMAC-SHA256 认证
    Signature = 3, // 数字签名（非对称）
}

#[repr(u8)]
pub enum EncryptionLevel {
    None    = 0,  // 明文传输
    Symmetric = 1, // 对称加密（AES-256-GCM）
    Asymmetric = 2, // 非对称加密
}
```

### 7.2 消息完整性验证流程

```
发送方 Agent                    内核                    接收方 Agent
     │                          │                          │
     │  send_msg(port, data)    │                          │
     │─────────────────────────>│                          │
     │                          │                          │
     │                  ┌───────┴───────┐                  │
     │                  │ 1. 验证发送方  │                  │
     │                  │    端口能力    │                  │
     │                  │ 2. 检查速率限制│                  │
     │                  │ 3. 计算消息    │                  │
     │                  │    HMAC        │                  │
     │                  │ 4. 加入审计日志│                  │
     │                  └───────┬───────┘                  │
     │                          │                          │
     │                          │  deliver_msg()           │
     │                          │─────────────────────────>│
     │                          │                          │
     │                          │                  ┌───────┴───────┐
     │                          │                  │ 1. 验证接收方  │
     │                          │                  │    端口能力    │
     │                          │                  │ 2. 验证 HMAC   │
     │                          │                  │ 3. 交付消息    │
     │                          │                  └───────┬───────┘
```

---

## 8. 内存安全

### 8.1 页表保护

```rust
/// 页表项权限标志
pub const PTE_PRESENT:    u64 = 1 << 0;   // 页面存在
pub const PTE_WRITABLE:   u64 = 1 << 1;   // 可写
pub const PTE_USER:       u64 = 1 << 2;   // 用户空间可访问
pub const PTE_NX:         u64 = 1 << 63;  // 不可执行 (No Execute)

/// 内存保护策略
#[repr(C)]
pub struct MemoryProtection {
    /// 代码段：只读 + 可执行
    pub code: PageFlags,   // PTE_PRESENT | PTE_USER
    /// 数据段：可读 + 可写 + 不可执行
    pub data: PageFlags,   // PTE_PRESENT | PTE_USER | PTE_WRITABLE | PTE_NX
    /// 栈段：可读 + 可写 + 不可执行 + 守护页
    pub stack: PageFlags,  // PTE_PRESENT | PTE_USER | PTE_WRITABLE | PTE_NX
    /// 堆段：可读 + 可写 + 不可执行
    pub heap: PageFlags,   // PTE_PRESENT | PTE_USER | PTE_WRITABLE | PTE_NX
    /// 共享内存：可配置
    pub shared: PageFlags, // 根据协商确定
}

/// 守护页配置
pub const GUARD_PAGE_SIZE: usize = 4096;         // 4KB 守护页
pub const STACK_GUARD_PAGES: usize = 4;           // 栈前后各 4 页守护
pub const HEAP_GUARD_PAGES: usize = 2;            // 堆末尾 2 页守护
pub const CODE_DATA_GUARD: usize = 1;             // 代码段与数据段之间 1 页守护
```

### 8.2 内存安全机制

| 机制 | 描述 | 硬件支持 |
|------|------|----------|
| **NX 位** | 数据页不可执行，防止代码注入 | x86_64 XD/EPT |
| **SMAP/SMEP** | 防止内核访问用户空间数据/执行用户空间代码 | Intel |
| **Shadow Stack** | 返回地址保护，防止 ROP 攻击 | Intel CET |
| **KASLR** | 内核地址空间布局随机化 | 软件 |
| **ASLR** | 用户空间地址空间布局随机化 | 软件 |
| **守护页** | 在关键区域间插入不可访问页 | MMU |
| **mprotect** | 运行时修改页面权限 | MMU |
| **栈 Canary** | 栈溢出检测 | 编译器 |
| **Safe Rust** | 内存安全的 Rust 代码 | 编译器 |

### 8.3 共享内存安全

```rust
/// 共享内存安全配置
#[repr(C)]
pub struct ShmSecurity {
    /// 共享内存 ID
    shm_id: u64,
    /// 创建者 Agent ID
    creator_id: u64,
    /// 参与者列表
    participants: Vec<ShmParticipant>,
    /// 内存保护标志
    protection: ShmProtection,
    /// 是否需要加密
    encrypted: bool,
    /// 加密密钥 ID（如果 encrypted 为 true）
    encryption_key_id: Option<KeyId>,
}

#[repr(C)]
pub struct ShmParticipant {
    /// Agent ID
    agent_id: u64,
    /// 访问权限
    access: ShmAccess,
    /// 是否可转让访问权
    transferable: bool,
}

#[repr(u8)]
pub enum ShmAccess {
    ReadOnly  = 0,
    WriteOnly = 1,
    ReadWrite = 2,
}
```

---

## 9. 虚拟化安全

### 9.1 VM 隔离模型

```
┌──────────────────────────────────────────────────┐
│              OmniAgent Hypervisor (L0)            │
├──────────┬──────────┬──────────┬────────────────┤
│  VM #1   │  VM #2   │  VM #3   │  Service VM    │
│ (Agent)  │ (Agent)  │ (Guest)  │ (I/O 服务)     │
│ VT-x/EPT │ VT-x/EPT │ VT-x/EPT │                │
├──────────┴──────────┴──────────┴────────────────┤
│              IOMMU (VT-d / AMD-Vi)               │
├──────────────────────────────────────────────────┤
│              物理硬件                             │
└──────────────────────────────────────────────────┘
```

### 9.2 虚拟化安全数据结构

```rust
/// VM 安全配置
#[repr(C)]
pub struct VmSecurityConfig {
    /// VM ID
    vm_id: u64,
    /// 隔离级别
    isolation_level: VmIsolation,
    /// EPT 权限配置
    ept_config: EptConfig,
    /// 设备直通列表
    passthrough_devices: Vec<PassthroughDevice>,
    /// IOMMU 保护域 ID
    iommu_domain_id: u32,
    /// VM 间通信策略
    inter_vm_policy: InterVmPolicy,
    /// 资源配额
    quota: VmResourceQuota,
}

#[repr(u8)]
pub enum VmIsolation {
    /// 标准隔离：EPT 隔离 + IOMMU 保护
    Standard = 0,
    /// 增强隔离：标准 + 内存加密 (TME/MKTME)
    Enhanced = 1,
    /// 完全隔离：独立加密密钥 + 安全启动验证
    Full     = 2,
}

/// 设备直通安全配置
#[repr(C)]
pub struct PassthroughDevice {
    /// PCI BDF (Bus:Device:Function)
    bdf: u32,
    /// 允许的 I/O 端口范围
    allowed_io_ranges: Vec<IoRange>,
    /// 允许的 MMIO 范围
    allowed_mmio_ranges: Vec<MmioRange>,
    /// 中断重映射
    interrupt_remapping: bool,
    /// IOMMU 保护
    iommu_protected: bool,
}
```

### 9.3 VM 逃逸防护

| 攻击向量 | 防护措施 |
|----------|----------|
| MMIO 漏洞 | EPT 权限严格限制 + MMIO 模拟在内核中完成 |
| 设备直通攻击 | IOMMU DMA 保护 + 中断重映射 |
| CPU 演化攻击 | MSR 访问过滤 + 敏感 MSR 拦截 |
| 定时器侧信道 | 虚拟化定时器隔离 + 噪声注入 |
| 缓存侧信道 | L3 缓存分区 (CAT) + 缓存刷新 |
| VM 间通信 | 仅通过 Hypervisor 管理的共享内存 |

---

## 10. 云端 AI 模型安全

### 10.1 安全架构

```
┌──────────────┐     TLS 1.3      ┌──────────────┐
│  OmniAgent   │◄────────────────►│  Cloud AI    │
│  AI Client   │                  │  Service     │
└──────┬───────┘                  └──────────────┘
       │
       ▼
┌──────────────────────────────────────────────┐
│              AI 安全中间层                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐ │
│  │ API 密钥  │ │ 输入过滤  │ │ 输出净化     │ │
│  │ 管理器    │ │ 引擎     │ │ 引擎         │ │
│  └──────────┘ └──────────┘ └──────────────┘ │
└──────────────────────────────────────────────┘
```

### 10.2 API 密钥管理

```rust
/// API 密钥安全存储
#[repr(C)]
pub struct ApiKeyEntry {
    /// 密钥 ID
    key_id: u64,
    /// 密钥提供商
    provider: AiProvider,
    /// 加密后的密钥材料（存储在 KeyVault 中）
    sealed_key_ref: KeyId,
    /// 允许使用此密钥的 Agent 列表
    allowed_agents: Vec<u64>,
    /// 每日调用限额
    daily_quota: u64,
    /// 每次调用最大 token 数
    max_tokens_per_call: u32,
    /// 密钥轮换周期（秒）
    rotation_period: u64,
    /// 上次轮换时间
    last_rotation: u64,
}

#[repr(u8)]
pub enum AiProvider {
    OpenAI    = 0,
    Anthropic = 1,
    Google    = 2,
    Local     = 3,  // 本地模型
    Custom    = 4,
}
```

### 10.3 模型输出净化

```rust
/// 输出净化规则
#[repr(C)]
pub struct OutputSanitizationRule {
    /// 规则 ID
    rule_id: u64,
    /// 匹配模式（正则表达式）
    pattern: String,
    /// 替换策略
    action: SanitizationAction,
    /// 严重程度
    severity: Severity,
}

#[repr(u8)]
pub enum SanitizationAction {
    /// 阻止输出
    Block    = 0,
    /// 替换为占位符
    Replace  = 1,
    /// 转义处理
    Escape   = 2,
    /// 记录警告但放行
    Warn     = 3,
}

/// 输入过滤引擎
pub struct InputFilter {
    /// 最大输入长度
    max_input_length: usize,
    /// 禁止的内容模式
    forbidden_patterns: Vec<Regex>,
    /// 提示注入检测
    prompt_injection_detector: PromptInjectionDetector,
    /// 敏感信息检测（防止密钥/令牌泄露）
    sensitive_data_detector: SensitiveDataDetector,
}
```

### 10.4 TLS 安全要求

| 参数 | 要求 |
|------|------|
| 最低 TLS 版本 | TLS 1.3 |
| 密码套件 | TLS_AES_256_GCM_SHA384, TLS_CHACHA20_POLY1305_SHA256 |
| 证书验证 | 严格验证，启用证书固定 (Certificate Pinning) |
| 前向保密 | 强制要求 (ECDHE 密钥交换) |
| 会话恢复 | 支持 PSK 恢复，最长会话 1 小时 |
| HSTS | 强制启用，max-age >= 31536000 |

---

## 11. 审计日志

### 11.1 日志格式

```rust
/// 审计日志条目
#[repr(C)]
pub struct AuditLogEntry {
    /// 日志序列号（单调递增）
    sequence: u64,
    /// 时间戳（纳秒精度）
    timestamp: u64,
    /// 事件类型
    event_type: AuditEventType,
    /// 事件严重程度
    severity: AuditSeverity,
    /// 源 Principal ID
    src_principal: u64,
    /// 目标 Principal ID（如适用）
    dst_principal: Option<u64>,
    /// 操作描述
    operation: AuditOperation,
    /// 操作结果
    result: AuditResult,
    /// 附加数据
    extra_data: Vec<u8>,
    /// 前一条日志的哈希（链式完整性）
    prev_hash: [u8; 32],
    /// 本条日志的哈希
    hash: [u8; 32],
    /// 数字签名（Ed25519）
    signature: [u8; 64],
}

#[repr(u8)]
pub enum AuditEventType {
    SyscallEntry    = 0,
    SyscallExit     = 1,
    IpcMessage      = 2,
    CapabilityOp    = 3,
    AgentLifecycle  = 4,
    AuthDecision    = 5,
    PolicyChange    = 6,
    SecurityEvent   = 7,
    BootEvent       = 8,
}

#[repr(u8)]
pub enum AuditSeverity {
    Info     = 0,
    Warning  = 1,
    Error    = 2,
    Critical = 3,
}
```

### 11.2 防篡改机制

```
日志条目 1              日志条目 2              日志条目 3
┌──────────┐          ┌──────────┐          ┌──────────┐
│ seq: 1   │          │ seq: 2   │          │ seq: 3   │
│ data:... │  hash───>│ prev_hash│  hash───>│ prev_hash│
│ hash: H1 │          │ hash: H2 │          │ hash: H3 │
│ sig: S1  │          │ sig: S2  │          │ sig: S3  │
└──────────┘          └──────────┘          └──────────┘
     │                      │                      │
     └──────────────────────┴──────────────────────┘
                            │
                    任何篡改都会破坏哈希链
                    可通过签名验证根信任
```

### 11.3 审计策略

| 事件类型 | 默认记录级别 | 可配置 |
|----------|-------------|--------|
| Agent 生命周期 | 全部记录 | 是 |
| 系统调用 | 仅安全相关 | 是 |
| IPC 通信 | 仅跨安全标签 | 是 |
| 能力操作 | 全部记录 | 否 |
| 授权决策 | 全部记录 | 否 |
| 策略变更 | 全部记录 | 否 |
| 安全事件 | 全部记录 | 否 |

---

## 12. 安全启动链

### 12.1 启动信任链

```
┌─────────┐   验证   ┌─────────┐   验证   ┌─────────┐
│  UEFI   │────────>│Bootloader│────────>│ Kernel  │
│ Firmware│  签名    │ (Rust)   │  哈希    │         │
└─────────┘         └─────────┘         └────┬────┘
                                                │ 验证
                                                ▼
                                          ┌──────────┐   验证   ┌──────────┐
                                          │ Services │────────>│Aqua Shell│
                                          │ (Agent   │  签名    │ Desktop  │
                                          │  Runtime)│          │          │
                                          └──────────┘          └──────────┘
```

### 12.2 启动测量

```rust
/// 启动测量值（类似 PCR 寄存器）
#[repr(C)]
pub struct BootMeasurements {
    /// PCR[0]: 固件代码
    pcr_firmware_code: [u8; 32],
    /// PCR[1]: 固件配置
    pcr_firmware_config: [u8; 32],
    /// PCR[2]: Bootloader 代码
    pcr_bootloader: [u8; 32],
    /// PCR[3]: 内核代码
    pcr_kernel: [u8; 32],
    /// PCR[4]: 内核配置
    pcr_kernel_config: [u8; 32],
    /// PCR[5]: 初始服务
    pcr_init_services: [u8; 32],
    /// PCR[6]: 服务配置
    pcr_service_config: [u8; 32],
    /// PCR[7]: 桌面环境
    pcr_desktop: [u8; 32],
}

/// 启动测量扩展操作
fn pcr_extend(pcr: &mut [u8; 32], data: &[u8]) {
    let mut hasher = Sha256::new();
    hasher.update(pcr);
    hasher.update(data);
    *pcr = hasher.finalize().into();
}
```

### 12.3 各阶段验证要求

| 阶段 | 验证方式 | 密钥来源 | 失败处理 |
|------|----------|----------|----------|
| UEFI → Bootloader | EDK2 Secure Boot | UEFI DB 密钥 | 拒绝启动 |
| Bootloader → Kernel | SHA-256 哈希验证 | 嵌入 Bootloader | 拒绝加载 |
| Kernel → Init Services | Ed25519 签名验证 | 内核内置公钥 | 进入恢复模式 |
| Services → Desktop | Ed25519 签名验证 | 策略引擎公钥 | 降级模式启动 |

---

## 13. 威胁缓解措施表

| 威胁编号 | 威胁描述 | 缓解措施 | 负责组件 | 优先级 |
|----------|----------|----------|----------|--------|
| T-REMOTE-001 | 远程代码执行 | 网络栈沙箱化 + seccomp 过滤 + NX 保护 | 网络服务 | P0 |
| T-REMOTE-002 | AI 响应注入 | 输出净化引擎 + 提示注入检测 + 沙箱执行 | AI 安全层 | P0 |
| T-REMOTE-003 | DNS 欺骗 | DNS-over-HTTPS + 证书固定 + Agent 通信签名 | 网络栈 | P1 |
| T-REMOTE-004 | TLS 终止攻击 | 端到端 TLS 1.3 + 证书固定 + HSTS | TLS 层 | P0 |
| T-LOCAL-001 | Agent 间攻击 | 能力令牌 + 端口 ACL + 消息 HMAC | IPC 子系统 | P0 |
| T-LOCAL-002 | 内核提权 | 最小内核接口 + 形式化验证 + 能力检查 | 微内核 | P0 |
| T-LOCAL-003 | 能力伪造 | 128 位随机令牌 + 内核独占生成 + 不可猜测空间 | 能力管理器 | P0 |
| T-LOCAL-004 | 内存破坏 | NX + ASLR + Stack Canary + Safe Rust + 守护页 | 内存管理器 | P0 |
| T-LOCAL-005 | 侧信道攻击 | 常量时间操作 + 缓存分区 + 噪声注入 | 全系统 | P1 |
| T-LOCAL-006 | 资源耗尽 | 硬性配额 + 速率限制 + OOM Kill + 公平调度 | 调度器 | P1 |
| T-PHYS-001 | 冷启动攻击 | 内存加密 (TME) + 密钥零化 + 启动时密钥生成 | KeyVault | P2 |
| T-PHYS-002 | DMA 攻击 | IOMMU (VT-d) + DMA 保护 + 设备白名单 | IOMMU 驱动 | P1 |
| T-PHYS-003 | 固件后门 | 安全启动链 + 启动测量 + 远程证明 | 启动管理器 | P2 |

---

## 14. 性能约束

### 14.1 安全操作性能目标

| 操作 | 目标延迟 | 最大延迟 | 备注 |
|------|----------|----------|------|
| 能力验证 | < 50 ns | < 200 ns | 哈希表查找 |
| 策略决策（缓存命中） | < 100 ns | < 500 ns | LRU 缓存 |
| 策略决策（缓存未命中） | < 5 μs | < 20 μs | 规则匹配 |
| 消息 HMAC 验证 | < 200 ns | < 1 μs | 4KB 消息 |
| 消息加密 (AES-256-GCM) | < 500 ns | < 2 μs | 4KB 消息 |
| 审计日志写入 | < 1 μs | < 5 μs | 批量提交 |
| 密钥派生 (HKDF) | < 10 μs | < 50 μs | 单次操作 |
| 密封存储操作 | < 100 μs | < 500 μs | 4KB 数据 |
| 安全飞地进入/退出 | < 5 μs | < 20 μs | 上下文切换 |
| 启动测量扩展 | < 100 ns | < 500 ns | SHA-256 |

### 14.2 内存开销

| 组件 | 常驻内存 | 最大内存 |
|------|----------|----------|
| 能力管理器 | 256 KB | 2 MB |
| 策略引擎 | 512 KB | 4 MB |
| 审计日志缓冲 | 1 MB | 8 MB |
| KeyVault | 128 KB | 512 KB |
| 密封存储缓存 | 256 KB | 2 MB |

---

## 15. 测试用例

### 15.1 能力安全测试

```rust
#[test]
fn test_capability_install_and_verify() {
    // 创建测试 Agent
    let agent = create_test_agent();
    // 安装能力令牌
    let cap = kernel_install_cap(&agent, CapabilityType::PortSend, 0x1001);
    // 验证能力存在
    assert!(kernel_verify_cap(&agent, &cap));
    // 验证不可伪造
    let forged = Capability { token_id: 0xDEAD, flags: cap.flags };
    assert!(!kernel_verify_cap(&agent, &forged));
}

#[test]
fn test_capability_revocation_cascade() {
    let agent_a = create_test_agent();
    let agent_b = create_test_agent();
    // 安装可撤销能力
    let cap = kernel_install_cap(&agent_a, CapabilityType::PortSend, 0x1001);
    // 委托给 Agent B
    let delegated = kernel_delegate_cap(&agent_a, &cap, &agent_b);
    assert!(kernel_verify_cap(&agent_b, &delegated));
    // 撤销原始能力
    kernel_revoke_cap(&agent_a, &cap);
    // 验证衍生能力也被撤销
    assert!(!kernel_verify_cap(&agent_b, &delegated));
}

#[test]
fn test_one_time_capability_consumed() {
    let agent = create_test_agent();
    let cap = kernel_install_cap_one_time(&agent, CapabilityType::EnclaveAccess);
    // 第一次使用应成功
    assert!(kernel_use_cap(&agent, &cap));
    // 第二次使用应失败
    assert!(!kernel_use_cap(&agent, &cap));
}
```

### 15.2 Agent 隔离测试

```rust
#[test]
fn test_address_space_isolation() {
    let agent_a = create_test_agent();
    let agent_b = create_test_agent();
    // Agent A 尝试访问 Agent B 的内存
    let result = agent_try_read_memory(&agent_a, agent_b.stack_top());
    assert_eq!(result, Err(SysError::EFAULT));
}

#[test]
fn test_ipc_port_acl_enforcement() {
    let agent_a = create_test_agent();
    let agent_b = create_test_agent();
    let agent_c = create_test_agent();
    // 创建仅允许 A→B 通信的端口
    let port = create_restricted_port(&agent_b, vec![agent_a.id()]);
    // A 发送消息应成功
    assert!(send_message(&agent_a, &port, b"hello").is_ok());
    // C 发送消息应被拒绝
    assert_eq!(
        send_message(&agent_c, &port, b"hello"),
        Err(SysError::EACCES)
    );
}

#[test]
fn test_resource_quota_enforcement() {
    let mut spec = AgentSpec::default();
    spec.memory_limit = 4 * 1024 * 1024; // 4 MB
    let agent = spawn_agent_with_spec(&spec);
    // 尝试分配超出配额的内存
    let result = agent_try_mmap(&agent, 8 * 1024 * 1024);
    assert_eq!(result, Err(SysError::ENOMEM));
}
```

### 15.3 安全飞地测试

```rust
#[test]
fn test_enclave_key_isolation() {
    let enclave = create_test_enclave();
    // 在飞地内生成密钥
    let key_id = enclave_generate_key(&enclave, CipherAlgorithm::Aes256Gcm);
    // 尝试从外部导出密钥应失败
    let result = enclave_export_key(&enclave, key_id);
    assert_eq!(result, Err(EnclaveError::KeyExportForbidden));
    // 在飞地内使用密钥应成功
    let result = enclave_encrypt(&enclave, key_id, b"test data");
    assert!(result.is_ok());
}

#[test]
fn test_sealed_storage_policy() {
    let enclave = create_test_enclave();
    let agent = create_test_agent();
    // 密封数据绑定到特定 Agent
    let policy = SealPolicy {
        bound_agent: Some(agent.id()),
        ..Default::default()
    };
    let sealed = enclave_seal(&enclave, b"secret data", &policy);
    // 同一 Agent 解封应成功
    let result = enclave_unseal(&enclave, &sealed, &agent);
    assert_eq!(result.unwrap(), b"secret data");
    // 不同 Agent 解封应失败
    let other_agent = create_test_agent();
    let result = enclave_unseal(&enclave, &sealed, &other_agent);
    assert_eq!(result, Err(EnclaveError::PolicyViolation));
}
```

### 15.4 审计日志测试

```rust
#[test]
fn test_audit_log_chain_integrity() {
    let entries = generate_test_audit_entries(100);
    // 验证哈希链完整性
    for i in 1..entries.len() {
        let expected_prev_hash = entries[i - 1].hash;
        assert_eq!(entries[i].prev_hash, expected_prev_hash);
    }
    // 篡改中间条目
    let mut tampered = entries[50].clone();
    tampered.operation = AuditOperation::AgentSpawn;
    // 验证篡改可被检测
    assert_ne!(tampered.hash, entries[50].hash);
    assert_ne!(entries[51].prev_hash, tampered.hash);
}

#[test]
fn test_audit_signature_verification() {
    let entries = generate_test_audit_entries(10);
    let public_key = get_audit_public_key();
    for entry in &entries {
        assert!(verify_signature(&public_key, &entry));
    }
}
```

### 15.5 安全启动测试

```rust
#[test]
fn test_boot_measurement_chain() {
    let measurements = simulate_boot_sequence();
    // 验证每个 PCR 值正确扩展
    assert_ne!(measurements.pcr_firmware_code, [0u8; 32]);
    assert_ne!(measurements.pcr_bootloader, [0u8; 32]);
    assert_ne!(measurements.pcr_kernel, [0u8; 32]);
    // 验证 PCR 扩展的确定性
    let measurements2 = simulate_boot_sequence();
    assert_eq!(measurements.pcr_kernel, measurements2.pcr_kernel);
}

#[test]
fn test_boot_tamper_detection() {
    let mut measurements = simulate_boot_sequence();
    // 篡改内核代码
    let original_pcr = measurements.pcr_kernel;
    measurements.pcr_kernel = [0xFF; 32];
    // 验证篡改可被检测
    assert_ne!(measurements.pcr_kernel, original_pcr);
    let expected = compute_expected_pcr(&measurements);
    assert_ne!(measurements.pcr_init_services, expected);
}
```

### 15.6 性能测试

```rust
#[test]
fn test_capability_verify_performance() {
    let agent = create_test_agent();
    let cap = kernel_install_cap(&agent, CapabilityType::PortSend, 0x1001);
    let iterations = 1_000_000;
    let start = Tsc::now();
    for _ in 0..iterations {
        black_box(kernel_verify_cap(&agent, &cap));
    }
    let elapsed = Tsc::now() - start;
    let avg_ns = elapsed.to_nanoseconds() / iterations;
    assert!(avg_ns < 200, "Capability verify took {} ns, target < 200 ns", avg_ns);
}

#[test]
fn test_policy_decision_cached_performance() {
    let engine = create_test_policy_engine();
    let query = create_test_policy_query();
    // 预热缓存
    engine.decide(&query);
    let iterations = 1_000_000;
    let start = Tsc::now();
    for _ in 0..iterations {
        black_box(engine.decide(&query));
    }
    let elapsed = Tsc::now() - start;
    let avg_ns = elapsed.to_nanoseconds() / iterations;
    assert!(avg_ns < 500, "Cached policy decision took {} ns, target < 500 ns", avg_ns);
}
```

---

## 附录 A：安全相关系统调用错误码

| 错误码 | 值 | 描述 |
|--------|-----|------|
| `ESEC_PERMISSION_DENIED` | 1000 | 安全策略拒绝操作 |
| `ESEC_CAP_INVALID` | 1001 | 无效的能力令牌 |
| `ESEC_CAP_REVOKED` | 1002 | 能力令牌已被撤销 |
| `ESEC_CAP_EXPIRED` | 1003 | 能力令牌已过期 |
| `ESEC_AUTH_FAILED` | 1004 | 认证失败 |
| `ESEC_AUTH_EXPIRED` | 1005 | 一次性授权已使用 |
| `ESEC_ISOLATION_VIOLATION` | 1006 | 隔离策略违规 |
| `ESEC_LABEL_MISMATCH` | 1007 | 安全标签不匹配 |
| `ESEC_POLICY_VIOLATION` | 1008 | 策略引擎拒绝 |
| `ESEC_ENCLAVE_ERROR` | 1009 | 安全飞地操作失败 |
| `ESEC_KEY_NOT_FOUND` | 1010 | 密钥不存在 |
| `ESEC_KEY_EXPORT_FORBIDDEN` | 1011 | 密钥导出被禁止 |
| `ESEC_SEAL_POLICY_VIOLATION` | 1012 | 密封策略违规 |
| `ESEC_AUDIT_FAILURE` | 1013 | 审计日志写入失败 |
| `ESEC_BOOT_TAMPERED` | 1014 | 启动完整性验证失败 |
| `ESEC_IOMMU_VIOLATION` | 1015 | IOMMU 保护违规 |
| `ESEC_VM_ESCAPE_ATTEMPT` | 1016 | VM 逃逸尝试 |
| `ESEC_INPUT_FILTERED` | 1017 | 输入被安全过滤器拦截 |
| `ESEC_OUTPUT_BLOCKED` | 1018 | 输出被净化引擎阻止 |

---

## 附录 B：安全配置默认值

| 配置项 | 默认值 | 范围 |
|--------|--------|------|
| 最大能力槽数 | 256 | 16 - 1024 |
| 能力令牌过期时间 | 无限期 | 1s - 无限期 |
| 策略缓存大小 | 4096 条目 | 256 - 65536 |
| 审计日志缓冲区大小 | 1 MB | 256 KB - 16 MB |
| 审计日志刷写间隔 | 1 秒 | 100 ms - 60 s |
| 密钥轮换周期 | 30 天 | 1 天 - 365 天 |
| 最大登录尝试次数 | 5 | 1 - 100 |
| 账户锁定时间 | 15 分钟 | 1 分钟 - 24 小时 |
| IPC 消息最大大小 | 64 KB | 4 KB - 1 MB |
| IPC 队列最大深度 | 128 | 8 - 1024 |
| 共享内存最大区域数 | 64 | 1 - 256 |
| VM 最大数量 | 16 | 1 - 64 |
| AI 输出净化严格度 | 中等 | 低/中/高/严格 |

---

> **文档维护者**: OmniAgent OS 安全团队
> **审核周期**: 每季度
> **下次审核日期**: 2026-07-25
