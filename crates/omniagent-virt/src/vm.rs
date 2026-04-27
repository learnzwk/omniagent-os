// VM 生命周期管理
//
// 定义虚拟机的状态、配置、资源配额等核心类型

use crate::error::VirtError;
use crate::vcpu::VcpuState;

/// VM 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VmState {
    /// 已创建
    Created = 0,
    /// 已配置
    Configured = 1,
    /// 运行中
    Running = 2,
    /// 已暂停
    Paused = 3,
    /// 保存中
    Saving = 4,
    /// 恢复中
    Restoring = 5,
    /// 已销毁
    Destroyed = 6,
    /// 失败
    Failed = 7,
}

impl VmState {
    /// 获取合法的状态转换目标列表
    pub fn allowed_transitions(&self) -> &'static [VmState] {
        match self {
            VmState::Created => &[VmState::Configured, VmState::Destroyed, VmState::Failed],
            VmState::Configured => &[VmState::Running, VmState::Destroyed, VmState::Failed],
            VmState::Running => &[VmState::Paused, VmState::Saving, VmState::Destroyed, VmState::Failed],
            VmState::Paused => &[VmState::Running, VmState::Destroyed, VmState::Failed],
            VmState::Saving => &[VmState::Paused, VmState::Destroyed, VmState::Failed],
            VmState::Restoring => &[VmState::Running, VmState::Destroyed, VmState::Failed],
            VmState::Destroyed => &[],
            VmState::Failed => &[VmState::Destroyed],
        }
    }
}

/// 启动设备
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootDevice {
    /// 磁盘启动
    Disk { path: String },
    /// 光驱启动
    Cdrom { path: String },
    /// 网络启动
    Network,
    /// 固件启动
    Firmware,
}

/// 网络后端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NetworkBackend {
    /// SLIRP 用户态网络
    User = 0,
    /// TAP 设备
    Tap = 1,
    /// veth pair
    Veth = 2,
    /// 网桥
    Bridge = 3,
}

/// 网络接口配置
#[derive(Debug, Clone)]
pub struct NetworkInterfaceConfig {
    /// 接口 ID
    pub id: String,
    /// 网络后端
    pub backend: NetworkBackend,
    /// MAC 地址
    pub mac_address: Option<[u8; 6]>,
    /// 主机端口映射
    pub host_port: Option<u16>,
}

/// 磁盘格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DiskFormat {
    /// 原始格式
    Raw = 0,
    /// QCOW2 格式
    Qcow2 = 1,
    /// VMDK 格式
    Vmdk = 2,
    /// VDI 格式
    Vdi = 3,
}

/// 块设备配置
#[derive(Debug, Clone)]
pub struct BlockDeviceConfig {
    /// 设备 ID
    pub id: String,
    /// 设备路径
    pub path: String,
    /// 是否只读
    pub readonly: bool,
    /// 磁盘格式
    pub format: DiskFormat,
    /// 大小（MB）
    pub size_mb: Option<u64>,
}

/// VM 配置
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// 虚拟机名称
    pub name: String,
    /// vCPU 数量
    pub vcpu_count: u32,
    /// 内存大小（MB）
    pub memory_mb: u64,
    /// 内核路径
    pub kernel_path: Option<String>,
    /// initrd 路径
    pub initrd_path: Option<String>,
    /// 内核命令行
    pub kernel_cmdline: Option<String>,
    /// 启动设备列表
    pub boot_devices: Vec<BootDevice>,
    /// 网络接口列表
    pub network_interfaces: Vec<NetworkInterfaceConfig>,
    /// 块设备列表
    pub block_devices: Vec<BlockDeviceConfig>,
    /// 是否启用 virtio
    pub enable_virtio: bool,
    /// 是否启用 APIC 虚拟化
    pub enable_apicv: bool,
    /// 所属 Agent ID
    pub owner_agent: Option<u64>,
}

impl VmConfig {
    /// 创建默认 VM 配置
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            vcpu_count: 1,
            memory_mb: 512,
            kernel_path: None,
            initrd_path: None,
            kernel_cmdline: None,
            boot_devices: Vec::new(),
            network_interfaces: Vec::new(),
            block_devices: Vec::new(),
            enable_virtio: true,
            enable_apicv: true,
            owner_agent: None,
        }
    }

    /// 验证配置是否合法
    pub fn validate(&self) -> Result<(), VirtError> {
        if self.vcpu_count == 0 {
            return Err(VirtError::InvalidConfig("vCPU 数量不能为 0".to_string()));
        }
        if self.memory_mb == 0 {
            return Err(VirtError::InvalidConfig("内存大小不能为 0".to_string()));
        }
        if self.name.is_empty() {
            return Err(VirtError::InvalidConfig("虚拟机名称不能为空".to_string()));
        }
        Ok(())
    }
}

/// VM 资源配额
#[derive(Debug, Clone)]
pub struct VmResourceQuota {
    /// 最大 vCPU 总数
    pub max_vcpus: u32,
    /// 最大内存总量（MB）
    pub max_memory_mb: u64,
    /// 最大块设备数
    pub max_block_devices: u32,
    /// 最大网络接口数
    pub max_network_interfaces: u32,
    /// CPU 时间百分比
    pub cpu_time_percent: u32,
    /// I/O 带宽（MB/s）
    pub io_bandwidth_mb: u64,
    /// 网络带宽（MB/s）
    pub network_bandwidth_mb: u64,
}

impl Default for VmResourceQuota {
    fn default() -> Self {
        Self {
            max_vcpus: 64,
            max_memory_mb: 131072, // 128GB
            max_block_devices: 256,
            max_network_interfaces: 64,
            cpu_time_percent: 100,
            io_bandwidth_mb: 1024,
            network_bandwidth_mb: 1024,
        }
    }
}

/// 虚拟机
pub struct VirtualMachine {
    /// 虚拟机 ID
    pub id: u64,
    /// 虚拟机配置
    pub config: VmConfig,
    /// 当前状态
    pub state: VmState,
    /// 创建时间（毫秒时间戳）
    pub created_at: u64,
    /// 运行时间（毫秒）
    pub uptime_ms: u64,
    /// vCPU 状态列表
    pub vcpu_states: Vec<VcpuState>,
}

impl VirtualMachine {
    /// 创建新的虚拟机
    pub fn new(id: u64, config: VmConfig) -> Self {
        let vcpu_count = config.vcpu_count as usize;
        Self {
            id,
            config,
            state: VmState::Created,
            created_at: 0, // 由调用者设置
            uptime_ms: 0,
            vcpu_states: vec![VcpuState::Idle; vcpu_count],
        }
    }

    /// 状态转换
    pub fn transition(&mut self, new_state: VmState) -> Result<(), VirtError> {
        let allowed = self.state.allowed_transitions();
        if allowed.contains(&new_state) {
            self.state = new_state;
            Ok(())
        } else {
            Err(VirtError::InvalidState {
                current: self.state,
                target: new_state,
            })
        }
    }

    /// 更新运行时间
    pub fn update_uptime(&mut self, delta_ms: u64) {
        self.uptime_ms += delta_ms;
    }

    /// 获取 vCPU 数量
    pub fn vcpu_count(&self) -> u32 {
        self.config.vcpu_count
    }
}

/// VM 资源使用情况
#[derive(Debug, Clone)]
pub struct VmResourceUsage {
    /// 总 vCPU 数
    pub total_vcpus: u32,
    /// 总内存（MB）
    pub total_memory_mb: u64,
    /// VM 数量
    pub vm_count: usize,
    /// 运行中的 VM 数量
    pub running_count: usize,
    /// 资源配额
    pub quota: VmResourceQuota,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_state_transitions() {
        // Created -> Configured 是合法的
        assert!(VmState::Created.allowed_transitions().contains(&VmState::Configured));
        // Configured -> Running 是合法的
        assert!(VmState::Configured.allowed_transitions().contains(&VmState::Running));
        // Running -> Paused 是合法的
        assert!(VmState::Running.allowed_transitions().contains(&VmState::Paused));
        // Paused -> Running 是合法的
        assert!(VmState::Paused.allowed_transitions().contains(&VmState::Running));
        // Destroyed 没有合法的转换
        assert!(VmState::Destroyed.allowed_transitions().is_empty());
        // Running -> Created 是不合法的
        assert!(!VmState::Running.allowed_transitions().contains(&VmState::Created));
    }

    #[test]
    fn test_vm_config_new() {
        let config = VmConfig::new("test-vm");
        assert_eq!(config.name, "test-vm");
        assert_eq!(config.vcpu_count, 1);
        assert_eq!(config.memory_mb, 512);
        assert!(config.enable_virtio);
        assert!(config.enable_apicv);
        assert!(config.boot_devices.is_empty());
        assert!(config.network_interfaces.is_empty());
        assert!(config.block_devices.is_empty());
    }

    #[test]
    fn test_vm_config_validate() {
        let mut config = VmConfig::new("test");
        assert!(config.validate().is_ok());

        config.vcpu_count = 0;
        assert!(config.validate().is_err());

        config.vcpu_count = 1;
        config.memory_mb = 0;
        assert!(config.validate().is_err());

        config.memory_mb = 512;
        config.name = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_virtual_machine_new() {
        let config = VmConfig::new("test-vm");
        let vm = VirtualMachine::new(1, config);
        assert_eq!(vm.id, 1);
        assert_eq!(vm.state, VmState::Created);
        assert_eq!(vm.uptime_ms, 0);
        assert_eq!(vm.vcpu_count(), 1);
        assert_eq!(vm.vcpu_states.len(), 1);
    }

    #[test]
    fn test_virtual_machine_transition() {
        let config = VmConfig::new("test-vm");
        let mut vm = VirtualMachine::new(1, config);

        // 合法转换
        assert!(vm.transition(VmState::Configured).is_ok());
        assert_eq!(vm.state, VmState::Configured);

        assert!(vm.transition(VmState::Running).is_ok());
        assert_eq!(vm.state, VmState::Running);

        assert!(vm.transition(VmState::Paused).is_ok());
        assert_eq!(vm.state, VmState::Paused);

        // 不合法转换
        assert!(vm.transition(VmState::Created).is_err());
    }

    #[test]
    fn test_virtual_machine_update_uptime() {
        let config = VmConfig::new("test-vm");
        let mut vm = VirtualMachine::new(1, config);
        assert_eq!(vm.uptime_ms, 0);

        vm.update_uptime(1000);
        assert_eq!(vm.uptime_ms, 1000);

        vm.update_uptime(500);
        assert_eq!(vm.uptime_ms, 1500);
    }

    #[test]
    fn test_virtual_machine_vcpu_count() {
        let mut config = VmConfig::new("test-vm");
        config.vcpu_count = 4;
        let vm = VirtualMachine::new(1, config);
        assert_eq!(vm.vcpu_count(), 4);
        assert_eq!(vm.vcpu_states.len(), 4);
    }

    #[test]
    fn test_boot_device_equality() {
        let d1 = BootDevice::Disk { path: "/dev/sda".to_string() };
        let d2 = BootDevice::Disk { path: "/dev/sda".to_string() };
        let d3 = BootDevice::Disk { path: "/dev/sdb".to_string() };
        assert_eq!(d1, d2);
        assert_ne!(d1, d3);
    }

    #[test]
    fn test_network_backend_repr() {
        assert_eq!(NetworkBackend::User as u8, 0);
        assert_eq!(NetworkBackend::Tap as u8, 1);
        assert_eq!(NetworkBackend::Veth as u8, 2);
        assert_eq!(NetworkBackend::Bridge as u8, 3);
    }

    #[test]
    fn test_disk_format_repr() {
        assert_eq!(DiskFormat::Raw as u8, 0);
        assert_eq!(DiskFormat::Qcow2 as u8, 1);
        assert_eq!(DiskFormat::Vmdk as u8, 2);
        assert_eq!(DiskFormat::Vdi as u8, 3);
    }

    #[test]
    fn test_vm_resource_quota_default() {
        let quota = VmResourceQuota::default();
        assert_eq!(quota.max_vcpus, 64);
        assert_eq!(quota.max_memory_mb, 131072);
        assert_eq!(quota.max_block_devices, 256);
        assert_eq!(quota.max_network_interfaces, 64);
        assert_eq!(quota.cpu_time_percent, 100);
        assert_eq!(quota.io_bandwidth_mb, 1024);
        assert_eq!(quota.network_bandwidth_mb, 1024);
    }

    #[test]
    fn test_vm_state_repr() {
        assert_eq!(VmState::Created as u8, 0);
        assert_eq!(VmState::Configured as u8, 1);
        assert_eq!(VmState::Running as u8, 2);
        assert_eq!(VmState::Paused as u8, 3);
        assert_eq!(VmState::Saving as u8, 4);
        assert_eq!(VmState::Restoring as u8, 5);
        assert_eq!(VmState::Destroyed as u8, 6);
        assert_eq!(VmState::Failed as u8, 7);
    }
}
