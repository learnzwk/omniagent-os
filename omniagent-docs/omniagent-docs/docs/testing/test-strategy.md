# OmniAgent OS 测试策略总览

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: 正式发布
> **责任团队**: 质量保障与测试工程组

---

## 1. 测试哲学

### 1.1 核心理念

OmniAgent OS 采用**测试驱动开发（TDD）**作为强制性开发流程。所有功能开发必须遵循经典的 **Red-Green-Refactor** 循环：

1. **Red（红色阶段）**：编写一个失败的测试，定义期望行为
2. **Green（绿色阶段）**：编写最小量的代码使测试通过
3. **Refactor（重构阶段）**：在测试保护下优化代码结构

```rust
// Red: 先写失败的测试
#[test]
fn test_ipc_message_round_trip() {
    let msg = IpcMessage::new("test-service", "ping", b"hello");
    let result = ipc_send_and_wait(msg, Duration::from_secs(1));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().payload(), b"hello");
}

// Green: 编写最小实现使测试通过
// Refactor: 优化 IPC 路径、减少拷贝、添加错误处理
```

### 1.2 设计原则

| 原则 | 描述 |
|------|------|
| **零信任测试** | 不信任任何未经验证的组件，每个模块独立测试 |
| **故障注入优先** | 主动测试异常路径，而非仅验证正常流程 |
| **契约测试** | 服务间通过契约定义接口，测试基于契约而非实现 |
| **确定性测试** | 测试结果必须可重复，避免依赖时序或随机性 |
| **快速反馈** | 单元测试执行时间 < 5 秒，集成测试 < 2 分钟 |

---

## 2. 测试层级架构

OmniAgent OS 采用八层测试金字塔，从底层内核到端到端场景逐层覆盖：

```
                    ┌─────────────┐
                    │   L8 安全    │  ← 模糊测试、渗透测试
                   ┌┴─────────────┴┐
                   │   L7 性能基准   │  ← 回归检测、基准对比
                  ┌┴───────────────┴┐
                  │   L6 端到端测试   │  ← 关键路径 100% 覆盖
                 ┌┴─────────────────┴┐
                 │   L5 UI 渲染测试    │  ← 像素级对比、交互验证
                ┌┴───────────────────┴┐
                │   L4 服务集成测试     │  ← IPC 契约、服务编排
               ┌┴─────────────────────┴┐
               │   L3 服务单元测试       │  ← 业务逻辑、状态管理
              ┌┴───────────────────────┴┐
              │   L2 内核集成测试         │  ← QEMU 启动、串口验证
             ┌┴─────────────────────────┴┐
             │   L1 内核单元测试           │  ← 纯逻辑、数据结构
             └───────────────────────────┘
```

### 2.1 各层详细定义

| 层级 | 名称 | 覆盖率要求 | 执行环境 | 预估用例数 |
|------|------|-----------|---------|-----------|
| L1 | 内核单元测试 | ≥ 90% | 宿主机 (cargo test) | ~2,000 |
| L2 | 内核集成测试 | ≥ 85% | QEMU 模拟器 | ~500 |
| L3 | 服务单元测试 | ≥ 85% | 宿主机 (cargo test) | ~3,000 |
| L4 | 服务集成测试 | ≥ 80% | QEMU + 服务沙箱 | ~800 |
| L5 | UI 渲染测试 | ≥ 70% | 真实 GPU + Vulkan | ~400 |
| L6 | 端到端测试 | 关键路径 100% | QEMU 全系统 | ~200 |
| L7 | 性能基准测试 | 全部基准点 | QEMU / 真实硬件 | ~150 |
| L8 | 安全模糊测试 | 全部攻击面 | QEMU + libFuzzer | ~100 |

### 2.2 层级间依赖关系

```yaml
# 测试执行顺序（CI 流水线）
stages:
  - lint:           # 代码风格检查
      - cargo clippy -- -D warnings
      - cargo fmt --check
  - l1_unit:        # 内核单元测试（最快反馈）
      - cargo test --lib kernel
      - timeout: 60s
  - l3_service_unit: # 服务单元测试
      - cargo test --lib services
      - timeout: 120s
  - l2_kernel_int:  # 内核集成测试
      - cargo test --test kernel-integration
      - timeout: 300s
  - l4_service_int: # 服务集成测试
      - cargo test --test service-integration
      - timeout: 300s
  - l5_ui:          # UI 渲染测试
      - cargo test --test ui-rendering
      - requires: [real-gpu]
      - timeout: 600s
  - l6_e2e:         # 端到端测试
      - cargo test --test e2e
      - timeout: 900s
  - l7_bench:       # 性能基准
      - cargo bench
      - timeout: 1800s
  - l8_fuzz:        # 安全模糊测试
      - cargo fuzz run fuzz-targets
      - timeout: 3600s
```

---

## 3. 测试工具链

### 3.1 核心工具矩阵

| 工具 | 用途 | 适用层级 | 集成方式 |
|------|------|---------|---------|
| `#[test]` + `cargo test` | 单元/集成测试 | L1, L3 | Rust 内置 |
| **QEMU Test Harness** | 内核启动与串口断言 | L2, L4, L6 | 自定义框架 |
| **criterion-rs** | 统计性性能基准 | L7 | Cargo bench |
| **cargo-fuzz** + libFuzzer | 安全模糊测试 | L8 | CI 集成 |
| **Vulkan Validation Layer** | GPU 渲染正确性 | L5 | 运行时层 |
| **tarpaulin / llvm-cov** | 代码覆盖率报告 | 全部 | CI 报告 |
| **proptest** | 属性测试（快速检查） | L1, L3 | 测试依赖 |
| **mockall** | Mock 对象生成 | L3, L4 | 测试依赖 |

### 3.2 QEMU 测试框架

```rust
// tests/qemu_harness.rs
pub struct QemuTestHarness {
    qemu: QemuInstance,
    serial_output: SerialPort,
    timeout: Duration,
}

impl QemuTestHarness {
    /// 启动 QEMU 实例并等待内核引导完成
    pub fn boot(kernel_path: &Path) -> Result<Self> {
        let mut qemu = QemuInstance::new("qemu-system-x86_64")
            .kernel(kernel_path)
            .serial("stdio")
            .nographic()
            .memory("512M")
            .cpu("qemu64")
            .spawn()?;

        let harness = Self {
            qemu,
            serial_output: SerialPort::new()?,
            timeout: Duration::from_secs(30),
        };

        // 等待内核引导完成标志
        harness.wait_for_string("OMNIAGENT_KERNEL_READY")?;
        Ok(harness)
    }

    /// 通过串口发送测试命令并等待结果
    pub fn run_test(&mut self, test_name: &str) -> TestResult {
        self.serial_output.write_line(format!("RUN_TEST:{}", test_name))?;
        self.wait_for_test_result(test_name)
    }

    /// 断言串口输出包含预期字符串
    pub fn assert_output_contains(&mut self, pattern: &str) -> Result<()> {
        let output = self.serial_output.read_until_timeout(self.timeout)?;
        ensure!(output.contains(pattern), "串口输出未包含: {}", pattern);
        Ok(())
    }
}
```

### 3.3 Criterion 基准测试配置

```toml
# Cargo.toml 中 criterion 配置
[benchmarks]
harness = false

[[bench]]
name = "ipc_latency"
harness = false

[[bench]]
name = "scheduler_overhead"
harness = false
```

```rust
// benches/ipc_latency.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_ipc_same_core(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_same_core");
    for msg_size in [64, 256, 1024, 4096] {
        group.bench_with_input(
            BenchmarkId::new("latency", msg_size),
            &msg_size,
            |b, &size| {
                let payload = vec![0u8; size];
                b.iter(|| ipc_send_sync("bench-service", &payload));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_ipc_same_core);
criterion_main!(benches);
```

---

## 4. 测试环境

### 4.1 环境矩阵

| 环境 | 用途 | 硬件要求 | 软件依赖 |
|------|------|---------|---------|
| **CI 标准环境** | L1-L4 自动化测试 | 4 核 CPU, 8GB RAM | QEMU 8.x, Rust nightly |
| **GPU 测试环境** | L5 UI 渲染测试 | NVIDIA/AMD GPU, Vulkan 1.3 | Mesa, Vulkan SDK |
| **性能基准环境** | L7 性能回归 | 物理机, 固定频率 CPU | criterion, perf |
| **模糊测试环境** | L8 安全测试 | 8 核 CPU, 32GB RAM | cargo-fuzz, AFL++ |
| **端到端环境** | L6 全系统测试 | QEMU + 完整镜像 | 全部工具链 |

### 4.2 QEMU 测试环境配置

```yaml
# .github/ci/qemu-config.yml
qemu_config:
  architecture: x86_64
  cpu: "qemu64,+sse4.2"
  memory: "1G"
  kernel: "target/x86_64-omniagent/debug/kernel"
  initrd: "target/x86_64-omniagent/debug/initrd.img"
  serial: "stdio"
  display: "none"
  devices:
    - virtio-net
    - virtio-blk
    - virtio-gpu  # 用于 L5 测试
  test_timeout: 300s
```

### 4.3 环境隔离策略

```
┌─────────────────────────────────────────────┐
│              CI Runner 容器                   │
│  ┌───────────┐  ┌───────────┐  ┌──────────┐ │
│  │ QEMU 实例1 │  │ QEMU 实例2 │  │ QEMU 实例N│ │
│  │ (L2 测试)  │  │ (L4 测试)  │  │ (L6 测试) │ │
│  └───────────┘  └───────────┘  └──────────┘ │
│  ┌─────────────────────────────────────────┐ │
│  │         宿主机 cargo test (L1, L3)       │ │
│  └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
```

---

## 5. CI/CD 流水线

### 5.1 流水线架构

```yaml
# .github/workflows/test.yml
name: OmniAgent OS Test Pipeline

on:
  pull_request:
    branches: [main, develop]
  push:
    branches: [main]

jobs:
  lint:
    name: 代码质量检查
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: clippy, rustfmt
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets -- -D warnings

  kernel-unit:
    name: L1 内核单元测试
    needs: lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo test --lib -p omniagent-kernel --no-fail-fast
      - name: 覆盖率报告
        run: cargo llvm-cov --lib -p omniagent-kernel --lcov > lcov.info
      - uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          name: kernel-unit

  kernel-integration:
    name: L2 内核集成测试
    needs: kernel-unit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: 安装 QEMU
        run: sudo apt-get install -y qemu-system-x86
      - run: cargo test --test kernel-integration --no-fail-fast

  service-unit:
    name: L3 服务单元测试
    needs: lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --workspace --lib --exclude omniagent-kernel

  e2e:
    name: L6 端到端测试
    needs: [kernel-integration, service-unit]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --test e2e -- --nocapture

  benchmark:
    name: L7 性能基准
    needs: e2e
    runs-on: [self-hosted, benchmark]
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - run: cargo bench -- --save-baseline main
      - name: 回归检测
        run: |
          cargo bench -- --baseline main 2>&1 | \
          grep -E "(REGRESSION|improvement)" || true

  fuzz:
    name: L8 安全模糊测试
    needs: e2e
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fuzz run ipc_fuzzer -- -max_total_time=300
      - run: cargo fuzz run syscall_fuzzer -- -max_total_time=300
```

### 5.2 PR 合入门控

| 检查项 | 阻断条件 | 说明 |
|--------|---------|------|
| L1 内核单元测试 | 任一失败 | 核心逻辑不可回归 |
| L2 内核集成测试 | 任一失败 | 启动流程不可回归 |
| L3 服务单元测试 | 任一失败 | 服务逻辑不可回归 |
| L4 服务集成测试 | 任一失败 | 服务间通信不可回归 |
| 代码覆盖率 | 低于层级阈值 | 新代码必须达标 |
| 性能回归 | > 5% 下降 | 需要性能审查 |
| 安全扫描 | 高危漏洞 | 必须修复后合入 |

---

## 6. 覆盖率报告

### 6.1 工具选择

```toml
# Cargo.toml - 覆盖率工具配置
[dev-dependencies]
cargo-llvm-cov = "0.6"    # 推荐：基于 LLVM 的精确覆盖率
# tarpaulin = "0.27"       # 备选：基于 ptrace 的覆盖率
```

### 6.2 覆盖率目标与监控

```yaml
# .github/coverage-config.yml
coverage:
  global_target: 85%

  targets:
    kernel_core:
      path: "kernel/src/core"
      minimum: 92%
    kernel_mm:
      path: "kernel/src/memory"
      minimum: 90%
    kernel_ipc:
      path: "kernel/src/ipc"
      minimum: 90%
    kernel_scheduler:
      path: "kernel/src/scheduler"
      minimum: 88%
    services_agent:
      path: "services/agent/src"
      minimum: 85%
    services_desktop:
      path: "services/desktop/src"
      minimum: 80%

  ignore:
    - "kernel/src/arch/**/boot.s"   # 汇编启动代码
    - "**/test_utils.rs"             # 测试辅助代码
```

### 6.3 覆盖率报告生成

```bash
# 生成 HTML 覆盖率报告
cargo llvm-cov --html --output-dir target/coverage-report

# 生成 LCOV 格式用于 CI 集成
cargo llvm-cov --lcov --output-path lcov.info

# 按模块生成覆盖率摘要
cargo llvm-cov --summary-only
```

---

## 7. 测试数据管理

### 7.1 Fixtures（测试固件）

```rust
// tests/fixtures/mod.rs
pub mod kernel {
    /// 预定义的页表映射配置
    pub fn page_table_config() -> PageTableConfig {
        PageTableConfig {
            virtual_range: 0x0000..0xFFFF_FFFF,
            physical_range: 0x1000_0000..0x2000_0000,
            flags: PageFlags::READ | PageFlags::WRITE,
        }
    }

    /// 预定义的进程描述符
    pub fn process_descriptor() -> ProcessDescriptor {
        ProcessDescriptor {
            pid: ProcessId::new(1),
            name: "test-process",
            priority: Priority::Normal,
            address_space: AddressSpaceId::new(1),
            capabilities: CapSet::all(),
        }
    }
}

pub mod ipc {
    /// 标准测试消息
    pub fn standard_message() -> IpcMessage {
        IpcMessage::new("test-service", "ping", b"hello world")
    }

    /// 大负载测试消息
    pub fn large_message(size: usize) -> IpcMessage {
        IpcMessage::new("test-service", "bulk", &vec![0xAB; size])
    }
}
```

### 7.2 Mocks（模拟对象）

```rust
// 使用 mockall 生成 IPC 服务的 Mock
use mockall::mock;

mock! {
    pub IpcClient {
        pub fn send(&self, service: &str, message: &[u8]) -> Result<IpcReply, IpcError>;
        pub fn subscribe(&self, event: &str) -> Result<SubscriptionId, IpcError>;
    }
}

#[test]
fn test_agent_with_mocked_ipc() {
    let mut mock_ipc = MockIpcClient::new();
    mock_ipc
        .expect_send()
        .withf(|svc, msg| svc == "scheduler" && msg.len() > 0)
        .times(1)
        .returning(|_, _| Ok(IpcReply::ok(b"scheduled")));

    let agent = Agent::new(mock_ipc);
    let result = agent.schedule_task("test-task");
    assert!(result.is_ok());
}
```

### 7.3 Fakes（伪造实现）

```rust
/// 内存分配器的 Fake 实现，使用简单 Vec 模拟
pub struct FakeAllocator {
    memory: Vec<u8>,
    allocations: Vec<(usize, usize)>, // (offset, size)
}

impl FakeAllocator {
    pub fn new(total_size: usize) -> Self {
        Self {
            memory: vec![0u8; total_size],
            allocations: Vec::new(),
        }
    }
}

impl FrameAllocator for FakeAllocator {
    fn allocate_frame(&mut self) -> Option<PhysicalAddress> {
        // 简单首次适应算法
        let offset = self.memory.iter().position(|&b| b == 0)?;
        self.memory[offset] = 1; // 标记为已分配
        Some(PhysicalAddress::new(offset * 4096))
    }

    fn free_frame(&mut self, addr: PhysicalAddress) {
        let offset = addr.as_usize() / 4096;
        self.memory[offset] = 0;
    }
}
```

---

## 8. 内核测试专项

### 8.1 no_std 测试策略

内核运行在 `no_std` 环境下，无法直接使用标准库的测试框架。我们采用以下策略：

```rust
// kernel/src/test_harness.rs
#![cfg(test)]

/// 自定义内核测试运行器
/// 在 QEMU 中通过串口输出测试结果
#[macro_export]
macro_rules! kernel_test {
    ($name:ident, $body:expr) => {
        #[no_mangle]
        pub extern "C" fn $name() -> bool {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                $body;
                true
            }));
            result.unwrap_or(false)
        }
    };
}

/// 测试结果通过串口报告
pub fn report_test_result(name: &str, passed: bool) {
    let status = if passed { "PASS" } else { "FAIL" };
    serial_println!("[TEST] {} - {}", name, status);
}
```

### 8.2 QEMU 串口输出断言

```python
# tests/qemu_serial_assert.py
import subprocess
import re
import sys

def run_qemu_test(kernel_path, timeout=60):
    """启动 QEMU 并从串口收集测试结果"""
    proc = subprocess.Popen(
        ["qemu-system-x86_64",
         "-kernel", kernel_path,
         "-serial", "stdio",
         "-nographic",
         "-m", "512M"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )

    results = {}
    try:
        for line in proc.stdout:
            decoded = line.decode("utf-8", errors="replace")
            match = re.match(r'\[TEST\] (\w+) - (PASS|FAIL)', decoded)
            if match:
                name, status = match.groups()
                results[name] = (status == "PASS")
    except subprocess.TimeoutExpired:
        proc.kill()
        raise RuntimeError(f"QEMU 测试超时 ({timeout}s)")

    return results

def assert_all_passed(results):
    """断言所有测试通过"""
    failed = [name for name, passed in results.items() if not passed]
    if failed:
        print(f"失败的测试: {', '.join(failed)}")
        sys.exit(1)
    print(f"全部 {len(results)} 个测试通过")
```

---

## 9. 服务测试专项

### 9.1 IPC 模拟策略

```rust
// services/test_support/ipc_mock.rs
/// IPC 模拟层：在测试中替换真实 IPC 通道
pub struct IpcMockLayer {
    handlers: HashMap<String, Box<dyn Fn(&[u8]) -> Vec<u8>>>,
    message_log: Vec<IpcMessageRecord>,
}

impl IpcMockLayer {
    pub fn register_handler<F>(&mut self, service: &str, handler: F)
    where
        F: Fn(&[u8]) -> Vec<u8> + 'static,
    {
        self.handlers.insert(service.to_string(), Box::new(handler));
    }

    /// 模拟 IPC 调用
    pub fn call(&mut self, service: &str, payload: &[u8]) -> Result<Vec<u8>, IpcError> {
        self.message_log.push(IpcMessageRecord {
            timestamp: Instant::now(),
            destination: service.to_string(),
            payload_len: payload.len(),
        });

        let handler = self.handlers.get(service)
            .ok_or(IpcError::ServiceNotFound(service.to_string()))?;
        Ok(handler(payload))
    }

    /// 验证 IPC 调用顺序
    pub fn assert_call_order(&self, expected: &[&str]) {
        let actual: Vec<&str> = self.message_log
            .iter()
            .map(|r| r.destination.as_str())
            .collect();
        assert_eq!(actual, expected, "IPC 调用顺序不符合预期");
    }
}
```

### 9.2 服务隔离测试

```rust
#[test]
fn test_service_isolation_ipc_failure() {
    // 模拟 IPC 通道断开
    let isolated_env = ServiceTestEnvironment::new()
        .with_ipc_failure_simulator()
        .build();

    let agent_service = isolated_env.spawn_service("agent-service")?;

    // 断开 IPC 后，服务应优雅降级而非崩溃
    isolated_env.simulate_ipc_failure();
    std::thread::sleep(Duration::from_millis(100));

    // 服务应仍在运行（健康检查通过）
    assert!(isolated_env.is_service_healthy("agent-service"));
    // 服务应报告 IPC 不可用状态
    let status = isolated_env.get_service_status("agent-service")?;
    assert_eq!(status.ipc_state, IpcState::Degraded);
}
```

---

## 10. 性能回归测试

### 10.1 基线管理

```yaml
# benches/baselines/x86_64.yml
baselines:
  ipc_same_core_64b:
    median_us: 2.5
    max_regression: 10%    # 允许最大 10% 回归
    alert_threshold: 20%   # 超过 20% 触发告警

  ipc_cross_core_64b:
    median_us: 8.0
    max_regression: 10%
    alert_threshold: 20%

  context_switch:
    median_us: 1.2
    max_regression: 15%
    alert_threshold: 25%

  page_alloc_4k:
    median_ns: 150
    max_regression: 10%
    alert_threshold: 20%

  agent_spawn:
    median_ms: 5.0
    max_regression: 15%
    alert_threshold: 30%

  boot_time:
    median_ms: 800
    max_regression: 10%
    alert_threshold: 20%
```

### 10.2 回归检测脚本

```rust
// benches/regression_detector.rs
use std::collections::HashMap;

pub struct RegressionDetector {
    baselines: HashMap<String, BenchmarkBaseline>,
}

impl RegressionDetector {
    /// 比较新基准数据与基线
    pub fn check_regression(&self, name: &str, new_value: f64) -> RegressionReport {
        let baseline = self.baselines.get(name)
            .expect(&format!("未找到基线: {}", name));

        let change_percent = (new_value - baseline.median) / baseline.median * 100.0;
        let severity = if change_percent > baseline.alert_threshold {
            Severity::Critical
        } else if change_percent > baseline.max_regression {
            Severity::Warning
        } else {
            Severity::Ok
        };

        RegressionReport {
            benchmark_name: name.to_string(),
            baseline_value: baseline.median,
            new_value,
            change_percent,
            severity,
        }
    }
}
```

### 10.3 告警机制

```yaml
# .github/workflows/benchmark-alert.yml
benchmark_alert:
  on_regression:
    warning:
      - comment_on_pr: true
      - message: "⚠️ 性能回归警告: {benchmark} 下降 {percent}%"
    critical:
      - block_merge: true
      - message: "🚨 性能回归严重: {benchmark} 下降 {percent}%，请审查"
      - assign_reviewer: performance-team
```

---

## 11. 安全测试

### 11.1 模糊测试目标

| 目标 | 模糊器 | 输入格式 | 运行时长 |
|------|--------|---------|---------|
| `ipc_fuzzer` | libFuzzer | 二进制消息流 | 4h/nightly |
| `syscall_fuzzer` | libFuzzer | 系统调用序列 | 4h/nightly |
| `fs_fuzzer` | AFL++ | 文件系统操作 | 8h/weekly |
| `net_fuzzer` | libFuzzer | 网络数据包 | 4h/nightly |
| `config_fuzzer` | libFuzzer | 配置文件内容 | 2h/nightly |

### 11.2 模糊测试实现

```rust
// fuzz/fuzz_targets/ipc_fuzzer.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // 将输入数据解析为 IPC 消息序列
    let messages = parse_ipc_messages(data);
    for msg in messages {
        // 在隔离环境中处理每条消息
        let result = test_ipc_handle_message(&msg);
        // 不应 panic，任何错误都应优雅处理
        match result {
            Ok(_) | Err(IpcError::InvalidMessage) => {},
            Err(e) => panic!("IPC 处理器意外错误: {:?}", e),
        }
    }
});
```

### 11.3 渗透测试计划

| 周期 | 测试范围 | 负责团队 |
|------|---------|---------|
| 每季度 | 内核权限提升、IPC 逃逸 | 安全团队 |
| 每半年 | Agent 沙箱逃逸、资源耗尽 | 安全团队 + Agent 团队 |
| 每年 | 全系统渗透测试 | 外部安全审计公司 |
| 按需 | 新功能安全评审 | 安全团队 |

---

## 12. 测试度量与报告

### 12.1 度量指标

| 指标 | 目标值 | 采集频率 |
|------|--------|---------|
| 代码覆盖率 | ≥ 85% | 每次 CI |
| 测试通过率 | 100% | 每次 CI |
| 测试执行时间 | < 30 分钟 | 每次 CI |
| 平均缺陷发现时间 | < 24 小时 | 持续 |
| 安全漏洞修复时间 | 高危 < 24h, 中危 < 72h | 持续 |
| 性能回归响应时间 | < 48 小时 | 持续 |

### 12.2 测试报告模板

```markdown
## OmniAgent OS 测试报告

**日期**: YYYY-MM-DD
**版本**: vX.Y.Z
**提交**: abc1234

### 摘要
- 总测试用例: X,XXX
- 通过: X,XXX | 失败: X | 跳过: X
- 覆盖率: XX.X%
- 性能回归: X 项
- 安全问题: X 项

### 各层结果
| 层级 | 用例数 | 通过率 | 覆盖率 | 执行时间 |
|------|--------|--------|--------|---------|
| L1   | ...    | ...%   | ...%   | ...s    |
| L2   | ...    | ...%   | ...%   | ...s    |
| ...  | ...    | ...%   | ...%   | ...s    |
```

---

## 附录 A: 测试目录结构

```
omniagent-os/
├── kernel/
│   └── src/
│       ├── memory/
│       │   └── tests/           # L1 内存管理单元测试
│       ├── ipc/
│       │   └── tests/           # L1 IPC 单元测试
│       └── scheduler/
│           └── tests/           # L1 调度器单元测试
├── services/
│   ├── agent/
│   │   └── tests/               # L3 Agent 服务单元测试
│   └── desktop/
│       └── tests/               # L3 桌面服务单元测试
├── tests/
│   ├── kernel_integration/      # L2 内核集成测试
│   ├── service_integration/     # L4 服务集成测试
│   ├── ui_rendering/            # L5 UI 渲染测试
│   └── e2e/                     # L6 端到端测试
├── benches/                     # L7 性能基准
│   ├── baselines/               # 基线数据
│   └── regression_detector.rs   # 回归检测
└── fuzz/                        # L8 模糊测试
    └── fuzz_targets/            # 模糊测试目标
```

## 附录 B: 常用测试命令

```bash
# 运行全部单元测试
cargo test --lib

# 运行内核单元测试
cargo test -p omniagent-kernel

# 运行 QEMU 集成测试
cargo test --test kernel-integration

# 运行性能基准
cargo bench

# 生成覆盖率报告
cargo llvm-cov --html --output-dir target/coverage

# 运行模糊测试
cargo fuzz run ipc_fuzzer

# 运行特定测试
cargo test test_ipc_message_round_trip -- --nocapture
```
