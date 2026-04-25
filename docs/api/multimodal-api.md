# 多模态交互 API 参考

> **模块名称**: `multimodal-api`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

多模态交互 API 提供 OmniAgent OS 中文本、语音、图像、视频等多种模态的统一交互接口。通过此 API，用户和 Agent 可以以自然的方式（语音、文字、图像、视频）与系统进行交互，系统支持模态间的转换和对齐，并提供本地推理和云端推理的无缝切换。

### 1.2 架构概览

```
┌──────────────────────────────────────────────────────────┐
│                Multimodal Interaction API                │
├──────────┬──────────┬──────────┬────────────────────────┤
│ Unified  │  Voice   │  Text    │  Image                 │
│ Dialog   │ Engine   │ Engine   │  Engine                │
├──────────┼──────────┼──────────┼────────────────────────┤
│  Video   │Cross-    │  Model   │  Cloud Provider        │
│ Engine   │Modal     │ Manager  │                        │
├──────────┴──────────┴──────────┴────────────────────────┤
│              Inference Runtime (Local/Cloud)             │
└──────────────────────────────────────────────────────────┘
```

---

## 2. 统一对话 API

### 2.1 多模态消息与响应

```rust
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// 多模态消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModalMessage {
    /// 会话 ID
    pub conversation_id: String,
    /// 消息内容（支持多部分）
    pub parts: Vec<MessagePart>,
    /// 上下文元数据
    pub context: MessageContext,
    /// 请求选项
    pub options: MessageOptions,
}

/// 消息部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePart {
    /// 文本
    Text { content: String },
    /// 图像
    Image {
        data: Vec<u8>,
        format: ImageFormat,
        description: Option<String>,
    },
    /// 音频
    Audio {
        data: Vec<u8>,
        format: AudioFormat,
        sample_rate: u32,
        channels: u16,
    },
    /// 视频
    Video {
        data: Vec<u8>,
        format: VideoFormat,
        duration: Duration,
        resolution: (u32, u32),
    },
    /// 文件引用
    FileRef {
        path: String,
        mime_type: String,
    },
    /// 结构化数据
    Structured { data: serde_json::Value },
}

/// 图像格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Bmp,
    Gif,
    Svg,
    RawRGBA,
}

/// 音频格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Wav,
    Mp3,
    OggVorbis,
    Flac,
    Aac,
    PcmF32,
    PcmS16,
}

/// 视频格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFormat {
    Mp4,
    WebM,
    Avi,
    Mov,
    RawFrames,
}

/// 消息上下文
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageContext {
    /// 对话历史长度
    pub history_length: usize,
    /// 系统提示
    pub system_prompt: Option<String>,
    /// 用户偏好
    pub user_preferences: HashMap<String, String>,
    /// 当前应用上下文
    pub app_context: Option<AppContext>,
    /// 地理位置信息
    pub location: Option<LocationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    pub app_id: String,
    pub window_title: String,
    pub selected_text: Option<String>,
    pub clipboard_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f32,
}

/// 消息选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageOptions {
    /// 流式响应
    pub stream: bool,
    /// 最大 token 数
    pub max_tokens: u32,
    /// 温度
    pub temperature: f32,
    /// 推理偏好
    pub inference_preference: InferencePreference,
    /// 超时时间
    pub timeout: Duration,
    /// 指定响应语言
    pub response_language: Option<String>,
}

impl Default for MessageOptions {
    fn default() -> Self {
        Self {
            stream: false,
            max_tokens: 4096,
            temperature: 0.7,
            inference_preference: InferencePreference::Auto,
            timeout: Duration::from_secs(30),
            response_language: None,
        }
    }
}

/// 多模态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModalResponse {
    /// 响应 ID
    pub response_id: String,
    /// 响应内容
    pub parts: Vec<ResponsePart>,
    /// Token 使用统计
    pub usage: TokenUsage,
    /// 推理来源
    pub inference_source: InferenceSource,
    /// 延迟
    pub latency: Duration,
    /// 置信度
    pub confidence: f64,
}

/// 响应部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePart {
    Text { content: String, markdown: bool },
    Image { data: Vec<u8>, format: ImageFormat },
    Audio { data: Vec<u8>, format: AudioFormat },
    Video { data: Vec<u8>, format: VideoFormat },
    Structured { data: serde_json::Value },
    /// Agent 动作建议
    ActionSuggestion {
        actions: Vec<SuggestedAction>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub id: String,
    pub label: String,
    pub description: String,
    pub action_type: String,
    pub parameters: HashMap<String, String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InferenceSource {
    Local,
    Cloud,
    Hybrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 统一对话 trait
pub trait UnifiedDialog: Send + Sync {
    /// 发送多模态消息并获取响应
    fn send_message(&self, message: MultiModalMessage) -> Result<MultiModalResponse, MultimodalError>;

    /// 发送消息并获取流式响应
    fn send_message_stream(
        &self,
        message: MultiModalMessage,
    ) -> Result<StreamHandle, MultimodalError>;
}

/// 流式响应句柄
pub struct StreamHandle {
    pub id: String,
}

impl StreamHandle {
    /// 获取下一个响应片段（阻塞）
    pub fn next_chunk(&self) -> Result<Option<ResponsePart>, MultimodalError> {
        todo!("流式响应实现")
    }

    /// 异步获取下一个响应片段
    pub async fn next_chunk_async(&self) -> Result<Option<ResponsePart>, MultimodalError> {
        todo!("异步流式响应实现")
    }

    /// 取消流式响应
    pub fn cancel(&self) -> Result<(), MultimodalError> {
        todo!("取消流式响应")
    }
}
```

---

## 3. 语音 API

### 3.1 语音识别与合成

```rust
/// 语音识别结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// 识别文本
    pub text: String,
    /// 置信度
    pub confidence: f64,
    /// 时间戳列表
    pub timestamps: Vec<WordTimestamp>,
    /// 语言检测
    pub detected_language: String,
    /// 说话人分离
    pub speakers: Option<Vec<SpeakerSegment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTimestamp {
    pub word: String,
    pub start_time: Duration,
    pub end_time: Duration,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerSegment {
    pub speaker_id: String,
    pub start_time: Duration,
    pub end_time: Duration,
    pub text: String,
}

/// 语音合成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechSynthesisConfig {
    /// 语音 ID
    pub voice_id: String,
    /// 语速 (0.5 - 2.0)
    pub speed: f32,
    /// 音调 (0.5 - 2.0)
    pub pitch: f32,
    /// 音量 (0.0 - 1.0)
    pub volume: f32,
    /// 情感
    pub emotion: Option<SpeechEmotion>,
    /// 输出格式
    pub output_format: AudioFormat,
    /// 采样率
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeechEmotion {
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Calm,
}

/// 语音流
pub struct VoiceStream {
    pub stream_id: String,
}

/// 语音引擎 trait
pub trait VoiceEngine: Send + Sync {
    /// 语音转文本
    fn transcribe_audio(
        &self,
        audio_data: &[u8],
        format: AudioFormat,
        language: Option<&str>,
    ) -> Result<TranscriptionResult, MultimodalError>;

    /// 文本转语音
    fn synthesize_speech(
        &self,
        text: &str,
        config: SpeechSynthesisConfig,
    ) -> Result<Vec<u8>, MultimodalError>;

    /// 开始语音流（实时识别）
    fn start_voice_stream(&self, config: VoiceStreamConfig) -> Result<VoiceStream, MultimodalError>;

    /// 停止语音流
    fn stop_voice_stream(&self, stream_id: &str) -> Result<TranscriptionResult, MultimodalError>;

    /// 列出可用语音
    fn list_voices(&self) -> Result<Vec<VoiceInfo>, MultimodalError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceStreamConfig {
    pub format: AudioFormat,
    pub sample_rate: u32,
    pub channels: u16,
    pub language: Option<String>,
    pub enable_speaker_diarization: bool,
    pub enable_punctuation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub voice_id: String,
    pub name: String,
    pub language: String,
    pub gender: String,
    pub preview_url: Option<String>,
}
```

---

## 4. 文本 API

### 4.1 自然语言理解与生成

```rust
/// 意图理解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentResult {
    /// 意图标签
    pub intent: String,
    /// 置信度
    pub confidence: f64,
    /// 提取的实体
    pub entities: Vec<Entity>,
    /// 情感分析
    pub sentiment: Sentiment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_type: String,
    pub value: String,
    pub start: usize,
    pub end: usize,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentiment {
    pub label: SentimentLabel,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentLabel {
    Positive,
    Negative,
    Neutral,
    Mixed,
}

/// 文本生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextGenerationConfig {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub repetition_penalty: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
}

impl Default for TextGenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.7,
            top_p: Some(0.9),
            top_k: None,
            stop_sequences: Vec::new(),
            repetition_penalty: 1.1,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        }
    }
}

/// 摘要结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryResult {
    pub summary: String,
    pub key_points: Vec<String>,
    pub original_length: usize,
    pub summary_length: usize,
    pub compression_ratio: f64,
}

/// 翻译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub translated_text: String,
    pub source_language: String,
    pub target_language: String,
    pub confidence: f64,
    pub alternatives: Vec<String>,
}

/// 代码分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAnalysisResult {
    pub language: String,
    pub description: String,
    pub functions: Vec<FunctionInfo>,
    pub complexity: CodeComplexity,
    pub issues: Vec<CodeIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub signature: String,
    pub description: String,
    pub parameters: Vec<ParameterInfo>,
    pub return_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    pub name: String,
    pub type_: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeComplexity {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub lines_of_code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIssue {
    pub severity: IssueSeverity,
    pub message: String,
    pub line: Option<usize>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// 文本引擎 trait
pub trait TextEngine: Send + Sync {
    /// 意图理解
    fn understand_intent(&self, text: &str, context: Option<&str>) -> Result<IntentResult, MultimodalError>;

    /// 文本生成
    fn generate_text(&self, prompt: &str, config: TextGenerationConfig) -> Result<String, MultimodalError>;

    /// 文本摘要
    fn summarize(&self, text: &str, max_length: Option<usize>) -> Result<SummaryResult, MultimodalError>;

    /// 翻译
    fn translate(&self, text: &str, target_language: &str) -> Result<TranslationResult, MultimodalError>;

    /// 代码分析
    fn analyze_code(&self, code: &str, language: Option<&str>) -> Result<CodeAnalysisResult, MultimodalError>;
}
```

---

## 5. 图像 API

### 5.1 图像理解与生成

```rust
/// 图像描述结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDescription {
    pub description: String,
    pub confidence: f64,
    pub objects: Vec<DetectedObject>,
    pub scene: String,
    pub colors: Vec<ColorInfo>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedObject {
    pub label: String,
    pub confidence: f64,
    pub bounding_box: BoundingBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorInfo {
    pub color: String, // 十六进制
    pub percentage: f64,
}

/// 图像问答结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageQAResult {
    pub answer: String,
    pub confidence: f64,
    pub reasoning: Option<String>,
}

/// OCR 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub blocks: Vec<TextBlock>,
    pub language: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
    pub bounding_box: BoundingBox,
    pub confidence: f64,
    pub block_type: TextBlockType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextBlockType {
    Paragraph,
    Heading,
    Caption,
    List,
    Table,
    Code,
}

/// 图像生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationConfig {
    pub width: u32,
    pub height: u32,
    pub num_images: u32,
    pub steps: u32,
    pub guidance_scale: f32,
    pub seed: Option<u64>,
    pub negative_prompt: Option<String>,
    pub style: Option<ImageStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageStyle {
    Photorealistic,
    Illustration,
    Anime,
    OilPainting,
    Watercolor,
    PixelArt,
    Sketch,
    None,
}

/// 图像分割结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationResult {
    pub masks: Vec<SegmentMask>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMask {
    pub label: String,
    pub confidence: f64,
    pub mask_data: Vec<u8>, // RLE 或二进制掩码
    pub bounding_box: BoundingBox,
}

/// 屏幕理解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenUnderstanding {
    pub description: String,
    pub ui_elements: Vec<UiElement>,
    pub accessibility_tree: Option<serde_json::Value>,
    pub interactive_elements: Vec<InteractiveElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    pub element_type: String,
    pub label: String,
    pub bounding_box: BoundingBox,
    pub text_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    pub element_type: String,
    pub label: String,
    pub action: String,
    pub bounding_box: BoundingBox,
}

/// 图像引擎 trait
pub trait ImageEngine: Send + Sync {
    /// 描述图像内容
    fn describe_image(&self, image_data: &[u8], format: ImageFormat) -> Result<ImageDescription, MultimodalError>;

    /// 图像问答
    fn answer_question(&self, image_data: &[u8], format: ImageFormat, question: &str) -> Result<ImageQAResult, MultimodalError>;

    /// OCR 文字识别
    fn recognize_text(&self, image_data: &[u8], format: ImageFormat) -> Result<OcrResult, MultimodalError>;

    /// 生成图像
    fn generate_image(&self, prompt: &str, config: ImageGenerationConfig) -> Result<Vec<Vec<u8>>, MultimodalError>;

    /// 图像分割
    fn segment_image(&self, image_data: &[u8], format: ImageFormat) -> Result<SegmentationResult, MultimodalError>;

    /// 屏幕理解
    fn understand_screen(&self, screenshot_data: &[u8], format: ImageFormat) -> Result<ScreenUnderstanding, MultimodalError>;
}
```

---

## 6. 视频 API

### 6.1 视频分析与生成

```rust
/// 视频分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoAnalysisResult {
    pub description: String,
    pub duration: Duration,
    pub scenes: Vec<VideoScene>,
    pub objects: Vec<TrackedObject>,
    pub transcript: Option<TranscriptionResult>,
    pub key_frames: Vec<KeyFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoScene {
    pub start_time: Duration,
    pub end_time: Duration,
    pub description: String,
    pub scene_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedObject {
    pub label: String,
    pub track: Vec<ObjectTrackPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectTrackPoint {
    pub timestamp: Duration,
    pub bounding_box: BoundingBox,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyFrame {
    pub timestamp: Duration,
    pub frame_data: Vec<u8>,
    pub description: String,
}

/// 视频摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSummary {
    pub summary: String,
    pub highlights: Vec<VideoHighlight>,
    pub chapters: Vec<VideoChapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoHighlight {
    pub start_time: Duration,
    pub end_time: Duration,
    pub description: String,
    pub importance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoChapter {
    pub title: String,
    pub start_time: Duration,
    pub end_time: Duration,
    pub summary: String,
}

/// 视频生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoGenerationConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: Duration,
    pub format: VideoFormat,
    pub quality: VideoQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoQuality {
    Low,
    Medium,
    High,
    Ultra,
}

/// 视频流处理器
pub struct VideoStreamProcessor {
    pub processor_id: String,
}

/// 视频引擎 trait
pub trait VideoEngine: Send + Sync {
    /// 分析视频
    fn analyze_video(&self, video_data: &[u8], format: VideoFormat) -> Result<VideoAnalysisResult, MultimodalError>;

    /// 视频摘要
    fn summarize_video(&self, video_data: &[u8], format: VideoFormat) -> Result<VideoSummary, MultimodalError>;

    /// 生成视频
    fn generate_video(&self, prompt: &str, config: VideoGenerationConfig) -> Result<Vec<u8>, MultimodalError>;

    /// 处理视频流
    fn process_stream(&self, config: VideoStreamConfig) -> Result<VideoStreamProcessor, MultimodalError>;

    /// 编辑视频
    fn edit_video(&self, video_data: &[u8], format: VideoFormat, edits: Vec<VideoEdit>) -> Result<Vec<u8>, MultimodalError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStreamConfig {
    pub input_format: VideoFormat,
    pub operations: Vec<String>,
    pub output_format: VideoFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VideoEdit {
    Trim { start: Duration, end: Duration },
    Concat { other_video: Vec<u8> },
    OverlayText { text: String, position: (f32, f32), font_size: u32 },
    AddSubtitle { srt_data: String },
    AdjustSpeed { factor: f32 },
    ExtractAudio,
}
```

---

## 7. 跨模态 API

### 7.1 模态转换与对齐

```rust
/// 模态对齐结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalityAlignment {
    pub source_modality: Modality,
    pub target_modality: Modality,
    pub alignment_score: f64,
    pub correspondences: Vec<Correspondence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correspondence {
    pub source_segment: String,
    pub target_segment: String,
    pub confidence: f64,
    pub timestamp_source: Option<Duration>,
    pub timestamp_target: Option<Duration>,
}

/// 模态转换结果
#[derive(Debug, Clone)]
pub struct ModalityConversion {
    pub source_modality: Modality,
    pub target_modality: Modality,
    pub output_data: Vec<u8>,
    pub output_format: String,
    pub confidence: f64,
}

/// 跨模态引擎 trait
pub trait CrossModalEngine: Send + Sync {
    /// 对齐不同模态的内容
    fn align_modalities(
        &self,
        source: &[u8],
        source_modality: Modality,
        target: &[u8],
        target_modality: Modality,
    ) -> Result<ModalityAlignment, MultimodalError>;

    /// 模态转换
    fn convert_modality(
        &self,
        input: &[u8],
        from: Modality,
        to: Modality,
        config: serde_json::Value,
    ) -> Result<ModalityConversion, MultimodalError>;
}
```

---

## 8. 模型管理 API

### 8.1 模型配置与选择

```rust
/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model_id: String,
    pub name: String,
    pub version: String,
    pub modality: Vec<Modality>,
    pub model_type: ModelType,
    pub size_bytes: u64,
    pub parameters: u64,
    pub quantization: Option<String>,
    pub max_context_length: u32,
    pub supported_features: Vec<String>,
    pub is_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelType {
    /// 语言模型
    Language,
    /// 视觉模型
    Vision,
    /// 音频模型
    Audio,
    /// 多模态模型
    MultiModal,
    /// 嵌入模型
    Embedding,
    /// 扩散模型
    Diffusion,
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfiguration {
    pub model_id: String,
    pub inference_preference: InferencePreference,
    pub gpu_layers: Option<u32>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub thread_count: Option<u32>,
}

/// 模型管理器 trait
pub trait ModelManager: Send + Sync {
    /// 列出所有可用模型
    fn list_models(&self, modality: Option<Modality>) -> Result<Vec<ModelInfo>, MultimodalError>;

    /// 配置模型
    fn configure_model(&self, config: ModelConfiguration) -> Result<(), MultimodalError>;

    /// 设置推理偏好
    fn set_preference(&self, preference: InferencePreference) -> Result<(), MultimodalError>;

    /// 加载模型
    fn load_model(&self, model_id: &str) -> Result<(), MultimodalError>;

    /// 卸载模型
    fn unload_model(&self, model_id: &str) -> Result<(), MultimodalError>;

    /// 获取模型信息
    fn get_model_info(&self, model_id: &str) -> Result<ModelInfo, MultimodalError>;
}
```

---

## 9. 云端提供商 API

### 9.1 云端推理配置

```rust
/// 云端提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProviderConfig {
    pub provider: CloudProvider,
    pub api_key: String,
    pub api_base: Option<String>,
    pub organization_id: Option<String>,
    pub default_model: String,
    pub max_concurrent_requests: u32,
    pub timeout: Duration,
    pub retry_config: RetryConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudProvider {
    OpenAI,
    Anthropic,
    Google,
    Mistral,
    Azure,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// 云端推理管理 trait
pub trait CloudProviderManager: Send + Sync {
    /// 配置云端提供商
    fn configure_provider(&self, config: CloudProviderConfig) -> Result<(), MultimodalError>;

    /// 执行云端推理
    fn inference_cloud(&self, request: InferenceRequest) -> Result<InferenceResponse, MultimodalError>;

    /// 测试提供商连接
    fn test_connection(&self, provider: CloudProvider) -> Result<bool, MultimodalError>;

    /// 获取提供商状态
    fn get_provider_status(&self) -> Result<ProviderStatus, MultimodalError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    pub finish_reason: String,
    pub latency: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider: CloudProvider,
    pub connected: bool,
    pub latency_ms: f64,
    pub available_models: Vec<String>,
    pub rate_limit_remaining: Option<u32>,
}
```

---

## 10. 错误处理

```rust
/// 多模态错误类型
#[derive(Debug, thiserror::Error)]
pub enum MultimodalError {
    #[error("不支持的模态: {0}")]
    UnsupportedModality(String),

    #[error("不支持的格式: {0}")]
    UnsupportedFormat(String),

    #[error("模型未加载: {0}")]
    ModelNotLoaded(String),

    #[error("模型不存在: {0}")]
    ModelNotFound(String),

    #[error("推理失败: {0}")]
    InferenceFailed(String),

    #[error("云端推理失败: {0}")]
    CloudInferenceFailed(String),

    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("API 密钥无效: {provider}")]
    InvalidApiKey { provider: String },

    #[error("速率限制: {provider}, 重试于 {retry_after:?}")]
    RateLimited { provider: String, retry_after: Duration },

    #[error("输入数据无效: {0}")]
    InvalidInput(String),

    #[error("输出数据无效: {0}")]
    InvalidOutput(String),

    #[error("超时: {0}")]
    Timeout(String),

    #[error("音频处理失败: {0}")]
    AudioProcessingFailed(String),

    #[error("图像处理失败: {0}")]
    ImageProcessingFailed(String),

    #[error("视频处理失败: {0}")]
    VideoProcessingFailed(String),

    #[error("流已关闭")]
    StreamClosed,
}
```

---

## 11. 使用示例

### 11.1 统一对话

```rust
use multimodal_api::*;

async fn dialog_example() -> Result<(), Box<dyn std::error::Error>> {
    let dialog = UnifiedDialogImpl::new();

    // 构建多模态消息
    let message = MultiModalMessage {
        conversation_id: "conv-123".to_string(),
        parts: vec![
            MessagePart::Text {
                content: "描述这张图片中的内容，并用中文回答".to_string(),
            },
            MessagePart::Image {
                data: std::fs::read("/path/to/image.png")?,
                format: ImageFormat::Png,
                description: None,
            },
        ],
        context: MessageContext {
            system_prompt: Some("你是一个专业的图像分析助手。".to_string()),
            response_language: Some("zh-CN".to_string()),
            ..Default::default()
        },
        options: MessageOptions {
            stream: false,
            max_tokens: 1024,
            temperature: 0.3,
            inference_preference: InferencePreference::Auto,
            timeout: Duration::from_secs(30),
            response_language: Some("zh-CN".to_string()),
        },
    };

    let response = dialog.send_message(message)?;
    println!("响应: {:?}", response.parts);

    Ok(())
}
```

### 11.2 语音交互

```rust
async fn voice_example() -> Result<(), Box<dyn std::error::Error>> {
    let voice = VoiceEngineImpl::new();

    // 语音识别
    let audio_data = std::fs::read("/path/to/speech.wav")?;
    let result = voice.transcribe_audio(&audio_data, AudioFormat::Wav, Some("zh-CN"))?;
    println!("识别结果: {}", result.text);

    // 语音合成
    let config = SpeechSynthesisConfig {
        voice_id: "zh-female-1".to_string(),
        speed: 1.0,
        pitch: 1.0,
        volume: 0.8,
        emotion: Some(SpeechEmotion::Happy),
        output_format: AudioFormat::Wav,
        sample_rate: 22050,
    };
    let audio = voice.synthesize_speech("你好，欢迎使用 OmniAgent OS", config)?;
    std::fs::write("/tmp/greeting.wav", audio)?;

    Ok(())
}
```

### 11.3 图像生成

```rust
async fn image_generation_example() -> Result<(), Box<dyn std::error::Error>> {
    let image_engine = ImageEngineImpl::new();

    let config = ImageGenerationConfig {
        width: 1024,
        height: 1024,
        num_images: 1,
        steps: 50,
        guidance_scale: 7.5,
        seed: Some(42),
        negative_prompt: Some("blurry, low quality".to_string()),
        style: Some(ImageStyle::Photorealistic),
    };

    let images = image_engine.generate_image(
        "A futuristic city at sunset with flying cars, cyberpunk style",
        config,
    )?;

    for (i, image_data) in images.iter().enumerate() {
        let path = format!("/tmp/generated_{}.png", i);
        std::fs::write(&path, image_data)?;
        println!("图像已保存: {}", path);
    }

    Ok(())
}
```

---

## 12. 性能约束

| 操作 | 延迟目标 | 吞吐量目标 | 说明 |
|------|---------|-----------|------|
| transcribe_audio (10s) | <2s | 5/s | 本地模型 |
| synthesize_speech | <500ms | 10/s | 本地模型 |
| understand_intent | <100ms | 50/s | 轻量级模型 |
| generate_text (500 tokens) | <2s | 5/s | 7B 本地模型 |
| summarize (10k chars) | <3s | 3/s | 本地模型 |
| translate (1k chars) | <1s | 10/s | 本地模型 |
| describe_image | <1s | 5/s | 视觉模型 |
| recognize_text (OCR) | <500ms | 10/s | 本地模型 |
| generate_image (1024x1024) | <10s | 0.5/s | 扩散模型 |
| analyze_video (1min) | <30s | 0.1/s | 含帧采样 |
| inference_cloud | <5s | 取决于网络 | 云端 API |

---

## 13. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_part_text() {
        let part = MessagePart::Text {
            content: "Hello".to_string(),
        };
        if let MessagePart::Text { content } = part {
            assert_eq!(content, "Hello");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_message_options_default() {
        let opts = MessageOptions::default();
        assert_eq!(opts.max_tokens, 4096);
        assert!((opts.temperature - 0.7).abs() < 0.01);
        assert!(!opts.stream);
    }

    #[test]
    fn test_image_formats() {
        let formats = [
            ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP,
            ImageFormat::Bmp, ImageFormat::Gif, ImageFormat::Svg,
        ];
        assert_eq!(formats.len(), 6);
    }

    #[test]
    fn test_audio_formats() {
        let formats = [
            AudioFormat::Wav, AudioFormat::Mp3, AudioFormat::OggVorbis,
            AudioFormat::Flac, AudioFormat::Aac, AudioFormat::PcmF32, AudioFormat::PcmS16,
        ];
        assert_eq!(formats.len(), 7);
    }

    #[test]
    fn test_text_generation_config_default() {
        let config = TextGenerationConfig::default();
        assert_eq!(config.max_tokens, 2048);
        assert_eq!(config.repetition_penalty, 1.1);
    }

    #[test]
    fn test_sentiment_labels() {
        let labels = [
            SentimentLabel::Positive,
            SentimentLabel::Negative,
            SentimentLabel::Neutral,
            SentimentLabel::Mixed,
        ];
        assert_eq!(labels.len(), 4);
    }

    #[test]
    fn test_image_generation_config() {
        let config = ImageGenerationConfig {
            width: 512,
            height: 512,
            num_images: 2,
            steps: 30,
            guidance_scale: 7.5,
            seed: Some(123),
            negative_prompt: None,
            style: None,
        };
        assert_eq!(config.num_images, 2);
        assert_eq!(config.seed, Some(123));
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_modality_values() {
        let modalities = [Modality::Text, Modality::Image, Modality::Audio, Modality::Video];
        assert_eq!(modalities.len(), 4);
    }

    #[test]
    fn test_inference_source() {
        let sources = [InferenceSource::Local, InferenceSource::Cloud, InferenceSource::Hybrid];
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn test_video_quality() {
        let qualities = [VideoQuality::Low, VideoQuality::Medium, VideoQuality::High, VideoQuality::Ultra];
        assert_eq!(qualities.len(), 4);
    }

    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox { x: 10.0, y: 20.0, width: 100.0, height: 50.0 };
        assert_eq!(bbox.x, 10.0);
        assert_eq!(bbox.width, 100.0);
    }
}
```

---

*本文档为 OmniAgent OS 多模态交互 API 参考，版本 0.1.0。*
