//! 统一传输模块
//! 模仿鸿蒙 DSoftBus 的统一传输层，提供消息发送、接收和统计功能

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use crate::softbus::error::SoftBusError;

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// 数据消息
    Data = 0,
    /// 控制消息
    Control = 1,
    /// 发现消息
    Discovery = 2,
    /// 认证消息
    Auth = 3,
    /// 心跳消息
    Heartbeat = 4,
    /// 确认消息
    Ack = 5,
}

/// 传输消息
#[derive(Debug, Clone)]
pub struct TransportMessage {
    /// 消息唯一标识
    pub msg_id: u64,
    /// 源设备 ID
    pub src_device: u64,
    /// 目标设备 ID
    pub dst_device: u64,
    /// 消息类型
    pub msg_type: MessageType,
    /// 消息负载
    pub payload: Vec<u8>,
    /// 优先级（0-255，值越大优先级越高）
    pub priority: u8,
    /// 生存时间
    pub ttl: u8,
}

/// 传输统计
#[derive(Debug)]
pub struct TransportStats {
    /// 已发送消息数
    pub messages_sent: AtomicU64,
    /// 已接收消息数
    pub messages_received: AtomicU64,
    /// 已发送字节数
    pub bytes_sent: AtomicU64,
    /// 已接收字节数
    pub bytes_received: AtomicU64,
    /// 错误计数
    pub errors: AtomicU64,
}

impl Default for TransportStats {
    fn default() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }
}

/// 统一传输层
/// 提供消息的发送、接收和排队管理
pub struct TransportLayer {
    /// 传输统计
    stats: TransportStats,
    /// 消息接收队列
    message_queue: Mutex<VecDeque<TransportMessage>>,
    /// 下一个消息 ID
    next_msg_id: AtomicU64,
}

impl TransportLayer {
    /// 创建新的传输层
    pub fn new() -> Self {
        Self {
            stats: TransportStats::default(),
            message_queue: Mutex::new(VecDeque::new()),
            next_msg_id: AtomicU64::new(1),
        }
    }

    /// 发送消息
    ///
    /// # 参数
    /// - `msg`: 要发送的传输消息
    ///
    /// # 返回
    /// 成功返回分配的消息 ID
    pub fn send(&self, mut msg: TransportMessage) -> Result<u64, SoftBusError> {
        // 检查 TTL
        if msg.ttl == 0 {
            self.stats.errors.fetch_add(1, Ordering::SeqCst);
            return Err(SoftBusError::InvalidMessage);
        }

        // 分配消息 ID
        let msg_id = self.next_msg_id.fetch_add(1, Ordering::SeqCst);
        msg.msg_id = msg_id;

        // 更新统计
        self.stats.messages_sent.fetch_add(1, Ordering::SeqCst);
        self.stats
            .bytes_sent
            .fetch_add(msg.payload.len() as u64, Ordering::SeqCst);

        // 将消息放入接收队列（模拟传输）
        let mut queue = self.message_queue.lock();
        queue.push_back(msg);

        Ok(msg_id)
    }

    /// 接收一条消息（从队列头部取出）
    ///
    /// # 返回
    /// 队列中有消息则返回 Some，否则返回 None
    pub fn receive(&self) -> Option<TransportMessage> {
        let mut queue = self.message_queue.lock();
        if let Some(msg) = queue.pop_front() {
            self.stats.messages_received.fetch_add(1, Ordering::SeqCst);
            self.stats
                .bytes_received
                .fetch_add(msg.payload.len() as u64, Ordering::SeqCst);
            Some(msg)
        } else {
            None
        }
    }

    /// 查看队列头部消息但不移除
    ///
    /// # 返回
    /// 队列中有消息则返回 Some，否则返回 None
    pub fn peek(&self) -> Option<TransportMessage> {
        let queue = self.message_queue.lock();
        queue.front().cloned()
    }

    /// 获取当前队列长度
    pub fn queue_len(&self) -> usize {
        let queue = self.message_queue.lock();
        queue.len()
    }

    /// 获取传输统计快照
    pub fn stats(&self) -> TransportStats {
        TransportStats {
            messages_sent: AtomicU64::new(self.stats.messages_sent.load(Ordering::SeqCst)),
            messages_received: AtomicU64::new(self.stats.messages_received.load(Ordering::SeqCst)),
            bytes_sent: AtomicU64::new(self.stats.bytes_sent.load(Ordering::SeqCst)),
            bytes_received: AtomicU64::new(self.stats.bytes_received.load(Ordering::SeqCst)),
            errors: AtomicU64::new(self.stats.errors.load(Ordering::SeqCst)),
        }
    }

    /// 重置传输统计
    pub fn reset_stats(&self) {
        self.stats.messages_sent.store(0, Ordering::SeqCst);
        self.stats.messages_received.store(0, Ordering::SeqCst);
        self.stats.bytes_sent.store(0, Ordering::SeqCst);
        self.stats.bytes_received.store(0, Ordering::SeqCst);
        self.stats.errors.store(0, Ordering::SeqCst);
    }
}

/// 全局传输层实例
pub static TRANSPORT: spin::Lazy<Mutex<TransportLayer>> = spin::Lazy::new(|| {
    Mutex::new(TransportLayer::new())
});

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用传输消息
    fn make_message(src: u64, dst: u64, msg_type: MessageType, payload: &[u8]) -> TransportMessage {
        TransportMessage {
            msg_id: 0,
            src_device: src,
            dst_device: dst,
            msg_type,
            payload: Vec::from(payload),
            priority: 5,
            ttl: 10,
        }
    }

    #[test]
    fn test_send_message() {
        let transport = TransportLayer::new();
        let msg = make_message(1, 2, MessageType::Data, b"hello");
        let msg_id = transport.send(msg).unwrap();
        assert_eq!(msg_id, 1);
        assert_eq!(transport.queue_len(), 1);
    }

    #[test]
    fn test_receive_message() {
        let transport = TransportLayer::new();
        let msg = make_message(1, 2, MessageType::Control, b"world");
        transport.send(msg).unwrap();

        let received = transport.receive();
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.src_device, 1);
        assert_eq!(received.dst_device, 2);
        assert_eq!(received.payload, Vec::from(b"world"));
        assert_eq!(transport.queue_len(), 0);

        // 队列为空时应返回 None
        let empty = transport.receive();
        assert!(empty.is_none());
    }

    #[test]
    fn test_peek_message() {
        let transport = TransportLayer::new();
        let msg = make_message(1, 2, MessageType::Heartbeat, b"ping");
        transport.send(msg).unwrap();

        // peek 不应移除消息
        let peeked = transport.peek();
        assert!(peeked.is_some());
        assert_eq!(transport.queue_len(), 1);

        // 再次 peek 仍应返回同一条消息
        let peeked2 = transport.peek();
        assert!(peeked2.is_some());
        assert_eq!(peeked.unwrap().msg_id, peeked2.unwrap().msg_id);
    }

    #[test]
    fn test_queue_len() {
        let transport = TransportLayer::new();
        assert_eq!(transport.queue_len(), 0);

        transport.send(make_message(1, 2, MessageType::Data, b"a")).unwrap();
        assert_eq!(transport.queue_len(), 1);

        transport.send(make_message(1, 2, MessageType::Data, b"b")).unwrap();
        assert_eq!(transport.queue_len(), 2);

        transport.receive();
        assert_eq!(transport.queue_len(), 1);
    }

    #[test]
    fn test_stats() {
        let transport = TransportLayer::new();

        transport.send(make_message(1, 2, MessageType::Data, b"hello")).unwrap();
        transport.send(make_message(1, 2, MessageType::Data, b"world")).unwrap();

        // 发送后统计
        let stats = transport.stats();
        assert_eq!(stats.messages_sent.load(Ordering::SeqCst), 2);
        assert_eq!(stats.bytes_sent.load(Ordering::SeqCst), 10); // "hello" + "world"

        // 接收一条消息
        transport.receive();
        let stats = transport.stats();
        assert_eq!(stats.messages_received.load(Ordering::SeqCst), 1);
        assert_eq!(stats.bytes_received.load(Ordering::SeqCst), 5); // "hello"
    }

    #[test]
    fn test_reset_stats() {
        let transport = TransportLayer::new();
        transport.send(make_message(1, 2, MessageType::Data, b"data")).unwrap();
        transport.receive();

        let stats_before = transport.stats();
        assert!(stats_before.messages_sent.load(Ordering::SeqCst) > 0);

        transport.reset_stats();

        let stats_after = transport.stats();
        assert_eq!(stats_after.messages_sent.load(Ordering::SeqCst), 0);
        assert_eq!(stats_after.messages_received.load(Ordering::SeqCst), 0);
        assert_eq!(stats_after.bytes_sent.load(Ordering::SeqCst), 0);
        assert_eq!(stats_after.bytes_received.load(Ordering::SeqCst), 0);
        assert_eq!(stats_after.errors.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_message_id_unique() {
        let transport = TransportLayer::new();

        let id1 = transport.send(make_message(1, 2, MessageType::Data, b"a")).unwrap();
        let id2 = transport.send(make_message(1, 2, MessageType::Data, b"b")).unwrap();
        let id3 = transport.send(make_message(1, 2, MessageType::Data, b"c")).unwrap();

        // 每条消息应有唯一 ID
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        // ID 应递增
        assert!(id2 > id1);
        assert!(id3 > id2);
    }

    /// 测试：TTL 为 0 的消息应被拒绝
    #[test]
    fn test_send_zero_ttl_rejected() {
        let transport = TransportLayer::new();

        let mut msg = make_message(1, 2, MessageType::Data, b"hello");
        msg.ttl = 0;

        let result = transport.send(msg);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SoftBusError::InvalidMessage);

        // 错误计数应增加
        let stats = transport.stats();
        assert_eq!(stats.errors.load(Ordering::SeqCst), 1);

        // 队列不应有消息
        assert_eq!(transport.queue_len(), 0);
    }

    /// 测试：多消息传输 - 发送和接收顺序一致
    #[test]
    fn test_multiple_messages_order() {
        let transport = TransportLayer::new();

        // 发送多条消息
        transport.send(make_message(1, 2, MessageType::Data, b"first")).unwrap();
        transport.send(make_message(1, 2, MessageType::Control, b"second")).unwrap();
        transport.send(make_message(1, 2, MessageType::Heartbeat, b"third")).unwrap();

        // 按顺序接收
        let msg1 = transport.receive().unwrap();
        assert_eq!(msg1.payload, Vec::from(b"first"));
        assert_eq!(msg1.msg_type, MessageType::Data);

        let msg2 = transport.receive().unwrap();
        assert_eq!(msg2.payload, Vec::from(b"second"));
        assert_eq!(msg2.msg_type, MessageType::Control);

        let msg3 = transport.receive().unwrap();
        assert_eq!(msg3.payload, Vec::from(b"third"));
        assert_eq!(msg3.msg_type, MessageType::Heartbeat);

        // 队列应为空
        assert!(transport.receive().is_none());
    }

    /// 测试：统计计数验证 - 发送和接收的字节数
    #[test]
    fn test_stats_byte_counting() {
        let transport = TransportLayer::new();

        // 发送不同大小的消息
        transport.send(make_message(1, 2, MessageType::Data, b"abc")).unwrap(); // 3 bytes
        transport.send(make_message(1, 2, MessageType::Data, b"de")).unwrap();  // 2 bytes
        transport.send(make_message(1, 2, MessageType::Data, b"")).unwrap();    // 0 bytes

        let stats = transport.stats();
        assert_eq!(stats.messages_sent.load(Ordering::SeqCst), 3);
        assert_eq!(stats.bytes_sent.load(Ordering::SeqCst), 5);

        // 接收两条消息
        transport.receive(); // 3 bytes
        transport.receive(); // 2 bytes

        let stats = transport.stats();
        assert_eq!(stats.messages_received.load(Ordering::SeqCst), 2);
        assert_eq!(stats.bytes_received.load(Ordering::SeqCst), 5);
    }

    /// 测试：空队列 peek 应返回 None
    #[test]
    fn test_peek_empty_queue() {
        let transport = TransportLayer::new();

        let peeked = transport.peek();
        assert!(peeked.is_none());
    }

    /// 测试：不同消息类型的传输
    #[test]
    fn test_different_message_types() {
        let transport = TransportLayer::new();

        // 发送各种类型的消息
        for msg_type in [
            MessageType::Data,
            MessageType::Control,
            MessageType::Discovery,
            MessageType::Auth,
            MessageType::Heartbeat,
            MessageType::Ack,
        ] {
            let msg = make_message(1, 2, msg_type, b"test");
            transport.send(msg).unwrap();
        }

        // 验证所有消息类型都被正确传输
        assert_eq!(transport.queue_len(), 6);

        let received = transport.receive().unwrap();
        assert_eq!(received.msg_type, MessageType::Data);

        let received = transport.receive().unwrap();
        assert_eq!(received.msg_type, MessageType::Control);

        let received = transport.receive().unwrap();
        assert_eq!(received.msg_type, MessageType::Discovery);

        let received = transport.receive().unwrap();
        assert_eq!(received.msg_type, MessageType::Auth);

        let received = transport.receive().unwrap();
        assert_eq!(received.msg_type, MessageType::Heartbeat);

        let received = transport.receive().unwrap();
        assert_eq!(received.msg_type, MessageType::Ack);
    }

    /// 测试：消息优先级和 TTL 字段保留
    #[test]
    fn test_message_fields_preserved() {
        let transport = TransportLayer::new();

        let mut msg = make_message(10, 20, MessageType::Data, b"payload");
        msg.priority = 200;
        msg.ttl = 5;

        transport.send(msg).unwrap();

        let received = transport.receive().unwrap();
        assert_eq!(received.src_device, 10);
        assert_eq!(received.dst_device, 20);
        assert_eq!(received.priority, 200);
        assert_eq!(received.ttl, 5);
        assert_eq!(received.payload, Vec::from(b"payload"));
    }

    /// 测试：错误恢复 - TTL=0 后正常消息仍可发送
    #[test]
    fn test_error_recovery_after_invalid() {
        let transport = TransportLayer::new();

        // 发送无效消息
        let mut msg = make_message(1, 2, MessageType::Data, b"bad");
        msg.ttl = 0;
        assert!(transport.send(msg).is_err());

        // 正常消息应仍可发送
        let good_msg = make_message(1, 2, MessageType::Data, b"good");
        let result = transport.send(good_msg);
        assert!(result.is_ok());

        // 验证只有正常消息在队列中
        assert_eq!(transport.queue_len(), 1);
        let received = transport.receive().unwrap();
        assert_eq!(received.payload, Vec::from(b"good"));
    }

    /// 测试：重置统计后重新计数
    #[test]
    fn test_stats_after_reset_and_resend() {
        let transport = TransportLayer::new();

        // 发送和接收
        transport.send(make_message(1, 2, MessageType::Data, b"data")).unwrap();
        transport.receive();

        // 重置
        transport.reset_stats();

        // 重新发送和接收
        transport.send(make_message(3, 4, MessageType::Control, b"ctrl")).unwrap();
        transport.send(make_message(5, 6, MessageType::Data, b"info")).unwrap();
        transport.receive();

        let stats = transport.stats();
        assert_eq!(stats.messages_sent.load(Ordering::SeqCst), 2);
        assert_eq!(stats.messages_received.load(Ordering::SeqCst), 1);
        assert_eq!(stats.bytes_sent.load(Ordering::SeqCst), 8); // "ctrl" + "info"
        assert_eq!(stats.bytes_received.load(Ordering::SeqCst), 4); // "ctrl"
        assert_eq!(stats.errors.load(Ordering::SeqCst), 0);
    }
}
