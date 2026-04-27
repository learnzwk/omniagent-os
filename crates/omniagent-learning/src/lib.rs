//! # OmniAgent Learning - 高级学习服务
//!
//! 本 crate 实现了 Agent 的学习、记忆和进化能力，包括：
//! - **记忆系统**：支持短期、长期、工作、情景和语义记忆，提供相似度搜索和记忆巩固
//! - **知识图谱**：基于三元组的知识表示与查询
//! - **学习引擎**：支持多种学习策略，管理学习会话与指标追踪

use core::sync::atomic::{AtomicU64, Ordering};
use std::collections::{HashMap, HashSet};

// ============================================================================
// 错误类型
// ============================================================================

/// 学习服务错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningError {
    /// 会话未找到
    SessionNotFound(String),
    /// 会话已存在
    SessionAlreadyExists(String),
    /// 会话未激活
    SessionNotActive(String),
    /// 无效配置
    InvalidConfig(String),
    /// 记忆操作错误
    MemoryError(String),
    /// 知识图谱操作错误
    KnowledgeError(String),
}

// ============================================================================
// 记忆系统
// ============================================================================

/// 记忆类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemoryType {
    /// 短期记忆 (会话内)
    ShortTerm = 0,
    /// 长期记忆 (持久化)
    LongTerm = 1,
    /// 工作记忆 (当前任务)
    Working = 2,
    /// 情景记忆 (事件)
    Episodic = 3,
    /// 语义记忆 (知识)
    Semantic = 4,
}

/// 记忆条目
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// 唯一标识符
    pub id: u64,
    /// 记忆类型
    pub memory_type: MemoryType,
    /// 记忆内容
    pub content: String,
    /// 向量嵌入 (用于相似度搜索)
    pub embedding: Option<Vec<f32>>,
    /// 重要性分数 (0.0-1.0)
    pub importance: f32,
    /// 访问次数
    pub access_count: u32,
    /// 创建时间戳
    pub created_at: u64,
    /// 最后访问时间戳
    pub last_accessed: u64,
    /// 过期时间戳 (None 表示永不过期)
    pub expires_at: Option<u64>,
    /// 标签列表
    pub tags: Vec<String>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 记忆统计信息
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// 总条目数
    pub total_entries: usize,
    /// 短期记忆数量
    pub short_term_count: usize,
    /// 长期记忆数量
    pub long_term_count: usize,
    /// 总重要性分数
    pub total_importance: f32,
    /// 平均重要性分数
    pub avg_importance: f32,
}

/// 记忆存储
pub struct MemoryStore {
    /// 所有记忆条目
    entries: HashMap<u64, MemoryEntry>,
    /// 短期记忆 ID 列表
    short_term: Vec<u64>,
    /// 长期记忆 ID 列表
    long_term: Vec<u64>,
    /// 下一个可用 ID
    next_id: AtomicU64,
}

impl MemoryStore {
    /// 创建新的记忆存储
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            short_term: Vec::new(),
            long_term: Vec::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// 存储记忆，返回分配的 ID
    pub fn store(&mut self, mut entry: MemoryEntry) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        entry.id = id;

        // 根据记忆类型分类存储
        match entry.memory_type {
            MemoryType::ShortTerm => {
                self.short_term.push(id);
            }
            MemoryType::LongTerm => {
                self.long_term.push(id);
            }
            _ => {
                // 工作记忆、情景记忆、语义记忆暂不加入特定列表
            }
        }

        self.entries.insert(id, entry);
        id
    }

    /// 检索记忆 (按 ID)
    pub fn retrieve(&self, id: u64) -> Option<&MemoryEntry> {
        self.entries.get(&id)
    }

    /// 搜索记忆 (按标签)，返回包含所有指定标签的记忆
    pub fn search_by_tags(&self, tags: &[&str]) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|entry| {
                let entry_tags: HashSet<&str> = entry.tags.iter().map(|s| s.as_str()).collect();
                tags.iter().all(|tag| entry_tags.contains(*tag))
            })
            .collect()
    }

    /// 搜索记忆 (按内容关键词)，不区分大小写
    pub fn search_by_keyword(&self, keyword: &str) -> Vec<&MemoryEntry> {
        let keyword_lower = keyword.to_lowercase();
        self.entries
            .values()
            .filter(|entry| entry.content.to_lowercase().contains(&keyword_lower))
            .collect()
    }

    /// 相似度搜索 (基于嵌入向量)，返回最相似的前 top_k 条记忆
    pub fn search_by_similarity(&self, embedding: &[f32], top_k: usize) -> Vec<&MemoryEntry> {
        let mut scored: Vec<(&MemoryEntry, f32)> = self
            .entries
            .values()
            .filter_map(|entry| {
                entry.embedding.as_ref().map(|e| {
                    let sim = cosine_similarity(embedding, e);
                    (entry, sim)
                })
            })
            .collect();

        // 按相似度降序排列
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored.into_iter().map(|(entry, _)| entry).collect()
    }

    /// 删除记忆，返回是否成功
    pub fn delete(&mut self, id: u64) -> bool {
        if self.entries.remove(&id).is_some() {
            self.short_term.retain(|&sid| sid != id);
            self.long_term.retain(|&lid| lid != id);
            true
        } else {
            false
        }
    }

    /// 短期 -> 长期记忆巩固
    /// 将重要性超过阈值且尚未在长期记忆中的短期记忆转为长期记忆
    /// 返回被巩固的记忆数量
    pub fn consolidate(&mut self, threshold: f32) -> usize {
        let to_consolidate: Vec<u64> = self
            .short_term
            .iter()
            .filter(|&&id| {
                self.entries
                    .get(&id)
                    .map_or(false, |entry| entry.importance >= threshold)
            })
            .copied()
            .collect();

        let count = to_consolidate.len();
        for id in &to_consolidate {
            if let Some(entry) = self.entries.get_mut(id) {
                entry.memory_type = MemoryType::LongTerm;
            }
            self.long_term.push(*id);
        }

        // 从短期记忆列表中移除已巩固的
        self.short_term.retain(|id| !to_consolidate.contains(id));
        count
    }

    /// 清理过期记忆，返回被清理的记忆数量
    pub fn cleanup_expired(&mut self, current_time: u64) -> usize {
        let expired: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry
                    .expires_at
                    .map_or(false, |expires| current_time >= expires)
            })
            .map(|(&id, _)| id)
            .collect();

        let count = expired.len();
        for id in &expired {
            self.delete(*id);
        }
        count
    }

    /// 获取记忆统计信息
    pub fn stats(&self) -> MemoryStats {
        let total_entries = self.entries.len();
        let short_term_count = self.short_term.len();
        let long_term_count = self.long_term.len();
        let total_importance: f32 = self.entries.values().map(|e| e.importance).sum();
        let avg_importance = if total_entries > 0 {
            total_importance / total_entries as f32
        } else {
            0.0
        };

        MemoryStats {
            total_entries,
            short_term_count,
            long_term_count,
            total_importance,
            avg_importance,
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 计算两个向量的余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

// ============================================================================
// 知识图谱
// ============================================================================

/// 知识三元组 (主体 - 关系 - 客体)
#[derive(Debug, Clone)]
pub struct KnowledgeTriple {
    /// 主体
    pub subject: String,
    /// 关系/谓词
    pub predicate: String,
    /// 客体
    pub object: String,
    /// 置信度 (0.0-1.0)
    pub confidence: f32,
    /// 来源
    pub source: String,
    /// 创建时间戳
    pub created_at: u64,
}

/// 知识图谱
pub struct KnowledgeGraph {
    /// 三元组列表
    triples: Vec<KnowledgeTriple>,
    /// 实体索引：实体名 -> 涉及的三元组索引列表
    entity_index: HashMap<String, Vec<usize>>,
}

impl KnowledgeGraph {
    /// 创建新的知识图谱
    pub fn new() -> Self {
        Self {
            triples: Vec::new(),
            entity_index: HashMap::new(),
        }
    }

    /// 添加三元组
    pub fn add_triple(&mut self, triple: KnowledgeTriple) {
        let idx = self.triples.len();

        // 更新主体索引
        self.entity_index
            .entry(triple.subject.clone())
            .or_default()
            .push(idx);

        // 更新客体索引
        self.entity_index
            .entry(triple.object.clone())
            .or_default()
            .push(idx);

        self.triples.push(triple);
    }

    /// 查询主体相关的所有三元组
    pub fn query_subject(&self, subject: &str) -> Vec<&KnowledgeTriple> {
        self.entity_index
            .get(subject)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&idx| self.triples.get(idx))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 查询客体相关的所有三元组
    pub fn query_object(&self, object: &str) -> Vec<&KnowledgeTriple> {
        self.entity_index
            .get(object)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&idx| self.triples.get(idx))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 查询特定关系/谓词的所有三元组
    pub fn query_predicate(&self, predicate: &str) -> Vec<&KnowledgeTriple> {
        self.triples
            .iter()
            .filter(|t| t.predicate == predicate)
            .collect()
    }

    /// 查询两跳路径：从 from 到 to 的所有路径
    /// 返回路径列表，每条路径由三元组组成
    pub fn query_path(&self, from: &str, to: &str) -> Vec<Vec<&KnowledgeTriple>> {
        let mut paths = Vec::new();

        // 第一跳：从 from 出发的所有三元组
        let first_hops = self.query_subject(from);
        for hop1 in &first_hops {
            if hop1.object == to {
                // 直接连接
                paths.push(vec![*hop1]);
            } else {
                // 第二跳：从 hop1.object 出发
                let second_hops = self.query_subject(&hop1.object);
                for hop2 in &second_hops {
                    if hop2.object == to {
                        paths.push(vec![*hop1, *hop2]);
                    }
                }
            }
        }

        paths
    }

    /// 获取与指定实体相关的所有实体集合
    pub fn related_entities(&self, entity: &str) -> HashSet<String> {
        let mut related = HashSet::new();

        if let Some(indices) = self.entity_index.get(entity) {
            for &idx in indices {
                if let Some(triple) = self.triples.get(idx) {
                    if triple.subject != entity {
                        related.insert(triple.subject.clone());
                    }
                    if triple.object != entity {
                        related.insert(triple.object.clone());
                    }
                }
            }
        }

        related
    }

    /// 合并另一个知识图谱
    pub fn merge(&mut self, other: &KnowledgeGraph) {
        for triple in &other.triples {
            // 检查是否已存在相同的三元组
            let exists = self.triples.iter().any(|t| {
                t.subject == triple.subject
                    && t.predicate == triple.predicate
                    && t.object == triple.object
            });
            if !exists {
                self.add_triple(triple.clone());
            }
        }
    }

    /// 获取三元组数量
    pub fn triple_count(&self) -> usize {
        self.triples.len()
    }

    /// 获取实体数量
    pub fn entity_count(&self) -> usize {
        self.entity_index.len()
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 学习策略与配置
// ============================================================================

/// 学习策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LearningStrategy {
    /// 强化学习
    Reinforcement = 0,
    /// 模仿学习
    Imitation = 1,
    /// 自监督学习
    SelfSupervised = 2,
    /// 联邦学习
    Federated = 3,
    /// 进化学习
    Evolutionary = 4,
    /// 记忆回放
    MemoryReplay = 5,
}

/// 学习配置
#[derive(Debug, Clone)]
pub struct LearningConfig {
    /// 学习策略
    pub strategy: LearningStrategy,
    /// 学习率
    pub learning_rate: f32,
    /// 批量大小
    pub batch_size: u32,
    /// 最大训练轮次
    pub max_epochs: u32,
    /// 探索率 (用于强化学习)
    pub exploration_rate: f32,
    /// 折扣因子 (用于强化学习)
    pub discount_factor: f32,
    /// 记忆回放缓冲区大小
    pub memory_replay_size: usize,
    /// 目标准确率
    pub target_accuracy: f32,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            strategy: LearningStrategy::Reinforcement,
            learning_rate: 0.001,
            batch_size: 32,
            max_epochs: 100,
            exploration_rate: 0.1,
            discount_factor: 0.99,
            memory_replay_size: 10000,
            target_accuracy: 0.95,
        }
    }
}

// ============================================================================
// 学习指标与会话
// ============================================================================

/// 学习指标
#[derive(Debug, Clone)]
pub struct LearningMetrics {
    /// 当前轮次
    pub epoch: u32,
    /// 损失值
    pub loss: f32,
    /// 准确率
    pub accuracy: f32,
    /// 奖励值
    pub reward: f32,
    /// 总步数
    pub steps: u64,
    /// 总回合数
    pub episodes: u32,
}

/// 学习会话
#[derive(Debug)]
pub struct LearningSession {
    /// 会话 ID
    pub id: String,
    /// 学习配置
    pub config: LearningConfig,
    /// 指标历史记录
    pub metrics_history: Vec<LearningMetrics>,
    /// 开始时间戳
    pub start_time: u64,
    /// 是否处于活跃状态
    pub is_active: bool,
}

impl LearningSession {
    /// 创建新的学习会话
    pub fn new(id: &str, config: LearningConfig) -> Self {
        Self {
            id: id.to_string(),
            config,
            metrics_history: Vec::new(),
            start_time: 0, // 由调用者设置
            is_active: true,
        }
    }

    /// 记录学习指标
    pub fn record_metrics(&mut self, metrics: LearningMetrics) {
        self.metrics_history.push(metrics);
    }

    /// 获取最新指标
    pub fn latest_metrics(&self) -> Option<&LearningMetrics> {
        self.metrics_history.last()
    }

    /// 获取最佳指标 (按准确率最高)
    pub fn best_metrics(&self) -> Option<&LearningMetrics> {
        self.metrics_history
            .iter()
            .max_by(|a, b| a.accuracy.partial_cmp(&b.accuracy).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// 是否已达到目标准确率
    pub fn is_target_reached(&self) -> bool {
        self.metrics_history
            .iter()
            .any(|m| m.accuracy >= self.config.target_accuracy)
    }

    /// 停止学习会话
    pub fn stop(&mut self) {
        self.is_active = false;
    }
}

// ============================================================================
// 学习引擎
// ============================================================================

/// 学习引擎：管理学习会话、记忆存储和知识图谱
pub struct LearningEngine {
    /// 学习会话映射
    sessions: HashMap<String, LearningSession>,
    /// 记忆存储
    memory_store: MemoryStore,
    /// 知识图谱
    knowledge_graph: KnowledgeGraph,
}

impl LearningEngine {
    /// 创建新的学习引擎
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            memory_store: MemoryStore::new(),
            knowledge_graph: KnowledgeGraph::new(),
        }
    }

    /// 创建学习会话
    pub fn create_session(
        &mut self,
        id: &str,
        config: LearningConfig,
    ) -> Result<(), LearningError> {
        if self.sessions.contains_key(id) {
            return Err(LearningError::SessionAlreadyExists(id.to_string()));
        }

        let session = LearningSession::new(id, config);
        self.sessions.insert(id.to_string(), session);
        Ok(())
    }

    /// 记录学习指标
    pub fn record_metrics(
        &mut self,
        session_id: &str,
        metrics: LearningMetrics,
    ) -> Result<(), LearningError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| LearningError::SessionNotFound(session_id.to_string()))?;

        if !session.is_active {
            return Err(LearningError::SessionNotActive(session_id.to_string()));
        }

        session.record_metrics(metrics);
        Ok(())
    }

    /// 获取会话状态
    pub fn session_status(&self, session_id: &str) -> Result<&LearningSession, LearningError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| LearningError::SessionNotFound(session_id.to_string()))
    }

    /// 停止学习会话
    pub fn stop_session(&mut self, session_id: &str) -> Result<(), LearningError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| LearningError::SessionNotFound(session_id.to_string()))?;

        session.stop();
        Ok(())
    }

    /// 获取记忆存储的不可变引用
    pub fn memory(&self) -> &MemoryStore {
        &self.memory_store
    }

    /// 获取记忆存储的可变引用
    pub fn memory_mut(&mut self) -> &mut MemoryStore {
        &mut self.memory_store
    }

    /// 获取知识图谱的不可变引用
    pub fn knowledge(&self) -> &KnowledgeGraph {
        &self.knowledge_graph
    }

    /// 获取知识图谱的可变引用
    pub fn knowledge_mut(&mut self) -> &mut KnowledgeGraph {
        &mut self.knowledge_graph
    }

    /// 列出所有会话 ID
    pub fn list_sessions(&self) -> Vec<&str> {
        self.sessions.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for LearningEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // MemoryEntry 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_entry_creation() {
        let entry = MemoryEntry {
            id: 1,
            memory_type: MemoryType::ShortTerm,
            content: "测试记忆内容".to_string(),
            embedding: None,
            importance: 0.8,
            access_count: 0,
            created_at: 1000,
            last_accessed: 1000,
            expires_at: None,
            tags: vec!["测试".to_string()],
            metadata: HashMap::new(),
        };

        assert_eq!(entry.id, 1);
        assert_eq!(entry.memory_type, MemoryType::ShortTerm);
        assert_eq!(entry.content, "测试记忆内容");
        assert_eq!(entry.importance, 0.8);
        assert!(entry.embedding.is_none());
        assert!(entry.expires_at.is_none());
    }

    #[test]
    fn test_memory_entry_with_embedding() {
        let entry = MemoryEntry {
            id: 2,
            memory_type: MemoryType::LongTerm,
            content: "带嵌入向量的记忆".to_string(),
            embedding: Some(vec![0.1, 0.2, 0.3]),
            importance: 0.9,
            access_count: 5,
            created_at: 2000,
            last_accessed: 3000,
            expires_at: Some(5000),
            tags: vec!["向量".to_string(), "重要".to_string()],
            metadata: HashMap::new(),
        };

        assert_eq!(entry.memory_type, MemoryType::LongTerm);
        assert_eq!(entry.embedding.as_ref().unwrap().len(), 3);
        assert_eq!(entry.access_count, 5);
        assert_eq!(entry.expires_at, Some(5000));
    }

    // -----------------------------------------------------------------------
    // MemoryStore 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_store_new() {
        let store = MemoryStore::new();
        assert_eq!(store.stats().total_entries, 0);
    }

    #[test]
    fn test_memory_store_store_and_retrieve() {
        let mut store = MemoryStore::new();

        let entry = MemoryEntry {
            id: 0, // 将被自动分配
            memory_type: MemoryType::ShortTerm,
            content: "存储测试".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let id = store.store(entry);
        assert_eq!(id, 1);

        let retrieved = store.retrieve(id).unwrap();
        assert_eq!(retrieved.content, "存储测试");
        assert_eq!(retrieved.memory_type, MemoryType::ShortTerm);
    }

    #[test]
    fn test_memory_store_retrieve_nonexistent() {
        let store = MemoryStore::new();
        assert!(store.retrieve(999).is_none());
    }

    #[test]
    fn test_memory_store_search_by_tags() {
        let mut store = MemoryStore::new();

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "Rust 是一门编程语言".to_string(),
            embedding: None,
            importance: 0.9,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec!["编程".to_string(), "Rust".to_string()],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "Python 也是一种编程语言".to_string(),
            embedding: None,
            importance: 0.8,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec!["编程".to_string(), "Python".to_string()],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "苹果是一种水果".to_string(),
            embedding: None,
            importance: 0.3,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec!["水果".to_string()],
            metadata: HashMap::new(),
        });

        // 搜索同时包含 "编程" 和 "Rust" 标签的记忆
        let results = store.search_by_tags(&["编程", "Rust"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Rust 是一门编程语言");

        // 搜索包含 "编程" 标签的记忆
        let results = store.search_by_tags(&["编程"]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_memory_store_search_by_keyword() {
        let mut store = MemoryStore::new();

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "Rust 是一门系统编程语言".to_string(),
            embedding: None,
            importance: 0.9,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "学习 Rust 编程很有趣".to_string(),
            embedding: None,
            importance: 0.7,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "Python 也很好".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        let results = store.search_by_keyword("rust");
        assert_eq!(results.len(), 2);

        let results = store.search_by_keyword("Python");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_memory_store_search_by_similarity() {
        let mut store = MemoryStore::new();

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "向量 A".to_string(),
            embedding: Some(vec![1.0, 0.0, 0.0]),
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "向量 B".to_string(),
            embedding: Some(vec![0.0, 1.0, 0.0]),
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "向量 C (与 A 相似)".to_string(),
            embedding: Some(vec![0.9, 0.1, 0.0]),
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        // 无嵌入向量的记忆不应出现在结果中
        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::Semantic,
            content: "无嵌入向量".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        let query = vec![1.0, 0.0, 0.0];
        let results = store.search_by_similarity(&query, 2);

        assert_eq!(results.len(), 2);
        // 向量 C (0.9, 0.1, 0.0) 应该比向量 B (0.0, 1.0, 0.0) 更相似
        assert_eq!(results[0].content, "向量 A");
        assert_eq!(results[1].content, "向量 C (与 A 相似)");
    }

    #[test]
    fn test_memory_store_delete() {
        let mut store = MemoryStore::new();

        let id = store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "待删除".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        assert!(store.retrieve(id).is_some());
        assert!(store.delete(id));
        assert!(store.retrieve(id).is_none());
        assert!(!store.delete(id)); // 再次删除应返回 false
    }

    #[test]
    fn test_memory_store_consolidate() {
        let mut store = MemoryStore::new();

        // 添加高重要性的短期记忆
        let id1 = store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "重要记忆".to_string(),
            embedding: None,
            importance: 0.9,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        // 添加低重要性的短期记忆
        let id2 = store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "不重要记忆".to_string(),
            embedding: None,
            importance: 0.2,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        assert_eq!(store.stats().short_term_count, 2);
        assert_eq!(store.stats().long_term_count, 0);

        // 巩固重要性 >= 0.5 的记忆
        let consolidated = store.consolidate(0.5);
        assert_eq!(consolidated, 1);

        assert_eq!(store.stats().short_term_count, 1);
        assert_eq!(store.stats().long_term_count, 1);

        // id1 应该已转为长期记忆
        assert_eq!(store.retrieve(id1).unwrap().memory_type, MemoryType::LongTerm);
        // id2 仍然是短期记忆
        assert_eq!(store.retrieve(id2).unwrap().memory_type, MemoryType::ShortTerm);
    }

    #[test]
    fn test_memory_store_cleanup_expired() {
        let mut store = MemoryStore::new();

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "已过期".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: Some(500),
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "未过期".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: Some(1500),
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "永不过期".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        assert_eq!(store.stats().total_entries, 3);

        let cleaned = store.cleanup_expired(1000);
        assert_eq!(cleaned, 1);
        assert_eq!(store.stats().total_entries, 2);
    }

    #[test]
    fn test_memory_store_stats() {
        let mut store = MemoryStore::new();

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "短期1".to_string(),
            embedding: None,
            importance: 0.3,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "短期2".to_string(),
            embedding: None,
            importance: 0.7,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        store.store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::LongTerm,
            content: "长期1".to_string(),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        let stats = store.stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.short_term_count, 2);
        assert_eq!(stats.long_term_count, 1);
        assert!((stats.total_importance - 1.5).abs() < 0.001);
        assert!((stats.avg_importance - 0.5).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // KnowledgeGraph 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_knowledge_graph_new() {
        let graph = KnowledgeGraph::new();
        assert_eq!(graph.triple_count(), 0);
        assert_eq!(graph.entity_count(), 0);
    }

    #[test]
    fn test_knowledge_graph_add_triple() {
        let mut graph = KnowledgeGraph::new();

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        assert_eq!(graph.triple_count(), 1);
        assert_eq!(graph.entity_count(), 2); // "Rust" 和 "编程语言"
    }

    #[test]
    fn test_knowledge_graph_query_subject() {
        let mut graph = KnowledgeGraph::new();

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "由".to_string(),
            object: "Mozilla".to_string(),
            confidence: 0.9,
            source: "文档".to_string(),
            created_at: 1000,
        });

        let results = graph.query_subject("Rust");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_knowledge_graph_query_object() {
        let mut graph = KnowledgeGraph::new();

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph.add_triple(KnowledgeTriple {
            subject: "Python".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        let results = graph.query_object("编程语言");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_knowledge_graph_query_predicate() {
        let mut graph = KnowledgeGraph::new();

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph.add_triple(KnowledgeTriple {
            subject: "Python".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "由".to_string(),
            object: "Mozilla".to_string(),
            confidence: 0.9,
            source: "文档".to_string(),
            created_at: 1000,
        });

        let results = graph.query_predicate("是一种");
        assert_eq!(results.len(), 2);

        let results = graph.query_predicate("由");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_knowledge_graph_related_entities() {
        let mut graph = KnowledgeGraph::new();

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "由".to_string(),
            object: "Mozilla".to_string(),
            confidence: 0.9,
            source: "文档".to_string(),
            created_at: 1000,
        });

        let related = graph.related_entities("Rust");
        assert!(related.contains("编程语言"));
        assert!(related.contains("Mozilla"));
        assert!(!related.contains("Rust"));
        assert_eq!(related.len(), 2);
    }

    #[test]
    fn test_knowledge_graph_merge() {
        let mut graph1 = KnowledgeGraph::new();
        let mut graph2 = KnowledgeGraph::new();

        graph1.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph2.add_triple(KnowledgeTriple {
            subject: "Python".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        // 重复的三元组
        graph2.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        assert_eq!(graph1.triple_count(), 1);
        graph1.merge(&graph2);
        // 重复的三元组不应被添加
        assert_eq!(graph1.triple_count(), 2);
    }

    #[test]
    fn test_knowledge_graph_query_path() {
        let mut graph = KnowledgeGraph::new();

        // Rust -> 编程语言 (直接路径)
        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "是一种".to_string(),
            object: "编程语言".to_string(),
            confidence: 0.95,
            source: "文档".to_string(),
            created_at: 1000,
        });

        // Rust -> Mozilla -> 组织 (两跳路径)
        graph.add_triple(KnowledgeTriple {
            subject: "Rust".to_string(),
            predicate: "由".to_string(),
            object: "Mozilla".to_string(),
            confidence: 0.9,
            source: "文档".to_string(),
            created_at: 1000,
        });

        graph.add_triple(KnowledgeTriple {
            subject: "Mozilla".to_string(),
            predicate: "是一个".to_string(),
            object: "组织".to_string(),
            confidence: 0.9,
            source: "文档".to_string(),
            created_at: 1000,
        });

        // 直接路径
        let paths = graph.query_path("Rust", "编程语言");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 1);

        // 两跳路径
        let paths = graph.query_path("Rust", "组织");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 2);

        // 不存在的路径
        let paths = graph.query_path("Rust", "不存在的实体");
        assert_eq!(paths.len(), 0);
    }

    // -----------------------------------------------------------------------
    // LearningConfig 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_learning_config_default() {
        let config = LearningConfig::default();

        assert_eq!(config.strategy, LearningStrategy::Reinforcement);
        assert!((config.learning_rate - 0.001).abs() < 1e-6);
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.max_epochs, 100);
        assert!((config.exploration_rate - 0.1).abs() < 1e-6);
        assert!((config.discount_factor - 0.99).abs() < 1e-6);
        assert_eq!(config.memory_replay_size, 10000);
        assert!((config.target_accuracy - 0.95).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // LearningSession 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_learning_session_new() {
        let config = LearningConfig::default();
        let session = LearningSession::new("test-session", config.clone());

        assert_eq!(session.id, "test-session");
        assert!(session.is_active);
        assert!(session.latest_metrics().is_none());
        assert!(session.best_metrics().is_none());
    }

    #[test]
    fn test_learning_session_record_metrics() {
        let config = LearningConfig::default();
        let mut session = LearningSession::new("test-session", config);

        session.record_metrics(LearningMetrics {
            epoch: 1,
            loss: 0.5,
            accuracy: 0.8,
            reward: 1.0,
            steps: 100,
            episodes: 1,
        });

        session.record_metrics(LearningMetrics {
            epoch: 2,
            loss: 0.3,
            accuracy: 0.9,
            reward: 2.0,
            steps: 200,
            episodes: 2,
        });

        assert_eq!(session.metrics_history.len(), 2);
    }

    #[test]
    fn test_learning_session_latest_metrics() {
        let config = LearningConfig::default();
        let mut session = LearningSession::new("test-session", config);

        assert!(session.latest_metrics().is_none());

        session.record_metrics(LearningMetrics {
            epoch: 1,
            loss: 0.5,
            accuracy: 0.8,
            reward: 1.0,
            steps: 100,
            episodes: 1,
        });

        session.record_metrics(LearningMetrics {
            epoch: 2,
            loss: 0.3,
            accuracy: 0.9,
            reward: 2.0,
            steps: 200,
            episodes: 2,
        });

        let latest = session.latest_metrics().unwrap();
        assert_eq!(latest.epoch, 2);
        assert!((latest.accuracy - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_learning_session_best_metrics() {
        let config = LearningConfig::default();
        let mut session = LearningSession::new("test-session", config);

        session.record_metrics(LearningMetrics {
            epoch: 1,
            loss: 0.5,
            accuracy: 0.7,
            reward: 1.0,
            steps: 100,
            episodes: 1,
        });

        session.record_metrics(LearningMetrics {
            epoch: 2,
            loss: 0.3,
            accuracy: 0.95,
            reward: 2.0,
            steps: 200,
            episodes: 2,
        });

        session.record_metrics(LearningMetrics {
            epoch: 3,
            loss: 0.1,
            accuracy: 0.85,
            reward: 3.0,
            steps: 300,
            episodes: 3,
        });

        let best = session.best_metrics().unwrap();
        assert_eq!(best.epoch, 2);
        assert!((best.accuracy - 0.95).abs() < 1e-6);
    }

    #[test]
    fn test_learning_session_target_reached() {
        let config = LearningConfig {
            target_accuracy: 0.9,
            ..LearningConfig::default()
        };
        let mut session = LearningSession::new("test-session", config);

        assert!(!session.is_target_reached());

        session.record_metrics(LearningMetrics {
            epoch: 1,
            loss: 0.5,
            accuracy: 0.8,
            reward: 1.0,
            steps: 100,
            episodes: 1,
        });

        assert!(!session.is_target_reached());

        session.record_metrics(LearningMetrics {
            epoch: 2,
            loss: 0.3,
            accuracy: 0.92,
            reward: 2.0,
            steps: 200,
            episodes: 2,
        });

        assert!(session.is_target_reached());
    }

    #[test]
    fn test_learning_session_stop() {
        let config = LearningConfig::default();
        let mut session = LearningSession::new("test-session", config);

        assert!(session.is_active);
        session.stop();
        assert!(!session.is_active);
    }

    // -----------------------------------------------------------------------
    // LearningEngine 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_learning_engine_new() {
        let engine = LearningEngine::new();
        assert_eq!(engine.list_sessions().len(), 0);
    }

    #[test]
    fn test_learning_engine_create_session() {
        let mut engine = LearningEngine::new();
        let config = LearningConfig::default();

        assert!(engine.create_session("session-1", config.clone()).is_ok());
        assert_eq!(engine.list_sessions().len(), 1);

        // 重复创建应返回错误
        let result = engine.create_session("session-1", config);
        assert!(matches!(result, Err(LearningError::SessionAlreadyExists(_))));
    }

    #[test]
    fn test_learning_engine_record_metrics() {
        let mut engine = LearningEngine::new();
        let config = LearningConfig::default();

        engine.create_session("session-1", config).unwrap();

        let metrics = LearningMetrics {
            epoch: 1,
            loss: 0.5,
            accuracy: 0.8,
            reward: 1.0,
            steps: 100,
            episodes: 1,
        };

        assert!(engine.record_metrics("session-1", metrics.clone()).is_ok());

        // 对不存在的会话记录指标应返回错误
        let result = engine.record_metrics("nonexistent", metrics);
        assert!(matches!(result, Err(LearningError::SessionNotFound(_))));
    }

    #[test]
    fn test_learning_engine_session_status() {
        let mut engine = LearningEngine::new();
        let config = LearningConfig::default();

        engine.create_session("session-1", config).unwrap();

        let status = engine.session_status("session-1").unwrap();
        assert_eq!(status.id, "session-1");
        assert!(status.is_active);

        // 不存在的会话
        let result = engine.session_status("nonexistent");
        assert!(matches!(result, Err(LearningError::SessionNotFound(_))));
    }

    #[test]
    fn test_learning_engine_stop_session() {
        let mut engine = LearningEngine::new();
        let config = LearningConfig::default();

        engine.create_session("session-1", config).unwrap();
        assert!(engine.session_status("session-1").unwrap().is_active);

        engine.stop_session("session-1").unwrap();
        assert!(!engine.session_status("session-1").unwrap().is_active);

        // 停止不存在的会话
        let result = engine.stop_session("nonexistent");
        assert!(matches!(result, Err(LearningError::SessionNotFound(_))));
    }

    #[test]
    fn test_learning_engine_record_metrics_inactive_session() {
        let mut engine = LearningEngine::new();
        let config = LearningConfig::default();

        engine.create_session("session-1", config).unwrap();
        engine.stop_session("session-1").unwrap();

        let metrics = LearningMetrics {
            epoch: 1,
            loss: 0.5,
            accuracy: 0.8,
            reward: 1.0,
            steps: 100,
            episodes: 1,
        };

        let result = engine.record_metrics("session-1", metrics);
        assert!(matches!(result, Err(LearningError::SessionNotActive(_))));
    }

    #[test]
    fn test_learning_engine_memory_access() {
        let mut engine = LearningEngine::new();

        let id = engine.memory_mut().store(MemoryEntry {
            id: 0,
            memory_type: MemoryType::ShortTerm,
            content: "通过引擎访问记忆".to_string(),
            embedding: None,
            importance: 0.7,
            access_count: 0,
            created_at: 100,
            last_accessed: 100,
            expires_at: None,
            tags: vec![],
            metadata: HashMap::new(),
        });

        assert!(engine.memory().retrieve(id).is_some());
    }

    #[test]
    fn test_learning_engine_knowledge_access() {
        let mut engine = LearningEngine::new();

        engine.knowledge_mut().add_triple(KnowledgeTriple {
            subject: "Agent".to_string(),
            predicate: "使用".to_string(),
            object: "学习引擎".to_string(),
            confidence: 0.9,
            source: "测试".to_string(),
            created_at: 1000,
        });

        assert_eq!(engine.knowledge().triple_count(), 1);
        assert_eq!(engine.knowledge().entity_count(), 2);
    }

    #[test]
    fn test_learning_engine_list_sessions() {
        let mut engine = LearningEngine::new();
        let config = LearningConfig::default();

        engine.create_session("s1", config.clone()).unwrap();
        engine.create_session("s2", config.clone()).unwrap();
        engine.create_session("s3", config).unwrap();

        let sessions = engine.list_sessions();
        assert_eq!(sessions.len(), 3);
    }

    // -----------------------------------------------------------------------
    // 辅助函数测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    // -----------------------------------------------------------------------
    // MemoryType repr 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_type_repr() {
        assert_eq!(MemoryType::ShortTerm as u8, 0);
        assert_eq!(MemoryType::LongTerm as u8, 1);
        assert_eq!(MemoryType::Working as u8, 2);
        assert_eq!(MemoryType::Episodic as u8, 3);
        assert_eq!(MemoryType::Semantic as u8, 4);
    }

    // -----------------------------------------------------------------------
    // LearningStrategy repr 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_learning_strategy_repr() {
        assert_eq!(LearningStrategy::Reinforcement as u8, 0);
        assert_eq!(LearningStrategy::Imitation as u8, 1);
        assert_eq!(LearningStrategy::SelfSupervised as u8, 2);
        assert_eq!(LearningStrategy::Federated as u8, 3);
        assert_eq!(LearningStrategy::Evolutionary as u8, 4);
        assert_eq!(LearningStrategy::MemoryReplay as u8, 5);
    }
}
