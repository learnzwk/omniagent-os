//! CRDT（无冲突复制数据类型）状态同步实现
//!
//! 包含：
//! - VectorClock：向量时钟，用于追踪因果关系
//! - CrdtCounter：G-Counter 计数器
//! - CrdtSet：OR-Set 集合
//! - CrdtRegister：LWW-Register 寄存器

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::node::NodeId;

/// 因果关系比较结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalOrder {
    /// self 先于 other（self < other）
    Before,
    /// self 后于 other（self > other）
    After,
    /// 并发（无因果关系）
    Concurrent,
    /// 相等
    Equal,
}

/// 向量时钟
///
/// 用于追踪分布式系统中事件的因果关系。每个节点维护自己的逻辑时钟，
/// 通过比较向量时钟可以判断事件的先后顺序。
#[derive(Debug, Clone)]
pub struct VectorClock {
    /// 各节点的时钟值
    entries: HashMap<NodeId, u64>,
}

impl VectorClock {
    /// 创建空的向量时钟
    pub fn new() -> Self {
        VectorClock {
            entries: HashMap::new(),
        }
    }

    /// 递增指定节点的时钟
    pub fn increment(&mut self, node: &NodeId) {
        let current = self.entries.get(node).copied().unwrap_or(0);
        self.entries.insert(node.clone(), current + 1);
    }

    /// 合并另一个向量时钟（取各分量的最大值）
    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &value) in &other.entries {
            let current = self.entries.get(node).copied().unwrap_or(0);
            self.entries.insert(node.clone(), current.max(value));
        }
    }

    /// 比较因果关系
    ///
    /// 返回两个向量时钟之间的因果关系：
    /// - Equal：所有分量相等
    /// - Before：self 的所有分量 <= other 的对应分量，且至少有一个严格小于
    /// - After：self 的所有分量 >= other 的对应分量，且至少有一个严格大于
    /// - Concurrent：既不是 Before 也不是 After
    pub fn compare(&self, other: &VectorClock) -> CausalOrder {
        // 收集所有涉及的节点
        let all_nodes: HashSet<&NodeId> = self.entries.keys()
            .chain(other.entries.keys())
            .collect();

        let mut has_less = false;
        let mut has_greater = false;

        for node in all_nodes {
            let self_val = self.entries.get(node).copied().unwrap_or(0);
            let other_val = other.entries.get(node).copied().unwrap_or(0);

            if self_val < other_val {
                has_less = true;
            } else if self_val > other_val {
                has_greater = true;
            }
        }

        if !has_less && !has_greater {
            CausalOrder::Equal
        } else if has_less && !has_greater {
            CausalOrder::Before
        } else if has_greater && !has_less {
            CausalOrder::After
        } else {
            CausalOrder::Concurrent
        }
    }

    /// 获取指定节点的时钟值
    pub fn get(&self, node: &NodeId) -> u64 {
        self.entries.get(node).copied().unwrap_or(0)
    }

    /// 获取时钟中的节点数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 判断时钟是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

/// CRDT 计数器（G-Counter 实现）
///
/// G-Counter（Grow-only Counter）是一种只能递增的分布式计数器。
/// 每个节点只允许递增自己的计数，合并时取各节点计数的最大值。
/// 总值 = 所有节点计数的总和。
#[derive(Debug, Clone)]
pub struct CrdtCounter {
    /// 各节点的计数值
    counts: HashMap<NodeId, u64>,
}

impl CrdtCounter {
    /// 创建空的 CRDT 计数器
    pub fn new() -> Self {
        CrdtCounter {
            counts: HashMap::new(),
        }
    }

    /// 本地递增指定节点的计数
    pub fn increment(&mut self, node: &NodeId) {
        let current = self.counts.get(node).copied().unwrap_or(0);
        self.counts.insert(node.clone(), current + 1);
    }

    /// 合并远程计数器（取各节点计数的最大值）
    pub fn merge(&mut self, other: &CrdtCounter) {
        for (node, &value) in &other.counts {
            let current = self.counts.get(node).copied().unwrap_or(0);
            self.counts.insert(node.clone(), current.max(value));
        }
    }

    /// 获取计数的总值（所有节点计数的总和）
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// 获取指定节点的计数值
    pub fn get(&self, node: &NodeId) -> u64 {
        self.counts.get(node).copied().unwrap_or(0)
    }

    /// 获取参与计数的节点数量
    pub fn node_count(&self) -> usize {
        self.counts.len()
    }
}

impl Default for CrdtCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// CRDT 集合（OR-Set 实现）
///
/// OR-Set（Observed-Remove Set）支持添加和删除操作。
/// 使用墓碑机制确保删除操作在合并后仍然有效。
#[derive(Debug, Clone)]
pub struct CrdtSet<T: Clone + Eq + Hash> {
    /// 当前存活的元素
    items: HashSet<T>,
    /// 已删除的元素（墓碑）
    tombstones: HashSet<T>,
}

impl<T: Clone + Eq + Hash + std::fmt::Debug> CrdtSet<T> {
    /// 创建空的 CRDT 集合
    pub fn new() -> Self {
        CrdtSet {
            items: HashSet::new(),
            tombstones: HashSet::new(),
        }
    }

    /// 添加元素
    ///
    /// 如果元素在墓碑中，则不添加（尊重已删除状态）
    pub fn add(&mut self, item: T) {
        if !self.tombstones.contains(&item) {
            self.items.insert(item);
        }
    }

    /// 移除元素
    ///
    /// 将元素移入墓碑集合
    pub fn remove(&mut self, item: &T) {
        self.items.remove(item);
        self.tombstones.insert(item.clone());
    }

    /// 合并另一个 CRDT 集合
    ///
    /// 合并规则：
    /// - 存活元素取并集，但排除在任一墓碑集合中的元素
    /// - 墓碑取并集
    pub fn merge(&mut self, other: &CrdtSet<T>) {
        // 合并墓碑
        for tombstone in &other.tombstones {
            self.tombstones.insert(tombstone.clone());
        }

        // 合并存活元素（排除墓碑中的元素）
        for item in &other.items {
            if !self.tombstones.contains(item) {
                self.items.insert(item.clone());
            }
        }

        // 清理本地存活元素中在墓碑中的项
        self.items.retain(|item| !self.tombstones.contains(item));
    }

    /// 检查是否包含指定元素
    pub fn contains(&self, item: &T) -> bool {
        self.items.contains(item)
    }

    /// 获取所有存活元素的引用
    pub fn items(&self) -> &HashSet<T> {
        &self.items
    }

    /// 获取存活元素数量
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 判断集合是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 获取墓碑数量
    pub fn tombstone_count(&self) -> usize {
        self.tombstones.len()
    }
}

impl<T: Clone + Eq + Hash + std::fmt::Debug> Default for CrdtSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// CRDT 寄存器（LWW-Register 实现）
///
/// LWW-Register（Last-Writer-Wins Register）使用时间戳和节点 ID 来解决冲突。
/// 当时间戳相同时，使用节点 ID 作为决胜条件（字典序比较）。
#[derive(Debug, Clone)]
pub struct CrdtRegister<T: Clone> {
    /// 当前值
    value: Option<T>,
    /// 最后写入时间戳
    timestamp: u64,
    /// 最后写入的节点
    node: NodeId,
}

impl<T: Clone> CrdtRegister<T> {
    /// 创建空的 CRDT 寄存器
    pub fn new(node: NodeId) -> Self {
        CrdtRegister {
            value: None,
            timestamp: 0,
            node,
        }
    }

    /// 创建带初始值的 CRDT 寄存器
    pub fn with_value(node: NodeId, value: T, timestamp: u64) -> Self {
        CrdtRegister {
            value: Some(value),
            timestamp,
            node,
        }
    }

    /// 设置值
    ///
    /// 仅当新时间戳大于当前时间戳，或时间戳相等但节点 ID 更大时才更新
    pub fn set(&mut self, value: T, timestamp: u64, node: &NodeId) {
        if timestamp > self.timestamp
            || (timestamp == self.timestamp && node.as_bytes() > self.node.as_bytes())
        {
            self.value = Some(value);
            self.timestamp = timestamp;
            self.node = node.clone();
        }
    }

    /// 合并另一个 CRDT 寄存器
    ///
    /// 使用 LWW 策略：时间戳大的胜出，时间戳相同则节点 ID 大的胜出
    pub fn merge(&mut self, other: &CrdtRegister<T>) {
        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.node.as_bytes() > self.node.as_bytes())
        {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.node = other.node.clone();
        }
    }

    /// 获取当前值的引用
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// 获取最后写入时间戳
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// 获取最后写入的节点
    pub fn node(&self) -> &NodeId {
        &self.node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== VectorClock 测试 ==========

    #[test]
    fn test_vector_clock_increment() {
        let node = NodeId::new();
        let mut clock = VectorClock::new();

        clock.increment(&node);
        assert_eq!(clock.get(&node), 1);

        clock.increment(&node);
        assert_eq!(clock.get(&node), 2);
    }

    #[test]
    fn test_vector_clock_merge() {
        let node_a = NodeId::new();
        let node_b = NodeId::new();

        let mut clock_a = VectorClock::new();
        clock_a.increment(&node_a);
        clock_a.increment(&node_a);

        let mut clock_b = VectorClock::new();
        clock_b.increment(&node_b);
        clock_b.increment(&node_b);
        clock_b.increment(&node_b);

        clock_a.merge(&clock_b);
        assert_eq!(clock_a.get(&node_a), 2);
        assert_eq!(clock_a.get(&node_b), 3);
    }

    #[test]
    fn test_vector_clock_compare_equal() {
        let node = NodeId::new();
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment(&node);
        clock2.increment(&node);

        assert_eq!(clock1.compare(&clock2), CausalOrder::Equal);
    }

    #[test]
    fn test_vector_clock_compare_before() {
        let node = NodeId::new();
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment(&node);
        clock2.increment(&node);
        clock2.increment(&node);

        assert_eq!(clock1.compare(&clock2), CausalOrder::Before);
        assert_eq!(clock2.compare(&clock1), CausalOrder::After);
    }

    #[test]
    fn test_vector_clock_compare_concurrent() {
        let node_a = NodeId::new();
        let node_b = NodeId::new();

        let mut clock_a = VectorClock::new();
        clock_a.increment(&node_a);

        let mut clock_b = VectorClock::new();
        clock_b.increment(&node_b);

        assert_eq!(clock_a.compare(&clock_b), CausalOrder::Concurrent);
    }

    // ========== CrdtCounter 测试 ==========

    #[test]
    fn test_crdt_counter_increment() {
        let node = NodeId::new();
        let mut counter = CrdtCounter::new();

        counter.increment(&node);
        counter.increment(&node);
        counter.increment(&node);

        assert_eq!(counter.value(), 3);
    }

    #[test]
    fn test_crdt_counter_merge() {
        let node_a = NodeId::new();
        let node_b = NodeId::new();

        let mut counter_a = CrdtCounter::new();
        counter_a.increment(&node_a);
        counter_a.increment(&node_a);

        let mut counter_b = CrdtCounter::new();
        counter_b.increment(&node_b);
        counter_b.increment(&node_b);
        counter_b.increment(&node_b);

        counter_a.merge(&counter_b);
        assert_eq!(counter_a.value(), 5);
    }

    #[test]
    fn test_crdt_counter_merge_max() {
        // 合并时取最大值，不会重复计数
        let node = NodeId::new();

        let mut counter_a = CrdtCounter::new();
        counter_a.increment(&node);
        counter_a.increment(&node);
        counter_a.increment(&node);

        let mut counter_b = CrdtCounter::new();
        counter_b.increment(&node);
        counter_b.increment(&node);

        counter_a.merge(&counter_b);
        // 应取最大值 3，而非 3+2=5
        assert_eq!(counter_a.value(), 3);
    }

    // ========== CrdtSet 测试 ==========

    #[test]
    fn test_crdt_set_add() {
        let mut set: CrdtSet<String> = CrdtSet::new();
        set.add("item1".to_string());
        set.add("item2".to_string());

        assert!(set.contains(&"item1".to_string()));
        assert!(set.contains(&"item2".to_string()));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_crdt_set_remove() {
        let mut set: CrdtSet<String> = CrdtSet::new();
        set.add("item1".to_string());
        set.add("item2".to_string());

        set.remove(&"item1".to_string());

        assert!(!set.contains(&"item1".to_string()));
        assert!(set.contains(&"item2".to_string()));
        assert_eq!(set.len(), 1);
        assert_eq!(set.tombstone_count(), 1);
    }

    #[test]
    fn test_crdt_set_merge() {
        let mut set_a: CrdtSet<String> = CrdtSet::new();
        set_a.add("common".to_string());
        set_a.add("only_a".to_string());

        let mut set_b: CrdtSet<String> = CrdtSet::new();
        set_b.add("common".to_string());
        set_b.add("only_b".to_string());

        set_a.merge(&set_b);

        assert!(set_a.contains(&"common".to_string()));
        assert!(set_a.contains(&"only_a".to_string()));
        assert!(set_a.contains(&"only_b".to_string()));
        assert_eq!(set_a.len(), 3);
    }

    #[test]
    fn test_crdt_set_merge_with_tombstone() {
        let mut set_a: CrdtSet<String> = CrdtSet::new();
        set_a.add("item".to_string());
        set_a.remove(&"item".to_string());

        let mut set_b: CrdtSet<String> = CrdtSet::new();
        set_b.add("item".to_string());

        // 合并后，item 应该仍然被删除（墓碑优先）
        set_b.merge(&set_a);
        assert!(!set_b.contains(&"item".to_string()));
    }

    #[test]
    fn test_crdt_set_no_add_after_remove() {
        let mut set: CrdtSet<String> = CrdtSet::new();
        set.add("item".to_string());
        set.remove(&"item".to_string());

        // 删除后不应能重新添加
        set.add("item".to_string());
        assert!(!set.contains(&"item".to_string()));
    }

    // ========== CrdtRegister 测试 ==========

    #[test]
    fn test_crdt_register_set() {
        let node = NodeId::new();
        let mut reg: CrdtRegister<String> = CrdtRegister::new(node.clone());

        assert!(reg.get().is_none());

        reg.set("hello".to_string(), 1, &node);
        assert_eq!(reg.get(), Some(&"hello".to_string()));
    }

    #[test]
    fn test_crdt_register_lww() {
        let node = NodeId::new();
        let mut reg: CrdtRegister<String> = CrdtRegister::new(node.clone());

        reg.set("first".to_string(), 1, &node);
        reg.set("second".to_string(), 5, &node);
        // 更晚的时间戳应该覆盖
        assert_eq!(reg.get(), Some(&"second".to_string()));

        // 更早的时间戳不应覆盖
        reg.set("third".to_string(), 3, &node);
        assert_eq!(reg.get(), Some(&"second".to_string()));
    }

    #[test]
    fn test_crdt_register_merge() {
        let node_a = NodeId::new();
        let node_b = NodeId::new();

        let mut reg_a: CrdtRegister<String> = CrdtRegister::new(node_a.clone());
        reg_a.set("from_a".to_string(), 10, &node_a);

        let mut reg_b: CrdtRegister<String> = CrdtRegister::new(node_b.clone());
        reg_b.set("from_b".to_string(), 20, &node_b);

        // 时间戳更大的应该胜出
        reg_a.merge(&reg_b);
        assert_eq!(reg_a.get(), Some(&"from_b".to_string()));
    }

    #[test]
    fn test_crdt_register_same_timestamp_node_id_decides() {
        let node_a = NodeId::from_bytes([0u8; 16]);
        let node_b = NodeId::from_bytes([1u8; 16]);

        let mut reg: CrdtRegister<String> = CrdtRegister::new(node_a.clone());
        reg.set("from_a".to_string(), 5, &node_a);

        // 相同时间戳，节点 ID 更大的胜出
        reg.set("from_b".to_string(), 5, &node_b);
        assert_eq!(reg.get(), Some(&"from_b".to_string()));
    }
}
