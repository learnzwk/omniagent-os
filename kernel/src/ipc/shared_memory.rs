//! 共享内存池模块
//! 模仿鸿蒙零拷贝 IPC 的共享内存管理，支持内存分配、释放、调整和所有权转移

use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use spin::Mutex;

use crate::ipc::error::IpcError;

/// 共享内存区域
#[derive(Debug)]
pub struct SharedMemoryRegion {
    /// 区域唯一标识
    pub id: u64,
    /// 基地址（模拟）
    pub base_addr: usize,
    /// 区域大小（字节）
    pub size: usize,
    /// 拥有者任务/Agent ID
    pub owner: u64,
    /// 引用计数
    pub ref_count: AtomicUsize,
    /// 权限标志（读/写/执行）
    pub permissions: u32,
}

impl Clone for SharedMemoryRegion {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            base_addr: self.base_addr,
            size: self.size,
            owner: self.owner,
            ref_count: AtomicUsize::new(self.ref_count.load(Ordering::SeqCst)),
            permissions: self.permissions,
        }
    }
}

/// 共享内存池
/// 管理所有共享内存区域的分配、释放和生命周期
pub struct SharedMemoryPool {
    /// 共享内存区域表
    regions: Mutex<BTreeMap<u64, SharedMemoryRegion>>,
    /// 总内存大小
    total_size: AtomicUsize,
    /// 已使用内存大小
    used_size: AtomicUsize,
    /// 下一个区域 ID
    next_id: AtomicU64,
    /// 最大区域数量
    max_regions: usize,
}

impl SharedMemoryPool {
    /// 创建新的共享内存池
    ///
    /// # 参数
    /// - `max_regions`: 最大区域数量
    pub fn new(max_regions: usize) -> Self {
        Self {
            regions: Mutex::new(BTreeMap::new()),
            total_size: AtomicUsize::new(0),
            used_size: AtomicUsize::new(0),
            next_id: AtomicU64::new(1),
            max_regions,
        }
    }

    /// 分配共享内存区域
    ///
    /// # 参数
    /// - `size`: 请求的内存大小（字节）
    /// - `owner`: 拥有者 ID
    ///
    /// # 返回
    /// 成功返回分配的区域 ID
    pub fn allocate(&self, size: usize, owner: u64) -> Result<u64, IpcError> {
        if size == 0 {
            return Err(IpcError::InvalidSize(size));
        }

        let mut regions = self.regions.lock();
        if regions.len() >= self.max_regions {
            return Err(IpcError::OutOfMemory);
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let base_addr = 0x1000_0000 + (id as usize) * 0x1000; // 模拟基地址

        let region = SharedMemoryRegion {
            id,
            base_addr,
            size,
            owner,
            ref_count: AtomicUsize::new(1),
            permissions: 0x06, // 默认读写权限
        };

        regions.insert(id, region);
        self.used_size.fetch_add(size, Ordering::SeqCst);
        self.total_size.fetch_add(size, Ordering::SeqCst);

        Ok(id)
    }

    /// 释放共享内存区域
    ///
    /// # 参数
    /// - `id`: 要释放的区域 ID
    pub fn deallocate(&self, id: u64) -> Result<(), IpcError> {
        let mut regions = self.regions.lock();
        if let Some(region) = regions.remove(&id) {
            self.used_size.fetch_sub(region.size, Ordering::SeqCst);
            self.total_size.fetch_sub(region.size, Ordering::SeqCst);
            Ok(())
        } else {
            Err(IpcError::InvalidHandle(id))
        }
    }

    /// 获取共享内存区域信息
    ///
    /// # 参数
    /// - `id`: 区域 ID
    pub fn get(&self, id: u64) -> Option<SharedMemoryRegion> {
        let regions = self.regions.lock();
        regions.get(&id).cloned()
    }

    /// 调整共享内存区域大小
    ///
    /// # 参数
    /// - `id`: 区域 ID
    /// - `new_size`: 新的大小
    pub fn resize(&self, id: u64, new_size: usize) -> Result<(), IpcError> {
        if new_size == 0 {
            return Err(IpcError::InvalidSize(new_size));
        }

        let mut regions = self.regions.lock();
        if let Some(region) = regions.get_mut(&id) {
            let old_size = region.size;
            self.used_size.fetch_sub(old_size, Ordering::SeqCst);
            self.used_size.fetch_add(new_size, Ordering::SeqCst);
            region.size = new_size;
            Ok(())
        } else {
            Err(IpcError::InvalidHandle(id))
        }
    }

    /// 转移共享内存区域所有权
    ///
    /// # 参数
    /// - `id`: 区域 ID
    /// - `new_owner`: 新拥有者 ID
    pub fn transfer(&self, id: u64, new_owner: u64) -> Result<(), IpcError> {
        let mut regions = self.regions.lock();
        if let Some(region) = regions.get_mut(&id) {
            region.owner = new_owner;
            Ok(())
        } else {
            Err(IpcError::InvalidHandle(id))
        }
    }

    /// 获取总内存大小
    pub fn total_size(&self) -> usize {
        self.total_size.load(Ordering::SeqCst)
    }

    /// 获取已使用内存大小
    pub fn used_size(&self) -> usize {
        self.used_size.load(Ordering::SeqCst)
    }

    /// 获取区域数量
    pub fn region_count(&self) -> usize {
        let regions = self.regions.lock();
        regions.len()
    }

    /// 获取统计信息
    ///
    /// # 返回
    /// (总大小, 已使用大小, 区域数量)
    pub fn stats(&self) -> (usize, usize, usize) {
        (
            self.total_size(),
            self.used_size(),
            self.region_count(),
        )
    }
}

/// 全局共享内存池实例
pub static SHM_POOL: spin::Lazy<Mutex<SharedMemoryPool>> = spin::Lazy::new(|| {
    Mutex::new(SharedMemoryPool::new(256))
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate() {
        let pool = SharedMemoryPool::new(256);
        let id = pool.allocate(4096, 1).unwrap();
        assert_eq!(id, 1);

        let region = pool.get(id).unwrap();
        assert_eq!(region.size, 4096);
        assert_eq!(region.owner, 1);
        assert_eq!(region.ref_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_allocate_multiple() {
        let pool = SharedMemoryPool::new(256);
        let id1 = pool.allocate(1024, 1).unwrap();
        let id2 = pool.allocate(2048, 2).unwrap();
        let id3 = pool.allocate(4096, 3).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(pool.region_count(), 3);
    }

    #[test]
    fn test_deallocate() {
        let pool = SharedMemoryPool::new(256);
        let id = pool.allocate(4096, 1).unwrap();
        assert_eq!(pool.region_count(), 1);

        assert!(pool.deallocate(id).is_ok());
        assert_eq!(pool.region_count(), 0);
        assert_eq!(pool.used_size(), 0);

        // 释放不存在的区域应返回错误
        let result = pool.deallocate(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get() {
        let pool = SharedMemoryPool::new(256);
        let id = pool.allocate(8192, 42).unwrap();

        let region = pool.get(id);
        assert!(region.is_some());
        let region = region.unwrap();
        assert_eq!(region.id, id);
        assert_eq!(region.owner, 42);
        assert_eq!(region.size, 8192);

        // 获取不存在的区域
        let not_found = pool.get(999);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_resize() {
        let pool = SharedMemoryPool::new(256);
        let id = pool.allocate(1024, 1).unwrap();
        assert_eq!(pool.used_size(), 1024);

        assert!(pool.resize(id, 2048).is_ok());
        assert_eq!(pool.used_size(), 2048);

        let region = pool.get(id).unwrap();
        assert_eq!(region.size, 2048);

        // 调整不存在的区域大小应返回错误
        let result = pool.resize(999, 4096);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer() {
        let pool = SharedMemoryPool::new(256);
        let id = pool.allocate(4096, 1).unwrap();

        assert!(pool.transfer(id, 2).is_ok());
        let region = pool.get(id).unwrap();
        assert_eq!(region.owner, 2);

        // 转移不存在的区域应返回错误
        let result = pool.transfer(999, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_stats() {
        let pool = SharedMemoryPool::new(256);
        pool.allocate(1024, 1).unwrap();
        pool.allocate(2048, 2).unwrap();

        let (total, used, count) = pool.stats();
        assert_eq!(total, 3072);
        assert_eq!(used, 3072);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_max_regions() {
        let pool = SharedMemoryPool::new(2);
        pool.allocate(1024, 1).unwrap();
        pool.allocate(1024, 2).unwrap();

        // 超过最大区域数量应返回错误
        let result = pool.allocate(1024, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_id() {
        let pool = SharedMemoryPool::new(256);

        // 对不存在的 ID 操作应返回错误
        assert!(pool.deallocate(1).is_err());
        assert!(pool.get(1).is_none());
        assert!(pool.resize(1, 100).is_err());
        assert!(pool.transfer(1, 2).is_err());
    }

    #[test]
    fn test_double_deallocate() {
        let pool = SharedMemoryPool::new(256);
        let id = pool.allocate(4096, 1).unwrap();

        assert!(pool.deallocate(id).is_ok());
        // 重复释放应返回错误
        let result = pool.deallocate(id);
        assert!(result.is_err());
    }
}
