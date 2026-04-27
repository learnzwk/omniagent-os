//! 审计链模块
//!
//! 实现鸿蒙风格的哈希链防篡改审计日志。
//! 每条审计记录包含前一条记录的哈希，形成链式结构，
//! 确保审计日志的完整性和不可篡改性。

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

// ============================================================================
// 审计事件类型
// ============================================================================

/// 审计事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventType {
    /// 认证事件
    Auth = 0,
    /// 文件访问
    FileAccess = 1,
    /// 网络访问
    NetworkAccess = 2,
    /// IPC 调用
    IpcCall = 3,
    /// 服务启动
    ServiceStart = 4,
    /// 服务停止
    ServiceStop = 5,
    /// 能力授予
    CapabilityGrant = 6,
    /// 能力撤销
    CapabilityRevoke = 7,
    /// 配置变更
    ConfigChange = 8,
}

// ============================================================================
// 审计结果
// ============================================================================

/// 审计结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    /// 成功
    Success = 0,
    /// 拒绝
    Denied = 1,
    /// 错误
    Error = 2,
}

// ============================================================================
// 审计记录
// ============================================================================

/// 审计记录
#[derive(Debug, Clone)]
pub struct AuditRecord {
    /// 记录 ID
    pub record_id: u64,
    /// 时间戳
    pub timestamp: u64,
    /// 事件类型
    pub event_type: AuditEventType,
    /// 操作者 ID
    pub actor_id: u64,
    /// 目标 ID
    pub target_id: u64,
    /// 操作描述
    pub action: String,
    /// 审计结果
    pub result: AuditResult,
    /// 前一条记录的哈希
    pub prev_hash: u64,
}

// ============================================================================
// 审计链
// ============================================================================

/// 审计链
///
/// 使用哈希链结构存储审计记录，每条记录包含前一条记录的哈希，
/// 确保日志的完整性和不可篡改性。
pub struct AuditChain {
    /// 审计记录列表
    records: Mutex<Vec<AuditRecord>>,
    /// 当前哈希值
    current_hash: AtomicU64,
    /// 下一个可用记录 ID
    next_id: AtomicU64,
    /// 最大记录数
    max_records: usize,
}

impl AuditChain {
    /// 创建新的审计链
    ///
    /// # 参数
    /// - `max_records`: 最大记录数量
    pub fn new(max_records: usize) -> Self {
        AuditChain {
            records: Mutex::new(Vec::new()),
            current_hash: AtomicU64::new(0), // 初始哈希为 0
            next_id: AtomicU64::new(1),
            max_records,
        }
    }

    /// 追加审计记录
    ///
    /// 创建新的审计记录并添加到链中，自动计算哈希。
    pub fn append(
        &self,
        event_type: AuditEventType,
        actor: u64,
        target: u64,
        action: &str,
        result: AuditResult,
    ) {
        let record_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let prev_hash = self.current_hash.load(Ordering::SeqCst);

        let record = AuditRecord {
            record_id,
            timestamp: 0, // 在实际内核中会使用系统时钟
            event_type,
            actor_id: actor,
            target_id: target,
            action: String::from(action),
            result,
            prev_hash,
        };

        // 计算当前记录的哈希
        let hash = self.compute_hash(&record, prev_hash);

        let mut records = self.records.lock();

        // 如果超过最大记录数，移除最旧的记录
        if records.len() >= self.max_records {
            records.remove(0);
        }

        records.push(record);
        self.current_hash.store(hash, Ordering::SeqCst);
    }

    /// 验证链完整性
    ///
    /// 检查每条记录的 prev_hash 是否与前一条记录的哈希一致。
    pub fn verify_chain(&self) -> bool {
        let records = self.records.lock();
        if records.is_empty() {
            return true;
        }

        // 第一条记录的 prev_hash 应为 0
        if records[0].prev_hash != 0 {
            return false;
        }

        for i in 1..records.len() {
            let expected_hash = self.compute_hash(&records[i - 1], records[i - 1].prev_hash);
            if records[i].prev_hash != expected_hash {
                return false;
            }
        }

        true
    }

    /// 获取最近的 N 条审计记录
    pub fn get_records(&self, count: usize) -> Vec<AuditRecord> {
        let records = self.records.lock();
        let start = if records.len() > count {
            records.len() - count
        } else {
            0
        };
        records[start..].to_vec()
    }

    /// 获取指定操作者的最近 N 条审计记录
    pub fn get_records_for_actor(&self, actor_id: u64, count: usize) -> Vec<AuditRecord> {
        let records = self.records.lock();
        let filtered: Vec<AuditRecord> = records
            .iter()
            .filter(|r| r.actor_id == actor_id)
            .cloned()
            .collect();
        let start = if filtered.len() > count {
            filtered.len() - count
        } else {
            0
        };
        filtered[start..].to_vec()
    }

    /// 获取记录总数
    pub fn record_count(&self) -> usize {
        let records = self.records.lock();
        records.len()
    }

    /// 计算记录的哈希值
    ///
    /// 使用简单的哈希算法组合记录字段和前一条记录的哈希。
    pub fn compute_hash(&self, record: &AuditRecord, prev_hash: u64) -> u64 {
        // 使用 FNV-1a 风格的简单哈希算法
        let mut hash = prev_hash.wrapping_add(0xcbf29ce484222325);

        hash ^= record.record_id;
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= record.actor_id;
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= record.target_id;
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= (record.event_type as u64);
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= (record.result as u64);
        hash = hash.wrapping_mul(0x100000001b3);

        // 对 action 字符串逐字节哈希
        for byte in record.action.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }

        hash
    }
}

/// 全局审计链实例
pub static AUDIT_CHAIN: spin::Lazy<Mutex<AuditChain>> = spin::Lazy::new(|| {
    Mutex::new(AuditChain::new(4096))
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === 测试: 创建审计链 ===
    #[test]
    fn test_new() {
        let chain = AuditChain::new(100);
        assert_eq!(chain.record_count(), 0);
        assert!(chain.verify_chain());
    }

    // === 测试: 追加记录 ===
    #[test]
    fn test_append() {
        let chain = AuditChain::new(100);
        chain.append(AuditEventType::Auth, 1, 0, "登录", AuditResult::Success);
        chain.append(AuditEventType::FileAccess, 1, 100, "读取文件", AuditResult::Success);

        assert_eq!(chain.record_count(), 2);

        let records = chain.get_records(10);
        assert_eq!(records[0].event_type, AuditEventType::Auth);
        assert_eq!(records[0].action, "登录");
        assert_eq!(records[1].event_type, AuditEventType::FileAccess);
    }

    // === 测试: 验证链完整性 ===
    #[test]
    fn test_verify_chain() {
        let chain = AuditChain::new(100);

        // 空链应验证通过
        assert!(chain.verify_chain());

        // 添加记录后应验证通过
        chain.append(AuditEventType::Auth, 1, 0, "登录", AuditResult::Success);
        chain.append(AuditEventType::FileAccess, 1, 100, "读取文件", AuditResult::Success);
        chain.append(AuditEventType::NetworkAccess, 2, 200, "网络请求", AuditResult::Denied);

        assert!(chain.verify_chain());
    }

    // === 测试: 获取记录 ===
    #[test]
    fn test_get_records() {
        let chain = AuditChain::new(100);
        for i in 0..10u64 {
            chain.append(AuditEventType::Auth, i, 0, "操作", AuditResult::Success);
        }

        // 获取最近 3 条
        let recent = chain.get_records(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].actor_id, 7);
        assert_eq!(recent[1].actor_id, 8);
        assert_eq!(recent[2].actor_id, 9);

        // 获取超过总数的记录
        let all = chain.get_records(100);
        assert_eq!(all.len(), 10);
    }

    // === 测试: 获取指定操作者的记录 ===
    #[test]
    fn test_get_records_for_actor() {
        let chain = AuditChain::new(100);
        chain.append(AuditEventType::Auth, 1, 0, "登录", AuditResult::Success);
        chain.append(AuditEventType::FileAccess, 2, 100, "读取文件", AuditResult::Success);
        chain.append(AuditEventType::Auth, 1, 0, "再次登录", AuditResult::Denied);
        chain.append(AuditEventType::NetworkAccess, 3, 200, "网络请求", AuditResult::Success);

        let actor1_records = chain.get_records_for_actor(1, 10);
        assert_eq!(actor1_records.len(), 2);
        assert_eq!(actor1_records[0].action, "登录");
        assert_eq!(actor1_records[1].action, "再次登录");

        let actor2_records = chain.get_records_for_actor(2, 10);
        assert_eq!(actor2_records.len(), 1);

        let actor99_records = chain.get_records_for_actor(99, 10);
        assert!(actor99_records.is_empty());
    }

    // === 测试: 最大记录数限制 ===
    #[test]
    fn test_max_records() {
        let chain = AuditChain::new(5);
        for i in 0..10u64 {
            chain.append(AuditEventType::Auth, i, 0, "操作", AuditResult::Success);
        }

        // 应只保留最近 5 条
        assert_eq!(chain.record_count(), 5);

        let records = chain.get_records(10);
        assert_eq!(records[0].actor_id, 5);
        assert_eq!(records[4].actor_id, 9);
    }

    // === 测试: 记录计数 ===
    #[test]
    fn test_record_count() {
        let chain = AuditChain::new(100);
        assert_eq!(chain.record_count(), 0);

        chain.append(AuditEventType::Auth, 1, 0, "操作", AuditResult::Success);
        assert_eq!(chain.record_count(), 1);

        chain.append(AuditEventType::FileAccess, 2, 100, "读取", AuditResult::Success);
        assert_eq!(chain.record_count(), 2);
    }

    // === 测试: 哈希计算 ===
    #[test]
    fn test_compute_hash() {
        let chain = AuditChain::new(100);

        let record1 = AuditRecord {
            record_id: 1,
            timestamp: 0,
            event_type: AuditEventType::Auth,
            actor_id: 1,
            target_id: 0,
            action: String::from("登录"),
            result: AuditResult::Success,
            prev_hash: 0,
        };

        let hash1 = chain.compute_hash(&record1, 0);
        assert_ne!(hash1, 0);

        // 相同记录和 prev_hash 应产生相同哈希
        let hash2 = chain.compute_hash(&record1, 0);
        assert_eq!(hash1, hash2);

        // 不同 prev_hash 应产生不同哈希
        let hash3 = chain.compute_hash(&record1, 999);
        assert_ne!(hash1, hash3);
    }

    // === 测试: 不同事件类型 ===
    #[test]
    fn test_event_types() {
        let chain = AuditChain::new(100);

        chain.append(AuditEventType::ServiceStart, 1, 10, "启动服务", AuditResult::Success);
        chain.append(AuditEventType::ServiceStop, 1, 10, "停止服务", AuditResult::Success);
        chain.append(AuditEventType::CapabilityGrant, 0, 1, "授予权限", AuditResult::Success);
        chain.append(AuditEventType::CapabilityRevoke, 0, 1, "撤销权限", AuditResult::Denied);
        chain.append(AuditEventType::ConfigChange, 0, 0, "修改配置", AuditResult::Success);

        assert_eq!(chain.record_count(), 5);
        assert!(chain.verify_chain());

        let records = chain.get_records(10);
        assert_eq!(records[0].event_type, AuditEventType::ServiceStart);
        assert_eq!(records[1].event_type, AuditEventType::ServiceStop);
        assert_eq!(records[2].event_type, AuditEventType::CapabilityGrant);
        assert_eq!(records[3].event_type, AuditEventType::CapabilityRevoke);
        assert_eq!(records[4].event_type, AuditEventType::ConfigChange);
    }
}
