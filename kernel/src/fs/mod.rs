//! 内核文件系统抽象层
//!
//! 提供虚拟文件系统 (VFS) 的内核抽象，包括：
//! - 错误类型 (FsError)
//! - 路径解析工具 (Path)
//! - VFS Inode trait 和内存实现 (VfsInode, MemoryInode)
//! - 文件描述符表 (FileDescriptorTable)
//! - VFS 管理器 (VfsManager)
//!
//! 兼容 no_std 环境，使用 alloc 进行堆分配。

pub mod error;
pub mod path;
pub mod inode;
pub mod fd_table;
pub mod vfs;

// 重新导出常用类型
pub use error::FsError;
pub use inode::{VfsInode, FileType, FileStat, MemoryInode};
pub use fd_table::{FileDescriptorTable, OpenFlags, FdEntry, SeekFrom};
pub use vfs::{VfsManager, DirEntry, VFS};
