//! IPC 通道模块
//! 模仿鸿蒙零拷贝 IPC 的通道机制，支持消息头零拷贝传输和通道生命周期管理

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use spin::Mutex;

use crate::ipc::error::IpcError;

/// IPC 消息头（固定大小，零拷贝）
///
/// 实际负载通过共享内存区域传输，消息头仅包含元数据，
/// 实现零拷贝传输语义。
#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcMessageHeader {
    /// 消息唯一标识
    pub msg_id: u64,
    /// 发送者 ID
    pub src_id: u64,
    /// 接收者 ID
    pub dst_id: u64,
    /// 消息类型
    pub msg_type: u32,
    /// 标志位
    pub flags: u32,
    /// 负载大小
    pub payload_size: u32,
    /// 共享内存区域 ID（零拷贝引用）
    pub shm_region_id: u64,
    /// 时间戳
    pub timestamp: u64,
}

/// IPC 通道
/// 支持消息的发送和接收，通过共享内存实现零拷贝传输
pub struct IpcChannel {
    /// 通道唯一标识
    pub id: u64,
    /// 通道名称
    pub name: String,
    /// 通道容量
    pub capacity: usize,
    /// 消息缓冲区
    pub buffer: Mutex<VecDeque<IpcMessageHeader>>,
    /// 发送者 ID
    pub sender_id: u64,
    /// 接收者 ID
    pub receiver_id: u64,
    /// 通道是否打开
    pub is_open: AtomicBool,
    /// 已发送消息计数
    pub msg_count: AtomicU64,
}

impl Clone for IpcChannel {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            capacity: self.capacity,
            buffer: Mutex::new(VecDeque::new()), // 克隆时不复制缓冲区内容
            sender_id: self.sender_id,
            receiver_id: self.receiver_id,
            is_open: AtomicBool::new(self.is_open.load(Ordering::SeqCst)),
            msg_count: AtomicU64::new(self.msg_count.load(Ordering::SeqCst)),
        }
    }
}

impl IpcChannel {
    /// 创建新的 IPC 通道
    ///
    /// # 参数
    /// - `id`: 通道 ID
    /// - `name`: 通道名称
    /// - `capacity`: 缓冲区容量
    /// - `sender`: 发送者 ID
    /// - `receiver`: 接收者 ID
    pub fn new(id: u64, name: &str, capacity: usize, sender: u64, receiver: u64) -> Self {
        Self {
            id,
            name: String::from(name),
            capacity,
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
            sender_id: sender,
            receiver_id: receiver,
            is_open: AtomicBool::new(true),
            msg_count: AtomicU64::new(0),
        }
    }

    /// 发送消息到通道
    ///
    /// # 参数
    /// - `header`: IPC 消息头
    pub fn send(&self, header: IpcMessageHeader) -> Result<(), IpcError> {
        if !self.is_open.load(Ordering::SeqCst) {
            return Err(IpcError::ChannelClosed);
        }

        let mut buffer = self.buffer.lock();
        if buffer.len() >= self.capacity {
            return Err(IpcError::ChannelFull);
        }

        buffer.push_back(header);
        self.msg_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// 尝试从通道接收消息
    ///
    /// # 返回
    /// 通道中有消息则返回 Some，否则返回 None
    pub fn try_receive(&self) -> Option<IpcMessageHeader> {
        if !self.is_open.load(Ordering::SeqCst) {
            return None;
        }

        let mut buffer = self.buffer.lock();
        buffer.pop_front()
    }

    /// 关闭通道
    pub fn close(&self) {
        self.is_open.store(false, Ordering::SeqCst);
    }

    /// 检查通道是否打开
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    /// 获取已发送消息计数
    pub fn msg_count(&self) -> u64 {
        self.msg_count.load(Ordering::SeqCst)
    }

    /// 检查通道是否已满
    pub fn is_full(&self) -> bool {
        let buffer = self.buffer.lock();
        buffer.len() >= self.capacity
    }

    /// 检查通道是否为空
    pub fn is_empty(&self) -> bool {
        let buffer = self.buffer.lock();
        buffer.is_empty()
    }
}

/// IPC 通道管理器
/// 管理所有 IPC 通道的创建、销毁和查询
pub struct IpcChannelManager {
    /// 通道表
    channels: Mutex<BTreeMap<u64, IpcChannel>>,
    /// 下一个通道 ID
    next_id: AtomicU64,
}

impl IpcChannelManager {
    /// 创建新的通道管理器
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 创建新的 IPC 通道
    ///
    /// # 参数
    /// - `name`: 通道名称
    /// - `capacity`: 缓冲区容量
    /// - `sender`: 发送者 ID
    /// - `receiver`: 接收者 ID
    ///
    /// # 返回
    /// 成功返回通道 ID
    pub fn create_channel(
        &self,
        name: &str,
        capacity: usize,
        sender: u64,
        receiver: u64,
    ) -> Result<u64, IpcError> {
        if capacity == 0 {
            return Err(IpcError::InvalidSize(capacity));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let channel = IpcChannel::new(id, name, capacity, sender, receiver);

        let mut channels = self.channels.lock();
        channels.insert(id, channel);
        Ok(id)
    }

    /// 销毁 IPC 通道
    ///
    /// # 参数
    /// - `id`: 通道 ID
    pub fn destroy_channel(&self, id: u64) -> Result<(), IpcError> {
        let mut channels = self.channels.lock();
        if channels.remove(&id).is_some() {
            Ok(())
        } else {
            Err(IpcError::ChannelNotFound(id))
        }
    }

    /// 获取通道
    ///
    /// # 参数
    /// - `id`: 通道 ID
    pub fn get_channel(&self, id: u64) -> Option<IpcChannel> {
        let channels = self.channels.lock();
        channels.get(&id).cloned()
    }

    /// 按名称查找通道
    ///
    /// # 参数
    /// - `name`: 通道名称
    ///
    /// # 返回
    /// 找到则返回通道 ID
    pub fn find_by_name(&self, name: &str) -> Option<u64> {
        let channels = self.channels.lock();
        channels
            .values()
            .find(|c| c.name == name)
            .map(|c| c.id)
    }

    /// 列出所有通道
    pub fn list_channels(&self) -> Vec<IpcChannel> {
        let channels = self.channels.lock();
        channels.values().cloned().collect()
    }

    /// 获取通道数量
    pub fn channel_count(&self) -> usize {
        let channels = self.channels.lock();
        channels.len()
    }
}

/// 全局 IPC 通道管理器实例
pub static IPC_MANAGER: spin::Lazy<Mutex<IpcChannelManager>> = spin::Lazy::new(|| {
    Mutex::new(IpcChannelManager::new())
});

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用消息头
    fn make_header(msg_id: u64, src: u64, dst: u64) -> IpcMessageHeader {
        IpcMessageHeader {
            msg_id,
            src_id: src,
            dst_id: dst,
            msg_type: 1,
            flags: 0,
            payload_size: 64,
            shm_region_id: 0,
            timestamp: 1000,
        }
    }

    #[test]
    fn test_create_channel() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("test-ch", 10, 1, 2).unwrap();
        assert_eq!(id, 1);

        let channel = manager.get_channel(id).unwrap();
        assert_eq!(channel.name, "test-ch");
        assert_eq!(channel.capacity, 10);
        assert_eq!(channel.sender_id, 1);
        assert_eq!(channel.receiver_id, 2);
        assert!(channel.is_open());
    }

    #[test]
    fn test_destroy_channel() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("to-destroy", 10, 1, 2).unwrap();
        assert_eq!(manager.channel_count(), 1);

        assert!(manager.destroy_channel(id).is_ok());
        assert_eq!(manager.channel_count(), 0);

        // 销毁不存在的通道应返回错误
        let result = manager.destroy_channel(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_receive() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("comm", 10, 1, 2).unwrap();
        let channel = manager.get_channel(id).unwrap();

        let header = make_header(1, 1, 2);
        assert!(channel.send(header).is_ok());
        assert_eq!(channel.msg_count(), 1);

        let received = channel.try_receive();
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.msg_id, 1);
        assert_eq!(received.src_id, 1);
        assert_eq!(received.dst_id, 2);
    }

    #[test]
    fn test_channel_full() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("small", 2, 1, 2).unwrap();
        let channel = manager.get_channel(id).unwrap();

        // 填满通道
        assert!(channel.send(make_header(1, 1, 2)).is_ok());
        assert!(channel.send(make_header(2, 1, 2)).is_ok());
        assert!(channel.is_full());

        // 超出容量应返回错误
        let result = channel.send(make_header(3, 1, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_close_channel() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("close-me", 10, 1, 2).unwrap();
        let channel = manager.get_channel(id).unwrap();

        assert!(channel.is_open());
        channel.close();
        assert!(!channel.is_open());

        // 关闭后发送应失败
        let result = channel.send(make_header(1, 1, 2));
        assert!(result.is_err());

        // 关闭后接收应返回 None
        let received = channel.try_receive();
        assert!(received.is_none());
    }

    #[test]
    fn test_get_channel() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("get-test", 10, 1, 2).unwrap();

        let channel = manager.get_channel(id);
        assert!(channel.is_some());

        let not_found = manager.get_channel(999);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_find_by_name() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("unique-name", 10, 1, 2).unwrap();

        let found_id = manager.find_by_name("unique-name");
        assert!(found_id.is_some());
        assert_eq!(found_id.unwrap(), id);

        let not_found = manager.find_by_name("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_list_channels() {
        let manager = IpcChannelManager::new();
        manager.create_channel("ch1", 10, 1, 2).unwrap();
        manager.create_channel("ch2", 20, 3, 4).unwrap();
        manager.create_channel("ch3", 30, 5, 6).unwrap();

        let channels = manager.list_channels();
        assert_eq!(channels.len(), 3);
    }

    #[test]
    fn test_msg_count() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("count-test", 10, 1, 2).unwrap();
        let channel = manager.get_channel(id).unwrap();

        assert_eq!(channel.msg_count(), 0);

        channel.send(make_header(1, 1, 2)).unwrap();
        assert_eq!(channel.msg_count(), 1);

        channel.send(make_header(2, 1, 2)).unwrap();
        assert_eq!(channel.msg_count(), 2);

        // 接收消息不影响计数
        channel.try_receive();
        assert_eq!(channel.msg_count(), 2);
    }

    #[test]
    fn test_is_empty_full() {
        let manager = IpcChannelManager::new();
        let id = manager.create_channel("ef-test", 2, 1, 2).unwrap();
        let channel = manager.get_channel(id).unwrap();

        // 初始状态应为空
        assert!(channel.is_empty());
        assert!(!channel.is_full());

        // 发送一条消息
        channel.send(make_header(1, 1, 2)).unwrap();
        assert!(!channel.is_empty());
        assert!(!channel.is_full());

        // 发送第二条消息（达到容量）
        channel.send(make_header(2, 1, 2)).unwrap();
        assert!(!channel.is_empty());
        assert!(channel.is_full());

        // 接收一条消息
        channel.try_receive();
        assert!(!channel.is_empty());
        assert!(!channel.is_full());

        // 接收所有消息
        channel.try_receive();
        assert!(channel.is_empty());
        assert!(!channel.is_full());
    }
}
