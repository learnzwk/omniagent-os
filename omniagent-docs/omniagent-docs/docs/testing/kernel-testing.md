# OmniAgent OS 内核测试方法

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: 正式发布
> **责任团队**: 内核工程与质量保障组

---

## 1. 概述

### 1.1 内核测试挑战

OmniAgent OS 内核运行在 `no_std` 环境下，无法使用 Rust 标准库的测试框架。内核测试面临以下核心挑战：

| 挑战 | 描述 | 解决方案 |
|------|------|---------|
| 无标准库支持 | `no_std` 环境缺少 `std::test` | 自定义测试运行器 |
| 硬件依赖 | 页表、中断等需要真实硬件 | QEMU 模拟 + 串口输出 |
| 并发测试 | 调度器、IPC 涉及多核并发 | 确定性调度 + 时序控制 |
| 崩溃恢复 | 测试可能触发内核 panic | QEMU 自动重启 + 结果收集 |
| 性能测量 | 需要精确的纳秒级计时 | TSC / APIC 计时器 |

### 1.2 测试分层

```
┌─────────────────────────────────────────┐
│           L2 内核集成测试                 │
│  QEMU 启动 → 串口测试 → 结果收集         │
├─────────────────────────────────────────┤
│           L1 内核单元测试                 │
│  纯逻辑测试 (宿主机 cargo test)          │
│  + no_std 自定义测试 (QEMU)              │
└─────────────────────────────────────────┘
```

---

## 2. 单元测试框架

### 2.1 宿主机可测试模块

部分内核模块不依赖硬件，可在宿主机上直接运行 `cargo test`：

```rust
// kernel/src/memory/frame_bitmap.rs
/// 帧位图分配器 - 纯逻辑，可在宿主机测试
pub struct FrameBitmap {
    bitmap: Vec<u64>,
    total_frames: usize,
    free_count: usize,
}

impl FrameBitmap {
    pub fn new(total_frames: usize) -> Self { /* ... */ }
    pub fn allocate(&mut self) -> Option<usize> { /* ... */ }
    pub fn free(&mut self, frame: usize) { /* ... */ }
    pub fn is_allocated(&self, frame: usize) -> bool { /* ... */ }
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
        for i in 0..64 {
            assert_eq!(bitmap.allocate(), Some(i));
        }
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
        bitmap.free(100); // 应触发 panic
    }
}
```

### 2.2 no_std 自定义测试框架

对于依赖硬件的内核模块，使用自定义测试框架在 QEMU 中运行：

```rust
// kernel/src/test_framework.rs
#![cfg(test)]

use core::sync::atomic::{AtomicUsize, Ordering};

static TESTS_PASSED: AtomicUsize = AtomicUsize::new(0);
static TESTS_FAILED: AtomicUsize = AtomicUsize::new(0);
static TESTS_RUN: AtomicUsize = AtomicUsize::new(0);

/// 内核测试注册宏
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

/// 测试报告器
pub struct TestReporter;

impl TestReporter {
    pub fn pass(&self, name: &str) {
        TESTS_PASSED.fetch_add(1, Ordering::SeqCst);
        TESTS_RUN.fetch_add(1, Ordering::SeqCst);
        serial_println!("[PASS] {}", name);
    }

    pub fn fail(&self, name: &str) {
        TESTS_FAILED.fetch_add(1, Ordering::SeqCst);
        TESTS_RUN.fetch_add(1, Ordering::SeqCst);
        serial_println!("[FAIL] {}", name);
    }

    pub fn summary(&self) {
        let run = TESTS_RUN.load(Ordering::SeqCst);
        let passed = TESTS_PASSED.load(Ordering::SeqCst);
        let failed = TESTS_FAILED.load(Ordering::SeqCst);
        serial_println!("\n=== 内核测试摘要 ===");
        serial_println!("总计: {} | 通过: {} | 失败: {}", run, passed, failed);
        serial_println!("====================\n");
    }
}

/// 测试入口点 - 由内核引导后调用
pub fn run_all_tests() {
    serial_println!("开始内核测试...\n");

    extern "C" {
        static __ktest_start: KernelTest;
        static __ktest_end: KernelTest;
    }

    unsafe {
        let start = &__ktest_start as *const KernelTest;
        let end = &__ktest_end as *const KernelTest;
        let count = (end as usize - start as usize) / core::mem::size_of::<KernelTest>();

        let reporter = TestReporter;
        for i in 0..count {
            let test = &*start.add(i);
            serial_println!("运行: {}...", test.name);
            (test.run)(&reporter);
        }

        reporter.summary();
    }
}
```

### 2.3 断言宏

```rust
// kernel/src/test_framework.rs (续)

#[macro_export]
macro_rules! kassert_eq {
    ($left:expr, $right:expr) => {
        if $left != $right {
            serial_println!(
                "  断言失败: {} == {} (左={}, 右={})",
                stringify!($left), stringify!($right),
                $left, $right
            );
            panic!("断言失败");
        }
    };
}

#[macro_export]
macro_rules! kassert {
    ($cond:expr) => {
        if !$cond {
            serial_println!("  断言失败: {}", stringify!($cond));
            panic!("断言失败");
        }
    };
}

#[macro_export]
macro_rules! kassert_ok {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                serial_println!("  断言失败: 期望 Ok, 得到 Err({:?})", e);
                panic!("断言失败");
            }
        }
    };
}
```

---

## 3. QEMU 集成测试

### 3.1 测试启动流程

```
┌──────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────┐
│  编译内核  │ →  │  启动 QEMU   │ →  │  等待引导完成  │ →  │  运行测试  │
│  (cargo)  │    │  (qemu-system)│    │  (串口监听)    │    │  (串口命令)│
└──────────┘    └──────────────┘    └──────────────┘    └──────────┘
                                                              │
┌──────────┐    ┌──────────────┐    ┌──────────────┐         │
│  生成报告  │ ←  │  解析结果     │ ←  │  收集串口输出  │ ←───────┘
│  (JUnit)  │    │  (正则匹配)   │    │  (超时控制)    │
└──────────┘    └──────────────┘    └──────────────┘
```

### 3.2 QEMU 测试工具

```rust
// tests/qemu_test_runner.rs
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};
use std::path::Path;

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
        Self {
            qemu_path: "qemu-system-x86_64".to_string(),
            kernel_path: kernel_path.to_string_lossy().to_string(),
            timeout: Duration::from_secs(120),
        }
    }

    /// 启动 QEMU 并运行内核测试
    pub fn run(&self) -> Vec<TestResult> {
        let mut child = Command::new(&self.qemu_path)
            .args(&[
                "-kernel", &self.kernel_path,
                "-serial", "stdio",
                "-nographic",
                "-m", "512M",
                "-cpu", "qemu64",
                "-smp", "2",  // 双核用于并发测试
                "-no-reboot", // panic 后不重启
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("无法启动 QEMU");

        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let start = Instant::now();
        let mut results = Vec::new();

        for line in reader.lines() {
            if start.elapsed() > self.timeout {
                child.kill().ok();
                panic!("QEMU 测试超时");
            }

            let line = line.unwrap();
            // 解析 [PASS] / [FAIL] 输出
            if let Some(rest) = line.strip_prefix("[PASS] ") {
                results.push(TestResult {
                    name: rest.trim().to_string(),
                    passed: true,
                    duration: start.elapsed(),
                });
            } else if let Some(rest) = line.strip_prefix("[FAIL] ") {
                results.push(TestResult {
                    name: rest.trim().to_string(),
                    passed: false,
                    duration: start.elapsed(),
                });
            }
        }

        let _ = child.wait();
        results
    }
}

#[test]
fn test_kernel_boot_and_run_tests() {
    let kernel_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/x86_64-omniagent/debug/kernel");

    let runner = QemuTestRunner::new(&kernel_path);
    let results = runner.run();

    let failed: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    assert!(failed.is_empty(), "内核测试失败: {:?}", failed);
    println!("内核测试全部通过: {} 项", results.len());
}
```

### 3.3 串口输出解析

```python
# tests/serial_parser.py
"""QEMU 串口输出解析器，生成 JUnit XML 报告"""
import re
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from typing import List

@dataclass
class KernelTestResult:
    name: str
    status: str  # "pass" or "fail"
    output: str = ""

class SerialOutputParser:
    def __init__(self):
        self.results: List[KernelTestResult] = []
        self.current_output: List[str] = []
        self.test_name: str = ""

    def parse(self, serial_output: str) -> List[KernelTestResult]:
        for line in serial_output.splitlines():
            if line.startswith("运行: "):
                self.test_name = line[4:]
                self.current_output = []
            elif line.startswith("[PASS] "):
                self.results.append(KernelTestResult(
                    name=line[6:].strip(),
                    status="pass",
                    output="\n".join(self.current_output)
                ))
            elif line.startswith("[FAIL] "):
                self.results.append(KernelTestResult(
                    name=line[6:].strip(),
                    status="fail",
                    output="\n".join(self.current_output)
                ))
            else:
                self.current_output.append(line)
        return self.results

    def to_junit_xml(self) -> str:
        testsuite = ET.Element("testsuite", name="kernel-tests")
        for result in self.results:
            testcase = ET.SubElement(testsuite, "testcase",
                name=result.name, classname="kernel")
            if result.status == "fail":
                ET.SubElement(testcase, "failure",
                    message="测试失败").text = result.output
        return ET.tostring(testsuite, encoding="unicode")
```

---

## 4. 内存管理测试

### 4.1 页表验证测试

```rust
kernel_test!(test_page_table_mapping, {
    // 创建新的页表
    let pt = PageTable::new();
    kassert_eq!(pt.entry_count(), 0);

    // 映射虚拟地址 0x1000 到物理地址 0x5000
    let virt = VirtualAddress::new(0x1000);
    let phys = PhysicalAddress::new(0x5000);
    let flags = PageFlags::READ | PageFlags::WRITE | PageFlags::PRESENT;

    pt.map_page(virt, phys, flags).expect("映射失败");
    kassert_eq!(pt.entry_count(), 1);

    // 验证映射正确性
    let entry = pt.lookup(virt).expect("查找失败");
    kassert_eq!(entry.physical_address(), phys);
    kassert!(entry.flags().contains(PageFlags::PRESENT));
    kassert!(entry.flags().contains(PageFlags::WRITE));

    // 解除映射
    pt.unmap_page(virt).expect("解除映射失败");
    kassert_eq!(pt.entry_count(), 0);
    kassert!(pt.lookup(virt).is_none());
});

kernel_test!(test_page_table_large_mapping, {
    let pt = PageTable::new();

    // 映射 100 个连续页面
    for i in 0..100 {
        let virt = VirtualAddress::new(0x1000 * (i + 1));
        let phys = PhysicalAddress::new(0x1000 * (i + 100));
        pt.map_page(virt, phys, PageFlags::READ | PageFlags::PRESENT)
            .expect("映射失败");
    }
    kassert_eq!(pt.entry_count(), 100);

    // 验证所有映射
    for i in 0..100 {
        let virt = VirtualAddress::new(0x1000 * (i + 1));
        let entry = pt.lookup(virt).expect("查找失败");
        kassert_eq!(entry.physical_address(), PhysicalAddress::new(0x1000 * (i + 100)));
    }
});
```

### 4.2 内存分配压力测试

```rust
kernel_test!(test_allocation_stress, {
    let mut allocator = FrameAllocator::new(4096); // 4096 个帧 = 16MB
    let mut allocated = Vec::new();

    // 分配所有可用帧
    for _ in 0..4096 {
        match allocator.allocate() {
            Some(frame) => allocated.push(frame),
            None => panic!("提前耗尽内存"),
        }
    }

    // 应该无法再分配
    kassert!(allocator.allocate().is_none());

    // 释放一半
    for frame in allocated.drain(..2048) {
        allocator.free(frame);
    }

    // 应该可以重新分配
    for _ in 0..2048 {
        kassert!(allocator.allocate().is_some());
    }
});

kernel_test!(test_slab_allocator_correctness, {
    let mut slab = SlabAllocator::<ProcessDescriptor>::new(64);

    // 分配多个对象
    let mut ptrs = Vec::new();
    for i in 0..64 {
        let ptr = slab.allocate();
        kassert!(ptr.is_some());
        ptrs.push(ptr.unwrap());
    }

    // 应该已满
    kassert!(slab.allocate().is_none());

    // 释放并重新分配
    slab.free(ptrs[10]);
    let new_ptr = slab.allocate();
    kassert!(new_ptr.is_some());
    kassert_eq!(new_ptr.unwrap(), ptrs[10]);
});
```

---

## 5. 调度器测试

### 5.1 定时验证测试

```rust
kernel_test!(test_scheduler_timing, {
    let scheduler = Scheduler::new(2); // 双核

    // 创建高优先级和低优先级进程
    let high_pid = scheduler.create_process(
        "high-priority",
        Priority::High,
        || { /* 空任务 */ }
    );
    let low_pid = scheduler.create_process(
        "low-priority",
        Priority::Low,
        || { /* 空任务 */ }
    );

    let start = read_tsc();
    scheduler.schedule(); // 运行一个时间片
    let elapsed = read_tsc() - start;

    // 高优先级进程应获得更多 CPU 时间
    let high_time = scheduler.process_cpu_time(high_pid);
    let low_time = scheduler.process_cpu_time(low_pid);
    kassert!(high_time > low_time * 2, "高优先级应获得更多 CPU 时间");
});

kernel_test!(test_priority_inversion_detection, {
    let scheduler = Scheduler::new(1);

    let high = scheduler.create_process("high", Priority::High, || {});
    let medium = scheduler.create_process("medium", Priority::Medium, || {});
    let low = scheduler.create_process("low", Priority::Low, || {});

    // 低优先级进程持有锁
    scheduler.acquire_lock(low, "shared-lock");

    // 高优先级进程尝试获取同一把锁
    scheduler.try_acquire_lock(high, "shared-lock");

    // 中等优先级进程不应阻塞高优先级（优先级继承应生效）
    let inversion_detected = scheduler.detect_priority_inversion();
    kassert!(!inversion_detected, "应通过优先级继承避免优先级反转");
});
```

### 5.2 负载均衡测试

```rust
kernel_test!(test_load_balancing, {
    let scheduler = Scheduler::new(4); // 四核

    // 在核心 0 上创建大量进程
    for i in 0..20 {
        scheduler.create_process_on_core(
            &format!("worker-{}", i),
            Priority::Normal,
            0, // 全部放在核心 0
            || { loop {} }
        );
    }

    // 触发负载均衡
    scheduler.balance_load();

    // 验证负载分布
    let core_loads: Vec<usize> = (0..4)
        .map(|core| scheduler.process_count_on_core(core))
        .collect();

    let max_load = *core_loads.iter().max().unwrap();
    let min_load = *core_loads.iter().min().unwrap();
    let imbalance = max_load as f64 / min_load as f64;

    kassert!(imbalance < 2.0, "负载不均衡: 各核负载 {:?}", core_loads);
});
```

---

## 6. IPC 测试

### 6.1 延迟测量

```rust
kernel_test!(test_ipc_latency, {
    let service = create_test_service("latency-service");

    // 预热
    for _ in 0..10 {
        ipc_send_sync(service, &IpcMessage::ping());
    }

    // 测量 1000 次 IPC 调用的延迟
    let mut latencies = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let start = read_tsc();
        ipc_send_sync(service, &IpcMessage::ping());
        let end = read_tsc();
        latencies.push(tsc_to_nanos(end - start));
    }

    latencies.sort();
    let p50 = latencies[500];
    let p99 = latencies[990];
    let p999 = latencies[999];

    serial_println!("  IPC 延迟: p50={}ns, p99={}ns, p999={}ns", p50, p99, p999);

    // 同核 IPC 延迟应 < 5 微秒
    kassert!(p99 < 5000, "IPC p99 延迟过高: {}ns", p99);
});

kernel_test!(test_ipc_cross_core_latency, {
    let service = create_test_service_on_core("cross-core-svc", 1);

    let mut latencies = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let start = read_tsc();
        ipc_send_sync(service, &IpcMessage::ping());
        let end = read_tsc();
        latencies.push(tsc_to_nanos(end - start));
    }

    latencies.sort();
    let p99 = latencies[990];

    // 跨核 IPC 延迟应 < 20 微秒
    kassert!(p99 < 20000, "跨核 IPC p99 延迟过高: {}ns", p99);
});
```

### 6.2 正确性验证

```rust
kernel_test!(test_ipc_message_integrity, {
    let service = create_test_service("integrity-service");

    // 测试不同大小的消息
    for size in [0, 1, 64, 256, 1024, 4096, 16384] {
        let payload: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let msg = IpcMessage::new("integrity-service", "echo", &payload);
        let reply = ipc_send_sync(service, &msg).expect("IPC 发送失败");

        kassert_eq!(reply.payload(), &payload,
            "消息大小 {} 的完整性校验失败", size);
    }
});

kernel_test!(test_ipc_concurrent_senders, {
    let service = create_test_service("concurrent-service");
    let num_senders = 8;
    let messages_per_sender = 100;

    // 启动多个发送者并发发送
    let handles: Vec<_> = (0..num_senders)
        .map(|id| {
            let svc = service.clone();
            spawn_thread(move || {
                for i in 0..messages_per_sender {
                    let msg = IpcMessage::new(
                        "concurrent-service",
                        "counter",
                        &[(id as u8), (i as u8)]
                    );
                    let reply = ipc_send_sync(&svc, &msg).expect("发送失败");
                    kassert!(reply.is_ok());
                }
            })
        })
        .collect();

    // 等待所有发送者完成
    for handle in handles {
        handle.join().expect("线程 panic");
    }

    // 验证服务收到的消息总数
    let total = service.message_count();
    kassert_eq!(total, num_senders * messages_per_sender);
});
```

---

## 7. 系统调用测试

### 7.1 参数验证

```rust
kernel_test!(test_syscall_invalid_parameters, {
    // 空指针缓冲区
    let result = syscall_write(1, VirtualAddress::new(0), 100);
    kassert_eq!(result, Err(SyscallError::InvalidAddress));

    // 负数长度
    let result = syscall_write(1, VirtualAddress::new(0x1000), usize::MAX);
    kassert_eq!(result, Err(SyscallError::InvalidParameter));

    // 无效文件描述符
    let result = syscall_write(999, VirtualAddress::new(0x1000), 10);
    kassert_eq!(result, Err(SyscallError::BadFd));

    // 超出权限的端口
    let result = syscall_bind_port(80);
    kassert_eq!(result, Err(SyscallError::PermissionDenied));
});

kernel_test!(test_syscall_buffer_overflow_protection, {
    let buffer = allocate_user_buffer(100);
    // 尝试读取超出缓冲区范围
    let result = syscall_read(0, buffer.addr(), 200); // 缓冲区只有 100 字节
    kassert_eq!(result, Err(SyscallError::BufferOverflow));
});
```

### 7.2 权限检查

```rust
kernel_test!(test_syscall_capability_check, {
    let process = create_test_process("unprivileged");

    // 无网络能力时尝试创建 socket
    let result = process.syscall_socket(SocketDomain::INET, SocketType::Stream);
    kassert_eq!(result, Err(SyscallError::CapabilityMissing("network")));

    // 授予网络能力后重试
    process.grant_capability(Capability::Network);
    let result = process.syscall_socket(SocketDomain::INET, SocketType::Stream);
    kassert!(result.is_ok());

    // 尝试执行需要管理员权限的操作
    let result = process.syscall_mount("/test", "tmpfs");
    kassert_eq!(result, Err(SyscallError::CapabilityMissing("admin")));
});
```

---

## 8. 中断测试

### 8.1 中断处理注册

```rust
kernel_test!(test_interrupt_handler_registration, {
    let idt = InterruptDescriptorTable::new();

    // 注册自定义中断处理程序
    let handler_called = AtomicBool::new(false);
    idt.register_handler(0x80, || {
        handler_called.store(true, Ordering::SeqCst);
    });

    // 触发软中断
    asm!("int $0x80" :: "N"(0x80));

    // 验证处理程序被调用
    kassert!(handler_called.load(Ordering::SeqCst), "中断处理程序未被调用");
});

kernel_test!(test_nested_interrupts, {
    let idt = InterruptDescriptorTable::new();
    let depth = AtomicUsize::new(0);
    let max_depth = AtomicUsize::new(0);

    idt.register_handler(0x80, || {
        let current = depth.fetch_add(1, Ordering::SeqCst) + 1;
        let prev_max = max_depth.load(Ordering::SeqCst);
        if current > prev_max {
            max_depth.store(current, Ordering::SeqCst);
        }

        if current < 3 {
            // 嵌套触发
            asm!("int $0x80" :: "N"(0x80));
        }

        depth.fetch_sub(1, Ordering::SeqCst);
    });

    asm!("int $0x80" :: "N"(0x80));

    kassert_eq!(max_depth.load(Ordering::SeqCst), 3, "嵌套深度应为 3");
    kassert_eq!(depth.load(Ordering::SeqCst), 0, "所有中断应已返回");
});
```

### 8.2 定时器中断测试

```rust
kernel_test!(test_timer_interrupt_accuracy, {
    let timer = ProgrammableIntervalTimer::new();
    let tick_count = AtomicUsize::new(0);

    timer.set_handler(|| {
        tick_count.fetch_add(1, Ordering::SeqCst);
    });

    // 设置 1ms 定时器
    timer.set_frequency(1000); // 1000 Hz = 1ms 间隔
    timer.enable();

    // 等待 100ms
    sleep_ms(100);

    timer.disable();

    let ticks = tick_count.load(Ordering::SeqCst);
    // 允许 +/- 10% 误差
    kassert!(ticks >= 90 && ticks <= 110,
        "定时器不准确: 期望 ~100 次, 实际 {} 次", ticks);
});
```

---

## 9. 崩溃测试

### 9.1 Panic 处理

```rust
kernel_test!(test_kernel_panic_recovery, {
    // 记录 panic 信息
    let panic_info = AtomicPtr::new(core::ptr::null_mut());

    // 触发内核 panic
    let result = catch_kernel_panic(|| {
        panic!("测试性内核崩溃");
    });

    kassert!(result.is_some(), "应捕获到内核 panic");
    let info = result.unwrap();
    kassert!(info.contains("测试性内核崩溃"), "panic 信息应包含消息");
});

kernel_test!(test_kernel_oops_recovery, {
    // 模拟内核 oops（非致命错误）
    let oops_handler = OopsHandler::new();
    oops_handler.set_action(OopsAction::Continue);

    // 触发 oops
    trigger_oops(OopsType::NullPointerDereference);

    // 验证系统继续运行
    kassert!(kernel_is_running(), "内核 oops 后应继续运行");

    // 验证 oops 被记录
    let log = oops_handler.get_log();
    kassert!(!log.is_empty(), "oops 应被记录到日志");
});
```

### 9.2 双重故障测试

```rust
kernel_test!(test_double_fault_handler, {
    let double_fault_count = AtomicUsize::new(0);

    // 注册双重故障处理程序
    idt.register_double_fault_handler(|| {
        double_fault_count.fetch_add(1, Ordering::SeqCst);
    });

    // 模拟双重故障（在处理页错误时再次触发异常）
    // 注意：此测试需要特殊的页表设置
    setup_double_fault_scenario();

    kassert_eq!(double_fault_count.load(Ordering::SeqCst), 1,
        "双重故障处理程序应被调用一次");
});
```

---

## 10. 性能基准测试

### 10.1 IPC 延迟基准

```rust
// benches/kernel/ipc_latency.rs
kernel_benchmark!(bench_ipc_zero_copy, {
    let service = create_test_service("bench-svc");
    let iterations = 10000;

    let start = read_tsc();
    for _ in 0..iterations {
        ipc_send_zero_copy(service, &[]);
    }
    let end = read_tsc();

    let total_ns = tsc_to_nanos(end - start);
    let avg_ns = total_ns / iterations;
    report_metric("ipc_zero_copy_latency_ns", avg_ns);
});

kernel_benchmark!(bench_ipc_4k_payload, {
    let service = create_test_service("bench-svc");
    let payload = vec![0u8; 4096];
    let iterations = 10000;

    let start = read_tsc();
    for _ in 0..iterations {
        ipc_send_sync(service, &IpcMessage::new("bench-svc", "echo", &payload));
    }
    let end = read_tsc();

    let total_ns = tsc_to_nanos(end - start);
    let avg_ns = total_ns / iterations;
    report_metric("ipc_4k_latency_ns", avg_ns);
});
```

### 10.2 上下文切换基准

```rust
kernel_benchmark!(bench_context_switch, {
    let iterations = 100000;

    let start = read_tsc();
    for _ in 0..iterations {
        scheduler.yield_current();
    }
    let end = read_tsc();

    let total_ns = tsc_to_nanos(end - start);
    let avg_ns = total_ns / iterations;
    report_metric("context_switch_ns", avg_ns);
});

kernel_benchmark!(bench_memory_allocation, {
    let mut allocator = FrameAllocator::new(65536);
    let iterations = 100000;

    let start = read_tsc();
    for _ in 0..iterations {
        if let Some(frame) = allocator.allocate() {
            allocator.free(frame);
        }
    }
    let end = read_tsc();

    let total_ns = tsc_to_nanos(end - start);
    let avg_ns = total_ns / iterations;
    report_metric("alloc_free_cycle_ns", avg_ns);
});
```

---

## 11. 内核模块测试用例清单

### 11.1 完整测试矩阵

| 模块 | 测试类别 | 用例数 | 优先级 |
|------|---------|--------|--------|
| **内存管理** | 页表映射/解除映射 | 15 | P0 |
| | 帧分配/释放 | 12 | P0 |
| | Slab 分配器 | 10 | P0 |
| | 内存分配压力 | 8 | P1 |
| | OOM 处理 | 5 | P1 |
| **调度器** | 进程创建/销毁 | 10 | P0 |
| | 优先级调度 | 12 | P0 |
| | 时间片轮转 | 8 | P0 |
| | 负载均衡 | 8 | P1 |
| | 优先级反转检测 | 6 | P0 |
| | 死锁检测 | 5 | P0 |
| **IPC** | 同步消息传递 | 15 | P0 |
| | 异步消息传递 | 10 | P0 |
| | 共享内存 | 8 | P1 |
| | 并发压力 | 10 | P0 |
| | 消息完整性 | 8 | P0 |
| **系统调用** | 参数验证 | 20 | P0 |
| | 权限检查 | 15 | P0 |
| | 缓冲区溢出防护 | 10 | P0 |
| | 错误码正确性 | 12 | P1 |
| **中断** | 处理程序注册 | 8 | P0 |
| | 嵌套中断 | 6 | P0 |
| | 定时器精度 | 5 | P1 |
| | 中断优先级 | 8 | P0 |
| **崩溃处理** | Panic 捕获 | 5 | P0 |
| | Oops 恢复 | 5 | P0 |
| | 双重故障 | 3 | P0 |
| | 崩溃转储 | 4 | P1 |

### 11.2 测试优先级定义

| 优先级 | 定义 | 要求 |
|--------|------|------|
| **P0** | 关键路径，失败阻断发布 | 每次提交必须通过 |
| **P1** | 重要功能，失败需要修复 | 每日构建必须通过 |
| **P2** | 边界情况，失败记录为缺陷 | 每周构建检查 |

---

## 附录 A: 内核测试命令参考

```bash
# 运行宿主机内核单元测试
cargo test -p omniagent-kernel --lib

# 运行 QEMU 内核集成测试
cargo test --test kernel-integration -- --nocapture

# 运行特定模块测试
cargo test -p omniagent-kernel --lib memory::tests

# 运行内核基准测试
cargo bench -p omniagent-kernel

# 生成内核覆盖率报告
cargo llvm-cov -p omniagent-kernel --html --output-dir target/kernel-coverage

# 运行内核模糊测试
cargo fuzz run -p omniagent-kernel syscall_fuzzer
```

## 附录 B: 常见测试失败排查

| 失败模式 | 可能原因 | 排查步骤 |
|---------|---------|---------|
| QEMU 启动超时 | 内核引导失败 | 检查串口输出的最后几行 |
| 测试结果丢失 | 串口缓冲区溢出 | 增大串口缓冲区或减少输出 |
| 不确定性行为 | 时序依赖 | 使用确定性调度器 |
| 内存泄漏 | 分配未释放 | 检查分配/释放计数 |
| 死锁 | 锁顺序错误 | 检查锁获取顺序和超时 |
