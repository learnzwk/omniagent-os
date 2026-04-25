# OmniAgent OS 量化记忆服务规范 (Quantized Memory Service Specification)

> **模块编号**: `omniagent-memory` | **版本**: v0.3.0-draft | **状态**: 设计阶段

---

## 1. 概述 (Purpose)

量化记忆服务是 OmniAgent OS 的认知基础设施，为 Agent 提供三层记忆体系：**工作记忆 (Working Memory, LRU 缓存)**、**情景记忆 (Episodic Memory, 时间索引)** 和 **语义记忆 (Semantic Memory, 向量索引)**。通过 INT8/INT4 量化压缩和 HNSW 向量索引，在保证检索精度的同时大幅降低内存占用。

### 1.1 设计目标

| 目标 | 描述 |
|------|------|
| 三层记忆 | 工作（短期 LRU）、情景（经验时间索引）、语义（长期向量索引） |
| 高压缩比 | INT8 4x 压缩，INT4 8x 压缩，目标 4x+ 综合 |
| 快速检索 | Top-10 召回延迟 < 10ms |
| 本地嵌入 | ONNX Runtime FFI 本地编码，无需外部 API |
| 自动整理 | 后台梦境进程整合工作→情景→语义 |
| 安全隔离 | AES-256-GCM 加密存储，Agent 间访问控制 |

### 1.2 架构总览

```
┌──────────────────────────────────────────────────────┐
│  store(key, memory) / recall(query, top_k)           │
└──────────────────────┬───────────────────────────────┘
┌──────────────────────▼───────────────────────────────┐
│  ┌──────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ Working  │  │  Episodic    │  │   Semantic     │  │
│  │ (LRU)    │  │  (Time-Idx)  │  │  (Vector-Idx)  │  │
│  └────┬─────┘  └──────┬───────┘  └───────┬────────┘  │
│  ┌────▼───────────────▼──────────────────▼─────────┐  │
│  │           Quantizer (INT8/INT4)                  │  │
│  └──────────────────────┬──────────────────────────┘  │
│  ┌──────────────────────▼──────────────────────────┐  │
│  │           HNSW Vector Index                      │  │
│  └──────────────────────┬──────────────────────────┘  │
│  ┌──────────────────────▼──────────────────────────┐  │
│  │     Local Embedder (ONNX Runtime via ort)        │  │
│  └─────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────┐  │
│  │     Dream / Consolidation Process                │  │
│  └─────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

---

## 2. 接口定义 (Interfaces)

### 2.1 核心特征

```rust
/// 记忆存储特征
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn store(&self, key: MemoryKey, memory: MemoryItem) -> Result<MemoryId, MemoryError>;
    async fn recall(&self, query: &str, top_k: usize, filter: Option<MemoryFilter>)
        -> Result<Vec<RecallResult>, MemoryError>;
    async fn delete(&self, memory_id: MemoryId) -> Result<(), MemoryError>;
    async fn get(&self, memory_id: MemoryId) -> Result<Option<MemoryItem>, MemoryError>;
    async fn count(&self, layer: MemoryLayer) -> Result<usize, MemoryError>;
}

/// 量化器特征
pub trait Quantizer: Send + Sync {
    fn quantize(&self, vector: &[f32]) -> Result<QuantizedVector, MemoryError>;
    fn dequantize(&self, quantized: &QuantizedVector) -> Vec<f32>;
    fn similarity(&self, a: &QuantizedVector, b: &QuantizedVector) -> f32;
    fn config(&self) -> &QuantizerConfig;
}

/// 文本嵌入器特征
pub trait Embedder: Send + Sync {
    fn encode(&self, text: &str) -> Result<Vec<f32>, MemoryError>;
    fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, MemoryError>;
    fn dimension(&self) -> usize;
}

/// 向量索引特征
pub trait VectorIndex: Send + Sync {
    fn add(&self, id: MemoryId, vector: QuantizedVector) -> Result<(), MemoryError>;
    fn search(&self, query: &QuantizedVector, top_k: usize, filter: Option<IndexFilter>)
        -> Result<Vec<SearchResult>, MemoryError>;
    fn remove(&self, id: MemoryId) -> Result<(), MemoryError>;
    fn len(&self) -> usize;
    async fn save(&self, path: &Path) -> Result<(), MemoryError>;
    async fn load(&self, path: &Path) -> Result<(), MemoryError>;
}
```

### 2.2 记忆管理器主接口

```rust
pub struct MemoryManager {
    working: Arc<WorkingMemory>, episodic: Arc<EpisodicMemory>,
    semantic: Arc<SemanticMemory>, embedder: Arc<dyn Embedder>,
    quantizer: Arc<dyn Quantizer>, encryption: Arc<MemoryEncryption>,
}

impl MemoryManager {
    pub fn new(config: MemoryConfig) -> Result<Self, MemoryError> {
        let embedder = Arc::new(OnnxEmbedder::new(&config.embedding_model)?);
        let quantizer = Arc::new(ScalarQuantizer::new(config.quantizer.clone()));
        let encryption = Arc::new(MemoryEncryption::new(&config.encryption)?);
        Ok(Self {
            working: Arc::new(WorkingMemory::new(config.working_capacity)),
            episodic: Arc::new(EpisodicMemory::new(config.episodic_capacity, quantizer.clone())),
            semantic: Arc::new(SemanticMemory::new(config.semantic_path.as_deref(), quantizer.clone(), embedder.clone())?),
            embedder, quantizer, encryption,
        })
    }

    /// 存储记忆（自动选择层级 + 加密）
    pub async fn store(&self, key: MemoryKey, memory: MemoryItem) -> Result<MemoryId, MemoryError> {
        let encrypted = self.encryption.encrypt(&memory.content)?;
        let mut item = memory; item.content = encrypted; item.metadata.encrypted = true;
        let layer = self.select_layer(&item);
        match layer {
            MemoryLayer::Working => self.working.store(key, item).await,
            MemoryLayer::Episodic | MemoryLayer::Semantic => {
                let embedding = self.embedder.encode(&memory.content)?;
                let quantized = self.quantizer.quantize(&embedding)?;
                item.embedding = Some(quantized.clone());
                match layer {
                    MemoryLayer::Episodic => self.episodic.store_with_embedding(key, item, quantized).await,
                    _ => self.semantic.store_with_embedding(key, item, quantized).await,
                }
            }
        }
    }

    /// 召回记忆（跨三层并行搜索 + 解密）
    pub async fn recall(&self, query: &str, top_k: usize, filter: Option<MemoryFilter>)
        -> Result<Vec<RecallResult>, MemoryError> {
        let query_emb = self.embedder.encode(query)?;
        let query_q = self.quantizer.quantize(&query_emb)?;
        let (w, e, s) = tokio::join!(
            self.working.recall(&query_q, top_k, filter.as_ref()),
            self.episodic.recall(&query_q, top_k, filter.as_ref()),
            self.semantic.recall(&query_q, top_k, filter.as_ref()),
        );
        let mut all = Vec::new(); all.extend(w?); all.extend(e?); all.extend(s?);
        all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        all.dedup_by(|a, b| a.memory_id == b.memory_id);
        let mut results: Vec<_> = all.into_iter().take(top_k).collect();
        for r in &mut results {
            if r.item.metadata.encrypted { r.item.content = self.encryption.decrypt(&r.item.content)?; }
        }
        Ok(results)
    }

    fn select_layer(&self, item: &MemoryItem) -> MemoryLayer {
        match item.importance { Importance::Transient => MemoryLayer::Working,
            Importance::Experience => MemoryLayer::Episodic,
            Importance::Knowledge => MemoryLayer::Semantic, }
    }
}
```

---

## 3. 数据结构 (Data Structures)

```rust
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryId(Uuid);

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryKey { pub agent_id: AgentId, pub namespace: String, pub local_key: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryLayer { Working, Episodic, Semantic }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub content: String, pub metadata: MemoryMetadata,
    pub embedding: Option<QuantizedVector>, pub importance: Importance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub created_at: SystemTime, pub last_accessed: SystemTime,
    pub source_agent: AgentId, pub source_type: MemorySource,
    pub confidence: f32, pub access_count: u64, pub encrypted: bool,
    pub tags: Vec<String>, pub related: Vec<MemoryId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemorySource { UserInput, AgentReasoning, SystemObservation, ExternalImport, DreamConsolidation }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Importance { Transient, Experience, Knowledge }

#[derive(Debug, Clone)]
pub struct RecallResult { pub memory_id: MemoryId, pub item: MemoryItem, pub score: f32, pub layer: MemoryLayer }

#[derive(Debug, Clone, Default)]
pub struct MemoryFilter {
    pub layer: Option<MemoryLayer>, pub source: Option<MemorySource>,
    pub time_range: Option<(SystemTime, SystemTime)>,
    pub min_confidence: Option<f32>, pub tags: Vec<String>,
}

/// 量化向量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedVector {
    pub data: QuantizedData, pub dimension: usize,
    pub config: QuantizerConfig, pub scale: f32, pub zero_point: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizedData { Int8(Vec<i8>), Int4(Vec<i8>), Float32(Vec<f32>) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizerConfig { pub precision: QuantPrecision, pub asymmetric: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantPrecision { Int8, Int4, None }

impl QuantPrecision {
    pub fn compression_ratio(&self) -> f32 {
        match self { Self::Int8 => 4.0, Self::Int4 => 8.0, Self::None => 1.0 }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult { pub id: MemoryId, pub score: f32, pub distance: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    pub max_connections: usize, pub ef_construction: usize,
    pub ef_search: usize, pub metric: DistanceMetric,
}

impl Default for HnswConfig {
    fn default() -> Self { Self { max_connections: 16, ef_construction: 200, ef_search: 50, metric: DistanceMetric::Cosine } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric { Cosine, Euclidean, DotProduct, Hamming }
```

---

## 4. 量化器实现 (Quantizer)

```rust
pub struct ScalarQuantizer { config: QuantizerConfig }

impl Quantizer for ScalarQuantizer {
    fn quantize(&self, vector: &[f32]) -> Result<QuantizedVector, MemoryError> {
        let dim = vector.len();
        let (min_val, max_val) = vector.iter().cloned().fold((f32::INFINITY, f32::NEG_INFINITY),
            |(mn, mx), v| (mn.min(v), mx.max(v)));
        let scale = (max_val - min_val) / 255.0;
        let zero_point = (-min_val / scale).round() as i32;

        let data = match self.config.precision {
            QuantPrecision::Int8 => QuantizedData::Int8(
                vector.iter().map(|&v| ((v / scale + zero_point as f32).round() as i32)
                    .clamp(i8::MIN as i32, i8::MAX as i32) as i8).collect()),
            QuantPrecision::Int4 => {
                let mut packed = Vec::with_capacity((dim + 1) / 2);
                let mut i = 0;
                while i < dim {
                    let v1 = ((vector[i] / scale + zero_point as f32).round() as i32).clamp(0, 15) as i8;
                    let v2 = if i + 1 < dim { ((vector[i+1] / scale + zero_point as f32).round() as i32).clamp(0, 15) as i8 } else { 0 };
                    packed.push((v1 << 4) | v2); i += 2;
                }
                QuantizedData::Int4(packed)
            }
            QuantPrecision::None => QuantizedData::Float32(vector.to_vec()),
        };
        Ok(QuantizedVector { data, dimension: dim, config: self.config.clone(), scale, zero_point })
    }

    fn dequantize(&self, q: &QuantizedVector) -> Vec<f32> {
        match &q.data {
            QuantizedData::Int8(d) => d.iter().map(|&v| (v as f32 - q.zero_point as f32) * q.scale).collect(),
            QuantizedData::Int4(d) => {
                let mut r = Vec::with_capacity(q.dimension);
                for &p in d { r.push(((p >> 4) as f32 - q.zero_point as f32) * q.scale);
                    if r.len() < q.dimension { r.push(((p & 0x0F) as f32 - q.zero_point as f32) * q.scale); } }
                r
            }
            QuantizedData::Float32(d) => d.clone(),
        }
    }

    fn similarity(&self, a: &QuantizedVector, b: &QuantizedVector) -> f32 {
        let av = self.dequantize(a); let bv = self.dequantize(b);
        let dot: f32 = av.iter().zip(bv.iter()).map(|(x, y)| x * y).sum();
        let na: f32 = av.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = bv.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
    }

    fn config(&self) -> &QuantizerConfig { &self.config }
}
```

---

## 5. 本地嵌入器 (Local Embedder)

```rust
/// 基于 ONNX Runtime FFI (ort crate) 的本地嵌入器
pub struct OnnxEmbedder {
    session: ort::Session, tokenizer: Arc<dyn Tokenizer>,
    dimension: usize, max_length: usize,
}

impl OnnxEmbedder {
    pub fn new(model_path: &str) -> Result<Self, MemoryError> {
        let session = ort::Session::builder()?
            .with_optimization_level(ort::GraphOptimizationLevel::ORT_ENABLE_ALL)?
            .with_intra_threads(4)?.commit_from_file(model_path)?;
        let tokenizer = Arc::new(SentencePieceTokenizer::load(
            &model_path.replace(".onnx", "_tokenizer.json"))?);
        Ok(Self { session, tokenizer, dimension: 768, max_length: 512 })
    }
}

impl Embedder for OnnxEmbedder {
    fn encode(&self, text: &str) -> Result<Vec<f32>, MemoryError> {
        let token_ids = self.tokenizer.encode(text, self.max_length)?;
        let input_ids = ort::Value::from_array(&[token_ids.as_slice()], &[1, token_ids.len()])?;
        let attn_mask = ort::Value::from_array(&[&vec![1i64; token_ids.len()]], &[1, token_ids.len()])?;
        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids, "attention_mask" => attn_mask]?)?;
        let hidden = outputs["last_hidden_state"].try_extract_tensor::<f32>()?;
        Ok(l2_normalize(&mean_pooling(hidden.view())))
    }

    fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, MemoryError> {
        texts.iter().map(|t| self.encode(t)).collect()
    }
    fn dimension(&self) -> usize { self.dimension }
}
```

---

## 6. HNSW 向量索引 (HNSW Vector Index)

```rust
/// 纯 Rust HNSW 图索引
pub struct HnswIndex {
    layers: Vec<RwLock<HnswLayer>>, entry_point: RwLock<Option<MemoryId>>,
    max_level: AtomicUsize, config: HnswConfig,
    nodes: DashMap<MemoryId, HnswNode>, distance_fn: Arc<dyn DistanceFunction>,
}

struct HnswLayer { neighbors: HashMap<MemoryId, Vec<MemoryId>> }
struct HnswNode { id: MemoryId, vector: QuantizedVector, level: usize }

impl HnswIndex {
    pub fn new(config: HnswConfig) -> Self {
        Self { layers: vec![RwLock::new(HnswLayer::new())],
            entry_point: RwLock::new(None), max_level: AtomicUsize::new(0),
            config, nodes: DashMap::new(),
            distance_fn: match config.metric {
                DistanceMetric::Cosine => Arc::new(CosineDistance),
                DistanceMetric::Euclidean => Arc::new(EuclideanDistance),
                _ => Arc::new(CosineDistance),
            },
        }
    }

    /// 添加向量（多层图构建）
    pub fn add(&self, id: MemoryId, vector: QuantizedVector) -> Result<(), MemoryError> {
        let level = self.random_level();
        while self.layers.len() <= level { self.layers.push(RwLock::new(HnswLayer::new())); }
        self.nodes.insert(id.clone(), HnswNode { id: id.clone(), vector, level });
        // 从顶层向下搜索并建立邻接关系（简化）
        let entry = self.entry_point.read().unwrap().clone();
        if entry.is_none() { *self.entry_point.write().unwrap() = Some(id); }
        Ok(())
    }

    /// 搜索最相似的 k 个向量
    pub fn search(&self, query: &QuantizedVector, top_k: usize, _filter: Option<IndexFilter>)
        -> Result<Vec<SearchResult>, MemoryError> {
        let entry = self.entry_point.read().unwrap().clone().ok_or(MemoryError::IndexEmpty)?;
        let mut candidates = BinaryHeap::new_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        let entry_node = self.nodes.get(&entry).unwrap();
        let dist = self.distance_fn.distance(&entry_node.vector, query);
        candidates.push(SearchResult { id: entry, score: 1.0 - dist, distance: dist });
        // 贪心搜索（简化）
        let mut visited = HashSet::new(); visited.insert(entry);
        while let Some(current) = candidates.pop() {
            let layer = self.layers[0].read().unwrap();
            if let Some(neighbors) = layer.neighbors.get(&current.id) {
                for nid in neighbors {
                    if visited.contains(nid) { continue; } visited.insert(nid.clone());
                    if let Some(n) = self.nodes.get(nid) {
                        let d = self.distance_fn.distance(&n.vector, query);
                        candidates.push(SearchResult { id: nid.clone(), score: 1.0 - d, distance: d });
                    }
                }
            }
        }
        let mut results: Vec<_> = candidates.into_iter().take(top_k).collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        Ok(results)
    }

    fn random_level(&self) -> usize {
        let m = self.config.max_connections as f64;
        (-(rand::random::<f64>() * m.ln())).floor() as usize
    }
}

pub trait DistanceFunction: Send + Sync { fn distance(&self, a: &QuantizedVector, b: &QuantizedVector) -> f32; }
```

---

## 7. 梦境整合进程 (Dream/Consolidation)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamConfig {
    pub interval: Duration, pub forgetting_threshold: f32,
    pub min_access_count: u64, pub working_to_episodic_threshold: usize,
    pub episodic_to_semantic_confidence: f32, pub batch_size: usize,
}

impl Default for DreamConfig {
    fn default() -> Self {
        Self { interval: Duration::from_secs(300), forgetting_threshold: 0.2,
            min_access_count: 2, working_to_episodic_threshold: 100,
            episodic_to_semantic_confidence: 0.8, batch_size: 50 }
    }
}

pub struct DreamProcess {
    working: Arc<WorkingMemory>, episodic: Arc<EpisodicMemory>,
    semantic: Arc<SemanticMemory>, quantizer: Arc<dyn Quantizer>,
    embedder: Arc<dyn Embedder>, config: DreamConfig, shutdown: Arc<AtomicBool>,
}

impl DreamProcess {
    pub fn start(&mut self) -> Result<(), MemoryError> {
        let shutdown = self.shutdown.clone(); let cfg = self.config.clone();
        let (w, e, s, q, emb) = (self.working.clone(), self.episodic.clone(),
            self.semantic.clone(), self.quantizer.clone(), self.embedder.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cfg.interval);
            loop {
                tokio::select! { _ = interval.tick() => {
                    // 阶段1: 工作→情景记忆转移
                    for (key, item) in w.get_eviction_candidates(0.8).into_iter().take(cfg.batch_size) {
                        if item.metadata.access_count >= cfg.min_access_count as u64 {
                            let emb_v = emb.encode(&item.content).unwrap();
                            let qv = q.quantize(&emb_v).unwrap();
                            e.store_with_embedding(key.clone(), item, qv).await.unwrap();
                        }
                    }
                    // 阶段2: 情景→语义记忆升级（高置信度）
                    // 阶段3: 遗忘低价值记忆
                } _ = tokio::signal::ctrl_c() => { break; } }
                if shutdown.load(Ordering::Relaxed) { break; }
            }
        });
        Ok(())
    }
}
```

---

## 8. 安全设计 (Security)

### 8.1 记忆加密 (AES-256-GCM)

```rust
pub struct MemoryEncryption { cipher: Aes256Gcm, key_id: String }

impl MemoryEncryption {
    pub fn new(config: &EncryptionConfig) -> Result<Self, MemoryError> {
        let key = config.load_key()?;
        Ok(Self { cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key)), key_id: config.key_id.clone() })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, MemoryError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher.encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| MemoryError::EncryptionError(e.to_string()))?;
        let mut output = nonce.to_vec(); output.extend_from_slice(&ciphertext);
        Ok(base64::encode(&output))
    }

    pub fn decrypt(&self, encrypted: &str) -> Result<String, MemoryError> {
        let data = base64::decode(encrypted).map_err(|e| MemoryError::DecryptionError(e.to_string()))?;
        let nonce = GenericArray::from_slice(&data[..12]);
        self.cipher.decrypt(nonce, &data[12..])
            .map_err(|e| MemoryError::DecryptionError(e.to_string()))
            .and_then(|p| String::from_utf8(p).map_err(|e| MemoryError::DecryptionError(e.to_string())))
    }
}
```

### 8.2 Agent 访问控制

```rust
pub struct MemoryAccessControl { acl: RwLock<HashMap<AgentId, MemoryAclEntry>> }

impl MemoryAccessControl {
    pub async fn check_access(&self, agent_id: &AgentId, memory_id: &MemoryId,
        perm: MemoryPermission) -> Result<bool, MemoryError> {
        let acl = self.acl.read().await;
        if let Some(entry) = acl.get(agent_id) {
            Ok(match perm { MemoryPermission::Read => true,
                MemoryPermission::Write => entry.can_write,
                MemoryPermission::Delete => entry.can_delete, })
        } else { Ok(false) }
    }
}
```

---

## 9. 错误处理 (Error Handling)

```rust
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("记忆不存在: {0}")] NotFound(MemoryId),
    #[error("存储容量已满")] CapacityExceeded,
    #[error("嵌入生成失败: {0}")] EmbeddingFailed(String),
    #[error("量化失败: {0}")] QuantizationFailed(String),
    #[error("索引为空")] IndexEmpty,
    #[error("加密失败: {0}")] EncryptionError(String),
    #[error("解密失败: {0}")] DecryptionError(String),
    #[error("ONNX Runtime 错误: {0}")] OnnxError(String),
    #[error("访问被拒绝: Agent {agent_id:?} 无权访问记忆 {memory_id:?}")]
    AccessDenied { agent_id: AgentId, memory_id: MemoryId },
}
```

---

## 10. 性能约束 (Performance Constraints)

| 操作 | 目标延迟 | 最大延迟 | 备注 |
|------|---------|---------|------|
| 记忆存储 (store) | < 1ms | < 5ms | 含加密和嵌入 |
| 记忆召回 (recall top-10) | < 10ms | < 50ms | 跨三层搜索 |
| 量化 1MB 向量 (INT8) | < 100ms | < 200ms | 4x 压缩 |
| 量化 1MB 向量 (INT4) | < 80ms | < 150ms | 8x 压缩 |
| 嵌入生成 (单条 768d) | < 20ms | < 50ms | ONNX Runtime |
| HNSW 搜索 (100 万向量) | < 5ms | < 10ms | top-10 |

### 内存占用估算

| 量化精度 | 768 维向量大小 | 100 万向量总大小 | 压缩比 |
|----------|---------------|-----------------|--------|
| Float32 | 3,072 B | 2.86 GB | 1x |
| INT8 | 768 B | 715 MB | 4x |
| INT4 | 384 B | 357 MB | 8x |

---

## 11. 测试用例 (Test Cases)

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_int8_quantization_roundtrip() {
        let q = ScalarQuantizer::new(QuantizerConfig { precision: QuantPrecision::Int8, asymmetric: true });
        let original: Vec<f32> = (0..768).map(|i| i as f32 / 768.0).collect();
        let quantized = q.quantize(&original).unwrap();
        let recovered = q.dequantize(&quantized);
        assert_eq!(recovered.len(), original.len());
        let max_error: f32 = original.iter().zip(recovered.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(max_error < 0.01, "最大量化误差: {}", max_error);
    }

    #[test]
    fn test_int4_compression() {
        let q = ScalarQuantizer::new(QuantizerConfig { precision: QuantPrecision::Int4, asymmetric: true });
        let quantized = q.quantize(&vec![0.5f32; 768]).unwrap();
        if let QuantizedData::Int4(d) = &quantized.data { assert_eq!(d.len(), 384); } else { panic!(); }
    }

    #[test]
    fn test_similarity_preserved() {
        let q = ScalarQuantizer::new(QuantizerConfig { precision: QuantPrecision::Int8, asymmetric: true });
        let a: Vec<f32> = (0..768).map(|i| (i % 10) as f32 / 10.0).collect();
        let b: Vec<f32> = (0..768).map(|i| ((i + 1) % 10) as f32 / 10.0).collect();
        let c: Vec<f32> = (0..768).map(|i| ((i + 100) % 10) as f32 / 10.0).collect();
        assert!(q.similarity(&q.quantize(&a).unwrap(), &q.quantize(&b).unwrap())
            > q.similarity(&q.quantize(&a).unwrap(), &q.quantize(&c).unwrap()));
    }

    #[tokio::test]
    async fn test_store_and_recall() {
        let mgr = MemoryManager::new(MemoryConfig::test_default()).unwrap();
        let key = MemoryKey::new("agent", "general", "test-1");
        let item = MemoryItem { content: "学习了 Rust 所有权系统".into(),
            metadata: MemoryMetadata { confidence: 0.9, tags: vec!["rust".into()], ..Default::default() },
            importance: Importance::Experience, ..Default::default() };
        let id = mgr.store(key, item).await.unwrap();
        let results = mgr.recall("Rust 所有权", 5, None).await.unwrap();
        assert!(results.iter().any(|r| r.memory_id == id));
    }

    #[tokio::test]
    async fn test_recall_with_filter() {
        let mgr = MemoryManager::new(MemoryConfig::test_default()).unwrap();
        for i in 0..10 {
            let key = MemoryKey::new("a", "t", format!("m-{}", i));
            let item = MemoryItem { content: format!("记忆 {}", i),
                metadata: MemoryMetadata { tags: if i < 5 { vec!["important".into()] } else { vec!["trivial".into()] },
                    confidence: if i < 5 { 0.9 } else { 0.3 }, ..Default::default() },
                ..Default::default() };
            mgr.store(key, item).await.unwrap();
        }
        let filter = MemoryFilter { tags: vec!["important".into()], min_confidence: Some(0.8), ..Default::default() };
        assert!(mgr.recall("记忆", 10, Some(filter)).await.unwrap().len() <= 5);
    }

    #[tokio::test]
    async fn test_hnsw_search_accuracy() {
        let idx = HnswIndex::new(HnswConfig::default());
        let q = ScalarQuantizer::new(QuantizerConfig::int8());
        for i in 0..1000 {
            let v: Vec<f32> = (0..128).map(|j| ((i * 13 + j * 7) % 100) as f32 / 100.0).collect();
            idx.add(MemoryId::from_index(i), q.quantize(&v).unwrap()).unwrap();
        }
        let query: Vec<f32> = (0..128).map(|j| (j % 100) as f32 / 100.0).collect();
        let results = idx.search(&q.quantize(&query).unwrap(), 10, None).unwrap();
        assert_eq!(results.len(), 10);
        for i in 0..results.len() - 1 { assert!(results[i].score >= results[i + 1].score); }
    }

    #[tokio::test]
    async fn test_dream_consolidation() {
        let mut mgr = MemoryManager::new(MemoryConfig::test_default()).unwrap();
        for i in 0..20 {
            let key = MemoryKey::new("a", "w", format!("w-{}", i));
            let item = MemoryItem { content: format!("工作记忆 {}", i),
                metadata: MemoryMetadata { access_count: 5, confidence: 0.8, ..Default::default() },
                importance: Importance::Transient, ..Default::default() };
            mgr.store(key, item).await.unwrap();
        }
        mgr.start_dream(DreamConfig { interval: Duration::from_millis(100), ..Default::default() }).unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(mgr.episodic.count().await.unwrap() > 0);
    }
}
```

---

## 12. 配置参考

```toml
[memory]
working_capacity = 1000
episodic_capacity = 100_000
semantic_path = "/var/lib/omniagent/memory/semantic"

[memory.quantizer]
precision = "int8"
asymmetric = true

[memory.embedding]
model_path = "/usr/share/omniagent/models/embedding.onnx"
max_length = 512
inference_threads = 4

[memory.hnsw]
max_connections = 16
ef_construction = 200
ef_search = 50
metric = "cosine"

[memory.dream]
interval = "5m"
forgetting_threshold = 0.2
min_access_count = 2
batch_size = 50

[memory.encryption]
algorithm = "aes-256-gcm"
key_id = "memory-master-key"
```

---

> **文档版本**: v0.3.0-draft | **最后更新**: 2026-04-25 | **作者**: OmniAgent OS 认知架构团队
