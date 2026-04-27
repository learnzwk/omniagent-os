//! OmniAgent 文件系统服务
//!
//! 提供虚拟文件系统 (VFS)、Agent 专用文件系统等功能。
//! 支持内存文件系统、目录操作、文件读写、权限管理等。

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

// === 文件系统类型 ===

/// 文件系统类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FsType {
    /// Agent 专用文件系统
    AgentFS = 0,
    /// 临时文件系统
    TempFS = 1,
    /// 内存文件系统
    MemFS = 2,
    /// FAT32 文件系统
    Fat32 = 3,
    /// ext2 简化版文件系统
    Ext2 = 4,
}

impl fmt::Display for FsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsType::AgentFS => write!(f, "AgentFS"),
            FsType::TempFS => write!(f, "TempFS"),
            FsType::MemFS => write!(f, "MemFS"),
            FsType::Fat32 => write!(f, "FAT32"),
            FsType::Ext2 => write!(f, "ext2"),
        }
    }
}

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
    /// FIFO 管道
    FIFO = 5,
    /// 套接字
    Socket = 6,
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::Regular => write!(f, "普通文件"),
            FileType::Directory => write!(f, "目录"),
            FileType::SymLink => write!(f, "符号链接"),
            FileType::CharDevice => write!(f, "字符设备"),
            FileType::BlockDevice => write!(f, "块设备"),
            FileType::FIFO => write!(f, "FIFO"),
            FileType::Socket => write!(f, "套接字"),
        }
    }
}

/// 文件权限 (Unix 风格)
#[derive(Debug, Clone, Copy)]
pub struct FilePermissions(pub u16);

impl FilePermissions {
    /// 所有者读权限
    pub const OWNER_READ: u16 = 0o400;
    /// 所有者写权限
    pub const OWNER_WRITE: u16 = 0o200;
    /// 所有者执行权限
    pub const OWNER_EXEC: u16 = 0o100;
    /// 组读权限
    pub const GROUP_READ: u16 = 0o040;
    /// 组写权限
    pub const GROUP_WRITE: u16 = 0o020;
    /// 组执行权限
    pub const GROUP_EXEC: u16 = 0o010;
    /// 其他读权限
    pub const OTHER_READ: u16 = 0o004;
    /// 其他写权限
    pub const OTHER_WRITE: u16 = 0o002;
    /// 其他执行权限
    pub const OTHER_EXEC: u16 = 0o001;

    /// 创建新的文件权限
    pub fn new(mode: u16) -> Self {
        FilePermissions(mode & 0o777)
    }

    /// 检查是否有读权限
    pub fn is_readable(&self) -> bool {
        (self.0 & (Self::OWNER_READ | Self::GROUP_READ | Self::OTHER_READ)) != 0
    }

    /// 检查是否有写权限
    pub fn is_writable(&self) -> bool {
        (self.0 & (Self::OWNER_WRITE | Self::GROUP_WRITE | Self::OTHER_WRITE)) != 0
    }

    /// 检查是否有执行权限
    pub fn is_executable(&self) -> bool {
        (self.0 & (Self::OWNER_EXEC | Self::GROUP_EXEC | Self::OTHER_EXEC)) != 0
    }

    /// 获取原始权限模式
    pub fn mode(&self) -> u16 {
        self.0
    }

    /// 创建默认文件权限 (0644)
    pub const fn default_file() -> Self {
        FilePermissions(0o644)
    }

    /// 创建默认目录权限 (0755)
    pub const fn default_dir() -> Self {
        FilePermissions(0o755)
    }
}

impl Default for FilePermissions {
    fn default() -> Self {
        Self::default_file()
    }
}

impl PartialEq for FilePermissions {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for FilePermissions {}

impl BitAnd for FilePermissions {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        FilePermissions(self.0 & rhs.0)
    }
}

impl BitOr for FilePermissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        FilePermissions(self.0 | rhs.0)
    }
}

impl BitAndAssign for FilePermissions {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for FilePermissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Not for FilePermissions {
    type Output = Self;
    fn not(self) -> Self {
        FilePermissions(!self.0 & 0o777)
    }
}

/// 文件属性
#[derive(Debug, Clone)]
pub struct FileAttr {
    /// 文件类型
    pub file_type: FileType,
    /// 文件大小 (字节)
    pub size: u64,
    /// 文件权限
    pub permissions: FilePermissions,
    /// 创建时间
    pub created_at: u64,
    /// 修改时间
    pub modified_at: u64,
    /// 访问时间
    pub accessed_at: u64,
    /// 用户 ID
    pub uid: u32,
    /// 组 ID
    pub gid: u32,
    /// 硬链接数
    pub nlinks: u32,
}

/// 目录条目
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// 文件名
    pub name: String,
    /// 文件类型
    pub file_type: FileType,
    /// INode 编号
    pub inode: u64,
}

/// 打开标志
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags(pub u32);

impl OpenFlags {
    /// 只读 (bit 0)
    pub const READ: OpenFlags = OpenFlags(1 << 0);
    /// 只写 (bit 1)
    pub const WRITE: OpenFlags = OpenFlags(1 << 1);
    /// 追加写入 (bit 2)
    pub const APPEND: OpenFlags = OpenFlags(1 << 2);
    /// 若文件不存在则创建 (bit 3)
    pub const CREATE: OpenFlags = OpenFlags(1 << 3);
    /// 截断文件为零长度 (bit 4)
    pub const TRUNCATE: OpenFlags = OpenFlags(1 << 4);
    /// 独占创建 (bit 5)
    pub const EXCLUSIVE: OpenFlags = OpenFlags(1 << 5);
    /// 必须是目录 (bit 6)
    pub const DIRECTORY: OpenFlags = OpenFlags(1 << 6);
    /// 读写
    pub const READ_WRITE: OpenFlags = OpenFlags((1 << 0) | (1 << 1));

    /// 检查是否可读
    pub fn is_readable(&self) -> bool {
        (self.0 & Self::READ.0) != 0
    }

    /// 检查是否可写
    pub fn is_writable(&self) -> bool {
        (self.0 & Self::WRITE.0) != 0 || (self.0 & Self::APPEND.0) != 0
    }

    /// 检查是否包含指定标志
    pub fn contains(&self, other: OpenFlags) -> bool {
        (self.0 & other.0) != 0
    }
}

impl PartialEq for OpenFlags {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for OpenFlags {}

impl BitOr for OpenFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        OpenFlags(self.0 | rhs.0)
    }
}

/// Seek 方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    /// 从起始位置
    Start(u64),
    /// 从当前位置
    Current(i64),
    /// 从末尾位置
    End(i64),
}

// === VFS (虚拟文件系统) ===

/// INode - 文件系统中的基本节点
#[derive(Debug, Clone)]
pub struct INode {
    /// INode 编号
    pub id: u64,
    /// 文件类型
    pub file_type: FileType,
    /// 文件大小
    pub size: u64,
    /// 文件权限
    pub permissions: FilePermissions,
    /// 文件内容 (内存 FS)
    pub data: Vec<u8>,
    /// 目录条目
    pub children: Vec<DirEntry>,
    /// 创建时间
    pub created_at: u64,
    /// 修改时间
    pub modified_at: u64,
    /// 访问时间
    pub accessed_at: u64,
    /// 父 INode 编号
    pub parent: Option<u64>,
}

impl INode {
    /// 创建新的 INode
    pub fn new(id: u64, file_type: FileType, permissions: FilePermissions) -> Self {
        INode {
            id,
            file_type,
            size: 0,
            permissions,
            data: Vec::new(),
            children: Vec::new(),
            created_at: 0,
            modified_at: 0,
            accessed_at: 0,
            parent: None,
        }
    }

    /// 创建目录 INode
    pub fn new_dir(id: u64, permissions: FilePermissions) -> Self {
        INode {
            id,
            file_type: FileType::Directory,
            size: 0,
            permissions,
            data: Vec::new(),
            children: Vec::new(),
            created_at: 0,
            modified_at: 0,
            accessed_at: 0,
            parent: None,
        }
    }
}

/// 文件描述符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDescriptor(pub u32);

impl Hash for FileDescriptor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// 打开的文件
#[derive(Clone)]
pub struct OpenFile {
    /// 关联的 INode 编号
    pub inode_id: u64,
    /// 当前偏移量
    pub offset: u64,
    /// 打开标志
    pub flags: OpenFlags,
    /// 是否可读
    pub is_readable: bool,
    /// 是否可写
    pub is_writable: bool,
}

/// VFS 错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// 文件或目录未找到
    NotFound(String),
    /// 文件或目录已存在
    AlreadyExists(String),
    /// 权限不足
    PermissionDenied(String),
    /// 路径不是目录
    NotADirectory(String),
    /// 路径是目录
    IsADirectory(String),
    /// 目录非空
    DirectoryNotEmpty(String),
    /// 空间不足
    NoSpace,
    /// 无效路径
    InvalidPath(String),
    /// 文件描述符未打开
    NotOpen(FileDescriptor),
    /// 无效的文件描述符
    InvalidDescriptor(FileDescriptor),
    /// 只读文件系统
    ReadOnly,
    /// 文件名过长
    NameTooLong(String),
    /// 打开文件过多
    TooManyOpenFiles,
    /// I/O 错误
    IoError(String),
    /// 操作不支持
    NotSupported(String),
    /// 无效偏移量
    InvalidOffset,
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotFound(s) => write!(f, "未找到: {}", s),
            FsError::AlreadyExists(s) => write!(f, "已存在: {}", s),
            FsError::PermissionDenied(s) => write!(f, "权限不足: {}", s),
            FsError::NotADirectory(s) => write!(f, "不是目录: {}", s),
            FsError::IsADirectory(s) => write!(f, "是目录: {}", s),
            FsError::DirectoryNotEmpty(s) => write!(f, "目录非空: {}", s),
            FsError::NoSpace => write!(f, "空间不足"),
            FsError::InvalidPath(s) => write!(f, "无效路径: {}", s),
            FsError::NotOpen(fd) => write!(f, "文件未打开: fd={}", fd.0),
            FsError::InvalidDescriptor(fd) => write!(f, "无效描述符: fd={}", fd.0),
            FsError::ReadOnly => write!(f, "只读文件系统"),
            FsError::NameTooLong(s) => write!(f, "文件名过长: {}", s),
            FsError::TooManyOpenFiles => write!(f, "打开文件过多"),
            FsError::IoError(s) => write!(f, "I/O 错误: {}", s),
            FsError::NotSupported(s) => write!(f, "不支持的操作: {}", s),
            FsError::InvalidOffset => write!(f, "无效偏移量"),
        }
    }
}

/// 文件系统统计信息
#[derive(Debug, Clone)]
pub struct FsStats {
    /// 总 INode 数
    pub total_inodes: u64,
    /// 已使用 INode 数
    pub used_inodes: u64,
    /// 总空间 (字节)
    pub total_size: u64,
    /// 已使用空间 (字节)
    pub used_size: u64,
    /// 打开的文件数
    pub open_files: u32,
    /// 文件系统类型
    pub fs_type: FsType,
}

/// 虚拟文件系统
pub struct VirtualFileSystem {
    /// INode 表
    inodes: BTreeMap<u64, INode>,
    /// 根 INode 编号
    root_inode: u64,
    /// 打开的文件表
    open_files: BTreeMap<u32, OpenFile>,
    /// 下一个文件描述符
    next_fd: u32,
    /// 下一个 INode 编号
    next_inode: u64,
    /// 文件系统类型
    fs_type: FsType,
}

impl VirtualFileSystem {
    /// 创建新的虚拟文件系统
    pub fn new(fs_type: FsType) -> Self {
        VirtualFileSystem {
            inodes: BTreeMap::new(),
            root_inode: 0,
            open_files: BTreeMap::new(),
            next_fd: 0,
            next_inode: 0,
            fs_type,
        }
    }

    /// 初始化根目录
    pub fn init_root(&mut self) {
        let root = INode::new_dir(0, FilePermissions::default_dir());
        self.root_inode = 0;
        self.inodes.insert(0, root);
        self.next_inode = 1;
    }

    /// 分配新的 INode 编号
    fn alloc_inode(&mut self) -> u64 {
        let id = self.next_inode;
        self.next_inode += 1;
        id
    }

    /// 分配新的文件描述符
    fn alloc_fd(&mut self) -> FileDescriptor {
        let fd = self.next_fd;
        self.next_fd += 1;
        FileDescriptor(fd)
    }

    /// 解析路径，返回对应的 INode 编号
    fn resolve_path(&self, path: &str) -> Result<u64, FsError> {
        if path.is_empty() {
            return Err(FsError::InvalidPath("路径为空".into()));
        }

        let path = path.trim_start_matches('/');

        if path.is_empty() {
            // 根目录
            return Ok(self.root_inode);
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current_inode = self.root_inode;

        for part in &parts {
            if part.len() > 255 {
                return Err(FsError::NameTooLong(part.to_string()));
            }

            let inode = self.inodes.get(&current_inode)
                .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", current_inode)))?;

            if inode.file_type != FileType::Directory {
                return Err(FsError::NotADirectory(format!("{} 不是目录", part)));
            }

            // 在子条目中查找
            let found = inode.children.iter().find(|entry| entry.name == *part);
            match found {
                Some(entry) => current_inode = entry.inode,
                None => return Err(FsError::NotFound(format!("{} 未找到", part))),
            }
        }

        Ok(current_inode)
    }

    /// 解析父目录路径，返回 (父 INode 编号, 文件名)
    fn resolve_parent<'a>(&self, path: &'a str) -> Result<(u64, &'a str), FsError> {
        if path.is_empty() {
            return Err(FsError::InvalidPath("路径为空".into()));
        }

        let path = path.trim_start_matches('/');

        if path.is_empty() || path == "/" {
            return Err(FsError::InvalidPath("不能在根目录上执行此操作".into()));
        }

        // 找到最后一个 '/'
        let last_slash = path.rfind('/').unwrap_or(0);
        let (parent_path, name) = if last_slash == 0 {
            ("/", &path[0..])
        } else {
            (&path[..last_slash], &path[last_slash + 1..])
        };

        if name.is_empty() || name.len() > 255 {
            return Err(FsError::InvalidPath(format!("无效文件名: '{}'", name)));
        }

        let parent_inode = self.resolve_path(parent_path)?;
        Ok((parent_inode, name))
    }

    /// 打开文件
    pub fn open(&mut self, path: &str, flags: OpenFlags) -> Result<FileDescriptor, FsError> {
        let inode_id = match self.resolve_path(path) {
            Ok(id) => {
                // 文件已存在
                let inode = self.inodes.get(&id).ok_or_else(|| {
                    FsError::NotFound(format!("INode {} 不存在", id))
                })?;

                // 如果是目录且没有 DIRECTORY 标志
                if inode.file_type == FileType::Directory && !flags.contains(OpenFlags::DIRECTORY) {
                    return Err(FsError::IsADirectory(path.to_string()));
                }

                // 如果是 TRUNCATE 且可写，清空文件
                if flags.contains(OpenFlags::TRUNCATE) && flags.is_writable() {
                    if let Some(inode) = self.inodes.get_mut(&id) {
                        inode.data.clear();
                        inode.size = 0;
                    }
                }

                id
            }
            Err(FsError::NotFound(_)) => {
                // 文件不存在，检查 CREATE 标志
                if flags.contains(OpenFlags::CREATE) {
                    // 检查 EXCLUSIVE 标志
                    self.create_file(path, FilePermissions::default_file())?
                } else {
                    return Err(FsError::NotFound(path.to_string()));
                }
            }
            Err(e) => return Err(e),
        };

        let fd = self.alloc_fd();
        let is_readable = flags.is_readable();
        let is_writable = flags.is_writable();

        let open_file = OpenFile {
            inode_id,
            offset: 0,
            flags,
            is_readable,
            is_writable,
        };

        self.open_files.insert(fd.0, open_file);

        // 更新访问时间
        if let Some(inode) = self.inodes.get_mut(&inode_id) {
            inode.accessed_at = 0; // 简化：使用 0 表示当前时间
        }

        Ok(fd)
    }

    /// 关闭文件
    pub fn close(&mut self, fd: FileDescriptor) -> Result<(), FsError> {
        if self.open_files.remove(&fd.0).is_none() {
            Err(FsError::NotOpen(fd))
        } else {
            Ok(())
        }
    }

    /// 读取文件
    pub fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize, FsError> {
        let open_file = self.open_files.get(&fd.0)
            .ok_or(FsError::NotOpen(fd))?
            .clone();

        if !open_file.is_readable {
            return Err(FsError::PermissionDenied("文件不可读".into()));
        }

        let inode = self.inodes.get_mut(&open_file.inode_id)
            .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", open_file.inode_id)))?;

        if inode.file_type == FileType::Directory {
            return Err(FsError::IsADirectory("不能读取目录".into()));
        }

        let start = open_file.offset as usize;
        if start >= inode.data.len() {
            return Ok(0); // EOF
        }

        let end = core::cmp::min(start + buf.len(), inode.data.len());
        let bytes_read = end - start;
        buf[..bytes_read].copy_from_slice(&inode.data[start..end]);

        // 更新偏移量
        if let Some(of) = self.open_files.get_mut(&fd.0) {
            of.offset += bytes_read as u64;
        }

        // 更新访问时间
        inode.accessed_at = 0;

        Ok(bytes_read)
    }

    /// 写入文件
    pub fn write(&mut self, fd: FileDescriptor, buf: &[u8]) -> Result<usize, FsError> {
        let open_file = self.open_files.get(&fd.0)
            .ok_or(FsError::NotOpen(fd))?
            .clone();

        if !open_file.is_writable {
            return Err(FsError::PermissionDenied("文件不可写".into()));
        }

        let inode = self.inodes.get_mut(&open_file.inode_id)
            .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", open_file.inode_id)))?;

        if inode.file_type == FileType::Directory {
            return Err(FsError::IsADirectory("不能写入目录".into()));
        }

        let offset = if open_file.flags.contains(OpenFlags::APPEND) {
            inode.data.len()
        } else {
            open_file.offset as usize
        };

        // 扩展文件大小
        if offset + buf.len() > inode.data.len() {
            inode.data.resize(offset + buf.len(), 0);
        }

        inode.data[offset..offset + buf.len()].copy_from_slice(buf);
        inode.size = inode.data.len() as u64;
        inode.modified_at = 0;

        // 更新偏移量
        if let Some(of) = self.open_files.get_mut(&fd.0) {
            of.offset = (offset + buf.len()) as u64;
        }

        Ok(buf.len())
    }

    /// Seek - 移动文件读写位置
    pub fn seek(&mut self, fd: FileDescriptor, pos: SeekFrom) -> Result<u64, FsError> {
        let open_file = self.open_files.get(&fd.0)
            .ok_or(FsError::NotOpen(fd))?
            .clone();

        let inode = self.inodes.get(&open_file.inode_id)
            .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", open_file.inode_id)))?;

        let file_size = inode.data.len() as i64;
        let new_offset = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(delta) => open_file.offset as i64 + delta,
            SeekFrom::End(delta) => file_size + delta,
        };

        if new_offset < 0 {
            return Err(FsError::InvalidOffset);
        }

        let new_offset = new_offset as u64;
        if let Some(of) = self.open_files.get_mut(&fd.0) {
            of.offset = new_offset;
        }

        Ok(new_offset)
    }

    /// 创建文件
    pub fn create_file(&mut self, path: &str, permissions: FilePermissions) -> Result<u64, FsError> {
        let (parent_id, name) = self.resolve_parent(path)?;

        // 检查父目录是否存在且是目录
        let parent = self.inodes.get(&parent_id)
            .ok_or_else(|| FsError::NotFound(format!("父目录 INode {} 不存在", parent_id)))?;

        if parent.file_type != FileType::Directory {
            return Err(FsError::NotADirectory("父路径不是目录".into()));
        }

        // 检查是否已存在
        if parent.children.iter().any(|e| e.name == name) {
            return Err(FsError::AlreadyExists(format!("{} 已存在", name)));
        }

        // 创建新 INode
        let inode_id = self.alloc_inode();
        let mut inode = INode::new(inode_id, FileType::Regular, permissions);
        inode.parent = Some(parent_id);
        self.inodes.insert(inode_id, inode);

        // 添加到父目录
        if let Some(parent) = self.inodes.get_mut(&parent_id) {
            parent.children.push(DirEntry {
                name: name.to_string(),
                file_type: FileType::Regular,
                inode: inode_id,
            });
        }

        Ok(inode_id)
    }

    /// 创建目录
    pub fn create_dir(&mut self, path: &str, permissions: FilePermissions) -> Result<u64, FsError> {
        let (parent_id, name) = self.resolve_parent(path)?;

        // 检查父目录
        let parent = self.inodes.get(&parent_id)
            .ok_or_else(|| FsError::NotFound(format!("父目录 INode {} 不存在", parent_id)))?;

        if parent.file_type != FileType::Directory {
            return Err(FsError::NotADirectory("父路径不是目录".into()));
        }

        // 检查是否已存在
        if parent.children.iter().any(|e| e.name == name) {
            return Err(FsError::AlreadyExists(format!("{} 已存在", name)));
        }

        // 创建新目录 INode
        let inode_id = self.alloc_inode();
        let mut inode = INode::new_dir(inode_id, permissions);
        inode.parent = Some(parent_id);
        self.inodes.insert(inode_id, inode);

        // 添加到父目录
        if let Some(parent) = self.inodes.get_mut(&parent_id) {
            parent.children.push(DirEntry {
                name: name.to_string(),
                file_type: FileType::Directory,
                inode: inode_id,
            });
        }

        Ok(inode_id)
    }

    /// 删除文件
    pub fn remove_file(&mut self, path: &str) -> Result<(), FsError> {
        let (parent_id, name) = self.resolve_parent(path)?;

        // 查找条目索引和 inode_id
        let (idx, inode_id, _is_dir) = {
            let parent = self.inodes.get(&parent_id)
                .ok_or_else(|| FsError::NotFound(format!("父目录 INode {} 不存在", parent_id)))?;

            let idx = parent.children.iter().position(|e| e.name == name)
                .ok_or_else(|| FsError::NotFound(format!("'{}' 未找到", name)))?;

            let entry = &parent.children[idx];

            if entry.file_type == FileType::Directory {
                return Err(FsError::IsADirectory(format!("'{}' 是目录，请使用 remove_dir", name)));
            }

            (idx, entry.inode, false)
        };

        // 检查文件是否被打开
        for open_file in self.open_files.values() {
            if open_file.inode_id == inode_id {
                return Err(FsError::IoError(format!("'{}' 正在使用中", name)));
            }
        }

        // 移除 INode
        self.inodes.remove(&inode_id);
        // 移除目录条目
        if let Some(parent) = self.inodes.get_mut(&parent_id) {
            parent.children.remove(idx);
        }

        Ok(())
    }

    /// 删除目录
    pub fn remove_dir(&mut self, path: &str) -> Result<(), FsError> {
        let (parent_id, name) = self.resolve_parent(path)?;

        // 查找条目索引和 inode_id
        let (idx, inode_id, _is_empty) = {
            let parent = self.inodes.get(&parent_id)
                .ok_or_else(|| FsError::NotFound(format!("父目录 INode {} 不存在", parent_id)))?;

            let idx = parent.children.iter().position(|e| e.name == name)
                .ok_or_else(|| FsError::NotFound(format!("'{}' 未找到", name)))?;

            let entry = &parent.children[idx];

            if entry.file_type != FileType::Directory {
                return Err(FsError::NotADirectory(format!("'{}' 不是目录", name)));
            }

            // 检查目录是否为空
            let dir_inode = self.inodes.get(&entry.inode)
                .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", entry.inode)))?;

            let is_empty = dir_inode.children.is_empty();
            if !is_empty {
                return Err(FsError::DirectoryNotEmpty(format!("'{}' 目录非空", name)));
            }

            (idx, entry.inode, is_empty)
        };

        // 移除 INode
        self.inodes.remove(&inode_id);
        // 移除目录条目
        if let Some(parent) = self.inodes.get_mut(&parent_id) {
            parent.children.remove(idx);
        }

        Ok(())
    }

    /// 列出目录内容
    pub fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let inode_id = self.resolve_path(path)?;

        let inode = self.inodes.get(&inode_id)
            .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", inode_id)))?;

        if inode.file_type != FileType::Directory {
            return Err(FsError::NotADirectory(path.to_string()));
        }

        Ok(inode.children.clone())
    }

    /// 获取文件属性
    pub fn stat(&self, path: &str) -> Result<FileAttr, FsError> {
        let inode_id = self.resolve_path(path)?;

        let inode = self.inodes.get(&inode_id)
            .ok_or_else(|| FsError::NotFound(format!("INode {} 不存在", inode_id)))?;

        Ok(FileAttr {
            file_type: inode.file_type,
            size: inode.size,
            permissions: inode.permissions,
            created_at: inode.created_at,
            modified_at: inode.modified_at,
            accessed_at: inode.accessed_at,
            uid: 0,
            gid: 0,
            nlinks: 1,
        })
    }

    /// 获取文件系统统计信息
    pub fn stats(&self) -> FsStats {
        let total_inodes = self.next_inode;
        let used_inodes = self.inodes.len() as u64;
        let total_size = 1024 * 1024 * 1024; // 1GB 默认
        let used_size: u64 = self.inodes.values().map(|n| n.data.len() as u64).sum();
        let open_files = self.open_files.len() as u32;

        FsStats {
            total_inodes,
            used_inodes,
            total_size,
            used_size,
            open_files,
            fs_type: self.fs_type,
        }
    }
}

// === AgentFS (Agent 专用文件系统) ===

/// Agent 文件系统 - 每个 Agent 有独立命名空间
pub struct AgentFileSystem {
    /// Agent ID 到独立 VFS 的映射
    agent_vfs: BTreeMap<u64, VirtualFileSystem>,
    /// 共享区域 VFS
    shared_vfs: VirtualFileSystem,
}

impl AgentFileSystem {
    /// 创建新的 Agent 文件系统
    pub fn new() -> Self {
        let mut shared_vfs = VirtualFileSystem::new(FsType::AgentFS);
        shared_vfs.init_root();

        AgentFileSystem {
            agent_vfs: BTreeMap::new(),
            shared_vfs,
        }
    }

    /// 为 Agent 创建独立文件系统
    pub fn create_agent_fs(&mut self, agent_id: u64) -> Result<(), FsError> {
        if self.agent_vfs.contains_key(&agent_id) {
            return Err(FsError::AlreadyExists(format!("Agent {} 文件系统已存在", agent_id)));
        }

        let mut vfs = VirtualFileSystem::new(FsType::AgentFS);
        vfs.init_root();
        self.agent_vfs.insert(agent_id, vfs);
        Ok(())
    }

    /// 删除 Agent 文件系统
    pub fn remove_agent_fs(&mut self, agent_id: u64) -> Result<(), FsError> {
        self.agent_vfs.remove(&agent_id)
            .ok_or_else(|| FsError::NotFound(format!("Agent {} 文件系统不存在", agent_id)))?;
        Ok(())
    }

    /// 获取 Agent 的 VFS
    fn get_agent_vfs(&self, agent_id: u64) -> Result<&VirtualFileSystem, FsError> {
        self.agent_vfs.get(&agent_id)
            .ok_or_else(|| FsError::NotFound(format!("Agent {} 文件系统不存在", agent_id)))
    }

    /// 获取 Agent 的 VFS (可变)
    fn get_agent_vfs_mut(&mut self, agent_id: u64) -> Result<&mut VirtualFileSystem, FsError> {
        self.agent_vfs.get_mut(&agent_id)
            .ok_or_else(|| FsError::NotFound(format!("Agent {} 文件系统不存在", agent_id)))
    }

    /// Agent 打开文件
    pub fn agent_open(&mut self, agent_id: u64, path: &str, flags: OpenFlags) -> Result<FileDescriptor, FsError> {
        self.get_agent_vfs_mut(agent_id)?.open(path, flags)
    }

    /// Agent 读取文件
    pub fn agent_read(&mut self, agent_id: u64, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize, FsError> {
        self.get_agent_vfs_mut(agent_id)?.read(fd, buf)
    }

    /// Agent 写入文件
    pub fn agent_write(&mut self, agent_id: u64, fd: FileDescriptor, buf: &[u8]) -> Result<usize, FsError> {
        self.get_agent_vfs_mut(agent_id)?.write(fd, buf)
    }

    /// Agent 关闭文件
    pub fn agent_close(&mut self, agent_id: u64, fd: FileDescriptor) -> Result<(), FsError> {
        self.get_agent_vfs_mut(agent_id)?.close(fd)
    }

    /// Agent 创建文件
    pub fn agent_create_file(&mut self, agent_id: u64, path: &str, permissions: FilePermissions) -> Result<u64, FsError> {
        self.get_agent_vfs_mut(agent_id)?.create_file(path, permissions)
    }

    /// Agent 创建目录
    pub fn agent_create_dir(&mut self, agent_id: u64, path: &str, permissions: FilePermissions) -> Result<u64, FsError> {
        self.get_agent_vfs_mut(agent_id)?.create_dir(path, permissions)
    }

    /// Agent 列出目录
    pub fn agent_list_dir(&self, agent_id: u64, path: &str) -> Result<Vec<DirEntry>, FsError> {
        self.get_agent_vfs(agent_id)?.list_dir(path)
    }

    /// 共享区域打开文件
    pub fn shared_open(&mut self, path: &str, flags: OpenFlags) -> Result<FileDescriptor, FsError> {
        self.shared_vfs.open(path, flags)
    }

    /// 共享区域写入文件
    pub fn shared_write(&mut self, fd: FileDescriptor, buf: &[u8]) -> Result<usize, FsError> {
        self.shared_vfs.write(fd, buf)
    }

    /// 共享区域读取文件
    pub fn shared_read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize, FsError> {
        self.shared_vfs.read(fd, buf)
    }

    /// 共享区域关闭文件
    pub fn shared_close(&mut self, fd: FileDescriptor) -> Result<(), FsError> {
        self.shared_vfs.close(fd)
    }

    /// 共享区域创建文件
    pub fn shared_create_file(&mut self, path: &str, permissions: FilePermissions) -> Result<u64, FsError> {
        self.shared_vfs.create_file(path, permissions)
    }

    /// 共享区域列出目录
    pub fn shared_list_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        self.shared_vfs.list_dir(path)
    }

    /// 列出所有 Agent
    pub fn list_agents(&self) -> Vec<u64> {
        self.agent_vfs.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === FilePermissions 测试 ===

    #[test]
    fn test_file_permissions_new() {
        let perm = FilePermissions::new(0o644);
        assert_eq!(perm.mode(), 0o644);
    }

    #[test]
    fn test_file_permissions_is_readable() {
        let perm = FilePermissions::new(0o644);
        assert!(perm.is_readable());

        let perm_no_read = FilePermissions::new(0o022);
        assert!(!perm_no_read.is_readable());
    }

    #[test]
    fn test_file_permissions_is_writable() {
        let perm = FilePermissions::new(0o644);
        assert!(perm.is_writable());

        let perm_readonly = FilePermissions::new(0o444);
        assert!(!perm_readonly.is_writable());
    }

    #[test]
    fn test_file_permissions_is_executable() {
        let perm = FilePermissions::new(0o755);
        assert!(perm.is_executable());

        let perm_no_exec = FilePermissions::new(0o644);
        assert!(!perm_no_exec.is_executable());
    }

    #[test]
    fn test_file_permissions_bit_ops() {
        let perm1 = FilePermissions::new(0o400);
        let perm2 = FilePermissions::new(0o200);
        let combined = perm1 | perm2;
        assert_eq!(combined.mode(), 0o600);

        let anded = combined & FilePermissions::new(0o100);
        assert_eq!(anded.mode(), 0o000);

        let notted = !FilePermissions::new(0o000);
        assert_eq!(notted.mode(), 0o777);
    }

    #[test]
    fn test_file_permissions_default() {
        let file_perm = FilePermissions::default_file();
        assert_eq!(file_perm.mode(), 0o644);

        let dir_perm = FilePermissions::default_dir();
        assert_eq!(dir_perm.mode(), 0o755);
    }

    // === OpenFlags 测试 ===

    #[test]
    fn test_open_flags_readable() {
        let flags = OpenFlags::READ;
        assert!(flags.is_readable());
        assert!(!flags.is_writable());
    }

    #[test]
    fn test_open_flags_writable() {
        let flags = OpenFlags::WRITE;
        assert!(flags.is_writable());
    }

    #[test]
    fn test_open_flags_append() {
        let flags = OpenFlags::APPEND;
        assert!(flags.is_writable());
    }

    #[test]
    fn test_open_flags_contains() {
        let flags = OpenFlags::READ | OpenFlags::CREATE | OpenFlags::WRITE;
        assert!(flags.contains(OpenFlags::READ));
        assert!(flags.contains(OpenFlags::CREATE));
        assert!(flags.contains(OpenFlags::WRITE));
        assert!(!flags.contains(OpenFlags::APPEND));
    }

    // === FileType 测试 ===

    #[test]
    fn test_file_type_values() {
        assert_eq!(FileType::Regular as u8, 0);
        assert_eq!(FileType::Directory as u8, 1);
        assert_eq!(FileType::SymLink as u8, 2);
        assert_eq!(FileType::CharDevice as u8, 3);
        assert_eq!(FileType::BlockDevice as u8, 4);
        assert_eq!(FileType::FIFO as u8, 5);
        assert_eq!(FileType::Socket as u8, 6);
    }

    #[test]
    fn test_file_type_display() {
        assert_eq!(format!("{}", FileType::Regular), "普通文件");
        assert_eq!(format!("{}", FileType::Directory), "目录");
    }

    // === FsType 测试 ===

    #[test]
    fn test_fs_type_values() {
        assert_eq!(FsType::AgentFS as u8, 0);
        assert_eq!(FsType::TempFS as u8, 1);
        assert_eq!(FsType::MemFS as u8, 2);
        assert_eq!(FsType::Fat32 as u8, 3);
        assert_eq!(FsType::Ext2 as u8, 4);
    }

    // === INode 测试 ===

    #[test]
    fn test_inode_new() {
        let inode = INode::new(1, FileType::Regular, FilePermissions::new(0o644));
        assert_eq!(inode.id, 1);
        assert_eq!(inode.file_type, FileType::Regular);
        assert_eq!(inode.permissions.mode(), 0o644);
        assert_eq!(inode.size, 0);
        assert!(inode.data.is_empty());
        assert!(inode.children.is_empty());
    }

    #[test]
    fn test_inode_new_dir() {
        let inode = INode::new_dir(2, FilePermissions::new(0o755));
        assert_eq!(inode.id, 2);
        assert_eq!(inode.file_type, FileType::Directory);
        assert_eq!(inode.permissions.mode(), 0o755);
    }

    // === VirtualFileSystem 测试 ===

    #[test]
    fn test_vfs_init_root() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let root = vfs.inodes.get(&0).unwrap();
        assert_eq!(root.file_type, FileType::Directory);
        assert_eq!(root.permissions.mode(), 0o755);
    }

    #[test]
    fn test_vfs_create_file() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let inode_id = vfs.create_file("/hello.txt", FilePermissions::new(0o644)).unwrap();
        assert!(inode_id > 0);

        let inode = vfs.inodes.get(&inode_id).unwrap();
        assert_eq!(inode.file_type, FileType::Regular);
        assert_eq!(inode.permissions.mode(), 0o644);
    }

    #[test]
    fn test_vfs_create_file_duplicate() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/hello.txt", FilePermissions::new(0o644)).unwrap();
        let result = vfs.create_file("/hello.txt", FilePermissions::new(0o644));
        assert!(matches!(result, Err(FsError::AlreadyExists(_))));
    }

    #[test]
    fn test_vfs_create_dir() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let inode_id = vfs.create_dir("/mydir", FilePermissions::new(0o755)).unwrap();
        assert!(inode_id > 0);

        let inode = vfs.inodes.get(&inode_id).unwrap();
        assert_eq!(inode.file_type, FileType::Directory);
    }

    #[test]
    fn test_vfs_create_nested_dir() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_dir("/parent", FilePermissions::new(0o755)).unwrap();
        let inode_id = vfs.create_dir("/parent/child", FilePermissions::new(0o755)).unwrap();
        assert!(inode_id > 0);

        // 验证嵌套结构
        let entries = vfs.list_dir("/parent").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "child");
    }

    #[test]
    fn test_vfs_open_read_write_close() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        // 创建文件
        vfs.create_file("/test.txt", FilePermissions::new(0o644)).unwrap();

        // 打开文件 (带 CREATE 标志也可以)
        let fd = vfs.open("/test.txt", OpenFlags::READ | OpenFlags::WRITE).unwrap();

        // 写入数据
        let data = b"Hello, OmniAgent!";
        let written = vfs.write(fd, data).unwrap();
        assert_eq!(written, data.len());

        // 关闭文件
        vfs.close(fd).unwrap();

        // 重新打开并读取
        let fd2 = vfs.open("/test.txt", OpenFlags::READ).unwrap();
        let mut buf = [0u8; 64];
        let read = vfs.read(fd2, &mut buf).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buf[..read], data);

        vfs.close(fd2).unwrap();
    }

    #[test]
    fn test_vfs_open_create() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        // 使用 CREATE 标志打开不存在的文件
        let fd = vfs.open("/newfile.txt", OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE).unwrap();

        // 写入数据
        vfs.write(fd, b"test data").unwrap();
        vfs.close(fd).unwrap();

        // 验证文件存在
        let entries = vfs.list_dir("/").unwrap();
        assert!(entries.iter().any(|e| e.name == "newfile.txt"));
    }

    #[test]
    fn test_vfs_open_not_found() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let result = vfs.open("/nonexistent.txt", OpenFlags::READ);
        assert!(matches!(result, Err(FsError::NotFound(_))));
    }

    #[test]
    fn test_vfs_seek() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/seek_test.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/seek_test.txt", OpenFlags::READ | OpenFlags::WRITE).unwrap();

        vfs.write(fd, b"0123456789").unwrap();

        // Seek 到起始位置
        let pos = vfs.seek(fd, SeekFrom::Start(5)).unwrap();
        assert_eq!(pos, 5);

        // 读取从位置 5 开始的数据
        let mut buf = [0u8; 5];
        let read = vfs.read(fd, &mut buf).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buf, b"56789");

        // Seek 到末尾前 3 个字节
        let pos = vfs.seek(fd, SeekFrom::End(-3)).unwrap();
        assert_eq!(pos, 7);

        let mut buf2 = [0u8; 3];
        let read2 = vfs.read(fd, &mut buf2).unwrap();
        assert_eq!(read2, 3);
        assert_eq!(&buf2, b"789");

        vfs.close(fd).unwrap();
    }

    #[test]
    fn test_vfs_seek_invalid_offset() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/seek_test.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/seek_test.txt", OpenFlags::READ | OpenFlags::WRITE).unwrap();

        vfs.write(fd, b"hello").unwrap();

        // 负偏移量应失败
        let result = vfs.seek(fd, SeekFrom::End(-100));
        assert_eq!(result, Err(FsError::InvalidOffset));

        vfs.close(fd).unwrap();
    }

    #[test]
    fn test_vfs_list_dir() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/a.txt", FilePermissions::new(0o644)).unwrap();
        vfs.create_file("/b.txt", FilePermissions::new(0o644)).unwrap();
        vfs.create_dir("/subdir", FilePermissions::new(0o755)).unwrap();

        let entries = vfs.list_dir("/").unwrap();
        assert_eq!(entries.len(), 3);

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"subdir"));
    }

    #[test]
    fn test_vfs_list_dir_empty() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let entries = vfs.list_dir("/").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_vfs_stat() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/stat_test.txt", FilePermissions::new(0o644)).unwrap();

        // 写入一些数据
        let fd = vfs.open("/stat_test.txt", OpenFlags::READ | OpenFlags::WRITE).unwrap();
        vfs.write(fd, b"hello world").unwrap();
        vfs.close(fd).unwrap();

        let attr = vfs.stat("/stat_test.txt").unwrap();
        assert_eq!(attr.file_type, FileType::Regular);
        assert_eq!(attr.size, 11);
        assert_eq!(attr.permissions.mode(), 0o644);
    }

    #[test]
    fn test_vfs_remove_file() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/to_remove.txt", FilePermissions::new(0o644)).unwrap();
        vfs.remove_file("/to_remove.txt").unwrap();

        let entries = vfs.list_dir("/").unwrap();
        assert!(entries.iter().all(|e| e.name != "to_remove.txt"));

        // 再次删除应报错
        let result = vfs.remove_file("/to_remove.txt");
        assert!(matches!(result, Err(FsError::NotFound(_))));
    }

    #[test]
    fn test_vfs_remove_dir() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_dir("/empty_dir", FilePermissions::new(0o755)).unwrap();
        vfs.remove_dir("/empty_dir").unwrap();

        let entries = vfs.list_dir("/").unwrap();
        assert!(entries.iter().all(|e| e.name != "empty_dir"));
    }

    #[test]
    fn test_vfs_remove_dir_not_empty() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_dir("/nonempty", FilePermissions::new(0o755)).unwrap();
        vfs.create_file("/nonempty/file.txt", FilePermissions::new(0o644)).unwrap();

        let result = vfs.remove_dir("/nonempty");
        assert!(matches!(result, Err(FsError::DirectoryNotEmpty(_))));
    }

    #[test]
    fn test_vfs_remove_file_is_directory() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_dir("/adir", FilePermissions::new(0o755)).unwrap();

        let result = vfs.remove_file("/adir");
        assert!(matches!(result, Err(FsError::IsADirectory(_))));
    }

    #[test]
    fn test_vfs_stats() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/f1.txt", FilePermissions::new(0o644)).unwrap();
        vfs.create_dir("/d1", FilePermissions::new(0o755)).unwrap();

        let stats = vfs.stats();
        assert_eq!(stats.fs_type, FsType::MemFS);
        assert_eq!(stats.used_inodes, 3); // root + f1 + d1
        assert_eq!(stats.open_files, 0);
    }

    // === 路径解析测试 ===

    #[test]
    fn test_resolve_path_absolute() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_dir("/a", FilePermissions::new(0o755)).unwrap();
        vfs.create_dir("/a/b", FilePermissions::new(0o755)).unwrap();
        vfs.create_file("/a/b/c.txt", FilePermissions::new(0o644)).unwrap();

        let inode_id = vfs.resolve_path("/a/b/c.txt").unwrap();
        let inode = vfs.inodes.get(&inode_id).unwrap();
        assert_eq!(inode.file_type, FileType::Regular);
    }

    #[test]
    fn test_resolve_path_root() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let inode_id = vfs.resolve_path("/").unwrap();
        assert_eq!(inode_id, 0);
    }

    #[test]
    fn test_resolve_path_not_found() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let result = vfs.resolve_path("/nonexistent/path");
        assert!(matches!(result, Err(FsError::NotFound(_))));
    }

    #[test]
    fn test_resolve_path_not_directory() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/file.txt", FilePermissions::new(0o644)).unwrap();

        let result = vfs.resolve_path("/file.txt/something");
        assert!(matches!(result, Err(FsError::NotADirectory(_))));
    }

    #[test]
    fn test_resolve_path_empty() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let result = vfs.resolve_path("");
        assert!(matches!(result, Err(FsError::InvalidPath(_))));
    }

    // === 错误处理测试 ===

    #[test]
    fn test_error_not_open() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let result = vfs.close(FileDescriptor(999));
        assert_eq!(result, Err(FsError::NotOpen(FileDescriptor(999))));
    }

    #[test]
    fn test_error_read_not_open() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        let mut buf = [0u8; 10];
        let result = vfs.read(FileDescriptor(999), &mut buf);
        assert_eq!(result, Err(FsError::NotOpen(FileDescriptor(999))));
    }

    #[test]
    fn test_error_write_readonly() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/readonly.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/readonly.txt", OpenFlags::READ).unwrap();

        let result = vfs.write(fd, b"test");
        assert!(matches!(result, Err(FsError::PermissionDenied(_))));

        vfs.close(fd).unwrap();
    }

    #[test]
    fn test_error_read_writable_only() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/writeonly.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/writeonly.txt", OpenFlags::WRITE).unwrap();

        let mut buf = [0u8; 10];
        let result = vfs.read(fd, &mut buf);
        assert!(matches!(result, Err(FsError::PermissionDenied(_))));

        vfs.close(fd).unwrap();
    }

    #[test]
    fn test_error_display() {
        let err = FsError::NotFound("test.txt".into());
        assert_eq!(format!("{}", err), "未找到: test.txt");

        let err = FsError::PermissionDenied("无权限".into());
        assert_eq!(format!("{}", err), "权限不足: 无权限");

        let err = FsError::DirectoryNotEmpty("mydir".into());
        assert_eq!(format!("{}", err), "目录非空: mydir");
    }

    #[test]
    fn test_vfs_open_truncate() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        // 创建文件并写入数据
        vfs.create_file("/truncate.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/truncate.txt", OpenFlags::READ | OpenFlags::WRITE).unwrap();
        vfs.write(fd, b"Hello World!").unwrap();
        vfs.close(fd).unwrap();

        // 用 TRUNCATE 标志重新打开
        let fd2 = vfs.open("/truncate.txt", OpenFlags::READ | OpenFlags::WRITE | OpenFlags::TRUNCATE).unwrap();

        // 文件应该被清空
        let mut buf = [0u8; 64];
        let read = vfs.read(fd2, &mut buf).unwrap();
        assert_eq!(read, 0);

        vfs.close(fd2).unwrap();
    }

    #[test]
    fn test_vfs_append() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/append.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/append.txt", OpenFlags::WRITE | OpenFlags::APPEND).unwrap();

        vfs.write(fd, b"Hello ").unwrap();
        vfs.write(fd, b"World!").unwrap();
        vfs.close(fd).unwrap();

        // 读取验证
        let fd2 = vfs.open("/append.txt", OpenFlags::READ).unwrap();
        let mut buf = [0u8; 64];
        let read = vfs.read(fd2, &mut buf).unwrap();
        assert_eq!(&buf[..read], b"Hello World!");
        vfs.close(fd2).unwrap();
    }

    #[test]
    fn test_vfs_read_eof() {
        let mut vfs = VirtualFileSystem::new(FsType::MemFS);
        vfs.init_root();

        vfs.create_file("/eof.txt", FilePermissions::new(0o644)).unwrap();
        let fd = vfs.open("/eof.txt", OpenFlags::READ).unwrap();

        let mut buf = [0u8; 10];
        let read = vfs.read(fd, &mut buf).unwrap();
        assert_eq!(read, 0); // 空文件，EOF

        vfs.close(fd).unwrap();
    }

    // === AgentFileSystem 测试 ===

    #[test]
    fn test_agent_fs_create() {
        let mut afs = AgentFileSystem::new();
        afs.create_agent_fs(1).unwrap();
        afs.create_agent_fs(2).unwrap();

        let agents = afs.list_agents();
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&1));
        assert!(agents.contains(&2));
    }

    #[test]
    fn test_agent_fs_duplicate() {
        let mut afs = AgentFileSystem::new();
        afs.create_agent_fs(1).unwrap();

        let result = afs.create_agent_fs(1);
        assert!(matches!(result, Err(FsError::AlreadyExists(_))));
    }

    #[test]
    fn test_agent_fs_remove() {
        let mut afs = AgentFileSystem::new();
        afs.create_agent_fs(1).unwrap();
        afs.remove_agent_fs(1).unwrap();

        assert!(afs.list_agents().is_empty());
    }

    #[test]
    fn test_agent_fs_remove_nonexistent() {
        let mut afs = AgentFileSystem::new();
        let result = afs.remove_agent_fs(999);
        assert!(matches!(result, Err(FsError::NotFound(_))));
    }

    #[test]
    fn test_agent_fs_isolated_namespaces() {
        let mut afs = AgentFileSystem::new();
        afs.create_agent_fs(1).unwrap();
        afs.create_agent_fs(2).unwrap();

        // Agent 1 创建文件
        let fd1 = afs.agent_open(1, "/agent1_file.txt", OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE).unwrap();
        afs.agent_write(1, fd1, b"Agent 1 data").unwrap();
        afs.agent_close(1, fd1).unwrap();

        // Agent 2 不应该看到 Agent 1 的文件
        let result = afs.agent_open(2, "/agent1_file.txt", OpenFlags::READ);
        assert!(matches!(result, Err(FsError::NotFound(_))));

        // Agent 2 创建自己的文件
        let fd2 = afs.agent_open(2, "/agent2_file.txt", OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE).unwrap();
        afs.agent_write(2, fd2, b"Agent 2 data").unwrap();
        afs.agent_close(2, fd2).unwrap();

        // Agent 1 不应该看到 Agent 2 的文件
        let result = afs.agent_open(1, "/agent2_file.txt", OpenFlags::READ);
        assert!(matches!(result, Err(FsError::NotFound(_))));
    }

    #[test]
    fn test_agent_fs_read_write() {
        let mut afs = AgentFileSystem::new();
        afs.create_agent_fs(42).unwrap();

        let fd = afs.agent_open(42, "/data.bin", OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE).unwrap();
        let written = afs.agent_write(42, fd, b"\x01\x02\x03\x04\x05").unwrap();
        assert_eq!(written, 5);

        // 需要重新打开或 seek 才能读取刚写入的数据
        // 这里我们关闭后重新打开
        afs.agent_close(42, fd).unwrap();

        let fd2 = afs.agent_open(42, "/data.bin", OpenFlags::READ).unwrap();
        let mut buf = [0u8; 5];
        let read = afs.agent_read(42, fd2, &mut buf).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buf, b"\x01\x02\x03\x04\x05");
        afs.agent_close(42, fd2).unwrap();
    }

    #[test]
    fn test_agent_fs_nonexistent_agent() {
        let mut afs = AgentFileSystem::new();

        let result = afs.agent_open(999, "/test.txt", OpenFlags::READ);
        assert!(matches!(result, Err(FsError::NotFound(_))));
    }

    #[test]
    fn test_shared_fs_operations() {
        let mut afs = AgentFileSystem::new();

        // 共享区域创建文件
        let fd = afs.shared_open("/shared.txt", OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE).unwrap();
        afs.shared_write(fd, b"Shared data").unwrap();
        afs.shared_close(fd).unwrap();

        // 读取共享文件
        let fd2 = afs.shared_open("/shared.txt", OpenFlags::READ).unwrap();
        let mut buf = [0u8; 64];
        let read = afs.shared_read(fd2, &mut buf).unwrap();
        assert_eq!(&buf[..read], b"Shared data");
        afs.shared_close(fd2).unwrap();
    }

    #[test]
    fn test_shared_fs_list_dir() {
        let mut afs = AgentFileSystem::new();

        afs.shared_create_file("/s1.txt", FilePermissions::new(0o644)).unwrap();
        afs.shared_create_file("/s2.txt", FilePermissions::new(0o644)).unwrap();

        let entries = afs.shared_list_dir("/").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_agent_fs_list_dir() {
        let mut afs = AgentFileSystem::new();
        afs.create_agent_fs(1).unwrap();

        afs.agent_create_file(1, "/a.txt", FilePermissions::new(0o644)).unwrap();
        afs.agent_create_dir(1, "/mydir", FilePermissions::new(0o755)).unwrap();

        let entries = afs.agent_list_dir(1, "/").unwrap();
        assert_eq!(entries.len(), 2);
    }
}
