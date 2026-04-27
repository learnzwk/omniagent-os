// VM 管理器
//
// 管理多个虚拟机的创建、配置、启动、暂停、恢复和销毁

use std::collections::HashMap;

use crate::error::VirtError;
use crate::vm::{VmConfig, VmResourceQuota, VmResourceUsage, VmState, VirtualMachine};

/// VM 管理器
pub struct VmManager {
    /// 虚拟机映射表
    vms: HashMap<u64, VirtualMachine>,
    /// 下一个 VM ID
    next_vm_id: u64,
    /// 资源配额
    quota: VmResourceQuota,
    /// 总 vCPU 数
    total_vcpus: u32,
    /// 总内存（MB）
    total_memory_mb: u64,
}

impl VmManager {
    /// 创建新的 VM 管理器
    pub fn new(quota: VmResourceQuota) -> Self {
        Self {
            vms: HashMap::new(),
            next_vm_id: 1,
            quota,
            total_vcpus: 0,
            total_memory_mb: 0,
        }
    }

    /// 创建 VM
    pub fn create_vm(&mut self, config: VmConfig) -> Result<u64, VirtError> {
        // 验证配置
        config.validate()?;

        // 检查名称唯一性
        for vm in self.vms.values() {
            if vm.config.name == config.name {
                return Err(VirtError::VmAlreadyExists(config.name.clone()));
            }
        }

        // 检查资源配额
        if self.total_vcpus + config.vcpu_count > self.quota.max_vcpus {
            return Err(VirtError::QuotaExceeded(format!(
                "vCPU 总数超限: 当前 {}, 请求 {}, 上限 {}",
                self.total_vcpus, config.vcpu_count, self.quota.max_vcpus
            )));
        }

        if self.total_memory_mb + config.memory_mb > self.quota.max_memory_mb {
            return Err(VirtError::QuotaExceeded(format!(
                "内存总量超限: 当前 {}MB, 请求 {}MB, 上限 {}MB",
                self.total_memory_mb, config.memory_mb, self.quota.max_memory_mb
            )));
        }

        let id = self.next_vm_id;
        self.next_vm_id += 1;

        let vm = VirtualMachine::new(id, config.clone());
        self.total_vcpus += config.vcpu_count;
        self.total_memory_mb += config.memory_mb;
        self.vms.insert(id, vm);

        Ok(id)
    }

    /// 配置 VM
    pub fn configure_vm(&mut self, id: u64) -> Result<(), VirtError> {
        let vm = self.vms.get_mut(&id).ok_or(VirtError::VmNotFound(id))?;
        vm.transition(VmState::Configured)
    }

    /// 启动 VM
    pub fn start_vm(&mut self, id: u64) -> Result<(), VirtError> {
        let vm = self.vms.get_mut(&id).ok_or(VirtError::VmNotFound(id))?;
        vm.transition(VmState::Running)
    }

    /// 暂停 VM
    pub fn pause_vm(&mut self, id: u64) -> Result<(), VirtError> {
        let vm = self.vms.get_mut(&id).ok_or(VirtError::VmNotFound(id))?;
        vm.transition(VmState::Paused)
    }

    /// 恢复 VM
    pub fn resume_vm(&mut self, id: u64) -> Result<(), VirtError> {
        let vm = self.vms.get_mut(&id).ok_or(VirtError::VmNotFound(id))?;
        vm.transition(VmState::Running)
    }

    /// 销毁 VM
    pub fn destroy_vm(&mut self, id: u64) -> Result<(), VirtError> {
        // 先获取可变引用进行状态转换
        let vm = self.vms.get_mut(&id).ok_or(VirtError::VmNotFound(id))?;
        vm.transition(VmState::Destroyed)?;

        // 释放资源
        let vm = self.vms.remove(&id).unwrap();
        self.total_vcpus -= vm.config.vcpu_count;
        self.total_memory_mb -= vm.config.memory_mb;

        Ok(())
    }

    /// 获取 VM
    pub fn get_vm(&self, id: u64) -> Option<&VirtualMachine> {
        self.vms.get(&id)
    }

    /// 列出所有 VM
    pub fn list_vms(&self) -> Vec<&VirtualMachine> {
        self.vms.values().collect()
    }

    /// VM 数量
    pub fn vm_count(&self) -> usize {
        self.vms.len()
    }

    /// 获取资源使用情况
    pub fn resource_usage(&self) -> VmResourceUsage {
        let running_count = self.vms.values().filter(|vm| vm.state == VmState::Running).count();
        VmResourceUsage {
            total_vcpus: self.total_vcpus,
            total_memory_mb: self.total_memory_mb,
            vm_count: self.vms.len(),
            running_count,
            quota: self.quota.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(name: &str, vcpus: u32, memory_mb: u64) -> VmConfig {
        let mut config = VmConfig::new(name);
        config.vcpu_count = vcpus;
        config.memory_mb = memory_mb;
        config
    }

    #[test]
    fn test_vm_manager_create() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let id = manager.create_vm(make_config("vm1", 2, 1024)).unwrap();
        assert_eq!(id, 1);

        let vm = manager.get_vm(id).unwrap();
        assert_eq!(vm.config.name, "vm1");
        assert_eq!(vm.state, VmState::Created);
    }

    #[test]
    fn test_vm_manager_create_multiple() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let id1 = manager.create_vm(make_config("vm1", 1, 512)).unwrap();
        let id2 = manager.create_vm(make_config("vm2", 2, 1024)).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(manager.vm_count(), 2);
    }

    #[test]
    fn test_vm_manager_create_duplicate_name() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        manager.create_vm(make_config("vm1", 1, 512)).unwrap();
        let result = manager.create_vm(make_config("vm1", 1, 512));
        assert!(matches!(result, Err(VirtError::VmAlreadyExists(_))));
    }

    #[test]
    fn test_vm_manager_create_quota_exceeded_vcpu() {
        let quota = VmResourceQuota {
            max_vcpus: 2,
            ..Default::default()
        };
        let mut manager = VmManager::new(quota);

        manager.create_vm(make_config("vm1", 2, 512)).unwrap();
        let result = manager.create_vm(make_config("vm2", 1, 512));
        assert!(matches!(result, Err(VirtError::QuotaExceeded(_))));
    }

    #[test]
    fn test_vm_manager_create_quota_exceeded_memory() {
        let quota = VmResourceQuota {
            max_memory_mb: 1024,
            ..Default::default()
        };
        let mut manager = VmManager::new(quota);

        manager.create_vm(make_config("vm1", 1, 1024)).unwrap();
        let result = manager.create_vm(make_config("vm2", 1, 1));
        assert!(matches!(result, Err(VirtError::QuotaExceeded(_))));
    }

    #[test]
    fn test_vm_manager_lifecycle() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let id = manager.create_vm(make_config("vm1", 1, 512)).unwrap();

        // Created -> Configured
        manager.configure_vm(id).unwrap();
        assert_eq!(manager.get_vm(id).unwrap().state, VmState::Configured);

        // Configured -> Running
        manager.start_vm(id).unwrap();
        assert_eq!(manager.get_vm(id).unwrap().state, VmState::Running);

        // Running -> Paused
        manager.pause_vm(id).unwrap();
        assert_eq!(manager.get_vm(id).unwrap().state, VmState::Paused);

        // Paused -> Running
        manager.resume_vm(id).unwrap();
        assert_eq!(manager.get_vm(id).unwrap().state, VmState::Running);

        // Running -> Destroyed
        manager.destroy_vm(id).unwrap();
        assert!(manager.get_vm(id).is_none());
    }

    #[test]
    fn test_vm_manager_destroy_not_found() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let result = manager.destroy_vm(999);
        assert!(matches!(result, Err(VirtError::VmNotFound(999))));
    }

    #[test]
    fn test_vm_manager_invalid_transition() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let id = manager.create_vm(make_config("vm1", 1, 512)).unwrap();

        // Created -> Running 不合法（需要先 Configured）
        let result = manager.start_vm(id);
        assert!(matches!(result, Err(VirtError::InvalidState { .. })));
    }

    #[test]
    fn test_vm_manager_list_vms() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        manager.create_vm(make_config("vm1", 1, 512)).unwrap();
        manager.create_vm(make_config("vm2", 2, 1024)).unwrap();
        manager.create_vm(make_config("vm3", 1, 256)).unwrap();

        let vms = manager.list_vms();
        assert_eq!(vms.len(), 3);
    }

    #[test]
    fn test_vm_manager_resource_usage() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let id1 = manager.create_vm(make_config("vm1", 2, 1024)).unwrap();
        let id2 = manager.create_vm(make_config("vm2", 1, 512)).unwrap();

        manager.configure_vm(id1).unwrap();
        manager.start_vm(id1).unwrap();
        manager.configure_vm(id2).unwrap();
        // vm2 保持 Configured 状态

        let usage = manager.resource_usage();
        assert_eq!(usage.total_vcpus, 3);
        assert_eq!(usage.total_memory_mb, 1536);
        assert_eq!(usage.vm_count, 2);
        assert_eq!(usage.running_count, 1);
    }

    #[test]
    fn test_vm_manager_destroy_releases_resources() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        let id = manager.create_vm(make_config("vm1", 4, 2048)).unwrap();
        assert_eq!(manager.resource_usage().total_vcpus, 4);
        assert_eq!(manager.resource_usage().total_memory_mb, 2048);

        manager.configure_vm(id).unwrap();
        manager.start_vm(id).unwrap();
        manager.destroy_vm(id).unwrap();

        assert_eq!(manager.resource_usage().total_vcpus, 0);
        assert_eq!(manager.resource_usage().total_memory_mb, 0);
        assert_eq!(manager.vm_count(), 0);
    }

    #[test]
    fn test_vm_manager_vm_count() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        assert_eq!(manager.vm_count(), 0);

        manager.create_vm(make_config("vm1", 1, 512)).unwrap();
        assert_eq!(manager.vm_count(), 1);

        manager.create_vm(make_config("vm2", 1, 512)).unwrap();
        assert_eq!(manager.vm_count(), 2);
    }

    #[test]
    fn test_vm_manager_get_vm_not_found() {
        let quota = VmResourceQuota::default();
        let manager = VmManager::new(quota);

        assert!(manager.get_vm(999).is_none());
    }

    #[test]
    fn test_vm_manager_invalid_config() {
        let quota = VmResourceQuota::default();
        let mut manager = VmManager::new(quota);

        // vCPU 数量为 0
        let result = manager.create_vm(make_config("bad", 0, 512));
        assert!(matches!(result, Err(VirtError::InvalidConfig(_))));
    }
}
