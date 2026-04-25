# QEMU 调试指南

> 本指南介绍如何使用 QEMU 和 GDB 对 OmniAgent OS 内核进行调试，涵盖断点设置、状态检查、崩溃分析及多核调试。

## QEMU 命令行调试选项

### 基本调试启动

```bash
# 启动 QEMU 并等待 GDB 连接
qemu-system-x86_64 \
    -drive format=raw,file=target/bootimage.bin \
    -s -S -serial mon:stdio -m 256M
```

| 参数 | 说明 |
|------|------|
| `-s` | 在 TCP 1234 端口开启 GDB 调试服务器（等同于 `-gdb tcp::1234`） |
| `-S` | 启动时暂停 CPU，等待 GDB 连接 |
| `-d` | 输出调试日志（如 `-d int,cpu_reset`） |
| `-D logfile` | 日志写入文件 |
| `-serial mon:stdio` | 串口输出到终端 |

### 调试日志选项

```bash
# 查看中断日志
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin -d int

# 查看 CPU 执行日志
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin -d cpu_reset,int

# 查看内存访问日志
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin -d mmu

# 日志输出到文件
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -d int -D qemu.log

# 组合：调试 + 串口日志
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -s -S -serial file:serial.log -d int -D qemu.log
```

### bootimage 集成

```bash
cargo bootimage --run -- --s -S
cargo bootimage --run -- --gdb tcp::5678 --serial file:serial.log
```

---

## GDB 远程调试设置

### 基本连接

```bash
# 终端 1: 启动 QEMU
cargo bootimage --run -- --s -S

# 终端 2: 启动 GDB
rust-gdb target/x86_64-unknown-none/debug/omniagent-kernel
(gdb) target remote localhost:1234
```

### .gdbinit 配置

```gdb
# .gdbinit（项目根目录）
set architecture i386:x86-64
set pagination off
set print pretty on
set print array on
set print elements 0
set disassembly-flavor att

file target/x86_64-unknown-none/debug/omniagent-kernel

# 自定义命令
define show_stack
    info frame
    info args
    info locals
    x/16i $pc-32
end

define show_page_table
    printf "PML4 (CR3=0x%lx):\n", $cr3
    x/512gx ($cr3 & 0xFFFFFFFFFFFFF000)
end
```

---

## 常用 GDB 命令

### 执行控制

```gdb
continue / c              # 继续执行
step / s                  # 单步（进入函数）
next / n                  # 单步（跳过函数）
nexti / ni                # 单步汇编指令
advance *0x100000         # 执行到指定地址
```

### 断点管理

```gdb
break kernel_main         # 函数断点
break *0x100200           # 地址断点
break syscall_handler if $rdi == 1  # 条件断点
tbreak page_fault         # 临时断点
hbreak *0x100200          # 硬件断点
info breakpoints          # 查看所有断点
delete 1                  # 删除断点
disable 1 / enable 1      # 禁用/启用断点
ignore 1 100              # 忽略断点 N 次
```

### 内存与寄存器

```gdb
# 内存检查
x/16gx 0x100000           # 十六进制查看
x/16dw 0x100000           # 十进制查看
x/16i 0x100000            # 反汇编查看
x/32gx $rsp               # 查看栈
x/s 0x100500              # 查看字符串

# 寄存器
info registers / i r      # 通用寄存器
print $cr0 / $cr2 / $cr3 / $cr4  # 控制寄存器
print $cs / $ds / $ss     # 段寄存器
set $rax = 0              # 修改寄存器
```

---

## 断点策略

### 内核入口

```gdb
break _start              # multiboot 入口
break kernel_main         # Rust 入口
break omniagent_kernel::arch::x86_64::boot::init
```

### 系统调用

```gdb
break syscall_handler
break syscall_handler if $rdi == 1    # SYS_WRITE
break syscall_handler if $rdi == 13   # SYS_IPC_SEND
break omniagent_kernel::syscall::write
```

### 页错误

```gdb
break page_fault_handler
break page_fault_handler if $cr2 == 0xdeadbeef
break page_fault_handler if ($errcode & 0x2) != 0  # 写操作导致
```

### IPC 与调度

```gdb
break omniagent_kernel::ipc::send_message
break omniagent_kernel::ipc::receive_message
break omniagent_kernel::process::scheduler::switch_context
```

---

## 检查内核状态

### 寄存器转储

```gdb
define dump_registers
    printf "=== General Purpose ===\n"
    info registers
    printf "\n=== Control ===\n"
    printf "CR0: 0x%016lx\n", $cr0
    printf "CR2: 0x%016lx (Fault Addr)\n", $cr2
    printf "CR3: 0x%016lx (PML4 Root)\n", $cr3
    printf "CR4: 0x%016lx\n", $cr4
    printf "RFLAGS: 0x%016lx\n", $rflags
end
```

### 页表检查

```gdb
define show_pml4
    set $pml4 = $cr3 & 0xFFFFFFFFFFFFF000
    printf "PML4 at 0x%016lx:\n", $pml4
    set $i = 0
    while $i < 512
        set $entry = *(unsigned long *)($pml4 + $i * 8)
        if $entry != 0
            printf "  [%3d] = 0x%016lx", $i, $entry
            if ($entry & 1) printf " P"
            if ($entry & 2) printf " RW"
            if ($entry & 4) printf " US"
            printf "\n"
        end
        set $i = $i + 1
    end
end
```

### 进程与栈

```gdb
# 进程列表
print omniagent_kernel::process::scheduler::current_process
print omniagent_kernel::process::scheduler::run_queue

# 栈回溯
backtrace / bt
backtrace full / bt full  # 含局部变量
frame 3                   # 切换栈帧
info frame / info args / info locals
```

---

## 串口日志

### QEMU 串口配置

```bash
# 串口到终端
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -serial mon:stdio -nographic

# 串口到文件
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -serial file:serial.log

# 串口到文件 + 终端
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -serial stdio 2>&1 | tee serial.log
```

### 内核日志宏

```rust
#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => {
        $crate::logging::_print(format_args!($($arg)*));
    };
}

// 使用
klog!("Page fault at 0x{:x}", fault_address);
klog!("Process {} created with pid {}", name, pid);
```

---

## 内核 Panic 分析

### Panic 处理器

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe { core::arch::x86_64::asm::cli(); }
    klog!("\n=== KERNEL PANIC ===");
    if let Some(loc) = info.location() {
        klog!("Location: {}:{}:{}", loc.file(), loc.line(), loc.column());
    }
    if let Some(msg) = info.payload().downcast_ref::<&str>() {
        klog!("Message: {}", msg);
    }
    dump_registers();
    dump_backtrace();
    loop { unsafe { core::arch::x86_64::asm::hlt(); } }
}
```

### GDB 分析 Panic

```gdb
break omniagent_kernel::panic::panic_handler
# 当 panic 触发时：
bt full                    # 完整调用栈
frame 1                    # 切换到触发 panic 的帧
list                       # 查看源码
info locals                # 查看局部变量
x/32gx $rsp                # 查看栈内存
```

---

## 性能分析

### QEMU 性能计数器

```bash
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -perf-map=auto -serial mon:stdio
# Ctrl+A C 进入 monitor: info profile
```

### 内核计时器

```rust
pub fn get_ticks() -> u64 {
    let high: u32; let low: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("edx") high, out("eax") low,
            options(nostack, nomem, preserves_flags));
    }
    ((high as u64) << 32) | (low as u64)
}

let start = get_ticks();
// ... 要测量的代码 ...
klog!("Operation took {} cycles", get_ticks() - start);
```

---

## 多核调试

### 启动多核

```bash
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -smp 4 -s -S -serial mon:stdio
```

### GDB 多核操作

```gdb
info threads              # 查看所有 CPU 线程
thread 2                  # 切换到 CPU 1
break page_fault_handler thread 1  # 在指定 CPU 设断点
continue &                # 继续所有 CPU
```

### 竞态条件调试

```gdb
break omniagent_kernel::sync::spinlock::acquire
break omniagent_kernel::sync::spinlock::release
break omniagent_kernel::ipc::send_message
watch omniagent_kernel::process::scheduler::run_queue.len
```

---

## 常见调试场景

### 场景 1: 内核启动黑屏

```bash
# 检查串口输出
qemu-system-x86_64 -drive format=raw,file=target/bootimage.bin \
    -serial file:serial.log -nographic
cat serial.log

# GDB 逐步跟踪
gdb -ex "target remote localhost:1234" -ex "break _start" -ex "continue" \
    target/x86_64-unknown-none/debug/omniagent-kernel
(gdb) nexti  # 逐指令执行
```

### 场景 2: 页错误调试

```gdb
break page_fault_handler
continue
print $cr2          # 出错的虚拟地址
print/x $errcode    # bit0:存在 bit1:写 bit2:用户态
bt full
```

### 场景 3: IPC 死锁

```gdb
break omniagent_kernel::ipc::send_message
break omniagent_kernel::sync::spinlock::acquire
continue
info threads
thread apply all bt
```

### 场景 4: 内存泄漏

```gdb
break omniagent_kernel::mm::heap::allocate
break omniagent_kernel::mm::heap::deallocate
commands 1
> printf "ALLOC %p size=%d\n", $rdi, $rsi
> continue
> end
commands 2
> printf "FREE %p\n", $rdi
> continue
> end
continue
```

### 调试技巧速查

| 技巧 | 命令 | 用途 |
|------|------|------|
| 自动断点命令 | `commands N ... end` | 断点触发时自动执行 |
| 条件断点 | `break func if expr` | 特定条件时中断 |
| 观察点 | `watch variable` | 变量变化时中断 |
| 捕获点 | `catch syscall open` | 系统调用时中断 |
