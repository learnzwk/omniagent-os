# 自动化引擎 API 参考

> **模块名称**: `automation-api`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

自动化引擎 API 提供 OmniAgent OS 中任务自动化、工作流编排、并行调度、事件触发和操作市场等功能。通过此 API，用户和 Agent 可以将复杂任务分解为原子操作，组合为可复用的工作流，并通过事件驱动和定时调度实现自动化执行。

### 1.2 架构概览

```
┌──────────────────────────────────────────────────────────┐
│                  Automation Engine                        │
├──────────┬──────────┬──────────┬─────────────────────────┤
│  Task    │Workflow  │Parallel  │ Event Trigger           │
│ Automate │ Engine   │Scheduler │                         │
├──────────┼──────────┼──────────┼─────────────────────────┤
│  Cron    │  Chain   │Checkpoint│ Operation Marketplace  │
│Scheduler │Orchestr.│ System   │                         │
├──────────┼──────────┼──────────┼─────────────────────────┤
│ Template │  Operation Registry   │  Execution Runtime    │
└──────────┴──────────┴──────────┴─────────────────────────┘
```

---

## 2. 任务自动化 API

### 2.1 指令解析与任务分解

```rust
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// 任务标识符
pub type TaskId = String;

/// 操作标识符
pub type OperationId = String;

/// 指令解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedInstruction {
    /// 原始指令文本
    pub raw_text: String,
    /// 解析出的意图
    pub intent: String,
    /// 提取的参数
    pub parameters: HashMap<String, String>,
    /// 置信度
    pub confidence: f64,
    /// 建议的操作链
    pub suggested_operations: Vec<OperationRef>,
}

/// 操作引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRef {
    pub operation_id: OperationId,
    pub name: String,
    pub description: String,
    pub parameters: HashMap<String, String>,
    pub estimated_duration: Duration,
}

/// 分解后的子任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    pub operation_id: OperationId,
    pub parameters: HashMap<String, String>,
    pub dependencies: Vec<TaskId>,
    pub estimated_duration: Duration,
    pub retry_policy: RetryPolicy,
    pub timeout: Duration,
}

/// 重试策略
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }
}

/// 任务分解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecomposition {
    pub root_task_id: TaskId,
    pub sub_tasks: Vec<SubTask>,
    pub total_estimated_duration: Duration,
    pub parallelizable_groups: Vec<Vec<TaskId>>,
}

/// 任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub status: ExecutionStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration: Duration,
    pub started_at: std::time::Instant,
    pub completed_at: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Skipped,
    Timeout,
}

/// 任务自动化 trait
pub trait TaskAutomation: Send + Sync {
    /// 解析自然语言指令
    fn parse_instruction(&self, instruction: &str) -> Result<ParsedInstruction, AutomationError>;

    /// 将复杂任务分解为子任务
    fn decompose_task(&self, instruction: &ParsedInstruction) -> Result<TaskDecomposition, AutomationError>;

    /// 顺序执行子任务
    fn execute_sequential(&self, decomposition: &TaskDecomposition) -> Result<Vec<TaskResult>, AutomationError>;
}
```

---

## 3. 工作流 API

### 3.1 工作流管理

```rust
/// 工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// 工作流 ID
    pub id: String,
    /// 工作流名称
    pub name: String,
    /// 工作流描述
    pub description: String,
    /// 工作流步骤
    pub steps: Vec<WorkflowStep>,
    /// 工作流变量
    pub variables: HashMap<String, serde_json::Value>,
    /// 错误处理策略
    pub error_strategy: ErrorStrategy,
    /// 超时时间
    pub timeout: Duration,
    /// 标签
    pub tags: Vec<String>,
    /// 版本
    pub version: u32,
}

/// 工作流步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// 步骤 ID
    pub id: String,
    /// 步骤名称
    pub name: String,
    /// 操作 ID
    pub operation_id: OperationId,
    /// 步骤参数
    pub parameters: HashMap<String, String>,
    /// 依赖的步骤 ID
    pub depends_on: Vec<String>,
    /// 条件表达式（可选）
    pub condition: Option<String>,
    /// 重试策略
    pub retry_policy: RetryPolicy,
    /// 超时时间
    pub timeout: Duration,
}

/// 错误处理策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorStrategy {
    /// 遇到错误立即停止
    FailFast,
    /// 跳过失败步骤继续
    SkipAndContinue,
    /// 重试失败步骤
    Retry,
    /// 回滚已完成的步骤
    Rollback,
}

/// 工作流执行实例
#[derive(Debug, Clone)]
pub struct WorkflowExecution {
    pub execution_id: String,
    pub workflow_id: String,
    pub status: ExecutionStatus,
    pub current_step: Option<String>,
    pub step_results: HashMap<String, TaskResult>,
    pub started_at: std::time::Instant,
    pub variables: HashMap<String, serde_json::Value>,
}

/// 工作流管理 trait
pub trait WorkflowEngine: Send + Sync {
    /// 创建工作流
    fn create_workflow(&self, workflow: Workflow) -> Result<String, AutomationError>;

    /// 启动工作流执行
    fn start_workflow(&self, workflow_id: &str, initial_vars: HashMap<String, serde_json::Value>) -> Result<String, AutomationError>;

    /// 暂停工作流
    fn pause_workflow(&self, execution_id: &str) -> Result<(), AutomationError>;

    /// 恢复工作流
    fn resume_workflow(&self, execution_id: &str) -> Result<(), AutomationError>;

    /// 取消工作流
    fn cancel_workflow(&self, execution_id: &str) -> Result<(), AutomationError>;

    /// 查询工作流执行状态
    fn get_workflow_status(&self, execution_id: &str) -> Result<WorkflowExecution, AutomationError>;
}
```

---

## 4. 并行调度 API

### 4.1 并行任务执行

```rust
/// 并行任务组
#[derive(Debug, Clone)]
pub struct ParallelTaskGroup {
    pub group_id: String,
    pub tasks: Vec<SubTask>,
    pub max_parallelism: usize,
    pub resource_limits: ResourceLimits,
}

/// 资源限制
#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    pub max_cpu_percent: u32,
    pub max_memory_mb: u64,
    pub max_gpu_percent: u32,
    pub max_network_mbps: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 80,
            max_memory_mb: 4096,
            max_gpu_percent: 50,
            max_network_mbps: 100,
        }
    }
}

/// 并行执行状态
#[derive(Debug, Clone)]
pub struct ParallelStatus {
    pub group_id: String,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub running_tasks: usize,
    pub pending_tasks: usize,
    pub progress: f64,
    pub estimated_remaining: Duration,
}

/// 并行调度器 trait
pub trait ParallelScheduler: Send + Sync {
    /// 调度并行任务组
    fn schedule_parallel(&self, group: ParallelTaskGroup) -> Result<String, AutomationError>;

    /// 查询并行执行状态
    fn get_parallel_status(&self, group_id: &str) -> Result<ParallelStatus, AutomationError>;

    /// 取消并行任务组
    fn cancel_parallel(&self, group_id: &str) -> Result<(), AutomationError>;
}
```

---

## 5. 事件触发 API

### 5.1 事件驱动

```rust
/// 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    pub source: String,
    pub payload: serde_json::Value,
    pub timestamp: std::time::Instant,
    pub metadata: HashMap<String, String>,
}

/// 触发器定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: String,
    pub name: String,
    /// 触发条件
    pub condition: TriggerCondition,
    /// 触发后执行的动作
    pub action: TriggerAction,
    /// 是否启用
    pub enabled: bool,
    /// 触发次数限制
    pub max_triggers: Option<u32>,
    /// 已触发次数
    pub trigger_count: u32,
    /// 冷却时间
    pub cooldown: Duration,
    pub last_triggered: Option<std::time::Instant>,
}

/// 触发条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerCondition {
    /// 事件匹配
    EventMatch {
        event_type: String,
        source_pattern: Option<String>,
        payload_filter: Option<serde_json::Value>,
    },
    /// 时间条件
    TimeCondition {
        start_time: String,
        end_time: String,
        days_of_week: Vec<u8>,
    },
    /// 系统条件
    SystemCondition {
        metric: String,
        operator: ComparisonOperator,
        threshold: f64,
    },
    /// 组合条件
    And(Vec<TriggerCondition>),
    Or(Vec<TriggerCondition>),
    Not(Box<TriggerCondition>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    GreaterThanOrEqual,
    LessThanOrEqual,
    NotEqual,
}

/// 触发动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerAction {
    /// 启动工作流
    StartWorkflow { workflow_id: String, variables: HashMap<String, serde_json::Value> },
    /// 发送通知
    SendNotification { title: String, body: String, urgency: u8 },
    /// 执行操作
    ExecuteOperation { operation_id: String, parameters: HashMap<String, String> },
    /// 调用 Agent
    CallAgent { agent_id: u64, message: String },
    /// 自定义回调
    Callback { url: String, payload: serde_json::Value },
}

/// 事件触发管理 trait
pub trait EventTriggerEngine: Send + Sync {
    /// 注册触发器
    fn register_trigger(&self, trigger: Trigger) -> Result<String, AutomationError>;

    /// 手动触发事件
    fn fire_event(&self, event: Event) -> Result<Vec<String>, AutomationError>;

    /// 列出所有触发器
    fn list_triggers(&self, filter: Option<TriggerFilter>) -> Result<Vec<Trigger>, AutomationError>;

    /// 删除触发器
    fn delete_trigger(&self, trigger_id: &str) -> Result<(), AutomationError>;
}

#[derive(Debug, Clone, Default)]
pub struct TriggerFilter {
    pub event_type: Option<String>,
    pub enabled_only: bool,
    pub name_pattern: Option<String>,
}
```

---

## 6. Cron 调度 API

### 6.1 定时任务

```rust
/// Cron 表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronExpression {
    /// 分钟 (0-59)
    pub minute: String,
    /// 小时 (0-23)
    pub hour: String,
    /// 日 (1-31)
    pub day_of_month: String,
    /// 月 (1-12)
    pub month: String,
    /// 星期 (0-6, 0=周日)
    pub day_of_week: String,
}

impl CronExpression {
    /// 解析 cron 表达式字符串
    /// 格式: "minute hour day_of_month month day_of_week"
    /// 示例: "0 */2 * * *" 表示每两小时执行一次
    pub fn parse(expr: &str) -> Result<Self, AutomationError> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(AutomationError::InvalidCronExpression(expr.to_string()));
        }
        Ok(Self {
            minute: parts[0].to_string(),
            hour: parts[1].to_string(),
            day_of_month: parts[2].to_string(),
            month: parts[3].to_string(),
            day_of_week: parts[4].to_string(),
        })
    }

    /// 标准表达式: 每分钟
    pub fn every_minute() -> Self {
        Self::parse("* * * * *").unwrap()
    }

    /// 每小时
    pub fn every_hour() -> Self {
        Self::parse("0 * * * *").unwrap()
    }

    /// 每天午夜
    pub fn daily_midnight() -> Self {
        Self::parse("0 0 * * *").unwrap()
    }
}

/// Cron 任务
#[derive(Debug, Clone)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub cron: CronExpression,
    pub action: TriggerAction,
    pub enabled: bool,
    pub last_run: Option<std::time::Instant>,
    pub next_run: Option<std::time::Instant>,
    pub run_count: u32,
    pub timezone: String,
}

/// Cron 调度器 trait
pub trait CronScheduler: Send + Sync {
    /// 创建定时任务
    fn schedule_cron(&self, name: &str, cron: CronExpression, action: TriggerAction) -> Result<String, AutomationError>;

    /// 取消定时任务
    fn cancel_cron(&self, job_id: &str) -> Result<(), AutomationError>;

    /// 列出所有定时任务
    fn list_cron_jobs(&self) -> Result<Vec<CronJob>, AutomationError>;

    /// 暂停定时任务
    fn pause_cron(&self, job_id: &str) -> Result<(), AutomationError>;

    /// 恢复定时任务
    fn resume_cron(&self, job_id: &str) -> Result<(), AutomationError>;
}
```

---

## 7. 链式编排 API

### 7.1 操作链

```rust
/// 操作链定义
#[derive(Debug, Clone)]
pub struct OperationChain {
    pub id: String,
    pub name: String,
    pub operations: Vec<ChainStep>,
    /// 链的错误处理策略
    pub error_strategy: ErrorStrategy,
    /// 输入数据
    pub input: serde_json::Value,
}

/// 链步骤
#[derive(Debug, Clone)]
pub struct ChainStep {
    pub step_id: String,
    pub operation_id: OperationId,
    pub parameters: HashMap<String, String>,
    /// 输入映射：从上一步输出中提取字段
    pub input_mapping: HashMap<String, String>,
    /// 输出映射：将结果字段映射到链上下文
    pub output_mapping: HashMap<String, String>,
    /// 条件执行
    pub condition: Option<String>,
    /// 失败时的回退操作
    pub fallback_operation: Option<OperationId>,
}

/// 链执行结果
#[derive(Debug, Clone)]
pub struct ChainResult {
    pub chain_id: String,
    pub status: ExecutionStatus,
    pub step_results: Vec<TaskResult>,
    pub final_output: Option<serde_json::Value>,
    pub total_duration: Duration,
    pub failed_step: Option<String>,
}

/// 链式编排器 trait
pub trait ChainOrchestrator: Send + Sync {
    /// 编排操作链
    fn chain_operations(&self, chain: OperationChain) -> Result<String, AutomationError>;

    /// 执行操作链
    fn execute_chain(&self, chain_id: &str) -> Result<ChainResult, AutomationError>;

    /// 查询链执行状态
    fn get_chain_status(&self, chain_id: &str) -> Result<ChainResult, AutomationError>;

    /// 取消链执行
    fn cancel_chain(&self, chain_id: &str) -> Result<(), AutomationError>;
}
```

---

## 8. 检查点 API

### 8.1 状态持久化

```rust
/// 检查点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    /// 关联的工作流/链 ID
    pub execution_id: String,
    /// 检查点名称
    pub name: String,
    /// 创建时间
    pub created_at: std::time::Instant,
    /// 执行上下文快照
    pub context: serde_json::Value,
    /// 已完成的步骤
    pub completed_steps: Vec<String>,
    /// 当前步骤
    pub current_step: Option<String>,
    /// 变量状态
    pub variables: HashMap<String, serde_json::Value>,
    /// 中间结果
    pub intermediate_results: HashMap<String, serde_json::Value>,
    /// 检查点大小（字节）
    pub size_bytes: u64,
}

/// 检查点管理 trait
pub trait CheckpointManager: Send + Sync {
    /// 保存检查点
    fn save_checkpoint(&self, execution_id: &str, name: &str) -> Result<String, AutomationError>;

    /// 从检查点恢复
    fn resume_from_checkpoint(&self, checkpoint_id: &str) -> Result<String, AutomationError>;

    /// 列出执行的所有检查点
    fn list_checkpoints(&self, execution_id: &str) -> Result<Vec<Checkpoint>, AutomationError>;

    /// 删除检查点
    fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<(), AutomationError>;
}
```

---

## 9. 操作市场 API

### 9.1 操作发布与安装

```rust
/// 操作定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: OperationId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    /// 操作类别
    pub category: OperationCategory,
    /// 输入参数定义
    pub input_schema: serde_json::Value,
    /// 输出定义
    pub output_schema: serde_json::Value,
    /// 执行函数引用
    pub executor: String,
    /// 依赖
    pub dependencies: Vec<String>,
    /// 标签
    pub tags: Vec<String>,
    /// 评分
    pub rating: f32,
    /// 下载次数
    pub downloads: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationCategory {
    FileSystem,
    Network,
    DataProcessing,
    AiMl,
    Communication,
    System,
    Security,
    Media,
    Agent,
    Custom,
}

/// 操作搜索结果
#[derive(Debug, Clone)]
pub struct OperationSearchResult {
    pub operations: Vec<Operation>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
}

/// 操作市场 trait
pub trait OperationMarketplace: Send + Sync {
    /// 发布操作
    fn publish_operation(&self, operation: Operation) -> Result<(), AutomationError>;

    /// 安装操作
    fn install_operation(&self, operation_id: &str, version: Option<&str>) -> Result<(), AutomationError>;

    /// 搜索操作
    fn search_operations(&self, query: &str, category: Option<OperationCategory>, page: usize, page_size: usize) -> Result<OperationSearchResult, AutomationError>;

    /// 卸载操作
    fn uninstall_operation(&self, operation_id: &str) -> Result<(), AutomationError>;

    /// 获取操作详情
    fn get_operation(&self, operation_id: &str) -> Result<Operation, AutomationError>;
}
```

---

## 10. 模板库 API

### 10.1 工作流模板

```rust
/// 工作流模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 模板参数定义
    pub parameter_schema: serde_json::Value,
    /// 工作流定义（含参数占位符）
    pub workflow_template: Workflow,
    /// 作者
    pub author: String,
    /// 版本
    pub version: String,
    /// 标签
    pub tags: Vec<String>,
    /// 创建时间
    pub created_at: std::time::Instant,
}

/// 模板库 trait
pub trait TemplateLibrary: Send + Sync {
    /// 加载模板
    fn load_template(&self, template_id: &str) -> Result<WorkflowTemplate, AutomationError>;

    /// 保存模板
    fn save_template(&self, template: WorkflowTemplate) -> Result<String, AutomationError>;

    /// 列出所有模板
    fn list_templates(&self, category: Option<String>) -> Result<Vec<WorkflowTemplate>, AutomationError>;

    /// 从模板实例化工作流
    fn instantiate(&self, template_id: &str, parameters: HashMap<String, serde_json::Value>) -> Result<Workflow, AutomationError>;

    /// 删除模板
    fn delete_template(&self, template_id: &str) -> Result<(), AutomationError>;
}
```

---

## 11. 错误处理

```rust
/// 自动化引擎错误类型
#[derive(Debug, thiserror::Error)]
pub enum AutomationError {
    #[error("指令解析失败: {0}")]
    ParseError(String),

    #[error("任务分解失败: {0}")]
    DecompositionFailed(String),

    #[error("任务执行失败: task_id={task_id}, reason={reason}")]
    TaskExecutionFailed { task_id: TaskId, reason: String },

    #[error("工作流不存在: {0}")]
    WorkflowNotFound(String),

    #[error("工作流状态无效: {0}")]
    InvalidWorkflowState(String),

    #[error("操作不存在: {0}")]
    OperationNotFound(OperationId),

    #[error("操作执行失败: {0}")]
    OperationFailed(String),

    #[error("触发器不存在: {0}")]
    TriggerNotFound(String),

    #[error("无效的 Cron 表达式: {0}")]
    InvalidCronExpression(String),

    #[error("检查点不存在: {0}")]
    CheckpointNotFound(String),

    #[error("检查点恢复失败: {0}")]
    CheckpointRestoreFailed(String),

    #[error("超时: {0}")]
    Timeout(String),

    #[error("资源不足: {0}")]
    ResourceExhausted(String),

    #[error("模板不存在: {0}")]
    TemplateNotFound(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),
}
```

---

## 12. 使用示例

### 12.1 任务自动化

```rust
use automation_api::*;

async fn task_automation_example() -> Result<(), Box<dyn std::error::Error>> {
    let engine = TaskAutomationEngine::new();

    // 解析自然语言指令
    let parsed = engine.parse_instruction(
        "将 /data/sales.csv 中的数据进行分析，生成月度报告，并发送给团队"
    )?;

    println!("意图: {}", parsed.intent);
    println!("置信度: {:.2}", parsed.confidence);

    // 分解任务
    let decomposition = engine.decompose_task(&parsed)?;
    println!("子任务数量: {}", decomposition.sub_tasks.len());

    // 顺序执行
    let results = engine.execute_sequential(&decomposition)?;
    for result in &results {
        println!("任务 {} -> {:?}", result.task_id, result.status);
    }

    Ok(())
}
```

### 12.2 工作流编排

```rust
async fn workflow_example() -> Result<(), Box<dyn std::error::Error>> {
    let wf_engine = WorkflowEngineImpl::new();

    // 创建工作流
    let workflow = Workflow {
        id: "data-pipeline".to_string(),
        name: "数据处理管道".to_string(),
        description: "自动数据处理和分析工作流".to_string(),
        steps: vec![
            WorkflowStep {
                id: "step-1".to_string(),
                name: "数据采集".to_string(),
                operation_id: "data-collect".to_string(),
                parameters: HashMap::from([
                    ("source".to_string(), "database".to_string()),
                    ("query".to_string(), "SELECT * FROM sales".to_string()),
                ]),
                depends_on: vec![],
                condition: None,
                retry_policy: RetryPolicy::default(),
                timeout: Duration::from_secs(300),
            },
            WorkflowStep {
                id: "step-2".to_string(),
                name: "数据清洗".to_string(),
                operation_id: "data-clean".to_string(),
                parameters: HashMap::new(),
                depends_on: vec!["step-1".to_string()],
                condition: None,
                retry_policy: RetryPolicy::default(),
                timeout: Duration::from_secs(120),
            },
            WorkflowStep {
                id: "step-3".to_string(),
                name: "生成报告".to_string(),
                operation_id: "report-generate".to_string(),
                parameters: HashMap::from([
                    ("format".to_string(), "pdf".to_string()),
                ]),
                depends_on: vec!["step-2".to_string()],
                condition: None,
                retry_policy: RetryPolicy::default(),
                timeout: Duration::from_secs(60),
            },
        ],
        variables: HashMap::new(),
        error_strategy: ErrorStrategy::FailFast,
        timeout: Duration::from_secs(600),
        tags: vec!["data".to_string(), "pipeline".to_string()],
        version: 1,
    };

    let workflow_id = wf_engine.create_workflow(workflow)?;
    let execution_id = wf_engine.start_workflow(&workflow_id, HashMap::new())?;

    // 查询状态
    let status = wf_engine.get_workflow_status(&execution_id)?;
    println!("工作流状态: {:?}", status.status);

    Ok(())
}
```

### 12.3 事件触发与 Cron 调度

```rust
async fn trigger_and_cron_example() -> Result<(), Box<dyn std::error::Error>> {
    let trigger_engine = EventTriggerEngineImpl::new();
    let cron_scheduler = CronSchedulerImpl::new();

    // 注册事件触发器
    let trigger = Trigger {
        id: "file-upload-trigger".to_string(),
        name: "文件上传处理".to_string(),
        condition: TriggerCondition::EventMatch {
            event_type: "file.uploaded".to_string(),
            source_pattern: Some("/data/uploads/*".to_string()),
            payload_filter: None,
        },
        action: TriggerAction::StartWorkflow {
            workflow_id: "data-pipeline".to_string(),
            variables: HashMap::new(),
        },
        enabled: true,
        max_triggers: None,
        trigger_count: 0,
        cooldown: Duration::from_secs(10),
        last_triggered: None,
    };
    trigger_engine.register_trigger(trigger)?;

    // 创建定时任务
    let cron = CronExpression::parse("0 9 * * 1-5")?; // 工作日每天9点
    let job_id = cron_scheduler.schedule_cron(
        "每日报告生成",
        cron,
        TriggerAction::StartWorkflow {
            workflow_id: "daily-report".to_string(),
            variables: HashMap::new(),
        },
    )?;

    println!("定时任务已创建: {}", job_id);

    Ok(())
}
```

### 12.4 操作链

```rust
async fn chain_example() -> Result<(), Box<dyn std::error::Error>> {
    let orchestrator = ChainOrchestratorImpl::new();

    let chain = OperationChain {
        id: "text-process-chain".to_string(),
        name: "文本处理链".to_string(),
        operations: vec![
            ChainStep {
                step_id: "translate".to_string(),
                operation_id: "translate-text".to_string(),
                parameters: HashMap::from([
                    ("target_lang".to_string(), "en".to_string()),
                ]),
                input_mapping: HashMap::from([
                    ("text".to_string(), "$.input.text".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("translated".to_string(), "$.result.text".to_string()),
                ]),
                condition: None,
                fallback_operation: None,
            },
            ChainStep {
                step_id: "summarize".to_string(),
                operation_id: "summarize-text".to_string(),
                parameters: HashMap::from([
                    ("max_length".to_string(), "200".to_string()),
                ]),
                input_mapping: HashMap::from([
                    ("text".to_string(), "$.translated".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("summary".to_string(), "$.result.text".to_string()),
                ]),
                condition: None,
                fallback_operation: None,
            },
        ],
        error_strategy: ErrorStrategy::FailFast,
        input: serde_json::json!({"text": "你好世界"}),
    };

    let chain_id = orchestrator.chain_operations(chain)?;
    let result = orchestrator.execute_chain(&chain_id)?;
    println!("链执行结果: {:?}", result.status);
    println!("最终输出: {:?}", result.final_output);

    Ok(())
}
```

---

## 13. 性能约束

| 操作 | 延迟目标 | 吞吐量目标 | 说明 |
|------|---------|-----------|------|
| parse_instruction | <200ms | 50/s | 含 NLP 推理 |
| decompose_task | <500ms | 20/s | 含依赖分析 |
| execute_sequential | 取决于任务 | - | 串行执行 |
| schedule_parallel | <10ms | 1000/s | 任务入队 |
| create_workflow | <5ms | 500/s | 持久化 |
| start_workflow | <50ms | 100/s | 含验证 |
| register_trigger | <5ms | 1000/s | 内存注册 |
| fire_event | <10ms | 500/s | 含条件匹配 |
| schedule_cron | <5ms | 500/s | 表达式解析 |
| save_checkpoint | <50ms | 50/s | 状态序列化 |
| resume_from_checkpoint | <100ms | 20/s | 状态反序列化 |
| publish_operation | <10ms | 100/s | 注册操作 |
| search_operations | <50ms | 100/s | 索引查询 |
| load_template | <10ms | 200/s | 模板加载 |

---

## 14. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_cron_expression_parse() {
        let cron = CronExpression::parse("0 */2 * * *").unwrap();
        assert_eq!(cron.minute, "0");
        assert_eq!(cron.hour, "*/2");
    }

    #[test]
    fn test_cron_expression_invalid() {
        let result = CronExpression::parse("invalid");
        assert!(matches!(result, Err(AutomationError::InvalidCronExpression(_))));
    }

    #[test]
    fn test_cron_presets() {
        let every_min = CronExpression::every_minute();
        assert_eq!(every_min.minute, "*");

        let every_hour = CronExpression::every_hour();
        assert_eq!(every_hour.minute, "0");
        assert_eq!(every_hour.hour, "*");

        let daily = CronExpression::daily_midnight();
        assert_eq!(daily.minute, "0");
        assert_eq!(daily.hour, "0");
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_cpu_percent, 80);
        assert_eq!(limits.max_memory_mb, 4096);
    }

    #[test]
    fn test_execution_status_values() {
        let statuses = [
            ExecutionStatus::Pending,
            ExecutionStatus::Running,
            ExecutionStatus::Completed,
            ExecutionStatus::Failed,
            ExecutionStatus::Cancelled,
            ExecutionStatus::Skipped,
            ExecutionStatus::Timeout,
        ];
        assert_eq!(statuses.len(), 7);
    }

    #[test]
    fn test_error_strategy() {
        assert_eq!(ErrorStrategy::FailFast, ErrorStrategy::FailFast);
        assert_ne!(ErrorStrategy::FailFast, ErrorStrategy::SkipAndContinue);
    }

    #[test]
    fn test_trigger_condition_serialization() {
        let condition = TriggerCondition::EventMatch {
            event_type: "file.uploaded".to_string(),
            source_pattern: None,
            payload_filter: None,
        };
        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains("file.uploaded"));
    }

    #[test]
    fn test_workflow_step_dependencies() {
        let step = WorkflowStep {
            id: "step-2".to_string(),
            depends_on: vec!["step-1".to_string()],
            ..Default::default()
        };
        assert_eq!(step.depends_on.len(), 1);
    }

    #[test]
    fn test_parallel_status_progress() {
        let status = ParallelStatus {
            total_tasks: 10,
            completed_tasks: 7,
            failed_tasks: 1,
            running_tasks: 2,
            pending_tasks: 0,
            progress: 0.7,
            estimated_remaining: Duration::from_secs(30),
        };
        assert!((status.progress - 0.7).abs() < 0.01);
        assert_eq!(status.completed_tasks + status.failed_tasks + status.running_tasks, 10);
    }

    #[test]
    fn test_operation_category() {
        let categories = [
            OperationCategory::FileSystem,
            OperationCategory::Network,
            OperationCategory::AiMl,
            OperationCategory::Agent,
        ];
        assert_eq!(categories.len(), 4);
    }

    #[test]
    fn test_chain_step_mapping() {
        let step = ChainStep {
            step_id: "translate".to_string(),
            operation_id: "translate-text".to_string(),
            input_mapping: HashMap::from([("text".to_string(), "$.input".to_string())]),
            output_mapping: HashMap::from([("result".to_string(), "$.output".to_string())]),
            ..Default::default()
        };
        assert_eq!(step.input_mapping.len(), 1);
        assert_eq!(step.output_mapping.len(), 1);
    }
}
```

---

*本文档为 OmniAgent OS 自动化引擎 API 参考，版本 0.1.0。*
