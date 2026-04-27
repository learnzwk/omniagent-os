// OmniAgent OS Phase 10: 模型路由器
// 根据推理偏好、隐私级别、延迟要求等因素做出最优路由决策

use std::collections::HashMap;

use crate::types::*;

/// 模型路由器，负责根据请求特征选择最优推理路径
pub struct ModelRouter {
    /// 当前推理偏好
    preference: InferencePreference,
    /// 已注册的本地模型
    local_models: HashMap<ModelId, LocalModelInfo>,
    /// 已注册的云端提供商
    cloud_providers: HashMap<CloudProvider, CloudProviderInfo>,
}

impl ModelRouter {
    /// 创建新的模型路由器
    pub fn new(preference: InferencePreference) -> Self {
        Self {
            preference,
            local_models: HashMap::new(),
            cloud_providers: HashMap::new(),
        }
    }

    /// 注册本地模型
    pub fn register_local_model(&mut self, info: LocalModelInfo) {
        self.local_models.insert(info.model_id.clone(), info);
    }

    /// 注册云端提供商
    pub fn register_cloud_provider(&mut self, info: CloudProviderInfo) {
        self.cloud_providers.insert(info.provider, info);
    }

    /// 注销本地模型
    pub fn unregister_local_model(&mut self, model_id: &ModelId) -> bool {
        self.local_models.remove(model_id).is_some()
    }

    /// 注销云端提供商
    pub fn unregister_cloud_provider(&mut self, provider: &CloudProvider) -> bool {
        self.cloud_providers.remove(provider).is_some()
    }

    /// 根据请求做出路由决策
    ///
    /// 路由逻辑：
    /// 1. 根据偏好过滤可用路径
    /// 2. 根据隐私级别进一步过滤
    /// 3. 根据延迟要求筛选
    /// 4. 选择最优路径
    pub fn route(&self, request: &ModelRequest) -> RoutingDecision {
        // 根据请求中的偏好覆盖全局偏好
        let effective_pref = request.preference;

        // 机密数据必须本地处理
        if request.privacy_level >= PrivacyLevel::Confidential {
            if let Some(decision) = self.find_best_local(&request.task) {
                return decision;
            }
            // 如果没有可用的本地模型，返回错误决策（使用第一个本地模型标记为不可用）
            return self.fallback_decision(&request.task);
        }

        match effective_pref {
            InferencePreference::LocalOnly => {
                if let Some(decision) = self.find_best_local(&request.task) {
                    decision
                } else {
                    self.fallback_decision(&request.task)
                }
            }
            InferencePreference::CloudOnly => {
                if let Some(decision) = self.find_best_cloud(&request.task) {
                    decision
                } else {
                    self.fallback_decision(&request.task)
                }
            }
            InferencePreference::LatencyFirst => {
                // 延迟优先：比较本地和云端的延迟
                let local = self.find_best_local(&request.task);
                let cloud = self.find_best_cloud(&request.task);

                match (local, cloud) {
                    (Some(l), Some(c)) => {
                        let local_latency = self.get_local_latency(&l);
                        let cloud_latency = self.get_cloud_latency(&c);
                        if local_latency <= cloud_latency {
                            l
                        } else {
                            c
                        }
                    }
                    (Some(l), None) => l,
                    (None, Some(c)) => c,
                    (None, None) => self.fallback_decision(&request.task),
                }
            }
            InferencePreference::AccuracyFirst => {
                // 精度优先：优先选择云端（通常精度更高）
                if let Some(decision) = self.find_best_cloud(&request.task) {
                    decision
                } else if let Some(decision) = self.find_best_local(&request.task) {
                    decision
                } else {
                    self.fallback_decision(&request.task)
                }
            }
            InferencePreference::PrivacyFirst => {
                // 隐私优先：优先本地，敏感数据不发送到云端
                if request.privacy_level >= PrivacyLevel::Sensitive {
                    if let Some(decision) = self.find_best_local(&request.task) {
                        return decision;
                    }
                }
                // 非敏感数据可以回退到云端
                if let Some(decision) = self.find_best_local(&request.task) {
                    decision
                } else if let Some(decision) = self.find_best_cloud(&request.task) {
                    decision
                } else {
                    self.fallback_decision(&request.task)
                }
            }
            InferencePreference::Auto => {
                // 自动模式：默认优先本地，不可用时回退云端
                if let Some(decision) = self.find_best_local(&request.task) {
                    decision
                } else if let Some(decision) = self.find_best_cloud(&request.task) {
                    decision
                } else {
                    self.fallback_decision(&request.task)
                }
            }
        }
    }

    /// 检查指定模型的可用性
    pub fn availability(&self, model_id: &ModelId) -> ModelAvailability {
        self.local_models
            .get(model_id)
            .map(|m| m.availability)
            .unwrap_or(ModelAvailability::Unavailable)
    }

    /// 设置全局推理偏好
    pub fn set_preference(&mut self, preference: InferencePreference) {
        self.preference = preference;
    }

    /// 获取当前推理偏好
    pub fn preference(&self) -> InferencePreference {
        self.preference
    }

    /// 列出所有本地模型
    pub fn list_local_models(&self) -> Vec<&LocalModelInfo> {
        self.local_models.values().collect()
    }

    /// 列出所有云端提供商
    pub fn list_cloud_providers(&self) -> Vec<&CloudProviderInfo> {
        self.cloud_providers.values().collect()
    }

    /// 获取本地模型数量
    pub fn local_models_count(&self) -> usize {
        self.local_models.len()
    }

    /// 获取云端提供商数量
    pub fn cloud_providers_count(&self) -> usize {
        self.cloud_providers.len()
    }

    /// 查找支持指定任务的最佳本地模型
    fn find_best_local(&self, task: &InferenceTask) -> Option<RoutingDecision> {
        // 筛选可用且支持该任务的本地模型
        let mut candidates: Vec<&LocalModelInfo> = self
            .local_models
            .values()
            .filter(|m| m.is_available() && m.supports_task(task))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // 按延迟排序，选择最快的
        candidates.sort_by_key(|m| m.avg_latency_ms as u32);

        let best = candidates[0];
        let decision = match best.backend {
            LocalBackend::Candle => RoutingDecision::LocalCandle {
                model_id: best.model_id.clone(),
            },
            LocalBackend::OnnxRuntime => RoutingDecision::LocalOnnx {
                model_id: best.model_id.clone(),
            },
            LocalBackend::Tract => RoutingDecision::LocalTract {
                model_id: best.model_id.clone(),
            },
        };

        Some(decision)
    }

    /// 查找支持指定任务的最佳云端提供商
    fn find_best_cloud(&self, _task: &InferenceTask) -> Option<RoutingDecision> {
        // 筛选已配置的云端提供商
        let mut candidates: Vec<&CloudProviderInfo> = self
            .cloud_providers
            .values()
            .filter(|p| p.is_configured && !p.available_models.is_empty())
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // 按延迟排序
        candidates.sort_by_key(|p| p.avg_latency_ms as u32);

        // 选择延迟最低的提供商的第一个模型
        let best = candidates[0];
        let model_id = best.available_models[0].clone();

        Some(RoutingDecision::Cloud {
            provider: best.provider,
            model_id,
        })
    }

    /// 获取本地决策的延迟
    fn get_local_latency(&self, decision: &RoutingDecision) -> f32 {
        let model_id = match decision {
            RoutingDecision::LocalCandle { model_id } => model_id,
            RoutingDecision::LocalOnnx { model_id } => model_id,
            RoutingDecision::LocalTract { model_id } => model_id,
            _ => return f32::MAX,
        };
        self.local_models
            .get(model_id)
            .map(|m| m.avg_latency_ms)
            .unwrap_or(f32::MAX)
    }

    /// 获取云端决策的延迟
    fn get_cloud_latency(&self, decision: &RoutingDecision) -> f32 {
        let provider = match decision {
            RoutingDecision::Cloud { provider, .. } => *provider,
            _ => return f32::MAX,
        };
        self.cloud_providers
            .get(&provider)
            .map(|p| p.avg_latency_ms)
            .unwrap_or(f32::MAX)
    }

    /// 生成回退决策
    fn fallback_decision(&self, task: &InferenceTask) -> RoutingDecision {
        let local = self.find_best_local(task);
        let cloud = self.find_best_cloud(task);

        match (local, cloud) {
            (Some(l), Some(c)) => RoutingDecision::Fallback {
                primary: Box::new(l),
                fallback: Box::new(c),
            },
            (Some(l), None) => l,
            (None, Some(c)) => c,
            (None, None) => {
                // 没有任何可用路径，返回一个默认的云端决策作为占位
                RoutingDecision::Cloud {
                    provider: CloudProvider::OpenAI,
                    model_id: ModelId::new("fallback-model"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_gen_task() -> InferenceTask {
        InferenceTask::TextGeneration
    }

    fn make_embedding_task() -> InferenceTask {
        InferenceTask::TextEmbedding
    }

    fn make_available_candle_model(id: &str, latency: f32) -> LocalModelInfo {
        LocalModelInfo::new(ModelId::new(id), LocalBackend::Candle, format!("/models/{}", id))
            .with_tasks(vec![make_text_gen_task()])
            .with_latency(latency)
            .with_availability(ModelAvailability::Available)
            .with_memory(1024 * 1024 * 512)
    }

    fn make_available_onnx_model(id: &str, latency: f32) -> LocalModelInfo {
        LocalModelInfo::new(ModelId::new(id), LocalBackend::OnnxRuntime, format!("/models/{}", id))
            .with_tasks(vec![make_text_gen_task(), make_embedding_task()])
            .with_latency(latency)
            .with_availability(ModelAvailability::Available)
            .with_memory(1024 * 1024 * 256)
    }

    fn make_available_tract_model(id: &str, latency: f32) -> LocalModelInfo {
        LocalModelInfo::new(ModelId::new(id), LocalBackend::Tract, format!("/models/{}", id))
            .with_tasks(vec![make_text_gen_task()])
            .with_latency(latency)
            .with_availability(ModelAvailability::Available)
            .with_memory(1024 * 1024 * 128)
    }

    fn make_cloud_provider(provider: CloudProvider, latency: f32) -> CloudProviderInfo {
        CloudProviderInfo::new(
            provider,
            format!("https://api.{:?}.com/v1", provider),
            format!("{:?}_key", provider),
        )
        .with_models(vec![ModelId::new(format!("{:?}-model", provider))])
        .with_latency(latency)
        .configured()
    }

    #[test]
    fn test_router_new() {
        let router = ModelRouter::new(InferencePreference::Auto);
        assert_eq!(router.preference(), InferencePreference::Auto);
        assert_eq!(router.local_models_count(), 0);
        assert_eq!(router.cloud_providers_count(), 0);
    }

    #[test]
    fn test_register_local_model() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        let model = make_available_candle_model("llama-7b", 50.0);
        router.register_local_model(model);

        assert_eq!(router.local_models_count(), 1);
        assert_eq!(router.list_local_models()[0].model_id.as_str(), "llama-7b");
    }

    #[test]
    fn test_register_cloud_provider() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        let provider = make_cloud_provider(CloudProvider::OpenAI, 200.0);
        router.register_cloud_provider(provider);

        assert_eq!(router.cloud_providers_count(), 1);
    }

    #[test]
    fn test_unregister_local_model() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        let model = make_available_candle_model("llama-7b", 50.0);
        router.register_local_model(model);

        assert!(router.unregister_local_model(&ModelId::new("llama-7b")));
        assert_eq!(router.local_models_count(), 0);
        assert!(!router.unregister_local_model(&ModelId::new("nonexistent")));
    }

    #[test]
    fn test_unregister_cloud_provider() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        let provider = make_cloud_provider(CloudProvider::OpenAI, 200.0);
        router.register_cloud_provider(provider);

        assert!(router.unregister_cloud_provider(&CloudProvider::OpenAI));
        assert_eq!(router.cloud_providers_count(), 0);
        assert!(!router.unregister_cloud_provider(&CloudProvider::Anthropic));
    }

    #[test]
    fn test_availability() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        assert_eq!(
            router.availability(&ModelId::new("unknown")),
            ModelAvailability::Unavailable
        );

        let model = make_available_candle_model("llama-7b", 50.0);
        router.register_local_model(model);
        assert_eq!(
            router.availability(&ModelId::new("llama-7b")),
            ModelAvailability::Available
        );
    }

    #[test]
    fn test_set_preference() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.set_preference(InferencePreference::LocalOnly);
        assert_eq!(router.preference(), InferencePreference::LocalOnly);
    }

    #[test]
    fn test_route_auto_with_local() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_route_auto_with_cloud() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("OpenAI-model")
            }
        );
    }

    #[test]
    fn test_route_local_only() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::LocalOnly);
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_route_cloud_only() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::CloudOnly);
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("OpenAI-model")
            }
        );
    }

    #[test]
    fn test_route_latency_first_local_faster() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 30.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::LatencyFirst);
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_route_latency_first_cloud_faster() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 500.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 100.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::LatencyFirst);
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("OpenAI-model")
            }
        );
    }

    #[test]
    fn test_route_accuracy_first_prefers_cloud() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::AccuracyFirst);
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("OpenAI-model")
            }
        );
    }

    #[test]
    fn test_route_privacy_first_with_sensitive_data() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("secret data".to_string()),
        )
        .with_preference(InferencePreference::PrivacyFirst)
        .with_privacy_level(PrivacyLevel::Sensitive);
        let decision = router.route(&request);

        // 敏感数据应选择本地模型
        assert_eq!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_route_confidential_data_forces_local() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("top secret".to_string()),
        )
        .with_privacy_level(PrivacyLevel::Confidential);
        let decision = router.route(&request);

        // 机密数据必须本地处理，即使偏好是 Auto
        assert_eq!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_route_fallback_when_nothing_available() {
        let router = ModelRouter::new(InferencePreference::Auto);

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        // 没有任何可用模型时返回默认回退
        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("fallback-model")
            }
        );
    }

    #[test]
    fn test_route_selects_fastest_local_model() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("slow-model", 200.0));
        router.register_local_model(make_available_onnx_model("fast-model", 30.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        // 应选择延迟最低的本地模型
        assert_eq!(
            decision,
            RoutingDecision::LocalOnnx {
                model_id: ModelId::new("fast-model")
            }
        );
    }

    #[test]
    fn test_route_ignores_unavailable_local_models() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        let mut model = make_available_candle_model("llama-7b", 50.0);
        model.availability = ModelAvailability::Error;
        router.register_local_model(model);
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        // 本地模型不可用，应回退到云端
        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::OpenAI,
                model_id: ModelId::new("OpenAI-model")
            }
        );
    }

    #[test]
    fn test_route_ignores_task_mismatch() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        // 模型只支持 TextGeneration，不支持 TextEmbedding
        router.register_local_model(make_available_candle_model("llama-7b", 50.0));

        let request = ModelRequest::new(
            make_embedding_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        // 任务不匹配，不应选择该本地模型
        assert_ne!(
            decision,
            RoutingDecision::LocalCandle {
                model_id: ModelId::new("llama-7b")
            }
        );
    }

    #[test]
    fn test_route_tract_backend() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_tract_model("tiny-model", 10.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        );
        let decision = router.route(&request);

        assert_eq!(
            decision,
            RoutingDecision::LocalTract {
                model_id: ModelId::new("tiny-model")
            }
        );
    }

    #[test]
    fn test_route_multiple_cloud_providers() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_cloud_provider(make_cloud_provider(CloudProvider::Anthropic, 150.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 300.0));

        let request = ModelRequest::new(
            make_text_gen_task(),
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::CloudOnly);
        let decision = router.route(&request);

        // 应选择延迟最低的云端提供商
        assert_eq!(
            decision,
            RoutingDecision::Cloud {
                provider: CloudProvider::Anthropic,
                model_id: ModelId::new("Anthropic-model")
            }
        );
    }

    #[test]
    fn test_list_local_models() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_local_model(make_available_candle_model("model-a", 50.0));
        router.register_local_model(make_available_onnx_model("model-b", 30.0));

        let models = router.list_local_models();
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_list_cloud_providers() {
        let mut router = ModelRouter::new(InferencePreference::Auto);
        router.register_cloud_provider(make_cloud_provider(CloudProvider::OpenAI, 200.0));
        router.register_cloud_provider(make_cloud_provider(CloudProvider::Anthropic, 150.0));

        let providers = router.list_cloud_providers();
        assert_eq!(providers.len(), 2);
    }
}
