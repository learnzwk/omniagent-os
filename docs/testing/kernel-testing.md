# OmniAgent OS 内核测试方法

> **文档版本**: v1.0.0 | **最后更新**: 2026-04-25 | **责任团队**: 内核工程与质量保障组

---

## 1. 概述

内核运行在 `no_std` 环境下，测试面临以下挑战：

| 挑战 | 解决方案 |
|------|---------|
| 无标准库支持 | 自定义测试运行器 |
| 硬件依赖 | QEMU 模拟 + 串口输出 |
| 并发测试 | 确定性调度 + 时序控制 |
| 崩溃恢复 | QEMU 自动重启 + 结果收集 |
| 性能测量 | TSC / APIC 计时器 |

```
┌─────────────────────────────────────────┐
│           L2 内核集成测试                 │
│  QEMU 启动 -> 串口测试 -> 结果收集       │
├─────────────────────────────────────────┤
│           L1 内核单元测试                 │
│  纯逻辑测试 (宿主机) + no_std (QEMU)     │
└─────────────────────────────────────────┘
```

---

## 2. 单元测试框架

### 2.1 宿主机可测试模块

部分内核模块不依赖硬件，可在宿主机直接运行：

```rust
// kernel/src/memory/frame_bitmap.rs
pub struct FrameBitmap {
    bitmap: Vec<u64>,
    total_frames: usize,
    free_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_first_frame() {
        let mut bitmap = FrameBitmap::new(1024);
        assert_eq!(bitmap.free_count(), 1024);
        let frame = bitmap.allocate();
        assert_eq!(frame, Some(0));
        assert!(bitmap.is_allocated(0));
        assert_eq!(bitmap.free_count(), 1023);
    }

    #[test]
    fn test_allocate_all_frames() {
        let mut bitmap = FrameBitmap::new(64);
        for i in 0..64 { assert_eq!(bitmap.allocate(), Some(i)); }
        assert_eq!(bitmap.allocate(), None);
    }

    #[test]
    fn test_free_and_reallocate() {
        let mut bitmap = FrameBitmap::new(64);
        let f1 = bitmap.allocate().unwrap();
        let f2 = bitmap.allocate().unwrap();
        bitmap.free(f1);
        assert_eq!(bitmap.allocate(), Some(f1));
    }

    #[test]
    #[should_panic(expected = "frame out of range")]
    fn test_free_invalid_frame() {
        let mut bitmap = FrameBitmap::new(64);
        bitmap.free(100);
    }
}
```

### 2.2 no_std 自定义测试框架

```rust
// kernel/src/test_framework.rs
#![cfg(test)]
use core::sync::atomic::{AtomicUsize, Ordering};

static TESTS_PASSED: AtomicUsize = AtomicUsize::new(0);
static TESTS_FAILED: AtomicUsize = AtomicUsize::new(0);

#[macro_export]
macro_rules! kernel_test {
    ($name:ident, $body:block) => {
        #[link_section = ".ktest"]
        #[used]
        pub static $name: $crate::test_framework::KernelTest = $crate::test_framework::KernelTest {
            name: stringify!($name),
            run: |reporter: &$crate::test_framework::TestReporter| {
                let result = core::panic::catch_unwind(core::panic::AssertUnwindSafe(|| $body));
                match result {
                    Ok(()) => reporter.pass(stringify!($name)),
                    Err(_) => reporter.fail(stringify!($name)),
                }
            },
        };
    };
}

pub struct TestReporter;
impl TestReporter {
    pub fn pass(&self, name: &str) {
        TESTS_PASSED.fetch_add(1, Ordering::SeqCst);
        serial_println!("[PASS] {}", name);
    }
    pub fn fail(&self, name: &str) {
        TESTS_FAILED.fetch_add(1, Ordering::SeqCst);
        serial_println!("[FAIL] {}", name);
    }
    pub fn summary(&self) {
        serial_println!("\n=== 总计: {} 通过: {} 失败: {} ===\n",
            TESTS_PASSED.load(Ordering::SeqCst) + TESTS_FAILED.load(Ordering::SeqCst),
            TESTS_PASSED.load(Ordering::SeqCst),
            TESTS_FAILED.load(Ordering::SeqCst));
    }
}

pub fn run_all_tests() {
    unsafe {
        let start = &__ktest_start as *const KernelTest;
        let end = &__ktest_end as *const KernelTest;
        let count = (end as usize - start as usize) / core::mem::size_of::<KernelTest>();
        let reporter = TestReporter;
        for i in 0..count { (*start.add(i)).run(&reporter); }
        reporter.summary();
    }
}
```

### 2.3 断言宏

```rust
#[macro_export]
macro_rules! kassert_eq {
    ($left:expr, $right:expr) => {
        if $left != $right {
            serial_println!("  断言失败: {} (左={:?}, 右={:?})", stringify!($left), $left, $right);
            panic!("断言失败");
        }
    };
}

#[macro_export]
macro_rules! kassert_ok {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => { serial_println!("  期望 Ok, 得到 Err({:?})", e); panic!(); }
        }
    };
}
```

---

## 3. QEMU 集成测试

### 3.1 测试启动流程

```
编译内核 -> 启动 QEMU -> 等待引导完成 -> 运行测试 -> 收集串口输出 -> 解析结果 -> 生成报告
```

### 3.2 QEMU 测试工具

```rust
// tests/qemu_test_runner.rs
pub struct QemuTestRunner {
    qemu_path: String,
    kernel_path: String,
    timeout: Duration,
}

#[derive(Debug)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration: Duration,
}

impl QemuTestRunner {
    pub fn new(kernel_path: &Path) -> Self {
        Self { qemu_path: "qemu-system-x86_64".into(),
            kernel_path: kernel_path.to_string_lossy().into(),
            timeout: Duration::from_secs(120) }
    }

    pub fn run(&self) -> Vec<TestResult> {
        let mut child = Command::new(&self.qemu_path)
            .args(&["-kernel", &self.kernel_path, "-serial", "stdio",
                "-nographic", "-m", "512M", "-smp", "2", "-no-reboot"])
            .stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("无法启动 QEMU");

        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let start = Instant::now();
        let mut results = Vec::new();

        for line in reader.lines() {
            if start.elapsed() > self.timeout { child.kill().ok(); panic!("超时"); }
            let line = line.unwrap();
            if let Some(rest) = line.strip_prefix("[PASS] ") {
                results.push(TestResult { name: rest.trim().into(), passed: true, duration: start.elapsed() });
            } else if let Some(rest) = line.strip_prefix("[FAIL] ") {
                results.push(TestResult { name: rest.trim().into(), passed: false, duration: start.elapsed() });
            }
        }
        results
    }
}

#[test]
fn test_kernel_boot_and_run_tests() {
    let runner = QemuTestRunner::new(&Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/x86_64-omniagent/debug/kernel"));
    let results = runner.run();
    let failed: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    assert!(failed.is_empty(), "失败: {:?}", failed);
}
```

---

## 4. 内存管理测试

```rust
kernel_test!(test_page_table_mapping, {
    let pt = PageTable::new();
    kassert_eq!(pt.entry_count(), 0);
    let virt = VirtualAddress::new(0x1000);
    let phys = PhysicalAddress::new(0x5000);
    pt.map_page(virt, phys, PageFlags::READ | PageFlags::WRITE | PageFlags::PRESENT).expect("映射失败");
    kassert_eq!(pt.entry_count(), 1);
    let entry = pt.lookup(virt).expect("查找失败");
    kassert_eq!(entry.physical_address(), phys);
    kassert!(entry.flags().contains(PageFlags::PRESENT));
    pt.unmap_page(virt).expect("解除映射失败");
    kassert_eq!(pt.entry_count(), 0);
});

kernel_test!(test_allocation_stress, {
    let mut allocator = FrameAllocator::new(4096);
    let mut allocated = Vec::new();
    for _ in 0..4096 { allocated.push(allocator.allocate().expect("提前耗尽")); }
    kassert!(allocator.allocate().is_none());
    for frame in allocated.drain(..2048) { allocator.free(frame); }
    for _ in 0..2048 { kassert!(allocator.allocate().is_some()); }
});
```

---

## 5. 调度器测试

```rust
kernel_test!(test_scheduler_timing, {
    let scheduler = Scheduler::new(2);
    let high_pid = scheduler.create_process("high", Priority::High, || {});
    let low_pid = scheduler.create_process("low", Priority::Low, || {});
    let start = read_tsc();
    scheduler.schedule();
    let high_time = scheduler.process_cpu_time(high_pid);
    let low_time = scheduler.process_cpu_time(low_pid);
    kassert!(high_time > low_time * 2, "高优先级应获得更多 CPU 时间");
});

kernel_test!(test_priority_inversion_detection, {
    let scheduler = Scheduler::new(1);
    let high = scheduler.create_process("high", Priority::High, || {});
    let low = scheduler.create_process("low", Priority::Low, || {});
    scheduler.acquire_lock(low, "shared-lock");
    scheduler.try_acquire_lock(high, "shared-lock");
    kassert!(!scheduler.detect_priority_inversion(), "应通过优先级继承避免反转");
});

kernel_test!(test_load_balancing, {
    let scheduler = Scheduler::new(4);
    for i in 0..20 { scheduler.create_process_on_core(&format!("w-{}", i), Priority::Normal, 0, || {}); }
    scheduler.balance_load();
    let loads: Vec<usize> = (0..4).map(|c| scheduler.process_count_on_core(c)).collect();
    let imbalance = *loads.iter().max().unwrap() as f64 / *loads.iter().min().unwrap() as f64;
    kassert!(imbalance < 2.0, "负载不均衡: {:?}", loads);
});
```

---

## 6. IPC 测试

```rust
kernel_test!(test_ipc_latency, {
    let service = create_test_service("latency-service");
    for _ in 0..10 { ipc_send_sync(service, &IpcMessage::ping()); }
    let mut latencies = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let start = read_tsc();
        ipc_send_sync(service, &IpcMessage::ping());
        latencies.push(tsc_to_nanos(read_tsc() - start));
    }
    latencies.sort();
    kassert!(latencies[990] < 5000, "IPC p99 延迟过高: {}ns", latencies[990]);
});

kernel_test!(test_ipc_message_integrity, {
    let service = create_test_service("integrity-service");
    for size in [0, 64, 1024, 4096] {
        let payload: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let reply = ipc_send_sync(service, &IpcMessage::new("svc", "echo", &payload)).expect("失败");
        kassert_eq!(reply.payload(), &payload, "消息大小 {} 完整性校验失败", size);
    }
});
```

---

## 7. 系统调用测试

```rust
kernel_test!(test_syscall_invalid_parameters, {
    let result = syscall_write(1, VirtualAddress::new(0), 100);
    kassert_eq!(result, Err(SyscallError::InvalidAddress));
    let result = syscall_write(999, VirtualAddress::new(0x1000), 10);
    kassert_eq!(result, Err(SyscallError::BadFd));
    let result = syscall_bind_port(80);
    kassert_eq!(result, Err(SyscallError::PermissionDenied));
});

kernel_test!(test_syscall_capability_check, {
    let process = create_test_process("unprivileged");
    let result = process.syscall_socket(SocketDomain::INET, SocketType::Stream);
    kassert_eq!(result, Err(SyscallError::CapabilityMissing("network")));
    process.grant_capability(Capability::Network);
    kassert!(process.syscall_socket(SocketDomain::INET, SocketType::Stream).is_ok());
});
```

---

## 8. 中断测试

```rust
kernel_test!(test_interrupt_handler_registration, {
    let idt = InterruptDescriptorTable::new();
    let handler_called = AtomicBool::new(false);
    idt.register_handler(0x80, || { handler_called.store(true, Ordering::SeqCst); });
    asm!("int $0x80");
    kassert!(handler_called.load(Ordering::SeqCst));
});

kernel_test!(test_nested_interrupts, {
    let idt = InterruptDescriptorTable::new();
    let depth = AtomicUsize::new(0);
    let max_depth = AtomicUsize::new(0);
    idt.register_handler(0x80, || {
        let current = depth.fetch_add(1, Ordering::SeqCst) + 1;
        let prev = max_depth.load(Ordering::SeqCst);
        if current > prev { max_depth.store(current, Ordering::SeqCst); }
        if current < 3 { asm!("int $0x80"); }
        depth.fetch_sub(1, Ordering::SeqCst);
    });
    asm!("int $0x80");
    kassert_eq!(max_depth.load(Ordering::SeqCst), 3);
    kassert_eq!(depth.load(Ordering::SeqCst), 0);
});
```

---

## 9. 崩溃测试

```rust
kernel_test!(test_kernel_panic_recovery, {
    let result = catch_kernel_panic(|| { panic!("测试性崩溃"); });
    kassert!(result.is_some());
    kassert!(result.unwrap().contains("测试性崩溃"));
});

kernel_test!(test_kernel_oops_recovery, {
    let oops_handler = OopsHandler::new();
    oops_handler.set_action(OopsAction::Continue);
    trigger_oops(OopsType::NullPointerDereference);
    kassert!(kernel_is_running());
    kassert!(!oops_handler.get_log().is_empty());
});
```

---

## 10. 性能基准测试

```rust
// benches/kernel/ipc_latency.rs
kernel_benchmark!(bench_ipc_zero_copy, {
    let service = create_test_service("bench-svc");
    let iterations = 10000;
    let flags = cpu::disable_interrupts();
    let start = read_tsc();
    for _ in 0..iterations { ipc_send_zero_copy(service, &[]); }
    let end = read_tsc();
    cpu::restore_interrupts(flags);
    let avg_ns = tsc_to_nanos((end - start) / iterations);
    report_metric("ipc_zero_copy_latency_ns", avg_ns);
});

kernel_benchmark!(bench_context_switch, {
    let iterations = 100000;
    let start = read_tsc();
    for _ in 0..iterations { scheduler::yield_current(); }
    let end = read_tsc();
    report_metric("context_switch_ns", tsc_to_nanos((end - start) / iterations));
});
```

---

## 11. 内核模块测试矩阵

| 模块 | 测试类别 | 用例数 | 优先级 |
|------|---------|--------|--------|
| **内存管理** | 页表映射/帧分配/Slab/OOM | 50 | P0 |
| **调度器** | 进程创建/优先级/负载均衡/死锁 | 49 | P0 |
| **IPC** | 同步/异步/共享内存/并发/完整性 | 51 | P0 |
| **系统调用** | 参数验证/权限/缓冲区溢出 | 57 | P0 |
| **中断** | 注册/嵌套/定时器/优先级 | 27 | P0 |
| **崩溃处理** | Panic/Oops/双重故障/转储 | 17 | P0 |

### 优先级定义

| 优先级 | 定义 | 要求 |
|--------|------|------|
| **P0** | 关键路径，失败阻断发布 | 每次提交必须通过 |
| **P1** | 重要功能，失败需要修复 | 每日构建必须通过 |
| **P2** | 边界情况，失败记录为缺陷 | 每周构建检查 |

---

## 附录: 常用命令与排查

```bash
cargo test -p omniagent-kernel --lib              # 宿主机内核单元测试
cargo test --test kernel-integration -- --nocapture # QEMU 集成测试
cargo bench -p omniagent-kernel                     # 内核基准测试
cargo llvm-cov -p omniagent-kernel --html           # 覆盖率报告
cargo fuzz run -p omniagent-kernel syscall_fuzzer   # 模糊测试
```

| 失败模式 | 可能原因 | 排查步骤 |
|---------|---------|---------|
| QEMU 启动超时 | 内核引导失败 | 检查串口输出最后几行 |
| 测试结果丢失 | 串口缓冲区溢出 | 增大缓冲区或减少输出 |
| 不确定性行为 | 时序依赖 | 使用确定性调度器 |
| 死锁 | 锁顺序错误 | 检查锁获取顺序和超时 |
