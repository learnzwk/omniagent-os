//! 系统调用编号定义
//!
//! 定义 OmniAgent OS 的所有系统调用编号，分为三个区间:
//! - 传统系统调用: 0-511 (兼容 Linux x86_64 编号)
//! - Agent 系统调用: 512-528 (Agent 生命周期管理)
//! - 虚拟化系统调用: 576-582 (虚拟机管理)

// === 传统系统调用 (0-511) ===
// 编号与 Linux x86_64 系统调用保持兼容
pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_OPEN: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_STAT: u64 = 4;
pub const SYS_FSTAT: u64 = 5;
pub const SYS_LSTAT: u64 = 6;
pub const SYS_POLL: u64 = 7;
pub const SYS_LSEEK: u64 = 8;
pub const SYS_MMAP: u64 = 9;
pub const SYS_MUNMAP: u64 = 10;
pub const SYS_MPROTECT: u64 = 11;
pub const SYS_BRK: u64 = 12;
pub const SYS_IOCTL: u64 = 16;
pub const SYS_WRITEV: u64 = 20;
pub const SYS_READV: u64 = 21;
pub const SYS_MADVISE: u64 = 28;
pub const SYS_GETPID: u64 = 39;
pub const SYS_FORK: u64 = 57;
pub const SYS_EXECVE: u64 = 59;
pub const SYS_EXIT: u64 = 60;
pub const SYS_SET_TID_ADDRESS: u64 = 96;
pub const SYS_SIGACTION: u64 = 131;
pub const SYS_FUTEX: u64 = 202;
pub const SYS_CLOCK_GETTIME: u64 = 228;
pub const SYS_WAIT4: u64 = 260;
pub const SYS_GETRANDOM: u64 = 318;
pub const SYS_RSEQ: u64 = 334;

// === Agent 系统调用 (512-528) ===
// Agent 生命周期管理、消息传递、能力控制等
pub const SYS_AGENT_SPAWN: u64 = 512;
pub const SYS_AGENT_KILL: u64 = 513;
pub const SYS_AGENT_QUERY: u64 = 514;
pub const SYS_AGENT_MSG: u64 = 515;
pub const SYS_AGENT_REGISTER: u64 = 516;
pub const SYS_AGENT_SUBSCRIBE: u64 = 517;
pub const SYS_AGENT_MIGRATE: u64 = 518;
pub const SYS_AGENT_MEMORY_SHARE: u64 = 519;
pub const SYS_AGENT_CAP_GRANT: u64 = 520;
pub const SYS_AGENT_CAP_REVOKE: u64 = 521;
pub const SYS_AGENT_BIND_PORT: u64 = 522;
pub const SYS_AGENT_EXPORT: u64 = 523;
pub const SYS_AGENT_IMPORT: u64 = 524;
pub const SYS_AGENT_SET_QUOTA: u64 = 525;
pub const SYS_AGENT_GET_QUOTA: u64 = 526;
pub const SYS_AGENT_SNAPSHOT: u64 = 527;
pub const SYS_AGENT_RESTORE: u64 = 528;

// === 虚拟化系统调用 (576-582) ===
// 虚拟机创建、启动、停止、内存映射等
pub const SYS_VM_CREATE: u64 = 576;
pub const SYS_VM_START: u64 = 577;
pub const SYS_VM_STOP: u64 = 578;
pub const SYS_VM_PAUSE: u64 = 579;
pub const SYS_VM_RESUME: u64 = 580;
pub const SYS_VM_MAP_MEMORY: u64 = 581;
pub const SYS_VM_IO_PORT: u64 = 582;

#[cfg(test)]
mod tests {
    use super::*;

    // === 传统系统调用编号必须小于 512 ===
    #[test]
    fn test_traditional_syscalls_below_512() {
        let traditional = [
            SYS_READ, SYS_WRITE, SYS_OPEN, SYS_CLOSE, SYS_STAT,
            SYS_FSTAT, SYS_LSTAT, SYS_POLL, SYS_LSEEK, SYS_MMAP,
            SYS_MUNMAP, SYS_MPROTECT, SYS_BRK, SYS_IOCTL, SYS_WRITEV,
            SYS_READV, SYS_MADVISE, SYS_GETPID, SYS_FORK, SYS_EXECVE,
            SYS_EXIT, SYS_SET_TID_ADDRESS, SYS_SIGACTION, SYS_FUTEX,
            SYS_CLOCK_GETTIME, SYS_WAIT4, SYS_GETRANDOM, SYS_RSEQ,
        ];
        for &num in &traditional {
            assert!(num < 512, "传统系统调用 {} 应小于 512", num);
        }
    }

    // === Agent 系统调用从 512 开始 ===
    #[test]
    fn test_agent_syscalls_start_at_512() {
        assert_eq!(SYS_AGENT_SPAWN, 512, "Agent 系统调用应从 512 开始");
    }

    // === Agent 系统调用 512-528 连续 ===
    #[test]
    fn test_agent_syscalls_sequential() {
        let agent_syscalls = [
            SYS_AGENT_SPAWN,      // 512
            SYS_AGENT_KILL,       // 513
            SYS_AGENT_QUERY,      // 514
            SYS_AGENT_MSG,        // 515
            SYS_AGENT_REGISTER,   // 516
            SYS_AGENT_SUBSCRIBE,  // 517
            SYS_AGENT_MIGRATE,    // 518
            SYS_AGENT_MEMORY_SHARE, // 519
            SYS_AGENT_CAP_GRANT,  // 520
            SYS_AGENT_CAP_REVOKE, // 521
            SYS_AGENT_BIND_PORT,  // 522
            SYS_AGENT_EXPORT,     // 523
            SYS_AGENT_IMPORT,     // 524
            SYS_AGENT_SET_QUOTA,  // 525
            SYS_AGENT_GET_QUOTA,  // 526
            SYS_AGENT_SNAPSHOT,   // 527
            SYS_AGENT_RESTORE,    // 528
        ];

        // 验证起始值和连续性
        for (i, &num) in agent_syscalls.iter().enumerate() {
            let expected = 512 + i as u64;
            assert_eq!(
                num, expected,
                "Agent 系统调用索引 {} 应为 {}，实际为 {}",
                i, expected, num
            );
        }
    }

    // === 所有系统调用编号唯一 ===
    #[test]
    fn test_agent_syscalls_unique() {
        let all_syscalls = [
            // 传统系统调用
            SYS_READ, SYS_WRITE, SYS_OPEN, SYS_CLOSE, SYS_STAT,
            SYS_FSTAT, SYS_LSTAT, SYS_POLL, SYS_LSEEK, SYS_MMAP,
            SYS_MUNMAP, SYS_MPROTECT, SYS_BRK, SYS_IOCTL, SYS_WRITEV,
            SYS_READV, SYS_MADVISE, SYS_GETPID, SYS_FORK, SYS_EXECVE,
            SYS_EXIT, SYS_SET_TID_ADDRESS, SYS_SIGACTION, SYS_FUTEX,
            SYS_CLOCK_GETTIME, SYS_WAIT4, SYS_GETRANDOM, SYS_RSEQ,
            // Agent 系统调用
            SYS_AGENT_SPAWN, SYS_AGENT_KILL, SYS_AGENT_QUERY, SYS_AGENT_MSG,
            SYS_AGENT_REGISTER, SYS_AGENT_SUBSCRIBE, SYS_AGENT_MIGRATE,
            SYS_AGENT_MEMORY_SHARE, SYS_AGENT_CAP_GRANT, SYS_AGENT_CAP_REVOKE,
            SYS_AGENT_BIND_PORT, SYS_AGENT_EXPORT, SYS_AGENT_IMPORT,
            SYS_AGENT_SET_QUOTA, SYS_AGENT_GET_QUOTA, SYS_AGENT_SNAPSHOT,
            SYS_AGENT_RESTORE,
            // 虚拟化系统调用
            SYS_VM_CREATE, SYS_VM_START, SYS_VM_STOP, SYS_VM_PAUSE,
            SYS_VM_RESUME, SYS_VM_MAP_MEMORY, SYS_VM_IO_PORT,
        ];

        // 使用排序后比较相邻元素来检测重复
        let mut sorted = all_syscalls.to_vec();
        sorted.sort();
        for window in sorted.windows(2) {
            assert_ne!(
                window[0], window[1],
                "系统调用编号 {} 出现重复",
                window[0]
            );
        }
    }

    // === 虚拟化系统调用从 576 开始 ===
    #[test]
    fn test_vm_syscalls_start_at_576() {
        assert_eq!(SYS_VM_CREATE, 576, "虚拟化系统调用应从 576 开始");
    }

    // === 虚拟化系统调用 576-582 连续 ===
    #[test]
    fn test_vm_syscalls_sequential() {
        let vm_syscalls = [
            SYS_VM_CREATE,     // 576
            SYS_VM_START,      // 577
            SYS_VM_STOP,       // 578
            SYS_VM_PAUSE,      // 579
            SYS_VM_RESUME,     // 580
            SYS_VM_MAP_MEMORY, // 581
            SYS_VM_IO_PORT,    // 582
        ];

        // 验证起始值和连续性
        for (i, &num) in vm_syscalls.iter().enumerate() {
            let expected = 576 + i as u64;
            assert_eq!(
                num, expected,
                "虚拟化系统调用索引 {} 应为 {}，实际为 {}",
                i, expected, num
            );
        }
    }
}
