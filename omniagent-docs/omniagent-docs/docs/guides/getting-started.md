# OmniAgent OS 快速入门指南

> 本指南帮助你搭建 OmniAgent OS 开发环境，完成首次构建与运行。

## 前置条件

### 必需工具

| 工具 | 最低版本 | 安装方式 |
|------|---------|---------|
| Rust (nightly) | 最新 nightly | `rustup toolchain install nightly` |
| QEMU | 7.0+ | `apt install qemu-system-x86` |
| NASM | 2.15+ | `apt install nasm` |
| build-essential | - | `apt install build-essential` |

### 可选工具

| 工具 | 用途 | 安装方式 |
|------|------|---------|
| GDB | 内核调试 | `apt install gdb` |
| bootimage | 磁盘镜像创建 | `cargo install bootimage` |

### 系统要求

- **操作系统**: Linux (推荐 Ubuntu 22.04+), macOS 或 WSL2
- **内存**: 至少 8 GB RAM（推荐 16 GB）
- **磁盘空间**: 至少 10 GB
- **CPU**: x86_64 架构，支持硬件虚拟化

### 验证安装

```bash
rustup show && rustc --version --verbose
qemu-system-x86_64 --version
nasm --version
rustup default nightly
```

---

## 克隆与构建

### 克隆仓库

```bash
git clone https://github.com/omniagent-os/omniagent.git
cd omniagent
rustup override set nightly
```

### 项目初始化

```bash
# 安装 Git hooks、验证工具链、安装 cargo 子命令
./scripts/init.sh
```

### 构建内核

```bash
# Debug 构建
cargo build

# Release 构建
cargo build --release

# 仅构建内核
cargo build -p omniagent-kernel

# 构建并生成 bootimage
cargo bootimage
```

构建成功后，镜像位于 `target/x86_64-unknown-none/debug/bootimage-omniagent-kernel.bin`。

---

## 在 QEMU 中运行

### 基本运行

```bash
# 使用 bootimage 直接运行
cargo bootimage --run

# 等效的手动 QEMU 命令
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-omniagent-kernel.bin \
    -m 256M -serial mon:stdio -no-reboot
```

### 常用 QEMU 选项

```bash
# 带调试支持
cargo bootimage --run -- --gdb tcp::1234 --no-reboot

# 串口日志输出到文件
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -serial file:serial.log -m 512M

# 多核运行
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -smp 4 -m 1G -serial mon:stdio

# 启用 KVM 加速
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -enable-kvm -cpu host -m 1G -serial mon:stdio
```

退出 QEMU：按 `Ctrl+A` 然后 `X`，或使用 `--no-reboot` 让内核 panic 后自动退出。

---

## 首次启动体验

启动成功后你会看到：

```
[    0.000] ██████  OmniAgent OS v0.1.0  ██████
[    0.000] Microkernel initialized
[    0.001] CPU: x86_64, Cores: 1, Features: SSE2, NX, APIC
[    0.002] Memory: 256 MB available, 8192 pages free
[    0.003] GDT loaded, IDT initialized (256 entries)
[    0.025] Loading Agent: shell (pid=1, priority=10)
[    0.030] Loading Agent: logd (pid=2, priority=5)
[    0.040] Agent runtime ready
[    0.050] === OmniAgent Shell ===
omniagent> _
```

### 交互命令

```bash
omniagent> help                    # 查看帮助
omniagent> agent list              # 列出运行中的 Agent
omniagent> agent info shell        # 查看 Agent 信息
omniagent> agent send shell "hello"  # 发送消息给 Agent
omniagent> sys info                # 查看系统资源
omniagent> sys mem                 # 查看内存使用
omniagent> shutdown                # 关机
```

---

## 项目结构概览

```
omniagent/
├── Cargo.toml                  # 工作区根配置
├── rust-toolchain.toml         # Rust nightly 版本锁定
├── Makefile                    # 构建快捷命令
├── .cargo/config.toml          # Cargo 编译器配置
├── kernel/
│   └── src/
│       ├── main.rs             # 内核入口点
│       ├── arch/x86_64/        # x86_64 架构代码
│       │   ├── boot.S          # NASM 引导程序
│       │   ├── idt.rs          # 中断描述符表
│       │   ├── gdt.rs          # 全局描述符表
│       │   └── paging.rs       # 分页管理
│       ├── mm/                 # 内存管理
│       ├── ipc/                # IPC 通信
│       ├── process/            # 进程调度
│       ├── driver/             # 驱动管理
│       └── agent/              # Agent 运行时
├── crates/
│   ├── omniagent-syscall/      # 系统调用接口
│   ├── omniagent-ipc/          # IPC 用户态库
│   ├── omniagent-driver/       # 驱动开发框架
│   └── libagent/               # Agent 开发库
├── agents/
│   ├── shell/                  # Shell Agent
│   ├── logd/                   # 日志守护 Agent
│   └── fsd/                    # 文件系统 Agent
├── tools/
│   ├── agent-pack/             # Agent 打包工具
│   └── omniagent-cli/          # 命令行工具
└── docs/
    ├── guides/                 # 开发者指南
    └── architecture/           # 架构文档
```

---

## 核心概念

### 微内核架构

内核仅提供最基础的功能：进程管理、内存管理、IPC 通信、中断路由。所有其他功能运行在用户态 Agent 中。

### Agent

Agent 是独立的用户态进程，具有唯一标识、优先级、能力集和资源配额：

```rust
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub priority: u8,           // 0-255，数值越低优先级越高
    pub capabilities: CapabilitySet,
    pub resources: ResourceQuota,
    pub security_label: SecurityLabel,
}
```

Agent 之间通过 IPC 通信，互不干扰。一个 Agent 的崩溃不会影响其他 Agent 或内核。

### IPC (进程间通信)

IPC 是 Agent 间通信的唯一方式：

```rust
use omniagent_ipc::{Channel, Message, Port};

// 服务端
let port = Port::create("my-service")?;
let (msg, reply) = port.receive()?;
reply.send(&Message::new("response"))?;

// 客户端
let channel = Channel::connect("my-service")?;
channel.send(&Message::new("hello"))?;
```

### 服务层次

```
┌──────────┐    IPC     ┌──────────┐    IPC     ┌──────────┐
│  Shell   │ ────────> │   FSD    │ ────────> │  BlkDrv  │
│  Agent   │ <──────── │  Agent   │ <──────── │  Agent   │
└──────────┘           └──────────┘           └──────────┘
     用户层              服务层               驱动层
```

---

## 常见开发工作流

### 修改内核代码

```bash
vim kernel/src/mm/pmm.rs
cargo check -p omniagent-kernel
cargo test -p omniagent-kernel
cargo bootimage --run
```

### 开发新的 Agent

```bash
cargo new --bin agents/my-agent
# 编辑 Cargo.toml 添加 libagent 依赖
cargo build -p my-agent
./tools/agent-pack/build.sh agents/my-agent
cargo bootimage --run
```

### 使用 Makefile 快捷命令

```bash
make              # cargo build
make run          # 构建并在 QEMU 中运行
make test         # 运行所有测试
make clean        # 清理构建产物
make fmt          # 格式化代码
make clippy       # 运行 lint 检查
make doc          # 生成文档
```

---

## 常见问题排查

### 编译错误

**`linker 'x86_64-elf-gcc' not found`**

```bash
# 使用 LLVM 链接器（推荐），在 .cargo/config.toml 中设置：
# [target.x86_64-unknown-none]
# linker = "rust-lld"
```

**`crate 'omniagent-syscall' not found`**

```bash
cd omniagent && cargo clean && cargo build
```

### QEMU 启动失败

**黑屏无输出**：确保串口重定向正确 `-serial mon:stdio -nographic`

**`cannot set up guest memory`**：减少内存 `-m 128M` 或关闭 KVM

### 运行时崩溃

**内核立即 panic**：

```bash
# 使用 GDB 调试
cargo bootimage --run -- --gdb tcp::1234 -S
gdb -ex "target remote localhost:1234" \
    -ex "break kernel_panic" -ex "continue" \
    target/x86_64-unknown-none/debug/omniagent-kernel
```

**三重故障**：通常由 GDT/IDT 配置错误引起，使用 `-d int` 查看中断信息。

### Rust 工具链问题

```bash
rustup update nightly
cargo install bootimage --force
cargo clean && cargo build
```

---

## 下一步

1. **[构建工具链配置](./build-toolchain.md)** - 构建系统配置与优化
2. **[QEMU 调试指南](./debugging-with-qemu.md)** - 内核调试技巧
3. **[设备驱动开发](./writing-device-drivers.md)** - 编写设备驱动
4. **[Agent 包开发](./creating-agent-packages.md)** - 创建和发布 Agent
5. **[贡献指南](./contributing.md)** - 参与项目贡献

### 社区资源

- **GitHub Issues**: [github.com/omniagent-os/omniagent/issues](https://github.com/omniagent-os/omniagent/issues)
- **API 文档**: 运行 `cargo doc --open`
