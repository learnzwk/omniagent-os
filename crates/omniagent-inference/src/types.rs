// OmniAgent OS Phase 10: AI 模型集成框架
// 核心类型定义模块

use std::collections::HashMap;

/// 模型唯一标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(pub String);

impl ModelId {
    /// 创建新的模型 ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 获取模型 ID 的字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 模型可用性状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelAvailability {
    /// 模型可用，随时可推理
    Available = 0,
    /// 模型正在加载中
    Loading = 1,
    /// 模型不可用
    Unavailable = 2,
    /// 模型处于错误状态
    Error = 3,
}

impl Default for ModelAvailability {
    fn default() -> Self {
        Self::Unavailable
    }
}

/// 推理偏好设置，控制路由决策策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InferencePreference {
    /// 自动选择最优路径
    Auto = 0,
    /// 仅使用本地模型
    LocalOnly = 1,
    /// 仅使用云端 API
    CloudOnly = 2,
    /// 延迟优先，选择最快的推理路径
    LatencyFirst = 3,
    /// 精度优先，选择效果最好的模型
    AccuracyFirst = 4,
    /// 隐私优先，尽量使用本地模型保护数据
    PrivacyFirst = 5,
}

impl Default for InferencePreference {
    fn default() -> Self {
        Self::Auto
    }
}

/// 隐私级别，决定数据可以发送到哪些地方
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PrivacyLevel {
    /// 公开数据，无限制
    Public = 0,
    /// 内部数据，仅限内网
    Internal = 1,
    /// 敏感数据，优先本地处理
    Sensitive = 2,
    /// 机密数据，必须本地处理
    Confidential = 3,
}

impl Default for PrivacyLevel {
    fn default() -> Self {
        Self::Public
    }
}

/// 推理任务类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceTask {
    /// 文本生成
    TextGeneration,
    /// 文本嵌入
    TextEmbedding,
    /// 图像生成
    ImageGeneration,
    /// 图像理解
    ImageUnderstanding,
    /// 语音识别
    SpeechRecognition,
    /// 语音合成
    SpeechSynthesis,
    /// 翻译任务
    Translation { from: String, to: String },
    /// 文本摘要
    Summarization,
    /// 代码生成
    CodeGeneration,
    /// 分类任务
    Classification,
}

/// 推理输入数据
#[derive(Debug, Clone)]
pub enum InferenceInput {
    /// 纯文本输入
    Text(String),
    /// 带上下文的文本输入
    TextWithContext { text: String, context: String },
    /// 图像输入
    Image { data: Vec<u8>, format: String },
    /// 音频输入
    Audio { data: Vec<u8>, format: String, sample_rate: u32 },
    /// 多模态输入，键值对列表
    MultiModal { parts: Vec<(String, Vec<u8>)> },
}

/// 模型请求，包含推理所需的所有信息
#[derive(Debug, Clone)]
pub struct ModelRequest {
    /// 推理任务类型
    pub task: InferenceTask,
    /// 推理输入数据
    pub input: InferenceInput,
    /// 推理偏好
    pub preference: InferencePreference,
    /// 最大允许延迟（毫秒）
    pub max_latency_ms: Option<u32>,
    /// 隐私级别
    pub privacy_level: PrivacyLevel,
    /// Token 预算限制
    pub budget_tokens: Option<u32>,
}

impl ModelRequest {
    /// 创建新的模型请求
    pub fn new(task: InferenceTask, input: InferenceInput) -> Self {
        Self {
            task,
            input,
            preference: InferencePreference::default(),
            max_latency_ms: None,
            privacy_level: PrivacyLevel::default(),
            budget_tokens: None,
        }
    }

    /// 设置推理偏好
    pub fn with_preference(mut self, preference: InferencePreference) -> Self {
        self.preference = preference;
        self
    }

    /// 设置最大延迟
    pub fn with_max_latency(mut self, ms: u32) -> Self {
        self.max_latency_ms = Some(ms);
        self
    }

    /// 设置隐私级别
    pub fn with_privacy_level(mut self, level: PrivacyLevel) -> Self {
        self.privacy_level = level;
        self
    }

    /// 设置 Token 预算
    pub fn with_budget_tokens(mut self, tokens: u32) -> Self {
        self.budget_tokens = Some(tokens);
        self
    }
}

/// 本地推理后端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LocalBackend {
    /// HuggingFace Candle 框架
    Candle = 0,
    /// ONNX Runtime
    OnnxRuntime = 1,
    /// Tract 推理框架
    Tract = 2,
}

/// 云端服务提供商
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CloudProvider {
    /// OpenAI (GPT 系列)
    OpenAI = 0,
    /// Anthropic (Claude 系列)
    Anthropic = 1,
    /// Google (Gemini 系列)
    Google = 2,
    /// Mistral AI
    Mistral = 3,
    /// Azure OpenAI 服务
    Azure = 4,
    /// 自定义提供商
    Custom = 5,
}

/// 路由决策结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// 使用本地 Candle 后端
    LocalCandle { model_id: ModelId },
    /// 使用本地 ONNX Runtime 后端
    LocalOnnx { model_id: ModelId },
    /// 使用本地 Tract 后端
    LocalTract { model_id: ModelId },
    /// 使用云端 API
    Cloud { provider: CloudProvider, model_id: ModelId },
    /// 带回退的决策
    Fallback {
        primary: Box<RoutingDecision>,
        fallback: Box<RoutingDecision>,
    },
}

/// 本地模型信息
#[derive(Debug, Clone)]
pub struct LocalModelInfo {
    /// 模型 ID
    pub model_id: ModelId,
    /// 推理后端
    pub backend: LocalBackend,
    /// 模型文件路径
    pub model_path: String,
    /// 内存占用（字节）
    pub memory_bytes: usize,
    /// 支持的任务类型列表
    pub supported_tasks: Vec<InferenceTask>,
    /// 平均推理延迟（毫秒）
    pub avg_latency_ms: f32,
    /// 当前可用性
    pub availability: ModelAvailability,
}

impl LocalModelInfo {
    /// 创建新的本地模型信息
    pub fn new(model_id: ModelId, backend: LocalBackend, model_path: String) -> Self {
        Self {
            model_id,
            backend,
            model_path,
            memory_bytes: 0,
            supported_tasks: Vec::new(),
            avg_latency_ms: 0.0,
            availability: ModelAvailability::Unavailable,
        }
    }

    /// 设置支持的任务类型
    pub fn with_tasks(mut self, tasks: Vec<InferenceTask>) -> Self {
        self.supported_tasks = tasks;
        self
    }

    /// 设置内存占用
    pub fn with_memory(mut self, bytes: usize) -> Self {
        self.memory_bytes = bytes;
        self
    }

    /// 设置平均延迟
    pub fn with_latency(mut self, ms: f32) -> Self {
        self.avg_latency_ms = ms;
        self
    }

    /// 设置可用性
    pub fn with_availability(mut self, avail: ModelAvailability) -> Self {
        self.availability = avail;
        self
    }

    /// 检查是否支持指定任务
    pub fn supports_task(&self, task: &InferenceTask) -> bool {
        self.supported_tasks.iter().any(|t| t == task)
    }

    /// 检查是否可用
    pub fn is_available(&self) -> bool {
        self.availability == ModelAvailability::Available
    }
}

/// 云端提供商信息
#[derive(Debug, Clone)]
pub struct CloudProviderInfo {
    /// 提供商标识
    pub provider: CloudProvider,
    /// API 端点地址
    pub api_endpoint: String,
    /// API 密钥名称（从安全存储获取实际密钥）
    pub api_key_name: String,
    /// 可用模型列表
    pub available_models: Vec<ModelId>,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f32,
    /// 是否已配置
    pub is_configured: bool,
}

impl CloudProviderInfo {
    /// 创建新的云端提供商信息
    pub fn new(provider: CloudProvider, api_endpoint: String, api_key_name: String) -> Self {
        Self {
            provider,
            api_endpoint,
            api_key_name,
            available_models: Vec::new(),
            avg_latency_ms: 0.0,
            is_configured: false,
        }
    }

    /// 设置可用模型
    pub fn with_models(mut self, models: Vec<ModelId>) -> Self {
        self.available_models = models;
        self
    }

    /// 设置平均延迟
    pub fn with_latency(mut self, ms: f32) -> Self {
        self.avg_latency_ms = ms;
        self
    }

    /// 标记为已配置
    pub fn configured(mut self) -> Self {
        self.is_configured = true;
        self
    }

    /// 检查是否有指定模型
    pub fn has_model(&self, model_id: &ModelId) -> bool {
        self.available_models.iter().any(|m| m == model_id)
    }
}

/// 推理结果
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// 推理输出
    pub output: InferenceOutput,
    /// 使用的模型 ID
    pub model_id: ModelId,
    /// 推理提供者
    pub provider: InferenceProvider,
    /// 推理延迟（毫秒）
    pub latency_ms: u32,
    /// Token 使用统计
    pub tokens_used: Option<TokenUsage>,
    /// 附加元数据
    pub metadata: HashMap<String, String>,
}

impl InferenceResult {
    /// 创建新的推理结果
    pub fn new(output: InferenceOutput, model_id: ModelId, provider: InferenceProvider) -> Self {
        Self {
            output,
            model_id,
            provider,
            latency_ms: 0,
            tokens_used: None,
            metadata: HashMap::new(),
        }
    }

    /// 设置延迟
    pub fn with_latency(mut self, ms: u32) -> Self {
        self.latency_ms = ms;
        self
    }

    /// 设置 Token 使用
    pub fn with_tokens(mut self, usage: TokenUsage) -> Self {
        self.tokens_used = Some(usage);
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// 推理输出类型
#[derive(Debug, Clone, PartialEq)]
pub enum InferenceOutput {
    /// 文本输出
    Text(String),
    /// 向量嵌入
    Embedding(Vec<f32>),
    /// 图像输出
    Image { data: Vec<u8>, format: String, width: u32, height: u32 },
    /// 音频输出
    Audio { data: Vec<u8>, format: String },
    /// 分类结果
    Classification { label: String, confidence: f32, alternatives: Vec<(String, f32)> },
    /// 流式响应句柄
    StreamHandle(u64),
}

/// 推理提供者标识
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InferenceProvider {
    /// Candle 本地推理
    Candle = 0,
    /// ONNX Runtime 本地推理
    OnnxRuntime = 1,
    /// Tract 本地推理
    Tract = 2,
    /// OpenAI 云端 API
    OpenAI = 3,
    /// Anthropic 云端 API
    Anthropic = 4,
    /// Google 云端 API
    Google = 5,
}

/// Token 使用统计
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    /// 提示 Token 数
    pub prompt_tokens: u32,
    /// 生成 Token 数
    pub completion_tokens: u32,
    /// 总 Token 数
    pub total_tokens: u32,
}

impl TokenUsage {
    /// 创建新的 Token 使用统计
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.wrapping_add(completion_tokens),
        }
    }
}

/// 推理错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceError {
    /// 模型未找到
    ModelNotFound(String),
    /// 模型未加载
    ModelNotLoaded(String),
    /// 模型加载失败
    ModelLoadFailed(String),
    /// 推理执行失败
    InferenceFailed(String),
    /// 推理超时
    Timeout(u32),
    /// 无效输入
    InvalidInput(String),
    /// 不支持的任务类型
    UnsupportedTask(InferenceTask),
    /// API 调用错误
    ApiError { provider: CloudProvider, message: String },
    /// 速率限制
    RateLimited { provider: CloudProvider, retry_after_ms: u32 },
    /// 内存不足
    OutOfMemory { required: usize, available: usize },
    /// 上下文过长
    ContextTooLarge { tokens: u32, max_tokens: u32 },
    /// 网络错误
    NetworkError(String),
    /// 认证错误
    AuthenticationError(String),
}

impl std::fmt::Display for InferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelNotFound(id) => write!(f, "模型未找到: {}", id),
            Self::ModelNotLoaded(id) => write!(f, "模型未加载: {}", id),
            Self::ModelLoadFailed(reason) => write!(f, "模型加载失败: {}", reason),
            Self::InferenceFailed(reason) => write!(f, "推理失败: {}", reason),
            Self::Timeout(ms) => write!(f, "推理超时: {}ms", ms),
            Self::InvalidInput(reason) => write!(f, "无效输入: {}", reason),
            Self::UnsupportedTask(task) => write!(f, "不支持的任务类型: {:?}", task),
            Self::ApiError { provider, message } => {
                write!(f, "API 错误 ({:?}): {}", provider, message)
            }
            Self::RateLimited { provider, retry_after_ms } => {
                write!(f, "速率限制 ({:?}), {}ms 后重试", provider, retry_after_ms)
            }
            Self::OutOfMemory { required, available } => {
                write!(f, "内存不足: 需要 {} 字节, 可用 {} 字节", required, available)
            }
            Self::ContextTooLarge { tokens, max_tokens } => {
                write!(f, "上下文过长: {} tokens 超过最大限制 {} tokens", tokens, max_tokens)
            }
            Self::NetworkError(reason) => write!(f, "网络错误: {}", reason),
            Self::AuthenticationError(reason) => write!(f, "认证错误: {}", reason),
        }
    }
}

impl std::error::Error for InferenceError {}

/// 推理统计信息
#[derive(Debug, Clone)]
pub struct InferenceStats {
    /// 总推理次数
    pub total_inferences: u64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// 已注册引擎数量
    pub engines_count: usize,
    /// 本地模型数量
    pub local_models_count: usize,
    /// 云端提供商数量
    pub cloud_providers_count: usize,
}

/// 流式推理块
#[derive(Debug, Clone)]
pub struct StreamChunk {
    /// 块 ID
    pub chunk_id: u64,
    /// 块内容
    pub content: String,
    /// 是否为最后一个块
    pub is_final: bool,
    /// 块延迟（毫秒）
    pub latency_ms: u32,
}

impl StreamChunk {
    /// 创建新的流式块
    pub fn new(chunk_id: u64, content: String) -> Self {
        Self {
            chunk_id,
            content,
            is_final: false,
            latency_ms: 0,
        }
    }

    /// 创建最终块
    pub fn final_chunk(chunk_id: u64, content: String) -> Self {
        Self {
            chunk_id,
            content,
            is_final: true,
            latency_ms: 0,
        }
    }

    /// 设置延迟
    pub fn with_latency(mut self, ms: u32) -> Self {
        self.latency_ms = ms;
        self
    }
}
