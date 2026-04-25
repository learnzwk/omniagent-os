# OmniAgent OS — 自动化引擎模块规格说明

> **模块编号**: OA-AUT-001 | **版本**: v1.0.0-draft | **状态**: 设计中
> **依赖**: 内核调度器 (OA-KRN-002)、权限管理 (OA-SEC-003)、持久化存储 (OA-STO-004)

## 1. 概述

自动化引擎是 OmniAgent OS 中最大的服务模块，负责将自然语言意图转化为可执行操作序列。由三个子引擎协同工作：任务自动化引擎（指令解析/DAG分解/条件路由/循环控制/错误恢复）、工作流自动化引擎（声明式定义/持久化调度/并行执行/事件触发）、复杂顺序操作处理器（链编排/上下文传递/沙箱隔离/操作市场）。

### 1.1 设计原则

| 原则 | 说明 |
|------|------|
| 幂等性 | 所有操作支持安全重试，重复执行不产生副作用 |
| 可观测性 | 每个操作步骤产出结构化日志和指标 |
| 可恢复性 | 任意步骤失败后可从最近检查点恢复 |
| 资源隔离 | 操作在沙箱中执行，限制 CPU/内存/网络/文件系统访问 |

### 1.2 性能约束

| 指标 | 目标值 | 测量条件 |
|------|--------|----------|
| 任务分解延迟 | < 5s | 100 个子任务以内 |
| 工作流启动延迟 | < 100ms | 冷启动，含状态加载 |
| 链式执行每步延迟 | < 10ms | 纯计算操作，不含 I/O |
| 错误恢复延迟 | < 500ms | 从检测到恢复策略启动 |
| 状态持久化延迟 | < 50ms | 单次检查点写入 |

---

## 2. Part A：任务自动化引擎

### 2.1 指令解析器 (Instruction Parser)

```rust
pub trait InstructionParser: Send + Sync {
    fn parse(&self, input: &str) -> Result<InstructionGraph, ParseError>;
    fn parse_incremental(&self, chunk: &str, ctx: &mut ParseContext) -> Result<ParseProgress, ParseError>;
    fn validate(&self, graph: &InstructionGraph) -> Result<ValidationReport, ParseError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionGraph {
    pub id: GraphId, pub nodes: Vec<InstructionNode>, pub edges: Vec<DependencyEdge>,
    pub variables: HashMap<String, Variable>, pub metadata: GraphMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionNode {
    pub id: NodeId, pub kind: NodeKind, pub action: ActionDescriptor,
    pub inputs: Vec<InputPort>, pub outputs: Vec<OutputPort>,
    pub retry_policy: RetryPolicy, pub timeout: Duration, pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeKind {
    Atomic(AtomicOp), Conditional(ConditionExpr), Loop(LoopSpec),
    Parallel(ParallelSpec), SubGraph(GraphId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescriptor {
    pub name: String, pub module: String, pub version: semver::Version,
    pub params: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32, pub backoff: BackoffStrategy, pub retry_on: Vec<ErrorCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Fixed { interval: Duration }, Linear { base: Duration, increment: Duration },
    Exponential { base: Duration, multiplier: f64, max: Duration },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCategory { Network, Timeout, Permission, DataNotFound, Validation, ResourceExhausted, Unknown }

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("语法解析失败: {0}")] SyntaxError(String),
    #[error("循环依赖检测: {0:?}")] CyclicDependency(Vec<NodeId>),
    #[error("类型不匹配: 节点 {node} 端口 {port}")] TypeMismatch { node: NodeId, port: String, expected: String, actual: String },
    #[error("未知操作: {0}")] UnknownAction(String),
}
```

### 2.2 任务分解器 (Task Decomposer)

```rust
pub trait TaskDecomposer: Send + Sync {
    fn decompose(&self, goal: &Goal) -> Result<Vec<SubTask>, DecomposeError>;
    fn analyze_dependencies(&self, tasks: &[SubTask]) -> DependencyMatrix;
    fn estimate_resources(&self, task: &SubTask) -> ResourceEstimate;
    fn optimize(&self, tasks: &[SubTask], deps: &DependencyMatrix) -> OptimizedPlan;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub description: String, pub constraints: Vec<Constraint>,
    pub priority: Priority, pub deadline: Option<Instant>, pub context: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: SubTaskId, pub description: String, pub action: ActionDescriptor,
    pub preconditions: Vec<Precondition>, pub postconditions: Vec<Postcondition>,
    pub estimated_duration: Duration, pub resource_requirements: ResourceEstimate,
    pub rollback_action: Option<ActionDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType { None, Sequential, WeakOrder, Exclusive, DataFlow }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedPlan {
    pub tasks: Vec<SubTask>, pub execution_order: Vec<Vec<SubTaskId>>,
    pub total_estimated_duration: Duration, pub peak_resource_usage: ResourceEstimate,
}

#[derive(Debug, thiserror::Error)]
pub enum DecomposeError {
    #[error("目标过于模糊: {0}")] AmbiguousGoal(String),
    #[error("缺少上下文: {0}")] MissingContext(String),
    #[error("分解深度超限: {0}")] DepthLimitExceeded(u32),
}
```

### 2.3 顺序执行器 (Sequential Executor)

```rust
pub trait SequentialExecutor: Send + Sync {
    fn execute(&self, dag: &InstructionGraph, ctx: &ExecutionContext) -> impl Future<Output = ExecutionResult> + Send;
    fn pause(&self, execution_id: ExecutionId) -> Result<(), ExecutionError>;
    fn resume(&self, execution_id: ExecutionId) -> Result<(), ExecutionError>;
    fn cancel(&self, execution_id: ExecutionId) -> Result<CancellationResult, ExecutionError>;
    fn status(&self, execution_id: ExecutionId) -> ExecutionStatus;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub execution_id: ExecutionId, pub variables: HashMap<String, Value>,
    pub credentials: SecureCredentialStore, pub resource_limits: ResourceLimits,
    pub tracing_id: TraceId, pub parent_execution: Option<ExecutionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus { Pending, Running, Paused, Completed, Failed { reason: String, recoverable: bool }, Cancelled, Recovering }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub execution_id: ExecutionId, pub status: ExecutionStatus,
    pub outputs: HashMap<NodeId, NodeOutput>, pub duration: Duration,
    pub resource_usage: ResourceUsageStats, pub error: Option<ExecutionError>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("节点执行失败: {node_id}")] NodeFailed { node_id: NodeId, reason: String },
    #[error("超时: 节点 {node_id}")] Timeout { node_id: NodeId, timeout: Duration },
    #[error("资源超限: {0}")] ResourceExceeded(String),
    #[error("沙箱违规: {0}")] SandboxViolation(String),
}
```

### 2.4 条件路由器 (Condition Router)

```rust
pub trait ConditionRouter: Send + Sync {
    fn evaluate(&self, condition: &ConditionExpr, ctx: &EvaluationContext) -> Result<bool, ConditionError>;
    fn route_if_else(&self, branches: &[IfElseBranch], ctx: &EvaluationContext) -> Result<RoutingDecision, ConditionError>;
    fn route_switch(&self, cases: &[SwitchCase], ctx: &EvaluationContext) -> Result<RoutingDecision, ConditionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionExpr {
    Compare { left: ExprOperand, op: CompareOp, right: ExprOperand },
    Logical { op: LogicOp, operands: Vec<ConditionExpr> },
    Exists { variable: String }, TypeCheck { variable: String, expected_type: ValueType },
    Custom { predicate_id: String, args: Vec<Value> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprOperand { Literal(Value), VariableRef(String), FunctionCall { name: String, args: Vec<ExprOperand> } }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompareOp { Eq, Ne, Lt, Le, Gt, Ge, Contains, Matches }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicOp { And, Or, Xor, Not }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub selected_branch: NodeId, pub evaluation_trace: Vec<(ConditionExpr, bool)>,
    pub execution_time: Duration,
}
```

### 2.5 循环控制器 (Loop Controller)

```rust
pub trait LoopController: Send + Sync {
    fn init_loop(&self, spec: &LoopSpec, ctx: &EvaluationContext) -> Result<LoopContext, LoopError>;
    fn should_continue(&self, ctx: &LoopContext) -> Result<bool, LoopError>;
    fn advance(&self, ctx: &mut LoopContext) -> Result<(), LoopError>;
    fn current_variables(&self, ctx: &LoopContext) -> HashMap<String, Value>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopSpec {
    pub loop_type: LoopType, pub max_iterations: u64, pub body_node: NodeId,
    pub iteration_variable: String, pub break_condition: Option<ConditionExpr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopType {
    ForRange { start: i64, end: i64, step: i64 },
    ForEach { collection_var: String },
    While { condition: ConditionExpr },
    Until { condition: ConditionExpr },
}

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("超过最大迭代: {current}/{max}")] MaxIterationsExceeded { current: u64, max: u64 },
    #[error("循环体执行失败: 迭代 {iteration}")] BodyExecutionFailed { iteration: u64, reason: String },
}
```

### 2.6 错误恢复策略 (5 级)

```rust
pub trait ErrorRecoveryManager: Send + Sync {
    fn analyze_and_recover(&self, error: &ExecutionError, ctx: &RecoveryContext) -> impl Future<Output = RecoveryResult> + Send;
    fn rollback(&self, execution_id: ExecutionId, to_checkpoint: CheckpointId) -> impl Future<Output = Result<(), RecoveryError>> + Send;
    fn request_human_intervention(&self, request: HumanInterventionRequest) -> InterventionTicket;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    Retry { max_attempts: u32, backoff: BackoffStrategy },                          // Level 1
    Skip { default_output: Value, log_level: LogLevel },                            // Level 2
    Rollback { checkpoint_id: CheckpointId, retry_after_rollback: bool },           // Level 3
    Degrade { fallback_action: ActionDescriptor, quality_impact: QualityImpact },   // Level 4
    HumanIntervention { reason: String, context_snapshot: Value, urgency: Urgency }, // Level 5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityImpact { None, Minor, Moderate, Severe }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Urgency { Low, Medium, High, Critical }
```

### 2.7 模板库 (Template Library)

```rust
pub trait TemplateLibrary: Send + Sync {
    fn search(&self, query: &TemplateQuery) -> Vec<Template>;
    fn get(&self, template_id: &TemplateId) -> Option<Template>;
    fn instantiate(&self, template_id: &TemplateId, params: HashMap<String, Value>) -> Result<InstructionGraph, TemplateError>;
    fn register(&self, template: Template) -> Result<TemplateId, TemplateError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: TemplateId, pub name: String, pub description: String,
    pub category: TemplateCategory, pub version: semver::Version,
    pub parameters: Vec<TemplateParameter>, pub graph_template: InstructionGraph,
    pub tags: Vec<String>, pub rating: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateCategory { FileProcessing, DataPipeline, Deployment, Testing, Monitoring, Communication, Custom(String) }
```

### 2.8 状态机

```
┌───────┐  parse()  ┌─────────┐ decompose() ┌───────────┐
│ IDLE  │─────────▶│ PARSING │───────────▶│DECOMPOSED │
└───────┘          └─────────┘            └─────┬─────┘
                                                │ execute()
                                                ▼
                                         ┌───────────┐
                                    ┌────│ EXECUTING │────┐
                                    │    └───────────┘    │
                                    ▼                    ▼
                              ┌──────────┐        ┌──────────┐
                              │ PAUSED   │        │ COMPLETED│
                              └──────────┘        └──────────┘
                                    │
                              resume()
                                    ▼
                              ┌───────────┐
                              │ EXECUTING │
                              └───────────┘

┌───────────┐  recover()  ┌────────────┐
│  FAILED   │───────────▶│ RECOVERING │──▶ EXECUTING / FAILED
└───────────┘            └────────────┘
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskState {
    Idle, Parsing { input: String, progress: f32 }, Decomposed { graph: InstructionGraph },
    Executing { progress: ExecutionProgress }, Paused { checkpoint: Checkpoint },
    Completed { result: ExecutionResult }, Failed { error: ExecutionError, recoverable: bool },
    Recovering { strategy: RecoveryStrategy, progress: f32 },
}

impl TaskState {
    pub fn can_transition(&self, target: &TaskState) -> bool {
        matches!((self, target),
            (TaskState::Idle, TaskState::Parsing { .. })
            | (TaskState::Parsing { .. }, TaskState::Decomposed { .. })
            | (TaskState::Decomposed { .. }, TaskState::Executing { .. })
            | (TaskState::Executing { .. }, TaskState::Paused { .. })
            | (TaskState::Executing { .. }, TaskState::Completed { .. })
            | (TaskState::Executing { .. }, TaskState::Failed { .. })
            | (TaskState::Paused { .. }, TaskState::Executing { .. })
            | (TaskState::Failed { recoverable: true, .. }, TaskState::Recovering { .. })
            | (TaskState::Recovering { .. }, TaskState::Executing { .. })
        )
    }
}
```

### 2.9 安全设计

| 安全维度 | 措施 |
|----------|------|
| 指令注入 | 输入验证 + 参数化操作调用，禁止直接执行原始字符串 |
| 权限控制 | 每个操作节点声明所需权限，执行前由权限管理器审核 |
| 资源限制 | 沙箱执行，CPU/内存/网络/磁盘全部受限 |
| 数据隔离 | 不同执行上下文之间的变量不可互相访问 |
| 审计日志 | 所有状态转换和操作执行记录不可变审计日志 |

### 2.10 测试用例

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_simple_instruction() {
        let parser = create_test_parser();
        let result = parser.parse("将 /tmp/data.csv 复制到 /backup/");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().nodes.len(), 1);
    }

    #[test]
    fn test_detect_cyclic_dependency() {
        let parser = create_test_parser();
        let graph = make_cyclic_graph(); // A→B→A
        let result = parser.validate(&graph);
        assert!(matches!(result, Err(ParseError::CyclicDependency(_))));
    }

    #[tokio::test]
    async fn test_decompose_goal() {
        let decomposer = create_test_decomposer();
        let goal = Goal { description: "部署应用到生产环境".into(), ..Default::default() };
        let tasks = decomposer.decompose(&goal).unwrap();
        assert!(tasks.iter().any(|t| t.description.contains("构建")));
        assert!(tasks.iter().any(|t| t.description.contains("部署")));
    }

    #[tokio::test]
    async fn test_loop_max_iterations() {
        let controller = create_test_loop_controller();
        let spec = LoopSpec { loop_type: LoopType::While { condition: always_true() }, max_iterations: 5, ..Default::default() };
        let mut ctx = controller.init_loop(&spec, &default()).unwrap();
        for _ in 0..5 { controller.advance(&mut ctx).unwrap(); }
        assert!(matches!(controller.should_continue(&ctx), Err(LoopError::MaxIterationsExceeded { .. })));
    }

    #[tokio::test]
    async fn test_retry_recovery() {
        let manager = create_test_recovery_manager();
        let error = ExecutionError::Timeout { node_id: NodeId::new(), timeout: Duration::from_secs(30) };
        let result = manager.analyze_and_recover(&error, &test_ctx()).await;
        assert!(result.success);
        assert!(matches!(result.strategy, RecoveryStrategy::Retry { .. }));
    }

    #[test]
    fn test_template_instantiation() {
        let library = create_test_template_library();
        let params = HashMap::from([("input_path".into(), Value::String("/data/in.csv".into()))]);
        assert!(library.instantiate(&TemplateId::new("csv-to-parquet"), params).is_ok());
    }
}
```

---

## 3. Part B：工作流自动化引擎

### 3.1 工作流定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: WorkflowId, pub name: String, pub version: semver::Version,
    pub description: String, pub variables: Vec<WorkflowVariable>,
    pub triggers: Vec<Trigger>, pub steps: Vec<WorkflowStep>,
    pub error_handling: WorkflowErrorPolicy, pub metadata: WorkflowMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVariable {
    pub name: String, pub var_type: ValueType, pub default_value: Option<Value>,
    pub source: VariableSource, pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableSource {
    Static, Input, Environment, Secret(String),
    Output { step_id: StepId, output_name: String }, Expression(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: StepId, pub name: String, pub action: ActionDescriptor,
    pub condition: Option<ConditionExpr>, pub retry_policy: RetryPolicy,
    pub timeout: Duration, pub on_failure: StepFailureAction,
    pub parallel_group: Option<String>, pub resource_requirements: ResourceEstimate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepFailureAction { Abort, Retry { max: u32 }, Skip, Fallback { action: ActionDescriptor }, ContinueWithError }

pub trait WorkflowParser: Send + Sync {
    fn parse_yaml(&self, yaml: &str) -> Result<WorkflowDefinition, WorkflowParseError>;
    fn parse_dsl(&self, dsl: &str) -> Result<WorkflowDefinition, WorkflowParseError>;
    fn validate(&self, workflow: &WorkflowDefinition) -> Result<Vec<ValidationWarning>, WorkflowParseError>;
}
```

### 3.2 工作流引擎

```rust
pub trait WorkflowEngine: Send + Sync {
    fn start(&self, workflow_id: &WorkflowId, inputs: HashMap<String, Value>) -> impl Future<Output = Result<WorkflowInstance, WorkflowError>> + Send;
    fn pause(&self, instance_id: &WorkflowInstanceId) -> Result<(), WorkflowError>;
    fn resume(&self, instance_id: &WorkflowInstanceId) -> Result<(), WorkflowError>;
    fn terminate(&self, instance_id: &WorkflowInstanceId, reason: &str) -> Result<(), WorkflowError>;
    fn get_instance(&self, instance_id: &WorkflowInstanceId) -> Option<WorkflowInstance>;
    fn list_instances(&self, filter: InstanceFilter) -> Vec<WorkflowInstance>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    pub id: WorkflowInstanceId, pub workflow_id: WorkflowId,
    pub status: WorkflowInstanceStatus, pub current_step: Option<StepId>,
    pub step_states: HashMap<StepId, StepState>, pub variables: HashMap<String, Value>,
    pub created_at: Instant, pub persistent_state: WorkflowPersistentState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowInstanceStatus {
    Pending, Running, Paused, Completed, Failed { step_id: StepId, reason: String },
    Terminated { reason: String }, WaitingForApproval { step_id: StepId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPersistentState {
    pub checkpoint_id: CheckpointId, pub serialized_context: Vec<u8>,
    pub version: u64, pub checksum: u64,
}
```

### 3.3 并行调度器

```rust
pub trait ParallelScheduler: Send + Sync {
    fn schedule(&self, dag: &ExecutionDAG, constraints: &SchedulingConstraints) -> impl Future<Output = ScheduleResult> + Send;
    fn update_constraints(&self, constraints: SchedulingConstraints);
    fn cancel(&self, schedule_id: &ScheduleId) -> Result<(), ScheduleError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingConstraints {
    pub max_parallel_tasks: usize, pub max_cpu_percent: u8,
    pub max_memory_mb: u64, pub max_gpu: Option<u32>,
    pub priority: Priority, pub deadline: Option<Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleResult {
    pub schedule_id: ScheduleId, pub execution_plan: Vec<ExecutionBatch>,
    pub estimated_duration: Duration, pub resource_allocation: ResourceAllocation,
}
```

### 3.4 事件触发器与 Cron 调度器

```rust
pub trait EventTrigger: Send + Sync {
    fn register(&self, trigger: &Trigger) -> Result<TriggerId, TriggerError>;
    fn unregister(&self, trigger_id: &TriggerId) -> Result<(), TriggerError>;
    fn fire(&self, event: &TriggerEvent) -> Result<Vec<WorkflowInstanceId>, TriggerError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Trigger {
    FileWatch { path: PathBuf, pattern: String, events: Vec<FileWatchEvent>, debounce: Duration },
    Cron { expression: String, timezone: Option<String> },
    AgentNotification { source_agent: AgentId, message_type: String, filter: Option<ConditionExpr> },
    DeviceEvent { device_id: DeviceId, event_types: Vec<String> },
    Webhook { path: String, method: HttpMethod, auth: Option<WebhookAuth> },
}

pub trait CronScheduler: Send + Sync {
    fn add_job(&self, job: CronJob) -> Result<CronJobId, CronError>;
    fn remove_job(&self, job_id: &CronJobId) -> Result<(), CronError>;
    fn next_execution(&self, job_id: &CronJobId) -> Option<Instant>;
    fn list_jobs(&self) -> Vec<CronJobInfo>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: CronJobId, pub name: String, pub expression: CronExpression,
    pub workflow_id: WorkflowId, pub inputs: HashMap<String, Value>,
    pub timezone: String, pub enabled: bool, pub persistent: bool,
    pub misfire_policy: MisfirePolicy,
}
```

### 3.5 工作流监控器

```rust
pub trait WorkflowMonitor: Send + Sync {
    fn get_status(&self, instance_id: &WorkflowInstanceId) -> WorkflowStatusSnapshot;
    fn get_metrics(&self, workflow_id: &WorkflowId) -> WorkflowMetrics;
    fn add_alert_rule(&self, rule: AlertRule) -> AlertRuleId;
    fn get_history(&self, filter: HistoryFilter) -> Vec<WorkflowExecutionRecord>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetrics {
    pub workflow_id: WorkflowId, pub total_runs: u64, pub success_rate: f64,
    pub avg_duration: Duration, pub p50_duration: Duration, pub p99_duration: Duration,
    pub failure_breakdown: HashMap<String, u64>,
}
```

### 3.6 工作流引擎测试用例

```rust
#[cfg(test)]
mod workflow_tests {
    #[test]
    fn test_parse_yaml_workflow() {
        let parser = create_test_workflow_parser();
        let yaml = r#"workflow: { name: "测试", version: "1.0.0", steps: [{ id: s1, action: test.echo }] }"#;
        let workflow = parser.parse_yaml(yaml).unwrap();
        assert_eq!(workflow.steps.len(), 1);
    }

    #[tokio::test]
    async fn test_workflow_lifecycle() {
        let engine = create_test_workflow_engine();
        let instance = engine.start(&test_workflow_id(), HashMap::new()).await.unwrap();
        assert_eq!(instance.status, WorkflowInstanceStatus::Running);
        engine.pause(&instance.id).unwrap();
        assert_eq!(engine.get_instance(&instance.id).unwrap().status, WorkflowInstanceStatus::Paused);
    }

    #[test]
    fn test_parallel_scheduler_constraints() {
        let scheduler = create_test_scheduler();
        let constraints = SchedulingConstraints { max_parallel_tasks: 2, ..Default::default() };
        let result = scheduler.schedule(&create_test_dag(5), &constraints).unwrap();
        assert!(result.execution_plan.iter().all(|b| b.tasks.len() <= 2));
    }

    #[test]
    fn test_cron_parsing() {
        let cron = CronExpression::parse("0 */6 * * *").unwrap();
        assert!(cron.next_after(Instant::now()).is_some());
    }
}
```

---

## 4. Part C：复杂顺序操作处理器

### 4.1 链式编排器

```rust
pub trait ChainOrchestrator: Send + Sync {
    fn build_chain(&self, operations: Vec<Operation>) -> Result<OperationChain, ChainError>;
    fn execute_chain(&self, chain: &OperationChain, initial_input: Value) -> impl Future<Output = ChainResult> + Send;
    fn insert_after(&self, chain_id: &ChainId, after: &OpId, operation: Operation) -> Result<(), ChainError>;
    fn remove(&self, chain_id: &ChainId, op_id: &OpId) -> Result<(), ChainError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationChain {
    pub id: ChainId, pub name: String, pub operations: Vec<LinkedOperation>,
    pub context_schema: ContextSchema, pub error_policy: ChainErrorPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedOperation {
    pub id: OpId, pub operation: Operation, pub input_mapping: Vec<PortMapping>,
    pub output_mapping: Vec<PortMapping>, pub timeout: Duration,
    pub retry_policy: RetryPolicy, pub condition: Option<ConditionExpr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: OpId, pub name: String, pub version: semver::Version,
    pub input_schema: Schema, pub output_schema: Schema,
    pub handler: OperationHandlerRef, pub resource_requirements: ResourceEstimate,
    pub sandbox_config: SandboxConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainResult {
    pub chain_id: ChainId, pub success: bool, pub final_output: Value,
    pub step_results: Vec<StepResult>, pub total_duration: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("操作不存在: {0}")] OperationNotFound(OpId),
    #[error("链式连接失败: {source} 输出与 {target} 输入不兼容")] IncompatibleChain { source: OpId, target: OpId },
    #[error("循环检测")] CyclicChain,
}
```

### 4.2 上下文传递

```rust
pub trait ContextManager: Send + Sync {
    fn create_context(&self, schema: &ContextSchema) -> ExecutionContext;
    fn pass_data(&self, from: &OpId, to: &OpId, data: &Value, mapping: &[PortMapping]) -> Result<Value, ContextError>;
    fn convert_type(&self, value: &Value, target_type: &ValueType) -> Result<Value, ContextError>;
    fn merge_contexts(&self, contexts: &[&ExecutionContext]) -> ExecutionContext;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformExpr {
    Identity, FieldSelect { field: String },
    FunctionCall { name: String, args: Vec<TransformExpr> },
    Conditional { condition: ConditionExpr, then: Box<TransformExpr>, else_: Box<TransformExpr> },
}
```

### 4.3 检查点/恢复

```rust
pub trait CheckpointManager: Send + Sync {
    fn create_checkpoint(&self, state: &CheckpointState) -> Result<Checkpoint, CheckpointError>;
    fn restore_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<CheckpointState, CheckpointError>;
    fn list_checkpoints(&self, chain_id: &ChainId) -> Vec<Checkpoint>;
    fn cleanup(&self, older_than: Duration) -> Result<u64, CheckpointError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    pub execution_context: HashMap<String, Value>, pub completed_steps: Vec<StepResult>,
    pub pending_steps: Vec<OpId>, pub variables: HashMap<String, Value>,
}
```

### 4.4 操作沙箱

```rust
pub trait OperationSandbox: Send + Sync {
    fn execute(&self, operation: &Operation, input: Value, config: &SandboxConfig) -> impl Future<Output = SandboxResult> + Send;
    fn destroy(&self, sandbox_id: &SandboxId) -> Result<(), SandboxError>;
    fn status(&self, sandbox_id: &SandboxId) -> Option<SandboxStatus>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub cpu_limit_millis: u64, pub memory_limit_mb: u64, pub disk_limit_mb: u64,
    pub network_policy: NetworkPolicy, pub allowed_paths: Vec<PathBuf>,
    pub read_only_paths: Vec<PathBuf>, pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkPolicy { Denied, AllowList { hosts: Vec<String>, ports: Vec<u16> }, OutboundOnly, Allowed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    pub success: bool, pub output: Value, pub stdout: String, pub stderr: String,
    pub exit_code: i32, pub duration: Duration, pub violations: Vec<SandboxViolation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType { CpuLimitExceeded, MemoryLimitExceeded, NetworkAccessDenied, FileAccessDenied { path: PathBuf }, TimeoutExceeded }
```

### 4.5 操作市场

```rust
pub trait OperationMarketplace: Send + Sync {
    fn publish(&self, op: &OperationPackage, publisher: &PublisherInfo) -> Result<OperationId, MarketplaceError>;
    fn install(&self, operation_id: &OperationId, version: Option<semver::Version>) -> Result<Operation, MarketplaceError>;
    fn search(&self, query: &MarketplaceQuery) -> Vec<OperationListing>;
    fn rate(&self, operation_id: &OperationId, rating: Rating, review: Option<String>) -> Result<(), MarketplaceError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationPackage {
    pub name: String, pub version: semver::Version, pub description: String,
    pub operation: Operation, pub dependencies: Vec<OperationDependency>,
    pub test_cases: Vec<TestCase>, pub tags: Vec<String>, pub category: MarketplaceCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketplaceCategory { DataProcessing, FileManagement, Network, Deployment, Testing, Security, AiMl, Custom(String) }
```

### 4.6 操作处理器测试用例

```rust
#[cfg(test)]
mod chain_tests {
    #[tokio::test]
    async fn test_chain_execution() {
        let orchestrator = create_test_chain_orchestrator();
        let chain = orchestrator.build_chain(vec![
            create_test_operation("fetch"), create_test_operation("transform"), create_test_operation("store"),
        ]).unwrap();
        let result = orchestrator.execute_chain(&chain, Value::Null).await;
        assert!(result.success && result.step_results.len() == 3);
    }

    #[tokio::test]
    async fn test_checkpoint_resume() {
        let mgr = create_test_checkpoint_manager();
        let state = CheckpointState { completed_steps: vec![make_step("s1")], pending_steps: vec!["s2".into()], ..Default::default() };
        let cp = mgr.create_checkpoint(&state).unwrap();
        let restored = mgr.restore_checkpoint(&cp.id).unwrap();
        assert_eq!(restored.completed_steps.len(), 1);
    }

    #[tokio::test]
    async fn test_sandbox_limits() {
        let sandbox = create_test_sandbox();
        let config = SandboxConfig { memory_limit_mb: 64, network_policy: NetworkPolicy::Denied, ..Default::default() };
        let result = sandbox.execute(&create_heavy_op(), Value::Null, &config).await;
        assert!(!result.success);
        assert!(result.violations.iter().any(|v| matches!(v.violation_type, ViolationType::MemoryLimitExceeded)));
    }

    #[test]
    fn test_marketplace_search() {
        let market = create_test_marketplace();
        let results = market.search(&MarketplaceQuery { keywords: vec!["csv".into()], verified_only: true, ..Default::default() });
        assert!(results.iter().all(|r| r.verified));
    }
}
```

---

## 5. 全局错误处理

| 级别 | 描述 | 处理方式 |
|------|------|----------|
| L1 | 临时性故障（网络超时） | 自动重试，指数退避 |
| L2 | 非关键步骤失败 | 记录日志，使用默认值继续 |
| L3 | 状态不一致 | 回滚到最近检查点 |
| L4 | 主方案不可用 | 切换到备用方案 |
| L5 | 无法自动恢复 | 暂停执行，通知人工 |

## 6. 配置参考

```toml
[automation]
[automation.decomposer]
max_depth = 10; max_subtasks = 100; llm_fallback = true

[automation.executor]
default_timeout = "300s"; max_parallel_tasks = 16; checkpoint_interval = "30s"; sandbox_enabled = true

[automation.recovery]
default_strategy = "retry"; max_recovery_attempts = 5; human_intervention_timeout = "24h"

[automation.workflow]
max_concurrent_workflows = 50; state_persistence = "sqlite"; checkpoint_compression = "zstd"

[automation.marketplace]
default_registry = "https://registry.omniagent.os"; verify_signatures = true
```

## 附录：性能基准

| 组件 | 操作 | 目标延迟 | 数据规模 |
|------|------|----------|----------|
| 指令解析器 | parse() | < 2s | 1000 token |
| 任务分解器 | decompose() | < 5s | 100 子任务 |
| 顺序执行器 | execute() | < 10ms/步 | 纯计算 |
| 条件路由器 | evaluate() | < 1ms | 单条件 |
| 循环控制器 | should_continue() | < 0.1ms | 单次检查 |
| 错误恢复 | analyze_and_recover() | < 500ms | 单节点 |
| 工作流引擎 | start() | < 100ms | 冷启动 |
| 并行调度器 | schedule() | < 50ms | 100 节点 DAG |
| 链式编排器 | execute_chain() | < 10ms/步 | 纯计算 |
| 检查点管理 | create_checkpoint() | < 50ms | 1MB 状态 |
