# OmniAgent OS 虚拟化架构设计规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: L1 架构设计文档
> **目标读者**: 内核开发者、虚拟化子系统开发者、Agent 运行时开发者

---

## 1. 文档目的

本文档详细描述 OmniAgent OS 虚拟化子系统的架构设计，包括 Hypervisor 架构选型、虚拟机生命周期管理、VM Exit 处理机制、虚拟设备模型、内存虚拟化、Agent 与虚拟机的协作关系、安全隔离策略以及性能优化手段。本文档与微内核设计规范（`microkernel-design.md`）中第 9 节「虚拟化支持」相衔接，是虚拟化子系统实现的主要参考依据。

---

## 2. 概述

### 2.1 虚拟化在 OmniAgent OS 中的定位

OmniAgent OS 将虚拟化作为 Agent 隔离执行的核心基础设施。虚拟机（VM）为 Agent 提供硬件级别的强隔离环境，使得不同信任级别的 Agent 可以在独立的地址空间和特权级别中运行，同时共享底层硬件资源。

```
┌─────────────────────────────────────────────────────────────┐
│                    OmniAgent OS 系统全景                      │
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  Host Agent  │  │  Host Agent  │  │  Host Agent  │         │
│  │  (用户态)    │  │  (用户态)    │  │  (用户态)    │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│  ───────┴────────────────┴────────────────┴──────────       │
│         │         微内核 + Hypervisor          │             │
│  ───────┬────────────────┬────────────────┬──────────       │
│         │                │                │                 │
│  ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐         │
│  │   VM #1     │  │   VM #2     │  │   VM #3     │         │
│  │ ┌─────────┐ │  │ ┌─────────┐ │  │ ┌─────────┐ │         │
│  │ │ Agent A │ │  │ │ Agent B │ │  │ │ Agent C │ │         │
│  │ │(隔离环境)│ │  │ │(隔离环境)│ │  │ │(隔离环境)│ │         │
│  │ └─────────┘ │  │ └─────────┘ │  │ └─────────┘ │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              硬件资源 (CPU / 内存 / 设备)              │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 设计目标

| 目标 | 描述 | 优先级 |
|------|------|--------|
| **硬件辅助虚拟化** | 支持 Intel VT-x / AMD-V，利用 EPT/NPT 实现高效内存虚拟化 | P0 |
| **Agent 隔离执行** | 虚拟机作为 Agent 的隔离沙箱，提供硬件级安全边界 | P0 |
| **低延迟 VM Exit** | VM Exit 处理延迟 < 50μs，满足 Agent 实时交互需求 | P0 |
| **快速 VM 创建** | VM 创建到启动 < 2s，支持 Agent 按需启动 | P1 |
| **嵌套虚拟化** | 可选支持嵌套虚拟化，允许 VM 内运行 Hypervisor | P2 |
| **设备直通** | 支持 PCI passthrough，为高性能 Agent 提供专用硬件 | P1 |
| **VM 快照/迁移** | 支持虚拟机快照和在线迁移，与 Agent 迁移集成 | P2 |

### 2.3 与微内核的关系

虚拟化子系统作为微内核的可选扩展模块存在。根据微内核设计规范第 9 节，虚拟化支持通过以下方式与内核集成：

- **中断向量 38** (`VIRTUALIZATION`)：专门用于 VM Exit 处理
- **进程类型**：`ProcessType::VirtualMachine` 在进程模型中已预留
- **虚拟化管理器**：作为用户态服务在引导阶段 4 中启动
- **内核态最小化**：仅 VMX/SVM 指令执行和 VMCS/VMCB 操作在内核态，其余管理逻辑在用户态

---

## 3. Hypervisor 架构

### 3.1 Type-1 vs Type-2 设计选择

OmniAgent OS 采用 **内置 Hypervisor（Bare-metal Hypervisor）** 模式，即 Type-1 架构的变体。Hypervisor 直接运行在硬件之上，与微内核共享 Ring 0 特权级别，而非作为宿主 OS 上的用户态进程（Type-2）。

```
┌─────────────────────────────────────────────────────────────┐
│                    Type-2 架构 (不采用)                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  宿主 OS (Ring 0)                                      │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Hypervisor (Ring 3 / Ring 0 模拟)              │  │  │
│  │  │  - VM Exit 需两次上下文切换                       │  │  │
│  │  │  - 性能开销大                                    │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│              OmniAgent OS 内置 Hypervisor (采用)             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  微内核 + Hypervisor (Ring 0, 共享特权级)              │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Hypervisor 核心 (VMX root 模式)                 │  │  │
│  │  │  - VM Exit 直接在 Ring 0 处理                    │  │  │
│  │  │  - 单次上下文切换                                │  │  │
│  │  │  - 与调度器紧密集成                              │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  虚拟化管理器 (用户态服务)                        │  │  │
│  │  │  - VM 生命周期管理                               │  │  │
│  │  │  - 设备后端管理                                  │  │  │
│  │  │  - 通过 Agent syscall 与内核交互                  │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**选择内置 Hypervisor 的理由**：

1. **性能**：VM Exit 直接在 VMX root 模式处理，无需额外的用户态-内核态切换
2. **安全**：Hypervisor 代码在 TCB 内，与微内核共享 Rust 编译时安全保证
3. **调度集成**：vCPU 直接作为 CFS 调度器的调度实体，无需额外的调度层
4. **内存效率**：EPT/NPT 页表直接由内核内存管理器管理，避免重复映射

### 3.2 VMX Root 模式进入与退出

Intel VT-x 引入了 VMX（Virtual Machine Extensions）操作模式。CPU 有两种运行模式：

| 模式 | 描述 | 特权级别 |
|------|------|---------|
| **VMX Root** | Hypervisor 运行模式 | Ring 0 |
| **VMX Non-Root** | 虚拟机运行模式 | Ring 0-3（受限） |

```rust
/// VMX 模式管理器
pub struct VmxManager {
    /// 每个 CPU 的 VMXON 区域物理地址
    vmxon_regions: PerCpu<VmxonRegion>,
    /// VMX 是否已启用
    enabled: AtomicBool,
}

/// VMXON 区域（每个 CPU 一个，4KB 对齐）
#[repr(C, align(4096))]
pub struct VmxonRegion {
    /// VMXON 修订标识符
    revision_id: u32,
    /// 数据区域（由 CPU 使用）
    data: [u8; 4092],
}

impl VmxManager {
    /// 在当前 CPU 上启用 VMX 操作
    ///
    /// 步骤:
    /// 1. 检查 CPUID 是否支持 VMX
    /// 2. 在 CR4 中设置 VMXE 位 (bit 13)
    /// 3. 确保 IA32_FEATURE_CONTROL MSR 允许 VMX
    /// 4. 执行 VMXON 指令
    pub fn enable_vmx_on_cpu(&self) -> Result<(), HypervisorError> {
        // 步骤 1: 检查 VMX 支持
        let cpuid = unsafe { core::arch::x86_64::__cpuid(1) };
        if cpuid.ecx & (1 << 5) == 0 {
            return Err(HypervisorError::VmxNotSupported);
        }

        // 步骤 2: 启用 CR4.VMXE
        unsafe {
            let mut cr4 = x86_64::registers::control::Cr4::read();
            cr4.set(x86_64::registers::control::Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS, true);
            cr4.write();
        }

        // 步骤 3: 检查 IA32_FEATURE_CONTROL MSR
        let feature_control = unsafe {
            x86_64::registers::model_specific::Msr::new(0x3A).read()
        };
        if feature_control & (1 << 0) == 0 {
            // 锁定位未设置，需要 BIOS/UEFI 启用
            return Err(HypervisorError::VmxLockedByBios);
        }
        if feature_control & (1 << 2) == 0 {
            return Err(HypervisorError::VmxDisabledInFeatureControl);
        }

        // 步骤 4: 执行 VMXON
        let vmxon_region = self.vmxon_regions.current();
        let vmxon_pa = vmxon_region.phys_addr();
        let result = unsafe { vmxon(vmx_pa) };

        if result {
            Ok(())
        } else {
            Err(HypervisorError::VmxonFailed)
        }
    }

    /// 在当前 CPU 上禁用 VMX 操作
    pub fn disable_vmx_on_cpu(&self) {
        unsafe { vmxoff() };
    }
}

/// VMXON 指令封装
#[inline]
unsafe fn vmxon(physical_address: u64) -> bool {
    let result: u64;
    core::arch::asm!(
        "vmxon [{}]",
        in(reg) physical_address,
        out("rax") result,
        options(nostack)
    );
    // CF=0 表示成功
    result == 0
}

/// VMXOFF 指令封装
#[inline]
unsafe fn vmxoff() {
    core::arch::asm!("vmxoff", options(nostack));
}
```

### 3.3 VMCS (Virtual Machine Control Structure) 管理

VMCS 是 VMX 架构的核心数据结构，用于控制虚拟机的行为。每个 vCPU 拥有一个 VMCS。

```rust
/// VMCS 区域（4KB 对齐）
#[repr(C, align(4096))]
pub struct VmcsRegion {
    /// VMCS 修订标识符（必须与 IA32_VMX_BASIC MSR 匹配）
    revision_id: u32,
    /// VMCS 数据区域（由 CPU 管理，软件不直接访问）
    data: [u8; 4092],
}

/// VMCS 字段编码
pub mod vmcs_fields {
    // === 控制字段 (16-bit, 32-bit, 64-bit, natural-width) ===

    // 16-bit 控制字段
    pub const VIRTUAL_PROCESSOR_ID: u32 = 0x0000_0000;
    pub const POSTED_INTERRUPT_NV: u32  = 0x0000_0002;
    pub const EPTP_INDEX: u32           = 0x0000_0004;

    // 64-bit 控制字段
    pub const ADDRESS_OF_IO_BITMAP_A: u32        = 0x0000_2000;
    pub const ADDRESS_OF_IO_BITMAP_B: u32        = 0x0000_2002;
    pub const ADDRESS_OF_MSR_BITMAPS: u32        = 0x0000_2004;
    pub const VIRTUAL_APIC_ADDRESS: u32          = 0x0000_2012;
    pub const APIC_ACCESS_ADDRESS: u32           = 0x0000_2014;
    pub const EPT_POINTER: u32                   = 0x0000_201A;
    pub const VMCS_LINK_POINTER: u32             = 0x0000_2800;

    // 32-bit 控制字段
    pub const PIN_BASED_VM_EXEC_CONTROL: u32     = 0x0000_4000;
    pub const CPU_BASED_VM_EXEC_CONTROL: u32     = 0x0000_4002;
    pub const EXCEPTION_BITMAP: u32              = 0x0000_4004;
    pub const PAGE_FAULT_ERROR_CODE_MASK: u32    = 0x0000_4006;
    pub const PRIMARY_CPU_BASED_VM_EXEC_CONTROL: u32 = 0x0000_4002;
    pub const SECONDARY_CPU_BASED_VM_EXEC_CONTROL: u32 = 0x0000_401E;
    pub const VM_EXIT_CONTROLS: u32              = 0x0000_400C;
    pub const VM_ENTRY_CONTROLS: u32             = 0x0000_4012;
    pub const VM_ENTRY_INTR_INFO_FIELD: u32      = 0x0000_4016;

    // === 客机状态字段 ===
    pub const GUEST_RIP: u32                    = 0x0000_6800;
    pub const GUEST_RSP: u32                    = 0x0000_6802;
    pub const GUEST_RFLAGS: u32                 = 0x0000_6804;
    pub const GUEST_CR3: u32                    = 0x0000_6802;
    pub const GUEST_IA32_EFER: u32              = 0x0000_2806;

    // === 宿主状态字段 ===
    pub const HOST_RIP: u32                     = 0x0000_6C16;
    pub const HOST_RSP: u32                     = 0x0000_6C14;
    pub const HOST_CR3: u32                     = 0x0000_6C02;
    pub const HOST_IA32_EFER: u32               = 0x0000_2C02;
}

/// VMCS 操作封装
pub struct Vmcs {
    /// VMCS 物理地址
    phys_addr: PhysAddr,
    /// 当前加载状态
    loaded: AtomicBool,
    /// 所属 CPU
    cpu_id: usize,
}

impl Vmcs {
    /// 创建新的 VMCS 区域
    pub fn new(revision_id: u32) -> Result<Self, HypervisorError> {
        let region = VmcsRegion::allocate()?;
        region.revision_id = revision_id;

        Ok(Vmcs {
            phys_addr: region.phys_addr(),
            loaded: AtomicBool::new(false),
            cpu_id: 0,
        })
    }

    /// 加载 VMCS 到当前 CPU（VMCLEAR + VMPTRLD）
    pub fn load(&self) -> Result<(), HypervisorError> {
        // 先 VMPTRLD 加载 VMCS 指针
        unsafe { vmptrld(self.phys_addr.as_u64()) };
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// 写入 VMCS 字段
    pub fn write_field(&self, field: u32, value: u64) {
        unsafe { vmwrite(field, value) };
    }

    /// 读取 VMCS 字段
    pub fn read_field(&self, field: u32) -> u64 {
        unsafe { vmread(field) }
    }
}

#[inline]
unsafe fn vmwrite(field: u32, value: u64) {
    core::arch::asm!(
        "vmwrite {}, {}",
        in(reg) field,
        in(reg) value,
        options(nostack)
    );
}

#[inline]
unsafe fn vmread(field: u32) -> u64 {
    let value: u64;
    core::arch::asm!(
        "vmread {}, {}",
        out(reg) value,
        in(reg) field,
        options(nostack)
    );
    value
}
```

### 3.4 EPT/NPT 二级地址转换

EPT（Extended Page Tables，Intel）和 NPT（Nested Page Tables，AMD）实现了从客机虚拟地址（GVA）到物理地址的二级转换：

```
客机虚拟地址 (GVA)
    │  客机页表 (CR3) 转换
    ▼
客机物理地址 (GPA)
    │  EPT/NPT 转换
    ▼
宿主物理地址 (HPA)

对比传统单级转换:
虚拟地址 (VA) ──→ 物理地址 (PA)  [一次页表查询]

EPT 二级转换:
GVA ──(客机页表)──→ GPA ──(EPT)──→ HPA  [两次页表查询]
```

```rust
/// EPT 指针配置
#[repr(transparent)]
pub struct EptPointer(u64);

impl EptPointer {
    /// 创建 EPTP 值
    ///
    /// # 参数
    /// - `ept_root`: EPT 页表根物理地址 (4KB 对齐)
    /// - `memory_type`: 内存类型 (0=UC, 6=WB)
    /// - `page_walk_length`: 页表遍历层级 (3 表示 4 级页表)
    /// - `accessed_dirty`: 是否启用 A/D 位
    pub fn new(
        ept_root: PhysAddr,
        memory_type: EptMemoryType,
        page_walk_length: u8,
        accessed_dirty: bool,
    ) -> Self {
        let mut eptp = ept_root.as_u64();
        // bits [2:0] = 页表遍历层级 - 1
        eptp |= (page_walk_length as u64) & 0x7;
        // bit [6] = 是否启用 Accessed/Dirty 位
        if accessed_dirty {
            eptp |= 1 << 6;
        }
        // bits [5:3] = 内存类型
        eptp |= (memory_type as u64) << 3;
        EptPointer(eptp)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EptMemoryType {
    /// Uncacheable
    Uncacheable = 0,
    /// Write-Back
    WriteBack = 6,
}

/// EPT 页表项标志
pub struct EptEntryFlags: u64 {
    pub const READ:      u64 = 1 << 0;
    pub const WRITE:     u64 = 1 << 1;
    pub const EXECUTE:   u64 = 1 << 2;
    /// 该项指向下一级页表 (而非最终页)
    pub const LARGE_PAGE: u64 = 1 << 7;
    /// Accessed 位 (需 EPTP 启用)
    pub const ACCESSED:  u64 = 1 << 8;
    /// Dirty 位 (需 EPTP 启用)
    pub const DIRTY:     u64 = 1 << 9;
    /// 忽略 PAT (使用内存类型)
    pub const IGNORE_PAT: u64 = 1 << 6;
}
```

---

## 4. 虚拟机生命周期

### 4.1 生命周期状态机

```
                    ┌──────────┐
                    │  Created  │  VM 对象已创建，资源已分配
                    └────┬─────┘
                         │ configure()
                         ▼
                    ┌──────────┐
                    │ Configured│  VMCS 已初始化，EPT 已设置
                    └────┬─────┘
                         │ launch()
                         ▼
                    ┌──────────┐
              ┌────►│ Running  │  VMLAUNCH/VMRESUME 执行中
              │     └────┬─────┘
              │          │ pause()
              │          ▼
              │     ┌──────────┐
              │     │  Paused  │  VM 已暂停，vCPU 不调度
              │     └────┬─────┘
              │          │ resume()
              │          │
              │          └──────────┘
              │
              │     destroy()
              ▼
         ┌──────────┐
         │Destroyed │  资源已回收
         └──────────┘
```

### 4.2 VM 创建流程

```rust
/// 虚拟机配置
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// 虚拟机名称
    pub name: String,
    /// 虚拟机 ID
    pub vm_id: VmId,
    /// vCPU 数量
    pub num_vcpus: u32,
    /// 内存大小 (字节)
    pub memory_size: u64,
    /// 内存起始地址 (GPA)
    pub memory_base_gpa: u64,
    /// 内核镜像路径
    pub kernel_image: Option<String>,
    /// 初始内核命令行
    pub kernel_cmdline: Option<String>,
    /// initrd 镜像路径
    pub initrd_image: Option<String>,
    /// 是否启用嵌套虚拟化
    pub nested_virt: bool,
    /// CPU 亲和性 (vCPU → 物理CPU 映射)
    pub cpu_affinity: Option<Vec<usize>>,
    /// 虚拟设备列表
    pub devices: Vec<VirtualDeviceConfig>,
    /// 资源配额
    pub quota: VmResourceQuota,
    /// 所属 Agent ID
    pub owner_agent: Option<AgentId>,
}

/// 虚拟设备配置
#[derive(Debug, Clone)]
pub enum VirtualDeviceConfig {
    /// Virtio 块设备
    VirtioBlk {
        backend_path: String,
        readonly: bool,
    },
    /// Virtio 网络设备
    VirtioNet {
        mac_address: [u8; 6],
        backend_type: VirtioNetBackend,
    },
    /// Virtio GPU 设备
    VirtioGpu {
        resolution: (u32, u32),
        vram_size: u64,
    },
    /// PCI 设备直通
    PciPassthrough {
        bdf: PciBdf,
        iommu_group: u32,
    },
    /// 串口控制台
    SerialConsole {
        output_buffer_size: usize,
    },
}

/// VM 资源配额
#[derive(Debug, Clone)]
pub struct VmResourceQuota {
    /// 最大 CPU 使用率 (百分比, 0-100)
    pub max_cpu_percent: u32,
    /// 最大内存 (字节, 0 表示无限制)
    pub max_memory: u64,
    /// 最大磁盘 I/O 带宽 (bytes/s)
    pub max_disk_bw: u64,
    /// 最大网络带宽 (bytes/s)
    pub max_net_bw: u64,
    /// 最大 vCPU 数量
    pub max_vcpus: u32,
}
```

### 4.3 VM 结构定义

```rust
/// 虚拟机结构
pub struct Vm {
    /// 虚拟机 ID
    pub id: VmId,
    /// 虚拟机名称
    pub name: String,
    /// 虚拟机状态
    pub state: SpinLock<VmState>,
    /// 虚拟机配置
    pub config: VmConfig,
    /// vCPU 列表
    pub vcpus: SpinLock<Vec<Vcpu>>,
    /// EPT 页表
    pub ept: SpinLock<EptTable>,
    /// 虚拟设备列表
    pub devices: SpinLock<Vec<Box<dyn VirtioDevice>>>,
    /// 虚拟内存区域
    pub memory_regions: SpinLock<Vec<VmMemoryRegion>>,
    /// 所属 Agent
    pub owner_agent: Option<AgentId>,
    /// 创建时间
    pub created_at: u64,
    /// 运行统计
    pub stats: VmStats,
}

/// 虚拟机状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmState {
    /// 已创建，未配置
    Created,
    /// 已配置，VMCS 已初始化
    Configured,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 正在销毁
    Destroying,
    /// 已销毁
    Destroyed,
}

/// 虚拟 CPU
pub struct Vcpu {
    /// vCPU ID
    pub id: VcpuId,
    /// 所属 VM
    pub vm_id: VmId,
    /// VMCS
    pub vmcs: Vmcs,
    /// VPID (Virtual Processor ID)
    pub vpid: u16,
    /// 当前运行状态
    pub state: VcpuState,
    /// 绑定的物理 CPU
    pub pinned_cpu: Option<usize>,
    /// 保存的客机寄存器状态
    pub guest_regs: GuestRegisters,
    /// APIC 状态
    pub lapic: VirtualLapic,
    /// 上次 VM Exit 信息
    pub last_exit: Option<VmExitInfo>,
    /// Exit 统计
    pub exit_stats: VmExitStats,
}

/// vCPU 运行状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VcpuState {
    /// 未初始化
    Uninitialized,
    /// 就绪，等待调度
    Ready,
    /// 正在执行 (VMX non-root)
    Running,
    /// 已暂停
    Halted,
    /// 等待中断
    WaitInterrupt,
}

/// 客机寄存器状态
#[repr(C)]
pub struct GuestRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub efer: u64,
    pub cs: SegmentRegister,
    pub ds: SegmentRegister,
    pub es: SegmentRegister,
    pub fs: SegmentRegister,
    pub gs: SegmentRegister,
    pub ss: SegmentRegister,
    pub tr: SegmentRegister,
    pub ldtr: SegmentRegister,
    pub gdtr: DescriptorTableRegister,
    pub idtr: DescriptorTableRegister,
    pub msr_ia32_pat: u64,
    pub msr_ia32_sysenter_cs: u64,
    pub msr_ia32_sysenter_esp: u64,
    pub msr_ia32_sysenter_eip: u64,
    pub msr_star: u64,
    pub msr_lstar: u64,
    pub msr_cstar: u64,
    pub msr_sfmask: u64,
    pub msr_kernel_gs_base: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SegmentRegister {
    pub selector: u16,
    pub base: u64,
    pub limit: u32,
    pub attributes: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DescriptorTableRegister {
    pub limit: u16,
    pub base: u64,
}
```

### 4.4 VM 创建实现

```rust
impl Vm {
    /// 创建新的虚拟机
    pub fn create(config: VmConfig) -> Result<Self, HypervisorError> {
        // 1. 验证配置
        Self::validate_config(&config)?;

        // 2. 分配 VM ID
        let id = VM_ID_ALLOCATOR.lock().allocate()?;

        // 3. 初始化 EPT 页表
        let ept = EptTable::new()?;

        // 4. 分配客机物理内存
        let guest_memory = GuestMemoryAllocator::allocate(
            config.memory_base_gpa,
            config.memory_size,
        )?;

        // 5. 将客机内存映射到 EPT
        for region in &guest_memory.regions {
            ept.map_region(
                region.guest_phys_addr,
                region.host_phys_addr,
                region.size,
                EptEntryFlags::READ | EptEntryFlags::WRITE | EptEntryFlags::EXECUTE,
            )?;
        }

        // 6. 加载内核镜像到客机内存
        if let Some(ref kernel_path) = config.kernel_image {
            let kernel_data = load_image(kernel_path)?;
            guest_memory.write_gpa(config.memory_base_gpa, &kernel_data)?;
        }

        // 7. 创建 vCPU
        let mut vcpus = Vec::new();
        for i in 0..config.num_vcpus {
            let vcpu = Vcpu::new(
                VcpuId(i),
                id,
                config.cpu_affinity.as_ref().and_then(|a| a.get(i as usize)).copied(),
            )?;
            vcpus.push(vcpu);
        }

        let vm = Vm {
            id,
            name: config.name.clone(),
            state: SpinLock::new(VmState::Created),
            config,
            vcpus: SpinLock::new(vcpus),
            ept: SpinLock::new(ept),
            devices: SpinLock::new(Vec::new()),
            memory_regions: SpinLock::new(guest_memory.regions),
            owner_agent: None,
            created_at: current_timestamp_ns(),
            stats: VmStats::default(),
        };

        Ok(vm)
    }

    /// 配置虚拟机（初始化 VMCS）
    pub fn configure(&self) -> Result<(), HypervisorError> {
        let mut state = self.state.lock();
        if *state != VmState::Created {
            return Err(HypervisorError::InvalidState);
        }

        // 初始化每个 vCPU 的 VMCS
        let mut vcpus = self.vcpus.lock();
        for vcpu in vcpus.iter_mut() {
            vcpu.init_vmcs(&self.ept.lock(), &self.config)?;
        }

        // 初始化虚拟设备
        let mut devices = self.devices.lock();
        for device_config in &self.config.devices {
            let device: Box<dyn VirtioDevice> = match device_config {
                VirtualDeviceConfig::VirtioBlk { backend_path, readonly } => {
                    Box::new(VirtioBlkDevice::new(backend_path, *readonly)?)
                }
                VirtualDeviceConfig::VirtioNet { mac_address, backend_type } => {
                    Box::new(VirtioNetDevice::new(*mac_address, *backend_type)?)
                }
                VirtualDeviceConfig::VirtioGpu { resolution, vram_size } => {
                    Box::new(VirtioGpuDevice::new(*resolution, *vram_size)?)
                }
                _ => continue,
            };
            devices.push(device);
        }

        *state = VmState::Configured;
        Ok(())
    }

    /// 启动虚拟机
    pub fn launch(&self) -> Result<(), HypervisorError> {
        let mut state = self.state.lock();
        if *state != VmState::Configured {
            return Err(HypervisorError::InvalidState);
        }
        drop(state);

        let vcpus = self.vcpus.lock();
        for vcpu in vcpus.iter() {
            vcpu.launch()?;
        }

        *self.state.lock() = VmState::Running;
        Ok(())
    }

    /// 暂停虚拟机
    pub fn pause(&self) -> Result<(), HypervisorError> {
        let mut state = self.state.lock();
        if *state != VmState::Running {
            return Err(HypervisorError::InvalidState);
        }

        let vcpus = self.vcpus.lock();
        for vcpu in vcpus.iter() {
            vcpu.pause()?;
        }

        *state = VmState::Paused;
        Ok(())
    }

    /// 恢复虚拟机
    pub fn resume(&self) -> Result<(), HypervisorError> {
        let mut state = self.state.lock();
        if *state != VmState::Paused {
            return Err(HypervisorError::InvalidState);
        }

        let vcpus = self.vcpus.lock();
        for vcpu in vcpus.iter() {
            vcpu.resume()?;
        }

        *state = VmState::Running;
        Ok(())
    }

    /// 销毁虚拟机
    pub fn destroy(&self) -> Result<(), HypervisorError> {
        let mut state = self.state.lock();
        *state = VmState::Destroying;
        drop(state);

        // 1. 暂停所有 vCPU
        let vcpus = self.vcpus.lock();
        for vcpu in vcpus.iter() {
            vcpu.force_halt()?;
        }
        drop(vcpus);

        // 2. 回收 EPT 页表
        self.ept.lock().destroy();

        // 3. 回收客机物理内存
        for region in self.memory_regions.lock().iter() {
            GuestMemoryAllocator::free(region);
        }

        // 4. 销毁虚拟设备
        self.devices.lock().clear();

        // 5. 回收 VM ID
        VM_ID_ALLOCATOR.lock().free(self.id);

        *self.state.lock() = VmState::Destroyed;
        Ok(())
    }
}
```

### 4.5 VCPU 运行控制

```rust
impl Vcpu {
    /// 启动 vCPU（首次使用 VMLAUNCH）
    pub fn launch(&self) -> Result<(), HypervisorError> {
        self.vmcs.load()?;
        self.setup_initial_state()?;

        // 设置 RSP 指向 VM Exit 栈
        self.vmcs.write_field(vmcs_fields::HOST_RSP, self.exit_stack_top());

        let result = unsafe { vmlaunch() };
        if result {
            Ok(())
        } else {
            // VMLAUNCH 失败，读取失败原因
            let fail_reason = self.vmcs.read_field(0x4400); // VM_INSTRUCTION_ERROR
            Err(HypervisorError::VmlaunchFailed(fail_reason))
        }
    }

    /// 恢复 vCPU 执行（使用 VMRESUME）
    pub fn resume(&self) -> Result<(), HypervisorError> {
        self.vmcs.load()?;

        let result = unsafe { vmresume() };
        if result {
            Ok(())
        } else {
            let fail_reason = self.vmcs.read_field(0x4400);
            Err(HypervisorError::VmresumeFailed(fail_reason))
        }
    }

    /// 暂停 vCPU
    pub fn pause(&self) -> Result<(), HypervisorError> {
        // 通过发送 NMI 或 IPI 强制触发 VM Exit
        // 然后在 VM Exit 处理中将 vCPU 标记为 Paused
        Ok(())
    }
}

#[inline]
unsafe fn vmlaunch() -> bool {
    let result: u64;
    core::arch::asm!(
        "vmlaunch",
        out("rax") result,
        options(nostack)
    );
    result == 0
}

#[inline]
unsafe fn vmresume() -> bool {
    let result: u64;
    core::arch::asm!(
        "vmresume",
        out("rax") result,
        options(nostack)
    );
    result == 0
}
```

---

## 5. VM Exit 处理

### 5.1 VM Exit 原因分类

当虚拟机执行特定敏感指令或触发特定事件时，CPU 自动从 VMX non-root 模式切换到 VMX root 模式，产生 VM Exit。

```rust
/// VM Exit 信息
#[derive(Debug, Clone)]
pub struct VmExitInfo {
    /// Exit 原因
    pub reason: VmExitReason,
    /// Exit 限定符（取决于具体原因）
    pub qualification: u64,
    /// 中断信息（如果是由中断引起的 Exit）
    pub interrupt_info: u64,
    /// 中断错误码
    pub interrupt_error_code: u64,
    /// 指令长度
    pub instruction_length: u32,
    /// 客机指令指针
    pub guest_rip: u64,
    /// 客机 RFLAGS
    pub guest_rflags: u64,
    /// 客机线性地址（如 EPT 违例）
    pub guest_linear_address: u64,
    /// 客机物理地址（如 EPT 违例）
    pub guest_physical_address: u64,
}

/// VM Exit 原因（完整分类）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmExitReason {
    // === 基本退出 (0-7) ===
    /// 外部中断
    ExternalInterrupt,                          // 0
    /// 不可屏蔽中断 (NMI) 窗口
    NmiWindow,                                  // 1
    /// CPUID 指令
    Cpuid,                                      // 10
    /// HLT 指令
    Hlt,                                        // 12
    /// INVLPG 指令
    Invlpg,                                     // 14
    /// RDTSC/RDTSCP 指令
    Rdtsc,                                      // 16
    /// VMCALL 指令
    Vmcall,                                     // 18

    // === 控制寄存器访问 ===
    /// CR0-CR4 访问
    ControlRegister { cr: u8, is_write: bool }, // 28, 29, 30, 31, 44, 49
    /// MOV DR 指令
    DebugRegister,                              // 31

    // === I/O 操作 ===
    /// I/O 指令 (IN/OUT/INS/OUTS)
    IoInstruction {                             // 30
        port: u16,
        size: u8,
        is_write: bool,
        string_op: bool,
        rep_prefix: bool,
    },

    // === MSR 访问 ===
    /// RDMSR/WRMSR 指令
    MsrAccess {                                 // 31, 32
        msr: u32,
        is_write: bool,
    },

    // === 中断相关 ===
    /// 中断窗口
    InterruptWindow,                            // 7
    /// NMI 窗口（已包含在基本退出中）
    Nmi,                                        // 2

    // === 异常和 EPT ===
    /// EPT 违例
    EptViolation,                               // 48
    /// EPT 配置错误
    EptMisconfig,                               // 49
    /// 页表修改通知
    PageTableUpdate,                            // 51

    // === 其他 ===
    /// 三字节 opcode
    TripleFault,                                // 2
    /// 任务切换
    TaskSwitch,                                 // 9
    /// APIC 访问
    ApicAccess,                                 // 43
    /// APIC 写仿真
    ApicWrite,                                  // 44
    /// TPR 低于阈值
    TprBelowThreshold,                          // 43
    /// 预取指令
    Prefetch,                                   // 14
    /// 未知的 Exit 原因
    Unknown(u32),
}

impl VmExitReason {
    /// 从 VMCS 读取 Exit 原因
    pub fn from_vmcs(vmcs: &Vmcs) -> Self {
        let basic_reason = vmcs.read_field(0x4402) as u32; // VM_EXIT_REASON
        let qualification = vmcs.read_field(0x6400);       // EXIT_QUALIFICATION

        match basic_reason {
            0  => VmExitReason::ExternalInterrupt,
            1  => VmExitReason::NmiWindow,
            7  => VmExitReason::InterruptWindow,
            10 => VmExitReason::Cpuid,
            12 => VmExitReason::Hlt,
            18 => VmExitReason::Vmcall,
            30 => VmExitReason::IoInstruction {
                port: (qualification & 0xFFFF) as u16,
                size: ((qualification >> 16) & 0x7) as u8,
                is_write: (qualification & (1 << 3)) != 0,
                string_op: (qualification & (1 << 4)) != 0,
                rep_prefix: (qualification & (1 << 5)) != 0,
            },
            48 => VmExitReason::EptViolation,
            49 => VmExitReason::EptMisconfig,
            _  => VmExitReason::Unknown(basic_reason),
        }
    }
}
```

### 5.2 VM Exit 处理流程

```
vCPU 执行 (VMX non-root)
    │
    │ 触发 VM Exit 事件
    ▼
┌──────────────────────────────┐
│ CPU 自动保存客机状态到 VMCS   │  硬件自动完成
│ - 保存 RSP, RIP, RFLAGS      │
│ - 保存控制寄存器             │
│ - 保存段寄存器               │
│ - 加载宿主状态               │
│ - 跳转到 HOST_RIP            │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ VM Exit 入口存根 (汇编)       │  保存额外寄存器
│ - 保存 callee-saved 寄存器   │  切换到 Hypervisor 栈
│ - 读取 Exit 原因             │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ VmExitHandler::handle()      │  Rust 处理入口
│ - 解析 Exit 原因             │
│ - 分发到具体处理器           │
└──────────────┬───────────────┘
               │
       ┌───────┼───────┬───────────┬──────────┐
       ▼       ▼       ▼           ▼          ▼
   ┌──────┐┌──────┐┌──────┐┌──────────┐┌────────┐
   │ I/O  ││CPUID ││HLT   ││EPT Viol. ││VMCALL  │
   │模拟  ││模拟  ││处理  ││处理      ││处理    │
   └──┬───┘└──┬───┘└──┬───┘└────┬─────┘└───┬────┘
      │       │       │          │          │
      └───────┴───────┴──────────┴──────────┘
               │
               ▼
┌──────────────────────────────┐
│ 更新客机状态                 │  修改 RIP 跳过模拟指令
│ - 调整 RIP (instruction_len) │
│ - 注入中断/异常 (如需要)     │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ VMRESUME                     │  恢复客机执行
│ - 恢复客机寄存器             │
│ - 返回 VMX non-root 模式    │
└──────────────────────────────┘
```

### 5.3 I/O 指令拦截和模拟

```rust
/// I/O 指令模拟器
pub struct IoEmulator;

impl IoEmulator {
    /// 处理 I/O Exit
    pub fn handle_io(
        vcpu: &mut Vcpu,
        port: u16,
        size: u8,
        is_write: bool,
        qualification: u64,
    ) -> Result<IoAction, HypervisorError> {
        // 1. 检查是否为虚拟设备端口
        if let Some(device) = Self::find_virtio_device(vcpu, port) {
            return device.handle_io(port, size, is_write, vcpu);
        }

        // 2. 检查是否为允许直通的端口
        if Self::is_passthrough_port(port) {
            // 直接在宿主上执行 I/O 指令
            return Self::direct_io(port, size, is_write);
        }

        // 3. 默认行为：返回 0xFF (无设备)
        Ok(IoAction::Complete)
    }

    /// 直接执行 I/O 指令
    fn direct_io(port: u16, size: u8, is_write: bool) -> Result<IoAction, HypervisorError> {
        match size {
            1 => {
                if is_write {
                    let val = unsafe { get_guest_register_byte() };
                    unsafe { core::arch::x86_64::ports::outb(port, val) };
                } else {
                    let val = unsafe { core::arch::x86_64::ports::inb(port) };
                    unsafe { set_guest_register_byte(val) };
                }
            }
            2 => {
                if is_write {
                    let val = unsafe { get_guest_register_word() };
                    unsafe { core::arch::x86_64::ports::outw(port, val) };
                } else {
                    let val = unsafe { core::arch::x86_64::ports::inw(port) };
                    unsafe { set_guest_register_word(val) };
                }
            }
            4 => {
                if is_write {
                    let val = unsafe { get_guest_register_dword() };
                    unsafe { core::arch::x86_64::ports::outl(port, val) };
                } else {
                    let val = unsafe { core::arch::x86_64::ports::inl(port) };
                    unsafe { set_guest_register_dword(val) };
                }
            }
            _ => return Err(HypervisorError::InvalidIoSize(size)),
        }
        Ok(IoAction::Complete)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IoAction {
    /// I/O 操作已完成
    Complete,
    /// 需要更多处理（异步 I/O）
    Pending,
    /// I/O 错误
    Error(IoError),
}
```

### 5.4 中断虚拟化 (APICv / Posted Interrupts)

```rust
/// 虚拟 LAPIC (Local APIC)
pub struct VirtualLapic {
    /// APIC 寄存器映射 (4KB MMIO 空间)
    registers: [u64; 64],
    /// APIC ID
    pub apic_id: u8,
    /// 版本
    pub version: u32,
    /// 中断请求位图
    irr: u256,    // Interrupt Request Register
    /// 中断服务位图
    isr: u256,    // In-Service Register
    /// 触发模式寄存器
    tmr: u256,    // Trigger Mode Register
    /// 中断优先级
    pub tpr: u8,
    /// 逻辑目标模式
    pub logical_dest: u8,
    /// Spurious Interrupt Vector
    pub spurious_vector: u32,
}

/// Posted Interrupt 配置
pub struct PostedInterruptConfig {
    /// Posted Interrupt Descriptor 地址 (4KB 对齐)
    pub descriptor_addr: PhysAddr,
    /// Posted Interrupt Notification Vector
    pub notification_vector: u8,
    /// 是否启用
    pub enabled: bool,
}

/// Posted Interrupt Descriptor (16 字节)
#[repr(C)]
pub struct PostedInterruptDescriptor {
    /// PIR (Posted Interrupt Requests), 256 bits
    pub pir: [u64; 4],
    /// Outstanding Notification (ON) bit
    pub on: u64,
    /// Suppress Notification (SN) bit
    pub sn: u64,
}
```

---

## 6. 虚拟设备模型

### 6.1 Virtio 设备框架

OmniAgent OS 采用 Virtio 标准作为虚拟设备框架，支持以下设备类型：

```
┌─────────────────────────────────────────────────────────────┐
│                     虚拟机 (Guest)                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  Virtio Blk  │  │  Virtio Net │  │  Virtio GPU │         │
│  │  (前端驱动)  │  │  (前端驱动)  │  │  (前端驱动)  │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│         └────────────────┼────────────────┘                 │
│                          │                                  │
│                   Virtqueue (共享内存)                        │
│                   ┌──────────────┐                          │
│                   │ VRING:       │                          │
│                   │ - Descriptor │                          │
│                   │ - Available  │                          │
│                   │ - Used       │                          │
│                   └──────┬───────┘                          │
└──────────────────────────┼──────────────────────────────────┘
                           │ VM Exit (I/O 端口 / MMIO)
┌──────────────────────────┼──────────────────────────────────┐
│                     Hypervisor                               │
│                   ┌──────┴───────┐                          │
│                   │ Virtio 设备  │                          │
│                   │ 后端处理     │                          │
│                   └──────┬───────┘                          │
└──────────────────────────┼──────────────────────────────────┘
                           │ IPC / 系统调用
┌──────────────────────────┼──────────────────────────────────┐
│                  虚拟化管理器 (用户态)                         │
│  ┌─────────────┐  ┌──────┴──────┐  ┌─────────────┐         │
│  │  块设备后端  │  │  网络后端   │  │  GPU 后端   │         │
│  │  (文件/块)  │  │  (tap/桥)   │  │  (渲染)     │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 VirtioDevice Trait 定义

```rust
/// Virtio 设备特征位
pub mod virtio_features {
    pub const VIRTIO_F_NOTIFY_ON_EMPTY: u64 = 1 << 24;
    pub const VIRTIO_F_ANY_LAYOUT: u64      = 1 << 27;
    pub const VIRTIO_F_RING_INDIRECT_DESC: u64 = 1 << 28;
    pub const VIRTIO_F_RING_EVENT_IDX: u64  = 1 << 29;
    pub const VIRTIO_F_VERSION_1: u64       = 1 << 32;
    pub const VIRTIO_F_ACCESS_PLATFORM: u64 = 1 << 33;
    pub const VIRTIO_F_RING_PACKED: u64     = 1 << 34;
    pub const VIRTIO_F_IN_ORDER: u64        = 1 << 35;
    pub const VIRTIO_F_ORDER_PLATFORM: u64  = 1 << 36;
    pub const VIRTIO_F_SR_IOV: u64          = 1 << 37;
}

/// Virtio 设备配置空间
pub trait VirtioConfigSpace: Send + Sync {
    /// 读取配置空间
    fn read(&self, offset: u32, size: u32) -> u64;
    /// 写入配置空间
    fn write(&mut self, offset: u32, size: u32, value: u64);
}

/// Virtio 设备 Trait
pub trait VirtioDevice: Send + Sync {
    /// 设备类型 ID
    fn device_type(&self) -> u32;

    /// 设备名称
    fn device_name(&self) -> &str;

    /// 获取支持的特性位
    fn get_features(&self) -> u64;

    /// 设置协商后的特性位
    fn set_features(&mut self, features: u64);

    /// 获取设备状态
    fn get_status(&self) -> u8;

    /// 设置设备状态
    fn set_status(&mut self, status: u8);

    /// 获取配置空间
    fn config_space(&self) -> &dyn VirtioConfigSpace;

    /// 获取可变配置空间
    fn config_space_mut(&mut self) -> &mut dyn VirtioConfigSpace;

    /// 获取 Virtqueue 数量
    fn num_queues(&self) -> u16;

    /// 获取指定 Virtqueue 的大小
    fn queue_size(&self, queue_index: u16) -> u16;

    /// 激活 Virtqueue
    fn activate_queue(
        &mut self,
        queue_index: u16,
        descriptor_table_addr: u64,
        available_ring_addr: u64,
        used_ring_addr: u64,
    ) -> Result<(), HypervisorError>;

    /// 处理队列通知 (Guest 写入 queue notify)
    fn handle_queue_notify(&mut self, queue_index: u16) -> Result<(), HypervisorError>;

    /// 处理 I/O 端口访问
    fn handle_io(
        &mut self,
        port: u16,
        size: u8,
        is_write: bool,
        vcpu: &Vcpu,
    ) -> Result<IoAction, HypervisorError>;

    /// 重置设备
    fn reset(&mut self);
}

/// Virtio 设备类型 ID
pub mod virtio_device_types {
    pub const VIRTIO_DEV_NET: u32 = 1;
    pub const VIRTIO_DEV_BLK: u32 = 2;
    pub const VIRTIO_DEV_GPU: u32 = 16;
    pub const VIRTIO_DEV_INPUT: u32 = 18;
    pub const VIRTIO_DEV_VSOCK: u32 = 19;
    pub const VIRTIO_DEV_FS: u32 = 26;
    pub const VIRTIO_DEV_MEM: u32 = 29;
}
```

### 6.3 Virtio 块设备实现

```rust
/// Virtio 块设备
pub struct VirtioBlkDevice {
    /// 设备状态
    status: u8,
    /// 协商的特性位
    features: u64,
    /// 配置空间
    config: VirtioBlkConfig,
    /// 后端存储
    backend: Box<dyn BlockBackend>,
    /// Virtqueue (请求队列 + 可能的队列)
    queues: [Option<VirtQueue>; 2],
    /// I/O 端口基址
    port_base: u16,
}

/// Virtio 块设备配置空间
#[repr(C)]
pub struct VirtioBlkConfig {
    /// 容量 (扇区数)
    pub capacity: u64,
    /// 最大段大小
    pub size_max: u32,
    /// 最大段数
    pub seg_max: u32,
    /// 块大小
    pub blk_size: u32,
    /// 拓扑
    pub topology: VirtioBlkTopology,
    /// 写入模式
    pub writeback: u8,
    /// 未使用的配置字段
    _reserved: [u8; 3],
}

#[repr(C)]
pub struct VirtioBlkTopology {
    pub physical_block_exp: u8,
    pub alignment_offset: u8,
    pub min_io_size: u16,
    pub opt_io_size: u32,
}

/// 块设备后端 Trait
pub trait BlockBackend: Send + Sync {
    /// 读取块
    fn read_block(&self, offset: u64, data: &mut [u8]) -> Result<(), IoError>;
    /// 写入块
    fn write_block(&self, offset: u64, data: &[u8]) -> Result<(), IoError>;
    /// 获取设备容量 (字节)
    fn capacity(&self) -> u64;
    /// 刷新缓存
    fn flush(&self) -> Result<(), IoError>;
}

/// Virtio 块设备请求头
#[repr(C)]
pub struct VirtioBlkReqHeader {
    pub type_: u32,    // VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, etc.
    pub reserved: u32,
    pub sector: u64,   // 起始扇区
}

pub mod virtio_blk_types {
    pub const VIRTIO_BLK_T_IN: u32 = 0;
    pub const VIRTIO_BLK_T_OUT: u32 = 1;
    pub const VIRTIO_BLK_T_FLUSH: u32 = 4;
    pub const VIRTIO_BLK_T_GET_ID: u32 = 8;
    pub const VIRTIO_BLK_T_DISCARD: u32 = 11;
    pub const VIRTIO_BLK_T_WRITE_ZEROES: u32 = 13;
}

/// Virtio 块设备响应状态
pub mod virtio_blk_status {
    pub const VIRTIO_BLK_S_OK: u8 = 0;
    pub const VIRTIO_BLK_S_IOERR: u8 = 1;
    pub const VIRTIO_BLK_S_UNSUPP: u8 = 2;
}
```

### 6.4 Virtio 网络设备

```rust
/// Virtio 网络设备
pub struct VirtioNetDevice {
    status: u8,
    features: u64,
    config: VirtioNetConfig,
    backend: Box<dyn NetBackend>,
    queues: [Option<VirtQueue>; 6],  // RX(2) + TX(2) + Ctrl(2)
    mac_address: [u8; 6],
    port_base: u16,
}

/// Virtio 网络设备配置空间
#[repr(C)]
pub struct VirtioNetConfig {
    pub mac: [u8; 6],
    pub status: u16,
    pub max_virtqueue_pairs: u16,
    pub mtu: u16,
    pub speed: u32,
    pub duplex: u8,
    pub rss_max_key_size: u8,
    pub rss_max_data_size: u16,
    pub rss_hash_types: u32,
}

/// 网络后端类型
#[derive(Debug, Clone, Copy)]
pub enum VirtioNetBackend {
    /// TAP 设备
    Tap,
    /// 网桥
    Bridge { bridge_name: String },
    /// 用户态网络栈
    UserStack,
    /// 与宿主 Agent 共享网络
    AgentShared { agent_id: AgentId },
}

/// 网络后端 Trait
pub trait NetBackend: Send + Sync {
    /// 发送数据包
    fn send_packet(&self, packet: &[u8]) -> Result<(), IoError>;
    /// 接收数据包
    fn receive_packet(&self, buffer: &mut [u8]) -> Result<usize, IoError>;
    /// 获取 MAC 地址
    fn mac_address(&self) -> [u8; 6];
    /// 获取 MTU
    fn mtu(&self) -> u16;
}
```

### 6.5 设备直通 (PCI Passthrough / VFIO)

```rust
/// PCI BDF (Bus:Device:Function) 标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciBdf {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciBdf {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }

    /// 格式化为 "BB:DD.F" 字符串
    pub fn to_string(&self) -> String {
        format!("{:02X}:{:02X}.{}", self.bus, self.device, self.function)
    }
}

/// VFIO 设备容器
pub struct VfioContainer {
    /// 容器文件描述符
    fd: i32,
    /// IOMMU 组列表
    iommu_groups: Vec<VfioIommuGroup>,
    /// DMA 映射
    dma_mappings: SpinLock<Vec<DmaMapping>>,
}

/// VFIO IOMMU 组
pub struct VfioIommuGroup {
    /// 组 ID
    pub group_id: u32,
    /// 组文件描述符
    fd: i32,
    /// 组内的设备列表
    pub devices: Vec<PciBdf>,
}

/// DMA 映射
pub struct DmaMapping {
    /// IOVA (I/O Virtual Address)
    pub iova: u64,
    /// 物理地址
    pub phys_addr: PhysAddr,
    /// 大小
    pub size: u64,
    /// 是否可读写
    pub read_write: bool,
}

/// 设备直通管理器
pub struct PciPassthroughManager;

impl PciPassthroughManager {
    /// 将 PCI 设备直通到虚拟机
    pub fn assign_device(
        vm: &Vm,
        bdf: PciBdf,
    ) -> Result<(), HypervisorError> {
        // 1. 从宿主解绑设备
        Self::unbind_from_host(bdf)?;

        // 2. 通过 VFIO 获取设备
        let container = VfioContainer::new()?;
        let group = container.get_iommu_group(bdf)?;

        // 3. 配置 IOMMU
        Self::setup_iommu(&group, vm)?;

        // 4. 设置 EPT 映射 (设备 BAR 空间)
        Self::map_device_bar_spaces(vm, &group)?;

        // 5. 配置中断直通
        Self::setup_interrupt_passthrough(vm, &group)?;

        Ok(())
    }

    /// 从虚拟机回收设备
    pub fn deassign_device(
        vm: &Vm,
        bdf: PciBdf,
    ) -> Result<(), HypervisorError> {
        // 1. 停止设备
        // 2. 回收 IOMMU 映射
        // 3. 将设备重新绑定到宿主驱动
        Ok(())
    }
}
```

---

## 7. 内存虚拟化

### 7.1 EPT/NPT 二级页表管理

```rust
/// EPT 页表
pub struct EptTable {
    /// PML4 根表物理地址
    root_pml4: PhysAddr,
    /// PML4 根表虚拟地址（用于内核访问）
    root_pml4_virt: VirtAddr,
    /// 已映射的 GPA 范围
    mapped_ranges: SpinLock<Vec<EptMappedRange>>,
    /// 大页使用统计
    huge_page_stats: EptHugePageStats,
}

/// EPT 映射范围
pub struct EptMappedRange {
    pub gpa_start: u64,
    pub gpa_end: u64,
    pub hpa_start: u64,
    pub flags: EptEntryFlags,
}

/// EPT 大页统计
pub struct EptHugePageStats {
    /// 2MB 大页数量
    pub pages_2mb: AtomicU32,
    /// 1GB 大页数量
    pub pages_1gb: AtomicU32,
    /// 4KB 普通页数量
    pub pages_4kb: AtomicU32,
}

impl EptTable {
    /// 创建新的 EPT 页表
    pub fn new() -> Result<Self, HypervisorError> {
        let frame = frame_alloc_aligned(12)?; // 4KB 对齐
        let root_pml4 = frame.phys_addr();
        let root_pml4_virt = phys_to_virt(root_pml4);

        // 清零 PML4 表
        unsafe {
            core::ptr::write_bytes(root_pml4_virt.as_mut_ptr(), 0, 4096);
        }

        Ok(EptTable {
            root_pml4,
            root_pml4_virt,
            mapped_ranges: SpinLock::new(Vec::new()),
            huge_page_stats: EptHugePageStats {
                pages_2mb: AtomicU32::new(0),
                pages_1gb: AtomicU32::new(0),
                pages_4kb: AtomicU32::new(0),
            },
        })
    }

    /// 映射 GPA → HPA
    pub fn map_region(
        &self,
        gpa: u64,
        hpa: PhysAddr,
        size: u64,
        flags: EptEntryFlags,
    ) -> Result<(), HypervisorError> {
        let mut offset = 0u64;

        // 尝试使用大页映射
        while offset < size {
            let remaining = size - offset;
            let current_gpa = gpa + offset;
            let current_hpa = hpa.as_u64() + offset;

            // 检查是否可以使用 1GB 大页
            if remaining >= (1 << 30)
                && (current_gpa & ((1 << 30) - 1)) == 0
                && (current_hpa & ((1 << 30) - 1)) == 0
            {
                self.map_huge_page_1gb(current_gpa, PhysAddr::new(current_hpa), flags)?;
                self.huge_page_stats.pages_1gb.fetch_add(1, Ordering::Relaxed);
                offset += 1 << 30;
            }
            // 检查是否可以使用 2MB 大页
            else if remaining >= (1 << 21)
                && (current_gpa & ((1 << 21) - 1)) == 0
                && (current_hpa & ((1 << 21) - 1)) == 0
            {
                self.map_huge_page_2mb(current_gpa, PhysAddr::new(current_hpa), flags)?;
                self.huge_page_stats.pages_2mb.fetch_add(1, Ordering::Relaxed);
                offset += 1 << 21;
            }
            // 使用 4KB 页
            else {
                self.map_page_4kb(current_gpa, PhysAddr::new(current_hpa), flags)?;
                self.huge_page_stats.pages_4kb.fetch_add(1, Ordering::Relaxed);
                offset += 4096;
            }
        }

        self.mapped_ranges.lock().push(EptMappedRange {
            gpa_start: gpa,
            gpa_end: gpa + size,
            hpa_start: hpa.as_u64(),
            flags,
        });

        Ok(())
    }

    /// 解除 GPA 映射
    pub fn unmap_region(&self, gpa: u64, size: u64) -> Result<(), HypervisorError> {
        // 遍历并解除每一页的映射
        // 更新统计计数
        // INVEPT 刷新 EPT 缓存
        self.invalidate_ept();
        Ok(())
    }

    /// INVEPT: 刷新 EPT 缓存
    pub fn invalidate_ept(&self) {
        unsafe {
            // Single-context INVEPT (类型 1)
            let eptp = self.root_pml4.as_u64();
            core::arch::asm!(
                "invept {}, [%{}]",
                const 1,  // INVEPT_TYPE_SINGLE_CONTEXT
                in(reg) eptp,
                options(nostack)
            );
        }
    }

    /// 销毁 EPT 页表，回收所有页表页
    pub fn destroy(&self) {
        // 递归释放所有非叶子页表页
        self.free_page_tables(self.root_pml4_virt, 4);
    }
}
```

### 7.2 内存 Ballooning

```rust
/// 内存 Balloon 设备
pub struct MemoryBalloon {
    /// 当前 Balloon 大小 (页数)
    current_pages: AtomicU32,
    /// 目标 Balloon 大小 (页数)
    target_pages: AtomicU32,
    /// 已分配给 Balloon 的物理页列表
    inflated_pages: SpinLock<Vec<PhysAddr>>,
    /// 所属 VM
    vm_id: VmId,
}

impl MemoryBalloon {
    /// 膨胀 Balloon（从 Guest 回收内存）
    pub fn inflate(&self, num_pages: u32) -> Result<u32, HypervisorError> {
        let mut inflated = self.inflated_pages.lock();
        let mut actual = 0u32;

        for _ in 0..num_pages {
            // 1. 通知 Guest Balloon 驱动分配一页
            // 2. Guest 通过 Virtqueue 返回页的 GPA
            // 3. Hypervisor 获取对应的 HPA
            // 4. 在 EPT 中解除该 GPA 映射
            // 5. 将 HPA 加入 inflated_pages 列表
            // 6. HPA 可被宿主重新使用
            actual += 1;
        }

        self.current_pages.fetch_add(actual, Ordering::SeqCst);
        Ok(actual)
    }

    /// 收缩 Balloon（将内存归还 Guest）
    pub fn deflate(&self, num_pages: u32) -> Result<u32, HypervisorError> {
        let mut inflated = self.inflated_pages.lock();
        let mut actual = 0u32;

        for _ in 0..num_pages.min(inflated.len() as u32) {
            if let Some(hpa) = inflated.pop() {
                // 1. 重新分配 HPA 给 Guest
                // 2. 在 EPT 中恢复 GPA → HPA 映射
                // 3. 通知 Guest Balloon 驱动该页可用
                actual += 1;
            }
        }

        self.current_pages.fetch_sub(actual, Ordering::SeqCst);
        Ok(actual)
    }

    /// 获取当前 Balloon 大小
    pub fn current_size_kb(&self) -> u64 {
        self.current_pages.load(Ordering::SeqCst) as u64 * 4
    }
}
```

### 7.3 内存去重 (KSM-like)

```rust
/// 内核同页合并 (Kernel Same-page Merging)
pub struct KernelSamePageMerging {
    /// 页内容哈希表 (内容哈希 → 物理页列表)
    hash_table: RwLock<HashMap<u64, Vec<KsmPageEntry>>>,
    /// 是否启用
    enabled: AtomicBool,
    /// 扫描间隔 (毫秒)
    scan_interval_ms: u32,
    /// 合并统计
    stats: KsmStats,
}

/// KSM 页条目
pub struct KsmPageEntry {
    /// 物理页地址
    pub phys_addr: PhysAddr,
    /// 所属 VM
    pub vm_id: VmId,
    /// 对应的 GPA
    pub gpa: u64,
    /// 引用计数 (多少个 GPA 指向此页)
    pub ref_count: AtomicU32,
    /// 是否为合并页 (写时复制)
    pub is_merged: AtomicBool,
}

/// KSM 统计
pub struct KsmStats {
    /// 扫描的总页数
    pub pages_scanned: AtomicU64,
    /// 合并的页数
    pub pages_merged: AtomicU64,
    /// 节省的内存 (字节)
    pub pages_shared: AtomicU64,
}

impl KernelSamePageMerging {
    /// 扫描并合并相同页面
    pub fn scan_and_merge(&self) -> Result<u32, HypervisorError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(0);
        }

        let mut merged_count = 0u32;

        // 遍历所有 VM 的客机内存
        for vm in VM_TABLE.lock().iter() {
            let ept = vm.ept.lock();
            for region in vm.memory_regions.lock().iter() {
                // 逐页扫描
                for page_offset in (0..region.size).step_by(4096) {
                    let gpa = region.guest_phys_addr + page_offset;
                    let content_hash = self.compute_page_hash(gpa)?;

                    // 查找是否有相同内容的页
                    if let Some(existing) = self.find_matching_page(content_hash, vm.id, gpa) {
                        // 合并: 将多个 GPA 指向同一个 HPA
                        self.merge_pages(existing, vm.id, gpa)?;
                        merged_count += 1;
                    } else {
                        // 新条目
                        self.add_page_entry(content_hash, vm.id, gpa);
                    }
                }
            }
        }

        self.stats.pages_scanned.fetch_add(
            merged_count as u64, Ordering::Relaxed
        );
        self.stats.pages_merged.fetch_add(
            merged_count as u64, Ordering::Relaxed
        );

        Ok(merged_count)
    }

    /// 计算页面内容哈希
    fn compute_page_hash(&self, gpa: u64) -> Result<u64, HypervisorError> {
        // 通过 EPT 将 GPA 转换为 HPA
        // 读取 4KB 页面内容
        // 计算 xxHash 或 SHA-256
        Ok(0) // 简化
    }
}
```

### 7.4 大页支持

```rust
/// 大页管理器
pub struct HugePageManager {
    /// 2MB 大页池
    pages_2mb: SpinLock<HugePagePool>,
    /// 1GB 大页池
    pages_1gb: SpinLock<HugePagePool>,
}

/// 大页池
pub struct HugePagePool {
    /// 可用大页列表
    free_pages: Vec<PhysAddr>,
    /// 已使用大页数量
    used_count: u32,
    /// 总大页数量
    total_count: u32,
    /// 大页大小 (字节)
    page_size: u64,
}

impl HugePageManager {
    /// 分配 2MB 大页
    pub fn alloc_2mb(&self) -> Result<PhysAddr, HypervisorError> {
        let mut pool = self.pages_2mb.lock();
        pool.free_pages.pop()
            .ok_or(HypervisorError::OutOfMemory)
    }

    /// 分配 1GB 大页
    pub fn alloc_1gb(&self) -> Result<PhysAddr, HypervisorError> {
        let mut pool = self.pages_1gb.lock();
        pool.free_pages.pop()
            .ok_or(HypervisorError::OutOfMemory)
    }

    /// 释放大页
    pub fn free_2mb(&self, addr: PhysAddr) {
        let mut pool = self.pages_2mb.lock();
        pool.free_pages.push(addr);
        pool.used_count -= 1;
    }

    /// 释放 1GB 大页
    pub fn free_1gb(&self, addr: PhysAddr) {
        let mut pool = self.pages_1gb.lock();
        pool.free_pages.push(addr);
        pool.used_count -= 1;
    }

    /// 将 4KB 页合并为 2MB 大页 (如果连续)
    pub fn promote_to_2mb(&self, base_addr: PhysAddr) -> Result<(), HypervisorError> {
        // 验证 512 个连续的 4KB 页
        // 释放 4KB 页
        // 分配 2MB 大页
        // 更新 EPT 映射
        Ok(())
    }
}
```

---

## 8. Agent 与虚拟机的关系

### 8.1 Agent 创建和管理 VM

```rust
/// Agent 虚拟化管理接口
pub struct AgentVmManager;

impl AgentVmManager {
    /// Agent 创建虚拟机
    ///
    /// 系统调用: AGENT_VM_CREATE (530)
    pub fn create_vm(
        agent_id: AgentId,
        config: VmConfig,
    ) -> Result<VmId, HypervisorError> {
        // 1. 验证 Agent 权限
        Self::check_agent_capability(agent_id, Capability::CreateVm)?;

        // 2. 验证资源配额
        Self::check_quota(agent_id, &config)?;

        // 3. 创建 VM
        let mut config = config;
        config.owner_agent = Some(agent_id);
        let vm = Vm::create(config)?;

        // 4. 注册到全局 VM 表
        VM_TABLE.lock().insert(vm.id, vm.clone());

        // 5. 关联 Agent 和 VM
        AGENT_VM_TABLE.lock().insert(agent_id, vm.id);

        Ok(vm.id)
    }

    /// Agent 销毁虚拟机
    ///
    /// 系统调用: AGENT_VM_DESTROY (531)
    pub fn destroy_vm(
        agent_id: AgentId,
        vm_id: VmId,
    ) -> Result<(), HypervisorError> {
        // 1. 验证 Agent 拥有该 VM
        Self::verify_ownership(agent_id, vm_id)?;

        // 2. 销毁 VM
        let vm_table = VM_TABLE.lock();
        if let Some(vm) = vm_table.get(&vm_id) {
            vm.destroy()?;
        }

        // 3. 清理关联
        AGENT_VM_TABLE.lock().remove(&agent_id);
        vm_table.lock().remove(&vm_id);

        Ok(())
    }

    /// Agent 控制 VM
    ///
    /// 系统调用: AGENT_VM_CONTROL (532)
    pub fn control_vm(
        agent_id: AgentId,
        vm_id: VmId,
        command: VmControlCommand,
    ) -> Result<(), HypervisorError> {
        Self::verify_ownership(agent_id, vm_id)?;

        let vm_table = VM_TABLE.lock();
        let vm = vm_table.get(&vm_id)
            .ok_or(HypervisorError::VmNotFound)?;

        match command {
            VmControlCommand::Start => vm.launch()?,
            VmControlCommand::Pause => vm.pause()?,
            VmControlCommand::Resume => vm.resume()?,
            VmControlCommand::Snapshot { path } => vm.snapshot(path)?,
            VmControlCommand::Restore { path } => vm.restore(path)?,
        }

        Ok(())
    }
}

/// VM 控制命令
pub enum VmControlCommand {
    Start,
    Pause,
    Resume,
    Snapshot { path: String },
    Restore { path: String },
}
```

### 8.2 VM 内 Agent 与宿主 Agent 通信

```
┌─────────────────────────────────────────────────────────────┐
│                     宿主环境                                  │
│  ┌──────────────┐                          ┌──────────────┐ │
│  │  Host Agent A │◄──── virtio-vsock ────►│  Host Agent B │ │
│  │  (PID: 100)  │                          │  (PID: 200)  │ │
│  └──────┬───────┘                          └──────────────┘ │
│         │                                                   │
│         │ IPC                                                │
│         ▼                                                   │
│  ┌──────────────┐                                           │
│  │  Vsock 服务   │  用户态服务，管理 virtio-vsock 通道        │
│  └──────┬───────┘                                           │
└─────────┼───────────────────────────────────────────────────┘
          │ virtio-vsock (VM Exit → 后端处理)
┌─────────┼───────────────────────────────────────────────────┐
│         │              虚拟机 #1                              │
│  ┌──────▼───────┐                                           │
│  │  Guest Agent  │  在 VM 内运行的 Agent                      │
│  │  (vsock 客户端)│                                          │
│  └──────────────┘                                           │
└─────────────────────────────────────────────────────────────┘
```

```rust
/// Virtio-vsock 设备 (用于 VM 内外 Agent 通信)
pub struct VirtioVsockDevice {
    status: u8,
    features: u64,
    config: VirtioVsockConfig,
    /// 连接表
    connections: SpinLock<HashMap<VsockConnectionKey, VsockConnection>>,
    /// CID (Context ID)
    guest_cid: u32,
    host_cid: u32,
}

/// Virtio-vsock 配置
#[repr(C)]
pub struct VirtioVsockConfig {
    pub guest_cid: u32,
}

/// vsock 连接键
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct VsockConnectionKey {
    pub src_cid: u32,
    pub src_port: u32,
    pub dst_cid: u32,
    pub dst_port: u32,
}

/// vsock 连接
pub struct VsockConnection {
    pub key: VsockConnectionKey,
    pub state: VsockConnectionState,
    pub rx_buffer: VecDeque<u8>,
    pub tx_buffer: VecDeque<u8>,
    pub buffer_size: usize,
    pub peer_buf_alloc: u32,
    pub peer_fwd_cnt: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VsockConnectionState {
    Closed,
    Connecting,
    Connected,
    Disconnecting,
    Listen,
}
```

### 8.3 VM 快照与 Agent 迁移

```rust
/// VM 快照
pub struct VmSnapshot {
    /// 快照 ID
    pub id: SnapshotId,
    /// VM ID
    pub vm_id: VmId,
    /// 创建时间
    pub created_at: u64,
    /// 内存快照 (压缩的客机内存)
    pub memory: Vec<u8>,
    /// vCPU 状态快照
    pub vcpu_states: Vec<VcpuSnapshot>,
    /// 设备状态快照
    pub device_states: Vec<DeviceSnapshot>,
    /// EPT 页表快照
    pub ept_snapshot: EptSnapshot,
    /// 快照大小 (字节)
    pub size: u64,
}

/// vCPU 快照
pub struct VcpuSnapshot {
    pub vcpu_id: VcpuId,
    pub registers: GuestRegisters,
    pub lapic_state: VirtualLapic,
    pub vmcs_fields: HashMap<u32, u64>,
}

/// 设备状态快照
pub struct DeviceSnapshot {
    pub device_type: u32,
    pub state_data: Vec<u8>,
}

/// EPT 快照
pub struct EptSnapshot {
    pub mapped_ranges: Vec<EptMappedRange>,
    pub page_table_data: Vec<u8>,
}

impl Vm {
    /// 创建 VM 快照
    pub fn snapshot(&self, path: String) -> Result<SnapshotId, HypervisorError> {
        // 1. 暂停 VM
        self.pause()?;

        // 2. 保存 vCPU 状态
        let vcpu_states = self.vcpus.lock().iter()
            .map(|vcpu| VcpuSnapshot {
                vcpu_id: vcpu.id,
                registers: vcpu.guest_regs.clone(),
                lapic_state: vcpu.lapic.clone(),
                vmcs_fields: self.dump_vmcs_fields(vcpu),
            })
            .collect();

        // 3. 保存客机内存 (使用差异快照减少数据量)
        let memory = self.dump_guest_memory()?;

        // 4. 保存设备状态
        let device_states = self.devices.lock().iter()
            .map(|dev| DeviceSnapshot {
                device_type: dev.device_type(),
                state_data: dev.snapshot_state(),
            })
            .collect();

        // 5. 保存 EPT 快照
        let ept_snapshot = self.ept.lock().snapshot()?;

        let snapshot = VmSnapshot {
            id: SNAPSHOT_ID_ALLOCATOR.lock().allocate()?,
            vm_id: self.id,
            created_at: current_timestamp_ns(),
            memory,
            vcpu_states,
            device_states,
            ept_snapshot,
            size: 0,
        };

        // 6. 序列化并写入存储
        let snapshot_data = serialize_snapshot(&snapshot)?;
        storage_write(&path, &snapshot_data)?;

        // 7. 恢复 VM
        self.resume()?;

        Ok(snapshot.id)
    }

    /// 从快照恢复 VM
    pub fn restore(&self, path: String) -> Result<(), HypervisorError> {
        // 1. 读取快照数据
        let snapshot_data = storage_read(&path)?;
        let snapshot = deserialize_snapshot(&snapshot_data)?;

        // 2. 验证快照
        if snapshot.vm_id != self.id {
            return Err(HypervisorError::SnapshotMismatch);
        }

        // 3. 暂停 VM
        self.pause()?;

        // 4. 恢复客机内存
        self.restore_guest_memory(&snapshot.memory)?;

        // 5. 恢复 vCPU 状态
        for vcpu_snap in &snapshot.vcpu_states {
            let vcpus = self.vcpus.lock();
            if let Some(vcpu) = vcpus.iter().find(|v| v.id == vcpu_snap.vcpu_id) {
                vcpu.restore_state(vcpu_snap)?;
            }
        }

        // 6. 恢复 EPT
        self.ept.lock().restore(&snapshot.ept_snapshot)?;

        // 7. 恢复设备状态
        for dev_snap in &snapshot.device_states {
            let mut devices = self.devices.lock();
            if let Some(dev) = devices.iter_mut().find(|d| d.device_type() == dev_snap.device_type) {
                dev.restore_state(&dev_snap.state_data)?;
            }
        }

        // 8. 恢复 VM
        self.resume()?;

        Ok(())
    }
}
```

---

## 9. 安全隔离

### 9.1 VM 间隔离

```rust
/// VM 间隔离策略
pub struct VmIsolationPolicy {
    /// IOMMU 配置
    pub iommu: IommuConfig,
    /// EPT 隔离
    pub ept_isolation: EptIsolationConfig,
    /// 中断隔离
    pub interrupt_isolation: InterruptIsolationConfig,
}

/// IOMMU 配置 (Intel VT-d / AMD-Vi)
pub struct IommuConfig {
    /// 是否启用 IOMMU
    pub enabled: bool,
    /// DMA 保护模式
    pub dma_protection: DmaProtectionMode,
    /// 中断重映射
    pub interrupt_remapping: bool,
    /// 传递模式 (Pass-through)
    pub pass_through: bool,
}

/// DMA 保护模式
#[derive(Debug, Clone, Copy)]
pub enum DmaProtectionMode {
    /// 严格模式：所有 DMA 需经过 IOMMU 转换
    Strict,
    /// 宽松模式：允许部分 DMA 直通
    Relaxed,
    /// 禁用：不使用 DMA 保护（仅调试用）
    Disabled,
}

/// EPT 隔离配置
pub struct EptIsolationConfig {
    /// 每个 VM 独立的 EPT 页表
    pub independent_ept: bool,
    /// 是否启用 EPT 访问权限检查
    pub access_check: bool,
    /// 是否启用 EPT A/D 位跟踪
    pub accessed_dirty_tracking: bool,
}

/// 中断隔离配置
pub struct InterruptIsolationConfig {
    /// 中断重映射
    pub interrupt_remapping: bool,
    /// Posted Interrupt 隔离
    pub posted_interrupt_isolation: bool,
    /// 中断路由限制
    pub interrupt_routing_restriction: bool,
}

/// IOMMU 域管理
pub struct IommuDomain {
    /// 域 ID
    pub domain_id: u32,
    /// 所属 VM
    pub vm_id: Option<VmId>,
    /// DMA 映射表
    pub dma_mappings: SpinLock<Vec<DmaMapping>>,
    /// 页表根地址
    pub page_table_root: PhysAddr,
}

impl IommuDomain {
    /// 创建 VM 专用的 IOMMU 域
    pub fn create_for_vm(vm_id: VmId) -> Result<Self, HypervisorError> {
        let domain_id = IOMMU_DOMAIN_ALLOCATOR.lock().allocate()?;
        let page_table_root = Self::allocate_page_table()?;

        let domain = IommuDomain {
            domain_id,
            vm_id: Some(vm_id),
            dma_mappings: SpinLock::new(Vec::new()),
            page_table_root,
        };

        // 配置 IOMMU 硬件
        domain.configure_iommu_hardware()?;

        Ok(domain)
    }

    /// 添加 DMA 映射
    pub fn map_dma(
        &self,
        iova: u64,
        phys_addr: PhysAddr,
        size: u64,
        flags: DmaMapFlags,
    ) -> Result<(), HypervisorError> {
        // 在 IOMMU 页表中添加映射
        // 确保 DMA 只能访问 VM 分配的内存区域
        Ok(())
    }

    /// 移除 DMA 映射
    pub fn unmap_dma(&self, iova: u64, size: u64) -> Result<(), HypervisorError> {
        // 在 IOMMU 页表中移除映射
        // 刷新 IOTLB
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DmaMapFlags {
    pub read: bool,
    pub write: bool,
    pub no_snoop: bool,
}
```

### 9.2 VM 与宿主隔离

```rust
/// VM 与宿主隔离验证器
pub struct VmHostIsolationVerifier;

impl VmHostIsolationVerifier {
    /// 验证 VM 无法访问宿主内存
    pub fn verify_memory_isolation(vm: &Vm) -> Result<(), SecurityViolation> {
        let ept = vm.ept.lock();

        // 1. 检查 EPT 中不存在宿主内核空间映射
        for range in ept.mapped_ranges.lock().iter() {
            if range.hpa_start >= KERNEL_PHYS_START && range.hpa_start < KERNEL_PHYS_END {
                return Err(SecurityViolation::EptMapsHostMemory {
                    vm_id: vm.id,
                    hpa: range.hpa_start,
                });
            }
        }

        // 2. 检查 EPT 中不存在其他 VM 的内存映射
        let vm_table = VM_TABLE.lock();
        for (other_id, other_vm) in vm_table.iter() {
            if *other_id == vm.id {
                continue;
            }
            for other_range in other_vm.memory_regions.lock().iter() {
                for range in ept.mapped_ranges.lock().iter() {
                    if Self::ranges_overlap(
                        range.hpa_start, range.hpa_start + (other_range.size as u64),
                        other_range.guest_phys_addr,
                        other_range.guest_phys_addr + other_range.size,
                    ) {
                        return Err(SecurityViolation::EptMapsOtherVmMemory {
                            vm_id: vm.id,
                            other_vm_id: *other_id,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// 验证 VM 无法执行特权指令
    pub fn verify_privilege_isolation(vm: &Vm) -> Result<(), SecurityViolation> {
        let vcpus = vm.vcpus.lock();
        for vcpu in vcpus.iter() {
            // 检查 VMCS 中的执行控制
            let pin_ctrl = vcpu.vmcs.read_field(vmcs_fields::PIN_BASED_VM_EXEC_CONTROL);
            let cpu_ctrl = vcpu.vmcs.read_field(vmcs_fields::PRIMARY_CPU_BASED_VM_EXEC_CONTROL);

            // 确保敏感指令被拦截
            // - INVLPG, MOV CR, MOV DR, etc.
            // - IO 指令被拦截
            // - MSR 访问被拦截
        }
        Ok(())
    }

    fn ranges_overlap(
        start1: u64, end1: u64,
        start2: u64, end2: u64,
    ) -> bool {
        start1 < end2 && start2 < end1
    }
}

/// 安全违规类型
#[derive(Debug, Clone)]
pub enum SecurityViolation {
    /// EPT 映射了宿主内存
    EptMapsHostMemory { vm_id: VmId, hpa: u64 },
    /// EPT 映射了其他 VM 的内存
    EptMapsOtherVmMemory { vm_id: VmId, other_vm_id: VmId },
    /// VMCS 配置不安全
    VmcsMisconfigured { vm_id: VmId, field: u32 },
    /// IOMMU 配置不安全
    IommuMisconfigured { vm_id: VmId },
}
```

### 9.3 安全启动链延伸到 VM

```rust
/// VM 安全启动扩展
pub struct VmSecureBoot {
    /// 宿主启动度量值
    pub host_boot_hash: [u8; 32],
    /// VM 启动策略
    pub policy: VmBootPolicy,
}

/// VM 启动策略
#[derive(Debug, Clone, Copy)]
pub enum VmBootPolicy {
    /// 不验证（开发模式）
    Unverified,
    /// 验证内核签名
    VerifyKernel,
    /// 完整度量（内核 + initrd + cmdline）
    FullMeasurement,
    /// 远程证明
    RemoteAttestation,
}

impl VmSecureBoot {
    /// 验证 VM 内核镜像
    pub fn verify_kernel(
        &self,
        kernel_data: &[u8],
        expected_hash: &[u8; 32],
    ) -> Result<(), SecurityViolation> {
        match self.policy {
            VmBootPolicy::Unverified => Ok(()),
            VmBootPolicy::VerifyKernel | VmBootPolicy::FullMeasurement => {
                let actual_hash = Self::sha256(kernel_data);
                if actual_hash != *expected_hash {
                    return Err(SecurityViolation::KernelHashMismatch {
                        expected: *expected_hash,
                        actual: actual_hash,
                    });
                }
                Ok(())
            }
            VmBootPolicy::RemoteAttestation => {
                // 额外的远程证明流程
                self.verify_kernel(kernel_data, expected_hash)?;
                self.perform_remote_attestation()?;
                Ok(())
            }
        }
    }

    /// 计算启动度量值
    pub fn compute_boot_measurement(
        &self,
        kernel_data: &[u8],
        initrd_data: Option<&[u8]>,
        cmdline: &str,
    ) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.host_boot_hash);
        hasher.update(kernel_data);
        if let Some(initrd) = initrd_data {
            hasher.update(initrd);
        }
        hasher.update(cmdline.as_bytes());
        hasher.finalize()
    }

    fn sha256(data: &[u8]) -> [u8; 32] {
        // SHA-256 实现
        [0u8; 32]
    }

    fn perform_remote_attestation(&self) -> Result<(), SecurityViolation> {
        // 与远程证明服务交互
        Ok(())
    }
}
```

---

## 10. 性能优化

### 10.1 APICv / Posted Interrupts

APICv（APIC Virtualization）是 Intel 提供的硬件辅助中断虚拟化技术，显著减少中断注入导致的 VM Exit：

```rust
/// APICv 配置
pub struct ApicvConfig {
    /// 是否启用 APICv
    pub enabled: bool,
    /// 是否启用 Posted Interrupts
    pub posted_interrupts: bool,
    /// 是否启用 Virtual Interrupt Delivery
    pub virtual_interrupt_delivery: bool,
    /// 是否启用 APIC-register virtualization
    pub apic_register_virtualization: bool,
    /// 是否启用 Virtual-Processor ID (VPID)
    pub vpid: bool,
}

impl ApicvConfig {
    /// 从 CPU 功能自动检测最优配置
    pub fn auto_detect() -> Self {
        let cpuid = unsafe { core::arch::x86_64::__cpuid(1) };
        let vmx_cap = unsafe {
            x86_64::registers::model_specific::Msr::new(0x481).read()
        };

        ApicvConfig {
            enabled: true,
            posted_interrupts: (vmx_cap & (1 << 7)) != 0,
            virtual_interrupt_delivery: (vmx_cap & (1 << 9)) != 0,
            apic_register_virtualization: (vmx_cap & (1 << 15)) != 0,
            vpid: true,
        }
    }

    /// 应用到 VMCS 执行控制
    pub fn apply_to_vmcs(&self, vmcs: &Vmcs) {
        let secondary = vmcs.read_field(vmcs_fields::SECONDARY_CPU_BASED_VM_EXEC_CONTROL);

        let mut secondary = secondary;
        if self.virtual_interrupt_delivery {
            secondary |= 1 << 9;  // Virtual-interrupt delivery
        }
        if self.apic_register_virtualization {
            secondary |= 1 << 15; // APIC-register virtualization
        }
        if self.vpid {
            secondary |= 1 << 5;  // Enable VPID
        }

        vmcs.write_field(vmcs_fields::SECONDARY_CPU_BASED_VM_EXEC_CONTROL, secondary);
    }
}
```

### 10.2 VMCS Shadowing

VMCS Shadowing 允许 L1 Hypervisor 高效支持嵌套虚拟化：

```rust
/// VMCS Shadowing 配置
pub struct VmcsShadowingConfig {
    /// 是否启用
    pub enabled: bool,
    /// Shadow VMCS 位图地址
    pub shadow_bitmap_addr: Option<PhysAddr>,
}

/// 嵌套虚拟化支持
pub struct NestedVirtSupport {
    /// 是否启用嵌套虚拟化
    pub enabled: bool,
    /// L2 VMCS 列表
    l2_vmcss: SpinLock<HashMap<Vpid, Vmcs>>,
    /// VMCS Shadowing
    pub vmcs_shadowing: VmcsShadowingConfig,
}

impl NestedVirtSupport {
    /// 处理 L1 Hypervisor 的 VMLAUNCH/VMRESUME
    pub fn handle_l1_launch(
        &self,
        vcpu: &mut Vcpu,
    ) -> Result<(), HypervisorError> {
        if !self.enabled {
            return Err(HypervisorError::NestedVirtDisabled);
        }

        // 1. 读取 L1 设置的 VMCS 字段
        // 2. 合并 L1 和 L0 的 VMCS 设置
        // 3. 如果支持 VMCS Shadowing，使用 Shadow VMCS
        // 4. 执行 VMLAUNCH 进入 L2

        Ok(())
    }

    /// 处理 L2 的 VM Exit
    pub fn handle_l2_exit(
        &self,
        vcpu: &mut Vcpu,
        exit_info: &VmExitInfo,
    ) -> Result<VmExitAction, HypervisorError> {
        // 1. 判断 Exit 是否需要反射给 L1
        // 2. 如果需要反射，准备 L1 的 VM Exit 信息
        // 3. 返回到 L1 处理

        Ok(VmExitAction::ReflectToL1)
    }
}

pub enum VmExitAction {
    /// Hypervisor 直接处理
    HandleDirectly,
    /// 反射给 L1 Hypervisor
    ReflectToL1,
    /// 注入到 L2
    InjectToL2,
}
```

### 10.3 EPT Caching 策略

```rust
/// EPT 缓存策略
pub struct EptCachePolicy {
    /// 默认内存类型
    pub default_memory_type: EptMemoryType,
    /// MMIO 区域使用 UC (Uncacheable)
    pub mmio_uncacheable: bool,
    /// 帧缓冲区使用 WC (Write-Combining)
    pub framebuffer_write_combining: bool,
    /// 是否启用 EPT A/D 位 (用于页表扫描优化)
    pub accessed_dirty_bits: bool,
}

impl EptCachePolicy {
    /// 为特定 GPA 范围选择最优内存类型
    pub fn memory_type_for_range(
        &self,
        gpa: u64,
        size: u64,
        device_type: Option<u32>,
    ) -> EptMemoryType {
        match device_type {
            Some(virtio_device_types::VIRTIO_DEV_GPU) => {
                // GPU 帧缓冲区使用 Write-Combining
                if self.framebuffer_write_combining {
                    EptMemoryType::WriteBack // WC 在实际实现中
                } else {
                    EptMemoryType::WriteBack
                }
            }
            Some(_) if self.mmio_uncacheable => {
                // 设备 MMIO 区域使用 Uncacheable
                EptMemoryType::Uncacheable
            }
            _ => self.default_memory_type,
        }
    }
}
```

### 10.4 批量 I/O 处理

```rust
/// 批量 I/O 处理器
pub struct BatchIoProcessor {
    /// 待处理的 I/O 请求队列
    pending_ios: SpinLock<VecDeque<PendingIo>>,
    /// 批量处理阈值
    batch_threshold: usize,
    /// 批量处理超时 (微秒)
    batch_timeout_us: u64,
}

/// 待处理的 I/O 请求
pub struct PendingIo {
    pub vm_id: VmId,
    pub vcpu_id: VcpuId,
    pub port: u16,
    pub size: u8,
    pub is_write: bool,
    pub value: u64,
    pub timestamp: u64,
}

impl BatchIoProcessor {
    /// 添加 I/O 请求到批量队列
    pub fn enqueue_io(&self, io: PendingIo) -> BatchIoAction {
        let mut pending = self.pending_ios.lock();
        pending.push_back(io);

        if pending.len() >= self.batch_threshold {
            // 达到批量阈值，立即处理
            BatchIoAction::Flush
        } else {
            // 等待更多请求或超时
            BatchIoAction::Defer
        }
    }

    /// 执行批量 I/O 处理
    pub fn flush_batch(&self) -> Result<u32, HypervisorError> {
        let mut pending = self.pending_ios.lock();
        let count = pending.len() as u32;

        // 按设备和端口分组
        let mut grouped: HashMap<(VmId, u16), Vec<&PendingIo>> = HashMap::new();
        for io in pending.iter() {
            grouped.entry((io.vm_id, io.port))
                .or_default()
                .push(io);
        }

        // 批量处理每组 I/O
        for ((vm_id, port), ios) in grouped.iter() {
            // 对于 Virtio 设备，可以合并多个请求
            // 对于块设备，可以合并连续的读写
            self.process_batch_group(*vm_id, *port, ios)?;
        }

        pending.clear();
        Ok(count)
    }

    fn process_batch_group(
        &self,
        vm_id: VmId,
        port: u16,
        ios: &[&PendingIo],
    ) -> Result<(), HypervisorError> {
        // 根据设备类型选择批量处理策略
        Ok(())
    }
}

pub enum BatchIoAction {
    /// 立即刷新批量队列
    Flush,
    /// 延迟处理，等待更多请求
    Defer,
}
```

---

## 11. Rust 接口定义

### 11.1 Hypervisor Trait

```rust
/// Hypervisor 核心 Trait
pub trait Hypervisor: Send + Sync {
    /// 初始化 Hypervisor
    fn init(&self) -> Result<(), HypervisorError>;

    /// 关闭 Hypervisor
    fn shutdown(&self) -> Result<(), HypervisorError>;

    /// 创建虚拟机
    fn create_vm(&self, config: VmConfig) -> Result<VmId, HypervisorError>;

    /// 销毁虚拟机
    fn destroy_vm(&self, vm_id: VmId) -> Result<(), HypervisorError>;

    /// 获取虚拟机引用
    fn get_vm(&self, vm_id: VmId) -> Result<&Vm, HypervisorError>;

    /// 获取虚拟机可变引用
    fn get_vm_mut(&self, vm_id: VmId) -> Result<&mut Vm, HypervisorError>;

    /// 列出所有虚拟机
    fn list_vms(&self) -> Vec<VmId>;

    /// 获取 Hypervisor 能力信息
    fn capabilities(&self) -> &HypervisorCapabilities;

    /// 获取全局统计信息
    fn stats(&self) -> &HypervisorStats;
}

/// Hypervisor 能力信息
pub struct HypervisorCapabilities {
    /// 虚拟化类型
    pub virt_type: VirtualizationType,
    /// 最大 VM 数量
    pub max_vms: u32,
    /// 每个 VM 最大 vCPU 数量
    pub max_vcpus_per_vm: u32,
    /// 是否支持 EPT/NPT
    pub ept_supported: bool,
    /// 是否支持 VPID
    pub vpid_supported: bool,
    /// 是否支持 APICv
    pub apicv_supported: bool,
    /// 是否支持嵌套虚拟化
    pub nested_virt_supported: bool,
    /// 是否支持设备直通
    pub passthrough_supported: bool,
    /// 最大 EPT 大页级别
    pub max_ept_huge_page_level: u8,
}

/// 虚拟化类型
#[derive(Debug, Clone, Copy)]
pub enum VirtualizationType {
    /// Intel VT-x
    IntelVtx,
    /// AMD-V (SVM)
    AmdSvm,
}

/// Hypervisor 全局统计
#[derive(Debug, Default)]
pub struct HypervisorStats {
    /// 当前活跃 VM 数量
    pub active_vms: AtomicU32,
    /// 当前活跃 vCPU 数量
    pub active_vcpus: AtomicU32,
    /// 总 VM Exit 次数
    pub total_vm_exits: AtomicU64,
    /// VM Exit 延迟统计
    pub exit_latency_stats: LatencyStats,
    /// 内存使用统计
    pub memory_stats: HypervisorMemoryStats,
}

/// 延迟统计
#[derive(Debug, Default)]
pub struct LatencyStats {
    pub min_ns: AtomicU64,
    pub max_ns: AtomicU64,
    pub avg_ns: AtomicU64,
    pub count: AtomicU64,
}

/// Hypervisor 内存统计
#[derive(Debug, Default)]
pub struct HypervisorMemoryStats {
    /// EPT 页表使用量 (字节)
    pub ept_memory_used: AtomicU64,
    /// 客机内存总量 (字节)
    pub guest_memory_total: AtomicU64,
    /// Balloon 回收量 (字节)
    pub balloon_reclaimed: AtomicU64,
    /// KSM 节省量 (字节)
    pub ksm_saved: AtomicU64,
}
```

### 11.2 VmExit Handler Trait

```rust
/// VM Exit 处理器 Trait
pub trait VmExitHandler: Send + Sync {
    /// 处理 VM Exit
    ///
    /// 返回 VmExitAction 指示后续操作:
    /// - Resume: 使用 VMRESUME 恢复客机执行
    /// - Halt: 暂停 vCPU
    /// - Shutdown: 关闭 VM
    fn handle_exit(
        &self,
        vcpu: &mut Vcpu,
        exit_info: &VmExitInfo,
    ) -> Result<VmExitAction, HypervisorError>;

    /// 获取处理器名称（用于调试）
    fn name(&self) -> &str;
}

/// VM Exit 处理结果
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmExitAction {
    /// 恢复客机执行 (VMRESUME)
    Resume,
    /// 暂停 vCPU
    Halt,
    /// 关闭 VM
    Shutdown,
    /// 重启 VM
    Reset,
    /// 反射到 L1 (嵌套虚拟化)
    ReflectToL1,
}

/// 默认 VM Exit 分发器
pub struct DefaultVmExitDispatcher {
    /// 已注册的处理器列表
    handlers: Vec<Box<dyn VmExitHandler>>,
}

impl DefaultVmExitDispatcher {
    /// 创建默认分发器
    pub fn new() -> Self {
        let mut handlers: Vec<Box<dyn VmExitHandler>> = Vec::new();

        // 注册各类型 Exit 处理器
        handlers.push(Box::new(CpuidExitHandler));
        handlers.push(Box::new(HltExitHandler));
        handlers.push(Box::new(IoExitHandler::new()));
        handlers.push(Box::new(MsrExitHandler));
        handlers.push(Box::new(EptViolationHandler));
        handlers.push(Box::new(CrAccessHandler));
        handlers.push(Box::new(VmcallExitHandler));

        Self { handlers }
    }

    /// 分发 VM Exit 到对应处理器
    pub fn dispatch(
        &self,
        vcpu: &mut Vcpu,
        exit_info: &VmExitInfo,
    ) -> Result<VmExitAction, HypervisorError> {
        for handler in &self.handlers {
            let action = handler.handle_exit(vcpu, exit_info)?;
            match action {
                VmExitAction::Resume => return Ok(VmExitAction::Resume),
                VmExitAction::Halt => return Ok(VmExitAction::Halt),
                VmExitAction::Shutdown => return Ok(VmExitAction::Shutdown),
                VmExitAction::Reset => return Ok(VmExitAction::Reset),
                VmExitAction::ReflectToL1 => return Ok(VmExitAction::ReflectToL1),
            }
        }
        Err(HypervisorError::UnhandledExit(exit_info.reason.clone()))
    }
}

/// CPUID Exit 处理器
pub struct CpuidExitHandler;

impl VmExitHandler for CpuidExitHandler {
    fn handle_exit(
        &self,
        vcpu: &mut Vcpu,
        exit_info: &VmExitInfo,
    ) -> Result<VmExitAction, HypervisorError> {
        // 读取客机请求的 CPUID leaf
        let leaf = vcpu.guest_regs.rax;
        let subleaf = vcpu.guest_regs.rcx;

        // 模拟 CPUID 响应
        let result = match leaf {
            // 基本信息伪装
            0x0 => CpuidResult {
                eax: 0x16,  // 最大基本 leaf
                ebx: 0x756e6547, // "Genu"
                ecx: 0x6c65746e, // "ntel"
                edx: 0x49656e69, // "ineI"
            },
            // 处理器信息
            0x1 => {
                let mut cpuid = unsafe { core::arch::x86_64::__cpuid(1) };
                // 隐藏 Hypervisor 位，设置正确的特征
                cpuid
            }
            // Hypervisor 信息 (CPUID leaf 0x40000000)
            0x4000_0000 => CpuidResult {
                eax: 0x4000_0010, // 最大 hypervisor leaf
                ebx: 0x4f41534f, // "OASO" (OmniAgent OS)
                ecx: 0x4147494e, // "NIGA"
                edx: 0x0053544f, // "OTS\0"
            },
            _ => unsafe { core::arch::x86_64::__cpuid(leaf as u32) },
        };

        vcpu.guest_regs.rax = result.eax as u64;
        vcpu.guest_regs.rbx = result.ebx as u64;
        vcpu.guest_regs.rcx = result.ecx as u64;
        vcpu.guest_regs.rdx = result.edx as u64;

        // 跳过 CPUID 指令
        vcpu.guest_regs.rip += exit_info.instruction_length as u64;

        Ok(VmExitAction::Resume)
    }

    fn name(&self) -> &str { "CpuidExitHandler" }
}

/// HLT Exit 处理器
pub struct HltExitHandler;

impl VmExitHandler for HltExitHandler {
    fn handle_exit(
        &self,
        vcpu: &mut Vcpu,
        _exit_info: &VmExitInfo,
    ) -> Result<VmExitAction, HypervisorError> {
        // 检查是否有待处理的中断
        if vcpu.lapic.has_pending_interrupt() {
            // 有中断，注入并恢复
            vcpu.inject_pending_interrupt()?;
            Ok(VmExitAction::Resume)
        } else {
            // 无中断，暂停 vCPU 等待
            vcpu.state = VcpuState::WaitInterrupt;
            Ok(VmExitAction::Halt)
        }
    }

    fn name(&self) -> &str { "HltExitHandler" }
}

struct CpuidResult {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
}
```

### 11.3 完整类型定义汇总

```rust
/// VM ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VmId(pub u64);

/// vCPU ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VcpuId(pub u32);

/// 快照 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotId(pub u64);

/// Hypervisor 错误类型
#[derive(Debug, Clone)]
pub enum HypervisorError {
    /// VMX 不支持
    VmxNotSupported,
    /// VMX 被 BIOS 锁定
    VmxLockedByBios,
    /// VMX 在 FEATURE_CONTROL 中禁用
    VmxDisabledInFeatureControl,
    /// VMXON 失败
    VmxonFailed,
    /// VMLAUNCH 失败
    VmlaunchFailed(u64),
    /// VMRESUME 失败
    VmresumeFailed(u64),
    /// VMCS 操作失败
    VmcsError(u32),
    /// VM 未找到
    VmNotFound,
    /// VM 状态无效
    InvalidState,
    /// 内存不足
    OutOfMemory,
    /// I/O 错误
    IoError(IoError),
    /// I/O 大小无效
    InvalidIoSize(u8),
    /// 未处理的 Exit
    UnhandledExit(VmExitReason),
    /// 嵌套虚拟化禁用
    NestedVirtDisabled,
    /// 配额超限
    QuotaExceeded,
    /// 权限不足
    PermissionDenied,
    /// 设备不存在
    DeviceNotFound,
    /// 快照不匹配
    SnapshotMismatch,
    /// 内部错误
    InternalError(String),
    /// 安全违规
    SecurityViolation(SecurityViolation),
}

/// I/O 错误
#[derive(Debug, Clone)]
pub enum IoError {
    /// 设备无响应
    NoDevice,
    /// 传输错误
    TransferError,
    /// 超时
    Timeout,
    /// 参数无效
    InvalidParameter,
}

/// VM 运行统计
#[derive(Debug, Default)]
pub struct VmStats {
    /// 总运行时间 (纳秒)
    pub total_run_time_ns: AtomicU64,
    /// VM Exit 总次数
    pub total_exits: AtomicU64,
    /// 各类型 Exit 次数
    pub exit_counts: SpinLock<HashMap<VmExitReason, AtomicU64>>,
    /// I/O 操作次数
    pub io_count: AtomicU64,
    /// 内存使用峰值 (字节)
    pub memory_peak: AtomicU64,
}

/// VM Exit 统计
#[derive(Debug, Default)]
pub struct VmExitStats {
    /// 总 Exit 次数
    pub total: AtomicU64,
    /// I/O Exit 次数
    pub io_exits: AtomicU64,
    /// EPT 违例次数
    pub ept_violations: AtomicU64,
    /// 中断注入次数
    pub interrupt_injections: AtomicU64,
    /// 平均处理延迟 (纳秒)
    pub avg_latency_ns: AtomicU64,
}

/// VirtQueue
pub struct VirtQueue {
    /// 队列索引
    pub index: u16,
    /// 队列大小
    pub size: u16,
    /// 描述符表地址 (GPA)
    pub descriptor_table_gpa: u64,
    /// Available 环地址 (GPA)
    pub available_ring_gpa: u64,
    /// Used 环地址 (GPA)
    pub used_ring_gpa: u64,
    /// Available 环索引
    pub last_avail_idx: u16,
    /// Used 环索引
    pub last_used_idx: u16,
    /// 是否已激活
    pub ready: bool,
    /// 是否启用通知抑制
    pub notify_suppress: bool,
}
```

---

## 12. 错误处理和性能约束

### 12.1 错误处理策略

| 错误场景 | 处理策略 | 影响范围 |
|---------|---------|---------|
| VMXON 失败 | 记录错误，禁用虚拟化子系统 | 全局 |
| VMLAUNCH 失败 | 记录 VMCS 错误码，暂停 VM | 单个 VM |
| VM Exit 处理错误 | 注入 #GP 到客机，继续执行 | 单个 vCPU |
| EPT 违例循环 | 检测循环，暂停 VM | 单个 VM |
| 内存分配失败 | 暂停 VM，通知虚拟化管理器 | 单个 VM |
| 设备 I/O 错误 | 重试 3 次，然后报告给客机 | 单个设备 |
| 安全违规 | 立即暂停 VM，记录审计日志 | 单个 VM |

```rust
/// VM Exit 错误恢复策略
pub struct VmExitErrorHandler;

impl VmExitErrorHandler {
    /// 处理 VM Exit 处理过程中的错误
    pub fn handle_error(
        &self,
        vcpu: &mut Vcpu,
        error: HypervisorError,
    ) -> VmExitAction {
        match error {
            HypervisorError::UnhandledExit(reason) => {
                log::warn!(
                    "Unhandled VM exit {:?} on vCPU {:?} of VM {:?}",
                    reason, vcpu.id, vcpu.vm_id
                );
                // 注入 #GP (异常向量 13) 到客机
                vcpu.inject_exception(13, 0);
                VmExitAction::Resume
            }
            HypervisorError::IoError(e) => {
                log::warn!(
                    "I/O error {:?} on vCPU {:?} of VM {:?}",
                    e, vcpu.id, vcpu.vm_id
                );
                // 报告 I/O 错误给客机
                VmExitAction::Resume
            }
            HypervisorError::SecurityViolation(v) => {
                log::error!(
                    "Security violation {:?} on VM {:?}, shutting down",
                    v, vcpu.vm_id
                );
                // 安全违规：立即关闭 VM
                VmExitAction::Shutdown
            }
            _ => {
                log::error!(
                    "Fatal hypervisor error {:?} on vCPU {:?}",
                    error, vcpu.id
                );
                VmExitAction::Shutdown
            }
        }
    }

    /// 检测 EPT 违例循环
    pub fn detect_ept_loop(vcpu: &Vcpu) -> bool {
        let recent = &vcpu.exit_stats;
        // 如果最近 100 次 Exit 中 EPT 违例超过 90 次，判定为循环
        recent.ept_violations.load(Ordering::Relaxed) > 90
            && recent.total.load(Ordering::Relaxed) > 100
    }
}
```

### 12.2 性能约束

| 指标 | 目标值 | 测量方法 | 备注 |
|------|--------|---------|------|
| VM 创建延迟 | < 2s | 从 `create()` 到 `Running` | 含内存分配和设备初始化 |
| VM Exit 处理延迟 | < 50μs | 从 VM Exit 到 VMRESUME | P99 延迟 |
| I/O Exit 延迟 | < 10μs | 单次 IN/OUT 指令模拟 | 端口映射命中时 |
| EPT 违例处理 | < 20μs | 单次 EPT Violation | 含页表遍历 |
| VM 暂停延迟 | < 100ms | 从 `pause()` 到 `Paused` | 所有 vCPU 已停止 |
| VM 恢复延迟 | < 50ms | 从 `resume()` 到 `Running` | 含 EPT 刷新 |
| VM 快照创建 | < 5s (4GB 内存) | 含内存压缩 | 差异快照模式 |
| VM 快照恢复 | < 10s (4GB 内存) | 含内存解压 | |
| 内存开销 (每 VM) | < 64MB | EPT + 元数据 | 不含客机内存 |
| vCPU 调度开销 | < 5μs | CFS 调度切换 | 与普通线程相当 |

```rust
/// 性能监控器
pub struct HypervisorPerfMonitor {
    /// TSC 频率 (Hz)
    tsc_freq: u64,
    /// 性能计数器
    counters: SpinLock<PerfCounters>,
}

#[derive(Debug, Default)]
pub struct PerfCounters {
    /// VM Exit 延迟直方图 (桶: 0-1μs, 1-5μs, 5-10μs, 10-50μs, 50-100μs, >100μs)
    pub exit_latency_histogram: [AtomicU64; 6],
    /// I/O 吞吐量 (bytes/s)
    pub io_throughput: AtomicU64,
    /// 内存分配延迟
    pub alloc_latency_ns: AtomicU64,
    /// EPT 缺页率 (次/s)
    pub ept_fault_rate: AtomicU64,
}

impl HypervisorPerfMonitor {
    /// 记录 VM Exit 延迟
    pub fn record_exit_latency(&self, latency_ns: u64) {
        let counters = self.counters.lock();
        let bucket = match latency_ns {
            0..=1000 => 0,
            1001..=5000 => 1,
            5001..=10000 => 2,
            10001..=50000 => 3,
            50001..=100000 => 4,
            _ => 5,
        };
        counters.exit_latency_histogram[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> PerfReport {
        let counters = self.counters.lock();
        let total: u64 = counters.exit_latency_histogram.iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();

        PerfReport {
            total_exits: total,
            p50_bucket: self.percentile_bucket(&counters, 0.50),
            p99_bucket: self.percentile_bucket(&counters, 0.99),
            ept_fault_rate: counters.ept_fault_rate.load(Ordering::Relaxed),
        }
    }

    fn percentile_bucket(&self, counters: &PerfCounters, pct: f64) -> usize {
        let total: u64 = counters.exit_latency_histogram.iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();
        if total == 0 { return 0; }

        let target = (total as f64 * pct) as u64;
        let mut cumulative = 0u64;
        for (i, bucket) in counters.exit_latency_histogram.iter().enumerate() {
            cumulative += bucket.load(Ordering::Relaxed);
            if cumulative >= target {
                return i;
            }
        }
        5
    }
}

pub struct PerfReport {
    pub total_exits: u64,
    pub p50_bucket: usize,
    pub p99_bucket: usize,
    pub ept_fault_rate: u64,
}
```

---

## 13. 测试用例

### 13.1 单元测试

```rust
#[cfg(test)]
mod virt_tests {
    use super::*;

    // === Hypervisor 初始化测试 ===

    #[test]
    fn test_vmx_capability_detection() {
        let support = detect_virtualization_support();
        // 在支持 VT-x 的平台上应返回 IntelVtx 或 AmdSvm
        match support {
            VirtualizationSupport::None => {
                // 在不支持虚拟化的环境（如 CI）中跳过
                println!("VMX not supported on this platform, skipping");
            }
            VirtualizationSupport::IntelVtx { ept_supported, .. } => {
                assert!(ept_supported, "EPT should be supported on modern Intel CPUs");
            }
            VirtualizationSupport::AmdSvm { npt_supported, .. } => {
                assert!(npt_supported, "NPT should be supported on modern AMD CPUs");
            }
        }
    }

    #[test]
    fn test_vmcs_creation() {
        let revision_id = unsafe {
            x86_64::registers::model_specific::Msr::new(0x480).read() as u32
        };
        let vmcs = Vmcs::new(revision_id);
        assert!(vmcs.is_ok(), "VMCS creation should succeed");
    }

    // === EPT 测试 ===

    #[test]
    fn test_ept_table_creation() {
        let ept = EptTable::new();
        assert!(ept.is_ok(), "EPT table creation should succeed");
    }

    #[test]
    fn test_ept_4kb_mapping() {
        let ept = EptTable::new().unwrap();
        let frame = frame_alloc().unwrap();

        let result = ept.map_region(
            0x1000_0000,   // GPA
            frame.phys_addr(), // HPA
            4096,           // 4KB
            EptEntryFlags::READ | EptEntryFlags::WRITE | EptEntryFlags::EXECUTE,
        );
        assert!(result.is_ok(), "4KB EPT mapping should succeed");
    }

    #[test]
    fn test_ept_2mb_mapping() {
        let ept = EptTable::new().unwrap();
        let frame = frame_alloc_aligned(21).unwrap(); // 2MB 对齐

        let result = ept.map_region(
            0x2000_0000,   // GPA (2MB 对齐)
            frame.phys_addr(), // HPA (2MB 对齐)
            2 * 1024 * 1024, // 2MB
            EptEntryFlags::READ | EptEntryFlags::WRITE,
        );
        assert!(result.is_ok(), "2MB EPT mapping should succeed");
    }

    #[test]
    fn test_ept_unmap() {
        let ept = EptTable::new().unwrap();
        let frame = frame_alloc().unwrap();

        ept.map_region(0x1000_0000, frame.phys_addr(), 4096,
            EptEntryFlags::READ | EptEntryFlags::WRITE).unwrap();
        let result = ept.unmap_region(0x1000_0000, 4096);
        assert!(result.is_ok(), "EPT unmap should succeed");
    }

    // === VM 生命周期测试 ===

    #[test]
    fn test_vm_config_validation() {
        let valid_config = VmConfig {
            name: "test-vm".to_string(),
            vm_id: VmId(1),
            num_vcpus: 2,
            memory_size: 512 * 1024 * 1024, // 512MB
            memory_base_gpa: 0,
            kernel_image: None,
            kernel_cmdline: None,
            initrd_image: None,
            nested_virt: false,
            cpu_affinity: None,
            devices: Vec::new(),
            quota: VmResourceQuota {
                max_cpu_percent: 100,
                max_memory: 0,
                max_disk_bw: 0,
                max_net_bw: 0,
                max_vcpus: 4,
            },
            owner_agent: None,
        };

        // 验证配置不 panic
        let _ = valid_config.clone();
    }

    #[test]
    fn test_vm_state_transitions() {
        // 验证状态转换逻辑
        assert!(VmState::Created != VmState::Configured);
        assert!(VmState::Configured != VmState::Running);
        assert!(VmState::Running != VmState::Paused);
        assert!(VmState::Paused != VmState::Destroyed);
    }

    // === VM Exit 测试 ===

    #[test]
    fn test_vm_exit_reason_parsing() {
        // 测试 Exit 原因解析
        let test_cases = [
            (0u32, VmExitReason::ExternalInterrupt),
            (10u32, VmExitReason::Cpuid),
            (12u32, VmExitReason::Hlt),
            (18u32, VmExitReason::Vmcall),
            (48u32, VmExitReason::EptViolation),
            (49u32, VmExitReason::EptMisconfig),
        ];

        for (raw, expected) in test_cases {
            // 验证枚举值与原始编码的对应关系
            assert_eq!(expected, expected, "Exit reason {} should match", raw);
        }
    }

    #[test]
    fn test_io_emulator_port_decode() {
        // 测试 I/O Exit 限定符解码
        let qualification = 0x0000_0003_0000_0064; // port=0x64, size=1, write
        let port = (qualification & 0xFFFF) as u16;
        let size = ((qualification >> 16) & 0x7) as u8;
        let is_write = (qualification & (1 << 3)) != 0;

        assert_eq!(port, 0x64);
        assert_eq!(size, 1);
        assert!(is_write);
    }

    // === Virtio 设备测试 ===

    #[test]
    fn test_virtio_device_type_ids() {
        assert_eq!(virtio_device_types::VIRTIO_DEV_NET, 1);
        assert_eq!(virtio_device_types::VIRTIO_DEV_BLK, 2);
        assert_eq!(virtio_device_types::VIRTIO_DEV_GPU, 16);
    }

    #[test]
    fn test_virtio_blk_config_layout() {
        // 验证 VirtioBlkConfig 的内存布局
        assert_eq!(core::mem::size_of::<VirtioBlkConfig>(), 56);
        assert_eq!(core::mem::offset_of!(VirtioBlkConfig, capacity), 0);
        assert_eq!(core::mem::offset_of!(VirtioBlkConfig, blk_size), 20);
    }

    // === 内存虚拟化测试 ===

    #[test]
    fn test_memory_balloon_inflate_deflate() {
        let balloon = MemoryBalloon::new(VmId(1));
        assert_eq!(balloon.current_size_kb(), 0);

        // 膨胀
        let inflated = balloon.inflate(10).unwrap();
        assert_eq!(inflated, 10);
        assert_eq!(balloon.current_size_kb(), 40); // 10 * 4KB

        // 收缩
        let deflated = balloon.deflate(5).unwrap();
        assert_eq!(deflated, 5);
        assert_eq!(balloon.current_size_kb(), 20); // 5 * 4KB
    }

    #[test]
    fn test_pci_bdf_parsing() {
        let bdf = PciBdf::new(0x00, 0x1F, 0x02);
        assert_eq!(bdf.bus, 0x00);
        assert_eq!(bdf.device, 0x1F);
        assert_eq!(bdf.function, 0x02);
    }

    // === 安全隔离测试 ===

    #[test]
    fn test_ept_isolation_verification() {
        // 创建两个 VM 的 EPT，验证没有重叠映射
        let ept1 = EptTable::new().unwrap();
        let ept2 = EptTable::new().unwrap();

        // 映射不同的物理内存
        let frame1 = frame_alloc().unwrap();
        let frame2 = frame_alloc().unwrap();

        ept1.map_region(0x1000_0000, frame1.phys_addr(), 4096,
            EptEntryFlags::READ | EptEntryFlags::WRITE).unwrap();
        ept2.map_region(0x1000_0000, frame2.phys_addr(), 4096,
            EptEntryFlags::READ | EptEntryFlags::WRITE).unwrap();

        // 两个 EPT 映射到不同的 HPA，验证隔离
        let ranges1 = ept1.mapped_ranges.lock();
        let ranges2 = ept2.mapped_ranges.lock();
        assert_ne!(ranges1[0].hpa_start, ranges2[0].hpa_start);
    }

    // === 性能约束测试 ===

    #[test]
    fn test_vm_exit_latency_under_50us() {
        // 模拟 VM Exit 处理延迟测量
        let start = tsc_read();
        // 模拟 Exit 处理
        simulate_exit_handling();
        let elapsed_ns = tsc_to_ns(tsc_read() - start);

        assert!(elapsed_ns < 50_000,
            "VM Exit handling latency {}ns exceeds 50μs", elapsed_ns);
    }

    #[test]
    fn test_ept_mapping_latency() {
        let ept = EptTable::new().unwrap();
        let frame = frame_alloc().unwrap();

        let start = tsc_read();
        for i in 0..1000 {
            let gpa = 0x1000_0000 + (i as u64) * 4096;
            ept.map_region(gpa, frame.phys_addr(), 4096,
                EptEntryFlags::READ | EptEntryFlags::WRITE).unwrap();
        }
        let elapsed_ns = tsc_to_ns(tsc_read() - start);

        let avg_ns = elapsed_ns / 1000;
        assert!(avg_ns < 10_000,
            "Average EPT mapping latency {}ns exceeds 10μs", avg_ns);
    }
}
```

### 13.2 集成测试矩阵

| 测试类别 | 测试项 | 通过标准 | 优先级 |
|---------|--------|---------|--------|
| Hypervisor | VMX 初始化 | 成功进入 VMX root 模式 | P0 |
| Hypervisor | VMXON/VMXOFF | 循环 100 次无错误 | P0 |
| VM 生命周期 | VM 创建 | < 2s 完成 | P0 |
| VM 生命周期 | VM 启动 | 成功 VMLAUNCH | P0 |
| VM 生命周期 | VM 暂停/恢复 | 所有 vCPU 正确暂停 | P0 |
| VM 生命周期 | VM 销毁 | 资源完全回收 | P0 |
| VM Exit | CPUID 模拟 | 返回正确值 | P0 |
| VM Exit | HLT 处理 | 正确暂停/恢复 | P0 |
| VM Exit | I/O 模拟 | virtio 设备正常工作 | P0 |
| VM Exit | EPT Violation | 按需映射，无死循环 | P0 |
| VM Exit | VMCALL | 正确处理 Hypercall | P1 |
| 内存 | EPT 4KB 映射 | 正确映射/解除映射 | P0 |
| 内存 | EPT 2MB 大页 | 正确使用大页 | P1 |
| 内存 | Ballooning | 膨胀/收缩正确 | P1 |
| 内存 | KSM | 相同页面正确合并 | P2 |
| 设备 | virtio-blk | 读写正确，性能达标 | P0 |
| 设备 | virtio-net | 收发包正确 | P0 |
| 设备 | PCI Passthrough | 设备在 VM 内可用 | P1 |
| 安全 | VM 间隔离 | EPT + IOMMU 验证通过 | P0 |
| 安全 | 安全启动 | 内核哈希验证正确 | P1 |
| 性能 | VM Exit 延迟 | P99 < 50μs | P0 |
| 性能 | I/O 吞吐 | > 500 MB/s (virtio-blk) | P1 |
| 性能 | 网络吞吐 | > 1 Gbps (virtio-net) | P1 |
| Agent | Agent 创建 VM | 通过 syscall 成功 | P0 |
| Agent | VM 内外通信 | vsock 延迟 < 1ms | P1 |
| 快照 | VM 快照创建 | < 5s (4GB 内存) | P2 |
| 快照 | VM 快照恢复 | < 10s (4GB 内存) | P2 |

### 13.3 压力测试

```rust
#[cfg(test)]
mod stress_tests {
    use super::*;

    /// 多 VM 并发创建和销毁
    #[test]
    fn test_concurrent_vm_lifecycle() {
        let num_vms = 16;
        let mut handles = Vec::new();

        for i in 0..num_vms {
            handles.push(std::thread::spawn(move || {
                let config = VmConfig::minimal(i);
                let vm = Vm::create(config).unwrap();
                vm.configure().unwrap();
                vm.launch().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(100));
                vm.pause().unwrap();
                vm.destroy().unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 高频 VM Exit 压力测试
    #[test]
    fn test_high_frequency_vm_exits() {
        // 模拟每秒 100,000 次 VM Exit
        let iterations = 100_000;
        let start = tsc_read();

        for _ in 0..iterations {
            simulate_vm_exit();
        }

        let elapsed_ms = tsc_to_ms(tsc_read() - start);
        let exits_per_sec = iterations as f64 / (elapsed_ms as f64 / 1000.0);

        assert!(exits_per_sec > 50_000.0,
            "VM Exit throughput {} exits/s below 50,000", exits_per_sec);
    }

    /// 内存压力测试：多 VM 同时分配大量内存
    #[test]
    fn test_memory_pressure() {
        let num_vms = 8;
        let memory_per_vm = 256 * 1024 * 1024; // 256MB

        for i in 0..num_vms {
            let config = VmConfig {
                memory_size: memory_per_vm,
                ..VmConfig::minimal(i)
            };
            let vm = Vm::create(config).unwrap();
            // 验证内存分配成功
            assert_eq!(vm.memory_regions.lock().len(), 1);
        }
    }
}
```

---

## 14. 与微内核设计规范的交叉引用

本文档与微内核设计规范（`microkernel-design.md`）的以下章节直接相关：

| 微内核章节 | 本文档章节 | 关系 |
|-----------|-----------|------|
| 第 2 节 微内核哲学 | 第 3 节 Hypervisor 架构 | Hypervisor 作为内核态扩展，遵循最小化 TCB 原则 |
| 第 3.1 节 进程抽象 | 第 4.3 节 VM 结构定义 | `ProcessType::VirtualMachine` 与 `Vm` 结构对应 |
| 第 4 节 线程管理 | 第 4.5 节 vCPU 运行控制 | vCPU 作为 CFS 调度实体 |
| 第 5 节 系统调用接口 | 第 8.1 节 Agent VM 管理 | AGENT_VM_CREATE/DESTROY/CONTROL 系统调用 |
| 第 6 节 中断处理 | 第 5.4 节 中断虚拟化 | 中断向量 38 用于 VM Exit |
| 第 7 节 设备驱动框架 | 第 6 节 虚拟设备模型 | 用户态驱动模型延伸到虚拟设备后端 |
| 第 8 节 引导序列 | 第 4.2 节 VM 创建流程 | 虚拟化管理器在引导阶段 4 启动 |
| 第 9 节 虚拟化支持 | 全文 | 本文是该节的完整展开 |
| 第 10 节 错误处理 | 第 12.1 节 错误处理策略 | VM Exit 错误与内核 Oops 机制集成 |
| 第 11 节 内核内存布局 | 第 7 节 内存虚拟化 | EPT 映射与内核物理内存布局协调 |

---

*本文档由 OmniAgent OS 内核团队维护，如有疑问请联系 kernel@omniagent.os*
