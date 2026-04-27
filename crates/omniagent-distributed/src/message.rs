//! 分布式消息传递
//!
//! 包含消息类型定义和消息总线实现。

use crate::crdt::VectorClock;
use crate::node::NodeId;

/// 分布式消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DistributedMessageType {
    /// 心跳消息
    Heartbeat = 0,
    /// Agent 创建请求
    AgentSpawn = 1,
    /// Agent 终止请求
    AgentKill = 2,
    /// Agent 间消息
    AgentMessage = 3,
    /// 状态同步
    StateSync = 4,
    /// 加入集群
    ClusterJoin = 5,
    /// 离开集群
    ClusterLeave = 6,
    /// CRDT 更新
    CrdtUpdate = 7,
    /// RPC 请求
    RpcRequest = 8,
    /// RPC 响应
    RpcResponse = 9,
}

impl DistributedMessageType {
    /// 从数值创建消息类型
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(DistributedMessageType::Heartbeat),
            1 => Some(DistributedMessageType::AgentSpawn),
            2 => Some(DistributedMessageType::AgentKill),
            3 => Some(DistributedMessageType::AgentMessage),
            4 => Some(DistributedMessageType::StateSync),
            5 => Some(DistributedMessageType::ClusterJoin),
            6 => Some(DistributedMessageType::ClusterLeave),
            7 => Some(DistributedMessageType::CrdtUpdate),
            8 => Some(DistributedMessageType::RpcRequest),
            9 => Some(DistributedMessageType::RpcResponse),
            _ => None,
        }
    }

    /// 获取消息类型的数值表示
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// 分布式消息
#[derive(Debug, Clone)]
pub struct DistributedMessage {
    /// 消息唯一 ID
    pub id: u64,
    /// 消息类型
    pub msg_type: DistributedMessageType,
    /// 源节点
    pub src: NodeId,
    /// 目标节点（None 表示广播）
    pub dst: Option<NodeId>,
    /// 消息负载
    pub payload: Vec<u8>,
    /// 消息时间戳
    pub timestamp: u64,
    /// 向量时钟（用于因果排序）
    pub vector_clock: VectorClock,
}

impl DistributedMessage {
    /// 创建新的分布式消息
    pub fn new(
        id: u64,
        msg_type: DistributedMessageType,
        src: NodeId,
        dst: Option<NodeId>,
        payload: Vec<u8>,
        timestamp: u64,
    ) -> Self {
        DistributedMessage {
            id,
            msg_type,
            src,
            dst,
            payload,
            timestamp,
            vector_clock: VectorClock::new(),
        }
    }

    /// 创建带向量时钟的消息
    pub fn with_clock(
        id: u64,
        msg_type: DistributedMessageType,
        src: NodeId,
        dst: Option<NodeId>,
        payload: Vec<u8>,
        timestamp: u64,
        clock: VectorClock,
    ) -> Self {
        DistributedMessage {
            id,
            msg_type,
            src,
            dst,
            payload,
            timestamp,
            vector_clock: clock,
        }
    }

    /// 判断是否为广播消息
    pub fn is_broadcast(&self) -> bool {
        self.dst.is_none()
    }

    /// 判断是否为单播消息
    pub fn is_unicast(&self) -> bool {
        self.dst.is_some()
    }

    /// 获取消息负载大小
    pub fn payload_size(&self) -> usize {
        self.payload.len()
    }
}

/// 消息总线
///
/// 管理本地消息队列和待发送消息，提供消息的发送、广播和接收功能。
pub struct MessageBus {
    /// 本地消息队列（待本地处理的消息）
    local_queue: Vec<DistributedMessage>,
    /// 待发送到远程节点的消息
    outgoing: Vec<DistributedMessage>,
    /// 消息 ID 计数器
    message_counter: u64,
}

impl MessageBus {
    /// 创建新的消息总线
    pub fn new() -> Self {
        MessageBus {
            local_queue: Vec::new(),
            outgoing: Vec::new(),
            message_counter: 0,
        }
    }

    /// 生成下一个消息 ID
    fn next_id(&mut self) -> u64 {
        self.message_counter += 1;
        self.message_counter
    }

    /// 发送消息到指定节点
    ///
    /// 消息会被放入 outgoing 队列等待发送
    pub fn send(&mut self, mut msg: DistributedMessage) {
        if msg.id == 0 {
            msg.id = self.next_id();
        }
        self.outgoing.push(msg);
    }

    /// 广播消息到所有节点
    ///
    /// 目标节点设为 None 表示广播
    pub fn broadcast(
        &mut self,
        msg_type: DistributedMessageType,
        payload: Vec<u8>,
        src: &NodeId,
        clock: &VectorClock,
    ) {
        let msg = DistributedMessage::with_clock(
            self.next_id(),
            msg_type,
            src.clone(),
            None, // 广播
            payload,
            0,    // 时间戳由调用者设置
            clock.clone(),
        );
        self.outgoing.push(msg);
    }

    /// 接收消息
    ///
    /// 从本地队列中取出一条消息
    pub fn receive(&mut self) -> Option<DistributedMessage> {
        if self.local_queue.is_empty() {
            None
        } else {
            Some(self.local_queue.remove(0))
        }
    }

    /// 获取所有待发送消息并清空 outgoing 队列
    pub fn drain_outgoing(&mut self) -> Vec<DistributedMessage> {
        std::mem::take(&mut self.outgoing)
    }

    /// 处理传入消息
    ///
    /// 将传入的消息放入本地队列
    pub fn handle_incoming(&mut self, messages: Vec<DistributedMessage>) {
        for msg in messages {
            self.local_queue.push(msg);
        }
    }

    /// 获取本地队列大小
    pub fn queue_size(&self) -> usize {
        self.local_queue.len()
    }

    /// 获取待发送队列大小
    pub fn outgoing_size(&self) -> usize {
        self.outgoing.len()
    }

    /// 获取已生成的消息总数
    pub fn message_count(&self) -> u64 {
        self.message_counter
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_message_type_from_u8() {
        assert_eq!(DistributedMessageType::from_u8(0), Some(DistributedMessageType::Heartbeat));
        assert_eq!(DistributedMessageType::from_u8(3), Some(DistributedMessageType::AgentMessage));
        assert_eq!(DistributedMessageType::from_u8(9), Some(DistributedMessageType::RpcResponse));
        assert_eq!(DistributedMessageType::from_u8(99), None);
    }

    #[test]
    fn test_distributed_message_type_as_u8() {
        assert_eq!(DistributedMessageType::Heartbeat.as_u8(), 0);
        assert_eq!(DistributedMessageType::AgentSpawn.as_u8(), 1);
        assert_eq!(DistributedMessageType::RpcResponse.as_u8(), 9);
    }

    #[test]
    fn test_distributed_message_new() {
        let src = NodeId::new();
        let dst = NodeId::new();
        let msg = DistributedMessage::new(
            1,
            DistributedMessageType::AgentMessage,
            src.clone(),
            Some(dst.clone()),
            vec![1, 2, 3],
            100,
        );

        assert_eq!(msg.id, 1);
        assert_eq!(msg.msg_type, DistributedMessageType::AgentMessage);
        assert_eq!(msg.src, src);
        assert_eq!(msg.dst, Some(dst));
        assert_eq!(msg.payload, vec![1, 2, 3]);
        assert_eq!(msg.timestamp, 100);
        assert!(msg.is_unicast());
        assert!(!msg.is_broadcast());
    }

    #[test]
    fn test_distributed_message_broadcast() {
        let src = NodeId::new();
        let msg = DistributedMessage::new(
            1,
            DistributedMessageType::Heartbeat,
            src,
            None,
            vec![],
            0,
        );

        assert!(msg.is_broadcast());
        assert!(!msg.is_unicast());
    }

    #[test]
    fn test_distributed_message_payload_size() {
        let src = NodeId::new();
        let msg = DistributedMessage::new(
            1,
            DistributedMessageType::AgentMessage,
            src,
            None,
            vec![1, 2, 3, 4, 5],
            0,
        );

        assert_eq!(msg.payload_size(), 5);
    }

    #[test]
    fn test_message_bus_send() {
        let src = NodeId::new();
        let dst = NodeId::new();
        let mut bus = MessageBus::new();

        let msg = DistributedMessage::new(
            0, // ID 为 0，应该自动分配
            DistributedMessageType::AgentMessage,
            src,
            Some(dst),
            vec![1, 2, 3],
            100,
        );

        bus.send(msg);
        assert_eq!(bus.outgoing_size(), 1);
        assert_eq!(bus.message_count(), 1);
    }

    #[test]
    fn test_message_bus_broadcast() {
        let src = NodeId::new();
        let mut bus = MessageBus::new();
        let clock = VectorClock::new();

        bus.broadcast(
            DistributedMessageType::Heartbeat,
            vec![1, 2, 3],
            &src,
            &clock,
        );

        assert_eq!(bus.outgoing_size(), 1);
        assert_eq!(bus.message_count(), 1);

        let outgoing = bus.drain_outgoing();
        assert_eq!(outgoing.len(), 1);
        assert!(outgoing[0].is_broadcast());
        assert_eq!(outgoing[0].msg_type, DistributedMessageType::Heartbeat);
    }

    #[test]
    fn test_message_bus_receive_empty() {
        let mut bus = MessageBus::new();
        assert!(bus.receive().is_none());
    }

    #[test]
    fn test_message_bus_handle_incoming_and_receive() {
        let src = NodeId::new();
        let mut bus = MessageBus::new();

        let msg = DistributedMessage::new(
            1,
            DistributedMessageType::AgentMessage,
            src,
            None,
            vec![42],
            100,
        );

        bus.handle_incoming(vec![msg]);
        assert_eq!(bus.queue_size(), 1);

        let received = bus.receive();
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.id, 1);
        assert_eq!(received.payload, vec![42]);

        // 队列应该为空
        assert_eq!(bus.queue_size(), 0);
        assert!(bus.receive().is_none());
    }

    #[test]
    fn test_message_bus_drain_outgoing() {
        let src = NodeId::new();
        let dst = NodeId::new();
        let mut bus = MessageBus::new();

        bus.send(DistributedMessage::new(
            0, DistributedMessageType::Heartbeat, src.clone(), Some(dst.clone()), vec![], 0,
        ));
        bus.send(DistributedMessage::new(
            0, DistributedMessageType::AgentMessage, src.clone(), Some(dst.clone()), vec![1], 0,
        ));
        bus.send(DistributedMessage::new(
            0, DistributedMessageType::StateSync, src, Some(dst), vec![2], 0,
        ));

        assert_eq!(bus.outgoing_size(), 3);

        let outgoing = bus.drain_outgoing();
        assert_eq!(outgoing.len(), 3);
        assert_eq!(bus.outgoing_size(), 0);
    }

    #[test]
    fn test_message_bus_send_preserves_id() {
        let src = NodeId::new();
        let dst = NodeId::new();
        let mut bus = MessageBus::new();

        // 指定 ID 的消息应保留原始 ID
        let msg = DistributedMessage::new(
            42,
            DistributedMessageType::AgentMessage,
            src,
            Some(dst),
            vec![],
            0,
        );

        bus.send(msg);
        let outgoing = bus.drain_outgoing();
        assert_eq!(outgoing[0].id, 42);
        // 指定 ID 不应影响计数器
        assert_eq!(bus.message_count(), 0);
    }
}
