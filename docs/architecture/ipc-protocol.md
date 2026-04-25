# OmniAgent OS IPC 协议规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: L1 架构设计文档
> **目标读者**: 内核开发者、服务开发者、Agent 框架开发者

---

## 1. 文档目的

本文档定义 OmniAgent OS 的进程间通信 (IPC) 协议规范。IPC 是微内核架构的核心机制，所有用户态服务之间的通信、Agent 间协作、驱动与设备管理器之间的交互均通过 IPC 完成。本文档涵盖消息格式、通道类型、零拷贝设计、端口命名、消息路由、流控机制、错误处理、安全模型以及性能目标。

---

## 2. 设计目标

| 目标 | 描述 | 优先级 |
|------|------|--------|
| **低延迟** | 同核 IPC 延迟 < 1μs | P0 |
| **零拷贝** | 大块数据传输零拷贝 | P0 |
| **安全** | Capability-based 端口访问控制 | P0 |
| **可靠性** | 消息不丢失、不重复 | P1 |
| **可扩展** | 支持多服务拓扑路由 | P1 |
| **流控** | 背压机制防止内存溢出 | P1 |
| **简洁** | 固定大小消息头，简单高效 | P2 |

---

## 3. 消息格式

### 3.1 消息结构总览

每条 IPC 消息由**固定大小的消息头**和**可变大小的载荷**组成：

```
┌──────────────────────────────────────────────────────────────┐
│                    IPC 消息结构                               │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │              消息头 (MessageHeader)                     │  │
│  │              固定 64 字节                               │  │
│  │                                                        │  │
│  │  ┌──────────┬──────────┬──────────┬──────────┐        │  │
│  │  │ source_id│ dest_id  │ msg_type │ flags    │        │  │
│  │  │ (8B)     │ (8B)     │ (4B)     │ (4B)     │        │  │
│  │  ├──────────┴──────────┼──────────┼──────────┤        │  │
│  │  │ sequence_num        │ tx_id    │ reserved │        │  │
│  │  │ (8B)                │ (8B)     │ (8B)     │        │  │
│  │  ├─────────────────────┴──────────┴──────────┤        │  │
│  │  │ payload_size        │ payload_fmt          │        │  │
│  │  │ (4B)                │ (4B)                 │        │  │
│  │  ├─────────────────────┴──────────────────────┤        │  │
│  │  │ shared_mem_handle    │ capability_token     │        │  │
│  │  │ (8B)                 │ (8B)                 │        │  │
│  │  └────────────────────────────────────────────┘        │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │              载荷 (Payload)                             │  │
│  │              0 ~ 4096 字节                              │  │
│  │                                                        │  │
│  │  格式由 payload_fmt 字段指定:                           │  │
│  │  - RAW: 原始字节                                       │  │
│  │  - BINCODE: bincode 序列化结构体                       │  │
│  │  - SHARED_MEM_REF: 共享内存引用 (无内联载荷)           │  │
│  │                                                        │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 消息头定义

```rust
/// 消息头 - 固定 64 字节
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct MessageHeader {
    /// 消息源端点 ID
    pub source_id: EndpointId,
    /// 消息目标端点 ID
    pub dest_id: EndpointId,
    /// 消息类型 (服务自定义)
    pub msg_type: u32,
    /// 消息标志
    pub flags: MessageFlags,
    /// 序列号 (单调递增，用于消息排序和去重)
    pub sequence_num: u64,
    /// 事务 ID (用于 RPC 请求-响应匹配)
    pub tx_id: u64,
    /// 保留字段 (对齐填充)
    pub reserved: u64,
    /// 载荷大小 (字节)
    pub payload_size: u32,
    /// 载荷格式
    pub payload_fmt: PayloadFormat,
    /// 共享内存句柄 (如果使用零拷贝路径)
    pub shared_mem_handle: u64,
    /// 能力令牌 (用于端口访问验证)
    pub capability_token: u64,
}

/// 端点标识符
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EndpointId(pub u64);

impl EndpointId {
    /// 创建新端点 ID
    pub fn new(process_id: u32, local_id: u32) -> Self {
        EndpointId(((process_id as u64) << 32) | (local_id as u64))
    }

    /// 获取进程 ID 部分
    pub fn process_id(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// 获取本地端点 ID 部分
    pub fn local_id(&self) -> u32 {
        self.0 as u32
    }
}
```

### 3.3 消息标志

```rust
bitflags::bitflags! {
    /// 消息标志位
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct MessageFlags: u32 {
        /// 请求消息 (RPC 请求)
        const REQUEST        = 1 << 0;
        /// 响应消息 (RPC 响应)
        const RESPONSE       = 1 << 1;
        /// 错误响应
        const ERROR          = 1 << 2;
        /// 单向通知 (无需响应)
        const NOTIFICATION   = 1 << 3;
        /// 使用共享内存传输载荷
        const SHARED_MEM     = 1 << 4;
        /// 紧急消息 (高优先级)
        const URGENT         = 1 << 5;
        /// 需要确认 (可靠传输)
        const ACK_REQUIRED   = 1 << 6;
        /// 已确认
        const ACKED          = 1 << 7;
        /// 批量消息 (包含多条子消息)
        const BATCH          = 1 << 8;
    }
}
```

### 3.4 载荷格式

```rust
/// 载荷格式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PayloadFormat {
    /// 原始字节 (无序列化)
    Raw = 0,
    /// bincode 序列化的结构化数据
    Bincode = 1,
    /// 共享内存引用 (载荷在共享内存中，消息头仅包含引用)
    SharedMemRef = 2,
    /// 文件描述符传递
    FdPass = 3,
    /// 能力传递
    CapabilityPass = 4,
}

/// 最大内联载荷大小
pub const MAX_INLINE_PAYLOAD: usize = 4096;

/// 消息头大小
pub const MESSAGE_HEADER_SIZE: usize = 64;
```

---

## 4. 通道类型

OmniAgent OS 提供三种 IPC 通道类型，适用于不同的通信场景：

### 4.1 通道类型对比

| 特性 | 同步 RPC | 异步消息队列 | 共享内存 |
|------|---------|------------|---------|
| **通信模式** | 请求-响应 | 单向/双向 | 双向读写 |
| **延迟** | 低 (~500ns) | 中 (~800ns) | 极低 (~100ns) |
| **吞吐** | 中 | 高 | 极高 |
| **数据拷贝** | 1 次 (内核缓冲) | 1 次 (内核缓冲) | 0 次 |
| **可靠性** | 保证 | 保证 | 需应用层协议 |
| **适用场景** | 服务调用 | 事件通知 | 大块数据传输 |
| **最大消息** | 4 KB | 4 KB | 1 GB |
| **流控** | 阻塞等待 | 队列深度限制 | 应用层 |
| **排序保证** | FIFO | FIFO | 无 |

### 4.2 同步 RPC 通道

同步 RPC 通道用于请求-响应模式的通信，如服务调用：

```rust
/// 同步 RPC 通道
pub struct RpcChannel {
    /// 通道 ID
    pub id: ChannelId,
    /// 服务端端点
    pub server: EndpointId,
    /// 客户端端点
    pub client: EndpointId,
    /// 通道状态
    pub state: ChannelState,
    /// 超时设置
    pub timeout: Duration,
}

/// RPC 调用状态机
pub enum ChannelState {
    /// 通道已创建，等待连接
    Created,
    /// 已连接，可进行 RPC 调用
    Connected,
    /// 等待响应中
    WaitingResponse { tx_id: u64 },
    /// 通道已关闭
    Closed,
}

/// RPC 调用流程
impl RpcChannel {
    /// 发送 RPC 请求并等待响应
    pub fn call(&mut self, request: &Message) -> Result<Message, IpcError> {
        // 1. 分配事务 ID
        let tx_id = self.next_tx_id();
        // 2. 设置消息标志为 REQUEST
        let mut msg = request.clone();
        msg.header.flags = MessageFlags::REQUEST;
        msg.header.tx_id = tx_id;
        // 3. 发送请求
        self.state = ChannelState::WaitingResponse { tx_id };
        ipc_send(self.client, &msg)?;
        // 4. 等待响应 (阻塞当前线程)
        let response = ipc_recv_with_timeout(self.client, self.timeout)?;
        // 5. 验证事务 ID 匹配
        if response.header.tx_id != tx_id {
            return Err(IpcError::TransactionMismatch);
        }
        // 6. 检查错误标志
        if response.header.flags.contains(MessageFlags::ERROR) {
            return Err(IpcError::RemoteError(response.header.msg_type));
        }
        self.state = ChannelState::Connected;
        Ok(response)
    }
}
```

**RPC 调用时序图**:

```
  客户端 (Agent A)                    服务端 (文件系统服务)
       │                                      │
       │  1. 准备请求消息                      │
       │  ┌──────────────────────┐            │
       │  │ Header:              │            │
       │  │   source = A         │            │
       │  │   dest = FS          │            │
       │  │   flags = REQUEST    │            │
       │  │   tx_id = 42         │            │
       │  │ Payload:             │            │
       │  │   {op: READ, path}   │            │
       │  └──────────────────────┘            │
       │                                      │
       │ ──────── IPC_SEND ────────────────►  │
       │                                      │  2. 接收请求
       │                                      │  3. 处理请求
       │                                      │  4. 准备响应
       │                                      │
       │ ◄──────── IPC_REPLY ──────────────  │
       │  ┌──────────────────────┐            │
       │  │ Header:              │            │
       │  │   source = FS        │            │
       │  │   dest = A           │            │
       │  │   flags = RESPONSE   │            │
       │  │   tx_id = 42         │            │
       │  │ Payload:             │            │
       │  │   {data: [...]}      │            │
       │  └──────────────────────┘            │
       │                                      │
       │  5. 匹配 tx_id，返回结果             │
       ▼                                      ▼
```

### 4.3 异步消息队列通道

异步消息队列用于事件通知、日志记录等不需要立即响应的场景：

```rust
/// 异步消息队列
pub struct MessageQueue {
    /// 队列 ID
    pub id: ChannelId,
    /// 环形缓冲区
    pub buffer: RingBuffer<Message>,
    /// 队列最大深度
    pub max_depth: usize,
    /// 当前深度
    pub depth: AtomicUsize,
    /// 等待的接收者列表
    pub waiters: SpinLock<Vec<ThreadId>>,
}

/// 环形缓冲区实现
pub struct RingBuffer<T> {
    data: Box<[UnsafeCell<MaybeUninit<T>>]>,
    head: AtomicUsize,  // 读指针
    tail: AtomicUsize,  // 写指针
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// 非阻塞入队
    pub fn try_push(&self, item: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) % self.capacity;
        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(item); // 队列满
        }
        unsafe {
            (*self.data[tail].get()).write(item);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    /// 非阻塞出队
    pub fn try_pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);
        if head == self.tail.load(Ordering::Acquire) {
            return None; // 队列空
        }
        let item = unsafe {
            (*self.data[head].get()).assume_init_read()
        };
        self.head.store((head + 1) % self.capacity, Ordering::Release);
        Some(item)
    }
}
```

### 4.4 共享内存通道

共享内存通道用于大块数据（如图像、音频、AI 模型数据）的零拷贝传输：

```rust
/// 共享内存区域
pub struct SharedMemoryRegion {
    /// 区域唯一标识符
    pub id: SharedMemId,
    /// 物理页帧列表
    pub frames: Vec<PhysFrame>,
    /// 区域大小 (字节)
    pub size: usize,
    /// 访问权限
    pub permissions: SharedMemPermissions,
    /// 映射到各进程的虚拟地址
    pub mappings: SpinLock<HashMap<ProcessId, VirtAddr>>,
    /// 引用计数
    pub ref_count: AtomicUsize,
}

/// 共享内存权限
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SharedMemPermissions: u32 {
        const READ    = 1 << 0;
        const WRITE   = 1 << 1;
        const EXECUTE = 1 << 2;
    }
}

/// 共享内存操作流程
pub fn shared_mem_create(size: usize) -> Result<SharedMemHandle, IpcError> {
    // 1. 计算需要的页帧数
    let num_frames = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    // 2. 分配连续物理页帧
    let frames = frame_alloc_contiguous(num_frames)?;
    // 3. 创建共享内存区域
    let region = SharedMemoryRegion::new(frames, size);
    // 4. 返回句柄
    Ok(SharedMemHandle { id: region.id, size })
}

pub fn shared_mem_map(
    handle: SharedMemHandle,
    process: ProcessId,
    permissions: SharedMemPermissions,
) -> Result<VirtAddr, IpcError> {
    // 1. 验证句柄有效性
    let region = get_shared_region(handle.id)?;
    // 2. 在目标进程的页表中映射物理帧
    let virt_addr = map_shared_pages(
        process,
        &region.frames,
        permissions,
    )?;
    // 3. 记录映射关系
    region.mappings.lock().insert(process, virt_addr);
    // 4. 增加引用计数
    region.ref_count.fetch_add(1, Ordering::Relaxed);
    Ok(virt_addr)
}
```

---

## 5. 零拷贝快速路径设计

### 5.1 零拷贝原理

传统 IPC 需要多次数据拷贝：

```
传统 IPC 数据路径:
发送方用户空间 ──拷贝1──► 内核缓冲区 ──拷贝2──► 接收方用户空间
     (2 次拷贝)
```

OmniAgent OS 的零拷贝快速路径：

```
零拷贝 IPC 数据路径:
发送方用户空间 ◄──── 共享内存页 ────► 接收方用户空间
     (0 次拷贝，仅传递引用)
```

### 5.2 零拷贝实现机制

```rust
/// 零拷贝消息发送
pub fn ipc_send_zero_copy(
    sender: EndpointId,
    receiver: EndpointId,
    data_ptr: *const u8,
    data_size: usize,
) -> Result<(), IpcError> {
    // 1. 验证数据指针在发送方地址空间内
    validate_user_pointer(data_ptr, data_size, sender.process_id())?;

    // 2. 查找或创建共享内存区域
    let shm_handle = if data_size <= MAX_INLINE_PAYLOAD {
        // 小消息：使用内联载荷 (1 次拷贝)
        return ipc_send_inline(sender, receiver, data_ptr, data_size);
    } else {
        // 大消息：使用共享内存 (0 次拷贝)
        shared_mem_create(data_size)?
    };

    // 3. 在发送方地址空间中，将数据页重新映射为共享页
    //    (使用 COW 机制，避免修改原始数据)
    let sender_vaddr = VirtAddr::new(data_ptr as u64);
    let page_range = page_range_from_addr(sender_vaddr, data_size);
    remap_pages_as_shared(sender.process_id(), &page_range, &shm_handle)?;

    // 4. 在接收方地址空间中映射相同的物理页
    let recv_vaddr = shared_mem_map(
        shm_handle,
        receiver.process_id(),
        SharedMemPermissions::READ,
    )?;

    // 5. 发送仅包含引用的消息头 (极小，快速传输)
    let header = MessageHeader {
        source_id: sender,
        dest_id: receiver,
        flags: MessageFlags::SHARED_MEM | MessageFlags::NOTIFICATION,
        shared_mem_handle: shm_handle.id.0,
        payload_size: data_size as u32,
        payload_fmt: PayloadFormat::SharedMemRef,
        ..Default::default()
    };

    // 6. 通过内核快速路径传递消息头
    deliver_message_header(header)?;

    Ok(())
}
```

### 5.3 同核零拷贝优化

当发送方和接收方在同一 CPU 核心上时，进一步优化：

```
同核零拷贝快速路径:

1. 发送方调用 ipc_send()
   │
   ▼
2. 内核验证消息头 (无数据拷贝)
   │
   ▼
3. 内核将消息头放入接收方的就绪队列
   │  (仅操作指针，无拷贝)
   ▼
4. 如果接收方正在等待 (阻塞在 ipc_recv())
   │  直接唤醒接收方，跳过调度
   ▼
5. 接收方从就绪队列取出消息头
   │  通过共享内存直接访问数据
   ▼
6. 完成 (总延迟: ~500ns)
```

---

## 6. 端口命名与发现

### 6.1 端口命名空间

```rust
/// 端口名称 (UTF-8 字符串，最长 128 字节)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PortName {
    bytes: [u8; 128],
    len: u8,
}

impl PortName {
    /// 从字符串创建端口名
    pub fn from_str(s: &str) -> Result<Self, IpcError> {
        if s.len() > 128 {
            return Err(IpcError::NameTooLong);
        }
        let mut bytes = [0u8; 128];
        bytes[..s.len()].copy_from_slice(s.as_bytes());
        Ok(PortName { bytes, len: s.len() as u8 })
    }
}

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
```

### 6.2 端口注册与发现

```rust
/// 端口注册表
pub struct PortRegistry {
    /// 端口名到端点的映射
    name_to_endpoint: RwLock<HashMap<PortName, EndpointId>>,
    /// 端点到端口名的映射 (反向查找)
    endpoint_to_name: RwLock<HashMap<EndpointId, PortName>>,
    /// 端口属性
    port_attributes: RwLock<HashMap<EndpointId, PortAttributes>>,
}

/// 端口属性
pub struct PortAttributes {
    /// 端口类型
    pub port_type: PortType,
    /// 最大并发连接数
    pub max_connections: u32,
    /// 当前连接数
    pub current_connections: AtomicU32,
    /// 消息队列深度
    pub queue_depth: u32,
    /// 是否允许多播
    pub multicast: bool,
}

/// 端口类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    /// 单播端口 (点对点)
    Unicast,
    /// 多播端口 (一对多)
    Multicast,
    /// 发布-订阅端口
    PublishSubscribe,
}

/// 注册端口
pub fn port_register(
    name: PortName,
    endpoint: EndpointId,
    attributes: PortAttributes,
) -> Result<(), IpcError> {
    let registry = PORT_REGISTRY.get().unwrap();
    let mut name_map = registry.name_to_endpoint.write();
    let mut attr_map = registry.port_attributes.write();

    // 检查名称是否已被注册
    if name_map.contains_key(&name) {
        return Err(IpcError::PortAlreadyExists);
    }

    name_map.insert(name.clone(), endpoint);
    attr_map.insert(endpoint, attributes);
    Ok(())
}

/// 发现端口
pub fn port_discover(name: &str) -> Result<EndpointId, IpcError> {
    let port_name = PortName::from_str(name)?;
    let registry = PORT_REGISTRY.get().unwrap();
    let name_map = registry.name_to_endpoint.read();

    name_map
        .get(&port_name)
        .copied()
        .ok_or(IpcError::PortNotFound)
}
```

---

## 7. 消息路由

### 7.1 多服务拓扑路由

```
┌─────────────────────────────────────────────────────────────────┐
│                      消息路由拓扑                                │
│                                                                 │
│  ┌──────────┐     ┌──────────┐     ┌──────────┐                │
│  │ Agent A  │     │ Agent B  │     │ Agent C  │                │
│  └────┬─────┘     └────┬─────┘     └────┬─────┘                │
│       │                │                │                       │
│       │    ┌───────────▼───────────┐    │                       │
│       └───►│    IPC 路由器         │◄───┘                       │
│           │    (内核态)            │                            │
│           │                        │                            │
│           │  路由规则:             │                            │
│           │  - 名称查找            │                            │
│           │  - 能力验证            │                            │
│           │  - 负载均衡            │                            │
│           │  - 优先级队列          │                            │
│           └───┬────┬────┬────┬────┘                            │
│               │    │    │    │                                  │
│       ┌───────▼┐ ┌─▼────▼┐ ┌▼──────────┐                      │
│       │ FS 服务 │ │ AI 服务│ │ 网络服务  │                      │
│       └────────┘ └───────┘ └───────────┘                      │
│                                                                 │
│  路由模式:                                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ 1. 直接路由: Agent A → FS 服务 (名称查找)               │   │
│  │ 2. 多播路由: Agent A → [Agent B, Agent C]              │   │
│  │ 3. 代理路由: Agent A → AI Router → 本地/云端           │   │
│  │ 4. 链式路由: Agent A → FS → 网络 → 远程存储            │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 7.2 路由决策流程

```rust
/// 消息路由器
pub struct MessageRouter {
    /// 路由表
    route_table: RwLock<RouteTable>,
    /// 端口注册表引用
    port_registry: &'static PortRegistry,
}

impl MessageRouter {
    /// 路由消息到目标端点
    pub fn route_message(&self, message: &Message) -> Result<RouteDecision, IpcError> {
        let dest = message.header.dest_id;

        // 1. 检查目标是否为本地端点
        if self.is_local_endpoint(dest) {
            return Ok(RouteDecision::Local { endpoint: dest });
        }

        // 2. 检查目标是否为命名端口
        if let Some(endpoint) = self.resolve_named_port(&message.header)? {
            return Ok(RouteDecision::Local { endpoint });
        }

        // 3. 检查是否为多播目标
        if let Some(recipients) = self.resolve_multicast(dest)? {
            return Ok(RouteDecision::Multicast { endpoints: recipients });
        }

        // 4. 检查是否需要远程路由
        if let Some(remote_node) = self.resolve_remote(dest)? {
            return Ok(RouteDecision::Remote { node: remote_node });
        }

        Err(IpcError::NoRouteToDestination)
    }
}

pub enum RouteDecision {
    /// 本地端点直接投递
    Local { endpoint: EndpointId },
    /// 多播投递到多个端点
    Multicast { endpoints: Vec<EndpointId> },
    /// 远程节点路由
    Remote { node: NodeId },
}
```

---

## 8. 流量控制与背压

### 8.1 背压机制

当接收方处理速度跟不上发送方时，系统通过背压机制防止内存溢出：

```rust
/// 流量控制器
pub struct FlowController {
    /// 每个端点的流量状态
    endpoint_state: RwLock<HashMap<EndpointId, FlowState>>,
    /// 系统级流量限制
    system_limits: SystemFlowLimits,
}

/// 端点流量状态
pub struct FlowState {
    /// 当前队列深度
    pub queue_depth: AtomicU32,
    /// 最大队列深度 (高水位)
    pub max_depth: u32,
    /// 低水位 (解除背压的阈值)
    pub low_watermark: u32,
    /// 当前是否处于背压状态
    pub backpressure: AtomicBool,
    /// 最近的吞吐测量
    pub throughput: AtomicU64,  // bytes/sec
}

/// 系统级流量限制
pub struct SystemFlowLimits {
    /// 系统总 IPC 带宽限制 (bytes/sec)
    pub max_system_bandwidth: u64,
    /// 单 Agent 最大 IPC 带宽 (bytes/sec)
    pub max_agent_bandwidth: u64,
    /// 单 Agent 最大消息队列深度
    pub max_agent_queue_depth: u32,
    /// 系统总消息队列深度
    pub max_system_queue_depth: u32,
}

/// 发送消息时的流控检查
pub fn flow_control_check(sender: EndpointId, receiver: EndpointId, size: u32) -> Result<(), IpcError> {
    let controller = FLOW_CONTROLLER.get().unwrap();
    let state = controller.get_flow_state(receiver)?;

    // 1. 检查接收方队列是否已满 (背压)
    let current_depth = state.queue_depth.load(Ordering::Acquire);
    if current_depth >= state.max_depth {
        // 背压生效：拒绝发送或阻塞
        return Err(IpcError::Backpressure {
            queue_depth: current_depth,
            max_depth: state.max_depth,
        });
    }

    // 2. 检查发送方带宽配额
    let sender_quota = controller.get_agent_bandwidth(sender.process_id())?;
    if !sender_quota.try_consume(size as u64) {
        return Err(IpcError::BandwidthExceeded);
    }

    // 3. 检查系统级限制
    let system_usage = controller.system_bandwidth_usage();
    if system_usage + size as u64 > controller.system_limits.max_system_bandwidth {
        return Err(IpcError::SystemBandwidthExceeded);
    }

    // 4. 更新队列深度
    state.queue_depth.fetch_add(1, Ordering::Release);

    Ok(())
}
```

### 8.2 背压状态机

```
                    正常状态
                 ┌──────────────┐
                 │              │
    队列深度 < 高水位  │   Normal     │
    ───────────────►│              │
                 └──────┬───────┘
                        │
                        │ 队列深度 >= 高水位
                        ▼
                 ┌──────────────┐
                 │              │
                 │  Backpressure│
                 │  (背压生效)   │
                 │              │
                 └──────┬───────┘
                        │
                        │ 队列深度 <= 低水位
                        ▼
                 ┌──────────────┐
                 │              │
    队列深度 <= 低水位  │   Normal     │
    ───────────────►│              │
                 └──────────────┘

示例配置:
  高水位 (max_depth) = 1024 条消息
  低水位 (low_watermark) = 256 条消息
  背压时: 发送方收到 EAGAIN 或阻塞等待
  解除背压: 接收方处理到低水位后通知发送方
```

---

## 9. 错误码定义

### 9.1 IPC 操作错误码

```rust
/// IPC 错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    /// 成功
    Ok = 0,
    /// 参数无效
    InvalidArgument = 1,
    /// 端口不存在
    PortNotFound = 2,
    /// 端口已存在
    PortAlreadyExists = 3,
    /// 端口名称过长
    NameTooLong = 4,
    /// 消息过大
    MessageTooLarge = 5,
    /// 消息队列已满 (背压)
    QueueFull = 6,
    /// 消息队列已空
    QueueEmpty = 7,
    /// 连接被拒绝
    ConnectionRefused = 8,
    /// 连接超时
    ConnectionTimeout = 9,
    /// 发送超时
    SendTimeout = 10,
    /// 接收超时
    RecvTimeout = 11,
    /// 事务 ID 不匹配
    TransactionMismatch = 12,
    /// 远程错误
    RemoteError(u32) = 13,
    /// 无路由到目标
    NoRouteToDestination = 14,
    /// 权限不足 (Capability 验证失败)
    PermissionDenied = 15,
    /// 能力令牌无效
    InvalidCapability = 16,
    /// 能力已过期
    CapabilityExpired = 17,
    /// 共享内存错误
    SharedMemError = 18,
    /// 共享内存映射失败
    MapFailed = 19,
    /// 带宽超限
    BandwidthExceeded = 20,
    /// 系统带宽超限
    SystemBandwidthExceeded = 21,
    /// 背压 (接收方处理不过来)
    Backpressure {
        queue_depth: u32,
        max_depth: u32,
    } = 22,
    /// 通道已关闭
    ChannelClosed = 23,
    /// 端点无效
    InvalidEndpoint = 24,
    /// 序列化错误
    SerializationError = 25,
    /// 反序列化错误
    DeserializationError = 26,
    /// 内部错误
    InternalError = 27,
}

impl IpcError {
    /// 转换为内核错误码
    pub fn to_kernel_error(&self) -> KernelError {
        match self {
            IpcError::PermissionDenied => KernelError::PermissionDenied,
            IpcError::InvalidArgument => KernelError::InvalidArgument,
            IpcError::InvalidCapability => KernelError::PermissionDenied,
            IpcError::Timeout => KernelError::Timeout,
            _ => KernelError::InternalError,
        }
    }

    /// 转换为 POSIX errno
    pub fn to_errno(&self) -> i32 {
        match self {
            IpcError::Ok => 0,
            IpcError::InvalidArgument => 22,    // EINVAL
            IpcError::PortNotFound => 2,        // ENOENT
            IpcError::QueueFull => 6,           // ENXIO
            IpcError::QueueEmpty => 6,          // ENXIO
            IpcError::ConnectionRefused => 111, // ECONNREFUSED
            IpcError::ConnectionTimeout => 110, // ETIMEDOUT
            IpcError::SendTimeout => 110,       // ETIMEDOUT
            IpcError::RecvTimeout => 110,       // ETIMEDOUT
            IpcError::PermissionDenied => 13,   // EACCES
            IpcError::ChannelClosed => 32,      // EPIPE
            IpcError::MessageTooLarge => 7,     // E2BIG
            IpcError::Backpressure { .. } => 11,// EAGAIN
            IpcError::BandwidthExceeded => 55,  // ENOBUFS
            _ => 5,                             // EIO
        }
    }
}
```

---

## 10. 安全模型

### 10.1 Capability-Based 端口访问控制

```rust
/// 端口访问能力
#[derive(Clone, Copy, Debug)]
pub struct PortCapability {
    /// 能力唯一标识符
    pub id: CapabilityId,
    /// 目标端口
    pub port: PortName,
    /// 允许的操作
    pub permissions: PortPermissions,
    /// 能力持有者 (Agent ID)
    pub holder: AgentId,
    /// 能力签发者
    pub issuer: AgentId,
    /// 创建时间
    pub created_at: u64,
    /// 过期时间 (0 表示永不过期)
    pub expires_at: u64,
    /// 使用次数限制 (0 表示无限制)
    pub max_uses: u32,
    /// 已使用次数
    pub used_count: AtomicU32,
    /// 能力签名 (防伪造)
    pub signature: [u8; 32],
}

bitflags::bitflags! {
    /// 端口权限
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PortPermissions: u32 {
        /// 发送消息
        const SEND       = 1 << 0;
        /// 接收消息
        const RECV       = 1 << 1;
        /// 连接端口
        const CONNECT    = 1 << 2;
        /// 监听端口
        const LISTEN     = 1 << 3;
        /// 转授能力
        const GRANT      = 1 << 4;
        /// 委托能力
        const DELEGATE   = 1 << 5;
        /// 管理端口 (修改属性)
        const ADMIN      = 1 << 6;
    }
}

/// 验证端口访问能力
pub fn verify_port_access(
    agent: AgentId,
    port: &PortName,
    required_permission: PortPermissions,
) -> Result<PortCapability, IpcError> {
    // 1. 查找 Agent 持有的该端口能力
    let cap = find_capability(agent, port)?;

    // 2. 检查权限是否满足
    if !cap.permissions.contains(required_permission) {
        return Err(IpcError::PermissionDenied);
    }

    // 3. 检查是否过期
    let now = current_timestamp();
    if cap.expires_at != 0 && now > cap.expires_at {
        revoke_capability(cap.id);
        return Err(IpcError::CapabilityExpired);
    }

    // 4. 检查使用次数
    if cap.max_uses != 0 {
        let used = cap.used_count.fetch_add(1, Ordering::Relaxed);
        if used >= cap.max_uses {
            revoke_capability(cap.id);
            return Err(IpcError::InvalidCapability);
        }
    }

    // 5. 验证签名
    if !verify_capability_signature(&cap) {
        return Err(IpcError::InvalidCapability);
    }

    Ok(cap)
}
```

### 10.2 消息安全

| 安全措施 | 描述 |
|---------|------|
| **Capability 验证** | 每次发送消息前验证发送方的端口 Capability |
| **消息完整性** | 可选的消息认证码 (MAC)，防止消息篡改 |
| **消息加密** | 敏感 Agent 间通信可启用端到端加密 |
| **消息大小限制** | 单条消息最大 4 KB 内联载荷，防止内存耗尽 |
| **速率限制** | 每个 Agent 有 IPC 带宽配额，防止 DoS |
| **来源验证** | 内核保证消息头中的 source_id 不可伪造 |

---

## 11. 序列化格式

### 11.1 bincode 结构化序列化

```rust
use serde::{Serialize, Deserialize};

/// 文件系统操作请求 (示例)
#[derive(Serialize, Deserialize, Debug)]
pub struct FsRequest {
    /// 操作类型
    pub operation: FsOperation,
    /// 文件路径
    pub path: String,
    /// 打开标志
    pub flags: u32,
    /// 文件模式
    pub mode: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FsOperation {
    Open,
    Close,
    Read { offset: u64, size: u32 },
    Write { offset: u64 },
    Stat,
    List,
    Delete,
}

/// 序列化/反序列化
pub fn serialize_payload<T: Serialize>(data: &T) -> Result<Vec<u8>, IpcError> {
    bincode::serialize(data).map_err(|_| IpcError::SerializationError)
}

pub fn deserialize_payload<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<T, IpcError> {
    bincode::deserialize(bytes).map_err(|_| IpcError::DeserializationError)
}
```

### 11.2 原始字节传输

对于已格式化的数据（如图像帧、音频缓冲区），直接使用原始字节传输：

```rust
/// 原始字节传输
pub fn send_raw_bytes(
    dest: EndpointId,
    data: &[u8],
) -> Result<(), IpcError> {
    if data.len() <= MAX_INLINE_PAYLOAD {
        // 小数据：内联传输
        let mut message = Message::new_inline(dest);
        message.header.payload_fmt = PayloadFormat::Raw;
        message.payload[..data.len()].copy_from_slice(data);
        message.header.payload_size = data.len() as u32;
        ipc_send_inline(message)
    } else {
        // 大数据：共享内存零拷贝
        ipc_send_zero_copy(
            current_endpoint(),
            dest,
            data.as_ptr(),
            data.len(),
        )
    }
}
```

---

## 12. 性能目标与优化

### 12.1 性能基准

| 场景 | 目标延迟 | 测量方法 |
|------|---------|---------|
| 同核同步 RPC (空消息) | < 500ns | TSC 循环测试 |
| 同核同步 RPC (4KB) | < 1μs | TSC 循环测试 |
| 跨核同步 RPC (空消息) | < 5μs | TSC + IPI |
| 异步消息投递 | < 800ns | TSC 循环测试 |
| 共享内存写入 | < 100ns | 直接内存访问 |
| 端口名称查找 | < 200ns | HashMap 查找 |
| 消息序列化 (bincode) | < 500ns | 1KB 结构体 |
| 消息反序列化 (bincode) | < 500ns | 1KB 结构体 |
| 零拷贝建立 (首次) | < 10μs | 页表映射 |
| 零拷贝建立 (后续) | < 100ns | 引用传递 |

### 12.2 性能优化策略

| 优化 | 描述 | 预期收益 |
|------|------|---------|
| **快速路径** | 同核、小消息跳过完整路由 | 延迟降低 40% |
| **批量发送** | `IPC_SEND_BATCH` 合并多条消息 | 吞吐提升 3x |
| **无锁队列** | 每核本地消息队列使用无锁设计 | 延迟降低 20% |
| **缓存友好** | 消息头 64 字节 = 1 条缓存行 | 减少缓存未命中 |
| **预分配** | 消息结构预分配，减少运行时分配 | 延迟降低 15% |
| **NUMA 感知** | 优先同 NUMA 节点通信 | 跨节点延迟降低 30% |

### 12.3 性能测试用例

```rust
#[cfg(test)]
mod ipc_perf_tests {
    use super::*;

    /// 基准测试: 同核空消息 RPC
    #[bench]
    fn bench_empty_rpc_same_core(b: &mut Bencher) {
        let (client, server) = create_rpc_channel_pair();
        b.iter(|| {
            let req = Message::new_empty();
            let resp = client.call(req).unwrap();
            assert!(resp.header.flags.contains(MessageFlags::RESPONSE));
        });
    }

    /// 基准测试: 4KB 载荷 RPC
    #[bench]
    fn bench_4kb_rpc_same_core(b: &mut Bencher) {
        let (client, server) = create_rpc_channel_pair();
        let payload = vec![0xABu8; 4096];
        b.iter(|| {
            let req = Message::new_with_payload(&payload);
            let resp = client.call(req).unwrap();
        });
    }

    /// 基准测试: 零拷贝大块传输
    #[bench]
    fn bench_zero_copy_large(b: &mut Bencher) {
        let data = vec![0u8; 1024 * 1024]; // 1 MB
        b.iter(|| {
            ipc_send_zero_copy(
                sender_endpoint(),
                receiver_endpoint(),
                data.as_ptr(),
                data.len(),
            ).unwrap();
        });
    }

    /// 基准测试: 共享内存吞吐
    #[bench]
    fn bench_shared_mem_throughput(b: &mut Bencher) {
        let region = create_shared_region(1024 * 1024); // 1 MB
        let ptr = region.as_ptr();
        b.iter(|| {
            unsafe {
                // 模拟写入
                core::ptr::write_volatile(ptr as *mut u8, 0xFF);
                // 内存屏障
                core::sync::atomic::fence(Ordering::SeqCst);
            }
        });
    }
}
```

---

## 13. 测试用例

### 13.1 功能测试

```rust
#[cfg(test)]
mod ipc_tests {
    use super::*;

    /// 测试: 基本消息发送和接收
    #[test]
    fn test_basic_send_recv() {
        let (sender, receiver) = create_endpoint_pair();
        let msg = create_test_message("hello");
        ipc_send(sender, &msg).unwrap();
        let received = ipc_recv(receiver).unwrap();
        assert_eq!(received.header.source_id, sender);
        assert_eq!(received.payload_as_str(), "hello");
    }

    /// 测试: RPC 请求-响应匹配
    #[test]
    fn test_rpc_transaction_matching() {
        let (client, server) = create_rpc_channel_pair();
        // 并发发送多个 RPC 请求
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let ch = client.clone();
                spawn(move || {
                    let req = create_rpc_request(i);
                    let resp = ch.call(req).unwrap();
                    assert_eq!(resp.header.tx_id, req.header.tx_id);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    /// 测试: 消息队列背压
    #[test]
    fn test_backpressure() {
        let sender = create_test_endpoint();
        let receiver = create_limited_endpoint(5); // 队列深度 5

        // 前 5 条应该成功
        for _ in 0..5 {
            ipc_send(sender, &create_test_message("data")).unwrap();
        }

        // 第 6 条应该触发背压
        let result = ipc_send_nonblocking(sender, &create_test_message("overflow"));
        assert_eq!(result, Err(IpcError::QueueFull));
    }

    /// 测试: 端口 Capability 验证
    #[test]
    fn test_capability_verification() {
        let agent = create_test_agent();
        let port = PortName::from_str("test.service").unwrap();

        // 无能力时应拒绝
        let result = verify_port_access(agent.id(), &port, PortPermissions::SEND);
        assert_eq!(result, Err(IpcError::PermissionDenied));

        // 授予能力后应成功
        grant_capability(agent.id(), &port, PortPermissions::SEND);
        let result = verify_port_access(agent.id(), &port, PortPermissions::SEND);
        assert!(result.is_ok());
    }

    /// 测试: 共享内存映射
    #[test]
    fn test_shared_memory() {
        let handle = shared_mem_create(PAGE_SIZE * 4).unwrap();
        let vaddr1 = shared_mem_map(handle.clone(), process_a(), READ_WRITE).unwrap();
        let vaddr2 = shared_mem_map(handle.clone(), process_b(), READ_WRITE).unwrap();

        // 进程 A 写入
        unsafe { *(vaddr1.as_mut_ptr() as *mut u64) = 0xDEADBEEF };

        // 进程 B 应该能看到
        let value = unsafe { *(vaddr2.as_ptr() as *const u64) };
        assert_eq!(value, 0xDEADBEEF);
    }

    /// 测试: 序列化/反序列化
    #[test]
    fn test_serialization() {
        let request = FsRequest {
            operation: FsOperation::Read { offset: 1024, size: 4096 },
            path: "/test/file.txt".to_string(),
            flags: 0,
            mode: 0o644,
        };
        let bytes = serialize_payload(&request).unwrap();
        let decoded: FsRequest = deserialize_payload(&bytes).unwrap();
        assert_eq!(decoded.path, "/test/file.txt");
    }
}
```

### 13.2 边界条件测试

| 测试场景 | 预期行为 |
|---------|---------|
| 发送 0 字节消息 | 成功，空载荷 |
| 发送 4 KB 消息 | 成功，内联传输 |
| 发送 4 KB + 1 消息 | 自动切换共享内存 |
| 发送到不存在的端点 | 返回 `PortNotFound` |
| 发送到已关闭的通道 | 返回 `ChannelClosed` |
| RPC 超时 | 返回 `RecvTimeout` |
| 无权限发送 | 返回 `PermissionDenied` |
| 共享内存映射越界 | 返回 `MapFailed` |
| 端口名 129 字节 | 返回 `NameTooLong` |
| 事务 ID 溢出 | 自动回绕，不影响匹配 |

---

*本文档由 OmniAgent OS IPC 团队维护，如有疑问请联系 ipc@omniagent.os*
