// OmniAgent OS Phase 11: 虚拟化支持 (KVM) 框架
//
// 本 crate 实现了虚拟化管理框架，包括：
// - VM 生命周期管理
// - vCPU 管理
// - VM Exit 处理
// - Virtio 设备框架
// - VM 管理器

pub mod error;
pub mod vm;
pub mod vcpu;
pub mod vmexit;
pub mod virtio;
pub mod manager;

// 重新导出核心类型
pub use error::VirtError;
pub use vm::{
    VmState, VmConfig, VmResourceQuota, VirtualMachine,
    BootDevice, NetworkInterfaceConfig, NetworkBackend,
    BlockDeviceConfig, DiskFormat, VmResourceUsage,
};
pub use vcpu::{VcpuState, GuestRegisters, Vcpu};
pub use vmexit::{
    VmExitReason, VmExitInfo, IoOperation, ExitAction,
    IoPortHandler, IoEmulator, VmExitHandler,
};
pub use virtio::{
    VirtioDeviceType, VirtioFeatures, VirtqDesc, VirtqAvailable,
    VirtqUsed, VirtqUsedElem, VirtQueue, VirtioDevice,
    VirtioBlockDevice, VirtioNetDevice,
};
pub use manager::VmManager;
