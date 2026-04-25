# 学习服务 API 参考

> **模块名称**: `learning-api`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

学习服务 API 提供 OmniAgent OS 中 Agent 和系统的自主学习能力。包括主动学习（探索、实验、技能创建）和被动学习（行为观察、反馈吸收、模式提取），以及知识图谱管理、技能进化、策略优化、迁移学习和遗忘机制。通过此 API，Agent 能够持续改进自身能力，适应不断变化的环境和用户需求。

### 1.2 架构概览

```
┌──────────────────────────────────────────────────────────┐
│                  Learning Service                         │
├──────────┬──────────┬──────────┬─────────────────────────┤
│ Active   │ Passive  │Knowledge │ Skill                   │
│ Learning │ Learning │  Graph   │ Evolution               │
├──────────┼──────────┼──────────┼─────────────────────────┤
│Strategy  │ Transfer │ Forgetting│ Learning                │
│ Optimiz. │ Learning │  System  │ Metrics                 │
├──────────┴──────────┴──────────┴─────────────────────────┤
│              Knowledge Base & Memory System               │
└──────────────────────────────────────────────────────────┘
```

---

## 2. 主动学习 API

### 2.1 知识缺口识别与探索

```rust
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Agent 标识符
pub type AgentId = u64;

/// 知识缺口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    /// 缺口 ID
    pub id: String,
    /// 缺口领域
    pub domain: String,
    /// 缺口描述
    pub description: String,
    /// 缺口严重程度 (0.0 - 1.0)
    pub severity: f64,
    /// 建议的学习资源
    pub suggested_resources: Vec<LearningResource>,
    /// 预估填补时间
    pub estimated_effort: Duration,
    /// 优先级
    pub priority: f64,
}

/// 学习资源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningResource {
    pub resource_type: ResourceType,
    pub title: String,
    pub source: String,
    pub relevance_score: f64,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Document,
    CodeExample,
    Tutorial,
    Dataset,
    AgentKnowledge,
    WebSearch,
}

/// 主题探索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationResult {
    pub topic: String,
    pub discovered_concepts: Vec<DiscoveredConcept>,
    pub relationships: Vec<ConceptRelation>,
    pub confidence: f64,
    pub exploration_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredConcept {
    pub concept_id: String,
    pub name: String,
    pub description: String,
    pub confidence: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRelation {
    pub from_concept: String,
    pub to_concept: String,
    pub relation_type: String,
    pub strength: f64,
}

/// 实验结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub experiment_id: String,
    pub hypothesis: String,
    pub outcome: ExperimentOutcome,
    pub observations: Vec<String>,
    pub metrics: HashMap<String, f64>,
    pub conclusion: String,
    pub reproducible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentOutcome {
    Confirmed,
    Rejected,
    Inconclusive,
    Partial,
}

/// 技能定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub skill_type: SkillType,
    /// 技能参数
    pub parameters: Vec<SkillParameter>,
    /// 技能步骤
    pub steps: Vec<SkillStep>,
    /// 技能等级
    pub proficiency: ProficiencyLevel,
    /// 使用次数
    pub usage_count: u64,
    /// 成功率
    pub success_rate: f64,
    /// 创建时间
    pub created_at: std::time::Instant,
    /// 最后使用时间
    pub last_used: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillType {
    /// 分析技能
    Analytical,
    /// 创造技能
    Creative,
    /// 操作技能
    Operational,
    /// 交互技能
    Interactive,
    /// 推理技能
    Reasoning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProficiencyLevel {
    Novice,
    Beginner,
    Intermediate,
    Advanced,
    Expert,
    Master,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStep {
    pub step_number: u32,
    pub description: String,
    pub action: String,
    pub expected_outcome: String,
}

/// 问答结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionAnswer {
    pub question: String,
    pub answer: String,
    pub confidence: f64,
    pub sources: Vec<String>,
    pub follow_up_questions: Vec<String>,
}

/// 主动学习 trait
pub trait ActiveLearning: Send + Sync {
    /// 识别知识缺口
    fn identify_gaps(&self, agent_id: AgentId, domain: Option<&str>) -> Result<Vec<KnowledgeGap>, LearningError>;

    /// 探索主题
    fn explore_topic(&self, agent_id: AgentId, topic: &str, depth: u32) -> Result<ExplorationResult, LearningError>;

    /// 运行实验
    fn run_experiment(&self, agent_id: AgentId, hypothesis: &str, config: ExperimentConfig) -> Result<ExperimentResult, LearningError>;

    /// 创建技能
    fn create_skill(&self, agent_id: AgentId, skill: Skill) -> Result<String, LearningError>;

    /// 提问
    fn ask_question(&self, agent_id: AgentId, question: &str, context: Option<&str>) -> Result<QuestionAnswer, LearningError>;
}

/// 实验配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub max_iterations: u32,
    pub timeout: Duration,
    pub evaluation_metrics: Vec<String>,
    pub control_group: bool,
    pub sample_size: u32,
}
```

---

## 3. 被动学习 API

### 3.1 行为观察与反馈吸收

```rust
/// 行为观察记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorObservation {
    pub observation_id: String,
    pub agent_id: AgentId,
    /// 观察类型
    pub observation_type: ObservationType,
    /// 触发条件
    pub trigger: String,
    /// 执行的动作
    pub action: String,
    /// 动作结果
    pub outcome: ActionOutcome,
    /// 上下文
    pub context: HashMap<String, String>,
    /// 时间戳
    pub timestamp: std::time::Instant,
    /// 持续时间
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationType {
    /// 用户操作
    UserAction,
    /// Agent 决策
    AgentDecision,
    /// 系统事件
    SystemEvent,
    /// 错误发生
    ErrorEvent,
    /// 成功完成
    SuccessEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionOutcome {
    Success,
    Failure,
    Partial,
    Timeout,
    Cancelled,
}

/// 反馈记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRecord {
    pub feedback_id: String,
    pub agent_id: AgentId,
    /// 反馈来源
    pub source: FeedbackSource,
    /// 反馈类型
    pub feedback_type: FeedbackType,
    /// 反馈内容
    pub content: String,
    /// 相关任务/操作 ID
    pub related_id: Option<String>,
    /// 评分 (1-5)
    pub rating: Option<u8>,
    /// 时间戳
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackSource {
    User,
    Agent,
    System,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackType {
    Positive,
    Negative,
    Correction,
    Suggestion,
    Rating,
}

/// 错误记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub error_id: String,
    pub agent_id: AgentId,
    pub error_type: String,
    pub error_message: String,
    pub context: HashMap<String, String>,
    pub stack_trace: Option<String>,
    pub severity: ErrorSeverity,
    pub resolution: Option<String>,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// 提取的模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedPattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub description: String,
    /// 模式规则
    pub rules: Vec<PatternRule>,
    /// 支持度
    pub support: f64,
    /// 置信度
    pub confidence: f64,
    /// 出现频率
    pub frequency: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    Behavioral,
    Temporal,
    Causal,
    Correlational,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRule {
    pub condition: String,
    pub consequence: String,
    pub probability: f64,
}

/// 学习到的偏好
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPreference {
    pub preference_id: String,
    pub agent_id: AgentId,
    pub category: String,
    pub key: String,
    pub value: serde_json::Value,
    pub confidence: f64,
    pub source: PreferenceSource,
    pub learned_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreferenceSource {
    DirectFeedback,
    ObservedBehavior,
    Inferred,
    Explicit,
}

/// 被动学习 trait
pub trait PassiveLearning: Send + Sync {
    /// 观察行为
    fn observe_behavior(&self, observation: BehaviorObservation) -> Result<(), LearningError>;

    /// 吸收反馈
    fn absorb_feedback(&self, feedback: FeedbackRecord) -> Result<(), LearningError>;

    /// 记录错误
    fn record_error(&self, error: ErrorRecord) -> Result<(), LearningError>;

    /// 提取模式
    fn extract_patterns(&self, agent_id: AgentId, domain: Option<&str>) -> Result<Vec<ExtractedPattern>, LearningError>;

    /// 学习偏好
    fn learn_preference(&self, preference: LearnedPreference) -> Result<(), LearningError>;
}
```

---

## 4. 知识图谱 API

### 4.1 图谱操作

```rust
/// 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: String,
    pub entity_type: String,
    pub name: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub embedding: Option<Vec<f32>>,
    pub created_at: std::time::Instant,
    pub updated_at: std::time::Instant,
}

/// 关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub relation_id: String,
    pub from_entity: String,
    pub to_entity: String,
    pub relation_type: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub weight: f64,
    pub confidence: f64,
    pub created_at: std::time::Instant,
}

/// 子图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraph {
    pub center_entity: String,
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub depth: u32,
    pub max_nodes: u32,
}

/// 图查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQueryResult {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub total_matches: usize,
    pub query_time: Duration,
}

/// 知识图谱 trait
pub trait KnowledgeGraph: Send + Sync {
    /// 添加实体
    fn add_entity(&self, entity: Entity) -> Result<String, LearningError>;

    /// 添加关系
    fn add_relation(&self, relation: Relation) -> Result<String, LearningError>;

    /// 查询图谱
    fn query_graph(&self, query: &GraphQuery) -> Result<GraphQueryResult, LearningError>;

    /// 获取子图
    fn get_subgraph(&self, center_entity_id: &str, depth: u32, max_nodes: u32) -> Result<SubGraph, LearningError>;

    /// 删除实体
    fn remove_entity(&self, entity_id: &str) -> Result<(), LearningError>;

    /// 更新实体
    fn update_entity(&self, entity_id: &str, properties: HashMap<String, serde_json::Value>) -> Result<(), LearningError>;
}

/// 图查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQuery {
    pub query_type: GraphQueryType,
    pub conditions: Vec<QueryCondition>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphQueryType {
    /// 精确匹配
    ExactMatch,
    /// 模糊匹配
    FuzzyMatch,
    /// 语义搜索
    SemanticSearch,
    /// 路径查询
    PathQuery,
    /// 邻居查询
    NeighborQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCondition {
    pub field: String,
    pub operator: QueryOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryOperator {
    Equals,
    NotEquals,
    Contains,
    GreaterThan,
    LessThan,
    Range,
    In,
    Exists,
}
```

---

## 5. 知识融合 API

### 5.1 多源知识整合

```rust
/// 知识源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSource {
    pub source_id: String,
    pub source_type: SourceType,
    pub name: String,
    pub reliability: f64,
    pub last_updated: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    AgentExperience,
    UserFeedback,
    ExternalData,
    SystemLog,
    Document,
    WebContent,
}

/// 融合结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionResult {
    pub fused_entities: Vec<Entity>,
    pub fused_relations: Vec<Relation>,
    pub conflicts: Vec<KnowledgeConflict>,
    pub new_knowledge_count: usize,
    pub updated_knowledge_count: usize,
}

/// 知识冲突
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeConflict {
    pub conflict_id: String,
    pub entity_id: String,
    pub field_name: String,
    pub source_a: KnowledgeSource,
    pub value_a: serde_json::Value,
    pub source_b: KnowledgeSource,
    pub value_b: serde_json::Value,
    pub resolution: ConflictResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// 保留较新的值
    KeepNewer,
    /// 保留较可靠的源
    KeepMoreReliable,
    /// 合并两个值
    Merge,
    /// 保留两个值（标记为待裁决，等待人工审核确认）
    KeepBoth,
    /// 人工审核
    ManualReview,
}

/// 知识融合 trait
pub trait KnowledgeFusion: Send + Sync {
    /// 融合多个知识源
    fn fuse_sources(&self, sources: &[KnowledgeSource], entities: &[Entity]) -> Result<FusionResult, LearningError>;

    /// 解决冲突
    fn resolve_conflict(&self, conflict_id: &str, resolution: ConflictResolution) -> Result<(), LearningError>;
}
```

---

## 6. 技能进化 API

### 6.1 技能优化与排名

```rust
/// 技能进化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEvolution {
    pub skill_id: String,
    pub evolution_type: EvolutionType,
    pub changes: Vec<SkillChange>,
    pub old_proficiency: ProficiencyLevel,
    pub new_proficiency: ProficiencyLevel,
    pub improvement_score: f64,
    pub evolved_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvolutionType {
    /// 参数优化
    ParameterOptimization,
    /// 步骤改进
    StepImprovement,
    /// 新增步骤
    StepAddition,
    /// 步骤删除
    StepRemoval,
    /// 条件分支
    ConditionalBranch,
    /// 错误处理改进
    ErrorHandlingImprovement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillChange {
    pub change_type: String,
    pub description: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub impact: f64,
}

/// 技能排名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRanking {
    pub skill_id: String,
    pub rank: u32,
    pub score: f64,
    pub criteria: Vec<RankingCriterion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingCriterion {
    pub name: String,
    pub weight: f64,
    pub score: f64,
}

/// 进化历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionHistory {
    pub skill_id: String,
    pub evolutions: Vec<SkillEvolution>,
    pub total_improvements: u32,
    pub current_proficiency: ProficiencyLevel,
}

/// 技能进化 trait
pub trait SkillEvolution: Send + Sync {
    /// 进化技能
    fn evolve_skill(&self, agent_id: AgentId, skill_id: &str) -> Result<SkillEvolution, LearningError>;

    /// 排名技能
    fn rank_skills(&self, agent_id: AgentId, domain: Option<&str>) -> Result<Vec<SkillRanking>, LearningError>;

    /// 获取进化历史
    fn get_evolution_history(&self, skill_id: &str) -> Result<EvolutionHistory, LearningError>;
}
```

---

## 7. 策略优化 API

### 7.1 决策策略改进

```rust
/// 策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub strategy_id: String,
    pub name: String,
    pub domain: String,
    pub rules: Vec<StrategyRule>,
    pub parameters: HashMap<String, f64>,
    pub performance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRule {
    pub condition: String,
    pub action: String,
    pub priority: u32,
    pub success_rate: f64,
}

/// 优化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub strategy_id: String,
    pub old_score: f64,
    pub new_score: f64,
    pub improvement: f64,
    pub parameter_changes: HashMap<String, ParameterChange>,
    pub rule_changes: Vec<RuleChange>,
    pub iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterChange {
    pub parameter: String,
    pub old_value: f64,
    pub new_value: f64,
    pub impact: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleChange {
    pub change_type: String,
    pub rule: StrategyRule,
}

/// 策略指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetrics {
    pub strategy_id: String,
    pub total_executions: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_execution_time: Duration,
    pub success_rate: f64,
    pub improvement_trend: f64,
    pub domain_coverage: f64,
}

/// 策略优化 trait
pub trait StrategyOptimization: Send + Sync {
    /// 优化策略
    fn optimize_strategy(&self, strategy_id: &str, config: OptimizationConfig) -> Result<OptimizationResult, LearningError>;

    /// 获取策略指标
    fn get_metrics(&self, strategy_id: &str) -> Result<StrategyMetrics, LearningError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    pub max_iterations: u32,
    pub target_improvement: f64,
    pub exploration_rate: f64,
    pub evaluation_samples: u32,
}
```

---

## 8. 迁移学习 API

### 8.1 知识迁移

```rust
/// 迁移请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub source_agent: AgentId,
    pub target_agent: AgentId,
    pub knowledge_types: Vec<String>,
    pub domains: Vec<String>,
    pub transfer_mode: TransferMode,
    pub validation_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferMode {
    /// 完整复制
    FullCopy,
    /// 选择性迁移
    Selective,
    /// 抽象迁移（仅迁移抽象知识）
    AbstractOnly,
    /// 增量迁移
    Incremental,
}

/// 迁移结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub transfer_id: String,
    pub source_agent: AgentId,
    pub target_agent: AgentId,
    pub transferred_entities: u32,
    pub transferred_relations: u32,
    pub transferred_skills: u32,
    pub validation_score: f64,
    pub estimated_impact: f64,
    pub warnings: Vec<String>,
}

/// 迁移报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferReport {
    pub transfer_id: String,
    pub source_agent: AgentId,
    pub target_agent: AgentId,
    pub summary: String,
    pub transferred_items: Vec<TransferredItem>,
    pub compatibility_score: f64,
    pub potential_issues: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferredItem {
    pub item_type: String,
    pub item_id: String,
    pub name: String,
    pub transfer_status: TransferStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    Success,
    Partial,
    Failed,
    Skipped,
}

/// 迁移学习 trait
pub trait TransferLearning: Send + Sync {
    /// 执行知识迁移
    fn transfer_knowledge(&self, request: TransferRequest) -> Result<TransferResult, LearningError>;

    /// 获取迁移报告
    fn get_transfer_report(&self, transfer_id: &str) -> Result<TransferReport, LearningError>;
}
```

---

## 9. 遗忘 API

### 9.1 知识衰减与修剪

```rust
/// 记忆保留评估
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionEvaluation {
    pub entity_id: String,
    pub retention_score: f64,
    pub last_accessed: std::time::Instant,
    pub access_frequency: f64,
    pub importance: f64,
    pub decay_rate: f64,
    pub should_prune: bool,
}

/// 遗忘统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingStats {
    pub total_entities: u32,
    pub pruned_entities: u32,
    pub decayed_entities: u32,
    pub retained_entities: u32,
    pub average_retention: f64,
    pub memory_usage_before: u64,
    pub memory_usage_after: u64,
    pub freed_memory: u64,
}

/// 遗忘配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingConfig {
    /// 最低保留分数
    pub min_retention_score: f64,
    /// 衰减速率
    pub decay_rate: f64,
    /// 是否启用基于重要性的保护
    pub importance_protection: bool,
    /// 重要实体最低保留分数
    pub important_entity_min_score: f64,
    /// 最大修剪比例
    pub max_prune_ratio: f64,
}

impl Default for ForgettingConfig {
    fn default() -> Self {
        Self {
            min_retention_score: 0.1,
            decay_rate: 0.01,
            importance_protection: true,
            important_entity_min_score: 0.5,
            max_prune_ratio: 0.2,
        }
    }
}

/// 遗忘 trait
pub trait Forgetting: Send + Sync {
    /// 评估记忆保留
    fn evaluate_retention(&self, agent_id: AgentId) -> Result<Vec<RetentionEvaluation>, LearningError>;

    /// 修剪知识
    fn prune_knowledge(&self, agent_id: AgentId, config: ForgettingConfig) -> Result<ForgettingStats, LearningError>;

    /// 获取遗忘统计
    fn get_forgetting_stats(&self, agent_id: AgentId) -> Result<ForgettingStats, LearningError>;
}
```

---

## 10. 学习指标 API

### 10.1 学习效果度量

```rust
/// 学习指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningMetrics {
    pub agent_id: AgentId,
    /// 总学习时长
    pub total_learning_time: Duration,
    /// 主动学习次数
    pub active_learning_count: u32,
    /// 被动学习次数
    pub passive_learning_count: u32,
    /// 技能数量
    pub skill_count: u32,
    /// 平均技能等级
    pub avg_proficiency: f64,
    /// 知识实体数量
    pub knowledge_entity_count: u32,
    /// 知识关系数量
    pub knowledge_relation_count: u32,
    /// 学习效率（新知识/小时）
    pub learning_efficiency: f64,
    /// 错误率趋势
    pub error_rate_trend: f64,
    /// 综合学习评分
    pub overall_score: f64,
}

/// 知识统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStats {
    pub agent_id: AgentId,
    pub total_entities: u32,
    pub total_relations: u32,
    pub entities_by_type: HashMap<String, u32>,
    pub relations_by_type: HashMap<String, u32>,
    pub domain_coverage: HashMap<String, f64>,
    pub knowledge_freshness: f64,
    pub knowledge_completeness: f64,
    pub average_confidence: f64,
    pub knowledge_growth_rate: f64,
}

/// 学习指标 trait
pub trait LearningMetricsAPI: Send + Sync {
    /// 获取学习指标
    fn get_learning_metrics(&self, agent_id: AgentId) -> Result<LearningMetrics, LearningError>;

    /// 获取知识统计
    fn get_knowledge_stats(&self, agent_id: AgentId) -> Result<KnowledgeStats, LearningError>;
}
```

---

## 11. 错误处理

```rust
/// 学习服务错误类型
#[derive(Debug, thiserror::Error)]
pub enum LearningError {
    #[error("Agent 不存在: {0}")]
    AgentNotFound(AgentId),

    #[error("知识实体不存在: {0}")]
    EntityNotFound(String),

    #[error("技能不存在: {0}")]
    SkillNotFound(String),

    #[error("策略不存在: {0}")]
    StrategyNotFound(String),

    #[error("知识冲突: {0}")]
    KnowledgeConflict(String),

    #[error("学习资源不可用: {0}")]
    ResourceUnavailable(String),

    #[error("实验失败: {0}")]
    ExperimentFailed(String),

    #[error("知识迁移失败: {0}")]
    TransferFailed(String),

    #[error("图谱查询失败: {0}")]
    GraphQueryFailed(String),

    #[error("进化失败: {0}")]
    EvolutionFailed(String),

    #[error("优化失败: {0}")]
    OptimizationFailed(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("超时: {0}")]
    Timeout(String),

    #[error("内部错误: {0}")]
    InternalError(String),
}
```

---

## 12. 使用示例

### 12.1 主动学习工作流

```rust
use learning_api::*;

async fn active_learning_example() -> Result<(), Box<dyn std::error::Error>> {
    let learner = ActiveLearningImpl::new();
    let agent_id: AgentId = 1;

    // 1. 识别知识缺口
    let gaps = learner.identify_gaps(agent_id, Some("data_science"))?;
    for gap in &gaps {
        println!("缺口: {} (严重程度: {:.2})", gap.description, gap.severity);
    }

    // 2. 探索主题
    let exploration = learner.explore_topic(agent_id, "machine learning basics", 3)?;
    println!("发现概念: {}", exploration.discovered_concepts.len());

    // 3. 运行实验
    let config = ExperimentConfig {
        max_iterations: 100,
        timeout: Duration::from_secs(300),
        evaluation_metrics: vec!["accuracy".to_string(), "speed".to_string()],
        control_group: true,
        sample_size: 1000,
    };
    let result = learner.run_experiment(agent_id, "增加数据量能提高准确率", config)?;
    println!("实验结果: {:?}", result.outcome);

    // 4. 创建技能
    let skill = Skill {
        skill_id: String::new(),
        name: "数据分析".to_string(),
        description: "对结构化数据进行分析和可视化".to_string(),
        domain: "data_science".to_string(),
        skill_type: SkillType::Analytical,
        parameters: vec![
            SkillParameter {
                name: "data_source".to_string(),
                param_type: "string".to_string(),
                required: true,
                default_value: None,
            },
        ],
        steps: vec![
            SkillStep {
                step_number: 1,
                description: "加载数据".to_string(),
                action: "load_data".to_string(),
                expected_outcome: "数据集已加载".to_string(),
            },
            SkillStep {
                step_number: 2,
                description: "数据清洗".to_string(),
                action: "clean_data".to_string(),
                expected_outcome: "数据已清洗".to_string(),
            },
        ],
        proficiency: ProficiencyLevel::Beginner,
        usage_count: 0,
        success_rate: 0.0,
        created_at: std::time::Instant::now(),
        last_used: None,
    };
    let skill_id = learner.create_skill(agent_id, skill)?;
    println!("技能已创建: {}", skill_id);

    Ok(())
}
```

### 12.2 知识图谱操作

```rust
async fn knowledge_graph_example() -> Result<(), Box<dyn std::error::Error>> {
    let graph = KnowledgeGraphImpl::new();

    // 添加实体
    let entity = Entity {
        entity_id: String::new(),
        entity_type: "concept".to_string(),
        name: "Rust".to_string(),
        properties: HashMap::from([
            ("category".to_string(), serde_json::json!("programming_language")),
            ("year_created".to_string(), serde_json::json!(2010)),
        ]),
        embedding: None,
        created_at: std::time::Instant::now(),
        updated_at: std::time::Instant::now(),
    };
    let entity_id = graph.add_entity(entity)?;

    // 添加关系
    let relation = Relation {
        relation_id: String::new(),
        from_entity: entity_id.clone(),
        to_entity: "systems_programming".to_string(),
        relation_type: "used_for".to_string(),
        properties: HashMap::new(),
        weight: 0.9,
        confidence: 0.95,
        created_at: std::time::Instant::now(),
    };
    graph.add_relation(relation)?;

    // 查询子图
    let subgraph = graph.get_subgraph(&entity_id, 2, 50)?;
    println!("子图包含 {} 个实体", subgraph.entities.len());

    Ok(())
}
```

### 12.3 遗忘机制

```rust
async fn forgetting_example() -> Result<(), Box<dyn std::error::Error>> {
    let forgetting = ForgettingImpl::new();
    let agent_id: AgentId = 1;

    // 评估记忆保留
    let evaluations = forgetting.evaluate_retention(agent_id)?;
    for eval in &evaluations {
        if eval.should_prune {
            println!("建议修剪: {} (保留分数: {:.2})", eval.entity_id, eval.retention_score);
        }
    }

    // 执行知识修剪
    let config = ForgettingConfig::default();
    let stats = forgetting.prune_knowledge(agent_id, config)?;
    println!("修剪了 {} 个实体，释放了 {} 字节", stats.pruned_entities, stats.freed_memory);

    Ok(())
}
```

---

## 13. 性能约束

| 操作 | 延迟目标 | 吞吐量目标 | 说明 |
|------|---------|-----------|------|
| identify_gaps | <500ms | 10/s | 含推理 |
| explore_topic (depth=3) | <2s | 5/s | 含多步推理 |
| run_experiment | <30s | 0.5/s | 取决于实验复杂度 |
| create_skill | <100ms | 50/s | 持久化 |
| observe_behavior | <10ms | 1000/s | 异步记录 |
| absorb_feedback | <10ms | 1000/s | 异步记录 |
| extract_patterns | <1s | 10/s | 含模式挖掘 |
| add_entity | <5ms | 500/s | 图谱写入 |
| query_graph | <50ms | 100/s | 索引查询 |
| get_subgraph (depth=2) | <100ms | 50/s | 图遍历 |
| fuse_sources (10 sources) | <2s | 5/s | 含冲突检测 |
| evolve_skill | <5s | 1/s | 含评估 |
| optimize_strategy | <10s | 0.5/s | 含多轮优化 |
| transfer_knowledge | <5s | 1/s | 含验证 |
| evaluate_retention | <1s | 10/s | 全量扫描 |
| prune_knowledge | <2s | 5/s | 含安全检查 |

---

## 14. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proficiency_ordering() {
        assert!(ProficiencyLevel::Novice < ProficiencyLevel::Beginner);
        assert!(ProficiencyLevel::Beginner < ProficiencyLevel::Intermediate);
        assert!(ProficiencyLevel::Intermediate < ProficiencyLevel::Advanced);
        assert!(ProficiencyLevel::Advanced < ProficiencyLevel::Expert);
        assert!(ProficiencyLevel::Expert < ProficiencyLevel::Master);
    }

    #[test]
    fn test_forgetting_config_default() {
        let config = ForgettingConfig::default();
        assert!((config.min_retention_score - 0.1).abs() < 0.01);
        assert!(config.importance_protection);
        assert_eq!(config.important_entity_min_score, 0.5);
    }

    #[test]
    fn test_experiment_outcome() {
        let outcomes = [
            ExperimentOutcome::Confirmed,
            ExperimentOutcome::Rejected,
            ExperimentOutcome::Inconclusive,
            ExperimentOutcome::Partial,
        ];
        assert_eq!(outcomes.len(), 4);
    }

    #[test]
    fn test_skill_types() {
        let types = [
            SkillType::Analytical,
            SkillType::Creative,
            SkillType::Operational,
            SkillType::Interactive,
            SkillType::Reasoning,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_observation_types() {
        let types = [
            ObservationType::UserAction,
            ObservationType::AgentDecision,
            ObservationType::SystemEvent,
            ObservationType::ErrorEvent,
            ObservationType::SuccessEvent,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_feedback_types() {
        let types = [
            FeedbackType::Positive,
            FeedbackType::Negative,
            FeedbackType::Correction,
            FeedbackType::Suggestion,
            FeedbackType::Rating,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_graph_query_types() {
        let types = [
            GraphQueryType::ExactMatch,
            GraphQueryType::FuzzyMatch,
            GraphQueryType::SemanticSearch,
            GraphQueryType::PathQuery,
            GraphQueryType::NeighborQuery,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_conflict_resolution() {
        let resolutions = [
            ConflictResolution::KeepNewer,
            ConflictResolution::KeepMoreReliable,
            ConflictResolution::Merge,
            ConflictResolution::KeepBoth,
            ConflictResolution::ManualReview,
        ];
        assert_eq!(resolutions.len(), 5);
    }

    #[test]
    fn test_transfer_modes() {
        let modes = [
            TransferMode::FullCopy,
            TransferMode::Selective,
            TransferMode::AbstractOnly,
            TransferMode::Incremental,
        ];
        assert_eq!(modes.len(), 4);
    }

    #[test]
    fn test_pattern_types() {
        let types = [
            PatternType::Behavioral,
            PatternType::Temporal,
            PatternType::Causal,
            PatternType::Correlational,
        ];
        assert_eq!(types.len(), 4);
    }

    #[test]
    fn test_entity_creation() {
        let entity = Entity {
            entity_id: "e-1".to_string(),
            entity_type: "concept".to_string(),
            name: "Test".to_string(),
            properties: HashMap::new(),
            embedding: None,
            created_at: std::time::Instant::now(),
            updated_at: std::time::Instant::now(),
        };
        assert_eq!(entity.entity_id, "e-1");
    }

    #[test]
    fn test_learning_metrics() {
        let metrics = LearningMetrics {
            agent_id: 1,
            total_learning_time: Duration::from_secs(3600),
            active_learning_count: 50,
            passive_learning_count: 200,
            skill_count: 10,
            avg_proficiency: 0.7,
            knowledge_entity_count: 500,
            knowledge_relation_count: 1200,
            learning_efficiency: 5.0,
            error_rate_trend: -0.1,
            overall_score: 0.75,
        };
        assert_eq!(metrics.skill_count, 10);
        assert!(metrics.error_rate_trend < 0.0);
    }
}
```

---

*本文档为 OmniAgent OS 学习服务 API 参考，版本 0.1.0。*
