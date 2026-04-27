//! # OmniAgent 多模态交互服务
//!
//! 本 crate 提供多模态交互的核心抽象，支持文本、图像、音频、视频等多种模态的
//! 输入输出和处理。包含以下主要模块：
//!
//! - **模态类型**：定义支持的模态和内容类型
//! - **多模态内容**：统一的内容表示
//! - **多模态会话**：对话管理和历史记录
//! - **AI 模型接口**：模型配置、推理请求/响应
//! - **错误类型**：统一的错误处理

use std::collections::HashMap;

// ============================================================================
// 模态类型
// ============================================================================

/// 模态类型
///
/// 表示内容的基本模态分类，用于多模态处理管道中的路由和转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Modality {
    /// 纯文本
    Text = 0,
    /// 图像
    Image = 1,
    /// 音频
    Audio = 2,
    /// 视频
    Video = 3,
    /// 代码
    Code = 4,
    /// 结构化数据 (JSON/XML 等)
    Structured = 5,
    /// 二进制数据
    Binary = 6,
}

impl Modality {
    /// 获取模态类型的名称
    pub fn name(&self) -> &'static str {
        match self {
            Modality::Text => "text",
            Modality::Image => "image",
            Modality::Audio => "audio",
            Modality::Video => "video",
            Modality::Code => "code",
            Modality::Structured => "structured",
            Modality::Binary => "binary",
        }
    }
}

impl std::fmt::Display for Modality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 内容类型
///
/// 表示具体的内容格式，用于 MIME 类型解析和内容处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    /// 纯文本
    PlainText,
    /// Markdown 格式
    Markdown,
    /// HTML 格式
    Html,
    /// JSON 格式
    Json,
    /// XML 格式
    Xml,
    /// CSV 格式
    Csv,
    /// PNG 图像
    Png,
    /// JPEG 图像
    Jpeg,
    /// WebP 图像
    WebP,
    /// GIF 图像
    Gif,
    /// BMP 图像
    Bmp,
    /// SVG 矢量图
    Svg,
    /// WAV 音频
    Wav,
    /// MP3 音频
    Mp3,
    /// OGG 音频
    Ogg,
    /// FLAC 音频
    Flac,
    /// MP4 视频
    Mp4,
    /// WebM 视频
    WebM,
    /// 原始数据
    Raw,
    /// 未知类型
    Unknown(String),
}

impl ContentType {
    /// 从 MIME 类型解析内容类型
    ///
    /// 支持常见的 MIME 类型映射，对于未知的类型返回 `Unknown`。
    pub fn from_mime(mime: &str) -> Self {
        // 去除可能的参数部分 (如 "text/plain; charset=utf-8")
        let mime_type = mime.split(';').next().unwrap_or(mime).trim();

        match mime_type {
            "text/plain" => ContentType::PlainText,
            "text/markdown" | "text/x-markdown" => ContentType::Markdown,
            "text/html" => ContentType::Html,
            "application/json" => ContentType::Json,
            "application/xml" | "text/xml" => ContentType::Xml,
            "text/csv" => ContentType::Csv,
            "image/png" => ContentType::Png,
            "image/jpeg" => ContentType::Jpeg,
            "image/webp" => ContentType::WebP,
            "image/gif" => ContentType::Gif,
            "image/bmp" => ContentType::Bmp,
            "image/svg+xml" => ContentType::Svg,
            "audio/wav" | "audio/x-wav" => ContentType::Wav,
            "audio/mpeg" | "audio/mp3" => ContentType::Mp3,
            "audio/ogg" => ContentType::Ogg,
            "audio/flac" => ContentType::Flac,
            "video/mp4" => ContentType::Mp4,
            "video/webm" => ContentType::WebM,
            "application/octet-stream" => ContentType::Raw,
            other => ContentType::Unknown(other.to_string()),
        }
    }

    /// 获取 MIME 类型字符串
    pub fn as_mime(&self) -> &str {
        match self {
            ContentType::PlainText => "text/plain",
            ContentType::Markdown => "text/markdown",
            ContentType::Html => "text/html",
            ContentType::Json => "application/json",
            ContentType::Xml => "application/xml",
            ContentType::Csv => "text/csv",
            ContentType::Png => "image/png",
            ContentType::Jpeg => "image/jpeg",
            ContentType::WebP => "image/webp",
            ContentType::Gif => "image/gif",
            ContentType::Bmp => "image/bmp",
            ContentType::Svg => "image/svg+xml",
            ContentType::Wav => "audio/wav",
            ContentType::Mp3 => "audio/mpeg",
            ContentType::Ogg => "audio/ogg",
            ContentType::Flac => "audio/flac",
            ContentType::Mp4 => "video/mp4",
            ContentType::WebM => "video/webm",
            ContentType::Raw => "application/octet-stream",
            ContentType::Unknown(s) => s.as_str(),
        }
    }

    /// 获取对应的模态类型
    pub fn modality(&self) -> Modality {
        match self {
            ContentType::PlainText
            | ContentType::Markdown
            | ContentType::Html => Modality::Text,
            ContentType::Json | ContentType::Xml | ContentType::Csv => Modality::Structured,
            ContentType::Png
            | ContentType::Jpeg
            | ContentType::WebP
            | ContentType::Gif
            | ContentType::Bmp
            | ContentType::Svg => Modality::Image,
            ContentType::Wav | ContentType::Mp3 | ContentType::Ogg | ContentType::Flac => {
                Modality::Audio
            }
            ContentType::Mp4 | ContentType::WebM => Modality::Video,
            ContentType::Raw | ContentType::Unknown(_) => Modality::Binary,
        }
    }

    /// 是否为文本类型
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            ContentType::PlainText
                | ContentType::Markdown
                | ContentType::Html
                | ContentType::Json
                | ContentType::Xml
                | ContentType::Csv
                | ContentType::Svg
        )
    }

    /// 是否为图像类型
    pub fn is_image(&self) -> bool {
        matches!(
            self,
            ContentType::Png
                | ContentType::Jpeg
                | ContentType::WebP
                | ContentType::Gif
                | ContentType::Bmp
                | ContentType::Svg
        )
    }

    /// 是否为音频类型
    pub fn is_audio(&self) -> bool {
        matches!(
            self,
            ContentType::Wav | ContentType::Mp3 | ContentType::Ogg | ContentType::Flac
        )
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_mime())
    }
}

// ============================================================================
// 多模态内容
// ============================================================================

/// 多模态内容
///
/// 统一表示各种模态的内容，包含原始数据、文本表示和元数据。
/// 是多模态交互系统中的核心数据结构。
#[derive(Debug, Clone)]
pub struct MultiModalContent {
    /// 内容 ID (唯一标识符)
    pub id: String,
    /// 模态类型
    pub modality: Modality,
    /// 内容类型
    pub content_type: ContentType,
    /// 原始二进制数据
    pub data: Vec<u8>,
    /// 文本表示 (如果有)
    pub text: Option<String>,
    /// 元数据键值对
    pub metadata: HashMap<String, String>,
    /// 创建时间 (Unix 时间戳毫秒)
    pub created_at: u64,
    /// 数据大小 (字节)
    pub size: usize,
}

impl MultiModalContent {
    /// 从文本创建多模态内容
    ///
    /// 自动设置模态类型和大小信息。
    pub fn from_text(text: &str, content_type: ContentType) -> Self {
        let modality = content_type.modality();
        let data = text.as_bytes().to_vec();
        let size = data.len();
        MultiModalContent {
            id: generate_id(),
            modality,
            content_type,
            data,
            text: Some(text.to_string()),
            metadata: HashMap::new(),
            created_at: current_timestamp_ms(),
            size,
        }
    }

    /// 从二进制数据创建多模态内容
    ///
    /// 自动设置模态类型和大小信息。
    pub fn from_binary(data: Vec<u8>, content_type: ContentType) -> Self {
        let modality = content_type.modality();
        let size = data.len();
        // 如果内容类型是文本类型，尝试解码为文本
        let text = if content_type.is_text() {
            String::from_utf8(data.clone()).ok()
        } else {
            None
        };
        MultiModalContent {
            id: generate_id(),
            modality,
            content_type,
            data,
            text,
            metadata: HashMap::new(),
            created_at: current_timestamp_ms(),
            size,
        }
    }

    /// 获取文本内容
    ///
    /// 如果内容有文本表示则返回，否则返回错误。
    pub fn text(&self) -> Result<&str, MultimodalError> {
        self.text
            .as_deref()
            .ok_or_else(|| MultimodalError::InvalidContent("该内容没有文本表示".to_string()))
    }

    /// 获取数据引用
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    /// 获取元数据
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// 转换为另一种模态 (接口)
    ///
    /// 当前为桩实现，仅支持相同模态的"转换"（返回克隆）。
    /// 实际的模态转换将在后续阶段实现。
    pub fn convert(&self, target: Modality) -> Result<MultiModalContent, MultimodalError> {
        if self.modality == target {
            return Ok(self.clone());
        }
        Err(MultimodalError::ConversionError {
            from: self.modality,
            to: target,
        })
    }
}

// ============================================================================
// 多模态会话
// ============================================================================

/// 会话角色
///
/// 标识会话中消息的发送者角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpeakerRole {
    /// 系统消息
    System = 0,
    /// 用户消息
    User = 1,
    /// 助手消息
    Assistant = 2,
    /// 工具消息
    Tool = 3,
}

impl SpeakerRole {
    /// 获取角色名称
    pub fn name(&self) -> &'static str {
        match self {
            SpeakerRole::System => "system",
            SpeakerRole::User => "user",
            SpeakerRole::Assistant => "assistant",
            SpeakerRole::Tool => "tool",
        }
    }
}

impl std::fmt::Display for SpeakerRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 会话消息
///
/// 表示会话中的一条消息，包含角色、内容和时间戳。
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    /// 消息 ID (自增)
    pub id: u64,
    /// 发送者角色
    pub role: SpeakerRole,
    /// 消息内容 (支持多模态)
    pub content: Vec<MultiModalContent>,
    /// 时间戳 (Unix 时间戳毫秒)
    pub timestamp: u64,
    /// 消息元数据
    pub metadata: HashMap<String, String>,
}

/// 多模态会话
///
/// 管理对话历史，支持多模态消息的添加、查询和上下文管理。
#[derive(Debug, Clone)]
pub struct Conversation {
    /// 会话 ID
    pub id: String,
    /// 消息历史
    pub messages: Vec<ConversationMessage>,
    /// 系统提示
    pub system_prompt: Option<String>,
    /// 最大消息数量 (0 表示无限制)
    pub max_messages: usize,
    /// 创建时间
    pub created_at: u64,
    /// 会话元数据
    pub metadata: HashMap<String, String>,
}

impl Conversation {
    /// 创建新的会话
    pub fn new(id: &str) -> Self {
        Conversation {
            id: id.to_string(),
            messages: Vec::new(),
            system_prompt: None,
            max_messages: 0,
            created_at: current_timestamp_ms(),
            metadata: HashMap::new(),
        }
    }

    /// 设置系统提示
    pub fn set_system_prompt(&mut self, prompt: &str) {
        self.system_prompt = Some(prompt.to_string());
    }

    /// 添加用户消息
    ///
    /// 返回消息 ID。
    pub fn add_user_message(&mut self, content: MultiModalContent) -> u64 {
        self.add_message(SpeakerRole::User, content)
    }

    /// 添加助手消息
    ///
    /// 返回消息 ID。
    pub fn add_assistant_message(&mut self, content: MultiModalContent) -> u64 {
        self.add_message(SpeakerRole::Assistant, content)
    }

    /// 添加工具消息
    ///
    /// 返回消息 ID。
    pub fn add_tool_message(&mut self, content: MultiModalContent) -> u64 {
        self.add_message(SpeakerRole::Tool, content)
    }

    /// 内部方法：添加消息
    fn add_message(&mut self, role: SpeakerRole, content: MultiModalContent) -> u64 {
        let id = self.messages.len() as u64 + 1;
        let message = ConversationMessage {
            id,
            role,
            content: vec![content],
            timestamp: current_timestamp_ms(),
            metadata: HashMap::new(),
        };
        self.messages.push(message);

        // 如果设置了最大消息数，移除最早的消息
        if self.max_messages > 0 && self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }

        id
    }

    /// 获取消息历史
    pub fn messages(&self) -> &[ConversationMessage] {
        &self.messages
    }

    /// 获取最近 N 条消息
    pub fn recent_messages(&self, n: usize) -> &[ConversationMessage] {
        let len = self.messages.len();
        if n >= len {
            &self.messages
        } else {
            &self.messages[len - n..]
        }
    }

    /// 消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 清除历史消息
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// 获取所有文本内容 (用于纯文本模型)
    ///
    /// 将所有消息提取为 (角色, 文本) 对，忽略非文本内容。
    pub fn text_history(&self) -> Vec<(SpeakerRole, String)> {
        let mut result = Vec::new();
        for msg in &self.messages {
            for content in &msg.content {
                if let Some(text) = &content.text {
                    result.push((msg.role, text.clone()));
                }
            }
        }
        result
    }

    /// 获取上下文窗口大小 (token 估算)
    ///
    /// 使用简单的字符数 / 4 估算 token 数量。
    /// 这是一个粗略的估算，实际应用中应使用分词器。
    pub fn context_size(&self) -> usize {
        let mut total_chars: usize = 0;

        // 系统提示
        if let Some(prompt) = &self.system_prompt {
            total_chars += prompt.len();
        }

        // 所有消息的文本内容
        for msg in &self.messages {
            for content in &msg.content {
                if let Some(text) = &content.text {
                    total_chars += text.len();
                }
            }
        }

        // 粗略估算：每 4 个字符约等于 1 个 token
        total_chars / 4
    }
}

// ============================================================================
// AI 模型接口
// ============================================================================

/// 模型提供商
///
/// 标识 AI 模型的来源和运行方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelProvider {
    /// 本地 Candle 模型
    LocalCandle = 0,
    /// 本地 ONNX Runtime 模型
    LocalOnnx = 1,
    /// OpenAI API
    OpenAI = 2,
    /// Anthropic API
    Anthropic = 3,
    /// 自定义提供商
    Custom = 4,
}

impl ModelProvider {
    /// 获取提供商名称
    pub fn name(&self) -> &'static str {
        match self {
            ModelProvider::LocalCandle => "local_candle",
            ModelProvider::LocalOnnx => "local_onnx",
            ModelProvider::OpenAI => "openai",
            ModelProvider::Anthropic => "anthropic",
            ModelProvider::Custom => "custom",
        }
    }
}

impl std::fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 模型配置
///
/// 包含 AI 模型的所有配置参数，用于初始化和调用模型。
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// 模型提供商
    pub provider: ModelProvider,
    /// 模型名称
    pub model_name: String,
    /// API 端点 (远程模型)
    pub api_endpoint: Option<String>,
    /// API 密钥 (远程模型)
    pub api_key: Option<String>,
    /// 最大生成 token 数
    pub max_tokens: u32,
    /// 采样温度
    pub temperature: f32,
    /// Top-P 采样参数
    pub top_p: f32,
    /// 频率惩罚
    pub frequency_penalty: f32,
    /// 存在惩罚
    pub presence_penalty: f32,
    /// 是否支持流式输出
    pub supports_streaming: bool,
    /// 支持的模态列表
    pub supported_modalities: Vec<Modality>,
}

impl ModelConfig {
    /// 创建 OpenAI 配置
    pub fn openai(api_key: &str, model: &str) -> Self {
        ModelConfig {
            provider: ModelProvider::OpenAI,
            model_name: model.to_string(),
            api_endpoint: Some("https://api.openai.com/v1".to_string()),
            api_key: Some(api_key.to_string()),
            max_tokens: 4096,
            temperature: 0.7,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            supports_streaming: true,
            supported_modalities: vec![Modality::Text, Modality::Image],
        }
    }

    /// 创建 Anthropic 配置
    pub fn anthropic(api_key: &str, model: &str) -> Self {
        ModelConfig {
            provider: ModelProvider::Anthropic,
            model_name: model.to_string(),
            api_endpoint: Some("https://api.anthropic.com".to_string()),
            api_key: Some(api_key.to_string()),
            max_tokens: 4096,
            temperature: 0.7,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            supports_streaming: true,
            supported_modalities: vec![Modality::Text, Modality::Image],
        }
    }

    /// 创建本地 Candle 配置
    pub fn local_candle(model_path: &str) -> Self {
        ModelConfig {
            provider: ModelProvider::LocalCandle,
            model_name: model_path.to_string(),
            api_endpoint: None,
            api_key: None,
            max_tokens: 2048,
            temperature: 0.8,
            top_p: 0.9,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            supports_streaming: false,
            supported_modalities: vec![Modality::Text],
        }
    }

    /// 创建本地 ONNX 配置
    pub fn local_onnx(model_path: &str) -> Self {
        ModelConfig {
            provider: ModelProvider::LocalOnnx,
            model_name: model_path.to_string(),
            api_endpoint: None,
            api_key: None,
            max_tokens: 2048,
            temperature: 0.8,
            top_p: 0.9,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            supports_streaming: false,
            supported_modalities: vec![Modality::Text],
        }
    }
}

/// 推理请求
///
/// 包含发送给 AI 模型的完整请求信息。
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// 模型配置
    pub model_config: ModelConfig,
    /// 会话上下文
    pub conversation: Conversation,
    /// 可用工具列表
    pub tools: Vec<ToolDefinition>,
    /// 是否使用流式输出
    pub stream: bool,
}

/// 推理响应
///
/// 包含 AI 模型返回的完整响应信息。
#[derive(Debug, Clone)]
pub struct InferenceResponse {
    /// 响应内容
    pub content: MultiModalContent,
    /// Token 使用统计
    pub usage: TokenUsage,
    /// 完成原因
    pub finish_reason: FinishReason,
    /// 使用的模型名称
    pub model: String,
    /// 创建时间
    pub created_at: u64,
}

/// Token 使用统计
#[derive(Debug, Clone)]
pub struct TokenUsage {
    /// 提示 token 数
    pub prompt_tokens: u32,
    /// 生成 token 数
    pub completion_tokens: u32,
    /// 总 token 数
    pub total_tokens: u32,
}

impl TokenUsage {
    /// 创建新的 Token 使用统计
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// 完成原因
///
/// 标识模型生成结束的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FinishReason {
    /// 正常结束
    Stop = 0,
    /// 达到最大长度
    Length = 1,
    /// 需要调用工具
    ToolCall = 2,
    /// 内容被过滤
    ContentFilter = 3,
    /// 发生错误
    Error = 4,
}

/// 工具定义
///
/// 描述可供 AI 模型调用的工具。
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: String,
    /// 处理器标识
    pub handler: Option<String>,
}

impl ToolDefinition {
    /// 创建新的工具定义
    pub fn new(name: &str, description: &str, parameters: &str) -> Self {
        ToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            parameters: parameters.to_string(),
            handler: None,
        }
    }

    /// 设置处理器
    pub fn with_handler(mut self, handler: &str) -> Self {
        self.handler = Some(handler.to_string());
        self
    }
}

/// AI 模型管理器
///
/// 管理多个 AI 模型的注册、切换和调用。
pub struct ModelManager {
    /// 已注册的模型
    models: HashMap<String, ModelConfig>,
    /// 当前活跃的模型名称
    active_model: Option<String>,
}

impl ModelManager {
    /// 创建新的模型管理器
    pub fn new() -> Self {
        ModelManager {
            models: HashMap::new(),
            active_model: None,
        }
    }

    /// 注册模型
    ///
    /// 将模型配置注册到管理器中。
    pub fn register_model(&mut self, name: &str, config: ModelConfig) -> Result<(), MultimodalError> {
        if self.models.contains_key(name) {
            return Err(MultimodalError::InvalidModelConfig(format!(
                "模型 '{}' 已存在",
                name
            )));
        }
        self.models.insert(name.to_string(), config);
        Ok(())
    }

    /// 设置活跃模型
    ///
    /// 切换当前使用的模型。
    pub fn set_active(&mut self, name: &str) -> Result<(), MultimodalError> {
        if !self.models.contains_key(name) {
            return Err(MultimodalError::ModelNotFound(name.to_string()));
        }
        self.active_model = Some(name.to_string());
        Ok(())
    }

    /// 获取活跃模型配置
    pub fn active_model(&self) -> Option<&ModelConfig> {
        self.active_model
            .as_ref()
            .and_then(|name| self.models.get(name))
    }

    /// 列出所有已注册的模型名称
    pub fn list_models(&self) -> Vec<&str> {
        self.models.keys().map(|s| s.as_str()).collect()
    }

    /// 推理 (桩实现)
    ///
    /// 当前返回一个模拟的响应。实际推理将在后续阶段实现。
    pub fn infer(&self, request: &InferenceRequest) -> Result<InferenceResponse, MultimodalError> {
        // 桩实现：返回模拟响应
        let response_text = format!(
            "[模拟响应] 使用模型 {} 处理了 {} 条消息",
            request.model_config.model_name,
            request.conversation.message_count()
        );

        let content = MultiModalContent::from_text(&response_text, ContentType::PlainText);

        let prompt_tokens = request.conversation.context_size() as u32;
        let completion_tokens = (response_text.len() / 4) as u32;

        Ok(InferenceResponse {
            content,
            usage: TokenUsage::new(prompt_tokens, completion_tokens),
            finish_reason: FinishReason::Stop,
            model: request.model_config.model_name.clone(),
            created_at: current_timestamp_ms(),
        })
    }

    /// 移除模型
    ///
    /// 从管理器中移除指定模型。如果移除的是活跃模型，则清除活跃状态。
    pub fn remove_model(&mut self, name: &str) -> Result<(), MultimodalError> {
        if self.models.remove(name).is_none() {
            return Err(MultimodalError::ModelNotFound(name.to_string()));
        }
        // 如果移除的是活跃模型，清除活跃状态
        if self.active_model.as_deref() == Some(name) {
            self.active_model = None;
        }
        Ok(())
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 错误类型
// ============================================================================

/// 多模态错误类型
///
/// 统一的多模态处理错误枚举。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultimodalError {
    /// 不支持的模态类型
    UnsupportedModality(Modality),
    /// 模态转换错误
    ConversionError {
        /// 源模态
        from: Modality,
        /// 目标模态
        to: Modality,
    },
    /// 无效内容
    InvalidContent(String),
    /// 模型未找到
    ModelNotFound(String),
    /// 推理错误
    InferenceError(String),
    /// 上下文过大
    ContextTooLarge {
        /// 请求的大小
        requested: usize,
        /// 最大允许的大小
        max: usize,
    },
    /// 无效的模型配置
    InvalidModelConfig(String),
    /// API 错误
    ApiError {
        /// 提供商
        provider: ModelProvider,
        /// 错误信息
        message: String,
    },
    /// 超时
    Timeout(u64),
    /// 内容过大
    ContentTooLarge {
        /// 实际大小
        size: usize,
        /// 最大允许大小
        max: usize,
    },
}

impl std::fmt::Display for MultimodalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultimodalError::UnsupportedModality(m) => {
                write!(f, "不支持的模态类型: {}", m)
            }
            MultimodalError::ConversionError { from, to } => {
                write!(f, "无法从 {} 转换为 {}", from, to)
            }
            MultimodalError::InvalidContent(msg) => write!(f, "无效内容: {}", msg),
            MultimodalError::ModelNotFound(name) => write!(f, "模型未找到: {}", name),
            MultimodalError::InferenceError(msg) => write!(f, "推理错误: {}", msg),
            MultimodalError::ContextTooLarge { requested, max } => {
                write!(f, "上下文过大: 请求 {} token，最大允许 {} token", requested, max)
            }
            MultimodalError::InvalidModelConfig(msg) => {
                write!(f, "无效的模型配置: {}", msg)
            }
            MultimodalError::ApiError { provider, message } => {
                write!(f, "API 错误 ({}): {}", provider, message)
            }
            MultimodalError::Timeout(ms) => write!(f, "操作超时: {}ms", ms),
            MultimodalError::ContentTooLarge { size, max } => {
                write!(f, "内容过大: {} 字节，最大允许 {} 字节", size, max)
            }
        }
    }
}

impl std::error::Error for MultimodalError {}

// ============================================================================
// 辅助函数
// ============================================================================

/// 生成唯一 ID
///
/// 使用简单的递增计数器生成 ID。
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("mm_{}", id)
}

/// 获取当前时间戳 (毫秒)
///
/// 返回一个基于单调计数器的模拟时间戳。
/// 在 no_std 环境中无法使用系统时间，这里使用简单计数器。
fn current_timestamp_ms() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TIMESTAMP: AtomicU64 = AtomicU64::new(1_700_000_000_000);
    TIMESTAMP.fetch_add(1, Ordering::Relaxed)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ContentType 测试
    // ========================================================================

    #[test]
    fn test_content_type_from_mime_text() {
        assert_eq!(ContentType::from_mime("text/plain"), ContentType::PlainText);
        assert_eq!(ContentType::from_mime("text/markdown"), ContentType::Markdown);
        assert_eq!(ContentType::from_mime("text/html"), ContentType::Html);
    }

    #[test]
    fn test_content_type_from_mime_structured() {
        assert_eq!(ContentType::from_mime("application/json"), ContentType::Json);
        assert_eq!(ContentType::from_mime("application/xml"), ContentType::Xml);
        assert_eq!(ContentType::from_mime("text/xml"), ContentType::Xml);
        assert_eq!(ContentType::from_mime("text/csv"), ContentType::Csv);
    }

    #[test]
    fn test_content_type_from_mime_image() {
        assert_eq!(ContentType::from_mime("image/png"), ContentType::Png);
        assert_eq!(ContentType::from_mime("image/jpeg"), ContentType::Jpeg);
        assert_eq!(ContentType::from_mime("image/webp"), ContentType::WebP);
        assert_eq!(ContentType::from_mime("image/gif"), ContentType::Gif);
        assert_eq!(ContentType::from_mime("image/bmp"), ContentType::Bmp);
        assert_eq!(ContentType::from_mime("image/svg+xml"), ContentType::Svg);
    }

    #[test]
    fn test_content_type_from_mime_audio() {
        assert_eq!(ContentType::from_mime("audio/wav"), ContentType::Wav);
        assert_eq!(ContentType::from_mime("audio/x-wav"), ContentType::Wav);
        assert_eq!(ContentType::from_mime("audio/mpeg"), ContentType::Mp3);
        assert_eq!(ContentType::from_mime("audio/ogg"), ContentType::Ogg);
        assert_eq!(ContentType::from_mime("audio/flac"), ContentType::Flac);
    }

    #[test]
    fn test_content_type_from_mime_video() {
        assert_eq!(ContentType::from_mime("video/mp4"), ContentType::Mp4);
        assert_eq!(ContentType::from_mime("video/webm"), ContentType::WebM);
    }

    #[test]
    fn test_content_type_from_mime_with_params() {
        // 测试带参数的 MIME 类型
        assert_eq!(
            ContentType::from_mime("text/plain; charset=utf-8"),
            ContentType::PlainText
        );
        assert_eq!(
            ContentType::from_mime("application/json; charset=utf-8"),
            ContentType::Json
        );
    }

    #[test]
    fn test_content_type_from_mime_unknown() {
        let result = ContentType::from_mime("application/unknown-type");
        assert!(matches!(result, ContentType::Unknown(_)));
        if let ContentType::Unknown(s) = result {
            assert_eq!(s, "application/unknown-type");
        }
    }

    #[test]
    fn test_content_type_as_mime() {
        assert_eq!(ContentType::PlainText.as_mime(), "text/plain");
        assert_eq!(ContentType::Json.as_mime(), "application/json");
        assert_eq!(ContentType::Png.as_mime(), "image/png");
        assert_eq!(ContentType::Mp3.as_mime(), "audio/mpeg");
        assert_eq!(ContentType::Mp4.as_mime(), "video/mp4");
    }

    #[test]
    fn test_content_type_modality() {
        // 文本模态
        assert_eq!(ContentType::PlainText.modality(), Modality::Text);
        assert_eq!(ContentType::Markdown.modality(), Modality::Text);
        assert_eq!(ContentType::Html.modality(), Modality::Text);

        // 结构化模态
        assert_eq!(ContentType::Json.modality(), Modality::Structured);
        assert_eq!(ContentType::Xml.modality(), Modality::Structured);
        assert_eq!(ContentType::Csv.modality(), Modality::Structured);

        // 图像模态
        assert_eq!(ContentType::Png.modality(), Modality::Image);
        assert_eq!(ContentType::Jpeg.modality(), Modality::Image);
        assert_eq!(ContentType::Svg.modality(), Modality::Image);

        // 音频模态
        assert_eq!(ContentType::Wav.modality(), Modality::Audio);
        assert_eq!(ContentType::Mp3.modality(), Modality::Audio);

        // 视频模态
        assert_eq!(ContentType::Mp4.modality(), Modality::Video);
        assert_eq!(ContentType::WebM.modality(), Modality::Video);

        // 二进制模态
        assert_eq!(ContentType::Raw.modality(), Modality::Binary);
        assert_eq!(
            ContentType::Unknown("test".to_string()).modality(),
            Modality::Binary
        );
    }

    #[test]
    fn test_content_type_is_text() {
        assert!(ContentType::PlainText.is_text());
        assert!(ContentType::Markdown.is_text());
        assert!(ContentType::Html.is_text());
        assert!(ContentType::Json.is_text()); // JSON 也是文本
        assert!(ContentType::Xml.is_text());
        assert!(ContentType::Csv.is_text());
        assert!(ContentType::Svg.is_text()); // SVG 是文本格式

        assert!(!ContentType::Png.is_text());
        assert!(!ContentType::Mp3.is_text());
        assert!(!ContentType::Mp4.is_text());
    }

    #[test]
    fn test_content_type_is_image() {
        assert!(ContentType::Png.is_image());
        assert!(ContentType::Jpeg.is_image());
        assert!(ContentType::WebP.is_image());
        assert!(ContentType::Gif.is_image());
        assert!(ContentType::Bmp.is_image());
        assert!(ContentType::Svg.is_image());

        assert!(!ContentType::PlainText.is_image());
        assert!(!ContentType::Mp3.is_image());
    }

    #[test]
    fn test_content_type_is_audio() {
        assert!(ContentType::Wav.is_audio());
        assert!(ContentType::Mp3.is_audio());
        assert!(ContentType::Ogg.is_audio());
        assert!(ContentType::Flac.is_audio());

        assert!(!ContentType::PlainText.is_audio());
        assert!(!ContentType::Png.is_audio());
        assert!(!ContentType::Mp4.is_audio());
    }

    // ========================================================================
    // MultiModalContent 测试
    // ========================================================================

    #[test]
    fn test_from_text() {
        let content = MultiModalContent::from_text("你好世界", ContentType::PlainText);

        assert_eq!(content.modality, Modality::Text);
        assert_eq!(content.content_type, ContentType::PlainText);
        assert_eq!(content.data, b"\xe4\xbd\xa0\xe5\xa5\xbd\xe4\xb8\x96\xe7\x95\x8c");
        assert_eq!(content.text, Some("你好世界".to_string()));
        assert_eq!(content.size, 12); // UTF-8 编码的字节数
        assert!(content.metadata.is_empty());
    }

    #[test]
    fn test_from_text_markdown() {
        let md = "# 标题\n\n正文内容";
        let content = MultiModalContent::from_text(md, ContentType::Markdown);

        assert_eq!(content.modality, Modality::Text);
        assert_eq!(content.content_type, ContentType::Markdown);
        assert_eq!(content.text(), Ok(md));
    }

    #[test]
    fn test_from_binary_image() {
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG 文件头
        let content = MultiModalContent::from_binary(data.clone(), ContentType::Png);

        assert_eq!(content.modality, Modality::Image);
        assert_eq!(content.content_type, ContentType::Png);
        assert_eq!(content.data, data);
        assert!(content.text.is_none()); // 图像没有文本表示
        assert_eq!(content.size, 4);
    }

    #[test]
    fn test_from_binary_text_type() {
        // 二进制数据但内容类型是文本
        let data = b"hello world".to_vec();
        let content = MultiModalContent::from_binary(data.clone(), ContentType::Json);

        assert_eq!(content.modality, Modality::Structured);
        assert_eq!(content.data, data);
        assert_eq!(content.text, Some("hello world".to_string()));
    }

    #[test]
    fn test_from_binary_invalid_utf8() {
        // 无效 UTF-8 数据，即使内容类型是文本
        let data = vec![0xFF, 0xFE];
        let content = MultiModalContent::from_binary(data, ContentType::PlainText);

        assert!(content.text.is_none());
    }

    #[test]
    fn test_content_text_success() {
        let content = MultiModalContent::from_text("测试", ContentType::PlainText);
        assert_eq!(content.text(), Ok("测试"));
    }

    #[test]
    fn test_content_text_failure() {
        let content = MultiModalContent::from_binary(vec![0x00, 0x01], ContentType::Png);
        assert!(content.text().is_err());
    }

    #[test]
    fn test_content_data() {
        let data = vec![1, 2, 3, 4, 5];
        let content = MultiModalContent::from_binary(data.clone(), ContentType::Raw);
        assert_eq!(content.data(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_content_metadata() {
        let mut content = MultiModalContent::from_text("test", ContentType::PlainText);

        assert!(content.get_metadata("key").is_none());

        content.set_metadata("key", "value");
        assert_eq!(content.get_metadata("key"), Some("value"));

        content.set_metadata("source", "upload");
        assert_eq!(content.get_metadata("source"), Some("upload"));
    }

    #[test]
    fn test_content_convert_same_modality() {
        let content = MultiModalContent::from_text("test", ContentType::PlainText);
        let converted = content.convert(Modality::Text).unwrap();

        assert_eq!(converted.modality, Modality::Text);
        assert_eq!(converted.text(), Ok("test"));
    }

    #[test]
    fn test_content_convert_different_modality() {
        let content = MultiModalContent::from_text("test", ContentType::PlainText);
        let result = content.convert(Modality::Image);

        assert!(result.is_err());
        if let Err(MultimodalError::ConversionError { from, to }) = result {
            assert_eq!(from, Modality::Text);
            assert_eq!(to, Modality::Image);
        } else {
            panic!("期望 ConversionError");
        }
    }

    // ========================================================================
    // Conversation 测试
    // ========================================================================

    #[test]
    fn test_conversation_new() {
        let conv = Conversation::new("test-conv");
        assert_eq!(conv.id, "test-conv");
        assert!(conv.messages.is_empty());
        assert!(conv.system_prompt.is_none());
        assert_eq!(conv.max_messages, 0);
    }

    #[test]
    fn test_conversation_set_system_prompt() {
        let mut conv = Conversation::new("test");
        conv.set_system_prompt("你是一个有用的助手。");
        assert_eq!(conv.system_prompt, Some("你是一个有用的助手。".to_string()));
    }

    #[test]
    fn test_conversation_add_user_message() {
        let mut conv = Conversation::new("test");
        let content = MultiModalContent::from_text("你好", ContentType::PlainText);
        let msg_id = conv.add_user_message(content);

        assert_eq!(msg_id, 1);
        assert_eq!(conv.message_count(), 1);
        assert_eq!(conv.messages()[0].role, SpeakerRole::User);
        assert_eq!(conv.messages()[0].content[0].text(), Ok("你好"));
    }

    #[test]
    fn test_conversation_add_assistant_message() {
        let mut conv = Conversation::new("test");
        let content = MultiModalContent::from_text("你好！有什么可以帮助你的？", ContentType::PlainText);
        let msg_id = conv.add_assistant_message(content);

        assert_eq!(msg_id, 1);
        assert_eq!(conv.messages()[0].role, SpeakerRole::Assistant);
    }

    #[test]
    fn test_conversation_add_tool_message() {
        let mut conv = Conversation::new("test");
        let content = MultiModalContent::from_text("{\"result\": 42}", ContentType::Json);
        let msg_id = conv.add_tool_message(content);

        assert_eq!(msg_id, 1);
        assert_eq!(conv.messages()[0].role, SpeakerRole::Tool);
    }

    #[test]
    fn test_conversation_multiple_messages() {
        let mut conv = Conversation::new("test");

        let id1 = conv.add_user_message(MultiModalContent::from_text("问题1", ContentType::PlainText));
        let id2 = conv.add_assistant_message(MultiModalContent::from_text("回答1", ContentType::PlainText));
        let id3 = conv.add_user_message(MultiModalContent::from_text("问题2", ContentType::PlainText));
        let id4 = conv.add_assistant_message(MultiModalContent::from_text("回答2", ContentType::PlainText));

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(id4, 4);
        assert_eq!(conv.message_count(), 4);
    }

    #[test]
    fn test_conversation_recent_messages() {
        let mut conv = Conversation::new("test");

        for i in 0..5 {
            conv.add_user_message(MultiModalContent::from_text(
                &format!("消息{}", i),
                ContentType::PlainText,
            ));
        }

        // 获取最近 3 条
        let recent = conv.recent_messages(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content[0].text(), Ok("消息2"));
        assert_eq!(recent[1].content[0].text(), Ok("消息3"));
        assert_eq!(recent[2].content[0].text(), Ok("消息4"));

        // 请求超过总数的最近消息
        let all_recent = conv.recent_messages(100);
        assert_eq!(all_recent.len(), 5);
    }

    #[test]
    fn test_conversation_clear() {
        let mut conv = Conversation::new("test");
        conv.add_user_message(MultiModalContent::from_text("消息", ContentType::PlainText));
        assert_eq!(conv.message_count(), 1);

        conv.clear();
        assert_eq!(conv.message_count(), 0);
        assert!(conv.messages().is_empty());
    }

    #[test]
    fn test_conversation_text_history() {
        let mut conv = Conversation::new("test");

        conv.add_user_message(MultiModalContent::from_text("你好", ContentType::PlainText));
        conv.add_assistant_message(MultiModalContent::from_text("你好！", ContentType::PlainText));
        // 添加一个没有文本表示的内容 (图像)
        conv.add_user_message(MultiModalContent::from_binary(
            vec![0x89, 0x50],
            ContentType::Png,
        ));

        let history = conv.text_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], (SpeakerRole::User, "你好".to_string()));
        assert_eq!(history[1], (SpeakerRole::Assistant, "你好！".to_string()));
    }

    #[test]
    fn test_conversation_context_size() {
        let mut conv = Conversation::new("test");
        conv.set_system_prompt("系统提示");

        // 系统提示: 4 个中文字符 = 12 字节 / 4 = 3 token
        let size = conv.context_size();
        assert!(size > 0);

        conv.add_user_message(MultiModalContent::from_text(
            "这是一条用户消息",
            ContentType::PlainText,
        ));
        let size_after = conv.context_size();
        assert!(size_after > size);
    }

    #[test]
    fn test_conversation_max_messages() {
        let mut conv = Conversation::new("test");
        conv.max_messages = 3;

        for i in 0..5 {
            conv.add_user_message(MultiModalContent::from_text(
                &format!("消息{}", i),
                ContentType::PlainText,
            ));
        }

        // 应该只保留最近 3 条
        assert_eq!(conv.message_count(), 3);
        assert_eq!(conv.messages()[0].content[0].text(), Ok("消息2"));
        assert_eq!(conv.messages()[2].content[0].text(), Ok("消息4"));
    }

    // ========================================================================
    // ModelConfig 测试
    // ========================================================================

    #[test]
    fn test_model_config_openai() {
        let config = ModelConfig::openai("sk-test-key", "gpt-4");

        assert_eq!(config.provider, ModelProvider::OpenAI);
        assert_eq!(config.model_name, "gpt-4");
        assert_eq!(config.api_key, Some("sk-test-key".to_string()));
        assert_eq!(
            config.api_endpoint,
            Some("https://api.openai.com/v1".to_string())
        );
        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.temperature, 0.7);
        assert!(config.supports_streaming);
        assert!(config.supported_modalities.contains(&Modality::Text));
        assert!(config.supported_modalities.contains(&Modality::Image));
    }

    #[test]
    fn test_model_config_anthropic() {
        let config = ModelConfig::anthropic("sk-ant-key", "claude-3-opus");

        assert_eq!(config.provider, ModelProvider::Anthropic);
        assert_eq!(config.model_name, "claude-3-opus");
        assert_eq!(config.api_key, Some("sk-ant-key".to_string()));
        assert_eq!(
            config.api_endpoint,
            Some("https://api.anthropic.com".to_string())
        );
        assert!(config.supports_streaming);
    }

    #[test]
    fn test_model_config_local_candle() {
        let config = ModelConfig::local_candle("/models/llama.bin");

        assert_eq!(config.provider, ModelProvider::LocalCandle);
        assert_eq!(config.model_name, "/models/llama.bin");
        assert!(config.api_endpoint.is_none());
        assert!(config.api_key.is_none());
        assert_eq!(config.max_tokens, 2048);
        assert!(!config.supports_streaming);
        assert_eq!(config.supported_modalities, vec![Modality::Text]);
    }

    #[test]
    fn test_model_config_local_onnx() {
        let config = ModelConfig::local_onnx("/models/model.onnx");

        assert_eq!(config.provider, ModelProvider::LocalOnnx);
        assert_eq!(config.model_name, "/models/model.onnx");
        assert!(config.api_endpoint.is_none());
        assert!(config.api_key.is_none());
        assert!(!config.supports_streaming);
    }

    // ========================================================================
    // ModelManager 测试
    // ========================================================================

    #[test]
    fn test_model_manager_new() {
        let manager = ModelManager::new();
        assert!(manager.active_model().is_none());
        assert!(manager.list_models().is_empty());
    }

    #[test]
    fn test_model_manager_register() {
        let mut manager = ModelManager::new();
        let config = ModelConfig::openai("key", "gpt-4");

        assert!(manager.register_model("gpt4", config).is_ok());
        assert_eq!(manager.list_models(), vec!["gpt4"]);
    }

    #[test]
    fn test_model_manager_register_duplicate() {
        let mut manager = ModelManager::new();
        let config1 = ModelConfig::openai("key1", "gpt-4");
        let config2 = ModelConfig::openai("key2", "gpt-4");

        assert!(manager.register_model("gpt4", config1).is_ok());
        assert!(manager.register_model("gpt4", config2).is_err());
    }

    #[test]
    fn test_model_manager_set_active() {
        let mut manager = ModelManager::new();
        manager
            .register_model("gpt4", ModelConfig::openai("key", "gpt-4"))
            .unwrap();

        assert!(manager.set_active("gpt4").is_ok());
        let active = manager.active_model().unwrap();
        assert_eq!(active.model_name, "gpt-4");
    }

    #[test]
    fn test_model_manager_set_active_not_found() {
        let mut manager = ModelManager::new();
        assert!(manager.set_active("nonexistent").is_err());
    }

    #[test]
    fn test_model_manager_list_models() {
        let mut manager = ModelManager::new();
        manager
            .register_model("gpt4", ModelConfig::openai("k1", "gpt-4"))
            .unwrap();
        manager
            .register_model("claude", ModelConfig::anthropic("k2", "claude-3"))
            .unwrap();
        manager
            .register_model("local", ModelConfig::local_candle("/model.bin"))
            .unwrap();

        let models = manager.list_models();
        assert_eq!(models.len(), 3);
        assert!(models.contains(&"gpt4"));
        assert!(models.contains(&"claude"));
        assert!(models.contains(&"local"));
    }

    #[test]
    fn test_model_manager_remove() {
        let mut manager = ModelManager::new();
        manager
            .register_model("gpt4", ModelConfig::openai("key", "gpt-4"))
            .unwrap();
        manager.set_active("gpt4").unwrap();

        assert!(manager.remove_model("gpt4").is_ok());
        assert!(manager.list_models().is_empty());
        assert!(manager.active_model().is_none()); // 活跃模型被清除
    }

    #[test]
    fn test_model_manager_remove_not_found() {
        let mut manager = ModelManager::new();
        assert!(manager.remove_model("nonexistent").is_err());
    }

    #[test]
    fn test_model_manager_remove_non_active() {
        let mut manager = ModelManager::new();
        manager
            .register_model("gpt4", ModelConfig::openai("key", "gpt-4"))
            .unwrap();
        manager
            .register_model("claude", ModelConfig::anthropic("key", "claude-3"))
            .unwrap();
        manager.set_active("gpt4").unwrap();

        // 移除非活跃模型，活跃模型应保持不变
        assert!(manager.remove_model("claude").is_ok());
        assert_eq!(manager.list_models(), vec!["gpt4"]);
        assert!(manager.active_model().is_some());
    }

    // ========================================================================
    // InferenceRequest/Response 测试
    // ========================================================================

    #[test]
    fn test_inference_request() {
        let mut conv = Conversation::new("test");
        conv.add_user_message(MultiModalContent::from_text("你好", ContentType::PlainText));

        let request = InferenceRequest {
            model_config: ModelConfig::openai("key", "gpt-4"),
            conversation: conv,
            tools: vec![],
            stream: false,
        };

        assert_eq!(request.model_config.model_name, "gpt-4");
        assert_eq!(request.conversation.message_count(), 1);
        assert!(request.tools.is_empty());
        assert!(!request.stream);
    }

    #[test]
    fn test_inference_request_with_tools() {
        let conv = Conversation::new("test");
        let tools = vec![
            ToolDefinition::new("search", "搜索工具", "{\"query\": \"string\"}"),
            ToolDefinition::new("calculator", "计算器", "{\"expression\": \"string\"}")
                .with_handler("calc_handler"),
        ];

        let request = InferenceRequest {
            model_config: ModelConfig::openai("key", "gpt-4"),
            conversation: conv,
            tools,
            stream: true,
        };

        assert_eq!(request.tools.len(), 2);
        assert_eq!(request.tools[0].name, "search");
        assert_eq!(request.tools[1].handler, Some("calc_handler".to_string()));
        assert!(request.stream);
    }

    #[test]
    fn test_inference_response() {
        let content = MultiModalContent::from_text("响应内容", ContentType::PlainText);
        let usage = TokenUsage::new(100, 50);

        let response = InferenceResponse {
            content,
            usage: usage.clone(),
            finish_reason: FinishReason::Stop,
            model: "gpt-4".to_string(),
            created_at: 1_700_000_000_000,
        };

        assert_eq!(response.content.text(), Ok("响应内容"));
        assert_eq!(response.usage.prompt_tokens, 100);
        assert_eq!(response.usage.completion_tokens, 50);
        assert_eq!(response.usage.total_tokens, 150);
        assert_eq!(response.finish_reason, FinishReason::Stop);
        assert_eq!(response.model, "gpt-4");
    }

    #[test]
    fn test_infer_stub() {
        let mut manager = ModelManager::new();
        manager
            .register_model("gpt4", ModelConfig::openai("key", "gpt-4"))
            .unwrap();
        manager.set_active("gpt4").unwrap();

        let mut conv = Conversation::new("test");
        conv.add_user_message(MultiModalContent::from_text("你好", ContentType::PlainText));

        let request = InferenceRequest {
            model_config: ModelConfig::openai("key", "gpt-4"),
            conversation: conv,
            tools: vec![],
            stream: false,
        };

        let response = manager.infer(&request).unwrap();
        assert_eq!(response.finish_reason, FinishReason::Stop);
        assert_eq!(response.model, "gpt-4");
        assert!(response.content.text().unwrap().contains("模拟响应"));
    }

    // ========================================================================
    // 辅助类型测试
    // ========================================================================

    #[test]
    fn test_modality_name() {
        assert_eq!(Modality::Text.name(), "text");
        assert_eq!(Modality::Image.name(), "image");
        assert_eq!(Modality::Audio.name(), "audio");
        assert_eq!(Modality::Video.name(), "video");
        assert_eq!(Modality::Code.name(), "code");
        assert_eq!(Modality::Structured.name(), "structured");
        assert_eq!(Modality::Binary.name(), "binary");
    }

    #[test]
    fn test_speaker_role_name() {
        assert_eq!(SpeakerRole::System.name(), "system");
        assert_eq!(SpeakerRole::User.name(), "user");
        assert_eq!(SpeakerRole::Assistant.name(), "assistant");
        assert_eq!(SpeakerRole::Tool.name(), "tool");
    }

    #[test]
    fn test_model_provider_name() {
        assert_eq!(ModelProvider::LocalCandle.name(), "local_candle");
        assert_eq!(ModelProvider::LocalOnnx.name(), "local_onnx");
        assert_eq!(ModelProvider::OpenAI.name(), "openai");
        assert_eq!(ModelProvider::Anthropic.name(), "anthropic");
        assert_eq!(ModelProvider::Custom.name(), "custom");
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(10, 20);
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition::new("weather", "天气查询", "{\"city\": \"string\"}");
        assert_eq!(tool.name, "weather");
        assert_eq!(tool.description, "天气查询");
        assert!(tool.handler.is_none());

        let tool_with_handler = tool.with_handler("weather_handler");
        assert_eq!(tool_with_handler.handler, Some("weather_handler".to_string()));
    }

    #[test]
    fn test_multimodal_error_display() {
        let err = MultimodalError::UnsupportedModality(Modality::Video);
        assert_eq!(format!("{}", err), "不支持的模态类型: video");

        let err = MultimodalError::ConversionError {
            from: Modality::Text,
            to: Modality::Image,
        };
        assert_eq!(format!("{}", err), "无法从 text 转换为 image");

        let err = MultimodalError::ModelNotFound("gpt-5".to_string());
        assert_eq!(format!("{}", err), "模型未找到: gpt-5");

        let err = MultimodalError::ContextTooLarge {
            requested: 10000,
            max: 4096,
        };
        assert_eq!(
            format!("{}", err),
            "上下文过大: 请求 10000 token，最大允许 4096 token"
        );

        let err = MultimodalError::ApiError {
            provider: ModelProvider::OpenAI,
            message: "rate limit".to_string(),
        };
        assert_eq!(format!("{}", err), "API 错误 (openai): rate limit");

        let err = MultimodalError::Timeout(30000);
        assert_eq!(format!("{}", err), "操作超时: 30000ms");

        let err = MultimodalError::ContentTooLarge {
            size: 50_000_000,
            max: 10_000_000,
        };
        assert_eq!(
            format!("{}", err),
            "内容过大: 50000000 字节，最大允许 10000000 字节"
        );
    }

    #[test]
    fn test_finish_reason_repr() {
        assert_eq!(FinishReason::Stop as u8, 0);
        assert_eq!(FinishReason::Length as u8, 1);
        assert_eq!(FinishReason::ToolCall as u8, 2);
        assert_eq!(FinishReason::ContentFilter as u8, 3);
        assert_eq!(FinishReason::Error as u8, 4);
    }
}
