# OmniAgent OS 启动流程规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **模块归属**: 内核架构 / 启动子系统
> **状态**: 规范草案

---

## 1. 概述

### 1.1 目的

本文档定义 OmniAgent OS 从上电到 Aqua Shell 就绪的完整启动流程规范。OmniAgent OS 是一个基于微内核架构的 Agent 原生操作系统，全部内核代码使用 Rust 编写，强调安全性、并发性和可扩展性。启动流程涵盖 11 个关键阶段，从硬件初始化到用户态服务就绪，总启动时间预算不超过 3 秒。

### 1.2 适用范围

- x86_64 架构（UEFI 2.7+ / BIOS 兼容）
- 多核 SMP 环境（BSP + AP）
- 硬件虚拟化支持（Intel VT-x / AMD-V）
- 嵌入式设备至服务器平台

### 1.3 术语定义

| 术语 | 定义 |
|------|------|
| BSP | Bootstrap Processor，引导处理器 |
| AP | Application Processor，应用处理器 |
| GDT | Global Descriptor Table，全局描述符表 |
| IDT | Interrupt Descriptor Table，中断描述符表 |
| HPET | High Precision Event Timer，高精度事件定时器 |
| CFS | Completely Fair Scheduler，完全公平调度器 |
| INIT-SIPI-SIPI | 启动 AP 的三步握手协议 |
| VMXON | 进入 VMX 操作模式的指令 |
| Multiboot2 | 多引导协议第二版 |

---

## 2. 启动阶段总览

### 2.1 阶段列表

| 阶段 | 名称 | 时间预算 | 累计时间 | 关键操作 |
|------|------|----------|----------|----------|
| S0 | BIOS/UEFI 固件 | 500ms | 500ms | 硬件自检、固件初始化 |
| S1 | 引导加载器 | 200ms | 700ms | Multiboot2 加载内核映像 |
| S2 | kernel_main 入口 | 50ms | 750ms | 解析引导信息、进入 Rust 入口 |
| S3 | CPU 初始化 | 100ms | 850ms | GDT/IDT/分页/CR4 设置 |
| S4 | 物理内存检测 | 50ms | 900ms | 内存映射、可用区域标记 |
| S5 | 内核堆初始化 | 80ms | 980ms | bumpalo 启动堆 → slab 运行时堆 |
| S6 | 中断控制器 | 60ms | 1040ms | PIC 禁用 → APIC 初始化 |
| S7 | 定时器初始化 | 40ms | 1080ms | HPET 校准 → Local APIC Timer |
| S8 | 调度器就绪 | 100ms | 1180ms | CFS 初始化、就绪队列建立 |
| S9 | 首个用户进程 | 150ms | 1330ms | init 进程创建与切换 |
| S10 | 服务启动序列 | 1200ms | 2530ms | Agent Runtime、设备驱动、Aqua Shell |

### 2.2 启动时间预算表

```
总预算: <3000ms (3秒)
├── 固件阶段 (S0):     500ms  ████████████████████
├── 引导加载 (S1):     200ms  ██████████
├── 内核入口 (S2):      50ms  ███
├── CPU 初始化 (S3):   100ms  █████
├── 内存检测 (S4):      50ms  ███
├── 内核堆 (S5):        80ms  ████
├── 中断控制 (S6):      60ms  ███
├── 定时器 (S7):        40ms  ██
├── 调度器 (S8):       100ms  █████
├── 首进程 (S9):       150ms  ███████
└── 服务启动 (S10):   1200ms  ████████████████████████████████████████
    总计:             2530ms  (余量 470ms)
```

---

## 3. 启动状态机

### 3.1 状态定义

```rust
/// 启动阶段枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum BootStage {
    /// 固件阶段（BIOS/UEFI）
    Firmware = 0,
    /// 引导加载器阶段
    Bootloader = 1,
    /// 内核入口
    KernelEntry = 2,
    /// CPU 初始化
    CpuInit = 3,
    /// 物理内存检测
    MemoryDetect = 4,
    /// 内核堆初始化
    HeapInit = 5,
    /// 中断控制器初始化
    InterruptInit = 6,
    /// 定时器初始化
    TimerInit = 7,
    /// 调度器就绪
    SchedulerReady = 8,
    /// 首个用户进程
    FirstProcess = 9,
    /// 服务启动序列
    ServiceStartup = 10,
    /// 启动完成，系统就绪
    SystemReady = 11,
}

/// 启动状态
#[derive(Debug)]
pub struct BootState {
    pub current_stage: BootStage,
    pub stage_start_time: u64,       // TSC 周期数
    pub stage_elapsed_us: u64,       // 微秒
    pub total_elapsed_us: u64,       // 累计微秒
    pub error: Option<BootError>,    // 阶段错误
    pub recovery_attempted: bool,    // 是否已尝试恢复
    pub watchdog_deadline_us: u64,   // 看门狗截止时间
}
```

### 3.2 状态转换图

```
                    ┌─────────────┐
                    │  Firmware   │
                    │   (S0)      │
                    └──────┬──────┘
                           │ Multiboot2 info
                           ▼
                    ┌─────────────┐
                    │ Bootloader  │
                    │   (S1)      │
                    └──────┬──────┘
                           │ entry point
                           ▼
                    ┌─────────────┐
                    │ KernelEntry │◄──────────────────┐
                    │   (S2)      │                   │
                    └──────┬──────┘                   │
                           │                           │
              ┌────────────┼────────────┐              │
              ▼            ▼            ▼              │
       ┌──────────┐ ┌──────────┐ ┌──────────┐        │
       │ CpuInit  │ │MemDetect │ │ HeapInit │        │
       │  (S3)    │ │  (S4)    │ │  (S5)    │        │
       └────┬─────┘ └────┬─────┘ └────┬─────┘        │
            │            │            │               │
            └────────────┼────────────┘               │
                         ▼                            │
                  ┌─────────────┐                     │
                  │InterruptInit│                     │
                  │   (S6)      │                     │
                  └──────┬──────┘                     │
                         ▼                            │
                  ┌─────────────┐                     │
                  │  TimerInit  │                     │
                  │   (S7)      │                     │
                  └──────┬──────┘                     │
                         ▼                            │
                  ┌─────────────┐                     │
                  │SchedulerRdy │                     │
                  │   (S8)      │                     │
                  └──────┬──────┘                     │
                         ▼                            │
                  ┌─────────────┐                     │
                  │FirstProcess │                     │
                  │   (S9)      │                     │
                  └──────┬──────┘                     │
                         ▼                            │
                  ┌─────────────┐                     │
                  │ServiceStart │─── [超时/失败] ─────┘
                  │   (S10)     │     降级模式
                  └──────┬──────┘
                         ▼
                  ┌─────────────┐
                  │ SystemReady │
                  │ (Aqua Shell)│
                  └─────────────┘
```

### 3.3 状态转换实现

```rust
impl BootState {
    /// 推进到下一阶段
    pub fn advance_to(&mut self, next: BootStage) -> Result<(), BootError> {
        if next as u8 <= self.current_stage as u8 {
            return Err(BootError::InvalidStageTransition {
                from: self.current_stage,
                to: next,
            });
        }

        let now = tsc::read();
        self.stage_elapsed_us = tsc::cycles_to_us(now - self.stage_start_time);
        self.total_elapsed_us += self.stage_elapsed_us;

        // 检查看门狗
        if self.total_elapsed_us > self.watchdog_deadline_us {
            return Err(BootError::WatchdogTimeout {
                stage: self.current_stage,
                elapsed_us: self.total_elapsed_us,
                deadline_us: self.watchdog_deadline_us,
            });
        }

        self.current_stage = next;
        self.stage_start_time = now;
        boot_log::info("进入阶段: {:?} (累计: {}us)", next, self.total_elapsed_us);
        Ok(())
    }

    /// 记录阶段错误并尝试恢复
    pub fn handle_error(&mut self, error: BootError) -> BootRecoveryAction {
        self.error = Some(error.clone());
        boot_log::error("阶段 {:?} 错误: {:?}", self.current_stage, error);

        match &error {
            BootError::MemoryDetectFailed => {
                // 内存检测失败：使用保守默认值
                BootRecoveryAction::Fallback(FallbackMode::MinimalMemory)
            }
            BootError::ApicInitFailed => {
                // APIC 失败：回退到 PIC
                BootRecoveryAction::Fallback(FallbackMode::PicMode)
            }
            BootError::HpetNotFound => {
                // HPET 不可用：使用 PIT
                BootRecoveryAction::Fallback(FallbackMode::PitTimer)
            }
            BootError::WatchdogTimeout { .. } => {
                BootRecoveryAction::Reboot
            }
            _ => BootRecoveryAction::Halt,
        }
    }
}
```

---

## 4. 各阶段详细规范

### 4.1 S0: BIOS/UEFI 固件阶段

固件负责硬件初始化和自检（POST），最终将控制权交给引导加载器。

**UEFI 模式要求**:
- UEFI 2.7+ 固件
- GPT 分区表
- EFI System Partition (ESP) 包含引导加载器
- Secure Boot 支持通过 shim 签名绕过

**BIOS 兼容模式要求**:
- Legacy BIOS 或 CSM (Compatibility Support Module)
- MBR 分区表
- 引导扇区包含 stage1 引导代码

### 4.2 S1: 引导加载器阶段

OmniAgent OS 使用 Rust 编写的引导加载器，遵循 Multiboot2 协议。

```rust
/// Multiboot2 引导信息标签解析
#[repr(C, packed)]
pub struct MultibootInfo {
    pub total_size: u32,
    pub reserved: u32,
    // 后续为可变长度标签
}

/// 引导信息解析结果
#[derive(Debug)]
pub struct ParsedBootInfo {
    pub memory_map: Vec<MemoryRegion>,
    pub framebuffer: Option<FramebufferInfo>,
    pub rsdp: Option<u64>,           // RSDP 物理地址
    pub module_count: usize,
    pub modules: Vec<BootModule>,
    pub cmdline: String,
    pub elf_sections: Vec<ElfSection>,
}

/// 内存区域
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base_addr: u64,
    pub length: u64,
    pub region_type: MemoryRegionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    Unusable,
    BootloaderReclaimable,
    KernelAndModules,
}

/// 帧缓冲区信息
#[derive(Debug, Clone)]
pub struct FramebufferInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u8,
    pub memory_model: FramebufferMemoryModel,
}

/// 引导模块
#[derive(Debug, Clone)]
pub struct BootModule {
    pub start: u64,
    pub end: u64,
    pub cmdline: String,
}
```

### 4.3 S2: kernel_main 入口

```rust
/// 内核入口点 — 由引导加载器跳转至此
#[no_mangle]
pub unsafe extern "C" fn kernel_main(multiboot_info_addr: u64) -> ! {
    // 1. 立即设置栈保护
    stack_guard::init();

    // 2. 初始化早期日志（使用串口/VGA）
    early_log::init(LevelFilter::Info);

    // 3. 解析 Multiboot2 引导信息
    let boot_info = match multiboot2::parse(multiboot_info_addr) {
        Ok(info) => {
            early_log::info!("Multiboot2 信息解析成功");
            info
        }
        Err(e) => {
            early_log::error!("Multiboot2 解析失败: {:?}, 使用保守默认值", e);
            boot_info::default_conservative()
        }
    };

    // 4. 初始化启动状态机
    let mut boot_state = BootState::new(BootStage::KernelEntry);
    boot_state.watchdog_deadline_us = 3_000_000; // 3秒看门狗

    // 5. 调用主初始化序列
    match init_sequence(&mut boot_state, &boot_info) {
        Ok(()) => {
            boot_state.advance_to(BootStage::SystemReady).unwrap();
            early_log::info!("系统启动完成, 耗时: {}us", boot_state.total_elapsed_us);
            // 跳转到用户态
            user_mode::enter();
        }
        Err(e) => {
            boot_log::critical!("启动失败: {:?}", e);
            boot_state.handle_error(e);
            halt_with_error();
        }
    }

    unreachable!()
}

/// 主初始化序列
fn init_sequence(
    boot_state: &mut BootState,
    boot_info: &ParsedBootInfo,
) -> Result<(), BootError> {
    // S3: CPU 初始化
    boot_state.advance_to(BootStage::CpuInit)?;
    cpu::init(boot_info)?;

    // S4: 物理内存检测
    boot_state.advance_to(BootStage::MemoryDetect)?;
    memory::detect(boot_info)?;

    // S5: 内核堆初始化
    boot_state.advance_to(BootStage::HeapInit)?;
    heap::init()?;

    // S6: 中断控制器
    boot_state.advance_to(BootStage::InterruptInit)?;
    interrupt::init()?;

    // S7: 定时器
    boot_state.advance_to(BootStage::TimerInit)?;
    timer::init()?;

    // S8: 调度器
    boot_state.advance_to(BootStage::SchedulerReady)?;
    scheduler::init()?;

    // S9: 首个用户进程
    boot_state.advance_to(BootStage::FirstProcess)?;
    process::spawn_init()?;

    // S10: 服务启动序列
    boot_state.advance_to(BootStage::ServiceStartup)?;
    services::start_all()?;

    Ok(())
}
```

### 4.4 S3: CPU 初始化

```rust
pub mod cpu {
    use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
    use x86_64::structures::idt::InterruptDescriptorTable;
    use x86_64::registers::control::{Cr4, Cr4Flags};
    use x86_64::registers::model_specific::Efer;
    use x86_64::instructions::tlb;
    use x86_64::VirtAddr;

    /// GDT 初始化
    pub fn init_gdt() -> (GlobalDescriptorTable, SegmentSelector) {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let user_code = gdt.add_entry(Descriptor::user_code_segment());
        let user_data = gdt.add_entry(Descriptor::user_data_segment());

        // TSS 段
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));

        gdt.load();
        (gdt, code_selector)
    }

    /// IDT 初始化
    pub fn init_idt(idt: &mut InterruptDescriptorTable) {
        // 设置异常处理
        idt.divide_error.set_handler_fn(exceptions::divide_error);
        idt.debug.set_handler_fn(exceptions::debug);
        idt.page_fault.set_handler_fn(exceptions::page_fault_handler);
        idt.double_fault.set_handler_fn(exceptions::double_fault)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

        // 设置 IRQ 处理（在 APIC 初始化后填充）
        idt[0x20].set_handler_fn(irq::timer_handler);
        idt[0x21].set_handler_fn(irq::keyboard_handler);

        idt.load();
    }

    /// CR4 寄存器初始化
    pub fn init_cr4() {
        let mut cr4 = Cr4::read();

        // 启用物理地址扩展
        cr4 |= Cr4Flags::PHYSICAL_ADDRESS_EXTENSION;

        // 启用 SMEP（禁止用户态执行内核页）
        cr4 |= Cr4Flags::SMEP_ENABLE;

        // 启用 SMAP（禁止内核访问用户页）
        cr4 |= Cr4Flags::SMAP_ENABLE;

        // 启用 PCID（进程上下文标识符）
        cr4 |= Cr4Flags::PCID_ENABLE;

        // 启用 OSFXSR 和 OSXMMEXCPT
        cr4 |= Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT;

        // 检查并启用 VMXE（虚拟化扩展）
        if cpu_feature::has_vmx() {
            cr4 |= Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS;
        }

        unsafe { Cr4::write(cr4); }
    }

    /// 完整 CPU 初始化
    pub fn init(boot_info: &ParsedBootInfo) -> Result<(), BootError> {
        init_gdt();
        init_idt(&mut IDT.lock());
        init_cr4();

        // 设置 EFER.LME（长模式使能）
        unsafe {
            Efer::update(|efer| {
                efer.set(EferFlags::LONG_MODE_ENABLE);
                efer.set(EferFlags::NO_EXECUTE_ENABLE);
            });
        }

        // 刷新 TLB
        tlb::flush_all();

        // 启用 SSE
        x86_64::instructions::enable_sse();

        boot_log::info!("CPU 初始化完成");
        Ok(())
    }
}
```

### 4.5 S4: 物理内存检测

```rust
pub mod memory {
    use crate::boot::boot_info::{MemoryRegion, MemoryRegionType};

    /// 物理内存管理器
    pub struct PhysicalMemoryManager {
        regions: Vec<MemoryRegion>,
        total_usable: u64,
        total_reserved: u64,
    }

    impl PhysicalMemoryManager {
        pub fn detect(boot_info: &ParsedBootInfo) -> Result<Self, BootError> {
            let mut pmem = Self {
                regions: Vec::new(),
                total_usable: 0,
                total_reserved: 0,
            };

            for region in &boot_info.memory_map {
                pmem.regions.push(*region);
                match region.region_type {
                    MemoryRegionType::Usable => pmem.total_usable += region.length,
                    _ => pmem.total_reserved += region.length,
                }
            }

            if pmem.total_usable < 64 * 1024 * 1024 {
                return Err(BootError::InsufficientMemory {
                    available: pmem.total_usable,
                    required: 64 * 1024 * 1024,
                });
            }

            boot_log::info!(
                "物理内存: 可用 {}MB, 保留 {}MB, 共 {} 个区域",
                pmem.total_usable / (1024 * 1024),
                pmem.total_reserved / (1024 * 1024),
                pmem.regions.len()
            );

            Ok(pmem)
        }
    }
}
```

### 4.6 S5: 内核堆初始化

```rust
pub mod heap {
    use alloc::alloc::{GlobalAlloc, Layout};
    use core::sync::atomic::{AtomicBool, Ordering};
    use bumpalo::Bump;

    /// 启动阶段堆分配器（bumpalo）
    pub struct BootHeap {
        bump: Bump,
        heap_start: usize,
        heap_size: usize,
    }

    impl BootHeap {
        pub fn new(start: usize, size: usize) -> Self {
            // 安全地将内存区域转换为 bumpalo 堆
            let bump = unsafe { Bump::from_raw_parts(start as *mut u8, size) };
            Self {
                bump,
                heap_start: start,
                heap_size: size,
            }
        }

        pub fn used(&self) -> usize {
            self.bump.allocated_bytes()
        }
    }

    /// 运行时 slab 分配器
    pub struct SlabAllocator {
        slabs: [SlabCache; 8],  // 8/16/32/64/128/256/512/1024 字节
        large_alloc: BTreeMap<usize, LargeBlock>,
        fallback_heap: Option<BootHeap>,
    }

    /// 堆初始化：从 bumpalo 过渡到 slab
    pub fn init() -> Result<(), BootError> {
        // 阶段1: 使用 bumpalo 分配初始堆
        let boot_heap = BootHeap::new(KERNEL_HEAP_START, BOOT_HEAP_SIZE);

        // 阶段2: 使用 bumpalo 分配 slab 分配器结构
        let slab = unsafe {
            let layout = Layout::new::<SlabAllocator>();
            let ptr = boot_heap.bump.alloc_layout(layout);
            ptr.as_ptr().write(SlabAllocator::new());
            ptr.as_ptr().read()
        };

        // 阶段3: 切换全局分配器
        GLOBAL_ALLOCATOR.switch_to_slab(slab);

        boot_log::info!(
            "内核堆初始化: bumpalo 使用 {}KB, 切换至 slab",
            boot_heap.used() / 1024
        );
        Ok(())
    }
}
```

### 4.7 S6: 中断控制器初始化

```rust
pub mod interrupt {
    use x86_64::structures::idt::InterruptDescriptorTable;

    /// 中断控制器抽象
    pub trait InterruptController {
        fn init(&mut self) -> Result<(), BootError>;
        fn enable_irq(&mut self, irq: u8);
        fn disable_irq(&mut self, irq: u8);
        fn send_eoi(&mut self, irq: u8);
        fn is_spurious(&self, irq: u8) -> bool;
    }

    /// APIC 中断控制器
    pub struct ApicController {
        local_apic: LocalApic,
        io_apic: IoApic,
        isa_irq_map: [u8; 16],  // ISA IRQ 到 GSI 的映射
    }

    impl InterruptController for ApicController {
        fn init(&mut self) -> Result<(), BootError> {
            // 1. 禁用 PIC（掩码所有中断）
            unsafe {
                Port::new(0x21).write(0xFF); // 主 PIC
                Port::new(0xA1).write(0xFF); // 从 PIC
            }

            // 2. 初始化 Local APIC
            self.local_apic.enable();

            // 3. 初始化 I/O APIC
            self.io_apic.init(&self.isa_irq_map);

            // 4. 设置中断向量偏移（0x20 开始）
            self.io_apic.set_irq(0, 0x20);  // Timer -> IRQ 0x20
            self.io_apic.set_irq(1, 0x21);  // Keyboard -> IRQ 0x21

            boot_log::info!("APIC 中断控制器初始化完成");
            Ok(())
        }

        fn enable_irq(&mut self, irq: u8) {
            self.io_apic.unmask(irq);
        }

        fn disable_irq(&mut self, irq: u8) {
            self.io_apic.mask(irq);
        }

        fn send_eoi(&mut self, _irq: u8) {
            self.local_apic.send_eoi();
        }

        fn is_spurious(&self, irq: u8) -> bool {
            self.local_apic.is_spurious_interrupt(irq)
        }
    }
}
```

### 4.8 S7: 定时器初始化

```rust
pub mod timer {
    /// 定时器抽象
    pub trait Timer {
        fn init(&mut self) -> Result<(), BootError>;
        fn frequency(&self) -> u64;
        fn current_ticks(&self) -> u64;
        fn set_periodic(&mut self, interval_us: u64);
        fn set_oneshot(&mut self, deadline_us: u64);
    }

    /// HPET 定时器（高精度）
    pub struct HpetTimer {
        base_addr: VirtAddr,
        period_fs: u64,  // 飞秒/周期
    }

    impl Timer for HpetTimer {
        fn init(&mut self) -> Result<(), BootError> {
            // 从 ACPI HPET 表获取基地址
            let hpet_table = acpi::find_table::<HpetTable>("HPET")
                .ok_or(BootError::HpetNotFound)?;

            self.base_addr = VirtAddr::new(hpet_table.base_address as u64);
            self.period_fs = hpet_table.period_femtoseconds;

            // 启用 HPET
            unsafe {
                let general = self.base_addr.as_ptr::<u64>();
                core::ptr::write_volatile(general, 1); // ENABLE_BIT
            }

            boot_log::info!(
                "HPET 初始化: 频率 {}MHz",
                1_000_000_000_000 / self.period_fs / 1_000_000
            );
            Ok(())
        }

        fn frequency(&self) -> u64 {
            1_000_000_000_000 / self.period_fs
        }
    }

    /// Local APIC Timer（用于调度器节拍）
    pub struct LapicTimer {
        calibrated_ticks_per_us: u32,
    }

    impl LapicTimer {
        /// 使用 HPET 校准 Local APIC Timer
        pub fn calibrate(hpet: &HpetTimer) -> Result<Self, BootError> {
            const CALIBRATION_US: u64 = 10_000; // 10ms 校准窗口

            let start_hpet = hpet.current_ticks();
            let start_apic = lapic::read_timer_count();

            // 等待校准窗口
            while hpet.current_ticks() - start_hpet < hpet.frequency() / 1_000_000 * CALIBRATION_US {
                core::hint::spin_loop();
            }

            let elapsed_apic = start_apic - lapic::read_timer_count();
            let ticks_per_us = elapsed_apic as u64 / CALIBRATION_US;

            boot_log::info!("Local APIC Timer 校准: {} ticks/us", ticks_per_us);
            Ok(Self {
                calibrated_ticks_per_us: ticks_per_us as u32,
            })
        }
    }
}
```

### 4.9 S8: 调度器初始化

```rust
pub mod scheduler {
    use crate::process::ProcessId;

    /// CFS 调度器
    pub struct CfsScheduler {
        runqueues: [RunQueue; MAX_CPUS],
        min_granularity_ns: u64,
        target_latency_ns: u64,
        nr_running: AtomicUsize,
    }

    /// 就绪队列（红黑树 + 双向链表）
    pub struct RunQueue {
        rb_tree: RedBlackTree<SchedEntity>,
        nr_running: usize,
        cpu_load: u64,
        curr: Option<ProcessId>,
    }

    /// 调度实体
    pub struct SchedEntity {
        pub pid: ProcessId,
        pub vruntime: u64,          // 虚拟运行时间
        pub weight: u64,            // 优先级权重
        pub exec_start: u64,        // 开始执行时间
        pub sum_exec_runtime: u64,  // 累计执行时间
    }

    impl CfsScheduler {
        pub fn init() -> Result<(), BootError> {
            let scheduler = CfsScheduler {
                runqueues: core::array::from_fn(|_| RunQueue::new()),
                min_granularity_ns: 1_000_000,   // 1ms
                target_latency_ns: 6_000_000,    // 6ms
                nr_running: AtomicUsize::new(0),
            };

            GLOBAL_SCHEDULER.init(scheduler);

            // 启动调度器节拍中断
            timer::set_periodic(SCHED_TICK_INTERVAL_US);

            boot_log::info!("CFS 调度器初始化完成");
            Ok(())
        }

        /// 选择下一个要运行的进程
        pub fn pick_next_task(&mut self, cpu_id: usize) -> Option<ProcessId> {
            let rq = &mut self.runqueues[cpu_id];
            // 选择 vruntime 最小的进程
            rq.rb_tree.min_by_key(|e| e.vruntime).map(|e| e.pid)
        }
    }
}
```

### 4.10 S9: 首个用户进程

```rust
pub mod process {
    /// init 进程创建
    pub fn spawn_init() -> Result<(), BootError> {
        // 1. 从 initramfs 加载 init ELF
        let init_elf = initramfs::load("/sbin/init")
            .map_err(|_| BootError::InitBinaryNotFound)?;

        // 2. 创建进程控制块
        let pcb = ProcessControlBlock::new(
            ProcessId::new(1),
            "init".into(),
            ProcessPriority::High,
            Credentials::root(),
        );

        // 3. 设置用户态页表
        let page_table = PageTable::new_user();
        page_table.map_elf(&init_elf)?;
        page_table.map_stack(USER_STACK_TOP, USER_STACK_SIZE)?;

        // 4. 设置用户态寄存器
        let context = UserContext {
            rip: init_elf.entry_point(),
            rsp: USER_STACK_TOP,
            rflags: RFlags::INTERRUPT_FLAG,
            cs: user_code_selector(),
            ss: user_data_selector(),
        };

        // 5. 注册进程
        PROCESS_TABLE.insert(pcb);

        // 6. 切换到用户态
        unsafe {
            context.switch_to_user();
        }

        Ok(())
    }
}
```

### 4.11 S10: 服务启动序列

```rust
pub mod services {
    /// 服务定义
    pub struct ServiceDescriptor {
        pub name: &'static str,
        pub binary_path: &'static str,
        pub dependencies: &'static [&'static str],
        pub priority: ServicePriority,
        pub timeout_ms: u64,
        pub critical: bool,  // 是否为关键服务
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ServicePriority {
        Critical = 0,   // Agent Runtime
        High = 1,       // 设备管理器
        Normal = 2,     // 网络栈
        Low = 3,        // 用户服务
        Shell = 4,      // Aqua Shell
    }

    /// 服务启动顺序
    static SERVICE_TABLE: &[ServiceDescriptor] = &[
        ServiceDescriptor {
            name: "agent-runtime",
            binary_path: "/sbin/agent-runtime",
            dependencies: &[],
            priority: ServicePriority::Critical,
            timeout_ms: 500,
            critical: true,
        },
        ServiceDescriptor {
            name: "device-manager",
            binary_path: "/sbin/devmgr",
            dependencies: &["agent-runtime"],
            priority: ServicePriority::High,
            timeout_ms: 300,
            critical: true,
        },
        ServiceDescriptor {
            name: "network-stack",
            binary_path: "/sbin/netstack",
            dependencies: &["device-manager"],
            priority: ServicePriority::Normal,
            timeout_ms: 200,
            critical: false,
        },
        ServiceDescriptor {
            name: "aqua-shell",
            binary_path: "/bin/aqua-shell",
            dependencies: &["agent-runtime", "device-manager"],
            priority: ServicePriority::Shell,
            timeout_ms: 200,
            critical: false,
        },
    ];

    pub fn start_all() -> Result<(), BootError> {
        // 拓扑排序确保依赖顺序
        let ordered = topological_sort(SERVICE_TABLE);

        for service in &ordered {
            boot_log::info!("启动服务: {}", service.name);

            let result = start_service_with_timeout(service);

            match result {
                Ok(()) => boot_log::info!("服务 {} 就绪", service.name),
                Err(ServiceError::Timeout) if !service.critical => {
                    boot_log::warn!("服务 {} 启动超时（非关键）", service.name);
                }
                Err(e) if service.critical => {
                    return Err(BootError::CriticalServiceFailed {
                        service: service.name,
                        error: e,
                    });
                }
                Err(e) => {
                    boot_log::warn!("服务 {} 启动失败: {:?}", service.name, e);
                }
            }
        }

        Ok(())
    }
}
```

---

## 5. 多核 SMP 启动

### 5.1 BSP → AP 启动流程

```rust
pub mod smp {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static AP_READY_COUNT: AtomicUsize = AtomicUsize::new(0);
    static AP_START_FLAG: AtomicBool = AtomicBool::new(false);

    /// AP 启动入口（运行在 AP 上）
    pub extern "C" fn ap_entry() -> ! {
        let cpu_id = CURRENT_CPU_ID.get();

        // 1. 设置 AP 栈
        stack::init_ap_stack(cpu_id);

        // 2. 初始化 GDT/IDT（每个 CPU 独立）
        cpu::init_gdt();
        cpu::init_idt(&mut IDT);

        // 3. 初始化 Local APIC
        lapic::init();

        // 4. 初始化 AP 定时器
        timer::init_ap_timer();

        // 5. 标记就绪
        AP_READY_COUNT.fetch_add(1, Ordering::SeqCst);

        // 6. 等待启动信号
        while !AP_START_FLAG.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }

        // 7. 进入空闲循环，等待调度器分配任务
        scheduler::idle_loop();

        unreachable!()
    }

    /// BSP 发起 AP 启动（INIT-SIPI-SIPI 协议）
    pub fn start_application_processors(acpi_info: &AcpiInfo) -> Result<(), BootError> {
        let ap_count = acpi_info.cpu_count() - 1; // 减去 BSP

        if ap_count == 0 {
            boot_log::info!("未检测到应用处理器");
            return Ok(());
        }

        boot_log::info!("启动 {} 个应用处理器...", ap_count);

        for (i, apic_id) in acpi_info.apic_ids().iter().enumerate().skip(1) {
            // 分配 AP 启动栈
            let stack_top = AP_STACK_BASE + (i * AP_STACK_SIZE);
            AP_STACKS[i].store(stack_top, Ordering::SeqCst);

            // 发送 INIT IPI
            lapic::send_init_ipi(*apic_id);
            delay_us(10_000); // 等待 10ms

            // 发送第一次 SIPI
            lapic::send_sipi(*apic_id, AP_START_PFN);
            delay_us(200); // 等待 200us

            // 发送第二次 SIPI
            lapic::send_sipi(*apic_id, AP_START_PFN);
            delay_us(200);
        }

        // 等待所有 AP 就绪（超时 1 秒）
        let deadline = tsc::cycles_to_us(tsc::read()) + 1_000_000;
        while AP_READY_COUNT.load(Ordering::SeqCst) < ap_count {
            if tsc::cycles_to_us(tsc::read()) > deadline {
                return Err(BootError::ApStartupTimeout {
                    ready: AP_READY_COUNT.load(Ordering::SeqCst),
                    expected: ap_count,
                });
            }
        }

        // 广播启动信号
        AP_START_FLAG.store(true, Ordering::SeqCst);

        boot_log::info!("所有 {} 个 AP 启动完成", ap_count);
        Ok(())
    }
}
```

---

## 6. 虚拟化模式进入

### 6.1 VMXON 进入流程

```rust
pub mod virt {
    use x86_64::registers::control::{Cr4, Cr4Flags};
    use x86_64::registers::model_specific::{Msr, Efer, EferFlags};

    /// VMX 进入条件检查
    pub fn check_vmx_support() -> Result<VmxCapabilities, BootError> {
        let cpuid = CpuId::new();
        let feature_info = cpuid.get_feature_info()
            .ok_or(BootError::VmxNotSupported)?;

        if !feature_info.has_vmx() {
            return Err(BootError::VmxNotSupported);
        }

        // 检查 IA32_FEATURE_CONTROL MSR
        let feature_ctrl = unsafe { Msr::new(0x3A).read() };
        if (feature_ctrl & 0x5) != 0x5 {
            // 尝试解锁 VMX
            unsafe {
                Msr::new(0x3A).write(feature_ctrl | 0x5);
            }
        }

        let vmx_basic = unsafe { Msr::new(0x480).read() };
        Ok(VmxCapabilities {
            revision_id: vmx_basic as u32,
            vmcs_size: ((vmx_basic >> 32) & 0x1FFF) as u16,
            memory_type: ((vmx_basic >> 50) & 0xF) as u8,
        })
    }

    /// 进入 VMX 操作模式
    pub fn enter_vmx_mode() -> Result<VmxRegion, BootError> {
        let caps = check_vmx_support()?;

        // 1. 启用 CR4.VMXE
        unsafe {
            Cr4::update(|cr4| cr4 |= Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS);
        }

        // 2. 分配 VMXON 区域（必须 4KB 对齐）
        let vmxon_region = VmxRegion::alloc_aligned(4096)?;
        vmxon_region.set_revision_id(caps.revision_id);

        // 3. 执行 VMXON
        let result = unsafe { vmxon(vmx_region.phys_addr()) };
        if result.is_err() {
            // 回滚 CR4.VMXE
            unsafe {
                Cr4::update(|cr4| cr4 &= !Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS);
            }
            return Err(BootError::VmxonFailed { status: result.err() });
        }

        boot_log::info!("VMX 模式已启用 (revision: {})", caps.revision_id);
        Ok(vmxon_region)
    }
}
```

---

## 7. 启动失败恢复

### 7.1 看门狗机制

```rust
/// 启动看门狗
pub struct BootWatchdog {
    deadline_us: u64,
    stage_deadlines: [u64; 12],  // 每阶段截止时间
    current_stage: BootStage,
}

impl BootWatchdog {
    pub fn new(total_budget_us: u64) -> Self {
        Self {
            deadline_us: total_budget_us,
            stage_deadlines: [
                500_000,   // S0
                700_000,   // S1
                750_000,   // S2
                850_000,   // S3
                900_000,   // S4
                980_000,   // S5
                1_040_000, // S6
                1_080_000, // S7
                1_180_000, // S8
                1_330_000, // S9
                2_530_000, // S10
                3_000_000, // SystemReady
            ],
            current_stage: BootStage::Firmware,
        }
    }

    /// 检查当前阶段是否超时
    pub fn check(&self, elapsed_us: u64) -> WatchdogStatus {
        let stage_idx = self.current_stage as usize;
        if elapsed_us > self.stage_deadlines[stage_idx] {
            WatchdogStatus::StageTimeout {
                stage: self.current_stage,
                elapsed_us,
                budget_us: self.stage_deadlines[stage_idx],
            }
        } else if elapsed_us > self.deadline_us {
            WatchdogStatus::TotalTimeout { elapsed_us }
        } else {
            WatchdogStatus::Ok
        }
    }
}
```

### 7.2 降级模式

```rust
#[derive(Debug, Clone, Copy)]
pub enum FallbackMode {
    /// 最小内存模式（64MB 限制）
    MinimalMemory,
    /// PIC 中断模式（APIC 不可用时）
    PicMode,
    /// PIT 定时器模式（HPET 不可用时）
    PitTimer,
    /// 单核模式（SMP 启动失败时）
    UniProcessor,
    /// 安全模式（禁用虚拟化）
    NoVirtualization,
    /// 紧急恢复模式（仅串口输出）
    RecoveryMode,
}

impl FallbackMode {
    /// 降级模式的功能限制
    pub fn limitations(&self) -> &[&str] {
        match self {
            FallbackMode::MinimalMemory => &[
                "最大并发 Agent 数量降至 10",
                "禁用内存映射文件",
                "限制内核堆为 16MB",
            ],
            FallbackMode::PicMode => &[
                "中断延迟增加约 1us",
                "不支持中断亲和性",
                "最大 15 条 IRQ 线路",
            ],
            FallbackMode::PitTimer => &[
                "定时器精度降至 ~1ms",
                "不支持高精度调度",
            ],
            FallbackMode::UniProcessor => &[
                "仅使用 BSP 核心",
                "吞吐量降低",
            ],
            FallbackMode::NoVirtualization => &[
                "无法运行 VM 客户机",
                "Agent 沙箱使用软件隔离",
            ],
            FallbackMode::RecoveryMode => &[
                "仅串口控制台",
                "最小功能集",
                "自动转储诊断信息",
            ],
        }
    }
}
```

---

## 8. 启动日志与诊断

### 8.1 日志系统

```rust
/// 启动日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BootLogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

/// 启动日志条目
#[derive(Debug, Clone)]
pub struct BootLogEntry {
    pub timestamp_us: u64,
    pub stage: BootStage,
    pub level: BootLogLevel,
    pub message: String,
    pub cpu_id: u8,
}

/// 启动诊断报告
#[derive(Debug)]
pub struct BootDiagnostics {
    pub total_time_us: u64,
    pub stage_times: [(BootStage, u64); 12],
    pub warnings: Vec<BootLogEntry>,
    pub errors: Vec<BootLogEntry>,
    pub fallback_modes: Vec<FallbackMode>,
    pub memory_summary: MemorySummary,
    pub cpu_summary: CpuSummary,
    pub services_started: Vec<String>,
    pub services_failed: Vec<(String, ServiceError)>,
}

impl BootDiagnostics {
    /// 生成诊断报告
    pub fn generate() -> Self {
        let logs = boot_log::drain();
        Self {
            total_time_us: tsc::cycles_to_us(tsc::read() - BOOT_START_TSC),
            stage_times: extract_stage_times(&logs),
            warnings: logs.iter().filter(|l| l.level == BootLogLevel::Warn).cloned().collect(),
            errors: logs.iter().filter(|l| l.level == BootLogLevel::Error).cloned().collect(),
            fallback_modes: ACTIVE_FALLBACKS.lock().clone(),
            memory_summary: memory::summary(),
            cpu_summary: cpu::summary(),
            services_started: services::started_list(),
            services_failed: services::failed_list(),
        }
    }

    /// 格式化输出诊断报告
    pub fn format_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== OmniAgent OS 启动诊断报告 ===\n\n");

        report.push_str(&format!("总启动时间: {}.{:03}ms\n\n",
            self.total_time_us / 1000,
            self.total_time_us % 1000
        ));

        report.push_str("各阶段耗时:\n");
        for (stage, time_us) in &self.stage_times {
            report.push_str(&format!("  {:?}: {}.{:03}ms\n",
                stage, time_us / 1000, time_us % 1000
            ));
        }

        if !self.fallback_modes.is_empty() {
            report.push_str("\n降级模式:\n");
            for mode in &self.fallback_modes {
                report.push_str(&format!("  - {:?}\n", mode));
            }
        }

        report
    }
}
```

---

## 9. 错误处理

### 9.1 启动错误类型

```rust
#[derive(Debug, Clone)]
pub enum BootError {
    InvalidMultiboot2 { reason: String },
    InvalidStageTransition { from: BootStage, to: BootStage },
    InsufficientMemory { available: u64, required: u64 },
    MemoryDetectFailed,
    HeapInitFailed { reason: String },
    ApicInitFailed,
    HpetNotFound,
    PicModeOnly,
    WatchdogTimeout { stage: BootStage, elapsed_us: u64, deadline_us: u64 },
    ApStartupTimeout { ready: usize, expected: usize },
    VmxNotSupported,
    VmxonFailed { status: Option<u64> },
    InitBinaryNotFound,
    CriticalServiceFailed { service: &'static str, error: ServiceError },
    SmpNotSupported,
}

#[derive(Debug)]
pub enum BootRecoveryAction {
    /// 使用降级模式继续
    Fallback(FallbackMode),
    /// 重试当前阶段
    Retry,
    /// 重启系统
    Reboot,
    /// 停止并显示错误
    Halt,
}
```

### 9.2 错误码表

| 错误码 | 名称 | 严重性 | 可恢复 |
|--------|------|--------|--------|
| E001 | InvalidMultiboot2 | 致命 | 否 |
| E002 | InvalidStageTransition | 致命 | 否 |
| E003 | InsufficientMemory | 致命 | 否 |
| E004 | MemoryDetectFailed | 高 | 是（降级） |
| E005 | HeapInitFailed | 致命 | 否 |
| E006 | ApicInitFailed | 高 | 是（PIC） |
| E007 | HpetNotFound | 低 | 是（PIT） |
| E008 | WatchdogTimeout | 致命 | 是（重启） |
| E009 | ApStartupTimeout | 中 | 是（单核） |
| E010 | VmxNotSupported | 低 | 是（禁用） |
| E011 | VmxonFailed | 中 | 是（禁用） |
| E012 | InitBinaryNotFound | 致命 | 否 |
| E013 | CriticalServiceFailed | 致命 | 是（重启） |

---

## 10. 安全考量

### 10.1 启动安全措施

| 措施 | 描述 | 阶段 |
|------|------|------|
| Secure Boot | UEFI 固件验证引导加载器签名 | S0-S1 |
| 内核映像校验 | SHA-256 哈希验证内核完整性 | S2 |
| NX 位 | 禁止数据段执行 | S3 |
| SMEP/SMAP | 隔离内核与用户态内存访问 | S3 |
| 栈保护 | canary 值检测栈溢出 | S2 |
| KASLR | 内核地址空间布局随机化 | S2 |
| 内存清零 | 内核堆初始化前清零 | S5 |

### 10.2 安全启动链

```
UEFI Secure Boot
    │ (验证 shim 签名)
    ▼
shim (MOK 密钥)
    │ (验证引导加载器签名)
    ▼
OmniAgent Bootloader
    │ (验证内核 SHA-256)
    ▼
kernel_main
    │ (NX + SMEP + SMAP + KASLR)
    ▼
init 进程
    │ (验证 initramfs 签名)
    ▼
Agent Runtime + Services
```

---

## 11. 性能约束

### 11.1 启动性能目标

| 指标 | 目标值 | 测量方法 |
|------|--------|----------|
| 总启动时间 | < 3s | TSC 计数器 |
| 内核初始化 (S2-S8) | < 500ms | 阶段计时 |
| 用户态就绪 (S9) | < 150ms | 进程创建计时 |
| 服务启动 (S10) | < 1.5s | 服务就绪事件 |
| AP 启动延迟 | < 100ms/AP | AP 就绪计数 |
| 内存初始化 | < 80ms | 堆分配计时 |

### 11.2 性能优化策略

1. **并行初始化**: S3-S5 阶段在 BSP 上串行执行，S6-S7 可与 AP 启动并行
2. **延迟加载**: 非关键驱动和服务延迟到 Aqua Shell 就绪后加载
3. **预计算表**: GDT/IDT 在编译期生成静态数据
4. **缓存友好**: 启动代码放置在 L1 缓存范围内

---

## 12. 测试用例

### 12.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_stage_ordering() {
        assert!(BootStage::Firmware < BootStage::Bootloader);
        assert!(BootStage::Bootloader < BootStage::KernelEntry);
        assert!(BootStage::ServiceStartup < BootStage::SystemReady);
    }

    #[test]
    fn test_boot_state_advance() {
        let mut state = BootState::new(BootStage::KernelEntry);
        assert!(state.advance_to(BootStage::CpuInit).is_ok());
        assert!(state.advance_to(BootStage::MemoryDetect).is_ok());
        // 不允许回退
        assert!(state.advance_to(BootStage::Bootloader).is_err());
    }

    #[test]
    fn test_boot_state_invalid_transition() {
        let mut state = BootState::new(BootStage::CpuInit);
        let result = state.advance_to(BootStage::CpuInit);
        assert!(matches!(result, Err(BootError::InvalidStageTransition { .. })));
    }

    #[test]
    fn test_watchdog_timeout_detection() {
        let watchdog = BootWatchdog::new(3_000_000);
        assert!(matches!(watchdog.check(1_000_000), WatchdogStatus::Ok));
        assert!(matches!(watchdog.check(4_000_000), WatchdogStatus::TotalTimeout { .. }));
    }

    #[test]
    fn test_memory_region_parsing() {
        let regions = vec![
            MemoryRegion { base_addr: 0, length: 0x9F000, region_type: MemoryRegionType::Usable },
            MemoryRegion { base_addr: 0x100000, length: 0x7EF0000, region_type: MemoryRegionType::Usable },
        ];
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn test_service_dependency_order() {
        let ordered = topological_sort(SERVICE_TABLE);
        // agent-runtime 必须在 device-manager 之前
        let rt_pos = ordered.iter().position(|s| s.name == "agent-runtime").unwrap();
        let dm_pos = ordered.iter().position(|s| s.name == "device-manager").unwrap();
        assert!(rt_pos < dm_pos);
    }

    #[test]
    fn test_fallback_mode_limitations() {
        let mode = FallbackMode::MinimalMemory;
        assert!(!mode.limitations().is_empty());
    }
}
```

### 12.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_full_boot_sequence() {
        // 在 QEMU 中模拟完整启动流程
        let result = qemu::boot_test("target/x86_64-omniagent/debug/kernel.bin");
        assert!(result.exit_code().is_none()); // 内核不应退出
        assert!(result.serial_output().contains("系统启动完成"));
        assert!(result.boot_time_ms() < 3000);
    }

    #[test]
    fn test_boot_with_minimal_memory() {
        // 64MB 内存启动测试
        let result = qemu::boot_test_with_memory("kernel.bin", 64);
        assert!(result.serial_output().contains("降级模式"));
        assert!(result.serial_output().contains("MinimalMemory"));
    }

    #[test]
    fn test_boot_without_apic() {
        // 禁用 APIC 启动测试
        let result = qemu::boot_test_with_flags("kernel.bin", &["-no-apic"]);
        assert!(result.serial_output().contains("PicMode"));
    }

    #[test]
    fn test_smp_boot() {
        // 4 核启动测试
        let result = qemu::boot_test_with_smp("kernel.bin", 4);
        assert!(result.serial_output().contains("3 个 AP 启动完成"));
    }
}
```

---

## 13. 附录

### 13.1 引导加载器配置示例

```toml
# omniagent-boot.toml
[boot]
timeout = 3
default = "omniagent"

[entry.omniagent]
kernel = "/boot/omniagent/kernel.bin"
initrd = "/boot/omniagent/initramfs.cpio.gz"
cmdline = "console=ttyS0 log_level=info kasan=off"

[entry.omniagent.recovery]
kernel = "/boot/omniagent/kernel.bin"
initrd = "/boot/omniagent/initramfs.cpio.gz"
cmdline = "console=ttyS0 log_level=debug single=true recovery=true"
```

### 13.2 ACPI 表依赖

| 表 | 用途 | 阶段 |
|----|------|------|
| MADT | APIC 信息、AP 启动 | S6, SMP |
| HPET | 高精度定时器 | S7 |
| DSDT | 设备配置 | S10 |
| FACP | 电源管理、FACS 地址 | S7 |
| RSDP/XSDT | ACPI 根表 | S3 |

### 13.3 参考资料

- Multiboot2 Specification v2.0
- Intel 64 and IA-32 Architectures Software Developer's Manual, Volume 3
- UEFI Specification v2.10
- ACPI Specification v6.5
