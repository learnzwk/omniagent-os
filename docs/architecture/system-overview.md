# OmniAgent OS 系统总览

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: L1 架构设计文档
> **目标读者**: 系统架构师、内核开发者、Agent 框架开发者

---

## 1. 文档目的

本文档为 OmniAgent OS 的顶层架构设计文档，旨在提供系统的全局视角。涵盖系统愿景、设计哲学、分层架构、组件交互、Agent-Native 设计原则、虚拟化架构、AI 双通道模型、关键技术决策以及安全模型。本文档是所有后续 L2/L3 详细设计文档的基础参考。

---

## 2. 系统愿景与设计哲学

### 2.1 愿景

OmniAgent OS 的愿景是构建一个**以 Agent 为一等公民的操作系统**，使 AI Agent 能够像传统进程一样被调度、隔离、通信和资源管理。系统不仅为人类用户提供桌面体验，也为 AI Agent 提供原生的执行环境、安全沙箱和高效的跨 Agent 协作机制。

### 2.2 设计哲学

| 原则 | 描述 |
|------|------|
| **Agent-First** | Agent 不是应用层的附加组件，而是操作系统内核直接感知和调度的实体 |
| **最小权限内核** | 微内核架构，仅将调度器、IPC、内存管理、中断处理、Agent 系统调用和定时器保留在内核态 |
| **零拷贝优先** | 所有高性能数据路径（IPC、GPU 共享内存、AI 推理管道）均采用零拷贝设计 |
| **安全隔离** | 基于 4 级页表的 Agent 隔离，配合软件 TEE（可信执行环境）实现纵深防御 |
| **双模 AI** | 本地模型与云端模型双通道并行，用户可自主选择推理后端 |
| **Rust 安全性** | 全系统使用 Rust 编写（`no_std` 内核），利用所有权系统消除内存安全漏洞 |
| **可虚拟化** | 原生支持硬件虚拟化（Intel VT-x / AMD-V），可作为 Type-1 Hypervisor 运行 |

### 2.3 与传统操作系统的区别

```
传统 OS:   硬件 → 内核(进程/线程/驱动/文件系统/网络/...) → 用户应用
OmniAgent: 硬件 → 微内核(调度/IPC/内存/中断/Agent syscall/定时器)
                 → 用户态服务(驱动/文件系统/网络/AI运行时)
                 → Agent 运行时(一等公民)
                 → 桌面环境(Aqua Shell)
```

---

## 3. 架构分层

### 3.1 四层架构模型

OmniAgent OS 采用严格的四层架构，自底向上分别为：

```
┌─────────────────────────────────────────────────────────────────┐
│                    Layer 4: 桌面与应用层                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  Aqua Shell   │  │  Vulkan 合成器│  │  多模态交互 (Candle/  │  │
│  │  桌面环境     │  │  (ash+Smithay)│  │   ort/tract)          │  │
│  └──────────────┘  └──────────────┘  └───────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 3: 服务层                               │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐   │
│  │ Agent    │ │ AI 推理  │ │ 文件系统 │ │ 网络              │   │
│  │ 运行时   │ │ 服务     │ │ 服务     │ │ 服务              │   │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐   │
│  │ 安全     │ │ 虚拟化   │ │ 设备驱动 │ │ 窗口管理          │   │
│  │ Enclave  │ │ 管理器   │ │ 框架     │ │ 服务              │   │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 2: 微内核层                             │
│  ┌────────┐ ┌─────┐ ┌────────┐ ┌──────┐ ┌──────┐ ┌─────────┐  │
│  │ CFS    │ │ IPC │ │ 虚拟   │ │ 中断 │ │ 定时 │ │ Agent   │  │
│  │ 调度器 │ │ 引擎│ │ 内存   │ │ 处理 │ │ 器  │ │ Syscall │  │
│  └────────┘ └─────┘ └────────┘ └──────┘ └──────┘ └─────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 1: 硬件抽象层 (HAL)                     │
│  ┌──────────────────────┐  ┌────────────────────────────────┐   │
│  │ x86_64 平台          │  │ aarch64 平台                   │   │
│  │ (x86_64 crate)       │  │ (aarch64 支持)                 │   │
│  │ IDT/GDT/APIC/MSR     │  │ GIC/Timer/MMU                 │   │
│  └──────────────────────┘  └────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 各层职责

#### Layer 1: 硬件抽象层 (HAL)

- **职责**: 屏蔽硬件差异，为微内核提供统一的硬件操作接口
- **关键组件**:
  - CPU 架构抽象（寄存器、特权级、页表操作）
  - 中断控制器抽象（x86 APIC / aarch64 GIC）
  - 定时器抽象（x86 LAPIC Timer / aarch64 Generic Timer）
  - 平台启动支持（bootloader crate 集成）
- **接口形式**: Rust trait 定义，各平台独立实现
- **关键 crate**: `x86_64`, `tock-registers`（aarch64 寄存器访问）

#### Layer 2: 微内核层

- **职责**: 提供操作系统核心原语，是系统唯一运行在 Ring 0 的代码
- **核心子系统**:
  - **CFS 调度器变体**: 支持 Agent 优先级类别的完全公平调度器
  - **IPC 引擎**: 零拷贝消息传递，同步 RPC + 异步消息队列 + 共享内存
  - **虚拟内存管理**: 4 级页表、按需分页、Agent 地址空间隔离
  - **中断处理**: IDT/APIC 驱动的中断分发框架
  - **Agent 系统调用**: syscall 号 512+ 专用于 Agent 操作
  - **定时器**: 高精度定时器服务，支持 one-shot 和 periodic 模式
- **关键 crate**: `spin`, `volatile`, `bumpalo`, `arceos-fairsched`

#### Layer 3: 服务层

- **职责**: 以用户态进程形式运行的所有操作系统服务
- **服务类型**:
  - **Agent 运行时**: Agent 生命周期管理、Agent 间通信协调
  - **AI 推理服务**: 本地模型加载/推理、云端模型 API 代理
  - **文件系统服务**: 支持 ext4/FAT32/自定义 AgentFS
  - **网络服务**: TCP/IP 协议栈、Agent 通信隧道
  - **安全 Enclave**: 软件 TEE、密钥管理、安全存储
  - **虚拟化管理器**: VM 创建/销毁、vCPU 调度、设备模拟
  - **设备驱动框架**: 用户态驱动通过 IPC 与硬件交互
  - **窗口管理服务**: Wayland 兼容的窗口合成协议

#### Layer 4: 桌面与应用层

- **职责**: 用户交互界面和应用运行环境
- **关键组件**:
  - **Aqua Shell**: 基于 Wayland 协议的桌面 Shell
  - **Vulkan 合成器**: 基于 `ash`（Vulkan 绑定）+ `Smithay`（Wayland 合成器框架）
  - **文本渲染**: `cosmic-text` 提供多语言文本整形和光栅化
  - **多模态交互**: `Candle`（PyTorch 兼容推理）+ `ort`（ONNX Runtime）+ `tract`（轻量推理）
  - **Agent 可视化**: Agent 状态面板、Agent 间通信可视化

---

## 4. 组件交互图

### 4.1 系统启动流程

```
BIOS/UEFI
    │
    ▼
bootloader crate (引导加载器)
    │  ┌─────────────────────────────┐
    │  │ 1. 设置页表（4级）           │
    │  │ 2. 加载内核到高半地址        │
    │  │ 3. 切换到长模式              │
    │  │ 4. 跳转到 kernel_main       │
    │  └─────────────────────────────┘
    ▼
kernel_main (微内核初始化)
    │  ┌─────────────────────────────┐
    │  │ 1. GDT/IDT 初始化           │
    │  │ 2. 物理内存检测与帧分配器    │
    │  │ 3. 虚拟内存管理器初始化      │
    │  │ 4. 堆分配器初始化 (bumpalo)  │
    │  │ 5. 调度器初始化 (CFS)        │
    │  │ 6. IPC 子系统初始化          │
    │  │ 7. 定时器初始化              │
    │  │ 8. Agent syscall 注册       │
    │  └─────────────────────────────┘
    ▼
用户态服务启动 (init 进程)
    │  ┌─────────────────────────────┐
    │  │ 1. 驱动管理器               │
    │  │ 2. 文件系统服务             │
    │  │ 3. 网络服务                 │
    │  │ 4. 安全 Enclave             │
    │  │ 5. AI 推理服务              │
    │  │ 6. 虚拟化管理器             │
    │  │ 7. 窗口管理服务             │
    │  │ 8. Aqua Shell 桌面          │
    │  └─────────────────────────────┘
    ▼
系统就绪，接受用户和 Agent 请求
```

### 4.2 运行时组件交互

```
┌──────────────┐     syscall      ┌──────────────────┐
│   Agent A    │◄────────────────►│                  │
│  (用户态)    │   512+ Agent API  │                  │
└──────┬───────┘                   │                  │
       │ IPC (零拷贝)              │    微内核        │
       │                           │  ┌────────────┐  │
┌──────▼───────┐     IPC           │  │ CFS 调度器  │  │
│   Agent B    │◄────────────────►│  │ IPC 引擎    │  │
│  (用户态)    │                   │  │ VM 管理器   │  │
└──────────────┘                   │  │ 中断处理    │  │
                                   │  └────────────┘  │
┌──────────────┐     IPC           │                  │
│ AI 推理服务  │◄────────────────►│                  │
│ (本地/云端)  │                   └────────┬─────────┘
└──────────────┘                            │
                                    ┌───────▼───────┐
                                    │     HAL       │
                                    │ x86_64/aarch64│
                                    └───────┬───────┘
                                            │
                                    ┌───────▼───────┐
                                    │    硬件       │
                                    │ CPU/MMU/设备  │
                                    └───────────────┘
```

### 4.3 AI 双通道数据流

```
┌──────────────┐
│  Agent 请求  │
│ (推理任务)   │
└──────┬───────┘
       │ Agent 服务调用 (通过 SYS_AGENT_MSG 515)
       ▼
┌──────────────────────────────────────────┐
│           AI 推理服务 (Router)            │
│  ┌────────────────────────────────────┐  │
│  │        模型选择策略                 │  │
│  │  - 用户偏好配置                    │  │
│  │  - 任务复杂度评估                  │  │
│  │  - 延迟/精度权衡                   │  │
│  │  - 网络可用性检测                  │  │
│  └────────────────────────────────────┘  │
│         │                    │            │
│    ┌────▼─────┐       ┌─────▼──────┐     │
│    │ 本地通道  │       │  云端通道   │     │
│    │          │       │            │     │
│    │ Candle   │       │ REST/gRPC  │     │
│    │ ort      │       │ API 代理   │     │
│    │ tract    │       │            │     │
│    └────┬─────┘       └─────┬──────┘     │
└─────────┼───────────────────┼────────────┘
          │                   │
    ┌─────▼──────┐     ┌─────▼──────┐
    │ 本地 GPU   │     │ 云端 AI    │
    │ /CPU 推理  │     │ 服务       │
    └─────┬──────┘     └─────┬──────┘
          │                   │
          └────────┬──────────┘
                   ▼
          ┌────────────────┐
          │  推理结果      │
          │  (零拷贝返回)  │
          └────────────────┘
```

---

## 5. Agent-Native 设计原则

### 5.1 Agent 作为一等公民

在 OmniAgent OS 中，Agent 不仅是运行在用户态的应用程序，更是操作系统内核直接感知的调度实体。这体现在以下方面：

1. **专用系统调用**: syscall 号 512+ 保留给 Agent 操作，包括 Agent 创建、销毁、通信、资源配额设置等
2. **调度器感知**: CFS 调度器变体为 Agent 定义了独立的优先级类别（见下表）
3. **地址空间隔离**: 每个 Agent 拥有独立的地址空间，通过 4 级页表实现硬件级隔离
4. **资源配额**: 内核为每个 Agent 维护 CPU 时间、内存、IPC 带宽的配额
5. **安全 Enclave**: Agent 可选择在软件 TEE 中运行敏感计算

### 5.2 Agent 优先级类别

| 优先级类别 | nice 值范围 | 说明 | 典型场景 |
|-----------|------------|------|---------|
| `AGENT_CRITICAL` | -20 ~ -16 | 关键 Agent，几乎不抢占 | 安全监控 Agent、系统健康 Agent |
| `AGENT_HIGH` | -15 ~ -6 | 高优先级 Agent | 实时交互 Agent、语音助手 |
| `AGENT_NORMAL` | -5 ~ 5 | 默认 Agent 优先级 | 通用任务 Agent、工具 Agent |
| `AGENT_LOW` | 6 ~ 15 | 低优先级 Agent | 后台分析 Agent、日志 Agent |
| `AGENT_BATCH` | 16 ~ 19 | 批处理 Agent | 数据处理 Agent、训练 Agent |

### 5.3 Agent 生命周期

```
                  ┌──────────┐
           ┌─────│  CREATED  │─────┐
           │     └─────┬────┘     │
           │           │ start()  │
           │           ▼          │
           │     ┌──────────┐     │
           │     │ RUNNING   │     │
           │     └──┬───┬───┘     │
           │        │   │ suspend()│
           │        │   ▼          │
           │        │ ┌──────────┐ │
           │        │ │SUSPENDED │ │
           │        │ └────┬─────┘ │
           │        │      │resume()│
           │        │      ▼       │
           │        │ ┌──────────┐ │
           │        │ │ RUNNING  │ │
           │        │ └────┬─────┘ │
           │        │      │error() │
           │        ▼      ▼       │
           │     ┌──────────┐      │
           └────►│TERMINATED│◄─────┘
                 └──────────┘
```

### 5.4 Agent 系统调用接口

```rust
/// Agent 系统调用号分配 (512+)
pub mod agent_syscall {
    pub const SYS_AGENT_SPAWN:         usize = 512; // 创建新 Agent
    pub const SYS_AGENT_KILL:          usize = 513; // 终止 Agent
    pub const SYS_AGENT_QUERY:         usize = 514; // 查询 Agent 状态
    pub const SYS_AGENT_MSG:           usize = 515; // 向 Agent 发送消息
    pub const SYS_AGENT_REGISTER:      usize = 516; // 注册 Agent 能力
    pub const SYS_AGENT_SUBSCRIBE:     usize = 517; // 订阅 Agent 事件
    pub const SYS_AGENT_MIGRATE:       usize = 518; // 迁移 Agent 到其他设备
    pub const SYS_AGENT_MEMORY_SHARE:  usize = 519; // 与 Agent 共享内存区域
    pub const SYS_AGENT_CAP_GRANT:     usize = 520; // 授予 Agent 能力
    pub const SYS_AGENT_CAP_REVOKE:    usize = 521; // 撤销 Agent 能力
    pub const SYS_AGENT_BIND_PORT:     usize = 522; // 绑定 Agent 通信端口
    pub const SYS_AGENT_EXPORT:        usize = 523; // 导出 Agent 服务
    pub const SYS_AGENT_IMPORT:        usize = 524; // 导入远程 Agent 服务
    pub const SYS_AGENT_SET_QUOTA:     usize = 525; // 设置 Agent 资源配额
    pub const SYS_AGENT_GET_QUOTA:     usize = 526; // 获取 Agent 资源配额
    pub const SYS_AGENT_SNAPSHOT:      usize = 527; // 创建 Agent 快照
    pub const SYS_AGENT_RESTORE:       usize = 528; // 从快照恢复 Agent
}
```

---

## 6. 虚拟化架构概述

### 6.1 虚拟化支持层级

OmniAgent OS 原生支持硬件辅助虚拟化，可作为 Type-1 Hypervisor 运行：

```
┌─────────────────────────────────────────┐
│              管理程序 (Host)              │
│           OmniAgent OS 微内核            │
│  ┌──────────────────────────────────┐   │
│  │        虚拟化管理器 (VMM)         │   │
│  │  - VM 创建/销毁                  │   │
│  │  - vCPU 调度                     │   │
│  │  - EPT/NPT 管理                  │   │
│  │  - 设备模拟 (virtio)             │   │
│  └──────────────────────────────────┘   │
│                                         │
│  ┌───────────┐  ┌───────────┐           │
│  │   VM #1   │  │   VM #2   │  ...      │
│  │ Guest OS  │  │ Guest OS  │           │
│  └───────────┘  └───────────┘           │
└─────────────────────────────────────────┘
```

### 6.2 虚拟化能力

| 特性 | 说明 |
|------|------|
| **Intel VT-x** | 支持 VMX 操作模式切换 |
| **AMD-V** | 支持 SVM 操作模式切换 |
| **EPT/NPT** | 扩展/嵌套页表，加速 Guest 物理地址转换 |
| **virtio** | 半虚拟化设备框架，减少 VM Exit 开销 |
| **vCPU 亲和性** | vCPU 可绑定到物理 CPU 核心 |
| **Agent VM** | Agent 可运行在独立 VM 中实现强隔离 |

### 6.3 虚拟化与 Agent 的结合

Agent 可以选择以三种隔离级别运行：

1. **进程级隔离**: Agent 作为普通用户态进程，通过页表隔离
2. **Enclave 隔离**: Agent 在软件 TEE 中运行，内核辅助加密内存
3. **VM 级隔离**: Agent 运行在独立虚拟机中，通过硬件虚拟化实现最强隔离

---

## 7. 云端/本地 AI 模型双通道设计

### 7.1 设计目标

- **透明切换**: 用户无需修改 Agent 代码即可切换推理后端
- **性能最优**: 优先使用本地模型，复杂任务自动路由到云端
- **隐私保护**: 敏感数据默认使用本地推理
- **离线可用**: 无网络时自动降级到本地模型

### 7.2 推理后端对比

| 特性 | Candle (本地) | ort (本地) | tract (本地) | 云端 API |
|------|:---:|:---:|:---:|:---:|
| 框架兼容 | PyTorch | ONNX | ONNX/TF | 多种 |
| 推理速度 | 快 | 快 | 极快 | 取决于网络 |
| 模型大小 | 中 | 中 | 小 | 无限制 |
| 精度支持 | FP32/FP16/BF16 | FP32/FP16/INT8 | FP32/FP16/INT8 | 多种 |
| 离线可用 | 是 | 是 | 是 | 否 |
| GPU 加速 | 有限 | CUDA/DirectML | WASM SIMD | N/A |
| 适用场景 | 通用推理 | 生产部署 | 边缘设备 | 大规模推理 |

### 7.3 模型路由策略

```rust
/// 推理路由决策
pub struct InferenceRouter {
    /// 用户配置的偏好
    user_preference: InferencePreference,
    /// 当前网络状态
    network_status: NetworkStatus,
    /// 任务复杂度评估器
    complexity_estimator: ComplexityEstimator,
}

pub enum InferencePreference {
    /// 始终使用本地模型
    LocalOnly,
    /// 始终使用云端模型
    CloudOnly,
    /// 自动选择（默认）
    Auto,
    /// 延迟优先（倾向于本地）
    LatencyFirst,
    /// 精度优先（倾向于云端大模型）
    AccuracyFirst,
    /// 隐私优先（敏感数据强制本地）
    PrivacyFirst,
}

pub enum RoutingDecision {
    /// 使用本地 Candle 后端
    LocalCandle { model_id: String },
    /// 使用本地 ort 后端
    LocalOrt { model_id: String },
    /// 使用本地 tract 后端
    LocalTract { model_id: String },
    /// 使用云端 API
    Cloud { provider: CloudProvider, model_id: String },
}
```

### 7.4 数据安全

- 本地推理数据不离开设备
- 云端推理使用 TLS 1.3 加密传输
- 敏感数据标记（通过 Agent Enclave 标签）强制本地推理
- 云端 API 响应缓存策略可配置

---

## 8. 关键设计决策与权衡

### 8.1 决策记录

| 决策 | 选择 | 替代方案 | 理由 |
|------|------|---------|------|
| 内核语言 | Rust (`no_std`) | C/C++, Zig | 内存安全、无数据竞争、丰富的类型系统 |
| 架构类型 | 微内核 | 宏内核、exokernel | 安全性、可靠性、模块化 |
| 引导方式 | `bootloader` crate | GRUB, Limine | 纯 Rust 工具链、无外部依赖 |
| 调度算法 | CFS 变体 | FIFO, RR, BPF 调度 | 公平性与响应性的平衡 |
| IPC 机制 | 零拷贝共享内存 | 消息拷贝, 共享队列 | 低延迟、高吞吐 |
| 页表级别 | 4 级 (PML4) | 5 级 (LA57) | 兼容性、性能平衡 |
| 桌面合成 | Vulkan (ash) | OpenGL, 软件渲染 | 现代化、高性能、跨平台 |
| 文本渲染 | cosmic-text | freetype, pango | Rust 原生、Unicode 完善 |
| AI 推理 | Candle+ort+tract | 单一后端 | 灵活性、多框架兼容 |
| 虚拟化 | 硬件辅助 (VT-x/AMD-V) | 软件模拟 | 性能、兼容性 |

### 8.2 微内核 vs 宏内核的权衡

**选择微内核的理由**:
- 故障隔离：驱动崩溃不影响内核稳定性
- 安全性：最小化 TCB（可信计算基）
- 模块化：服务可独立开发、测试、更新
- Agent 隔离：天然支持 Agent 间的地址空间隔离

**微内核的性能代价**:
- IPC 额外开销（通过零拷贝设计缓解）
- 用户态驱动需要更多上下文切换（通过批量处理缓解）
- 系统调用路径更长（通过快速系统调用优化）

### 8.3 Rust `no_std` 的限制与应对

| 限制 | 影响 | 应对策略 |
|------|------|---------|
| 无 `std` 库 | 无 `String`, `Vec`, `Box` 等 | 使用 `alloc` crate；内核早期使用 `bumpalo` |
| 无浮点异常 | FP 操作需手动处理 | Agent 空间启用 SSE/AVX；内核避免浮点 |
| 无异步运行时 | 无 `tokio`/`async-std` | 自研基于调度器的协程机制 |
| 无堆分配（早期） | 启动阶段无动态内存 | `bumpalo` bump allocator → 后期切换 slab |
| 无 I/O | 无 `println!` 等 | UART/帧缓冲直接操作；VGA 文本模式 |

---

## 9. 技术栈总结

### 9.1 核心 crate 依赖

```toml
[dependencies]
# 内核核心
x86_64 = "0.15"              # x86_64 架构抽象
bootloader = "0.11"           # UEFI/BIOS 引导加载
spin = "0.9"                  # 自旋锁原语
volatile = "0.5"              # Volatile 内存访问
bumpalo = "3.16"              # Bump 分配器（启动阶段）

# ArceOS 组件
arceos-fairsched = "0.1"      # CFS 调度器实现

# IPC 与序列化
bincode = "1.3"               # 二进制序列化
serde = { version = "1.0", features = ["derive"] }

# 桌面与图形
ash = "0.38"                  # Vulkan 绑定
smithay = "0.4"               # Wayland 合成器框架
cosmic-text = "0.12"          # 文本整形与光栅化

# AI 推理
candle-core = "0.7"           # Candle 推理框架
ort = "2.0"                   # ONNX Runtime 绑定
tract = "0.21"                # 轻量推理引擎

# 虚拟化
kvm-bindings = "0.8"          # KVM/ioctls 绑定
vm-memory = "0.15"            # 虚拟机内存管理

# 安全
ring = "0.17"                 # 加密原语
sha2 = "0.10"                 # 哈希算法
```

### 9.2 构建工具链

```toml
[package]
name = "omniagent-kernel"
version = "0.1.0"
edition = "2021"

[profile.dev]
panic = "abort"               # 内核 panic 直接中止

[profile.release]
panic = "abort"
opt-level = 3
lto = true                    # 链接时优化
codegen-units = 1             # 单编译单元，更好的优化
strip = true                  # 去除符号表
```

---

## 10. 性能目标

### 10.1 核心性能指标

| 指标 | 目标值 | 测量方法 |
|------|--------|---------|
| 同核 IPC 延迟 | < 1μs | 时间戳计数器 (TSC) |
| 跨核 IPC 延迟 | < 5μs | TSC + 核间中断 |
| 系统调用延迟 | < 200ns | SYSCALL/SYSRET 路径 |
| Agent 创建时间 | < 100μs | 从 `SYS_AGENT_SPAWN` 到 `RUNNING` |
| 上下文切换时间 | < 500ns | 线程切换 TSC 测量 |
| 页表切换开销 | < 100ns | CR3 切换 TSC 测量 |
| 中断响应延迟 | < 10μs | 从中断触发到处理函数入口 |
| 零拷贝吞吐 | > 10 GB/s | 共享内存带宽测试 |
| AI 本地推理延迟 | < 50ms | 端到端（小模型） |
| AI 云端推理延迟 | < 500ms | 端到端（含网络） |
| Vulkan 合成帧率 | > 60 FPS | 1080p 桌面合成 |
| VM 启动时间 | < 2s | 轻量 Guest (Linux) |

### 10.2 资源预算

| 资源 | 内核预算 | 说明 |
|------|---------|------|
| 内核内存 | < 4 MB | 包含内核代码、数据、页表 |
| Agent 最小内存 | 1 MB | Agent 基本运行时 |
| Agent 默认内存 | 64 MB | 标准工作负载 |
| IPC 消息最大 | 4 KB | 固定大小消息头 + 可变载荷 |
| 共享内存最大 | 1 GB | 单 Agent 配对 |
| 最大并发 Agent | 1024 | 系统级限制 |
| 最大并发 VM | 16 | 硬件资源限制 |

---

## 11. 安全模型概述

### 11.1 纵深防御策略

```
┌─────────────────────────────────────────┐
│          Layer 5: 应用安全               │
│  Agent 代码签名验证、能力声明审计         │
├─────────────────────────────────────────┤
│          Layer 4: 服务安全               │
│  服务间 capability 隔离、最小权限原则     │
├─────────────────────────────────────────┤
│          Layer 3: IPC 安全               │
│  Capability-based 端口访问控制           │
│  消息完整性校验、流量控制                 │
├─────────────────────────────────────────┤
│          Layer 2: 内存安全               │
│  4 级页表硬件隔离、NX 保护               │
│  Agent 地址空间隔离、SMAP/SMEP           │
├─────────────────────────────────────────┤
│          Layer 1: 内核安全               │
│  Rust 内存安全保证、KASLR                │
│  Supervisor Mode Execution Prevention   │
│  Stack Canary、Control Flow Integrity   │
├─────────────────────────────────────────┤
│          Layer 0: 硬件安全               │
│  Secure Boot、TPM 2.0 支持              │
│  Intel SGX / AMD SEV (可选)             │
│  IOMMU (VT-d / AMD-Vi)                  │
└─────────────────────────────────────────┘
```

### 11.2 安全 Enclave (软件 TEE)

OmniAgent OS 实现了软件层面的可信执行环境：

- **安全存储**: 敏感数据加密存储，密钥由 Enclave 管理
- **安全计算**: Agent 可在 Enclave 中执行敏感操作
- **远程证明**: Enclave 可向第三方证明其运行状态
- **密钥管理**: 硬件根密钥 + 软件密钥层次结构

### 11.3 Capability-Based 安全模型

系统采用 Capability-Based 安全模型替代传统的 UID/GID 模型：

```rust
/// 端口访问能力
#[derive(Clone, Copy)]
pub struct Capability {
    /// 能力标识符
    pub id: u64,
    /// 允许的操作位掩码
    pub permissions: PermissionFlags,
    /// 能力持有者
    pub holder: AgentId,
    /// 能力来源（创建者）
    pub issuer: AgentId,
    /// 过期时间
    pub expires_at: Option<u64>,
}

pub struct PermissionFlags: u32 {
    const READ    = 1 << 0;
    const WRITE   = 1 << 1;
    const EXECUTE = 1 << 2;
    const GRANT   = 1 << 3;  // 可转授
    const DELEGATE = 1 << 4; // 可委托
}
```

### 11.4 威胁模型

| 威胁 | 防御措施 |
|------|---------|
| 内核内存破坏 | Rust 所有权系统 + 编译时检查 |
| Agent 间越权访问 | Capability-based 访问控制 |
| IPC 消息篡改 | 消息认证码 (MAC) |
| 侧信道攻击 | 常量时间算法、缓存分区 |
| 驱动故障 | 用户态驱动 + 故障隔离 |
| 物理内存访问 | IOMMU、DMA 保护 |
| 供应链攻击 | Secure Boot、代码签名 |
| 虚拟机逃逸 | EPT 隔离、最小化 hypercall 接口 |

---

## 12. 术语表

| 术语 | 定义 |
|------|------|
| **Agent** | AI 智能体，系统的调度和管理单元 |
| **HAL** | Hardware Abstraction Layer，硬件抽象层 |
| **TEE** | Trusted Execution Environment，可信执行环境 |
| **CFS** | Completely Fair Scheduler，完全公平调度器 |
| **IPC** | Inter-Process Communication，进程间通信 |
| **EPT** | Extended Page Tables，扩展页表 |
| **NPT** | Nested Page Tables，嵌套页表 |
| **VMM** | Virtual Machine Monitor，虚拟机监控器 |
| **TSC** | Time Stamp Counter，时间戳计数器 |
| **KASLR** | Kernel Address Space Layout Randomization |
| **SMAP** | Supervisor Mode Access Prevention |
| **SMEP** | Supervisor Mode Execution Prevention |
| **VT-x** | Intel Virtualization Technology |
| **AMD-V** | AMD Virtualization (SVM) |

---

## 13. 参考文档

| 文档 | 路径 |
|------|------|
| 微内核设计规范 | `docs/architecture/microkernel-design.md` |
| IPC 协议规范 | `docs/architecture/ipc-protocol.md` |
| 内存模型规范 | `docs/architecture/memory-model.md` |
| Agent 运行时设计 | `docs/architecture/agent-runtime.md` (规划中) |
| 安全 Enclave 设计 | `docs/architecture/security-enclave.md` (规划中) |
| 虚拟化设计 | `docs/architecture/virtualization.md` (规划中) |

---

*本文档由 OmniAgent OS 架构团队维护，如有疑问请联系 arch@omniagent.os*
