//! 安全审计日志模块
//!
//! 提供安全事件的记录、查询和过滤功能。
//! 审计日志是安全合规的重要组成部分，记录所有安全相关的事件。

/// 审计日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AuditLevel {
    /// 调试级别
    Debug = 0,
    /// 信息级别
    Info = 1,
    /// 警告级别
    Warning = 2,
    /// 错误级别
    Error = 3,
    /// 严重级别
    Critical = 4,
}

impl std::fmt::Display for AuditLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditLevel::Debug => write!(f, "DEBUG"),
            AuditLevel::Info => write!(f, "INFO"),
            AuditLevel::Warning => write!(f, "WARNING"),
            AuditLevel::Error => write!(f, "ERROR"),
            AuditLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// 审计事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventType {
    /// 系统调用
    Syscall {
        /// 系统调用号
        number: u64,
        /// 参数描述
        args: String,
    },
    /// 能力检查
    CapabilityCheck {
        /// 检查的能力名称
        capability: String,
        /// 是否授予
        granted: bool,
    },
    /// Agent 创建
    AgentSpawn {
        /// Agent ID
        agent_id: String,
    },
    /// Agent 终止
    AgentKill {
        /// Agent ID
        agent_id: String,
    },
    /// Agent 间消息
    AgentMessage {
        /// 发送方
        src: String,
        /// 接收方
        dst: String,
    },
    /// 文件访问
    FileAccess {
        /// 文件路径
        path: String,
        /// 操作类型
        operation: String,
    },
    /// 网络访问
    NetworkAccess {
        /// 网络端点
        endpoint: String,
        /// 操作类型
        operation: String,
    },
    /// 安全违规
    SecurityViolation {
        /// 违规类型
        violation_type: String,
    },
    /// 认证事件
    Authentication {
        /// 用户名
        user: String,
        /// 是否成功
        success: bool,
    },
    /// 授权事件
    Authorization {
        /// 请求者
        requester: String,
        /// 资源
        resource: String,
        /// 决策
        decision: String,
    },
    /// 自定义事件
    Custom {
        /// 事件类型
        event_type: String,
        /// 详细信息
        details: String,
    },
}

impl AuditEventType {
    /// 获取事件类型的前缀名称（用于过滤）
    pub fn type_prefix(&self) -> &str {
        match self {
            AuditEventType::Syscall { .. } => "Syscall",
            AuditEventType::CapabilityCheck { .. } => "CapabilityCheck",
            AuditEventType::AgentSpawn { .. } => "AgentSpawn",
            AuditEventType::AgentKill { .. } => "AgentKill",
            AuditEventType::AgentMessage { .. } => "AgentMessage",
            AuditEventType::FileAccess { .. } => "FileAccess",
            AuditEventType::NetworkAccess { .. } => "NetworkAccess",
            AuditEventType::SecurityViolation { .. } => "SecurityViolation",
            AuditEventType::Authentication { .. } => "Authentication",
            AuditEventType::Authorization { .. } => "Authorization",
            AuditEventType::Custom { event_type, .. } => event_type.as_str(),
        }
    }
}

/// 安全审计日志条目
#[derive(Debug, Clone)]
pub struct SecurityAuditEntry {
    /// 时间戳
    pub timestamp: u64,
    /// 日志级别
    pub level: AuditLevel,
    /// 事件类型
    pub event_type: AuditEventType,
    /// 关联的 Agent ID（可选）
    pub agent_id: Option<String>,
    /// 日志消息
    pub message: String,
}

/// 审计日志过滤器
///
/// 用于查询审计日志时进行过滤。
/// 所有过滤条件都是 AND 关系。
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// 最低日志级别
    pub min_level: Option<AuditLevel>,
    /// 最高日志级别
    pub max_level: Option<AuditLevel>,
    /// Agent ID 过滤
    pub agent_id: Option<String>,
    /// 起始时间戳（包含）
    pub since: Option<u64>,
    /// 结束时间戳（包含）
    pub until: Option<u64>,
    /// 事件类型前缀过滤
    pub event_type_prefix: Option<String>,
    /// 结果数量限制
    pub limit: Option<usize>,
}

impl AuditFilter {
    /// 创建一个新的空过滤器
    pub fn new() -> Self {
        AuditFilter::default()
    }

    /// 设置最低日志级别
    pub fn min_level(mut self, level: AuditLevel) -> Self {
        self.min_level = Some(level);
        self
    }

    /// 设置最高日志级别
    pub fn max_level(mut self, level: AuditLevel) -> Self {
        self.max_level = Some(level);
        self
    }

    /// 设置 Agent ID 过滤
    pub fn agent_id(mut self, id: &str) -> Self {
        self.agent_id = Some(id.to_string());
        self
    }

    /// 设置起始时间戳
    pub fn since(mut self, timestamp: u64) -> Self {
        self.since = Some(timestamp);
        self
    }

    /// 设置结束时间戳
    pub fn until(mut self, timestamp: u64) -> Self {
        self.until = Some(timestamp);
        self
    }

    /// 设置事件类型前缀
    pub fn event_type_prefix(mut self, prefix: &str) -> Self {
        self.event_type_prefix = Some(prefix.to_string());
        self
    }

    /// 设置结果数量限制
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 检查条目是否匹配过滤器
    fn matches(&self, entry: &SecurityAuditEntry) -> bool {
        // 检查最低级别
        if let Some(ref min) = self.min_level {
            if entry.level < *min {
                return false;
            }
        }

        // 检查最高级别
        if let Some(ref max) = self.max_level {
            if entry.level > *max {
                return false;
            }
        }

        // 检查 Agent ID
        if let Some(ref agent) = self.agent_id {
            if entry.agent_id.as_ref() != Some(agent) {
                return false;
            }
        }

        // 检查起始时间
        if let Some(since) = self.since {
            if entry.timestamp < since {
                return false;
            }
        }

        // 检查结束时间
        if let Some(until) = self.until {
            if entry.timestamp > until {
                return false;
            }
        }

        // 检查事件类型前缀
        if let Some(ref prefix) = self.event_type_prefix {
            if !entry.event_type.type_prefix().starts_with(prefix.as_str()) {
                return false;
            }
        }

        true
    }
}

/// 安全审计日志
///
/// 环形缓冲区式的审计日志，当日志条目超过最大容量时，
/// 最早的条目会被丢弃。
pub struct SecurityAuditLog {
    /// 日志条目列表
    entries: Vec<SecurityAuditEntry>,
    /// 最大条目数
    max_entries: usize,
    /// 时间戳计数器（简化版）
    timestamp_counter: u64,
}

impl SecurityAuditLog {
    /// 创建一个新的安全审计日志
    ///
    /// # 参数
    /// - `max_entries`: 最大条目数
    pub fn new(max_entries: usize) -> Self {
        SecurityAuditLog {
            entries: Vec::with_capacity(max_entries),
            max_entries,
            timestamp_counter: 0,
        }
    }

    /// 记录一个事件
    ///
    /// # 参数
    /// - `level`: 日志级别
    /// - `event_type`: 事件类型
    /// - `agent_id`: 关联的 Agent ID（可选）
    /// - `message`: 日志消息
    pub fn log(
        &mut self,
        level: AuditLevel,
        event_type: AuditEventType,
        agent_id: Option<&str>,
        message: &str,
    ) {
        self.timestamp_counter += 1;

        let entry = SecurityAuditEntry {
            timestamp: self.timestamp_counter,
            level,
            event_type,
            agent_id: agent_id.map(|s| s.to_string()),
            message: message.to_string(),
        };

        // 如果超过最大容量，移除最早的条目
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }

        self.entries.push(entry);
    }

    /// 查询日志
    ///
    /// 根据过滤器条件查询匹配的日志条目。
    pub fn query(&self, filter: &AuditFilter) -> Vec<&SecurityAuditEntry> {
        let mut results: Vec<&SecurityAuditEntry> = self
            .entries
            .iter()
            .filter(|entry| filter.matches(entry))
            .collect();

        // 应用数量限制
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        results
    }

    /// 获取所有条目
    pub fn entries(&self) -> &[SecurityAuditEntry] {
        &self.entries
    }

    /// 清除日志
    pub fn clear(&mut self) {
        self.entries.clear();
        self.timestamp_counter = 0;
    }

    /// 返回条目数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 检查日志是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_level_ordering() {
        assert!(AuditLevel::Debug < AuditLevel::Info);
        assert!(AuditLevel::Info < AuditLevel::Warning);
        assert!(AuditLevel::Warning < AuditLevel::Error);
        assert!(AuditLevel::Error < AuditLevel::Critical);
    }

    #[test]
    fn test_audit_event_type_prefix() {
        assert_eq!(
            AuditEventType::Syscall { number: 1, args: "test".to_string() }.type_prefix(),
            "Syscall"
        );
        assert_eq!(
            AuditEventType::AgentSpawn { agent_id: "a1".to_string() }.type_prefix(),
            "AgentSpawn"
        );
        assert_eq!(
            AuditEventType::Custom { event_type: "MyEvent".to_string(), details: "x".to_string() }.type_prefix(),
            "MyEvent"
        );
    }

    #[test]
    fn test_audit_log_log_and_entries() {
        let mut log = SecurityAuditLog::new(100);

        log.log(
            AuditLevel::Info,
            AuditEventType::AgentSpawn { agent_id: "agent-1".to_string() },
            Some("system"),
            "Agent agent-1 已创建",
        );

        log.log(
            AuditLevel::Warning,
            AuditEventType::SecurityViolation { violation_type: "unauthorized_access".to_string() },
            Some("agent-2"),
            "未授权访问尝试",
        );

        assert_eq!(log.len(), 2);

        let entries = log.entries();
        assert_eq!(entries[0].level, AuditLevel::Info);
        assert_eq!(entries[0].timestamp, 1);
        assert_eq!(entries[1].level, AuditLevel::Warning);
        assert_eq!(entries[1].timestamp, 2);
    }

    #[test]
    fn test_audit_log_clear() {
        let mut log = SecurityAuditLog::new(100);

        log.log(
            AuditLevel::Info,
            AuditEventType::AgentSpawn { agent_id: "agent-1".to_string() },
            None,
            "测试消息",
        );

        assert_eq!(log.len(), 1);
        log.clear();
        assert_eq!(log.len(), 0);
        assert!(log.is_empty());
    }

    #[test]
    fn test_audit_log_max_entries() {
        let mut log = SecurityAuditLog::new(3);

        for i in 0..5 {
            log.log(
                AuditLevel::Info,
                AuditEventType::Custom {
                    event_type: format!("event-{}", i),
                    details: format!("detail-{}", i),
                },
                None,
                &format!("消息 {}", i),
            );
        }

        // 应该只保留最后 3 条
        assert_eq!(log.len(), 3);
        let entries = log.entries();
        assert_eq!(entries[0].timestamp, 3);
        assert_eq!(entries[1].timestamp, 4);
        assert_eq!(entries[2].timestamp, 5);
    }

    #[test]
    fn test_audit_filter_by_level() {
        let mut log = SecurityAuditLog::new(100);

        log.log(AuditLevel::Debug, AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() }, None, "调试");
        log.log(AuditLevel::Info, AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() }, None, "信息");
        log.log(AuditLevel::Error, AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() }, None, "错误");

        let filter = AuditFilter::new().min_level(AuditLevel::Warning);
        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].level, AuditLevel::Error);
    }

    #[test]
    fn test_audit_filter_by_agent_id() {
        let mut log = SecurityAuditLog::new(100);

        log.log(
            AuditLevel::Info,
            AuditEventType::AgentSpawn { agent_id: "agent-1".to_string() },
            Some("agent-1"),
            "Agent-1 创建",
        );
        log.log(
            AuditLevel::Info,
            AuditEventType::AgentSpawn { agent_id: "agent-2".to_string() },
            Some("agent-2"),
            "Agent-2 创建",
        );

        let filter = AuditFilter::new().agent_id("agent-1");
        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].agent_id.as_deref(), Some("agent-1"));
    }

    #[test]
    fn test_audit_filter_by_time_range() {
        let mut log = SecurityAuditLog::new(100);

        log.log(AuditLevel::Info, AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() }, None, "事件1");
        log.log(AuditLevel::Info, AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() }, None, "事件2");
        log.log(AuditLevel::Info, AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() }, None, "事件3");

        let filter = AuditFilter::new().since(2).until(2);
        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].timestamp, 2);
    }

    #[test]
    fn test_audit_filter_by_event_type_prefix() {
        let mut log = SecurityAuditLog::new(100);

        log.log(
            AuditLevel::Info,
            AuditEventType::AgentSpawn { agent_id: "agent-1".to_string() },
            None,
            "Agent 创建",
        );
        log.log(
            AuditLevel::Info,
            AuditEventType::AgentKill { agent_id: "agent-1".to_string() },
            None,
            "Agent 终止",
        );
        log.log(
            AuditLevel::Info,
            AuditEventType::FileAccess { path: "/test".to_string(), operation: "read".to_string() },
            None,
            "文件访问",
        );

        let filter = AuditFilter::new().event_type_prefix("Agent");
        let results = log.query(&filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_audit_filter_by_limit() {
        let mut log = SecurityAuditLog::new(100);

        for i in 0..10 {
            log.log(
                AuditLevel::Info,
                AuditEventType::Custom { event_type: "test".to_string(), details: "".to_string() },
                None,
                &format!("消息 {}", i),
            );
        }

        let filter = AuditFilter::new().limit(3);
        let results = log.query(&filter);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_audit_filter_combined() {
        let mut log = SecurityAuditLog::new(100);

        log.log(AuditLevel::Debug, AuditEventType::AgentSpawn { agent_id: "a1".to_string() }, Some("a1"), "调试");
        log.log(AuditLevel::Info, AuditEventType::AgentSpawn { agent_id: "a1".to_string() }, Some("a1"), "信息");
        log.log(AuditLevel::Error, AuditEventType::AgentKill { agent_id: "a2".to_string() }, Some("a2"), "错误");
        log.log(AuditLevel::Critical, AuditEventType::SecurityViolation { violation_type: "test".to_string() }, Some("a1"), "严重");

        // 级别 >= Warning 且 Agent ID = a1
        let filter = AuditFilter::new()
            .min_level(AuditLevel::Warning)
            .agent_id("a1");

        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].level, AuditLevel::Critical);
    }

    #[test]
    fn test_audit_log_is_empty() {
        let log = SecurityAuditLog::new(100);
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }
}
