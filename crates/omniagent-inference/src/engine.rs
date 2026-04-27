// OmniAgent OS Phase 10: 推理引擎 trait 和推理管理器
// 定义统一的推理接口，管理多个推理引擎的生命周期

use std::collections::HashMap;

use crate::router::ModelRouter;
use crate::types::*;

/// 推理引擎 trait，所有推理后端（本地/云端）必须实现此接口
pub trait InferenceEngine: Send + Sync {
    /// 返回引擎名称
    fn name(&self) -> &str;

    /// 返回引擎提供者标识
    fn provider(&self) -> InferenceProvider;

    /// 返回支持的任务类型列表
    fn supported_tasks(&self) -> Vec<InferenceTask>;

    /// 检查引擎是否可用
    fn is_available(&self) -> bool;

    /// 执行推理
    fn infer(&self, request: &ModelRequest) -> Result<InferenceResult, InferenceError>;
}

/// 推理管理器，统一管理所有推理引擎和路由决策
pub struct InferenceManager {
    /// 已注册的推理引擎，键为引擎名称
    engines: HashMap<String, Box<dyn InferenceEngine>>,
    /// 模型路由器
    router: ModelRouter,
    /// 总推理次数
    inference_count: u64,
    /// 总延迟（毫秒）
    total_latency_ms: u64,
}

impl InferenceManager {
    /// 创建新的推理管理器
    pub fn new(preference: InferencePreference) -> Self {
        Self {
            engines: HashMap::new(),
            router: ModelRouter::new(preference),
            inference_count: 0,
            total_latency_ms: 0,
        }
    }

    /// 注册推理引擎
    pub fn register_engine(&mut self, name: &str, engine: Box<dyn InferenceEngine>) {
        self.engines.insert(name.to_string(), engine);
    }

    /// 注销推理引擎
    pub fn unregister_engine(&mut self, name: &str) -> bool {
        self.engines.remove(name).is_some()
    }

    /// 检查是否有指定名称的引擎
    pub fn has_engine(&self, name: &str) -> bool {
        self.engines.contains_key(name)
    }

    /// 获取引擎数量
    pub fn engines_count(&self) -> usize {
        self.engines.len()
    }

    /// 执行推理
    ///
    /// 流程：
    /// 1. 通过路由器获取路由决策
    /// 2. 根据决策选择合适的引擎
    /// 3. 执行推理并返回结果
    pub fn infer(&mut self, request: &ModelRequest) -> Result<InferenceResult, InferenceError> {
        let decision = self.router.route(request);

        // 根据路由决策查找合适的引擎
        let engine_name = self.find_engine_for_decision(&decision, request)?;

        let engine = self
            .engines
            .get(&engine_name)
            .ok_or_else(|| InferenceError::ModelNotLoaded(engine_name.clone()))?;

        if !engine.is_available() {
            return Err(InferenceError::ModelNotLoaded(engine_name));
        }

        // 检查任务是否支持
        let supported = engine.supported_tasks();
        if !supported.iter().any(|t| t == &request.task) {
            return Err(InferenceError::UnsupportedTask(request.task.clone()));
        }

        // 执行推理
        let result = engine.infer(request)?;

        // 更新统计
        self.inference_count += 1;
        self.total_latency_ms += result.latency_ms as u64;

        Ok(result)
    }

    /// 获取路由器的不可变引用
    pub fn router(&self) -> &ModelRouter {
        &self.router
    }

    /// 获取路由器的可变引用
    pub fn router_mut(&mut self) -> &mut ModelRouter {
        &mut self.router
    }

    /// 获取推理统计信息
    pub fn stats(&self) -> InferenceStats {
        InferenceStats {
            total_inferences: self.inference_count,
            avg_latency_ms: if self.inference_count > 0 {
                self.total_latency_ms as f64 / self.inference_count as f64
            } else {
                0.0
            },
            engines_count: self.engines.len(),
            local_models_count: self.router.local_models_count(),
            cloud_providers_count: self.router.cloud_providers_count(),
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.inference_count = 0;
        self.total_latency_ms = 0;
    }

    /// 根据路由决策查找合适的引擎名称
    fn find_engine_for_decision(
        &self,
        decision: &RoutingDecision,
        request: &ModelRequest,
    ) -> Result<String, InferenceError> {
        match decision {
            RoutingDecision::LocalCandle { model_id } => {
                self.find_local_engine(model_id, InferenceProvider::Candle)
            }
            RoutingDecision::LocalOnnx { model_id } => {
                self.find_local_engine(model_id, InferenceProvider::OnnxRuntime)
            }
            RoutingDecision::LocalTract { model_id } => {
                self.find_local_engine(model_id, InferenceProvider::Tract)
            }
            RoutingDecision::Cloud { provider, model_id } => {
                self.find_cloud_engine(provider, model_id)
            }
            RoutingDecision::Fallback { primary, fallback } => {
                // 尝试主路径
                match self.find_engine_for_decision(primary, request) {
                    Ok(name) => Ok(name),
                    Err(_) => self.find_engine_for_decision(fallback, request),
                }
            }
        }
    }

    /// 查找本地引擎
    fn find_local_engine(
        &self,
        model_id: &ModelId,
        provider: InferenceProvider,
    ) -> Result<String, InferenceError> {
        // 查找匹配提供者和模型的引擎
        for (name, engine) in &self.engines {
            if engine.provider() == provider && engine.is_available() {
                return Ok(name.clone());
            }
        }
        Err(InferenceError::ModelNotFound(model_id.to_string()))
    }

    /// 查找云端引擎
    fn find_cloud_engine(
        &self,
        provider: &CloudProvider,
        model_id: &ModelId,
    ) -> Result<String, InferenceError> {
        let expected_provider = match provider {
            CloudProvider::OpenAI => InferenceProvider::OpenAI,
            CloudProvider::Anthropic => InferenceProvider::Anthropic,
            CloudProvider::Google => InferenceProvider::Google,
            _ => return Err(InferenceError::ModelNotFound(model_id.to_string())),
        };

        for (name, engine) in &self.engines {
            if engine.provider() == expected_provider && engine.is_available() {
                return Ok(name.clone());
            }
        }
        Err(InferenceError::ModelNotFound(model_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用于测试的模拟推理引擎
    struct MockEngine {
        name: String,
        provider: InferenceProvider,
        tasks: Vec<InferenceTask>,
        available: bool,
        result_text: String,
    }

    impl MockEngine {
        fn new(name: &str, provider: InferenceProvider) -> Self {
            Self {
                name: name.to_string(),
                provider,
                tasks: vec![InferenceTask::TextGeneration],
                available: true,
                result_text: format!("mock result from {}", name),
            }
        }

        fn with_tasks(mut self, tasks: Vec<InferenceTask>) -> Self {
            self.tasks = tasks;
            self
        }

        fn unavailable(mut self) -> Self {
            self.available = false;
            self
        }

        fn with_result(mut self, text: &str) -> Self {
            self.result_text = text.to_string();
            self
        }
    }

    impl InferenceEngine for MockEngine {
        fn name(&self) -> &str {
            &self.name
        }

        fn provider(&self) -> InferenceProvider {
            self.provider
        }

        fn supported_tasks(&self) -> Vec<InferenceTask> {
            self.tasks.clone()
        }

        fn is_available(&self) -> bool {
            self.available
        }

        fn infer(&self, _request: &ModelRequest) -> Result<InferenceResult, InferenceError> {
            Ok(InferenceResult::new(
                InferenceOutput::Text(self.result_text.clone()),
                ModelId::new(&self.name),
                self.provider,
            )
            .with_latency(42)
            .with_tokens(TokenUsage::new(10, 20)))
        }
    }

    #[test]
    fn test_manager_new() {
        let manager = InferenceManager::new(InferencePreference::Auto);
        assert_eq!(manager.engines_count(), 0);
        let stats = manager.stats();
        assert_eq!(stats.total_inferences, 0);
        assert_eq!(stats.engines_count, 0);
    }

    #[test]
    fn test_register_engine() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);
        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle);
        manager.register_engine("candle-llama", Box::new(engine));

        assert!(manager.has_engine("candle-llama"));
        assert_eq!(manager.engines_count(), 1);
    }

    #[test]
    fn test_unregister_engine() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);
        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle);
        manager.register_engine("candle-llama", Box::new(engine));

        assert!(manager.unregister_engine("candle-llama"));
        assert!(!manager.has_engine("candle-llama"));
        assert_eq!(manager.engines_count(), 0);
        assert!(!manager.unregister_engine("nonexistent"));
    }

    #[test]
    fn test_infer_with_local_engine() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        // 注册本地模型
        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        // 注册引擎
        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle);
        manager.register_engine("candle-llama", Box::new(engine));

        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        );
        let result = manager.infer(&request).unwrap();

        assert_eq!(
            result.output,
            InferenceOutput::Text("mock result from candle-llama".to_string())
        );
        assert_eq!(result.latency_ms, 42);
        assert_eq!(result.tokens_used, Some(TokenUsage::new(10, 20)));
    }

    #[test]
    fn test_infer_updates_stats() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle);
        manager.register_engine("candle-llama", Box::new(engine));

        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        );

        // 执行多次推理
        manager.infer(&request).unwrap();
        manager.infer(&request).unwrap();
        manager.infer(&request).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.total_inferences, 3);
        assert!((stats.avg_latency_ms - 42.0).abs() < 0.01);
        assert_eq!(stats.engines_count, 1);
        assert_eq!(stats.local_models_count, 1);
    }

    #[test]
    fn test_infer_unsupported_task() {
        let mut manager = InferenceManager::new(InferencePreference::LocalOnly);

        // 引擎只支持 TextGeneration，注册为本地模型
        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle)
            .with_tasks(vec![InferenceTask::TextGeneration]);
        manager.register_engine("candle-llama", Box::new(engine));

        // 请求 TextEmbedding 任务，本地模型不支持该任务
        let request = ModelRequest::new(
            InferenceTask::TextEmbedding,
            InferenceInput::Text("hello".to_string()),
        );
        let result = manager.infer(&request);

        // LocalOnly 偏好下，没有匹配的本地模型，路由器返回回退决策
        // 但 LocalOnly 不会回退到云端，所以最终没有可用引擎
        assert!(result.is_err());
    }

    #[test]
    fn test_infer_engine_not_available() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        // 引擎不可用
        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle).unavailable();
        manager.register_engine("candle-llama", Box::new(engine));

        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        );
        let result = manager.infer(&request);

        assert!(result.is_err());
    }

    #[test]
    fn test_infer_no_engine_registered() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        );
        let result = manager.infer(&request);

        assert!(result.is_err());
    }

    #[test]
    fn test_stats_initial() {
        let manager = InferenceManager::new(InferencePreference::Auto);
        let stats = manager.stats();

        assert_eq!(stats.total_inferences, 0);
        assert_eq!(stats.avg_latency_ms, 0.0);
        assert_eq!(stats.engines_count, 0);
        assert_eq!(stats.local_models_count, 0);
        assert_eq!(stats.cloud_providers_count, 0);
    }

    #[test]
    fn test_reset_stats() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_latency(50.0)
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        let engine = MockEngine::new("candle-llama", InferenceProvider::Candle);
        manager.register_engine("candle-llama", Box::new(engine));

        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        );
        manager.infer(&request).unwrap();
        manager.infer(&request).unwrap();

        manager.reset_stats();
        let stats = manager.stats();
        assert_eq!(stats.total_inferences, 0);
        assert_eq!(stats.avg_latency_ms, 0.0);
    }

    #[test]
    fn test_router_access() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        // 通过 router_mut 注册模型
        let model = LocalModelInfo::new(
            ModelId::new("llama-7b"),
            LocalBackend::Candle,
            "/models/llama-7b".to_string(),
        )
        .with_tasks(vec![InferenceTask::TextGeneration])
        .with_availability(ModelAvailability::Available);
        manager.router_mut().register_local_model(model);

        // 通过 router 读取
        assert_eq!(manager.router().local_models_count(), 1);
    }

    #[test]
    fn test_infer_with_cloud_engine() {
        let mut manager = InferenceManager::new(InferencePreference::Auto);

        // 注册云端提供商
        let provider = CloudProviderInfo::new(
            CloudProvider::OpenAI,
            "https://api.openai.com/v1".to_string(),
            "openai_key".to_string(),
        )
        .with_models(vec![ModelId::new("gpt-4")])
        .with_latency(200.0)
        .configured();
        manager.router_mut().register_cloud_provider(provider);

        // 注册云端引擎
        let engine = MockEngine::new("openai-gpt4", InferenceProvider::OpenAI)
            .with_result("Hello from GPT-4");
        manager.register_engine("openai-gpt4", Box::new(engine));

        let request = ModelRequest::new(
            InferenceTask::TextGeneration,
            InferenceInput::Text("hello".to_string()),
        )
        .with_preference(InferencePreference::CloudOnly);
        let result = manager.infer(&request).unwrap();

        assert_eq!(
            result.output,
            InferenceOutput::Text("Hello from GPT-4".to_string())
        );
    }
}
