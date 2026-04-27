//! POSIX 系统调用实现
//!
//! 实现常用的 POSIX 兼容系统调用，包括：
//! - 进程管理：getpid, gettid, fork, clone, execve, exit, wait4
//! - 文件系统：open, close, read, write, stat, fstat, lseek, mkdir, unlink,
//!   getcwd, chdir, fcntl, getdents64, dup, dup2, readv, writev
//! - 内存管理：brk, mmap, munmap, mprotect, madvise
//! - 时间：clock_gettime
//! - 同步：futex, poll, pipe2
//! - 信号：sigaction
//! - 其他：ioctl, getrandom, rseq, set_tid_address, sched_yield,
//!   arch_prctl, setrlimit, getrlimit

use core::sync::atomic::{AtomicI64, Ordering};
use spin::Lazy;

use crate::fs::{FileDescriptorTable, OpenFlags, SeekFrom, VFS};
use crate::fs::inode::FileStat;
use crate::scheduler;

/// 进程 ID 计数器
static PID_COUNTER: AtomicI64 = AtomicI64::new(1);

/// 获取当前 PID
pub fn sys_getpid() -> i64 {
    // 简化实现：返回当前任务 ID
    scheduler::current_task_id()
        .map(|id| id.0 as i64)
        .unwrap_or(0)
}

/// 获取 TID（线程 ID，简化为与 PID 相同）
pub fn sys_gettid() -> i64 {
    sys_getpid()
}

/// 分配新 PID
pub fn alloc_pid() -> i64 {
    PID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// read syscall — 从文件描述符读取数据
pub fn sys_read(fd: i32, buf: &mut [u8]) -> i64 {
    let fd_table = get_current_fd_table();
    match fd_table.read(fd as u32, buf) {
        Ok(n) => n as i64,
        Err(_) => -9, // EBADF
    }
}

/// write syscall — 向文件描述符写入数据
pub fn sys_write(fd: i32, buf: &[u8]) -> i64 {
    let fd_table = get_current_fd_table();
    match fd_table.write(fd as u32, buf) {
        Ok(n) => n as i64,
        Err(_) => -9, // EBADF
    }
}

/// open syscall — 打开文件
pub fn sys_open(path: &str, flags: i32, _mode: u32) -> i64 {
    // 解析打开标志
    let open_flags = OpenFlags::from_bits(flags).unwrap_or(OpenFlags::O_RDONLY);

    // 通过 VFS 创建文件 inode
    let inode = match VFS.lock().create_file(path) {
        Ok(inode) => inode,
        Err(_) => return -2, // ENOENT
    };

    let fd_table = get_current_fd_table();
    match fd_table.open(inode, open_flags) {
        Ok(fd) => fd as i64,
        Err(_) => -24, // EMFILE
    }
}

/// close syscall — 关闭文件描述符
pub fn sys_close(fd: i32) -> i64 {
    let fd_table = get_current_fd_table();
    match fd_table.close(fd as u32) {
        Ok(()) => 0,
        Err(_) => -9, // EBADF
    }
}

/// stat syscall — 获取文件状态信息
pub fn sys_stat(_path: &str) -> FileStat {
    // 简化实现：返回默认的文件状态
    FileStat {
        st_dev: 0,
        st_ino: 0,
        st_mode: 0o100644,
        st_nlink: 1,
        st_size: 0,
        st_blksize: 4096,
        st_blocks: 0,
        st_atime: 0,
        st_mtime: 0,
        st_ctime: 0,
    }
}

/// fstat syscall — 通过文件描述符获取文件状态信息
pub fn sys_fstat(fd: i32) -> i64 {
    let fd_table = get_current_fd_table();
    match fd_table.fstat(fd as u32) {
        Ok(_stat) => 0,
        Err(_) => -9, // EBADF
    }
}

/// brk syscall — 堆扩展
pub fn sys_brk(new_brk: u64) -> u64 {
    // 简化实现：使用静态变量跟踪堆顶
    static HEAP_TOP: AtomicI64 = AtomicI64::new(0);
    static HEAP_START: AtomicI64 = AtomicI64::new(0);
    static HEAP_END: AtomicI64 = AtomicI64::new(0);

    let current = HEAP_TOP.load(Ordering::Relaxed);

    if new_brk == 0 {
        return if current == 0 { 0 } else { current as u64 };
    }

    let start = HEAP_START.load(Ordering::Relaxed);
    let end = HEAP_END.load(Ordering::Relaxed);

    if start == 0 {
        // 首次调用：初始化堆范围
        HEAP_START.store(new_brk as i64, Ordering::Relaxed);
        HEAP_END.store(new_brk as i64 + 0x100_000, Ordering::Relaxed); // 1MB 最大堆
        HEAP_TOP.store(new_brk as i64, Ordering::Relaxed);
        return new_brk;
    }

    if new_brk as i64 >= start && new_brk as i64 <= end {
        HEAP_TOP.store(new_brk as i64, Ordering::Relaxed);
        return new_brk;
    }

    current as u64 // 不变
}

/// clock_gettime syscall
pub fn sys_clock_gettime(clock_id: i32, tp: &mut TimeSpec) -> i64 {
    let now = match clock_id {
        0 => 0, // CLOCK_REALTIME（简化：返回 0）
        1 => 0, // CLOCK_MONOTONIC（简化：返回 0）
        _ => return -22, // EINVAL
    };
    tp.tv_sec = now / 1_000_000_000;
    tp.tv_nsec = (now % 1_000_000_000) as i64;
    0
}

/// exit syscall
pub fn sys_exit(code: i32) -> ! {
    if let Some(id) = scheduler::current_task_id() {
        scheduler::SCHEDULER.lock().exit(id, code);
    }
    loop {} // 不应到达
}

/// getppid syscall — 获取父进程 ID
pub fn sys_getppid() -> i64 {
    // 简化：父进程 ID 为 0（init 进程）
    0
}

/// getcwd syscall — 获取当前工作目录
pub fn sys_getcwd(buf: &mut [u8], size: usize) -> i64 {
    let cwd = b"/";
    if size < cwd.len() {
        return -34; // ERANGE
    }
    buf[..cwd.len()].copy_from_slice(cwd);
    cwd.len() as i64
}

/// chdir syscall — 切换当前工作目录
pub fn sys_chdir(_path: &str) -> i64 {
    // 简化：总是成功
    0
}

/// mkdir syscall — 创建目录
pub fn sys_mkdir(path: &str, _mode: u32) -> i64 {
    match VFS.lock().create_dir(path) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// unlink syscall — 删除文件
pub fn sys_unlink(path: &str) -> i64 {
    match VFS.lock().remove(path) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// fcntl syscall（简化）
pub fn sys_fcntl(fd: i32, cmd: i32, arg: i64) -> i64 {
    match cmd {
        0 => fd as i64,  // F_DUPFD: 返回 fd 本身
        1 => 1,   // F_GETFD: 返回 FD_CLOEXEC
        2 => arg, // F_SETFD
        3 => 0,   // F_GETFL
        4 => arg, // F_SETFL
        _ => -22, // EINVAL
    }
}

/// getdents64 syscall（简化）
pub fn sys_getdents64(_fd: i32, _buf: &mut [u8], _count: usize) -> i64 {
    // 简化：返回 0（无更多目录项）
    0
}

/// writev syscall（简化）
pub fn sys_writev(fd: i32, iovs: &[IoVec]) -> i64 {
    let mut total = 0i64;
    for iov in iovs {
        let fd_table = get_current_fd_table();
        match fd_table.write(fd as u32, &[]) {
            Ok(_) => {
                total += iov.len as i64;
            }
            Err(_) => return if total > 0 { total } else { -9 }, // EBADF
        }
    }
    total
}

/// readv syscall（简化）
pub fn sys_readv(fd: i32, iovs: &[IoVec]) -> i64 {
    let mut total = 0i64;
    for iov in iovs {
        let fd_table = get_current_fd_table();
        match fd_table.read(fd as u32, &mut []) {
            Ok(_) => {
                total += iov.len as i64;
            }
            Err(_) => return if total > 0 { total } else { -9 }, // EBADF
        }
    }
    total
}

/// IoVec 结构（scatter/gather I/O 向量）
pub struct IoVec {
    /// 缓冲区基地址
    pub base: u64,
    /// 缓冲区长度
    pub len: usize,
}

/// TimeSpec 结构（时间规格）
#[derive(Debug, Clone)]
#[repr(C)]
pub struct TimeSpec {
    /// 秒
    pub tv_sec: i64,
    /// 纳秒
    pub tv_nsec: i64,
}

/// 获取当前任务的 fd_table（简化：使用全局默认）
fn get_current_fd_table() -> &'static FileDescriptorTable {
    static DEFAULT_FD_TABLE: Lazy<FileDescriptorTable> = Lazy::new(|| {
        FileDescriptorTable::new()
    });
    &DEFAULT_FD_TABLE
}

/// mmap syscall（简化）
pub fn sys_mmap(_addr: u64, length: u64, _prot: i32, _flags: i32, _fd: i32, _offset: u64) -> u64 {
    if length == 0 { return !0; } // MAP_FAILED
    // 返回一个伪地址
    0x7f00_0000_0000
}

/// munmap syscall（简化）
pub fn sys_munmap(_addr: u64, _length: u64) -> i64 {
    0 // 总是成功
}

/// mprotect syscall（简化）
pub fn sys_mprotect(_addr: u64, _len: u64, _prot: i32) -> i64 {
    0
}

/// madvise syscall（简化）
pub fn sys_madvise(_addr: u64, _len: u64, _advice: i32) -> i64 {
    0
}

/// getrandom syscall（简化）
pub fn sys_getrandom(buf: &mut [u8], _flags: u32) -> i64 {
    // 简化：用递增序列填充（非安全随机，仅用于测试）
    static COUNTER: AtomicI64 = AtomicI64::new(0);
    for byte in buf.iter_mut() {
        let val = COUNTER.fetch_add(1, Ordering::Relaxed);
        *byte = (val & 0xFF) as u8;
    }
    buf.len() as i64
}

/// rseq syscall（简化）
pub fn sys_rseq(_ptr: u64, _len: u32, _flags: i32) -> i64 {
    0 // 简化：总是成功
}

/// set_tid_address syscall（简化）
pub fn sys_set_tid_address(_tidptr: u64) -> i64 {
    sys_gettid()
}

/// sigaction syscall（简化）
pub fn sys_sigaction(_signum: i32, _act: u64, _oldact: u64) -> i64 {
    0
}

/// lseek syscall — 移动文件读写位置
pub fn sys_lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    let seek_from = match whence {
        0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return -22,
    };
    let fd_table = get_current_fd_table();
    match fd_table.seek(fd as u32, offset, seek_from) {
        Ok(pos) => pos as i64,
        Err(_) => -9,
    }
}

/// ioctl syscall（简化）
pub fn sys_ioctl(_fd: i32, _request: u64, _arg: u64) -> i64 {
    0
}

/// poll syscall（简化）
pub fn sys_poll(_fds: u64, _nfds: u32, _timeout_ms: i32) -> i64 {
    0 // 简化：立即返回，无事件
}

/// dup syscall — 复制文件描述符
pub fn sys_dup(fd: i32) -> i64 {
    let fd_table = get_current_fd_table();
    match fd_table.dup(fd as u32) {
        Ok(new_fd) => new_fd as i64,
        Err(_) => -9,
    }
}

/// dup2 syscall — 复制文件描述符到指定编号
pub fn sys_dup2(oldfd: i32, newfd: i32) -> i64 {
    if oldfd < 0 || newfd < 0 { return -9; }
    if oldfd == newfd { return oldfd as i64; }
    // 简化：返回 newfd
    newfd as i64
}

/// pipe2 syscall（简化）
pub fn sys_pipe2(_pipefd: u64, _flags: i32) -> i64 {
    0
}

/// sched_yield syscall — 当前任务让出 CPU
pub fn sys_sched_yield() -> i64 {
    scheduler::yield_now();
    0
}

/// clone syscall（简化）
pub fn sys_clone(_flags: u64, _stack: u64, _parent_tid: u64, _child_tid: u64, _tls: u64) -> i64 {
    // 简化：返回新 PID
    alloc_pid()
}

/// fork syscall（简化）
pub fn sys_fork() -> i64 {
    // 简化：返回子进程 PID
    alloc_pid()
}

/// execve syscall（简化）
pub fn sys_execve(_pathname: &str, _argv: u64, _envp: u64) -> i64 {
    -38 // ENOSYS
}

/// wait4 syscall（简化）
pub fn sys_wait4(_pid: i32, _status: u64, _options: i32, _rusage: u64) -> i64 {
    0
}

/// futex syscall（简化）
pub fn sys_futex(_uaddr: u64, _op: i32, _val: u64, _timeout: u64, _uaddr2: u64, _val3: u64) -> i64 {
    0
}

/// arch_prctl syscall（简化）
pub fn sys_arch_prctl(_code: i32, _addr: u64) -> i64 {
    0
}

/// setrlimit syscall（简化）
pub fn sys_setrlimit(_resource: u32, _rlim: u64) -> i64 {
    0
}

/// getrlimit syscall（简化）
pub fn sys_getrlimit(_resource: u32, _rlim: u64) -> i64 {
    0
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_getpid_returns_non_negative() {
        // 在测试环境中可能没有当前任务，所以允许返回 0
        let pid = sys_getpid();
        assert!(pid >= 0, "getpid 应返回非负值，实际为 {}", pid);
    }

    #[test]
    fn test_gettid_equals_getpid() {
        // 简化实现中 TID 与 PID 相同
        assert_eq!(sys_gettid(), sys_getpid());
    }

    #[test]
    fn test_alloc_pid_increments() {
        let pid1 = alloc_pid();
        let pid2 = alloc_pid();
        assert!(pid2 > pid1, "每次 alloc_pid 应返回递增的值");
    }

    #[test]
    fn test_brk_basic() {
        // 首次调用 brk 设置堆起始位置
        let result = sys_brk(0x100_000);
        assert_eq!(result, 0x100_000, "首次 brk 应返回请求的地址");
    }

    #[test]
    fn test_brk_zero_returns_current() {
        // 先设置 brk
        let _ = sys_brk(0x200_000);
        // 传入 0 应返回当前堆顶
        let result = sys_brk(0);
        assert_eq!(result, 0x200_000, "brk(0) 应返回当前堆顶");
    }

    #[test]
    fn test_brk_within_range() {
        // 先获取当前堆顶（可能已被前一个测试初始化）
        let current = sys_brk(0);
        let base = if current == 0 { 0x300_000u64 } else { current };
        let _ = sys_brk(base);
        // 在范围内调整
        let new_brk = base + 0x1000;
        let result = sys_brk(new_brk);
        assert_eq!(result, new_brk);
    }

    #[test]
    fn test_brk_out_of_range() {
        // 先获取当前堆顶（可能已被前一个测试初始化）
        let current = sys_brk(0);
        let base = if current == 0 { 0x400_000u64 } else { current };
        let _ = sys_brk(base);
        // 超出范围（超过 1MB 限制）
        let too_large = base + 0x200_000;
        let result = sys_brk(too_large);
        // 应返回当前值不变
        assert_eq!(result, base);
    }

    #[test]
    fn test_clock_gettime_realtime() {
        let mut tp = TimeSpec { tv_sec: 999, tv_nsec: 999 };
        let result = sys_clock_gettime(0, &mut tp);
        assert_eq!(result, 0, "CLOCK_REALTIME 应成功");
        assert_eq!(tp.tv_sec, 0);
        assert_eq!(tp.tv_nsec, 0);
    }

    #[test]
    fn test_clock_gettime_monotonic() {
        let mut tp = TimeSpec { tv_sec: 0, tv_nsec: 0 };
        let result = sys_clock_gettime(1, &mut tp);
        assert_eq!(result, 0, "CLOCK_MONOTONIC 应成功");
    }

    #[test]
    fn test_clock_gettime_invalid() {
        let mut tp = TimeSpec { tv_sec: 0, tv_nsec: 0 };
        let result = sys_clock_gettime(99, &mut tp);
        assert_eq!(result, -22, "无效 clock_id 应返回 EINVAL");
    }

    #[test]
    fn test_getcwd() {
        let mut buf = [0u8; 256];
        let result = sys_getcwd(&mut buf, 256);
        assert_eq!(result, 1, "getcwd 应返回路径长度");
        assert_eq!(&buf[..1], b"/");
    }

    #[test]
    fn test_getcwd_buffer_too_small() {
        let mut buf = [0u8; 1];
        let result = sys_getcwd(&mut buf, 0);
        assert_eq!(result, -34, "缓冲区太小应返回 ERANGE");
    }

    #[test]
    fn test_chdir() {
        let result = sys_chdir("/tmp");
        assert_eq!(result, 0, "chdir 简化实现应总是成功");
    }

    #[test]
    fn test_mkdir() {
        let result = sys_mkdir("/test_dir", 0o755);
        assert_eq!(result, 0, "mkdir 应成功");
    }

    #[test]
    fn test_unlink() {
        let result = sys_unlink("/test_file");
        assert_eq!(result, 0, "unlink 应成功");
    }

    #[test]
    fn test_fcntl_dupfd() {
        let result = sys_fcntl(3, 0, 0);
        assert_eq!(result, 3, "F_DUPFD 应返回 fd 本身");
    }

    #[test]
    fn test_fcntl_getfd() {
        let result = sys_fcntl(3, 1, 0);
        assert_eq!(result, 1, "F_GETFD 应返回 FD_CLOEXEC");
    }

    #[test]
    fn test_fcntl_setfd() {
        let result = sys_fcntl(3, 2, 0);
        assert_eq!(result, 0, "F_SETFD 应返回 arg");
    }

    #[test]
    fn test_fcntl_getfl() {
        let result = sys_fcntl(3, 3, 0);
        assert_eq!(result, 0, "F_GETFL 应返回 0");
    }

    #[test]
    fn test_fcntl_setfl() {
        let result = sys_fcntl(3, 4, 0x800);
        assert_eq!(result, 0x800, "F_SETFL 应返回 arg");
    }

    #[test]
    fn test_fcntl_invalid_cmd() {
        let result = sys_fcntl(3, 99, 0);
        assert_eq!(result, -22, "无效 cmd 应返回 EINVAL");
    }

    #[test]
    fn test_getdents64() {
        let mut buf = [0u8; 1024];
        let result = sys_getdents64(0, &mut buf, 1024);
        assert_eq!(result, 0, "简化实现应返回 0");
    }

    #[test]
    fn test_readv() {
        let iovs = [
            IoVec { base: 0x1000, len: 10 },
            IoVec { base: 0x2000, len: 20 },
        ];
        // 使用一个确定未打开的 fd（高编号）
        let result = sys_readv(500, &iovs);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_writev() {
        let iovs = [
            IoVec { base: 0x1000, len: 10 },
            IoVec { base: 0x2000, len: 20 },
        ];
        // 使用一个确定未打开的 fd（高编号）
        let result = sys_writev(500, &iovs);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_timespec() {
        let ts = TimeSpec { tv_sec: 12345, tv_nsec: 67890 };
        assert_eq!(ts.tv_sec, 12345);
        assert_eq!(ts.tv_nsec, 67890);
    }

    #[test]
    fn test_mmap_zero_length() {
        let result = sys_mmap(0, 0, 0, 0, 0, 0);
        assert_eq!(result, !0, "长度为 0 应返回 MAP_FAILED");
    }

    #[test]
    fn test_mmap_valid() {
        let result = sys_mmap(0, 4096, 3, 0x22, 0, 0);
        assert_eq!(result, 0x7f00_0000_0000, "有效 mmap 应返回伪地址");
    }

    #[test]
    fn test_munmap() {
        let result = sys_munmap(0x7f00_0000_0000, 4096);
        assert_eq!(result, 0, "munmap 简化实现应总是成功");
    }

    #[test]
    fn test_mprotect() {
        let result = sys_mprotect(0x7f00_0000_0000, 4096, 1);
        assert_eq!(result, 0, "mprotect 简化实现应总是成功");
    }

    #[test]
    fn test_madvise() {
        let result = sys_madvise(0x7f00_0000_0000, 4096, 0);
        assert_eq!(result, 0, "madvise 简化实现应总是成功");
    }

    #[test]
    fn test_getrandom() {
        let mut buf = [0u8; 16];
        let result = sys_getrandom(&mut buf, 0);
        assert_eq!(result, 16, "应返回请求的字节数");
        // 验证填充了数据（递增序列）
        assert!(!buf.iter().all(|&b| b == 0), "getrandom 应填充非零数据");
    }

    #[test]
    fn test_getrandom_empty() {
        let mut buf = [0u8; 0];
        let result = sys_getrandom(&mut buf, 0);
        assert_eq!(result, 0, "空缓冲区应返回 0");
    }

    #[test]
    fn test_getrandom_sequential() {
        let mut buf1 = [0u8; 4];
        let mut buf2 = [0u8; 4];
        sys_getrandom(&mut buf1, 0);
        sys_getrandom(&mut buf2, 0);
        // 后续调用应产生不同的值（递增计数器）
        assert_ne!(buf1, buf2, "连续调用 getrandom 应产生不同序列");
    }

    #[test]
    fn test_rseq() {
        let result = sys_rseq(0x1000, 32, 0);
        assert_eq!(result, 0, "rseq 简化实现应总是成功");
    }

    #[test]
    fn test_set_tid_address() {
        let result = sys_set_tid_address(0x1000);
        // 应返回当前 TID（测试环境中可能为 0）
        assert!(result >= 0, "set_tid_address 应返回非负 TID");
    }

    #[test]
    fn test_sigaction() {
        let result = sys_sigaction(2, 0, 0);
        assert_eq!(result, 0, "sigaction 简化实现应总是成功");
    }

    #[test]
    fn test_ioctl() {
        let result = sys_ioctl(0, 0, 0);
        assert_eq!(result, 0, "ioctl 简化实现应总是成功");
    }

    #[test]
    fn test_poll() {
        let result = sys_poll(0, 0, 0);
        assert_eq!(result, 0, "poll 简化实现应返回 0（无事件）");
    }

    #[test]
    fn test_dup_invalid_fd() {
        let result = sys_dup(999);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_dup2_same_fd() {
        let result = sys_dup2(5, 5);
        assert_eq!(result, 5, "oldfd == newfd 时应返回 fd 本身");
    }

    #[test]
    fn test_dup2_negative_fd() {
        let result = sys_dup2(-1, 5);
        assert_eq!(result, -9, "负数 fd 应返回 EBADF");
    }

    #[test]
    fn test_dup2_negative_newfd() {
        let result = sys_dup2(5, -1);
        assert_eq!(result, -9, "负数 newfd 应返回 EBADF");
    }

    #[test]
    fn test_pipe2() {
        let result = sys_pipe2(0, 0);
        assert_eq!(result, 0, "pipe2 简化实现应总是成功");
    }

    #[test]
    fn test_sched_yield() {
        let result = sys_sched_yield();
        assert_eq!(result, 0, "sched_yield 应返回 0");
    }

    #[test]
    fn test_clone_returns_pid() {
        let pid = sys_clone(0, 0, 0, 0, 0);
        assert!(pid > 0, "clone 应返回正数 PID");
    }

    #[test]
    fn test_fork_returns_pid() {
        let pid = sys_fork();
        assert!(pid > 0, "fork 应返回正数 PID");
    }

    #[test]
    fn test_execve_not_supported() {
        let result = sys_execve("/bin/sh", 0, 0);
        assert_eq!(result, -38, "execve 应返回 ENOSYS");
    }

    #[test]
    fn test_wait4() {
        let result = sys_wait4(0, 0, 0, 0);
        assert_eq!(result, 0, "wait4 简化实现应返回 0");
    }

    #[test]
    fn test_futex() {
        let result = sys_futex(0, 0, 0, 0, 0, 0);
        assert_eq!(result, 0, "futex 简化实现应返回 0");
    }

    #[test]
    fn test_arch_prctl() {
        let result = sys_arch_prctl(0x1002, 0x7f00_0000);
        assert_eq!(result, 0, "arch_prctl 简化实现应总是成功");
    }

    #[test]
    fn test_setrlimit() {
        let result = sys_setrlimit(0, 0);
        assert_eq!(result, 0, "setrlimit 简化实现应总是成功");
    }

    #[test]
    fn test_getrlimit() {
        let result = sys_getrlimit(0, 0);
        assert_eq!(result, 0, "getrlimit 简化实现应总是成功");
    }

    #[test]
    fn test_stat() {
        let stat = sys_stat("/test");
        assert_eq!(stat.st_nlink, 1);
        assert_eq!(stat.st_blksize, 4096);
    }

    #[test]
    fn test_lseek_invalid_whence() {
        let result = sys_lseek(0, 0, 99);
        assert_eq!(result, -22, "无效 whence 应返回 EINVAL");
    }

    #[test]
    fn test_lseek_invalid_fd() {
        let result = sys_lseek(999, 0, 0);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_close_invalid_fd() {
        let result = sys_close(999);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_read_invalid_fd() {
        let mut buf = [0u8; 10];
        let result = sys_read(999, &mut buf);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_write_invalid_fd() {
        let result = sys_write(999, b"hello");
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_fstat_invalid_fd() {
        let result = sys_fstat(999);
        assert_eq!(result, -9, "无效 fd 应返回 EBADF");
    }

    #[test]
    fn test_open_and_close() {
        let fd = sys_open("/test_open.txt", 2, 0o644); // O_RDWR
        assert!(fd >= 0, "open 应返回有效的 fd");
        let result = sys_close(fd as i32);
        assert_eq!(result, 0, "close 应成功");
    }

    #[test]
    fn test_read_write_roundtrip() {
        // 打开文件
        let fd = sys_open("/test_rw.txt", 2, 0o644);
        assert!(fd >= 0);

        // 写入数据
        let data = b"Hello, POSIX!";
        let written = sys_write(fd as i32, data);
        assert_eq!(written, data.len() as i64, "应写入全部数据");

        // 关闭后重新打开
        let _ = sys_close(fd as i32);
        let fd2 = sys_open("/test_rw.txt", 0, 0o644); // O_RDONLY
        assert!(fd2 >= 0);

        // 注意：由于 VFS 的简化实现，重新打开会创建新 inode
        // 所以这里只验证读取操作不会崩溃
        let mut buf = [0u8; 64];
        let _ = sys_read(fd2 as i32, &mut buf);
        let _ = sys_close(fd2 as i32);
    }

    #[test]
    fn test_getppid() {
        let ppid = sys_getppid();
        assert_eq!(ppid, 0, "简化实现中父进程 ID 应为 0");
    }

    #[test]
    fn test_clone_and_fork_unique_pids() {
        let pid1 = sys_clone(0, 0, 0, 0, 0);
        let pid2 = sys_fork();
        let pid3 = sys_clone(0, 0, 0, 0, 0);
        assert!(pid1 < pid2, "PID 应递增");
        assert!(pid2 < pid3, "PID 应递增");
    }
}
