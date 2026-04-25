# OmniAgent OS 性能基准测试规范

> **文档版本**: v1.0.0 | **最后更新**: 2026-04-25 | **责任团队**: 性能工程与内核优化组

---

## 1. 基准测试框架

### 1.1 框架选型

OmniAgent OS 采用双层基准测试框架：

| 层级 | 框架 | 适用场景 | 特性 |
|------|------|---------|------|
| **用户态** | criterion-rs | 服务、Agent、桌面 | 统计分析、回归检测、HTML 报告 |
| **内核态** | 自定义框架 | IPC、调度器、内存管理 | TSC 计时、QEMU 串口输出 |

### 1.2 Criterion 配置

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "ipc_benchmark"
harness = false
```

```toml
# criterion.toml
[output]
confidence_level = 0.95
warm_up_time = 5.0
measurement_time = 30.0
sample_size = 100
threshold = 0.05  # 5% 回归阈值
```

### 1.3 内核基准测试框架

```rust
// kernel/src/benchmark.rs
pub struct KernelBenchmark {
    name: &'static str,
    iterations: u64,
    warmup_iterations: u64,
}

impl KernelBenchmark {
    pub fn new(name: &'static str) -> Self {
        Self { name, iterations: 10000, warmup_iterations: 1000 }
    }

    pub fn run<F: FnMut()>(&self, mut f: F) -> BenchmarkResult {
        for _ in 0..self.warmup_iterations { f(); }
        let flags = cpu::disable_interrupts();
        let start = read_tsc();
        for _ in 0..self.iterations { f(); }
        let end = read_tsc();
        cpu::restore_interrupts(flags);
        let avg_ns = tsc_to_nanos((end - start) / self.iterations);
        serial_println!("[BENCH] {} | avg: {}ns", self.name, avg_ns);
        BenchmarkResult { name: self.name, avg_ns, iterations: self.iterations }
    }
}
```

---

## 2. IPC 基准测试

### 2.1 同核/跨核延迟

```rust
fn bench_ipc_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_latency");
    for size in [0, 64, 256, 1024, 4096] {
        let payload = vec![0xABu8; size];
        group.bench_with_input(BenchmarkId::new("same_core", size), &payload, |b, p| {
            b.iter(|| ipc::send_sync("bench-service", "echo", p).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("cross_core", size), &payload, |b, p| {
            let svc = ipc::get_service_on_core("bench-service", 1);
            b.iter(|| ipc::send_sync_to(svc, "echo", p).unwrap());
        });
    }
    group.finish();
}
```

### 2.2 IPC 基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 同核零拷贝延迟 (0B) | < 500 ns | P99 |
| 同核延迟 (4KB) | < 5 us | P99 |
| 跨核延迟 (64B) | < 3 us | P99 |
| 跨核延迟 (4KB) | < 10 us | P99 |
| 同核吞吐 | > 1M msg/s | 持续流式 |
| 跨核吞吐 | > 500K msg/s | 持续流式 |

---

## 3. 调度器基准测试

```rust
fn bench_scheduler(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler");
    group.bench_function("context_switch_yield", |b| {
        let (t1, t2) = create_yield_pair();
        b.iter(|| { scheduler::yield_to(t2); scheduler::yield_to(t1); });
    });
    group.bench_function("context_switch_blocking", |b| {
        let (tx, rx) = create_channel_pair();
        b.iter(|| { tx.send(1); rx.recv(); });
    });
    group.bench_function("high_priority_preempt", |b| {
        b.iter(|| { let h = spawn_high_priority(|| {}); low_priority_work(); h.join(); });
    });
    group.finish();
}
```

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 上下文切换 (yield) | < 500 ns | P99, 同核 |
| 上下文切换 (阻塞) | < 2 us | P99 |
| 调度延迟 (唤醒→运行) | < 5 us | P99 |
| 高优先级抢占延迟 | < 10 us | P99 |

---

## 4. 内存管理基准测试

```rust
fn bench_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");
    group.bench_function("frame_alloc_free", |b| {
        let mut alloc = get_frame_allocator();
        b.iter(|| { if let Some(f) = alloc.allocate() { alloc.free(f); } });
    });
    group.bench_function("slab_alloc_64b", |b| {
        let mut slab = SlabAllocator::<[u8; 64]>::new(1024);
        b.iter(|| { if let Some(o) = slab.allocate() { slab.free(o); } });
    });
    group.bench_function("cow_page_fault", |b| {
        let page = allocate_shared_page();
        b.iter(|| write_to_shared_page(page));
    });
    group.finish();
}
```

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 帧分配+释放周期 | < 200 ns | 单次操作 |
| Slab 分配 (64B) | < 50 ns | 单次操作 |
| COW 页错误 | < 5 us | 含 TLB 刷新 |
| 按需分页 | < 10 us | 含磁盘读取 |

---

## 5. Agent 基准测试

```rust
fn bench_agent(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent");
    group.bench_function("spawn_minimal", |b| {
        b.iter(|| agent::spawn(AgentConfig { name: "bench", ..Default::default() }));
    });
    group.bench_function("message_256b", |b| {
        let a = spawn_test_agent();
        let msg = AgentMessage::text(&"A".repeat(256));
        b.iter(|| a.send(&msg));
    });
    group.bench_function("pool_dispatch_8", |b| {
        let pool = AgentPool::new(8, AgentConfig::default());
        let task = TaskDefinition::simple("pool-task");
        b.iter(|| pool.dispatch(&task));
    });
    group.finish();
}
```

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 最小 Agent 生成 | < 5 ms | 无上下文加载 |
| 带上下文 Agent 生成 | < 50 ms | 16MB 上下文 |
| Agent 消息传递 (256B) | < 100 us | 同核 |
| Agent 池调度 (8 Agent) | < 1 ms | 单任务分发 |

---

## 6. 自动化与多模态基准测试

### 6.1 自动化基准

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 简单任务分解 | < 50 ms | 本地推理 |
| 复杂任务分解 | < 500 ms | 本地推理 |
| 线性链执行 (10 步) | < 100 ms | 纯调度开销 |
| 并行扇出 (8 路) | < 50 ms | 纯调度开销 |
| DAG 工作流 (20 节点) | < 200 ms | 含依赖解析 |

### 6.2 多模态基准

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| ASR 首个 token | < 200 ms | 10s 音频, 小模型 |
| ASR 完整识别 | < 2 s | 10s 音频, 小模型 |
| TTS 首个音频块 | < 150 ms | 100 字符, 小模型 |
| TTS 完整合成 | < 1 s | 1000 字符, 小模型 |
| 图像生成 (512x512) | < 5 s | 中等模型 |
| 文本嵌入 | < 10 ms | 512 tokens, 小模型 |

### 6.3 学习系统基准

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 简单知识查询 | < 10 ms | 10K 节点图谱 |
| 复杂知识查询 | < 100 ms | 10K 节点图谱 |
| 模式提取 (1K 日志) | < 500 ms | 单次批处理 |
| 批量反馈 (100 条) | < 100 ms | 含模型更新 |

---

## 7. 桌面与系统基准测试

### 7.1 桌面基准

```rust
fn bench_desktop(c: &mut Criterion) {
    let mut group = c.benchmark_group("desktop");
    group.bench_function("composite_10_windows", |b| {
        let comp = Compositor::new(WindowManager::new());
        let wins = create_test_windows(10);
        b.iter(|| comp.compose_frame(&wins));
    });
    group.bench_function("window_open", |b| {
        let wm = WindowManager::new();
        b.iter(|| wm.create_window(WindowSpec::default()));
    });
    group.bench_function("spotlight_query", |b| {
        let sp = Spotlight::with_index("bench_index_10k");
        b.iter(|| sp.search("配置文件"));
    });
    group.finish();
}
```

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 合成器 FPS (10 窗口) | >= 60 FPS | 1080p, Vulkan |
| 合成器 FPS (50 窗口) | >= 30 FPS | 1080p, Vulkan |
| 窗口打开时间 | < 50 ms | 含动画 |
| Spotlight 查询 | < 100 ms | 10K 文件索引 |

### 7.2 系统基准

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 冷启动时间 | < 3 s | QEMU, 512MB RAM |
| 热启动时间 | < 1 s | QEMU, 512MB RAM |
| 内核内存占用 | < 8 MB | 空闲状态 |
| 空闲系统内存 | < 64 MB | 含桌面管理器 |
| 关机时间 | < 500 ms | 优雅关闭 |

---

## 8. 回归检测

### 8.1 基线管理

```yaml
# benches/baselines/main.yml
version: "1.0"
environment: "x86_64-qemu-4core-8gb"
baselines:
  ipc:
    same_core_0b: { median_ns: 450, deviation: 0.05 }
    cross_core_64b: { median_ns: 2500, deviation: 0.08 }
  scheduler:
    context_switch_yield: { median_ns: 380, deviation: 0.10 }
  memory:
    frame_alloc_cycle: { median_ns: 150, deviation: 0.10 }
  agent:
    spawn_minimal: { median_ms: 3.5, deviation: 0.15 }
  system:
    cold_boot: { median_ms: 2500, deviation: 0.10 }
```

### 8.2 回归检测算法

```rust
pub struct RegressionChecker {
    baselines: BaselineStore,
    regression_threshold: f64,  // 10%
    critical_threshold: f64,   // 25%
}

impl RegressionChecker {
    pub fn check(&self, name: &str, new_data: &[f64]) -> RegressionResult {
        let baseline = self.baselines.get(name).expect("未找到基线");
        let is_significant = self.mann_whitney_test(&baseline.samples, new_data, 0.95);
        if !is_significant { return RegressionResult::NoChange; }
        let old = median(&baseline.samples);
        let new = median(new_data);
        let change = (new - old) / old * 100.0;
        match () {
            _ if change > self.critical_threshold =>
                RegressionResult::CriticalRegression { name: name.into(), change },
            _ if change > self.regression_threshold =>
                RegressionResult::Regression { name: name.into(), change },
            _ if change < -self.regression_threshold =>
                RegressionResult::Improvement { name: name.into(), change },
            _ => RegressionResult::NoChange,
        }
    }
}
```

---

## 9. 执行环境与可重复性

### 9.1 环境标准化

```yaml
environment:
  cpu: { model: "Intel Xeon E5-2686 v4", cores: 4, governor: "performance" }
  memory: { size: "8GB", type: "DDR4" }
  storage: { type: "NVMe SSD" }
  qemu: { version: "8.2", options: "-cpu qemu64 -smp 4 -m 4096M" }
  gpu: { model: "NVIDIA Tesla T4", driver: "Vulkan 1.3" }
```

### 9.2 可重复性保证

| 措施 | 描述 |
|------|------|
| 固定 CPU 频率 | 禁用频率调节，锁定最大频率 |
| 隔离 CPU 核心 | 使用 `taskset` 或 `cgroups` 隔离测试核心 |
| 多次采样 | 每个基准至少运行 100 次迭代 |
| 统计显著性 | 使用 Mann-Whitney U 检验确保结果可靠 |
| 环境快照 | 记录完整的硬件和软件配置 |

### 9.3 执行脚本

```bash
#!/bin/bash
set -euo pipefail
echo "=== OmniAgent OS 性能基准测试 ==="
sudo cpupower frequency-set -g performance > /dev/null 2>&1 || true
cargo bench -- --save-baseline "$(date +%Y%m%d)" 2>&1 | tee bench_results.txt
cargo test --test kernel-benchmarks -- --nocapture 2>&1 | tee kernel_bench.txt
cargo run --package bench-analyzer -- \
    --baseline benches/baselines/main.yml \
    --results bench_results.txt --report benches/regression_report.md
```

---

## 附录: 基准测试目录结构

```
benches/
├── baselines/main.yml          # main 分支基线
├── environment.yml             # 环境配置
├── run_all_benchmarks.sh       # 执行脚本
├── ipc_benchmark.rs            # IPC 基准
├── scheduler_benchmark.rs      # 调度器基准
├── memory_benchmark.rs         # 内存基准
├── agent_benchmark.rs          # Agent 基准
├── automation_benchmark.rs     # 自动化基准
├── multimodal_benchmark.rs     # 多模态基准
├── learning_benchmark.rs       # 学习系统基准
├── desktop_benchmark.rs        # 桌面基准
└── system_benchmark.rs         # 系统级基准
```
