# OmniAgent OS — 多模态交互服务模块规格说明

> **模块编号**: OA-MMI-002 | **版本**: v1.0.0-draft | **状态**: 设计中
> **依赖**: 内核调度器 (OA-KRN-002)、GPU 资源管理 (OA-GPU-005)、模型运行时 (OA-MRT-006)

## 1. 概述

多模态交互服务是 OmniAgent OS 的核心人机交互层，提供语音、文本、图像、视频四种模态的输入输出能力。采用双通道 AI 模型架构——本地模型 (Candle/ort) 保障隐私和低延迟，云端 API (OpenAI/Anthropic) 提供高性能推理——由智能路由器根据任务需求自动选择。

### 1.1 设计原则

| 原则 | 说明 |
|------|------|
| 模态无关 | 用户无需关心输入模态，系统自动检测并路由 |
| 隐私优先 | 敏感数据优先使用本地模型，云端调用需用户授权 |
| 流式优先 | 所有生成类操作支持流式输出，降低首字延迟 |
| 渐进增强 | 本地模型提供基础能力，云端模型提供增强能力 |

### 1.2 架构总览

```
┌─────────────────────────────────────────────────────────┐
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐           │
│  │ 语音   │ │ 文本   │ │ 图像   │ │ 视频   │           │
│  └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘           │
│      └──────────┴──────────┴──────────┘                 │
│                        │                                │
│              ┌─────────▼─────────┐                       │
│              │   模态路由器       │                       │
│              └─────────┬─────────┘                       │
│              ┌─────────▼─────────┐                       │
│              │   模型路由器       │                       │
│              └────┬────────┬────┘                        │
│          ┌───────▼──┐ ┌──▼────────┐                      │
│          │ 本地模型  │ │ 云端 API   │                      │
│          │(Candle/  │ │(OpenAI/   │                      │
│          │ ort)     │ │ Anthropic)│                      │
│          └──────────┘ └──────────┘                      │
│              ┌─────────▼─────────┐                       │
│              │  跨模态融合引擎    │                       │
│              └───────────────────┘                       │
└─────────────────────────────────────────────────────────┘
```

### 1.3 性能约束

| 模态 | 操作 | 目标延迟 | 备注 |
|------|------|----------|------|
| 语音 | ASR 流式识别 | < 300ms | 首 token 延迟 |
| 语音 | TTS 流式合成 | < 200ms | 首 token 延迟 |
| 文本 | NLU 意图识别 | < 100ms | 本地模型 |
| 文本 | NLG 文本生成 | < 500ms | 首 token 延迟 |
| 图像 | VQA 问答 | < 2s | 本地 Moondream2 |
| 图像 | OCR 识别 | < 3s | 单页文档 |
| 图像 | 图像生成 | < 30s | 512x512, 本地 SD1.5 |
| 视频 | 关键帧提取 | < 1s/帧 | 1080p 输入 |
| 跨模态 | CLIP 对齐 | < 500ms | 单对比较 |

---

## 2. 双通道 AI 模型架构

### 2.1 模型路由器

```rust
pub trait ModelRouter: Send + Sync {
    fn select_model(&self, request: &ModelRequest) -> ModelSelection;
    fn availability(&self, model_id: &ModelId) -> ModelAvailability;
    fn set_preference(&self, preference: ModelPreference);
    fn stats(&self) -> RouterStats;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub task_type: TaskType, pub modality: Modality,
    pub quality_requirement: QualityLevel, pub latency_requirement: Duration,
    pub privacy_level: PrivacyLevel, pub cost_budget: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType { Recognition, Generation, Understanding, Analysis, Conversion }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modality { Voice, Text, Image, Video, MultiModal }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityLevel { Draft, Standard, High, Maximum }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyLevel { Public, Internal, Sensitive, Confidential }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelPreference { LocalOnly, CloudOnly, Auto, Specific(ModelId), Fallback { primary: ModelId, fallback: ModelId } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub model_id: ModelId, pub channel: ModelChannel, pub reason: SelectionReason,
    pub estimated_latency: Duration, pub estimated_cost: f64, pub quality_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectionReason { PrivacyRequired, LatencyOptimal, QualityOptimal, CostOptimal, UserPreference, Fallback }
```

### 2.2 模型配置表

| 模型 | 用途 | 运行时 | 量化 | 显存 | 推理延迟 |
|------|------|--------|------|------|----------|
| Whisper-large-v3 | ASR | candle | int8 | 1.5 GB | ~200ms |
| Piper TTS | TTS | ort | int8 | 64 MB | ~100ms |
| Parler-TTS | TTS (高质量) | candle | fp16 | 2.0 GB | ~500ms |
| Phi-3-mini | NLU | candle | int4 | 2.0 GB | ~50ms |
| NLLB-200 | 翻译 | candle | int8 | 1.2 GB | ~300ms |
| Moondream2 | VQA | candle | int4 | 1.5 GB | ~1.5s |
| PaddleOCR | OCR | ort | int8 | 256 MB | ~2s |
| SD 1.5 | 图像生成 | candle | fp16 | 4.0 GB | ~25s |
| SAM-vit-b | 分割 | candle | int8 | 1.0 GB | ~500ms |
| YOLOv8-nano | 检测 | ort | int8 | 128 MB | ~30ms |
| CLIP-vit-base | 对齐 | candle | int8 | 512 MB | ~200ms |
| AnimateDiff | 视频生成 | candle | fp16 | 6.0 GB | ~60s |

---

## 3. 语音模态

### 3.1 ASR — 自动语音识别

```rust
pub trait AsrEngine: Send + Sync {
    fn recognize_stream(&self, config: &AsrConfig) -> (AsrStreamSender, AsrStreamReceiver);
    fn recognize(&self, audio: &AudioInput, config: &AsrConfig) -> impl Future<Output = Result<AsrResult, AsrError>> + Send;
    fn supported_languages(&self) -> Vec<LanguageInfo>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    pub language: Option<String>, pub model_size: AsrModelSize,
    pub beam_size: u32, pub temperature: f32, pub word_timestamps: bool,
    pub hotwords: Vec<String>, pub max_audio_duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AsrModelSize { Tiny, Base, Small, Medium, Large, LargeV3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrResult {
    pub text: String, pub language: String, pub segments: Vec<AsrSegment>,
    pub confidence: f32, pub processing_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrSegment {
    pub start: Duration, pub end: Duration, pub text: String,
    pub confidence: f32, pub word_timestamps: Vec<WordTimestamp>,
}

pub type AsrStreamSender = tokio::sync::mpsc::Sender<AudioChunk>;
pub type AsrStreamReceiver = tokio::sync::mpsc::Receiver<AsrPartialResult>;
```

### 3.2 TTS — 文本转语音

```rust
pub trait TtsEngine: Send + Sync {
    fn synthesize_stream(&self, text: &str, config: &TtsConfig) -> (TtsStreamSender, TtsStreamReceiver);
    fn synthesize(&self, text: &str, config: &TtsConfig) -> impl Future<Output = Result<TtsResult, TtsError>> + Send;
    fn available_voices(&self) -> Vec<VoiceInfo>;
    fn clone_voice(&self, samples: &AudioInput) -> Result<VoiceId, TtsError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    pub voice_id: Option<VoiceId>, pub language: String,
    pub speed: f32, pub pitch: f32, pub emotion: Option<Emotion>,
    pub output_format: AudioFormat, pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceInfo { pub id: VoiceId, pub name: String, pub language: String, pub quality: VoiceQuality, pub is_neural: bool }
```

### 3.3 VAD — 语音活动检测

```rust
pub trait VadEngine: Send + Sync {
    fn process_frame(&self, frame: &[f32], config: &VadConfig) -> VadResult;
    fn stream(&self, config: &VadConfig) -> (VadSender, VadReceiver);
    fn segment(&self, audio: &AudioInput, config: &VadConfig) -> Vec<VoiceSegment>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub threshold: f32, pub min_speech_duration: Duration,
    pub min_silence_duration: Duration, pub sample_rate: u32,
}
```

### 3.4 声纹识别

```rust
pub trait VoiceprintEngine: Send + Sync {
    fn enroll(&self, user_id: &UserId, audio_samples: &[AudioInput]) -> Result<(), VoiceprintError>;
    fn verify(&self, audio: &AudioInput) -> Result<VoiceprintMatch, VoiceprintError>;
    fn identify(&self, audio: &AudioInput) -> Result<Vec<CandidateMatch>, VoiceprintError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceprintMatch { pub user_id: UserId, pub confidence: f32, pub is_verified: bool }
```

### 3.5 语音管线

```
麦克风 → VAD(实时) → 降噪(RNNoise) → ASR(Whisper流式) → NLU(Phi-3) → Agent → TTS(Piper流式) → 扬声器
```

### 3.6 语音测试用例

```rust
#[cfg(test)]
mod voice_tests {
    #[tokio::test]
    async fn test_asr_basic() {
        let asr = create_test_asr();
        let result = asr.recognize(&load_audio("zh_hello.wav"), &AsrConfig { language: Some("zh".into()), ..Default::default() }).await.unwrap();
        assert!(!result.text.is_empty() && result.confidence > 0.8);
    }

    #[tokio::test]
    async fn test_tts_first_token_latency() {
        let tts = create_test_tts();
        let (_tx, mut rx) = tts.synthesize_stream("测试", &TtsConfig::default());
        let start = Instant::now();
        let first = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(first.is_ok() && start.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn test_vad_segmentation() {
        let segments = create_test_vad().segment(&load_audio("conversation.wav"), &VadConfig::default());
        assert!(!segments.is_empty());
        for i in 1..segments.len() { assert!(segments[i].start >= segments[i-1].end); }
    }

    #[tokio::test]
    async fn test_voiceprint_verify() {
        let vp = create_test_voiceprint();
        vp.enroll(&UserId::new("user1"), &load_samples("user1", 5)).unwrap();
        let result = vp.verify(&load_audio("user1_test.wav")).unwrap();
        assert!(result.is_verified && result.confidence > 0.9);
    }
}
```

---

## 4. 文本模态

### 4.1 NLU — 自然语言理解

```rust
pub trait NluEngine: Send + Sync {
    fn understand(&self, text: &str, context: &NluContext) -> impl Future<Output = Result<NluResult, NluError>> + Send;
    fn recognize_intent(&self, text: &str) -> Result<Intent, NluError>;
    fn extract_entities(&self, text: &str, schema: &EntitySchema) -> Result<Vec<Entity>, NluError>;
    fn update_context(&self, context: &mut NluContext, result: &NluResult);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NluResult {
    pub intent: Intent, pub entities: Vec<Entity>, pub sentiment: Sentiment,
    pub confidence: f32, pub alternatives: Vec<IntentCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent { pub name: String, pub confidence: f32, pub action: Option<String>, pub parameters: HashMap<String, Value> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity { pub entity_type: String, pub value: String, pub start: usize, pub end: usize, pub confidence: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType { Text, Number, Date, Time, DateTime, Duration, Boolean, Enum(Vec<String>), Custom(String) }
```

### 4.2 NLG — 自然语言生成

```rust
pub trait NlgEngine: Send + Sync {
    fn generate_stream(&self, prompt: &NlgPrompt) -> impl Stream<Item = Result<NlgChunk, NlgError>> + Send;
    fn generate(&self, prompt: &NlgPrompt) -> impl Future<Output = Result<NlgResult, NlgError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlgPrompt {
    pub system_prompt: String, pub user_message: String, pub context: Vec<ChatMessage>,
    pub max_tokens: u32, pub temperature: f32, pub top_p: f32,
    pub stop_sequences: Vec<String>, pub format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat { PlainText, Markdown, Json, Xml, Code { language: String } }
```

### 4.3 摘要器与翻译器

```rust
pub trait Summarizer: Send + Sync {
    fn extractive(&self, document: &Document, config: &SummaryConfig) -> Result<Summary, SummaryError>;
    fn abstractive(&self, document: &Document, config: &SummaryConfig) -> impl Future<Output = Result<Summary, SummaryError>> + Send;
    fn hybrid(&self, document: &Document, config: &SummaryConfig) -> impl Future<Output = Result<Summary, SummaryError>> + Send;
}

pub trait Translator: Send + Sync {
    fn translate(&self, text: &str, source: &str, target: &str) -> impl Future<Output = Result<TranslationResult, TranslationError>> + Send;
    fn detect_language(&self, text: &str) -> Result<LanguageDetection, TranslationError>;
    fn supported_languages(&self) -> Vec<LanguageInfo>;
}
```

### 4.4 代码理解

```rust
pub trait CodeUnderstandingEngine: Send + Sync {
    fn parse_ast(&self, code: &str, language: &str) -> Result<AstNode, CodeError>;
    fn analyze_dependencies(&self, code: &str, language: &str) -> Result<DependencyGraph, CodeError>;
    fn summarize(&self, code: &str, language: &str) -> impl Future<Output = Result<CodeSummary, CodeError>> + Send;
}
```

### 4.5 文本测试用例

```rust
#[cfg(test)]
mod text_tests {
    #[tokio::test]
    async fn test_nlu_intent() {
        let nlu = create_test_nlu();
        let result = nlu.recognize_intent("帮我创建定时任务每天早上8点备份").unwrap();
        assert_eq!(result.name, "create_cron_job");
        assert!(result.confidence > 0.8);
    }

    #[tokio::test]
    async fn test_translation() {
        let result = create_test_translator().translate("Hello, world!", "en", "zh").await.unwrap();
        assert!(result.translated_text.contains("世界"));
    }

    #[test]
    fn test_ast_parsing() {
        let ast = create_test_code_engine().parse_ast("fn main() { println!(\"Hi\"); }", "rust").unwrap();
        assert_eq!(ast.kind, "source_file");
    }
}
```

---

## 5. 图像模态

### 5.1 VQA — 视觉问答

```rust
pub trait VqaEngine: Send + Sync {
    fn describe(&self, image: &ImageInput) -> impl Future<Output = Result<String, VqaError>> + Send;
    fn answer(&self, image: &ImageInput, question: &str) -> impl Future<Output = Result<VqaAnswer, VqaError>> + Send;
    fn chat(&self, image: &ImageInput, history: &[ChatMessage], question: &str) -> impl Future<Output = Result<VqaAnswer, VqaError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInput { pub data: ImageData, pub format: ImageFormat, pub width: u32, pub height: u32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageData { Rgb(Vec<u8>), Rgba(Vec<u8>), File(PathBuf), Url(String), Base64(String) }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageFormat { Png, Jpeg, Webp, Bmp, Gif }
```

### 5.2 OCR

```rust
pub trait OcrEngine: Send + Sync {
    fn recognize(&self, image: &ImageInput, config: &OcrConfig) -> impl Future<Output = Result<OcrResult, OcrError>> + Send;
    fn recognize_table(&self, image: &ImageInput) -> impl Future<Output = Result<TableResult, OcrError>> + Send;
    fn recognize_handwriting(&self, image: &ImageInput) -> impl Future<Output = Result<OcrResult, OcrError>> + Send;
    fn recognize_document(&self, pdf_path: &Path, config: &OcrConfig) -> impl Future<Output = Result<DocumentOcrResult, OcrError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    pub languages: Vec<String>, pub detect_orientation: bool,
    pub detect_tables: bool, pub detect_layout: bool, pub confidence_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult { pub text: String, pub blocks: Vec<TextBlock>, pub language: String, pub confidence: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock { pub text: String, pub bounding_box: BoundingBox, pub confidence: f32, pub block_type: TextBlockType }
```

### 5.3 图像生成

```rust
pub trait ImageGenerator: Send + Sync {
    fn text_to_image(&self, prompt: &str, config: &ImageGenConfig) -> impl Future<Output = Result<ImageOutput, ImageGenError>> + Send;
    fn image_to_image(&self, source: &ImageInput, prompt: &str, config: &ImageGenConfig) -> impl Future<Output = Result<ImageOutput, ImageGenError>> + Send;
    fn inpaint(&self, image: &ImageInput, mask: &ImageInput, prompt: &str, config: &ImageGenConfig) -> impl Future<Output = Result<ImageOutput, ImageGenError>> + Send;
    fn generate_stream(&self, prompt: &str, config: &ImageGenConfig) -> impl Stream<Item = Result<ImageGenProgress, ImageGenError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenConfig {
    pub width: u32, pub height: u32, pub steps: u32, pub guidance_scale: f32,
    pub seed: Option<u64>, pub sampler: SamplerType, pub negative_prompt: Option<String>,
}
```

### 5.4 分割与检测

```rust
pub trait SegmentationEngine: Send + Sync {
    fn segment(&self, image: &ImageInput, prompts: &[SegmentPrompt]) -> impl Future<Output = Result<SegmentationResult, SegError>> + Send;
    fn auto_segment(&self, image: &ImageInput) -> impl Future<Output = Result<SegmentationResult, SegError>> + Send;
    fn detect_objects(&self, image: &ImageInput, config: &DetectionConfig) -> impl Future<Output = Result<DetectionResult, SegError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SegmentPrompt { Point { x: f32, y: f32, label: PromptLabel }, Box { x: f32, y: f32, w: f32, h: f32 }, Text { description: String } }
```

### 5.5 屏幕理解

```rust
pub trait ScreenUnderstandingEngine: Send + Sync {
    fn analyze_screenshot(&self, screenshot: &ImageInput) -> impl Future<Output = Result<ScreenAnalysis, ScreenError>> + Send;
    fn recognize_elements(&self, screenshot: &ImageInput) -> impl Future<Output = Result<Vec<UiElement>, ScreenError>> + Send;
    fn suggest_actions(&self, screenshot: &ImageInput, goal: &str) -> impl Future<Output = Result<Vec<SuggestedAction>, ScreenError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiElementType { Button, TextField, Checkbox, Dropdown, Link, Image, Icon, Label, List, Table, Tab, Menu, Dialog, Window }
```

### 5.6 图像测试用例

```rust
#[cfg(test)]
mod image_tests {
    #[tokio::test]
    async fn test_vqa_describe() {
        let desc = create_test_vqa().describe(&load_image("cat.jpg")).await.unwrap();
        assert!(!desc.is_empty());
    }

    #[tokio::test]
    async fn test_ocr() {
        let result = create_test_ocr().recognize(&load_image("receipt.jpg"), &OcrConfig::default()).await.unwrap();
        assert!(!result.text.is_empty() && result.confidence > 0.7);
    }

    #[tokio::test]
    async fn test_image_gen() {
        let result = create_test_gen().text_to_image("橘猫坐在窗台上", &ImageGenConfig { width: 512, height: 512, steps: 20, ..Default::default() }).await.unwrap();
        assert!(!result.nsfw_detected);
    }

    #[tokio::test]
    async fn test_detection() {
        let result = create_test_seg().detect_objects(&load_image("street.jpg"), &DetectionConfig::default()).await.unwrap();
        assert!(!result.detections.is_empty());
    }
}
```

---

## 6. 视频模态

### 6.1 视频理解

```rust
pub trait VideoUnderstandingEngine: Send + Sync {
    fn extract_keyframes(&self, video: &VideoInput, config: &KeyframeConfig) -> impl Future<Output = Result<Vec<Keyframe>, VideoError>> + Send;
    fn analyze_frames(&self, video: &VideoInput, config: &FrameAnalysisConfig) -> impl Stream<Item = Result<FrameAnalysis, VideoError>> + Send;
    fn answer_question(&self, video: &VideoInput, question: &str) -> impl Future<Output = Result<VideoAnswer, VideoError>> + Send;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInput { pub source: VideoSource, pub format: VideoFormat, pub duration: Option<Duration>, pub resolution: Option<(u32, u32)> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VideoSource { File(PathBuf), Stream { url: String, protocol: StreamProtocol }, Camera { device_id: u32 }, Frames { frames: Vec<ImageInput>, fps: f32 } }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamProtocol { Rtsp, Rtmp, Hls, WebRTC }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeConfig { pub max_keyframes: u32, pub min_interval: Duration, pub scene_threshold: f32, pub method: KeyframeMethod }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyframeMethod { SceneDetection, Uniform, MotionBased, AiSelected }
```

### 6.2 视频摘要与生成

```rust
pub trait VideoSummarizer: Send + Sync {
    fn summarize(&self, video: &VideoInput, config: &VideoSummaryConfig) -> impl Future<Output = Result<VideoSummary, VideoError>> + Send;
    fn generate_timeline(&self, video: &VideoInput) -> impl Future<Output = Result<VideoTimeline, VideoError>> + Send;
}

pub trait VideoGenerator: Send + Sync {
    fn text_to_video(&self, prompt: &str, config: &VideoGenConfig) -> impl Future<Output = Result<VideoOutput, VideoError>> + Send;
    fn image_to_video(&self, image: &ImageInput, prompt: &str, config: &VideoGenConfig) -> impl Future<Output = Result<VideoOutput, VideoError>> + Send;
}
```

### 6.3 实时视频流处理

```rust
pub trait RealtimeStreamProcessor: Send + Sync {
    fn start(&self, source: &VideoSource, config: &StreamConfig) -> impl Future<Output = Result<StreamHandle, StreamError>> + Send;
    fn stop(&self, handle: &StreamHandle) -> Result<(), StreamError>;
    fn results(&self, handle: &StreamHandle) -> StreamResultReceiver;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub target_fps: f32, pub enable_face_detection: bool, pub enable_gesture_recognition: bool,
    pub enable_object_tracking: bool, pub max_tracked_objects: u32, pub alert_rules: Vec<StreamAlertRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamFrameResult {
    pub frame_number: u64, pub faces: Vec<FaceDetection>, pub gestures: Vec<Gesture>,
    pub tracked_objects: Vec<TrackedObject>, pub alerts: Vec<StreamAlert>,
}
```

### 6.4 视频编辑

```rust
pub trait VideoEditor: Send + Sync {
    fn trim(&self, video: &VideoInput, start: Duration, end: Duration) -> impl Future<Output = Result<VideoOutput, VideoError>> + Send;
    fn merge(&self, videos: &[VideoInput], config: &MergeConfig) -> impl Future<Output = Result<VideoOutput, VideoError>> + Send;
    fn add_subtitles(&self, video: &VideoInput, subtitles: &[SubtitleEntry]) -> impl Future<Output = Result<VideoOutput, VideoError>> + Send;
}
```

### 6.5 视频测试用例

```rust
#[cfg(test)]
mod video_tests {
    #[tokio::test]
    async fn test_keyframe_extraction() {
        let kfs = create_test_video_engine().extract_keyframes(&load_video("interview.mp4"), &KeyframeConfig { max_keyframes: 10, ..Default::default() }).await.unwrap();
        assert!(!kfs.is_empty() && kfs.len() <= 10);
    }

    #[tokio::test]
    async fn test_video_summary() {
        let summary = create_test_summarizer().summarize(&load_video("tutorial.mp4"), &VideoSummaryConfig::default()).await.unwrap();
        assert!(!summary.text_summary.is_empty());
    }

    #[tokio::test]
    async fn test_realtime_face_detection() {
        let handle = create_test_stream().start(&VideoSource::File("cam.mp4".into()), &StreamConfig { enable_face_detection: true, ..Default::default() }).await.unwrap();
        let mut rx = create_test_stream().results(&handle);
        assert!(tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.is_ok());
        create_test_stream().stop(&handle).unwrap();
    }
}
```

---

## 7. 跨模态融合

### 7.1 模态路由器

```rust
pub trait ModalRouter: Send + Sync {
    fn detect_modality(&self, input: &MultiModalInput) -> DetectedModality;
    fn route(&self, input: &MultiModalInput) -> Box<dyn ModalityHandler>;
    fn register_handler(&self, modality: Modality, handler: Box<dyn ModalityHandler>);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiModalInput { Text { content: String }, Audio { data: AudioInput }, Image { data: ImageInput }, Video { data: VideoInput }, Multi { parts: Vec<MultiModalPart> } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedModality { pub primary: Modality, pub secondary: Vec<Modality>, pub confidence: f32 }
```

### 7.2 跨模态对齐

```rust
pub trait CrossModalAligner: Send + Sync {
    fn align_text_image(&self, text: &str, image: &ImageInput) -> impl Future<Output = Result<AlignmentScore, AlignmentError>> + Send;
    fn align_text_audio(&self, text: &str, audio: &AudioInput) -> impl Future<Output = Result<AlignmentScore, AlignmentError>> + Send;
    fn search_by_text(&self, query: &str, images: &[ImageInput], top_k: usize) -> impl Future<Output = Result<Vec<ImageMatch>, AlignmentError>> + Send;
    fn embed_text(&self, text: &str) -> Result<Embedding, AlignmentError>;
    fn embed_image(&self, image: &ImageInput) -> Result<Embedding, AlignmentError>;
}
```

### 7.3 统一对话接口

```rust
pub trait UnifiedDialog: Send + Sync {
    fn send_message(&self, message: MultiModalMessage) -> impl Future<Output = Result<MultiModalMessage, DialogError>> + Send;
    fn send_message_stream(&self, message: MultiModalMessage) -> impl Stream<Item = Result<MultiModalMessage, DialogError>> + Send;
    fn get_history(&self, conversation_id: &ConversationId) -> Vec<MultiModalMessage>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModalMessage {
    pub id: MessageId, pub conversation_id: ConversationId, pub role: MessageRole,
    pub content: MultiModalContent, pub timestamp: Instant, pub metadata: MessageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiModalContent { Text(String), Audio(AudioData), Image(ImageData), Video(VideoSource), Mixed(Vec<ContentPart>) }
```

### 7.4 模态转换器

```rust
pub trait ModalConverter: Send + Sync {
    fn text_to_speech(&self, text: &str, config: &TtsConfig) -> impl Future<Output = Result<AudioData, ConversionError>> + Send;
    fn speech_to_text(&self, audio: &AudioInput, config: &AsrConfig) -> impl Future<Output = Result<String, ConversionError>> + Send;
    fn image_to_text(&self, image: &ImageInput) -> impl Future<Output = Result<String, ConversionError>> + Send;
    fn text_to_image(&self, text: &str, config: &ImageGenConfig) -> impl Future<Output = Result<ImageData, ConversionError>> + Send;
    fn video_to_text(&self, video: &VideoInput) -> impl Future<Output = Result<String, ConversionError>> + Send;
    fn supported_conversions(&self) -> Vec<ConversionPath>;
}
```

### 7.5 融合测试用例

```rust
#[cfg(test)]
mod fusion_tests {
    #[test]
    fn test_modality_detection() {
        let detected = create_test_router().detect_modality(&MultiModalInput::Text { content: "你好".into() });
        assert_eq!(detected.primary, Modality::Text);
    }

    #[tokio::test]
    async fn test_clip_alignment() {
        let score = create_test_clip().align_text_image("一只狗在草地上", &load_image("dog.jpg")).await.unwrap();
        assert!(score.score > 0.5);
    }

    #[tokio::test]
    async fn test_unified_dialog() {
        let reply = create_test_dialog().send_message(MultiModalMessage { content: MultiModalContent::Text("描述这张图".into()), ..Default::default() }).await.unwrap();
        assert!(!matches!(reply.content, MultiModalContent::Text(ref t) if t.is_empty()));
    }
}
```

---

## 8. 安全设计

| 安全维度 | 措施 |
|----------|------|
| 隐私保护 | 敏感内容标记，自动路由到本地模型，云端传输加密 |
| 内容安全 | NSFW 检测，有害内容过滤，输入输出双重审核 |
| 模型安全 | 模型签名验证，沙箱执行，资源限制 |
| 数据安全 | 音频/图像/视频数据传输加密，存储加密 |
| 访问控制 | 模型 API 调用鉴权，声纹认证，操作审计 |
| 资源保护 | GPU 显存配额，并发请求限流，请求队列管理 |

## 9. GPU 资源需求

| 场景 | 最低显存 | 推荐显存 |
|------|----------|----------|
| 仅文本交互 | 2 GB | 4 GB |
| 语音交互 | 3 GB | 6 GB |
| 图像理解 | 4 GB | 8 GB |
| 图像生成 | 6 GB | 12 GB |
| 视频理解 | 4 GB | 8 GB |
| 视频生成 | 8 GB | 16 GB |
| 全模态 | 12 GB | 24 GB |
| CPU-only | 0 GB | 0 GB |

## 10. 配置参考

```toml
[multimodal.router]
default_channel = "auto"; privacy_level = "internal"; fallback_to_cloud = true; cost_limit_monthly = 50.0

[multimodal.voice]
asr_model = "whisper-large-v3"; tts_model = "piper"; vad_threshold = 0.5; stream_buffer_ms = 100

[multimodal.text]
nlu_model = "phi-3-mini"; translator_model = "nllb-200-distilled"; max_context_tokens = 8192

[multimodal.image]
vqa_model = "moondream2"; ocr_engine = "paddleocr"; image_gen_model = "sd-1.5"; nsfw_filter_enabled = true

[multimodal.video]
max_video_duration = "30m"; keyframe_method = "scene_detection"; stream_target_fps = 15
```
