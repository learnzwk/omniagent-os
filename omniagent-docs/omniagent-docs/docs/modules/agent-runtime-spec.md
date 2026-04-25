# OmniAgent OS Agent Runtime 规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **模块归属**: 内核模块 / Agent 运行时子系统
> **状态**: 规范草案

---

## 1. 概述

### 1.1 目的

本文档定义 OmniAgent OS Agent Runtime 子系统的完整规范。Agent Runtime 是 OmniAgent OS 的核心创新组件，提供 Agent 的全生命周期管理、动态组装、并发调度、通信机制和自主进化能力。作为 Agent 原生操作系统，OmniAgent OS 将 Agent 视为一等公民，内核原生支持 Agent 的创建、调度、隔离和通信。

### 1.2 设计哲学

| 原则 | 描述 |
|------|------|
| Agent 一等公民 | Agent 是内核调度的基本单元，与进程同等重要 |
| 能力驱动安全 | 基于能力的细粒度权限控制，取代传统 DAC/MAC |
| 自主进化 | Agent 可通过遗传算法自我优化参数和策略 |
| 零拷贝通信 | Agent 间通过共享内存实现高效数据交换 |
| 动态组装 | Expert Factory 根据用户意图动态生成最优 Agent 配置 |

### 1.3 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                       用户空间                                │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                  libagent (用户库)                    │    │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────────────┐   │    │
│  │  │Agent API │ │通信 API  │ │知识共享 API        │   │    │
│  │  └────┬─────┘ └────┬─────┘ └────────┬──────────┘   │    │
│  └───────┼────────────┼────────────────┼──────────────┘    │
│          │            │                │                    │
│  ────────┴────────────┴────────────────┴─────────────────── │
│                   系统调用接口                               │
│  ────────────────────────────────────────────────────────── │
│                       内核空间                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                Agent Runtime 内核                     │    │
│  │  ┌───────────┐  ┌───────────┐  ┌────────────────┐  │    │
│  │  │Agent 管理  │  │Agent 池   │  │Expert Factory  │  │    │
│  │  │生命周期    │  │工作窃取    │  │动态组装        │  │    │
│  │  └───────────┘  └───────────┘  └────────────────┘  │    │
│  │  ┌───────────┐  ┌───────────┐  ┌────────────────┐  │    │
│  │  │Agent 通信  │  │Agent 进化  │  │知识共享引擎    │  │    │
│  │  │Pub/Sub    │  │遗传算法    │  │跨Agent传输     │  │    │
│  │  └───────────┘  └───────────┘  └────────────────┘  │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              底层支撑子系统                            │    │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │    │
│  │  │调度器 CFS │ │内存管理器 │ │IPC 子系统│ │安全模块│ │    │
│  │  └──────────┘ └──────────┘ └──────────┘ └────────┘ │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Agent 生命周期

### 2.1 生命周期状态机

```
                    ┌──────────┐
                    │  Created │
                    │  (已创建) │
                    └────┬─────┘
                         │ configure()
                         ▼
                    ┌──────────┐
                    │Configured│
                    │  (已配置) │
                    └────┬─────┘
                         │ start()
                         ▼
                    ┌──────────┐         pause()
               ┌───▶│ Running  │──────────────────┐
               │    │ (运行中)  │                  │
               │    └────┬─────┘                  ▼
               │         │ kill()            ┌──────────┐
               │         ▼                   │  Paused  │
               │    ┌──────────┐             │  (暂停)   │
               │    │Stopping  │             └────┬─────┘
               │    │(停止中)   │                  │ resume()
               │    └────┬─────┘                  │
               │         │                        │
               │         ▼                        │
               │    ┌──────────┐                  │
               └────│ Stopped  │◀─────────────────┘
                    │ (已停止)  │
                    └────┬─────┘
                         │ destroy()
                         ▼
                    ┌──────────┐
                    │ Destroyed│
                    │ (已销毁)  │
                    └──────────┘

    错误路径:
    Running ──[OOM/异常]──▶ Error ──[recover]──▶ Running
                               │
                               └──[fail]──▶ Stopped
```

### 2.2 状态定义

```rust
/// Agent 生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AgentState {
    /// 已创建，尚未配置
    Created = 0,
    /// 已配置，等待启动
    Configured = 1,
    /// 正在运行
    Running = 2,
    /// 已暂停
    Paused = 3,
    /// 正在停止
    Stopping = 4,
    /// 已停止（可重新启动）
    Stopped = 5,
    /// 错误状态（可恢复）
    Error = 6,
    /// 已销毁（不可恢复）
    Destroyed = 7,
}

/// Agent 状态转换验证
impl AgentState {
    /// 验证状态转换是否合法
    pub fn can_transition_to(&self, target: AgentState) -> bool {
        matches!(
            (self, target),
            (AgentState::Created, AgentState::Configured)
            | (AgentState::Created, AgentState::Destroyed)
            | (AgentState::Configured, AgentState::Running)
            | (AgentState::Configured, AgentState::Destroyed)
            | (AgentState::Running, AgentState::Paused)
            | (AgentState::Running, AgentState::Stopping)
            | (AgentState::Running, AgentState::Error)
            | (AgentState::Paused, AgentState::Running)
            | (AgentState::Paused, AgentState::Stopping)
            | (AgentState::Paused, AgentState::Destroyed)
            | (AgentState::Stopping, AgentState::Stopped)
            | (AgentState::Stopped, AgentState::Configured)
            | (AgentState::Stopped, AgentState::Destroyed)
            | (AgentState::Error, AgentState::Running)
            | (AgentState::Error, AgentState::Stopped)
            | (AgentState::Error, AgentState::Destroyed)
        )
    }
}
```

### 2.3 Agent 控制块

```rust
/// Agent 控制块（内核数据结构）
pub struct AgentControlBlock {
    /// Agent 唯一标识
    pub id: AgentId,
    /// Agent 名称
    pub name: String,
    /// 当前状态
    pub state: AtomicAgentState,
    /// Agent 规格描述
    pub spec: AgentSpec,
    /// 关联的进程 ID
    pub process_id: Option<ProcessId>,
    /// 关联的线程 ID
    pub thread_ids: Vec<ThreadId>,
    /// 创建时间
    pub created_at: u64,
    /// 上次状态变更时间
    pub last_state_change: AtomicU64,
    /// 执行统计
    pub stats: AgentStats,
    /// 安全上下文
    pub security_ctx: SecurityContext,
    /// 通信端点
    pub comm_endpoints: Vec<CommEndpoint>,
    /// 知识库引用
    pub knowledge_bases: Vec<KnowledgeBaseId>,
    /// 父 Agent（用于层级关系）
    pub parent: Option<AgentId>,
    /// 子 Agent 列表
    pub children: Vec<AgentId>,
}

/// Agent ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentId(u64);

impl AgentId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 原子 Agent 状态包装
pub struct AtomicAgentState(AtomicU8);

impl AtomicAgentState {
    pub fn new(state: AgentState) -> Self {
        Self(AtomicU8::new(state as u8))
    }

    pub fn load(&self, ordering: Ordering) -> AgentState {
        unsafe { core::mem::transmute(self.0.load(ordering)) }
    }

    pub fn compare_exchange(
        &self,
        expected: AgentState,
        new: AgentState,
        success: Ordering,
        failure: Ordering,
    ) -> Result<AgentState, AgentState> {
        self.0.compare_exchange(
            expected as u8, new as u8, success, failure
        ).map(|v| unsafe { core::mem::transmute(v) })
         .map_err(|v| unsafe { core::mem::transmute(v) })
    }
}

/// Agent 执行统计
#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    pub total_cpu_time_ns: AtomicU64,
    pub total_messages_sent: AtomicU64,
    pub total_messages_received: AtomicU64,
    pub total_tasks_completed: AtomicU64,
    pub total_tasks_failed: AtomicU64,
    pub memory_peak_bytes: AtomicU64,
    pub context_switches: AtomicU64,
    pub last_heartbeat: AtomicU64,
}
```

---

## 3. AgentSpec 规格

### 3.1 AgentSpec 定义

```rust
/// Agent 规格描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Agent 类型标识
    pub agent_type: AgentType,
    /// 能力集
    pub capabilities: Vec<Capability>,
    /// 资源配额
    pub resource_quota: ResourceQuota,
    /// 知识库配置
    pub knowledge_config: KnowledgeConfig,
    /// 优先级
    pub priority: AgentPriority,
    /// 安全标签
    pub security_label: SecurityLabel,
    /// 通信配置
    pub comm_config: CommConfig,
    /// 进化参数
    pub evolution_config: Option<EvolutionConfig>,
    /// 超时设置
    pub timeout: TimeoutConfig,
    /// 环境变量
    pub env: BTreeMap<String, String>,
    /// 自定义参数
    pub params: BTreeMap<String, Value>,
}

/// Agent 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// 通用 Agent
    General,
    /// 专家 Agent（领域专精）
    Expert,
    /// 协调 Agent（管理子 Agent）
    Coordinator,
    /// 监控 Agent（系统监控）
    Monitor,
    /// 沙箱 Agent（不可信代码执行）
    Sandbox,
    /// 系统 Agent（内核服务）
    System,
}

/// 能力定义
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    /// 能力名称
    pub name: String,
    /// 能力版本
    pub version: semver::Version,
    /// 能力参数
    pub params: BTreeMap<String, Value>,
    /// 所需权限
    pub required_permissions: Vec<Permission>,
}

/// 预定义能力
pub mod capabilities {
    pub const FILE_READ: &str = "file.read";
    pub const FILE_WRITE: &str = "file.write";
    pub const NETWORK_TCP: &str = "network.tcp";
    pub const NETWORK_UDP: &str = "network.udp";
    pub const PROCESS_SPAWN: &str = "process.spawn";
    pub const AGENT_SPAWN: &str = "agent.spawn";
    pub const AGENT_COMMUNICATE: &str = "agent.communicate";
    pub const KNOWLEDGE_READ: &str = "knowledge.read";
    pub const KNOWLEDGE_WRITE: &str = "knowledge.write";
    pub const SYSTEM_INFO: &str = "system.info";
    pub const HARDWARE_ACCESS: &str = "hardware.access";
    pub const VIRTUALIZATION: &str = "virtualization.manage";
}

/// 权限定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum Permission {
    ReadFile = 1,
    WriteFile = 2,
    CreateSocket = 4,
    BindPort = 8,
    SpawnProcess = 16,
    SpawnAgent = 32,
    SharedMemory = 64,
    RawIo = 128,
    KernelModule = 256,
    SystemAdmin = 512,
}

/// 资源配额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// 最大 CPU 时间（纳秒/秒）
    pub cpu_time_quota_ns: u64,
    /// 最大物理内存（字节）
    pub max_memory_bytes: u64,
    /// 最大共享内存（字节）
    pub max_shared_memory_bytes: u64,
    /// 最大打开文件数
    pub max_open_files: usize,
    /// 最大子 Agent 数
    pub max_child_agents: usize,
    /// 最大线程数
    pub max_threads: usize,
    /// 最大消息队列深度
    pub max_message_queue_depth: usize,
    /// 最大网络带宽（字节/秒）
    pub max_network_bandwidth: u64,
    /// 磁盘 I/O 配额（字节/秒）
    pub max_io_bytes_per_sec: u64,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            cpu_time_quota_ns: 800_000_000,     // 80% CPU
            max_memory_bytes: 256 * 1024 * 1024, // 256MB
            max_shared_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_open_files: 1024,
            max_child_agents: 32,
            max_threads: 8,
            max_message_queue_depth: 4096,
            max_network_bandwidth: 100 * 1024 * 1024, // 100MB/s
            max_io_bytes_per_sec: 50 * 1024 * 1024,   // 50MB/s
        }
    }
}

/// Agent 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum AgentPriority {
    /// 最低优先级（后台任务）
    Low = 0,
    /// 低于正常
    BelowNormal = 1,
    /// 正常优先级
    Normal = 2,
    /// 高于正常
    AboveNormal = 3,
    /// 高优先级
    High = 4,
    /// 实时优先级（系统 Agent）
    Realtime = 5,
}

/// 安全标签
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityLabel {
    /// 安全等级
    pub level: SecurityLevel,
    /// 安全域
    pub compartment: String,
    /// 可信度评分（0-100）
    pub trust_score: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SecurityLevel {
    Untrusted = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// 通信配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommConfig {
    /// 订阅的主题列表
    pub subscriptions: Vec<String>,
    /// 允许通信的 Agent 白名单
    pub allowed_peers: Vec<AgentId>,
    /// 消息大小限制
    pub max_message_size: usize,
    /// 是否允许广播
    pub allow_broadcast: bool,
}

impl Default for CommConfig {
    fn default() -> Self {
        Self {
            subscriptions: Vec::new(),
            allowed_peers: Vec::new(),
            max_message_size: 64 * 1024, // 64KB
            allow_broadcast: false,
        }
    }
}

/// 超时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// 任务执行超时（毫秒）
    pub task_timeout_ms: u64,
    /// 消息发送超时（毫秒）
    pub send_timeout_ms: u64,
    /// 消息接收超时（毫秒）
    pub recv_timeout_ms: u64,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 心跳丢失容忍次数
    pub heartbeat_missed_tolerance: u8,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            task_timeout_ms: 30_000,      // 30秒
            send_timeout_ms: 5_000,       // 5秒
            recv_timeout_ms: 0,           // 无限等待
            heartbeat_interval_ms: 1_000, // 1秒
            heartbeat_missed_tolerance: 3,
        }
    }
}

/// 知识库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeConfig {
    /// 关联的知识库 ID 列表
    pub knowledge_base_ids: Vec<KnowledgeBaseId>,
    /// 读写权限
    pub access_mode: KnowledgeAccessMode,
    /// 知识缓存大小
    pub cache_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeAccessMode {
    ReadOnly,
    ReadWrite,
    WriteOnly,
}
```

---

## 4. Expert Factory（动态 Agent 组装）

### 4.1 Expert Factory 设计

```rust
/// Expert Factory：根据用户意图动态组装 Agent
pub struct ExpertFactory {
    /// 可用的领域模板库
    domain_templates: BTreeMap<String, DomainTemplate>,
    /// 可用的工具注册表
    tool_registry: ToolRegistry,
    /// 知识库索引
    knowledge_index: KnowledgeIndex,
    /// 组装策略
    assembly_strategy: AssemblyStrategy,
    /// 历史组装记录（用于优化）
    assembly_history: SpinLock<Vec<AssemblyRecord>>,
}

/// 领域模板
#[derive(Debug, Clone)]
pub struct DomainTemplate {
    /// 领域名称
    pub domain: String,
    /// 推荐的工具集
    pub recommended_tools: Vec<ToolId>,
    /// 推荐的知识库
    pub recommended_knowledge: Vec<KnowledgeBaseId>,
    /// 默认能力集
    pub default_capabilities: Vec<Capability>,
    /// 默认资源配额
    pub default_quota: ResourceQuota,
    /// 安全建议
    pub security_recommendations: SecurityRecommendations,
}

/// 组装请求
#[derive(Debug, Clone)]
pub struct AssemblyRequest {
    /// 用户意图描述
    pub intent: String,
    /// 目标领域
    pub domain: Option<String>,
    /// 指定工具
    pub tools: Vec<ToolId>,
    /// 指定知识库
    pub knowledge_bases: Vec<KnowledgeBaseId>,
    /// 自定义能力
    pub capabilities: Vec<Capability>,
    /// 资源限制
    pub resource_limits: Option<ResourceQuota>,
    /// 安全约束
    pub security_constraints: Vec<SecurityConstraint>,
}

/// 组装结果
#[derive(Debug)]
pub struct AssemblyResult {
    /// 生成的 AgentSpec
    pub spec: AgentSpec,
    /// 组装决策说明
    pub decisions: Vec<AssemblyDecision>,
    /// 置信度评分（0-1）
    pub confidence: f64,
    /// 组装耗时
    pub assembly_time_us: u64,
}

/// 组装决策
#[derive(Debug, Clone)]
pub struct AssemblyDecision {
    pub decision_type: DecisionType,
    pub rationale: String,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum DecisionType {
    ToolSelected,
    KnowledgeBaseSelected,
    CapabilityAdded,
    QuotaAdjusted,
    SecurityLabelAssigned,
    PrioritySet,
}

impl ExpertFactory {
    /// 根据用户意图组装 Agent
    pub fn assemble(&self, request: &AssemblyRequest) -> Result<AssemblyResult, FactoryError> {
        let start = current_time_us();

        // 1. 意图解析：确定领域
        let domain = self.resolve_domain(&request.intent, request.domain.as_deref())?;

        // 2. 获取领域模板
        let template = self.domain_templates.get(&domain)
            .ok_or(FactoryError::UnknownDomain(domain.clone()))?;

        // 3. 选择工具集
        let tools = self.select_tools(
            &request.intent,
            &request.tools,
            &template.recommended_tools,
        )?;

        // 4. 选择知识库
        let knowledge = self.select_knowledge(
            &request.intent,
            &request.knowledge_bases,
            &template.recommended_knowledge,
        )?;

        // 5. 确定能力集
        let capabilities = self.determine_capabilities(
            &request.capabilities,
            &template.default_capabilities,
            &tools,
        );

        // 6. 计算资源配额
        let quota = self.calculate_quota(
            request.resource_limits.as_ref(),
            &template.default_quota,
            &tools,
        );

        // 7. 分配安全标签
        let security_label = self.assign_security_label(
            &request.security_constraints,
            &tools,
            &capabilities,
        );

        // 8. 生成 AgentSpec
        let spec = AgentSpec {
            agent_type: AgentType::Expert,
            capabilities,
            resource_quota: quota,
            knowledge_config: KnowledgeConfig {
                knowledge_base_ids: knowledge,
                access_mode: KnowledgeAccessMode::ReadWrite,
                cache_size: 1024,
            },
            priority: AgentPriority::Normal,
            security_label,
            comm_config: CommConfig::default(),
            evolution_config: Some(EvolutionConfig::default()),
            timeout: TimeoutConfig::default(),
            env: BTreeMap::new(),
            params: BTreeMap::new(),
        };

        let decisions = vec![
            AssemblyDecision {
                decision_type: DecisionType::ToolSelected,
                rationale: format!("为领域 '{}' 选择了 {} 个工具", domain, tools.len()),
                alternatives: vec![],
            },
        ];

        Ok(AssemblyResult {
            spec,
            decisions,
            confidence: 0.85,
            assembly_time_us: current_time_us() - start,
        })
    }

    /// 意图解析：从自然语言描述推断领域
    fn resolve_domain(&self, intent: &str, hint: Option<&str>) -> Result<String, FactoryError> {
        // 如果用户指定了领域，直接使用
        if let Some(domain) = hint {
            return Ok(domain.to_string());
        }

        // 使用关键词匹配推断领域
        let domain_keywords = [
            ("network", &["网络", "network", "socket", "连接", "通信"]),
            ("filesystem", &["文件", "file", "磁盘", "存储", "目录"]),
            ("security", &["安全", "security", "加密", "认证", "防火墙"]),
            ("monitoring", &["监控", "monitor", "指标", "日志", "告警"]),
            ("data", &["数据", "data", "分析", "处理", "转换"]),
        ];

        for (domain, keywords) in &domain_keywords {
            for kw in *keywords {
                if intent.contains(kw) {
                    return Ok(domain.to_string());
                }
            }
        }

        Ok("general".to_string())
    }
}
```

---

## 5. Agent Pool（Agent 池与调度）

### 5.1 Agent 池设计

```rust
/// Agent 池：管理所有活跃 Agent 的调度和负载均衡
pub struct AgentPool {
    /// 每个 CPU 核心的本地队列
    local_queues: [SpinLock<VecDeque<AgentId>>; MAX_CPUS],
    /// 全局共享队列（用于跨核心窃取）
    global_queue: SpinLock<VecDeque<AgentId>>,
    /// Agent 控制块表
    agents: SpinLock<BTreeMap<AgentId, Arc<AgentControlBlock>>>,
    /// 负载均衡器
    load_balancer: LoadBalancer,
    /// 池统计
    stats: PoolStats,
    /// 最大 Agent 数量
    max_agents: usize,
    /// 当前 Agent 数量
    current_count: AtomicUsize,
}

/// 负载均衡器
pub struct LoadBalancer {
    /// 每个 CPU 核心的负载追踪
    cpu_loads: [AtomicU64; MAX_CPUS],
    /// 负载均衡策略
    strategy: LoadBalanceStrategy,
    /// 上次均衡时间
    last_balance_time: AtomicU64,
    /// 均衡间隔（微秒）
    balance_interval_us: u64,
}

/// 负载均衡策略
#[derive(Debug, Clone, Copy)]
pub enum LoadBalanceStrategy {
    /// 工作窃取（默认）
    WorkStealing,
    /// 最少负载优先
    LeastLoaded,
    /// 轮询
    RoundRobin,
    /// 加权轮询
    WeightedRoundRobin,
}

/// 池统计
#[derive(Debug, Default)]
pub struct PoolStats {
    pub total_agents_spawned: AtomicU64,
    pub total_agents_completed: AtomicU64,
    pub total_agents_killed: AtomicU64,
    pub total_steals: AtomicU64,
    pub avg_queue_depth: AtomicU64,
    pub max_queue_depth: AtomicU64,
}

impl AgentPool {
    /// 创建 Agent 池
    pub fn new(max_agents: usize) -> Self {
        Self {
            local_queues: core::array::from_fn(|_| SpinLock::new(VecDeque::new())),
            global_queue: SpinLock::new(VecDeque::new()),
            agents: SpinLock::new(BTreeMap::new()),
            load_balancer: LoadBalancer {
                cpu_loads: core::array::from_fn(|_| AtomicU64::new(0)),
                strategy: LoadBalanceStrategy::WorkStealing,
                last_balance_time: AtomicU64::new(0),
                balance_interval_us: 10_000, // 10ms
            },
            stats: PoolStats::default(),
            max_agents,
            current_count: AtomicUsize::new(0),
        }
    }

    /// 将 Agent 加入调度队列
    pub fn enqueue(&self, agent_id: AgentId, cpu_id: usize) -> Result<(), PoolError> {
        if self.current_count.load(Ordering::SeqCst) >= self.max_agents {
            return Err(PoolError::PoolFull {
                current: self.current_count.load(Ordering::SeqCst),
                max: self.max_agents,
            });
        }

        self.local_queues[cpu_id].lock().push_back(agent_id);
        Ok(())
    }

    /// 从本地队列取出下一个 Agent（当前 CPU）
    pub fn dequeue_local(&self, cpu_id: usize) -> Option<AgentId> {
        self.local_queues[cpu_id].lock().pop_front()
    }

    /// 工作窃取：从其他核心窃取 Agent
    pub fn steal_work(&self, thief_cpu: usize) -> Option<AgentId> {
        let cpu_count = smp::cpu_count();

        // 从其他核心的队列尾部窃取一半
        for _ in 0..cpu_count - 1 {
            let victim_cpu = (thief_cpu + 1 + self.stats.total_steals.load(Ordering::Relaxed) as usize)
                % cpu_count;

            if victim_cpu == thief_cpu {
                continue;
            }

            let mut victim_queue = self.local_queues[victim_cpu].lock();
            if victim_queue.len() > 1 {
                // 窃取一半
                let steal_count = victim_queue.len() / 2;
                let stolen: Vec<AgentId> = victim_queue.drain_back(steal_count).collect();

                let mut local_queue = self.local_queues[thief_cpu].lock();
                for agent_id in stolen {
                    local_queue.push_back(agent_id);
                }

                self.stats.total_steals.fetch_add(steal_count as u64, Ordering::Relaxed);

                return local_queue.pop_front();
            }
        }

        // 尝试从全局队列获取
        self.global_queue.lock().pop_front()
    }

    /// 获取下一个待执行的 Agent
    pub fn pick_next_agent(&self, cpu_id: usize) -> Option<Arc<AgentControlBlock>> {
        // 1. 尝试本地队列
        if let Some(agent_id) = self.dequeue_local(cpu_id) {
            return self.agents.lock().get(&agent_id).cloned();
        }

        // 2. 工作窃取
        if let Some(agent_id) = self.steal_work(cpu_id) {
            return self.agents.lock().get(&agent_id).cloned();
        }

        None
    }

    /// 执行负载均衡
    pub fn balance_load(&self) {
        let now = current_time_us();
        let last = self.load_balancer.last_balance_time.load(Ordering::Relaxed);

        if now - last < self.load_balancer.balance_interval_us {
            return;
        }

        self.load_balancer.last_balance_time.store(now, Ordering::Relaxed);

        // 计算平均负载
        let cpu_count = smp::cpu_count();
        let total_load: u64 = (0..cpu_count)
            .map(|i| self.local_queues[i].lock().len() as u64)
            .sum();
        let avg_load = total_load / cpu_count as u64;

        // 将超过平均负载的队列中的 Agent 移到全局队列
        for i in 0..cpu_count {
            let queue_len = self.local_queues[i].lock().len() as u64;
            if queue_len > avg_load * 2 {
                let excess = (queue_len - avg_load) as usize;
                let mut queue = self.local_queues[i].lock();
                let moved: Vec<AgentId> = queue.drain_back(excess.min(queue.len())).collect();
                drop(queue);
                let mut global = self.global_queue.lock();
                for agent_id in moved {
                    global.push_back(agent_id);
                }
            }
        }
    }
}
```

---

## 6. Agent 通信

### 6.1 通信模型

```rust
/// Agent 消息
#[derive(Debug, Clone)]
pub struct AgentMessage {
    /// 消息唯一 ID
    pub id: MessageId,
    /// 发送者
    pub sender: AgentId,
    /// 接收者
    pub receiver: AgentId,
    /// 消息类型
    pub msg_type: MessageType,
    /// 消息主题（用于 Pub/Sub）
    pub topic: Option<String>,
    /// 消息负载
    pub payload: MessagePayload,
    /// 时间戳
    pub timestamp: u64,
    /// 优先级
    pub priority: u8,
    /// TTL（微秒）
    pub ttl_us: u64,
}

/// 消息 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(u64);

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// 直接消息（点对点）
    Direct,
    /// 发布消息（Pub/Sub）
    Publish,
    /// 广播消息
    Broadcast,
    /// 请求-响应
    Request,
    /// 响应
    Response,
    /// 系统消息
    System(SystemMessageType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMessageType {
    AgentSpawned,
    AgentTerminated,
    AgentStateChanged,
    ResourceWarning,
    Heartbeat,
    Shutdown,
}

/// 消息负载
#[derive(Debug, Clone)]
pub enum MessagePayload {
    /// 空负载
    Empty,
    /// 字节数据
    Bytes(Vec<u8>),
    /// 共享内存引用（零拷贝）
    SharedMemory(SharedMemoryRef),
    /// 结构化数据
    Structured(Value),
}

/// 共享内存引用（零拷贝消息传递）
#[derive(Debug, Clone)]
pub struct SharedMemoryRef {
    pub shm_id: SharedMemoryId,
    pub offset: usize,
    pub length: usize,
    pub readonly: bool,
}

/// 通信管理器
pub struct CommManager {
    /// 每个 Agent 的消息队列
    message_queues: SpinLock<BTreeMap<AgentId, MessageQueue>>,
    /// Pub/Sub 主题注册表
    topic_subscribers: SpinLock<BTreeMap<String, Vec<AgentId>>>,
    /// 广播组
    broadcast_groups: SpinLock<BTreeMap<String, Vec<AgentId>>>,
    /// 等待响应的请求映射
    pending_requests: SpinLock<BTreeMap<MessageId, PendingRequest>>,
}

/// 消息队列
pub struct MessageQueue {
    pub queue: VecDeque<AgentMessage>,
    pub max_depth: usize,
    pub notify_waker: Option<Waker>,
}

/// 等待中的请求
struct PendingRequest {
    pub requester: AgentId,
    pub request_id: MessageId,
    pub timeout_us: u64,
    pub submitted_at: u64,
}

impl CommManager {
    /// 发送直接消息
    pub fn send_direct(
        &self,
        sender: AgentId,
        receiver: AgentId,
        payload: MessagePayload,
        priority: u8,
    ) -> Result<MessageId, CommError> {
        // 验证发送权限
        self.verify_send_permission(sender, receiver)?;

        let msg = AgentMessage {
            id: MessageId::new(),
            sender,
            receiver,
            msg_type: MessageType::Direct,
            topic: None,
            payload,
            timestamp: current_time_us(),
            priority,
            ttl_us: 0,
        };

        self.enqueue_message(receiver, msg.clone())?;
        self.update_stats(sender, receiver);

        Ok(msg.id)
    }

    /// 发布消息到主题
    pub fn publish(
        &self,
        sender: AgentId,
        topic: &str,
        payload: MessagePayload,
    ) -> Result<usize, CommError> {
        let subscribers = self.topic_subscribers.lock();
        let agents = subscribers.get(topic)
            .ok_or(CommError::TopicNotFound(topic.to_string()))?;

        let mut delivered = 0;
        for &subscriber in agents {
            if subscriber == sender {
                continue; // 不发送给自己
            }

            let msg = AgentMessage {
                id: MessageId::new(),
                sender,
                receiver: subscriber,
                msg_type: MessageType::Publish,
                topic: Some(topic.to_string()),
                payload: payload.clone(),
                timestamp: current_time_us(),
                priority: 5,
                ttl_us: 0,
            };

            if self.enqueue_message(subscriber, msg).is_ok() {
                delivered += 1;
            }
        }

        Ok(delivered)
    }

    /// 广播消息
    pub fn broadcast(
        &self,
        sender: AgentId,
        group: &str,
        payload: MessagePayload,
    ) -> Result<usize, CommError> {
        let groups = self.broadcast_groups.lock();
        let members = groups.get(group)
            .ok_or(CommError::GroupNotFound(group.to_string()))?;

        let mut delivered = 0;
        for &member in members {
            if member == sender {
                continue;
            }

            let msg = AgentMessage {
                id: MessageId::new(),
                sender,
                receiver: member,
                msg_type: MessageType::Broadcast,
                topic: Some(group.to_string()),
                payload: payload.clone(),
                timestamp: current_time_us(),
                priority: 3,
                ttl_us: 0,
            };

            if self.enqueue_message(member, msg).is_ok() {
                delivered += 1;
            }
        }

        Ok(delivered)
    }

    /// 接收消息（阻塞）
    pub fn receive(
        &self,
        receiver: AgentId,
        timeout_us: Option<u64>,
    ) -> Result<AgentMessage, CommError> {
        let deadline = timeout_us.map(|t| current_time_us() + t);

        loop {
            // 尝试从队列取出消息
            if let Some(msg) = self.dequeue_message(receiver) {
                return Ok(msg);
            }

            // 检查超时
            if let Some(deadline) = deadline {
                if current_time_us() >= deadline {
                    return Err(CommError::ReceiveTimeout);
                }
            }

            // 让出 CPU
            core::hint::spin_loop();
        }
    }

    /// 将消息加入目标 Agent 的队列
    fn enqueue_message(&self, target: AgentId, msg: AgentMessage) -> Result<(), CommError> {
        let mut queues = self.message_queues.lock();
        let queue = queues.get_mut(&target)
            .ok_or(CommError::AgentNotFound(target))?;

        if queue.queue.len() >= queue.max_depth {
            return Err(CommError::QueueFull(target));
        }

        queue.queue.push_back(msg);

        // 唤醒等待的 Agent
        if let Some(waker) = &queue.notify_waker {
            waker.wake();
        }

        Ok(())
    }
}
```

---

## 7. Agent Evolution（进化引擎）

### 7.1 遗传算法

```rust
/// 进化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    /// 评估周期（秒）
    pub evaluation_period_s: u64,
    /// 种群大小
    pub population_size: usize,
    /// 变异率（0-1）
    pub mutation_rate: f64,
    /// 交叉率（0-1）
    pub crossover_rate: f64,
    /// 精英保留数量
    pub elite_count: usize,
    /// 适应度函数权重
    pub fitness_weights: FitnessWeights,
    /// 最大进化代数
    pub max_generations: usize,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            evaluation_period_s: 60,
            population_size: 20,
            mutation_rate: 0.1,
            crossover_rate: 0.7,
            elite_count: 2,
            fitness_weights: FitnessWeights::default(),
            max_generations: 100,
        }
    }
}

/// 适应度权重
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessWeights {
    pub task_completion_rate: f64,  // 任务完成率权重
    pub response_time: f64,         // 响应时间权重
    pub resource_efficiency: f64,   // 资源效率权重
    pub error_rate: f64,            // 错误率权重
    pub knowledge_quality: f64,     // 知识质量权重
}

impl Default for FitnessWeights {
    fn default() -> Self {
        Self {
            task_completion_rate: 0.3,
            response_time: 0.2,
            resource_efficiency: 0.2,
            error_rate: 0.2,
            knowledge_quality: 0.1,
        }
    }
}

/// 进化引擎
pub struct EvolutionEngine {
    /// 当前种群
    population: SpinLock<Vec<Individual>>,
    /// 进化配置
    config: EvolutionConfig,
    /// 当前代数
    generation: AtomicUsize,
    /// 最佳个体
    best_individual: SpinLock<Option<Individual>>,
    /// 进化历史
    history: SpinLock<Vec<GenerationRecord>>,
}

/// 个体（Agent 参数的编码）
#[derive(Debug, Clone)]
pub struct Individual {
    /// 基因组（Agent 参数编码）
    pub genome: Genome,
    /// 适应度评分
    pub fitness: f64,
    /// Agent ID（如果正在运行）
    pub agent_id: Option<AgentId>,
    /// 评估结果详情
    pub evaluation: Option<EvaluationResult>,
}

/// 基因组
#[derive(Debug, Clone)]
pub struct Genome {
    /// 参数编码
    pub parameters: Vec<Gene>,
}

/// 基因
#[derive(Debug, Clone)]
pub enum Gene {
    /// 连续值
    Continuous { name: String, value: f64, min: f64, max: f64 },
    /// 离散值
    Discrete { name: String, value: i64, options: Vec<i64> },
    /// 布尔值
    Boolean { name: String, value: bool },
}

/// 评估结果
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub task_completion_rate: f64,
    pub avg_response_time_us: u64,
    pub resource_efficiency: f64,
    pub error_rate: f64,
    pub knowledge_quality: f64,
    pub overall_fitness: f64,
}

/// 代记录
#[derive(Debug, Clone)]
pub struct GenerationRecord {
    pub generation: usize,
    pub best_fitness: f64,
    pub avg_fitness: f64,
    pub worst_fitness: f64,
    pub diversity: f64,
    pub timestamp_us: u64,
}

impl EvolutionEngine {
    /// 执行一代进化
    pub fn evolve(&self) -> Result<EvolutionReport, EvolutionError> {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst);

        // 1. 评估当前种群
        let mut population = self.population.lock();
        for individual in population.iter_mut() {
            individual.fitness = self.evaluate(individual)?;
        }

        // 2. 记录统计
        let fitnesses: Vec<f64> = population.iter().map(|i| i.fitness).collect();
        let best_fitness = fitnesses.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg_fitness = fitnesses.iter().sum::<f64>() / fitnesses.len() as f64;
        let worst_fitness = fitnesses.iter().cloned().fold(f64::INFINITY, f64::min);

        // 3. 选择（锦标赛选择）
        let selected = self.tournament_select(&population, self.config.population_size);

        // 4. 交叉
        let mut offspring = Vec::new();
        for i in (0..selected.len()).step_by(2) {
            if i + 1 < selected.len() {
                let (child1, child2) = self.crossover(&selected[i], &selected[i + 1]);
                offspring.push(child1);
                offspring.push(child2);
            }
        }

        // 5. 变异
        for individual in &mut offspring {
            self.mutate(individual);
        }

        // 6. 精英保留
        let mut sorted = population.clone();
        sorted.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
        for elite in sorted.iter().take(self.config.elite_count) {
            offspring.push(elite.clone());
        }

        // 7. 更新种群
        *population = offspring;

        // 8. 更新最佳个体
        if let Some(best) = sorted.first() {
            *self.best_individual.lock() = Some(best.clone());
        }

        // 9. 记录历史
        self.history.lock().push(GenerationRecord {
            generation: gen,
            best_fitness,
            avg_fitness,
            worst_fitness,
            diversity: self.calculate_diversity(&population),
            timestamp_us: current_time_us(),
        });

        Ok(EvolutionReport {
            generation: gen,
            best_fitness,
            avg_fitness,
            population_size: population.len(),
        })
    }

    /// 评估个体适应度
    fn evaluate(&self, individual: &Individual) -> Result<f64, EvolutionError> {
        let weights = &self.config.fitness_weights;
        let eval = individual.evaluation.as_ref()
            .ok_or(EvolutionError::NotEvaluated)?;

        let fitness = eval.task_completion_rate * weights.task_completion_rate
            + (1.0 / eval.avg_response_time_us as f64) * weights.response_time
            + eval.resource_efficiency * weights.resource_efficiency
            + (1.0 - eval.error_rate) * weights.error_rate
            + eval.knowledge_quality * weights.knowledge_quality;

        Ok(fitness)
    }

    /// 交叉操作
    fn crossover(&self, parent1: &Individual, parent2: &Individual) -> (Individual, Individual) {
        if rand::random::<f64>() > self.config.crossover_rate {
            return (parent1.clone(), parent2.clone());
        }

        let (genome1, genome2) = self.crossover_genomes(&parent1.genome, &parent2.genome);
        (
            Individual { genome: genome1, fitness: 0.0, agent_id: None, evaluation: None },
            Individual { genome: genome2, fitness: 0.0, agent_id: None, evaluation: None },
        )
    }

    /// 变异操作
    fn mutate(&self, individual: &mut Individual) {
        for gene in &mut individual.genome.parameters {
            if rand::random::<f64>() < self.config.mutation_rate {
                match gene {
                    Gene::Continuous { min, max, value, .. } => {
                        let range = max - min;
                        *value += (rand::random::<f64>() - 0.5) * range * 0.1;
                        *value = value.clamp(*min, *max);
                    }
                    Gene::Discrete { options, value, .. } => {
                        if !options.is_empty() {
                            let idx = rand::random::<usize>() % options.len();
                            *value = options[idx];
                        }
                    }
                    Gene::Boolean { value, .. } => {
                        *value = !*value;
                    }
                }
            }
        }
    }

    /// 锦标赛选择
    fn tournament_select(&self, population: &[Individual], count: usize) -> Vec<Individual> {
        let mut selected = Vec::with_capacity(count);
        let tournament_size = 3;

        for _ in 0..count {
            let mut best = None;
            for _ in 0..tournament_size {
                let idx = rand::random::<usize>() % population.len();
                match best {
                    None => best = Some(population[idx].clone()),
                    Some(ref mut b) if population[idx].fitness > b.fitness => {
                        *b = population[idx].clone();
                    }
                    _ => {}
                }
            }
            selected.push(best.unwrap());
        }

        selected
    }
}
```

---

## 8. 知识共享

### 8.1 知识共享协议

```rust
/// 知识库 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnowledgeBaseId(u64);

/// 知识条目
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub id: KnowledgeEntryId,
    pub key: String,
    pub value: KnowledgeValue,
    pub source_agent: AgentId,
    pub created_at: u64,
    pub updated_at: u64,
    pub access_count: AtomicU64,
    pub confidence: f64,
    pub ttl_us: u64,
}

#[derive(Debug, Clone)]
pub enum KnowledgeValue {
    Text(String),
    Binary(Vec<u8>),
    Structured(Value),
    Reference(KnowledgeRef),
}

/// 知识引用（跨 Agent 共享）
#[derive(Debug, Clone)]
pub struct KnowledgeRef {
    pub knowledge_base_id: KnowledgeBaseId,
    pub entry_id: KnowledgeEntryId,
    pub version: u64,
}

/// 知识共享管理器
pub struct KnowledgeSharingManager {
    /// 知识库集合
    knowledge_bases: SpinLock<BTreeMap<KnowledgeBaseId, KnowledgeBase>>,
    /// 传输队列
    transfer_queue: SpinLock<VecDeque<KnowledgeTransfer>>,
}

/// 知识库
pub struct KnowledgeBase {
    pub id: KnowledgeBaseId,
    pub name: String,
    pub entries: BTreeMap<KnowledgeEntryId, KnowledgeEntry>,
    pub owner: AgentId,
    pub access_control: KnowledgeAccessControl,
}

/// 知识访问控制
pub struct KnowledgeAccessControl {
    pub readers: Vec<AgentId>,
    pub writers: Vec<AgentId>,
    pub public_read: bool,
}

/// 知识传输
struct KnowledgeTransfer {
    pub source_agent: AgentId,
    pub target_agent: AgentId,
    pub entries: Vec<KnowledgeEntry>,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, Copy)]
pub enum TransferStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl KnowledgeSharingManager {
    /// 写入知识
    pub fn write_knowledge(
        &self,
        agent_id: AgentId,
        kb_id: KnowledgeBaseId,
        key: &str,
        value: KnowledgeValue,
    ) -> Result<KnowledgeEntryId, KnowledgeError> {
        let mut bases = self.knowledge_bases.lock();
        let kb = bases.get_mut(&kb_id)
            .ok_or(KnowledgeError::KnowledgeBaseNotFound(kb_id))?;

        // 验证写权限
        if !kb.access_control.public_read && !kb.access_control.writers.contains(&agent_id) {
            return Err(KnowledgeError::PermissionDenied);
        }

        let entry_id = KnowledgeEntryId::new();
        let entry = KnowledgeEntry {
            id: entry_id,
            key: key.to_string(),
            value,
            source_agent: agent_id,
            created_at: current_time_us(),
            updated_at: current_time_us(),
            access_count: AtomicU64::new(0),
            confidence: 1.0,
            ttl_us: 0,
        };

        kb.entries.insert(entry_id, entry);
        Ok(entry_id)
    }

    /// 读取知识
    pub fn read_knowledge(
        &self,
        agent_id: AgentId,
        kb_id: KnowledgeBaseId,
        key: &str,
    ) -> Result<KnowledgeEntry, KnowledgeError> {
        let bases = self.knowledge_bases.lock();
        let kb = bases.get(&kb_id)
            .ok_or(KnowledgeError::KnowledgeBaseNotFound(kb_id))?;

        // 验证读权限
        if !kb.access_control.public_read && !kb.access_control.readers.contains(&agent_id) {
            return Err(KnowledgeError::PermissionDenied);
        }

        kb.entries.values()
            .find(|e| e.key == key)
            .cloned()
            .ok_or(KnowledgeError::EntryNotFound(key.to_string()))
    }

    /// 跨 Agent 知识传输
    pub fn transfer_knowledge(
        &self,
        source: AgentId,
        target: AgentId,
        kb_id: KnowledgeBaseId,
        keys: &[String],
    ) -> Result<u32, KnowledgeError> {
        let bases = self.knowledge_bases.lock();
        let kb = bases.get(&kb_id)
            .ok_or(KnowledgeError::KnowledgeBaseNotFound(kb_id))?;

        let entries: Vec<KnowledgeEntry> = kb.entries.values()
            .filter(|e| keys.contains(&e.key))
            .cloned()
            .collect();

        let count = entries.len() as u32;

        // 加入传输队列
        self.transfer_queue.lock().push_back(KnowledgeTransfer {
            source_agent: source,
            target_agent: target,
            entries,
            status: TransferStatus::Pending,
        });

        Ok(count)
    }
}
```

---

## 9. libagent 用户空间库

### 9.1 API 定义

```rust
//! libagent - OmniAgent OS Agent 用户空间库
//!
//! 提供创建、管理和通信 Agent 的高级 API

/// Agent 句柄
pub struct Agent {
    id: AgentId,
    state: AgentState,
    inner: Arc<AgentInner>,
}

struct AgentInner {
    tx: Sender<AgentRequest>,
    rx: Receiver<AgentEvent>,
}

/// Agent 构建器
pub struct AgentBuilder {
    spec: AgentSpec,
}

impl AgentBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            spec: AgentSpec {
                agent_type: AgentType::General,
                capabilities: Vec::new(),
                resource_quota: ResourceQuota::default(),
                knowledge_config: KnowledgeConfig::default(),
                priority: AgentPriority::Normal,
                security_label: SecurityLabel::default(),
                comm_config: CommConfig::default(),
                evolution_config: None,
                timeout: TimeoutConfig::default(),
                env: BTreeMap::new(),
                params: BTreeMap::new(),
            },
        }
    }

    pub fn agent_type(mut self, t: AgentType) -> Self { self.spec.agent_type = t; self }
    pub fn capability(mut self, cap: Capability) -> Self { self.spec.capabilities.push(cap); self }
    pub fn priority(mut self, p: AgentPriority) -> Self { self.spec.priority = p; self }
    pub fn memory_limit(mut self, bytes: u64) -> Self { self.spec.resource_quota.max_memory_bytes = bytes; self }
    pub fn cpu_quota(mut self, ns: u64) -> Self { self.spec.resource_quota.cpu_time_quota_ns = ns; self }
    pub fn subscribe(mut self, topic: &str) -> Self { self.spec.comm_config.subscriptions.push(topic.to_string()); self }
    pub fn enable_evolution(mut self, config: EvolutionConfig) -> Self { self.spec.evolution_config = Some(config); self }

    /// 构建 Agent
    pub fn build(self) -> Result<Agent, AgentError> {
        let id = unsafe { sys_agent_spawn(&self.spec)? };
        Ok(Agent {
            id,
            state: AgentState::Created,
            inner: Arc::new(AgentInner::new(id)),
        })
    }
}

impl Agent {
    /// 启动 Agent
    pub fn start(&mut self) -> Result<(), AgentError> {
        self.transition(AgentState::Running)?;
        unsafe { sys_agent_start(self.id)? };
        Ok(())
    }

    /// 暂停 Agent
    pub fn pause(&mut self) -> Result<(), AgentError> {
        self.transition(AgentState::Paused)?;
        unsafe { sys_agent_pause(self.id)? };
        Ok(())
    }

    /// 恢复 Agent
    pub fn resume(&mut self) -> Result<(), AgentError> {
        self.transition(AgentState::Running)?;
        unsafe { sys_agent_resume(self.id)? };
        Ok(())
    }

    /// 终止 Agent
    pub fn kill(&mut self) -> Result<(), AgentError> {
        self.transition(AgentState::Stopped)?;
        unsafe { sys_agent_kill(self.id)? };
        Ok(())
    }

    /// 发送消息
    pub fn send(&self, target: AgentId, payload: &[u8]) -> Result<MessageId, AgentError> {
        let msg_id = unsafe {
            sys_agent_send(self.id, target, payload.as_ptr(), payload.len())
        }?;
        Ok(msg_id)
    }

    /// 接收消息
    pub fn receive(&self, timeout_ms: Option<u64>) -> Result<AgentMessage, AgentError> {
        unsafe {
            sys_agent_receive(self.id, timeout_ms.unwrap_or(0))
        }
    }

    /// 发布到主题
    pub fn publish(&self, topic: &str, payload: &[u8]) -> Result<usize, AgentError> {
        let topic_c = CString::new(topic)?;
        let count = unsafe {
            sys_agent_publish(self.id, topic_c.as_ptr(), payload.as_ptr(), payload.len())
        }?;
        Ok(count)
    }

    /// 订阅主题
    pub fn subscribe(&self, topic: &str) -> Result<(), AgentError> {
        let topic_c = CString::new(topic)?;
        unsafe { sys_agent_subscribe(self.id, topic_c.as_ptr()) }?;
        Ok(())
    }

    /// 获取统计信息
    pub fn stats(&self) -> Result<AgentStats, AgentError> {
        unsafe { sys_agent_stats(self.id) }
    }

    /// 等待 Agent 终止
    pub fn wait(&self) -> Result<(), AgentError> {
        loop {
            let state = unsafe { sys_agent_state(self.id)? };
            if matches!(state, AgentState::Stopped | AgentState::Destroyed) {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    fn transition(&mut self, target: AgentState) -> Result<(), AgentError> {
        if !self.state.can_transition_to(target) {
            return Err(AgentError::InvalidStateTransition {
                from: self.state,
                to: target,
            });
        }
        self.state = target;
        Ok(())
    }
}

/// 系统调用封装
mod sys {
    pub unsafe fn sys_agent_spawn(spec: &AgentSpec) -> Result<AgentId, AgentError>;
    pub unsafe fn sys_agent_start(id: AgentId) -> Result<(), AgentError>;
    pub unsafe fn sys_agent_pause(id: AgentId) -> Result<(), AgentError>;
    pub unsafe fn sys_agent_resume(id: AgentId) -> Result<(), AgentError>;
    pub unsafe fn sys_agent_kill(id: AgentId) -> Result<(), AgentError>;
    pub unsafe fn sys_agent_send(sender: AgentId, receiver: AgentId, data: *const u8, len: usize) -> Result<MessageId, AgentError>;
    pub unsafe fn sys_agent_receive(id: AgentId, timeout_ms: u64) -> Result<AgentMessage, AgentError>;
    pub unsafe fn sys_agent_publish(id: AgentId, topic: *const i8, data: *const u8, len: usize) -> Result<usize, AgentError>;
    pub unsafe fn sys_agent_subscribe(id: AgentId, topic: *const i8) -> Result<(), AgentError>;
    pub unsafe fn sys_agent_stats(id: AgentId) -> Result<AgentStats, AgentError>;
    pub unsafe fn sys_agent_state(id: AgentId) -> Result<AgentState, AgentError>;
}
```

---

## 10. 错误处理

### 10.1 错误类型

```rust
/// Agent 运行时错误
#[derive(Debug, Clone)]
pub enum AgentError {
    /// 无效的状态转换
    InvalidStateTransition { from: AgentState, to: AgentState },
    /// Agent 未找到
    NotFound(AgentId),
    /// Agent 创建失败
    SpawnFailed(SpawnFailureReason),
    /// 内存不足
    OutOfMemory { requested: u64, available: u64 },
    /// 操作超时
    Timeout { operation: String, elapsed_us: u64, deadline_us: u64 },
    /// 能力不足
    CapabilityDenied { required: String, possessed: Vec<String> },
    /// 资源配额超限
    QuotaExceeded { resource: String, current: u64, limit: u64 },
    /// 消息发送失败
    SendFailed(CommError),
    /// 消息接收失败
    ReceiveFailed(CommError),
    /// 安全违规
    SecurityViolation(String),
    /// 系统错误
    SystemError(i32),
}

/// 创建失败原因
#[derive(Debug, Clone)]
pub enum SpawnFailureReason {
    /// Agent 池已满
    PoolFull,
    /// 内存分配失败
    MemoryAllocationFailed,
    /// 安全验证失败
    SecurityCheckFailed,
    /// 能力验证失败
    CapabilityVerificationFailed,
    /// 资源配额不足
    InsufficientQuota,
    /// 二进制文件不存在
    BinaryNotFound(String),
    /// 配置无效
    InvalidConfig(String),
}

/// 通信错误
#[derive(Debug, Clone)]
pub enum CommError {
    /// Agent 未找到
    AgentNotFound(AgentId),
    /// 队列已满
    QueueFull(AgentId),
    /// 主题不存在
    TopicNotFound(String),
    /// 广播组不存在
    GroupNotFound(String),
    /// 接收超时
    ReceiveTimeout,
    /// 发送权限不足
    PermissionDenied,
    /// 消息过大
    MessageTooLarge { size: usize, max: usize },
    /// 无效消息
    InvalidMessage(String),
}

/// Agent 池错误
#[derive(Debug, Clone)]
pub enum PoolError {
    /// 池已满
    PoolFull { current: usize, max: usize },
    /// Agent 已存在
    AlreadyExists(AgentId),
    /// Agent 不在池中
    NotInPool(AgentId),
}

/// 工厂错误
#[derive(Debug, Clone)]
pub enum FactoryError {
    /// 未知领域
    UnknownDomain(String),
    /// 工具不可用
    ToolUnavailable(String),
    /// 知识库不可用
    KnowledgeUnavailable(KnowledgeBaseId),
    /// 组装超时
    AssemblyTimeout,
    /// 安全约束冲突
    SecurityConflict(String),
}

/// 知识错误
#[derive(Debug, Clone)]
pub enum KnowledgeError {
    /// 知识库未找到
    KnowledgeBaseNotFound(KnowledgeBaseId),
    /// 条目未找到
    EntryNotFound(String),
    /// 权限不足
    PermissionDenied,
    /// 传输失败
    TransferFailed,
    /// 数据损坏
    CorruptedData,
}

/// 进化错误
#[derive(Debug, Clone)]
pub enum EvolutionError {
    /// 未评估
    NotEvaluated,
    /// 种群为空
    EmptyPopulation,
    /// 达到最大代数
    MaxGenerationsReached,
    /// 适应度评估失败
    EvaluationFailed(String),
}
```

### 10.2 错误码表

| 错误码 | 名称 | 严重性 | 可恢复 |
|--------|------|--------|--------|
| A001 | InvalidStateTransition | 高 | 否 |
| A002 | NotFound | 中 | 否 |
| A003 | SpawnFailed | 高 | 是（重试） |
| A004 | OutOfMemory | 致命 | 是（OOM handler） |
| A005 | Timeout | 中 | 是（重试） |
| A006 | CapabilityDenied | 高 | 否 |
| A007 | QuotaExceeded | 中 | 否 |
| A008 | SecurityViolation | 致命 | 否 |
| C001 | QueueFull | 低 | 是（等待） |
| C002 | ReceiveTimeout | 低 | 是（重试） |
| E001 | EmptyPopulation | 中 | 否 |
| K001 | PermissionDenied | 高 | 否 |

---

## 11. 安全机制

### 11.1 能力验证

```rust
/// 能力验证器
pub struct CapabilityVerifier {
    /// 全局能力注册表
    registry: CapabilityRegistry,
}

impl CapabilityVerifier {
    /// 验证 Agent 是否具有指定能力
    pub fn verify(
        &self,
        agent: &AgentControlBlock,
        required: &Capability,
    ) -> Result<CapabilityProof, SecurityError> {
        // 1. 检查 Agent 是否声明了该能力
        let has_capability = agent.spec.capabilities.iter().any(|c| {
            c.name == required.name && c.version >= required.version
        });

        if !has_capability {
            return Err(SecurityError::CapabilityDenied {
                agent: agent.id,
                required: required.name.clone(),
            });
        }

        // 2. 检查安全标签是否允许
        if required.required_permissions.iter().any(|p| {
            !self.permission_allowed(agent.security_ctx.trust_level, *p)
        }) {
            return Err(SecurityError::PermissionDenied);
        }

        // 3. 生成能力证明
        Ok(CapabilityProof {
            agent_id: agent.id,
            capability: required.name.clone(),
            granted_at: current_time_us(),
            expires_at: current_time_us() + 3600_000_000, // 1小时
        })
    }

    /// 检查权限是否被允许
    fn permission_allowed(&self, trust_level: SecurityLevel, perm: Permission) -> bool {
        match trust_level {
            SecurityLevel::Untrusted => false,
            SecurityLevel::Low => matches!(perm,
                Permission::ReadFile | Permission::AgentCommunicate
            ),
            SecurityLevel::Medium => !matches!(perm,
                Permission::KernelModule | Permission::SystemAdmin | Permission::RawIo
            ),
            SecurityLevel::High => !matches!(perm,
                Permission::KernelModule | Permission::SystemAdmin
            ),
            SecurityLevel::Critical => true,
        }
    }
}
```

### 11.2 资源限制执行

```rust
/// 资源限制执行器
pub struct ResourceEnforcer {
    /// 每个 Agent 的资源使用追踪
    usage: SpinLock<BTreeMap<AgentId, ResourceUsage>>,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub cpu_time_used_ns: u64,
    pub memory_used_bytes: u64,
    pub shared_memory_used_bytes: u64,
    pub open_files: usize,
    pub child_agents: usize,
    pub threads: usize,
    pub messages_in_queue: usize,
    pub network_bytes_sent: u64,
    pub io_bytes: u64,
}

impl ResourceEnforcer {
    /// 在操作前检查资源限制
    pub fn check_before_alloc(
        &self,
        agent_id: AgentId,
        resource: ResourceType,
        amount: u64,
    ) -> Result<(), AgentError> {
        let usage = self.usage.lock().get(&agent_id)
            .ok_or(AgentError::NotFound(agent_id))?;

        let agent = AGENT_POOL.get_agent(agent_id)?;
        let quota = &agent.spec.resource_quota;

        match resource {
            ResourceType::Memory => {
                if usage.memory_used_bytes + amount > quota.max_memory_bytes {
                    return Err(AgentError::QuotaExceeded {
                        resource: "memory".into(),
                        current: usage.memory_used_bytes,
                        limit: quota.max_memory_bytes,
                    });
                }
            }
            ResourceType::SharedMemory => {
                if usage.shared_memory_used_bytes + amount > quota.max_shared_memory_bytes {
                    return Err(AgentError::QuotaExceeded {
                        resource: "shared_memory".into(),
                        current: usage.shared_memory_used_bytes,
                        limit: quota.max_shared_memory_bytes,
                    });
                }
            }
            ResourceType::ChildAgent => {
                if usage.child_agents as u64 + amount > quota.max_child_agents as u64 {
                    return Err(AgentError::QuotaExceeded {
                        resource: "child_agents".into(),
                        current: usage.child_agents as u64,
                        limit: quota.max_child_agents as u64,
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }
}
```

---

## 12. 性能约束

### 12.1 性能目标

| 指标 | 目标值 | 测量条件 |
|------|--------|----------|
| 最大并发 Agent 数 | > 100 | 8核系统 |
| Agent 创建延迟 | < 10ms | 包含进程创建 + 页表设置 |
| Agent 销毁延迟 | < 5ms | 包含资源回收 |
| 消息传递延迟 | < 100us | 同核直接消息 |
| 消息传递延迟（跨核） | < 500us | 跨核直接消息 |
| 广播延迟 | < 1ms | 100 个接收者 |
| Pub/Sub 发布延迟 | < 200us | 50 个订阅者 |
| 工作窃取延迟 | < 50us | 跨核窃取 |
| Expert Factory 组装 | < 50ms | 包含意图解析 + 工具选择 |
| 进化一代耗时 | < 100ms | 20 个体种群 |
| 知识写入 | < 10us | 单条目 |
| 知识读取 | < 5us | 单条目缓存命中 |

### 12.2 性能优化策略

1. **无锁消息队列**: 使用 per-CPU 无锁队列减少消息传递延迟
2. **批量工作窃取**: 每次窃取半个队列而非单个 Agent
3. **共享内存消息**: 大消息使用共享内存引用而非复制
4. **Agent 预分配**: 预分配 Agent 控制块和页表结构
5. **知识缓存**: 热点知识条目缓存在 Agent 本地

---

## 13. 测试用例

### 13.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_transitions() {
        // 合法转换
        assert!(AgentState::Created.can_transition_to(AgentState::Configured));
        assert!(AgentState::Configured.can_transition_to(AgentState::Running));
        assert!(AgentState::Running.can_transition_to(AgentState::Paused));
        assert!(AgentState::Paused.can_transition_to(AgentState::Running));
        assert!(AgentState::Running.can_transition_to(AgentState::Stopping));
        assert!(AgentState::Stopping.can_transition_to(AgentState::Stopped));
        assert!(AgentState::Stopped.can_transition_to(AgentState::Configured));
        assert!(AgentState::Error.can_transition_to(AgentState::Running));

        // 非法转换
        assert!(!AgentState::Created.can_transition_to(AgentState::Running));
        assert!(!AgentState::Running.can_transition_to(AgentState::Created));
        assert!(!AgentState::Destroyed.can_transition_to(AgentState::Running));
    }

    #[test]
    fn test_atomic_agent_state() {
        let state = AtomicAgentState::new(AgentState::Created);
        assert_eq!(state.load(Ordering::SeqCst), AgentState::Created);

        let result = state.compare_exchange(
            AgentState::Created, AgentState::Configured,
            Ordering::SeqCst, Ordering::SeqCst,
        );
        assert!(result.is_ok());
        assert_eq!(state.load(Ordering::SeqCst), AgentState::Configured);
    }

    #[test]
    fn test_agent_builder() {
        let agent = AgentBuilder::new("test-agent")
            .agent_type(AgentType::Expert)
            .capability(Capability::file_read())
            .priority(AgentPriority::High)
            .memory_limit(128 * 1024 * 1024)
            .subscribe("test-topic")
            .build();

        assert!(agent.is_ok());
        let agent = agent.unwrap();
        assert_eq!(agent.state, AgentState::Created);
    }

    #[test]
    fn test_resource_quota_default() {
        let quota = ResourceQuota::default();
        assert_eq!(quota.max_memory_bytes, 256 * 1024 * 1024);
        assert_eq!(quota.max_child_agents, 32);
        assert_eq!(quota.max_threads, 8);
    }

    #[test]
    fn test_agent_pool_enqueue_dequeue() {
        let pool = AgentPool::new(100);
        let agent_id = AgentId::new(1);

        assert!(pool.enqueue(agent_id, 0).is_ok());
        let retrieved = pool.dequeue_local(0);
        assert_eq!(retrieved, Some(agent_id));
    }

    #[test]
    fn test_agent_pool_full() {
        let pool = AgentPool::new(1);
        assert!(pool.enqueue(AgentId::new(1), 0).is_ok());
        assert!(matches!(pool.enqueue(AgentId::new(2), 0), Err(PoolError::PoolFull { .. })));
    }

    #[test]
    fn test_message_direct_send() {
        let comm = CommManager::new();
        let sender = AgentId::new(1);
        let receiver = AgentId::new(2);

        // 注册接收者
        comm.register_agent(receiver, 1024);

        let result = comm.send_direct(sender, receiver, MessagePayload::Empty, 5);
        assert!(result.is_ok());

        let msg = comm.dequeue_message(receiver);
        assert!(msg.is_some());
        assert_eq!(msg.unwrap().sender, sender);
    }

    #[test]
    fn test_message_queue_full() {
        let comm = CommManager::new();
        let sender = AgentId::new(1);
        let receiver = AgentId::new(2);

        comm.register_agent(receiver, 2); // 队列深度为 2

        assert!(comm.send_direct(sender, receiver, MessagePayload::Empty, 5).is_ok());
        assert!(comm.send_direct(sender, receiver, MessagePayload::Empty, 5).is_ok());
        assert!(matches!(
            comm.send_direct(sender, receiver, MessagePayload::Empty, 5),
            Err(CommError::QueueFull(_))
        ));
    }

    #[test]
    fn test_capability_verification() {
        let verifier = CapabilityVerifier::new();
        let agent = create_test_agent_with_capabilities(&["file.read"]);

        // 具有能力
        let result = verifier.verify(&agent, &Capability::file_read());
        assert!(result.is_ok());

        // 不具有能力
        let result = verifier.verify(&agent, &Capability::network_tcp());
        assert!(matches!(result, Err(SecurityError::CapabilityDenied { .. })));
    }

    #[test]
    fn test_expert_factory_assembly() {
        let factory = ExpertFactory::new_test();

        let request = AssemblyRequest {
            intent: "监控网络连接状态".to_string(),
            domain: Some("network".to_string()),
            tools: vec![],
            knowledge_bases: vec![],
            capabilities: vec![],
            resource_limits: None,
            security_constraints: vec![],
        };

        let result = factory.assemble(&request);
        assert!(result.is_ok());
        let assembly = result.unwrap();
        assert!(assembly.confidence > 0.5);
        assert_eq!(assembly.spec.agent_type, AgentType::Expert);
    }

    #[test]
    fn test_evolution_crossover() {
        let engine = EvolutionEngine::new(EvolutionConfig::default());
        let parent1 = create_test_individual(0.8);
        let parent2 = create_test_individual(0.6);

        let (child1, child2) = engine.crossover(&parent1, &parent2);
        // 子代应与父代不同（大多数情况下）
        assert!(child1.genome.parameters.len() > 0);
        assert!(child2.genome.parameters.len() > 0);
    }

    #[test]
    fn test_evolution_mutation() {
        let engine = EvolutionEngine::new(EvolutionConfig {
            mutation_rate: 1.0, // 100% 变异率
            ..EvolutionConfig::default()
        });

        let mut individual = create_test_individual(0.5);
        let original = individual.genome.clone();
        engine.mutate(&mut individual);

        // 100% 变异率下至少有一个基因应改变
        let changed = individual.genome.parameters.iter().zip(original.parameters.iter())
            .any(|(new, old)| new != old);
        assert!(changed);
    }

    #[test]
    fn test_knowledge_write_read() {
        let manager = KnowledgeSharingManager::new();
        let kb_id = manager.create_knowledge_base("test_kb", AgentId::new(1));

        let entry_id = manager.write_knowledge(
            AgentId::new(1),
            kb_id,
            "network.latency",
            KnowledgeValue::Text("42ms".to_string()),
        ).unwrap();

        let entry = manager.read_knowledge(AgentId::new(1), kb_id, "network.latency").unwrap();
        assert_eq!(entry.id, entry_id);
    }
}
```

### 13.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_agent_lifecycle_full() {
        let mut agent = AgentBuilder::new("lifecycle-test")
            .agent_type(AgentType::General)
            .build().unwrap();

        assert_eq!(agent.state, AgentState::Created);

        agent.start().unwrap();
        assert_eq!(agent.state, AgentState::Running);

        agent.pause().unwrap();
        assert_eq!(agent.state, AgentState::Paused);

        agent.resume().unwrap();
        assert_eq!(agent.state, AgentState::Running);

        agent.kill().unwrap();
        assert_eq!(agent.state, AgentState::Stopped);
    }

    #[test]
    fn test_agent_communication_roundtrip() {
        let mut sender = AgentBuilder::new("sender").build().unwrap();
        let mut receiver = AgentBuilder::new("receiver")
            .subscribe("test-topic")
            .build().unwrap();

        sender.start().unwrap();
        receiver.start().unwrap();

        // 直接消息
        sender.send(receiver.id, b"hello").unwrap();
        let msg = receiver.receive(Some(1000)).unwrap();
        assert_eq!(msg.sender, sender.id);

        // Pub/Sub
        sender.publish("test-topic", b"broadcast").unwrap();
        let msg = receiver.receive(Some(1000)).unwrap();
        assert_eq!(msg.topic.as_deref(), Some("test-topic"));
    }

    #[test]
    fn test_concurrent_agents() {
        let mut agents: Vec<Agent> = (0..100)
            .map(|i| {
                AgentBuilder::new(&format!("agent-{}", i))
                    .priority(AgentPriority::Low)
                    .memory_limit(16 * 1024 * 1024)
                    .build()
                    .unwrap()
            })
            .collect();

        // 并发启动
        for agent in &mut agents {
            agent.start().unwrap();
        }

        // 验证所有 Agent 都在运行
        for agent in &agents {
            assert_eq!(agent.state, AgentState::Running);
        }

        // 并发终止
        for agent in &mut agents {
            agent.kill().unwrap();
        }
    }

    #[test]
    fn test_agent_pool_work_stealing() {
        let pool = AgentPool::new(100);

        // 在 CPU 0 上加载大量 Agent
        for i in 0..50 {
            pool.enqueue(AgentId::new(i), 0).unwrap();
        }

        // CPU 1 应能窃取工作
        let stolen = pool.steal_work(1);
        assert!(stolen.is_some());
    }

    #[test]
    fn test_resource_quota_enforcement() {
        let enforcer = ResourceEnforcer::new();
        let agent = AgentBuilder::new("quota-test")
            .memory_limit(1024 * 1024) // 1MB
            .build().unwrap();

        agent.start().unwrap();

        // 在限额内
        assert!(enforcer.check_before_alloc(agent.id, ResourceType::Memory, 512 * 1024).is_ok());

        // 超出限额
        assert!(matches!(
            enforcer.check_before_alloc(agent.id, ResourceType::Memory, 2 * 1024 * 1024),
            Err(AgentError::QuotaExceeded { .. })
        ));
    }
}
```

### 13.3 压力测试

```rust
#[cfg(test)]
mod stress_tests {
    #[test]
    fn test_100_agents_spawn_kill_cycle() {
        for cycle in 0..10 {
            let mut agents: Vec<Agent> = Vec::new();

            // 创建 100 个 Agent
            for i in 0..100 {
                let agent = AgentBuilder::new(&format!("stress-{}-{}", cycle, i))
                    .memory_limit(8 * 1024 * 1024)
                    .build().unwrap();
                agents.push(agent);
            }

            // 启动所有
            for agent in &mut agents {
                agent.start().unwrap();
            }

            // 发送消息
            for i in 0..agents.len() - 1 {
                agents[i].send(agents[i + 1].id, b"ping").unwrap();
            }

            // 终止所有
            for agent in &mut agents {
                agent.kill().unwrap();
            }
        }
    }

    #[test]
    fn test_message_throughput() {
        let mut sender = AgentBuilder::new("throughput-sender").build().unwrap();
        let mut receiver = AgentBuilder::new("throughput-receiver")
            .build().unwrap();

        sender.start().unwrap();
        receiver.start().unwrap();

        let message_count = 10_000;
        let start = current_time_us();

        for _ in 0..message_count {
            sender.send(receiver.id, b"throughput-test").unwrap();
        }

        let elapsed = current_time_us() - start;
        let avg_latency_us = elapsed / message_count;

        assert!(avg_latency_us < 100, "平均消息延迟 {}us 超过 100us", avg_latency_us);
    }

    #[test]
    fn test_evolution_convergence() {
        let engine = EvolutionEngine::new(EvolutionConfig {
            population_size: 50,
            mutation_rate: 0.05,
            max_generations: 200,
            ..EvolutionConfig::default()
        });

        let mut best_fitness = 0.0;

        for gen in 0..200 {
            let report = engine.evolve().unwrap();
            if report.best_fitness > best_fitness {
                best_fitness = report.best_fitness;
            }
        }

        // 适应度应随代数增长
        assert!(best_fitness > 0.5, "进化未收敛，最佳适应度: {}", best_fitness);
    }
}
```

---

## 14. 参考资料

- POSIX Agent 模式参考
- Erlang/OTP 进程模型
- Akka Actor 框架
- Capability-based Security (Dennis & Van Horn, 1966)
- Genetic Algorithms in Scheduling (Holland, 1975)
- Work-Stealing Scheduling (Blumofe & Leiserson, 1999)
