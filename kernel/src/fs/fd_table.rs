//! 文件描述符表
//!
//! 管理进程的打开文件描述符。
//! 使用 spin::Mutex 保证线程安全，兼容 no_std 环境。

use alloc::sync::Arc;
use spin::Mutex;

use crate::fs::error::FsError;
use crate::fs::inode::{FileStat, FileType, VfsInode};

bitflags::bitflags! {
    /// 文件打开标志
    pub struct OpenFlags: i32 {
        /// 只读
        const O_RDONLY = 0;
        /// 只写
        const O_WRONLY = 1;
        /// 读写
        const O_RDWR   = 2;
        /// 若文件不存在则创建
        const O_CREAT  = 0o100;
        /// 截断文件为零长度
        const O_TRUNC  = 0o1000;
        /// 追加写入
        const O_APPEND = 0o2000;
    }
}

impl Default for OpenFlags {
    fn default() -> Self {
        OpenFlags::O_RDONLY
    }
}

/// 文件描述符表项
pub struct FdEntry {
    /// 文件描述符编号
    pub fd: u32,
    /// 关联的 Inode
    pub inode: Arc<dyn VfsInode>,
    /// 打开标志
    pub flags: OpenFlags,
    /// 当前读写偏移量
    pub offset: Mutex<u64>,
    /// 文件类型
    pub file_type: FileType,
}

impl core::fmt::Debug for FdEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FdEntry")
            .field("fd", &self.fd)
            .field("flags", &self.flags)
            .field("offset", &self.offset.lock())
            .field("file_type", &self.file_type)
            .finish()
    }
}

/// Seek 起始位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    /// 从文件起始位置
    Start(u64),
    /// 从文件末尾位置
    End(i64),
    /// 从当前读写位置
    Current(i64),
}

/// 最大文件描述符数量
const MAX_FDS: usize = 1024;

/// 文件描述符表
///
/// 管理进程打开的所有文件描述符。
/// 每个文件描述符关联一个 Inode、打开标志和偏移量。
pub struct FileDescriptorTable {
    /// 文件描述符数组
    fds: Mutex<[Option<FdEntry>; MAX_FDS]>,
    /// 下一个可用的文件描述符
    next_fd: Mutex<u32>,
}

impl FileDescriptorTable {
    /// 创建新的空文件描述符表
    pub fn new() -> Self {
        // 使用 const 初始化数组
        const NONE: Option<FdEntry> = None;
        FileDescriptorTable {
            fds: Mutex::new([NONE; MAX_FDS]),
            next_fd: Mutex::new(0),
        }
    }

    /// 打开文件，分配文件描述符
    ///
    /// 返回分配的文件描述符编号。
    pub fn open(&self, inode: Arc<dyn VfsInode>, flags: OpenFlags) -> Result<u32, FsError> {
        let mut fds = self.fds.lock();
        let mut next_fd = self.next_fd.lock();

        // 从 next_fd 开始搜索空闲槽位
        let mut found_idx: Option<usize> = None;
        for _ in 0..MAX_FDS {
            let idx = *next_fd as usize;
            if fds[idx].is_none() {
                found_idx = Some(idx);
                break;
            }
            *next_fd = (*next_fd + 1) % MAX_FDS as u32;
        }

        let idx = found_idx.ok_or(FsError::FdTableFull)?;
        let fd = idx as u32;
        let file_type = inode.file_type();

        fds[idx] = Some(FdEntry {
            fd,
            inode,
            flags,
            offset: Mutex::new(0),
            file_type,
        });

        *next_fd = (fd + 1) % MAX_FDS as u32;

        Ok(fd)
    }

    /// 关闭文件描述符
    pub fn close(&self, fd: u32) -> Result<(), FsError> {
        let mut fds = self.fds.lock();
        let idx = fd as usize;

        if idx >= MAX_FDS {
            return Err(FsError::InvalidFd(fd as i32));
        }

        if fds[idx].is_none() {
            return Err(FsError::NotOpen);
        }

        fds[idx] = None;
        Ok(())
    }

    /// 获取文件描述符的克隆副本
    ///
    /// 返回 FdEntry 的克隆（Arc 共享 Inode）。
    pub fn get(&self, fd: u32) -> Result<FdEntry, FsError> {
        let fds = self.fds.lock();
        let idx = fd as usize;

        if idx >= MAX_FDS {
            return Err(FsError::InvalidFd(fd as i32));
        }

        let entry = fds[idx].as_ref().ok_or(FsError::NotOpen)?;

        // 先读取偏移量值，避免嵌套锁导致借用问题
        let current_offset = *entry.offset.lock();

        // 克隆 FdEntry（共享 Arc<dyn VfsInode>）
        let cloned = FdEntry {
            fd: entry.fd,
            inode: entry.inode.clone(),
            flags: entry.flags,
            offset: Mutex::new(current_offset),
            file_type: entry.file_type,
        };

        Ok(cloned)
    }

    /// 复制文件描述符 (dup)
    ///
    /// 创建一个新的文件描述符，指向相同的 Inode 和偏移量。
    pub fn dup(&self, fd: u32) -> Result<u32, FsError> {
        let entry = self.get(fd)?;

        // 读取当前偏移量
        let current_offset = *entry.offset.lock();

        // 分配新的 fd
        let new_fd = {
            let mut fds = self.fds.lock();
            let mut next_fd = self.next_fd.lock();

            let mut found_idx: Option<usize> = None;
            for _ in 0..MAX_FDS {
                let idx = *next_fd as usize;
                if fds[idx].is_none() {
                    found_idx = Some(idx);
                    break;
                }
                *next_fd = (*next_fd + 1) % MAX_FDS as u32;
            }

            let idx = found_idx.ok_or(FsError::FdTableFull)?;
            let new_fd_num = idx as u32;

            fds[idx] = Some(FdEntry {
                fd: new_fd_num,
                inode: entry.inode.clone(),
                flags: entry.flags,
                offset: Mutex::new(current_offset),
                file_type: entry.file_type,
            });

            *next_fd = (new_fd_num + 1) % MAX_FDS as u32;
            new_fd_num
        };

        Ok(new_fd)
    }

    /// 从文件描述符读取数据
    pub fn read(&self, fd: u32, buf: &mut [u8]) -> Result<usize, FsError> {
        // 先在锁内提取所需信息
        let (inode, _file_type, current_offset) = {
            let fds = self.fds.lock();
            let idx = fd as usize;

            if idx >= MAX_FDS {
                return Err(FsError::InvalidFd(fd as i32));
            }

            let entry = fds[idx].as_ref().ok_or(FsError::NotOpen)?;

            // 检查读权限
            if entry.flags.contains(OpenFlags::O_WRONLY) {
                return Err(FsError::PermissionDenied);
            }

            if entry.file_type == FileType::Directory {
                return Err(FsError::IsADirectory);
            }

            let current_offset = *entry.offset.lock();
            (entry.inode.clone(), entry.file_type, current_offset)
        };

        // 在锁外执行 I/O
        let bytes_read = inode.read(current_offset, buf)?;

        // 更新偏移量
        {
            let fds = self.fds.lock();
            let idx = fd as usize;
            if let Some(entry) = fds[idx].as_ref() {
                *entry.offset.lock() = current_offset + bytes_read as u64;
            }
        }

        Ok(bytes_read)
    }

    /// 向文件描述符写入数据
    pub fn write(&self, fd: u32, buf: &[u8]) -> Result<usize, FsError> {
        // 先在锁内提取所需信息
        let (inode, flags, _file_type, current_offset) = {
            let fds = self.fds.lock();
            let idx = fd as usize;

            if idx >= MAX_FDS {
                return Err(FsError::InvalidFd(fd as i32));
            }

            let entry = fds[idx].as_ref().ok_or(FsError::NotOpen)?;

            // 检查写权限（访问模式为 O_RDONLY 时拒绝写入）
            let access_mode = entry.flags.bits() & 0x3;
            if access_mode == 0 {
                // O_RDONLY = 0
                return Err(FsError::PermissionDenied);
            }

            if entry.file_type == FileType::Directory {
                return Err(FsError::IsADirectory);
            }

            let current_offset = *entry.offset.lock();
            (entry.inode.clone(), entry.flags, entry.file_type, current_offset)
        };

        // 在锁外执行 I/O
        let write_offset = if flags.contains(OpenFlags::O_APPEND) {
            inode.size()
        } else {
            current_offset
        };

        let bytes_written = inode.write(write_offset, buf)?;

        // 更新偏移量
        {
            let fds = self.fds.lock();
            let idx = fd as usize;
            if let Some(entry) = fds[idx].as_ref() {
                if flags.contains(OpenFlags::O_APPEND) {
                    *entry.offset.lock() = inode.size();
                } else {
                    *entry.offset.lock() = current_offset + bytes_written as u64;
                }
            }
        }

        Ok(bytes_written)
    }

    /// 移动文件读写位置 (seek)
    ///
    /// `offset` 参数的含义取决于 `whence`：
    /// - `SeekFrom::Start`: offset 是绝对位置（忽略 offset 参数，使用 whence 中的值）
    /// - `SeekFrom::Current`: offset 是相对于当前位置的偏移量
    /// - `SeekFrom::End`: offset 是相对于文件末尾的偏移量
    pub fn seek(&self, fd: u32, offset: i64, whence: SeekFrom) -> Result<u64, FsError> {
        let fds = self.fds.lock();
        let idx = fd as usize;

        if idx >= MAX_FDS {
            return Err(FsError::InvalidFd(fd as i32));
        }

        let entry = fds[idx].as_ref().ok_or(FsError::NotOpen)?;
        let file_size = entry.inode.size() as i64;
        let mut current_offset = entry.offset.lock();

        let new_offset = match whence {
            SeekFrom::Start(_) => offset,
            SeekFrom::End(_) => file_size + offset,
            SeekFrom::Current(_) => *current_offset as i64 + offset,
        };

        if new_offset < 0 {
            return Err(FsError::InvalidOffset);
        }

        *current_offset = new_offset as u64;
        Ok(new_offset as u64)
    }

    /// 获取文件状态信息 (fstat)
    pub fn fstat(&self, fd: u32) -> Result<FileStat, FsError> {
        let fds = self.fds.lock();
        let idx = fd as usize;

        if idx >= MAX_FDS {
            return Err(FsError::InvalidFd(fd as i32));
        }

        let entry = fds[idx].as_ref().ok_or(FsError::NotOpen)?;
        entry.inode.stat()
    }

    /// 获取当前打开的文件描述符数量
    pub fn open_count(&self) -> usize {
        let fds = self.fds.lock();
        fds.iter().filter(|e| e.is_some()).count()
    }
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use alloc::vec::Vec;

    #[test]
    fn test_fd_table_open_close() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        // 打开文件
        let fd = table.open(inode.clone(), OpenFlags::O_RDWR).unwrap();
        assert_eq!(fd, 0);
        assert_eq!(table.open_count(), 1);

        // 关闭文件
        table.close(fd).unwrap();
        assert_eq!(table.open_count(), 0);

        // 再次关闭应报错
        assert!(matches!(table.close(fd), Err(FsError::NotOpen)));
    }

    #[test]
    fn test_fd_table_read_write() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd = table.open(inode.clone(), OpenFlags::O_RDWR).unwrap();

        // 写入数据
        let data = b"Hello, World!";
        let written = table.write(fd, data).unwrap();
        assert_eq!(written, data.len());

        // Seek 回起始位置
        table.seek(fd, 0, SeekFrom::Start(0)).unwrap();

        // 读取数据
        let mut buf = [0u8; 64];
        let read = table.read(fd, &mut buf).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buf[..read], data);
    }

    #[test]
    fn test_fd_table_seek() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd = table.open(inode.clone(), OpenFlags::O_RDWR).unwrap();

        // 写入数据
        table.write(fd, b"0123456789").unwrap();

        // Seek 到位置 5
        let pos = table.seek(fd, 5, SeekFrom::Start(0)).unwrap();
        assert_eq!(pos, 5);

        // 从当前位置 seek +3
        let pos = table.seek(fd, 3, SeekFrom::Current(0)).unwrap();
        assert_eq!(pos, 8);

        // 从末尾 seek -3
        let pos = table.seek(fd, -3, SeekFrom::End(0)).unwrap();
        assert_eq!(pos, 7);

        // Seek 到负偏移量应失败
        let result = table.seek(fd, -100, SeekFrom::End(0));
        assert!(matches!(result, Err(FsError::InvalidOffset)));
    }

    #[test]
    fn test_fd_table_dup() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd1 = table.open(inode.clone(), OpenFlags::O_RDWR).unwrap();

        // 写入数据
        table.write(fd1, b"dup test").unwrap();

        // 复制 fd
        let fd2 = table.dup(fd1).unwrap();
        assert_ne!(fd1, fd2);
        assert_eq!(table.open_count(), 2);

        // 通过 fd2 读取数据（seek 到起始位置）
        table.seek(fd2, 0, SeekFrom::Start(0)).unwrap();
        let mut buf = [0u8; 64];
        let read = table.read(fd2, &mut buf).unwrap();
        assert_eq!(&buf[..read], b"dup test");

        // 关闭 fd1，fd2 仍然有效
        table.close(fd1).unwrap();
        assert_eq!(table.open_count(), 1);

        // fd2 仍然可以读取
        table.seek(fd2, 0, SeekFrom::Start(0)).unwrap();
        let mut buf2 = [0u8; 64];
        let read2 = table.read(fd2, &mut buf2).unwrap();
        assert_eq!(&buf2[..read2], b"dup test");
    }

    #[test]
    fn test_fd_table_fstat() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd = table.open(inode.clone(), OpenFlags::O_RDWR).unwrap();

        // 写入数据
        table.write(fd, b"stat test data").unwrap();

        // fstat
        let stat = table.fstat(fd).unwrap();
        assert_eq!(stat.st_size, 14);
        assert_eq!(stat.st_nlink, 1);
        assert_eq!(stat.st_blksize, 4096);
    }

    #[test]
    fn test_fd_table_invalid_fd() {
        let table = FileDescriptorTable::new();

        // 超出范围的 fd 应返回 InvalidFd
        let mut buf = [0u8; 10];
        assert!(matches!(table.read(2000, &mut buf), Err(FsError::InvalidFd(_))));

        // 写入超出范围的 fd
        assert!(matches!(table.write(2000, b"test"), Err(FsError::InvalidFd(_))));

        // 关闭超出范围的 fd
        assert!(matches!(table.close(2000), Err(FsError::InvalidFd(_))));

        // fstat 超出范围的 fd
        assert!(matches!(table.fstat(2000), Err(FsError::InvalidFd(_))));

        // seek 超出范围的 fd
        assert!(matches!(
            table.seek(2000, 0, SeekFrom::Start(0)),
            Err(FsError::InvalidFd(_))
        ));

        // get 超出范围的 fd
        assert!(matches!(table.get(2000), Err(FsError::InvalidFd(_))));

        // 范围内但未打开的 fd 应返回 NotOpen
        assert!(matches!(table.read(999, &mut buf), Err(FsError::NotOpen)));
        assert!(matches!(table.write(999, b"test"), Err(FsError::NotOpen)));
        assert!(matches!(table.close(999), Err(FsError::NotOpen)));
        assert!(matches!(table.fstat(999), Err(FsError::NotOpen)));
        assert!(matches!(table.get(999), Err(FsError::NotOpen)));
    }

    #[test]
    fn test_fd_table_max_fds() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        // 打开 MAX_FDS 个文件
        let mut fds = Vec::new();
        for _ in 0..MAX_FDS {
            let fd = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();
            fds.push(fd);
        }
        assert_eq!(table.open_count(), MAX_FDS);

        // 再打开一个应失败
        let result = table.open(inode.clone(), OpenFlags::O_RDONLY);
        assert!(matches!(result, Err(FsError::FdTableFull)));

        // 关闭一个后可以再打开
        table.close(fds[0]).unwrap();
        assert_eq!(table.open_count(), MAX_FDS - 1);
        let fd = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();
        assert_eq!(table.open_count(), MAX_FDS);
    }

    #[test]
    fn test_fd_table_read_only_permission() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();

        // 只读 fd 不能写入
        let result = table.write(fd, b"test");
        assert!(matches!(result, Err(FsError::PermissionDenied)));
    }

    #[test]
    fn test_fd_table_write_only_permission() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd = table.open(inode.clone(), OpenFlags::O_WRONLY).unwrap();

        // 只写 fd 不能读取
        let mut buf = [0u8; 10];
        let result = table.read(fd, &mut buf);
        assert!(matches!(result, Err(FsError::PermissionDenied)));
    }

    #[test]
    fn test_fd_table_append() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        let fd = table.open(inode.clone(), OpenFlags::O_WRONLY | OpenFlags::O_APPEND).unwrap();

        // 追加写入
        table.write(fd, b"Hello ").unwrap();
        table.write(fd, b"World!").unwrap();

        // 关闭后重新打开读取
        table.close(fd).unwrap();
        let fd2 = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();
        let mut buf = [0u8; 64];
        let read = table.read(fd2, &mut buf).unwrap();
        assert_eq!(&buf[..read], b"Hello World!");
    }

    #[test]
    fn test_fd_table_multiple_fds_sequential() {
        let table = FileDescriptorTable::new();
        let inode = Arc::new(crate::fs::inode::MemoryInode::new_file());

        // 打开多个 fd，编号应递增
        let fd0 = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();
        let fd1 = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();
        let fd2 = table.open(inode.clone(), OpenFlags::O_RDONLY).unwrap();

        assert_eq!(fd0, 0);
        assert_eq!(fd1, 1);
        assert_eq!(fd2, 2);
        assert_eq!(table.open_count(), 3);
    }
}
