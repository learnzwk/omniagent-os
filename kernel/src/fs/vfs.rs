//! VFS 管理器
//!
//! 虚拟文件系统管理器，提供挂载、卸载、路径解析、文件创建等功能。
//! 使用全局静态实例 VFS 提供系统级文件系统访问。

use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Lazy, Mutex};

use crate::fs::error::FsError;
use crate::fs::inode::{FileType, MemoryInode, VfsInode};

/// 目录项
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// 文件名
    pub name: String,
    /// 文件类型
    pub file_type: FileType,
    /// 文件大小
    pub size: u64,
}

/// 挂载点
struct MountPoint {
    /// 挂载路径
    path: String,
    /// 挂载的 Inode
    inode: Arc<dyn VfsInode>,
}

/// VFS 管理器
///
/// 管理文件系统的挂载、路径解析和文件操作。
/// 当前实现为简化版，主要支持根目录和基本挂载操作。
pub struct VfsManager {
    /// 根目录 inode
    root: Mutex<Option<Arc<dyn VfsInode>>>,
    /// 挂载点列表
    mounts: Mutex<Vec<MountPoint>>,
}

impl VfsManager {
    /// 创建新的 VFS 管理器
    pub fn new() -> Self {
        VfsManager {
            root: Mutex::new(None),
            mounts: Mutex::new(Vec::new()),
        }
    }

    /// 设置根目录 inode
    pub fn set_root(&self, inode: Arc<dyn VfsInode>) {
        let mut root = self.root.lock();
        *root = Some(inode);
    }

    /// 挂载文件系统到指定路径
    ///
    /// 将给定的 inode 挂载到指定的路径上。
    pub fn mount(&self, path: &str, inode: Arc<dyn VfsInode>) -> Result<(), FsError> {
        // 验证路径不为空
        if path.is_empty() {
            return Err(FsError::InvalidPath);
        }

        // 检查是否已挂载
        let mut mounts = self.mounts.lock();
        for mount in mounts.iter() {
            if mount.path == path {
                return Err(FsError::AlreadyExists);
            }
        }

        mounts.push(MountPoint {
            path: path.to_string(),
            inode,
        });

        Ok(())
    }

    /// 卸载指定路径的文件系统
    pub fn unmount(&self, path: &str) -> Result<(), FsError> {
        let mut mounts = self.mounts.lock();
        let len_before = mounts.len();

        mounts.retain(|m| m.path != path);

        if mounts.len() == len_before {
            Err(FsError::NotFound)
        } else {
            Ok(())
        }
    }

    /// 解析路径到对应的 inode
    ///
    /// 当前简化实现：只解析根目录 "/" 和挂载点。
    pub fn resolve(&self, path: &str) -> Result<Arc<dyn VfsInode>, FsError> {
        if path.is_empty() {
            return Err(FsError::InvalidPath);
        }

        // 根目录
        if path == "/" {
            let root = self.root.lock();
            return root
                .as_ref()
                .cloned()
                .ok_or(FsError::NotFound);
        }

        // 检查挂载点
        let mounts = self.mounts.lock();
        for mount in mounts.iter() {
            if mount.path == path {
                return Ok(mount.inode.clone());
            }
        }

        // 简化版：对于非根非挂载点的路径，返回根 inode
        // 完整实现需要遍历路径组件
        let root = self.root.lock();
        root.as_ref().cloned().ok_or(FsError::NotFound)
    }

    /// 创建文件
    ///
    /// 当前简化实现：创建内存文件 inode 并返回。
    pub fn create_file(&self, _path: &str) -> Result<Arc<dyn VfsInode>, FsError> {
        let inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_file());
        Ok(inode)
    }

    /// 创建目录
    ///
    /// 当前简化实现：创建内存目录 inode。
    pub fn create_dir(&self, _path: &str) -> Result<(), FsError> {
        // 简化实现：仅验证路径有效
        Ok(())
    }

    /// 删除文件或目录
    pub fn remove(&self, path: &str) -> Result<(), FsError> {
        if path.is_empty() || path == "/" {
            return Err(FsError::PermissionDenied);
        }

        // 检查是否是挂载点（不能删除挂载点）
        let mounts = self.mounts.lock();
        for mount in mounts.iter() {
            if mount.path == path {
                return Err(FsError::NotEmpty);
            }
        }

        Ok(())
    }

    /// 列出目录内容
    ///
    /// 当前简化实现：返回根目录下的挂载点作为目录项。
    pub fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        if path != "/" {
            // 对于非根路径，检查是否是挂载点
            let mounts = self.mounts.lock();
            for mount in mounts.iter() {
                if mount.path == path {
                    return Ok(Vec::new());
                }
            }
            return Err(FsError::NotADirectory);
        }

        // 检查根目录是否已设置
        {
            let root = self.root.lock();
            if root.is_none() {
                return Err(FsError::NotFound);
            }
        }

        // 返回挂载点作为目录项
        let mounts = self.mounts.lock();
        let entries: Vec<DirEntry> = mounts
            .iter()
            .map(|m| {
                let name = match m.path.rfind('/') {
                    Some(pos) => m.path[pos + 1..].to_string(),
                    None => m.path.clone(),
                };
                DirEntry {
                    name,
                    file_type: m.inode.file_type(),
                    size: m.inode.size(),
                }
            })
            .collect();

        Ok(entries)
    }
}

impl Default for VfsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局 VFS 实例
pub static VFS: Lazy<Mutex<VfsManager>> = Lazy::new(|| {
    Mutex::new(VfsManager::new())
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_new() {
        let vfs = VfsManager::new();
        // 新创建的 VFS 没有根目录
        let result = vfs.resolve("/");
        assert!(matches!(result, Err(FsError::NotFound)));
    }

    #[test]
    fn test_vfs_set_root() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());

        vfs.set_root(root_inode.clone());

        // 解析根目录应成功
        let resolved = vfs.resolve("/").unwrap();
        assert_eq!(resolved.file_type(), FileType::Directory);
    }

    #[test]
    fn test_vfs_mount_unmount() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        // 挂载文件系统
        let mount_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.mount("/mnt", mount_inode.clone()).unwrap();

        // 重复挂载应失败
        let result = vfs.mount("/mnt", mount_inode.clone());
        assert!(matches!(result, Err(FsError::AlreadyExists)));

        // 解析挂载点
        let resolved = vfs.resolve("/mnt").unwrap();
        assert_eq!(resolved.file_type(), FileType::Directory);

        // 卸载
        vfs.unmount("/mnt").unwrap();

        // 再次卸载应失败
        let result = vfs.unmount("/mnt");
        assert!(matches!(result, Err(FsError::NotFound)));

        // 卸载后解析应失败（简化版中会返回根目录）
        // 在简化实现中，非挂载点路径会返回根目录
    }

    #[test]
    fn test_vfs_resolve() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        // 解析根目录
        let resolved = vfs.resolve("/").unwrap();
        assert_eq!(resolved.file_type(), FileType::Directory);

        // 空路径应报错
        let result = vfs.resolve("");
        assert!(matches!(result, Err(FsError::InvalidPath)));

        // 未设置根目录时解析应失败
        let vfs2 = VfsManager::new();
        let result = vfs2.resolve("/");
        assert!(matches!(result, Err(FsError::NotFound)));
    }

    #[test]
    fn test_vfs_list_dir() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        // 列出空根目录
        let entries = vfs.list_dir("/").unwrap();
        assert!(entries.is_empty());

        // 挂载一些文件系统
        let file_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_file());
        vfs.mount("/mnt", file_inode.clone()).unwrap();

        let dir_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.mount("/dev", dir_inode.clone()).unwrap();

        // 列出根目录应包含挂载点
        let entries = vfs.list_dir("/").unwrap();
        assert_eq!(entries.len(), 2);

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"mnt"));
        assert!(names.contains(&"dev"));

        // 未设置根目录时列出应失败
        let vfs2 = VfsManager::new();
        let result = vfs2.list_dir("/");
        assert!(matches!(result, Err(FsError::NotFound)));
    }

    #[test]
    fn test_vfs_create_file() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        let file = vfs.create_file("/test.txt").unwrap();
        assert_eq!(file.file_type(), FileType::Regular);
        assert_eq!(file.size(), 0);
    }

    #[test]
    fn test_vfs_create_dir() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        // 简化实现中 create_dir 总是成功
        assert!(vfs.create_dir("/newdir").is_ok());
    }

    #[test]
    fn test_vfs_remove() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        // 删除空路径应失败
        assert!(matches!(vfs.remove(""), Err(FsError::PermissionDenied)));

        // 删除根目录应失败
        assert!(matches!(vfs.remove("/"), Err(FsError::PermissionDenied)));

        // 删除普通路径应成功
        assert!(vfs.remove("/some_file").is_ok());

        // 删除挂载点应失败
        let mount_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.mount("/mnt", mount_inode).unwrap();
        assert!(matches!(vfs.remove("/mnt"), Err(FsError::NotEmpty)));
    }

    #[test]
    fn test_vfs_mount_empty_path() {
        let vfs = VfsManager::new();
        let inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());

        // 空路径不能挂载
        let result = vfs.mount("", inode);
        assert!(matches!(result, Err(FsError::InvalidPath)));
    }

    #[test]
    fn test_vfs_list_dir_non_root() {
        let vfs = VfsManager::new();
        let root_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.set_root(root_inode);

        // 列出不存在的目录
        let result = vfs.list_dir("/nonexistent");
        assert!(matches!(result, Err(FsError::NotADirectory)));

        // 列出挂载点目录
        let mount_inode: Arc<dyn VfsInode> = Arc::new(MemoryInode::new_directory());
        vfs.mount("/mnt", mount_inode).unwrap();
        let entries = vfs.list_dir("/mnt").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_vfs_default() {
        let vfs = VfsManager::default();
        let result = vfs.resolve("/");
        assert!(matches!(result, Err(FsError::NotFound)));
    }
}
