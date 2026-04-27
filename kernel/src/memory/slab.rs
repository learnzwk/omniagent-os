//! Slab 分配器模块
//!
//! 实现内核对象的 Slab 分配，提供高效的固定大小对象分配/释放。
//! 使用空闲链表（free list）实现 O(1) 的分配和释放操作。
//! 内部使用 Vec<u8> 作为后备内存池（模拟 slab 页面）。

use core::fmt;

#[cfg(test)]
use std::vec::Vec;
#[cfg(test)]
use std::string::String;
#[cfg(not(test))]
use alloc::vec::Vec;
#[cfg(not(test))]
use alloc::string::String;
#[cfg(not(test))]
use alloc::string::ToString;

use spin::Mutex;

/// 默认的 slab 页面大小（字节）
const SLAB_PAGE_SIZE: usize = 4096;

/// kmalloc 预定义缓存大小列表
const KMALLOC_SIZES: &[(usize, &str)] = &[
    (32, "kmalloc_32"),
    (64, "kmalloc_64"),
    (128, "kmalloc_128"),
    (256, "kmalloc_256"),
    (512, "kmalloc_512"),
    (1024, "kmalloc_1024"),
    (2048, "kmalloc_2048"),
    (4096, "kmalloc_4096"),
];

/// kmalloc 默认对齐
const KMALLOC_DEFAULT_ALIGN: usize = 8;

/// Slab 分配器错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum SlabError {
    /// 缓存名无效（空字符串）
    InvalidCacheName,
    /// 缓存未找到
    CacheNotFound(String),
    /// 无效指针（空指针或不属于任何缓存）
    InvalidPointer,
    /// 分配失败（缓存已满且无法扩展）
    AllocationFailed,
    /// 无效对齐（非零且非 2 的幂）
    InvalidAlignment,
    /// 缓存已存在
    CacheAlreadyExists(String),
    /// 对象大小为零
    ZeroSize,
}

impl fmt::Display for SlabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlabError::InvalidCacheName => write!(f, "无效的缓存名称"),
            SlabError::CacheNotFound(name) => write!(f, "缓存未找到: {}", name),
            SlabError::InvalidPointer => write!(f, "无效的指针"),
            SlabError::AllocationFailed => write!(f, "分配失败：缓存已满且无法扩展"),
            SlabError::InvalidAlignment => write!(f, "无效的对齐值，对齐必须是 2 的幂"),
            SlabError::CacheAlreadyExists(name) => write!(f, "缓存已存在: {}", name),
            SlabError::ZeroSize => write!(f, "对象大小不能为零"),
        }
    }
}

/// Slab 缓存统计信息
#[derive(Debug, Clone)]
pub struct SlabCacheStats {
    /// 缓存名称
    pub name: String,
    /// 对象大小（字节）
    pub object_size: usize,
    /// 对齐要求
    pub align: usize,
    /// 总对象数（空闲 + 已使用）
    pub total_objects: usize,
    /// 空闲对象数
    pub free_objects: usize,
    /// 已使用对象数
    pub used_objects: usize,
    /// Slab 页面数量
    pub slab_count: usize,
}

/// 单个 Slab 缓存，管理固定大小对象的分配/释放
///
/// 每个 SlabCache 内部维护一个或多个 slab 页面（Vec<u8>），
/// 以及一个空闲链表用于 O(1) 的分配和释放。
pub struct SlabCache {
    /// 缓存名称
    name: &'static str,
    /// 对象大小（字节）
    object_size: usize,
    /// 对齐要求
    align: usize,
    /// Slab 页面列表（后备内存池）
    slabs: Vec<Vec<u8>>,
    /// 空闲链表，存储可分配对象的指针
    free_list: Vec<*mut u8>,
    /// 已使用对象数
    used_count: usize,
}

// 安全性：SlabCache 仅在内核中使用，内部的裸指针指向 slab 页面内的有效内存。
// 所有访问都通过 Mutex 保护，不会出现数据竞争。
unsafe impl Send for SlabCache {}
unsafe impl Sync for SlabCache {}

impl SlabCache {
    /// 创建新的 Slab 缓存
    ///
    /// # 参数
    /// - `name`: 缓存名称
    /// - `object_size`: 对象大小（字节）
    /// - `align`: 对齐要求（必须是 2 的幂）
    fn new(name: &'static str, object_size: usize, align: usize) -> Result<Self, SlabError> {
        if object_size == 0 {
            return Err(SlabError::ZeroSize);
        }
        if align == 0 || !align.is_power_of_two() {
            return Err(SlabError::InvalidAlignment);
        }

        let mut cache = SlabCache {
            name,
            object_size,
            align,
            slabs: Vec::new(),
            free_list: Vec::new(),
            used_count: 0,
        };

        // 初始分配一个 slab 页面
        cache.add_slab();

        Ok(cache)
    }

    /// 添加一个新的 slab 页面并填充空闲链表
    fn add_slab(&mut self) {
        let mut slab = Vec::with_capacity(SLAB_PAGE_SIZE);
        slab.resize(SLAB_PAGE_SIZE, 0);
        let base = slab.as_mut_ptr() as usize;

        // 计算对齐后的步长（每个对象占用的空间，包含对齐填充）
        let stride = (self.object_size + self.align - 1) & !(self.align - 1);

        // 计算第一个对齐的偏移量
        let first_offset = (self.align - (base % self.align)) % self.align;

        // 遍历 slab 页面，将每个对象槽位加入空闲链表
        let mut offset = first_offset;
        while offset + self.object_size <= SLAB_PAGE_SIZE {
            let ptr = unsafe { slab.as_mut_ptr().add(offset) };
            self.free_list.push(ptr);
            offset += stride;
        }

        self.slabs.push(slab);
    }

    /// 从缓存分配一个对象
    ///
    /// 如果空闲链表为空，自动扩展 slab 页面。
    fn alloc(&mut self) -> Result<*mut u8, SlabError> {
        // 空闲链表为空时，扩展缓存
        if self.free_list.is_empty() {
            self.add_slab();
        }

        // 扩展后仍为空（对象太大无法放入 slab 页面）
        if self.free_list.is_empty() {
            return Err(SlabError::AllocationFailed);
        }

        let ptr = self.free_list.pop().unwrap();
        self.used_count += 1;
        Ok(ptr)
    }

    /// 释放一个对象回缓存
    ///
    /// 验证指针是否属于此缓存的某个 slab 页面。
    fn free(&mut self, ptr: *mut u8) -> Result<(), SlabError> {
        if ptr.is_null() {
            return Err(SlabError::InvalidPointer);
        }

        // 验证指针是否属于此缓存的某个 slab 页面
        let ptr_addr = ptr as usize;
        let mut found = false;
        for slab in &self.slabs {
            let slab_start = slab.as_ptr() as usize;
            let slab_end = slab_start + slab.len();
            if ptr_addr >= slab_start && ptr_addr < slab_end {
                found = true;
                break;
            }
        }

        if !found {
            return Err(SlabError::InvalidPointer);
        }

        // 将指针放回空闲链表
        self.free_list.push(ptr);
        self.used_count = self.used_count.saturating_sub(1);
        Ok(())
    }

    /// 获取缓存统计信息
    fn stats(&self) -> SlabCacheStats {
        SlabCacheStats {
            name: self.name.to_string(),
            object_size: self.object_size,
            align: self.align,
            total_objects: self.free_list.len() + self.used_count,
            free_objects: self.free_list.len(),
            used_objects: self.used_count,
            slab_count: self.slabs.len(),
        }
    }

    /// 获取缓存名称
    fn name(&self) -> &'static str {
        self.name
    }
}

/// Slab 全局分配器
///
/// 管理多个 SlabCache，提供按名称分配/释放和通用 kmalloc/kfree 接口。
pub struct SlabAllocator {
    /// 缓存列表，使用 Mutex 保护内部状态
    caches: Mutex<Vec<SlabCache>>,
}

impl SlabAllocator {
    /// 创建新的 Slab 分配器实例
    pub const fn new() -> Self {
        SlabAllocator {
            caches: Mutex::new(Vec::new()),
        }
    }

    /// 创建一个新的 Slab 缓存
    ///
    /// # 参数
    /// - `name`: 缓存名称（不能为空）
    /// - `object_size`: 对象大小（字节，不能为零）
    /// - `align`: 对齐要求（必须是 2 的幂）
    ///
    /// # 错误
    /// - `SlabError::InvalidCacheName`: 缓存名为空
    /// - `SlabError::CacheAlreadyExists`: 同名缓存已存在
    /// - `SlabError::ZeroSize`: 对象大小为零
    /// - `SlabError::InvalidAlignment`: 对齐无效
    pub fn create_cache(
        &self,
        name: &'static str,
        object_size: usize,
        align: usize,
    ) -> Result<(), SlabError> {
        if name.is_empty() {
            return Err(SlabError::InvalidCacheName);
        }

        let mut caches = self.caches.lock();

        // 检查缓存是否已存在
        for cache in caches.iter() {
            if cache.name() == name {
                return Err(SlabError::CacheAlreadyExists(name.to_string()));
            }
        }

        let cache = SlabCache::new(name, object_size, align)?;
        caches.push(cache);
        Ok(())
    }

    /// 从指定缓存分配一个对象
    ///
    /// # 参数
    /// - `cache_name`: 缓存名称
    ///
    /// # 错误
    /// - `SlabError::InvalidCacheName`: 缓存名为空
    /// - `SlabError::CacheNotFound`: 指定缓存不存在
    /// - `SlabError::AllocationFailed`: 分配失败
    pub fn alloc(&self, cache_name: &str) -> Result<*mut u8, SlabError> {
        if cache_name.is_empty() {
            return Err(SlabError::InvalidCacheName);
        }

        let mut caches = self.caches.lock();

        for cache in caches.iter_mut() {
            if cache.name() == cache_name {
                return cache.alloc();
            }
        }

        Err(SlabError::CacheNotFound(cache_name.to_string()))
    }

    /// 释放一个对象回指定缓存
    ///
    /// # 参数
    /// - `cache_name`: 缓存名称
    /// - `ptr`: 要释放的对象指针
    ///
    /// # 错误
    /// - `SlabError::InvalidCacheName`: 缓存名为空
    /// - `SlabError::CacheNotFound`: 指定缓存不存在
    /// - `SlabError::InvalidPointer`: 指针无效
    pub fn free(&self, cache_name: &str, ptr: *mut u8) -> Result<(), SlabError> {
        if cache_name.is_empty() {
            return Err(SlabError::InvalidCacheName);
        }

        let mut caches = self.caches.lock();

        for cache in caches.iter_mut() {
            if cache.name() == cache_name {
                return cache.free(ptr);
            }
        }

        Err(SlabError::CacheNotFound(cache_name.to_string()))
    }

    /// 通用内核内存分配（kmalloc）
    ///
    /// 自动选择最接近且能容纳请求大小的缓存。
    /// 如果没有合适的缓存，返回 AllocationFailed 错误。
    ///
    /// # 参数
    /// - `size`: 请求的内存大小（字节）
    /// - `align`: 对齐要求（必须是 2 的幂）
    ///
    /// # 错误
    /// - `SlabError::ZeroSize`: 大小为零
    /// - `SlabError::InvalidAlignment`: 对齐无效
    /// - `SlabError::AllocationFailed`: 没有合适的缓存
    pub fn kmalloc(&self, size: usize, align: usize) -> Result<*mut u8, SlabError> {
        if size == 0 {
            return Err(SlabError::ZeroSize);
        }
        if align == 0 || !align.is_power_of_two() {
            return Err(SlabError::InvalidAlignment);
        }

        let mut caches = self.caches.lock();

        // 查找能满足大小和对齐要求的最小缓存（向上取整到最近缓存大小）
        let mut best_idx: Option<usize> = None;
        let mut best_size = usize::MAX;

        for (i, cache) in caches.iter().enumerate() {
            if cache.object_size >= size && cache.align >= align && cache.object_size < best_size {
                best_idx = Some(i);
                best_size = cache.object_size;
            }
        }

        if let Some(idx) = best_idx {
            return caches[idx].alloc();
        }

        Err(SlabError::AllocationFailed)
    }

    /// 通用内核内存释放（kfree）
    ///
    /// 自动查找指针所属的缓存并释放。
    ///
    /// # 参数
    /// - `ptr`: 要释放的指针
    /// - `size`: 原始分配大小（用于辅助查找，当前实现中未使用）
    ///
    /// # 错误
    /// - `SlabError::InvalidPointer`: 指针无效或不属于任何缓存
    pub fn kfree(&self, ptr: *mut u8, _size: usize) -> Result<(), SlabError> {
        if ptr.is_null() {
            return Err(SlabError::InvalidPointer);
        }

        let mut caches = self.caches.lock();

        // 遍历所有缓存，查找指针所属的 slab 页面
        let ptr_addr = ptr as usize;
        for cache in caches.iter_mut() {
            for slab in &cache.slabs {
                let slab_start = slab.as_ptr() as usize;
                let slab_end = slab_start + slab.len();
                if ptr_addr >= slab_start && ptr_addr < slab_end {
                    return cache.free(ptr);
                }
            }
        }

        Err(SlabError::InvalidPointer)
    }

    /// 获取指定缓存的统计信息
    ///
    /// # 参数
    /// - `cache_name`: 缓存名称
    ///
    /// # 返回
    /// 如果缓存存在，返回 Some(SlabCacheStats)；否则返回 None。
    pub fn cache_stats(&self, cache_name: &str) -> Option<SlabCacheStats> {
        if cache_name.is_empty() {
            return None;
        }

        let caches = self.caches.lock();

        for cache in caches.iter() {
            if cache.name() == cache_name {
                return Some(cache.stats());
            }
        }

        None
    }

    /// 初始化 kmalloc 预定义缓存
    ///
    /// 创建一系列标准大小的缓存供 kmalloc 使用。
    pub fn init_kmalloc_caches(&self) -> Result<(), SlabError> {
        for &(size, name) in KMALLOC_SIZES {
            self.create_cache(name, size, KMALLOC_DEFAULT_ALIGN)?;
        }
        Ok(())
    }
}

/// 初始化默认的 kmalloc 缓存
///
/// 创建一系列标准大小的缓存供内核通用内存分配使用。
/// 通常在内核启动的内存初始化阶段调用。
pub fn init_default_caches() {
    let alloc = &*SLAB_ALLOCATOR;
    // 创建预定义缓存
    let _ = alloc.create_cache("kmalloc-32", 32, 8);
    let _ = alloc.create_cache("kmalloc-64", 64, 8);
    let _ = alloc.create_cache("kmalloc-128", 128, 16);
    let _ = alloc.create_cache("kmalloc-256", 256, 16);
    let _ = alloc.create_cache("kmalloc-512", 512, 32);
    let _ = alloc.create_cache("kmalloc-1024", 1024, 64);
    let _ = alloc.create_cache("kmalloc-2048", 2048, 128);
    let _ = alloc.create_cache("kmalloc-4096", 4096, 256);
}

/// 全局 Slab 分配器实例（使用 spin::Lazy 延迟初始化）
pub static SLAB_ALLOCATOR: spin::Lazy<SlabAllocator> = spin::Lazy::new(|| SlabAllocator::new());

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建一个干净的 SlabAllocator 实例用于测试
    fn create_test_allocator() -> SlabAllocator {
        SlabAllocator::new()
    }

    /// 辅助函数：创建并初始化带 kmalloc 缓存的分配器
    fn create_allocator_with_kmalloc() -> SlabAllocator {
        let allocator = SlabAllocator::new();
        allocator.init_kmalloc_caches().unwrap();
        allocator
    }

    /// 测试 1：创建缓存成功
    #[test]
    fn test_create_cache() {
        let allocator = create_test_allocator();

        // 创建一个缓存应该成功
        let result = allocator.create_cache("test_cache", 64, 8);
        assert!(result.is_ok(), "创建缓存应该成功");

        // 创建同名缓存应该失败
        let result2 = allocator.create_cache("test_cache", 64, 8);
        assert_eq!(result2, Err(SlabError::CacheAlreadyExists("test_cache".to_string())));

        // 创建空名称缓存应该失败
        let result3 = allocator.create_cache("", 64, 8);
        assert_eq!(result3, Err(SlabError::InvalidCacheName));

        // 创建零大小缓存应该失败
        let result4 = allocator.create_cache("zero_cache", 0, 8);
        assert_eq!(result4, Err(SlabError::ZeroSize));

        // 创建无效对齐缓存应该失败
        let result5 = allocator.create_cache("bad_align", 64, 3);
        assert_eq!(result5, Err(SlabError::InvalidAlignment));

        // 创建零对齐缓存应该失败
        let result6 = allocator.create_cache("zero_align", 64, 0);
        assert_eq!(result6, Err(SlabError::InvalidAlignment));
    }

    /// 测试 2：从缓存分配对象
    #[test]
    fn test_alloc_from_cache() {
        let allocator = create_test_allocator();
        allocator.create_cache("alloc_test", 32, 8).unwrap();

        // 从缓存分配对象应该成功
        let ptr = allocator.alloc("alloc_test").unwrap();
        assert!(!ptr.is_null(), "分配的指针不应为空");

        // 多次分配应该返回不同的指针
        let ptr2 = allocator.alloc("alloc_test").unwrap();
        assert!(!ptr2.is_null());
        assert_ne!(ptr, ptr2, "两次分配的指针应该不同");
    }

    /// 测试 3：释放对象回缓存
    #[test]
    fn test_free_to_cache() {
        let allocator = create_test_allocator();
        allocator.create_cache("free_test", 32, 8).unwrap();

        // 分配一个对象
        let ptr = allocator.alloc("free_test").unwrap();
        assert!(!ptr.is_null());

        // 释放对象应该成功
        let result = allocator.free("free_test", ptr);
        assert!(result.is_ok(), "释放有效指针应该成功");

        // 释放空指针应该失败
        let result2 = allocator.free("free_test", core::ptr::null_mut());
        assert_eq!(result2, Err(SlabError::InvalidPointer));
    }

    /// 测试 4：多次分配释放循环
    #[test]
    fn test_alloc_dealloc_cycle() {
        let allocator = create_test_allocator();
        allocator.create_cache("cycle_test", 64, 8).unwrap();

        // 执行多次分配-释放循环
        for _ in 0..100 {
            let ptr = allocator.alloc("cycle_test").unwrap();
            assert!(!ptr.is_null());
            allocator.free("cycle_test", ptr).unwrap();
        }

        // 循环后统计信息应该与初始状态一致
        let stats = allocator.cache_stats("cycle_test").unwrap();
        assert_eq!(stats.used_objects, 0, "循环后已使用对象数应为零");
    }

    /// 测试 5：缓存耗尽时扩展
    #[test]
    fn test_cache_exhaustion() {
        let allocator = create_test_allocator();
        // 使用较大的对象大小，使得一个 slab 页面只能容纳少量对象
        allocator.create_cache("exhaust_test", 512, 8).unwrap();

        let initial_stats = allocator.cache_stats("exhaust_test").unwrap();
        let initial_slab_count = initial_stats.slab_count;
        let initial_total = initial_stats.total_objects;

        // 分配超过初始容量的对象
        for _ in 0..initial_total + 10 {
            let ptr = allocator.alloc("exhaust_test");
            assert!(ptr.is_ok(), "缓存耗尽时应自动扩展");
        }

        // 验证 slab 页面数量增加了
        let stats = allocator.cache_stats("exhaust_test").unwrap();
        assert!(
            stats.slab_count > initial_slab_count,
            "缓存耗尽后 slab 页面数量应增加，从 {} 到 {}",
            initial_slab_count,
            stats.slab_count
        );
    }

    /// 测试 6：统计信息正确
    #[test]
    fn test_cache_stats() {
        let allocator = create_test_allocator();
        allocator.create_cache("stats_test", 128, 8).unwrap();

        // 初始统计
        let stats = allocator.cache_stats("stats_test").unwrap();
        assert_eq!(stats.name, "stats_test");
        assert_eq!(stats.object_size, 128);
        assert_eq!(stats.align, 8);
        assert_eq!(stats.used_objects, 0);
        assert!(stats.total_objects > 0, "初始应有空闲对象");
        assert_eq!(stats.free_objects, stats.total_objects);
        assert_eq!(stats.slab_count, 1);

        // 分配后统计
        let _ptr1 = allocator.alloc("stats_test").unwrap();
        let _ptr2 = allocator.alloc("stats_test").unwrap();
        let stats2 = allocator.cache_stats("stats_test").unwrap();
        assert_eq!(stats2.used_objects, 2);
        assert_eq!(stats2.free_objects, stats2.total_objects - 2);

        // 不存在的缓存
        let stats3 = allocator.cache_stats("nonexistent");
        assert!(stats3.is_none());

        // 空缓存名
        let stats4 = allocator.cache_stats("");
        assert!(stats4.is_none());
    }

    /// 测试 7：通用分配释放（kmalloc/kfree）
    #[test]
    fn test_kmalloc_kfree() {
        let allocator = create_allocator_with_kmalloc();

        // 使用 kmalloc 分配不同大小的内存
        let ptr1 = allocator.kmalloc(32, 8).unwrap();
        assert!(!ptr1.is_null());

        let ptr2 = allocator.kmalloc(64, 8).unwrap();
        assert!(!ptr2.is_null());
        assert_ne!(ptr1, ptr2);

        // 使用 kfree 释放
        allocator.kfree(ptr1, 32).unwrap();
        allocator.kfree(ptr2, 64).unwrap();

        // 释放空指针应该失败
        let result = allocator.kfree(core::ptr::null_mut(), 32);
        assert_eq!(result, Err(SlabError::InvalidPointer));
    }

    /// 测试 8：对齐要求正确
    #[test]
    fn test_alignment() {
        let allocator = create_test_allocator();

        // 创建 16 字节对齐的缓存
        allocator.create_cache("align_test", 32, 16).unwrap();

        // 分配多个对象，验证对齐
        for _ in 0..10 {
            let ptr = allocator.alloc("align_test").unwrap();
            let addr = ptr as usize;
            assert_eq!(
                addr % 16,
                0,
                "分配的地址 {:#x} 应该 16 字节对齐",
                addr
            );
        }

        // 创建 32 字节对齐的缓存
        allocator.create_cache("align_test_32", 64, 32).unwrap();
        for _ in 0..10 {
            let ptr = allocator.alloc("align_test_32").unwrap();
            let addr = ptr as usize;
            assert_eq!(
                addr % 32,
                0,
                "分配的地址 {:#x} 应该 32 字节对齐",
                addr
            );
        }
    }

    /// 测试 9：多缓存独立运行
    #[test]
    fn test_multiple_caches() {
        let allocator = create_test_allocator();

        // 创建多个不同大小的缓存
        allocator.create_cache("small", 16, 4).unwrap();
        allocator.create_cache("medium", 128, 8).unwrap();
        allocator.create_cache("large", 1024, 16).unwrap();

        // 从不同缓存分配
        let ptr_small = allocator.alloc("small").unwrap();
        let ptr_medium = allocator.alloc("medium").unwrap();
        let ptr_large = allocator.alloc("large").unwrap();

        assert!(!ptr_small.is_null());
        assert!(!ptr_medium.is_null());
        assert!(!ptr_large.is_null());

        // 释放到各自的缓存
        allocator.free("small", ptr_small).unwrap();
        allocator.free("medium", ptr_medium).unwrap();
        allocator.free("large", ptr_large).unwrap();

        // 验证各缓存统计独立
        let stats_small = allocator.cache_stats("small").unwrap();
        let stats_medium = allocator.cache_stats("medium").unwrap();
        let stats_large = allocator.cache_stats("large").unwrap();

        assert_eq!(stats_small.used_objects, 0);
        assert_eq!(stats_medium.used_objects, 0);
        assert_eq!(stats_large.used_objects, 0);
        assert_ne!(stats_small.object_size, stats_medium.object_size);
        assert_ne!(stats_medium.object_size, stats_large.object_size);
    }

    /// 测试 10：无效缓存名错误
    #[test]
    fn test_invalid_cache_name() {
        let allocator = create_test_allocator();
        allocator.create_cache("valid_cache", 32, 8).unwrap();

        // 从不存在的缓存分配
        let result = allocator.alloc("nonexistent_cache");
        assert_eq!(
            result,
            Err(SlabError::CacheNotFound("nonexistent_cache".to_string()))
        );

        // 释放到不存在的缓存
        let ptr = allocator.alloc("valid_cache").unwrap();
        let result2 = allocator.free("wrong_cache", ptr);
        assert_eq!(
            result2,
            Err(SlabError::CacheNotFound("wrong_cache".to_string()))
        );

        // 清理
        allocator.free("valid_cache", ptr).unwrap();

        // 空缓存名
        let result3 = allocator.alloc("");
        assert_eq!(result3, Err(SlabError::InvalidCacheName));

        let result4 = allocator.free("", core::ptr::null_mut());
        assert_eq!(result4, Err(SlabError::InvalidCacheName));
    }

    /// 测试 11：释放无效指针错误
    #[test]
    fn test_free_invalid_ptr() {
        let allocator = create_test_allocator();
        allocator.create_cache("invalid_ptr_test", 32, 8).unwrap();

        // 释放空指针
        let result = allocator.free("invalid_ptr_test", core::ptr::null_mut());
        assert_eq!(result, Err(SlabError::InvalidPointer));

        // 释放一个不属于此缓存的指针（栈上的地址）
        let stack_var: u8 = 42;
        let stack_ptr: *mut u8 = &stack_var as *const u8 as *mut u8;
        let result2 = allocator.free("invalid_ptr_test", stack_ptr);
        assert_eq!(result2, Err(SlabError::InvalidPointer));

        // 释放一个随意的堆地址（不属于任何 slab）
        let fake_ptr = 0xDEAD_BEEF as *mut u8;
        let result3 = allocator.free("invalid_ptr_test", fake_ptr);
        assert_eq!(result3, Err(SlabError::InvalidPointer));
    }

    /// 测试 12：大小向上取整到最近缓存
    #[test]
    fn test_kmalloc_size_rounding() {
        let allocator = create_allocator_with_kmalloc();

        // 请求 1 字节，应该分配 32 字节的缓存
        let ptr1 = allocator.kmalloc(1, 8).unwrap();
        assert!(!ptr1.is_null());
        allocator.kfree(ptr1, 1).unwrap();

        // 请求 33 字节，应该分配 64 字节的缓存
        let ptr2 = allocator.kmalloc(33, 8).unwrap();
        assert!(!ptr2.is_null());
        allocator.kfree(ptr2, 33).unwrap();

        // 请求 65 字节，应该分配 128 字节的缓存
        let ptr3 = allocator.kmalloc(65, 8).unwrap();
        assert!(!ptr3.is_null());
        allocator.kfree(ptr3, 65).unwrap();

        // 请求恰好 32 字节，应该分配 32 字节的缓存
        let ptr4 = allocator.kmalloc(32, 8).unwrap();
        assert!(!ptr4.is_null());
        allocator.kfree(ptr4, 32).unwrap();

        // 请求 4096 字节，应该分配 4096 字节的缓存
        let ptr5 = allocator.kmalloc(4096, 8).unwrap();
        assert!(!ptr5.is_null());
        allocator.kfree(ptr5, 4096).unwrap();

        // 请求超过最大缓存的大小，应该失败
        let result = allocator.kmalloc(8192, 8);
        assert_eq!(result, Err(SlabError::AllocationFailed));

        // 请求零大小，应该失败
        let result2 = allocator.kmalloc(0, 8);
        assert_eq!(result2, Err(SlabError::ZeroSize));

        // 无效对齐，应该失败
        let result3 = allocator.kmalloc(32, 0);
        assert_eq!(result3, Err(SlabError::InvalidAlignment));

        let result4 = allocator.kmalloc(32, 3);
        assert_eq!(result4, Err(SlabError::InvalidAlignment));
    }

    /// 测试 13：init_default_caches 初始化默认缓存
    #[test]
    fn test_init_default_caches() {
        init_default_caches();
        let alloc = &*SLAB_ALLOCATOR;
        assert!(alloc.cache_stats("kmalloc-32").is_some());
        assert!(alloc.cache_stats("kmalloc-64").is_some());
        assert!(alloc.cache_stats("kmalloc-128").is_some());
        assert!(alloc.cache_stats("kmalloc-256").is_some());
        assert!(alloc.cache_stats("kmalloc-512").is_some());
        assert!(alloc.cache_stats("kmalloc-1024").is_some());
        assert!(alloc.cache_stats("kmalloc-2048").is_some());
        assert!(alloc.cache_stats("kmalloc-4096").is_some());

        // 验证缓存大小正确
        let stats_64 = alloc.cache_stats("kmalloc-64").unwrap();
        assert_eq!(stats_64.object_size, 64);

        let stats_4096 = alloc.cache_stats("kmalloc-4096").unwrap();
        assert_eq!(stats_4096.object_size, 4096);

        // 验证不存在的缓存返回 None
        assert!(alloc.cache_stats("nonexistent").is_none());
    }
}
