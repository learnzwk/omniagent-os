//! libagent - OmniAgent OS 用户态 Agent 库
//!
//! 提供 Agent 生命周期管理、通信、配置等 API

use omniagent_syscall::agent;
use omniagent_ipc::{ChannelId, PortId};
use serde::{Deserialize, Serialize};

/// Agent 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

impl AgentId {
    pub const INVALID: AgentId = AgentId(0);
    pub const SYSTEM: AgentId = AgentId(1);

    pub fn new(id: u64) -> Self { AgentId(id) }
    pub fn is_valid(&self) -> bool { self.0 != 0 }
    pub fn is_system(&self) -> bool { *self == Self::SYSTEM }
}

impl Default for AgentId {
    fn default() -> Self { Self::INVALID }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Agent({})", self.0)
    }
}

/// Agent 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    System = 0,
    Service = 1,
    Expert = 2,
    Worker = 3,
    Monitor = 4,
}

/// Agent 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Created = 0,
    Configuring = 1,
    Running = 2,
    Paused = 3,
    Suspended = 4,
    Terminating = 5,
    Terminated = 6,
}

/// Agent 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AgentPriority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Realtime = 4,
}

/// Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub agent_type: AgentType,
    pub priority: AgentPriority,
    pub max_memory_bytes: u64,
    pub max_fds: u32,
    pub stack_size: usize,
    pub entry_point: String,
    pub capabilities: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            agent_type: AgentType::Worker,
            priority: AgentPriority::Normal,
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_fds: 256,
            stack_size: 8 * 1024 * 1024, // 8MB
            entry_point: String::new(),
            capabilities: Vec::new(),
        }
    }
}

impl AgentConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    pub fn with_type(mut self, t: AgentType) -> Self {
        self.agent_type = t;
        self
    }

    pub fn with_priority(mut self, p: AgentPriority) -> Self {
        self.priority = p;
        self
    }

    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.max_memory_bytes = bytes;
        self
    }
}

/// Agent 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: AgentId,
    pub to: AgentId,
    pub subject: String,
    pub payload: Vec<u8>,
}

impl AgentMessage {
    pub fn new(from: AgentId, to: AgentId, subject: &str) -> Self {
        Self {
            from,
            to,
            subject: subject.to_string(),
            payload: Vec::new(),
        }
    }

    pub fn with_payload(mut self, data: Vec<u8>) -> Self {
        self.payload = data;
        self
    }
}

/// Agent 句柄 (运行中的 Agent 引用)
#[derive(Debug)]
pub struct AgentHandle {
    pub id: AgentId,
    pub state: AgentState,
    pub config: AgentConfig,
}

impl AgentHandle {
    pub fn new(id: AgentId, config: AgentConfig) -> Self {
        Self {
            id,
            state: AgentState::Created,
            config,
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == AgentState::Running
    }

    pub fn id(&self) -> AgentId {
        self.id
    }
}

/// Agent 构建器
pub struct AgentBuilder {
    config: AgentConfig,
}

impl AgentBuilder {
    pub fn new(name: &str) -> Self {
        Self { config: AgentConfig::new(name) }
    }

    pub fn agent_type(mut self, t: AgentType) -> Self {
        self.config.agent_type = t;
        self
    }

    pub fn priority(mut self, p: AgentPriority) -> Self {
        self.config.priority = p;
        self
    }

    pub fn memory(mut self, bytes: u64) -> Self {
        self.config.max_memory_bytes = bytes;
        self
    }

    pub fn stack_size(mut self, size: usize) -> Self {
        self.config.stack_size = size;
        self
    }

    pub fn capability(mut self, cap: &str) -> Self {
        self.config.capabilities.push(cap.to_string());
        self
    }

    pub fn build(self) -> AgentConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id() {
        let id = AgentId::INVALID;
        assert!(!id.is_valid());
        let id = AgentId::new(42);
        assert!(id.is_valid());
    }

    #[test]
    fn test_agent_id_display() {
        let id = AgentId::new(42);
        assert_eq!(format!("{}", id), "Agent(42)");
    }

    #[test]
    fn test_agent_id_system() {
        assert!(AgentId::SYSTEM.is_system());
        assert!(!AgentId::new(42).is_system());
    }

    #[test]
    fn test_agent_config_default() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.agent_type, AgentType::Worker);
        assert_eq!(cfg.priority, AgentPriority::Normal);
        assert_eq!(cfg.max_memory_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn test_agent_config_builder() {
        let cfg = AgentConfig::new("test-agent")
            .with_type(AgentType::Expert)
            .with_priority(AgentPriority::High)
            .with_memory(128 * 1024 * 1024);
        assert_eq!(cfg.name, "test-agent");
        assert_eq!(cfg.agent_type, AgentType::Expert);
        assert_eq!(cfg.priority, AgentPriority::High);
        assert_eq!(cfg.max_memory_bytes, 128 * 1024 * 1024);
    }

    #[test]
    fn test_agent_message() {
        let msg = AgentMessage::new(AgentId::new(1), AgentId::new(2), "hello")
            .with_payload(vec![1, 2, 3]);
        assert_eq!(msg.from, AgentId::new(1));
        assert_eq!(msg.to, AgentId::new(2));
        assert_eq!(msg.payload, vec![1, 2, 3]);
    }

    #[test]
    fn test_agent_handle() {
        let handle = AgentHandle::new(
            AgentId::new(1),
            AgentConfig::new("test"),
        );
        assert!(!handle.is_running());
        assert_eq!(handle.state, AgentState::Created);
    }

    #[test]
    fn test_agent_builder() {
        let cfg = AgentBuilder::new("my-agent")
            .agent_type(AgentType::Service)
            .priority(AgentPriority::Realtime)
            .memory(256 * 1024 * 1024)
            .capability("file:read")
            .capability("net:connect")
            .build();
        assert_eq!(cfg.name, "my-agent");
        assert_eq!(cfg.agent_type, AgentType::Service);
        assert_eq!(cfg.capabilities.len(), 2);
    }

    #[test]
    fn test_agent_state_values() {
        assert_eq!(AgentState::Created as u8, 0);
        assert_eq!(AgentState::Running as u8, 2);
        assert_eq!(AgentState::Terminated as u8, 6);
    }

    #[test]
    fn test_agent_priority_ordering() {
        assert!(AgentPriority::Realtime > AgentPriority::High);
        assert!(AgentPriority::High > AgentPriority::Normal);
        assert!(AgentPriority::Normal > AgentPriority::Low);
        assert!(AgentPriority::Low > AgentPriority::Idle);
    }
}
