//! 系统调用分发器
//!
//! 从寄存器读取 syscall 号和参数，路由到对应处理函数。
//! 支持 Agent 系统调用的完整分发，传统系统调用路由到 POSIX 模块，
//! 虚拟化系统调用暂未实现，返回 E_NOTSUP。

use crate::syscall::abi::*;
use crate::syscall::numbers::*;
use crate::syscall::posix;
use crate::agent::pool::AgentPool;
use crate::agent::communication::CommManager;
use core::sync::atomic::{AtomicU64, Ordering};

/// 全局 Agent 池
/// 使用 Mutex<Option<...>> 模式支持测试中重置
static AGENT_POOL: spin::Mutex<Option<AgentPool>> = spin::Mutex::new(None);

/// 全局通信管理器
static COMM_MANAGER: spin::Mutex<Option<CommManager>> = spin::Mutex::new(None);

/// Syscall 计数器
static SYSCALL_COUNT: AtomicU64 = AtomicU64::new(0);

/// Syscall 参数 (从寄存器读取)
///
/// x86_64 System V ABI 寄存器映射:
/// - rax: syscall 号
/// - rdi: 第 1 个参数
/// - rsi: 第 2 个参数
/// - rdx: 第 3 个参数
/// - r10: 第 4 个参数
/// - r8:  第 5 个参数
/// - r9:  第 6 个参数
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SyscallArgs {
    pub number: u64,    // rax
    pub arg1: u64,      // rdi
    pub arg2: u64,      // rsi
    pub arg3: u64,      // rdx
    pub arg4: u64,      // r10
    pub arg5: u64,      // r8
    pub arg6: u64,      // r9
}

/// 初始化 syscall 子系统
///
/// 创建并初始化 Agent 池和通信管理器。
/// 可以多次调用 (幂等)，每次都会重新创建。
pub fn init() {
    {
        let mut pool = AGENT_POOL.lock();
        let new_pool = AgentPool::new();
        new_pool.init();
        *pool = Some(new_pool);
    }
    {
        let mut comm = COMM_MANAGER.lock();
        *comm = Some(CommManager::new());
    }
}

/// 重置 syscall 子系统 (仅用于测试)
///
/// 清除所有全局状态，使下一个 init() 调用重新初始化。
#[cfg(test)]
pub fn reset() {
    *AGENT_POOL.lock() = None;
    *COMM_MANAGER.lock() = None;
    SYSCALL_COUNT.store(0, Ordering::Relaxed);
}

/// 获取当前 syscall 计数 (用于测试和调试)
pub fn syscall_count() -> u64 {
    SYSCALL_COUNT.load(Ordering::Relaxed)
}

/// 重置 syscall 计数 (仅用于测试)
#[cfg(test)]
pub fn reset_syscall_count() {
    SYSCALL_COUNT.store(0, Ordering::Relaxed);
}

/// Syscall 分发入口
///
/// 根据 syscall 号将请求路由到对应的处理函数。
///
/// # Safety
/// 调用者必须确保指针参数有效。本函数会验证指针非空，
/// 但不会验证指针指向的内存区域的完整有效性。
pub unsafe fn dispatch(args: &SyscallArgs) -> i64 {
    SYSCALL_COUNT.fetch_add(1, Ordering::Relaxed);

    let result = match args.number {
        // === 传统系统调用（路由到 POSIX 模块）===
        SYS_READ => {
            // read(fd, buf, count) — 简化：返回 0
            0
        }
        SYS_WRITE => {
            // write(fd, buf, count) — 简化：返回写入字节数
            args.arg3 as i64
        }
        SYS_OPEN => {
            // open(pathname, flags, mode) — 简化：返回新 fd
            posix::sys_open("ignored", args.arg2 as i32, args.arg3 as u32)
        }
        SYS_CLOSE => {
            // close(fd) — 简化：总是成功
            0
        }
        SYS_STAT => {
            // stat(pathname, statbuf) — 简化：总是成功
            0
        }
        SYS_FSTAT => {
            // fstat(fd, statbuf) — 简化：总是成功
            0
        }
        SYS_LSTAT => {
            // lstat(pathname, statbuf) — 简化：总是成功
            0
        }
        SYS_POLL => posix::sys_poll(args.arg1, args.arg2 as u32, args.arg3 as i32),
        SYS_LSEEK => posix::sys_lseek(args.arg1 as i32, args.arg2 as i64, args.arg3 as i32),
        SYS_MMAP => posix::sys_mmap(args.arg1, args.arg2, args.arg3 as i32, args.arg4 as i32, args.arg5 as i32, args.arg6) as i64,
        SYS_MUNMAP => posix::sys_munmap(args.arg1, args.arg2),
        SYS_MPROTECT => posix::sys_mprotect(args.arg1, args.arg2, args.arg3 as i32),
        SYS_BRK => posix::sys_brk(args.arg1) as i64,
        SYS_IOCTL => posix::sys_ioctl(args.arg1 as i32, args.arg2, args.arg3),
        SYS_WRITEV => {
            // writev — 简化：返回 0
            0
        }
        SYS_READV => {
            // readv — 简化：返回 0
            0
        }
        SYS_MADVISE => posix::sys_madvise(args.arg1, args.arg2, args.arg3 as i32),
        SYS_GETPID => posix::sys_getpid(),
        SYS_FORK => posix::sys_fork(),
        SYS_EXECVE => posix::sys_execve("ignored", args.arg2, args.arg3),
        SYS_EXIT => posix::sys_exit(args.arg1 as i32),
        SYS_SET_TID_ADDRESS => posix::sys_set_tid_address(args.arg1),
        SYS_SIGACTION => posix::sys_sigaction(args.arg1 as i32, args.arg2, args.arg3),
        SYS_FUTEX => posix::sys_futex(args.arg1, args.arg2 as i32, args.arg3, args.arg4, args.arg5, args.arg6),
        SYS_CLOCK_GETTIME => {
            // clock_gettime — 简化：返回 0
            0
        }
        SYS_WAIT4 => posix::sys_wait4(args.arg1 as i32, args.arg2, args.arg3 as i32, args.arg4),
        SYS_GETRANDOM => {
            // getrandom — 简化：返回 0
            0
        }
        SYS_RSEQ => posix::sys_rseq(args.arg1, args.arg2 as u32, args.arg3 as i32),

        // === Agent 系统调用 ===
        SYS_AGENT_SPAWN => handle_agent_spawn(args),
        SYS_AGENT_KILL => handle_agent_kill(args),
        SYS_AGENT_QUERY => handle_agent_query(args),
        SYS_AGENT_MSG => handle_agent_msg(args),
        SYS_AGENT_REGISTER => handle_agent_register(args),
        SYS_AGENT_SUBSCRIBE => handle_agent_subscribe(args),
        SYS_AGENT_MIGRATE => handle_agent_migrate(args),
        SYS_AGENT_MEMORY_SHARE => handle_agent_memory_share(args),
        SYS_AGENT_CAP_GRANT => handle_agent_cap_grant(args),
        SYS_AGENT_CAP_REVOKE => handle_agent_cap_revoke(args),
        SYS_AGENT_BIND_PORT => handle_agent_bind_port(args),
        SYS_AGENT_EXPORT => handle_agent_export(args),
        SYS_AGENT_IMPORT => handle_agent_import(args),
        SYS_AGENT_SET_QUOTA => handle_agent_set_quota(args),
        SYS_AGENT_GET_QUOTA => handle_agent_get_quota(args),
        SYS_AGENT_SNAPSHOT => handle_agent_snapshot(args),
        SYS_AGENT_RESTORE => handle_agent_restore(args),

        // === 虚拟化系统调用 (暂未实现) ===
        SYS_VM_CREATE | SYS_VM_START | SYS_VM_STOP | SYS_VM_PAUSE |
        SYS_VM_RESUME | SYS_VM_MAP_MEMORY | SYS_VM_IO_PORT => {
            E_NOTSUP as i64
        }

        _ => E_INVAL as i64,
    };

    result
}

// ============================================================================
// Agent Syscall 处理函数
// ============================================================================

/// 创建新 Agent
///
/// 参数:
/// - arg1: *const AgentSpec - Agent 规格描述符指针
/// - arg2: spec_len - 规格描述符长度 (必须 >= size_of::<AgentSpec>())
/// - arg3: cap_slot - 能力槽位 (保留，暂未使用)
///
/// 返回: 成功时返回 AgentHandle 值，失败时返回负错误码
unsafe fn handle_agent_spawn(args: &SyscallArgs) -> i64 {
    let spec_ptr = args.arg1 as *const AgentSpec;
    let spec_len = args.arg2 as usize;
    // cap_slot (arg3) 暂未使用

    // 验证指针非空
    if spec_ptr.is_null() {
        return E_FAULT as i64;
    }

    // 验证 spec_len >= size_of::<AgentSpec>()
    if spec_len < core::mem::size_of::<AgentSpec>() {
        return E_INVAL as i64;
    }

    // 读取 AgentSpec
    let spec = &*spec_ptr;

    // 验证版本号
    if spec.version != 1 {
        return E_INVAL as i64;
    }

    // 调用 Agent 池创建 Agent (creator_pid 暂用 0)
    let pool_guard = AGENT_POOL.lock();
    let pool = pool_guard.as_ref().expect("Agent 池未初始化");
    match pool.spawn(spec, 0) {
        Ok(handle) => handle.0 as i64,
        Err(e) => error_to_i64(e),
    }
}

/// 终止 Agent
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: signal - 终止信号 (0=强制终止, 1=优雅终止)
///
/// 返回: 成功时返回 0，失败时返回负错误码
unsafe fn handle_agent_kill(args: &SyscallArgs) -> i64 {
    let handle = AgentHandle(args.arg1);
    let signal = args.arg2 as u32;

    let pool_guard = AGENT_POOL.lock();
    let pool = pool_guard.as_ref().expect("Agent 池未初始化");
    match pool.kill(handle, signal) {
        Ok(()) => E_OK as i64,
        Err(e) => error_to_i64(e),
    }
}

/// 查询 Agent 信息
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: *mut AgentInfo - 输出缓冲区指针
/// - arg3: info_len - 缓冲区长度 (必须 >= size_of::<AgentInfo>())
///
/// 返回: 成功时返回 0，失败时返回负错误码
unsafe fn handle_agent_query(args: &SyscallArgs) -> i64 {
    let handle = AgentHandle(args.arg1);
    let info_ptr = args.arg2 as *mut AgentInfo;
    let info_len = args.arg3 as usize;

    // 验证指针非空
    if info_ptr.is_null() {
        return E_FAULT as i64;
    }

    // 验证缓冲区长度
    if info_len < core::mem::size_of::<AgentInfo>() {
        return E_INVAL as i64;
    }

    // 查询 Agent 信息
    let pool_guard = AGENT_POOL.lock();
    let pool = pool_guard.as_ref().expect("Agent 池未初始化");
    match pool.query(handle) {
        Ok(info) => {
            // 将 AgentInfo 写入用户态缓冲区
            *info_ptr = info;
            E_OK as i64
        }
        Err(e) => error_to_i64(e),
    }
}

/// 发送消息
///
/// 参数:
/// - arg1: src_handle - 发送者 Agent 句柄
/// - arg2: dst_handle - 接收者 Agent 句柄
/// - arg3: *const AgentMsgHeader - 消息头指针
/// - arg4: flags - 消息标志
///
/// 返回: 成功时返回消息 ID，失败时返回负错误码
unsafe fn handle_agent_msg(args: &SyscallArgs) -> i64 {
    let src_handle = AgentHandle(args.arg1);
    let dst_handle = AgentHandle(args.arg2);
    let header_ptr = args.arg3 as *const AgentMsgHeader;
    // flags (arg4) 暂未使用

    // 验证指针非空
    if header_ptr.is_null() {
        return E_FAULT as i64;
    }

    // 读取消息头
    let header = &*header_ptr;

    // 调用通信管理器发送消息
    let comm_guard = COMM_MANAGER.lock();
    let comm = comm_guard.as_ref().expect("通信管理器未初始化");
    match comm.send_message(src_handle, dst_handle, header) {
        Ok(msg_id) => msg_id as i64,
        Err(e) => error_to_i64(e),
    }
}

/// 注册 Agent 能力 (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - Agent 句柄
/// - arg2: cap_id - 能力 ID
/// - arg3: *const u8 - 能力数据指针
/// - arg4: data_len - 能力数据长度
unsafe fn handle_agent_register(args: &SyscallArgs) -> i64 {
    // 验证基本参数
    let _handle = AgentHandle(args.arg1);
    let _cap_id = args.arg2;

    // 参数解析框架: 验证指针参数
    let data_ptr = args.arg3 as *const u8;
    let _data_len = args.arg4 as usize;

    if !data_ptr.is_null() && _data_len == 0 {
        return E_INVAL as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 订阅事件
///
/// 参数:
/// - arg1: subscriber_handle - 订阅者 Agent 句柄
/// - arg2: target_handle - 目标 Agent 句柄
/// - arg3: *const EventMask - 事件掩码指针
///
/// 返回: 成功时返回 0，失败时返回负错误码
unsafe fn handle_agent_subscribe(args: &SyscallArgs) -> i64 {
    let subscriber = AgentHandle(args.arg1);
    let _target = AgentHandle(args.arg2);
    let mask_ptr = args.arg3 as *const EventMask;

    // 验证指针非空
    if mask_ptr.is_null() {
        return E_FAULT as i64;
    }

    // 读取事件掩码
    let mask = &*mask_ptr;

    // 简化实现: 使用通信管理器记录订阅关系
    // 使用目标句柄的数值作为主题名称
    let topic_bytes = _target.0.to_le_bytes();
    let comm_guard = COMM_MANAGER.lock();
    let comm = comm_guard.as_ref().expect("通信管理器未初始化");
    match comm.subscribe(subscriber, &topic_bytes, mask) {
        Ok(()) => E_OK as i64,
        Err(e) => error_to_i64(e),
    }
}

/// 迁移 Agent (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 源 Agent 句柄
/// - arg2: *const MigrationToken - 迁移令牌指针
/// - arg3: flags - 迁移标志
unsafe fn handle_agent_migrate(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let token_ptr = args.arg2 as *const MigrationToken;
    let _flags = args.arg3 as u32;

    // 参数解析框架: 验证指针
    if token_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 共享内存 (暂未实现)
///
/// 参数:
/// - arg1: src_handle - 源 Agent 句柄
/// - arg2: dst_handle - 目标 Agent 句柄
/// - arg3: *const ShmSpec - 共享内存规格指针
unsafe fn handle_agent_memory_share(args: &SyscallArgs) -> i64 {
    let _src = AgentHandle(args.arg1);
    let _dst = AgentHandle(args.arg2);
    let spec_ptr = args.arg3 as *const ShmSpec;

    // 参数解析框架: 验证指针
    if spec_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 授予能力 (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: cap - 能力位索引
/// - arg3: grant - 是否授予 (非零=授予, 零=不操作)
unsafe fn handle_agent_cap_grant(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let _cap = args.arg2 as usize;
    let _grant = args.arg3;

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 撤销能力 (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: cap - 能力位索引
unsafe fn handle_agent_cap_revoke(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let _cap = args.arg2 as usize;

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 绑定端口 (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - Agent 句柄
/// - arg2: port_id - 端口 ID
/// - arg3: flags - 绑定标志
unsafe fn handle_agent_bind_port(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let _port_id = args.arg2;
    let _flags = args.arg3 as u32;

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 导出 Agent (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 源 Agent 句柄
/// - arg2: *const u8 - 导出数据指针
/// - arg3: data_len - 导出数据长度
unsafe fn handle_agent_export(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let data_ptr = args.arg2 as *const u8;
    let _data_len = args.arg3 as usize;

    if data_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 导入 Agent (暂未实现)
///
/// 参数:
/// - arg1: *const u8 - 导入数据指针
/// - arg2: data_len - 导入数据长度
/// - arg3: flags - 导入标志
unsafe fn handle_agent_import(args: &SyscallArgs) -> i64 {
    let data_ptr = args.arg1 as *const u8;
    let _data_len = args.arg2 as usize;
    let _flags = args.arg3 as u32;

    if data_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 设置资源配额 (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: *const ResourceQuota - 配额规格指针
unsafe fn handle_agent_set_quota(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let quota_ptr = args.arg2 as *const ResourceQuota;

    if quota_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 获取资源配额 (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: *mut ResourceQuota - 输出缓冲区指针
unsafe fn handle_agent_get_quota(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let quota_ptr = args.arg2 as *mut ResourceQuota;

    if quota_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 快照 Agent (暂未实现)
///
/// 参数:
/// - arg1: AgentHandle - 目标 Agent 句柄
/// - arg2: *const u8 - 快照目标路径指针
/// - arg3: path_len - 路径长度
unsafe fn handle_agent_snapshot(args: &SyscallArgs) -> i64 {
    let _handle = AgentHandle(args.arg1);
    let path_ptr = args.arg2 as *const u8;
    let _path_len = args.arg3 as usize;

    if path_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

/// 恢复 Agent (暂未实现)
///
/// 参数:
/// - arg1: *const u8 - 快照源路径指针
/// - arg2: path_len - 路径长度
/// - arg3: flags - 恢复标志
unsafe fn handle_agent_restore(args: &SyscallArgs) -> i64 {
    let path_ptr = args.arg1 as *const u8;
    let _path_len = args.arg2 as usize;
    let _flags = args.arg3 as u32;

    if path_ptr.is_null() {
        return E_FAULT as i64;
    }

    // Phase 3+ 实现
    E_NOTSUP as i64
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 将 SyscallError 转换为 i64 错误码
fn error_to_i64(e: SyscallError) -> i64 {
    match e {
        SyscallError::EINVAL => E_INVAL as i64,
        SyscallError::ESRCH => E_SRCH as i64,
        SyscallError::EAGAIN => E_AGAIN as i64,
        SyscallError::ENOMEM => E_NOMEM as i64,
        SyscallError::EFAULT => E_FAULT as i64,
        SyscallError::EACCES => E_ACCES as i64,
        SyscallError::EPERM => E_PERM as i64,
        SyscallError::ENOENT => E_NOENT as i64,
        SyscallError::EBUSY => E_BUSY as i64,
        SyscallError::EEXIST => E_EXIST as i64,
        SyscallError::ENOTSUP => E_NOTSUP as i64,
        SyscallError::EAGENT_QUEUE_FULL => E_BUSY as i64,
        SyscallError::EAGENT_MIGRATING => E_BUSY as i64,
        _ => E_INVAL as i64,
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用 AgentSpec
    fn make_test_spec(name: &str) -> AgentSpec {
        let mut spec = AgentSpec::default();
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(63);
        spec.name[..len].copy_from_slice(&name_bytes[..len]);
        spec.version = 1;
        spec.agent_type = AgentType::Generic;
        spec
    }

    /// 创建默认 SyscallArgs (仅设置 number)
    fn make_args(number: u64) -> SyscallArgs {
        SyscallArgs {
            number,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        }
    }

    // === SyscallArgs 结构体布局测试 ===
    #[test]
    fn test_syscall_args_layout() {
        // SyscallArgs 必须是 8 个 u64 (56 字节)
        assert_eq!(
            core::mem::size_of::<SyscallArgs>(),
            56,
            "SyscallArgs 大小必须为 56 字节 (8 x u64)"
        );

        // 验证 repr(C) 布局: 字段按声明顺序排列
        let args = SyscallArgs {
            number: 100,
            arg1: 1,
            arg2: 2,
            arg3: 3,
            arg4: 4,
            arg5: 5,
            arg6: 6,
        };

        assert_eq!(args.number, 100);
        assert_eq!(args.arg1, 1);
        assert_eq!(args.arg2, 2);
        assert_eq!(args.arg3, 3);
        assert_eq!(args.arg4, 4);
        assert_eq!(args.arg5, 5);
        assert_eq!(args.arg6, 6);

        // 验证 Clone 和 Copy
        let args2 = args;
        assert_eq!(args2.number, 100);
        let args3 = args.clone();
        assert_eq!(args3.number, 100);
    }

    // === 无效 syscall 号返回 E_INVAL ===
    #[test]
    fn test_dispatch_invalid_syscall() {
        reset();
        init();

        // 使用一个不存在的 syscall 号 (不在任何区间内)
        let args = make_args(9999);
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_INVAL as i64);

        // 使用另一个无效号
        let args = make_args(1024);
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_INVAL as i64);
    }

    // === 传统 syscall 路由到 POSIX 模块 ===
    #[test]
    fn test_dispatch_traditional_syscalls() {
        reset();
        init();

        // 这些 syscall 现在路由到 posix 模块，不再返回 E_NOTSUP
        // 验证它们不再返回 E_NOTSUP
        let traditional_syscalls = [
            SYS_READ, SYS_WRITE, SYS_OPEN, SYS_CLOSE, SYS_STAT,
            SYS_FSTAT, SYS_LSTAT, SYS_POLL, SYS_LSEEK, SYS_MMAP,
            SYS_MUNMAP, SYS_MPROTECT, SYS_BRK, SYS_IOCTL, SYS_WRITEV,
            SYS_READV, SYS_MADVISE, SYS_GETPID, SYS_FORK, SYS_EXECVE,
            SYS_SET_TID_ADDRESS, SYS_SIGACTION, SYS_FUTEX,
            SYS_CLOCK_GETTIME, SYS_WAIT4, SYS_GETRANDOM, SYS_RSEQ,
        ];

        for &sysno in &traditional_syscalls {
            let args = make_args(sysno);
            let result = unsafe { dispatch(&args) };
            assert_ne!(
                result, E_NOTSUP as i64,
                "传统 syscall {} 不应再返回 E_NOTSUP（已路由到 POSIX 模块）",
                sysno
            );
        }
    }

    // === 虚拟化 syscall 返回 E_NOTSUP ===
    #[test]
    fn test_dispatch_unimplemented_vm() {
        reset();
        init();

        let vm_syscalls = [
            SYS_VM_CREATE, SYS_VM_START, SYS_VM_STOP, SYS_VM_PAUSE,
            SYS_VM_RESUME, SYS_VM_MAP_MEMORY, SYS_VM_IO_PORT,
        ];

        for &sysno in &vm_syscalls {
            let args = make_args(sysno);
            let result = unsafe { dispatch(&args) };
            assert_eq!(
                result, E_NOTSUP as i64,
                "虚拟化 syscall {} 应返回 E_NOTSUP",
                sysno
            );
        }
    }

    // === Agent 创建测试 ===
    #[test]
    fn test_dispatch_agent_spawn() {
        reset();
        init();

        let spec = make_test_spec("spawn_test");
        let spec_len = core::mem::size_of::<AgentSpec>();

        let args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: spec_len as u64,
            arg3: 0, // cap_slot
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert!(result > 0, "Agent 创建应返回正数句柄，实际为 {}", result);
    }

    // === Agent 创建: 空指针 ===
    #[test]
    fn test_dispatch_agent_spawn_null_ptr() {
        reset();
        init();

        let args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: 0, // 空指针
            arg2: core::mem::size_of::<AgentSpec>() as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);
    }

    // === Agent 创建: spec_len 太小 ===
    #[test]
    fn test_dispatch_agent_spawn_small_len() {
        reset();
        init();

        let spec = make_test_spec("test");

        let args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: 10, // 远小于 size_of::<AgentSpec>()
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_INVAL as i64);
    }

    // === Agent 创建: 版本号错误 ===
    #[test]
    fn test_dispatch_agent_spawn_bad_version() {
        reset();
        init();

        let mut spec = make_test_spec("bad_version");
        spec.version = 99; // 错误版本号

        let args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: core::mem::size_of::<AgentSpec>() as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_INVAL as i64);
    }

    // === Agent 查询测试 ===
    #[test]
    fn test_dispatch_agent_query() {
        reset();
        init();

        // 先创建一个 Agent
        let spec = make_test_spec("query_test");
        let spec_len = core::mem::size_of::<AgentSpec>();

        let spawn_args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: spec_len as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let handle_val = unsafe { dispatch(&spawn_args) };
        assert!(handle_val > 0);

        // 查询该 Agent
        let mut info: AgentInfo = AgentInfo::default();
        let query_args = SyscallArgs {
            number: SYS_AGENT_QUERY,
            arg1: handle_val as u64,
            arg2: &mut info as *mut AgentInfo as u64,
            arg3: core::mem::size_of::<AgentInfo>() as u64,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&query_args) };
        assert_eq!(result, E_OK as i64);
        assert_eq!(info.handle, AgentHandle(handle_val as u64));
        assert_eq!(info.state, AgentState::Creating);

        // 验证名称
        let name_str = core::str::from_utf8(&info.name).unwrap_or("");
        assert!(name_str.starts_with("query_test"));
    }

    // === Agent 查询: 空指针 ===
    #[test]
    fn test_dispatch_agent_query_null_ptr() {
        reset();
        init();

        let args = SyscallArgs {
            number: SYS_AGENT_QUERY,
            arg1: 1, // 有效句柄
            arg2: 0, // 空指针
            arg3: core::mem::size_of::<AgentInfo>() as u64,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);
    }

    // === Agent 查询: 缓冲区太小 ===
    #[test]
    fn test_dispatch_agent_query_small_buffer() {
        reset();
        init();

        let mut info: AgentInfo = AgentInfo::default();

        let args = SyscallArgs {
            number: SYS_AGENT_QUERY,
            arg1: 1,
            arg2: &mut info as *mut AgentInfo as u64,
            arg3: 10, // 远小于 size_of::<AgentInfo>()
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_INVAL as i64);
    }

    // === Agent 终止测试 ===
    #[test]
    fn test_dispatch_agent_kill() {
        reset();
        init();

        // 先创建一个 Agent
        let spec = make_test_spec("kill_test");
        let spawn_args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: core::mem::size_of::<AgentSpec>() as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let handle_val = unsafe { dispatch(&spawn_args) };
        assert!(handle_val > 0);

        // 终止该 Agent (强制终止, signal=0)
        let kill_args = SyscallArgs {
            number: SYS_AGENT_KILL,
            arg1: handle_val as u64,
            arg2: 0, // SIG_KILL
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&kill_args) };
        assert_eq!(result, E_OK as i64);

        // 再次查询应失败 (Agent 已被移除)
        let mut info: AgentInfo = AgentInfo::default();
        let query_args = SyscallArgs {
            number: SYS_AGENT_QUERY,
            arg1: handle_val as u64,
            arg2: &mut info as *mut AgentInfo as u64,
            arg3: core::mem::size_of::<AgentInfo>() as u64,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&query_args) };
        assert_eq!(result, E_SRCH as i64);
    }

    // === Agent 终止: 无效句柄 ===
    #[test]
    fn test_dispatch_agent_kill_invalid_handle() {
        reset();
        init();

        let args = SyscallArgs {
            number: SYS_AGENT_KILL,
            arg1: 0, // 无效句柄
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_INVAL as i64);
    }

    // === 消息发送测试 ===
    #[test]
    fn test_dispatch_agent_msg() {
        reset();
        init();

        // 创建两个 Agent
        let spec1 = make_test_spec("sender");
        let spec2 = make_test_spec("receiver");
        let spec_len = core::mem::size_of::<AgentSpec>();

        let spawn_args1 = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec1 as *const AgentSpec as u64,
            arg2: spec_len as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let src_handle = unsafe { dispatch(&spawn_args1) };
        assert!(src_handle > 0);

        let spawn_args2 = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec2 as *const AgentSpec as u64,
            arg2: spec_len as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let dst_handle = unsafe { dispatch(&spawn_args2) };
        assert!(dst_handle > 0);

        // 发送消息
        let header = AgentMsgHeader {
            msg_type: 1,
            flags: MSG_SYNC,
            payload_size: 128,
            ..AgentMsgHeader::default()
        };

        let msg_args = SyscallArgs {
            number: SYS_AGENT_MSG,
            arg1: src_handle as u64,
            arg2: dst_handle as u64,
            arg3: &header as *const AgentMsgHeader as u64,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&msg_args) };
        assert!(result > 0, "消息发送应返回正数 msg_id，实际为 {}", result);
    }

    // === 消息发送: 空指针 ===
    #[test]
    fn test_dispatch_agent_msg_null_ptr() {
        reset();
        init();

        let args = SyscallArgs {
            number: SYS_AGENT_MSG,
            arg1: 1,
            arg2: 2,
            arg3: 0, // 空指针
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);
    }

    // === 事件订阅测试 ===
    #[test]
    fn test_dispatch_agent_subscribe() {
        reset();
        init();

        // 创建两个 Agent
        let spec1 = make_test_spec("subscriber");
        let spec2 = make_test_spec("target");
        let spec_len = core::mem::size_of::<AgentSpec>();

        let spawn_args1 = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec1 as *const AgentSpec as u64,
            arg2: spec_len as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let sub_handle = unsafe { dispatch(&spawn_args1) };
        assert!(sub_handle > 0);

        let spawn_args2 = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec2 as *const AgentSpec as u64,
            arg2: spec_len as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let tgt_handle = unsafe { dispatch(&spawn_args2) };
        assert!(tgt_handle > 0);

        // 订阅事件
        let mask = EventMask::ALL;
        let sub_args = SyscallArgs {
            number: SYS_AGENT_SUBSCRIBE,
            arg1: sub_handle as u64,
            arg2: tgt_handle as u64,
            arg3: &mask as *const EventMask as u64,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&sub_args) };
        assert_eq!(result, E_OK as i64);
    }

    // === 事件订阅: 空指针 ===
    #[test]
    fn test_dispatch_agent_subscribe_null_ptr() {
        reset();
        init();

        let args = SyscallArgs {
            number: SYS_AGENT_SUBSCRIBE,
            arg1: 1,
            arg2: 2,
            arg3: 0, // 空指针
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);
    }

    // === Syscall 计数器递增测试 ===
    #[test]
    fn test_syscall_count() {
        reset();
        init();

        let before = syscall_count();

        // 发送一个传统 syscall
        let args = make_args(SYS_READ);
        let _ = unsafe { dispatch(&args) };
        assert_eq!(syscall_count(), before + 1);

        // 发送一个无效 syscall
        let args = make_args(9999);
        let _ = unsafe { dispatch(&args) };
        assert_eq!(syscall_count(), before + 2);

        // 发送一个 Agent syscall
        let spec = make_test_spec("count_test");
        let spawn_args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: core::mem::size_of::<AgentSpec>() as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let _ = unsafe { dispatch(&spawn_args) };
        assert_eq!(syscall_count(), before + 3);

        // 发送一个虚拟化 syscall
        let args = make_args(SYS_VM_CREATE);
        let _ = unsafe { dispatch(&args) };
        assert_eq!(syscall_count(), before + 4);
    }

    // === 初始化函数测试 ===
    #[test]
    fn test_init() {
        // reset + init 应该可以多次调用 (幂等)
        reset();
        init();
        reset();
        init();
        reset();
        init();

        // 初始化后应该能正常创建 Agent
        let spec = make_test_spec("init_test");
        let args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: core::mem::size_of::<AgentSpec>() as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&args) };
        assert!(result > 0, "初始化后应能创建 Agent");
    }

    // === 未实现的 Agent syscall 返回 E_NOTSUP ===
    #[test]
    fn test_dispatch_agent_register_unimplemented() {
        reset();
        init();

        // SYS_AGENT_REGISTER 应返回 E_NOTSUP (arg3=0 且 data_len=0 是合法的)
        let args = SyscallArgs {
            number: SYS_AGENT_REGISTER,
            arg1: 1,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64);

        // 不需要指针参数的未实现 syscall: 使用全零参数即可
        let no_ptr_unimplemented = [
            SYS_AGENT_CAP_GRANT,
            SYS_AGENT_CAP_REVOKE,
            SYS_AGENT_BIND_PORT,
        ];

        for &sysno in &no_ptr_unimplemented {
            let args = make_args(sysno);
            let result = unsafe { dispatch(&args) };
            assert_eq!(
                result, E_NOTSUP as i64,
                "未实现的 Agent syscall {} 应返回 E_NOTSUP",
                sysno
            );
        }

        // 需要指针参数的未实现 syscall: 提供有效指针绕过空指针检查
        let token = MigrationToken {
            token_id: 0,
            src_node_id: [0u8; 16],
            dest_node_id: [0u8; 16],
            timestamp_ns: 0,
            checksum: 0,
            flags: 0,
        };
        let shm_spec = ShmSpec {
            size: 0,
            src_addr: 0,
            dst_addr: 0,
            prot: 0,
            flags: 0,
        };
        let quota = ResourceQuota::default();
        let path_byte: u8 = 0;

        // SYS_AGENT_MIGRATE: 需要有效的 token 指针
        let args = SyscallArgs {
            number: SYS_AGENT_MIGRATE,
            arg1: 1,
            arg2: &token as *const MigrationToken as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_MIGRATE 应返回 E_NOTSUP");

        // SYS_AGENT_MEMORY_SHARE: 需要有效的 ShmSpec 指针
        let args = SyscallArgs {
            number: SYS_AGENT_MEMORY_SHARE,
            arg1: 1,
            arg2: 2,
            arg3: &shm_spec as *const ShmSpec as u64,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_MEMORY_SHARE 应返回 E_NOTSUP");

        // SYS_AGENT_EXPORT: 需要有效的数据指针
        let args = SyscallArgs {
            number: SYS_AGENT_EXPORT,
            arg1: 1,
            arg2: &path_byte as *const u8 as u64,
            arg3: 1,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_EXPORT 应返回 E_NOTSUP");

        // SYS_AGENT_IMPORT: 需要有效的数据指针
        let args = SyscallArgs {
            number: SYS_AGENT_IMPORT,
            arg1: &path_byte as *const u8 as u64,
            arg2: 1,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_IMPORT 应返回 E_NOTSUP");

        // SYS_AGENT_SET_QUOTA: 需要有效的 quota 指针
        let args = SyscallArgs {
            number: SYS_AGENT_SET_QUOTA,
            arg1: 1,
            arg2: &quota as *const ResourceQuota as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_SET_QUOTA 应返回 E_NOTSUP");

        // SYS_AGENT_GET_QUOTA: 需要有效的 quota 指针
        let mut out_quota = ResourceQuota::default();
        let args = SyscallArgs {
            number: SYS_AGENT_GET_QUOTA,
            arg1: 1,
            arg2: &mut out_quota as *mut ResourceQuota as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_GET_QUOTA 应返回 E_NOTSUP");

        // SYS_AGENT_SNAPSHOT: 需要有效的路径指针
        let args = SyscallArgs {
            number: SYS_AGENT_SNAPSHOT,
            arg1: 1,
            arg2: &path_byte as *const u8 as u64,
            arg3: 1,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_SNAPSHOT 应返回 E_NOTSUP");

        // SYS_AGENT_RESTORE: 需要有效的路径指针
        let args = SyscallArgs {
            number: SYS_AGENT_RESTORE,
            arg1: &path_byte as *const u8 as u64,
            arg2: 1,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_NOTSUP as i64, "SYS_AGENT_RESTORE 应返回 E_NOTSUP");
    }

    // === 未实现的 Agent syscall: 空指针验证 ===
    #[test]
    fn test_dispatch_unimplemented_null_ptr() {
        reset();
        init();

        // SYS_AGENT_MIGRATE: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_MIGRATE,
            arg1: 1,
            arg2: 0, // 空指针
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);

        // SYS_AGENT_EXPORT: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_EXPORT,
            arg1: 1,
            arg2: 0, // 空指针
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);

        // SYS_AGENT_IMPORT: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_IMPORT,
            arg1: 0, // 空指针
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);

        // SYS_AGENT_SET_QUOTA: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_SET_QUOTA,
            arg1: 1,
            arg2: 0, // 空指针
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);

        // SYS_AGENT_GET_QUOTA: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_GET_QUOTA,
            arg1: 1,
            arg2: 0, // 空指针
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);

        // SYS_AGENT_SNAPSHOT: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_SNAPSHOT,
            arg1: 1,
            arg2: 0, // 空指针
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);

        // SYS_AGENT_RESTORE: 空指针应返回 E_FAULT
        let args = SyscallArgs {
            number: SYS_AGENT_RESTORE,
            arg1: 0, // 空指针
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let result = unsafe { dispatch(&args) };
        assert_eq!(result, E_FAULT as i64);
    }

    // === error_to_i64 转换测试 ===
    #[test]
    fn test_error_to_i64() {
        assert_eq!(error_to_i64(SyscallError::EINVAL), E_INVAL as i64);
        assert_eq!(error_to_i64(SyscallError::ESRCH), E_SRCH as i64);
        assert_eq!(error_to_i64(SyscallError::EAGAIN), E_AGAIN as i64);
        assert_eq!(error_to_i64(SyscallError::ENOMEM), E_NOMEM as i64);
        assert_eq!(error_to_i64(SyscallError::EFAULT), E_FAULT as i64);
        assert_eq!(error_to_i64(SyscallError::EACCES), E_ACCES as i64);
        assert_eq!(error_to_i64(SyscallError::EPERM), E_PERM as i64);
        assert_eq!(error_to_i64(SyscallError::ENOENT), E_NOENT as i64);
        assert_eq!(error_to_i64(SyscallError::EBUSY), E_BUSY as i64);
        assert_eq!(error_to_i64(SyscallError::EEXIST), E_EXIST as i64);
        assert_eq!(error_to_i64(SyscallError::ENOTSUP), E_NOTSUP as i64);
    }

    // === Agent 创建: 优雅终止 ===
    #[test]
    fn test_dispatch_agent_kill_graceful() {
        reset();
        init();

        let spec = make_test_spec("graceful_kill");
        let spawn_args = SyscallArgs {
            number: SYS_AGENT_SPAWN,
            arg1: &spec as *const AgentSpec as u64,
            arg2: core::mem::size_of::<AgentSpec>() as u64,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };
        let handle_val = unsafe { dispatch(&spawn_args) };
        assert!(handle_val > 0);

        // 优雅终止 (signal=1)
        let kill_args = SyscallArgs {
            number: SYS_AGENT_KILL,
            arg1: handle_val as u64,
            arg2: 1, // SIG_TERM
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
        };

        let result = unsafe { dispatch(&kill_args) };
        assert_eq!(result, E_OK as i64);
    }

    // === Agent 创建: 多个 Agent 创建和查询 ===
    #[test]
    fn test_dispatch_multiple_agents() {
        reset();
        init();

        // 创建多个 Agent
        let mut handles = Vec::new();
        for i in 0..5u32 {
            let name = format!("agent_{}", i);
            let spec = make_test_spec(&name);
            let args = SyscallArgs {
                number: SYS_AGENT_SPAWN,
                arg1: &spec as *const AgentSpec as u64,
                arg2: core::mem::size_of::<AgentSpec>() as u64,
                arg3: 0,
                arg4: 0,
                arg5: 0,
                arg6: 0,
            };
            let handle = unsafe { dispatch(&args) };
            assert!(handle > 0);
            handles.push(handle);
        }

        // 查询每个 Agent
        for &h in &handles {
            let mut info: AgentInfo = AgentInfo::default();
            let args = SyscallArgs {
                number: SYS_AGENT_QUERY,
                arg1: h as u64,
                arg2: &mut info as *mut AgentInfo as u64,
                arg3: core::mem::size_of::<AgentInfo>() as u64,
                arg4: 0,
                arg5: 0,
                arg6: 0,
            };
            let result = unsafe { dispatch(&args) };
            assert_eq!(result, E_OK as i64);
            assert_eq!(info.handle, AgentHandle(h as u64));
        }
    }
}
