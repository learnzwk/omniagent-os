#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
use core::panic::PanicInfo;

// 导入内核库 (仅在非测试模式下使用，因为 bin 入口点只在裸金属上运行)
#[cfg(not(test))]
use omniagent_kernel::{println, drivers, arch, interrupts, time, memory, syscall, agent, scheduler};

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // === Phase 1: 硬件初始化 ===
    // S3: 串口初始化 + 日志系统
    drivers::serial::init_serial();
    drivers::serial::init_logger();

    // === Phase 2: 架构初始化 ===
    // S4: 加载 GDT (全局描述符表)
    unsafe { arch::x86_64::gdt::load_gdt(); }

    // S5: 加载 IDT + CPU 异常处理
    unsafe { interrupts::init_idt(); }

    // S6: 禁用 PIC + 初始化 Local APIC
    arch::x86_64::pic::disable_pic();
    unsafe { arch::x86_64::apic::init_local_apic(); }

    // === Phase 3: 内存初始化 ===
    // 注意：实际启动时需要从 bootloader 获取内存信息
    // 这里使用固定的测试值（真实环境中由 multiboot2 提供）
    memory::heap::init_heap(0x100_000, 0x100_000);  // 1MB 堆（测试值）
    memory::slab::init_default_caches();

    // === Phase 4: 子系统初始化 ===
    // S7: 初始化 PIT 定时器
    unsafe { time::timer::init_pit_timer(); }

    // S8: 初始化 PS/2 键盘
    unsafe { drivers::keyboard::init_keyboard(); }

    // 初始化系统调用分发器（包含 Agent 池和通信管理器初始化）
    syscall::dispatch::init();

    // 初始化 Agent 子系统
    agent::pool::init();
    agent::communication::init();

    // === Phase 5: 调度器初始化 ===
    scheduler::init();

    println!("=== OmniAgent OS v0.2.0 ===");
    println!("All subsystems initialized");
    println!("Ready.");

    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("!!! KERNEL PANIC !!!");
    if let Some(location) = info.location() {
        println!("  at {}", location);
    }
    loop {}
}
