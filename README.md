# OmniAgent OS

<p align="center">
  <strong>Agent-Native Microkernel Operating System</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Version-v0.2.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/Rust-1.95-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Architecture-x86__64-blue" alt="x86_64">
  <img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-green" alt="License">
  <img src="https://img.shields.io/badge/Tests-1260%20passing-brightgreen" alt="Tests">
  <img src="https://img.shields.io/badge/Crates-18%20(1%20kernel%20%2B%2017%20userspace)-yellow" alt="Crates">
  <img src="https://img.shields.io/badge/Code-47K%20lines-yellow" alt="Code">
</p>

---

OmniAgent OS 是一个以 **Agent 为第一公民** 的微内核操作系统，使用 Rust 编写，运行在 x86_64 架构上。系统将 AI Agent 深度集成到操作系统内核层面，提供系统调用级别的 Agent 管理、多模态交互、分布式协作和自动化工作流能力。

## 核心特性

- **微内核架构** — GDT/IDT/PIC/APIC/定时器/键盘/串口，最小化内核信任域
- **Slab 分配器** — 替换 bump allocator，支持对象级高效内存复用
- **虚拟内存管理** — 4 级页表 (PML4)，支持映射/取消映射/权限控制
- **CFS 调度器** — 完全公平调度器变体，5 级优先级，红黑树就绪队列
- **Agent 原生** — 17 个专用系统调用 (512-528)，Agent 作为内核一等公民管理
- **零拷贝 IPC** — 64 字节 `#[repr(C)]` 消息头，支持同步/异步/广播/零拷贝传输
- **内核文件系统** — VFS 层 + 文件描述符表，支持挂载/目录/文件操作
- **内核网络层** — TCP/UDP Socket 抽象，网络接口管理
- **块设备驱动** — 驱动框架 + RAM Disk 实现
- **安全能力桥接** — CapBitmap 与 Capability 双向映射，细粒度权限控制
- **POSIX 兼容** — 35+ POSIX syscall 实现，标准应用程序兼容
- **多模态 AI** — 本地模型 (Candle/ONNX) + 云端 API (OpenAI/Anthropic) 双通道
- **自动化引擎** — DAG 任务调度、工作流编排、触发器系统
- **量化内存** — Q4/Q8/F16/B1 张量存储，为 AI 推理优化的内存管理
- **高级学习** — 记忆系统 (短期/长期/情景/语义)、知识图谱、多种学习策略
- **分布式服务** — CRDT 状态同步、向量时钟、集群成员管理
- **Aqua Shell 桌面** — 窗口管理器、Dock/MenuBar/AgentBar/Spotlight UI 组件、主题引擎
- **Vulkan 合成器** — GPU 加速渲染框架

## 项目架构

```
omniagent-os/
├── kernel/                          # 微内核 (no_std, x86_64)
│   └── src/
│       ├── main.rs                  # 内核入口 + 5 阶段启动序列
│       ├── lib.rs                   # 内核库根
│       ├── vga.rs                   # VGA 80x25 文本模式
│       ├── arch/x86_64/             # 架构相关
│       │   ├── gdt.rs               # 全局描述符表
│       │   ├── idt.rs               # 中断描述符表
│       │   ├── pic.rs               # 8259A PIC (禁用)
│       │   ├── apic.rs              # 本地 APIC
│       │   └── port_io.rs           # 端口 I/O
│       ├── interrupts/              # CPU 异常处理
│       ├── memory/                  # 物理内存 + Slab 分配器 + 虚拟内存
│       │   ├── slab.rs              # Slab 对象分配器
│       │   └── vm/                  # 虚拟内存管理 (4 级页表)
│       ├── scheduler/               # CFS 调度器 (5 级优先级)
│       ├── fs/                      # 内核 VFS + 文件描述符表
│       ├── net/                     # 内核网络层 (TCP/UDP Socket)
│       ├── security/                # 安全能力桥接 (CapBitmap ↔ Capability)
│       ├── drivers/                 # 串口 + PS/2 键盘 + 块设备
│       │   └── block/               # 块设备驱动框架 + RAM Disk
│       ├── time/                    # PIT 8254 定时器
│       ├── boot/                    # Multiboot2 引导信息
│       ├── agent/                   # Agent 控制块 + 池 + 通信
│       └── syscall/                 # ABI 类型 + 编号 + 分发器 + POSIX
│           └── posix.rs             # 35+ POSIX syscall 实现
│
├── crates/
│   ├── omniagent-syscall/           # 系统调用编号定义
│   ├── omniagent-ipc/               # IPC 消息格式 (64B 头)
│   ├── omniagent-driver/            # 设备驱动 trait
│   ├── libagent/                    # Agent Runtime 用户态 API (x86_64 内联汇编 syscall)
│   ├── omniagent-automation/        # 自动化引擎 (DAG 调度)
│   ├── omniagent-memory/            # 量化内存服务
│   ├── omniagent-multimodal/        # 多模态交互 (AI 接口)
│   ├── omniagent-learning/          # 高级学习系统
│   ├── omniagent-distributed/       # 分布式服务 (CRDT)
│   ├── omniagent-shell/             # Aqua Shell 桌面环境
│   │   ├── dock.rs                  # Dock 栏组件
│   │   ├── menu_bar.rs              # 菜单栏组件
│   │   └── agent_bar.rs             # Agent 状态栏组件
│   ├── omniagent-desktop/           # 桌面集成层 (窗口管理 + Spotlight)
│   ├── omniagent-security/          # 安全子系统
│   ├── omniagent-compositor/        # Vulkan 合成器 + GPU 加速
│   ├── omniagent-inference/         # AI 推理引擎 (本地+云端)
│   ├── omniagent-virt/              # 虚拟化支持 (KVM)
│   ├── omniagent-fs/                # 文件系统 (VFS + AgentFS)
│   └── omniagent-net/               # 网络栈 (TCP/UDP/DNS)
│
├── docs/                            # 技术文档 (43 份)
│   ├── architecture/                # 系统架构设计
│   ├── modules/                     # 模块规范
│   ├── api/                         # API 文档
│   ├── guides/                      # 开发指南
│   ├── testing/                     # 测试策略
│   └── security/                    # 安全设计
│
├── Makefile                         # 构建/测试/运行
├── Cargo.toml                       # Workspace 配置
└── .github/workflows/ci.yml         # CI/CD
```

## 快速开始

### 前置要求

- Rust 1.75+ (stable)
- QEMU (x86_64)
- NASM (汇编器)
- `bootimage` crate: `cargo install bootimage`

### 构建

```bash
# 克隆仓库
git clone https://github.com/learnzwk/omniagent-os.git
cd omniagent-os

# 构建全部
make build

# 运行内核 (QEMU)
make run

# 调试模式
make run-debug
```

### 测试

```bash
# 运行全部测试 (1260 个)
make test

# 仅用户态 crate 测试 (847 个)
cargo test --workspace --exclude omniagent-kernel

# 仅内核测试 (413 个)
cargo test --target x86_64-unknown-linux-gnu -p omniagent-kernel -- --test-threads=1
```

### 代码质量

```bash
make fmt          # 格式化
make clippy       # Lint 检查
make doc          # 生成文档
```

## Agent 系统调用

OmniAgent OS 为 Agent 提供了 17 个专用系统调用 (编号 512-528)：

| 编号 | 名称 | 功能 |
|------|------|------|
| 512 | `agent_spawn` | 创建 Agent |
| 513 | `agent_kill` | 终止 Agent |
| 514 | `agent_query` | 查询 Agent 状态 |
| 515 | `agent_msg` | Agent 间消息 |
| 516 | `agent_register` | 注册能力 |
| 517 | `agent_subscribe` | 订阅事件 |
| 518 | `agent_migrate` | 迁移 Agent |
| 519 | `agent_memory_share` | 共享内存 |
| 520-521 | `agent_cap_grant/revoke` | 能力授予/撤销 |
| 522-524 | `agent_bind_port/export/import` | 端口/服务管理 |
| 525-526 | `agent_set/get_quota` | 资源配额 |
| 527-528 | `agent_snapshot/restore` | 快照/恢复 |

## 统计数据

| 指标 | 数值 |
|------|------|
| 版本 | v0.2.0 |
| Rust 源文件 | 122 |
| 代码行数 | 46,854 |
| 测试函数 | 1,260 (847 用户态 + 413 内核) |
| Crate 数量 | 18 (1 内核 + 17 用户态) |
| 技术文档 | 43 |
| Agent Syscall | 17 |
| POSIX Syscall | 35+ |
| 支持模态 | 7 (文本/图像/音频/视频/代码/结构化/二进制) |

## 开发路线

- [x] **Phase 1** — 微内核核心 (GDT/IDT/PIC/APIC/定时器/键盘/串口)
- [x] **Phase 2** — Agent 系统调用 ABI + 分发器 + 管理子系统
- [x] **Phase 3** — Agent Runtime 用户态库
- [x] **Phase 4A** — 自动化引擎 (DAG 调度 + 工作流)
- [x] **Phase 4B** — 量化内存服务
- [x] **Phase 5** — 多模态交互 (云 + 本地 AI)
- [x] **Phase 6** — 高级学习 (记忆 + 知识图谱)
- [x] **Phase 7A** — 分布式服务 (CRDT)
- [x] **Phase 7B** — Aqua Shell 桌面框架
- [x] **Phase 8** — 安全模块
- [x] **Phase 9** — Vulkan 合成器 + GPU 加速框架
- [x] **Phase 10** — AI 推理引擎 (本地 Candle/ONNX + 云端 OpenAI/Anthropic)
- [x] **Phase 11** — 虚拟化支持 (KVM/Virtio)
- [x] **Phase 12** — 文件系统 (VFS/AgentFS) + 网络栈 (TCP/UDP/DNS)
- [x] **P0 内核核心** — Slab 分配器 + 4 级页表虚拟内存 + CFS 调度器 + 5 阶段启动
- [x] **P1 系统服务** — 内核 VFS + 内核网络层 + libagent syscall 封装 + 安全能力桥接 + 块设备驱动
- [x] **P2 桌面与 POSIX** — omniagent-desktop 集成层 + Shell UI 组件 + 35+ POSIX syscall

## 技术栈

| 层次 | 技术 |
|------|------|
| 内核 | Rust `no_std`, x86_64, Multiboot2, bootloader |
| 内存 | Slab 分配器, 4 级页表, 物理帧分配器 |
| 调度 | CFS 变体 (5 级优先级, 红黑树) |
| 文件系统 | 内核 VFS, 文件描述符表, AgentFS |
| 网络 | 内核 TCP/UDP Socket, 网络接口管理 |
| 驱动 | 块设备框架, RAM Disk, 串口, PS/2 键盘 |
| 安全 | CapBitmap, Capability, 策略引擎, SHA-256 |
| POSIX | 35+ syscall (read/write/open/close/fork/exec/...) |
| AI 本地 | Candle, ONNX Runtime, tract |
| AI 云端 | OpenAI API, Anthropic API |
| 桌面 | Vulkan (ash), Smithay, cosmic-text, Dock/MenuBar/AgentBar/Spotlight |
| 分布式 | CRDT, 向量时钟 |

## 许可证

本项目采用双重许可证：

- [MIT License](https://opensource.org/licenses/MIT)
- [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)

---

<p align="center">
  <sub>Built with Rust — Agent-Native by Design</sub>
</p>
