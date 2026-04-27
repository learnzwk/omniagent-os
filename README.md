# OmniAgent OS

<p align="center">
  <strong>Agent-Native Microkernel Operating System</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.95-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Architecture-x86__64-blue" alt="x86_64">
  <img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-green" alt="License">
  <img src="https://img.shields.io/badge/Tests-815%20passing-brightgreen" alt="Tests">
  <img src="https://img.shields.io/badge/Code-35K%20lines-yellow" alt="Code">
</p>

---

OmniAgent OS 是一个以 **Agent 为第一公民** 的微内核操作系统，使用 Rust 编写，运行在 x86_64 架构上。系统将 AI Agent 深度集成到操作系统内核层面，提供系统调用级别的 Agent 管理、多模态交互、分布式协作和自动化工作流能力。

## 核心特性

- **微内核架构** — GDT/IDT/PIC/APIC/定时器/键盘/串口，最小化内核信任域
- **Agent 原生** — 17 个专用系统调用 (512-528)，Agent 作为内核一等公民管理
- **零拷贝 IPC** — 64 字节 `#[repr(C)]` 消息头，支持同步/异步/广播/零拷贝传输
- **多模态 AI** — 本地模型 (Candle/ONNX) + 云端 API (OpenAI/Anthropic) 双通道
- **自动化引擎** — DAG 任务调度、工作流编排、触发器系统
- **量化内存** — Q4/Q8/F16/B1 张量存储，为 AI 推理优化的内存管理
- **高级学习** — 记忆系统 (短期/长期/情景/语义)、知识图谱、多种学习策略
- **分布式服务** — CRDT 状态同步、向量时钟、集群成员管理
- **Aqua Shell 桌面** — 窗口管理器、UI 组件系统、主题引擎 (Vulkan compositor 规划中)
- **安全子系统** — 能力系统、策略驱动访问控制、SHA-256、安全审计日志

## 项目架构

```
omniagent-os/
├── kernel/                          # 微内核 (no_std, x86_64)
│   └── src/
│       ├── main.rs                  # 内核入口 + 启动序列
│       ├── lib.rs                   # 内核库根
│       ├── vga.rs                   # VGA 80x25 文本模式
│       ├── arch/x86_64/             # 架构相关
│       │   ├── gdt.rs               # 全局描述符表
│       │   ├── idt.rs               # 中断描述符表
│       │   ├── pic.rs               # 8259A PIC (禁用)
│       │   ├── apic.rs              # 本地 APIC
│       │   └── port_io.rs           # 端口 I/O
│       ├── interrupts/              # CPU 异常处理
│       ├── memory/                  # 物理内存 + 帧分配器 + 堆
│       ├── drivers/                 # 串口 + PS/2 键盘
│       ├── time/                    # PIT 8254 定时器
│       ├── boot/                    # Multiboot2 引导信息
│       ├── agent/                   # Agent 控制块 + 池 + 通信
│       └── syscall/                 # ABI 类型 + 编号 + 分发器
│
├── crates/
│   ├── omniagent-syscall/           # 系统调用编号定义
│   ├── omniagent-ipc/               # IPC 消息格式 (64B 头)
│   ├── omniagent-driver/            # 设备驱动 trait
│   ├── libagent/                    # Agent Runtime 用户态 API
│   ├── omniagent-automation/        # 自动化引擎 (DAG 调度)
│   ├── omniagent-memory/            # 量化内存服务
│   ├── omniagent-multimodal/        # 多模态交互 (AI 接口)
│   ├── omniagent-learning/          # 高级学习系统
│   ├── omniagent-distributed/       # 分布式服务 (CRDT)
│   ├── omniagent-shell/             # Aqua Shell 桌面环境
│   ├── omniagent-security/          # 安全子系统
│   ├── omniagent-compositor/        # Vulkan 合成器 + GPU 加速
│   ├── omniagent-inference/         # AI 推理引擎 (本地+云端)
│   ├── omniagent-virt/              # 虚拟化支持 (KVM)
│   ├── omniagent-fs/                # 文件系统 (VFS + AgentFS)
│   └── omniagent-net/               # 网络栈 (TCP/UDP/DNS)
│
├── docs/                            # 技术文档 (39 份)
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
# 运行全部测试 (620 个)
make test

# 仅用户态 crate 测试
cargo test --workspace --exclude omniagent-kernel

# 仅内核测试
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
| Rust 源文件 | 91 |
| 代码行数 | 35,013 |
| 测试函数 | 815 |
| Crate 数量 | 16 (1 内核 + 15 用户态) |
| 技术文档 | 39 |
| Agent Syscall | 17 |
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

## 技术栈

| 层次 | 技术 |
|------|------|
| 内核 | Rust `no_std`, x86_64, Multiboot2, bootloader |
| 调度 | CFS (规划 ArceOS 集成) |
| AI 本地 | Candle, ONNX Runtime, tract |
| AI 云端 | OpenAI API, Anthropic API |
| 桌面 | Vulkan (ash), Smithay, cosmic-text |
| 分布式 | CRDT, 向量时钟 |
| 安全 | 能力系统, SHA-256, 策略引擎 |

## 许可证

本项目采用双重许可证：

- [MIT License](https://opensource.org/licenses/MIT)
- [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)

---

<p align="center">
  <sub>Built with Rust — Agent-Native by Design</sub>
</p>
