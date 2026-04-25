# Agent API 参考

> **模块名称**: `agent-api`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

Agent API 是 OmniAgent OS 的核心接口，提供 Agent 全生命周期管理、通信、查询、进化、知识管理和任务调度等功能。所有 Agent 操作均通过此 API 进行，支持同步和异步两种调用模式。API 采用 Rust trait 抽象，便于实现不同的后端（本地、远程、混合）。

### 1.2 架构概览

```
┌──────────────────────────────────────────────────────┐
│                    Agent API                         │
├──────────┬──────────┬──────────┬─────────────────────┤
│Lifecycle │  Comm    │  Query   │  Evolution          │
├──────────┼──────────┼──────────┼─────────────────────┤
│Knowledge │  Expert  │  Pool    │  Cloud AI           │
├──────────┴──────────┴──────────┴─────────────────────┤
│              Agent Runtime (内核级)                   │
└──────────────────────────────────────────────────────┘
```

---

## 2. Agent 生命周期 API

### 2.1 接口定义

```rust
use std::collections::HashMap;
use std::time::Duration;

/// Agent 标识符
pub type AgentId = u64;

/// Agent 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// 已创建，未启动
    Created,
    /// 正在运行
    Running,
    /// 已暂停
    Paused,
    /// 正在停止
    Stopping,
    /// 已停止
    Stopped,
    /// 出错
    Error,
}

/// Agent 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    /// 通用 Agent
    General,
    /// 专家 Agent
    Expert,
    /// 工具 Agent
    Tool,
    /// 监控 Agent
    Monitor,
    /// 调度 Agent
    Scheduler,
}

/// Agent 配置
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Agent 名称
    pub name: String,
    /// Agent 类型
    pub agent_type: AgentType,
    /// 优先级 (0-255)
    pub priority: u8,
    /// 内存限制（字节）
    pub memory_limit: u64,
    /// CPU 亲和性
    pub cpu_affinity: Option<Vec<usize>>,
    /// 超时时间
    pub timeout: Option<Duration>,
    /// 环境变量
    pub env: HashMap<String, String>,
    /// 启动参数
    pub params: HashMap<String, String>,
    /// 模型配置
    pub model_config: Option<ModelConfig>,
    /// 能力声明
    pub capabilities: Vec<String>,
}

/// 模型配置
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// 模型名称
    pub model_name: String,
    /// 推理偏好
    pub preference: InferencePreference,
    /// 温度参数
    pub temperature: f32,
    /// 最大 token 数
    pub max_tokens: u32,
    /// Top-P 采样
    pub top_p: Option<f32>,
    /// Top-K 采样
    pub top_k: Option<u32>,
}

/// 推理偏好
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferencePreference {
    /// 优先本地推理
    Local,
    /// 优先云端推理
    Cloud,
    /// 自动选择
    Auto,
}

/// Agent 生命周期管理 trait
pub trait AgentLifecycle: Send + Sync {
    /// 创建 Agent 实例
    fn spawn(&self, config: AgentConfig) -> Result<AgentId, AgentError>;

    /// 配置 Agent（运行时修改配置）
    fn configure(&self, id: AgentId, config: AgentConfig) -> Result<(), AgentError>;

    /// 启动 Agent
    fn start(&self, id: AgentId) -> Result<(), AgentError>;

    /// 暂停 Agent
    fn pause(&self, id: AgentId) -> Result<(), AgentError>;

    /// 恢复 Agent
    fn resume(&self, id: AgentId) -> Result<(), AgentError>;

    /// 优雅停止 Agent
    fn stop(&self, id: AgentId) -> Result<(), AgentError>;

    /// 强制终止 Agent
    fn kill(&self, id: AgentId) -> Result<(), AgentError>;
}
```

### 2.2 默认实现

```rust
/// Agent 生命周期管理器
pub struct AgentLifecycleManager {
    agents: HashMap<AgentId, AgentInstance>,
    next_id: AtomicU64,
}

use std::sync::atomic::AtomicU64;

struct AgentInstance {
    config: AgentConfig,
    state: AgentState,
    created_at: std::time::Instant,
}

impl AgentLifecycle for AgentLifecycleManager {
    fn spawn(&self, config: AgentConfig) -> Result<AgentId, AgentError> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let instance = AgentInstance {
            config,
            state: AgentState::Created,
            created_at: std::time::Instant::now(),
        };
        self.agents.insert(id, instance);
        Ok(id)
    }

    fn start(&self, id: AgentId) -> Result<(), AgentError> {
        let agent = self.agents.get(&id)
            .ok_or(AgentError::NotFound(id))?;
        if agent.state != AgentState::Created && agent.state != AgentState::Paused {
            return Err(AgentError::InvalidState {
                id,
                expected: "Created or Paused".to_string(),
                actual: format!("{:?}", agent.state),
            });
        }
        // 启动 Agent 运行时...
        Ok(())
    }

    fn pause(&self, id: AgentId) -> Result<(), AgentError> {
        let agent = self.agents.get(&id)
            .ok_or(AgentError::NotFound(id))?;
        if agent.state != AgentState::Running {
            return Err(AgentError::InvalidState {
                id,
                expected: "Running".to_string(),
                actual: format!("{:?}", agent.state),
            });
        }
        Ok(())
    }

    fn resume(&self, id: AgentId) -> Result<(), AgentError> {
        let agent = self.agents.get(&id)
            .ok_or(AgentError::NotFound(id))?;
        if agent.state != AgentState::Paused {
            return Err(AgentError::InvalidState {
                id,
                expected: "Paused".to_string(),
                actual: format!("{:?}", agent.state),
            });
        }
        Ok(())
    }

    fn stop(&self, id: AgentId) -> Result<(), AgentError> {
        self.agents.get(&id)
            .ok_or(AgentError::NotFound(id))?;
        // 优雅停止...
        Ok(())
    }

    fn kill(&self, id: AgentId) -> Result<(), AgentError> {
        self.agents.get(&id)
            .ok_or(AgentError::NotFound(id))?;
        // 强制终止...
        Ok(())
    }

    fn configure(&self, id: AgentId, config: AgentConfig) -> Result<(), AgentError> {
        let agent = self.agents.get_mut(&id)
            .ok_or(AgentError::NotFound(id))?;
        agent.config = config;
        Ok(())
    }
}
```

---

## 3. Agent 通信 API

### 3.1 消息传递

```rust
/// Agent 消息
#[derive(Debug, Clone)]
pub struct AgentMessage {
    /// 消息 ID
    pub id: String,
    /// 发送者
    pub from: AgentId,
    /// 接收者（None 表示广播）
    pub to: Option<AgentId>,
    /// 消息类型
    pub msg_type: MessageType,
    /// 消息负载
    pub payload: Vec<u8>,
    /// 时间戳
    pub timestamp: std::time::Instant,
    /// 是否需要回复
    pub reply_requested: bool,
    /// 关联的消息 ID（用于回复）
    pub in_reply_to: Option<String>,
    /// 优先级
    pub priority: u8,
    /// TTL（跳数）
    pub ttl: u8,
}

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// 文本消息
    Text,
    /// 结构化数据
    Structured,
    /// 二进制数据
    Binary,
    /// 控制消息（系统级）
    Control,
    /// 错误消息
    Error,
    /// 心跳
    Heartbeat,
}

/// 主题订阅
#[derive(Debug, Clone)]
pub struct Subscription {
    pub agent_id: AgentId,
    pub topic: String,
    pub filter: Option<MessageFilter>,
}

/// 消息过滤器
#[derive(Debug, Clone)]
pub struct MessageFilter {
    pub msg_types: Vec<MessageType>,
    pub min_priority: u8,
    pub sender_filter: Option<Vec<AgentId>>,
}

/// Agent 通信 trait
pub trait AgentCommunication: Send + Sync {
    /// 发送消息给指定 Agent
    fn send_message(&self, from: AgentId, to: AgentId, msg: AgentMessage) -> Result<(), AgentError>;

    /// 广播消息给所有 Agent
    fn broadcast(&self, from: AgentId, msg: AgentMessage) -> Result<(), AgentError>;

    /// 订阅主题
    fn subscribe(&self, agent_id: AgentId, topic: &str, filter: Option<MessageFilter>) -> Result<(), AgentError>;

    /// 取消订阅
    fn unsubscribe(&self, agent_id: AgentId, topic: &str) -> Result<(), AgentError>;

    /// 接收消息（阻塞）
    fn receive(&self, agent_id: AgentId, timeout: Option<Duration>) -> Result<AgentMessage, AgentError>;

    /// 发送并等待回复
    fn request(&self, from: AgentId, to: AgentId, msg: AgentMessage, timeout: Duration) -> Result<AgentMessage, AgentError>;
}
```

---

## 4. Agent 查询 API

### 4.1 状态查询

```rust
/// Agent 详细信息
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    pub agent_type: AgentType,
    pub state: AgentState,
    pub priority: u8,
    pub memory_usage: u64,
    pub cpu_usage: f32,
    pub message_count: u64,
    pub error_count: u64,
    pub uptime: Duration,
    pub capabilities: Vec<String>,
    pub created_at: std::time::Instant,
}

/// Agent 能力描述
#[derive(Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: Vec<CapabilityParam>,
}

#[derive(Debug, Clone)]
pub struct CapabilityParam {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub description: String,
}

/// Agent 查询 trait
pub trait AgentQuery: Send + Sync {
    /// 获取 Agent 状态
    fn get_status(&self, id: AgentId) -> Result<AgentState, AgentError>;

    /// 获取 Agent 详细信息
    fn get_info(&self, id: AgentId) -> Result<AgentInfo, AgentError>;

    /// 列出所有 Agent
    fn list_agents(&self, filter: Option<AgentFilter>) -> Result<Vec<AgentInfo>, AgentError>;

    /// 查询 Agent 能力
    fn query_capability(&self, id: AgentId, capability: &str) -> Result<Capability, AgentError>;
}

/// Agent 过滤器
#[derive(Debug, Clone, Default)]
pub struct AgentFilter {
    pub state: Option<AgentState>,
    pub agent_type: Option<AgentType>,
    pub name_pattern: Option<String>,
    pub min_priority: Option<u8>,
    pub has_capability: Option<String>,
}
```

---

## 5. Agent 进化 API

### 5.1 进化机制

```rust
/// 适应度评估结果
#[derive(Debug, Clone)]
pub struct FitnessEvaluation {
    pub agent_id: AgentId,
    /// 适应度分数 (0.0 - 1.0)
    pub score: f64,
    /// 评估维度
    pub dimensions: Vec<FitnessDimension>,
    /// 评估时间
    pub evaluated_at: std::time::Instant,
}

/// 适应度维度
#[derive(Debug, Clone)]
pub struct FitnessDimension {
    pub name: String,
    pub score: f64,
    pub weight: f64,
}

/// 变异配置
#[derive(Debug, Clone)]
pub struct MutationConfig {
    /// 变异率 (0.0 - 1.0)
    pub mutation_rate: f64,
    /// 变异策略
    pub strategy: MutationStrategy,
    /// 变异范围
    pub magnitude: f64,
    /// 可变异的参数
    pub target_params: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationStrategy {
    /// 随机变异
    Random,
    /// 高斯变异
    Gaussian,
    /// 自适应变异
    Adaptive,
}

/// 选择策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// 锦标赛选择
    Tournament { size: usize },
    /// 轮盘赌选择
    Roulette,
    /// 精英选择
    Elite { count: usize },
    /// 排名选择
    RankBased,
}

/// Agent 进化 trait
pub trait AgentEvolution: Send + Sync {
    /// 评估适应度
    fn evaluate_fitness(&self, id: AgentId) -> Result<FitnessEvaluation, AgentError>;

    /// 执行变异
    fn mutate(&self, id: AgentId, config: MutationConfig) -> Result<AgentId, AgentError>;

    /// 选择最优 Agent
    fn select_best(&self, candidates: &[AgentId], strategy: SelectionStrategy) -> Result<AgentId, AgentError>;
}
```

---

## 6. Agent 知识 API

### 6.1 知识管理

```rust
/// 知识条目
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    /// 知识 ID
    pub id: String,
    /// 来源 Agent
    pub source_agent: AgentId,
    /// 知识类型
    pub knowledge_type: KnowledgeType,
    /// 知识内容
    pub content: Vec<u8>,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f64,
    /// 标签
    pub tags: Vec<String>,
    /// 创建时间
    pub created_at: std::time::Instant,
    /// 过期时间
    pub expires_at: Option<std::time::Instant>,
    /// 访问计数
    pub access_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeType {
    /// 事实知识
    Fact,
    /// 规则知识
    Rule,
    /// 经验知识
    Experience,
    /// 技能知识
    Skill,
    /// 概念知识
    Concept,
}

/// 知识查询
#[derive(Debug, Clone)]
pub struct KnowledgeQuery {
    pub query: String,
    pub knowledge_types: Option<Vec<KnowledgeType>>,
    pub min_confidence: Option<f64>,
    pub tags: Option<Vec<String>>,
    pub limit: usize,
}

/// Agent 知识管理 trait
pub trait AgentKnowledge: Send + Sync {
    /// 共享知识给其他 Agent
    fn share_knowledge(&self, from: AgentId, to: AgentId, knowledge: KnowledgeEntry) -> Result<String, AgentError>;

    /// 获取知识
    fn acquire_knowledge(&self, agent_id: AgentId, knowledge_id: &str) -> Result<KnowledgeEntry, AgentError>;

    /// 查询知识
    fn query_knowledge(&self, agent_id: AgentId, query: &KnowledgeQuery) -> Result<Vec<KnowledgeEntry>, AgentError>;
}
```

---

## 7. Expert Factory API

### 7.1 专家创建

```rust
/// 专家模板
#[derive(Debug, Clone)]
pub struct ExpertTemplate {
    /// 模板 ID
    pub id: String,
    /// 模板名称
    pub name: String,
    /// 模板描述
    pub description: String,
    /// 专家类型
    pub expert_type: String,
    /// 默认配置
    pub default_config: AgentConfig,
    /// 所需能力
    pub required_capabilities: Vec<String>,
    /// 输入/输出模式定义
    pub io_schema: serde_json::Value,
    /// 版本
    pub version: String,
}

/// Expert Factory trait
pub trait ExpertFactory: Send + Sync {
    /// 从模板创建专家 Agent
    fn create_expert(&self, template_id: &str, overrides: Option<AgentConfig>) -> Result<AgentId, AgentError>;

    /// 列出所有可用模板
    fn list_templates(&self) -> Result<Vec<ExpertTemplate>, AgentError>;

    /// 注册自定义模板
    fn register_template(&self, template: ExpertTemplate) -> Result<(), AgentError>;
}
```

---

## 8. Agent Pool API

### 8.1 任务池

```rust
/// 任务提交
#[derive(Debug, Clone)]
pub struct TaskSubmission {
    /// 任务描述
    pub description: String,
    /// 任务类型
    pub task_type: TaskType,
    /// 输入数据
    pub input: Vec<u8>,
    /// 优先级
    pub priority: u8,
    /// 超时时间
    pub timeout: Duration,
    /// 所需能力
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// 一次性任务
    OneShot,
    /// 周期性任务
    Periodic { interval: Duration },
    /// 持续任务
    Continuous,
}

/// 任务结果
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// 任务 ID
    pub task_id: String,
    /// 执行 Agent
    pub agent_id: AgentId,
    /// 结果数据
    pub output: Vec<u8>,
    /// 执行状态
    pub status: TaskStatus,
    /// 执行耗时
    pub duration: Duration,
    /// 错误信息
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// 等待中
    Pending,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 超时
    Timeout,
    /// 已取消
    Cancelled,
}

/// 池状态
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub total_agents: usize,
    pub idle_agents: usize,
    pub busy_agents: usize,
    pub pending_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
}

/// Agent Pool trait
pub trait AgentPool: Send + Sync {
    /// 提交任务
    fn submit_task(&self, task: TaskSubmission) -> Result<String, AgentError>;

    /// 获取任务结果
    fn get_result(&self, task_id: &str) -> Result<TaskResult, AgentError>;

    /// 查询池状态
    fn pool_status(&self) -> Result<PoolStatus, AgentError>;
}
```

---

## 9. Cloud AI API

### 9.1 云端推理

```rust
/// 云端 AI 提供商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    OpenAI,
    Anthropic,
    Google,
    Custom { endpoint: String },
}

/// 云端提供商配置
#[derive(Debug, Clone)]
pub struct CloudProviderConfig {
    pub provider: CloudProvider,
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout: Duration,
    pub retry_count: u32,
    pub retry_delay: Duration,
}

/// 推理请求
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Vec<String>,
    pub stream: bool,
}

/// 推理响应
#[derive(Debug, Clone)]
pub struct InferenceResponse {
    pub text: String,
    pub model: String,
    pub usage: TokenUsage,
    pub finish_reason: String,
    pub latency: Duration,
}

/// Token 使用统计
#[derive(Debug, Clone, Copy)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Cloud AI trait
pub trait CloudAI: Send + Sync {
    /// 配置云端提供商
    fn configure_cloud_provider(&self, config: CloudProviderConfig) -> Result<(), AgentError>;

    /// 本地推理
    fn inference_local(&self, request: InferenceRequest) -> Result<InferenceResponse, AgentError>;

    /// 云端推理
    fn inference_cloud(&self, request: InferenceRequest) -> Result<InferenceResponse, AgentError>;

    /// 设置模型偏好
    fn set_model_preference(&self, preference: InferencePreference) -> Result<(), AgentError>;
}
```

---

## 10. 错误处理

```rust
/// Agent 错误类型
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent 不存在: {0}")]
    NotFound(AgentId),

    #[error("Agent 状态无效: id={id}, 期望={expected}, 实际={actual}")]
    InvalidState {
        id: AgentId,
        expected: String,
        actual: String,
    },

    #[error("Agent 通信失败: {0}")]
    CommunicationFailed(String),

    #[error("消息超时")]
    MessageTimeout,

    #[error("知识不存在: {0}")]
    KnowledgeNotFound(String),

    #[error("能力不支持: {0}")]
    CapabilityNotSupported(String),

    #[error("进化失败: {0}")]
    EvolutionFailed(String),

    #[error("任务失败: {0}")]
    TaskFailed(String),

    #[error("云端推理失败: {0}")]
    CloudInferenceFailed(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("资源不足: {0}")]
    ResourceExhausted(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),
}
```

---

## 11. 使用示例

### 11.1 完整 Agent 工作流

```rust
use agent_api::*;

async fn agent_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建生命周期管理器
    let lifecycle = AgentLifecycleManager::new();

    // 2. 创建 Agent
    let config = AgentConfig {
        name: "data-analyzer".to_string(),
        agent_type: AgentType::Expert,
        priority: 10,
        memory_limit: 512 * 1024 * 1024,
        capabilities: vec!["data_analysis".to_string(), "report_generation".to_string()],
        model_config: Some(ModelConfig {
            model_name: "omniagent-7b".to_string(),
            preference: InferencePreference::Auto,
            temperature: 0.7,
            max_tokens: 4096,
            top_p: Some(0.9),
            top_k: None,
        }),
        ..Default::default()
    };

    let agent_id = lifecycle.spawn(config)?;
    println!("Agent 已创建: {}", agent_id);

    // 3. 启动 Agent
    lifecycle.start(agent_id)?;
    println!("Agent 已启动");

    // 4. 发送消息
    let comm = AgentCommunicationManager::new();
    let msg = AgentMessage {
        id: uuid::Uuid::new_v4().to_string(),
        from: 0, // 系统 Agent
        to: Some(agent_id),
        msg_type: MessageType::Text,
        payload: b"分析这份销售数据并生成报告".to_vec(),
        timestamp: std::time::Instant::now(),
        reply_requested: true,
        in_reply_to: None,
        priority: 10,
        ttl: 10,
    };
    comm.send_message(0, agent_id, msg.clone())?;

    // 5. 等待回复
    let response = comm.request(0, agent_id, msg, Duration::from_secs(30))?;
    println!("收到回复: {:?}", String::from_utf8_lossy(&response.payload));

    // 6. 评估适应度
    let evolution = AgentEvolutionManager::new();
    let fitness = evolution.evaluate_fitness(agent_id)?;
    println!("适应度分数: {:.2}", fitness.score);

    // 7. 暂停 Agent
    lifecycle.pause(agent_id)?;

    // 8. 停止 Agent
    lifecycle.stop(agent_id)?;
    println!("Agent 已停止");

    Ok(())
}
```

### 11.2 Agent Pool 使用

```rust
async fn pool_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let pool = AgentPoolManager::new();

    // 提交任务
    let task = TaskSubmission {
        description: "翻译以下文本为英文".to_string(),
        task_type: TaskType::OneShot,
        input: b"你好，世界！".to_vec(),
        priority: 5,
        timeout: Duration::from_secs(60),
        required_capabilities: vec!["translation".to_string()],
    };

    let task_id = pool.submit_task(task)?;
    println!("任务已提交: {}", task_id);

    // 查询池状态
    let status = pool.pool_status()?;
    println!("空闲 Agent: {}", status.idle_agents);
    println!("待处理任务: {}", status.pending_tasks);

    // 获取结果
    loop {
        let result = pool.get_result(&task_id)?;
        match result.status {
            TaskStatus::Completed => {
                println!("翻译结果: {:?}", String::from_utf8_lossy(&result.output));
                break;
            }
            TaskStatus::Failed => {
                println!("任务失败: {:?}", result.error);
                break;
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    Ok(())
}
```

### 11.3 Expert Factory 使用

```rust
async fn expert_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let factory = ExpertFactoryManager::new();

    // 列出可用模板
    let templates = factory.list_templates()?;
    for t in &templates {
        println!("模板: {} - {}", t.name, t.description);
    }

    // 创建专家
    let overrides = AgentConfig {
        name: "my-code-reviewer".to_string(),
        priority: 15,
        model_config: Some(ModelConfig {
            model_name: "omniagent-13b".to_string(),
            preference: InferencePreference::Local,
            temperature: 0.3,
            max_tokens: 8192,
            top_p: None,
            top_k: None,
        }),
        ..Default::default()
    };
    let expert_id = factory.create_expert("code-review", Some(overrides))?;
    println!("专家 Agent 已创建: {}", expert_id);

    Ok(())
}
```

---

## 12. 性能约束

| 操作 | 延迟目标 | 吞吐量目标 | 说明 |
|------|---------|-----------|------|
| Agent spawn | <100ms | 100/s | 不含模型加载 |
| Agent start | <50ms | 200/s | 冷启动 |
| Agent pause | <10ms | 1000/s | 状态保存 |
| Agent resume | <20ms | 500/s | 状态恢复 |
| Agent stop | <50ms | 200/s | 优雅关闭 |
| send_message | <1ms | 100,000/s | 同进程 |
| broadcast | <5ms | 10,000/s | 100 个 Agent |
| get_status | <1ms | 10,000/s | 内存查询 |
| evaluate_fitness | <100ms | 10/s | 含推理 |
| mutate | <200ms | 5/s | 含模型更新 |
| submit_task | <5ms | 1000/s | 任务入队 |
| inference_local | <500ms | 2/s | 7B 模型 |
| inference_cloud | <2s | 取决于网络 | 云端 API |

---

## 13. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.name, "");
        assert_eq!(config.priority, 0);
        assert!(config.env.is_empty());
    }

    #[test]
    fn test_agent_state_transitions() {
        let lifecycle = AgentLifecycleManager::new();
        let config = AgentConfig {
            name: "test".to_string(),
            ..Default::default()
        };
        let id = lifecycle.spawn(config).unwrap();
        assert!(lifecycle.start(id).is_ok());
        assert!(lifecycle.pause(id).is_ok());
        assert!(lifecycle.resume(id).is_ok());
        assert!(lifecycle.stop(id).is_ok());
    }

    #[test]
    fn test_agent_not_found() {
        let lifecycle = AgentLifecycleManager::new();
        let result = lifecycle.start(99999);
        assert!(matches!(result, Err(AgentError::NotFound(_))));
    }

    #[test]
    fn test_invalid_state_transition() {
        let lifecycle = AgentLifecycleManager::new();
        let config = AgentConfig {
            name: "test".to_string(),
            ..Default::default()
        };
        let id = lifecycle.spawn(config).unwrap();
        // 未启动就暂停应该失败
        let result = lifecycle.pause(id);
        assert!(matches!(result, Err(AgentError::InvalidState { .. })));
    }

    #[test]
    fn test_message_creation() {
        let msg = AgentMessage {
            id: "msg-1".to_string(),
            from: 1,
            to: Some(2),
            msg_type: MessageType::Text,
            payload: b"hello".to_vec(),
            timestamp: std::time::Instant::now(),
            reply_requested: true,
            in_reply_to: None,
            priority: 5,
            ttl: 10,
        };
        assert_eq!(msg.from, 1);
        assert!(msg.reply_requested);
    }

    #[test]
    fn test_knowledge_entry() {
        let entry = KnowledgeEntry {
            id: "k-1".to_string(),
            source_agent: 1,
            knowledge_type: KnowledgeType::Fact,
            content: b"Earth orbits the Sun".to_vec(),
            confidence: 0.99,
            tags: vec!["astronomy".to_string()],
            created_at: std::time::Instant::now(),
            expires_at: None,
            access_count: 0,
        };
        assert!((entry.confidence - 0.99).abs() < 0.001);
    }

    #[test]
    fn test_fitness_evaluation() {
        let eval = FitnessEvaluation {
            agent_id: 1,
            score: 0.85,
            dimensions: vec![
                FitnessDimension {
                    name: "accuracy".to_string(),
                    score: 0.9,
                    weight: 0.6,
                },
                FitnessDimension {
                    name: "speed".to_string(),
                    score: 0.75,
                    weight: 0.4,
                },
            ],
            evaluated_at: std::time::Instant::now(),
        };
        assert!((eval.score - 0.85).abs() < 0.01);
        assert_eq!(eval.dimensions.len(), 2);
    }

    #[test]
    fn test_cloud_provider_config() {
        let config = CloudProviderConfig {
            provider: CloudProvider::OpenAI,
            api_key: "sk-test".to_string(),
            model: "gpt-4".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            timeout: Duration::from_secs(30),
            retry_count: 3,
            retry_delay: Duration::from_secs(1),
        };
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.retry_count, 3);
    }

    #[test]
    fn test_task_submission() {
        let task = TaskSubmission {
            description: "test task".to_string(),
            task_type: TaskType::OneShot,
            input: b"data".to_vec(),
            priority: 5,
            timeout: Duration::from_secs(60),
            required_capabilities: vec!["test".to_string()],
        };
        assert_eq!(task.priority, 5);
    }

    #[test]
    fn test_agent_filter() {
        let filter = AgentFilter {
            state: Some(AgentState::Running),
            agent_type: Some(AgentType::Expert),
            name_pattern: Some("code-*".to_string()),
            min_priority: Some(5),
            has_capability: Some("analysis".to_string()),
        };
        assert!(filter.state.is_some());
        assert_eq!(filter.min_priority, Some(5));
    }
}
```

---

*本文档为 OmniAgent OS Agent API 参考，版本 0.1.0。*
