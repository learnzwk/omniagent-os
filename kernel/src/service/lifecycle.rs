//! 服务生命周期管理
//!
//! 实现鸿蒙风格的服务生命周期事件记录和管理，
//! 包括事件日志、重启策略判断等功能。

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

// ============================================================================
// 生命周期事件
// ============================================================================

/// 服务生命周期事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// 已注册
    Registered,
    /// 初始化中
    Initializing,
    /// 已启动
    Started,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
    /// 已失败
    Failed,
    /// 已重启
    Restarted,
}

// ============================================================================
// 生命周期事件记录
// ============================================================================

/// 生命周期事件记录
#[derive(Debug, Clone)]
pub struct LifecycleEventRecord {
    /// 服务 ID
    pub service_id: u64,
    /// 事件类型
    pub event: LifecycleEvent,
    /// 时间戳
    pub timestamp: u64,
    /// 事件原因
    pub reason: String,
}

// ============================================================================
// 生命周期管理器
// ============================================================================

/// 服务生命周期管理器
///
/// 记录服务生命周期事件，管理事件日志，
/// 提供重启策略判断。
pub struct LifecycleManager {
    /// 事件日志
    event_log: Mutex<Vec<LifecycleEventRecord>>,
    /// 最大日志条数
    max_log: usize,
}

impl LifecycleManager {
    /// 创建新的生命周期管理器
    ///
    /// # 参数
    /// - `max_log`: 最大事件日志条数
    pub fn new(max_log: usize) -> Self {
        LifecycleManager {
            event_log: Mutex::new(Vec::new()),
            max_log,
        }
    }

    /// 记录生命周期事件
    ///
    /// # 参数
    /// - `service_id`: 服务 ID
    /// - `event`: 事件类型
    /// - `reason`: 事件原因描述
    pub fn record_event(&self, service_id: u64, event: LifecycleEvent, reason: &str) {
        let mut log = self.event_log.lock();
        let record = LifecycleEventRecord {
            service_id,
            event,
            timestamp: 0, // 在实际内核中会使用系统时钟
            reason: String::from(reason),
        };

        // 如果超过最大日志条数，移除最旧的记录
        if log.len() >= self.max_log {
            log.remove(0);
        }

        log.push(record);
    }

    /// 获取指定服务的所有事件记录
    ///
    /// # 参数
    /// - `service_id`: 服务 ID
    ///
    /// # 返回
    /// 该服务的所有事件记录列表
    pub fn get_events(&self, service_id: u64) -> Vec<LifecycleEventRecord> {
        let log = self.event_log.lock();
        log.iter()
            .filter(|r| r.service_id == service_id)
            .cloned()
            .collect()
    }

    /// 获取最近的 N 条事件记录
    ///
    /// # 参数
    /// - `count`: 要获取的记录数量
    ///
    /// # 返回
    /// 最近的事件记录列表（从旧到新排序）
    pub fn get_recent_events(&self, count: usize) -> Vec<LifecycleEventRecord> {
        let log = self.event_log.lock();
        let start = if log.len() > count {
            log.len() - count
        } else {
            0
        };
        log[start..].to_vec()
    }

    /// 判断服务是否应该重启
    ///
    /// 根据重启次数和最大重启次数判断是否允许重启。
    ///
    /// # 参数
    /// - `service_id`: 服务 ID（保留参数，用于未来扩展）
    /// - `restart_count`: 当前已重启次数
    /// - `max_restart`: 最大允许重启次数
    ///
    /// # 返回
    /// 如果允许重启返回 true
    pub fn should_restart(&self, _service_id: u64, restart_count: u32, max_restart: u32) -> bool {
        restart_count < max_restart
    }

    /// 获取事件日志总数
    pub fn event_count(&self) -> usize {
        let log = self.event_log.lock();
        log.len()
    }
}

/// 全局生命周期管理器
pub static LIFECYCLE_MANAGER: spin::Lazy<Mutex<LifecycleManager>> = spin::Lazy::new(|| {
    Mutex::new(LifecycleManager::new(1024))
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === 测试: 记录事件 ===
    #[test]
    fn test_record_event() {
        let manager = LifecycleManager::new(100);
        manager.record_event(1, LifecycleEvent::Registered, "初始注册");
        manager.record_event(1, LifecycleEvent::Started, "启动成功");

        let events = manager.get_events(1);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, LifecycleEvent::Registered);
        assert_eq!(events[0].reason, "初始注册");
        assert_eq!(events[1].event, LifecycleEvent::Started);
        assert_eq!(events[1].reason, "启动成功");
    }

    // === 测试: 获取指定服务的事件 ===
    #[test]
    fn test_get_events() {
        let manager = LifecycleManager::new(100);
        manager.record_event(1, LifecycleEvent::Registered, "注册");
        manager.record_event(2, LifecycleEvent::Registered, "注册");
        manager.record_event(1, LifecycleEvent::Started, "启动");
        manager.record_event(3, LifecycleEvent::Failed, "崩溃");

        let events_1 = manager.get_events(1);
        assert_eq!(events_1.len(), 2);

        let events_2 = manager.get_events(2);
        assert_eq!(events_2.len(), 1);

        let events_99 = manager.get_events(99);
        assert_eq!(events_99.len(), 0);
    }

    // === 测试: 获取最近事件 ===
    #[test]
    fn test_get_recent() {
        let manager = LifecycleManager::new(100);
        for i in 0..10 {
            manager.record_event(i, LifecycleEvent::Registered, "注册");
        }

        // 获取最近 3 条
        let recent = manager.get_recent_events(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].service_id, 7);
        assert_eq!(recent[1].service_id, 8);
        assert_eq!(recent[2].service_id, 9);

        // 请求超过总数的记录
        let all_recent = manager.get_recent_events(100);
        assert_eq!(all_recent.len(), 10);
    }

    // === 测试: 应该重启 ===
    #[test]
    fn test_should_restart() {
        let manager = LifecycleManager::new(100);

        // 重启次数 < 最大次数，应该重启
        assert!(manager.should_restart(1, 0, 3));
        assert!(manager.should_restart(1, 1, 3));
        assert!(manager.should_restart(1, 2, 3));

        // 重启次数 = 最大次数，不应该重启
        assert!(!manager.should_restart(1, 3, 3));
        assert!(!manager.should_restart(1, 5, 3));
    }

    // === 测试: 不应该重启 ===
    #[test]
    fn test_should_not_restart() {
        let manager = LifecycleManager::new(100);

        // 重启次数已达上限
        assert!(!manager.should_restart(1, 3, 3));
        assert!(!manager.should_restart(1, 10, 5));

        // 最大重启次数为 0，不允许重启
        assert!(!manager.should_restart(1, 0, 0));
    }

    // === 测试: 事件计数 ===
    #[test]
    fn test_event_count() {
        let manager = LifecycleManager::new(100);
        assert_eq!(manager.event_count(), 0);

        manager.record_event(1, LifecycleEvent::Registered, "注册");
        assert_eq!(manager.event_count(), 1);

        manager.record_event(2, LifecycleEvent::Registered, "注册");
        manager.record_event(3, LifecycleEvent::Registered, "注册");
        assert_eq!(manager.event_count(), 3);
    }

    // === 测试: 最大日志限制 ===
    #[test]
    fn test_max_log() {
        // 创建最大容量为 5 的管理器
        let manager = LifecycleManager::new(5);

        // 记录 8 条事件
        for i in 0..8 {
            manager.record_event(i, LifecycleEvent::Registered, "注册");
        }

        // 日志应只保留最近 5 条
        assert_eq!(manager.event_count(), 5);

        // 最旧的记录应该是 service_id=3
        let recent = manager.get_recent_events(5);
        assert_eq!(recent[0].service_id, 3);
        assert_eq!(recent[4].service_id, 7);
    }
}
