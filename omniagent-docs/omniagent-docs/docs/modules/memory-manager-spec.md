# OmniAgent OS 内存管理器规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **模块归属**: 内核模块 / 内存管理子系统
> **状态**: 规范草案

---

## 1. 概述

### 1.1 目的

本文档定义 OmniAgent OS 内存管理子系统的完整规范。内存管理器是微内核架构中最核心的子系统之一，负责物理内存分配、虚拟地址空间管理、内核堆和用户堆的维护、进程间共享内存、Agent 内存隔离以及虚拟化内存扩展（EPT/NPT）。所有实现基于 Rust 的 `x86_64` crate提供的基础抽象，确保类型安全和零成本抽象。

### 1.2 设计目标

| 目标 | 描述 |
|------|------|
| 安全性 | 利用 Rust 类型系统和所有权模型防止内存安全漏洞 |
| 高性能 | 分配延迟 <100ns，页映射 <500ns，缺页处理 <1us |
| 隔离性 | 每个 Agent 拥有独立页表，共享区域受控 |
| 可扩展 | 支持从嵌入式设备（64MB）到服务器（1TB+） |
| 虚拟化 | 原生支持 EPT/NPT，为 VM 客户机提供内存虚拟化 |

### 1.3 架构概览

```
┌─────────────────────────────────────────────────────┐
│                   用户空间                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │ Agent A  │  │ Agent B  │  │ libagent (API)   │   │
│  │ 页表 A   │  │ 页表 B   │  │ 用户堆分配器      │   │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘   │
│       │              │                  │             │
│  ─────┴──────────────┴──────────────────┴──────────── │
│                   系统调用接口                         │
│  ──────────────────────────────────────────────────── │
│                   内核空间                            │
│  ┌──────────────────────────────────────────────┐    │
│  │              虚拟内存管理器                    │    │
│  │  ┌─────────┐  ┌──────────┐  ┌────────────┐  │    │
│  │  │页表管理  │  │共享内存   │  │缺页处理    │  │    │
│  │  │PML4→PT  │  │IPC零拷贝  │  │5种故障类型  │  │    │
│  │  └─────────┘  └──────────┘  └────────────┘  │    │
│  └──────────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────────┐    │
│  │              物理内存管理器                    │    │
│  │  ┌─────────┐  ┌──────────┐  ┌────────────┐  │    │
│  │  │帧分配器  │  │内核堆    │  │OOM 处理    │  │    │
│  │  │Bitmap   │  │bump→slab │  │策略引擎    │  │    │
│  │  └─────────┘  └──────────┘  └────────────┘  │    │
│  └──────────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────────┐    │
│  │              虚拟化内存 (EPT/NPT)             │    │
│  └──────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

---

## 2. 物理内存帧分配器

### 2.1 帧分配器接口

```rust
use x86_64::structures::paging::{
    frame::PhysFrame,
    frame_alloc::{FrameAllocator, FrameDeallocator},
    page::{Page, Size4KiB},
    PageSize,
};
use x86_64::PhysAddr;

/// 帧分配器 trait（兼容 x86_64 crate）
pub trait OmniFrameAllocator: FrameAllocator<Size4KiB> + FrameDeallocator<Size4KiB> {
    /// 获取可用帧总数
    fn total_frames(&self) -> u64;

    /// 获取已使用帧数
    fn used_frames(&self) -> u64;

    /// 获取空闲帧数
    fn free_frames(&self) -> u64 {
        self.total_frames() - self.used_frames()
    }

    /// 分配连续帧（用于 DMA、大页等）
    fn allocate_contiguous(&mut self, count: usize) -> Option<PhysFrameRange>;

    /// 获取内存使用统计
    fn statistics(&self) -> MemoryStatistics;
}

/// 物理帧范围
#[derive(Debug, Clone)]
pub struct PhysFrameRange {
    pub start: PhysFrame,
    pub end: PhysFrame,
}

impl PhysFrameRange {
    pub fn len(&self) -> u64 {
        (self.end.start_address().as_u64() - self.start.start_address().as_u64()) / 4096
    }
}
```

### 2.2 Bitmap 帧分配器实现

```rust
/// Bitmap 帧分配器
pub struct BitmapFrameAllocator {
    /// 位图：每个 bit 代表一个 4KB 帧
    bitmap: Vec<u64>,
    /// 可用内存区域列表
    memory_regions: Vec<MemoryRegion>,
    /// 下一次搜索的起始位置（优化分配速度）
    next_free: AtomicU64,
    /// 总帧数
    total_frames: u64,
    /// 已分配帧数
    used_frames: AtomicU64,
}

/// 帧分配器错误
#[derive(Debug, Clone)]
pub enum FrameAllocError {
    /// 内存不足
    OutOfMemory,
    /// 请求的帧数无效
    InvalidFrameCount,
    /// 地址超出范围
    AddressOutOfRange(PhysAddr),
    /// 帧已被分配
    FrameAlreadyAllocated(PhysAddr),
}

impl BitmapFrameAllocator {
    /// 从引导信息创建帧分配器
    pub fn from_boot_info(boot_info: &ParsedBootInfo) -> Result<Self, FrameAllocError> {
        let mut allocator = Self {
            bitmap: Vec::new(),
            memory_regions: Vec::new(),
            next_free: AtomicU64::new(0),
            total_frames: 0,
            used_frames: AtomicU64::new(0),
        };

        // 计算最大物理地址
        let max_addr = boot_info.memory_map.iter()
            .map(|r| r.base_addr + r.length)
            .max()
            .unwrap_or(0);

        allocator.total_frames = max_addr / 4096;

        // 初始化位图（全部标记为已分配）
        let bitmap_size = (allocator.total_frames + 63) / 64;
        allocator.bitmap = vec![u64::MAX; bitmap_size as usize];

        // 标记可用区域为空闲
        for region in &boot_info.memory_map {
            if region.region_type == MemoryRegionType::Usable {
                let start_frame = region.base_addr / 4096;
                let end_frame = (region.base_addr + region.length) / 4096;
                for frame in start_frame..end_frame {
                    allocator.clear_bit(frame);
                }
                allocator.memory_regions.push(*region);
            }
        }

        // 保留内核和引导加载器占用的帧
        allocator.reserve_kernel_frames(boot_info);

        Ok(allocator)
    }

    /// 设置位图中的某一位（标记为已分配）
    fn set_bit(&self, frame: u64) {
        let idx = frame as usize / 64;
        let bit = frame as usize % 64;
        self.bitmap[idx] |= 1 << bit;
    }

    /// 清除位图中的某一位（标记为空闲）
    fn clear_bit(&self, frame: u64) {
        let idx = frame as usize / 64;
        let bit = frame as usize % 64;
        self.bitmap[idx] &= !(1 << bit);
    }

    /// 测试位图中的某一位
    fn test_bit(&self, frame: u64) -> bool {
        let idx = frame as usize / 64;
        let bit = frame as usize % 64;
        (self.bitmap[idx] & (1 << bit)) != 0
    }

    /// 保留内核占用的帧
    fn reserve_kernel_frames(&mut self, boot_info: &ParsedBootInfo) {
        let kernel_start = boot_info.kernel_start / 4096;
        let kernel_end = (boot_info.kernel_end + 4095) / 4096;
        for frame in kernel_start..kernel_end {
            self.set_bit(frame);
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let start = self.next_free.load(Ordering::Relaxed);

        // 从 next_free 开始搜索
        for frame in start..self.total_frames {
            if !self.test_bit(frame) {
                self.set_bit(frame);
                self.next_free.store(frame + 1, Ordering::Relaxed);
                self.used_frames.fetch_add(1, Ordering::Relaxed);
                let addr = PhysAddr::new(frame * 4096);
                return Some(PhysFrame::containing_address(addr));
            }
        }

        // 回绕到开头搜索
        for frame in 0..start {
            if !self.test_bit(frame) {
                self.set_bit(frame);
                self.next_free.store(frame + 1, Ordering::Relaxed);
                self.used_frames.fetch_add(1, Ordering::Relaxed);
                let addr = PhysAddr::new(frame * 4096);
                return Some(PhysFrame::containing_address(addr));
            }
        }

        None // 内存耗尽
    }
}

unsafe impl FrameDeallocator<Size4KiB> for BitmapFrameAllocator {
    fn deallocate_frame(&mut self, frame: PhysFrame) {
        let frame_num = frame.start_address().as_u64() / 4096;
        assert!(frame_num < self.total_frames, "无效帧地址");
        assert!(self.test_bit(frame_num), "帧未分配");
        self.clear_bit(frame_num);
        self.used_frames.fetch_sub(1, Ordering::Relaxed);
    }
}
```

---

## 3. 页表管理

### 3.1 四级页表结构

```
虚拟地址 (48位有效):
┌──────────┬──────────┬──────────┬──────────┬────────┐
│ PML4[47:39]│ PDPT[38:30]│ PD[29:21] │ PT[20:12] │ 偏移[11:0] │
│  9 bits   │  9 bits   │  9 bits  │  9 bits  │ 12 bits│
└─────┬─────┴─────┬─────┴─────┬─────┴─────┬─────┴───────┘
      │           │           │           │
      ▼           ▼           ▼           ▼
  ┌───────┐   ┌───────┐   ┌───────┐   ┌───────┐
  │ PML4E │──▶│ PDPTE │──▶│  PDE  │──▶│  PTE  │──▶ 物理页
  └───────┘   └───────┘   └───────┘   └───────┘
  (512项)     (512项)     (512项)     (512项)
```

### 3.2 页表管理器接口

```rust
use x86_64::structures::paging::{
    mapper::{Mapper, MapperFlush, Translate},
    page_table::{PageTable, PageTableFlags, PageTableEntry},
    Page,
};
use x86_64::VirtAddr;

/// 页表管理器
pub struct PageTableManager {
    /// PML4 表的物理地址
    pml4_phys: PhysAddr,
    /// 当前活跃的页表（内核）
    active_pml4: VirtAddr,
    /// 帧分配器引用
    frame_allocator: SpinLock<&'static mut dyn OmniFrameAllocator>,
}

/// 页表操作错误
#[derive(Debug, Clone)]
pub enum PageTableError {
    /// 帧分配失败
    FrameAllocationFailed,
    /// 映射已存在
    AlreadyMapped(VirtAddr),
    /// 地址未对齐
    Misaligned(VirtAddr),
    /// 无效的虚拟地址
    InvalidVirtualAddress(VirtAddr),
    /// 页表层级不足
    PageTableHierarchyError,
}

/// 映射选项
#[derive(Debug, Clone)]
pub struct MapOptions {
    pub flags: PageTableFlags,
    pub user_accessible: bool,
    pub execute_disable: bool,
    pub write_through: bool,
    pub cache_disable: bool,
    pub global: bool,
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            flags: PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            user_accessible: false,
            execute_disable: true,  // 默认 NX
            write_through: false,
            cache_disable: false,
            global: true,
        }
    }
}

impl MapOptions {
    /// 用户空间映射选项
    pub fn user() -> Self {
        Self {
            flags: PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            user_accessible: true,
            execute_disable: false,
            write_through: false,
            cache_disable: false,
            global: false,
        }
    }

    /// 只读代码段映射
    pub fn code_segment() -> Self {
        Self {
            flags: PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
            user_accessible: true,
            execute_disable: false,
            write_through: false,
            cache_disable: false,
            global: false,
        }
    }

    /// 栈映射（不可执行）
    pub fn stack() -> Self {
        Self {
            flags: PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            user_accessible: true,
            execute_disable: true,
            write_through: false,
            cache_disable: false,
            global: false,
        }
    }
}
```

### 3.3 页表管理器实现

```rust
impl PageTableManager {
    /// 创建新的页表（用于新进程）
    pub fn new_user_page_table(
        frame_allocator: &mut dyn OmniFrameAllocator,
    ) -> Result<(VirtAddr, PhysAddr), PageTableError> {
        // 分配 PML4 页
        let pml4_frame = frame_allocator.allocate_frame()
            .ok_or(PageTableError::FrameAllocationFailed)?;

        let pml4_virt = unsafe {
            VirtAddr::new(pml4_frame.start_address().as_u64())
        };

        // 清零 PML4
        let pml4 = unsafe { &mut *(pml4_virt.as_mut_ptr::<PageTable>()) };
        pml4.zero();

        // 复制内核空间映射（高半部分 256 项）
        let kernel_pml4 = unsafe { &mut *(0xFFFFFFFFFFFFF000u64 as *mut PageTable) };
        for i in 256..512 {
            pml4[i] = kernel_pml4[i].clone();
        }

        Ok((pml4_virt, pml4_frame.start_address()))
    }

    /// 映射页面
    pub fn map_page(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame,
        options: &MapOptions,
    ) -> Result<MapperFlush<Size4KiB>, PageTableError> {
        let mut flags = options.flags;

        if options.execute_disable {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        if options.write_through {
            flags |= PageTableFlags::WRITE_THROUGH;
        }
        if options.cache_disable {
            flags |= PageTableFlags::CACHE_DISABLE;
        }
        if options.global {
            flags |= PageTableFlags::GLOBAL;
        }

        let (mut mapper, _) = unsafe {
            self.mapper()
        };

        // 使用 x86_64 crate 的 Mapper trait
        unsafe {
            mapper.map_to(page, frame, flags, &mut *self.frame_allocator.lock())
                .map_err(|_| PageTableError::AlreadyMapped(page.start_address()))
        }
    }

    /// 取消映射
    pub fn unmap_page(
        &mut self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysFrame, MapperFlush<Size4KiB>), PageTableError> {
        let (mut mapper, _) = unsafe { self.mapper() };
        unsafe {
            mapper.unmap(page)
                .map_err(|_| PageTableError::InvalidVirtualAddress(page.start_address()))
        }
    }

    /// 切换页表（CR3 写入）
    pub unsafe fn switch_to(&self, pml4_phys: PhysAddr) {
        x86_64::registers::control::Cr3::write(
            pml4_phys,
            Cr3Flags::empty()
        );
    }

    /// 查询虚拟地址映射
    pub fn translate(&self, addr: VirtAddr) -> Option<TranslateResult> {
        let (mapper, _) = unsafe { self.mapper() };
        mapper.translate(addr)
    }
}
```

---

## 4. 内核堆管理

### 4.1 两阶段堆策略

```rust
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, Ordering};

/// 全局堆分配器
pub struct OmniHeapAllocator {
    /// 启动阶段：bumpalo 分配器
    boot_heap: SpinLock<Option<BootHeap>>,
    /// 运行时：slab 分配器
    slab_heap: SpinLock<Option<SlabAllocator>>,
    /// 是否已切换到 slab
    switched: AtomicBool,
}

/// 启动堆（bumpalo 封装）
pub struct BootHeap {
    bump: bumpalo::Bump,
    start_addr: usize,
    size: usize,
}

impl BootHeap {
    pub fn new(start: usize, size: usize) -> Self {
        let bump = unsafe {
            bumpalo::Bump::from_raw_parts(start as *mut u8, size)
        };
        Self { bump, start_addr: start, size }
    }

    pub fn allocated(&self) -> usize {
        self.bump.allocated_bytes()
    }

    pub fn remaining(&self) -> usize {
        self.size - self.allocated()
    }
}

/// Slab 分配器
pub struct SlabAllocator {
    /// 固定大小缓存（8, 16, 32, 64, 128, 256, 512, 1024 字节）
    caches: [SlabCache; SLAB_CACHE_COUNT],
    /// 大块分配追踪
    large_allocations: SpinLock<BTreeMap<usize, LargeAllocation>>,
    /// 统计信息
    stats: AtomicMemoryStats,
}

/// Slab 缓存
pub struct SlabCache {
    /// 对象大小
    object_size: usize,
    /// 空闲链表
    free_list: SpinLock<Vec<*mut u8>>,
    /// Slab 页面列表
    slabs: SpinLock<Vec<SlabPage>>,
    /// 每页对象数
    objects_per_slab: usize,
}

/// Slab 页面
struct SlabPage {
    addr: VirtAddr,
    used_count: usize,
    total_count: usize,
}

/// 大块分配记录
struct LargeAllocation {
    addr: VirtAddr,
    size: usize,
    layout: Layout,
}

/// Slab 缓存大小等级
const SLAB_SIZES: [usize; 8] = [8, 16, 32, 64, 128, 256, 512, 1024];
const SLAB_CACHE_COUNT: usize = SLAB_SIZES.len();

/// 原子内存统计
pub struct AtomicMemoryStats {
    pub total_allocated: AtomicU64,
    pub total_freed: AtomicU64,
    pub allocation_count: AtomicU64,
    pub deallocation_count: AtomicU64,
    pub peak_usage: AtomicU64,
}

impl AtomicMemoryStats {
    pub const fn new() -> Self {
        Self {
            total_allocated: AtomicU64::new(0),
            total_freed: AtomicU64::new(0),
            allocation_count: AtomicU64::new(0),
            deallocation_count: AtomicU64::new(0),
            peak_usage: AtomicU64::new(0),
        }
    }

    pub fn current_usage(&self) -> u64 {
        self.total_allocated.load(Ordering::Relaxed)
            - self.total_freed.load(Ordering::Relaxed)
    }

    pub fn record_alloc(&self, size: u64) {
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        self.allocation_count.fetch_add(1, Ordering::Relaxed);
        // 更新峰值
        let current = self.current_usage();
        let mut peak = self.peak_usage.load(Ordering::Relaxed);
        while current > peak {
            match self.peak_usage.compare_exchange_weak(
                peak, current,
                Ordering::Relaxed, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(e) => peak = e,
            }
        }
    }

    pub fn record_free(&self, size: u64) {
        self.total_freed.fetch_add(size, Ordering::Relaxed);
        self.deallocation_count.fetch_add(1, Ordering::Relaxed);
    }
}
```

### 4.2 全局分配器实现

```rust
unsafe impl GlobalAlloc for OmniHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.switched.load(Ordering::Acquire) {
            // 启动阶段：使用 bumpalo
            if let Some(ref boot) = *self.boot_heap.lock() {
                return boot.bump.alloc_layout(layout);
            }
        }

        // 运行时：使用 slab
        if let Some(ref slab) = *self.slab_heap.lock() {
            return slab.allocate(layout);
        }

        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !self.switched.load(Ordering::Acquire) {
            // bumpalo 不支持单独释放
            return;
        }

        if let Some(ref slab) = *self.slab_heap.lock() {
            slab.deallocate(ptr, layout);
        }
    }
}

impl SlabAllocator {
    /// 分配内存
    pub fn allocate(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());

        // 查找合适的 slab 缓存
        for (i, &cache_size) in SLAB_SIZES.iter().enumerate() {
            if size <= cache_size {
                return self.caches[i].allocate();
            }
        }

        // 大块分配
        self.allocate_large(layout)
    }

    /// 释放内存
    pub fn deallocate(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align());

        for (i, &cache_size) in SLAB_SIZES.iter().enumerate() {
            if size <= cache_size {
                self.caches[i].deallocate(ptr);
                return;
            }
        }

        self.deallocate_large(ptr, layout);
    }
}
```

---

## 5. 用户空间堆分配器

### 5.1 用户堆设计

```rust
/// 用户空间堆分配器（通过 mmap 系统调用扩展）
pub struct UserHeapAllocator {
    /// 堆起始地址
    heap_start: *mut u8,
    /// 当前堆顶部（brk）
    heap_brk: AtomicUsize,
    /// 堆大小限制
    heap_limit: usize,
    /// 是否启用
    enabled: AtomicBool,
}

/// 用户堆配置
#[derive(Debug, Clone)]
pub struct UserHeapConfig {
    /// 初始堆大小
    pub initial_size: usize,
    /// 最大堆大小
    pub max_size: usize,
    /// 增长粒度
    pub growth_granularity: usize,
    /// 是否启用内存过量提交
    pub overcommit: bool,
}

impl Default for UserHeapConfig {
    fn default() -> Self {
        Self {
            initial_size: 4 * 1024 * 1024,    // 4MB
            max_size: 512 * 1024 * 1024,       // 512MB
            growth_granularity: 64 * 1024,      // 64KB
            overcommit: false,
        }
    }
}

/// brk 系统调用
pub fn sys_brk(addr: usize) -> SyscallResult {
    let current = USER_HEAP.brk.load(Ordering::SeqCst);
    let new_brk = if addr == 0 {
        current // 返回当前 brk
    } else if addr < USER_HEAP.start() {
        return Err(SyscallError::EINVAL);
    } else if addr > USER_HEAP.limit() {
        return Err(SyscallError::ENOMEM);
    } else {
        // 扩展或收缩堆
        let pages_needed = (addr - current + 4095) / 4096;
        if addr > current {
            // 分配新页面
            for i in 0..pages_needed {
                let page_addr = current + i * 4096;
                let frame = FRAME_ALLOCATOR.lock().allocate_frame()
                    .ok_or(SyscallError::ENOMEM)?;
                PAGE_TABLE_MANAGER.lock().map_page(
                    Page::from_start_address(VirtAddr::new(page_addr)).unwrap(),
                    frame,
                    &MapOptions::user(),
                )?;
            }
        }
        addr
    };

    USER_HEAP.brk.store(new_brk, Ordering::SeqCst);
    Ok(SyscallResult::Value(new_brk))
}
```

---

## 6. 共享内存与 IPC 零拷贝

### 6.1 共享内存区域

```rust
/// 共享内存区域
pub struct SharedMemoryRegion {
    /// 区域唯一标识
    pub id: SharedMemoryId,
    /// 物理帧列表
    pub frames: Vec<PhysFrame>,
    /// 大小（字节）
    pub size: usize,
    /// 创建者
    pub owner: ProcessId,
    /// 访问权限
    pub permissions: SharedMemoryPermissions,
    /// 引用计数
    pub ref_count: AtomicUsize,
    /// 名称（可选）
    pub name: Option<String>,
}

/// 共享内存权限
#[derive(Debug, Clone, Copy)]
pub struct SharedMemoryPermissions {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub owner_only: bool,
}

/// 共享内存 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SharedMemoryId(u64);

/// 共享内存管理器
pub struct SharedMemoryManager {
    regions: SpinLock<BTreeMap<SharedMemoryId, SharedMemoryRegion>>,
    next_id: AtomicU64,
    /// 总共享内存使用量
    total_shared_bytes: AtomicU64,
}

impl SharedMemoryManager {
    /// 创建共享内存区域
    pub fn create(
        &self,
        size: usize,
        owner: ProcessId,
        permissions: SharedMemoryPermissions,
        name: Option<String>,
    ) -> Result<SharedMemoryId, SharedMemoryError> {
        let page_count = (size + 4095) / 4096;

        // 分配物理帧
        let frames = {
            let mut alloc = FRAME_ALLOCATOR.lock();
            let mut frames = Vec::with_capacity(page_count);
            for _ in 0..page_count {
                let frame = alloc.allocate_frame()
                    .ok_or(SharedMemoryError::OutOfMemory)?;
                frames.push(frame);
            }
            frames
        };

        let id = SharedMemoryId(self.next_id.fetch_add(1, Ordering::SeqCst));

        let region = SharedMemoryRegion {
            id,
            frames,
            size: page_count * 4096,
            owner,
            permissions,
            ref_count: AtomicUsize::new(1),
            name,
        };

        self.total_shared_bytes.fetch_add(region.size as u64, Ordering::Relaxed);
        self.regions.lock().insert(id, region);

        Ok(id)
    }

    /// 将共享内存映射到进程地址空间
    pub fn map_to_process(
        &self,
        shm_id: SharedMemoryId,
        target_pid: ProcessId,
        map_addr: Option<VirtAddr>,
    ) -> Result<VirtAddr, SharedMemoryError> {
        let regions = self.regions.lock();
        let region = regions.get(&shm_id)
            .ok_or(SharedMemoryError::InvalidId)?;

        // 检查权限
        if region.permissions.owner_only && target_pid != region.owner {
            return Err(SharedMemoryError::PermissionDenied);
        }

        // 映射到目标进程页表
        let target_page_table = process::get_page_table(target_pid)?;
        let base_addr = map_addr.unwrap_or_else(|| {
            process::find_free_region(target_pid, region.size)
        });

        for (i, frame) in region.frames.iter().enumerate() {
            let page = Page::from_start_address(
                VirtAddr::new(base_addr.as_u64() + i as u64 * 4096)
            ).unwrap();

            target_page_table.map_page(page, *frame, &MapOptions::user())?;
        }

        region.ref_count.fetch_add(1, Ordering::SeqCst);
        Ok(base_addr)
    }

    /// 解除映射
    pub fn unmap_from_process(
        &self,
        shm_id: SharedMemoryId,
        target_pid: ProcessId,
    ) -> Result<(), SharedMemoryError> {
        let regions = self.regions.lock();
        let region = regions.get(&shm_id)
            .ok_or(SharedMemoryError::InvalidId)?;

        let count = region.ref_count.fetch_sub(1, Ordering::SeqCst);
        if count == 1 {
            // 最后一个引用，释放物理帧
            drop(regions);
            self.destroy(shm_id);
        }

        Ok(())
    }
}
```

---

## 7. Agent 内存隔离

### 7.1 隔离策略

```rust
/// Agent 内存隔离策略
pub struct AgentMemoryIsolation {
    /// Agent ID
    pub agent_id: AgentId,
    /// 独立页表
    pub page_table: PageTableManager,
    /// 允许的共享区域列表
    pub allowed_shared_regions: Vec<SharedMemoryId>,
    /// 内存配额
    pub quota: MemoryQuota,
    /// 安全标签
    pub security_label: SecurityLabel,
}

/// 内存配额
#[derive(Debug, Clone)]
pub struct MemoryQuota {
    /// 最大物理内存（字节）
    pub max_physical: u64,
    /// 最大虚拟内存（字节）
    pub max_virtual: u64,
    /// 最大共享内存（字节）
    pub max_shared: u64,
    /// 最大映射数
    pub max_mappings: usize,
}

/// 安全标签
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityLabel {
    pub level: SecurityLevel,
    pub compartment: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecurityLevel {
    Untrusted = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Kernel = 4,
}

impl AgentMemoryIsolation {
    /// 验证内存访问权限
    pub fn check_access(
        &self,
        addr: VirtAddr,
        size: usize,
        access_type: AccessType,
    ) -> Result<(), MemoryAccessError> {
        // 1. 检查地址是否在 Agent 的合法范围内
        if !self.is_valid_range(addr, size) {
            return Err(MemoryAccessError::InvalidRange { addr, size });
        }

        // 2. 检查是否超出配额
        let current_usage = self.current_usage();
        if current_usage + size as u64 > self.quota.max_physical {
            return Err(MemoryAccessError::QuotaExceeded {
                current: current_usage,
                requested: size as u64,
                limit: self.quota.max_physical,
            });
        }

        // 3. 检查安全标签
        if access_type == AccessType::Write && self.security_label.level == SecurityLevel::Untrusted {
            // 非可信 Agent 的写操作需要额外验证
            return Err(MemoryAccessError::SecurityViolation {
                reason: "Untrusted agent write blocked".into(),
            });
        }

        Ok(())
    }
}
```

---

## 8. 缺页中断处理

### 8.1 缺页故障类型

```rust
/// 缺页故障类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultType {
    /// 缺页（页面未映射）
    NotPresent,
    /// 写入只读页
    WriteToReadOnly,
    /// 用户态访问内核页
    UserAccessKernel,
    /// 执行不可执行页（NX 违规）
    ExecuteNoExecute,
    /// 保留位被设置
    ReservedBitSet,
    /// SMAP 违规（内核访问用户页）
    SmapViolation,
}

/// 缺页错误信息
#[derive(Debug, Clone)]
pub struct PageFaultInfo {
    pub fault_address: VirtAddr,
    pub fault_type: PageFaultType,
    pub instruction_pointer: VirtAddr,
    pub error_code: u64,
    pub process_id: ProcessId,
    pub is_user_mode: bool,
}

/// 缺页处理结果
#[derive(Debug, Clone, Copy)]
pub enum PageFaultResult {
    /// 已成功处理
    Resolved,
    /// 需要扩展栈
    StackGrowth,
    /// 写时复制
    CopyOnWrite,
    /// 共享内存按需映射
    DemandMap,
    /// 无法恢复的故障
    KillProcess,
}

impl PageFaultInfo {
    /// 从 CR2 和错误码解析
    pub fn from_registers(error_code: u64) -> Self {
        let fault_address = VirtAddr::new(unsafe { x86_64::registers::control::Cr2::read() });
        let is_present = (error_code & 0x1) != 0;
        let is_write = (error_code & 0x2) != 0;
        let is_user = (error_code & 0x4) != 0;
        let is_reserved = (error_code & 0x8) != 0;
        let is_instruction = (error_code & 0x10) != 0;
        let is_smap = (error_code & 0x20) != 0;

        let fault_type = if is_smap {
            PageFaultType::SmapViolation
        } else if is_reserved {
            PageFaultType::ReservedBitSet
        } else if is_instruction && !is_present {
            PageFaultType::ExecuteNoExecute
        } else if is_user && !is_present {
            PageFaultType::UserAccessKernel
        } else if is_write && is_present {
            PageFaultType::WriteToReadOnly
        } else {
            PageFaultType::NotPresent
        };

        Self {
            fault_address,
            fault_type,
            instruction_pointer: VirtAddr::new(unsafe {
                x86_64::registers::rip::read()
            }),
            error_code,
            process_id: scheduler::current_pid(),
            is_user_mode: is_user,
        }
    }
}
```

### 8.2 缺页处理状态机

```
                    ┌──────────────┐
                    │  缺页中断触发  │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │ 解析故障信息  │
                    │ CR2 + Error  │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ 内核缺页  │ │用户缺页   │ │SMAP违规  │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │            │            │
             ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │KASLR/VM  │ │按需分页   │ │临时允许  │
        │分配      │ │COW/栈扩展│ │访问      │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │            │            │
             └────────────┼────────────┘
                          ▼
                   ┌──────────────┐
                   │ 恢复执行/终止 │
                   └──────────────┘
```

### 8.3 缺页处理实现

```rust
/// 缺页中断处理函数
pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    let info = PageFaultInfo::from_registers(error_code);

    let result = match info.fault_type {
        PageFaultType::NotPresent => handle_not_present(&info),
        PageFaultType::WriteToReadOnly => handle_cow(&info),
        PageFaultType::UserAccessKernel => handle_user_kernel_access(&info),
        PageFaultType::ExecuteNoExecute => handle_nx_violation(&info),
        PageFaultType::ReservedBitSet => handle_reserved_bit(&info),
        PageFaultType::SmapViolation => handle_smap_violation(&info),
    };

    match result {
        PageFaultResult::Resolved
        | PageFaultResult::StackGrowth
        | PageFaultResult::CopyOnWrite
        | PageFaultResult::DemandMap => {
            // 恢复执行
        }
        PageFaultResult::KillProcess => {
            // 发送 SIGSEGV 给进程
            process::send_signal(info.process_id, Signal::Segfault(info));
        }
    }
}

/// 处理缺页（按需分页）
fn handle_not_present(info: &PageFaultInfo) -> PageFaultResult {
    let proc = process::get(info.process_id);

    // 检查是否是栈扩展
    if proc.stack_region.contains(info.fault_address) {
        let new_page = Page::containing_address(info.fault_address);
        let frame = match FRAME_ALLOCATOR.lock().allocate_frame() {
            Some(f) => f,
            None => return PageFaultResult::KillProcess,
        };

        proc.page_table.map_page(new_page, frame, &MapOptions::stack())
            .map(|_| PageFaultResult::StackGrowth)
            .unwrap_or(PageFaultResult::KillProcess)
    }
    // 检查是否是 mmap 按需映射
    else if let Some(vma) = proc.find_vma(info.fault_address) {
        if vma.is_demand_paged {
            let frame = FRAME_ALLOCATOR.lock().allocate_frame()
                .unwrap(); // OOM 由 OOM handler 处理
            proc.page_table.map_page(
                Page::containing_address(info.fault_address),
                frame,
                &vma.map_options,
            ).unwrap();
            PageFaultResult::DemandMap
        } else {
            PageFaultResult::KillProcess
        }
    } else {
        PageFaultResult::KillProcess
    }
}

/// 处理写时复制
fn handle_cow(info: &PageFaultInfo) -> PageFaultResult {
    let proc = process::get(info.process_id);

    // 分配新帧
    let new_frame = FRAME_ALLOCATOR.lock().allocate_frame()
        .ok_or(PageFaultResult::KillProcess).ok()?;

    // 复制旧帧内容到新帧
    let old_frame = proc.page_table.translate(info.fault_address)
        .and_then(|r| r.frame())
        .unwrap();

    unsafe {
        let src = old_frame.start_address().as_u64 as *const u8;
        let dst = new_frame.start_address().as_u64 as *mut u8;
        core::ptr::copy_nonoverlapping(src, dst, 4096);
    }

    // 重新映射为可写
    let page = Page::containing_address(info.fault_address);
    proc.page_table.unmap_page(page).unwrap();
    let mut opts = MapOptions::user();
    opts.flags |= PageTableFlags::WRITABLE;
    proc.page_table.map_page(page, new_frame, &opts).unwrap();

    PageFaultResult::CopyOnWrite
}
```

---

## 9. 内存保护机制

### 9.1 保护特性

| 特性 | 全称 | 描述 | 启用阶段 |
|------|------|------|----------|
| NX | No-Execute | 标记页面为不可执行 | CPU 初始化 |
| SMAP | Supervisor Mode Access Prevention | 禁止内核直接访问用户页 | CPU 初始化 |
| SMEP | Supervisor Mode Execution Prevention | 禁止内核执行用户页代码 | CPU 初始化 |
| WP | Write Protect | CR0.WP，保护只读页 | CPU 初始化 |
| PCID | Process Context Identifier | 加速 TLB 刷新 | CPU 初始化 |

### 9.2 保护机制实现

```rust
pub mod protection {
    /// 内存保护配置
    pub struct MemoryProtectionConfig {
        pub nx_enabled: bool,
        pub smap_enabled: bool,
        pub smep_enabled: bool,
        pub wp_enabled: bool,
        pub pcid_enabled: bool,
    }

    impl MemoryProtectionConfig {
        /// 启用所有保护机制
        pub fn enable_all() -> Self {
            unsafe {
                // CR0.WP
                x86_64::registers::control::Cr0::update(|cr0| {
                    cr0 |= x86_64::registers::control::Cr0Flags::WRITE_PROTECT;
                });

                // CR4: SMEP, SMAP, PCID
                x86_64::registers::control::Cr4::update(|cr4| {
                    cr4 |= x86_64::registers::control::Cr4Flags::SMEP_ENABLE;
                    cr4 |= x86_64::registers::control::Cr4Flags::SMAP_ENABLE;
                    cr4 |= x86_64::registers::control::Cr4Flags::PCID_ENABLE;
                });

                // EFER.NXE
                x86_64::registers::model_specific::Efer::update(|efer| {
                    efer.set(x86_64::registers::model_specific::EferFlags::NO_EXECUTE_ENABLE);
                });
            }

            Self {
                nx_enabled: true,
                smap_enabled: true,
                smep_enabled: true,
                wp_enabled: true,
                pcid_enabled: true,
            }
        }

        /// 临时允许内核访问用户页（用于 copy_from_user 等）
        pub fn temporarily_allow_user_access<F, R>(f: F) -> R
        where
            F: FnOnce() -> R,
        {
            unsafe {
                // 清除 AC 标志（允许用户页访问）
                asm!("stac");
                let result = f();
                // 设置 AC 标志（恢复 SMAP 保护）
                asm!("clac");
                result
            }
        }
    }
}
```

---

## 10. 虚拟化内存（EPT/NPT）

### 10.1 扩展页表

```rust
/// EPT 指针
pub struct EptPointer {
    pub phys_addr: PhysAddr,
}

/// EPT 映射标志
pub struct EptFlags: u64 {
    const READ = 1 << 0;
    const WRITE = 1 << 1;
    const EXECUTE = 1 << 2;
    const ACCESSED = 1 << 8;
    const DIRTY = 1 << 9;
    const MEMORY_TYPE = 1 << 16;
    const IGNORE_PAT = 1 << 17;
}

/// EPT 管理器
pub struct EptManager {
    /// EPT 页表根
    ept_pml4: PhysAddr,
    /// 帧分配器
    frame_allocator: &'static mut dyn OmniFrameAllocator,
    /// 映射统计
    mapped_pages: AtomicU64,
}

impl EptManager {
    /// 创建新的 EPT
    pub fn new(frame_allocator: &'static mut dyn OmniFrameAllocator) -> Result<Self, EptError> {
        let pml4_frame = frame_allocator.allocate_frame()
            .ok_or(EptError::FrameAllocationFailed)?;

        // 清零 EPT PML4
        let pml4_virt = unsafe {
            VirtAddr::new(pml4_frame.start_address().as_u64())
        };
        unsafe {
            core::ptr::write_bytes(pml4_virt.as_mut_ptr::<u8>(), 0, 4096);
        }

        Ok(Self {
            ept_pml4: pml4_frame.start_address(),
            frame_allocator,
            mapped_pages: AtomicU64::new(0),
        })
    }

    /// 映射 GPA → HPA
    pub fn map(
        &mut self,
        guest_phys: PhysAddr,
        host_phys: PhysAddr,
        flags: EptFlags,
    ) -> Result<(), EptError> {
        // 遍历 EPT 四级页表
        let page_offset = guest_phys.as_u64() & 0xFFF;
        let pt_index = ((guest_phys.as_u64() >> 12) & 0x1FF) as usize;
        let pd_index = ((guest_phys.as_u64() >> 21) & 0x1FF) as usize;
        let pdpt_index = ((guest_phys.as_u64() >> 30) & 0x1FF) as usize;
        let pml4_index = ((guest_phys.as_u64() >> 39) & 0x1FF) as usize;

        // ... 逐级分配和映射 ...

        self.mapped_pages.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 获取 EPTP
    pub fn ept_pointer(&self) -> EptPointer {
        EptPointer {
            phys_addr: self.ept_pml4,
        }
    }
}
```

---

## 11. 内存统计与监控

### 11.1 监控 API

```rust
/// 内存统计信息
#[derive(Debug, Clone)]
pub struct MemoryStatistics {
    /// 总物理内存
    pub total_physical: u64,
    /// 可用物理内存
    pub available_physical: u64,
    /// 内核使用
    pub kernel_used: u64,
    /// 用户空间使用
    pub user_used: u64,
    /// 共享内存使用
    pub shared_used: u64,
    /// 缓存/缓冲区
    pub cached: u64,
    /// 页面换入次数
    pub page_ins: u64,
    /// 页面换出次数
    pub page_outs: u64,
    /// 缺页中断次数
    pub page_faults: u64,
    /// OOM 杀死次数
    pub oom_kills: u64,
    /// 峰值使用
    pub peak_usage: u64,
}

/// 内存监控 API
pub trait MemoryMonitor {
    /// 获取全局统计
    fn global_stats(&self) -> MemoryStatistics;

    /// 获取指定进程的内存使用
    fn process_stats(&self, pid: ProcessId) -> ProcessMemoryStats;

    /// 获取指定 Agent 的内存使用
    fn agent_stats(&self, agent_id: AgentId) -> AgentMemoryStats;

    /// 设置内存使用阈值告警
    fn set_threshold_alert(
        &self,
        threshold_percent: u8,
        callback: fn(MemoryStatistics),
    );

    /// 获取内存使用历史
    fn history(&self, duration_us: u64) -> Vec<(u64, MemoryStatistics)>;
}

/// 进程内存统计
#[derive(Debug, Clone)]
pub struct ProcessMemoryStats {
    pub pid: ProcessId,
    pub virtual_size: u64,
    pub resident_set_size: u64,
    pub shared_clean: u64,
    pub shared_dirty: u64,
    pub private_clean: u64,
    pub private_dirty: u64,
    pub swap_usage: u64,
}

/// Agent 内存统计
#[derive(Debug, Clone)]
pub struct AgentMemoryStats {
    pub agent_id: AgentId,
    pub total_mapped: u64,
    pub shared_regions: usize,
    pub quota_used: u64,
    pub quota_limit: u64,
    pub utilization_percent: f64,
}
```

---

## 12. OOM 处理策略

### 12.1 OOM 管理器

```rust
/// OOM 处理策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OomStrategy {
    /// 杀死内存使用最多的进程
    KillLargest,
    /// 杀死优先级最低的进程
    KillLowestPriority,
    /// 杀死最近分配最多的进程
    KillLargestRecentAlloc,
    /// 杀死违反配额的 Agent
    KillQuotaViolator,
    /// 拒绝分配（返回错误）
    DenyAllocation,
}

/// OOM 管理器
pub struct OomManager {
    strategy: OomStrategy,
    /// 内存压力等级
    pressure_level: AtomicU8,
    /// OOM 事件回调
    callbacks: SpinLock<Vec<Box<dyn Fn(OomEvent)>>>,
    /// 历史记录
    history: SpinLock<VecDeque<OomEvent>>,
}

/// 内存压力等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum MemoryPressure {
    Low = 0,       // < 50% 使用
    Medium = 1,    // 50-75% 使用
    High = 2,      // 75-90% 使用
    Critical = 3,  // > 90% 使用
    Oom = 4,       // 内存耗尽
}

/// OOM 事件
#[derive(Debug, Clone)]
pub struct OomEvent {
    pub timestamp_us: u64,
    pub pressure_level: MemoryPressure,
    pub action_taken: OomAction,
    pub freed_bytes: u64,
    pub victim_pid: Option<ProcessId>,
}

#[derive(Debug, Clone)]
pub enum OomAction {
    ProcessKilled { pid: ProcessId, reason: String },
    CacheReclaimed { bytes: u64 },
    AllocationDenied { size: u64 },
    Warning { message: String },
}

impl OomManager {
    /// 处理内存不足
    pub fn handle_oom(&self, requested_size: u64) -> Result<(), OomError> {
        let stats = MEMORY_MONITOR.global_stats();
        let usage_percent = (stats.kernel_used + stats.user_used) * 100 / stats.total_physical;

        // 更新压力等级
        let pressure = if usage_percent > 95 {
            MemoryPressure::Oom
        } else if usage_percent > 90 {
            MemoryPressure::Critical
        } else if usage_percent > 75 {
            MemoryPressure::High
        } else if usage_percent > 50 {
            MemoryPressure::Medium
        } else {
            MemoryPressure::Low
        };
        self.pressure_level.store(pressure as u8, Ordering::SeqCst);

        // 根据策略处理
        match self.strategy {
            OomStrategy::DenyAllocation => {
                Err(OomError::AllocationDenied { requested_size })
            }
            OomStrategy::KillLargest => {
                self.kill_largest_process(requested_size)
            }
            OomStrategy::KillLowestPriority => {
                self.kill_lowest_priority_process(requested_size)
            }
            OomStrategy::KillQuotaViolator => {
                self.kill_quota_violator(requested_size)
            }
            _ => Err(OomError::AllocationDenied { requested_size }),
        }
    }

    /// 杀死内存使用最大的进程
    fn kill_largest_process(&self, needed: u64) -> Result<(), OomError> {
        let processes = process::all_processes();
        let victim = processes.iter()
            .max_by_key(|p| p.memory_stats().resident_set_size);

        if let Some(proc) = victim {
            let freed = proc.memory_stats().resident_set_size;
            process::kill(proc.pid(), Signal::Kill);
            self.record_event(OomEvent {
                timestamp_us: current_time_us(),
                pressure_level: MemoryPressure::Oom,
                action_taken: OomAction::ProcessKilled {
                    pid: proc.pid(),
                    reason: format!("OOM: largest consumer ({}MB)", freed / (1024*1024)),
                },
                freed_bytes: freed,
                victim_pid: Some(proc.pid()),
            });
            Ok(())
        } else {
            Err(OomError::NoVictimFound)
        }
    }
}
```

---

## 13. 性能约束

### 13.1 性能目标

| 操作 | 目标延迟 | 测量条件 |
|------|----------|----------|
| 小对象分配（<1KB） | < 100ns | slab 缓存命中 |
| 大对象分配（>4KB） | < 500ns | 帧分配 + 映射 |
| 页面映射 | < 500ns | 已有页表层级 |
| 页面取消映射 | < 200ns | TLB 刷新除外 |
| 缺页处理（按需分页） | < 1us | 帧分配 + 零填充 + 映射 |
| 缺页处理（COW） | < 5us | 页面复制 + 重映射 |
| TLB 刷新（全局） | < 10us | CR3 写入 |
| TLB 刷新（单页） | < 100ns | INVLPG |
| 共享内存创建 | < 10us | 4KB 区域 |
| 共享内存映射 | < 2us | 每页 |

### 13.2 性能优化策略

1. **Slab 缓存预取**: 每个 CPU 核心维护本地 slab 缓存，减少锁竞争
2. **TLB 批量刷新**: 使用 PCID 避免全局 TLB 刷新
3. **大页支持**: 2MB/1GB 大页减少 TLB miss
4. **延迟释放**: 帧释放后不立即清零，按需清零（安全考虑除外）
5. **NUMA 感知**: 优先在本地 NUMA 节点分配

---

## 14. 测试用例

### 14.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap_frame_allocator_basic() {
        let regions = vec![
            MemoryRegion {
                base_addr: 0x100000,
                length: 0x100000, // 1MB = 256 frames
                region_type: MemoryRegionType::Usable,
            },
        ];
        let boot_info = create_test_boot_info(regions);
        let mut allocator = BitmapFrameAllocator::from_boot_info(&boot_info).unwrap();

        assert_eq!(allocator.total_frames(), 256);
        assert!(allocator.free_frames() > 0);

        // 分配一帧
        let frame = allocator.allocate_frame().unwrap();
        assert_eq!(frame.start_address().as_u64(), 0x100000);

        // 释放帧
        allocator.deallocate_frame(frame);
        assert!(allocator.free_frames() > 0);
    }

    #[test]
    fn test_bitmap_frame_allocator_exhaustion() {
        let regions = vec![
            MemoryRegion {
                base_addr: 0x100000,
                length: 0x2000, // 2 frames
                region_type: MemoryRegionType::Usable,
            },
        ];
        let boot_info = create_test_boot_info(regions);
        let mut allocator = BitmapFrameAllocator::from_boot_info(&boot_info).unwrap();

        let f1 = allocator.allocate_frame().unwrap();
        let f2 = allocator.allocate_frame().unwrap();
        let f3 = allocator.allocate_frame();
        assert!(f3.is_none()); // 内存耗尽

        allocator.deallocate_frame(f1);
        let f4 = allocator.allocate_frame();
        assert!(f4.is_some()); // 释放后可再次分配
    }

    #[test]
    fn test_page_table_map_unmap() {
        let mut pt = create_test_page_table();
        let page = Page::from_start_address(VirtAddr::new(0x400000)).unwrap();
        let frame = create_test_frame(0x100000);

        let flush = pt.map_page(page, frame, &MapOptions::default()).unwrap();
        flush.flush_all();

        let translated = pt.translate(VirtAddr::new(0x400000));
        assert!(translated.is_some());

        let (unmapped_frame, flush) = pt.unmap_page(page).unwrap();
        flush.flush_all();
        assert_eq!(unmapped_frame.start_address().as_u64(), 0x100000);
    }

    #[test]
    fn test_shared_memory_create_map() {
        let manager = SharedMemoryManager::new();
        let shm_id = manager.create(4096, ProcessId::new(1),
            SharedMemoryPermissions::read_write(), None).unwrap();

        let addr = manager.map_to_process(shm_id, ProcessId::new(2), None).unwrap();
        assert!(!addr.is_zero());

        manager.unmap_from_process(shm_id, ProcessId::new(2)).unwrap();
    }

    #[test]
    fn test_page_fault_type_parsing() {
        // 缺页（未映射）
        let info = PageFaultInfo::from_error_code(0x00);
        assert_eq!(info.fault_type, PageFaultType::NotPresent);

        // 写入只读页
        let info = PageFaultInfo::from_error_code(0x03);
        assert_eq!(info.fault_type, PageFaultType::WriteToReadOnly);

        // 用户态访问
        let info = PageFaultInfo::from_error_code(0x05);
        assert_eq!(info.fault_type, PageFaultType::UserAccessKernel);

        // NX 违规
        let info = PageFaultInfo::from_error_code(0x10);
        assert_eq!(info.fault_type, PageFaultType::ExecuteNoExecute);
    }

    #[test]
    fn test_memory_quota_enforcement() {
        let quota = MemoryQuota {
            max_physical: 1024 * 1024, // 1MB
            max_virtual: 4 * 1024 * 1024,
            max_shared: 256 * 1024,
            max_mappings: 100,
        };
        let isolation = AgentMemoryIsolation::new(
            AgentId::new(1), quota, SecurityLabel::medium()
        );

        // 在限额内
        assert!(isolation.check_access(VirtAddr::new(0x1000), 4096, AccessType::Read).is_ok());

        // 超出限额
        let result = isolation.check_access(VirtAddr::new(0x1000), 2 * 1024 * 1024, AccessType::Read);
        assert!(matches!(result, Err(MemoryAccessError::QuotaExceeded { .. })));
    }

    #[test]
    fn test_oom_manager_pressure_levels() {
        let oom = OomManager::new(OomStrategy::KillLargest);

        // 模拟不同压力等级
        oom.update_pressure(30); // Low
        assert_eq!(oom.pressure_level(), MemoryPressure::Low);

        oom.update_pressure(80); // High
        assert_eq!(oom.pressure_level(), MemoryPressure::High);

        oom.update_pressure(96); // Oom
        assert_eq!(oom.pressure_level(), MemoryPressure::Oom);
    }

    #[test]
    fn test_map_options_variants() {
        let kernel_opts = MapOptions::default();
        assert!(kernel_opts.global);
        assert!(!kernel_opts.user_accessible);

        let user_opts = MapOptions::user();
        assert!(user_opts.user_accessible);
        assert!(!user_opts.global);

        let code_opts = MapOptions::code_segment();
        assert!(!code_opts.execute_disable);

        let stack_opts = MapOptions::stack();
        assert!(stack_opts.execute_disable);
    }
}
```

### 14.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_memory_stress_allocation() {
        // 压力测试：分配和释放 10000 次
        for _ in 0..10_000 {
            let layout = Layout::from_size_align(128, 8).unwrap();
            let ptr = unsafe { alloc(layout) };
            assert!(!ptr.is_null());
            unsafe { dealloc(ptr, layout) };
        }
    }

    #[test]
    fn test_shared_memory_ipc() {
        // 两个进程通过共享内存通信
        let shm_id = create_shared_memory(4096);
        let writer_addr = map_shared(shm_id, PID_WRITER);
        let reader_addr = map_shared(shm_id, PID_READER);

        // 写入数据
        unsafe { (writer_addr as *mut u64).write(0xDEADBEEF) };

        // 读取验证
        let value = unsafe { (reader_addr as *const u64).read() };
        assert_eq!(value, 0xDEADBEEF);
    }

    #[test]
    fn test_demand_paging() {
        // 触发按需分页
        let page = allocate_virtual_page();
        assert_eq!(page.frame_count(), 0); // 尚未分配物理帧

        // 触发缺页
        unsafe { *(page.addr() as *mut u8) = 42 };
        assert_eq!(page.frame_count(), 1); // 已分配物理帧
    }

    #[test]
    fn test_cow_fork() {
        // fork 后修改触发 COW
        let parent_page = allocate_and_write(0x1234);
        let child_pid = fork();

        if child_pid == 0 {
            // 子进程：写入触发 COW
            unsafe { *(parent_page.addr() as *mut u32) = 0x5678 };
            // 验证子进程有独立副本
            assert_eq!(unsafe { *(parent_page.addr() as *const u32) }, 0x5678);
        } else {
            // 父进程：值不变
            assert_eq!(unsafe { *(parent_page.addr() as *const u32) }, 0x1234);
        }
    }
}
```

### 14.3 性能基准测试

```rust
#[cfg(test)]
mod benchmarks {
    #[bench]
    fn bench_small_alloc(b: &mut Bencher) {
        b.iter(|| {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = unsafe { alloc(layout) };
            unsafe { dealloc(ptr, layout) };
        });
    }

    #[bench]
    fn bench_page_map(b: &mut Bencher) {
        b.iter(|| {
            let page = Page::from_start_address(VirtAddr::new(0x800000)).unwrap();
            let frame = allocate_test_frame();
            map_page(page, frame);
            unmap_page(page);
        });
    }

    #[bench]
    fn bench_page_fault(b: &mut Bencher) {
        b.iter(|| {
            trigger_demand_page_fault();
        });
    }
}
```

---

## 15. 参考资料

- x86_64 crate 文档: https://docs.rs/x86_64
- Intel SDM Volume 3, Chapter 4: Paging
- Intel SDM Volume 3, Chapter 28: VMX Support for Address Translation
- bumpalo crate: https://docs.rs/bumpalo
- Linux mm 子系统文档
