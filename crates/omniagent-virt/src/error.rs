// 虚拟化错误类型定义

use crate::vm::VmState;

/// 虚拟化错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtError {
    /// 虚拟机未找到
    VmNotFound(u64),
    /// 无效的状态转换
    InvalidState { current: VmState, target: VmState },
    /// 虚拟机已存在
    VmAlreadyExists(String),
    /// vCPU 未找到
    VcpuNotFound(u32),
    /// 无效配置
    InvalidConfig(String),
    /// 内存不足
    NoMemory { requested: u64, available: u64 },
    /// I/O 错误
    IoError(String),
    /// 设备错误
    DeviceError(String),
    /// Hypervisor 不可用
    HypervisorNotAvailable,
    /// VM Exit 错误
    VmExitError(String),
    /// 设备未找到
    DeviceNotFound(String),
    /// 配额超限
    QuotaExceeded(String),
}

impl std::fmt::Display for VirtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VirtError::VmNotFound(id) => write!(f, "虚拟机未找到: {}", id),
            VirtError::InvalidState { current, target } => {
                write!(f, "无效的状态转换: {:?} -> {:?}", current, target)
            }
            VirtError::VmAlreadyExists(name) => write!(f, "虚拟机已存在: {}", name),
            VirtError::VcpuNotFound(id) => write!(f, "vCPU 未找到: {}", id),
            VirtError::InvalidConfig(msg) => write!(f, "无效配置: {}", msg),
            VirtError::NoMemory { requested, available } => {
                write!(f, "内存不足: 请求 {}MB, 可用 {}MB", requested, available)
            }
            VirtError::IoError(msg) => write!(f, "I/O 错误: {}", msg),
            VirtError::DeviceError(msg) => write!(f, "设备错误: {}", msg),
            VirtError::HypervisorNotAvailable => write!(f, "Hypervisor 不可用"),
            VirtError::VmExitError(msg) => write!(f, "VM Exit 错误: {}", msg),
            VirtError::DeviceNotFound(name) => write!(f, "设备未找到: {}", name),
            VirtError::QuotaExceeded(msg) => write!(f, "配额超限: {}", msg),
        }
    }
}

impl std::error::Error for VirtError {}
