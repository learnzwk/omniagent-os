//! 健康监控模块
//!
//! 实现服务健康检查、失败计数和重启决策功能。
//! 模仿鸿蒙内核的健康监控机制。

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use spin::Mutex;

// ============================================================================
// 健康状态
// ============================================================================

/// 健康检查结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// 健康
    Healthy = 0,
    /// 降级
    Degraded = 1,
    /// 不健康
    Unhealthy = 2,
    /// 未知
    Unknown = 3,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "健康"),
            HealthStatus::Degraded => write!(f, "降级"),
            HealthStatus::Unhealthy => write!(f, "不健康"),
            HealthStatus::Unknown => write!(f, "未知"),
        }
    }
}

// ============================================================================
// 健康检查配置
// ============================================================================

/// 健康检查配置
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// 检查间隔（毫秒）
    pub check_interval_ms: u32,
    /// 超时时间（毫秒）
    pub timeout_ms: u32,
    /// 最大失败次数（超过后判定为不健康）
    pub max_failures: u32,
}

// ============================================================================
// 健康监控器
// ============================================================================

/// 服务健康监控器
///
/// 跟踪服务的健康状态，记录失败次数，判断是否需要重启。
pub struct HealthMonitor {
    /// 服务健康状态映射表
    health_status: Mutex<BTreeMap<u64, HealthStatus>>,
    /// 服务失败计数映射表
    failure_counts: Mutex<BTreeMap<u64, u32>>,
    /// 健康检查配置映射表
    configs: Mutex<BTreeMap<u64, HealthCheckConfig>>,
}

impl HealthMonitor {
    /// 创建新的健康监控器
    pub fn new() -> Self {
        HealthMonitor {
            health_status: Mutex::new(BTreeMap::new()),
            failure_counts: Mutex::new(BTreeMap::new()),
            configs: Mutex::new(BTreeMap::new()),
        }
    }

    /// 注册服务进行健康监控
    ///
    /// 注册后初始健康状态为 Unknown。
    pub fn register(&self, service_id: u64, config: HealthCheckConfig) {
        {
            let mut statuses = self.health_status.lock();
            statuses.insert(service_id, HealthStatus::Unknown);
        }
        {
            let mut counts = self.failure_counts.lock();
            counts.insert(service_id, 0);
        }
        {
            let mut configs = self.configs.lock();
            configs.insert(service_id, config);
        }
    }

    /// 报告服务健康状态
    pub fn report_health(&self, service_id: u64, status: HealthStatus) {
        let mut statuses = self.health_status.lock();
        statuses.insert(service_id, status);
    }

    /// 获取服务健康状态
    ///
    /// 如果服务未注册，返回 Unknown。
    pub fn get_health(&self, service_id: u64) -> HealthStatus {
        let statuses = self.health_status.lock();
        statuses
            .get(&service_id)
            .copied()
            .unwrap_or(HealthStatus::Unknown)
    }

    /// 记录一次失败
    ///
    /// 增加失败计数，如果超过最大失败次数则标记为 Unhealthy。
    pub fn record_failure(&self, service_id: u64) {
        let max_failures = {
            let configs = self.configs.lock();
            configs
                .get(&service_id)
                .map(|c| c.max_failures)
                .unwrap_or(3)
        };

        let new_count = {
            let mut counts = self.failure_counts.lock();
            let count = counts.get(&service_id).copied().unwrap_or(0) + 1;
            counts.insert(service_id, count);
            count
        };

        // 如果失败次数超过阈值，标记为不健康
        if new_count >= max_failures {
            let mut statuses = self.health_status.lock();
            statuses.insert(service_id, HealthStatus::Unhealthy);
        }
    }

    /// 记录一次成功
    ///
    /// 重置失败计数，标记为 Healthy。
    pub fn record_success(&self, service_id: u64) {
        {
            let mut counts = self.failure_counts.lock();
            counts.insert(service_id, 0);
        }
        {
            let mut statuses = self.health_status.lock();
            statuses.insert(service_id, HealthStatus::Healthy);
        }
    }

    /// 判断服务是否应该重启
    ///
    /// 当服务状态为 Unhealthy 时返回 true。
    pub fn should_restart(&self, service_id: u64) -> bool {
        let statuses = self.health_status.lock();
        statuses
            .get(&service_id)
            .map(|&s| s == HealthStatus::Unhealthy)
            .unwrap_or(false)
    }

    /// 列出所有不健康的服务 ID
    pub fn list_unhealthy(&self) -> Vec<u64> {
        let statuses = self.health_status.lock();
        statuses
            .iter()
            .filter(|(_, &status)| status == HealthStatus::Unhealthy)
            .map(|(&id, _)| id)
            .collect()
    }
}

/// 全局健康监控器实例
pub static HEALTH_MONITOR: spin::Lazy<Mutex<HealthMonitor>> = spin::Lazy::new(|| {
    Mutex::new(HealthMonitor::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建默认健康检查配置
    fn default_config() -> HealthCheckConfig {
        HealthCheckConfig {
            check_interval_ms: 5000,
            timeout_ms: 1000,
            max_failures: 3,
        }
    }

    // === 测试: 创建健康监控器 ===
    #[test]
    fn test_new() {
        let monitor = HealthMonitor::new();
        assert_eq!(monitor.get_health(1), HealthStatus::Unknown);
        assert!(!monitor.should_restart(1));
    }

    // === 测试: 注册服务 ===
    #[test]
    fn test_register() {
        let monitor = HealthMonitor::new();
        monitor.register(1, default_config());

        assert_eq!(monitor.get_health(1), HealthStatus::Unknown);
    }

    // === 测试: 报告健康状态 ===
    #[test]
    fn test_report_health() {
        let monitor = HealthMonitor::new();
        monitor.register(1, default_config());

        monitor.report_health(1, HealthStatus::Healthy);
        assert_eq!(monitor.get_health(1), HealthStatus::Healthy);

        monitor.report_health(1, HealthStatus::Degraded);
        assert_eq!(monitor.get_health(1), HealthStatus::Degraded);
    }

    // === 测试: 记录失败 ===
    #[test]
    fn test_record_failure() {
        let monitor = HealthMonitor::new();
        monitor.register(1, default_config()); // max_failures = 3

        // 未达到阈值时不应标记为不健康
        monitor.record_failure(1);
        assert_eq!(monitor.get_health(1), HealthStatus::Unknown);

        monitor.record_failure(1);
        assert_eq!(monitor.get_health(1), HealthStatus::Unknown);

        // 达到阈值时应标记为不健康
        monitor.record_failure(1);
        assert_eq!(monitor.get_health(1), HealthStatus::Unhealthy);
        assert!(monitor.should_restart(1));
    }

    // === 测试: 记录成功 ===
    #[test]
    fn test_record_success() {
        let monitor = HealthMonitor::new();
        monitor.register(1, default_config());

        // 先记录失败
        monitor.record_failure(1);
        monitor.record_failure(1);

        // 成功后应重置
        monitor.record_success(1);
        assert_eq!(monitor.get_health(1), HealthStatus::Healthy);
        assert!(!monitor.should_restart(1));

        // 再次失败应从 0 开始计数
        monitor.record_failure(1);
        assert_eq!(monitor.get_health(1), HealthStatus::Healthy);
    }

    // === 测试: 判断是否应重启 ===
    #[test]
    fn test_should_restart() {
        let monitor = HealthMonitor::new();
        monitor.register(1, default_config());

        assert!(!monitor.should_restart(1));

        // 手动标记为不健康
        monitor.report_health(1, HealthStatus::Unhealthy);
        assert!(monitor.should_restart(1));

        // 恢复健康后不应重启
        monitor.report_health(1, HealthStatus::Healthy);
        assert!(!monitor.should_restart(1));
    }

    // === 测试: 列出不健康服务 ===
    #[test]
    fn test_list_unhealthy() {
        let monitor = HealthMonitor::new();
        monitor.register(1, default_config());
        monitor.register(2, default_config());
        monitor.register(3, default_config());

        monitor.report_health(1, HealthStatus::Healthy);
        monitor.report_health(2, HealthStatus::Unhealthy);
        monitor.report_health(3, HealthStatus::Degraded);

        let unhealthy = monitor.list_unhealthy();
        assert_eq!(unhealthy.len(), 1);
        assert!(unhealthy.contains(&2));
    }
}
