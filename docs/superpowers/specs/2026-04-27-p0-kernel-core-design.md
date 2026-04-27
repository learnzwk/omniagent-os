# OmniAgent OS 系统完善设计文档 — P0 内核核心能力

> **文档版本**: v1.0.0
> **日期**: 2026-04-27
> **状态**: 待审阅
> **范围**: 调度器、虚拟内存管理、内核启动完善、Slab 分配器

---

## 0. 执行摘要

本文档定义 OmniAgent OS 从"骨架阶段"推进到"功能可运行阶段"所需的 P0（最高优先级）内核核心能力设计。这些模块是 Agent 能够真正被调度执行、拥有独立地址空间、系统资源可管理的基础前提。

**四个模块的依赖关系：**
```
Slab 分配器 ──→ 虚拟内存管理 ──→ 调度器
                                   │
内核启动完善 ←─────────────────────┘
```

- **Slab 分配器**：替换 bump allocator，提供内存释放能力，是虚拟内存管理的前置依赖
- **虚拟内存管理**：4 级页表、地址空间隔离，是调度器的前置依赖
- **调度器**：CFS 变体，Agent 可被调度执行
- **内核启动完善**：将所有子系统正确初始化并串联

---

## 1. Slab 分配器

### 1.1 设计动机

当前内核使用 `BumpAllocator`，只分配不释放。这对于短期运行的初始化代码可以接受，但对于长期运行的操作系统内核不可接受——内核对象（TCB、文件描述符、页表等）需要频繁创建和销毁。

### 1.2 设计方案

采用简化版 Slab 分配器，针对 `no_std` 环境优化：

**核心思路：**
- 按对象大小分类为固定大小的缓存（cache），每个缓存管理一组 slab
- 每个 slab 是一页或多页物理内存，被划分为等大小的对象槽位
- 使用空闲链表（free list）跟踪可用槽位，O(1) 分配/释放
- 支持对象构造/析构回调（用于 TCB 等需要初始化的对象）

**与 Linux Slab 的区别：**
- 不支持 `SLAB_HWCACHE_ALIGN`（简化实现）
- 不支持 `slab coloring`（避免复杂性）
- 不支持 per-CPU slab（单核优先，后续扩展）
- 使用 `spin::Mutex` 而非自旋锁原语

### 1.3 数据结构

```rust
/// Slab 分配器错误
#[derive(Debug, Clone)]
pub enum SlabError {
    OutOfMemory,
    InvalidSize(usize),
    CacheNotFound(&'static str),
    Poisoned,
}

/// Slab 缓存：管理特定大小对象的分配
pub struct SlabCache {
    /// 缓存名称（调试用）
    name: &'static str,
    /// 对象大小（字节）
    object_size: usize,
    /// 对象对齐要求
    align: usize,
    /// 每个 slab 包含的对象数量
    objects_per_slab: usize,
    /// 空闲对象链表头
    free_list: Option<&'static mut SlabObjectHeader>,
    /// 已分配对象数量
    allocated: AtomicUsize,
    /// 所有 slab 页面列表
    slabs: SlabList,
    /// 对象构造回调
    constructor: Option<fn(&mut [u8])>,
    /// 对象析构回调
    destructor: Option<fn(&mut [u8])>,
}

/// Slab 对象头部：嵌入在空闲对象中
#[repr(C)]
struct SlabObjectHeader {
    /// 指向下一个空闲对象
    next: Option<&'static mut SlabObjectHeader>,
    /// 魔数（调试用，检测 use-after-free）
    magic: u32,
}

/// Slab 页面：一个或多个连续物理帧
struct SlabPage {
    /// 页面起始物理地址
    base_addr: usize,
    /// 此页面中的对象数量
    object_count: usize,
    /// 此页面中空闲对象数量
    free_count: AtomicUsize,
    /// 是否已满（优化快速路径）
    is_full: AtomicBool,
}

/// Slab 列表：部分满 → 空 → 全满
struct SlabList {
    partial: Vec<&'static mut SlabPage>,
    empty: Vec<&'static mut SlabPage>,
    full: Vec<&'static mut SlabPage>,
}

/// 全局 Slab 分配器
pub struct SlabAllocator {
    /// 已注册的缓存
    caches: Mutex<BTreeMap<&'static str, SlabCache>>,
    /// 总分配次数
    total_allocs: AtomicU64,
    /// 总释放次数
    total_frees: AtomicU64,
    /// 用于分配 slab 页面的后备帧分配器
    frame_allocator: Option<&'static BitmapFrameAllocator>,
}
```

### 1.4 API 设计

```rust
impl SlabAllocator {
    /// 创建命名缓存
    pub fn create_cache(
        &self,
        name: &'static str,
        object_size: usize,
        align: usize,
        constructor: Option<fn(&mut [u8])>,
        destructor: Option<fn(&mut [u8])>,
    ) -> Result<(), SlabError>;

    /// 从命名缓存分配对象
    pub fn alloc(&self, cache_name: &str) -> Result<*mut u8, SlabError>;

    /// 释放对象到命名缓存
    pub fn free(&self, cache_name: &str, ptr: *mut u8) -> Result<(), SlabError>;

    /// 获取缓存统计
    pub fn cache_stats(&self, cache_name: &str) -> Option<SlabCacheStats>;

    /// 通用分配（自动选择合适大小的缓存）
    pub fn kmalloc(&self, size: usize, align: usize) -> Result<*mut u8, SlabError>;

    /// 通用释放
    pub fn kfree(&self, ptr: *mut u8, size: usize) -> Result<(), SlabError>;
}

/// 缓存统计信息
pub struct SlabCacheStats {
    pub name: &'static str,
    pub object_size: usize,
    pub allocated: usize,
    pub total_objects: usize,
    pub slab_count: usize,
}
```

### 1.5 预定义缓存

内核启动时创建以下标准缓存：

| 缓存名 | 对象大小 | 用途 |
|--------|---------|------|
| `task_struct` | 512B | 任务控制块 (TCB) |
| `file_desc` | 64B | 文件描述符 |
| `vm_area` | 48B | 虚拟内存区域 |
| `page_table` | 4096B | 页表页 |
| `socket` | 256B | 网络套接字 |
| `inode` | 128B | VFS inode |
| `dentry` | 96B | 目录项缓存 |
| `kmalloc-64` | 64B | 通用小对象 |
| `kmalloc-128` | 128B | 通用中对象 |
| `kmalloc-256` | 256B | 通用中对象 |
| `kmalloc-512` | 512B | 通用中对象 |
| `kmalloc-1024` | 1024B | 通用大对象 |
| `kmalloc-2048` | 2048B | 通用大对象 |

### 1.6 测试策略

```
TDD 测试用例（每个必须先写失败测试）：
1. test_create_cache — 创建缓存成功
2. test_alloc_from_cache — 从缓存分配对象
3. test_free_to_cache — 释放对象回缓存
4. test_alloc_dealloc_cycle — 多次分配释放循环
5. test_cache_exhaustion — 缓存耗尽时分配新 slab
6. test_cache_stats — 统计信息正确
7. test_kmalloc_kfree — 通用分配释放
8. test_alignment — 对齐要求正确
9. test_magic_detection — use-after-free 检测
10. test_multiple_caches — 多缓存独立运行
11. test_constructor_destructor — 构造/析构回调正确调用
12. test_concurrent_alloc_free — 并发分配释放安全
```

### 1.7 文件结构

```
kernel/src/memory/
├── mod.rs              # 更新：添加 slab 模块导出
├── slab.rs             # 新建：Slab 分配器核心实现
└── slab_tests.rs       # 新建：Slab 分配器测试（或内联在 slab.rs 的 #[cfg(test)] 中）
```

---

## 2. 虚拟内存管理

### 2.1 设计动机

当前内核只有物理内存管理（帧分配器 + bump 堆），没有虚拟内存管理。这意味着：
- 无法为 Agent 提供独立的地址空间
- 无法实现内存保护（所有代码共享同一地址空间）
- 无法实现按需分页（demand paging）
- 无法支持 mmap 系统调用

### 2.2 设计方案

实现完整的 4 级页表管理（PML4 → PDPT → PD → PT），兼容 x86_64 架构规范。

**关键设计决策：**

1. **不使用 `x86_64` crate 的页表抽象**：该 crate 依赖 `bootloader` 的特定接口，我们自行实现页表操作以获得完全控制
2. **惰性页表映射**：内核启动时只映射必要的地址范围，按需扩展
3. **写时复制（COW）**：Agent fork 时共享物理页，写入时才复制
4. **反向映射**：通过 Page 结构体跟踪每个物理页被哪些虚拟地址引用

### 2.3 核心数据结构

```rust
// ============ 地址类型 ============

/// 虚拟地址（64位，仅使用低48位）
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(pub u64);

/// 物理地址（64位）
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(pub u64);

/// 页号（4KB 页）
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PageNum(pub u64);

// ============ 页表条目 ============

/// 页表条目标志
pub struct PageTableFlags: u64 {
    const PRESENT         = 1 << 0;
    const WRITABLE        = 1 << 1;
    const USER_ACCESSIBLE = 1 << 2;
    const WRITE_THROUGH   = 1 << 3;
    const NO_CACHE        = 1 << 4;
    const ACCESSED        = 1 << 5;
    const DIRTY           = 1 << 6;
    const HUGE_PAGE       = 1 << 7;
    const GLOBAL          = 1 << 8;
    const NO_EXECUTE      = 1 << 63;
}

/// 页表条目
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub fn is_unused(&self) -> bool { self.0 == 0 }
    pub fn flags(&self) -> PageTableFlags { PageTableFlags::from_bits_truncate(self.0) }
    pub fn phys_addr(&self) -> PhysAddr { PhysAddr(self.0 & 0x000f_ffff_ffff_f000) }
    pub fn set_addr(&mut self, addr: PhysAddr) { /* 保留旧 flags，更新地址 */ }
    pub fn set_flags(&mut self, flags: PageTableFlags) { /* 保留旧地址，更新 flags */ }
}

// ============ 页表 ============

/// 4 级页表（PML4）
pub struct PageTable {
    /// PML4 的物理地址
    pml4_frame: PhysFrame,
    /// 此页表映射的用户/内核虚拟地址范围
    mapper: PageMapper,
}

/// 页表映射器：提供高层映射接口
pub struct PageMapper {
    pml4: VirtAddr,
    /// 用于分配页表页面的帧分配器
    frame_alloc: fn() -> Option<PhysFrame>,
    /// 用于映射页表页面的临时映射
    temporary_map: fn(PhysFrame) -> VirtAddr,
}

impl PageMapper {
    /// 映射一个 4KB 页
    pub fn map_to(
        &mut self,
        page: Page,
        frame: PhysFrame,
        flags: PageTableFlags,
    ) -> Result<MapperFlush, MapToError>;

    /// 取消映射一个 4KB 页
    pub fn unmap(&mut self, page: Page) -> Result<(PhysFrame, MapperFlush), UnmapError>;

    /// 更改映射的标志
    pub fn update_flags(&mut self, page: Page, flags: PageTableFlags) -> Result<MapperFlush, MapToError>;

    /// 将虚拟地址范围转换为物理帧迭代器
    pub fn translate(&self, addr: VirtAddr) -> Option<PhysAddr>;
}

/// 映射刷新：TLB 需要刷新
pub struct MapperFlush {
    addr: VirtAddr,
    /// 刷新整个 TLB 还是单个地址
    whole: bool,
}

impl MapperFlush {
    pub fn flush_all() -> Self { Self { addr: VirtAddr(0), whole: true } }
    pub fn ignore() -> Self { Self { addr: VirtAddr(0), whole: false } }
    /// 执行 TLB 刷新
    pub unsafe fn flush(self) { /* invlpg 或 reload_cr3 */ }
}

// ============ 地址空间 ============

/// 虚拟地址空间
pub struct AddressSpace {
    /// 页表
    page_table: PageTable,
    /// 虚拟内存区域列表
    areas: Vec<VmArea>,
    /// 地址空间类型
    kind: AddressSpaceKind,
    /// 此地址空间被引用的次数
    ref_count: AtomicUsize,
}

/// 虚拟内存区域
pub struct VmArea {
    /// 区域起始虚拟地址（页对齐）
    start: VirtAddr,
    /// 区域结束虚拟地址（页对齐，exclusive）
    end: VirtAddr,
    /// 区域标志
    flags: VmFlags,
    /// 关联的物理帧（或 COW 引用）
    backing: VmBacking,
    /// 区域名称（调试用）
    name: &'static str,
}

/// 虚拟内存区域标志
pub struct VmFlags: u32 {
    const READ       = 1 << 0;
    const WRITE      = 1 << 1;
    const EXECUTE    = 1 << 2;
    const USER       = 1 << 3;
    const COW        = 1 << 4;  // 写时复制
    const GUARD      = 1 << 5;  // 保护页（不可访问）
    const STACK      = 1 << 6;  // 栈区域（自动向下扩展）
    const HEAP       = 1 << 7;  // 堆区域（自动向上扩展）
}

/// 地址空间类型
pub enum AddressSpaceKind {
    /// 内核地址空间（高半部分）
    Kernel,
    /// 用户/Agent 地址空间
    User {
        /// Agent 句柄
        agent_handle: u64,
        /// 代码段范围
        code_range: (VirtAddr, VirtAddr),
        /// 数据段范围
        data_range: (VirtAddr, VirtAddr),
        /// 堆范围
        heap_range: (VirtAddr, VirtAddr),
        /// 栈范围
        stack_range: (VirtAddr, VirtAddr),
    },
}
```

### 2.4 x86_64 内存布局

```
虚拟地址空间布局（48位，256TB）：

0x0000_0000_0000_0000 ───────────────────── 用户空间低半部分
  │ 代码段 (.text)     │  Agent 可执行代码
  │ 数据段 (.data/.bss) │  Agent 全局数据
  │ 堆 (heap)          │  向上增长 ↑
  │ ...                │  空闲
  │ 栈 (stack)         │  向下增长 ↓
  │ ...                │  空闲
  │ 共享内存区域        │  Agent 间共享
  │ mmap 区域          │  动态映射
0x0000_7FFF_FFFF_F000 ───────────────────── 用户空间顶部

0xFFFF_8000_0000_0000 ───────────────────── 内核空间底部
  │ 物理内存直接映射    │  phys + 0xFFFF_8000_0000_0000
  │ 内核代码段          │  .text, .rodata
  │ 内核数据段          │  .data, .bss
  │ 内核堆             │  Slab 分配器
  │ 内核栈             │  每核 8KB
  │ 设备映射区域        │  MMIO
  │ VFS 缓存           │  页缓存
0xFFFF_FFFF_FFFF_FFFF ───────────────────── 内核空间顶部
```

### 2.5 内核地址空间初始化

```rust
/// 初始化内核虚拟地址空间
///
/// 在内核启动早期调用，建立内核自身的页表映射：
/// 1. 物理内存直接映射（identity map + high map）
/// 2. 内核代码段映射（.text: R-X）
/// 3. 内核数据段映射（.data, .bss: RW-）
/// 4. 内核堆映射（RW-）
/// 5. MMIO 设备区域映射（UC-）
pub fn init_kernel_address_space(boot_info: &BootInfo) -> Result<PageTable, MapToError>;
```

### 2.6 缺页处理

```rust
/// 缺页错误类型
pub enum PageFaultType {
    /// 页不存在（首次访问）
    NotPresent,
    /// 权限不足（写只读页 → COW）
    ProtectionFault,
    /// 写时复制触发
    CopyOnWrite,
    /// 栈自动扩展
    StackGrowth,
    /// 堆自动扩展
    HeapGrowth,
}

/// 缺页处理程序
pub fn handle_page_fault(
    fault_addr: VirtAddr,
    error_code: u64,
    current_space: &mut AddressSpace,
) -> Result<(), PageFaultError>;
```

### 2.7 测试策略

```
TDD 测试用例：
1. test_page_table_entry_flags — PTE 标志位正确设置/读取
2. test_page_table_entry_addr — PTE 地址正确设置/读取
3. test_virt_addr_page_align — 虚拟地址页对齐计算
4. test_phys_to_virt_direct_map — 物理地址到直接映射虚拟地址转换
5. test_map_4kb_page — 映射单个 4KB 页成功
6. test_map_and_translate — 映射后翻译地址正确
7. test_unmap_page — 取消映射成功
8. test_map_large_range — 映射大范围地址
9. test_vm_area_create — 创建虚拟内存区域
10. test_vm_area_overlap_check — 重叠检测
11. test_address_space_user_create — 创建用户地址空间
12. test_address_space_kernel_init — 内核地址空间初始化
13. test_cow_trigger — 写时复制触发
14. test_stack_growth — 栈自动扩展
15. test_heap_growth — 堆自动扩展
16. test_page_fault_handler — 缺页处理程序
17. test_tlb_flush — TLB 刷新
```

### 2.8 文件结构

```
kernel/src/memory/
├── mod.rs              # 更新：添加 vm 模块导出
├── vm/
│   ├── mod.rs          # 新建：虚拟内存模块声明
│   ├── addr.rs         # 新建：VirtAddr, PhysAddr, PageNum
│   ├── pte.rs          # 新建：PageTableEntry, PageTableFlags
│   ├── page_table.rs   # 新建：PageTable, PageMapper
│   ├── address_space.rs # 新建：AddressSpace, VmArea
│   └── page_fault.rs   # 新建：缺页处理
```

---

## 3. 调度器

### 3.1 设计动机

当前内核没有调度器。Agent 创建后停留在 `Creating` 状态，无法被推进到 `Ready` 或 `Running`。没有调度器，操作系统无法多任务，Agent 无法执行。

### 3.2 设计方案

基于现有 `scheduler-spec.md` 规范，实现简化版 CFS 调度器，适配当前 `no_std` 环境。

**关键设计决策：**

1. **不依赖 `arceos-fairsched`**：该 crate 需要特定配置，我们自行实现 CFS 核心逻辑
2. **使用 `BTreeMap` 替代红黑树**：Rust 标准库的 `BTreeMap` 是红黑树实现，满足 O(log n) 要求
3. **单核优先**：先实现单核调度，多核 SMP 作为后续扩展
4. **与现有 AgentControlBlock 集成**：调度器操作 ACB 的状态字段
5. **上下文切换使用内联汇编**：不依赖外部 crate

### 3.3 核心数据结构

```rust
// ============ 任务控制块 (TCB) ============

/// 任务 ID
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TaskId(pub u64);

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    Created = 0,
    Ready   = 1,
    Running = 2,
    Blocked = 3,
    Zombie  = 4,
}

/// 任务标志
pub struct TaskFlags: u64 {
    const NEED_RESCHED = 1 << 0;  // 需要重新调度
    const EXITED       = 1 << 1;  // 已退出
    const IS_KERNEL    = 1 << 2;  // 内核任务
    const IS_AGENT     = 1 << 3;  // Agent 任务
    const IS_IDLE      = 1 << 4;  // 空闲任务
}

/// 任务控制块
pub struct TaskControlBlock {
    /// 任务 ID
    pub id: TaskId,
    /// 任务状态
    pub state: AtomicU8,
    /// 任务标志
    pub flags: AtomicU64,
    /// 调度信息
    pub sched_info: SchedInfo,
    /// 上下文帧（保存在内核栈上）
    pub context: ContextFrame,
    /// 内核栈
    pub kernel_stack: KernelStack,
    /// 关联的地址空间
    pub address_space: Option<AddressSpace>,
    /// 关联的 Agent 句柄（如果是 Agent 任务）
    pub agent_handle: Option<u64>,
    /// 等待通道（用于 sleep/wakeup）
    pub wait_channel: AtomicU64,
    /// 退出状态码
    pub exit_code: AtomicI32,
    /// 父任务 ID
    pub parent: AtomicU64,
    /// 子任务列表
    pub children: SpinLock<Vec<TaskId>>,
}

/// 内核栈
pub struct KernelStack {
    /// 栈顶地址（高地址）
    top: VirtAddr,
    /// 栈底地址（低地址）
    bottom: VirtAddr,
    /// 栈大小
    size: usize,
}

/// 调度信息
pub struct SchedInfo {
    /// 虚拟运行时间（CFS 排序键）
    pub vruntime: AtomicU64,
    /// 实际运行时间
    pub runtime: AtomicU64,
    /// 优先级类
    pub priority: PriorityClass,
    /// 优先级权重
    pub weight: u32,
    /// 时间片剩余
    pub time_slice_remain: AtomicU64,
    /// 上次调度时间戳
    pub last_sched_tick: AtomicU64,
}

// ============ 优先级 ============

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PriorityClass {
    Idle     = 0,
    Normal   = 1,
    Agent    = 2,
    High     = 3,
    Realtime = 4,
}

// ============ 运行队列 ============

/// 运行队列
pub struct RunQueue {
    /// 当前运行的任务
    current: AtomicU64,  // TaskId or 0
    /// 各优先级类的就绪队列
    trees: [PriorityTree; 5],
    /// 全局最小 vruntime
    min_vruntime: AtomicU64,
    /// 就绪任务数
    nr_running: AtomicU32,
    /// 运行队列锁
    lock: SpinLock<()>,
}

/// 优先级树
pub struct PriorityTree {
    /// BTreeMap: vruntime → TaskId
    tree: BTreeMap<u64, u64>,
    /// 任务数
    nr_tasks: u32,
    /// 总权重
    total_weight: u64,
}

// ============ 全局调度器 ============

/// 全局调度器
pub static SCHEDULER: Lazy<SpinLock<Scheduler>> = Lazy::new(|| {
    SpinLock::new(Scheduler::new())
});

pub struct Scheduler {
    /// 运行队列（单核版，后续扩展为 [RunQueue; MAX_CPUS]）
    run_queue: RunQueue,
    /// 任务表：TaskId → TCB
    tasks: BTreeMap<u64, Box<TaskControlBlock>>,
    /// 下一个任务 ID
    next_task_id: AtomicU64,
    /// 空闲任务
    idle_task: Option<TaskId>,
    /// 统计信息
    stats: SchedulerStats,
    /// 配置
    config: SchedulerConfig,
}

// ============ 上下文帧 ============

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ContextFrame {
    pub rax: u64, pub rbx: u64, pub rcx: u64, pub rdx: u64,
    pub rsi: u64, pub rdi: u64, pub rbp: u64,
    pub r8: u64,  pub r9: u64,  pub r10: u64, pub r11: u64,
    pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub rip: u64, pub cs: u64,  pub rflags: u64,
    pub rsp: u64, pub ss: u64,
}
```

### 3.4 调度器 API

```rust
impl Scheduler {
    /// 初始化调度器（创建空闲任务）
    pub fn init(&mut self);

    /// 创建新任务
    pub fn create_task(
        &mut self,
        entry: u64,
        stack_top: u64,
        priority: PriorityClass,
        is_user: bool,
        agent_handle: Option<u64>,
    ) -> Result<TaskId, SchedulerError>;

    /// 将任务加入就绪队列
    pub fn enqueue(&mut self, task_id: TaskId) -> Result<(), SchedulerError>;

    /// 将任务移出就绪队列（阻塞）
    pub fn dequeue(&mut self, task_id: TaskId) -> Result<(), SchedulerError>;

    /// 主动让出 CPU
    pub fn yield_now(&mut self);

    /// 任务睡眠
    pub fn sleep(&mut self, task_id: TaskId, channel: u64);

    /// 唤醒任务
    pub fn wake_up(&mut self, task_id: TaskId);

    /// 终止任务
    pub fn exit(&mut self, task_id: TaskId, exit_code: i32);

    /// 调度入口：选择下一个任务并执行上下文切换
    pub fn schedule(&mut self);

    /// 获取当前运行的任务
    pub fn current_task(&self) -> Option<&TaskControlBlock>;

    /// 定时器 tick 处理
    pub fn timer_tick(&mut self);
}
```

### 3.5 与 AgentControlBlock 的集成

调度器与现有 Agent 子系统的集成点：

```rust
/// Agent 创建时，同时创建对应的 TCB
pub fn agent_spawn_with_scheduler(agent_spec: &AgentSpec) -> Result<AgentHandle, SyscallError> {
    // 1. 通过 AgentPool 创建 ACB
    let handle = AGENT_POOL.lock().spawn(agent_spec)?;

    // 2. 创建对应的 TCB
    let task_id = SCHEDULER.lock().create_task(
        agent_spec.entry_point,
        agent_spec.stack_size as u64,
        PriorityClass::Agent,  // Agent 优先级
        true,                  // 用户态
        Some(handle.0),
    )?;

    // 3. 将 TCB 加入就绪队列
    SCHEDULER.lock().enqueue(task_id)?;

    // 4. 更新 ACB 状态为 Ready
    AGENT_POOL.lock().update_state(handle, AgentState::Ready)?;

    Ok(handle)
}
```

### 3.6 上下文切换

```rust
/// 汇编上下文切换（内联汇编）
///
/// 保存 callee-saved 寄存器，切换栈指针，恢复 callee-saved 寄存器
#[naked]
pub unsafe extern "sysv64" fn context_switch(
    prev_frame: *mut ContextFrame,
    next_frame: *const ContextFrame,
) {
    core::arch::naked_asm!(
        // 保存 callee-saved 寄存器到 prev 栈
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // 保存 RSP 到 prev->rsp
        "mov [rdi + 120], rsp",  // rsp offset in ContextFrame

        // 切换到 next 栈
        "mov rsp, [rsi + 120]",

        // 恢复 callee-saved 寄存器
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",

        "ret",
    );
}
```

### 3.7 测试策略

```
TDD 测试用例：
1. test_priority_class_ordering — 优先级排序正确
2. test_priority_weights — 权重值正确
3. test_task_state_transitions — 状态转换验证
4. test_create_task — 创建任务成功
5. test_enqueue_dequeue — 入队出队
6. test_pick_next_task_priority — 按优先级选择
7. test_pick_next_task_vruntime — 同优先级按 vruntime 选择
8. test_vruntime_update — vruntime 更新计算
9. test_vruntime_agent_slower — Agent vruntime 增长更慢
10. test_time_slice_calculation — 时间片计算
11. test_context_frame_init — 上下文帧初始化
12. test_scheduler_init — 调度器初始化
13. test_yield_now — 主动让出
14. test_sleep_wakeup — 睡眠唤醒
15. test_task_exit — 任务退出
16. test_timer_tick_preempt — 定时器触发抢占
17. test_idle_task — 空闲任务
18. test_scheduler_stats — 统计信息
19. test_multiple_tasks_fairness — 多任务公平性
20. test_agent_priority_boost — Agent 优先级提升效果
```

### 3.8 文件结构

```
kernel/src/scheduler/
├── mod.rs              # 新建：调度器模块声明 + 重导出
├── task.rs             # 新建：TaskControlBlock, TaskId, TaskState
├── priority.rs         # 新建：PriorityClass, SchedInfo
├── run_queue.rs        # 新建：RunQueue, PriorityTree
├── context.rs          # 新建：ContextFrame, context_switch
├── scheduler.rs        # 新建：Scheduler 全局调度器
└── error.rs            # 新建：SchedulerError
```

---

## 4. 内核启动完善

### 4.1 当前问题

当前 `main.rs` 的启动流程缺少关键初始化步骤：

```rust
// 当前（不完整）：
drivers::serial::init_serial();
drivers::serial::init_logger();
unsafe { arch::x86_64::gdt::load_gdt(); }
unsafe { interrupts::init_idt(); }
arch::x86_64::pic::disable_pic();
unsafe { arch::x86_64::apic::init_local_apic(); }
unsafe { time::timer::init_pit_timer(); }
unsafe { drivers::keyboard::init_keyboard(); }
println!("=== OmniAgent OS v0.1.0 ===");
loop {}  // ← 死循环，没有调度
```

**缺失：**
1. 内核堆未初始化（`init_heap()` 未调用）
2. Syscall 子系统未初始化
3. Agent 子系统未初始化
4. 虚拟内存未初始化
5. 调度器未初始化
6. 没有进入调度循环

### 4.2 完善后的启动流程

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // === Phase 1: 硬件初始化 ===
    drivers::serial::init_serial();
    drivers::serial::init_logger();

    // === Phase 2: 架构初始化 ===
    unsafe { arch::x86_64::gdt::load_gdt(); }
    unsafe { interrupts::init_idt(); }
    arch::x86_64::pic::disable_pic();
    unsafe { arch::x86_64::apic::init_local_apic(); }

    // === Phase 3: 内存初始化 ===
    let boot_info = boot::multiboot2::parse_boot_info();
    memory::frame_allocator::init(&boot_info);
    memory::heap::init_heap(boot_info.heap_start, boot_info.heap_size);
    memory::slab::init();  // Slab 分配器初始化
    memory::vm::init_kernel_address_space(&boot_info);

    // === Phase 4: 子系统初始化 ===
    unsafe { time::timer::init_pit_timer(); }
    unsafe { drivers::keyboard::init_keyboard(); }
    syscall::dispatch::init();      // Syscall 子系统
    agent::pool::init();            // Agent 池
    agent::communication::init();   // Agent 通信

    // === Phase 5: 调度器初始化 ===
    scheduler::init();
    // 创建内核初始任务（init task）
    scheduler::create_kernel_task(init_task_entry, "init");

    println!("=== OmniAgent OS v0.1.0 ===");
    println!("All subsystems initialized");
    println!("Starting scheduler...");

    // === Phase 6: 进入调度循环 ===
    scheduler::run();  // 永不返回
}

/// 内核初始任务
fn init_task_entry() {
    println!("Init task started");
    // 在这里启动系统服务...
    // 例如：文件系统服务、网络服务、桌面环境等
    loop {
        scheduler::yield_now();
    }
}
```

### 4.3 Panic Handler 改进

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 输出 panic 信息到串口
    println!("!!! KERNEL PANIC !!!");
    if let Some(location) = info.location() {
        println!("  at {}:{}:{}", location.file(), location.line(), location.column());
    }
    if let Some(message) = info.payload().downcast_ref::<&str>() {
        println!("  {}", message);
    }
    // 停机
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
```

### 4.4 测试策略

```
TDD 测试用例：
1. test_boot_sequence_order — 验证启动顺序正确
2. test_heap_initialized — 堆初始化后可分配
3. test_slab_initialized — Slab 初始化后可分配
4. test_syscall_initialized — Syscall 子系统初始化后可分发
5. test_agent_pool_initialized — Agent 池初始化后可创建 Agent
6. test_scheduler_initialized — 调度器初始化后可创建任务
7. test_panic_handler_output — Panic 输出正确信息
```

### 4.5 文件变更

```
kernel/src/
├── main.rs             # 修改：完善启动流程
└── boot/
    └── multiboot2.rs   # 修改：增强引导信息解析
```

---

## 5. 跨模块集成

### 5.1 内核模块依赖图

```
                    ┌──────────────┐
                    │   main.rs    │
                    └──────┬───────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
    ┌─────▼─────┐   ┌─────▼─────┐   ┌─────▼─────┐
    │  memory   │   │ scheduler │   │  syscall  │
    │  slab     │   │           │   │  dispatch │
    │  vm       │   │           │   │           │
    └─────┬─────┘   └─────┬─────┘   └─────┬─────┘
          │                │                │
          │         ┌──────▼──────┐         │
          │         │   agent    │◄────────┘
          │         │   pool     │
          │         │   comm     │
          │         └─────────────┘
          │
    ┌─────▼─────┐
    │  frame    │
    │ allocator │
    └───────────┘
```

### 5.2 lib.rs 更新

```rust
// kernel/src/lib.rs
#![cfg_attr(not(test), no_std)]

pub mod boot;
pub mod vga;
pub mod drivers;
pub mod arch;
pub mod interrupts;
pub mod memory;    // 更新：导出 slab, vm 子模块
pub mod time;
pub mod syscall;
pub mod agent;
pub mod scheduler; // 新增

pub const KERNEL_VERSION: &str = "0.2.0";
pub const KERNEL_NAME: &str = "OmniAgent OS";
```

---

## 6. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 页表操作复杂度高 | 实现周期长 | 分阶段实现：先内核映射，再用户映射 |
| 上下文切换汇编错误 | 系统崩溃 | QEMU + GDB 调试，逐步验证 |
| Slab 分配器与帧分配器耦合 | 内存管理复杂 | 清晰的接口边界，Slab 通过 trait 获取帧 |
| 调度器与 Agent 状态同步 | 状态不一致 | 使用原子操作，严格的状态转换验证 |
| 测试覆盖不足 | 运行时错误 | TDD 强制每个功能有测试 |

---

## 7. 成功标准

| 标准 | 验证方法 |
|------|---------|
| Slab 分配器可分配/释放 | 单元测试 + 内核启动后分配成功 |
| 虚拟内存可映射/取消映射 | 单元测试 + QEMU 中验证 |
| 调度器可切换任务 | 单元测试 + 创建多个任务验证轮转 |
| 内核启动完整 | QEMU 中看到所有子系统初始化日志 |
| 所有测试通过 | `cargo test --workspace` 全绿 |
| 无编译警告 | `cargo clippy -- -D warnings` |
