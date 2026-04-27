//! VFS Inode 抽象层
//!
//! 定义虚拟文件系统的 Inode trait 和内存 Inode 实现。
//! 所有文件系统后端都需要实现 VfsInode trait。

use alloc::vec::Vec;
use spin::Mutex;

use crate::fs::error::FsError;

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FileType {
    /// 普通文件
    Regular = 0,
    /// 目录
    Directory = 1,
    /// 符号链接
    SymLink = 2,
    /// 字符设备
    CharDevice = 3,
    /// 块设备
    BlockDevice = 4,
    /// 套接字
    Socket = 5,
    /// FIFO 管道
    Fifo = 6,
}

/// 文件状态信息（兼容 POSIX stat 结构）
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FileStat {
    /// 设备号
    pub st_dev: u64,
    /// INode 编号
    pub st_ino: u64,
    /// 文件模式（权限 + 类型）
    pub st_mode: u32,
    /// 硬链接数
    pub st_nlink: u32,
    /// 文件大小（字节）
    pub st_size: u64,
    /// 块大小
    pub st_blksize: u32,
    /// 分配的块数
    pub st_blocks: u64,
    /// 最后访问时间
    pub st_atime: u64,
    /// 最后修改时间
    pub st_mtime: u64,
    /// 创建时间
    pub st_ctime: u64,
}

/// VFS Inode trait
///
/// 所有文件系统后端都需要实现此 trait。
/// 提供统一的读写、状态查询接口。
pub trait VfsInode: Send + Sync {
    /// 从指定偏移量读取数据到缓冲区
    ///
    /// 返回实际读取的字节数。
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 从指定偏移量写入数据
    ///
    /// 返回实际写入的字节数。
    fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, FsError>;

    /// 获取文件状态信息
    fn stat(&self) -> Result<FileStat, FsError>;

    /// 获取文件类型
    fn file_type(&self) -> FileType;

    /// 获取文件大小（字节）
    fn size(&self) -> u64;
}

/// 内存 Inode 实现
///
/// 用于测试和内存文件系统。数据存储在堆分配的 Vec 中，
/// 使用 spin::Mutex 保证线程安全。
pub struct MemoryInode {
    /// 文件数据
    data: Mutex<Vec<u8>>,
    /// 文件类型
    file_type: FileType,
    /// 创建时间
    created_time: u64,
    /// 修改时间
    modified_time: u64,
}

impl MemoryInode {
    /// 创建新的文件 Inode
    pub fn new_file() -> Self {
        MemoryInode {
            data: Mutex::new(Vec::new()),
            file_type: FileType::Regular,
            created_time: 0,
            modified_time: 0,
        }
    }

    /// 创建新的目录 Inode
    pub fn new_directory() -> Self {
        MemoryInode {
            data: Mutex::new(Vec::new()),
            file_type: FileType::Directory,
            created_time: 0,
            modified_time: 0,
        }
    }
}

impl VfsInode for MemoryInode {
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        let data = self.data.lock();
        let offset = offset as usize;

        if offset >= data.len() {
            return Ok(0); // EOF
        }

        let end = core::cmp::min(offset + buf.len(), data.len());
        let bytes_read = end - offset;
        buf[..bytes_read].copy_from_slice(&data[offset..end]);
        Ok(bytes_read)
    }

    fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, FsError> {
        let mut data = self.data.lock();
        let offset = offset as usize;

        // 扩展文件大小
        if offset + buf.len() > data.len() {
            data.resize(offset + buf.len(), 0);
        }

        data[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(buf.len())
    }

    fn stat(&self) -> Result<FileStat, FsError> {
        let data = self.data.lock();
        let mode = match self.file_type {
            FileType::Regular => 0o100000 | 0o644,
            FileType::Directory => 0o040000 | 0o755,
            FileType::SymLink => 0o120000 | 0o777,
            FileType::CharDevice => 0o020000 | 0o660,
            FileType::BlockDevice => 0o060000 | 0o660,
            FileType::Socket => 0o140000 | 0o660,
            FileType::Fifo => 0o010000 | 0o660,
        };

        let size = data.len() as u64;
        let blksize = 4096u32;
        let blocks = (size + blksize as u64 - 1) / blksize as u64;

        Ok(FileStat {
            st_dev: 0,
            st_ino: 0,
            st_mode: mode,
            st_nlink: 1,
            st_size: size,
            st_blksize: blksize,
            st_blocks: blocks,
            st_atime: self.created_time,
            st_mtime: self.modified_time,
            st_ctime: self.created_time,
        })
    }

    fn file_type(&self) -> FileType {
        self.file_type
    }

    fn size(&self) -> u64 {
        self.data.lock().len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    #[test]
    fn test_memory_inode_new_file() {
        let inode = MemoryInode::new_file();
        assert_eq!(inode.file_type(), FileType::Regular);
        assert_eq!(inode.size(), 0);

        let stat = inode.stat().unwrap();
        assert_eq!(stat.st_size, 0);
        assert_eq!(stat.st_nlink, 1);
        // 普通文件模式: 0o100644
        assert_eq!(stat.st_mode, 0o100000 | 0o644);
    }

    #[test]
    fn test_memory_inode_new_directory() {
        let inode = MemoryInode::new_directory();
        assert_eq!(inode.file_type(), FileType::Directory);
        assert_eq!(inode.size(), 0);

        let stat = inode.stat().unwrap();
        assert_eq!(stat.st_size, 0);
        // 目录模式: 0o040755
        assert_eq!(stat.st_mode, 0o040000 | 0o755);
    }

    #[test]
    fn test_memory_inode_read_write() {
        let inode = Arc::new(MemoryInode::new_file());

        // 写入数据
        let data = b"Hello, OmniAgent OS!";
        let written = inode.write(0, data).unwrap();
        assert_eq!(written, data.len());
        assert_eq!(inode.size(), data.len() as u64);

        // 读取数据
        let mut buf = [0u8; 64];
        let read = inode.read(0, &mut buf).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buf[..read], data);

        // 从中间读取
        let mut buf2 = [0u8; 5];
        let read2 = inode.read(7, &mut buf2).unwrap();
        assert_eq!(read2, 5);
        assert_eq!(&buf2, b"OmniA");

        // 读取超过文件末尾
        let mut buf3 = [0u8; 10];
        let read3 = inode.read(100, &mut buf3).unwrap();
        assert_eq!(read3, 0); // EOF
    }

    #[test]
    fn test_memory_inode_stat() {
        let inode = MemoryInode::new_file();

        // 写入数据后检查 stat
        inode.write(0, b"test data here").unwrap();

        let stat = inode.stat().unwrap();
        assert_eq!(stat.st_size, 14);
        assert_eq!(stat.st_dev, 0);
        assert_eq!(stat.st_ino, 0);
        assert_eq!(stat.st_nlink, 1);
        assert_eq!(stat.st_blksize, 4096);
        // 14 字节 -> 1 个块
        assert_eq!(stat.st_blocks, 1);
    }

    #[test]
    fn test_memory_inode_size() {
        let inode = MemoryInode::new_file();
        assert_eq!(inode.size(), 0);

        inode.write(0, b"hello").unwrap();
        assert_eq!(inode.size(), 5);

        // 追加写入
        inode.write(5, b" world").unwrap();
        assert_eq!(inode.size(), 11);

        // 在非连续位置写入（会填充 0）
        inode.write(20, b"end").unwrap();
        assert_eq!(inode.size(), 23);
    }

    #[test]
    fn test_file_type_values() {
        assert_eq!(FileType::Regular as u8, 0);
        assert_eq!(FileType::Directory as u8, 1);
        assert_eq!(FileType::SymLink as u8, 2);
        assert_eq!(FileType::CharDevice as u8, 3);
        assert_eq!(FileType::BlockDevice as u8, 4);
        assert_eq!(FileType::Socket as u8, 5);
        assert_eq!(FileType::Fifo as u8, 6);
    }

    #[test]
    fn test_file_stat_size() {
        // 确保 FileStat 是 repr(C) 且大小合理
        assert!(core::mem::size_of::<FileStat>() > 0);
    }

    #[test]
    fn test_memory_inode_write_beyond_end() {
        let inode = MemoryInode::new_file();

        // 在偏移量 10 处写入（文件当前为空）
        let written = inode.write(10, b"hello").unwrap();
        assert_eq!(written, 5);
        assert_eq!(inode.size(), 15);

        // 读取前 15 字节，前 10 字节应为 0
        let mut buf = [0u8; 15];
        inode.read(0, &mut buf).unwrap();
        assert_eq!(&buf[..10], &[0u8; 10]);
        assert_eq!(&buf[10..], b"hello");
    }
}
