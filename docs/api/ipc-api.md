# IPC API 参考

> **模块名称**: `omniagent-ipc`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 定位

IPC（进程间通信）API 是 OmniAgent OS 微内核架构的核心通信接口。在微内核设计中，所有用户态服务（文件系统、设备驱动、网络协议栈、AI 推理等）均以独立任务运行，彼此之间通过 IPC 进行协作。本 API 提供高效的、类型安全的、支持零拷贝的进程间通信能力，特别针对 AI Agent 之间的高频低延迟通信场景进行了深度优化。

### 1.2 设计目标

| 目标 | 描述 | 优先级 |
|------|------|--------|
| **低延迟** | 同核同步 IPC 延迟 < 500ns | P0 |
| **零拷贝** | 大块数据传输零拷贝，吞吐 > 10 GB/s | P0 |
| **安全** | Capability-based 端口访问控制 | P0 |
| **可靠性** | 消息不丢失、不重复，FIFO 排序保证 | P1 |
| **流控** | 基于信用的背压机制防止内存溢出 | P1 |
| **可扩展** | 支持多服务拓扑路由和广播/多播 | P1 |
| **简洁** | 固定 64 字节消息头，bincode 序列化 | P2 |

### 1.3 架构概览

```
┌──────────────────────────────────────────────────────────────┐
│                    IPC API 层                                │
├──────────┬──────────┬──────────┬────────────────────────────┤
│  通道    │  端口    │  消息    │  流控 / 广播               │
│  Channel │  Port    │  Message │  Flow / Broadcast          │
├──────────┴──────────┴──────────┴────────────────────────────┤
│              IPC Manager (内核级)                            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │ 通道管理 │ │ 端口管理 │ │ 序列化   │ │ 共享内存管理  │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

---

## 2. 通道 API

### 2.1 Channel Trait 定义

`Channel` 是所有 IPC 通道类型的统一抽象接口。三种实现分别适用于不同的通信场景。

```rust
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, AtomicI32, Ordering};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use serde::{Serialize, Deserialize};

/// 通道 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub u64);

/// 通道类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChannelType {
    /// 同步通道：发送方阻塞直到接收方接收
    Synchronous = 0,
    /// 异步通道：发送方将消息放入缓冲区后立即返回
    Asynchronous = 1,
    /// 共享内存通道：双方通过共享内存区域直接交换数据
    SharedMemory = 2,
}

/// 通道统计信息
#[derive(Debug, Clone, Default)]
pub struct ChannelStats {
    /// 已发送消息数
    pub messages_sent: u64,
    /// 已接收消息数
    pub messages_received: u64,
    /// 已发送字节数
    pub bytes_sent: u64,
    /// 已接收字节数
    pub bytes_received: u64,
    /// 当前队列深度
    pub current_depth: u32,
    /// 峰值队列深度
    pub peak_depth: u32,
    /// 发送超时次数
    pub send_timeouts: u64,
    /// 接收超时次数
    pub recv_timeouts: u64,
}

/// 通道配置
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// 通道类型
    pub channel_type: ChannelType,
    /// 通道容量（异步通道的消息缓冲区大小）
    pub capacity: usize,
    /// 最大消息大小（字节）
    pub max_message_size: usize,
    /// 是否启用流量控制
    pub flow_control: bool,
    /// 信用值（异步通道的初始信用）
    pub initial_credits: u32,
    /// 优先级 (0-255)
    pub priority: u8,
    /// 超时时间，None 表示无限等待
    pub timeout: Option<Duration>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            channel_type: ChannelType::Asynchronous,
            capacity: 256,
            max_message_size: 4096,
            flow_control: true,
            initial_credits: 64,
            priority: 0,
            timeout: None,
        }
    }
}

/// 通道 Trait — 所有通道类型必须实现此接口
pub trait Channel: Send + Sync {
    /// 获取通道 ID
    fn id(&self) -> ChannelId;

    /// 获取通道类型
    fn channel_type(&self) -> ChannelType;

    /// 发送消息
    ///
    /// # 参数
    /// - `msg`: 要发送的消息
    /// - `timeout`: 超时时间，None 表示无限等待
    ///
    /// # 错误
    /// - `IpcError::ChannelFull`: 通道已满
    /// - `IpcError::PeerDead`: 对端已关闭
    /// - `IpcError::Timeout`: 发送超时
    fn send(&self, msg: Message, timeout: Option<Duration>) -> IpcResult<()>;

    /// 接收消息
    ///
    /// # 参数
    /// - `timeout`: 超时时间，None 表示无限等待
    ///
    /// # 错误
    /// - `IpcError::ChannelEmpty`: 通道为空
    /// - `IpcError::PeerDead`: 对端已关闭
    /// - `IpcError::Timeout`: 接收超时
    fn receive(&self, timeout: Option<Duration>) -> IpcResult<Message>;

    /// 关闭通道
    fn close(&self) -> IpcResult<()>;

    /// 检查通道是否已关闭
    fn is_closed(&self) -> bool;

    /// 获取通道统计信息
    fn stats(&self) -> ChannelStats;
}
```

### 2.2 SyncChannel — 同步 RPC 通道

同步通道不使用缓冲区，发送方直接将消息复制到接收方的地址空间。适用于请求-响应模式的服务调用场景。

```rust
/// 同步通道最大消息大小
pub const SYNC_MAX_MSG_SIZE: usize = 64 * 1024; // 64KB

/// 同步 RPC 通道
///
/// 发送方阻塞直到接收方接收消息，适用于低延迟请求-响应模式。
pub struct SyncChannel {
    /// 通道 ID
    id: ChannelId,
    /// 发送端等待队列
    sender_waiters: SpinLock<VecDeque<TaskId>>,
    /// 接收端等待队列
    receiver_waiters: SpinLock<VecDeque<TaskId>>,
    /// 发送端是否已关闭
    sender_closed: AtomicBool,
    /// 接收端是否已关闭
    receiver_closed: AtomicBool,
    /// 统计信息
    stats: SpinLock<ChannelStats>,
}

impl SyncChannel {
    /// 创建同步通道对
    ///
    /// 返回 (sender, receiver) 两端。
    pub fn create_pair() -> (SyncChannelSender, SyncChannelReceiver) {
        let id = ChannelId(next_channel_id());
        let shared = std::sync::Arc::new(SyncChannelCore {
            id,
            sender_waiters: SpinLock::new(VecDeque::new()),
            receiver_waiters: SpinLock::new(VecDeque::new()),
            sender_closed: AtomicBool::new(false),
            receiver_closed: AtomicBool::new(false),
            stats: SpinLock::new(ChannelStats::default()),
        });
        (
            SyncChannelSender { core: shared.clone() },
            SyncChannelReceiver { core: shared },
        )
    }

    /// 同步 RPC 调用：发送请求并等待响应
    ///
    /// 自动分配事务 ID 并匹配响应消息。
    pub fn call(&self, request: &Message, timeout: Duration) -> IpcResult<Message> {
        let tx_id = self.next_tx_id();
        let mut msg = request.clone();
        msg.header.flags |= MessageFlags::REQUEST;
        msg.header.tx_id = tx_id;

        self.send(msg, Some(timeout))?;

        loop {
            let response = self.receive(Some(timeout))?;
            if response.header.tx_id == tx_id {
                if response.header.flags.contains(MessageFlags::ERROR) {
                    return Err(IpcError::RemoteError(response.header.msg_type));
                }
                return Ok(response);
            }
        }
    }
}

/// 同步通道发送端
pub struct SyncChannelSender {
    core: std::sync::Arc<SyncChannelCore>,
}

/// 同步通道接收端
pub struct SyncChannelReceiver {
    core: std::sync::Arc<SyncChannelCore>,
}

/// 同步通道核心（发送端和接收端共享）
struct SyncChannelCore {
    id: ChannelId,
    sender_waiters: SpinLock<VecDeque<TaskId>>,
    receiver_waiters: SpinLock<VecDeque<TaskId>>,
    sender_closed: AtomicBool,
    receiver_closed: AtomicBool,
    stats: SpinLock<ChannelStats>,
}
```

### 2.3 AsyncChannel — 异步消息队列

异步通道使用环形缓冲区存储消息，发送方将消息放入缓冲区后立即返回。适用于事件通知、日志记录等生产者-消费者模式。

```rust
/// 异步通道
///
/// 使用环形缓冲区存储消息，支持流量控制（基于信用）。
/// 当缓冲区满时，根据流量控制策略决定是阻塞还是返回错误。
pub struct AsyncChannel {
    /// 通道 ID
    id: ChannelId,
    /// 消息环形缓冲区
    buffer: SpinLock<RingBuffer<Message>>,
    /// 流量控制器
    flow_control: SpinLock<FlowController>,
    /// 通道是否已关闭
    closed: AtomicBool,
    /// 统计信息
    stats: SpinLock<ChannelStats>,
}

impl AsyncChannel {
    /// 创建异步通道
    pub fn new(config: &ChannelConfig) -> Self {
        let capacity = config.capacity.max(1);
        Self {
            id: ChannelId(next_channel_id()),
            buffer: SpinLock::new(RingBuffer::new(capacity)),
            flow_control: SpinLock::new(FlowController::new(
                config.initial_credits,
                config.capacity as u32,
            )),
            closed: AtomicBool::new(false),
            stats: SpinLock::new(ChannelStats::default()),
        }
    }

    /// 非阻塞发送（立即返回，不等待）
    pub fn try_send(&self, msg: Message) -> IpcResult<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }

        // 检查信用
        {
            let fc = self.flow_control.lock();
            if !fc.consume_credit() {
                return Err(IpcError::ChannelFull);
            }
        }

        // 放入缓冲区
        {
            let mut buf = self.buffer.lock();
            buf.push(msg).map_err(|_| IpcError::ChannelFull)?;
        }

        // 更新统计
        {
            let mut stats = self.stats.lock();
            stats.messages_sent += 1;
        }

        // 唤醒等待的接收方
        wake_receivers(self.id);
        Ok(())
    }

    /// 非阻塞接收（立即返回，不等待）
    pub fn try_receive(&self) -> IpcResult<Message> {
        if self.closed.load(Ordering::Acquire) {
            let buf = self.buffer.lock();
            if buf.is_empty() {
                return Err(IpcError::ChannelClosed);
            }
        }

        let msg = {
            let mut buf = self.buffer.lock();
            buf.pop().ok_or(IpcError::ChannelEmpty)?
        };

        // 归还信用
        {
            let mut fc = self.flow_control.lock();
            fc.return_credit();
            fc.wake_sender_if_needed();
        }

        // 更新统计
        {
            let mut stats = self.stats.lock();
            stats.messages_received += 1;
        }

        Ok(msg)
    }
}

impl Channel for AsyncChannel {
    fn id(&self) -> ChannelId { self.id }
    fn channel_type(&self) -> ChannelType { ChannelType::Asynchronous }

    fn send(&self, msg: Message, timeout: Option<Duration>) -> IpcResult<()> {
        match self.try_send(msg) {
            Ok(()) => Ok(()),
            Err(IpcError::ChannelFull) => {
                let timeout_ns = timeout
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                let fc = self.flow_control.lock();
                fc.wait_for_credit(timeout_ns)?;
                self.try_send(msg) // 重试
            }
            Err(e) => Err(e),
        }
    }

    fn receive(&self, timeout: Option<Duration>) -> IpcResult<Message> {
        match self.try_receive() {
            Ok(msg) => Ok(msg),
            Err(IpcError::ChannelEmpty) => {
                let timeout_ns = timeout
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                block_current_task_with_timeout(self.id, timeout_ns)?;
                self.try_receive() // 重试
            }
            Err(e) => Err(e),
        }
    }

    fn close(&self) -> IpcResult<()> {
        self.closed.store(true, Ordering::Release);
        wake_all_waiters(self.id);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn stats(&self) -> ChannelStats {
        self.stats.lock().clone()
    }
}
```

### 2.4 SharedMemoryChannel — 零拷贝共享内存通道

共享内存通道用于大块数据（如图像帧、音频缓冲区、AI 模型数据）的零拷贝传输。双方通过映射同一物理内存区域直接交换数据，无需内核态数据拷贝。

```rust
/// 共享内存通道最大大小
pub const SHM_MAX_SIZE: usize = 1024 * 1024 * 1024; // 1GB

/// 共享内存区域描述符
#[derive(Debug, Clone)]
pub struct SharedMemoryRegion {
    /// 共享内存 ID
    pub id: SharedMemId,
    /// 共享内存大小（字节）
    pub size: usize,
    /// 发送方虚拟地址
    pub sender_vaddr: u64,
    /// 接收方虚拟地址
    pub receiver_vaddr: u64,
    /// 权限标志
    pub permissions: ShmFlags,
}

/// 共享内存 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SharedMemId(pub u64);

/// 共享内存权限标志
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShmFlags: u32 {
        const READ       = 0x01;
        const WRITE      = 0x02;
        const EXECUTE    = 0x04;
        const CACHED     = 0x08;
        const UNCACHED   = 0x10;
    }
}

/// 共享内存环形缓冲区头部（位于共享内存起始位置）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShmRingHeader {
    /// 写偏移
    pub write_offset: AtomicU64,
    /// 读偏移
    pub read_offset: AtomicU64,
    /// 缓冲区大小
    pub buffer_size: u64,
    /// 数据起始偏移（头部之后）
    pub data_offset: u64,
    /// 已关闭标志
    pub closed: AtomicBool,
}

/// 共享内存通道
pub struct SharedMemoryChannel {
    /// 通道 ID
    id: ChannelId,
    /// 共享内存区域
    region: SharedMemoryRegion,
    /// 环形缓冲区头部指针
    ring_header: *mut ShmRingHeader,
    /// 发送端信号量
    send_sem: AtomicU32,
    /// 接收端信号量
    recv_sem: AtomicU32,
    /// 通道是否已关闭
    closed: AtomicBool,
}

impl SharedMemoryChannel {
    /// 创建共享内存通道
    pub fn new(size: usize) -> IpcResult<Self> {
        if size == 0 || size > SHM_MAX_SIZE {
            return Err(IpcError::MessageTooLarge {
                size,
                max: SHM_MAX_SIZE,
            });
        }

        let region = allocate_shared_memory(size)?;
        let header_size = std::mem::size_of::<ShmRingHeader>();
        let buffer_size = size - header_size;

        Ok(Self {
            id: ChannelId(next_channel_id()),
            region,
            ring_header: region.sender_vaddr as *mut ShmRingHeader,
            send_sem: AtomicU32::new(0),
            recv_sem: AtomicU32::new(0),
            closed: AtomicBool::new(false),
        })
    }

    /// 零拷贝发送数据
    ///
    /// 数据直接写入共享内存区域，接收方通过映射读取，
    /// 无需内核态数据拷贝。
    pub fn send_zero_copy(&self, data: &[u8]) -> IpcResult<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }

        let header = unsafe { &*self.ring_header };
        let write_pos = header.write_offset.load(Ordering::Acquire);
        let read_pos = header.read_offset.load(Ordering::Acquire);
        let buffer_size = header.buffer_size as usize;
        let data_start = header.data_offset as usize;

        // 计算可用空间
        let available = if write_pos >= read_pos {
            buffer_size - (write_pos as usize - read_pos as usize) - 1
        } else {
            read_pos as usize - write_pos as usize - 1
        };

        if data.len() > available {
            return Err(IpcError::ChannelFull);
        }

        // 写入共享内存（零拷贝）
        let shm_ptr = unsafe {
            (self.region.sender_vaddr as *mut u8).add(data_start)
        };

        unsafe {
            let write_idx = write_pos as usize % buffer_size;
            if write_idx + data.len() <= buffer_size {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    shm_ptr.add(write_idx),
                    data.len(),
                );
            } else {
                let first_part = buffer_size - write_idx;
                let second_part = data.len() - first_part;
                core::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    shm_ptr.add(write_idx),
                    first_part,
                );
                core::ptr::copy_nonoverlapping(
                    data.as_ptr().add(first_part),
                    shm_ptr,
                    second_part,
                );
            }
        }

        // 更新写偏移并通知接收方
        header.write_offset.store(
            write_pos + data.len() as u64,
            Ordering::Release,
        );
        self.send_sem.fetch_add(1, Ordering::Release);
        notify_peer(self.id);

        Ok(())
    }

    /// 零拷贝接收数据
    ///
    /// 返回指向共享内存中数据的切片引用，避免任何数据拷贝。
    pub fn receive_zero_copy(&self) -> IpcResult<ShmDataRef> {
        let header = unsafe { &*self.ring_header };
        let write_pos = header.write_offset.load(Ordering::Acquire);
        let read_pos = header.read_offset.load(Ordering::Acquire);

        if write_pos == read_pos {
            return Err(IpcError::ChannelEmpty);
        }

        let available = if write_pos > read_pos {
            write_pos as usize - read_pos as usize
        } else {
            header.buffer_size as usize - read_pos as usize + write_pos as usize
        };

        let shm_ptr = unsafe {
            (self.region.receiver_vaddr as *const u8)
                .add(header.data_offset as usize)
        };
        let read_idx = read_pos as usize % header.buffer_size as usize;

        Ok(ShmDataRef {
            ptr: unsafe { shm_ptr.add(read_idx) },
            len: available,
            channel_id: self.id,
        })
    }

    /// 确认已消费数据，推进读偏移
    pub fn ack_receive(&self, len: usize) -> IpcResult<()> {
        let header = unsafe { &*self.ring_header };
        let read_pos = header.read_offset.load(Ordering::Acquire);
        header.read_offset.store(
            read_pos + len as u64,
            Ordering::Release,
        );
        Ok(())
    }
}

/// 共享内存数据引用（零拷贝视图）
pub struct ShmDataRef {
    ptr: *const u8,
    len: usize,
    channel_id: ChannelId,
}

impl ShmDataRef {
    /// 获取数据切片
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// 获取数据长度
    pub fn len(&self) -> usize {
        self.len
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
```

---

## 3. 端口 API

### 3.1 Port 定义

端口（Port）是 IPC 通信的命名端点。服务通过注册端口来暴露通信接口，客户端通过端口名来发现和连接服务。

```rust
/// 端口 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(pub u64);

/// 端口名称
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortName {
    /// 服务名称（如 "fs", "net", "agent-registry"）
    pub service: String,
    /// 实例编号（同一服务可有多个实例）
    pub instance: u32,
}

impl PortName {
    /// 创建端口名称
    pub fn new(service: &str, instance: u32) -> Self {
        Self {
            service: service.to_string(),
            instance,
        }
    }

    /// 从点分字符串解析端口名
    ///
    /// 格式: "service_name.instance_id"
    pub fn from_str(s: &str) -> IpcResult<Self> {
        let parts: Vec<&str> = s.rsplitn(2, '.').collect();
        if parts.len() != 2 {
            return Err(IpcError::InvalidPortName(s.to_string()));
        }
        let instance: u32 = parts[0].parse()
            .map_err(|_| IpcError::InvalidPortName(s.to_string()))?;
        Ok(Self {
            service: parts[1].to_string(),
            instance,
        })
    }
}

/// 端口权限
#[derive(Debug, Clone, Copy)]
pub struct PortPermissions {
    /// 读权限（接收消息）
    pub read: bool,
    /// 写权限（发送消息）
    pub write: bool,
    /// 连接权限
    pub connect: bool,
    /// 管理权限（修改端口配置）
    pub admin: bool,
}

impl PortPermissions {
    /// 所有权限
    pub fn all() -> Self {
        Self { read: true, write: true, connect: true, admin: true }
    }

    /// 无权限
    pub fn none() -> Self {
        Self { read: false, write: false, connect: false, admin: false }
    }
}

/// 端口状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PortState {
    /// 已注册，等待连接
    Listening = 0,
    /// 已连接
    Connected = 1,
    /// 已关闭
    Closed = 2,
}

/// 端口
pub struct Port {
    /// 端口 ID
    pub id: PortId,
    /// 端口名称
    pub name: PortName,
    /// 端口所有者（任务 ID）
    pub owner: TaskId,
    /// 端口权限
    pub permissions: PortPermissions,
    /// 端口状态
    pub state: PortState,
    /// 关联的通道列表
    pub channels: SpinLock<Vec<ChannelId>>,
    /// 最大连接数
    pub max_connections: u32,
}

/// 任务 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);
```

### 3.2 PortNamespace — 端口命名空间管理

```rust
/// 端口命名空间
///
/// 管理端口名称到端口 ID 的映射，提供注册、查找、连接、注销等操作。
pub struct PortNamespace {
    /// 端口名称到端口 ID 的映射
    name_to_id: SpinLock<HashMap<PortName, PortId>>,
    /// 端口 ID 到端口的映射
    ports: SpinLock<HashMap<PortId, Port>>,
    /// 全局端口 ID 分配器
    next_port_id: AtomicU64,
}

impl PortNamespace {
    /// 创建新的端口命名空间
    pub fn new() -> Self {
        Self {
            name_to_id: SpinLock::new(HashMap::new()),
            ports: SpinLock::new(HashMap::new()),
            next_port_id: AtomicU64::new(1),
        }
    }

    /// 注册新端口
    ///
    /// 将服务名称绑定到端口 ID，使其他任务可以通过名称发现此端口。
    pub fn register(
        &self,
        name: PortName,
        owner: TaskId,
        perms: PortPermissions,
    ) -> IpcResult<PortId> {
        // 检查名称是否已存在
        {
            let mapping = self.name_to_id.lock();
            if mapping.contains_key(&name) {
                return Err(IpcError::PortAlreadyExists {
                    name: name.service.clone(),
                });
            }
        }

        // 分配端口 ID
        let id = PortId(self.next_port_id.fetch_add(1, Ordering::Relaxed));

        // 创建端口
        let port = Port {
            id,
            name: name.clone(),
            owner,
            permissions: perms,
            state: PortState::Listening,
            channels: SpinLock::new(Vec::new()),
            max_connections: 256,
        };

        // 注册映射
        self.name_to_id.lock().insert(name, id);
        self.ports.lock().insert(id, port);

        Ok(id)
    }

    /// 查找端口
    ///
    /// 通过端口名称查找对应的端口 ID。
    pub fn lookup(&self, name: &PortName) -> IpcResult<PortId> {
        self.name_to_id.lock()
            .get(name)
            .copied()
            .ok_or(IpcError::PortNotFound {
                name: name.service.clone(),
            })
    }

    /// 绑定端口到指定通道
    ///
    /// 将已有通道关联到端口，用于已建立连接的场景。
    pub fn bind_channel(&self, port_id: PortId, channel_id: ChannelId) -> IpcResult<()> {
        let ports = self.ports.lock();
        let port = ports.get(&port_id)
            .ok_or(IpcError::InvalidPortId(port_id))?;
        let mut channels = port.channels.lock();
        if channels.len() >= port.max_connections as usize {
            return Err(IpcError::TooManyConnections {
                port: port_id,
                max: port.max_connections,
            });
        }
        channels.push(channel_id);
        Ok(())
    }

    /// 连接到端口
    ///
    /// 创建新通道并关联到目标端口，返回通道 ID。
    pub fn connect(&self, port_id: PortId, requester: TaskId) -> IpcResult<ChannelId> {
        let ports = self.ports.lock();
        let port = ports.get(&port_id)
            .ok_or(IpcError::InvalidPortId(port_id))?;

        if !port.permissions.connect {
            return Err(IpcError::PermissionDenied {
                operation: "connect",
            });
        }

        if port.channels.lock().len() >= port.max_connections as usize {
            return Err(IpcError::TooManyConnections {
                port: port_id,
                max: port.max_connections,
            });
        }

        // 创建新通道对
        let channel_id = create_channel_pair(port.owner, requester)?;
        port.channels.lock().push(channel_id);
        Ok(channel_id)
    }

    /// 注销端口
    ///
    /// 仅端口所有者或管理员可执行此操作。
    pub fn unregister(&self, id: PortId, requester: TaskId) -> IpcResult<()> {
        let mut ports = self.ports.lock();
        let port = ports.get(&id)
            .ok_or(IpcError::InvalidPortId(id))?;

        if port.owner != requester {
            return Err(IpcError::PermissionDenied {
                operation: "unregister",
            });
        }

        // 关闭所有关联通道
        let channels: Vec<ChannelId> = port.channels.lock().clone();
        for ch_id in channels {
            let _ = close_channel(ch_id);
        }

        let name = port.name.clone();
        ports.remove(&id);
        self.name_to_id.lock().remove(&name);
        Ok(())
    }

    /// 列出所有已注册端口
    pub fn list_ports(&self) -> Vec<(PortName, PortId, PortState)> {
        let ports = self.ports.lock();
        ports.values()
            .map(|p| (p.name.clone(), p.id, p.state))
            .collect()
    }
}
```

### 3.3 名称服务发现

系统预定义了一组知名端口，供核心服务使用：

```rust
/// 系统预定义端口名称
pub mod well_known_ports {
    use super::PortName;

    /// 文件系统服务
    pub const FILESYSTEM: &str = "sys.filesystem";
    /// 网络服务
    pub const NETWORK: &str = "sys.network";
    /// 设备管理器
    pub const DEVICE_MANAGER: &str = "sys.devices";
    /// AI 推理服务
    pub const AI_INFERENCE: &str = "sys.ai.inference";
    /// 安全 Enclave
    pub const SECURITY_ENCLAVE: &str = "sys.security.enclave";
    /// 虚拟化管理器
    pub const VIRTUALIZATION: &str = "sys.virtualization";
    /// 窗口管理服务
    pub const WINDOW_MANAGER: &str = "sys.window-manager";
    /// Agent 注册表
    pub const AGENT_REGISTRY: &str = "sys.agent-registry";
    /// 日志服务
    pub const LOG: &str = "sys.log";
    /// 定时器服务
    pub const TIMER: &str = "sys.timer";
}

/// 名称服务发现
///
/// 通过服务名称查找端点 ID，支持缓存加速。
pub struct NameService {
    /// 端口命名空间引用
    namespace: &'static PortNamespace,
    /// 查找缓存
    cache: SpinLock<HashMap<String, (PortId, u64)>>,
}

impl NameService {
    /// 发现服务
    ///
    /// 优先从缓存查找，缓存未命中时查询命名空间。
    pub fn discover(&self, service_name: &str) -> IpcResult<PortId> {
        // 检查缓存
        {
            let cache = self.cache.lock();
            if let Some(&(_, ts)) = cache.get(service_name) {
                let now = current_time_ns();
                if now - ts < 5_000_000_000 { // 5 秒缓存有效期
                    // 缓存命中，从命名空间验证
                    if let Ok(port_id) = self.namespace.lookup(
                        &PortName::from_str(service_name)?
                    ) {
                        return Ok(port_id);
                    }
                }
            }
        }

        // 缓存未命中，查询命名空间
        let port_id = self.namespace.lookup(
            &PortName::from_str(service_name)?
        )?;

        // 更新缓存
        self.cache.lock().insert(
            service_name.to_string(),
            (port_id, current_time_ns()),
        );

        Ok(port_id)
    }
}
```

---

## 4. 消息 API

### 4.1 Message 和 MessageHeader

```rust
/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    /// 普通数据消息
    Data = 0,
    /// 请求消息（期望响应）
    Request = 1,
    /// 响应消息
    Response = 2,
    /// 通知消息（不期望响应）
    Notification = 3,
    /// 共享内存描述符
    SharedMem = 4,
    /// 广播消息
    Broadcast = 5,
    /// 错误消息
    Error = 6,
}

/// 消息标志
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MessageFlags: u32 {
        /// 紧急消息（高优先级）
        const URGENT       = 0x01;
        /// 零拷贝消息（payload 为共享内存引用）
        const ZERO_COPY    = 0x02;
        /// 需要确认（可靠传输）
        const ACK_REQUIRED = 0x04;
        /// 单向消息
        const ONE_WAY      = 0x08;
        /// 请求消息 (RPC 请求)
        const REQUEST      = 0x10;
        /// 响应消息 (RPC 响应)
        const RESPONSE     = 0x20;
        /// 错误响应
        const ERROR        = 0x40;
        /// 已确认
        const ACKED        = 0x80;
    }
}

/// 消息优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum MessagePriority {
    /// 低优先级（后台任务）
    Low = 0,
    /// 普通优先级
    Normal = 128,
    /// 高优先级（交互式任务）
    High = 192,
    /// 紧急优先级（系统关键消息）
    Urgent = 255,
}

/// 消息头 — 固定大小
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MessageHeader {
    /// 消息类型
    pub msg_type: MessageType,
    /// 发送者任务 ID
    pub sender: TaskId,
    /// 接收者任务 ID
    pub receiver: TaskId,
    /// 消息负载长度（字节）
    pub payload_len: u32,
    /// 消息标志
    pub flags: MessageFlags,
    /// 消息序列号（单调递增，用于排序和去重）
    pub sequence: u64,
    /// 事务 ID（用于 RPC 请求-响应匹配）
    pub tx_id: u64,
    /// 时间戳（纳秒）
    pub timestamp: u64,
    /// 消息优先级
    pub priority: MessagePriority,
}

/// 最大内联载荷大小
pub const MAX_INLINE_PAYLOAD: usize = 4096;

/// 完整消息
#[derive(Debug, Clone)]
pub struct Message {
    /// 消息头
    pub header: MessageHeader,
    /// 消息负载
    pub payload: Vec<u8>,
}
```

### 4.2 消息构造与操作

```rust
impl Message {
    /// 创建新消息
    pub fn new(msg_type: MessageType, sender: TaskId, receiver: TaskId) -> Self {
        Self {
            header: MessageHeader {
                msg_type,
                sender,
                receiver,
                payload_len: 0,
                flags: MessageFlags::empty(),
                sequence: next_sequence_num(),
                tx_id: 0,
                timestamp: current_time_ns(),
                priority: MessagePriority::Normal,
            },
            payload: Vec::new(),
        }
    }

    /// 创建带负载的消息
    pub fn with_payload(
        msg_type: MessageType,
        sender: TaskId,
        receiver: TaskId,
        payload: Vec<u8>,
    ) -> Self {
        let payload_len = payload.len() as u32;
        Self {
            header: MessageHeader {
                msg_type,
                sender,
                receiver,
                payload_len,
                flags: MessageFlags::empty(),
                sequence: next_sequence_num(),
                tx_id: 0,
                timestamp: current_time_ns(),
                priority: MessagePriority::Normal,
            },
            payload,
        }
    }

    /// 创建 RPC 请求消息
    pub fn request(sender: TaskId, receiver: TaskId) -> Self {
        let mut msg = Self::new(MessageType::Request, sender, receiver);
        msg.header.flags = MessageFlags::REQUEST;
        msg
    }

    /// 创建 RPC 响应消息
    pub fn response(request: &Message, payload: Vec<u8>) -> Self {
        let mut msg = Self::with_payload(
            MessageType::Response,
            request.header.receiver,
            request.header.sender,
            payload,
        );
        msg.header.flags = MessageFlags::RESPONSE;
        msg.header.tx_id = request.header.tx_id;
        msg
    }

    /// 创建通知消息
    pub fn notification(sender: TaskId, receiver: TaskId, payload: Vec<u8>) -> Self {
        let mut msg = Self::with_payload(
            MessageType::Notification,
            sender,
            receiver,
            payload,
        );
        msg.header.flags = MessageFlags::ONE_WAY;
        msg
    }

    /// 设置消息优先级
    pub fn set_priority(&mut self, priority: MessagePriority) {
        self.header.priority = priority;
        if priority >= MessagePriority::High {
            self.header.flags |= MessageFlags::URGENT;
        }
    }

    /// 获取消息负载的字符串视图
    pub fn payload_as_str(&self) -> &str {
        std::str::from_utf8(&self.payload).unwrap_or("<invalid utf8>")
    }
}
```

### 4.3 bincode 序列化/反序列化

```rust
/// 序列化载荷
///
/// 使用 bincode 将结构化数据序列化为字节向量。
pub fn serialize_payload<T: Serialize>(data: &T) -> IpcResult<Vec<u8>> {
    bincode::serialize(data).map_err(|e| IpcError::SerializationFailed {
        reason: e.to_string(),
    })
}

/// 反序列化载荷
///
/// 使用 bincode 将字节向量反序列化为结构化数据。
pub fn deserialize_payload<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> IpcResult<T> {
    bincode::deserialize(bytes).map_err(|e| IpcError::DeserializationFailed {
        reason: e.to_string(),
    })
}

impl Message {
    /// 设置消息负载（使用 bincode 序列化）
    pub fn set_payload<T: Serialize>(&mut self, data: &T) -> IpcResult<()> {
        self.payload = serialize_payload(data)?;
        self.header.payload_len = self.payload.len() as u32;
        Ok(())
    }

    /// 获取消息负载（使用 bincode 反序列化）
    pub fn get_payload<T: DeserializeOwned>(&self) -> IpcResult<T> {
        deserialize_payload(&self.payload)
    }
}
```

---

## 5. 流控 API

### 5.1 FlowController — 基于信用的流量控制

```rust
/// 流量控制器
///
/// 基于信用（credit）的流量控制机制。发送方每发送一条消息消耗一个信用，
/// 接收方每处理一条消息归还一个信用。当信用耗尽时，发送方被阻塞或拒绝。
pub struct FlowController {
    /// 当前可用信用
    credits: AtomicI32,
    /// 最大信用值
    max_credits: u32,
    /// 等待信用的任务队列
    waiters: SpinLock<VecDeque<TaskId>>,
    /// 低水位线（低于此值时触发信用补充）
    low_watermark: u32,
    /// 高水位线（高于此值时停止发送）
    high_watermark: u32,
    /// 带宽配额（字节/秒），0 表示无限制
    bandwidth_quota: AtomicU64,
    /// 当前周期已发送字节数
    bytes_sent_this_period: AtomicU64,
    /// 带宽测量周期起始时间
    period_start: AtomicU64,
}

impl FlowController {
    /// 创建新的流量控制器
    pub fn new(initial_credits: u32, max_credits: u32) -> Self {
        Self {
            credits: AtomicI32::new(initial_credits as i32),
            max_credits,
            waiters: SpinLock::new(VecDeque::new()),
            low_watermark: max_credits / 4,
            high_watermark: max_credits * 3 / 4,
            bandwidth_quota: AtomicU64::new(0),
            bytes_sent_this_period: AtomicU64::new(0),
            period_start: AtomicU64::new(current_time_ns()),
        }
    }

    /// 消耗一个信用（发送方调用）
    ///
    /// 使用 CAS 操作确保并发安全。返回 true 表示消耗成功，
    /// 返回 false 表示信用已耗尽。
    pub fn consume_credit(&self) -> bool {
        loop {
            let current = self.credits.load(Ordering::Acquire);
            if current <= 0 {
                return false;
            }
            if self.credits.compare_exchange_weak(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return true;
            }
        }
    }

    /// 归还信用（接收方调用）
    pub fn return_credit(&self) {
        let current = self.credits.fetch_add(1, Ordering::AcqRel);
        if current < 0 {
            // 之前有发送方在等待，唤醒它
            self.wake_sender_if_needed();
        }
    }

    /// 批量归还信用
    pub fn return_credits(&self, count: u32) {
        let current = self.credits.fetch_add(count as i32, Ordering::AcqRel);
        if current < 0 {
            self.wake_sender_if_needed();
        }
    }

    /// 等待信用可用
    ///
    /// 当信用耗尽时，发送方调用此方法阻塞等待。
    pub fn wait_for_credit(&self, timeout_ns: u64) -> IpcResult<()> {
        let task_id = current_task_id();
        self.waiters.lock().push_back(task_id);
        block_task_with_timeout(task_id, timeout_ns)?;
        Ok(())
    }

    /// 唤醒等待的发送方
    pub fn wake_sender_if_needed(&self) {
        let mut waiters = self.waiters.lock();
        if let Some(tid) = waiters.pop_front() {
            wake_task(tid);
        }
    }

    /// 检查是否处于背压状态
    ///
    /// 当队列深度超过高水位线时返回 true。
    pub fn is_backpressure(&self) -> bool {
        let current = self.credits.load(Ordering::Acquire);
        current <= 0
    }

    /// 获取当前信用数
    pub fn available_credits(&self) -> i32 {
        self.credits.load(Ordering::Acquire)
    }

    /// 设置带宽配额（字节/秒）
    ///
    /// 超过配额的发送将被拒绝。
    pub fn set_bandwidth_quota(&self, bytes_per_sec: u64) {
        self.bandwidth_quota.store(bytes_per_sec, Ordering::Release);
    }

    /// 检查并消耗带宽配额
    ///
    /// 返回 true 表示在配额内，返回 false 表示已超限。
    pub fn try_consume_bandwidth(&self, bytes: u64) -> bool {
        let quota = self.bandwidth_quota.load(Ordering::Acquire);
        if quota == 0 {
            return true; // 无限制
        }

        // 检查是否需要重置周期
        let now = current_time_ns();
        let period_start = self.period_start.load(Ordering::Acquire);
        if now - period_start >= 1_000_000_000 { // 1 秒周期
            self.bytes_sent_this_period.store(0, Ordering::Release);
            self.period_start.store(now, Ordering::Release);
        }

        // CAS 更新已发送字节数
        loop {
            let current = self.bytes_sent_this_period.load(Ordering::Acquire);
            if current + bytes > quota {
                return false;
            }
            if self.bytes_sent_this_period.compare_exchange_weak(
                current,
                current + bytes,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return true;
            }
        }
    }
}
```

---

## 6. 广播 API

### 6.1 BroadcastGroup — 广播组管理

```rust
/// 广播组 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BroadcastGroupId(pub u64);

/// 广播组
///
/// 支持一对多消息投递，每个成员可配置消息过滤条件。
pub struct BroadcastGroup {
    /// 广播组 ID
    pub id: BroadcastGroupId,
    /// 广播组名称
    pub name: String,
    /// 成员列表
    members: RwLock<Vec<BroadcastMember>>,
    /// 消息序列号
    sequence: AtomicU64,
    /// 最大成员数
    pub max_members: usize,
    /// 广播组是否活跃
    active: AtomicBool,
}

/// 广播组成员
#[derive(Debug, Clone)]
pub struct BroadcastMember {
    /// 成员任务 ID
    pub task_id: TaskId,
    /// 成员加入时间
    pub joined_at: u64,
    /// 成员的消息过滤条件
    pub filter: Option<MessageFilter>,
}

/// 消息过滤器
#[derive(Debug, Clone)]
pub struct MessageFilter {
    /// 只接收指定类型的消息
    pub msg_types: Vec<MessageType>,
    /// 只接收来自指定发送者的消息
    pub senders: Option<Vec<TaskId>>,
}

impl MessageFilter {
    /// 仅过滤消息类型
    pub fn only_type(msg_type: MessageType) -> Self {
        Self {
            msg_types: vec![msg_type],
            senders: None,
        }
    }

    /// 过滤多种消息类型
    pub fn types(types: Vec<MessageType>) -> Self {
        Self {
            msg_types: types,
            senders: None,
        }
    }

    /// 仅接收指定发送者的消息
    pub fn from_senders(senders: Vec<TaskId>) -> Self {
        Self {
            msg_types: Vec::new(),
            senders: Some(senders),
        }
    }

    /// 检查消息是否匹配过滤条件
    pub fn matches(&self, msg: &Message) -> bool {
        // 检查消息类型
        if !self.msg_types.is_empty() {
            if !self.msg_types.contains(&msg.header.msg_type) {
                return false;
            }
        }
        // 检查发送者
        if let Some(ref senders) = self.senders {
            if !senders.contains(&msg.header.sender) {
                return false;
            }
        }
        true
    }
}

/// 广播结果
#[derive(Debug, Clone)]
pub struct BroadcastResult {
    /// 消息序列号
    pub sequence: u64,
    /// 成功投递数
    pub delivered: u32,
    /// 投递失败数
    pub failed: u32,
    /// 被过滤数
    pub filtered: u32,
}

impl BroadcastGroup {
    /// 创建新的广播组
    pub fn new(name: &str, max_members: usize) -> Self {
        Self {
            id: BroadcastGroupId(next_broadcast_id()),
            name: name.to_string(),
            members: RwLock::new(Vec::new()),
            sequence: AtomicU64::new(0),
            max_members,
            active: AtomicBool::new(true),
        }
    }

    /// 向所有成员广播消息
    pub fn broadcast(&self, msg: &Message) -> IpcResult<BroadcastResult> {
        if !self.active.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }

        let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
        let members = self.members.read();

        let mut result = BroadcastResult {
            sequence: seq,
            delivered: 0,
            failed: 0,
            filtered: 0,
        };

        for member in members.iter() {
            // 应用消息过滤
            if let Some(ref filter) = member.filter {
                if !filter.matches(msg) {
                    result.filtered += 1;
                    continue;
                }
            }

            // 投递消息
            match deliver_message_to(member.task_id, msg) {
                Ok(()) => result.delivered += 1,
                Err(_) => result.failed += 1,
            }
        }

        Ok(result)
    }

    /// 加入广播组
    pub fn join(&self, task_id: TaskId, filter: Option<MessageFilter>) -> IpcResult<()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }

        let mut members = self.members.write();
        if members.len() >= self.max_members {
            return Err(IpcError::BroadcastGroupFull {
                group: self.id,
                max: self.max_members,
            });
        }

        // 检查是否已在组中
        if members.iter().any(|m| m.task_id == task_id) {
            return Err(IpcError::AlreadyInBroadcastGroup {
                task: task_id,
                group: self.id,
            });
        }

        members.push(BroadcastMember {
            task_id,
            joined_at: current_time_ns(),
            filter,
        });
        Ok(())
    }

    /// 离开广播组
    pub fn leave(&self, task_id: TaskId) -> IpcResult<()> {
        let mut members = self.members.write();
        let before = members.len();
        members.retain(|m| m.task_id != task_id);
        if members.len() == before {
            return Err(IpcError::NotInBroadcastGroup {
                task: task_id,
                group: self.id,
            });
        }
        Ok(())
    }

    /// 获取当前成员数
    pub fn member_count(&self) -> usize {
        self.members.read().len()
    }

    /// 销毁广播组
    pub fn destroy(&self) -> IpcResult<()> {
        self.active.store(false, Ordering::Release);
        // 通知所有成员
        let members = self.members.read();
        for member in members.iter() {
            let _ = deliver_message_to(
                member.task_id,
                &Message::notification(
                    TaskId(0),
                    member.task_id,
                    b"broadcast_group_destroyed".to_vec(),
                ),
            );
        }
        self.members.write().clear();
        Ok(())
    }
}
```

---

## 7. 错误处理

### 7.1 IpcError 枚举

```rust
/// IPC 错误类型
///
/// 涵盖通道操作、端口管理、消息处理、共享内存、广播等所有 IPC 子系统错误。
#[derive(Debug, Clone)]
pub enum IpcError {
    // ── 通道错误 ──
    /// 通道已满（异步通道缓冲区溢出）
    ChannelFull,
    /// 通道为空（无可接收消息）
    ChannelEmpty,
    /// 通道已关闭
    ChannelClosed,
    /// 对端已死亡
    PeerDead,

    // ── 消息错误 ──
    /// 消息过大
    MessageTooLarge { size: usize, max: usize },
    /// 序列化失败
    SerializationFailed { reason: String },
    /// 反序列化失败
    DeserializationFailed { reason: String },
    /// 事务 ID 不匹配
    TransactionMismatch,
    /// 远程错误
    RemoteError(u32),

    // ── 端口错误 ──
    /// 端口不存在
    PortNotFound { name: String },
    /// 端口已存在
    PortAlreadyExists { name: String },
    /// 无效的端口 ID
    InvalidPortId(PortId),
    /// 无效的端口名称
    InvalidPortName(String),

    // ── 权限错误 ──
    /// 权限不足
    PermissionDenied { operation: &'static str },
    /// 连接数过多
    TooManyConnections { port: PortId, max: u32 },

    // ── 共享内存错误 ──
    /// 共享内存映射失败
    ShmMapFailed { reason: String },
    /// 共享内存大小无效
    ShmInvalidSize,

    // ── 流控错误 ──
    /// 带宽超限
    BandwidthExceeded,
    /// 背压（接收方处理不过来）
    Backpressure { queue_depth: u32, max_depth: u32 },

    // ── 广播错误 ──
    /// 广播组已满
    BroadcastGroupFull { group: BroadcastGroupId, max: usize },
    /// 不在广播组中
    NotInBroadcastGroup { task: TaskId, group: BroadcastGroupId },
    /// 已在广播组中
    AlreadyInBroadcastGroup { task: TaskId, group: BroadcastGroupId },

    // ── 通用错误 ──
    /// 无效的通道 ID
    InvalidChannelId(ChannelId),
    /// 操作超时
    Timeout,
    /// 内部错误
    InternalError(String),
}

pub type IpcResult<T> = Result<T, IpcError>;

#[cfg(feature = "std")]
impl std::error::Error for IpcError {}

impl core::fmt::Display for IpcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ChannelFull => write!(f, "通道已满"),
            Self::ChannelEmpty => write!(f, "通道为空"),
            Self::ChannelClosed => write!(f, "通道已关闭"),
            Self::PeerDead => write!(f, "对端已死亡"),
            Self::MessageTooLarge { size, max } => {
                write!(f, "消息过大: {} 字节 (最大 {} 字节)", size, max)
            }
            Self::SerializationFailed { reason } => {
                write!(f, "序列化失败: {}", reason)
            }
            Self::DeserializationFailed { reason } => {
                write!(f, "反序列化失败: {}", reason)
            }
            Self::TransactionMismatch => write!(f, "事务 ID 不匹配"),
            Self::RemoteError(code) => write!(f, "远程错误: {}", code),
            Self::PortNotFound { name } => write!(f, "端口不存在: {}", name),
            Self::PortAlreadyExists { name } => write!(f, "端口已存在: {}", name),
            Self::InvalidPortId(id) => write!(f, "无效的端口 ID: {:?}", id),
            Self::InvalidPortName(name) => write!(f, "无效的端口名称: {}", name),
            Self::PermissionDenied { operation } => {
                write!(f, "权限不足: 无法执行 '{}'", operation)
            }
            Self::TooManyConnections { port, max } => {
                write!(f, "连接数过多: 端口 {:?} 最大连接数 {}", port, max)
            }
            Self::ShmMapFailed { reason } => {
                write!(f, "共享内存映射失败: {}", reason)
            }
            Self::ShmInvalidSize => write!(f, "共享内存大小无效"),
            Self::BandwidthExceeded => write!(f, "带宽超限"),
            Self::Backpressure { queue_depth, max_depth } => {
                write!(f, "背压: 队列深度 {}/{}", queue_depth, max_depth)
            }
            Self::BroadcastGroupFull { group, max } => {
                write!(f, "广播组已满: {:?} 最大成员数 {}", group, max)
            }
            Self::NotInBroadcastGroup { task, group } => {
                write!(f, "任务 {:?} 不在广播组 {:?} 中", task, group)
            }
            Self::AlreadyInBroadcastGroup { task, group } => {
                write!(f, "任务 {:?} 已在广播组 {:?} 中", task, group)
            }
            Self::InvalidChannelId(id) => write!(f, "无效的通道 ID: {:?}", id),
            Self::Timeout => write!(f, "操作超时"),
            Self::InternalError(msg) => write!(f, "内部错误: {}", msg),
        }
    }
}
```

---

## 8. 使用示例

### 8.1 同步 RPC 调用

```rust
use omniagent_ipc::*;

/// 客户端通过同步通道调用文件系统服务
fn rpc_call_example() -> IpcResult<()> {
    // 1. 发现文件系统服务端口
    let namespace = PortNamespace::new();
    let fs_port = namespace.lookup(&PortName::new("sys.filesystem", 0))?;

    // 2. 连接到服务端口，获取通道
    let channel_id = namespace.connect(fs_port, TaskId(1))?;
    let channel = get_sync_channel(channel_id)?;

    // 3. 构造 RPC 请求
    let mut request = Message::request(TaskId(1), TaskId(0));
    let fs_req = FsReadRequest {
        path: "/data/config.json".to_string(),
        offset: 0,
        size: 4096,
    };
    request.set_payload(&fs_req)?;

    // 4. 发送请求并等待响应
    let response = channel.call(&request, Duration::from_secs(5))?;

    // 5. 解析响应
    let fs_resp: FsReadResponse = response.get_payload()?;
    println!("读取了 {} 字节数据", fs_resp.data.len());

    Ok(())
}
```

### 8.2 异步消息队列通信

```rust
/// 生产者-消费者模式：Agent 间异步消息交换
fn async_channel_example() -> IpcResult<()> {
    // 1. 创建异步通道
    let config = ChannelConfig {
        channel_type: ChannelType::Asynchronous,
        capacity: 1024,
        flow_control: true,
        initial_credits: 128,
        ..Default::default()
    };
    let channel = AsyncChannel::new(&config);

    // 2. 生产者发送消息
    let sender_channel = channel.clone();
    let producer = std::thread::spawn(move || {
        for i in 0..1000 {
            let msg = Message::with_payload(
                MessageType::Data,
                TaskId(1),
                TaskId(2),
                format!("message-{}", i).into_bytes(),
            );
            match sender_channel.try_send(msg) {
                Ok(()) => {},
                Err(IpcError::ChannelFull) => {
                    // 等待信用恢复
                    std::thread::sleep(Duration::from_micros(10));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    });

    // 3. 消费者接收消息
    let receiver_channel = channel.clone();
    let consumer = std::thread::spawn(move || {
        let mut count = 0;
        while count < 1000 {
            match receiver_channel.try_receive() {
                Ok(msg) => {
                    count += 1;
                    println!("收到: {}", msg.payload_as_str());
                }
                Err(IpcError::ChannelEmpty) => {
                    std::thread::yield_now();
                }
                Err(IpcError::ChannelClosed) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(count)
    });

    producer.join().unwrap()?;
    let received = consumer.join().unwrap()?;
    println!("总共接收 {} 条消息", received);

    Ok(())
}
```

### 8.3 零拷贝大块数据传输

```rust
/// 通过共享内存通道传输 AI 模型推理数据
fn shared_memory_example() -> IpcResult<()> {
    // 1. 创建共享内存通道（64MB）
    let shm_channel = SharedMemoryChannel::new(64 * 1024 * 1024)?;

    // 2. 发送方：准备图像数据并零拷贝发送
    let image_data: Vec<u8> = vec![0xFF; 1920 * 1080 * 3]; // 1080p RGB
    shm_channel.send_zero_copy(&image_data)?;

    // 3. 接收方：零拷贝读取数据
    let data_ref = shm_channel.receive_zero_copy()?;
    println!("接收到 {} 字节数据（零拷贝）", data_ref.len());

    // 4. 确认消费
    shm_channel.ack_receive(data_ref.len())?;

    Ok(())
}
```

### 8.4 广播与消息过滤

```rust
/// 事件通知：系统事件广播给多个 Agent
fn broadcast_example() -> IpcResult<()> {
    // 1. 创建广播组
    let group = BroadcastGroup::new("system-events", 100);

    // 2. Agent 加入广播组（带过滤条件）
    group.join(
        TaskId(1),
        Some(MessageFilter::only_type(MessageType::Notification)),
    )?;
    group.join(TaskId(2), None)?; // 接收所有消息
    group.join(
        TaskId(3),
        Some(MessageFilter::types(vec![
            MessageType::Notification,
            MessageType::Error,
        ])),
    )?;

    // 3. 广播系统事件
    let event = Message::notification(
        TaskId(0),
        TaskId(0),
        b"system:memory_low".to_vec(),
    );
    let result = group.broadcast(&event)?;
    println!(
        "广播完成: 序列号={}, 投递={}, 失败={}, 过滤={}",
        result.sequence, result.delivered, result.failed, result.filtered
    );

    // 4. Agent 退出广播组
    group.leave(TaskId(1))?;

    Ok(())
}
```

---

## 9. 性能约束

### 9.1 延迟/吞吐量目标

| 操作 | 延迟目标 | 吞吐量目标 | 测量条件 |
|------|---------|-----------|---------|
| 同步消息发送+接收 | < 500ns | 200万/s | 核内，64 字节消息 |
| 同步 RPC (4KB) | < 1us | 100万/s | 核内，4KB 载荷 |
| 跨核同步 RPC | < 5us | 20万/s | 含 IPI 开销 |
| 异步消息发送 | < 200ns | 500万/s | 核内，64 字节消息 |
| 异步消息接收 | < 300ns | 300万/s | 核内，64 字节消息 |
| 零拷贝传输 | < 100ns | > 10 GB/s | 仅指针传递 |
| 端口名称查找 | < 200ns | 500万/s | HashMap 查找 |
| 广播 (100 接收者) | < 5us | 20万/s | 核内，64 字节消息 |
| bincode 序列化 | < 500ns | 200万/s | 1KB 结构体 |
| bincode 反序列化 | < 500ns | 200万/s | 1KB 结构体 |
| 共享内存建立 (首次) | < 10us | 10万/s | 含页表映射 |
| 共享内存建立 (后续) | < 100ns | 1000万/s | 引用传递 |

### 9.2 资源限制

| 资源 | 默认上限 | 说明 |
|------|---------|------|
| 最大通道数 | 65536 | 全局并发通道 |
| 最大端口数 | 4096 | 已注册端口 |
| 最大广播组数 | 256 | 系统广播组 |
| 最大广播组成员 | 1024 | 单组上限 |
| 最大共享内存 | 1 GB | 单区域上限 |
| 同步消息大小 | 64 KB | 单条消息 |
| 异步消息大小 | 1 MB | 单条消息 |
| 内联载荷 | 4 KB | 消息头内联 |

---

## 10. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ── 通道测试 ──

    #[test]
    fn test_channel_config_default() {
        let config = ChannelConfig::default();
        assert_eq!(config.channel_type, ChannelType::Asynchronous);
        assert_eq!(config.capacity, 256);
        assert_eq!(config.max_message_size, 4096);
        assert!(config.flow_control);
        assert_eq!(config.initial_credits, 64);
    }

    #[test]
    fn test_sync_channel_send_receive() {
        let (sender, receiver) = SyncChannel::create_pair();
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        sender.send(msg.clone(), None).unwrap();
        let received = receiver.receive(None).unwrap();
        assert_eq!(received.header.msg_type, MessageType::Data);
        assert_eq!(received.header.sender, TaskId(1));
    }

    #[test]
    fn test_sync_channel_peer_dead() {
        let (sender, receiver) = SyncChannel::create_pair();
        receiver.close().unwrap();
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        let result = sender.send(msg, None);
        assert!(matches!(result, Err(IpcError::PeerDead)));
    }

    #[test]
    fn test_async_channel_buffered_send() {
        let config = ChannelConfig {
            channel_type: ChannelType::Asynchronous,
            capacity: 10,
            ..Default::default()
        };
        let channel = AsyncChannel::new(&config);

        for _ in 0..10 {
            let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
            channel.try_send(msg).unwrap();
        }

        // 第 11 条消息应失败（通道满）
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        assert!(matches!(channel.try_send(msg), Err(IpcError::ChannelFull)));
    }

    #[test]
    fn test_async_channel_close_and_drain() {
        let config = ChannelConfig {
            capacity: 5,
            ..Default::default()
        };
        let channel = AsyncChannel::new(&config);

        for _ in 0..3 {
            let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
            channel.try_send(msg).unwrap();
        }

        channel.close().unwrap();
        assert!(channel.is_closed());

        // 关闭后仍可读取缓冲区中的消息
        let msg = channel.try_receive().unwrap();
        assert_eq!(msg.header.msg_type, MessageType::Data);
    }

    // ── 端口测试 ──

    #[test]
    fn test_port_register_and_lookup() {
        let ns = PortNamespace::new();
        let name = PortName::new("test-service", 0);
        let port_id = ns.register(name.clone(), TaskId(1), PortPermissions::all())
            .unwrap();
        let found_id = ns.lookup(&name).unwrap();
        assert_eq!(port_id, found_id);
    }

    #[test]
    fn test_port_duplicate_registration() {
        let ns = PortNamespace::new();
        let name = PortName::new("duplicate", 0);
        ns.register(name.clone(), TaskId(1), PortPermissions::all()).unwrap();
        let result = ns.register(name.clone(), TaskId(2), PortPermissions::all());
        assert!(matches!(result, Err(IpcError::PortAlreadyExists { .. })));
    }

    #[test]
    fn test_port_unregister_permission_denied() {
        let ns = PortNamespace::new();
        let name = PortName::new("protected", 0);
        let port_id = ns.register(name, TaskId(1), PortPermissions::all()).unwrap();
        let result = ns.unregister(port_id, TaskId(999));
        assert!(matches!(result, Err(IpcError::PermissionDenied { .. })));
    }

    #[test]
    fn test_port_name_from_str() {
        let name = PortName::from_str("my-service.3").unwrap();
        assert_eq!(name.service, "my-service");
        assert_eq!(name.instance, 3);
    }

    #[test]
    fn test_port_name_invalid_format() {
        let result = PortName::from_str("no-instance");
        assert!(matches!(result, Err(IpcError::InvalidPortName(_))));
    }

    // ── 消息测试 ──

    #[test]
    fn test_message_creation() {
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        assert_eq!(msg.header.msg_type, MessageType::Data);
        assert_eq!(msg.header.sender, TaskId(1));
        assert_eq!(msg.header.receiver, TaskId(2));
        assert!(msg.payload.is_empty());
    }

    #[test]
    fn test_message_rpc_request_response() {
        let request = Message::request(TaskId(1), TaskId(2));
        assert!(request.header.flags.contains(MessageFlags::REQUEST));

        let response = Message::response(&request, b"ok".to_vec());
        assert!(response.header.flags.contains(MessageFlags::RESPONSE));
        assert_eq!(response.header.tx_id, request.header.tx_id);
        assert_eq!(response.header.sender, TaskId(2));
        assert_eq!(response.header.receiver, TaskId(1));
    }

    #[test]
    fn test_message_serialization() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestData {
            value: u64,
            name: String,
        }

        let data = TestData { value: 42, name: "test".into() };
        let mut msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        msg.set_payload(&data).unwrap();

        let restored: TestData = msg.get_payload().unwrap();
        assert_eq!(restored.value, 42);
        assert_eq!(restored.name, "test");
    }

    #[test]
    fn test_message_priority() {
        let mut msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        msg.set_priority(MessagePriority::Urgent);
        assert_eq!(msg.header.priority, MessagePriority::Urgent);
        assert!(msg.header.flags.contains(MessageFlags::URGENT));
    }

    // ── 流控测试 ──

    #[test]
    fn test_flow_control_credit_depletion() {
        let fc = FlowController::new(3, 10);
        assert!(fc.consume_credit());
        assert!(fc.consume_credit());
        assert!(fc.consume_credit());
        // 信用耗尽
        assert!(!fc.consume_credit());

        // 归还信用后可继续
        fc.return_credit();
        assert!(fc.consume_credit());
    }

    #[test]
    fn test_flow_control_batch_return() {
        let fc = FlowController::new(0, 10);
        assert!(!fc.consume_credit());
        fc.return_credits(5);
        assert_eq!(fc.available_credits(), 5);
    }

    #[test]
    fn test_flow_control_bandwidth_quota() {
        let fc = FlowController::new(100, 100);
        fc.set_bandwidth_quota(1000); // 1000 字节/秒

        // 前 1000 字节应通过
        assert!(fc.try_consume_bandwidth(500));
        assert!(fc.try_consume_bandwidth(500));
        // 超出配额
        assert!(!fc.try_consume_bandwidth(1));
    }

    // ── 广播测试 ──

    #[test]
    fn test_broadcast_delivery() {
        let group = BroadcastGroup::new("test-group", 100);
        group.join(TaskId(1), None).unwrap();
        group.join(TaskId(2), None).unwrap();
        group.join(TaskId(3), None).unwrap();

        let msg = Message::notification(TaskId(0), TaskId(0), b"hello".to_vec());
        let result = group.broadcast(&msg).unwrap();
        assert_eq!(result.delivered, 3);
        assert_eq!(result.failed, 0);
        assert_eq!(result.filtered, 0);
    }

    #[test]
    fn test_broadcast_with_filter() {
        let group = BroadcastGroup::new("filtered-group", 100);
        group.join(
            TaskId(1),
            Some(MessageFilter::only_type(MessageType::Data)),
        ).unwrap();
        group.join(TaskId(2), None).unwrap();

        let msg = Message::notification(TaskId(0), TaskId(0), b"event".to_vec());
        let result = group.broadcast(&msg).unwrap();
        assert_eq!(result.delivered, 1);  // 只有 TaskId(2) 接收
        assert_eq!(result.filtered, 1);   // TaskId(1) 被过滤
    }

    #[test]
    fn test_broadcast_join_duplicate() {
        let group = BroadcastGroup::new("dup-group", 100);
        group.join(TaskId(1), None).unwrap();
        let result = group.join(TaskId(1), None);
        assert!(matches!(result, Err(IpcError::AlreadyInBroadcastGroup { .. })));
    }

    #[test]
    fn test_broadcast_leave_not_member() {
        let group = BroadcastGroup::new("leave-group", 100);
        let result = group.leave(TaskId(999));
        assert!(matches!(result, Err(IpcError::NotInBroadcastGroup { .. })));
    }

    #[test]
    fn test_broadcast_group_full() {
        let group = BroadcastGroup::new("full-group", 2);
        group.join(TaskId(1), None).unwrap();
        group.join(TaskId(2), None).unwrap();
        let result = group.join(TaskId(3), None);
        assert!(matches!(result, Err(IpcError::BroadcastGroupFull { .. })));
    }

    // ── 错误处理测试 ──

    #[test]
    fn test_ipc_error_display() {
        let err = IpcError::PermissionDenied { operation: "connect" };
        let msg = format!("{}", err);
        assert!(msg.contains("权限不足"));
        assert!(msg.contains("connect"));

        let err = IpcError::MessageTooLarge { size: 8192, max: 4096 };
        let msg = format!("{}", err);
        assert!(msg.contains("8192"));
        assert!(msg.contains("4096"));

        let err = IpcError::Backpressure {
            queue_depth: 1024,
            max_depth: 1024,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("背压"));
    }

    #[test]
    fn test_ring_buffer_operations() {
        let mut rb: RingBuffer<u32> = RingBuffer::new(4);
        assert!(rb.is_empty());
        assert!(rb.push(1).is_ok());
        assert!(rb.push(2).is_ok());
        assert!(rb.push(3).is_ok());
        assert!(rb.push(4).is_ok());
        assert!(rb.is_full());
        assert!(rb.push(5).is_err());
        assert_eq!(rb.pop(), Some(1));
        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.len(), 2);
        assert!(!rb.is_full());
        assert!(!rb.is_empty());
    }

    // ── 共享内存测试 ──

    #[test]
    fn test_shared_memory_invalid_size() {
        let result = SharedMemoryChannel::new(0);
        assert!(matches!(result, Err(IpcError::ShmInvalidSize)));

        let result = SharedMemoryChannel::new(SHM_MAX_SIZE + 1);
        assert!(matches!(result, Err(IpcError::MessageTooLarge { .. })));
    }

    #[test]
    fn test_shared_memory_channel_id_unique() {
        let ch1 = SharedMemoryChannel::new(4096).unwrap();
        let ch2 = SharedMemoryChannel::new(4096).unwrap();
        assert_ne!(ch1.id(), ch2.id());
    }
}
```

---

*本文档为 OmniAgent OS IPC API 参考，版本 0.1.0。与 [IPC 协议规范](../architecture/ipc-protocol.md) 和 [IPC 模块规格](../modules/ipc-spec.md) 保持一致。*
