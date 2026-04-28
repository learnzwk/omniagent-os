//! 文件系统错误类型
//!
//! 定义内核文件系统层使用的所有错误类型。
//! 兼容 no_std 环境，使用 alloc 进行堆分配。

use core::fmt;

/// 文件系统错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum FsError {
    /// 文件或目录未找到
    NotFound,
    /// 文件或目录已存在
    AlreadyExists,
    /// 权限不足
    PermissionDenied,
    /// 路径不是目录
    NotADirectory,
    /// 路径是目录（不能对目录执行文件操作）
    IsADirectory,
    /// 无效路径
    InvalidPath,
    /// 文件名过长
    NameTooLong,
    /// 磁盘空间不足
    NoSpace,
    /// 目录非空（不能删除非空目录）
    NotEmpty,
    /// 无效的文件描述符
    InvalidFd(i32),
    /// 文件描述符表已满
    FdTableFull,
    /// 文件未打开
    NotOpen,
    /// I/O 错误
    IoError {
        /// 错误原因描述
        reason: alloc::string::String,
    },
    /// 无效的偏移量
    InvalidOffset,
    /// 操作会阻塞
    WouldBlock,
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotFound => write!(f, "文件或目录未找到"),
            FsError::AlreadyExists => write!(f, "文件或目录已存在"),
            FsError::PermissionDenied => write!(f, "权限不足"),
            FsError::NotADirectory => write!(f, "不是目录"),
            FsError::IsADirectory => write!(f, "是目录"),
            FsError::InvalidPath => write!(f, "无效路径"),
            FsError::NameTooLong => write!(f, "文件名过长"),
            FsError::NoSpace => write!(f, "磁盘空间不足"),
            FsError::NotEmpty => write!(f, "目录非空"),
            FsError::InvalidFd(fd) => write!(f, "无效的文件描述符: {}", fd),
            FsError::FdTableFull => write!(f, "文件描述符表已满"),
            FsError::NotOpen => write!(f, "文件未打开"),
            FsError::IoError { reason } => write!(f, "I/O 错误: {}", reason),
            FsError::InvalidOffset => write!(f, "无效的偏移量"),
            FsError::WouldBlock => write!(f, "操作会阻塞"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", FsError::NotFound), "文件或目录未找到");
        assert_eq!(format!("{}", FsError::AlreadyExists), "文件或目录已存在");
        assert_eq!(format!("{}", FsError::PermissionDenied), "权限不足");
        assert_eq!(format!("{}", FsError::NotADirectory), "不是目录");
        assert_eq!(format!("{}", FsError::IsADirectory), "是目录");
        assert_eq!(format!("{}", FsError::InvalidPath), "无效路径");
        assert_eq!(format!("{}", FsError::NameTooLong), "文件名过长");
        assert_eq!(format!("{}", FsError::NoSpace), "磁盘空间不足");
        assert_eq!(format!("{}", FsError::NotEmpty), "目录非空");
        assert_eq!(format!("{}", FsError::InvalidFd(42)), "无效的文件描述符: 42");
        assert_eq!(format!("{}", FsError::FdTableFull), "文件描述符表已满");
        assert_eq!(format!("{}", FsError::NotOpen), "文件未打开");
        assert_eq!(
            format!("{}", FsError::IoError { reason: alloc::string::String::from("设备错误") }),
            "I/O 错误: 设备错误"
        );
        assert_eq!(format!("{}", FsError::InvalidOffset), "无效的偏移量");
        assert_eq!(format!("{}", FsError::WouldBlock), "操作会阻塞");
    }

    #[test]
    fn test_error_clone() {
        let err = FsError::NotFound;
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));

        let err = FsError::InvalidFd(-1);
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));
    }

    #[test]
    fn test_error_debug() {
        let err = FsError::IoError { reason: alloc::string::String::from("测试") };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("IoError"));
        assert!(debug_str.contains("测试"));
    }
}
