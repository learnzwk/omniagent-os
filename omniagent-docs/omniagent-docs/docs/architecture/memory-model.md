# OmniAgent OS 内存模型规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: L1 架构设计文档
> **目标读者**: 内核开发者、虚拟化开发者、Agent 运行时开发者

---

## 1. 文档目的

本文档详细描述 OmniAgent OS 的内存管理模型，包括物理内存管理、虚拟内存管理（4 级页表）、内核与用户空间地址布局、Agent 隔离机制、共享内存设计、内存分配策略、按需分页、内存保护机制、NUMA 感知以及虚拟化内存（EPT/NPT）支持。

---

## 2. 物理内存管理

### 2.1 物理内存检测

系统启动时，通过 bootloader crate 传递的内存映射信息检测可用物理内存：

```rust
/// 物理内存区域类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryRegionType {
    /// 可用内存
    Usable,
    /// 已被占用（内核代码、bootloader 等）
    Reserved,
    /// ACPI 可回收内存
    AcpiReclaimable,
    /// ACPI NVS 内存
    AcpiNvs,
    /// 内存映射 I/O
    MemoryMappedIo,
    /// 不可用（损坏的内存区域）
    Unusable,
}

/// 物理内存区域
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// 起始物理地址
    pub start: PhysAddr,
    /// 结束物理地址 (不包含)
    pub end: PhysAddr,
    /// 区域类型
    pub region_type: MemoryRegionType,
}

impl MemoryRegion {
    /// 区域大小 (字节)
    pub fn size(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
    }
}
```

### 2.2 帧分配器

物理内存以 4 KB 页帧为单位进行管理，使用 `x86_64` crate 提供的帧分配器接口：

```rust
use x86_64::structures::paging::{
    PhysFrame, Size4KiB, FrameAllocator, UnusedPhysFrame,
};

/// 物理帧分配器
pub struct OmniFrameAllocator {
    /// 可用帧的位图
    bitmap: SpinLock<FrameBitmap>,
    /// 可用帧总数
    total_frames: AtomicU64,
    /// 已分配帧数
    allocated_frames: AtomicU64,
    /// 帧分配统计
    stats: FrameAllocStats,
}

/// 帧位图 (每个 bit 代表一个 4KB 页帧)
pub struct FrameBitmap {
    /// 位图数据
    data: Box<[u64]>,
    /// 位图覆盖的起始帧号
    base_frame: u64,
    /// 位图覆盖的帧数
    frame_count: u64,
    /// 下一次搜索的起始位置 (优化分配速度)
    next_free_hint: u64,
}

/// 帧分配统计
pub struct FrameAllocStats {
    /// 总分配次数
    pub total_allocations: AtomicU64,
    /// 总释放次数
    pub total_deallocations: AtomicU64,
    /// 连续分配次数
    pub contiguous_allocations: AtomicU64,
    /// 分配失败次数
    pub allocation_failures: AtomicU64,
    /// 当前碎片率 (0.0 ~ 1.0)
    pub fragmentation_ratio: AtomicU64,  // 定点数，除以 10000
}

unsafe impl FrameAllocator<Size4KiB> for OmniFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let mut bitmap = self.bitmap.lock();
        let frame_idx = bitmap.find_free_frame()?;
        bitmap.set_used(frame_idx);
        self.allocated_frames.fetch_add(1, Ordering::Relaxed);
        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
        let frame_number = bitmap.base_frame + frame_idx;
        Some(PhysFrame::containing_address(
            PhysAddr::new(frame_number * PAGE_SIZE)
        ))
    }
}

/// 分配连续物理帧 (用于 DMA、大块共享内存等)
pub fn allocate_contiguous_frames(count: usize) -> Result<Vec<PhysFrame>, MemoryError> {
    let allocator = FRAME_ALLOCATOR.get().unwrap();
    let mut bitmap = allocator.bitmap.lock();

    // 搜索 count 个连续空闲帧
    let start_idx = bitmap.find_free_range(count)?;
    for i in 0..count {
        bitmap.set_used(start_idx + i);
    }

    let frames: Vec<PhysFrame> = (0..count)
        .map(|i| {
            let frame_number = bitmap.base_frame + start_idx + i;
            PhysFrame::containing_address(
                PhysAddr::new(frame_number * PAGE_SIZE)
            )
        })
        .collect();

    allocator.allocated_frames.fetch_add(count as u64, Ordering::Relaxed);
    allocator.stats.contiguous_allocations.fetch_add(1, Ordering::Relaxed);
    Ok(frames)
}
```

### 2.3 物理内存布局示例

```
物理地址空间 (以 16 GB 系统为例)

0x0000_0000 ┌─────────────────────────────────────┐
            │ 传统内存区域 (0 - 1 MB)             │
            │ - IVT + BDA (0 - 0x500)            │
            │ - EBDA (0x70000 - 0xA0000)         │
            │ - VGA 缓冲区 (0xA0000 - 0xC0000)   │
            │ - ROM 区域 (0xC0000 - 0x100000)    │
0x0010_0000 ├─────────────────────────────────────┤
            │ 内核代码和数据                       │
            │ - .text: ~1 MB                     │
            │ - .rodata: ~256 KB                 │
            │ - .data + .bss: ~512 KB            │
            │ 总计约 2 MB                         │
0x0030_0000 ├─────────────────────────────────────┤
            │ 内核堆 (bumpalo)                    │
            │ 初始 4 MB，可增长                    │
0x0070_0000 ├─────────────────────────────────────┤
            │                                     │
            │     可用物理内存                     │
            │     (由帧分配器管理)                 │
            │                                     │
            │     总计约 15.5 GB                  │
            │                                     │
0x3FFF_0000 ├─────────────────────────────────────┤
            │ ACPI 表 (可回收)                     │
0x4000_0000 ├─────────────────────────────────────┤
            │                                     │
            │     设备 MMIO 区域                   │
            │     - PCIe 配置空间                 │
            │     - 设备 BAR 映射                 │
            │     - APIC 寄存器                   │
            │                                     │
0xFFFF_FFFF └─────────────────────────────────────┘
```

---

## 3. 虚拟内存：4 级页表

### 3.1 页表层次结构

OmniAgent OS 使用 x86_64 的 4 级页表（PML4 → PDPT → PD → PT）：

```
虚拟地址 (64 位)

  63    48 47    39 38    30 29    21 20    12 11     0
┌────────┬────────┬────────┬────────┬────────┬────────┐
│ 符号扩展│  PML4  │  PDPT  │   PD   │   PT   │ 偏移   │
│ (全0/1) │ 索引   │ 索引   │ 索引   │ 索引   │        │
│ 16 bits │ 9 bits │ 9 bits │ 9 bits │ 9 bits │12 bits │
└────────┴────────┴────────┴────────┴────────┴────────┘

页表遍历过程:

CR3 寄存器
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│ PML4 (Page Map Level 4)                                     │
│ 512 个条目，每个 8 字节                                      │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Entry 0  │ Entry 1  │ ... │ Entry 511                   │ │
│ │          │          │     │                              │ │
│ │ ┌──────┐ │ ┌──────┐ │     │ ┌──────┐                   │ │
│ │ │PDPT 0│ │ │PDPT 1│ │     │ │PDPT N│                   │ │
│ │ └──┬───┘ │ └──────┘ │     │ └──────┘                   │ │
│ └────┼─────┘           │     └────────────────────────────┘ │
└─────┼───────────────────┘                                    │
      ▼                                                        │
┌──────────────────────────────────────────────────────────────┐
│ PDPT (Page Directory Pointer Table)                          │
│ 512 个条目                                                    │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Entry 0  │ Entry 1  │ ... │ Entry 511                   │ │
│ │ ┌──────┐ │ ┌──────┐ │     │ ┌──────┐                   │ │
│ │ │ PD 0 │ │ │ PD 1 │ │     │ │ PD N │                   │ │
│ │ └──┬───┘ │ └──────┘ │     │ └──────┘                   │ │
│ └────┼─────┘           │     └────────────────────────────┘ │
└─────┼───────────────────┘                                    │
      ▼                                                        │
┌──────────────────────────────────────────────────────────────┐
│ PD (Page Directory)                                          │
│ 512 个条目                                                    │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Entry 0  │ Entry 1  │ ... │ Entry 511                   │ │
│ │ ┌──────┐ │ ┌──────┐ │     │ ┌──────┐                   │ │
│ │ │ PT 0 │ │ │ PT 1 │ │     │ │ PT N │                   │ │
│ │ └──┬───┘ │ └──────┘ │     │ └──────┘                   │ │
│ └────┼─────┘           │     └────────────────────────────┘ │
└─────┼───────────────────┘                                    │
      ▼                                                        │
┌──────────────────────────────────────────────────────────────┐
│ PT (Page Table)                                              │
│ 512 个条目，每个指向一个 4KB 物理页                           │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Entry 0  │ Entry 1  │ ... │ Entry 511                   │ │
│ │ ┌──────┐ │ ┌──────┐ │     │ ┌──────┐                   │ │
│ │ │4KB页0│ │ │4KB页1│ │     │ │4KB页N│                   │ │
│ │ └──────┘ │ └──────┘ │     │ └──────┘                   │ │
│ └──────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘

地址空间覆盖范围:
  - PML4 索引: 512 × 512 GB = 256 TB (总虚拟地址空间)
  - PDPT 索引: 512 × 1 GB = 512 GB
  - PD 索引: 512 × 2 MB = 1 GB
  - PT 索引: 512 × 4 KB = 2 MB
```

### 3.2 页表条目格式

```rust
/// 页表条目标志
use x86_64::structures::paging::PageTableFlags;

/// 常用标志组合
pub const FLAGS_KERNEL_CODE: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::GLOBAL;

pub const FLAGS_KERNEL_DATA: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::GLOBAL;

pub const FLAGS_KERNEL_RO: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::GLOBAL;

pub const FLAGS_USER_CODE: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::USER_ACCESSIBLE;

pub const FLAGS_USER_DATA: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::USER_ACCESSIBLE;

pub const FLAGS_USER_RO: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::USER_ACCESSIBLE;

pub const FLAGS_SHARED_MEM: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::USER_ACCESSIBLE
    | PageTableFlags::NO_CACHE;  // 共享内存使用 UC 避免缓存一致性问题

/// 页表条目详细格式 (x86_64)
/// ┌─────────────────────────────────────────────────────────────┐
/// │ 63 62 61 52 51  ┌───┐ 8 7 6 5 4 3 2 1 0                   │
/// │ XD PK AV  ─── │NX │ ─ W P D A PCD PWT U/S R/W P          │
/// └─────────────────────────────────────────────────────────────┘
///
/// Bit 0:  P  (Present)         - 页面存在
/// Bit 1:  R/W (Read/Write)     - 读写权限
/// Bit 2:  U/S (User/Supervisor)- 用户/内核访问
/// Bit 3:  PWT (Page Write Through) - 写穿透缓存
/// Bit 4:  PCD (Page Cache Disable)  - 禁用缓存
/// Bit 5:  A  (Accessed)        - 已被访问
/// Bit 6:  D  (Dirty)           - 已被写入
/// Bit 7:  PS (Page Size)       - 大页标志 (PD/PDPT 级)
/// Bit 8:  G  (Global)          - 全局页 (TLB 不随 CR3 切换刷新)
/// Bit 63: XD (Execute Disable) - 禁止执行 (NX 位)
/// Bits 12-51: 物理页帧地址
/// Bits 52-62: 可用位 (软件使用)
```

### 3.3 页表操作接口

```rust
/// 映射单个 4KB 页
pub fn map_page(
    page: Page<Size4KiB>,
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>> {
    let mut page_table = unsafe {
        active_level_4_table()
    };
    let mut mapper = unsafe {
        OffsetPageTable::new(&mut page_table, VirtAddr::new(PHYS_OFFSET))
    };
    mapper.map_to(page, frame, flags, &mut FRAME_ALLOCATOR)
}

/// 映射 2MB 大页
pub fn map_large_page(
    page: Page<Size2MiB>,
    frame: PhysFrame<Size2MiB>,
    flags: PageTableFlags,
) -> Result<MapperFlush<Size2MiB>, MapToError<Size2MiB>> {
    let mut mapper = get_mapper();
    let flags = flags | PageTableFlags::HUGE_PAGE;
    mapper.map_to(page, frame, flags, &mut FRAME_ALLOCATOR)
}

/// 解除页映射
pub fn unmap_page(page: Page<Size4KiB>) -> Result<(PhysFrame<Size4KiB>, MapperFlush<Size4KiB>), UnmapError> {
    let mut mapper = get_mapper();
    mapper.unmap(page)
}

/// 修改页表权限 (不改映射)
pub fn update_permissions(
    page: Page<Size4KiB>,
    new_flags: PageTableFlags,
) -> Result<MapperFlush<Size4KiB>, TranslateError> {
    let mut mapper = get_mapper();
    let entry = mapper.update_flags(page, new_flags)?;
    Ok(MapperFlush::new(page))
}
```

---

## 4. 内核地址空间布局

### 4.1 高半地址映射

内核使用高半地址空间（`0xFFFF_8000_0000_0000` 以上），采用直接物理内存映射：

```rust
/// 物理内存偏移量 (内核虚拟地址 = 物理地址 + PHYS_OFFSET)
pub const PHYS_OFFSET: u64 = 0xFFFF_8000_0000_0000;

/// 内核代码起始虚拟地址
pub const KERNEL_VIRT_START: u64 = 0xFFFFFFFF_8000_0000;

/// 内核栈起始地址 (每个 CPU)
pub const KERNEL_STACK_BASE: u64 = 0xFFFFFFFF_F000_0000;

/// 设备 MMIO 映射基址
pub const MMIO_VIRT_BASE: u64 = 0xFFFF_FE00_0000_0000;

/// Fixmap (固定映射) 基址
pub const FIXMAP_BASE: u64 = 0xFFFF_FE80_0000_0000;

/// APIC 寄存器映射地址
pub const LOCAL_APIC_VIRT: u64 = 0xFFFF_FE00_0000_0000;
pub const IO_APIC_VIRT: u64   = 0xFFFF_FE00_0000_1000;
```

### 4.2 内核地址空间详细布局

```
内核虚拟地址空间 (高半部分)

0xFFFF_FFFF_FFFF_FFFF ┌─────────────────────────────────────┐
                      │ 非规范地址区域                       │
                      │ (Canonical Hole)                     │
                      │ 用于捕获错误指针                      │
                      │ 大小: 128 TB                         │
0xFFFF_8000_0000_0000 ├─────────────────────────────────────┤
                      │ 物理内存直接映射区域                  │
                      │ phys_addr + PHYS_OFFSET              │
                      │                                     │
                      │ 用途:                               │
                      │ - 访问任意物理内存                    │
                      │ - DMA 缓冲区映射                     │
                      │ - 帧分配器操作                       │
                      │                                     │
                      │ 映射属性: WB (Write-Back)            │
                      │                                     │
                      │ 覆盖范围: 0 ~ 物理内存上限            │
                      │                                     │
0xFFFF_F000_0000_0000 ├─────────────────────────────────────┤
                      │ Fixmap (固定映射区域)                 │
                      │                                     │
                      │ ┌─────────────────────────────────┐ │
                      │ │ Fixmap Entry 0: 临时页表映射    │ │
                      │ │ Fixmap Entry 1: 早期帧缓冲      │ │
                      │ │ Fixmap Entry 2: ACPI 表访问     │ │
                      │ │ Fixmap Entry 3: SMP 启动页      │ │
                      │ │ ...                             │ │
                      │ │ Fixmap Entry N: 保留             │ │
                      │ └─────────────────────────────────┘ │
                      │                                     │
0xFFFF_FE80_0000_0000 ├─────────────────────────────────────┤
                      │ 设备 MMIO 映射区域                   │
                      │                                     │
                      │ ┌─────────────────────────────────┐ │
                      │ │ Local APIC 寄存器               │ │
                      │ │ I/O APIC 寄存器                 │ │
                      │ │ PCIe 配置空间                   │ │
                      │ │ PCIe BAR 映射                   │ │
                      │ │ HPET 寄存器                     │ │
                      │ │ UART 串口寄存器                 │ │
                      │ │ GPU MMIO 寄存器                 │ │
                      │ │ ...                             │ │
                      │ └─────────────────────────────────┘ │
                      │                                     │
                      │ 映射属性: UC (Uncacheable)          │
                      │                                     │
0xFFFF_FE00_0000_0000 ├─────────────────────────────────────┤
                      │ 内核栈区域 (per-CPU)                 │
                      │                                     │
                      │ ┌─────────────────────────────────┐ │
                      │ │ CPU 0:                          │ │
                      │ │   Normal Stack: 16 KB           │ │
                      │ │   IST #1 (DF): 4 KB             │ │
                      │ │   IST #2 (PF): 4 KB             │ │
                      │ │   IST #3 (MC): 4 KB             │ │
                      │ │ CPU 1: (同上)                   │ │
                      │ │ ...                             │ │
                      │ └─────────────────────────────────┘ │
                      │                                     │
0xFFFFFFFF_F000_0000 ├─────────────────────────────────────┤
                      │ 内核代码和数据                       │
                      │ (由 bootloader 加载)                │
                      │                                     │
                      │ ┌─────────────────────────────────┐ │
                      │ │ .text   (只读+可执行)           │ │
                      │ │ .rodata (只读)                  │ │
                      │ │ .data   (可读写)                │ │
                      │ │ .bss    (可读写, 零初始化)      │ │
                      │ └─────────────────────────────────┘ │
                      │                                     │
                      │ 映射属性:                           │
                      │   .text:   R-X, Global             │
                      │   .rodata: R--, Global             │
                      │   .data:   RW-, Global             │
                      │   .bss:    RW-, Global             │
                      │                                     │
0xFFFFFFFF_8000_0000 ├─────────────────────────────────────┤
                      │ (未使用)                            │
0xFFFF_FFFF_FFFF_FFFF └─────────────────────────────────────┘
```

---

## 5. 用户空间地址空间布局

### 5.1 标准用户进程地址空间

每个用户进程拥有独立的地址空间，布局如下：

```rust
/// 用户空间地址范围
pub const USER_VIRT_START: u64 = 0x0000_0000_0000_0000;
pub const USER_VIRT_END:   u64 = 0x0000_7FFF_FFFF_FFFF;

/// 用户空间关键地址
pub const USER_TEXT_START:    u64 = 0x0000_0001_0000_0000;  // 代码段起始
pub const USER_HEAP_START:    u64 = 0x0000_0002_0000_0000;  // 堆起始
pub const USER_STACK_TOP:     u64 = 0x0000_7FFF_FFFF_F000;  // 栈顶
pub const USER_STACK_SIZE:    u64 = 0x0000_0000_0080_0000;  // 栈大小 (8 MB)
pub const USER_MMAP_BASE:     u64 = 0x0000_1000_0000_0000;  // mmap 区域
pub const USER_SHM_BASE:      u64 = 0x0000_2000_0000_0000;  // 共享内存区域

/// 用户进程地址空间描述
pub struct UserAddressSpace {
    /// 页表根 (PML4) 物理地址
    pub pml4_phys: PhysAddr,
    /// 代码段范围
    pub text_range: Range<VirtAddr>,
    /// 堆范围
    pub heap_range: Range<VirtAddr>,
    /// 栈范围
    pub stack_range: Range<VirtAddr>,
    /// mmap 映射列表
    pub mmap_regions: SpinLock<Vec<MmapRegion>>,
    /// 共享内存映射列表
    pub shm_regions: SpinLock<Vec<SharedMemMapping>>,
}

/// mmap 映射区域
pub struct MmapRegion {
    /// 虚拟地址范围
    pub virt_range: Range<VirtAddr>,
    /// 映射的物理帧
    pub frames: Vec<PhysFrame>,
    /// 权限标志
    pub flags: PageTableFlags,
    /// 映射类型
    pub map_type: MmapType,
    /// 引用计数
    pub ref_count: AtomicUsize,
}

#[derive(Debug, Clone, Copy)]
pub enum MmapType {
    /// 私有匿名映射
    PrivateAnonymous,
    /// 共享匿名映射
    SharedAnonymous,
    /// 文件映射
    FileBacked { inode: u64, offset: u64 },
    /// IPC 共享内存
    IpcShared { shm_id: u64 },
}
```

### 5.2 用户空间布局图

```
用户进程虚拟地址空间

0x0000_0000_0000_0000 ┌─────────────────────────────────────┐
                      │ NULL 保护页                          │
                      │ (不可访问，捕获空指针)               │
                      │ 大小: 64 KB                         │
0x0000_0000_0001_0000 ├─────────────────────────────────────┤
                      │ 程序代码段 (.text)                   │
                      │ 权限: R-X (User)                    │
                      │ 大小: 可变                          │
                      ├─────────────────────────────────────┤
                      │ 只读数据 (.rodata)                   │
                      │ 权限: R-- (User)                    │
                      ├─────────────────────────────────────┤
                      │ 已初始化数据 (.data)                 │
                      │ 权限: RW- (User)                    │
                      ├─────────────────────────────────────┤
                      │ 未初始化数据 (.bss)                  │
                      │ 权限: RW- (User)                    │
0x0000_0002_0000_0000 ├─────────────────────────────────────┤
                      │ 堆 (Heap)                           │
                      │ 权限: RW- (User)                    │
                      │ 向上增长 (brk)                      │
                      │ 初始: 1 页, 按需扩展                 │
                      │ 最大: 由配额限制                     │
                      │                                     │
                      │         ... 空闲 ...                 │
                      │                                     │
0x0000_1000_0000_0000 ├─────────────────────────────────────┤
                      │ mmap 区域                            │
                      │ 动态库映射、文件映射等               │
                      │ 向上增长                            │
                      │                                     │
                      │         ... 空闲 ...                 │
                      │                                     │
0x0000_2000_0000_0000 ├─────────────────────────────────────┤
                      │ 共享内存区域 (IPC)                   │
                      │ Agent 间共享数据                     │
                      │ 权限: 可配置                        │
                      │                                     │
                      │         ... 空闲 ...                 │
                      │                                     │
0x0000_7FFF_F7F0_0000 ├─────────────────────────────────────┤
                      │ 栈 (Stack)                          │
                      │ 权限: RW- (User)                    │
                      │ 向下增长                            │
                      │ 大小: 8 MB (默认)                   │
                      │ 栈保护页 (guard page)               │
0x0000_7FFF_FFFF_FFFF └─────────────────────────────────────┘
```

---

## 6. Agent 隔离机制

### 6.1 隔离模型

每个 Agent 运行在独立的地址空间中，通过 4 级页表实现硬件级隔离：

```
Agent A 地址空间              Agent B 地址空间
┌────────────────────┐       ┌────────────────────┐
│ Agent A 代码/数据   │       │ Agent B 代码/数据   │
│ (私有)             │       │ (私有)             │
├────────────────────┤       ├────────────────────┤
│ Agent A 堆         │       │ Agent B 堆         │
│ (私有)             │       │ (私有)             │
├────────────────────┤       ├────────────────────┤
│                    │       │                    │
│  ┌──────────────┐  │       │  ┌──────────────┐  │
│  │ 共享内存区域  │◄─┼───────┼─►│ 共享内存区域  │  │
│  │ (可控共享)    │  │       │  │ (可控共享)    │  │
│  └──────────────┘  │       │  └──────────────┘  │
│                    │       │                    │
├────────────────────┤       ├────────────────────┤
│ Agent A 栈         │       │ Agent B 栈         │
│ (私有)             │       │ (私有)             │
└────────────────────┘       └────────────────────┘

隔离规则:
1. Agent A 无法访问 Agent B 的私有内存
2. 共享内存区域需要双方显式同意
3. 内核验证所有跨 Agent 的内存操作
4. Capability 控制共享权限
```

### 6.2 Agent 地址空间创建

```rust
/// 创建 Agent 地址空间
pub fn create_agent_address_space(
    agent_id: AgentId,
    config: &AgentMemoryConfig,
) -> Result<UserAddressSpace, MemoryError> {
    // 1. 分配新的 PML4 页 (页表根)
    let pml4_frame = FRAME_ALLOCATOR.allocate_frame()
        .ok_or(MemoryError::OutOfFrames)?;
    let pml4_virt = VirtAddr::new(pml4_frame.start_address().as_u64() + PHYS_OFFSET);

    // 2. 初始化 PML4 (全部清零，表示未映射)
    unsafe {
        core::ptr::write_bytes(pml4_virt.as_mut_ptr(), 0, PAGE_SIZE);
    }

    // 3. 复制内核页表映射到高半部分
    copy_kernel_mappings(pml4_virt);

    // 4. 映射 Agent 代码段
    map_agent_text(pml4_virt, &config.text)?;

    // 5. 映射 Agent 数据段
    map_agent_data(pml4_virt, &config.data)?;

    // 6. 设置 Agent 栈
    map_agent_stack(pml4_virt, config.stack_size)?;

    // 7. 设置 Agent 堆 (初始 1 页)
    map_agent_heap(pml4_virt)?;

    // 8. 设置 Agent 上下文区域
    map_agent_context(pml4_virt, agent_id)?;

    // 9. 设置栈保护页 (guard page)
    map_guard_page(pml4_virt, config.stack_size)?;

    // 10. 设置 NX 位 (数据区域不可执行)
    set_nx_for_data_regions(pml4_virt);

    Ok(UserAddressSpace {
        pml4_phys: pml4_frame.start_address(),
        ..Default::default()
    })
}

/// Agent 内存配置
pub struct AgentMemoryConfig {
    /// 代码段 (ELF 加载信息)
    pub text: SegmentInfo,
    /// 数据段
    pub data: SegmentInfo,
    /// 栈大小 (字节)
    pub stack_size: u64,
    /// 堆大小限制 (字节)
    pub heap_limit: u64,
    /// 内存配额 (字节)
    pub memory_quota: u64,
    /// 初始共享内存区域
    pub initial_shm: Vec<SharedMemConfig>,
}
```

### 6.3 Agent 内存配额管理

```rust
/// Agent 内存配额
pub struct MemoryQuota {
    /// 总内存配额 (字节)
    pub total_quota: u64,
    /// 当前已使用 (字节)
    pub current_usage: AtomicU64,
    /// 堆配额 (字节)
    pub heap_quota: u64,
    /// 栈配额 (字节)
    pub stack_quota: u64,
    /// 共享内存配额 (字节)
    pub shm_quota: u64,
    /// mmap 配额 (字节)
    pub mmap_quota: u64,
}

impl MemoryQuota {
    /// 请求内存分配
    pub fn allocate(&self, size: u64, region: MemoryRegion) -> Result<(), MemoryError> {
        let new_usage = self.current_usage.fetch_add(size, Ordering::Relaxed) + size;
        if new_usage > self.total_quota {
            self.current_usage.fetch_sub(size, Ordering::Relaxed);
            return Err(MemoryError::QuotaExceeded {
                requested: size,
                quota: self.total_quota,
                current: new_usage - size,
            });
        }

        // 检查区域特定配额
        match region {
            MemoryRegion::Heap => {
                if self.heap_quota_exceeded(size) {
                    self.current_usage.fetch_sub(size, Ordering::Relaxed);
                    return Err(MemoryError::HeapQuotaExceeded);
                }
            }
            MemoryRegion::SharedMem => {
                if self.shm_quota_exceeded(size) {
                    self.current_usage.fetch_sub(size, Ordering::Relaxed);
                    return Err(MemoryError::ShmQuotaExceeded);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 释放内存
    pub fn deallocate(&self, size: u64) {
        self.current_usage.fetch_sub(size, Ordering::Relaxed);
    }
}
```

---

## 7. 共享内存区域

### 7.1 IPC 零拷贝共享内存

```rust
/// 共享内存区域管理器
pub struct SharedMemoryManager {
    /// 所有共享内存区域
    regions: RwLock<HashMap<SharedMemId, SharedMemoryRegion>>,
    /// 下一个可用 ID
    next_id: AtomicU64,
}

/// 共享内存区域
pub struct SharedMemoryRegion {
    /// 区域 ID
    pub id: SharedMemId,
    /// 物理帧列表
    pub frames: Vec<PhysFrame>,
    /// 区域大小
    pub size: usize,
    /// 创建者
    pub creator: ProcessId,
    /// 参与者列表及权限
    pub participants: RwLock<HashMap<ProcessId, SharedMemPermissions>>,
    /// 引用计数
    pub ref_count: AtomicUsize,
    /// 创建时间
    pub created_at: u64,
    /// 是否允许新增参与者
    pub allow_new_participants: bool,
}

/// 创建共享内存区域
pub fn create_shared_memory(
    creator: ProcessId,
    size: usize,
    initial_participants: Vec<(ProcessId, SharedMemPermissions)>,
) -> Result<SharedMemHandle, MemoryError> {
    // 1. 计算页帧数
    let num_frames = (size + PAGE_SIZE - 1) / PAGE_SIZE;

    // 2. 分配物理帧
    let frames = allocate_contiguous_frames(num_frames)?;

    // 3. 创建共享内存区域
    let region = SharedMemoryRegion {
        id: SharedMemId::new(),
        frames: frames.clone(),
        size,
        creator,
        participants: RwLock::new(
            initial_participants.into_iter().collect()
        ),
        ref_count: AtomicUsize::new(1),
        created_at: current_timestamp(),
        allow_new_participants: true,
    };

    // 4. 在所有参与者的地址空间中映射
    for (pid, perms) in &initial_participants {
        let vaddr = map_shared_memory_to_process(
            pid,
            &frames,
            *perms,
        )?;
        region.participants.write().get_mut(pid).unwrap().vaddr = Some(vaddr);
    }

    // 5. 注册到管理器
    let manager = SHM_MANAGER.get().unwrap();
    manager.regions.write().insert(region.id, region);

    Ok(SharedMemHandle { id: region.id, size })
}

/// 将共享内存映射到进程地址空间
pub fn map_shared_memory_to_process(
    process: ProcessId,
    frames: &[PhysFrame],
    permissions: SharedMemPermissions,
) -> Result<VirtAddr, MemoryError> {
    // 1. 获取进程页表
    let pml4 = get_process_pml4(process)?;

    // 2. 在进程的共享内存区域中分配虚拟地址空间
    let vaddr = allocate_shm_vaddr_range(frames.len() * PAGE_SIZE, process)?;

    // 3. 逐页映射
    for (i, frame) in frames.iter().enumerate() {
        let page = Page::from_start_address(
            vaddr + i * PAGE_SIZE
        ).unwrap();

        let flags = page_table_flags_from_permissions(permissions);
        map_page_to_table(pml4, page, *frame, flags)?;
    }

    // 4. 刷新 TLB
    tlb::flush_all();

    Ok(vaddr)
}
```

### 7.2 共享内存生命周期

```
创建共享内存
    │
    ▼
┌──────────────┐
│   CREATED    │  物理帧已分配，无映射
└──────┬───────┘
       │ 映射到第一个进程
       ▼
┌──────────────┐
│   MAPPED     │  至少一个进程有映射
└──────┬───────┘
       │ 添加更多参与者
       ▼
┌──────────────┐
│   SHARED     │  多个进程有映射
└──────┬───────┘
       │ 参与者逐步解除映射
       ▼
┌──────────────┐
│  LAST_REF    │  仅剩一个引用
└──────┬───────┘
       │ 最后一个引用解除
       ▼
┌──────────────┐
│  DESTROYED   │  物理帧回收
└──────────────┘
```

---

## 8. 内存分配策略

### 8.1 分配器层次结构

```
内存分配层次:

启动阶段 (boot):
  ┌─────────────────────────────────────────┐
  │  Bump Allocator (bumpalo)               │
  │  - 仅分配，不释放                        │
  │  - O(1) 分配速度                        │
  │  - 用于内核早期初始化                    │
  │  - 大小: 初始 4 MB                      │
  └─────────────────────────────────────────┘
            │
            │ 内核初始化完成后切换
            ▼
运行阶段 (runtime):
  ┌─────────────────────────────────────────┐
  │  Slab Allocator (内核对象)              │
  │  - 按对象大小分类缓存                   │
  │  - 快速分配/释放                        │
  │  - 用于: 进程控制块、线程控制块、        │
  │    页表、IPC 消息等固定大小对象          │
  └─────────────────────────────────────────┘
  ┌─────────────────────────────────────────┐
  │  Buddy Allocator (页级分配)             │
  │  - 2^n 页分配                           │
  │  - 减少外部碎片                         │
  │  - 用于: 页表分配、大块内核缓冲区        │
  └─────────────────────────────────────────┘
  ┌─────────────────────────────────────────┐
  │  用户空间分配器 (每个进程独立)           │
  │  - 基于 brk/sbrk 的堆管理              │
  │  - 基于 mmap 的大块分配                │
  │  - Agent 运行时可自定义分配策略         │
  └─────────────────────────────────────────┘
```

### 8.2 Slab 分配器

```rust
/// Slab 分配器
pub struct SlabAllocator {
    /// 不同大小类别的 slab 缓存
    caches: [SlabCache; NUM_SLAB_CACHES],
}

/// Slab 缓存 (单一大小类别)
pub struct SlabCache {
    /// 对象大小
    pub object_size: usize,
    /// 空闲对象链表
    pub free_list: SpinLock<SlabFreeList>,
    /// 已分配的 slab 页列表
    pub slabs: SpinLock<Vec<SlabPage>>,
    /// 每个 slab 可容纳的对象数
    pub objects_per_slab: usize,
    /// 统计信息
    pub stats: SlabCacheStats,
}

/// Slab 大小类别
pub const SLAB_SIZE_CLASSES: [usize; 12] = [
    32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
];

/// Slab 缓存统计
pub struct SlabCacheStats {
    /// 已分配对象数
    pub allocated: AtomicUsize,
    /// 空闲对象数
    pub free: AtomicUsize,
    /// 总 slab 页数
    pub slab_pages: AtomicUsize,
    /// 分配次数
    pub alloc_count: AtomicU64,
    /// 释放次数
    pub free_count: AtomicU64,
}

impl SlabCache {
    /// 从 slab 缓存分配对象
    pub fn allocate(&self) -> Option<*mut u8> {
        // 1. 尝试从空闲链表获取
        if let Some(ptr) = self.free_list.lock().pop() {
            self.stats.allocated.fetch_add(1, Ordering::Relaxed);
            self.stats.free.fetch_sub(1, Ordering::Relaxed);
            self.stats.alloc_count.fetch_add(1, Ordering::Relaxed);
            return Some(ptr);
        }

        // 2. 空闲链表为空，分配新 slab 页
        let slab_page = self.grow()?;
        let ptr = slab_page.allocate_object()?;
        self.stats.allocated.fetch_add(1, Ordering::Relaxed);
        self.stats.alloc_count.fetch_add(1, Ordering::Relaxed);
        Some(ptr)
    }

    /// 释放对象回 slab 缓存
    pub fn deallocate(&self, ptr: *mut u8) {
        self.free_list.lock().push(ptr);
        self.stats.allocated.fetch_sub(1, Ordering::Relaxed);
        self.stats.free.fetch_add(1, Ordering::Relaxed);
        self.stats.free_count.fetch_add(1, Ordering::Relaxed);
    }
}
```

### 8.3 Buddy 分配器

```rust
/// Buddy 分配器 (页级)
pub struct BuddyAllocator {
    /// 各阶空闲链表 (order 0 ~ MAX_ORDER)
    free_lists: [SpinLock<Vec<PhysFrame>>; MAX_ORDER + 1],
    /// 页帧使用状态位图
    page_bitmap: SpinLock<PageBitmap>,
}

const MAX_ORDER: usize = 10;  // 最大 2^10 = 1024 页 = 4 MB

impl BuddyAllocator {
    /// 分配 2^order 个连续页
    pub fn allocate_pages(&self, order: usize) -> Option<PhysFrame> {
        // 1. 从 order 开始向上查找空闲块
        for current_order in order..=MAX_ORDER {
            let mut list = self.free_lists[current_order].lock();
            if let Some(frame) = list.pop() {
                // 2. 如果找到的块比需要的大，分裂
                for split_order in (order..current_order).rev() {
                    let buddy = frame.start_address()
                        + (1 << split_order) * PAGE_SIZE;
                    self.free_lists[split_order].lock()
                        .push(PhysFrame::containing_address(buddy));
                }
                return Some(frame);
            }
        }
        None
    }

    /// 释放 2^order 个连续页
    pub fn free_pages(&self, frame: PhysFrame, order: usize) {
        // 1. 尝试与 buddy 合并
        let mut current_frame = frame;
        let mut current_order = order;

        while current_order < MAX_ORDER {
            let buddy_addr = current_frame.start_address().as_u64()
                ^ (1 << current_order) * PAGE_SIZE;
            let buddy = PhysFrame::containing_address(
                PhysAddr::new(buddy_addr)
            );

            // 2. 检查 buddy 是否空闲
            if self.is_buddy_free(buddy, current_order) {
                // 3. 从空闲链表移除 buddy
                self.remove_from_free_list(buddy, current_order);
                // 4. 合并
                current_frame = core::cmp::min(
                    current_frame, buddy,
                    |a, b| a.start_address().as_u64().cmp(&b.start_address().as_u64())
                );
                current_order += 1;
            } else {
                break;
            }
        }

        // 5. 将合并后的块放入对应阶的空闲链表
        self.free_lists[current_order].lock().push(current_frame);
    }
}
```

---

## 9. 按需分页与缺页处理

### 9.1 缺页异常处理流程

```
缺页异常触发 (#PF, 向量号 14)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│ 1. 读取 CR2 寄存器 (触发缺页的虚拟地址)                       │
│ 2. 读取错误码 (P=0/1, W/R, U/S, RSVD, I/D)                  │
└──────┬───────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│ 3. 判断缺页类型                                              │
│    ┌────────────────────────────────────────────────────┐   │
│    │ 错误码格式:                                         │   │
│    │ Bit 0 (P): 0=页不存在, 1=权限违规                   │   │
│    │ Bit 1 (W/R): 0=读, 1=写                            │   │
│    │ Bit 2 (U/S): 0=内核, 1=用户                         │   │
│    │ Bit 3 (RSVD): 保留位被设置 (页表条目错误)           │   │
│    │ Bit 4 (I/D): 0=数据访问, 1=指令取                   │   │
│    └────────────────────────────────────────────────────┘   │
└──────┬───────────────────────────────────────────────────────┘
       │
       ├── P=0 (页不存在) ──────────────────────┐
       │                                        ▼
       │                          ┌──────────────────────────┐
       │                          │ 4a. 按需分页              │
       │                          │ - 分配物理帧             │
       │                          │ - 填充页内容             │
       │                          │   (零页/文件页/COW)      │
       │                          │ - 更新页表               │
       │                          │ - 刷新 TLB               │
       │                          │ - 重新执行触发指令       │
       │                          └──────────────────────────┘
       │
       ├── P=1, W=1 (写时复制) ────────────────┐
       │                                        ▼
       │                          ┌──────────────────────────┐
       │                          │ 4b. Copy-on-Write        │
       │                          │ - 分配新物理帧           │
       │                          │ - 复制原页内容           │
       │                          │ - 更新页表为可写         │
       │                          │ - 减少原页引用计数       │
       │                          │ - 重新执行触发指令       │
       │                          └──────────────────────────┘
       │
       ├── P=1, W=0, U=0 (内核读权限违规) ───┐
       │                                        ▼
       │                          ┌──────────────────────────┐
       │                          │ 4c. 内核 Oops            │
       │                          │ - 记录错误               │
       │                          │ - 杀死当前进程           │
       │                          │ (SMAP 违规)              │
       │                          └──────────────────────────┘
       │
       ├── P=1, U=1 (用户态权限违规) ─────────┐
       │                                        ▼
       │                          ┌──────────────────────────┐
       │                          │ 4d. SIGSEGV              │
       │                          │ - 向用户进程发送信号     │
       │                          │ - 默认行为: 终止进程     │
       │                          └──────────────────────────┘
       │
       └── RSVD=1 (保留位错误) ───────────────┐
                                                ▼
                                  ┌──────────────────────────┐
                                  │ 4e. 内核 Oops            │
                                  │ - 页表条目格式错误       │
                                  │ - 可能是内核 Bug         │
                                  └──────────────────────────┘
```

### 9.2 缺页处理实现

```rust
/// 缺页异常处理函数
pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_vaddr = read_cr2();

    // 验证错误地址在有效范围内
    if !is_valid_virt_addr(fault_vaddr) {
        handle_oops(OopsSeverity::Major, stack_frame);
        return;
    }

    let current_process = scheduler::current_process();

    match error_code {
        // 页不存在 (按需分页)
        pf if !pf.is_present() => {
            handle_demand_paging(current_process, fault_vaddr, error_code);
        }

        // 写时复制
        pf if pf.is_present() && pf.is_write() && !pf.is_from_user() => {
            handle_cow_fault(current_process, fault_vaddr);
        }

        // 栈增长
        pf if is_stack_growth(current_process, fault_vaddr) => {
            handle_stack_growth(current_process, fault_vaddr);
        }

        // 权限违规
        _ => {
            // 用户态权限违规 → 发送 SIGSEGV
            if error_code.is_from_user() {
                send_signal(current_process, Signal::SIGSEGV);
            } else {
                // 内核态权限违规 → Oops
                handle_oops(OopsSeverity::Major, stack_frame);
            }
        }
    }
}

/// 按需分页处理
fn handle_demand_paging(
    process: ProcessId,
    vaddr: VirtAddr,
    error_code: PageFaultErrorCode,
) {
    // 1. 检查虚拟地址是否在进程的合法范围内
    let addr_space = get_address_space(process);
    if !addr_space.is_valid_range(vaddr) {
        send_signal(process, Signal::SIGSEGV);
        return;
    }

    // 2. 分配物理帧
    let frame = match FRAME_ALLOCATOR.allocate_frame() {
        Some(f) => f,
        None => {
            // 内存不足，尝试回收
            if !reclaim_memory() {
                // OOM，杀死进程
                kill_process(process, KillReason::OutOfMemory);
                return;
            }
            FRAME_ALLOCATOR.allocate_frame().unwrap()
        }
    };

    // 3. 确定页内容来源
    let page_content = match addr_space.get_backing_store(vaddr) {
        BackingStore::Anonymous => PageContent::Zero,
        BackingStore::File { file, offset } => {
            PageContent::File { file, offset }
        }
        BackingStore::SharedMemory { shm_id, offset } => {
            PageContent::Shared { shm_id, offset }
        }
    };

    // 4. 填充页内容
    fill_page_content(frame, &page_content);

    // 5. 映射到进程地址空间
    let flags = calculate_page_flags(&page_content, error_code);
    map_page_to_process(process, vaddr, frame, flags);

    // 6. 刷新 TLB
    tlb::flush(vaddr);
}
```

---

## 10. 内存保护

### 10.1 保护机制总览

| 保护机制 | 硬件支持 | 说明 |
|---------|---------|------|
| **NX 位** | XD (Execute Disable) | 数据页不可执行，防止代码注入 |
| **Supervisor/User 位** | U/S | 内核页用户态不可访问 |
| **写保护** | WP (CR0 bit 16) | 内核态只读页写入触发 #PF |
| **SMAP** | CR4 bit 21 | 内核态禁止访问用户态数据 |
| **SMEP** | CR4 bit 20 | 内核态禁止执行用户态代码 |
| **PXE** | CR4 bit 14 | 保护模式异常 |
| **PCID** | CR4 bit 17 | 进程上下文 ID，优化 TLB |
| **PKRU** | XSAVE PKRU | 保护密钥，用户态页保护 |
| **MCE** | Machine Check | 内存错误检测 |

### 10.2 保护位设置策略

```rust
/// 各内存区域的保护位设置

/// 内核代码段: R-X, Supervisor, Global
const KERNEL_CODE_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::GLOBAL;

/// 内核只读数据: R--, Supervisor, Global
const KERNEL_RO_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::GLOBAL;

/// 内核可读写数据: RW-, Supervisor, Global
const KERNEL_RW_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::GLOBAL;

/// 用户代码段: R-X, User
const USER_CODE_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::USER_ACCESSIBLE;

/// 用户只读数据: R--, User
const USER_RO_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::USER_ACCESSIBLE;

/// 用户可读写数据: RW-, User
const USER_RW_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::USER_ACCESSIBLE;

/// 共享内存 (只读): R--, User
const SHM_RO_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::USER_ACCESSIBLE;

/// 共享内存 (读写): RW-, User
const SHM_RW_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::USER_ACCESSIBLE;

/// 设备 MMIO: RW-, Supervisor, Uncacheable
const MMIO_FLAGS: PageTableFlags = PageTableFlags::PRESENT
    | PageTableFlags::WRITABLE
    | PageTableFlags::NO_CACHE
    | PageTableFlags::WRITE_THROUGH;
```

### 10.3 SMAP/SMEP 配置

```rust
/// 启用所有内存保护特性
pub fn enable_memory_protections() {
    use x86_64::registers::control::{Cr0, Cr4};

    // 启用 Write Protect (CR0.WP = 1)
    // 内核态写入只读页会触发 #PF
    unsafe {
        Cr0::write(
            Cr0::read() | Cr0::WRITE_PROTECT
        );
    }

    // 启用 SMEP (CR4.SMEP = 1)
    // 内核态执行用户态代码会触发 #PF
    unsafe {
        Cr4::write(
            Cr4::read() | Cr4::SMEP_ENABLE
        );
    }

    // 启用 SMAP (CR4.SMAP = 1)
    // 内核态访问用户态数据会触发 #PF
    // (使用 stac/clac 指令临时允许)
    unsafe {
        Cr4::write(
            Cr4::read() | Cr4::SMAP_ENABLE
        );
    }

    // 启用 PCID (CR4.PCIDE = 1)
    // 优化 TLB 刷新
    unsafe {
        Cr4::write(
            Cr4::read() | Cr4::PCID_ENABLE
        );
    }

    // 启用全局页 (CR4.PGE = 1)
    // 内核页在 CR3 切换时保留在 TLB 中
    unsafe {
        Cr4::write(
            Cr4::read() | Cr4::PAGE_GLOBAL_ENABLE
        );
    }
}

/// 临时允许内核访问用户态数据 (SMAP 绕过)
#[inline(always)]
pub unsafe fn allow_user_access() {
    core::arch::asm!("stac");
}

/// 恢复禁止内核访问用户态数据
#[inline(always)]
pub unsafe fn disallow_user_access() {
    core::arch::asm!("clac");
}
```

---

## 11. NUMA 感知 (未来考虑)

### 11.1 NUMA 架构概述

```
NUMA 系统拓扑:

┌─────────────────────────────────────────────────────────────┐
│                    Node 0                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────────┐  │
│  │ CPU 0    │  │ CPU 1    │  │ 本地内存 (Node 0)        │  │
│  │ (Core)   │  │ (Core)   │  │ 大小: 8 GB               │  │
│  └────┬─────┘  └────┬─────┘  │ 延迟: ~80ns              │  │
│       │              │        └──────────────────────────┘  │
│       └──────┬───────┘                                     │
│              │ QPI / Infinity Fabric                        │
├──────────────┼──────────────────────────────────────────────┤
│              │                                              │
│       ┌──────┴───────┐                                     │
│       │              │        ┌──────────────────────────┐  │
│  ┌────▼─────┐  ┌─────▼────┐  │ 本地内存 (Node 1)        │  │
│  │ CPU 2    │  │ CPU 3    │  │ 大小: 8 GB               │  │
│  │ (Core)   │  │ (Core)   │  │ 延迟: ~80ns              │  │
│  └──────────┘  └──────────┘  │ 跨节点延迟: ~120ns        │  │
│                               └──────────────────────────┘  │
│                    Node 1                                    │
└─────────────────────────────────────────────────────────────┘

内存访问延迟:
  本地内存访问:   ~80 ns
  远程内存访问:   ~120 ns (1.5x 慢)
  跨 2 跳访问:    ~160 ns (2x 慢)
```

### 11.2 NUMA 感知策略 (规划)

```rust
/// NUMA 策略 (未来实现)
pub enum NumaPolicy {
    /// 默认: 优先分配当前 CPU 所在节点的内存
    Local,
    /// 交叉分配: 跨节点交错分配 (增加带宽)
    Interleave,
    /// 绑定: 仅从指定节点分配
    Bind { node_id: u32 },
    /// 首选: 优先从指定节点分配
    Preferred { node_id: u32 },
}

/// NUMA 感知的帧分配 (未来实现)
pub fn allocate_frame_numa(policy: NumaPolicy) -> Option<PhysFrame> {
    match policy {
        NumaPolicy::Local => {
            let node = current_numa_node();
            numa_node_allocator(node).allocate_frame()
        }
        NumaPolicy::Interleave => {
            // 轮询各节点分配
            let node = next_interleave_node();
            numa_node_allocator(node).allocate_frame()
        }
        NumaPolicy::Bind { node_id } => {
            numa_node_allocator(node_id).allocate_frame()
        }
        NumaPolicy::Preferred { node_id } => {
            numa_node_allocator(node_id).allocate_frame()
                .or_else(|| fallback_allocate_frame())
        }
    }
}
```

---

## 12. 虚拟化内存：EPT/NPT

### 12.1 扩展页表 (EPT) 概述

当 OmniAgent OS 作为 Hypervisor 运行时，使用 EPT (Intel) 或 NPT (AMD) 管理 Guest 的物理地址到 Host 物理地址的转换：

```
地址转换层次:

非虚拟化:
  虚拟地址 (VA) ──页表──► 物理地址 (PA)

虚拟化:
  Guest 虚拟地址 (GVA) ──Guest 页表──► Guest 物理地址 (GPA)
                                              │
                                    ┌─────────┘
                                    │ EPT/NPT
                                    ▼
                              Host 物理地址 (HPA)

EPT 页表层次 (Intel VT-x):
  GVA ──Guest CR3──► GPA ──EPT──► HPA

  ┌──────────────────────────────────────────────────┐
  │ EPT PML4 (EPTP 指向)                              │
  │   └─ EPT PDPT                                     │
  │       └─ EPT PD                                   │
  │           └─ EPT PT ──► HPA                       │
  └──────────────────────────────────────────────────┘

NPT 页表层次 (AMD-V):
  与 EPT 类似，使用 VM_HSAVE_PA_AREA 指定
```

### 12.2 EPT 管理

```rust
/// EPT (Extended Page Tables) 管理器
pub struct EptManager {
    /// 每个 VM 的 EPT 根
    ept_roots: RwLock<HashMap<VmId, EptRoot>>,
}

/// EPT 根结构
pub struct EptRoot {
    /// EPT PML4 物理地址 (写入 EPTP MSR)
    pub eptp: PhysAddr,
    /// EPT 页表层级
    pub tables: EptTables,
    /// EPT 映射统计
    pub stats: EptStats,
}

/// EPT 页表
pub struct EptTables {
    /// EPT PML4
    pub pml4: PhysFrame,
    /// EPT PDPT
    pub pdpt: Vec<PhysFrame>,
    /// EPT PD
    pub pd: Vec<PhysFrame>,
    /// EPT PT
    pub pt: Vec<PhysFrame>,
}

/// 创建 VM 的 EPT
pub fn create_vm_ept(vm_id: VmId, guest_ram_size: u64) -> Result<EptRoot, VirtError> {
    // 1. 分配 EPT PML4
    let ept_pml4 = FRAME_ALLOCATOR.allocate_frame()
        .ok_or(VirtError::OutOfMemory)?;

    // 2. 初始化 EPT PML4 (全零)
    zero_frame(ept_pml4);

    // 3. 为 Guest RAM 建立恒等映射
    //    GPA 0 ~ guest_ram_size → HPA (分配的物理帧)
    let num_pages = (guest_ram_size + PAGE_SIZE - 1) / PAGE_SIZE;
    for i in 0..num_pages {
        let gpa = i * PAGE_SIZE;
        let hpa_frame = FRAME_ALLOCATOR.allocate_frame()
            .ok_or(VirtError::OutOfMemory)?;

        // 使用 2MB 大页 (如果对齐)
        if gpa % SIZE_2MB == 0 && i + 512 <= num_pages {
            map_ept_large_page(ept_pml4, gpa, hpa_frame, EPT_FLAGS_READ | EPT_FLAGS_WRITE | EPT_FLAGS_EXEC)?;
            i += 511; // 跳过已映射的页
        } else {
            map_ept_page(ept_pml4, gpa, hpa_frame, EPT_FLAGS_READ | EPT_FLAGS_WRITE | EPT_FLAGS_EXEC)?;
        }
    }

    // 4. 映射 MMIO 区域 (passthrough)
    map_ept_mmio_regions(ept_pml4)?;

    let ept_root = EptRoot {
        eptp: ept_pml4.start_address(),
        tables: EptTables { pml4: ept_pml4, .. },
        stats: EptStats::default(),
    };

    Ok(ept_root)
}

/// EPT 违例处理
pub fn handle_ept_violation(vm: &mut Vm, exit_info: &VmExitInfo) -> VmExitAction {
    let gpa = exit_info.guest_physical_address;
    let fault_code = exit_info.qualification;

    match fault_code {
        // 读违例
        EPT_READ_VIOLATION => {
            // 检查是否需要映射新的 GPA → HPA
            if let Some(hpa) = vm.resolve_gpa(gpa) {
                map_ept_page(vm.ept_root.eptp, gpa, hpa, EPT_FLAGS_READ);
                VmExitAction::Resume
            } else {
                // 未映射的 GPA，注入 #PF 到 Guest
                inject_page_fault(vm, gpa, PF_ERROR_READ);
                VmExitAction::Resume
            }
        }

        // 写违例 (可能需要 COW)
        EPT_WRITE_VIOLATION => {
            if vm.is_cow_page(gpa) {
                let new_hpa = FRAME_ALLOCATOR.allocate_frame().unwrap();
                copy_frame(vm.resolve_gpa(gpa).unwrap(), new_hpa);
                map_ept_page(vm.ept_root.eptp, gpa, new_hpa, EPT_FLAGS_READ | EPT_FLAGS_WRITE);
                VmExitAction::Resume
            } else {
                inject_page_fault(vm, gpa, PF_ERROR_WRITE);
                VmExitAction::Resume
            }
        }

        // 执行违例
        EPT_EXEC_VIOLATION => {
            // NX 位阻止执行
            inject_page_fault(vm, gpa, PF_ERROR_EXECUTE);
            VmExitAction::Resume
        }

        _ => VmExitAction::Shutdown,
    }
}
```

### 12.3 EPT/NPT 性能优化

| 优化技术 | 描述 | 收益 |
|---------|------|------|
| **EPT 大页** | 使用 2MB/1GB 大页减少 EPT 层级 | 减少 VM Exit 次数 |
| **VPID** | Virtual Processor ID，避免 TLB 全刷 | 减少 VM Entry/Exit 延迟 |
| **EPT A/D 位** | 硬件维护访问/脏位 | 减少软件跟踪开销 |
| **EPT 缓存** | 预映射常用 GPA 范围 | 减少 EPT Violation |
| **内存气球** | 动态调整 Guest 内存 | 提高内存利用率 |

---

## 13. 内存错误处理

### 13.1 错误码定义

```rust
/// 内存管理错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// 物理帧耗尽
    OutOfFrames,
    /// 虚拟地址空间不足
    OutOfVirtualSpace,
    /// 地址未映射
    NotMapped,
    /// 地址已映射
    AlreadyMapped,
    /// 权限不足
    PermissionDenied,
    /// 地址未对齐
    AlignmentError,
    /// 内存配额超限
    QuotaExceeded {
        requested: u64,
        quota: u64,
        current: u64,
    },
    /// 堆配额超限
    HeapQuotaExceeded,
    /// 共享内存配额超限
    ShmQuotaExceeded,
    /// 无效物理地址
    InvalidPhysicalAddress,
    /// 无效虚拟地址
    InvalidVirtualAddress,
    /// 页表条目错误
    PageTableEntryError,
    /// TLB 刷新失败
    TlbFlushError,
}

impl MemoryError {
    /// 转换为 POSIX errno
    pub fn to_errno(&self) -> i32 {
        match self {
            MemoryError::OutOfFrames => 12,          // ENOMEM
            MemoryError::OutOfVirtualSpace => 12,    // ENOMEM
            MemoryError::NotMapped => 14,            // EFAULT
            MemoryError::AlreadyMapped => 17,        // EEXIST
            MemoryError::PermissionDenied => 13,     // EACCES
            MemoryError::AlignmentError => 22,       // EINVAL
            MemoryError::QuotaExceeded { .. } => 55, // ENOBUFS
            MemoryError::InvalidPhysicalAddress => 22, // EINVAL
            MemoryError::InvalidVirtualAddress => 14,  // EFAULT
            _ => 5,                                  // EIO
        }
    }
}
```

### 13.2 OOM (Out of Memory) 处理

```rust
/// 内存回收策略
pub fn reclaim_memory() -> bool {
    let mut reclaimed = 0;

    // 1. 回收 slab 缓存中的空闲 slab 页
    reclaimed += reclaim_slab_caches();

    // 2. 回收文件缓存页
    reclaimed += reclaim_file_cache();

    // 3. 回收未使用的共享内存区域
    reclaimed += reclaim_unused_shm();

    // 4. 压缩页表 (合并空闲大页)
    reclaimed += compact_page_tables();

    // 5. 如果仍然不足，杀死低优先级 Agent
    if reclaimed < MIN_RECLAIM_TARGET {
        reclaimed += kill_low_priority_agents();
    }

    reclaimed >= MIN_RECLAIM_TARGET
}

/// OOM Killer
fn kill_low_priority_agents() -> usize {
    let agents = get_all_agents();
    let mut candidates: Vec<_> = agents.iter()
        .filter(|a| a.priority_class == AgentPriorityClass::AGENT_BATCH)
        .collect();
    candidates.sort_by_key(|a| a.memory_usage);
    candidates.reverse(); // 内存占用最大的优先

    let mut reclaimed = 0;
    for agent in candidates {
        reclaimed += agent.memory_usage as usize;
        kill_agent(agent.id, KillReason::OutOfMemory);
        if reclaimed >= MIN_RECLAIM_TARGET {
            break;
        }
    }
    reclaimed
}
```

---

## 14. 测试用例

### 14.1 功能测试

```rust
#[cfg(test)]
mod memory_tests {
    use super::*;

    /// 测试: 帧分配与释放
    #[test]
    fn test_frame_alloc_dealloc() {
        let frame = FRAME_ALLOCATOR.allocate_frame().unwrap();
        let addr = frame.start_address();
        assert!(addr.as_u64() % PAGE_SIZE == 0);
        FRAME_ALLOCATOR.deallocate_frame(frame);
    }

    /// 测试: 连续帧分配
    #[test]
    fn test_contiguous_alloc() {
        let frames = allocate_contiguous_frames(16).unwrap();
        for i in 1..frames.len() {
            let expected = frames[i-1].start_address().as_u64() + PAGE_SIZE;
            assert_eq!(frames[i].start_address().as_u64(), expected);
        }
    }

    /// 测试: 页表映射
    #[test]
    fn test_page_mapping() {
        let frame = FRAME_ALLOCATOR.allocate_frame().unwrap();
        let page = Page::from_start_address(VirtAddr::new(0x1000_0000)).unwrap();
        map_page(page, frame, FLAGS_USER_DATA).unwrap();
        assert!(is_page_mapped(page));
        unmap_page(page).unwrap();
        assert!(!is_page_mapped(page));
    }

    /// 测试: Agent 地址空间隔离
    #[test]
    fn test_agent_isolation() {
        let agent_a = create_test_agent();
        let agent_b = create_test_agent();

        // Agent A 写入私有内存
        let addr_a = agent_a.private_data_addr();
        unsafe { *(addr_a.as_mut_ptr() as *mut u64) = 0xAAAA; }

        // Agent B 的相同虚拟地址应不可见
        let addr_b = agent_b.private_data_addr();
        let value_b = unsafe { *(addr_b.as_ptr() as *const u64) };
        assert_ne!(value_b, 0xAAAA);
    }

    /// 测试: 共享内存可见性
    #[test]
    fn test_shared_memory_visibility() {
        let shm = create_shared_memory(PROCESS_A, PAGE_SIZE, vec![
            (PROCESS_A, READ_WRITE),
            (PROCESS_B, READ_WRITE),
        ]).unwrap();

        let vaddr_a = shm.mapping_for(PROCESS_A);
        let vaddr_b = shm.mapping_for(PROCESS_B);

        // 进程 A 写入
        unsafe { *(vaddr_a.as_mut_ptr() as *mut u64) = 0xBEEF; }

        // 进程 B 应该可见
        let value = unsafe { *(vaddr_b.as_ptr() as *const u64) };
        assert_eq!(value, 0xBEEF);
    }

    /// 测试: NX 保护
    #[test]
    #[should_panic]
    fn test_nx_protection() {
        let frame = FRAME_ALLOCATOR.allocate_frame().unwrap();
        let page = Page::from_start_address(VirtAddr::new(0x2000_0000)).unwrap();
        // 映射为数据页 (无执行权限)
        map_page(page, frame, FLAGS_USER_DATA).unwrap();
        // 尝试执行应触发 SIGSEGV
        unsafe {
            let code_ptr = page.start_address().as_ptr::<u8>();
            let func: extern "sysv64" fn() = core::mem::transmute(code_ptr);
            func();
        }
    }

    /// 测试: 内存配额
    #[test]
    fn test_memory_quota() {
        let quota = MemoryQuota::new(1024 * 1024); // 1 MB
        assert!(quota.allocate(512 * 1024, MemoryRegion::Heap).is_ok());
        assert!(quota.allocate(512 * 1024, MemoryRegion::Heap).is_ok());
        assert!(quota.allocate(1, MemoryRegion::Heap).is_err()); // 超限
    }

    /// 测试: 缺页处理 (按需分页)
    #[test]
    fn test_demand_paging() {
        let process = create_test_process();
        // 预留虚拟地址范围但不映射
        let vaddr = reserve_vaddr_range(process, PAGE_SIZE);
        // 首次访问应触发缺页
        let value = unsafe { *(vaddr.as_ptr() as *const u64) };
        // 缺页处理后应返回零页
        assert_eq!(value, 0);
    }

    /// 测试: COW (写时复制)
    #[test]
    fn test_copy_on_write() {
        let frame = FRAME_ALLOCATOR.allocate_frame().unwrap();
        // 写入初始数据
        unsafe { *(frame.start_address().as_u64() as *mut u64) = 0x1234; }

        // fork 进程 (共享页表，标记 COW)
        let child = fork_process(PARENT_PROCESS);

        // 子进程写入应触发 COW
        let child_vaddr = child.translate(frame.start_address());
        unsafe { *(child_vaddr.as_mut_ptr() as *mut u64) = 0x5678; }

        // 父进程数据不应改变
        let parent_value = unsafe { *(frame.start_address().as_u64() as *const u64) };
        assert_eq!(parent_value, 0x1234);
    }
}
```

### 14.2 性能测试

| 测试项 | 目标 | 测量方法 |
|--------|------|---------|
| 帧分配延迟 | < 100ns | TSC 循环 |
| 连续 16 帧分配 | < 1μs | TSC 循环 |
| 页表映射 (4KB) | < 500ns | TSC 循环 |
| 页表映射 (2MB 大页) | < 300ns | TSC 循环 |
| TLB 刷新 (single) | < 50ns | INVLPG 指令 |
| TLB 刷新 (full) | < 5μs | MOV CR3 |
| 缺页处理 (零页) | < 2μs | 首次访问 |
| COW 缺页处理 | < 5μs | 写时触发 |
| Slab 分配 (256B) | < 50ns | 循环分配 |
| Slab 释放 (256B) | < 30ns | 循环释放 |
| EPT 映射 (4KB) | < 1μs | VM 内部测试 |
| EPT Violation 处理 | < 10μs | VM Exit 路径 |

---

## 15. 安全考虑

### 15.1 内存安全措施

| 威胁 | 防御措施 |
|------|---------|
| 缓冲区溢出 | Rust 边界检查 + NX 保护 |
| 整数溢出 (地址计算) | Rust 溢出检查 (`debug_assert!`) |
| 内核态访问用户数据 | SMAP 保护 |
| 内核态执行用户代码 | SMEP 保护 |
| 物理内存直接访问 | IOMMU (VT-d / AMD-Vi) |
| DMA 攻击 | IOMMU + bounce buffer |
| Rowhammer | ECC 内存 + 定期刷新 |
| 冷启动攻击 | 内存加密 (AES-XTS) |
| 页表劫持 | 页表只读映射 (CR0.WP) |
| Double Fetch | 内核态拷贝到本地后再验证 |
| Use-after-free | Rust 所有权系统 + Slab allocator 隔离 |

### 15.2 KASLR (内核地址空间布局随机化)

```rust
/// KASLR: 随机化内核加载地址
pub fn apply_kaslr(boot_info: &mut BootInfo) {
    // 在 bootloader 阶段已随机选择内核加载地址
    // 内核运行时使用以下偏移:

    let kaslr_offset = boot_info.kernel_random_offset;

    // 所有内核地址引用都通过 PHYS_OFFSET + kaslr_offset 计算
    // 攻击者无法预测内核代码和数据的位置
}
```

---

*本文档由 OmniAgent OS 内存管理团队维护，如有疑问请联系 memory@omniagent.os*
