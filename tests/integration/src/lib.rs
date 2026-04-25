//! OmniAgent OS 集成测试
//!
//! 验证所有 workspace crate 之间的兼容性和互操作性

/// 测试 syscall 和 IPC 类型的互操作性
#[test]
fn test_syscall_ipc_compat() {
    use omniagent_ipc::{MessageHeader, MessageType, PortId};
    use omniagent_syscall::agent;

    // 验证 Agent syscall 号可用于 IPC 消息头中的端口 ID
    let port = PortId(agent::SYS_AGENT_MSG as u32);
    assert!(port.is_valid());

    // 验证消息头可以携带 Agent syscall 相关信息
    let mut hdr = MessageHeader::request(1, port.0);
    assert_eq!(hdr.msg_type, MessageType::Request);
}

/// 测试驱动框架和 IPC 类型的互操作性
#[test]
fn test_driver_ipc_compat() {
    use omniagent_driver::{DeviceDriver, DeviceType, DriverError, DriverId, DriverState};
    use omniagent_ipc::PortId;

    // 验证驱动 ID 和端口 ID 可以共存
    let driver_id = DriverId::new(1);
    let port_id = PortId(1);
    assert!(driver_id.is_valid());
    assert!(port_id.is_valid());
}

/// 测试 libagent 和 syscall 定义的互操作性
#[test]
fn test_libagent_syscall_compat() {
    use libagent::{AgentConfig, AgentId, AgentType};
    use omniagent_syscall::agent;

    // 验证 Agent 配置可以与 syscall 定义配合使用
    let config = AgentConfig::new("test-agent")
        .with_type(AgentType::Expert);
    assert_eq!(config.name, "test-agent");
    assert_eq!(config.agent_type, AgentType::Expert);

    // 验证 Agent syscall 号范围正确
    assert!(agent::SYS_AGENT_SPAWN >= 512);
}

/// 测试 libagent 和 IPC 类型的互操作性
#[test]
fn test_libagent_ipc_compat() {
    use libagent::{AgentId, AgentMessage};
    use omniagent_ipc::MessageHeader;

    // 验证 Agent 消息可以映射到 IPC 消息头
    let msg = AgentMessage::new(AgentId::new(1), AgentId::new(2), "test");
    let hdr = MessageHeader::request(msg.from.0 as u32, msg.to.0 as u32);
    assert_eq!(hdr.src_port, 1);
    assert_eq!(hdr.dst_port, 2);
}

/// 测试全链路类型兼容性
#[test]
fn test_full_chain_compat() {
    use libagent::{AgentBuilder, AgentType, AgentPriority};
    use omniagent_ipc::{ChannelId, IpcError, MessageHeader, MessageType, PortId};
    use omniagent_syscall::agent;
    use omniagent_driver::{DeviceDriver, DeviceType, DriverId};

    // 创建 Agent 配置
    let config = AgentBuilder::new("fullchain-test")
        .agent_type(AgentType::Service)
        .priority(AgentPriority::High)
        .build();

    // 创建 IPC 消息头
    let hdr = MessageHeader::request(1, 2);
    assert_eq!(hdr.msg_type, MessageType::Request);

    // 验证所有类型可以共存
    let _agent_id = libagent::AgentId::new(42);
    let _port = PortId::new();
    let _channel = ChannelId::new();
    let _driver = DriverId::new(1);
    let _syscall = agent::SYS_AGENT_SPAWN;
    let _error = IpcError::ChannelNotFound;

    assert_eq!(config.agent_type, AgentType::Service);
}
