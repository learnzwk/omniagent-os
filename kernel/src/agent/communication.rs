//! Agent 通信管理
//!
//! 管理 Agent 间消息路由和发布/订阅机制。
//! 提供点对点消息传递、主题广播和事件订阅功能。

use crate::syscall::abi::*;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

/// 消息队列条目
#[derive(Debug, Clone, Copy)]
pub struct MessageEntry {
    /// 消息头
    pub header: AgentMsgHeader,
    /// 发送者句柄
    pub src_handle: AgentHandle,
    /// 接收者句柄
    pub dst_handle: AgentHandle,
}

/// Agent 收件箱
///
/// 固定大小的环形缓冲区，存储待处理的消息。
pub struct AgentMailbox {
    /// 消息数组
    messages: [Option<MessageEntry>; MAILBOX_CAPACITY],
    /// 队列头 (读取位置)
    head: usize,
    /// 队列尾 (写入位置)
    tail: usize,
    /// 当前消息数量
    count: usize,
}

impl AgentMailbox {
    /// 创建空的收件箱
    pub fn new() -> Self {
        AgentMailbox {
            messages: [None; MAILBOX_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// 推入消息
    ///
    /// 如果邮箱已满，返回 EAGENT_QUEUE_FULL 错误。
    pub fn push(&mut self, entry: MessageEntry) -> Result<(), SyscallError> {
        if self.count >= MAILBOX_CAPACITY {
            return Err(SyscallError::EAGENT_QUEUE_FULL);
        }
        self.messages[self.tail] = Some(entry);
        self.tail = (self.tail + 1) % MAILBOX_CAPACITY;
        self.count += 1;
        Ok(())
    }

    /// 弹出消息
    ///
    /// 如果邮箱为空，返回 None。
    pub fn pop(&mut self) -> Option<MessageEntry> {
        if self.count == 0 {
            return None;
        }
        let entry = self.messages[self.head].take();
        self.head = (self.head + 1) % MAILBOX_CAPACITY;
        self.count -= 1;
        entry
    }

    /// 检查邮箱是否为空
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// 获取当前消息数量
    pub fn len(&self) -> usize {
        self.count
    }

    /// 检查邮箱是否已满
    pub fn is_full(&self) -> bool {
        self.count >= MAILBOX_CAPACITY
    }
}

/// 主题订阅记录
#[derive(Debug, Clone, Copy)]
pub struct Subscription {
    /// 订阅者句柄
    pub subscriber: AgentHandle,
    /// 订阅主题 (UTF-8, 最大 64 字节含 NUL)
    pub topic: [u8; 64],
    /// 事件掩码
    pub event_mask: EventMask,
}

impl PartialEq for Subscription {
    fn eq(&self, other: &Self) -> bool {
        self.subscriber == other.subscriber && self.topic == other.topic
    }
}

impl Eq for Subscription {}

/// 最大订阅数量
#[cfg(not(test))]
const MAX_SUBSCRIPTIONS: usize = 1024;
#[cfg(test)]
const MAX_SUBSCRIPTIONS: usize = 64;

/// 最大邮箱数量
#[cfg(not(test))]
const MAX_MAILBOXES: usize = 4096;
#[cfg(test)]
const MAX_MAILBOXES: usize = 64;

/// 每个邮箱最大消息数
#[cfg(not(test))]
const MAILBOX_CAPACITY: usize = 256;
#[cfg(test)]
const MAILBOX_CAPACITY: usize = 32;

/// 通信管理器
///
/// 管理所有 Agent 的邮箱和主题订阅。
pub struct CommManager {
    /// Agent 邮箱数组 (按 handle index 索引)
    mailboxes: Mutex<[AgentMailbox; MAX_MAILBOXES]>,
    /// 主题订阅数组
    subscriptions: Mutex<[Option<Subscription>; MAX_SUBSCRIPTIONS]>,
    /// 全局消息计数器
    msg_counter: AtomicU64,
}

impl CommManager {
    /// 创建新的通信管理器
    ///
    /// 使用 unsafe 进行零初始化，因为 Rust 1.75 不支持 const 中的数组 map。
    /// 安全性: [AgentMailbox; MAX_MAILBOXES] 全零等价于空邮箱。
    pub const fn new() -> Self {
        CommManager {
            mailboxes: Mutex::new(unsafe { core::mem::zeroed() }),
            subscriptions: Mutex::new([None; MAX_SUBSCRIPTIONS]),
            msg_counter: AtomicU64::new(1),
        }
    }

    /// 发送消息
    ///
    /// 从 src 向 dst 发送消息，消息被放入目标 Agent 的邮箱。
    /// 返回分配的消息 ID。
    pub fn send_message(
        &self,
        src: AgentHandle,
        dst: AgentHandle,
        header: &AgentMsgHeader,
    ) -> Result<u64, SyscallError> {
        if !dst.is_valid() {
            return Err(SyscallError::EINVAL);
        }

        let idx = dst.index();
        if idx >= MAX_MAILBOXES {
            return Err(SyscallError::ESRCH);
        }

        // 分配消息 ID
        let msg_id = self.msg_counter.fetch_add(1, Ordering::Relaxed);

        // 构造消息条目
        let mut entry_header = *header;
        entry_header.msg_id = msg_id;

        let entry = MessageEntry {
            header: entry_header,
            src_handle: src,
            dst_handle: dst,
        };

        // 推入目标邮箱
        let mut mailboxes = self.mailboxes.lock();
        mailboxes[idx]
            .push(entry)
            .map_err(|_| SyscallError::EAGENT_QUEUE_FULL)?;

        Ok(msg_id)
    }

    /// 广播消息
    ///
    /// 向所有订阅指定主题的 Agent 发送消息。
    /// 返回实际接收到消息的 Agent 数量。
    pub fn broadcast(
        &self,
        src: AgentHandle,
        topic: &[u8],
        header: &AgentMsgHeader,
    ) -> Result<u32, SyscallError> {
        // 分配消息 ID
        let msg_id = self.msg_counter.fetch_add(1, Ordering::Relaxed);

        let mut entry_header = *header;
        entry_header.msg_id = msg_id;

        let subscriptions = self.subscriptions.lock();
        let mut mailboxes = self.mailboxes.lock();

        let mut delivered = 0u32;
        for sub_opt in subscriptions.iter() {
            if let Some(sub) = sub_opt {
                // 跳过发送者自己
                if sub.subscriber == src {
                    continue;
                }
                // 检查主题匹配
                if topic_matches(&sub.topic, topic) {
                    let entry = MessageEntry {
                        header: entry_header,
                        src_handle: src,
                        dst_handle: sub.subscriber,
                    };

                    let idx = sub.subscriber.index();
                    if idx < MAX_MAILBOXES {
                        if mailboxes[idx].push(entry).is_ok() {
                            delivered += 1;
                        }
                    }
                }
            }
        }

        Ok(delivered)
    }

    /// 接收消息
    ///
    /// 从指定 Agent 的邮箱中取出一条消息。
    /// 如果邮箱为空，返回 None。
    pub fn receive(&self, handle: AgentHandle) -> Option<MessageEntry> {
        if !handle.is_valid() {
            return None;
        }
        let idx = handle.index();
        if idx >= MAX_MAILBOXES {
            return None;
        }
        let mut mailboxes = self.mailboxes.lock();
        mailboxes[idx].pop()
    }

    /// 订阅主题
    ///
    /// 为指定 Agent 注册主题订阅。
    pub fn subscribe(
        &self,
        subscriber: AgentHandle,
        topic: &[u8],
        mask: &EventMask,
    ) -> Result<(), SyscallError> {
        if !subscriber.is_valid() {
            return Err(SyscallError::EINVAL);
        }

        // 构造主题数组
        let mut topic_arr = [0u8; 64];
        let len = topic.len().min(63);
        topic_arr[..len].copy_from_slice(&topic[..len]);

        let sub = Subscription {
            subscriber,
            topic: topic_arr,
            event_mask: *mask,
        };

        let mut subscriptions = self.subscriptions.lock();

        // 检查是否已订阅同一主题
        for existing in subscriptions.iter() {
            if let Some(e) = existing {
                if e == &sub {
                    return Ok(()); // 已订阅，幂等操作
                }
            }
        }

        // 查找空闲槽位
        for slot in subscriptions.iter_mut() {
            if slot.is_none() {
                *slot = Some(sub);
                return Ok(());
            }
        }

        Err(SyscallError::EAGAIN)
    }

    /// 取消订阅
    ///
    /// 移除指定 Agent 对指定主题的订阅。
    pub fn unsubscribe(
        &self,
        subscriber: AgentHandle,
        topic: &[u8],
    ) -> Result<(), SyscallError> {
        if !subscriber.is_valid() {
            return Err(SyscallError::EINVAL);
        }

        let mut topic_arr = [0u8; 64];
        let len = topic.len().min(63);
        topic_arr[..len].copy_from_slice(&topic[..len]);

        let mut subscriptions = self.subscriptions.lock();

        for slot in subscriptions.iter_mut() {
            if let Some(sub) = slot {
                if sub.subscriber == subscriber && topic_matches(&sub.topic, topic) {
                    *slot = None;
                    return Ok(());
                }
            }
        }

        Err(SyscallError::ENOENT)
    }

    /// 获取指定 Agent 邮箱中的消息数量
    pub fn mailbox_len(&self, handle: AgentHandle) -> usize {
        if !handle.is_valid() {
            return 0;
        }
        let idx = handle.index();
        if idx >= MAX_MAILBOXES {
            return 0;
        }
        let mailboxes = self.mailboxes.lock();
        mailboxes[idx].len()
    }

    /// 检查指定 Agent 的邮箱是否为空
    pub fn mailbox_is_empty(&self, handle: AgentHandle) -> bool {
        if !handle.is_valid() {
            return true;
        }
        let idx = handle.index();
        if idx >= MAX_MAILBOXES {
            return true;
        }
        let mailboxes = self.mailboxes.lock();
        mailboxes[idx].is_empty()
    }
}

/// 全局通信管理器实例
static COMM_MANAGER: spin::Lazy<Mutex<CommManager>> = spin::Lazy::new(|| {
    Mutex::new(CommManager::new())
});

/// 初始化全局通信管理器
///
/// 创建通信管理器实例，准备邮箱和订阅表。
/// 在内核启动的子系统初始化阶段调用。
pub fn init() {
    // 通过 Lazy 的首次访问自动创建实例
    // 此函数确保在启动阶段显式初始化
    let _comm = COMM_MANAGER.lock();
}

/// 获取全局通信管理器的引用
///
/// 返回全局通信管理器的锁守卫，用于执行消息路由操作。
pub fn global_comm_manager() -> &'static spin::Lazy<Mutex<CommManager>> {
    &COMM_MANAGER
}

/// 检查主题是否匹配
///
/// 支持精确匹配和前缀匹配 (以 '*' 结尾的主题)。
fn topic_matches(pattern: &[u8; 64], topic: &[u8]) -> bool {
    // 找到 pattern 中 NUL 终止符的位置
    let pattern_len = pattern.iter().position(|&b| b == 0).unwrap_or(64);

    // 前缀匹配: pattern 以 '*' 结尾 (如 "agent.*")
    if pattern_len > 0 && pattern[pattern_len - 1] == b'*' {
        let prefix = &pattern[..pattern_len - 1];
        return topic.len() >= prefix.len() && &topic[..prefix.len()] == prefix;
    }

    // 精确匹配
    if pattern_len == topic.len() {
        return &pattern[..pattern_len] == topic;
    }

    false
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mailbox_push_pop() {
        let mut mailbox = AgentMailbox::new();

        let header = AgentMsgHeader::default();
        let entry = MessageEntry {
            header,
            src_handle: AgentHandle(1),
            dst_handle: AgentHandle(2),
        };

        // 推入消息
        assert!(mailbox.push(entry).is_ok());
        assert_eq!(mailbox.len(), 1);
        assert!(!mailbox.is_empty());

        // 弹出消息
        let popped = mailbox.pop();
        assert!(popped.is_some());
        let popped = popped.unwrap();
        assert_eq!(popped.src_handle, AgentHandle(1));
        assert_eq!(popped.dst_handle, AgentHandle(2));
        assert_eq!(mailbox.len(), 0);
        assert!(mailbox.is_empty());
    }

    #[test]
    fn test_mailbox_fifo_order() {
        let mut mailbox = AgentMailbox::new();

        // 推入多条消息
        for i in 0..10u64 {
            let header = AgentMsgHeader {
                msg_id: i,
                ..AgentMsgHeader::default()
            };
            let entry = MessageEntry {
                header,
                src_handle: AgentHandle(i + 1),
                dst_handle: AgentHandle(100),
            };
            mailbox.push(entry).unwrap();
        }

        // 验证 FIFO 顺序
        for i in 0..10u64 {
            let popped = mailbox.pop().unwrap();
            assert_eq!(popped.header.msg_id, i);
            assert_eq!(popped.src_handle, AgentHandle(i + 1));
        }

        assert!(mailbox.is_empty());
    }

    #[test]
    fn test_mailbox_full() {
        let mut mailbox = AgentMailbox::new();

        // 填满邮箱
        for i in 0..MAILBOX_CAPACITY as u64 {
            let entry = MessageEntry {
                header: AgentMsgHeader {
                    msg_id: i,
                    ..AgentMsgHeader::default()
                },
                src_handle: AgentHandle(1),
                dst_handle: AgentHandle(2),
            };
            assert!(mailbox.push(entry).is_ok(), "推入消息 {} 失败", i);
        }

        assert!(mailbox.is_full());
        assert_eq!(mailbox.len(), MAILBOX_CAPACITY);

        // 再推入应该失败
        let entry = MessageEntry {
            header: AgentMsgHeader::default(),
            src_handle: AgentHandle(1),
            dst_handle: AgentHandle(2),
        };
        assert_eq!(mailbox.push(entry), Err(SyscallError::EAGENT_QUEUE_FULL));
    }

    #[test]
    fn test_mailbox_empty() {
        let mut mailbox = AgentMailbox::new();

        assert!(mailbox.is_empty());
        assert_eq!(mailbox.len(), 0);

        // 弹出空邮箱
        assert!(mailbox.pop().is_none());
    }

    #[test]
    fn test_comm_send_message() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let dst = AgentHandle(2);

        let header = AgentMsgHeader {
            msg_type: 1,
            payload_size: 128,
            ..AgentMsgHeader::default()
        };

        // 发送消息
        let msg_id = comm.send_message(src, dst, &header).unwrap();
        assert!(msg_id > 0);

        // 接收消息
        let entry = comm.receive(dst).unwrap();
        assert_eq!(entry.src_handle, src);
        assert_eq!(entry.dst_handle, dst);
        assert_eq!(entry.header.msg_id, msg_id);
        assert_eq!(entry.header.msg_type, 1);
        assert_eq!(entry.header.payload_size, 128);
    }

    #[test]
    fn test_comm_send_multiple_messages() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let dst = AgentHandle(2);

        // 发送多条消息 (msg_id 由内核自动分配)
        let mut last_msg_id = 0;
        for _ in 0..5 {
            let header = AgentMsgHeader::default();
            let msg_id = comm.send_message(src, dst, &header).unwrap();
            assert!(msg_id > last_msg_id);
            last_msg_id = msg_id;
        }

        assert_eq!(comm.mailbox_len(dst), 5);

        // 按顺序接收
        for _ in 0..5 {
            let entry = comm.receive(dst).unwrap();
            assert_eq!(entry.src_handle, src);
            assert_eq!(entry.dst_handle, dst);
        }

        assert!(comm.mailbox_is_empty(dst));
    }

    #[test]
    fn test_comm_send_nonexistent() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let dst = AgentHandle::INVALID;

        let header = AgentMsgHeader::default();

        // 发送给无效句柄
        let result = comm.send_message(src, dst, &header);
        assert_eq!(result, Err(SyscallError::EINVAL));
    }

    #[test]
    fn test_comm_send_out_of_range() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let dst = AgentHandle(99999); // 超出邮箱数组范围

        let header = AgentMsgHeader::default();

        let result = comm.send_message(src, dst, &header);
        assert_eq!(result, Err(SyscallError::ESRCH));
    }

    #[test]
    fn test_comm_broadcast() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let subscriber1 = AgentHandle(2);
        let subscriber2 = AgentHandle(3);
        let subscriber3 = AgentHandle(4);

        // 订阅主题
        comm.subscribe(subscriber1, b"events", &EventMask::ALL).unwrap();
        comm.subscribe(subscriber2, b"events", &EventMask::ALL).unwrap();
        comm.subscribe(subscriber3, b"other", &EventMask::ALL).unwrap();

        // 广播消息
        let header = AgentMsgHeader {
            msg_type: 1,
            ..AgentMsgHeader::default()
        };
        let delivered = comm.broadcast(src, b"events", &header).unwrap();

        // 应该只投递给订阅了 "events" 的 Agent (不包括发送者)
        assert_eq!(delivered, 2);

        // 验证 subscriber1 收到消息
        let entry = comm.receive(subscriber1);
        assert!(entry.is_some());

        // 验证 subscriber2 收到消息
        let entry = comm.receive(subscriber2);
        assert!(entry.is_some());

        // 验证 subscriber3 没有收到消息 (订阅的是 "other")
        let entry = comm.receive(subscriber3);
        assert!(entry.is_none());
    }

    #[test]
    fn test_comm_broadcast_excludes_sender() {
        let comm = CommManager::new();
        let src = AgentHandle(1);

        // 发送者也订阅了主题
        comm.subscribe(src, b"topic", &EventMask::ALL).unwrap();

        let header = AgentMsgHeader::default();
        let delivered = comm.broadcast(src, b"topic", &header).unwrap();

        // 发送者不应收到自己的广播
        assert_eq!(delivered, 0);
        assert!(comm.mailbox_is_empty(src));
    }

    #[test]
    fn test_comm_subscribe_unsubscribe() {
        let comm = CommManager::new();
        let subscriber = AgentHandle(1);

        // 订阅
        assert!(comm.subscribe(subscriber, b"test.topic", &EventMask::ALL).is_ok());

        // 重复订阅 (幂等)
        assert!(comm.subscribe(subscriber, b"test.topic", &EventMask::ALL).is_ok());

        // 取消订阅
        assert!(comm.unsubscribe(subscriber, b"test.topic").is_ok());

        // 再次取消订阅 (应失败)
        assert_eq!(
            comm.unsubscribe(subscriber, b"test.topic"),
            Err(SyscallError::ENOENT)
        );
    }

    #[test]
    fn test_comm_subscribe_invalid_handle() {
        let comm = CommManager::new();

        let result = comm.subscribe(AgentHandle::INVALID, b"topic", &EventMask::ALL);
        assert_eq!(result, Err(SyscallError::EINVAL));
    }

    #[test]
    fn test_comm_unsubscribe_invalid_handle() {
        let comm = CommManager::new();

        let result = comm.unsubscribe(AgentHandle::INVALID, b"topic");
        assert_eq!(result, Err(SyscallError::EINVAL));
    }

    #[test]
    fn test_comm_max_subscriptions() {
        let comm = CommManager::new();

        // 填满订阅表
        for i in 0..MAX_SUBSCRIPTIONS {
            let subscriber = AgentHandle((i + 1) as u64);
            let topic = format!("topic_{:04}", i);
            let result = comm.subscribe(subscriber, topic.as_bytes(), &EventMask::ALL);
            assert!(result.is_ok(), "订阅 {} 失败", i);
        }

        // 再订阅应该失败
        let result = comm.subscribe(
            AgentHandle(MAX_SUBSCRIPTIONS as u64 + 1),
            b"overflow",
            &EventMask::ALL,
        );
        assert_eq!(result, Err(SyscallError::EAGAIN));
    }

    #[test]
    fn test_comm_receive_empty() {
        let comm = CommManager::new();
        let handle = AgentHandle(1);

        // 空邮箱
        assert!(comm.receive(handle).is_none());
        assert!(comm.mailbox_is_empty(handle));
        assert_eq!(comm.mailbox_len(handle), 0);
    }

    #[test]
    fn test_comm_receive_invalid_handle() {
        let comm = CommManager::new();

        assert!(comm.receive(AgentHandle::INVALID).is_none());
        assert!(comm.receive(AgentHandle(99999)).is_none());
    }

    #[test]
    fn test_topic_matches_exact() {
        let mut pattern = [0u8; 64];
        pattern[..5].copy_from_slice(b"hello");

        assert!(topic_matches(&pattern, b"hello"));
        assert!(!topic_matches(&pattern, b"world"));
        assert!(!topic_matches(&pattern, b"hellox"));
        assert!(!topic_matches(&pattern, b"hell"));
    }

    #[test]
    fn test_topic_matches_wildcard() {
        let mut pattern = [0u8; 64];
        pattern[..7].copy_from_slice(b"agent.*");

        assert!(topic_matches(&pattern, b"agent.1"));
        assert!(topic_matches(&pattern, b"agent.abc"));
        assert!(topic_matches(&pattern, b"agent."));
        assert!(!topic_matches(&pattern, b"agent"));
        assert!(!topic_matches(&pattern, b"other.1"));
    }

    #[test]
    fn test_comm_broadcast_wildcard_topic() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let sub1 = AgentHandle(2);
        let sub2 = AgentHandle(3);

        // 订阅通配符主题
        comm.subscribe(sub1, b"agent.*", &EventMask::ALL).unwrap();
        comm.subscribe(sub2, b"agent.create", &EventMask::ALL).unwrap();

        // 广播到 "agent.create" (sub1 通配符匹配, sub2 精确匹配)
        let header = AgentMsgHeader::default();
        let delivered = comm.broadcast(src, b"agent.create", &header).unwrap();
        assert_eq!(delivered, 2);

        // 广播到 "agent.delete" (只有 sub1 通配符匹配)
        let delivered = comm.broadcast(src, b"agent.delete", &header).unwrap();
        assert_eq!(delivered, 1);
    }

    #[test]
    fn test_comm_mailbox_len() {
        let comm = CommManager::new();
        let src = AgentHandle(1);
        let dst = AgentHandle(2);

        assert_eq!(comm.mailbox_len(dst), 0);

        let header = AgentMsgHeader::default();
        comm.send_message(src, dst, &header).unwrap();
        assert_eq!(comm.mailbox_len(dst), 1);

        comm.send_message(src, dst, &header).unwrap();
        assert_eq!(comm.mailbox_len(dst), 2);

        comm.receive(dst).unwrap();
        assert_eq!(comm.mailbox_len(dst), 1);
    }

    /// 测试：模块级 init 函数
    #[test]
    fn test_comm_init() {
        // 调用模块级 init 函数
        init();

        // 验证全局通信管理器可访问且功能正常
        let comm = COMM_MANAGER.lock();

        // 验证空邮箱
        assert!(comm.mailbox_is_empty(AgentHandle(1)));
        assert_eq!(comm.mailbox_len(AgentHandle(1)), 0);

        // 验证可以发送消息
        let src = AgentHandle(1);
        let dst = AgentHandle(2);
        let header = AgentMsgHeader::default();
        assert!(comm.send_message(src, dst, &header).is_ok());
        assert_eq!(comm.mailbox_len(dst), 1);
    }
}
