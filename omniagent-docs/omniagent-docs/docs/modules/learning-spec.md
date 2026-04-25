# OmniAgent OS — 高级学习服务模块规格说明

> **模块编号**: OA-LRN-003 | **版本**: v1.0.0-draft | **状态**: 设计中
> **依赖**: 内核调度器 (OA-KRN-002)、知识存储 (OA-KNO-007)、多模态交互 (OA-MMI-002)

## 1. 概述

高级学习服务是 OmniAgent OS 的智能核心，赋予系统持续学习和自我进化的能力。由三个子系统构成：主动学习引擎（知识缺口发现/搜索验证/自实验/技能创建/主动提问）、被动学习引擎（行为观察/反馈吸收/错误学习/模式提取/偏好学习）、知识演化系统（知识图谱/融合/技能进化/策略优化/迁移学习/遗忘管理）。

### 1.1 设计原则

| 原则 | 说明 |
|------|------|
| 持续性 | 学习过程贯穿系统整个生命周期，无需人工干预 |
| 可解释性 | 每个学习决策可追溯其来源和依据 |
| 渐进性 | 知识积累是渐进的，避免剧烈的行为变化 |
| 安全性 | 学习过程不破坏已有能力，新知识经过验证后才生效 |
| 可控性 | 用户可查看、调整、限制学习范围和方式 |

### 1.2 性能约束

| 指标 | 目标值 | 测量条件 |
|------|--------|----------|
| 反馈集成延迟 | < 1s | 单条反馈从接收到权重更新完成 |
| 知识图谱查询 | < 10ms | 单次模式查询，100 万节点规模 |
| 模式提取延迟 | < 5s | 1000 条交互记录分析 |
| 知识缺口识别 | < 3s | 基于当前知识图谱扫描 |
| 技能创建延迟 | < 10s | 从模式识别到技能注册 |
| 知识融合延迟 | < 2s | 单次多源融合操作 |

### 1.3 学习指标体系

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearningMetrics {
    // 主动学习
    pub knowledge_gaps_found: u64, pub knowledge_gaps_resolved: u64,
    pub experiments_conducted: u64, pub experiments_successful: f64,
    pub skills_created: u64, pub skills_active: u64, pub questions_asked: u64,
    // 被动学习
    pub interactions_observed: u64, pub feedback_absorbed: u64,
    pub positive_feedback_ratio: f64, pub errors_learned: u64,
    pub error_prevention_rate: f64, pub patterns_extracted: u64, pub preferences_learned: u64,
    // 知识演化
    pub knowledge_nodes: u64, pub knowledge_edges: u64, pub knowledge_fusions: u64,
    pub conflicts_resolved: u64, pub skills_evolved: u64, pub strategies_optimized: u64,
    pub transfers_applied: u64, pub knowledge_pruned: u64, pub retention_rate: f64,
    // 系统级
    pub total_learning_time: Duration, pub avg_learning_accuracy: f64,
    pub knowledge_coverage: f64, pub adaptation_speed: f64,
}

pub trait MetricsCollector: Send + Sync {
    fn record(&self, event: LearningEvent);
    fn snapshot(&self) -> LearningMetrics;
    fn export(&self) -> serde_json::Value;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LearningEvent {
    KnowledgeGapFound { topic: String, confidence: f32 },
    KnowledgeGapResolved { topic: String, source: KnowledgeSource },
    ExperimentCompleted { hypothesis_id: HypothesisId, success: bool },
    SkillCreated { skill_id: SkillId, pattern_source: String },
    FeedbackReceived { feedback_type: FeedbackType, value: f32 },
    ErrorRecorded { error_type: String, context_hash: u64 },
    PatternExtracted { pattern_id: PatternId, frequency: u64 },
    KnowledgeFused { sources: Vec<KnowledgeSource>, conflicts: u32 },
    SkillEvolved { skill_id: SkillId, improvement: f32 },
    KnowledgePruned { node_count: u64, reason: PruneReason },
}
```

---

## 2. 主动学习引擎

### 2.1 好奇心引擎 (Curiosity Engine)

```rust
pub trait CuriosityEngine: Send + Sync {
    fn identify_knowledge_gaps(&self, context: &LearningContext) -> impl Future<Output = Result<Vec<KnowledgeGap>, CuriosityError>> + Send;
    fn propose_exploration_plan(&self, gaps: &[KnowledgeGap]) -> ExplorationPlan;
    fn prioritize_gaps(&self, gaps: &[KnowledgeGap], context: &LearningContext) -> Vec<PrioritizedGap>;
    fn track_gap(&self, gap_id: &GapId) -> Option<GapStatus>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningContext {
    pub current_task: Option<String>, pub recent_interactions: Vec<InteractionRecord>,
    pub user_profile: UserProfile, pub system_capabilities: Vec<Capability>,
    pub time_context: TimeContext, pub resource_budget: ResourceBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub id: GapId, pub topic: Topic, pub description: String, pub gap_type: GapType,
    pub severity: GapSeverity, pub discovered_at: Instant, pub related_gaps: Vec<GapId>,
    pub estimated_effort: Duration, pub potential_impact: ImpactAssessment,
    pub sources_to_explore: Vec<KnowledgeSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic { pub domain: String, pub sub_domain: Option<String>, pub specific_area: String, pub keywords: Vec<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapType { UnknownDomain, ShallowUnderstanding, OutdatedKnowledge, MissingSkill, MissingConnection, ContradictoryKnowledge }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapSeverity { Low, Medium, High, Critical }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment { pub task_relevance: f32, pub frequency: f32, pub breadth: f32, pub urgency: f32, pub overall_score: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationPlan {
    pub gaps: Vec<ExplorationTask>, pub estimated_duration: Duration,
    pub resource_requirements: ResourceEstimate, pub priority_order: Vec<GapId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationTask {
    pub gap_id: GapId, pub steps: Vec<ExplorationStep>,
    pub success_criteria: Vec<ConditionExpr>, pub fallback_plan: Option<ExplorationPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExplorationAction {
    SearchWeb { query: String, sources: Vec<String> },
    SearchLocalDocs { path: PathBuf, pattern: String },
    SearchCodebase { query: String, language: Option<String> },
    QueryKnowledgeGraph { pattern: GraphQueryPattern },
    AskUser { question: String, context: String },
    RunExperiment { hypothesis: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapStatus { Discovered, Exploring { progress: f32 }, Resolved { resolution: GapResolution }, Deferred { reason: String }, Abandoned { reason: String } }
```

### 2.2 知识搜索器 (Knowledge Seeker)

```rust
pub trait KnowledgeSeeker: Send + Sync {
    fn search(&self, query: &KnowledgeQuery) -> impl Future<Output = Result<Vec<KnowledgeCandidate>, SeekerError>> + Send;
    fn cross_validate(&self, knowledge: &KnowledgeCandidate) -> impl Future<Output = Result<ValidationReport, SeekerError>> + Send;
    fn fuse_sources(&self, candidates: &[KnowledgeCandidate]) -> impl Future<Output = Result<FusedKnowledge, SeekerError>> + Send;
    fn assess_source_reliability(&self, source: &KnowledgeSource) -> ReliabilityScore;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQuery {
    pub topic: Topic, pub question: String, pub depth: SearchDepth,
    pub max_results: usize, pub allowed_sources: Vec<KnowledgeSource>,
    pub freshness_requirement: Option<Duration>, pub accuracy_requirement: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchDepth { Shallow, Medium, Deep, Exhaustive }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KnowledgeSource { Web { url: String, domain: String }, LocalDocument { path: PathBuf }, Codebase { repository: String }, KnowledgeGraph { graph_id: String }, UserProvided { user_id: UserId, timestamp: Instant }, AgentExperience { agent_id: AgentId, task_type: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub candidate_id: CandidateId, pub is_valid: bool, pub confidence: f32,
    pub cross_sources: Vec<CrossValidationResult>, pub contradictions: Vec<Contradiction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedKnowledge { pub content: String, pub confidence: f32, pub sources: Vec<KnowledgeSource>, pub contradictions_resolved: u32 }
```

### 2.3 自实验引擎 (Self-Experiment Engine)

```rust
pub trait SelfExperimentEngine: Send + Sync {
    fn design_experiment(&self, hypothesis: &Hypothesis, constraints: &ExperimentConstraints) -> Result<Experiment, ExperimentError>;
    fn execute_experiment(&self, experiment: &Experiment) -> impl Future<Output = Result<ExperimentResult, ExperimentError>> + Send;
    fn analyze_results(&self, experiment: &Experiment, results: &[ExperimentResult]) -> ExperimentAnalysis;
    fn manage_ab_test(&self, test: &AbTest) -> impl Future<Output = Result<AbTestResult, ExperimentError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId, pub statement: String, pub variables: Vec<Variable>,
    pub expected_outcome: String, pub confidence_before: f32, pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: ExperimentId, pub hypothesis: Hypothesis, pub methodology: ExperimentMethodology,
    pub steps: Vec<ExperimentStep>, pub data_collection: DataCollectionPlan,
    pub controls: Vec<ControlGroup>, pub estimated_duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExperimentMethodology {
    A_B_Test { variants: Vec<Variant> },
    ControlledExperiment { control: String, treatment: String },
    ObservationalStudy, Simulation, Benchmark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConstraints {
    pub max_duration: Duration, pub max_resource_usage: ResourceEstimate,
    pub safety_limits: SafetyLimits, pub sandbox_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentAnalysis {
    pub hypothesis_supported: bool, pub confidence_after: f32, pub confidence_change: f32,
    pub statistical_significance: f32, pub effect_size: f32,
    pub recommendations: Vec<String>, pub knowledge_to_update: Vec<KnowledgeUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbTest {
    pub id: AbTestId, pub name: String, pub control: Variant, pub treatments: Vec<Variant>,
    pub metric: String, pub min_sample_size: usize, pub significance_level: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbTestResult {
    pub test_id: AbTestId, pub winner: Option<String>,
    pub results: HashMap<String, VariantResult>, pub confidence: f32,
}
```

### 2.4 技能创建器 (Skill Creator)

```rust
pub trait SkillCreator: Send + Sync {
    fn analyze_patterns(&self, patterns: &[ExtractedPattern]) -> impl Future<Output = Result<Vec<SkillProposal>, SkillError>> + Send;
    fn create_skill(&self, proposal: &SkillProposal) -> impl Future<Output = Result<Skill, SkillError>> + Send;
    fn validate_skill(&self, skill: &Skill) -> Result<ValidationReport, SkillError>;
    fn publish_skill(&self, skill: &Skill) -> Result<SkillId, SkillError>;
    fn optimize_skill(&self, skill_id: &SkillId, performance_data: &[SkillExecution]) -> impl Future<Output = Result<Skill, SkillError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedPattern {
    pub id: PatternId, pub description: String, pub frequency: u64,
    pub actions: Vec<ActionDescriptor>, pub context: PatternContext,
    pub variability: f32, pub success_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId, pub name: String, pub version: semver::Version,
    pub actions: Vec<ActionDescriptor>, pub parameters: Vec<SkillParameter>,
    pub preconditions: Vec<ConditionExpr>, pub postconditions: Vec<Postcondition>,
    pub trigger_conditions: Vec<ConditionExpr>, pub performance_history: SkillPerformanceHistory,
    pub created_from: Option<PatternId>, pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String, pub param_type: ValueType, pub required: bool,
    pub default_value: Option<Value>, pub learned_default: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPerformanceHistory {
    pub total_executions: u64, pub success_count: u64, pub avg_duration: Duration,
    pub user_satisfaction: f32, pub trend: PerformanceTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceTrend { Improving, Stable, Declining, InsufficientData }
```

### 2.5 主动提问 (Active Questioning)

```rust
pub trait ActiveQuestioningEngine: Send + Sync {
    fn should_ask(&self, context: &QuestioningContext) -> QuestioningDecision;
    fn formulate_question(&self, gap: &KnowledgeGap, context: &QuestioningContext) -> Question;
    fn process_answer(&self, question_id: &QuestionId, answer: &UserAnswer) -> impl Future<Output = Result<AnswerProcessingResult, QuestioningError>> + Send;
    fn get_pending_questions(&self) -> Vec<Question>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestioningContext {
    pub current_task: Option<String>, pub knowledge_confidence: f32,
    pub user_availability: UserAvailability, pub question_budget: QuestionBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestioningDecision {
    Ask { urgency: QuestionUrgency, reason: String },
    Defer { reason: String, retry_after: Duration },
    Skip { reason: String },
    ResearchIndependently { plan: ExplorationPlan },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: QuestionId, pub gap_id: GapId, pub question_text: String,
    pub question_type: QuestionType, pub suggested_answers: Vec<SuggestedAnswer>,
    pub urgency: QuestionUrgency, pub expires_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestionType { YesNo, MultipleChoice { options: Vec<String> }, Rating { min: i32, max: i32 }, OpenEnded, Confirmation }
```

### 2.6 主动学习测试用例

```rust
#[cfg(test)]
mod active_tests {
    #[tokio::test]
    async fn test_identify_gaps() {
        let engine = create_test_curiosity();
        let gaps = engine.identify_knowledge_gaps(&LearningContext { current_task: Some("部署微服务".into()), ..Default::default() }).await.unwrap();
        assert!(!gaps.is_empty());
    }

    #[test]
    fn test_propose_plan() {
        let plan = create_test_curiosity().propose_exploration_plan(&vec![create_test_gap("K8s", GapType::ShallowUnderstanding)]);
        assert!(!plan.gaps.is_empty());
    }

    #[tokio::test]
    async fn test_knowledge_search() {
        let seeker = create_test_seeker();
        let candidates = seeker.search(&KnowledgeQuery { topic: Topic { domain: "Rust".into(), specific_area: "async".into(), ..Default::default() }, ..Default::default() }).await.unwrap();
        assert!(!candidates.is_empty());
    }

    #[tokio::test]
    async fn test_experiment() {
        let engine = create_test_experiment();
        let experiment = engine.design_experiment(&Hypothesis { statement: "并行处理可减少50%时间".into(), ..Default::default() }, &ExperimentConstraints { sandbox_required: true, ..Default::default() }).unwrap();
        let result = engine.execute_experiment(&experiment).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_skill_creation() {
        let creator = create_test_skill_creator();
        let pattern = ExtractedPattern { frequency: 15, success_rate: 0.95, actions: vec![make_action("git_pull"), make_action("run_tests")], ..Default::default() };
        let proposals = creator.analyze_patterns(&[pattern]).await.unwrap();
        assert!(!proposals.is_empty());
    }

    #[test]
    fn test_questioning_decision() {
        let decision = create_test_questioning().should_ask(&QuestioningContext { knowledge_confidence: 0.3, user_availability: UserAvailability { is_available: true, ..Default::default() }, question_budget: QuestionBudget { max_per_hour: 5, used_this_hour: 2, ..Default::default() }, ..Default::default() });
        assert!(matches!(decision, QuestioningDecision::Ask { .. }));
    }
}
```

---

## 3. 被动学习引擎

### 3.1 行为观察器 (Behavior Observer)

```rust
pub trait BehaviorObserver: Send + Sync {
    fn record_interaction(&self, interaction: &InteractionRecord);
    fn extract_patterns(&self, filter: &PatternFilter) -> impl Future<Output = Result<Vec<ExtractedPattern>, ObserverError>> + Send;
    fn get_statistics(&self, time_range: &TimeRange) -> BehaviorStatistics;
    fn detect_anomalies(&self, recent: &[InteractionRecord]) -> Vec<BehaviorAnomaly>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionRecord {
    pub id: InteractionId, pub timestamp: Instant, pub actor: Actor,
    pub action: ActionDescriptor, pub inputs: HashMap<String, Value>,
    pub outputs: Option<Value>, pub context: InteractionContext,
    pub outcome: InteractionOutcome, pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Actor { User { user_id: UserId }, Agent { agent_id: AgentId }, System }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteractionOutcome { Success, Failure { reason: String }, Partial { completed: f32 }, Cancelled, TimedOut }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorStatistics {
    pub total_interactions: u64, pub success_rate: f64, pub avg_duration: Duration,
    pub most_common_actions: Vec<(String, u64)>, pub peak_activity_hours: Vec<u32>,
}
```

### 3.2 反馈吸收器 (Feedback Absorber)

```rust
pub trait FeedbackAbsorber: Send + Sync {
    fn absorb(&self, feedback: &UserFeedback) -> impl Future<Output = Result<FeedbackResult, FeedbackError>> + Send;
    fn absorb_batch(&self, feedbacks: &[UserFeedback]) -> impl Future<Output = Result<Vec<FeedbackResult>, FeedbackError>> + Send;
    fn get_feedback_history(&self, filter: &FeedbackFilter) -> Vec<UserFeedback>;
    fn compute_impact(&self, feedback: &UserFeedback) -> FeedbackImpact;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedback {
    pub id: FeedbackId, pub timestamp: Instant, pub target: FeedbackTarget,
    pub feedback_type: FeedbackType, pub value: f32, pub comment: Option<String>,
    pub context: FeedbackContext, pub user_id: UserId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackTarget { Skill { skill_id: SkillId }, Action { action_id: String }, Response { message_id: MessageId }, Workflow { workflow_id: WorkflowId }, SystemBehavior { aspect: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackType { ThumbsUp, ThumbsDown, Rating { min: i32, max: i32 }, Correction { corrected_value: Value }, Preference { preferred: String, alternatives: Vec<String> }, ExplicitInstruction { instruction: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResult {
    pub feedback_id: FeedbackId, pub absorbed: bool,
    pub weight_adjustment: WeightAdjustment, pub behavior_changes: Vec<BehaviorChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorChange { pub component: String, pub change_type: ChangeType, pub description: String, pub confidence: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType { ParameterAdjustment, StrategySwitch, PriorityChange, FeatureToggle, ThresholdAdjustment }
```

### 3.3 错误学习器 (Error Learner)

```rust
pub trait ErrorLearner: Send + Sync {
    fn record_error(&self, error: &ErrorRecord);
    fn analyze_error_patterns(&self, filter: &ErrorFilter) -> impl Future<Output = Result<Vec<ErrorPattern>, ErrorLearnError>> + Send;
    fn generate_prevention_rules(&self, patterns: &[ErrorPattern]) -> Vec<PreventionRule>;
    fn apply_rule(&self, rule: &PreventionRule) -> Result<RuleApplication, ErrorLearnError>;
    fn evaluate_rule(&self, rule_id: &RuleId) -> Option<RuleEffectiveness>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub id: ErrorId, pub timestamp: Instant, pub error_type: String,
    pub error_message: String, pub context: ErrorContext, pub severity: ErrorSeverity,
    pub recoverable: bool, pub impact: ErrorImpact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity { Warning, Minor, Major, Critical }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreventionRule {
    pub id: RuleId, pub name: String, pub trigger_condition: ConditionExpr,
    pub prevention_action: PreventionAction, pub auto_apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreventionAction {
    PreCheck { checks: Vec<PreCheck> }, InputValidation { rules: Vec<ValidationRule> },
    FallbackActivation { fallback: ActionDescriptor }, RateLimit { max_per_minute: u32 },
    CircuitBreaker { threshold: u32, reset_timeout: Duration },
}
```

### 3.4 模式提取器 (Pattern Extractor)

```rust
pub trait PatternExtractor: Send + Sync {
    fn extract_sequence_patterns(&self, records: &[InteractionRecord]) -> impl Future<Output = Result<Vec<SequencePattern>, PatternError>> + Send;
    fn extract_association_rules(&self, records: &[InteractionRecord]) -> impl Future<Output = Result<Vec<AssociationRule>, PatternError>> + Send;
    fn extract_temporal_patterns(&self, records: &[InteractionRecord]) -> impl Future<Output = Result<Vec<TemporalPattern>, PatternError>> + Send;
    fn extract_contextual_patterns(&self, records: &[InteractionRecord]) -> impl Future<Output = Result<Vec<ContextualPattern>, PatternError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencePattern {
    pub id: PatternId, pub sequence: Vec<ActionDescriptor>, pub frequency: u64,
    pub support: f32, pub confidence: f32, pub success_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociationRule {
    pub id: PatternId, pub antecedent: Vec<String>, pub consequent: Vec<String>,
    pub support: f32, pub confidence: f32, pub lift: f32,
}
```

### 3.5 偏好学习器 (Preference Learner)

```rust
pub trait PreferenceLearner: Send + Sync {
    fn learn_preference(&self, signal: &PreferenceSignal) -> impl Future<Output = Result<LearnedPreference, PreferenceError>> + Send;
    fn get_preferences(&self, user_id: &UserId) -> PreferenceModel;
    fn predict_preference(&self, user_id: &UserId, context: &PreferenceContext) -> Option<PreferencePrediction>;
    fn update_weights(&self, feedback: &UserFeedback) -> Result<(), PreferenceError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreferenceSignalType {
    ExplicitChoice { chosen: String, alternatives: Vec<String> },
    ImplicitBehavior { action: String, frequency: u32 },
    Correction { original: Value, corrected: Value },
    TimingPreference { time_range: TimeRange },
    FormatPreference { format: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub user_id: UserId, pub interaction_style: InteractionStyle,
    pub output_format: OutputFormatPreference, pub work_habits: WorkHabits,
    pub tool_preferences: HashMap<String, String>, pub communication_preferences: CommunicationPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionStyle { pub verbosity: VerbosityLevel, pub formality: FormalityLevel, pub proactivity: ProactivityLevel, pub detail_level: DetailLevel }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerbosityLevel { Concise, Normal, Detailed }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FormalityLevel { Casual, Normal, Formal }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProactivityLevel { Reactive, Suggestive, Proactive }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkHabits { pub active_hours: Vec<TimeRange>, pub typical_tasks: Vec<String>, pub preferred_tools: Vec<String>, pub work_rhythm: WorkRhythm }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkRhythm { MorningPerson, NightOwl, Flexible, BurstWorker }
```

### 3.6 被动学习测试用例

```rust
#[cfg(test)]
mod passive_tests {
    #[test]
    fn test_record_and_extract() {
        let observer = create_test_observer();
        let records = create_test_interactions(50);
        for r in &records { observer.record_interaction(r); }
        let patterns = tokio_test::block_on(observer.extract_patterns(&PatternFilter { min_frequency: 3, ..Default::default() })).unwrap();
        assert!(!patterns.is_empty());
    }

    #[tokio::test]
    async fn test_feedback_absorption() {
        let absorber = create_test_absorber();
        let result = absorber.absorb(&UserFeedback { feedback_type: FeedbackType::ThumbsDown, target: FeedbackTarget::Skill { skill_id: SkillId::new("test") }, ..Default::default() }).await.unwrap();
        assert!(result.absorbed);
    }

    #[tokio::test]
    async fn test_error_learning() {
        let learner = create_test_error_learner();
        for e in create_test_errors(30) { learner.record_error(&e); }
        let patterns = learner.analyze_error_patterns(&ErrorFilter::default()).await.unwrap();
        let rules = learner.generate_prevention_rules(&patterns);
        assert!(!rules.is_empty());
    }

    #[tokio::test]
    async fn test_preference_learning() {
        let learner = create_test_preference_learner();
        learner.learn_preference(&PreferenceSignal { signal_type: PreferenceSignalType::FormatPreference { format: "markdown".into() }, ..Default::default() }).await.unwrap();
        assert_eq!(learner.get_preferences(&UserId::new("u1")).output_format.default_format, "markdown");
    }
}
```

---

## 4. 知识演化系统

### 4.1 知识图谱 (Knowledge Graph)

```rust
pub trait KnowledgeGraph: Send + Sync {
    fn add_node(&self, node: KnowledgeNode) -> Result<NodeId, GraphError>;
    fn add_edge(&self, edge: KnowledgeEdge) -> Result<(), GraphError>;
    fn query(&self, pattern: &GraphQueryPattern) -> Result<SubGraph, GraphError>;
    fn update(&self, updates: Vec<GraphUpdate>) -> Result<UpdateResult, GraphError>;
    fn get_node(&self, node_id: &NodeId) -> Option<KnowledgeNode>;
    fn get_relations(&self, node_id: &NodeId, depth: u32) -> Vec<KnowledgeEdge>;
    fn search(&self, query: &str, limit: usize) -> Vec<SearchResult>;
    fn stats(&self) -> GraphStats;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub id: NodeId, pub node_type: NodeType, pub label: String,
    pub properties: HashMap<String, Value>, pub embedding: Option<Vec<f32>>,
    pub source: KnowledgeSource, pub confidence: f32, pub version: u64,
    pub access_count: u64, pub created_at: Instant, pub updated_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType { Concept, Entity, Skill, Rule, Fact, Procedure, Event, Relation }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub id: EdgeId, pub source: NodeId, pub target: NodeId,
    pub relation_type: RelationType, pub weight: f32, pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationType { IsA, PartOf, DependsOn, RelatedTo, Causes, CausedBy, SimilarTo, Contradicts, Supports, Refines, Precedes, Custom(String) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQueryPattern {
    pub start_node: Option<NodeFilter>, pub edge_filter: Option<EdgeFilter>,
    pub end_node: Option<NodeFilter>, pub max_depth: u32, pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraph { pub nodes: Vec<KnowledgeNode>, pub edges: Vec<KnowledgeEdge>, pub total_matching: usize }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphOperation { AddNode, UpdateNode, RemoveNode, AddEdge, UpdateEdge, RemoveEdge, MergeNodes { source: NodeId, target: NodeId } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats { pub total_nodes: u64, pub total_edges: u64, pub nodes_by_type: HashMap<NodeType, u64>, pub avg_node_confidence: f32, pub storage_size_bytes: u64 }
```

### 4.2 知识融合 (Knowledge Fusion)

```rust
pub trait KnowledgeFusionEngine: Send + Sync {
    fn fuse(&self, sources: &[KnowledgeSource], strategy: &FusionStrategy) -> impl Future<Output = Result<FusionResult, FusionError>> + Send;
    fn detect_conflicts(&self, knowledge: &[KnowledgeNode]) -> Vec<KnowledgeConflict>;
    fn resolve_conflict(&self, conflict: &KnowledgeConflict, strategy: &ResolutionStrategy) -> impl Future<Output = Result<ConflictResolution, FusionError>> + Send;
    fn deduplicate(&self, nodes: &[KnowledgeNode]) -> Vec<DeduplicationResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionStrategy {
    pub conflict_resolution: ResolutionStrategy, pub confidence_aggregation: AggregationMethod,
    pub source_priority: Vec<KnowledgeSource>, pub similarity_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionStrategy { HighestConfidence, Merge, MostRecent, MostTrustedSource, HumanArbitration, PendingResolution }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationMethod { Average, Weighted, Max, Min, Bayesian }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeConflict { pub id: ConflictId, pub conflict_type: ConflictType, pub nodes: Vec<KnowledgeNode>, pub severity: ConflictSeverity }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType { Contradiction, Overlap, Outdated, SourceDisagreement, SemanticDuplicate }
```

### 4.3 技能进化器 (Skill Evolver)

```rust
pub trait SkillEvolver: Send + Sync {
    fn optimize_parameters(&self, skill_id: &SkillId, data: &[SkillExecution]) -> impl Future<Output = Result<Skill, EvolveError>> + Send;
    fn combine_mutate(&self, skills: &[SkillId]) -> impl Future<Output = Result<Vec<Skill>, EvolveError>> + Send;
    fn genetic_select(&self, population: &[Skill], fitness_fn: &FitnessFunction) -> impl Future<Output = Result<GeneticResult, EvolveError>> + Send;
    fn evaluate_fitness(&self, skill: &Skill) -> FitnessScore;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessFunction { pub weights: FitnessWeights, pub constraints: Vec<Constraint> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessWeights { pub success_rate: f32, pub speed: f32, pub user_satisfaction: f32, pub resource_efficiency: f32, pub generalization: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneticResult { pub selected: Vec<Skill>, pub offspring: Vec<Skill>, pub generation: u32, pub best_fitness: f32, pub improvement: f32 }
```

### 4.4 策略优化器 (Strategy Optimizer)

```rust
pub trait StrategyOptimizer: Send + Sync {
    fn optimize_strategy(&self, strategy: &ExecutionStrategy, metrics: &[ExecutionMetric]) -> impl Future<Output = Result<OptimizedStrategy, OptimizeError>> + Send;
    fn suggest_improvements(&self, strategy: &ExecutionStrategy) -> Vec<StrategySuggestion>;
    fn compare_strategies(&self, strategies: &[ExecutionStrategy]) -> impl Future<Output = Result<StrategyComparison, OptimizeError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStrategy { pub id: StrategyId, pub name: String, pub steps: Vec<StrategyStep>, pub resource_allocation: ResourceAllocation, pub error_handling: ErrorHandlingConfig }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedStrategy { pub strategy: ExecutionStrategy, pub improvements: Vec<StrategySuggestion>, pub estimated_improvement: f32, pub confidence: f32 }
```

### 4.5 迁移学习器 (Transfer Learner)

```rust
pub trait TransferLearner: Send + Sync {
    fn discover_transferable(&self, source_domain: &str, target_domain: &str) -> impl Future<Output = Result<Vec<TransferCandidate>, TransferError>> + Send;
    fn transfer(&self, candidate: &TransferCandidate) -> impl Future<Output = Result<TransferResult, TransferError>> + Send;
    fn evaluate_transfer(&self, transfer_id: &TransferId) -> Option<TransferEvaluation>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferType { DirectCopy, Analogy, Abstraction, Decomposition, Recombination }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult { pub transfer_id: TransferId, pub transferred_nodes: Vec<KnowledgeNode>, pub adapted_nodes: Vec<KnowledgeNode>, pub success: bool, pub validation_score: f32 }
```

### 4.6 遗忘管理器 (Forgetting Manager)

```rust
pub trait ForgettingManager: Send + Sync {
    fn compute_retention_score(&self, node: &KnowledgeNode) -> RetentionScore;
    fn prune(&self, policy: &PruningPolicy) -> impl Future<Output = Result<PruningResult, ForgetError>> + Send;
    fn archive(&self, node_ids: &[NodeId]) -> Result<u32, ForgetError>;
    fn get_forgetting_stats(&self) -> ForgettingStats;
    fn restore(&self, node_id: &NodeId) -> Result<KnowledgeNode, ForgetError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionScore {
    pub node_id: NodeId, pub score: f32, pub factors: RetentionFactors,
    pub recommendation: RetentionRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionFactors { pub recency: f32, pub frequency: f32, pub importance: f32, pub confidence: f32, pub uniqueness: f32, pub domain_relevance: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetentionRecommendation { Keep, Archive, Prune, Consolidate }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningPolicy {
    pub max_nodes: Option<u64>, pub min_retention_score: f32,
    pub archive_before_prune: bool, pub protected_domains: Vec<String>,
    pub protected_node_types: Vec<NodeType>, pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningResult {
    pub nodes_pruned: u64, pub nodes_archived: u64, pub edges_removed: u64,
    pub storage_freed: u64, pub retention_rate: f32,
}
```

### 4.7 知识演化测试用例

```rust
#[cfg(test)]
mod evolution_tests {
    #[tokio::test]
    async fn test_graph_crud() {
        let graph = create_test_graph();
        let id = graph.add_node(KnowledgeNode { node_type: NodeType::Concept, label: "Rust 所有权".into(), confidence: 0.95, ..Default::default() }).unwrap();
        assert_eq!(graph.get_node(&id).unwrap().label, "Rust 所有权");
        assert_eq!(graph.stats().total_nodes, 1);
    }

    #[tokio::test]
    async fn test_graph_query() {
        let graph = create_populated_graph();
        let sub = graph.query(&GraphQueryPattern { start_node: Some(NodeFilter { label_pattern: Some("Rust".into()), ..Default::default() }), max_depth: 2, ..Default::default() }).unwrap();
        assert!(!sub.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_fusion_conflict() {
        let fusion = create_test_fusion();
        let conflicts = fusion.detect_conflicts(&vec![
            KnowledgeNode { label: "用 Alpine".into(), confidence: 0.9, ..Default::default() },
            KnowledgeNode { label: "用 Ubuntu".into(), confidence: 0.85, ..Default::default() },
        ]);
        assert!(!conflicts.is_empty());
    }

    #[tokio::test]
    async fn test_skill_evolution() {
        let evolver = create_test_evolver();
        let optimized = evolver.optimize_parameters(&test_skill_id(), &create_executions(50)).await.unwrap();
        assert!(optimized.version > semver::Version::new(0, 1, 0));
    }

    #[tokio::test]
    async fn test_forgetting() {
        let mgr = create_test_forgetting();
        let result = mgr.prune(&PruningPolicy { min_retention_score: 0.5, archive_before_prune: true, dry_run: false, ..Default::default() }).await;
        assert!(result.nodes_archived > 0 || result.nodes_pruned > 0);
    }

    #[tokio::test]
    async fn test_strategy_optimization() {
        let optimizer = create_test_optimizer();
        let optimized = optimizer.optimize_strategy(&create_test_strategy(), &create_metrics(100)).await.unwrap();
        assert!(!optimized.improvements.is_empty());
    }
}
```

---

## 5. 安全设计

| 安全维度 | 措施 |
|----------|------|
| 知识完整性 | 所有知识更新附带来源和置信度，支持审计追溯 |
| 学习边界 | 用户可配置学习范围，禁止学习敏感操作 |
| 隐私保护 | 偏好学习数据本地存储，不外传 |
| 实验安全 | 自实验在沙箱中执行，设置安全限制 |
| 知识污染防护 | 多源交叉验证，低置信度知识标记为"待验证" |
| 遗忘安全 | 受保护领域知识不可自动修剪，修剪操作可撤销 |

## 6. 配置参考

```toml
[learning.active]
curiosity_scan_interval = "1h"; experiment_sandbox_enabled = true
skill_auto_creation_threshold = 10; question_budget_per_hour = 5

[learning.passive]
observation_buffer_size = 10000; pattern_min_frequency = 3; pattern_min_confidence = 0.7
feedback_integration_timeout = "1s"; preference_decay_rate = 0.01

[learning.evolution]
graph_storage_backend = "sqlite"; graph_max_nodes = 1000000
fusion_conflict_strategy = "highest_confidence"; pruning_interval = "24h"
pruning_min_retention_score = 0.3; protected_domains = ["security", "authentication"]
genetic_population_size = 50; genetic_mutation_rate = 0.1; transfer_min_similarity = 0.6
```

## 附录 A：学习状态机

```
┌───────────┐  observe  ┌───────────┐  extract  ┌──────────────┐
│ OBSERVING │──────────▶│ ANALYZING │──────────▶│PATTERN_FOUND │
└───────────┘           └───────────┘           └──────┬───────┘
                                                       │
                                          ┌────────────┤
                                          ▼            ▼
                                   ┌──────────┐ ┌──────────┐
                                   │ SKILL_CR │ │ RULE_CR  │
                                   └────┬─────┘ └────┬─────┘
                                        │            │
                                   pass │       pass │
                                        ▼            ▼
                                   ┌───────────┐ ┌──────────┐
                                   │ VALIDATING│ │ TESTING  │
                                   └─────┬─────┘ └────┬─────┘
                                        ▼            ▼
                                   ┌───────────┐ ┌──────────┐
                                   │  ACTIVE   │ │  ACTIVE  │
                                   └─────┬─────┘ └────┬─────┘
                                   decay │            │
                                         ▼            ▼
                                   ┌───────────┐ ┌──────────┐
                                   │ ARCHIVING │ │ ARCHIVING│
                                   └───────────┘ └──────────┘
                                         │
                                   prune │
                                         ▼
                                   ┌───────────┐
                                   │  PRUNED   │
                                   └───────────┘
```

## 附录 B：性能基准

| 组件 | 操作 | 目标延迟 | 数据规模 |
|------|------|----------|----------|
| 好奇心引擎 | identify_knowledge_gaps() | < 3s | 100 万节点 |
| 知识搜索器 | search() | < 2s | 5 个来源 |
| 自实验引擎 | execute_experiment() | 视实验 | 沙箱限制 |
| 技能创建器 | analyze_patterns() | < 10s | 1000 条记录 |
| 行为观察器 | extract_patterns() | < 5s | 1000 条记录 |
| 反馈吸收器 | absorb() | < 1s | 单条反馈 |
| 错误学习器 | analyze_error_patterns() | < 5s | 100 条错误 |
| 偏好学习器 | learn_preference() | < 1s | 单条信号 |
| 知识图谱 | query() | < 10ms | 100 万节点 |
| 知识融合 | fuse() | < 2s | 10 个来源 |
| 技能进化器 | optimize_parameters() | < 30s | 50 次执行 |
| 策略优化器 | optimize_strategy() | < 10s | 100 次执行 |
| 遗忘管理器 | prune() | < 5s | 100 万节点 |
