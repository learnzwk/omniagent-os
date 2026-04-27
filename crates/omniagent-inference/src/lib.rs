// OmniAgent OS Phase 10: AI 模型集成框架
//
// 提供统一的 AI 推理引擎框架，支持：
// - 本地模型推理（Candle / ONNX Runtime / Tract）
// - 云端 API 调用（OpenAI / Anthropic / Google / Mistral / Azure）
// - 智能路由决策（延迟优先 / 精度优先 / 隐私优先）
// - 流式推理会话管理

/// 核心类型定义
pub mod types;

/// 模型路由器
pub mod router;

/// 推理引擎和管理器
pub mod engine;

/// 流式推理
pub mod stream;

// 重新导出常用类型
pub use types::{
    CloudProvider, CloudProviderInfo, InferenceError, InferenceInput, InferenceOutput,
    InferencePreference, InferenceProvider, InferenceResult, InferenceStats, InferenceTask,
    LocalBackend, LocalModelInfo, ModelAvailability, ModelId, ModelRequest, PrivacyLevel,
    RoutingDecision, StreamChunk, TokenUsage,
};
pub use engine::{InferenceEngine, InferenceManager};
pub use router::ModelRouter;
pub use stream::{StreamManager, StreamSession};

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== PrivacyLevel 测试 ====================

    #[test]
    fn test_privacy_level_ordering() {
        // 隐私级别应有正确的排序关系
        assert!(PrivacyLevel::Public < PrivacyLevel::Internal);
        assert!(PrivacyLevel::Internal < PrivacyLevel::Sensitive);
        assert!(PrivacyLevel::Sensitive < PrivacyLevel::Confidential);
    }

    #[test]
    fn test_privacy_level_default() {
        assert_eq!(PrivacyLevel::default(), PrivacyLevel::Public);
    }

    #[test]
    fn test_privacy_level_repr() {
        assert_eq!(PrivacyLevel::Public as u8, 0);
        assert_eq!(PrivacyLevel::Internal as u8, 1);
        assert_eq!(PrivacyLevel::Sensitive as u8, 2);
        assert_eq!(PrivacyLevel::Confidential as u8, 3);
    }

    // ==================== InferencePreference 测试 ====================

    #[test]
    fn test_inference_preference_default() {
        assert_eq!(InferencePreference::default(), InferencePreference::Auto);
    }

    #[test]
    fn test_inference_preference_repr() {
        assert_eq!(InferencePreference::Auto as u8, 0);
        assert_eq!(InferencePreference::LocalOnly as u8, 1);
        assert_eq!(InferencePreference::CloudOnly as u8, 2);
        assert_eq!(InferencePreference::LatencyFirst as u8, 3);
        assert_eq!(InferencePreference::AccuracyFirst as u8, 4);
        assert_eq!(InferencePreference::PrivacyFirst as u8, 5);
    }

    #[test]
    fn test_inference_preference_equality() {
        assert_eq!(InferencePreference::Auto, InferencePreference::Auto);
        assert_ne!(InferencePreference::LocalOnly, InferencePreference::CloudOnly);
    }

    // ==================== InferenceError 测试 ====================

    #[test]
    fn test_inference_error_display() {
        let err = InferenceError::ModelNotFound("llama-7b".to_string());
        assert_eq!(format!("{}", err), "模型未找到: llama-7b");

        let err = InferenceError::ModelNotLoaded("gpt-4".to_string());
        assert_eq!(format!("{}", err), "模型未加载: gpt-4");

        let err = InferenceError::ModelLoadFailed("磁盘空间不足".to_string());
        assert_eq!(format!("{}", err), "模型加载失败: 磁盘空间不足");

        let err = InferenceError::InferenceFailed("CUDA 错误".to_string());
        assert_eq!(format!("{}", err), "推理失败: CUDA 错误");

        let err = InferenceError::Timeout(5000);
        assert_eq!(format!("{}", err), "推理超时: 5000ms");

        let err = InferenceError::InvalidInput("空文本".to_string());
        assert_eq!(format!("{}", err), "无效输入: 空文本");

        let err = InferenceError::UnsupportedTask(InferenceTask::ImageGeneration);
        assert!(format!("{}", err).contains("不支持的任务类型"));

        let err = InferenceError::ApiError {
            provider: CloudProvider::OpenAI,
            message: "API 密钥无效".to_string(),
        };
        assert!(format!("{}", err).contains("OpenAI"));
        assert!(format!("{}", err).contains("API 密钥无效"));

        let err = InferenceError::RateLimited {
            provider: CloudProvider::Anthropic,
            retry_after_ms: 30000,
        };
        assert!(format!("{}", err).contains("速率限制"));
        assert!(format!("{}", err).contains("30000ms"));

        let err = InferenceError::OutOfMemory {
            required: 4096,
            available: 1024,
        };
        assert!(format!("{}", err).contains("4096"));
        assert!(format!("{}", err).contains("1024"));

        let err = InferenceError::ContextTooLarge {
            tokens: 100000,
            max_tokens: 32000,
        };
        assert!(format!("{}", err).contains("100000"));
        assert!(format!("{}", err).contains("32000"));

        let err = InferenceError::NetworkError("连接超时".to_string());
        assert_eq!(format!("{}", err), "网络错误: 连接超时");

        let err = InferenceError::AuthenticationError("Token 过期".to_string());
        assert_eq!(format!("{}", err), "认证错误: Token 过期");
    }

    #[test]
    fn test_inference_error_equality() {
        assert_eq!(
            InferenceError::ModelNotFound("a".to_string()),
            InferenceError::ModelNotFound("a".to_string())
        );
        assert_ne!(
            InferenceError::ModelNotFound("a".to_string()),
            InferenceError::ModelNotFound("b".to_string())
        );
        assert_eq!(InferenceError::Timeout(100), InferenceError::Timeout(100));
        assert_ne!(InferenceError::Timeout(100), InferenceError::Timeout(200));
    }

    // ==================== TokenUsage 测试 ====================

    #[test]
    fn test_token_usage_new() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_token_usage_zero() {
        let usage = TokenUsage::new(0, 0);
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_token_usage_large_values() {
        let usage = TokenUsage::new(u32::MAX, 1);
        assert_eq!(usage.prompt_tokens, u32::MAX);
        assert_eq!(usage.completion_tokens, 1);
        // 溢出检查：u32::MAX + 1 会溢出
        assert_eq!(usage.total_tokens, 0); // 溢出回绕
    }

    #[test]
    fn test_token_usage_equality() {
        assert_eq!(TokenUsage::new(10, 20), TokenUsage::new(10, 20));
        assert_ne!(TokenUsage::new(10, 20), TokenUsage::new(20, 10));
    }

    #[test]
    fn test_token_usage_copy() {
        let usage = TokenUsage::new(10, 20);
        let usage2 = usage;
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage2.prompt_tokens, 10);
    }

    // ==================== RoutingDecision 测试 ====================

    #[test]
    fn test_routing_decision_local_candle() {
        let decision = RoutingDecision::LocalCandle {
            model_id: ModelId::new("llama-7b"),
        };
        assert_eq!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_routing_decision_local_onnx() {
        let decision = RoutingDecision::LocalOnnx {
            model_id: ModelId::new("whisper"),
        };
        assert_eq!(
            decision,
            RoutingDecision::LocalOnnx {
                model_id: ModelId::new("whisper")
            }
        );
    }

    #[test]
    fn test_routing_decision_local_tract() {
        let decision = RoutingDecision::LocalTract {
            model_id: ModelId::new("tinybert"),
        };
        assert_eq!(
            decision,
            RoutingDecision::LocalTract {
                model_id: ModelId::new("tinybert")
            }
        );
    }

    #[test]
    fn test_routing_decision_cloud() {
        let decision = RoutingDecision::Cloud {
            provider: CloudProvider::OpenAI,
            model_id: ModelId::new("gpt-4"),
        };
        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("gpt-4")
            }
        );
    }

    #[test]
    fn test_routing_decision_fallback() {
        let primary = RoutingDecision::LocalCandle {
            model_id: ModelId::new("llama-7b"),
        };
        let fallback = RoutingDecision::Cloud {
            provider: CloudProvider::OpenAI,
            model_id: ModelId::new("gpt-4"),
        };
        let decision = RoutingDecision::Fallback {
            primary: Box::new(primary),
            fallback: Box::new(fallback),
        };

        assert_eq!(
            decision,
            RoutingDecision::Fallback {
                primary: Box::new(RoutingDecision::LocalCandle {
                    model_id: ModelId::new("llama-7b")
                }),
                fallback: Box::new(RoutingDecision::Cloud {
                    provider: CloudProvider::OpenAI,
                    model_id: ModelId::new("gpt-4")
                })
            }
        );
    }

    #[test]
    fn test_routing_decision_clone() {
        let decision = RoutingDecision::LocalCandle {
            model_id: ModelId::new("llama-7b"),
        };
        let decision2 = decision.clone();
        assert_eq!(decision, decision2);
    }

    #[test]
    fn test_routing_decision_debug() {
        let decision = RoutingDecision::Cloud {
            provider: CloudProvider::Anthropic,
            model_id: ModelId::new("claude-3"),
        };
        let debug_str = format!("{:?}", decision);
        assert!(debug_str.contains("Anthropic"));
        assert!(debug_str.contains("claude-3"));
    }

    // ==================== ModelId 测试 ====================

    #[test]
    fn test_model_id_new() {
        let id = ModelId::new("llama-7b");
        assert_eq!(id.as_str(), "llama-7b");
    }

    #[test]
    fn test_model_id_display() {
        let id = ModelId::new("gpt-4");
        assert_eq!(format!("{}", id), "gpt-4");
    }

    #[test]
    fn test_model_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ModelId::new("model-a"));
        set.insert(ModelId::new("model-b"));
        set.insert(ModelId::new("model-a")); // 重复
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_model_id_equality() {
        assert_eq!(ModelId::new("a"), ModelId::new("a"));
        assert_ne!(ModelId::new("a"), ModelId::new("b"));
    }

    // ==================== ModelAvailability 测试 ====================

    #[test]
    fn test_model_availability_default() {
        assert_eq!(ModelAvailability::default(), ModelAvailability::Unavailable);
    }

    #[test]
    fn test_model_availability_repr() {
        assert_eq!(ModelAvailability::Available as u8, 0);
        assert_eq!(ModelAvailability::Loading as u8, 1);
        assert_eq!(ModelAvailability::Unavailable as u8, 2);
        assert_eq!(ModelAvailability::Error as u8, 3);
    }

    // ==================== InferenceTask 测试 ====================

    #[test]
    fn test_inference_task_equality() {
        assert_eq!(InferenceTask::TextGeneration, InferenceTask::TextGeneration);
        assert_ne!(InferenceTask::TextGeneration, InferenceTask::TextEmbedding);
    }

    #[test]
    fn test_inference_task_translation() {
        let task = InferenceTask::Translation {
            from: "en".to_string(),
            to: "zh".to_string(),
        };
        assert_eq!(
            task,
            InferenceTask::Translation {
                from: "en".to_string(),
                to: "zh".to_string()
            }
        );
    }

    // ==================== InferenceInput 测试 ====================

    #[test]
    fn test_inference_input_text() {
        let input = InferenceInput::Text("hello world".to_string());
        match input {
            InferenceInput::Text(s) => assert_eq!(s, "hello world"),
            _ => panic!("应为 Text 变体"),
        }
    }

    #[test]
    fn test_inference_input_text_with_context() {
        let input = InferenceInput::TextWithContext {
            text: "question".to_string(),
            context: "background info".to_string(),
        };
        match input {
            InferenceInput::TextWithContext { text, context } => {
                assert_eq!(text, "question");
                assert_eq!(context, "background info");
            }
            _ => panic!("应为 TextWithContext 变体"),
        }
    }

    #[test]
    fn test_inference_input_image() {
        let input = InferenceInput::Image {
            data: vec![0xFF, 0xD8],
            format: "jpeg".to_string(),
        };
        match input {
            InferenceInput::Image { data, format } => {
                assert_eq!(data, vec![0xFF, 0xD8]);
                assert_eq!(format, "jpeg");
            }
            _ => panic!("应为 Image 变体"),
        }
    }

    #[test]
    fn test_inference_input_audio() {
        let input = InferenceInput::Audio {
            data: vec![0, 1, 2],
            format: "wav".to_string(),
            sample_rate: 16000,
        };
        match input {
            InferenceInput::Audio { data, format, sample_rate } => {
                assert_eq!(data, vec![0, 1, 2]);
                assert_eq!(format, "wav");
                assert_eq!(sample_rate, 16000);
            }
            _ => panic!("应为 Audio 变体"),
        }
    }

    #[test]
    fn test_inference_input_multimodal() {
        let input = InferenceInput::MultiModal {
            parts: vec![
                ("text".to_string(), b"hello".to_vec()),
                ("image".to_string(), vec![0xFF, 0xD8]),
            ],
        };
        match input {
            InferenceInput::MultiModal { parts } => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0].0, "text");
                assert_eq!(parts[1].0, "image");
            }
            _ => panic!("应为 MultiModal 变体"),
        }
    }

    // ==================== ModelRequest 测试 ====================

    #[test]
    fn test_model_request_builder() {
        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::LocalOnly)
        .with_max_latency(1000)
        .with_privacy_level(PrivacyLevel::Sensitive)
        .with_budget_tokens(500);

        assert_eq!(request.preference, InferencePreference::LocalOnly);
        assert_eq!(request.max_latency_ms, Some(1000));
        assert_eq!(request.privacy_level, PrivacyLevel::Sensitive);
        assert_eq!(request.budget_tokens, Some(500));
    }

    #[test]
    fn test_model_request_defaults() {
        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        );

        assert_eq!(request.preference, InferencePreference::Auto);
        assert_eq!(request.max_latency_ms, None);
        assert_eq!(request.privacy_level, PrivacyLevel::Public);
        assert_eq!(request.budget_tokens, None);
    }

    // ==================== InferenceResult 测试 ====================

    #[test]
    fn test_inference_result_builder() {
        let result = InferenceResult::new(
            InferenceOutput::Text("generated text".to_string()),
            ModelId::new("llama-7b"),
            InferenceProvider::Candle,
        )
        .with_latency(42)
        .with_tokens(TokenUsage::new(10, 20))
        .with_metadata("key", "value");

        assert_eq!(
            result.output,
            InferenceOutput::Text("generated text".to_string())
        );
        assert_eq!(result.latency_ms, 42);
        assert_eq!(result.tokens_used, Some(TokenUsage::new(10, 20)));
        assert_eq!(result.metadata.get("key").unwrap(), "value");
    }

    // ==================== InferenceOutput 测试 ====================

    #[test]
    fn test_inference_output_variants() {
        let text = InferenceOutput::Text("hello".to_string());
        let embedding = InferenceOutput::Embedding(vec![0.1, 0.2, 0.3]);
        let image = InferenceOutput::Image {
            data: vec![0xFF],
            format: "png".to_string(),
            width: 100,
            height: 100,
        };
        let audio = InferenceOutput::Audio {
            data: vec![0, 1],
            format: "wav".to_string(),
        };
        let classification = InferenceOutput::Classification {
            label: "positive".to_string(),
            confidence: 0.95,
            alternatives: vec![("negative".to_string(), 0.05)],
        };
        let stream = InferenceOutput::StreamHandle(42);

        // 验证各变体可以创建和比较
        assert!(matches!(text, InferenceOutput::Text(_)));
        assert!(matches!(embedding, InferenceOutput::Embedding(_)));
        assert!(matches!(image, InferenceOutput::Image { .. }));
        assert!(matches!(audio, InferenceOutput::Audio { .. }));
        assert!(matches!(classification, InferenceOutput::Classification { .. }));
        assert!(matches!(stream, InferenceOutput::StreamHandle(_)));
    }

    // ==================== LocalBackend 测试 ====================

    #[test]
    fn test_local_backend_repr() {
        assert_eq!(LocalBackend::Candle as u8, 0);
        assert_eq!(LocalBackend::OnnxRuntime as u8, 1);
        assert_eq!(LocalBackend::Tract as u8, 2);
    }

    // ==================== CloudProvider 测试 ====================

    #[test]
    fn test_cloud_provider_repr() {
        assert_eq!(CloudProvider::OpenAI as u8, 0);
        assert_eq!(CloudProvider::Anthropic as u8, 1);
        assert_eq!(CloudProvider::Google as u8, 2);
        assert_eq!(CloudProvider::Mistral as u8, 3);
        assert_eq!(CloudProvider::Azure as u8, 4);
        assert_eq!(CloudProvider::Custom as u8, 5);
    }

    #[test]
    fn test_cloud_provider_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CloudProvider::OpenAI);
        set.insert(CloudProvider::Anthropic);
        set.insert(CloudProvider::OpenAI); // 重复
        assert_eq!(set.len(), 2);
    }

    // ==================== InferenceProvider 测试 ====================

    #[test]
    fn test_inference_provider_repr() {
        assert_eq!(InferenceProvider::Candle as u8, 0);
        assert_eq!(InferenceProvider::OnnxRuntime as u8, 1);
        assert_eq!(InferenceProvider::Tract as u8, 2);
        assert_eq!(InferenceProvider::OpenAI as u8, 3);
        assert_eq!(InferenceProvider::Anthropic as u8, 4);
        assert_eq!(InferenceProvider::Google as u8, 5);
    }

    // ==================== StreamChunk 测试 ====================

    #[test]
    fn test_stream_chunk_new() {
        let chunk = StreamChunk::new(0, "hello".to_string());
        assert_eq!(chunk.chunk_id, 0);
        assert_eq!(chunk.content, "hello");
        assert!(!chunk.is_final);
        assert_eq!(chunk.latency_ms, 0);
    }

    #[test]
    fn test_stream_chunk_final() {
        let chunk = StreamChunk::final_chunk(5, "done".to_string());
        assert_eq!(chunk.chunk_id, 5);
        assert_eq!(chunk.content, "done");
        assert!(chunk.is_final);
    }

    #[test]
    fn test_stream_chunk_with_latency() {
        let chunk = StreamChunk::new(0, "hello".to_string()).with_latency(42);
        assert_eq!(chunk.latency_ms, 42);
    }

    // ==================== LocalModelInfo 测试 ====================

    #[test]
    fn test_local_model_info_builder() {
        let info = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration, InferenceTask::Summarization])
        .with_memory(1024 * 1024 * 1024)
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);

        assert_eq!(info.model_id.as_str(), "llama-7b");
        assert_eq!(info.backend, LocalBackend::Candle);
        assert_eq!(info.model_path, "/models/llama-7b");
        assert_eq!(info.supported_tasks.len(), 2);
        assert_eq!(info.memory_bytes, 1024 * 1024 * 1024);
        assert!((info.avg_latency_ms - 50.0).abs() < 0.001);
        assert!(info.is_available());
    }

    #[test]
    fn test_local_model_info_supports_task() {
        let info = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration]);

        assert!(info.supports_task(&InferenceTask::TextGeneration));
        assert!(!info.supports_task(&InferenceTask::TextEmbedding));
    }

    // ==================== CloudProviderInfo 测试 ====================

    #[test]
    fn test_cloud_provider_info_builder() {
        let info = CloudProviderInfo::new(
            CloudProvider::OpenAI,
            "https://api.openai.com/v1".to_string(),
            "openai_api_key".to_string(),
        )
        .with_models(vec![ModelId::new("gpt-4"), ModelId::new("gpt-3.5-turbo")])
        .with_latency(200.0)
        .configured();

        assert_eq!(info.provider, CloudProvider::OpenAI);
        assert_eq!(info.api_endpoint, "https://api.openai.com/v1");
        assert_eq!(info.api_key_name, "openai_api_key");
        assert_eq!(info.available_models.len(), 2);
        assert!(info.is_configured);
        assert!(info.has_model(&ModelId::new("gpt-4")));
        assert!(!info.has_model(&ModelId::new("claude-3")));
    }

    // ==================== InferenceStats 测试 ====================

    #[test]
    fn test_inference_stats() {
        let stats = InferenceStats {
            total_inferences: 100,
            avg_latency_ms: 42.5,
            engines_count: 3,
            local_models_count: 5,
            cloud_providers_count: 2,
        };

        assert_eq!(stats.total_inferences, 100);
        assert!((stats.avg_latency_ms - 42.5).abs() < 0.001);
        assert_eq!(stats.engines_count, 3);
        assert_eq!(stats.local_models_count, 5);
        assert_eq!(stats.cloud_providers_count, 2);
    }

    #[test]
    fn test_inference_stats_clone() {
        let stats = InferenceStats {
            total_inferences: 10,
            avg_latency_ms: 50.0,
            engines_count: 1,
            local_models_count: 2,
            cloud_providers_count: 3,
        };
        let stats2 = stats.clone();
        assert_eq!(stats.total_inferences, stats2.total_inferences);
    }
}
