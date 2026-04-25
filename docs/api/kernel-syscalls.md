# 内核系统调用 API 参考

> **模块名称**: `kernel-syscalls`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

本文档定义了 OmniAgent OS 微内核的完整系统调用 (syscall) 接口。系统调用是用户空间程序与内核交互的唯一途径，涵盖文件 I/O、进程管理、内存管理、进程间通信、设备管理、Agent 操作和虚拟化等类别。OmniAgent OS 在传统 POSIX 风格系统调用基础上，扩展了 Agent 专用系统调用（编号 512+），为 Agent-Native 架构提供内核级支持。

### 1.2 ABI 规范

| 属性 | 值 |
|------|-----|
| 调用约定 | System V AMD64 ABI (`sysv64`) |
| 系统调用号寄存器 | `rax` |
| 参数寄存器 | `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` |
| 返回值寄存器 | `rax` |
| 错误指示 | 返回值为负数时表示错误（`-errno`） |
| 最大参数数量 | 6 |
| ABI 版本 | 1.0 |

---

## 2. 系统调用表

### 2.1 传统系统调用 (0-511)

#### 文件 I/O 类

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 0 | `open` | `(path: *const u8, flags: u32, mode: u32)` | `i64` | 打开文件 |
| 1 | `close` | `(fd: u32)` | `i64` | 关闭文件描述符 |
| 2 | `read` | `(fd: u32, buf: *mut u8, count: usize)` | `i64` | 读取文件 |
| 3 | `write` | `(fd: u32, buf: *const u8, count: usize)` | `i64` | 写入文件 |
| 4 | `seek` | `(fd: u32, offset: i64, whence: u32)` | `i64` | 文件指针定位 |
| 5 | `stat` | `(path: *const u8, buf: *mut Stat)` | `i64` | 获取文件状态 |
| 6 | `fstat` | `(fd: u32, buf: *mut Stat)` | `i64` | 获取文件描述符状态 |
| 7 | `mkdir` | `(path: *const u8, mode: u32)` | `i64` | 创建目录 |
| 8 | `rmdir` | `(path: *const u8)` | `i64` | 删除目录 |
| 9 | `unlink` | `(path: *const u8)` | `i64` | 删除文件 |
| 10 | `rename` | `(old: *const u8, new: *const u8)` | `i64` | 重命名文件 |
| 11 | `opendir` | `(path: *const u8)` | `i64` | 打开目录流 |
| 12 | `readdir` | `(dirfd: u32, entry: *mut DirEntry)` | `i64` | 读取目录项 |
| 13 | `poll` | `(fds: *mut PollFd, nfds: u32, timeout: i32)` | `i64` | I/O 多路复用 |
| 14 | `ioctl` | `(fd: u32, request: u64, arg: *mut u8)` | `i64` | 设备控制 |
| 15 | `mmap` | `(addr: *mut u8, len: usize, prot: u32, flags: u32, fd: u32, offset: u64)` | `i64` | 内存映射 |
| 16 | `munmap` | `(addr: *mut u8, len: usize)` | `i64` | 取消内存映射 |

#### 进程管理类

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 32 | `fork` | `()` | `i64` | 创建子进程 |
| 33 | `execve` | `(path: *const u8, argv: *const *const u8, envp: *const *const u8)` | `i64` | 执行程序 |
| 34 | `exit` | `(status: i32)` | `!` | 终止当前进程 |
| 35 | `waitpid` | `(pid: i32, status: *mut i32, options: u32)` | `i64` | 等待子进程 |
| 36 | `getpid` | `()` | `i64` | 获取进程 ID |
| 37 | `getppid` | `()` | `i64` | 获取父进程 ID |
| 38 | `kill` | `(pid: i32, sig: u32)` | `i64` | 发送信号 |
| 39 | `sigaction` | `(sig: u32, act: *const SigAction, oldact: *mut SigAction)` | `i64` | 设置信号处理 |
| 40 | `sigreturn` | `()` | `i64` | 从信号处理返回 |
| 41 | `clone` | `(flags: u64, stack: *mut u8, parent_tid: *mut i32, tls: *mut u8, child_tid: *mut i32)` | `i64` | 创建线程 |
| 42 | `futex` | `(addr: *mut u32, op: u32, val: u32, timeout: *const Timespec)` | `i64` | 快速用户空间互斥锁 |
| 43 | `setpriority` | `(which: u32, who: u32, prio: i32)` | `i64` | 设置进程优先级 |
| 44 | `getpriority` | `(which: u32, who: u32)` | `i64` | 获取进程优先级 |
| 45 | `sched_yield` | `()` | `i64` | 让出 CPU |
| 46 | `nanosleep` | `(req: *const Timespec, rem: *mut Timespec)` | `i64` | 高精度睡眠 |

#### 内存管理类

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 64 | `brk` | `(addr: *mut u8)` | `i64` | 设置堆顶 |
| 65 | `sbrk` | `(increment: isize)` | `i64` | 调整堆大小 |
| 66 | `mprotect` | `(addr: *mut u8, len: usize, prot: u32)` | `i64` | 设置内存保护 |
| 67 | `madvise` | `(addr: *mut u8, len: usize, advice: u32)` | `i64` | 内存使用建议 |
| 68 | `mincore` | `(addr: *mut u8, len: usize, vec: *mut u8)` | `i64` | 查询页面驻留状态 |
| 69 | `shmget` | `(key: u64, size: usize, flags: u32)` | `i64` | 创建共享内存 |
| 70 | `shmat` | `(shmid: i32, addr: *mut u8, flags: u32)` | `i64` | 附加共享内存 |
| 71 | `shmdt` | `(addr: *const u8)` | `i64` | 分离共享内存 |
| 72 | `shmctl` | `(shmid: i32, cmd: u32, buf: *mut u8)` | `i64` | 共享内存控制 |

#### IPC 类

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 96 | `pipe` | `(fds: *mut [u32; 2])` | `i64` | 创建管道 |
| 97 | `socket` | `(domain: u32, stype: u32, protocol: u32)` | `i64` | 创建套接字 |
| 98 | `bind` | `(sockfd: u32, addr: *const SockAddr, addrlen: u32)` | `i64` | 绑定地址 |
| 99 | `listen` | `(sockfd: u32, backlog: u32)` | `i64` | 监听连接 |
| 100 | `accept` | `(sockfd: u32, addr: *mut SockAddr, addrlen: *mut u32)` | `i64` | 接受连接 |
| 101 | `connect` | `(sockfd: u32, addr: *const SockAddr, addrlen: u32)` | `i64` | 发起连接 |
| 102 | `send` | `(sockfd: u32, buf: *const u8, len: usize, flags: u32)` | `i64` | 发送数据 |
| 103 | `recv` | `(sockfd: u32, buf: *mut u8, len: usize, flags: u32)` | `i64` | 接收数据 |
| 104 | `sendmsg` | `(sockfd: u32, msg: *const Msghdr, flags: u32)` | `i64` | 发送消息 |
| 105 | `recvmsg` | `(sockfd: u32, msg: *mut Msghdr, flags: u32)` | `i64` | 接收消息 |
| 106 | `shutdown` | `(sockfd: u32, how: u32)` | `i64` | 关闭套接字 |
| 107 | `epoll_create` | `(size: u32)` | `i64` | 创建 epoll 实例 |
| 108 | `epoll_ctl` | `(epfd: u32, op: u32, fd: u32, event: *const EpollEvent)` | `i64` | epoll 控制 |
| 109 | `epoll_wait` | `(epfd: u32, events: *mut EpollEvent, maxevents: u32, timeout: i32)` | `i64` | epoll 等待 |

#### 设备管理类

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 128 | `device_open` | `(name: *const u8, flags: u32)` | `i64` | 打开设备 |
| 129 | `device_close` | `(handle: u32)` | `i64` | 关闭设备 |
| 130 | `device_read` | `(handle: u32, buf: *mut u8, count: usize)` | `i64` | 读取设备 |
| 131 | `device_write` | `(handle: u32, buf: *const u8, count: usize)` | `i64` | 写入设备 |
| 132 | `device_ioctl` | `(handle: u32, request: u64, arg: *mut u8)` | `i64` | 设备控制 |
| 133 | `device_map` | `(handle: u32, offset: u64, size: usize)` | `i64` | 设备内存映射 |
| 134 | `gpu_submit` | `(queue: u32, cmds: *const GpuCmd, count: u32)` | `i64` | 提交 GPU 命令 |

### 2.2 Agent 系统调用 (512+)

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 512 | `agent_spawn` | `(config: *const AgentConfig)` | `i64` | 创建 Agent 实例 |
| 513 | `agent_configure` | `(id: u64, config: *const AgentConfig)` | `i64` | 配置 Agent |
| 514 | `agent_start` | `(id: u64)` | `i64` | 启动 Agent |
| 515 | `agent_pause` | `(id: u64)` | `i64` | 暂停 Agent |
| 516 | `agent_resume` | `(id: u64)` | `i64` | 恢复 Agent |
| 517 | `agent_stop` | `(id: u64)` | `i64` | 停止 Agent |
| 518 | `agent_kill` | `(id: u64, signal: u32)` | `i64` | 终止 Agent |
| 519 | `agent_send` | `(from: u64, to: u64, msg: *const AgentMsg, len: u32)` | `i64` | Agent 间消息 |
| 520 | `agent_broadcast` | `(from: u64, msg: *const AgentMsg, len: u32)` | `i64` | 广播消息 |
| 521 | `agent_subscribe` | `(id: u64, topic: *const u8, topic_len: u32)` | `i64` | 订阅主题 |
| 522 | `agent_unsubscribe` | `(id: u64, topic: *const u8, topic_len: u32)` | `i64` | 取消订阅 |
| 523 | `agent_status` | `(id: u64, buf: *mut AgentStatus)` | `i64` | 查询 Agent 状态 |
| 524 | `agent_list` | `(buf: *mut u64, max: u32)` | `i64` | 列出所有 Agent |
| 525 | `agent_capability` | `(id: u64, cap: *const u8, cap_len: u32)` | `i64` | 查询能力 |
| 526 | `agent_knowledge_share` | `(from: u64, to: u64, data: *const u8, len: u32)` | `i64` | 共享知识 |
| 527 | `agent_knowledge_query` | `(id: u64, query: *const u8, qlen: u32, buf: *mut u8, blen: u32)` | `i64` | 查询知识 |
| 528 | `agent_evolve` | `(id: u64, strategy: u32)` | `i64` | 触发进化 |
| 529 | `agent_pool_submit` | `(task: *const AgentTask)` | `i64` | 提交任务到池 |
| 530 | `agent_pool_result` | `(task_id: u64, buf: *mut u8, blen: u32)` | `i64` | 获取池结果 |

#### 虚拟化类

| 编号 | 名称 | 参数 | 返回值 | 说明 |
|------|------|------|--------|------|
| 576 | `vm_create` | `(config: *const VmConfig)` | `i64` | 创建虚拟机 |
| 577 | `vm_start` | `(vmid: u64)` | `i64` | 启动虚拟机 |
| 578 | `vm_stop` | `(vmid: u64)` | `i64` | 停止虚拟机 |
| 579 | `vm_pause` | `(vmid: u64)` | `i64` | 暂停虚拟机 |
| 580 | `vm_resume` | `(vmid: u64)` | `i64` | 恢复虚拟机 |
| 581 | `vm_map_memory` | `(vmid: u64, gpa: u64, hva: u64, size: u64, flags: u32)` | `i64` | 映射虚拟机内存 |
| 582 | `vm_io_port` | `(vmid: u64, port: u16, data: *mut u8, size: u32, write: bool)` | `i64` | 虚拟机 I/O 端口 |

---

## 3. 错误码定义

```rust
/// 内核错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SyscallError {
    /// 操作成功
    Ok              = 0,
    /// 无效参数
    InvalidArgument = -1,
    /// 权限不足
    PermissionDenied = -2,
    /// 文件不存在
    NoSuchFile      = -3,
    /// 文件已存在
    FileExists      = -4,
    /// 非文件
    NotAFile        = -5,
    /// 非目录
    NotADirectory   = -6,
    /// 输入/输出错误
    IoError         = -7,
    /// 设备无空间
    NoSpace         = -8,
    /// 内存不足
    OutOfMemory     = -9,
    /// 总线错误
    BusError        = -10,
    /// 系统调用中断
    Interrupted     = -11,
    /// 资源忙
    Busy            = -12,
    /// 已存在
    AlreadyExists   = -13,
    /// 不支持的系统调用
    NotSupported    = -14,
    /// 超时
    Timeout         = -15,
    /// 连接被拒绝
    ConnectionRefused = -16,
    /// 地址已在使用
    AddressInUse    = -17,
    /// 地址不可用
    AddressNotAvailable = -18,
    /// 连接重置
    ConnectionReset = -19,
    /// 进程不存在
    NoSuchProcess   = -20,
    /// Agent 不存在
    NoSuchAgent     = -21,
    /// Agent 状态无效
    InvalidAgentState = -22,
    /// Agent 权限不足
    AgentPermissionDenied = -23,
    /// 能力不支持
    CapabilityNotSupported = -24,
    /// 虚拟机错误
    VmError         = -25,
    /// 未知错误
    Unknown         = -99,
}

impl SyscallError {
    /// 从原始返回值解析错误码
    pub fn from_return_value(ret: i64) -> Result<i64, Self> {
        if ret < 0 {
            Err(Self::from_code(-ret as i32))
        } else {
            Ok(ret)
        }
    }

    fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Ok,
            1 => Self::InvalidArgument,
            2 => Self::PermissionDenied,
            3 => Self::NoSuchFile,
            4 => Self::FileExists,
            5 => Self::NotAFile,
            6 => Self::NotADirectory,
            7 => Self::IoError,
            8 => Self::NoSpace,
            9 => Self::OutOfMemory,
            10 => Self::BusError,
            11 => Self::Interrupted,
            12 => Self::Busy,
            13 => Self::AlreadyExists,
            14 => Self::NotSupported,
            15 => Self::Timeout,
            16 => Self::ConnectionRefused,
            17 => Self::AddressInUse,
            18 => Self::AddressNotAvailable,
            19 => Self::ConnectionReset,
            20 => Self::NoSuchProcess,
            21 => Self::NoSuchAgent,
            22 => Self::InvalidAgentState,
            23 => Self::AgentPermissionDenied,
            24 => Self::CapabilityNotSupported,
            25 => Self::VmError,
            _ => Self::Unknown,
        }
    }
}

impl std::fmt::Display for SyscallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SyscallError({})", self as *const _ as i32)
    }
}

impl std::error::Error for SyscallError {}
```

---

## 4. 能力 (Capability) 系统

```rust
/// 进程能力集
#[derive(Debug, Clone)]
pub struct Capabilities {
    pub bits: u64,
}

impl Capabilities {
    pub const FILE_READ: u64      = 1 << 0;
    pub const FILE_WRITE: u64     = 1 << 1;
    pub const FILE_EXEC: u64      = 1 << 2;
    pub const NET_BIND: u64       = 1 << 3;
    pub const NET_CONNECT: u64    = 1 << 4;
    pub const PROCESS_FORK: u64   = 1 << 5;
    pub const PROCESS_KILL: u64   = 1 << 6;
    pub const MEM_MMAP: u64       = 1 << 7;
    pub const DEVICE_ACCESS: u64  = 1 << 8;
    pub const AGENT_SPAWN: u64    = 1 << 9;
    pub const AGENT_COMM: u64     = 1 << 10;
    pub const AGENT_EVOLVE: u64   = 1 << 11;
    pub const VM_CREATE: u64      = 1 << 12;
    pub const SYS_ADMIN: u64      = 1 << 63;

    pub fn has(&self, cap: u64) -> bool {
        (self.bits & cap) != 0
    }

    pub fn grant(&mut self, cap: u64) {
        self.bits |= cap;
    }

    pub fn revoke(&mut self, cap: u64) {
        self.bits &= !cap;
    }
}
```

---

## 5. Rust FFI 绑定

### 5.1 原始系统调用签名

```rust
/// 内核系统调用 FFI 绑定
/// 所有系统调用均使用 System V AMD64 调用约定
pub mod syscalls {
    /// 文件 I/O 系统调用
    pub mod file {
        /// 打开文件
        ///
        /// - `path`: 文件路径（以 null 结尾）
        /// - `flags`: 打开标志（O_RDONLY, O_WRONLY, O_RDWR, O_CREAT, O_TRUNC, O_APPEND）
        /// - `mode`: 文件权限（创建时使用）
        /// - 返回: 文件描述符或负错误码
        pub unsafe fn open(path: *const u8, flags: u32, mode: u32) -> i64 {
            syscall3(0, path as u64, flags as u64, mode as u64)
        }

        /// 关闭文件描述符
        pub unsafe fn close(fd: u32) -> i64 {
            syscall1(1, fd as u64)
        }

        /// 读取文件
        pub unsafe fn read(fd: u32, buf: *mut u8, count: usize) -> i64 {
            syscall3(2, fd as u64, buf as u64, count as u64)
        }

        /// 写入文件
        pub unsafe fn write(fd: u32, buf: *const u8, count: usize) -> i64 {
            syscall3(3, fd as u64, buf as u64, count as u64)
        }

        /// 文件指针定位
        pub unsafe fn seek(fd: u32, offset: i64, whence: u32) -> i64 {
            syscall3(4, fd as u64, offset as u64, whence as u64)
        }

        /// 获取文件状态
        pub unsafe fn stat(path: *const u8, buf: *mut super::Stat) -> i64 {
            syscall2(5, path as u64, buf as u64)
        }

        /// 内存映射
        pub unsafe fn mmap(
            addr: *mut u8,
            len: usize,
            prot: u32,
            flags: u32,
            fd: u32,
            offset: u64,
        ) -> i64 {
            syscall6(15, addr as u64, len as u64, prot as u64, flags as u64, fd as u64, offset)
        }

        /// 取消内存映射
        pub unsafe fn munmap(addr: *mut u8, len: usize) -> i64 {
            syscall2(16, addr as u64, len as u64)
        }
    }

    /// 进程管理系统调用
    pub mod process {
        /// 创建子进程
        pub unsafe fn fork() -> i64 {
            syscall0(32)
        }

        /// 执行程序
        pub unsafe fn execve(
            path: *const u8,
            argv: *const *const u8,
            envp: *const *const u8,
        ) -> i64 {
            syscall3(33, path as u64, argv as u64, envp as u64)
        }

        /// 终止当前进程
        pub unsafe fn exit(status: i32) -> ! {
            syscall1(34, status as u64);
            unreachable!()
        }

        /// 等待子进程
        pub unsafe fn waitpid(pid: i32, status: *mut i32, options: u32) -> i64 {
            syscall3(35, pid as u64, status as u64, options as u64)
        }

        /// 获取进程 ID
        pub unsafe fn getpid() -> i64 {
            syscall0(36)
        }

        /// 获取父进程 ID
        pub unsafe fn getppid() -> i64 {
            syscall0(37)
        }

        /// 发送信号
        pub unsafe fn kill(pid: i32, sig: u32) -> i64 {
            syscall2(38, pid as u64, sig as u64)
        }

        /// 创建线程
        pub unsafe fn clone(
            flags: u64,
            stack: *mut u8,
            parent_tid: *mut i32,
            tls: *mut u8,
            child_tid: *mut i32,
        ) -> i64 {
            syscall5(41, flags, stack as u64, parent_tid as u64, tls as u64, child_tid as u64)
        }

        /// futex 系统调用
        pub unsafe fn futex(
            addr: *mut u32,
            op: u32,
            val: u32,
            timeout: *const super::Timespec,
        ) -> i64 {
            syscall4(42, addr as u64, op as u64, val as u64, timeout as u64)
        }
    }

    /// Agent 系统调用
    pub mod agent {
        /// 创建 Agent 实例
        pub unsafe fn spawn(config: *const super::AgentConfig) -> i64 {
            syscall1(512, config as u64)
        }

        /// 启动 Agent
        pub unsafe fn start(id: u64) -> i64 {
            syscall1(514, id)
        }

        /// 暂停 Agent
        pub unsafe fn pause(id: u64) -> i64 {
            syscall1(515, id)
        }

        /// 停止 Agent
        pub unsafe fn stop(id: u64) -> i64 {
            syscall1(517, id)
        }

        /// Agent 间发送消息
        pub unsafe fn send(from: u64, to: u64, msg: *const super::AgentMsg, len: u32) -> i64 {
            syscall4(519, from, to, msg as u64, len as u64)
        }

        /// 广播消息
        pub unsafe fn broadcast(from: u64, msg: *const super::AgentMsg, len: u32) -> i64 {
            syscall3(520, from, msg as u64, len as u64)
        }

        /// 查询 Agent 状态
        pub unsafe fn status(id: u64, buf: *mut super::AgentStatus) -> i64 {
            syscall2(523, id, buf as u64)
        }

        /// 列出所有 Agent
        pub unsafe fn list(buf: *mut u64, max: u32) -> i64 {
            syscall2(524, buf as u64, max as u64)
        }

        /// 触发 Agent 进化
        pub unsafe fn evolve(id: u64, strategy: u32) -> i64 {
            syscall2(528, id, strategy as u64)
        }
    }

    /// 虚拟化系统调用
    pub mod vm {
        /// 创建虚拟机
        pub unsafe fn create(config: *const super::VmConfig) -> i64 {
            syscall1(576, config as u64)
        }

        /// 启动虚拟机
        pub unsafe fn start(vmid: u64) -> i64 {
            syscall1(577, vmid)
        }

        /// 停止虚拟机
        pub unsafe fn stop(vmid: u64) -> i64 {
            syscall1(578, vmid)
        }

        /// 映射虚拟机内存
        pub unsafe fn map_memory(
            vmid: u64,
            gpa: u64,
            hva: u64,
            size: u64,
            flags: u32,
        ) -> i64 {
            syscall5(581, vmid, gpa, hva, size, flags as u64)
        }
    }
}

/// 底层系统调用入口（内联汇编）
#[inline(always)]
unsafe fn syscall0(nr: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
unsafe fn syscall1(nr: u64, a1: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a1,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
unsafe fn syscall2(nr: u64, a1: u64, a2: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
unsafe fn syscall3(nr: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
unsafe fn syscall4(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
unsafe fn syscall5(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
unsafe fn syscall6(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> i64 {
    let ret: i64;
    std::arch::asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        in("r9") a6,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}
```

---

## 6. 数据结构定义

```rust
/// 文件状态信息
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_size: u64,
    pub st_blksize: u32,
    pub st_blocks: u64,
    pub st_atime: u64,
    pub st_mtime: u64,
    pub st_ctime: u64,
}

/// 时间规格
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

/// 目录项
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DirEntry {
    pub inode: u64,
    pub name: [u8; 256],
    pub name_len: u16,
    pub type_: u8,
}

/// 信号动作
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SigAction {
    pub handler: u64,
    pub flags: u64,
    pub mask: u64,
}

/// Poll 文件描述符
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PollFd {
    pub fd: u32,
    pub events: u16,
    pub revents: u16,
}

/// 套接字地址
#[derive(Debug, Clone)]
#[repr(C)]
pub struct SockAddr {
    pub family: u16,
    pub data: [u8; 14],
}

/// 消息头
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Msghdr {
    pub name: *mut u8,
    pub namelen: u32,
    pub iov: *mut IoVec,
    pub iovlen: u32,
    pub control: *mut u8,
    pub controllen: u32,
    pub flags: u32,
}

/// I/O 向量
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IoVec {
    pub iov_base: *mut u8,
    pub iov_len: usize,
}

/// Epoll 事件
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

/// Agent 配置
#[derive(Debug, Clone)]
#[repr(C)]
pub struct AgentConfig {
    pub name: [u8; 64],
    pub name_len: u32,
    pub agent_type: u32,
    pub priority: u32,
    pub memory_limit: u64,
    pub capabilities: u64,
    pub model_path: [u8; 256],
    pub model_path_len: u32,
}

/// Agent 消息
#[derive(Debug, Clone)]
#[repr(C)]
pub struct AgentMsg {
    pub msg_type: u32,
    pub flags: u32,
    pub data: [u8; 4096],
    pub data_len: u32,
}

/// Agent 状态
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AgentStatus {
    pub id: u64,
    pub state: u32,
    pub cpu_usage: u32,
    pub memory_usage: u64,
    pub message_count: u64,
    pub uptime: u64,
}

/// Agent 任务
#[derive(Debug, Clone)]
#[repr(C)]
pub struct AgentTask {
    pub task_type: u32,
    pub priority: u32,
    pub data: [u8; 4096],
    pub data_len: u32,
    pub timeout_ms: u64,
}

/// GPU 命令
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuCmd {
    pub cmd_type: u32,
    pub offset: u64,
    pub size: u64,
    pub flags: u32,
}

/// 虚拟机配置
#[derive(Debug, Clone)]
#[repr(C)]
pub struct VmConfig {
    pub name: [u8; 64],
    pub name_len: u32,
    pub vcpu_count: u32,
    pub memory_size: u64,
    pub kernel_path: [u8; 256],
    pub kernel_path_len: u32,
    pub initrd_path: [u8; 256],
    pub initrd_path_len: u32,
    pub flags: u64,
}
```

---

## 7. libagent 安全封装

### 7.1 文件 I/O 封装

```rust
use std::path::Path;
use std::ffi::CString;

/// libagent: 文件操作安全封装
pub mod fs {
    use super::*;

    /// 打开文件
    pub fn open(path: &Path, flags: OpenFlags) -> Result<File, SyscallError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| SyscallError::InvalidArgument)?;
        let fd = unsafe {
            syscalls::file::open(c_path.as_ptr(), flags.bits(), 0o644)
        };
        let fd = SyscallError::from_return_value(fd)? as u32;
        Ok(File { fd })
    }

    /// 读取文件内容
    pub fn read_to_string(path: &Path) -> Result<String, SyscallError> {
        let mut file = open(path, OpenFlags::RDONLY)?;
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let n = file.read(&mut tmp)?;
            if n == 0 { break; }
            buf.extend_from_slice(&tmp[..n]);
        }
        String::from_utf8(buf).map_err(|_| SyscallError::IoError)
    }

    /// 写入文件
    pub fn write(path: &Path, data: &[u8]) -> Result<(), SyscallError> {
        let mut file = open(path, OpenFlags::WRONLY | OpenFlags::CREAT | OpenFlags::TRUNC)?;
        file.write_all(data)
    }
}

bitflags::bitflags! {
    /// 文件打开标志
    #[derive(Debug, Clone, Copy)]
    pub struct OpenFlags: u32 {
        const RDONLY   = 0o0;
        const WRONLY   = 0o1;
        const RDWR     = 0o2;
        const CREAT    = 0o100;
        const TRUNC    = 0o1000;
        const APPEND   = 0o2000;
        const NONBLOCK = 0o4000;
    }
}

/// 文件描述符封装
pub struct File {
    fd: u32,
}

impl File {
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        let n = unsafe { syscalls::file::read(self.fd, buf.as_mut_ptr(), buf.len()) };
        SyscallError::from_return_value(n).map(|n| n as usize)
    }

    pub fn write_all(&mut self, data: &[u8]) -> Result<(), SyscallError> {
        let mut offset = 0;
        while offset < data.len() {
            let n = unsafe {
                syscalls::file::write(self.fd, data[offset..].as_ptr(), data.len() - offset)
            };
            let n = SyscallError::from_return_value(n)? as usize;
            if n == 0 {
                return Err(SyscallError::IoError);
            }
            offset += n;
        }
        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe { syscalls::file::close(self.fd); }
    }
}
```

### 7.2 Agent 操作封装

```rust
/// libagent: Agent 操作安全封装
pub mod agent {
    use super::*;

    /// Agent 实例句柄
    pub struct AgentHandle {
        id: u64,
    }

    impl AgentHandle {
        /// 创建新 Agent
        pub fn spawn(config: &AgentConfig) -> Result<Self, SyscallError> {
            let id = unsafe { syscalls::agent::spawn(config) };
            let id = SyscallError::from_return_value(id)? as u64;
            Ok(Self { id })
        }

        /// 启动 Agent
        pub fn start(&self) -> Result<(), SyscallError> {
            let ret = unsafe { syscalls::agent::start(self.id) };
            SyscallError::from_return_value(ret)?;
            Ok(())
        }

        /// 暂停 Agent
        pub fn pause(&self) -> Result<(), SyscallError> {
            let ret = unsafe { syscalls::agent::pause(self.id) };
            SyscallError::from_return_value(ret)?;
            Ok(())
        }

        /// 停止 Agent
        pub fn stop(&self) -> Result<(), SyscallError> {
            let ret = unsafe { syscalls::agent::stop(self.id) };
            SyscallError::from_return_value(ret)?;
            Ok(())
        }

        /// 发送消息给另一个 Agent
        pub fn send_message(&self, to: u64, msg: &AgentMsg) -> Result<(), SyscallError> {
            let ret = unsafe {
                syscalls::agent::send(self.id, to, msg, msg.data_len)
            };
            SyscallError::from_return_value(ret)?;
            Ok(())
        }

        /// 获取 Agent 状态
        pub fn status(&self) -> Result<AgentStatus, SyscallError> {
            let mut status = AgentStatus {
                id: self.id,
                state: 0,
                cpu_usage: 0,
                memory_usage: 0,
                message_count: 0,
                uptime: 0,
            };
            let ret = unsafe { syscalls::agent::status(self.id, &mut status) };
            SyscallError::from_return_value(ret)?;
            Ok(status)
        }

        /// 触发进化
        pub fn evolve(&self, strategy: EvolutionStrategy) -> Result<(), SyscallError> {
            let ret = unsafe { syscalls::agent::evolve(self.id, strategy as u32) };
            SyscallError::from_return_value(ret)?;
            Ok(())
        }
    }

    /// 进化策略
    #[derive(Debug, Clone, Copy)]
    pub enum EvolutionStrategy {
        Genetic = 0,
        Gradient = 1,
        Reinforcement = 2,
    }

    /// 列出所有 Agent
    pub fn list_agents() -> Result<Vec<u64>, SyscallError> {
        let mut buf = [0u64; 256];
        let count = unsafe { syscalls::agent::list(buf.as_mut_ptr(), 256) };
        let count = SyscallError::from_return_value(count)? as usize;
        Ok(buf[..count].to_vec())
    }
}
```

---

## 8. ABI 版本控制

```rust
/// ABI 版本信息
pub const ABI_VERSION_MAJOR: u32 = 1;
pub const ABI_VERSION_MINOR: u32 = 0;
pub const ABI_VERSION_PATCH: u32 = 0;

/// 完整 ABI 版本字符串
pub const ABI_VERSION_STR: &str = "1.0.0";

/// ABI 兼容性检查
pub fn check_abi_compatibility(major: u32, minor: u32) -> bool {
    // 主版本号必须一致，次版本号可以向上兼容
    major == ABI_VERSION_MAJOR && minor <= ABI_VERSION_MINOR
}

/// 系统调用特性标志
#[derive(Debug, Clone, Copy)]
pub struct SyscallFeatures {
    pub flags: u64,
}

impl SyscallFeatures {
    /// 支持 Agent 系统调用
    pub const AGENT_SYSCALLS: u64    = 1 << 0;
    /// 支持虚拟化系统调用
    pub const VM_SYSCALLS: u64       = 1 << 1;
    /// 支持 GPU 直接提交
    pub const GPU_SUBMIT: u64        = 1 << 2;
    /// 支持共享内存
    pub const SHARED_MEMORY: u64     = 1 << 3;
    /// 支持异步 I/O
    pub const ASYNC_IO: u64          = 1 << 4;

    pub fn has(&self, feature: u64) -> bool {
        (self.flags & feature) != 0
    }
}
```

---

## 9. 使用示例

### 9.1 文件 I/O 示例

```rust
use libagent::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 写入文件
    let data = b"Hello, OmniAgent OS!";
    fs::write("/tmp/hello.txt", data)?;

    // 读取文件
    let content = fs::read_to_string("/tmp/hello.txt")?;
    println!("文件内容: {}", content);

    // 使用底层 API
    let path = std::path::Path::new("/proc/cpuinfo");
    let mut file = fs::open(path, libagent::OpenFlags::RDONLY)?;
    let mut buf = [0u8; 1024];
    let n = file.read(&mut buf)?;
    println!("读取了 {} 字节", n);

    Ok(())
}
```

### 9.2 Agent 操作示例

```rust
use libagent::agent::{self, AgentHandle, AgentConfig, AgentMsg, EvolutionStrategy};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 Agent 配置
    let config = AgentConfig {
        name_len: 10,
        name: {
            let mut buf = [0u8; 64];
            buf[..10].copy_from_slice(b"my-agent\0");
            buf
        },
        agent_type: 0,
        priority: 10,
        memory_limit: 256 * 1024 * 1024, // 256MB
        capabilities: libagent::Capabilities::AGENT_SPAWN
            | libagent::Capabilities::AGENT_COMM,
        ..Default::default()
    };

    // 创建并启动 Agent
    let agent = AgentHandle::spawn(&config)?;
    agent.start()?;

    // 查询状态
    let status = agent.status()?;
    println!("Agent {} 状态: {}", agent.id, status.state);

    // 发送消息
    let msg = AgentMsg {
        msg_type: 1,
        flags: 0,
        data_len: 5,
        data: {
            let mut buf = [0u8; 4096];
            buf[..5].copy_from_slice(b"hello");
            buf
        },
    };
    agent.send_message(42, &msg)?;

    // 触发进化
    agent.evolve(EvolutionStrategy::Genetic)?;

    // 列出所有 Agent
    let agents = agent::list_agents()?;
    println!("运行中的 Agent: {:?}", agents);

    // 停止 Agent
    agent.stop()?;

    Ok(())
}
```

### 9.3 IPC 示例

```rust
use libagent::syscalls;

fn ipc_server() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 TCP 套接字
    let sockfd = unsafe {
        syscalls::file::socket(
            2,  // AF_INET
            1,  // SOCK_STREAM
            0,  // IPPROTO_TCP
        )
    };
    let sockfd = libagent::SyscallError::from_return_value(sockfd)? as u32;

    // 绑定地址
    let addr = libagent::SockAddr {
        family: 2, // AF_INET
        data: {
            let mut buf = [0u8; 14];
            // 端口 8080
            buf[0] = 0x1F;
            buf[1] = 0x90;
            // IP 127.0.0.1
            buf[2] = 127;
            buf[3] = 0;
            buf[4] = 0;
            buf[5] = 1;
            buf
        },
    };

    unsafe {
        syscalls::file::bind(sockfd, &addr, std::mem::size_of::<libagent::SockAddr>() as u32);
        syscalls::file::listen(sockfd, 128);
    }

    println!("服务器监听在 127.0.0.1:8080");

    // 接受连接（简化示例）
    let mut client_addr = libagent::SockAddr::default();
    let mut addr_len = std::mem::size_of::<libagent::SockAddr>() as u32;
    let client_fd = unsafe {
        syscalls::file::accept(sockfd, &mut client_addr, &mut addr_len)
    };

    println!("接受客户端连接");

    Ok(())
}
```

---

## 10. 性能说明

| 系统调用类别 | 典型延迟 | 说明 |
|-------------|---------|------|
| 文件 I/O (缓存命中) | <1us | 页缓存命中时 |
| 文件 I/O (磁盘读取) | 50us-10ms | 取决于存储设备 |
| 进程 fork | 10-100us | 取决于内存大小 |
| 线程 clone | 1-10us | 轻量级 |
| futex (无竞争) | <100ns | 快速路径 |
| futex (有竞争) | 1-100us | 需要内核调度 |
| mmap | 1-50us | 取决于映射大小 |
| Agent spawn | 100us-1ms | 包含模型加载 |
| Agent send | <10us | 进程间消息传递 |
| Agent broadcast | <100us | 广播到所有订阅者 |
| Agent evolve | 10ms-1s | 取决于进化策略 |
| VM create | 50ms-500ms | 取决于配置 |
| socket (本地) | <5us | Unix domain socket |
| socket (TCP) | 10-100us | 网络套接字 |
| epoll_wait | <1us (有事件) | 事件就绪时 |

---

## 11. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_parsing() {
        assert_eq!(
            SyscallError::from_return_value(-3),
            Err(SyscallError::NoSuchFile)
        );
        assert_eq!(
            SyscallError::from_return_value(42),
            Ok(42)
        );
    }

    #[test]
    fn test_capabilities() {
        let mut caps = Capabilities { bits: 0 };
        assert!(!caps.has(Capabilities::FILE_READ));
        caps.grant(Capabilities::FILE_READ);
        assert!(caps.has(Capabilities::FILE_READ));
        caps.revoke(Capabilities::FILE_READ);
        assert!(!caps.has(Capabilities::FILE_READ));
    }

    #[test]
    fn test_abi_compatibility() {
        assert!(check_abi_compatibility(1, 0));
        assert!(check_abi_compatibility(1, 1));
        assert!(!check_abi_compatibility(2, 0));
    }

    #[test]
    fn test_open_flags() {
        let flags = OpenFlags::RDWR | OpenFlags::CREAT | OpenFlags::TRUNC;
        assert!(flags.contains(OpenFlags::RDWR));
        assert!(flags.contains(OpenFlags::CREAT));
        assert!(!flags.contains(OpenFlags::APPEND));
    }

    #[test]
    fn test_stat_struct_size() {
        assert_eq!(std::mem::size_of::<Stat>(), 104);
    }

    #[test]
    fn test_agent_config_layout() {
        assert!(std::mem::size_of::<AgentConfig>() > 0);
        assert_eq!(std::mem::align_of::<AgentConfig>(), 8);
    }

    #[test]
    fn test_syscall_features() {
        let features = SyscallFeatures {
            flags: SyscallFeatures::AGENT_SYSCALLS | SyscallFeatures::VM_SYSCALLS,
        };
        assert!(features.has(SyscallFeatures::AGENT_SYSCALLS));
        assert!(features.has(SyscallFeatures::VM_SYSCALLS));
        assert!(!features.has(SyscallFeatures::GPU_SUBMIT));
    }

    #[test]
    fn test_file_drop_closes_fd() {
        // 验证 File 的 Drop 实现（集成测试需要内核环境）
        // 这里仅验证编译通过
        let _file = File { fd: 0 };
    }

    #[test]
    fn test_error_display() {
        let err = SyscallError::NoSuchFile;
        assert!(format!("{:?}", err).contains("NoSuchFile"));
    }
}
```

---

*本文档为 OmniAgent OS 内核系统调用 API 参考，版本 0.1.0。*
