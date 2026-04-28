#![cfg_attr(not(test), no_std)]

/// 传统系统调用号范围 (0-511)
pub mod traditional {
    pub const SYS_READ: usize           = 0;
    pub const SYS_WRITE: usize          = 1;
    pub const SYS_OPEN: usize           = 2;
    pub const SYS_CLOSE: usize          = 3;
    pub const SYS_STAT: usize           = 4;
    pub const SYS_FSTAT: usize          = 5;
    pub const SYS_LSTAT: usize          = 6;
    pub const SYS_POLL: usize           = 7;
    pub const SYS_LSEEK: usize          = 8;
    pub const SYS_MMAP: usize           = 9;
    pub const SYS_MUNMAP: usize         = 10;
    pub const SYS_MPROTECT: usize       = 11;
    pub const SYS_BRK: usize            = 12;
    pub const SYS_IOCTL: usize          = 16;
    pub const SYS_WRITEV: usize         = 20;
    pub const SYS_READV: usize          = 21;
    pub const SYS_MADVISE: usize        = 28;
    pub const SYS_GETPID: usize         = 39;
    pub const SYS_FORK: usize           = 57;
    pub const SYS_EXECVE: usize         = 59;
    pub const SYS_EXIT: usize           = 60;
    pub const SYS_SET_TID_ADDRESS: usize = 96;
    pub const SYS_SIGACTION: usize      = 131;
    pub const SYS_FUTEX: usize          = 202;
    pub const SYS_CLOCK_GETTIME: usize  = 228;
    pub const SYS_WAIT4: usize          = 260;
    pub const SYS_GETRANDOM: usize      = 318;
    pub const SYS_RSEQ: usize           = 334;
}

/// Agent 系统调用号范围 (512+)
pub mod agent {
    pub const SYS_AGENT_SPAWN: usize        = 512;
    pub const SYS_AGENT_KILL: usize         = 513;
    pub const SYS_AGENT_QUERY: usize        = 514;
    pub const SYS_AGENT_MSG: usize          = 515;
    pub const SYS_AGENT_REGISTER: usize     = 516;
    pub const SYS_AGENT_SUBSCRIBE: usize    = 517;
    pub const SYS_AGENT_MIGRATE: usize      = 518;
    pub const SYS_AGENT_MEMORY_SHARE: usize = 519;
    pub const SYS_AGENT_CAP_GRANT: usize    = 520;
    pub const SYS_AGENT_CAP_REVOKE: usize   = 521;
    pub const SYS_AGENT_BIND_PORT: usize    = 522;
    pub const SYS_AGENT_EXPORT: usize       = 523;
    pub const SYS_AGENT_IMPORT: usize       = 524;
    pub const SYS_AGENT_SET_QUOTA: usize    = 525;
    pub const SYS_AGENT_GET_QUOTA: usize    = 526;
    pub const SYS_AGENT_SNAPSHOT: usize     = 527;
    pub const SYS_AGENT_RESTORE: usize      = 528;
}

/// 系统调用结果类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallResult {
    pub value: isize,
}

impl SyscallResult {
    pub const OK: SyscallResult = SyscallResult { value: 0 };
    pub const INVAL: SyscallResult = SyscallResult { value: -22 };
    pub const PERM: SyscallResult = SyscallResult { value: -1 };
    pub const NOENT: SyscallResult = SyscallResult { value: -2 };
    pub const NOMEM: SyscallResult = SyscallResult { value: -12 };
    pub const BUSY: SyscallResult = SyscallResult { value: -16 };
    pub const AGAIN: SyscallResult = SyscallResult { value: -11 };
    pub const NOTSUP: SyscallResult = SyscallResult { value: -95 };

    pub fn is_ok(&self) -> bool {
        self.value >= 0
    }

    pub fn is_err(&self) -> bool {
        self.value < 0
    }

    pub fn unwrap(&self) -> usize {
        if self.value < 0 {
            panic!("SyscallResult unwrap on error: {}", self.value);
        }
        self.value as usize
    }

    pub fn from_raw(value: isize) -> Self {
        Self { value }
    }
}

#[cfg(not(test))]
impl core::fmt::Display for SyscallResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_ok() {
            write!(f, "SyscallResult(ok: {})", self.value)
        } else {
            write!(f, "SyscallResult(err: {})", self.value)
        }
    }
}

#[cfg(test)]
impl std::fmt::Display for SyscallResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_ok() {
            write!(f, "SyscallResult(ok: {})", self.value)
        } else {
            write!(f, "SyscallResult(err: {})", self.value)
        }
    }
}

#[cfg(test)]
impl std::error::Error for SyscallResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_syscalls_start_at_512() {
        assert_eq!(agent::SYS_AGENT_SPAWN, 512);
    }

    #[test]
    fn test_all_agent_syscalls_are_unique() {
        let syscalls = [
            agent::SYS_AGENT_SPAWN,
            agent::SYS_AGENT_KILL,
            agent::SYS_AGENT_QUERY,
            agent::SYS_AGENT_MSG,
            agent::SYS_AGENT_REGISTER,
            agent::SYS_AGENT_SUBSCRIBE,
            agent::SYS_AGENT_MIGRATE,
            agent::SYS_AGENT_MEMORY_SHARE,
            agent::SYS_AGENT_CAP_GRANT,
            agent::SYS_AGENT_CAP_REVOKE,
            agent::SYS_AGENT_BIND_PORT,
            agent::SYS_AGENT_EXPORT,
            agent::SYS_AGENT_IMPORT,
            agent::SYS_AGENT_SET_QUOTA,
            agent::SYS_AGENT_GET_QUOTA,
            agent::SYS_AGENT_SNAPSHOT,
            agent::SYS_AGENT_RESTORE,
        ];
        let mut sorted = syscalls;
        sorted.sort();
        let original_len = sorted.len();
        let mut i = 0;
        while i + 1 < sorted.len() {
            if sorted[i] == sorted[i + 1] {
                panic!("Duplicate syscall number found: {}", sorted[i]);
            }
            i += 1;
        }
        assert_eq!(original_len, sorted.len(), "Duplicate syscall numbers found");
    }

    #[test]
    fn test_agent_syscalls_are_sequential() {
        assert_eq!(agent::SYS_AGENT_SPAWN, 512);
        assert_eq!(agent::SYS_AGENT_KILL, 513);
        assert_eq!(agent::SYS_AGENT_QUERY, 514);
        assert_eq!(agent::SYS_AGENT_MSG, 515);
        assert_eq!(agent::SYS_AGENT_RESTORE, 528);
    }

    #[test]
    fn test_syscall_result_ok() {
        assert!(SyscallResult::OK.is_ok());
        assert!(!SyscallResult::OK.is_err());
        assert_eq!(SyscallResult::OK.unwrap(), 0);
    }

    #[test]
    fn test_syscall_result_error() {
        assert!(SyscallResult::INVAL.is_err());
        assert!(!SyscallResult::INVAL.is_ok());
    }

    #[test]
    #[should_panic]
    fn test_syscall_result_unwrap_panic() {
        SyscallResult::INVAL.unwrap();
    }

    #[test]
    fn test_syscall_result_from_raw() {
        let ok = SyscallResult::from_raw(42);
        assert!(ok.is_ok());
        assert_eq!(ok.unwrap(), 42);

        let err = SyscallResult::from_raw(-22);
        assert!(err.is_err());
    }

    #[test]
    fn test_traditional_syscalls_are_below_512() {
        let trad_syscalls = [
            traditional::SYS_READ,
            traditional::SYS_WRITE,
            traditional::SYS_OPEN,
            traditional::SYS_CLOSE,
            traditional::SYS_MMAP,
            traditional::SYS_EXIT,
            traditional::SYS_FUTEX,
            traditional::SYS_GETRANDOM,
        ];
        for &s in &trad_syscalls {
            assert!(s < 512, "Traditional syscall {} is >= 512", s);
        }
    }
}
