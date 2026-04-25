# OmniAgent OS 测试策略总览

> **文档版本**: v1.0.0 | **最后更新**: 2026-04-25 | **责任团队**: 质量保障与测试工程组

---

## 1. 测试哲学

OmniAgent OS 采用**测试驱动开发（TDD）**作为强制性开发流程。所有功能开发必须遵循 **Red-Green-Refactor** 循环：

1. **Red**：编写一个失败的测试，定义期望行为
2. **Green**：编写最小量的代码使测试通过
3. **Refactor**：在测试保护下优化代码结构

```rust
#[test]
fn test_ipc_message_round_trip() {
    let msg = IpcMessage::new("test-service", "ping", b"hello");
    let result = ipc_send_and_wait(msg, Duration::from_secs(1));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().payload(), b"hello");
}
```

### 设计原则

| 原则 | 描述 |
|------|------|
| **零信任测试** | 不信任任何未经验证的组件，每个模块独立测试 |
| **故障注入优先** | 主动测试异常路径，而非仅验证正常流程 |
| **契约测试** | 服务间通过契约定义接口，测试基于契约而非实现 |
| **确定性测试** | 测试结果必须可重复，避免依赖时序或随机性 |
| **快速反馈** | 单元测试 < 5 秒，集成测试 < 2 分钟 |

---

## 2. 测试层级架构

OmniAgent OS 采用八层测试金字塔：

| 层级 | 名称 | 覆盖率要求 | 执行环境 | 预估用例数 |
|------|------|-----------|---------|-----------|
| L1 | 内核单元测试 | >= 90% | 宿主机 (cargo test) | ~2,000 |
| L2 | 内核集成测试 | >= 85% | QEMU 模拟器 | ~500 |
| L3 | 服务单元测试 | >= 85% | 宿主机 (cargo test) | ~3,000 |
| L4 | 服务集成测试 | >= 80% | QEMU + 服务沙箱 | ~800 |
| L5 | UI 渲染测试 | >= 70% | 真实 GPU + Vulkan | ~400 |
| L6 | 端到端测试 | 关键路径 100% | QEMU 全系统 | ~200 |
| L7 | 性能基准测试 | 全部基准点 | QEMU / 真实硬件 | ~150 |
| L8 | 安全模糊测试 | 全部攻击面 | QEMU + libFuzzer | ~100 |

### CI 执行顺序

```yaml
stages:
  - lint:            cargo clippy -- -D warnings && cargo fmt --check
  - l1_unit:         cargo test --lib kernel (timeout: 60s)
  - l3_service_unit: cargo test --lib services (timeout: 120s)
  - l2_kernel_int:   cargo test --test kernel-integration (timeout: 300s)
  - l4_service_int:  cargo test --test service-integration (timeout: 300s)
  - l5_ui:           cargo test --test ui-rendering (requires: real-gpu)
  - l6_e2e:          cargo test --test e2e (timeout: 900s)
  - l7_bench:        cargo bench (timeout: 1800s)
  - l8_fuzz:         cargo fuzz run fuzz-targets (timeout: 3600s)
```

---

## 3. 测试工具链

| 工具 | 用途 | 适用层级 | 集成方式 |
|------|------|---------|---------|
| `#[test]` + `cargo test` | 单元/集成测试 | L1, L3 | Rust 内置 |
| **QEMU Test Harness** | 内核启动与串口断言 | L2, L4, L6 | 自定义框架 |
| **criterion-rs** | 统计性性能基准 | L7 | Cargo bench |
| **cargo-fuzz** + libFuzzer | 安全模糊测试 | L8 | CI 集成 |
| **Vulkan Validation Layer** | GPU 渲染正确性 | L5 | 运行时层 |
| **tarpaulin / llvm-cov** | 代码覆盖率报告 | 全部 | CI 报告 |
| **proptest** | 属性测试 | L1, L3 | 测试依赖 |
| **mockall** | Mock 对象生成 | L3, L4 | 测试依赖 |

### QEMU 测试框架

```rust
pub struct QemuTestHarness {
    qemu: QemuInstance,
    serial_output: SerialPort,
    timeout: Duration,
}

impl QemuTestHarness {
    pub fn boot(kernel_path: &Path) -> Result<Self> {
        let mut qemu = QemuInstance::new("qemu-system-x86_64")
            .kernel(kernel_path).serial("stdio").nographic()
            .memory("512M").cpu("qemu64").spawn()?;
        let harness = Self { qemu, serial_output: SerialPort::new()?,
            timeout: Duration::from_secs(30) };
        harness.wait_for_string("OMNIAGENT_KERNEL_READY")?;
        Ok(harness)
    }

    pub fn run_test(&mut self, test_name: &str) -> TestResult {
        self.serial_output.write_line(format!("RUN_TEST:{}", test_name))?;
        self.wait_for_test_result(test_name)
    }
}
```

---

## 4. 测试环境

| 环境 | 用途 | 硬件要求 | 软件依赖 |
|------|------|---------|---------|
| **CI 标准环境** | L1-L4 自动化测试 | 4 核 CPU, 8GB RAM | QEMU 8.x, Rust nightly |
| **GPU 测试环境** | L5 UI 渲染测试 | NVIDIA/AMD GPU, Vulkan 1.3 | Mesa, Vulkan SDK |
| **性能基准环境** | L7 性能回归 | 物理机, 固定频率 CPU | criterion, perf |
| **模糊测试环境** | L8 安全测试 | 8 核 CPU, 32GB RAM | cargo-fuzz, AFL++ |
| **端到端环境** | L6 全系统测试 | QEMU + 完整镜像 | 全部工具链 |

### QEMU 配置

```yaml
qemu_config:
  architecture: x86_64
  cpu: "qemu64,+sse4.2"
  memory: "1G"
  kernel: "target/x86_64-omniagent/debug/kernel"
  serial: "stdio"
  display: "none"
  devices: [virtio-net, virtio-blk, virtio-gpu]
  test_timeout: 300s
```

---

## 5. CI/CD 流水线

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
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with: { components: clippy, rustfmt }
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets -- -D warnings

  kernel-unit:
    needs: lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --lib -p omniagent-kernel --no-fail-fast
      - run: cargo llvm-cov --lib -p omniagent-kernel --lcov > lcov.info
      - uses: codecov/codecov-action@v4

  kernel-integration:
    needs: kernel-unit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: sudo apt-get install -y qemu-system-x86
      - run: cargo test --test kernel-integration --no-fail-fast

  fuzz:
    needs: [kernel-integration, service-unit]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fuzz run ipc_fuzzer -- -max_total_time=300
      - run: cargo fuzz run syscall_fuzzer -- -max_total_time=300
```

### PR 合入门控

| 检查项 | 阻断条件 | 说明 |
|--------|---------|------|
| L1 内核单元测试 | 任一失败 | 核心逻辑不可回归 |
| L2 内核集成测试 | 任一失败 | 启动流程不可回归 |
| 代码覆盖率 | 低于层级阈值 | 新代码必须达标 |
| 性能回归 | > 5% 下降 | 需要性能审查 |
| 安全扫描 | 高危漏洞 | 必须修复后合入 |

---

## 6. 覆盖率报告

```toml
[dev-dependencies]
cargo-llvm-cov = "0.6"
```

```yaml
coverage:
  global_target: 85%
  targets:
    kernel_core:      { path: "kernel/src/core",      minimum: 92% }
    kernel_mm:        { path: "kernel/src/memory",     minimum: 90% }
    kernel_ipc:       { path: "kernel/src/ipc",        minimum: 90% }
    kernel_scheduler: { path: "kernel/src/scheduler",  minimum: 88% }
    services_agent:   { path: "services/agent/src",    minimum: 85% }
  ignore:
    - "kernel/src/arch/**/boot.s"
    - "**/test_utils.rs"
```

---

## 7. 测试数据管理

### Fixtures（测试固件）

```rust
pub mod kernel {
    pub fn page_table_config() -> PageTableConfig {
        PageTableConfig {
            virtual_range: 0x0000..0xFFFF_FFFF,
            physical_range: 0x1000_0000..0x2000_0000,
            flags: PageFlags::READ | PageFlags::WRITE,
        }
    }
}

pub mod ipc {
    pub fn standard_message() -> IpcMessage {
        IpcMessage::new("test-service", "ping", b"hello world")
    }
}
```

### Mocks（模拟对象）

```rust
use mockall::mock;
mock! {
    pub IpcClient {
        pub fn send(&self, service: &str, message: &[u8]) -> Result<IpcReply, IpcError>;
    }
}

#[test]
fn test_agent_with_mocked_ipc() {
    let mut mock_ipc = MockIpcClient::new();
    mock_ipc.expect_send()
        .withf(|svc, msg| svc == "scheduler" && msg.len() > 0)
        .returning(|_, _| Ok(IpcReply::ok(b"scheduled")));
    let agent = Agent::new(mock_ipc);
    assert!(agent.schedule_task("test-task").is_ok());
}
```

### Fakes（伪造实现）

```rust
pub struct FakeAllocator {
    memory: Vec<u8>,
    allocations: Vec<(usize, usize)>,
}

impl FrameAllocator for FakeAllocator {
    fn allocate_frame(&mut self) -> Option<PhysicalAddress> {
        let offset = self.memory.iter().position(|&b| b == 0)?;
        self.memory[offset] = 1;
        Some(PhysicalAddress::new(offset * 4096))
    }
    fn free_frame(&mut self, addr: PhysicalAddress) {
        self.memory[addr.as_usize() / 4096] = 0;
    }
}
```

---

## 8. 内核测试专项

### no_std 测试策略

```rust
#[macro_export]
macro_rules! kernel_test {
    ($name:ident, $body:expr) => {
        #[no_mangle]
        pub extern "C" fn $name() -> bool {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { $body; true }))
                .unwrap_or(false)
        }
    };
}

pub fn report_test_result(name: &str, passed: bool) {
    let status = if passed { "PASS" } else { "FAIL" };
    serial_println!("[TEST] {} - {}", name, status);
}
```

### QEMU 串口断言

```python
def run_qemu_test(kernel_path, timeout=60):
    proc = subprocess.Popen(
        ["qemu-system-x86_64", "-kernel", kernel_path,
         "-serial", "stdio", "-nographic", "-m", "512M"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    results = {}
    for line in proc.stdout:
        match = re.match(r'\[TEST\] (\w+) - (PASS|FAIL)', line.decode())
        if match:
            name, status = match.groups()
            results[name] = (status == "PASS")
    return results
```

---

## 9. 服务测试专项

```rust
pub struct IpcMockLayer {
    handlers: HashMap<String, Box<dyn Fn(&[u8]) -> Vec<u8>>>,
    message_log: Vec<IpcMessageRecord>,
}

impl IpcMockLayer {
    pub fn call(&mut self, service: &str, payload: &[u8]) -> Result<Vec<u8>, IpcError> {
        self.message_log.push(IpcMessageRecord {
            destination: service.to_string(), payload_len: payload.len() });
        let handler = self.handlers.get(service)
            .ok_or(IpcError::ServiceNotFound(service.to_string()))?;
        Ok(handler(payload))
    }
}

#[test]
fn test_service_isolation_ipc_failure() {
    let env = ServiceTestEnvironment::new().with_ipc_failure_simulator().build();
    env.spawn_service("agent-service")?;
    env.simulate_ipc_failure();
    assert!(env.is_service_healthy("agent-service"));
    assert_eq!(env.get_service_status("agent-service")?.ipc_state, IpcState::Degraded);
}
```

---

## 10. 性能回归测试

```yaml
baselines:
  ipc_same_core_64b:  { median_us: 2.5,  max_regression: 10%, alert_threshold: 20% }
  ipc_cross_core_64b: { median_us: 8.0,  max_regression: 10%, alert_threshold: 20% }
  context_switch:     { median_us: 1.2,  max_regression: 15%, alert_threshold: 25% }
  agent_spawn:        { median_ms: 5.0,  max_regression: 15%, alert_threshold: 30% }
  boot_time:          { median_ms: 800,  max_regression: 10%, alert_threshold: 20% }
```

```rust
pub struct RegressionDetector {
    baselines: HashMap<String, BenchmarkBaseline>,
}

impl RegressionDetector {
    pub fn check_regression(&self, name: &str, new_value: f64) -> RegressionReport {
        let baseline = self.baselines.get(name).unwrap();
        let change = (new_value - baseline.median) / baseline.median * 100.0;
        let severity = if change > baseline.alert_threshold { Severity::Critical }
            else if change > baseline.max_regression { Severity::Warning }
            else { Severity::Ok };
        RegressionReport { benchmark_name: name.into(), baseline_value: baseline.median,
            new_value, change_percent: change, severity }
    }
}
```

---

## 11. 安全测试

### 模糊测试目标

| 目标 | 模糊器 | 输入格式 | 运行时长 |
|------|--------|---------|---------|
| `ipc_fuzzer` | libFuzzer | 二进制消息流 | 4h/nightly |
| `syscall_fuzzer` | libFuzzer | 系统调用序列 | 4h/nightly |
| `fs_fuzzer` | AFL++ | 文件系统操作 | 8h/weekly |
| `net_fuzzer` | libFuzzer | 网络数据包 | 4h/nightly |

```rust
fuzz_target!(|data: &[u8]| {
    let messages = parse_ipc_messages(data);
    for msg in messages {
        match test_ipc_handle_message(&msg) {
            Ok(_) | Err(IpcError::InvalidMessage) => {},
            Err(e) => panic!("IPC 处理器意外错误: {:?}", e),
        }
    }
});
```

### 渗透测试计划

| 周期 | 测试范围 | 负责团队 |
|------|---------|---------|
| 每季度 | 内核权限提升、IPC 逃逸 | 安全团队 |
| 每半年 | Agent 沙箱逃逸、资源耗尽 | 安全团队 + Agent 团队 |
| 每年 | 全系统渗透测试 | 外部安全审计公司 |

---

## 12. 测试度量

| 指标 | 目标值 | 采集频率 |
|------|--------|---------|
| 代码覆盖率 | >= 85% | 每次 CI |
| 测试通过率 | 100% | 每次 CI |
| 测试执行时间 | < 30 分钟 | 每次 CI |
| 安全漏洞修复时间 | 高危 < 24h, 中危 < 72h | 持续 |
| 性能回归响应时间 | < 48 小时 | 持续 |

### 目录结构

```
omniagent-os/
├── kernel/src/{memory,ipc,scheduler}/tests/  # L1
├── services/{agent,desktop}/tests/            # L3
├── tests/{kernel_integration,service_integration,ui_rendering,e2e}/  # L2,L4,L5,L6
├── benches/baselines/                         # L7
└── fuzz/fuzz_targets/                         # L8
```
