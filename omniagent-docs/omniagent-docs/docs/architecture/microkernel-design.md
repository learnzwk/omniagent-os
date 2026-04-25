# OmniAgent OS 微内核设计规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: L1 架构设计文档
> **目标读者**: 内核开发者、系统程序员、Agent 运行时开发者

---

## 1. 文档目的

本文档详细描述 OmniAgent OS 微内核的设计规范，包括微内核哲学、进程模型、线程管理、系统调用接口、中断处理架构、设备驱动框架、引导序列、虚拟化支持、错误处理机制以及内核内存布局。本文档是内核实现的主要参考依据。

---

## 2. 微内核哲学

### 2.1 核心原则

OmniAgent OS 严格遵循微内核架构原则，仅将以下功能保留在内核态（Ring 0）：

| 子系统 | 职责 | 必须在内核态的理由 |
|--------|------|-------------------|
| **CFS 调度器** | 线程/Agent 调度决策 | 需直接操作 CPU 状态和定时器 |
| **IPC 引擎** | 消息路由、端口管理 | 需操作地址空间映射 |
| **虚拟内存** | 页表管理、地址空间 | 需操作 CR3/MMU 寄存器 |
| **中断处理** | IDT 管理、中断分发 | 硬件中断必须在 Ring 0 处理 |
| **Agent Syscall** | Agent 专用系统调用 | Agent 是一等公民，需内核直接支持 |
| **定时器** | 高精度时间管理 | 需直接访问硬件定时器 |

**不在内核态的功能**（全部以用户态服务实现）：

- 文件系统
- 网络协议栈
- 设备驱动（通过 IPC 与硬件交互）
- 窗口管理和图形合成
- AI 推理引擎
- 安全策略执行

### 2.2 最小化 TCB

```
┌─────────────────────────────────────────────────┐
│                  TCB (可信计算基)                │
│  ┌───────────────────────────────────────────┐  │
│  │              微内核代码                     │  │
│  │  ~15,000 行 Rust 代码 (预估)              │  │
│  │  - 调度器:    ~3,000 行                   │  │
│  │  - IPC 引擎:  ~2,500 行                   │  │
│  │  - 内存管理:  ~4,000 行                   │  │
│  │  - 中断处理:  ~1,500 行                   │  │
│  │  - Agent syscall: ~2,000 行               │  │
│  │  - 定时器:    ~1,000 行                   │  │
│  │  - 启动代码:  ~1,000 行                   │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  依赖的安全保证:                                 │
│  - Rust 编译器内存安全检查                      │
│  - `unsafe` 块审计清单                          │
│  - 形式化验证 (关键路径)                        │
└─────────────────────────────────────────────────┘
```

### 2.3 `no_std` 约束

内核在 `no_std` 环境下运行，无标准库支持：

```rust
#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(core_intrinsics)]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // 内核 panic 处理（详见第 9 节）
    kernel_panic_handler(_info);
    loop {}
}
```

---

## 3. 进程模型与地址空间布局

### 3.1 进程抽象

OmniAgent OS 中，进程是资源分配的基本单位，线程是调度的基本单位：

```rust
/// 进程控制块 (Process Control Block)
pub struct Process {
    /// 进程唯一标识符
    pub pid: ProcessId,
    /// 进程类型
    pub process_type: ProcessType,
    /// 进程状态
    pub state: ProcessState,
    /// 页表根地址 (CR3)
    pub page_table_root: PhysAddr,
    /// 虚拟地址空间布局
    pub address_space: AddressSpace,
    /// 进程内的线程列表
    pub threads: SpinLock<Vec<ThreadId>>,
    /// 打开的端口列表
    pub ports: SpinLock<Vec<PortId>>,
    /// 进程资源配额
    pub quota: ResourceQuota,
    /// 进程创建时间
    pub created_at: u64,
    /// Agent 特有字段（如果是 Agent 类型）
    pub agent_info: Option<AgentInfo>,
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessType {
    /// 内核进程（init 等）
    Kernel,
    /// 系统服务（驱动、文件系统等）
    SystemService,
    /// AI Agent
    Agent,
    /// 虚拟机
    VirtualMachine,
    /// 普通用户应用
    UserApplication,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessState {
    Created,
    Ready,
    Running,
    Blocked,
    Suspended,
    Terminated,
}
```

### 3.2 地址空间布局

每个进程拥有独立的虚拟地址空间，布局如下：

```
64 位虚拟地址空间 (0x0000_0000_0000_0000 ~ 0xFFFF_FFFF_FFFF_FFFF)

用户空间 (低半部分, 0x0000_0000_0000_0000 ~ 0x0000_7FFF_FFFF_FFFF)
┌─────────────────────────────────────────────────────────────┐
│ 0x0000_0000_0000_0000                                        │
│ ┌─────────────────────────┐                                 │
│ │     不可访问区域         │  空指针捕获区域                  │
│ │     (NULL guard)        │                                 │
│ └─────────────────────────┘                                 │
│ 0x0000_0000_0001_0000                                        │
│ ┌─────────────────────────┐                                 │
│ │     .text 段            │  程序代码 (只读+可执行)          │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     .rodata 段          │  只读数据                       │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     .data 段            │  已初始化数据 (可读写)           │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     .bss 段             │  未初始化数据 (可读写)           │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     堆 (Heap)           │  向上增长                       │
│ │     ▲                   │  brk/sbrk 管理                  │
│ │     │                   │                                 │
│ └─────────────────────────┘                                 │
│                                                             │
│         ... 空闲区域 ...                                     │
│                                                             │
│ ┌─────────────────────────┐                                 │
│ │     共享内存区域         │  IPC 零拷贝缓冲区               │
│ │     (mmap regions)      │  Agent 间共享数据               │
│ └─────────────────────────┘                                 │
│                                                             │
│         ... 空闲区域 ...                                     │
│                                                             │
│ ┌─────────────────────────┐                                 │
│ │     栈 (Stack)          │  向下增长                       │
│ │     │                   │  默认 8 MB                      │
│ │     ▼                   │                                 │
│ └─────────────────────────┘                                 │
│ 0x0000_7FFF_FFFF_FFFF                                        │
└─────────────────────────────────────────────────────────────┘

内核空间 (高半部分, 0xFFFF_8000_0000_0000 ~ 0xFFFF_FFFF_FFFF_FFFF)
  - 仅内核态可访问
  - 详见第 10 节「内核内存布局」
```

### 3.3 Agent 地址空间扩展

Agent 进程的地址空间在标准布局基础上增加了以下区域：

```
Agent 专用区域 (在标准用户空间内):

┌──────────────────────────────────────────┐
│ 0x0000_1000_0000_0000                     │
│ ┌──────────────────────────────────────┐ │
│ │  Agent 上下文区域                     │ │
│ │  - Agent 状态元数据                   │ │
│ │  - 感知-行动循环缓冲区               │ │
│ │  - 工具调用栈                         │ │
│ └──────────────────────────────────────┘ │
│ ┌──────────────────────────────────────┐ │
│ │  Agent 感知输入缓冲区                 │ │
│ │  - 多模态输入 (文本/图像/音频)        │ │
│ │  - 环境状态快照                       │ │
│ └──────────────────────────────────────┘ │
│ ┌──────────────────────────────────────┐ │
│ │  Agent 输出缓冲区                     │ │
│ │  - 动作指令                           │ │
│ │  - 通信消息                           │ │
│ └──────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

---

## 4. 线程管理与上下文切换

### 4.1 线程控制块

```rust
/// 线程控制块 (Thread Control Block)
pub struct Thread {
    /// 线程唯一标识符
    pub tid: ThreadId,
    /// 所属进程
    pub pid: ProcessId,
    /// 线程状态
    pub state: ThreadState,
    /// 优先级 (nice 值)
    pub priority: i8,
    /// Agent 优先级类别
    pub agent_class: Option<AgentPriorityClass>,
    /// 调度器实体 (CFS 运行时数据)
    pub sched_entity: SchedEntity,
    /// 保存的寄存器上下文
    pub context: ThreadContext,
    /// 线程栈信息
    pub stack: StackInfo,
    /// 等待的 IPC 消息
    pub waiting_ipc: Option<IpcWaitHandle>,
    /// 定时器相关
    pub timer: Option<TimerHandle>,
    /// CPU 亲和性
    pub cpu_affinity: u64,
    /// 时间统计
    pub time_stats: ThreadTimeStats,
}

/// 线程寄存器上下文 (x86_64)
#[repr(C)]
pub struct ThreadContext {
    // Callee-saved registers (被调用者保存)
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // 程序计数器和栈指针
    pub rip: u64,
    pub rsp: u64,
    // 状态标志
    pub rflags: u64,
    // 段寄存器
    pub cs: u64,
    pub ss: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreadState {
    New,
    Ready,
    Running,
    Blocked,
    Terminated,
}
```

### 4.2 CFS 调度器变体

OmniAgent OS 使用基于 `arceos-fairsched` 的 CFS 调度器变体，增加了 Agent 优先级类别支持：

```rust
/// 调度器实体
pub struct SchedEntity {
    /// 虚拟运行时间 (vruntime)
    pub vruntime: u64,
    /// 实际运行时间
    pub runtime: u64,
    /// 权重 (基于 nice 值计算)
    pub weight: u32,
    /// Agent 优先级类别
    pub agent_class: Option<AgentPriorityClass>,
}

/// nice 值到权重的映射表 (与 Linux 兼容)
pub const NICE_TO_WEIGHT: [u32; 20] = [
    /* -20 */ 88761, 71755, 56483, 46273, 36291,
    /* -15 */ 29154, 23254, 18705, 14949, 11916,
    /* -10 */  9548,  7620,  6100,  4904,  3906,
    /*  -5 */  3121,  2501,  1991,  1586,  1277,
    /*   0 */  1024,   820,   655,   526,   423,
    /*   5 */   335,   272,   215,   172,   137,
    /*  10 */   110,    87,    70,    56,    45,
    /*  15 */    36,    29,    23,    18,    15,
];

/// Agent 优先级类别对调度的影响
pub struct AgentSchedPolicy {
    /// 最小保证时间片比例
    pub min_timeslice_ratio: f64,
    /// 最大延迟容忍
    pub max_latency_ns: u64,
    /// 是否允许抢占
    pub preemptible: bool,
    /// CPU 保留策略
    pub cpu_reservation: Option<u32>,
}
```

### 4.3 上下文切换流程

```
线程 A 运行中                    线程 B 开始运行
    │                                │
    ▼                                ▼
┌─────────────┐                ┌─────────────┐
│ 时钟中断 /   │                │ 恢复寄存器   │
│ 抢占点触发   │                │ (Thread B)  │
└──────┬──────┘                └──────┬──────┘
       │                              ▲
       ▼                              │
┌─────────────┐                ┌──────┴──────┐
│ 保存当前     │                │ 切换页表     │
│ 寄存器       │                │ (CR3 ← B)   │
│ (Thread A)   │                └──────┬──────┘
└──────┬──────┘                       │
       │                              │
       ▼                              │
┌─────────────┐                       │
│ 更新 A 的    │                       │
│ vruntime    │                       │
└──────┬──────┘                       │
       │                              │
       ▼                              │
┌─────────────┐                       │
│ CFS 选择     │                       │
│ 下一个线程 B │───────────────────────┘
│ (最小vruntime)│
└─────────────┘
```

**上下文切换关键代码**:

```rust
#[naked]
pub unsafe extern "sysv64" fn context_switch(
    _prev_context: *mut ThreadContext,
    _next_context: *const ThreadContext,
    _next_page_table: PhysAddr,
) {
    asm!(
        // 保存 callee-saved 寄存器
        "mov [rdi + 0x00], rbx",
        "mov [rdi + 0x08], rbp",
        "mov [rdi + 0x10], r12",
        "mov [rdi + 0x18], r13",
        "mov [rdi + 0x20], r14",
        "mov [rdi + 0x28], r15",
        "mov [rdi + 0x30], rdi",  // 保存后恢复 rsp 前
        // 加载新线程的寄存器
        "mov rbx, [rsi + 0x00]",
        "mov rbp, [rsi + 0x08]",
        "mov r12, [rsi + 0x10]",
        "mov r13, [rsi + 0x18]",
        "mov r14, [rsi + 0x20]",
        "mov r15, [rsi + 0x28]",
        // 切换栈
        "mov rsp, [rsi + 0x38]",
        // 切换页表
        "mov rax, rdx",
        "mov cr3, rax",
        // 跳转到新线程的 RIP
        "mov rax, [rsi + 0x30]",
        "jmp rax",
        options(noreturn)
    );
}
```

---

## 5. 系统调用接口

### 5.1 系统调用分发

系统调用通过 `syscall` / `sysret` 指令进入内核（x86_64），编号通过 `rax` 寄存器传递：

```rust
/// 系统调用入口 (x86_64)
pub unsafe extern "sysv64" fn syscall_handler(ctx: &mut SyscallContext) {
    let syscall_num = ctx.rax;

    let result = match syscall_num {
        // === 传统系统调用 (0-511) ===
        0 => sys_read(ctx),
        1 => sys_write(ctx),
        2 => sys_open(ctx),
        3 => sys_close(ctx),
        // ... 其他传统系统调用 ...

        // === Agent 系统调用 (512+) ===
        512 => sys_agent_create(ctx),
        513 => sys_agent_destroy(ctx),
        514 => sys_agent_spawn(ctx),
        515 => sys_agent_suspend(ctx),
        516 => sys_agent_resume(ctx),
        517 => sys_agent_send(ctx),
        518 => sys_agent_recv(ctx),
        519 => sys_agent_share_mem(ctx),
        520 => sys_agent_set_quota(ctx),
        521 => sys_agent_get_status(ctx),
        522 => sys_agent_register(ctx),
        523 => sys_agent_discover(ctx),
        524 => sys_agent_enclave_enter(ctx),
        525 => sys_agent_enclave_exit(ctx),
        526 => sys_ai_inference(ctx),
        527 => sys_ai_model_load(ctx),
        528 => sys_ai_model_unload(ctx),

        _ => Err(SyscallError::ENOSYS),
    };

    // 返回值通过 rax 传递
    match result {
        Ok(val) => ctx.rax = val as u64,
        Err(e) => ctx.rax = e.to_errno() as u64,
    }
}
```

### 5.2 系统调用寄存器约定 (x86_64 System V ABI)

| 寄存器 | 用途 |
|--------|------|
| `rax` | 系统调用号 / 返回值 |
| `rdi` | 第 1 个参数 |
| `rsi` | 第 2 个参数 |
| `rdx` | 第 3 个参数 |
| `r10` | 第 4 个参数 |
| `r8` | 第 5 个参数 |
| `r9` | 第 6 个参数 |
| `rcx` | 用户态返回地址 (被 `syscall` 指令覆盖) |
| `r11` | 保存的 RFLAGS |

### 5.3 Agent 系统调用详细设计

#### `AGENT_CREATE` (512)

```rust
/// 创建新 Agent
///
/// 参数:
///   rdi: *const AgentCreateInfo - Agent 创建参数
///   rsi: *mut AgentId - 输出: 新 Agent 的 ID
///
/// 返回:
///   0: 成功
///   -EINVAL: 参数无效
///   -ENOMEM: 内存不足
///   -EQUOTA: 超出 Agent 数量配额
#[repr(C)]
pub struct AgentCreateInfo {
    /// Agent 名称
    pub name: [u8; 64],
    /// Agent 优先级类别
    pub priority_class: AgentPriorityClass,
    /// 内存配额 (字节)
    pub memory_quota: u64,
    /// CPU 时间配额 (纳秒/秒)
    pub cpu_quota: u64,
    /// 是否在 Enclave 中运行
    pub use_enclave: bool,
    /// 初始能力列表
    pub initial_capabilities: [CapabilityId; 16],
    /// 能力数量
    pub capability_count: u32,
    /// Agent 入口点
    pub entry_point: VirtAddr,
    /// Agent 栈大小
    pub stack_size: u64,
}
```

#### `AGENT_SEND` (517)

```rust
/// Agent 间发送消息
///
/// 参数:
///   rdi: AgentId - 目标 Agent
///   rsi: *const MessageHeader - 消息头
///   rdx: *const u8 - 消息载荷 (可为 NULL 表示纯共享内存)
///   r10: usize - 载荷大小
///   r8: *const SharedMemHandle - 共享内存句柄 (可选)
///
/// 返回:
///   0: 成功
///   -EINVAL: 参数无效
///   -ENOTFOUND: 目标 Agent 不存在
///   -EACCES: 无权限发送
///   -EAGAIN: 目标消息队列已满 (非阻塞模式)
///   -ETIMEDOUT: 发送超时
```

#### `AI_INFERENCE` (526)

```rust
/// AI 推理请求
///
/// 参数:
///   rdi: *const InferenceRequest - 推理请求
///   rsi: *mut InferenceResponse - 输出: 推理响应
///
/// 返回:
///   0: 成功
///   -EINVAL: 参数无效
///   -ENOENT: 模型未加载
///   -ENOMEM: 推理内存不足
///   -ETIMEDOUT: 推理超时
///   -ENETDOWN: 云端不可达且本地无可用模型
#[repr(C)]
pub struct InferenceRequest {
    /// 推理偏好
    pub preference: InferencePreference,
    /// 模型 ID
    pub model_id: [u8; 64],
    /// 输入数据 (共享内存引用)
    pub input_handle: SharedMemHandle,
    /// 输入数据大小
    pub input_size: u64,
    /// 推理参数
    pub params: InferenceParams,
    /// 超时时间 (毫秒)
    pub timeout_ms: u32,
}

#[repr(C)]
pub struct InferenceParams {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: f32,
    pub top_k: u32,
    pub repeat_penalty: f32,
}
```

### 5.4 系统调用性能优化

| 优化技术 | 描述 |
|---------|------|
| `syscall`/`sysret` | 比 `int 0x80` 快约 3 倍 |
| 参数预校验 | 在内核入口处快速校验用户指针有效性 |
| 批量系统调用 | `AGENT_SEND_BATCH` 减少多次陷入内核 |
| vDSO 支持 | 将 `gettimeofday` 等只读调用映射到用户态 |
| KPTI 缓解 | 内核页表与用户页表分离，减少 Meltdown 攻击面 |

---

## 6. 中断处理架构

### 6.1 IDT (中断描述符表)

```rust
/// 中断描述符表条目
pub struct IdtEntry {
    pub base_low: u16,       // 偏移 [0:15]
    pub selector: u16,       // 代码段选择子
    pub ist: u8,             // IST 偏移 (0 = 不使用)
    pub type_attr: u8,       // 类型和属性
    pub base_mid: u16,       // 偏移 [16:31]
    pub base_high: u32,      // 偏移 [32:63]
    pub reserved: u32,       // 保留
}

/// 中断向量号分配
pub mod interrupt_vectors {
    // CPU 异常 (0-31)
    pub const DIVIDE_ERROR: u8          = 0;
    pub const DEBUG: u8                 = 1;
    pub const NMI: u8                   = 2;
    pub const BREAKPOINT: u8            = 3;
    pub const OVERFLOW: u8              = 4;
    pub const BOUND_RANGE: u8           = 5;
    pub const INVALID_OPCODE: u8        = 6;
    pub const DEVICE_NOT_AVAILABLE: u8  = 7;
    pub const DOUBLE_FAULT: u8          = 8;   // 使用 IST 1
    pub const INVALID_TSS: u8           = 10;
    pub const SEGMENT_NOT_PRESENT: u8   = 11;
    pub const STACK_SEGMENT: u8         = 12;
    pub const GENERAL_PROTECTION: u8    = 13;
    pub const PAGE_FAULT: u8            = 14;  // 使用 IST 2
    pub const X87_FPU: u8               = 16;
    pub const ALIGNMENT_CHECK: u8       = 17;
    pub const MACHINE_CHECK: u8         = 18;  // 使用 IST 3

    // 可编程中断 (32-255)
    pub const TIMER: u8                 = 32;  // LAPIC Timer
    pub const KEYBOARD: u8              = 33;
    pub const SERIAL: u8                = 34;
    pub const ACPI: u8                  = 35;
    pub const IPC_NOTIFY: u8            = 36;  // IPC 通知中断
    pub const AGENT_EVENT: u8           = 37;  // Agent 事件中断
    pub const VIRTUALIZATION: u8        = 38;  // VM Exit 处理

    // 系统调用 (通过 syscall 指令，不使用 IDT)
    // syscall 号 0-511: 传统系统调用
    // syscall 号 512+: Agent 系统调用
}
```

### 6.2 APIC 中断控制器

```
┌─────────────────────────────────────────────────┐
│                  Local APIC (每个核心)            │
│  ┌─────────────┐  ┌──────────────┐              │
│  │  Timer      │  │  LVT entries │              │
│  │  (周期/单次) │  │  - Timer     │              │
│  │             │  │  - LINT0/1   │              │
│  │             │  │  - Error     │              │
│  │             │  │  - PMC       │              │
│  └─────────────┘  └──────────────┘              │
│  ┌─────────────┐  ┌──────────────┐              │
│  │  ICR        │  │  TPR         │              │
│  │  (核间中断)  │  │  (中断优先级) │              │
│  └─────────────┘  └──────────────┘              │
└─────────────────────────────────────────────────┘
         │ IPI (核间中断)
         ▼
┌─────────────────────────────────────────────────┐
│                  I/O APIC                        │
│  ┌──────────────────────────────────────┐       │
│  │  24 个 Redirection Entry             │       │
│  │  - IRQ → Vector 映射                 │       │
│  │  - 固定 / SMI / NMI / ExtINT 模式   │       │
│  │  - 目标 CPU 亲和性                   │       │
│  └──────────────────────────────────────┘       │
└─────────────────────────────────────────────────┘
         │ IRQ 线
         ▼
┌─────────────────────────────────────────────────┐
│              外部设备                             │
│  键盘 / 串口 / 网卡 / 磁盘控制器 / GPU           │
└─────────────────────────────────────────────────┘
```

### 6.3 中断处理状态机

```
中断触发 (硬件/软件)
    │
    ▼
┌──────────────┐
│  CPU 响应中断  │  保存 RFLAGS, CS, RIP 到栈
│  (自动硬件)   │  清除 IF 标志 (禁止嵌套中断)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  IST 栈切换   │  (对于 Double Fault, Page Fault 等)
│  (如配置)     │  使用独立的 IST 栈，避免栈损坏
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  中断存根     │  (汇编) 保存所有通用寄存器
│  (Stub)      │  切换到内核栈 (如果来自用户态)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Rust 处理函数│  解析中断帧
│  (Handler)   │  调用注册的处理回调
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  发送 EOI    │  通知 Local APIC 中断处理完成
│  (APIC)      │  允许后续中断
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  恢复寄存器   │  恢复所有通用寄存器
│  IRETQ 返回   │  恢复 RFLAGS, CS, RIP
└──────────────┘
```

---

## 7. 设备驱动框架

### 7.1 用户态驱动模型

OmniAgent OS 的所有设备驱动运行在用户态，通过 IPC 与硬件交互：

```
┌──────────────────┐     IPC (同步 RPC)     ┌──────────────────┐
│   用户态驱动      │◄────────────────────►│   驱动管理器      │
│   (Driver Process)│                       │   (Service)       │
│                  │                        │                  │
│  ┌────────────┐  │     MMIO 映射请求      │  ┌────────────┐  │
│  │ 驱动逻辑   │  │◄─────────────────────│  │ 设备枚举   │  │
│  └────────────┘  │                       │  └────────────┘  │
│  ┌────────────┐  │     中断通知          │  ┌────────────┐  │
│  │ MMIO 访问  │  │◄─────────────────────│  │ 中断路由   │  │
│  │ (映射区域)  │  │                       │  └────────────┘  │
│  └────────────┘  │     DMA 配置          │  ┌────────────┐  │
│  ┌────────────┐  │◄─────────────────────│  │ DMA 管理   │  │
│  │ DMA 缓冲区  │  │                       │  └────────────┘  │
│  └────────────┘  │                       │                  │
└──────────────────┘                       └──────────────────┘
```

### 7.2 驱动接口定义

```rust
/// 驱动接口 trait
pub trait DeviceDriver: Send + Sync {
    /// 驱动名称
    fn name(&self) -> &str;

    /// 支持的设备 ID 列表
    fn supported_devices(&self) -> &[PciDeviceId];

    /// 探测设备
    fn probe(&mut self, device: &DeviceInfo) -> Result<DriverProbeResult>;

    /// 初始化设备
    fn init(&mut self, resources: DriverResources) -> Result<()>;

    /// 处理中断
    fn handle_interrupt(&mut self, irq: u8) -> InterruptAction;

    /// 移除设备
    fn remove(&mut self);

    /// 暂停设备 (电源管理)
    fn suspend(&mut self) -> Result<()>;

    /// 恢复设备
    fn resume(&mut self) -> Result<()>;
}

pub enum DriverProbeResult {
    /// 驱动支持此设备
    Claimed,
    /// 驱动不支持此设备
    NotSupported,
}

pub enum InterruptAction {
    /// 中断已处理
    Handled,
    /// 中断不是此设备的
    NotMine,
    /// 需要重新调度中断处理
    Reschedule,
}
```

---

## 8. 引导序列

### 8.1 完整引导流程

```
阶段 1: BIOS/UEFI 固件
    │  POST (加电自检)
    │  初始化硬件
    │  加载 bootloader
    ▼

阶段 2: bootloader crate
    │  ┌─────────────────────────────────────┐
    │  │ 1. 读取 multiboot2/UEFI 信息        │
    │  │ 2. 检测物理内存布局                  │
    │  │ 3. 设置初始页表 (4 级, 2MB 大页)    │
    │  │ 4. 加载内核 ELF 到 0xFFFFFFFF80000000│
    │  │ 5. 加载 initrd (如有)               │
    │  │ 6. 设置 GDT (内核代码/数据段)        │
    │  │ 7. 切换到长模式 (Long Mode)         │
    │  │ 8. 设置栈 (boot 栈)                 │
    │  │ 9. 跳转到 kernel_main               │
    │  └─────────────────────────────────────┘
    ▼

阶段 3: kernel_main 入口
    │  ┌─────────────────────────────────────┐
    │  │ 1. 清零 BSS 段                      │
    │  │ 2. 初始化 VGA/帧缓冲 (早期输出)     │
    │  │ 3. 解析 bootloader 传递的信息       │
    │  │ 4. 初始化 GDT (含 TSS)              │
    │  │ 5. 初始化 IDT (中断描述符表)         │
    │  │ 6. 初始化 PIC → APIC (禁用 PIC)     │
    │  │ 7. 初始化物理帧分配器               │
    │  │ 8. 初始化内核页表                   │
    │  │ 9. 初始化内核堆 (bumpalo)           │
    │  │ 10. 初始化调度器 (CFS)              │
    │  │ 11. 初始化 IPC 子系统               │
    │  │ 12. 初始化定时器 (LAPIC Timer)      │
    │  │ 13. 注册 Agent 系统调用             │
    │  │ 14. 启用中断                        │
    │  └─────────────────────────────────────┘
    ▼

阶段 4: 子系统初始化
    │  ┌─────────────────────────────────────┐
    │  │ 1. 创建 init 进程 (PID 1)           │
    │  │ 2. 启动 AP (应用处理器)             │
    │  │ 3. 加载用户态服务                   │
    │  │    - 驱动管理器                     │
    │  │    - 文件系统服务                   │
    │  │    - 网络服务                       │
    │  │    - 安全 Enclave                   │
    │  │    - AI 推理服务                    │
    │  │    - 虚拟化管理器                   │
    │  │    - 窗口管理服务                   │
    │  │ 4. 启动 Aqua Shell 桌面             │
    │  │ 5. 系统就绪，进入正常调度           │
    │  └─────────────────────────────────────┘
    ▼

阶段 5: 正常运行
    │  调度器循环
    │  中断处理
    │  Agent 调度
    │  IPC 消息路由
    ▼
```

### 8.2 `kernel_main` 伪代码

```rust
#[no_mangle]
pub unsafe extern "C" fn kernel_main(boot_info: &BootInfo) -> ! {
    // 阶段 1: 早期初始化
    early_init(boot_info);

    // 阶段 2: 核心子系统初始化
    gdt::init();                    // GDT + TSS
    idt::init();                    // IDT
    pic::disable();                 // 禁用 8259 PIC
    apic::init();                   // 初始化 Local APIC + I/O APIC
    memory::frame_alloc_init();     // 物理帧分配器
    memory::kernel_page_table_init(); // 内核页表
    memory::heap_init();            // 内核堆 (bumpalo)
    scheduler::init();              // CFS 调度器
    ipc::init();                    // IPC 子系统
    timer::init();                  // LAPIC 定时器
    agent::syscall_init();          // Agent 系统调用注册

    // 阶段 3: 启用中断
    x86_64::instructions::interrupts::enable();

    // 阶段 4: 启动用户态
    process::create_init_process();
    smp::start_ap_processors();

    // 阶段 5: 进入调度循环
    scheduler::run();

    unreachable!()
}
```

---

## 9. 虚拟化支持

### 9.1 硬件辅助虚拟化

OmniAgent OS 支持 Intel VT-x 和 AMD-V 硬件虚拟化：

```rust
/// 虚拟化能力检测
pub fn detect_virtualization_support() -> VirtualizationSupport {
    let cpuid = unsafe { core::arch::x86_64::__cpuid(1) };

    if cpuid.ecx & (1 << 5) != 0 {
        // Intel VT-x: CPUID.01H:ECX.VMX[bit 5]
        VirtualizationSupport::IntelVtx {
            ept_supported: check_ept_support(),
            vpids_supported: check_vpid_support(),
            vmx_controls: read_vmx_controls(),
        }
    } else {
        // 检查 AMD-V
        let cpuid_ext = unsafe { core::arch::x86_64::__cpuid(0x80000001) };
        if cpuid_ext.ecx & (1 << 2) != 0 {
            VirtualizationSupport::AmdSvm {
                npt_supported: check_npt_support(),
                asid_supported: true,
            }
        } else {
            VirtualizationSupport::None
        }
    }
}

pub enum VirtualizationSupport {
    IntelVtx {
        ept_supported: bool,
        vpids_supported: bool,
        vmx_controls: VmxControls,
    },
    AmdSvm {
        npt_supported: bool,
        asid_supported: bool,
    },
    None,
}
```

### 9.2 VM Exit 处理

```rust
/// VM Exit 原因分类
pub enum VmExitReason {
    /// 外部中断
    ExternalInterrupt,
    /// NMI 窗口
    NmiWindow,
    /// CPUID 指令
    Cpuid,
    /// HLT 指令
    Hlt,
    /// IN/OUT 指令 (I/O 端口访问)
    IoInstruction { port: u16, size: u8, is_write: bool },
    /// MSR 读写
    MsrAccess { msr: u32, is_write: bool },
    /// EPT 违例
    EptViolation { gpa: u64, fault_code: u64 },
    /// 页表修改
    PageTableUpdate,
    /// 控制寄存器访问
    ControlRegister { cr: u8, is_write: bool },
}
```

---

## 10. 错误处理

### 10.1 内核 Panic 处理

```rust
#[panic_handler]
fn kernel_panic(info: &PanicInfo) -> ! {
    // 1. 禁用中断
    unsafe { x86_64::instructions::interrupts::disable() };

    // 2. 获取当前 CPU 信息
    let cpu_id = smp::current_cpu_id();

    // 3. 输出 panic 信息到串口和帧缓冲
    println!("\n=== KERNEL PANIC (CPU {}) ===", cpu_id);
    if let Some(location) = info.location() {
        println!("Location: {}:{}:{}", location.file(), location.line(), location.column());
    }
    if let Some(message) = info.message() {
        println!("Message: {}", message);
    }

    // 4. 输出寄存器转储
    dump_registers();

    // 5. 输出调用栈
    dump_stack_trace();

    // 6. 输出当前进程/线程信息
    dump_process_info();

    // 7. 通知其他 CPU 停止
    smp::stop_all_processors();

    // 8. 闪烁错误指示灯
    error_indicator_blink();

    // 9. 永久停机
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}
```

### 10.2 Kernel Oops 机制

对于非致命错误（如驱动引发的缺页异常），系统采用 Oops 机制：

```rust
/// 内核 Oops 严重级别
pub enum OopsSeverity {
    /// 警告：可恢复，继续执行
    Warning,
    /// 轻微：杀死当前进程，系统继续
    Minor,
    /// 严重：杀死当前进程，可能影响其他进程
    Major,
    /// 致命：等价于 panic
    Fatal,
}

/// 处理内核 Oops
pub fn handle_oops(severity: OopsSeverity, context: &ExceptionContext) {
    // 记录 Oops 信息
    log_oops(severity, context);

    match severity {
        OopsSeverity::Warning => {
            // 仅记录日志，继续执行
        }
        OopsSeverity::Minor | OopsSeverity::Major => {
            // 杀死引发 Oops 的进程
            if let Some(pid) = scheduler::current_process_id() {
                scheduler::kill_process(pid, KillReason::KernelOops);
            }
        }
        OopsSeverity::Fatal => {
            // 触发 panic
            panic!("Fatal kernel oops");
        }
    }
}
```

### 10.3 错误码定义

```rust
/// 内核错误码
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KernelError {
    /// 参数无效
    InvalidArgument,
    /// 内存不足
    OutOfMemory,
    /// 地址空间无效
    InvalidAddress,
    /// 权限不足
    PermissionDenied,
    /// 资源不存在
    NotFound,
    /// 资源忙
    Busy,
    /// 超时
    Timeout,
    /// 不支持的操作
    Unsupported,
    /// 配额超限
    QuotaExceeded,
    /// 内部错误
    InternalError,
}

impl KernelError {
    /// 转换为 POSIX errno
    pub fn to_errno(&self) -> i32 {
        match self {
            KernelError::InvalidArgument => 22,   // EINVAL
            KernelError::OutOfMemory => 12,        // ENOMEM
            KernelError::InvalidAddress => 14,     // EFAULT
            KernelError::PermissionDenied => 13,   // EACCES
            KernelError::NotFound => 2,            // ENOENT
            KernelError::Busy => 16,               // EBUSY
            KernelError::Timeout => 110,           // ETIMEDOUT
            KernelError::Unsupported => 95,        // EOPNOTSUPP
            KernelError::QuotaExceeded => 55,      // ENOBUFS
            KernelError::InternalError => 5,       // EIO
        }
    }
}
```

---

## 11. 内核内存布局

### 11.1 物理内存布局 (x86_64)

```
物理地址空间
┌─────────────────────────────────────────────────────────────┐
│ 0x0000_0000                                                  │
│ ┌─────────────────────────┐                                 │
│ │     传统内存区域         │  IVT, BDA, EBDA                │
│ │     (0 - 1 MB)          │  保留，不使用                   │
│ └─────────────────────────┘                                 │
│ 0x0010_0000 (1 MB)                                          │
│ ┌─────────────────────────┐                                 │
│ │     内核代码段           │  .text, .rodata                │
│ │     (加载地址)           │  由 bootloader 放置            │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     内核数据段           │  .data, .bss                   │
│ │                         │                                 │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     内核堆               │  bumpalo 分配区域              │
│ │     (可增长)             │  初始 4 MB                     │
│ └─────────────────────────┘                                 │
│                                                             │
│         ... 可用物理内存 ...                                  │
│                                                             │
│ ┌─────────────────────────┐                                 │
│ │     帧分配器管理的       │  可用物理页帧                   │
│ │     自由页帧             │  通过 bitmap/链表管理           │
│ └─────────────────────────┘                                 │
│                                                             │
│         ... 设备 MMIO 区域 ...                                │
│                                                             │
│ 0xFFFF_FFFF                                                  │
└─────────────────────────────────────────────────────────────┘
```

### 11.2 内核虚拟地址空间布局

```
内核虚拟地址空间 (高半部分)
┌─────────────────────────────────────────────────────────────┐
│ 0xFFFF_FFFF_FFFF_FFFF                                        │
│ ┌─────────────────────────┐                                 │
│ │     不可访问区域         │  非规范地址区域                 │
│ │     (Canonical hole)     │  用于捕获错误指针               │
│ └─────────────────────────┘                                 │
│ 0xFFFF_8000_0000_0000                                        │
│ ┌─────────────────────────┐                                 │
│ │     内核代码映射         │  物理内存直接映射               │
│ │     (phys + 0xFFFF_8000_│  phys_addr + PHYS_OFFSET        │
│ │      0000_0000)         │                                 │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     内核堆               │  内核动态分配区域               │
│ │     (bumpalo / slab)     │                                 │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     内核栈 (per-CPU)     │  每个核心 16 KB                 │
│ │     ┌─────────────────┐ │  包含异常栈 (IST)               │
│ │     │ IST #1 (DF)     │ │  Double Fault 栈                │
│ │     │ IST #2 (PF)     │ │  Page Fault 栈                  │
│ │     │ IST #3 (MC)     │ │  Machine Check 栈               │
│ │     │ Normal Stack    │ │  普通内核栈                     │
│ │     └─────────────────┘ │                                 │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     设备 MMIO 映射       │  设备寄存器映射区域             │
│ │     (VMA: 0xFFFF_...    │  页属性: UC / WC                │
│ │      _MMIO_BASE)        │                                 │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     APIC 映射            │  Local APIC / I/O APIC         │
│ │     (固定地址)           │  页属性: UC                     │
│ └─────────────────────────┘                                 │
│ ┌─────────────────────────┐                                 │
│ │     内核临时映射         │  用于页表操作等临时映射         │
│ │     (fixmap)            │  固定数量的虚拟页               │
│ └─────────────────────────┘                                 │
│ 0xFFFF_8000_0000_0000                                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 12. 关键 Crate 依赖详解

### 12.1 `x86_64` crate

| 模块 | 用途 |
|------|------|
| `x86_64::structures::idt` | IDT 和中断处理 |
| `x86_64::structures::gdt` | GDT 和段描述符 |
| `x86_64::structures::paging` | 页表操作 (PML4/PDPT/PD/PT) |
| `x86_64::instructions::interrupts` | 中断使能/禁用 |
| `x86_64::instructions::port` | I/O 端口访问 |
| `x86_64::instructions::tlb` | TLB 刷新 |
| `x86_64::registers::control` | CR0/CR2/CR3/CR4 操作 |
| `x86_64::registers::model_specific` | MSR 读写 |
| `x86_64::addr::VirtAddr` / `PhysAddr` | 虚拟/物理地址类型 |

### 12.2 `bootloader` crate

- 自动处理 multiboot2 头部和引导协议
- 设置页表并切换到长模式
- 将内核加载到高半地址
- 传递内存映射和帧缓冲信息

### 12.3 `spin` crate

```rust
use spin::mutex::SpinMutex;
use spin::rwlock::RwLock;

/// 全局进程表
static PROCESS_TABLE: SpinMutex<Vec<Process>> = SpinMutex::new(Vec::new());

/// 全局端口命名空间
static PORT_NAMESPACE: RwLock<PortNamespace> = RwLock::new(PortNamespace::new());
```

### 12.4 `volatile` crate

```rust
use volatile::Volatile;
use volatile::ReadOnly;
use volatile::WriteOnly;

/// VGA 文本缓冲区
pub struct VgaBuffer {
    chars: [[Volatile<VgaChar>; 80]; 25],
}

/// MMIO 寄存器访问
pub struct DeviceRegisters {
    pub control: Volatile<u32>,
    pub status: ReadOnly<u32>,
    pub data: WriteOnly<u32>,
}
```

### 12.5 `bumpalo` crate

```rust
use bumpalo::Bump;

/// 内核早期堆分配器
static mut KERNEL_HEAP: Option<Bump> = None;

pub fn heap_init(start: VirtAddr, size: usize) {
    unsafe {
        KERNEL_HEAP = Some(Bump::from_slice(
            core::slice::from_raw_parts_mut(start.as_mut_ptr(), size)
        ));
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;

struct KernelAllocator;

unsafe impl core::alloc::GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        KERNEL_HEAP.as_ref().unwrap().alloc(layout)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // bumpalo 不支持单独释放，仅在内核早期使用
    }
}
```

### 12.6 `arceos-fairsched` crate

提供 CFS 调度器核心实现，OmniAgent OS 在此基础上扩展了 Agent 优先级类别：

- 虚拟运行时间 (vruntime) 管理
- 红黑树就绪队列
- 权重计算和 nice 值映射
- 时间片分配策略
- 负载均衡支持

---

## 13. 测试策略

### 13.1 内核测试框架

```rust
#[cfg(test)]
mod kernel_tests {
    use super::*;

    /// 测试页表映射
    #[test]
    fn test_page_table_mapping() {
        // 分配物理帧
        let frame = frame_alloc().expect("frame allocation failed");
        // 映射到虚拟地址
        let virt_addr = VirtAddr::new(0x1000_0000);
        map_page(virt_addr, frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        // 验证映射
        assert!(is_mapped(virt_addr));
        // 解除映射
        unmap_page(virt_addr);
        assert!(!is_mapped(virt_addr));
    }

    /// 测试系统调用分发
    #[test]
    fn test_syscall_dispatch() {
        let mut ctx = SyscallContext::new_test();
        ctx.rax = 512; // AGENT_CREATE
        // ... 验证系统调用路由正确
    }

    /// 测试调度器
    #[test]
    fn test_cfs_scheduling() {
        let mut scheduler = CfsScheduler::new();
        let t1 = create_test_thread(1, 0);  // nice = 0
        let t2 = create_test_thread(2, 5);  // nice = 5
        scheduler.enqueue(t1);
        scheduler.enqueue(t2);
        // nice=0 的线程应获得更多运行时间
        let next = scheduler.pick_next();
        assert_eq!(next.unwrap().tid.0, 1);
    }

    /// 测试 IPC 延迟
    #[test]
    fn test_ipc_latency() {
        let start = tsc_read();
        // 发送并接收消息
        ipc_send_test();
        ipc_recv_test();
        let elapsed = tsc_read() - start;
        let latency_ns = elapsed_to_ns(elapsed);
        assert!(latency_ns < 1000, "IPC latency exceeded 1μs: {}ns", latency_ns);
    }
}
```

### 13.2 集成测试矩阵

| 测试类别 | 测试项 | 通过标准 |
|---------|--------|---------|
| 引导 | BIOS 引导 | 成功进入 kernel_main |
| 引导 | UEFI 引导 | 成功进入 kernel_main |
| 内存 | 帧分配器 | 分配/释放无泄漏 |
| 内存 | 页表映射 | 映射/解除映射正确 |
| 内存 | 堆分配 | 10000 次分配无崩溃 |
| 调度 | 线程创建 | 创建 1024 线程 |
| 调度 | 上下文切换 | < 500ns |
| 调度 | Agent 优先级 | 高优先级抢占正确 |
| IPC | 同步 RPC | 延迟 < 1μs |
| IPC | 共享内存 | 吞吐 > 10 GB/s |
| 中断 | 定时器中断 | 精度 < 1μs |
| 中断 | 键盘中断 | 响应 < 10μs |
| Agent | Agent 创建/销毁 | < 100μs |
| Agent | Agent 间通信 | 正确路由 |
| 虚拟化 | VM 创建/启动 | < 2s |
| 虚拟化 | VM Exit 处理 | 延迟 < 50μs |

---

## 14. 安全考虑

### 14.1 内核态安全措施

| 措施 | 说明 |
|------|------|
| Rust 所有权系统 | 编译时消除数据竞争和内存安全漏洞 |
| KASLR | 内核代码段地址随机化 |
| SMAP/SMEP | 防止内核访问/执行用户态内存 |
| NX 位 | 数据页不可执行 |
| Stack Canary | 栈溢出保护 |
| `unsafe` 审计 | 所有 `unsafe` 代码块需安全审查 |
| 最小特权 | 内核仅包含必要功能 |

### 14.2 系统调用安全

- 所有用户指针在内核态使用前必须验证
- Agent 系统调用需要 Capability 验证
- 系统调用参数边界检查
- 返回值不泄露内核地址信息

---

*本文档由 OmniAgent OS 内核团队维护，如有疑问请联系 kernel@omniagent.os*
