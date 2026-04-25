# OmniAgent OS — IPC 模块规格说明

> **模块名称**: `omniagent-ipc`
> **版本**: v0.1.0-draft
> **状态**: 设计阶段
> **依赖**: `bincode`, `spin`, `buddy_system_allocator`, `log`

---

## 1. 概述

### 1.1 目的

IPC（进程间通信）模块是 OmniAgent OS 微内核架构的核心通信基础设施。在微内核设计中，所有服务（文件系统、设备驱动、网络协议栈等）都以独立任务运行，通过 IPC 进行通信。本模块提供高效的、类型安全的、支持零拷贝的进程间通信机制，特别针对 AI Agent 之间的高频通信场景进行了优化。

### 1.2 设计目标

| 目标 | 指标 |
|------|------|
| 同步消息延迟 | < 500ns（核内） |
| 异步消息延迟 | < 2μs（核内） |
| 零拷贝传输吞吐 | > 10 GB/s |
| 最大并发通道数 | 65536 |
| 消息大小限制 | 同步 64KB，异步 1MB，共享内存 1GB |
| 广播延迟 | < 5μs（100 个接收者） |

### 1.3 IPC 服务架构

```
┌─────────────────────────────────────────────────────────────┐
│                    用户空间 (User Space)                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ Agent A  │  │ Agent B  │  │ Service  │  │ Service  │    │
│  │          │  │          │  │  FS       │  │  Net      │    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘    │
│       │             │             │             │           │
│  ─────┴─────────────┴─────────────┴─────────────┴─────────  │
│                    libagent (用户空间库)                      │
│  ─────────────────────────────────────────────────────────── │
│                    系统调用接口 (Syscall)                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    内核空间 (Kernel Space)                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              IPC Manager (IPC 管理器)                  │   │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────────────┐    │   │
│  │  │  端口管理  │ │ 通道管理  │ │  共享内存管理      │    │   │
│  │  │  器       │ │  器       │ │                   │    │   │
│  │  └──────────┘ └──────────┘ └───────────────────┘    │   │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────────────┐    │   │
│  │  │  消息序列  │ │  流量控制  │ │  广播/多播管理     │    │   │
│  │  │  化/反序列 │ │  (信用)   │ │                   │    │   │
│  │  └──────────┘ └──────────┘ └───────────────────┘    │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 通道抽象

### 2.1 通道类型

IPC 模块提供三种通道类型，适用于不同的通信场景：

```rust
/// 通道类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChannelType {
    /// 同步通道：发送方阻塞直到接收方接收
    /// 适用于请求-响应模式
    Synchronous = 0,
    /// 异步通道：发送方将消息放入缓冲区后立即返回
    /// 适用于生产者-消费者模式
    Asynchronous = 1,
    /// 共享内存通道：双方通过共享内存区域直接交换数据
    /// 适用于大数据量传输
    SharedMemory = 2,
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
    /// 优先级
    pub priority: u8,
    /// 超时时间（纳秒），0 表示无限等待
    pub timeout_ns: u64,
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
            timeout_ns: 0,
        }
    }
}
```

### 2.2 通道 Trait 定义

```rust
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
    /// - `timeout_ns`: 超时时间（纳秒），0 表示无限等待
    ///
    /// # 返回
    /// - `Ok(())`: 发送成功
    /// - `Err(IpcError::ChannelFull)`: 通道已满
    /// - `Err(IpcError::PeerDead)`: 对端已关闭
    /// - `Err(IpcError::Timeout)`: 发送超时
    fn send(&self, msg: Message, timeout_ns: u64) -> IpcResult<()>;

    /// 接收消息
    ///
    /// # 参数
    /// - `timeout_ns`: 超时时间（纳秒），0 表示无限等待
    ///
    /// # 返回
    /// - `Ok(Message)`: 接收成功
    /// - `Err(IpcError::ChannelEmpty)`: 通道为空
    /// - `Err(IpcError::PeerDead)`: 对端已关闭
    /// - `Err(IpcError::Timeout)`: 接收超时
    fn receive(&self, timeout_ns: u64) -> IpcResult<Message>;

    /// 关闭通道
    fn close(&self) -> IpcResult<()>;

    /// 检查通道是否已关闭
    fn is_closed(&self) -> bool;

    /// 获取通道统计信息
    fn stats(&self) -> ChannelStats;
}
```

### 2.3 同步通道实现

```rust
/// 同步通道
///
/// 同步通道不使用缓冲区，发送方直接将消息复制到接收方的地址空间。
/// 如果接收方尚未就绪，发送方阻塞等待。
pub struct SyncChannel {
    /// 通道 ID
    id: ChannelId,
    /// 发送端
    sender: SyncSender,
    /// 接收端
    receiver: SyncReceiver,
    /// 通道状态
    state: AtomicU8,
    /// 统计信息
    stats: SpinLock<ChannelStats>,
}

struct SyncSender {
    /// 等待接收的任务队列
    wait_queue: SpinLock<VecDeque<TaskId>>,
    /// 发送端是否已关闭
    closed: AtomicBool,
}

struct SyncReceiver {
    /// 等待发送的任务队列
    wait_queue: SpinLock<VecDeque<TaskId>>,
    /// 接收端是否已关闭
    closed: AtomicBool,
}

impl Channel for SyncChannel {
    fn id(&self) -> ChannelId { self.id }

    fn channel_type(&self) -> ChannelType { ChannelType::Synchronous }

    fn send(&self, msg: Message, timeout_ns: u64) -> IpcResult<()> {
        if self.receiver.closed.load(Ordering::Acquire) {
            return Err(IpcError::PeerDead);
        }
        if self.sender.closed.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }

        // 检查消息大小
        if msg.payload.len() > SYNC_MAX_MSG_SIZE {
            return Err(IpcError::MessageTooLarge {
                size: msg.payload.len(),
                max: SYNC_MAX_MSG_SIZE,
            });
        }

        // 尝试直接传输给等待中的接收方
        {
            let mut waiters = self.receiver.wait_queue.lock();
            if let Some(receiver_tid) = waiters.pop_front() {
                // 直接将消息传递给接收方
                deliver_message(receiver_tid, msg);
                return Ok(());
            }
        }

        // 没有等待的接收方，阻塞当前任务
        if timeout_ns == 0 {
            block_current_task_with_msg(msg);
        } else {
            block_current_task_with_timeout(timeout_ns)?;
        }

        Ok(())
    }

    fn receive(&self, timeout_ns: u64) -> IpcResult<Message> {
        if self.sender.closed.load(Ordering::Acquire)
            && self.sender.wait_queue.lock().is_empty()
        {
            return Err(IpcError::PeerDead);
        }

        // 检查是否有等待的发送方
        {
            let mut waiters = self.sender.wait_queue.lock();
            if let Some(sender_tid) = waiters.pop_front() {
                // 从发送方获取消息
                return take_message_from_sender(sender_tid);
            }
        }

        // 没有等待的发送方，阻塞当前任务
        if timeout_ns == 0 {
            block_current_task_for_receive();
        } else {
            block_current_task_with_timeout(timeout_ns)?;
        }

        receive_delivered_message()
    }

    fn close(&self) -> IpcResult<()> {
        self.sender.closed.store(true, Ordering::Release);
        self.receiver.closed.store(true, Ordering::Release);
        // 唤醒所有等待中的任务
        wake_all_waiters(&self.sender.wait_queue);
        wake_all_waiters(&self.receiver.wait_queue);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.sender.closed.load(Ordering::Acquire)
            || self.receiver.closed.load(Ordering::Acquire)
    }

    fn stats(&self) -> ChannelStats {
        self.stats.lock().clone()
    }
}

const SYNC_MAX_MSG_SIZE: usize = 64 * 1024; // 64KB
```

### 2.4 异步通道实现

```rust
/// 异步通道
///
/// 异步通道使用环形缓冲区存储消息，发送方将消息放入缓冲区后立即返回。
/// 当缓冲区满时，根据流量控制策略决定是阻塞还是丢弃。
pub struct AsyncChannel {
    /// 通道 ID
    id: ChannelId,
    /// 消息环形缓冲区
    buffer: SpinLock<RingBuffer<Message>>,
    /// 流量控制状态
    flow_control: SpinLock<FlowController>,
    /// 通道状态
    state: AtomicU8,
    /// 统计信息
    stats: SpinLock<ChannelStats>,
}

/// 环形缓冲区
pub struct RingBuffer<T> {
    data: Vec<Option<T>>,
    head: usize,  // 读指针
    tail: usize,  // 写指针
    count: usize, // 当前元素数量
    capacity: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let mut data = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            data.push(None);
        }
        Self {
            data,
            head: 0,
            tail: 0,
            count: 0,
            capacity,
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.count == self.capacity {
            return Err(item);
        }
        self.data[self.tail] = Some(item);
        self.tail = (self.tail + 1) % self.capacity;
        self.count += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.count == 0 {
            return None;
        }
        let item = self.data[self.head].take();
        self.head = (self.head + 1) % self.capacity;
        self.count -= 1;
        item
    }

    pub fn is_full(&self) -> bool { self.count == self.capacity }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn len(&self) -> usize { self.count }
}

impl Channel for AsyncChannel {
    fn id(&self) -> ChannelId { self.id }

    fn channel_type(&self) -> ChannelType { ChannelType::Asynchronous }

    fn send(&self, msg: Message, timeout_ns: u64) -> IpcResult<()> {
        // 检查信用
        {
            let mut fc = self.flow_control.lock();
            if !fc.consume_credit() {
                if timeout_ns != 0 {
                    fc.wait_for_credit(timeout_ns)?;
                } else {
                    return Err(IpcError::ChannelFull);
                }
            }
        }

        // 放入缓冲区
        {
            let mut buf = self.buffer.lock();
            if let Err(_msg) = buf.push(msg) {
                return Err(IpcError::ChannelFull);
            }
        }

        // 更新统计
        {
            let mut stats = self.stats.lock();
            stats.messages_sent += 1;
            stats.bytes_sent += msg.payload.len() as u64;
        }

        // 唤醒等待的接收方
        wake_receivers(self.id);
        Ok(())
    }

    fn receive(&self, timeout_ns: u64) -> IpcResult<Message> {
        // 从缓冲区取出消息
        let msg = {
            let mut buf = self.buffer.lock();
            buf.pop().ok_or(IpcError::ChannelEmpty)
        }?;

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
            stats.bytes_received += msg.payload.len() as u64;
        }

        Ok(msg)
    }

    fn close(&self) -> IpcResult<()> {
        self.state.store(CHANNEL_CLOSED, Ordering::Release);
        wake_all_waiters(self.id);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.state.load(Ordering::Acquire) == CHANNEL_CLOSED
    }

    fn stats(&self) -> ChannelStats {
        self.stats.lock().clone()
    }
}
```

---

## 3. 端口命名空间与发现

### 3.1 端口定义

端口（Port）是 IPC 通信的命名端点。服务通过注册端口来暴露自己的通信接口，客户端通过端口名来发现和连接服务。

```rust
/// 端口 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(u64);

/// 端口名称
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortName {
    /// 服务名称（如 "fs", "net", "agent-manager"）
    pub service: String,
    /// 实例编号（同一服务可有多个实例）
    pub instance: u32,
}

impl PortName {
    pub fn new(service: &str, instance: u32) -> Self {
        Self {
            service: service.to_string(),
            instance,
        }
    }

    /// 系统端口：文件系统服务
    pub const fn fs_port() -> PortName {
        // 使用 const string 需要特殊处理
        PortName { service: "fs".into(), instance: 0 }
    }
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

/// 端口权限
#[derive(Debug, Clone, Copy)]
pub struct PortPermissions {
    /// 读权限
    pub read: bool,
    /// 写权限
    pub write: bool,
    /// 连接权限
    pub connect: bool,
    /// 管理权限（修改端口配置）
    pub admin: bool,
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
```

### 3.2 端口命名空间管理器

```rust
/// 端口命名空间
pub struct PortNamespace {
    /// 端口名称到端口 ID 的映射
    name_to_id: SpinLock<HashMap<PortName, PortId>>,
    /// 端口 ID 到端口的映射
    ports: SpinLock<HashMap<PortId, Port>>,
    /// 全局端口 ID 分配器
    next_port_id: AtomicU64,
}

impl PortNamespace {
    /// 注册新端口
    pub fn register(&self, name: PortName, owner: TaskId, perms: PortPermissions)
        -> IpcResult<PortId>
    {
        // 检查名称是否已存在
        {
            let mapping = self.name_to_id.lock();
            if mapping.contains_key(&name) {
                return Err(IpcError::PortAlreadyExists { name });
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

        // 注册
        self.name_to_id.lock().insert(name, id);
        self.ports.lock().insert(id, port);

        Ok(id)
    }

    /// 查找端口
    pub fn lookup(&self, name: &PortName) -> IpcResult<PortId> {
        self.name_to_id.lock()
            .get(name)
            .copied()
            .ok_or(IpcError::PortNotFound {
                name: name.service.clone(),
            })
    }

    /// 注销端口
    pub fn unregister(&self, id: PortId, requester: TaskId) -> IpcResult<()> {
        let mut ports = self.ports.lock();
        let port = ports.get(&id).ok_or(IpcError::InvalidPortId(id))?;

        // 验证权限
        if port.owner != requester {
            return Err(IpcError::PermissionDenied {
                operation: "unregister",
            });
        }

        let name = port.name.clone();
        ports.remove(&id);
        self.name_to_id.lock().remove(&name);
        Ok(())
    }

    /// 连接到端口
    pub fn connect(&self, port_id: PortId, requester: TaskId)
        -> IpcResult<ChannelId>
    {
        let ports = self.ports.lock();
        let port = ports.get(&port_id).ok_or(IpcError::InvalidPortId(port_id))?;

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

        // 创建新通道
        let channel_id = create_channel_pair(port.owner, requester)?;
        port.channels.lock().push(channel_id);
        Ok(channel_id)
    }
}
```

---

## 4. 消息序列化

### 4.1 消息格式

```rust
/// 消息头
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MessageHeader {
    /// 消息类型
    pub msg_type: MessageType,
    /// 发送者任务 ID
    pub sender: TaskId,
    /// 接收者任务 ID
    pub receiver: TaskId,
    /// 消息长度（不含头部）
    pub payload_len: u32,
    /// 消息标志
    pub flags: MessageFlags,
    /// 消息序列号
    pub sequence: u64,
    /// 时间戳
    pub timestamp: u64,
}

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    /// 普通数据消息
    Data       = 0,
    /// 请求消息（期望响应）
    Request    = 1,
    /// 响应消息
    Response   = 2,
    /// 通知消息（不期望响应）
    Notification = 3,
    /// 共享内存描述符
    SharedMem  = 4,
    /// 广播消息
    Broadcast  = 5,
    /// 错误消息
    Error      = 6,
}

/// 消息标志
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MessageFlags: u32 {
        /// 紧急消息（高优先级）
        const URGENT     = 0x01;
        /// 零拷贝消息（payload 为共享内存引用）
        const ZERO_COPY  = 0x02;
        /// 需要确认
        const ACK_REQUIRED = 0x04;
        /// 单向消息
        const ONE_WAY    = 0x08;
    }
}

/// 完整消息
#[derive(Debug, Clone)]
pub struct Message {
    /// 消息头
    pub header: MessageHeader,
    /// 消息负载
    pub payload: Vec<u8>,
}

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
                sequence: 0,
                timestamp: current_time_ns(),
            },
            payload: Vec::new(),
        }
    }

    /// 设置消息负载（使用 bincode 序列化）
    pub fn set_payload<T: Serialize>(&mut self, data: &T) -> IpcResult<()> {
        self.payload = bincode::serialize(data)
            .map_err(|e| IpcError::SerializationFailed {
                reason: e.to_string(),
            })?;
        self.header.payload_len = self.payload.len() as u32;
        Ok(())
    }

    /// 获取消息负载（使用 bincode 反序列化）
    pub fn get_payload<T: DeserializeOwned>(&self) -> IpcResult<T> {
        bincode::deserialize(&self.payload)
            .map_err(|e| IpcError::DeserializationFailed {
                reason: e.to_string(),
            })
    }
}
```

---

## 5. 零拷贝共享内存传输

### 5.1 共享内存通道

```rust
/// 共享内存区域描述符
#[derive(Debug, Clone)]
pub struct SharedMemoryRegion {
    /// 共享内存的物理页帧列表
    pub frames: Vec<Frame>,
    /// 共享内存大小（字节）
    pub size: usize,
    /// 虚拟地址（发送方视角）
    pub sender_vaddr: u64,
    /// 虚拟地址（接收方视角）
    pub receiver_vaddr: u64,
    /// 权限标志
    pub flags: ShmFlags,
}

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

/// 共享内存通道
pub struct SharedMemoryChannel {
    /// 通道 ID
    id: ChannelId,
    /// 共享内存区域
    region: SharedMemoryRegion,
    /// 环形缓冲区头部（位于共享内存起始位置）
    ring_header: *mut ShmRingHeader,
    /// 发送端信号量
    send_sem: AtomicU32,
    /// 接收端信号量
    recv_sem: AtomicU32,
}

/// 共享内存环形缓冲区头部
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShmRingHeader {
    /// 写偏移
    write_offset: AtomicU64,
    /// 读偏移
    read_offset: AtomicU64,
    /// 缓冲区大小
    buffer_size: u64,
    /// 数据起始偏移（头部之后）
    data_offset: u64,
    /// 已关闭标志
    closed: AtomicBool,
}

impl SharedMemoryChannel {
    /// 通过共享内存发送数据（零拷贝）
    ///
    /// 数据直接写入共享内存区域，接收方通过映射读取，
    /// 无需内核态数据拷贝。
    pub fn send_zero_copy(&self, data: &[u8]) -> IpcResult<()> {
        let header = unsafe { &*self.ring_header };

        // 计算可用空间
        let write_pos = header.write_offset.load(Ordering::Acquire);
        let read_pos = header.read_offset.load(Ordering::Acquire);
        let buffer_size = header.buffer_size;
        let data_start = header.data_offset as usize;

        let available = if write_pos >= read_pos {
            buffer_size as usize - (write_pos as usize - read_pos as usize) - 1
        } else {
            read_pos as usize - write_pos as usize - 1
        };

        if data.len() > available {
            return Err(IpcError::ChannelFull);
        }

        // 将数据写入共享内存（零拷贝）
        let shm_ptr = unsafe {
            (self.region.sender_vaddr as *mut u8).add(data_start)
        };

        unsafe {
            // 处理环形回绕
            let write_idx = write_pos as usize % buffer_size as usize;
            if write_idx + data.len() <= buffer_size as usize {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    shm_ptr.add(write_idx),
                    data.len(),
                );
            } else {
                let first_part = buffer_size as usize - write_idx;
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

        // 更新写偏移
        header.write_offset.store(
            write_pos + data.len() as u64,
            Ordering::Release,
        );

        // 通知接收方
        self.send_sem.fetch_add(1, Ordering::Release);
        notify_peer(self.id.receiver);

        Ok(())
    }

    /// 通过共享内存接收数据（零拷贝）
    ///
    /// 返回一个指向共享内存中数据的切片引用，
    /// 避免任何数据拷贝。
    pub fn receive_zero_copy(&self) -> IpcResult<&[u8]> {
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

        // 返回共享内存中的数据引用（零拷贝）
        let shm_ptr = unsafe {
            (self.region.receiver_vaddr as *const u8)
                .add(header.data_offset as usize)
        };

        let read_idx = read_pos as usize % header.buffer_size as usize;

        // 注意：返回的引用生命周期与 &self 绑定
        let data = unsafe {
            if read_idx + available <= header.buffer_size as usize {
                core::slice::from_raw_parts(shm_ptr.add(read_idx), available)
            } else {
                // 环形回绕情况：需要返回两个不连续区域的合并视图
                // 实际实现中可能需要拷贝或使用特殊的数据结构
                let first_part = header.buffer_size as usize - read_idx;
                let second_part = available - first_part;
                // 简化处理：拷贝到连续缓冲区
                // 生产实现应使用 scatter-gather 或 IOV
                unimplemented!("环形回绕零拷贝读取")
            }
        };

        Ok(data)
    }
}
```

---

## 6. 流量控制

### 6.1 基于信用的流量控制

```rust
/// 流量控制器
pub struct FlowController {
    /// 当前可用信用
    pub credits: AtomicI32,
    /// 最大信用值
    pub max_credits: u32,
    /// 等待信用的任务队列
    pub waiters: SpinLock<VecDeque<TaskId>>,
    /// 低水位线（低于此值时触发信用补充）
    pub low_watermark: u32,
    /// 高水位线（高于此值时停止发送）
    pub high_watermark: u32,
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
        }
    }

    /// 消耗一个信用（发送方调用）
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
}
```

---

## 7. 广播与多播

### 7.1 广播机制

```rust
/// 广播组
pub struct BroadcastGroup {
    /// 广播组 ID
    pub id: BroadcastGroupId,
    /// 广播组名称
    pub name: String,
    /// 成员列表
    pub members: RwLock<Vec<BroadcastMember>>,
    /// 消息序列号
    pub sequence: AtomicU64,
    /// 最大成员数
    pub max_members: usize,
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

impl BroadcastGroup {
    /// 向所有成员广播消息
    pub fn broadcast(&self, msg: &Message) -> IpcResult<BroadcastResult> {
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
        let mut members = self.members.write();
        if members.len() >= self.max_members {
            return Err(IpcError::BroadcastGroupFull {
                group: self.id,
                max: self.max_members,
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
```

---

## 8. 错误处理

### 8.1 IPC 错误类型

```rust
/// IPC 错误类型
#[derive(Debug, Clone)]
pub enum IpcError {
    /// 通道已满（异步通道缓冲区溢出）
    ChannelFull,
    /// 通道为空（无可接收消息）
    ChannelEmpty,
    /// 通道已关闭
    ChannelClosed,
    /// 对端已死亡
    PeerDead,
    /// 消息过大
    MessageTooLarge { size: usize, max: usize },
    /// 权限不足
    PermissionDenied { operation: &'static str },
    /// 端口不存在
    PortNotFound { name: String },
    /// 端口已存在
    PortAlreadyExists { name: PortName },
    /// 无效的端口 ID
    InvalidPortId(PortId),
    /// 连接数过多
    TooManyConnections { port: PortId, max: u32 },
    /// 序列化失败
    SerializationFailed { reason: String },
    /// 反序列化失败
    DeserializationFailed { reason: String },
    /// 超时
    Timeout,
    /// 广播组已满
    BroadcastGroupFull { group: BroadcastGroupId, max: usize },
    /// 不在广播组中
    NotInBroadcastGroup { task: TaskId, group: BroadcastGroupId },
    /// 共享内存映射失败
    ShmMapFailed { reason: &'static str },
    /// 无效的通道 ID
    InvalidChannelId(ChannelId),
}

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
            Self::PermissionDenied { operation } => {
                write!(f, "权限不足: 无法执行 '{}'", operation)
            }
            Self::PortNotFound { name } => {
                write!(f, "端口不存在: {}", name)
            }
            Self::PortAlreadyExists { name } => {
                write!(f, "端口已存在: {:?}", name)
            }
            Self::InvalidPortId(id) => write!(f, "无效的端口 ID: {:?}", id),
            Self::TooManyConnections { port, max } => {
                write!(f, "连接数过多: 端口 {:?} 最大连接数 {}", port, max)
            }
            Self::SerializationFailed { reason } => {
                write!(f, "序列化失败: {}", reason)
            }
            Self::DeserializationFailed { reason } => {
                write!(f, "反序列化失败: {}", reason)
            }
            Self::Timeout => write!(f, "操作超时"),
            Self::BroadcastGroupFull { group, max } => {
                write!(f, "广播组已满: {:?} 最大成员数 {}", group, max)
            }
            Self::NotInBroadcastGroup { task, group } => {
                write!(f, "任务 {:?} 不在广播组 {:?} 中", task, group)
            }
            Self::ShmMapFailed { reason } => {
                write!(f, "共享内存映射失败: {}", reason)
            }
            Self::InvalidChannelId(id) => write!(f, "无效的通道 ID: {:?}", id),
        }
    }
}

pub type IpcResult<T> = Result<T, IpcError>;
```

---

## 9. 性能目标与优化

### 9.1 性能指标

| 操作 | 目标延迟 | 测量条件 |
|------|----------|----------|
| 同步消息发送+接收 | < 500ns | 核内，64 字节消息 |
| 异步消息发送 | < 200ns | 核内，64 字节消息 |
| 异步消息接收 | < 300ns | 核内，64 字节消息 |
| 零拷贝传输 | < 100ns | 仅指针传递，无数据拷贝 |
| 端口查找 | < 100ns | HashMap 查找 |
| 广播（100 接收者） | < 5μs | 核内，64 字节消息 |
| 跨核消息 | < 2μs | 含 IPI 开销 |
| bincode 序列化 | < 50ns | 256 字节结构体 |

### 9.2 优化策略

1. **批量消息处理**：支持批量发送/接收，减少系统调用次数
2. **无锁环形缓冲区**：异步通道使用 CAS 操作实现无锁并发
3. **内存池**：预分配消息缓冲区，避免运行时分配
4. **CPU 亲和性**：同一通信对的通道尽量绑定到同一核心
5. **惰性反序列化**：消息负载仅在用户请求时才进行反序列化

---

## 10. 安全考虑

### 10.1 安全机制

1. **端口权限检查**：所有端口操作都验证调用者的权限
2. **消息大小限制**：防止恶意任务发送超大消息耗尽内核内存
3. **共享内存权限隔离**：共享内存区域有严格的读写权限控制
4. **信用耗尽保护**：流量控制防止快速发送方淹没慢速接收方
5. **消息来源验证**：接收方可验证消息发送者的身份

```rust
/// 安全审计日志
pub struct IpcAuditLog {
    /// 审计记录环形缓冲区
    entries: RingBuffer<AuditEntry>,
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// 时间戳
    pub timestamp: u64,
    /// 操作类型
    pub operation: IpcOperation,
    /// 源任务 ID
    pub src_task: TaskId,
    /// 目标任务/端口 ID
    pub dst: IpcTarget,
    /// 操作结果
    pub result: IpcResultCode,
}

#[derive(Debug, Clone, Copy)]
pub enum IpcOperation {
    PortRegister,
    PortLookup,
    PortConnect,
    MessageSend,
    MessageReceive,
    ShmCreate,
    ShmMap,
    Broadcast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcResultCode {
    Success,
    PermissionDenied,
    ResourceExhausted,
    InvalidParameter,
    InternalError,
}
```

---

## 11. 测试用例

### 11.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_channel_send_receive() {
        let (sender, receiver) = create_sync_channel().unwrap();
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        sender.send(msg.clone(), 0).unwrap();
        let received = receiver.receive(0).unwrap();
        assert_eq!(received.header.msg_type, MessageType::Data);
    }

    #[test]
    fn test_async_channel_buffered_send() {
        let config = ChannelConfig {
            channel_type: ChannelType::Asynchronous,
            capacity: 10,
            ..Default::default()
        };
        let (sender, receiver) = create_async_channel(config).unwrap();

        for i in 0..10 {
            let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
            sender.send(msg, 0).unwrap();
        }

        // 第 11 条消息应失败（通道满）
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        assert!(matches!(sender.send(msg, 0), Err(IpcError::ChannelFull)));
    }

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
        assert_eq!(restored, data);
    }

    #[test]
    fn test_flow_control_credit_depletion() {
        let fc = FlowController::new(3, 10);
        assert!(fc.consume_credit());
        assert!(fc.consume_credit());
        assert!(fc.consume_credit());
        // 信用耗尽
        assert!(!fc.consume_credit());

        // 归还信用
        fc.return_credit();
        assert!(fc.consume_credit());
    }

    #[test]
    fn test_broadcast_delivery() {
        let group = BroadcastGroup::new("test-group", 100);
        group.join(TaskId(1), None).unwrap();
        group.join(TaskId(2), None).unwrap();
        group.join(TaskId(3), None).unwrap();

        let msg = Message::new(MessageType::Notification, TaskId(0), TaskId(0));
        let result = group.broadcast(&msg).unwrap();
        assert_eq!(result.delivered, 3);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_broadcast_with_filter() {
        let group = BroadcastGroup::new("filtered-group", 100);
        group.join(TaskId(1), Some(MessageFilter::only_type(MessageType::Data))).unwrap();
        group.join(TaskId(2), None).unwrap();

        let msg = Message::new(MessageType::Notification, TaskId(0), TaskId(0));
        let result = group.broadcast(&msg).unwrap();
        assert_eq!(result.delivered, 1); // 只有 TaskId(2) 接收
        assert_eq!(result.filtered, 1);  // TaskId(1) 被过滤
    }

    #[test]
    fn test_peer_dead_error() {
        let (sender, receiver) = create_sync_channel().unwrap();
        receiver.close().unwrap();
        let msg = Message::new(MessageType::Data, TaskId(1), TaskId(2));
        let result = sender.send(msg, 0);
        assert!(matches!(result, Err(IpcError::PeerDead)));
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
    }

    #[test]
    fn test_ipc_error_display() {
        let err = IpcError::PermissionDenied { operation: "connect" };
        let msg = format!("{}", err);
        assert!(msg.contains("权限不足"));
    }
}
```

### 11.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    /// 测试：Agent 间高频通信
    ///
    /// 两个 Agent 通过异步通道交换 10000 条消息
    /// 验证无消息丢失且延迟在目标范围内
    #[test]
    fn test_high_frequency_agent_communication() {
        // 创建两个 Agent 任务
        // 建立异步通道
        // 发送 10000 条消息
        // 验证接收数量和延迟
    }

    /// 测试：零拷贝大文件传输
    ///
    /// 通过共享内存通道传输 100MB 数据
    /// 验证吞吐量 > 10 GB/s
    #[test]
    fn test_zero_copy_large_transfer() {
        // 创建共享内存通道
        // 分配 100MB 共享内存
        // 写入数据
        // 验证接收方读取的数据正确
    }

    /// 测试：广播风暴防护
    ///
    /// 模拟恶意任务快速发送广播消息
    /// 验证流量控制有效限制发送速率
    #[test]
    fn test_broadcast_storm_protection() {
        // 创建广播组
        // 以最大速率发送广播
        // 验证信用机制生效
    }

    /// 测试：跨核 IPC 延迟
    ///
    /// 在不同 CPU 核心上的任务之间通信
    /// 验证延迟 < 2μs
    #[test]
    fn test_cross_core_ipc_latency() {
        // 将发送方绑定到 CPU 0
        // 将接收方绑定到 CPU 1
        // 测量消息往返延迟
    }
}
```

---

## 12. 配置参数

```rust
/// IPC 全局配置
pub struct IpcConfig {
    /// 最大通道数
    pub max_channels: u32,
    /// 最大端口数
    pub max_ports: u32,
    /// 最大广播组数
    pub max_broadcast_groups: u32,
    /// 默认异步通道容量
    pub default_async_capacity: u32,
    /// 默认初始信用
    pub default_initial_credits: u32,
    /// 最大共享内存大小（字节）
    pub max_shm_size: usize,
    /// 消息内存池大小
    pub message_pool_size: usize,
    /// 审计日志大小
    pub audit_log_size: usize,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            max_channels: 65536,
            max_ports: 4096,
            max_broadcast_groups: 256,
            default_async_capacity: 256,
            default_initial_credits: 64,
            max_shm_size: 1024 * 1024 * 1024, // 1GB
            message_pool_size: 4096,
            audit_log_size: 1024,
        }
    }
}
```

---

## 13. 附录

### 13.1 与其他 IPC 机制对比

| 特性 | OmniAgent IPC | Linux IPC | seL4 IPC | Zircon IPC |
|------|---------------|-----------|----------|------------|
| 同步消息延迟 | < 500ns | ~1μs | < 200ns | < 500ns |
| 零拷贝 | 支持 | 有限 | 原生支持 | 支持 |
| 广播/多播 | 原生支持 | 不支持 | 不支持 | 不支持 |
| 流量控制 | 信用制 | 无 | 无 | 无 |
| 序列化 | bincode | 无 | 无 | 无 |
| Agent 优化 | 专用 | 无 | 无 | 无 |
