//! 设备抽象层
//!
//! 实现设备注册、注销、状态管理和驱动绑定功能。
//! 支持设备分类、状态跟踪和按条件查询。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

// ============================================================================
// 错误类型
// ============================================================================

/// 设备管理器错误类型
#[derive(Debug, Clone)]
pub enum DeviceManagerError {
    /// 设备未找到
    DeviceNotFound(u64),
    /// 驱动未找到
    DriverNotFound(u64),
    /// 设备已注册
    AlreadyRegistered(u64),
    /// 设备未连接
    NotConnected,
    /// 操作失败
    OperationFailed { reason: &'static str },
}

impl fmt::Display for DeviceManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceManagerError::DeviceNotFound(id) => {
                write!(f, "设备未找到: {}", id)
            }
            DeviceManagerError::DriverNotFound(id) => {
                write!(f, "驱动未找到: {}", id)
            }
            DeviceManagerError::AlreadyRegistered(id) => {
                write!(f, "设备已注册: {}", id)
            }
            DeviceManagerError::NotConnected => {
                write!(f, "设备未连接")
            }
            DeviceManagerError::OperationFailed { reason } => {
                write!(f, "操作失败: {}", reason)
            }
        }
    }
}

// ============================================================================
// 设备类
// ============================================================================

/// 设备类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// 未知设备
    Unknown = 0,
    /// 块设备
    Block = 1,
    /// 字符设备
    Char = 2,
    /// 网络设备
    Network = 3,
    /// 输入设备
    Input = 4,
    /// 显示设备
    Display = 5,
    /// 音频设备
    Audio = 6,
    /// 传感器设备
    Sensor = 7,
    /// Agent 设备
    Agent = 8,
}

// ============================================================================
// 设备状态
// ============================================================================

/// 设备状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// 已断开
    Disconnected = 0,
    /// 已连接
    Connected = 1,
    /// 初始化中
    Initializing = 2,
    /// 活跃
    Active = 3,
    /// 已挂起
    Suspended = 4,
    /// 错误
    Error = 5,
    /// 已移除
    Removed = 6,
}

// ============================================================================
// 设备描述
// ============================================================================

/// 设备描述
#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    /// 设备 ID
    pub device_id: u64,
    /// 设备名称
    pub name: String,
    /// 设备类
    pub device_class: DeviceClass,
    /// 设备状态
    pub status: DeviceStatus,
    /// 厂商 ID
    pub vendor_id: u32,
    /// 产品 ID
    pub product_id: u32,
    /// 绑定的驱动 ID
    pub driver_id: Option<u64>,
    /// 父设备 ID
    pub parent_id: Option<u64>,
    /// 设备能力列表
    pub capabilities: Vec<String>,
}

// ============================================================================
// 设备管理器
// ============================================================================

/// 设备管理器
///
/// 管理系统中所有设备的注册、注销、状态变更和驱动绑定。
pub struct DeviceManager {
    /// 设备映射表
    devices: Mutex<BTreeMap<u64, DeviceDescriptor>>,
    /// 下一个可用设备 ID
    next_id: AtomicU64,
}

impl DeviceManager {
    /// 创建新的设备管理器
    pub fn new() -> Self {
        DeviceManager {
            devices: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 注册设备
    ///
    /// 将设备描述注册到管理器中，自动分配设备 ID。
    pub fn register_device(&self, mut desc: DeviceDescriptor) -> Result<u64, DeviceManagerError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        desc.device_id = id;

        {
            let mut devices = self.devices.lock();
            devices.insert(id, desc);
        }

        Ok(id)
    }

    /// 注销设备
    ///
    /// 从管理器中移除指定设备。
    pub fn unregister_device(&self, device_id: u64) -> Result<(), DeviceManagerError> {
        let mut devices = self.devices.lock();
        devices
            .remove(&device_id)
            .ok_or(DeviceManagerError::DeviceNotFound(device_id))?;
        Ok(())
    }

    /// 获取设备描述
    pub fn get_device(&self, device_id: u64) -> Option<DeviceDescriptor> {
        let devices = self.devices.lock();
        devices.get(&device_id).cloned()
    }

    /// 更新设备状态
    pub fn update_status(&self, device_id: u64, status: DeviceStatus) -> Result<(), DeviceManagerError> {
        let mut devices = self.devices.lock();
        let device = devices
            .get_mut(&device_id)
            .ok_or(DeviceManagerError::DeviceNotFound(device_id))?;
        device.status = status;
        Ok(())
    }

    /// 绑定驱动到设备
    pub fn bind_driver(&self, device_id: u64, driver_id: u64) -> Result<(), DeviceManagerError> {
        let mut devices = self.devices.lock();
        let device = devices
            .get_mut(&device_id)
            .ok_or(DeviceManagerError::DeviceNotFound(device_id))?;
        device.driver_id = Some(driver_id);
        Ok(())
    }

    /// 解绑设备驱动
    pub fn unbind_driver(&self, device_id: u64) -> Result<(), DeviceManagerError> {
        let mut devices = self.devices.lock();
        let device = devices
            .get_mut(&device_id)
            .ok_or(DeviceManagerError::DeviceNotFound(device_id))?;
        device.driver_id = None;
        Ok(())
    }

    /// 列出所有设备
    pub fn list_devices(&self) -> Vec<DeviceDescriptor> {
        let devices = self.devices.lock();
        devices.values().cloned().collect()
    }

    /// 按设备类列出设备
    pub fn list_by_class(&self, class: DeviceClass) -> Vec<DeviceDescriptor> {
        let devices = self.devices.lock();
        devices
            .values()
            .filter(|d| d.device_class == class)
            .cloned()
            .collect()
    }

    /// 按设备状态列出设备
    pub fn list_by_status(&self, status: DeviceStatus) -> Vec<DeviceDescriptor> {
        let devices = self.devices.lock();
        devices
            .values()
            .filter(|d| d.status == status)
            .cloned()
            .collect()
    }

    /// 获取设备总数
    pub fn device_count(&self) -> usize {
        let devices = self.devices.lock();
        devices.len()
    }

    /// 获取活跃设备数量
    pub fn active_count(&self) -> usize {
        let devices = self.devices.lock();
        devices
            .values()
            .filter(|d| d.status == DeviceStatus::Active)
            .count()
    }
}

/// 全局设备管理器实例
pub static DEVICE_MANAGER: spin::Lazy<Mutex<DeviceManager>> = spin::Lazy::new(|| {
    Mutex::new(DeviceManager::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用设备描述
    fn make_device(name: &str, class: DeviceClass, vendor_id: u32, product_id: u32) -> DeviceDescriptor {
        DeviceDescriptor {
            device_id: 0,
            name: String::from(name),
            device_class: class,
            status: DeviceStatus::Connected,
            vendor_id,
            product_id,
            driver_id: None,
            parent_id: None,
            capabilities: Vec::new(),
        }
    }

    // === 测试: 创建设备管理器 ===
    #[test]
    fn test_new() {
        let mgr = DeviceManager::new();
        assert_eq!(mgr.device_count(), 0);
        assert_eq!(mgr.active_count(), 0);
    }

    // === 测试: 注册设备 ===
    #[test]
    fn test_register_device() {
        let mgr = DeviceManager::new();
        let desc = make_device("test_dev", DeviceClass::Block, 0x1234, 0x5678);
        let id = mgr.register_device(desc).unwrap();
        assert_eq!(id, 1);
        assert_eq!(mgr.device_count(), 1);

        let device = mgr.get_device(id).unwrap();
        assert_eq!(device.name, "test_dev");
        assert_eq!(device.device_class, DeviceClass::Block);
    }

    // === 测试: 注销设备 ===
    #[test]
    fn test_unregister_device() {
        let mgr = DeviceManager::new();
        let id = mgr.register_device(make_device("to_remove", DeviceClass::Network, 1, 1)).unwrap();
        assert_eq!(mgr.device_count(), 1);

        assert!(mgr.unregister_device(id).is_ok());
        assert_eq!(mgr.device_count(), 0);
        assert!(mgr.get_device(id).is_none());

        // 注销不存在的设备
        assert!(mgr.unregister_device(999).is_err());
    }

    // === 测试: 获取设备 ===
    #[test]
    fn test_get_device() {
        let mgr = DeviceManager::new();
        let id = mgr.register_device(make_device("get_test", DeviceClass::Char, 1, 1)).unwrap();

        let device = mgr.get_device(id).unwrap();
        assert_eq!(device.name, "get_test");

        // 不存在的设备
        assert!(mgr.get_device(999).is_none());
    }

    // === 测试: 更新设备状态 ===
    #[test]
    fn test_update_status() {
        let mgr = DeviceManager::new();
        let id = mgr.register_device(make_device("status_dev", DeviceClass::Input, 1, 1)).unwrap();

        assert!(mgr.update_status(id, DeviceStatus::Active).is_ok());
        assert_eq!(mgr.get_device(id).unwrap().status, DeviceStatus::Active);
        assert_eq!(mgr.active_count(), 1);

        // 更新不存在的设备
        assert!(mgr.update_status(999, DeviceStatus::Error).is_err());
    }

    // === 测试: 绑定驱动 ===
    #[test]
    fn test_bind_driver() {
        let mgr = DeviceManager::new();
        let id = mgr.register_device(make_device("bind_dev", DeviceClass::Audio, 1, 1)).unwrap();

        assert!(mgr.bind_driver(id, 42).is_ok());
        assert_eq!(mgr.get_device(id).unwrap().driver_id, Some(42));

        // 解绑驱动
        assert!(mgr.unbind_driver(id).is_ok());
        assert_eq!(mgr.get_device(id).unwrap().driver_id, None);

        // 对不存在的设备操作
        assert!(mgr.bind_driver(999, 1).is_err());
        assert!(mgr.unbind_driver(999).is_err());
    }

    // === 测试: 列出所有设备 ===
    #[test]
    fn test_list_devices() {
        let mgr = DeviceManager::new();
        mgr.register_device(make_device("dev1", DeviceClass::Block, 1, 1)).unwrap();
        mgr.register_device(make_device("dev2", DeviceClass::Network, 2, 2)).unwrap();
        mgr.register_device(make_device("dev3", DeviceClass::Audio, 3, 3)).unwrap();

        let devices = mgr.list_devices();
        assert_eq!(devices.len(), 3);
    }

    // === 测试: 按设备类列出 ===
    #[test]
    fn test_list_by_class() {
        let mgr = DeviceManager::new();
        mgr.register_device(make_device("blk1", DeviceClass::Block, 1, 1)).unwrap();
        mgr.register_device(make_device("blk2", DeviceClass::Block, 2, 2)).unwrap();
        mgr.register_device(make_device("net1", DeviceClass::Network, 3, 3)).unwrap();

        let block_devs = mgr.list_by_class(DeviceClass::Block);
        assert_eq!(block_devs.len(), 2);

        let net_devs = mgr.list_by_class(DeviceClass::Network);
        assert_eq!(net_devs.len(), 1);

        let audio_devs = mgr.list_by_class(DeviceClass::Audio);
        assert_eq!(audio_devs.len(), 0);
    }

    // === 测试: 按设备状态列出 ===
    #[test]
    fn test_list_by_status() {
        let mgr = DeviceManager::new();
        let id1 = mgr.register_device(make_device("active1", DeviceClass::Block, 1, 1)).unwrap();
        let id2 = mgr.register_device(make_device("active2", DeviceClass::Network, 2, 2)).unwrap();
        let id3 = mgr.register_device(make_device("suspended1", DeviceClass::Audio, 3, 3)).unwrap();

        mgr.update_status(id1, DeviceStatus::Active).unwrap();
        mgr.update_status(id2, DeviceStatus::Active).unwrap();
        mgr.update_status(id3, DeviceStatus::Suspended).unwrap();

        let active = mgr.list_by_status(DeviceStatus::Active);
        assert_eq!(active.len(), 2);

        let suspended = mgr.list_by_status(DeviceStatus::Suspended);
        assert_eq!(suspended.len(), 1);
    }

    // === 测试: 活跃设备计数 ===
    #[test]
    fn test_active_count() {
        let mgr = DeviceManager::new();
        let id1 = mgr.register_device(make_device("a1", DeviceClass::Block, 1, 1)).unwrap();
        let id2 = mgr.register_device(make_device("a2", DeviceClass::Network, 2, 2)).unwrap();

        assert_eq!(mgr.active_count(), 0);

        mgr.update_status(id1, DeviceStatus::Active).unwrap();
        assert_eq!(mgr.active_count(), 1);

        mgr.update_status(id2, DeviceStatus::Active).unwrap();
        assert_eq!(mgr.active_count(), 2);

        mgr.update_status(id1, DeviceStatus::Suspended).unwrap();
        assert_eq!(mgr.active_count(), 1);
    }
}
