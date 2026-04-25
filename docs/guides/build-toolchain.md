# 构建工具链配置

> 本指南详细介绍 OmniAgent OS 的构建系统配置，包括 Rust 工具链、Cargo 工作区、交叉编译、CI 流水线以及性能分析工具。

## Rust Nightly 工具链

### rust-toolchain.toml

项目根目录下的 `rust-toolchain.toml` 锁定工具链版本：

```toml
[toolchain]
channel = "nightly-2024-12-01"
components = ["rust-src", "rustfmt", "clippy", "llvm-tools-preview"]
targets = ["x86_64-unknown-none", "aarch64-unknown-none"]
```

| 字段 | 说明 |
|------|------|
| `channel` | 固定到特定 nightly 版本，避免破坏性更新 |
| `components` | `rust-src` 用于裸机编译，`llvm-tools-preview` 用于二进制分析 |
| `targets` | 裸机目标，不含标准库 |

### 工具链管理

```bash
rustup install nightly-2024-12-01
rustup default nightly-2024-12-01
rustup component add rust-src llvm-tools-preview
```

### Nightly 特性依赖

```rust
#![feature(naked_functions)]      // 裸函数，用于上下文切换
#![feature(asm_const)]            // 内联汇编中的常量
#![feature(abi_x86_interrupt)]    // x86 中断调用约定
#![feature(core_intrinsics)]      // 核心内建函数
#![feature(once_cell)]            // 一次性初始化
```

---

## 目标平台配置

### x86_64-unknown-none

```toml
# .cargo/config.toml
[build]
target = "x86_64-unknown-none"

[target.x86_64-unknown-none]
runner = "bootimage runner"
rustflags = [
    "-C", "link-arg=-nostartfiles",
    "-C", "link-arg=-static",
    "-C", "link-arg=-no-pie",
    "-C", "link-arg=-Tkernel/src/arch/x86_64/linker.ld",
    "-C", "code-model=kernel",
    "-C", "relocation-model=static",
    "-C", "panic=abort",
]
```

| 选项 | 说明 |
|------|------|
| `code-model=kernel` | 内核代码模型，代码段位于低地址 |
| `relocation-model=static` | 静态重定位 |
| `panic=abort` | panic 时直接终止，不展开栈 |

### 链接脚本

```ld
/* kernel/src/arch/x86_64/linker.ld */
ENTRY(_start)
SECTIONS {
    . = 1M;
    .text BLOCK(4K) : ALIGN(4K) {
        *(.multiboot)
        *(.text .text.*)
    }
    .rodata BLOCK(4K) : ALIGN(4K) { *(.rodata .rodata.*) }
    .data BLOCK(4K) : ALIGN(4K)   { *(.data .data.*) }
    .bss BLOCK(4K) : ALIGN(4K)    { *(COMMON) *(.bss .bss.*) }
    /DISCARD/ : { *(.eh_frame) *(.note .note.*) *(.comment) }
    . = ALIGN(16); . += 16K;
    _kernel_stack_top = .;
}
```

---

## Cargo 工作区结构

### 根 Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "kernel",
    "crates/omniagent-syscall",
    "crates/omniagent-ipc",
    "crates/omniagent-driver",
    "crates/libagent",
    "agents/shell", "agents/logd", "agents/netd", "agents/fsd",
    "tools/agent-pack", "tools/omniagent-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
omniagent-syscall = { path = "crates/omniagent-syscall" }
omniagent-ipc = { path = "crates/omniagent-ipc" }
omniagent-driver = { path = "crates/omniagent-driver" }
libagent = { path = "crates/libagent" }
bitflags = "2.5"
spin = "0.9"
volatile = "0.5"
log = "0.4"
serde = { version = "1.0", default-features = false, features = ["derive"] }
```

### 内核 crate

```toml
# kernel/Cargo.toml
[package]
name = "omniagent-kernel"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["staticlib"]

[dependencies]
bitflags.workspace = true
spin.workspace = true
```

---

## 构建 Profile

```toml
[profile.dev]
panic = "abort"
opt-level = 0
debug = true
lto = false
incremental = true

[profile.release]
panic = "abort"
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true

[profile.test]
opt-level = 1
debug = 2
```

---

## 交叉编译 aarch64

```bash
rustup target add aarch64-unknown-none --toolchain nightly-2024-12-01
sudo apt install gcc-aarch64-linux-gnu
```

```toml
# .cargo/config.toml
[target.aarch64-unknown-none]
linker = "aarch64-linux-gnu-gcc"
rustflags = [
    "-C", "link-arg=-Tkernel/src/arch/aarch64/linker.ld",
    "-C", "target-feature=+strict-align",
    "-C", "panic=abort",
]
```

```bash
cargo build --target aarch64-unknown-none --release
qemu-system-aarch64 -machine virt -cpu cortex-a72 -m 512M \
    -nographic -kernel target/aarch64-unknown-none/release/omniagent-kernel
```

条件编译处理不同架构：

```rust
#[cfg(target_arch = "x86_64")]
mod arch {
    pub fn enable_interrupts() {
        unsafe { x86_64::instructions::interrupts::enable(); }
    }
}
#[cfg(target_arch = "aarch64")]
mod arch {
    pub fn enable_interrupts() {
        unsafe { llvm_asm!("msr daifclr, #2" :::: "volatile"); }
    }
}
```

---

## CI 流水线

### GitHub Actions

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
env:
  CARGO_TERM_COLOR: always

jobs:
  build-and-test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-none, aarch64-unknown-none]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: rust-src, rustfmt, clippy, llvm-tools-preview
          targets: ${{ matrix.target }}
      - name: Install dependencies
        run: sudo apt install -y qemu-system-x86 qemu-system-arm nasm gcc-aarch64-linux-gnu
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: ~/.cargo/registry ~/.cargo/git target
          key: ${{ runner.os }}-${{ matrix.target }}-${{ hashFiles('**/Cargo.lock') }}
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Build
        run: cargo build --target ${{ matrix.target }}
      - name: Unit tests
        run: cargo test --lib --all
      - name: QEMU boot test
        if: matrix.target == 'x86_64-unknown-none'
        run: |
          cargo bootimage --build
          timeout 60s qemu-system-x86_64 \
            -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-*.bin \
            -serial file:serial.log -no-reboot -display none || true
          grep -q "All tests passed" serial.log
```

---

## QEMU 集成

### bootimage 配置

```toml
# Cargo.toml
[package.metadata.bootimage]
test-success-exit-code = 33
test-timeout = 300
run-args = ["-serial", "mon:stdio", "-m", "256M"]
```

```bash
cargo bootimage          # 构建镜像
cargo bootimage --run    # 构建并运行
cargo bootimage --test   # 运行 QEMU 测试
```

---

## Makefile 目标

```makefile
.PHONY: build run test clean fmt clippy doc kernel user

build: kernel user
kernel:
	cargo build -p omniagent-kernel
user:
	cargo build --workspace --exclude omniagent-kernel
run:
	cargo bootimage --run
run-debug:
	cargo bootimage --run -- --s -S
test:
	cargo test --workspace --exclude omniagent-kernel
	cargo bootimage --test
clean:
	cargo clean && rm -rf target/bootimage
fmt:
	cargo fmt --all
clippy:
	cargo clippy --all-targets -- -D warnings
doc:
	cargo doc --no-deps --all --open
iso: build
	mkisofs -R -b boot/grub/i386-pc/eltorito.img \
		-no-emul-boot -o target/omniagent.iso target/iso/
```

---

## 依赖管理

```bash
cargo update                    # 更新所有依赖
cargo update -p bitflags        # 仅更新指定依赖
cargo install cargo-audit && cargo audit          # 安全审计
cargo install cargo-deny && cargo deny check licenses  # 许可检查
```

```toml
# deny.toml
[licenses]
unlicensed = "deny"
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC"]
[bans]
multiple-versions = "warn"
wildcards = "deny"
[sources]
unknown-registry = "deny"
```

---

## 性能分析

### cargo-flamegraph

```bash
cargo install cargo-flamegraph
sudo cargo flamegraph --root --bin omniagent-kernel
```

### Criterion 基准测试

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
[[bench]]
name = "ipc_benchmark"
harness = false
```

```rust
// benches/ipc_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_ipc_send(c: &mut Criterion) {
    let channel = omniagent_ipc::Channel::connect("bench").unwrap();
    let msg = omniagent_ipc::Message::new("data");
    c.bench_function("ipc_send", |b| {
        b.iter(|| channel.send(black_box(&msg)).unwrap())
    });
}
criterion_group!(benches, bench_ipc_send);
criterion_main!(benches);
```

```bash
cargo bench --package omniagent-kernel
```

### 二进制大小分析

```bash
cargo install cargo-bloat
cargo bloat --release --crates
cargo bloat --release --functions -n 20
size target/x86_64-unknown-none/release/omniagent-kernel
```
