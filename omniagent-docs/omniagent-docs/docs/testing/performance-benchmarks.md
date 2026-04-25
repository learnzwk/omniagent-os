# OmniAgent OS 性能基准测试规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: 正式发布
> **责任团队**: 性能工程与内核优化组

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

[[bench]]
name = "agent_benchmark"
harness = false

[[bench]]
name = "desktop_benchmark"
harness = false
```

```toml
# criterion.toml - 全局配置
[plot]
plotting_backend = "plotters"

[output]
confidence_level = 0.95
warm_up_time = 5.0
measurement_time = 30.0
sample_size = 100
threshold = 0.05  # 5% 回归阈值
noise_threshold = 0.01
```

### 1.3 内核基准测试框架

```rust
// kernel/src/benchmark.rs
/// 内核基准测试运行器
pub struct KernelBenchmark {
    name: &'static str,
    iterations: u64,
    warmup_iterations: u64,
}

impl KernelBenchmark {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            iterations: 10000,
            warmup_iterations: 1000,
        }
    }

    /// 运行基准测试并输出结果
    pub fn run<F>(&self, mut f: F) -> BenchmarkResult
    where
        F: FnMut(),
    {
        // 预热阶段
        for _ in 0..self.warmup_iterations {
            f();
        }

        // 禁用中断以确保精确计时
        let flags = cpu::disable_interrupts();

        let start = read_tsc();
        for _ in 0..self.iterations {
            f();
        }
        let end = read_tsc();

        cpu::restore_interrupts(flags);

        let total_cycles = end - start;
        let avg_cycles = total_cycles / self.iterations;
        let avg_ns = tsc_to_nanos(avg_cycles);

        let result = BenchmarkResult {
            name: self.name,
            iterations: self.iterations,
            total_cycles,
            avg_cycles,
            avg_ns,
        };

        result.report();
        result
    }
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub name: &'static str,
    pub iterations: u64,
    pub total_cycles: u64,
    pub avg_cycles: u64,
    pub avg_ns: u64,
}

impl BenchmarkResult {
    pub fn report(&self) {
        serial_println!(
            "[BENCH] {} | 迭代: {} | 平均: {}ns ({} cycles)",
            self.name, self.iterations, self.avg_ns, self.avg_cycles
        );
    }
}
```

---

## 2. IPC 基准测试

### 2.1 同核 IPC 延迟

```rust
// benches/ipc_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_ipc_same_core_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_same_core_latency");

    for size in [0, 64, 256, 1024, 4096] {
        let payload = vec![0xABu8; size];
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &payload,
            |b, payload| {
                b.iter(|| {
                    ipc::send_sync("bench-service", "echo", payload)
                        .expect("IPC 失败")
                });
            },
        );
    }
    group.finish();
}

fn bench_ipc_same_core_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_same_core_throughput");
    group.throughput(criterion::Throughput::Elements(1000));
    group.bench_function("1k_messages", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                ipc::send_sync("bench-service", "ping", &[])
                    .expect("IPC 失败");
            }
        });
    });
    group.finish();
}
```

### 2.2 跨核 IPC 延迟

```rust
fn bench_ipc_cross_core_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_cross_core_latency");

    for size in [64, 256, 1024, 4096] {
        let payload = vec![0xABu8; size];
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &payload,
            |b, payload| {
                // 确保服务运行在另一个核心
                let service = ipc::get_service_on_core("bench-service", 1);
                b.iter(|| {
                    ipc::send_sync_to(service, "echo", payload)
                        .expect("IPC 失败")
                });
            },
        );
    }
    group.finish();
}
```

### 2.3 IPC 基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 同核零拷贝延迟 (0B) | < 500 ns | P99, 单次调用 |
| 同核延迟 (64B) | < 1 us | P99, 单次调用 |
| 同核延迟 (4KB) | < 5 us | P99, 单次调用 |
| 跨核延迟 (64B) | < 3 us | P99, 单次调用 |
| 跨核延迟 (4KB) | < 10 us | P99, 单次调用 |
| 同核吞吐 | > 1M msg/s | 持续流式发送 |
| 跨核吞吐 | > 500K msg/s | 持续流式发送 |

---

## 3. 调度器基准测试

### 3.1 上下文切换时间

```rust
fn bench_context_switch(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler");

    group.bench_function("context_switch_yield", |b| {
        // 创建两个交替 yield 的线程
        let (t1, t2) = create_yield_pair();
        b.iter(|| {
            scheduler::yield_to(t2);
            scheduler::yield_to(t1);
        });
    });

    group.bench_function("context_switch_blocking", |b| {
        let (sender, receiver) = create_channel_pair();
        b.iter(|| {
            sender.send(1);
            receiver.recv();
        });
    });

    group.bench_function("context_switch_timer", |b| {
        b.iter(|| {
            // 等待下一个定时器 tick
            scheduler::sleep_ticks(1);
        });
    });

    group.finish();
}
```

### 3.2 调度延迟

```rust
fn bench_scheduling_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduling_latency");

    group.bench_function("wake_to_running", |b| {
        b.iter(|| {
            let (waker, waiter) = create_wait_pair();
            waker.wake();
            waiter.wait_for_schedule();
        });
    });

    group.bench_function("high_priority_preempt", |b| {
        b.iter(|| {
            // 低优先级运行中，高优先级就绪
            let high = spawn_high_priority(|| {});
            low_priority_work();
            high.join();
        });
    });

    group.finish();
}
```

### 3.3 调度器基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 上下文切换 (yield) | < 500 ns | P99, 同核 |
| 上下文切换 (阻塞) | < 2 us | P99, 含通道操作 |
| 调度延迟 (唤醒→运行) | < 5 us | P99 |
| 高优先级抢占延迟 | < 10 us | P99 |
| 定时器精度 | < 1% 误差 | 1000Hz 定时器 |

---

## 4. 内存管理基准测试

### 4.1 分配速度

```rust
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    group.bench_function("frame_alloc_free_cycle", |b| {
        let mut allocator = get_frame_allocator();
        b.iter(|| {
            if let Some(frame) = allocator.allocate() {
                allocator.free(frame);
            }
        });
    });

    group.bench_function("slab_alloc_free_64b", |b| {
        let mut slab = SlabAllocator::<[u8; 64]>::new(1024);
        b.iter(|| {
            if let Some(obj) = slab.allocate() {
                slab.free(obj);
            }
        });
    });

    group.bench_function("slab_alloc_free_4k", |b| {
        let mut slab = SlabAllocator::<[u8; 4096]>::new(256);
        b.iter(|| {
            if let Some(obj) = slab.allocate() {
                slab.free(obj);
            }
        });
    });

    group.finish();
}
```

### 4.2 页错误处理

```rust
fn bench_page_fault_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_fault");

    group.bench_function("cow_page_fault", |b| {
        let shared_page = allocate_shared_page();
        b.iter(|| {
            // 写入共享页面触发 COW
            write_to_shared_page(shared_page);
        });
    });

    group.bench_function("demand_paging", |b| {
        b.iter(|| {
            // 访问未映射页面触发按需分页
            access_unmapped_page();
        });
    });

    group.finish();
}
```

### 4.3 内存基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 帧分配+释放周期 | < 200 ns | 单次操作 |
| Slab 分配 (64B) | < 50 ns | 单次操作 |
| Slab 分配 (4KB) | < 100 ns | 单次操作 |
| COW 页错误 | < 5 us | 含 TLB 刷新 |
| 按需分页 | < 10 us | 含磁盘读取 |
| TLB 未命中惩罚 | < 100 ns | L2 TLB 命中 |

---

## 5. Agent 基准测试

### 5.1 Agent 生成时间

```rust
fn bench_agent_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent");

    group.bench_function("agent_spawn_minimal", |b| {
        b.iter(|| {
            agent::spawn(AgentConfig {
                name: "bench-agent",
                capabilities: CapSet::none(),
                memory_limit: MemoryLimit::new(1), // 1MB
                ..Default::default()
            })
        });
    });

    group.bench_function("agent_spawn_with_context", |b| {
        let context = AgentContext::preloaded("benchmark-context");
        b.iter(|| {
            agent::spawn(AgentConfig {
                name: "bench-agent",
                capabilities: CapSet::standard(),
                memory_limit: MemoryLimit::new(16),
                initial_context: Some(context.clone()),
                ..Default::default()
            })
        });
    });

    group.finish();
}
```

### 5.2 Agent 消息传递

```rust
fn bench_agent_messaging(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_messaging");

    group.bench_function("agent_send_256b", |b| {
        let agent = spawn_test_agent();
        let msg = AgentMessage::text(&"A".repeat(256));
        b.iter(|| agent.send(&msg));
    });

    group.bench_function("agent_send_structured", |b| {
        let agent = spawn_test_agent();
        let task = TaskDefinition::new("benchmark-task")
            .with_steps(10)
            .with_constraints(Constraints::default());
        b.iter(|| agent.send_task(&task));
    });

    group.bench_function("agent_pool_dispatch", |b| {
        let pool = AgentPool::new(8, AgentConfig::default());
        let task = TaskDefinition::simple("pool-task");
        b.iter(|| pool.dispatch(&task));
    });

    group.finish();
}
```

### 5.3 Agent 基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 最小 Agent 生成 | < 5 ms | 无上下文加载 |
| 带上下文 Agent 生成 | < 50 ms | 16MB 上下文 |
| Agent 消息传递 (256B) | < 100 us | 同核 |
| Agent 任务分发 | < 500 us | 含序列化 |
| Agent 池调度 (8 Agent) | < 1 ms | 单任务分发 |

---

## 6. 自动化基准测试

### 6.1 任务分解

```rust
fn bench_task_decomposition(c: &mut Criterion) {
    let mut group = c.benchmark_group("automation");

    group.bench_function("decompose_simple_task", |b| {
        let engine = AutomationEngine::new();
        let task = "将文件 A 复制到目录 B";
        b.iter(|| engine.decompose(task));
    });

    group.bench_function("decompose_complex_workflow", |b| {
        let engine = AutomationEngine::new();
        let task = "分析过去一周的销售数据，生成报告并发送给管理层";
        b.iter(|| engine.decompose(task));
    });

    group.finish();
}
```

### 6.2 工作流执行

```rust
fn bench_workflow_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow");

    group.bench_function("linear_chain_10_steps", |b| {
        let workflow = Workflow::linear(10);
        b.iter(|| workflow.execute());
    });

    group.bench_function("parallel_fan_out_8", |b| {
        let workflow = Workflow::parallel_fan_out(8);
        b.iter(|| workflow.execute());
    });

    group.bench_function("dag_workflow_20_nodes", |b| {
        let workflow = Workflow::dag_from_file("bench_dag_20.json");
        b.iter(|| workflow.execute());
    });

    group.finish();
}
```

### 6.3 自动化基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 简单任务分解 | < 50 ms | 本地推理 |
| 复杂任务分解 | < 500 ms | 本地推理 |
| 线性链执行 (10 步) | < 100 ms | 纯调度开销 |
| 并行扇出 (8 路) | < 50 ms | 纯调度开销 |
| DAG 工作流 (20 节点) | < 200 ms | 含依赖解析 |

---

## 7. 多模态基准测试

### 7.1 语音识别 (ASR)

```rust
fn bench_asr(c: &mut Criterion) {
    let mut group = c.benchmark_group("multimodal_asr");

    group.bench_function("asr_10s_audio", |b| {
        let audio = load_test_audio("test_10s.wav");
        let engine = AsrEngine::new(ModelSize::Small);
        b.iter(|| engine.recognize(&audio));
    });

    group.bench_function("asr_streaming_first_token", |b| {
        let audio = load_test_audio("test_10s.wav");
        let engine = AsrEngine::new(ModelSize::Small);
        b.iter(|| {
            let mut stream = engine.stream(&audio);
            stream.next_token() // 首个 token 延迟
        });
    });

    group.finish();
}
```

### 7.2 语音合成 (TTS)

```rust
fn bench_tts(c: &mut Criterion) {
    let mut group = c.benchmark_group("multimodal_tts");

    group.bench_function("tts_first_token_100chars", |b| {
        let engine = TtsEngine::new(ModelSize::Small);
        let text = "这是一段用于测试语音合成性能的文本内容。".repeat(3);
        b.iter(|| {
            let mut stream = engine.synthesize(&text);
            stream.next_audio_chunk() // 首个音频块延迟
        });
    });

    group.bench_function("tts_full_1000chars", |b| {
        let engine = TtsEngine::new(ModelSize::Small);
        let text = "测试文本。".repeat(100);
        b.iter(|| engine.synthesize_full(&text));
    });

    group.finish();
}
```

### 7.3 图像生成与嵌入

```rust
fn bench_image_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("multimodal_image");

    group.bench_function("text_to_image_512x512", |b| {
        let engine = ImageGenEngine::new(ModelSize::Medium);
        let prompt = "a serene mountain landscape at sunset";
        b.iter(|| engine.generate(prompt, 512, 512));
    });

    group.bench_function("image_embedding", |b| {
        let engine = EmbeddingEngine::new(ModelSize::Small);
        let image = load_test_image("test_256x256.png");
        b.iter(|| engine.embed_image(&image));
    });

    group.finish();
}
```

### 7.4 多模态基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| ASR 首个 token | < 200 ms | 10s 音频, 小模型 |
| ASR 完整识别 | < 2 s | 10s 音频, 小模型 |
| TTS 首个音频块 | < 150 ms | 100 字符, 小模型 |
| TTS 完整合成 | < 1 s | 1000 字符, 小模型 |
| 图像生成 (512x512) | < 5 s | 中等模型 |
| 图像嵌入 | < 50 ms | 256x256, 小模型 |
| 文本嵌入 | < 10 ms | 512 tokens, 小模型 |

---

## 8. 学习系统基准测试

### 8.1 知识图谱

```rust
fn bench_knowledge_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("learning");

    group.bench_function("kg_query_simple", |b| {
        let kg = KnowledgeGraph::load("bench_graph_10k.json");
        b.iter(|| kg.query("什么是微内核?"));
    });

    group.bench_function("kg_query_complex", |b| {
        let kg = KnowledgeGraph::load("bench_graph_10k.json");
        b.iter(|| kg.query("OmniAgent OS 的安全架构如何保护 Agent 隔离?"));
    });

    group.bench_function("kg_pattern_extraction", |b| {
        let kg = KnowledgeGraph::load("bench_graph_10k.json");
        let logs = load_test_logs("bench_logs_1k.json");
        b.iter(|| kg.extract_patterns(&logs));
    });

    group.finish();
}
```

### 8.2 反馈集成

```rust
fn bench_feedback_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("learning_feedback");

    group.bench_function("feedback_process_single", |b| {
        let learner = LearningEngine::new();
        let feedback = Feedback::positive("task-123", "执行正确");
        b.iter(|| learner.integrate_feedback(&feedback));
    });

    group.bench_function("feedback_batch_100", |b| {
        let learner = LearningEngine::new();
        let feedbacks: Vec<_> = (0..100)
            .map(|i| Feedback::scored(&format!("task-{}", i), 0.8))
            .collect();
        b.iter(|| learner.integrate_feedback_batch(&feedbacks));
    });

    group.finish();
}
```

### 8.3 学习系统基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 简单知识查询 | < 10 ms | 10K 节点图谱 |
| 复杂知识查询 | < 100 ms | 10K 节点图谱 |
| 模式提取 (1K 日志) | < 500 ms | 单次批处理 |
| 单条反馈处理 | < 5 ms | 含模型更新 |
| 批量反馈 (100 条) | < 100 ms | 含模型更新 |

---

## 9. 桌面基准测试

### 9.1 合成器性能

```rust
fn bench_compositor(c: &mut Criterion) {
    let mut group = c.benchmark_group("desktop_compositor");

    group.bench_function("composite_10_windows", |b| {
        let compositor = Compositor::new(WindowManager::new());
        let windows = create_test_windows(10);
        b.iter(|| compositor.compose_frame(&windows));
    });

    group.bench_function("composite_50_windows", |b| {
        let compositor = Compositor::new(WindowManager::new());
        let windows = create_test_windows(50);
        b.iter(|| compositor.compose_frame(&windows));
    });

    group.bench_function("composite_with_effects", |b| {
        let compositor = Compositor::new(WindowManager::new());
        let windows = create_test_windows(10);
        compositor.enable_effects(Effects::BLUR | Effects::SHADOW);
        b.iter(|| compositor.compose_frame(&windows));
    });

    group.finish();
}
```

### 9.2 窗口操作

```rust
fn bench_window_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("desktop_windows");

    group.bench_function("window_open", |b| {
        let wm = WindowManager::new();
        b.iter(|| wm.create_window(WindowSpec::default()));
    });

    group.bench_function("window_close", |b| {
        let wm = WindowManager::new();
        let win = wm.create_window(WindowSpec::default());
        b.iter(|| wm.close_window(win));
    });

    group.bench_function("window_resize", |b| {
        let wm = WindowManager::new();
        let win = wm.create_window(WindowSpec::default());
        b.iter(|| wm.resize_window(win, Size::new(1024, 768)));
    });

    group.finish();
}
```

### 9.3 Spotlight 搜索

```rust
fn bench_spotlight_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("desktop_spotlight");

    group.bench_function("spotlight_index_10k_files", |b| {
        let indexer = SpotlightIndexer::new();
        let files = generate_test_files(10000);
        b.iter(|| indexer.index_files(&files));
    });

    group.bench_function("spotlight_query", |b| {
        let spotlight = Spotlight::with_index("bench_index_10k");
        b.iter(|| spotlight.search("配置文件"));
    });

    group.finish();
}
```

### 9.4 桌面基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 合成器 FPS (10 窗口) | >= 60 FPS | 1080p, Vulkan |
| 合成器 FPS (50 窗口) | >= 30 FPS | 1080p, Vulkan |
| 窗口打开时间 | < 50 ms | 含动画 |
| 窗口关闭时间 | < 20 ms | 含动画 |
| Spotlight 索引 (10K 文件) | < 2 s | 首次索引 |
| Spotlight 查询 | < 100 ms | 10K 文件索引 |

---

## 10. 系统级基准测试

### 10.1 启动时间

```rust
fn bench_boot_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("system_boot");
    group.measurement_time(std::time::Duration::from_secs(60));

    group.bench_function("cold_boot", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            let qemu = QemuInstance::boot_kernel("kernel");
            qemu.wait_for_service("desktop-manager");
            let elapsed = start.elapsed();
            elapsed
        });
    });

    group.bench_function("warm_boot", |b| {
        let qemu = QemuInstance::boot_kernel("kernel");
        qemu.wait_for_service("desktop-manager");
        b.iter(|| {
            let start = std::time::Instant::now();
            qemu.reboot();
            qemu.wait_for_service("desktop-manager");
            start.elapsed()
        });
    });

    group.finish();
}
```

### 10.2 内存占用

```rust
fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("system_memory");

    group.bench_function("kernel_memory", |b| {
        b.iter(|| {
            let qemu = QemuInstance::boot_kernel("kernel");
            qemu.wait_for_string("KERNEL_READY");
            qemu.query_memory_usage("kernel")
        });
    });

    group.bench_function("idle_system_memory", |b| {
        b.iter(|| {
            let qemu = QemuInstance::boot_full_system();
            qemu.wait_for_service("desktop-manager");
            qemu.query_total_memory_usage()
        });
    });

    group.finish();
}
```

### 10.3 系统基准目标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 冷启动时间 | < 3 s | QEMU, 512MB RAM |
| 热启动时间 | < 1 s | QEMU, 512MB RAM |
| 内核内存占用 | < 8 MB | 空闲状态 |
| 空闲系统内存 | < 64 MB | 含桌面管理器 |
| 关机时间 | < 500 ms | 优雅关闭 |

---

## 11. 回归检测

### 11.1 基线管理

```yaml
# benches/baselines/main.yml
# 基线数据 - 在 main 分支上采集
version: "1.0"
environment: "x86_64-qemu-4core-8gb"
timestamp: "2026-04-25T00:00:00Z"

baselines:
  ipc:
    same_core_0b:
      median_ns: 450
      deviation: 0.05
    same_core_64b:
      median_ns: 850
      deviation: 0.05
    same_core_4096b:
      median_ns: 4200
      deviation: 0.08
    cross_core_64b:
      median_ns: 2500
      deviation: 0.08

  scheduler:
    context_switch_yield:
      median_ns: 380
      deviation: 0.10
    scheduling_latency:
      median_ns: 3500
      deviation: 0.10

  memory:
    frame_alloc_cycle:
      median_ns: 150
      deviation: 0.10
    slab_alloc_64b:
      median_ns: 35
      deviation: 0.10

  agent:
    spawn_minimal:
      median_ms: 3.5
      deviation: 0.15
    message_256b:
      median_us: 75
      deviation: 0.10

  system:
    cold_boot:
      median_ms: 2500
      deviation: 0.10
    kernel_memory:
      median_kb: 6144
      deviation: 0.10
```

### 11.2 回归检测算法

```rust
// benches/regression.rs
pub struct RegressionChecker {
    baselines: BaselineStore,
    config: RegressionConfig,
}

#[derive(Default)]
pub struct RegressionConfig {
    /// 回归阈值百分比 (超过此值标记为回归)
    pub regression_threshold: f64,  // 默认 10%
    /// 严重回归阈值 (超过此值阻断合并)
    pub critical_threshold: f64,   // 默认 25%
    /// 最小样本量
    pub min_samples: usize,        // 默认 50
    /// 置信区间
    pub confidence: f64,           // 默认 0.95
}

impl RegressionChecker {
    /// 检查单个基准是否回归
    pub fn check(&self, name: &str, new_data: &[f64]) -> RegressionResult {
        let baseline = self.baselines.get(name)
            .expect(&format!("未找到基线: {}", name));

        // 使用 Mann-Whitney U 检验比较新旧数据
        let is_significant = self.mann_whitney_test(
            &baseline.samples,
            new_data,
            self.config.confidence,
        );

        if !is_significant {
            return RegressionResult::NoChange;
        }

        let old_median = median(&baseline.samples);
        let new_median = median(new_data);
        let change_pct = (new_median - old_median) / old_median * 100.0;

        if change_pct > self.config.critical_threshold {
            RegressionResult::CriticalRegression {
                name: name.to_string(),
                old_median,
                new_median,
                change_pct,
            }
        } else if change_pct > self.config.regression_threshold {
            RegressionResult::Regression {
                name: name.to_string(),
                old_median,
                new_median,
                change_pct,
            }
        } else if change_pct < -self.config.regression_threshold {
            RegressionResult::Improvement {
                name: name.to_string(),
                old_median,
                new_median,
                change_pct,
            }
        } else {
            RegressionResult::NoChange
        }
    }

    /// 生成回归报告
    pub fn generate_report(&self, results: &[RegressionResult]) -> String {
        let mut report = String::new();
        report.push_str("# 性能回归报告\n\n");

        let critical: Vec<_> = results.iter()
            .filter(|r| matches!(r, RegressionResult::CriticalRegression { .. }))
            .collect();
        let regressions: Vec<_> = results.iter()
            .filter(|r| matches!(r, RegressionResult::Regression { .. }))
            .collect();
        let improvements: Vec<_> = results.iter()
            .filter(|r| matches!(r, RegressionResult::Improvement { .. }))
            .collect();

        report.push_str(&format!("## 严重回归: {} 项\n", critical.len()));
        for r in &critical {
            report.push_str(&format!("  - {}\n", r));
        }

        report.push_str(&format!("\n## 一般回归: {} 项\n", regressions.len()));
        for r in &regressions {
            report.push_str(&format!("  - {}\n", r));
        }

        report.push_str(&format!("\n## 性能改善: {} 项\n", improvements.len()));
        for r in &improvements {
            report.push_str(&format!("  - {}\n", r));
        }

        report
    }
}
```

---

## 12. 基准测试执行环境

### 12.1 环境标准化

```yaml
# benches/environment.yml
environment:
  name: "standard-benchmark"
  cpu:
    model: "Intel Xeon E5-2686 v4"  # 或等效
    cores: 4
    frequency: "2.3GHz (固定频率)"
    governor: "performance"
  memory:
    size: "8GB"
    type: "DDR4"
  storage:
    type: "NVMe SSD"
  os:
    host: "Ubuntu 24.04 LTS"
    kernel_version: "6.8"
  qemu:
    version: "8.2"
    options: "-cpu qemu64 -smp 4 -m 4096M"
  gpu:
    model: "NVIDIA Tesla T4"  # 用于桌面基准
    driver: "Vulkan 1.3"
```

### 12.2 可重复性保证

| 措施 | 描述 |
|------|------|
| **固定 CPU 频率** | 禁用频率调节，锁定最大频率 |
| **隔离 CPU 核心** | 使用 `taskset` 或 `cgroups` 隔离测试核心 |
| **禁用后台服务** | 测试期间停止不必要的系统服务 |
| **多次采样** | 每个基准至少运行 100 次迭代 |
| **统计显著性** | 使用 Mann-Whitney U 检验确保结果可靠 |
| **环境快照** | 记录完整的硬件和软件配置 |
| **温度控制** | 确保散热良好，避免热降频 |

### 12.3 执行脚本

```bash
#!/bin/bash
# benches/run_all_benchmarks.sh
set -euo pipefail

echo "=== OmniAgent OS 性能基准测试 ==="
echo "环境信息:"
echo "  CPU: $(lscpu | grep 'Model name' | awk -F: '{print $2}')"
echo "  内存: $(free -h | grep Mem | awk '{print $2}')"
echo "  QEMU: $(qemu-system-x86_64 --version | head -1)"
echo ""

# 固定 CPU 频率
echo "设置 CPU 性能模式..."
sudo cpupower frequency-set -g performance > /dev/null 2>&1 || true

# 运行用户态基准
echo "运行用户态基准..."
cargo bench -- --save-baseline "$(date +%Y%m%d)" 2>&1 | tee bench_results.txt

# 运行内核基准
echo "运行内核基准..."
cargo test --test kernel-benchmarks -- --nocapture 2>&1 | tee kernel_bench_results.txt

# 回归检测
echo "检查性能回归..."
cargo run --package bench-analyzer -- \
    --baseline benches/baselines/main.yml \
    --results bench_results.txt \
    --report benches/regression_report.md

echo "完成! 报告: benches/regression_report.md"
```

---

## 附录 A: 基准测试目录结构

```
benches/
├── baselines/                    # 基线数据
│   ├── main.yml                 # main 分支基线
│   └── history/                 # 历史基线
├── regression.rs                # 回归检测逻辑
├── environment.yml              # 环境配置
├── run_all_benchmarks.sh        # 执行脚本
├── ipc_benchmark.rs             # IPC 基准
├── scheduler_benchmark.rs       # 调度器基准
├── memory_benchmark.rs          # 内存基准
├── agent_benchmark.rs           # Agent 基准
├── automation_benchmark.rs      # 自动化基准
├── multimodal_benchmark.rs      # 多模态基准
├── learning_benchmark.rs        # 学习系统基准
├── desktop_benchmark.rs         # 桌面基准
└── system_benchmark.rs          # 系统级基准
```

## 附录 B: 基准测试结果示例

```
ipc_same_core_latency/0         time:   [423.5 ns 428.1 ns 433.2 ns]
ipc_same_core_latency/64        time:   [812.3 ns 820.5 ns 829.8 ns]
ipc_same_core_latency/256       time:   [1.23 us  1.25 us  1.27 us]
ipc_same_core_latency/1024      time:   [2.45 us  2.48 us  2.51 us]
ipc_same_core_latency/4096      time:   [4.12 us  4.18 us  4.25 us]
ipc_cross_core_latency/64       time:   [2.35 us  2.38 us  2.41 us]
ipc_cross_core_latency/4096     time:   [8.95 us  9.02 us  9.10 us]
scheduler/context_switch_yield  time:   [365 ns  370 ns  376 ns]
memory/frame_alloc_cycle        time:   [142 ns  145 ns  148 ns]
agent/spawn_minimal             time:   [3.21 ms 3.28 ms 3.35 ms]
system/cold_boot                time:   [2.35 s  2.41 s  2.48 s]
```
