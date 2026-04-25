# OmniAgent OS Agent Syscall ABI 规范

> **文档编号**: OA-ARCH-005
> **版本**: 1.0.0
> **状态**: 稳定 (Stable)
> **最后更新**: 2026-04-25
> **作者**: OmniAgent OS 架构组

---

## 1. 概述

### 1.1 文档目的

本文档定义了 OmniAgent OS 的 Agent 系统调用二进制接口 (ABI)，是内核与用户态 Agent 之间的核心契约。所有 Agent 运行时、Agent 开发框架以及内核态 Agent 管理器都必须严格遵守本规范。

### 1.2 适用范围

- Agent 运行时 (Agent Runtime)
- Agent 开发 SDK
- 内核 Agent 子系统
- 安全审计模块
- 虚拟化层 Agent 接口

### 1.3 术语定义

| 术语 | 定义 |
|------|------|
| Agent | OmniAgent OS 中的一等公民计算实体，拥有独立地址空间和调度身份 |
| Syscall | 用户态到内核态的受控入口点 |
| Capability | 不可伪造的安全令牌，代表特定权限 |
| AgentSpec | 描述 Agent 启动配置的结构体 |
| Handle | 内核分配的 Agent 标识符，不透明于用户态 |

### 1.4 设计原则

1. **最小权限**: 每个 syscall 仅暴露完成其功能所需的最小接口
2. **零拷贝优先**: 消息传递优先使用共享内存，减少数据拷贝
3. **向前兼容**: 新增 syscall 不影响已有 syscall 的行为
4. **可审计**: 所有 Agent 操作可通过审计日志追踪

---

## 2. 系统调用约定

### 2.1 指令与寄存器映射

OmniAgent OS 使用 x86_64 `syscall` 指令进行系统调用，遵循 System V AMD64 ABI 的变体约定：

```
┌─────────────────────────────────────────────────────────┐
│                  Syscall 寄存器映射                       │
├──────────┬───────────────────────────────────────────────┤
│  寄存器   │  用途                                        │
├──────────┼───────────────────────────────────────────────┤
│   rax    │  系统调用号 (Syscall Number)                   │
│   rdi    │  第 1 参数 (Arg1)                             │
│   rsi    │  第 2 参数 (Arg2)                             │
│   rdx    │  第 3 参数 (Arg3)                             │
│   r10    │  第 4 参数 (Arg4)                             │
│   r8     │  第 5 参数 (Arg5)                             │
│   r9     │  第 6 参数 (Arg6)                             │
│   rcx    │  返回地址 (由 syscall 指令自动填充)             │
│   r11    │  RFLAGS (由 syscall 指令自动保存)              │
│   rax    │  返回值 (系统调用完成后)                        │
├──────────┴───────────────────────────────────────────────┤
│  注意: r10 用作第 4 参数而非 rcx，因为 syscall 指令       │
│  会覆盖 rcx 为返回地址。这与 Linux x86_64 约定一致。       │
└─────────────────────────────────────────────────────────┘
```

### 2.2 返回值约定

| 返回值范围 | 含义 |
|-----------|------|
| `0..=4095` | 成功，值为结果或零 |
| `-4095..=-1` | 错误，取绝对值对应 errno |
| `>4095` | 成功，值为指针或句柄 (高半部分) |

### 2.3 栈帧要求

```
用户态栈布局 (syscall 前):
┌─────────────────────┐  ← 高地址
│  参数 7+ (栈传递)    │
│  ...                │
│  参数 8             │
├─────────────────────┤
│  返回地址            │
├─────────────────────┤  ← rsp (16 字节对齐)
│  (保留区域)          │
└─────────────────────┘  ← 低地址
```

### 2.4 调用示例 (Rust 内联汇编)

```rust
#[inline(always)]
unsafe fn syscall6(
    nr: usize,
    a1: usize, a2: usize, a3: usize,
    a4: usize, a5: usize, a6: usize,
) -> isize {
    let ret: isize;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr as isize => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8")  a5,
        in("r9")  a6,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}
```

---

## 3. 系统调用号分配

### 3.1 传统系统调用 (0-511)

| 编号 | 名称 | 功能描述 | 参数 |
|------|------|---------|------|
| 0 | `SYS_READ` | 从文件描述符读取 | fd, buf, count |
| 1 | `SYS_WRITE` | 写入文件描述符 | fd, buf, count |
| 2 | `SYS_OPEN` | 打开文件 | path, flags, mode |
| 3 | `SYS_CLOSE` | 关闭文件描述符 | fd |
| 4 | `SYS_STAT` | 获取文件状态 | path, statbuf |
| 5 | `SYS_FSTAT` | 获取 fd 状态 | fd, statbuf |
| 6 | `SYS_LSTAT` | 获取链接状态 | path, statbuf |
| 7 | `SYS_POLL` | I/O 多路复用 | fds, nfds, timeout |
| 8 | `SYS_LSEEK` | 文件偏移定位 | fd, offset, whence |
| 9 | `SYS_MMAP` | 内存映射 | addr, len, prot, flags, fd, off |
| 10 | `SYS_MUNMAP` | 解除内存映射 | addr, len |
| 11 | `SYS_MPROTECT` | 修改内存保护 | addr, len, prot |
| 12 | `SYS_BRK` | 设置堆断点 | brk |
| 16 | `SYS_IOCTL` | 设备控制 | fd, request, argp |
| 20 | `SYS_WRITEV` | 分散写入 | fd, iov, iovcnt |
| 21 | `SYS_READV` | 分散读取 | fd, iov, iovcnt |
| 28 | `SYS_MADVISE` | 内存使用建议 | addr, len, advice |
| 39 | `SYS_GETPID` | 获取进程 ID | - |
| 57 | `SYS_FORK` | 创建子进程 | - |
| 59 | `SYS_EXECVE` | 执行程序 | filename, argv, envp |
| 60 | `SYS_EXIT` | 终止进程 | status |
| 96 | `SYS_SET_TID_ADDRESS` | 设置线程 ID 地址 | tidptr |
| 131 | `SYS_SIGACTION` | 设置信号处理 | sig, act, oldact |
| 202 | `SYS_FUTEX` | 快速用户空间互斥 | uaddr, op, val, timeout |
| 228 | `SYS_CLOCK_GETTIME` | 获取时钟时间 | clkid, tp |
| 260 | `SYS_WAIT4` | 等待子进程 | pid, status, options |
| 318 | `SYS_GETRANDOM` | 获取随机数 | buf, count, flags |
| 334 | `SYS_RSEQ` | 可重启序列 | rseq, rseq_len, flags |

### 3.2 Agent 系统调用 (512+)

| 编号 | 名称 | 功能描述 | 参数数量 |
|------|------|---------|---------|
| 512 | `SYS_AGENT_SPAWN` | 创建新 Agent | 3 |
| 513 | `SYS_AGENT_KILL` | 终止 Agent | 2 |
| 514 | `SYS_AGENT_QUERY` | 查询 Agent 状态 | 3 |
| 515 | `SYS_AGENT_MSG` | 向 Agent 发送消息 | 4 |
| 516 | `SYS_AGENT_REGISTER` | 注册 Agent 能力 | 3 |
| 517 | `SYS_AGENT_SUBSCRIBE` | 订阅 Agent 事件 | 3 |
| 518 | `SYS_AGENT_MIGRATE` | 迁移 Agent 到其他设备 | 4 |
| 519 | `SYS_AGENT_MEMORY_SHARE` | 与 Agent 共享内存区域 | 4 |
| 520 | `SYS_AGENT_CAP_GRANT` | 授予 Agent 能力 | 4 |
| 521 | `SYS_AGENT_CAP_REVOKE` | 撤销 Agent 能力 | 3 |
| 522 | `SYS_AGENT_BIND_PORT` | 绑定 Agent 通信端口 | 3 |
| 523 | `SYS_AGENT_EXPORT` | 导出 Agent 服务 | 3 |
| 524 | `SYS_AGENT_IMPORT` | 导入远程 Agent 服务 | 3 |
| 525 | `SYS_AGENT_SET_QUOTA` | 设置 Agent 资源配额 | 3 |
| 526 | `SYS_AGENT_GET_QUOTA` | 获取 Agent 资源配额 | 2 |
| 527 | `SYS_AGENT_SNAPSHOT` | 创建 Agent 快照 | 3 |
| 528 | `SYS_AGENT_RESTORE` | 从快照恢复 Agent | 3 |

> **保留范围**: 529-599 保留给未来 Agent 扩展。600-767 保留给虚拟化相关 syscall。768-1023 保留给安全 enclave syscall。

---

## 4. Agent 系统调用详细规范

### 4.1 SYS_AGENT_SPAWN (512)

#### 4.1.1 功能描述

创建一个新的 Agent 实例。内核根据 `AgentSpec` 配置分配资源、建立地址空间、注册能力，并返回 Agent 句柄。

#### 4.1.2 函数签名

```rust
/// 创建新 Agent
///
/// # 参数
/// - `spec_ptr`: 指向 AgentSpec 结构体的用户态指针
/// - `spec_len`: AgentSpec 结构体的字节长度 (用于版本兼容性检查)
/// - `cap_slot`: 用于接收新 Agent 句柄的能力槽位索引
///
/// # 返回值
/// - 成功: 返回 Agent Handle (>= 0)
/// - 失败: 返回负的 errno 值
fn sys_agent_spawn(spec_ptr: *const AgentSpec, spec_len: usize, cap_slot: u32) -> isize;
```

#### 4.1.3 AgentSpec 结构体定义

```rust
/// Agent 规范结构体 - 描述 Agent 的完整启动配置
///
/// 版本 1 对应 spec_len = 256 字节
/// 结构体使用固定大小字段，确保 ABI 稳定性
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AgentSpec {
    /// 结构体版本号，当前为 1
    pub version: u32,

    /// Agent 类型标志
    pub agent_type: AgentType,

    /// Agent 名称 (UTF-8, 最大 64 字节含 NUL)
    pub name: [u8; 64],

    /// Agent 入口点 (ELF 入口虚拟地址)
    pub entry_point: u64,

    /// 代码段大小 (字节)
    pub code_size: u64,

    /// 初始堆大小 (字节, 0 表示使用默认值 4MB)
    pub heap_size: u64,

    /// 初始栈大小 (字节, 0 表示使用默认值 2MB)
    pub stack_size: u64,

    /// CPU 亲和性掩码 (0 表示不限制)
    pub cpu_affinity: u64,

    /// 调度优先级 (0-255, 128 为默认)
    pub priority: u8,

    /// 调度策略
    pub sched_policy: SchedPolicy,

    /// 最大内存限制 (字节, 0 表示无限制)
    pub memory_limit: u64,

    /// 最大文件描述符数 (0 表示使用系统默认)
    pub max_fds: u32,

    /// 能力集 (初始能力位图)
    pub capabilities: CapBitmap,

    /// 通信端口数量
    pub port_count: u16,

    /// 标志位 (AgentFlags 组合)
    pub flags: u32,

    /// 资源配额
    pub quota: ResourceQuota,

    /// 安全标签 (用于强制访问控制)
    pub security_label: [u8; 32],

    /// 初始化参数 (传递给 Agent 的配置数据)
    pub init_param: [u8; 32],

    /// 保留字段 (用于未来扩展, 必须置零)
    pub reserved: [u8; 16],
}

/// Agent 类型枚举
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    /// 通用计算 Agent
    Generic = 0,
    /// AI 推理 Agent (可访问 GPU/NPU)
    AIInference = 1,
    /// 数据处理 Agent (批量 I/O 优化)
    DataProcessing = 2,
    /// 网络 Agent (可绑定低级网络端口)
    Network = 3,
    /// 系统 Agent (特权操作, 需要内核签名)
    System = 4,
    /// 沙箱 Agent (最小权限)
    Sandbox = 5,
    /// 虚拟化 Agent (可创建 VM)
    Virtualization = 6,
}

/// 调度策略
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedPolicy {
    /// 完全公平调度器 (默认)
    CFS = 0,
    /// 实时 FIFO
    RTFifo = 1,
    /// 实时 Round-Robin
    RTRR = 2,
    /// 空闲调度 (仅当 CPU 空闲时运行)
    Idle = 3,
    /// 批处理调度
    Batch = 4,
}

/// 资源配额
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ResourceQuota {
    /// CPU 时间配额 (微秒/秒, 0 表示无限制)
    pub cpu_time_us: u32,
    /// 最大并发线程数
    pub max_threads: u16,
    /// 最大共享内存区域数
    pub max_shm_regions: u16,
    /// 网络带宽限制 (Kbps, 0 表示无限制)
    pub net_bandwidth_kbps: u32,
    /// 磁盘 I/O 带宽限制 (KB/s, 0 表示无限制)
    pub io_bandwidth_kbs: u32,
    /// 保留
    pub reserved: [u8; 8],
}

/// Agent 标志位
pub mod agent_flags {
    pub const NONE: u32 = 0;
    /// 启用 Agent 快照功能
    pub const SNAPSHOT_ENABLED: u32 = 1 << 0;
    /// 允许 Agent 迁移
    pub const MIGRATABLE: u32 = 1 << 1;
    /// 启用 Agent 间直接内存共享
    pub const DIRECT_SHM: u32 = 1 << 2;
    /// Agent 可访问 GPU
    pub const GPU_ACCESS: u32 = 1 << 3;
    /// Agent 可访问网络
    pub const NET_ACCESS: u32 = 1 << 4;
    /// Agent 可访问文件系统
    pub const FS_ACCESS: u32 = 1 << 5;
    /// Agent 在后台运行 (不占用桌面资源)
    pub const BACKGROUND: u32 = 1 << 6;
    /// Agent 自动重启 (崩溃后)
    pub const AUTO_RESTART: u32 = 1 << 7;
    /// Agent 为持久化 Agent (跨重启存活)
    pub const PERSISTENT: u32 = 1 << 8;
    /// Agent 运行在安全 enclave 中
    pub const ENCLAVED: u32 = 1 << 9;
    /// Agent 可创建子 Agent
    pub const CAN_SPAWN: u32 = 1 << 10;
}

/// 能力位图 (128 位, 覆盖所有预定义能力)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CapBitmap {
    pub bits: [u64; 2],
}
```

#### 4.1.4 能力要求

| 调用者能力 | 要求 |
|-----------|------|
| `CAP_SPAWN_AGENT` | 基础 Agent 创建权限 |
| `CAP_SPAWN_SYSTEM_AGENT` | 创建 System 类型 Agent |
| `CAP_SPAWN_ENCLAVED` | 创建 Enclave Agent |
| `CAP_ADMIN` | 绕过资源配额限制 |

#### 4.1.5 错误码

| errno | 值 | 描述 |
|-------|----|------|
| `EINVAL` | 22 | AgentSpec 结构体无效 (版本不匹配、字段非法) |
| `ENOMEM` | 12 | 内存不足，无法分配 Agent 资源 |
| `EACCES` | 13 | 缺少必要的能力 |
| `EEXIST` | 17 | 同名 Agent 已存在 |
| `EAGAIN` | 11 | 系统资源暂时不足 (Agent 数量上限) |
| `EPERM` | 1 | 操作不允许 (安全策略拒绝) |
| `EFAULT` | 14 | spec_ptr 指向无效的用户态地址 |
| `ENOSPC` | 28 | 磁盘空间不足 (持久化 Agent) |
| `EOVERFLOW` | 75 | spec_len 超过最大允许值 |

#### 4.1.6 状态机

```
                    ┌──────────┐
                    │  INVALID │ (AgentSpec 校验失败)
                    └────┬─────┘
                         │ 校验通过
                         ▼
                    ┌──────────┐
         ┌─────────│ ALLOCATE │─────────┐
         │         └────┬─────┘         │
         │              │ 分配成功       │ 分配失败
         │              ▼               ▼
         │         ┌──────────┐    ┌──────────┐
         │         │  SETUP   │    │  ERROR   │
         │         └────┬─────┘    └──────────┘
         │              │
         │              ▼
         │         ┌──────────┐
         │         │ LOADING  │
         │         └────┬─────┘
         │              │
         │              ▼
         │         ┌──────────┐
         │         │ INIT     │
         │         └────┬─────┘
         │              │
         │      ┌───────┴───────┐
         │      │               │
         │      ▼               ▼
         │ ┌──────────┐  ┌──────────┐
         │ │  READY   │  │  FAILED  │
         │ └────┬─────┘  └──────────┘
         │      │
         │      ▼
         │ ┌──────────┐
         └─│ CLEANUP  │ (资源回收)
           └──────────┘
```

#### 4.1.7 性能目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 冷启动 (无快照) | < 10ms | 4 核 CPU, 16GB RAM, SSD |
| 热启动 (有快照) | < 2ms | 快照已在内存中 |
| 并发创建吞吐 | > 1000/s | 64 核 CPU, 无 I/O Agent |

---

### 4.2 SYS_AGENT_KILL (513)

#### 4.2.1 功能描述

终止指定的 Agent 实例，释放其占用的所有系统资源。

#### 4.2.2 函数签名

```rust
/// 终止 Agent
///
/// # 参数
/// - `agent_handle`: 目标 Agent 的句柄
/// - `signal`: 终止信号 (0 = 强制终止, 1 = 优雅终止)
///
/// # 返回值
/// - 成功: 返回 0
/// - 失败: 返回负的 errno 值
fn sys_agent_kill(agent_handle: AgentHandle, signal: u32) -> isize;
```

#### 4.2.3 终止信号

| 信号值 | 名称 | 行为 |
|--------|------|------|
| 0 | `SIG_KILL` | 立即终止，不执行清理回调 |
| 1 | `SIG_TERM` | 优雅终止，执行清理回调后退出 |
| 2 | `SIG_COREDUMP` | 终止并生成核心转储 |
| 3 | `SIG_FREEZE` | 冻结 Agent (保留状态，停止调度) |
| 4 | `SIG_THAW` | 解冻 Agent (恢复调度) |

#### 4.2.4 能力要求

| 调用者能力 | 要求 |
|-----------|------|
| `CAP_KILL_AGENT` | 终止自己创建的 Agent |
| `CAP_KILL_ANY_AGENT` | 终止任意 Agent |
| `CAP_ADMIN` | 终止 System 类型 Agent |

#### 4.2.5 错误码

| errno | 值 | 描述 |
|-------|----|------|
| `ESRCH` | 3 | Agent 句柄不存在 |
| `EACCES` | 13 | 无权终止目标 Agent |
| `EINVAL` | 22 | 信号值无效 |
| `EBUSY` | 16 | Agent 正在迁移中，无法终止 |

---

### 4.3 SYS_AGENT_QUERY (514)

#### 4.3.1 功能描述

查询指定 Agent 的运行状态、资源使用情况和元数据。

#### 4.3.2 函数签名

```rust
/// 查询 Agent 状态
///
/// # 参数
/// - `agent_handle`: 目标 Agent 句柄
/// - `info_ptr`: 指向 AgentInfo 缓冲区的用户态指针
/// - `info_len`: 缓冲区长度
///
/// # 返回值
/// - 成功: 返回实际写入的字节数
/// - 失败: 返回负的 errno 值
fn sys_agent_query(agent_handle: AgentHandle, info_ptr: *mut AgentInfo, info_len: usize) -> isize;
```

#### 4.3.3 AgentInfo 结构体

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AgentInfo {
    /// Agent 句柄
    pub handle: AgentHandle,
    /// Agent 状态
    pub state: AgentState,
    /// Agent 类型
    pub agent_type: AgentType,
    /// Agent 名称
    pub name: [u8; 64],
    /// 创建者 PID
    pub creator_pid: u64,
    /// 创建时间戳 (纳秒, 从系统启动)
    pub create_time_ns: u64,
    /// CPU 使用时间 (纳秒)
    pub cpu_time_ns: u64,
    /// 内存使用量 (字节, RSS)
    pub memory_used: u64,
    /// 峰值内存使用 (字节)
    pub memory_peak: u64,
    /// 线程数
    pub thread_count: u32,
    /// 活跃连接数
    pub connection_count: u32,
    /// 消息发送计数
    pub msg_sent: u64,
    /// 消息接收计数
    pub msg_received: u64,
    /// 上次活跃时间 (纳秒)
    pub last_active_ns: u64,
    /// 安全标签
    pub security_label: [u8; 32],
    /// 运行所在 CPU 核心
    pub current_cpu: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// 正在创建
    Creating = 0,
    /// 已就绪，等待调度
    Ready = 1,
    /// 正在运行
    Running = 2,
    /// 等待 I/O 或事件
    Waiting = 3,
    /// 已冻结
    Frozen = 4,
    /// 正在迁移
    Migrating = 5,
    /// 正在终止
    Terminating = 6,
    /// 已终止
    Terminated = 7,
    /// 创建失败
    Failed = 8,
}
```

#### 4.3.4 性能目标

| 指标 | 目标值 |
|------|--------|
| 查询延迟 | < 100us (无锁快速路径) |
| 批量查询 (32 Agent) | < 500us |

---

### 4.4 SYS_AGENT_MSG (515)

#### 4.4.1 功能描述

向目标 Agent 发送消息。支持同步和异步两种模式，优先使用共享内存进行零拷贝传输。

#### 4.4.2 函数签名

```rust
/// 向 Agent 发送消息
///
/// # 参数
/// - `src_handle`: 发送者 Agent 句柄
/// - `dst_handle`: 接收者 Agent 句柄
/// - `msg_ptr`: 消息头指针 (AgentMsgHeader)
/// - `flags`: 消息标志 (MSG_SYNC, MSG_ASYNC, MSG_URGENT)
///
/// # 返回值
/// - 成功: 返回消息 ID (>= 0)
/// - 失败: 返回负的 errno 值
fn sys_agent_msg(
    src_handle: AgentHandle,
    dst_handle: AgentHandle,
    msg_ptr: *const AgentMsgHeader,
    flags: u32,
) -> isize;
```

#### 4.4.3 消息结构体

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AgentMsgHeader {
    /// 消息类型
    pub msg_type: u32,
    /// 消息标志
    pub flags: u32,
    /// 消息 ID (由内核填充)
    pub msg_id: u64,
    /// 时间戳
    pub timestamp_ns: u64,
    /// 消息体大小 (字节)
    pub payload_size: u64,
    /// 共享内存区域 ID (零拷贝传输时使用, 0 表示内联数据)
    pub shm_region_id: u32,
    /// 优先级 (0-7, 0 最高)
    pub priority: u8,
    /// 保留
    pub reserved: [u8; 7],
}

/// 消息标志
pub mod msg_flags {
    pub const SYNC: u32 = 1 << 0;      // 同步消息，等待回复
    pub const ASYNC: u32 = 1 << 1;     // 异步消息，立即返回
    pub const URGENT: u32 = 1 << 2;    // 紧急消息，优先投递
    pub const NOCOPY: u32 = 1 << 3;    // 零拷贝 (使用共享内存)
    pub const BROADCAST: u32 = 1 << 4; // 广播消息
    pub const RELIABLE: u32 = 1 << 5;  // 可靠投递 (需确认)
}
```

#### 4.4.4 消息传递状态机

```
发送方                          内核                      接收方
  │                              │                          │
  │── SYS_AGENT_MSG ──────────→  │                          │
  │                              │── 权限检查                │
  │                              │── 消息入队                │
  │  ←── msg_id ──────────────  │                          │
  │                              │                          │
  │  [SYNC]                      │── 通知接收方              │
  │  等待回复...                 │                          │── 读取消息
  │                              │                          │── 处理消息
  │                              │                          │── 发送回复
  │  ←── reply ──────────────── │←── reply ──────────────  │
  │                              │                          │
  │  [ASYNC]                     │                          │
  │  继续执行...                 │                          │── 读取消息 (稍后)
  │                              │                          │
```

#### 4.4.5 错误码

| errno | 值 | 描述 |
|-------|----|------|
| `ESRCH` | 3 | 目标 Agent 不存在 |
| `EACCES` | 13 | 无权向目标 Agent 发送消息 |
| `EFAULT` | 14 | msg_ptr 指向无效地址 |
| `EMSGSIZE` | 90 | 消息体超过最大限制 (16MB) |
| `EAGAIN` | 11 | 接收方消息队列已满 |
| `ENOTCONN` | 107 | 目标 Agent 未注册通信端口 |

---

### 4.5 SYS_AGENT_REGISTER (516)

#### 4.5.1 功能描述

为 Agent 注册一个命名能力 (Capability)，使其可被其他 Agent 发现和调用。

#### 4.5.2 函数签名

```rust
/// 注册 Agent 能力
///
/// # 参数
/// - `agent_handle`: Agent 句柄
/// - `cap_ptr`: 能力描述符指针
/// - `cap_len`: 能力描述符长度
///
/// # 返回值
/// - 成功: 返回能力 ID (>= 0)
/// - 失败: 返回负的 errno 值
fn sys_agent_register(agent_handle: AgentHandle, cap_ptr: *const AgentCapability, cap_len: usize) -> isize;
```

#### 4.5.3 能力描述符

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AgentCapability {
    /// 能力名称 (UTF-8, 最大 128 字节)
    pub name: [u8; 128],
    /// 能力版本 (语义化版本, 主.次.修)
    pub version_major: u16,
    pub version_minor: u16,
    pub version_patch: u16,
    /// 能力类型
    pub cap_type: CapabilityType,
    /// 接口描述符 (可选, 指向接口定义的共享内存 ID)
    pub interface_shm_id: u32,
    /// 最大并发调用数 (0 表示无限制)
    pub max_concurrent: u32,
    /// 超时时间 (毫秒, 0 表示无超时)
    pub timeout_ms: u32,
    /// 信任级别
    pub trust_level: TrustLevel,
    /// 保留
    pub reserved: [u8; 16],
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityType {
    /// RPC 服务
    RpcService = 0,
    /// 数据流
    DataStream = 1,
    /// 事件源
    EventSource = 2,
    /// 共享资源
    SharedResource = 3,
    /// AI 模型推理
    AIInference = 4,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// 仅限同一创建者
    Private = 0,
    /// 同一安全域内可见
    Domain = 1,
    /// 全局可见 (只读)
    PublicRead = 2,
    /// 全局可见 (读写)
    Public = 3,
}
```

---

### 4.6 SYS_AGENT_SUBSCRIBE (517)

#### 4.6.1 功能描述

订阅目标 Agent 的事件通知。支持多种事件类型和过滤条件。

#### 4.6.2 函数签名

```rust
/// 订阅 Agent 事件
///
/// # 参数
/// - `subscriber_handle`: 订阅者 Agent 句柄
/// - `target_handle`: 目标 Agent 句柄 (0 表示全局事件)
/// - `event_mask_ptr`: 事件掩码指针
///
/// # 返回值
/// - 成功: 返回订阅 ID (>= 0)
/// - 失败: 返回负的 errno 值
fn sys_agent_subscribe(
    subscriber_handle: AgentHandle,
    target_handle: AgentHandle,
    event_mask_ptr: *const EventMask,
) -> isize;
```

#### 4.6.3 事件类型

```rust
pub mod agent_events {
    pub const STATE_CHANGED: u64 = 1 << 0;   // Agent 状态变更
    pub const MSG_RECEIVED: u64 = 1 << 1;    // 收到消息
    pub const CAP_REGISTERED: u64 = 1 << 2;  // 新能力注册
    pub const CAP_REVOKED: u64 = 1 << 3;     // 能力撤销
    pub const RESOURCE_LOW: u64 = 1 << 4;    // 资源不足警告
    pub const ERROR: u64 = 1 << 5;           // Agent 错误事件
    pub const MIGRATION: u64 = 1 << 6;       // 迁移事件
    pub const SNAPSHOT: u64 = 1 << 7;        // 快照事件
    pub const HEARTBEAT: u64 = 1 << 8;       // 心跳事件
    pub const CUSTOM_START: u64 = 1 << 16;   // 自定义事件起始位
}
```

---

### 4.7 SYS_AGENT_MIGRATE (518)

#### 4.7.1 功能描述

将 Agent 迁移到同一网络中的另一台设备。支持热迁移 (Agent 继续运行) 和冷迁移 (Agent 暂停后迁移)。

#### 4.7.2 函数签名

```rust
/// 迁移 Agent
///
/// # 参数
/// - `agent_handle`: 要迁移的 Agent 句柄
/// - `dest_addr_ptr`: 目标设备地址 (IPv6, 16 字节)
/// - `flags`: 迁移标志
/// - `token_ptr`: 认证令牌指针
///
/// # 返回值
/// - 成功: 返回迁移操作 ID (>= 0)
/// - 失败: 返回负的 errno 值
fn sys_agent_migrate(
    agent_handle: AgentHandle,
    dest_addr_ptr: *const [u8; 16],
    flags: u32,
    token_ptr: *const MigrationToken,
) -> isize;
```

#### 4.7.3 迁移标志

```rust
pub mod migrate_flags {
    pub const LIVE: u32 = 1 << 0;           // 热迁移
    pub const COLD: u32 = 1 << 1;           // 冷迁移
    pub const FORCE: u32 = 1 << 2;          // 强制迁移 (忽略非关键错误)
    pub const VERIFY: u32 = 1 << 3;         // 迁移后验证
    pub const COMPRESS: u32 = 1 << 4;       // 压缩传输
    pub const ENCRYPTED: u32 = 1 << 5;      // 加密传输
    pub const CHECKPOINT: u32 = 1 << 6;     // 仅创建检查点 (不实际迁移)
}
```

#### 4.7.4 迁移状态机

```
        ┌───────────┐
        │  RUNNING  │
        └─────┬─────┘
              │ 开始迁移
              ▼
        ┌───────────┐
        │ CHECKPOINT│ ──→ 保存内存快照 + 寄存器状态
        └─────┬─────┘
              │ 快照完成
              ▼
        ┌───────────┐
        │ TRANSFER  │ ──→ 传输快照到目标设备
        └─────┬─────┘
              │ 传输完成
              ▼
        ┌───────────┐
        │  VERIFY   │ ──→ 验证目标端状态一致性
        └─────┬─────┘
         成功  │     │ 失败
              │     ▼
              │  ┌───────────┐
              │  │  ROLLBACK │ ──→ 恢复原端运行
              │  └───────────┘
              ▼
        ┌───────────┐
        │ SWITCHOVER│ ──→ 切换流量到目标端
        └─────┬─────┘
              │ 切换完成
              ▼
        ┌───────────┐
        │ CLEANUP   │ ──→ 释放源端资源
        └───────────┘
```

#### 4.7.5 错误码

| errno | 值 | 描述 |
|-------|----|------|
| `ENOTSUP` | 95 | Agent 不支持迁移 (未设置 MIGRATABLE 标志) |
| `EHOSTUNREACH` | 113 | 目标设备不可达 |
| `ETIMEDOUT` | 110 | 迁移超时 |
| `ENOMEM` | 12 | 迁移过程中内存不足 |
| `EACCES` | 13 | 目标设备拒绝迁移 (认证失败) |
| `EALREADY` | 114 | Agent 已在迁移中 |
| `EBUSY` | 16 | Agent 正在处理关键操作，无法迁移 |

---

### 4.8 SYS_AGENT_MEMORY_SHARE (519)

#### 4.8.1 功能描述

在两个 Agent 之间建立共享内存区域。共享内存使用硬件页表机制实现零拷贝数据交换。

#### 4.8.2 函数签名

```rust
/// 与 Agent 共享内存区域
///
/// # 参数
/// - `src_handle`: 源 Agent 句柄
/// - `dst_handle`: 目标 Agent 句柄
/// - `shm_spec_ptr`: 共享内存规范指针
/// - `flags`: 共享标志
///
/// # 返回值
/// - 成功: 返回共享内存区域 ID (>= 0)
/// - 失败: 返回负的 errno 值
fn sys_agent_memory_share(
    src_handle: AgentHandle,
    dst_handle: AgentHandle,
    shm_spec_ptr: *const ShmSpec,
    flags: u32,
) -> isize;
```

#### 4.8.3 共享内存规范

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShmSpec {
    /// 共享区域大小 (字节, 必须是页大小的整数倍)
    pub size: u64,
    /// 源 Agent 中的虚拟地址 (0 表示由内核分配)
    pub src_vaddr: u64,
    /// 权限掩码 (源 Agent)
    pub src_prot: u32,
    /// 权限掩码 (目标 Agent)
    pub dst_prot: u32,
    /// 对齐要求 (字节, 0 表示默认页对齐)
    pub alignment: u32,
    /// 保留
    pub reserved: [u8; 12],
}

/// 共享内存权限
pub mod shm_prot {
    pub const READ: u32 = 0x1;
    pub const WRITE: u32 = 0x2;
    pub const EXEC: u32 = 0x4;
}
```

#### 4.8.4 安全约束

1. 共享内存区域必须页对齐
2. 源 Agent 和目标 Agent 必须都具备 `CAP_SHARE_MEMORY` 能力
3. 源 Agent 只能共享自己地址空间内的合法映射区域
4. 内核维护共享内存引用计数，任一方终止时自动清理
5. 安全 enclave 内的 Agent 共享内存需经过 enclave 审批

---

## 5. 错误处理框架

### 5.1 错误码体系

OmniAgent OS 采用分层错误码体系，确保错误信息的精确性和可操作性：

```rust
/// Agent syscall 错误码
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentError {
    // 通用错误 (1-99)
    EPERM           = 1,    // 操作不允许
    ENOENT          = 2,    // 无此实体
    ESRCH           = 3,    // 无此进程/Agent
    EINTR           = 4,    // 系统调用被中断
    EIO             = 5,    // I/O 错误
    ENXIO           = 6,    // 无此设备或地址
    EAGAIN          = 11,   // 资源暂时不可用
    ENOMEM          = 12,   // 内存不足
    EACCES          = 13,   // 权限不足
    EFAULT          = 14,   // 错误的地址
    EBUSY           = 16,   // 设备或资源忙
    EEXIST          = 17,   // 文件已存在
    EINVAL          = 22,   // 无效参数
    ENOSPC          = 28,   // 设备无空间
    EROFS           = 30,   // 只读文件系统
    EOVERFLOW       = 75,   // 值溢出
    ENOTCONN        = 107,  // 未连接
    ETIMEDOUT       = 110,  // 连接超时
    EALREADY        = 114,  // 操作已在进行
    ENOTSUP         = 95,   // 不支持的操作
    EHOSTUNREACH    = 113,  // 目标主机不可达

    // Agent 专用错误 (1000-1999)
    EAGENT_NOT_FOUND    = 1001, // Agent 不存在
    EAGENT_BUSY         = 1002, // Agent 忙碌
    EAGENT_DEAD         = 1003, // Agent 已终止
    EAGENT_FROZEN       = 1004, // Agent 已冻结
    EAGENT_MIGRATING    = 1005, // Agent 正在迁移
    EAGENT_QUOTA_EXCEEDED = 1006, // Agent 超出配额
    EAGENT_CAP_MISSING  = 1007, // 缺少必要能力
    EAGENT_AUTH_FAILED  = 1008, // Agent 认证失败
    EAGENT_SANDBOX_VIOLATION = 1009, // 沙箱违规
    EAGENT_MSG_TOO_LARGE = 1010, // 消息过大
    EAGENT_QUEUE_FULL   = 1011, // 消息队列满
    EAGENT_SHM_INVALID  = 1012, // 共享内存无效
    EAGENT_SNAPSHOT_FAILED = 1013, // 快照失败
    EAGENT_RESTORE_FAILED = 1014, // 恢复失败
}
```

### 5.2 错误传播机制

```
用户态 Agent                    内核 Agent 子系统
     │                              │
     │── syscall (rax=512) ──────→  │
     │                              │── 参数校验
     │                              │── 能力检查
     │                              │── 执行操作
     │                              │
     │  [错误路径]                   │
     │  ←── rax = -EACCES ───────  │
     │                              │── 记录审计日志
     │  errno = EACCES              │
     │  perror("agent_spawn")       │
```

---

## 6. 性能约束

### 6.1 系统调用性能目标

| Syscall | 目标延迟 (P99) | 目标吞吐 | 备注 |
|---------|---------------|---------|------|
| `SYS_AGENT_SPAWN` | < 10ms | > 1000/s | 冷启动, 无 I/O |
| `SYS_AGENT_KILL` | < 1ms | > 10000/s | 强制终止 |
| `SYS_AGENT_QUERY` | < 100us | > 100000/s | 快速路径无锁 |
| `SYS_AGENT_MSG` (同步) | < 500us | > 50000/s | 含一次往返 |
| `SYS_AGENT_MSG` (异步) | < 10us | > 500000/s | 仅入队 |
| `SYS_AGENT_REGISTER` | < 50us | > 50000/s | |
| `SYS_AGENT_SUBSCRIBE` | < 50us | > 50000/s | |
| `SYS_AGENT_MIGRATE` | < 5s | N/A | 取决于内存大小和网络 |
| `SYS_AGENT_MEMORY_SHARE` | < 200us | > 20000/s | 含页表更新 |

### 6.2 性能优化策略

1. **快速路径**: 常用 syscall (QUERY, MSG_ASYNC) 使用无锁快速路径
2. **批量操作**: 支持批量查询和批量消息发送
3. **共享内存池**: 预分配消息缓冲区池，减少动态分配
4. **NUMA 感知**: Agent 调度考虑 NUMA 拓扑，减少跨节点通信
5. **内核态缓存**: Agent 元数据使用 RCU (Read-Copy-Update) 保护

### 6.3 性能监控接口

```rust
/// Syscall 性能统计 (通过 debugfs 或 agent_query 获取)
#[repr(C)]
pub struct SyscallPerfStats {
    pub total_calls: u64,
    pub total_time_ns: u64,
    pub min_time_ns: u64,
    pub max_time_ns: u64,
    pub p50_time_ns: u64,
    pub p99_time_ns: u64,
    pub error_count: u64,
}
```

---

## 7. ABI 稳定性保证

### 7.1 版本控制

```
ABI 版本格式: MAJOR.MINOR.PATCH

MAJOR: 不兼容变更 (结构体布局改变, syscall 语义变更)
MINOR: 向后兼容新增 (新增 syscall, 新增结构体字段在末尾)
PATCH: Bug 修复 (不影响 ABI)
```

### 7.2 兼容性规则

| 变更类型 | MAJOR | MINOR | PATCH |
|---------|-------|-------|-------|
| 新增 syscall 号 | X | | |
| 废弃 syscall 号 | X | | |
| 结构体末尾新增字段 | | X | |
| 结构体中间插入字段 | X | | |
| 修改字段类型/大小 | X | | |
| 修改字段语义 | X | | |
| 修改错误码含义 | X | | |
| 新增错误码 | | X | |
| 修改性能目标 | | | X |

### 7.3 结构体版本检测

所有用户态传入内核的结构体都通过 `version` 和长度字段进行版本协商：

```rust
// 内核侧版本检查逻辑 (伪代码)
fn check_spec_version(spec_ptr: *const AgentSpec, spec_len: usize) -> Result<(), AgentError> {
    let spec = unsafe { &*spec_ptr };

    // 检查最小支持版本
    if spec.version < MIN_SUPPORTED_VERSION {
        return Err(AgentError::EINVAL);
    }

    // 检查结构体大小是否合理
    if spec_len < core::mem::size_of::<AgentSpecV1>() {
        return Err(AgentError::EINVAL);
    }

    // 如果结构体比内核已知的版本更大，说明来自更新的内核
    // 只使用内核已知的部分，忽略扩展字段
    if spec.version > CURRENT_VERSION {
        // 使用 spec_len 判断实际大小
        // 只读取内核理解的部分
    }

    Ok(())
}
```

### 7.4 废弃策略

1. **废弃通知**: 废弃的 syscall 在内核日志中输出警告
2. **过渡期**: 废弃 syscall 至少保留 3 个 MAJOR 版本
3. **编译期检测**: 用户态 SDK 在编译期检测废弃 API 使用
4. **运行时替代**: 废弃 syscall 内部重定向到新实现

---

## 8. 安全考虑

### 8.1 Syscall 安全检查流程

```
用户态调用
    │
    ▼
┌─────────────────┐
│ 1. 参数验证      │ ← 地址范围检查, 指针非空检查
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 2. 能力检查      │ ← 验证调用者是否持有必要能力
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 3. 资源配额检查  │ ← 检查是否超出配额限制
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 4. 安全策略检查  │ ← PBAC/RBAC 策略引擎
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 5. 执行操作      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 6. 审计日志记录  │ ← 记录操作详情
└─────────────────┘
```

### 8.2 能力要求汇总表

| Syscall | 必要能力 | 可选能力 (扩展功能) |
|---------|---------|-------------------|
| `SPAWN` | `CAP_SPAWN_AGENT` | `CAP_SPAWN_SYSTEM_AGENT`, `CAP_SPAWN_ENCLAVED` |
| `KILL` | `CAP_KILL_AGENT` | `CAP_KILL_ANY_AGENT` |
| `QUERY` | `CAP_QUERY_AGENT` | `CAP_QUERY_ANY_AGENT` |
| `MSG` | `CAP_SEND_MSG` | `CAP_SEND_URGENT` |
| `REGISTER` | `CAP_REGISTER_CAP` | `CAP_REGISTER_PUBLIC` |
| `SUBSCRIBE` | `CAP_SUBSCRIBE_EVENT` | `CAP_SUBSCRIBE_GLOBAL` |
| `MIGRATE` | `CAP_MIGRATE_AGENT` | `CAP_MIGRATE_CROSS_DOMAIN` |
| `MEMORY_SHARE` | `CAP_SHARE_MEMORY` | `CAP_SHARE_EXEC` |

### 8.3 时间侧信道防护

为防止通过 syscall 延迟推断系统状态，所有 Agent syscall 在错误路径上执行恒定时间返回：

```rust
// 恒定时间错误返回示例
fn syscall_return_error(errno: i32) {
    // 无论错误类型，执行相同数量的内存访问
    let _sink = volatile_read(&DUMMY_MEMORY[errno as usize % 64]);
    // 确保返回时间不泄露错误类型信息
    barrier::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
```

---

## 9. 测试用例

### 9.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // 测试 AgentSpec 结构体布局
    #[test]
    fn test_agentspec_layout() {
        assert_eq!(core::mem::size_of::<AgentSpec>(), 256);
        assert_eq!(core::mem::align_of::<AgentSpec>(), 8);
    }

    // 测试 AgentInfo 结构体布局
    #[test]
    fn test_agentinfo_layout() {
        assert_eq!(core::mem::size_of::<AgentInfo>(), 264);
    }

    // 测试 AgentMsgHeader 结构体布局
    #[test]
    fn test_msg_header_layout() {
        assert_eq!(core::mem::size_of::<AgentMsgHeader>(), 48);
    }

    // 测试能力位图操作
    #[test]
    fn test_cap_bitmap() {
        let mut bitmap = CapBitmap { bits: [0, 0] };
        bitmap.set(0);
        bitmap.set(64);
        assert!(bitmap.test(0));
        assert!(bitmap.test(64));
        assert!(!bitmap.test(1));
    }

    // 测试错误码范围
    #[test]
    fn test_error_codes() {
        assert!(AgentError::EPERM as i32 > 0);
        assert!(AgentError::EAGENT_NOT_FOUND as i32 >= 1000);
    }

    // 测试 AgentSpec 版本检查
    #[test]
    fn test_version_check() {
        let spec = AgentSpec::new_v1();
        assert_eq!(spec.version, 1);
        assert!(spec.is_compatible());
    }
}
```

### 9.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    // 测试: 创建 Agent 后立即查询状态
    #[test]
    fn test_spawn_and_query() {
        let spec = AgentSpec::new_v1()
            .with_name("test_agent")
            .with_type(AgentType::Sandbox)
            .with_heap_size(1024 * 1024);

        let handle = unsafe { sys_agent_spawn(&spec, core::mem::size_of::<AgentSpec>(), 0) };
        assert!(handle >= 0);

        let mut info = AgentInfo::zeroed();
        let ret = unsafe {
            sys_agent_query(handle as AgentHandle, &mut info, core::mem::size_of::<AgentInfo>())
        };
        assert!(ret > 0);
        assert_eq!(info.state, AgentState::Ready);

        // 清理
        let _ = unsafe { sys_agent_kill(handle as AgentHandle, 0) };
    }

    // 测试: 无能力时创建 Agent 应返回 EACCES
    #[test]
    fn test_spawn_without_capability() {
        // 在无 CAP_SPAWN_AGENT 能力的进程中调用
        let spec = AgentSpec::new_v1();
        let ret = unsafe { sys_agent_spawn(&spec, core::mem::size_of::<AgentSpec>(), 0) };
        assert_eq!(ret, -(AgentError::EACCES as isize));
    }

    // 测试: 向不存在的 Agent 发送消息应返回 ESRCH
    #[test]
    fn test_msg_nonexistent_agent() {
        let fake_handle = 0xDEAD as AgentHandle;
        let msg = AgentMsgHeader::new(0);
        let ret = unsafe {
            sys_agent_msg(0 as AgentHandle, fake_handle, &msg, msg_flags::ASYNC)
        };
        assert_eq!(ret, -(AgentError::ESRCH as isize));
    }

    // 测试: 共享内存权限检查
    #[test]
    fn test_shm_permission_check() {
        let shm_spec = ShmSpec {
            size: 4096,
            src_vaddr: 0,
            src_prot: shm_prot::READ | shm_prot::WRITE,
            dst_prot: shm_prot::READ,  // 目标只读
            alignment: 0,
            reserved: [0; 12],
        };

        // 验证权限掩码正确性
        assert_eq!(shm_spec.src_prot & shm_prot::EXEC, 0);
        assert_ne!(shm_spec.dst_prot & shm_prot::READ, 0);
    }

    // 测试: 并发 Agent 创建
    #[test]
    fn test_concurrent_spawn() {
        const NUM_AGENTS: usize = 100;
        let mut handles = Vec::new();

        for i in 0..NUM_AGENTS {
            let spec = AgentSpec::new_v1()
                .with_name(&format!("concurrent_{}", i))
                .with_type(AgentType::Sandbox);

            let handle = unsafe {
                sys_agent_spawn(&spec, core::mem::size_of::<AgentSpec>(), 0)
            };
            assert!(handle >= 0, "Failed to spawn agent {}", i);
            handles.push(handle);
        }

        // 清理所有 Agent
        for handle in handles {
            let _ = unsafe { sys_agent_kill(handle as AgentHandle, 0) };
        }
    }

    // 测试: Agent 迁移标志组合
    #[test]
    fn test_migrate_flags() {
        let flags = migrate_flags::LIVE | migrate_flags::ENCRYPTED | migrate_flags::VERIFY;
        assert!(flags & migrate_flags::LIVE != 0);
        assert!(flags & migrate_flags::ENCRYPTED != 0);
        assert!(flags & migrate_flags::COLD == 0); // LIVE 和 COLD 互斥
    }
}
```

### 9.3 性能测试

```rust
#[cfg(test)]
mod perf_tests {
    // 测试: Agent 查询延迟 < 100us
    #[test]
    fn test_query_latency() {
        let handle = setup_test_agent();

        let start = Tsc::read();
        for _ in 0..10000 {
            let mut info = AgentInfo::zeroed();
            unsafe {
                sys_agent_query(handle, &mut info, core::mem::size_of::<AgentInfo>());
            }
        }
        let elapsed = Tsc::read() - start;
        let avg_ns = elapsed.to_nanoseconds() / 10000;

        assert!(avg_ns < 100_000, "Query latency {}ns exceeds 100us target", avg_ns);
    }

    // 测试: Agent 创建吞吐 > 1000/s
    #[test]
    fn test_spawn_throughput() {
        let start = Instant::now();
        const COUNT: usize = 1000;

        for _ in 0..COUNT {
            let spec = AgentSpec::new_v1()
                .with_type(AgentType::Sandbox);
            unsafe { sys_agent_spawn(&spec, core::mem::size_of::<AgentSpec>(), 0); }
        }

        let elapsed = start.elapsed();
        let throughput = COUNT as f64 / elapsed.as_secs_f64();
        assert!(throughput > 1000.0, "Spawn throughput {} < 1000/s", throughput);
    }

    // 测试: 异步消息吞吐 > 500000/s
    #[test]
    fn test_async_msg_throughput() {
        let (src, dst) = setup_agent_pair();

        let start = Instant::now();
        const COUNT: usize = 500000;
        let msg = AgentMsgHeader::new(0);

        for _ in 0..COUNT {
            unsafe {
                sys_agent_msg(src, dst, &msg, msg_flags::ASYNC);
            }
        }

        let elapsed = start.elapsed();
        let throughput = COUNT as f64 / elapsed.as_secs_f64();
        assert!(throughput > 500_000.0, "Async msg throughput {} < 500000/s", throughput);
    }
}
```

### 9.4 边界条件测试

```rust
#[cfg(test)]
mod edge_case_tests {
    // 测试: AgentSpec 名称边界 (恰好 64 字节)
    #[test]
    fn test_agentspec_name_boundary() {
        let mut spec = AgentSpec::new_v1();
        // 填充 63 字节 + NUL = 64 字节 (合法)
        for i in 0..63 {
            spec.name[i] = b'a';
        }
        spec.name[63] = 0;
        assert!(spec.validate_name().is_ok());

        // 填充 64 字节无 NUL (非法)
        spec.name[63] = b'a';
        assert!(spec.validate_name().is_err());
    }

    // 测试: 最大消息大小
    #[test]
    fn test_max_message_size() {
        const MAX_MSG_SIZE: usize = 16 * 1024 * 1024; // 16MB
        let msg = AgentMsgHeader {
            payload_size: MAX_MSG_SIZE as u64 + 1,
            ..AgentMsgHeader::new(0)
        };
        assert_eq!(validate_msg_size(&msg), Err(AgentError::EMSGSIZE));
    }

    // 测试: 共享内存大小必须页对齐
    #[test]
    fn test_shm_alignment() {
        let spec = ShmSpec {
            size: 4095, // 非页对齐
            ..ShmSpec::default()
        };
        assert!(validate_shm_spec(&spec).is_err());

        let spec = ShmSpec {
            size: 4096, // 页对齐
            ..ShmSpec::default()
        };
        assert!(validate_shm_spec(&spec).is_ok());
    }

    // 测试: 零大小共享内存
    #[test]
    fn test_zero_shm_size() {
        let spec = ShmSpec { size: 0, ..ShmSpec::default() };
        assert!(validate_shm_spec(&spec).is_err());
    }
}
```

---

## 10. 附录

### 10.1 完整 Syscall 号分配表

```
0-63:     文件 I/O
64-127:   进程管理
128-191:  内存管理
192-255:  网络相关
256-319:  信号与同步
320-383:  时间与定时器
384-447:  文件系统
448-511:  设备与杂项
512-599:  Agent 核心 syscall (本文档定义)
600-767:  虚拟化 syscall (预留)
768-895:  安全 Enclave syscall (预留)
896-1023: 扩展预留
```

### 10.2 AgentHandle 类型定义

```rust
/// Agent 句柄 - 内核分配的不透明标识符
///
/// 用户态不得解析句柄内部结构
/// 句柄仅在创建它的内核实例中有效
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentHandle(u64);

impl AgentHandle {
    /// 无效句柄常量
    pub const INVALID: AgentHandle = AgentHandle(0);

    /// 检查句柄是否有效
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}
```

### 10.3 参考文档

- [OA-ARCH-001: 系统架构总览](./system-overview.md)
- [OA-ARCH-003: Agent 运行时规范](./agent-runtime.md)
- [OA-ARCH-006: 安全模型规范](./security-model.md)
- [OA-ARCH-007: 启动流程规范](./boot-process.md)
- [Rust `core::arch::asm!` 文档](https://doc.rust-lang.org/core/arch/macro.asm.html)
- [System V AMD64 ABI 规范](https://gitlab.com/x86-psABIs/x86-64-ABI)
