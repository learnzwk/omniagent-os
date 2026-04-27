//! 驱动匹配模块
//!
//! 实现驱动注册、注销和设备-驱动自动匹配功能。
//! 支持按设备类、厂商 ID 匹配，以及优先级排序。

use alloc::string::String;
use alloc::vec::Vec;

use spin::Mutex;

use crate::device_manager::device::{
    DeviceClass, DeviceDescriptor, DeviceManagerError, DeviceStatus,
};

// ============================================================================
// 驱动描述
// ============================================================================

/// 驱动描述
#[derive(Debug, Clone)]
pub struct DriverDescriptor {
    /// 驱动 ID
    pub driver_id: u64,
    /// 驱动名称
    pub name: String,
    /// 支持的设备类列表
    pub supported_classes: Vec<DeviceClass>,
    /// 支持的厂商 ID 列表
    pub supported_vendor_ids: Vec<u32>,
    /// 驱动优先级（数值越小优先级越高）
    pub priority: u32,
    /// 是否已加载
    pub is_loaded: bool,
}

// ============================================================================
// 驱动匹配器
// ============================================================================

/// 驱动匹配器
///
/// 管理系统中所有驱动，提供设备-驱动匹配功能。
pub struct DriverMatcher {
    /// 驱动列表
    drivers: Mutex<Vec<DriverDescriptor>>,
}

impl DriverMatcher {
    /// 创建新的驱动匹配器
    pub fn new() -> Self {
        DriverMatcher {
            drivers: Mutex::new(Vec::new()),
        }
    }

    /// 注册驱动
    pub fn register_driver(&self, desc: DriverDescriptor) -> Result<(), DeviceManagerError> {
        let mut drivers = self.drivers.lock();
        // 检查是否已注册
        if drivers.iter().any(|d| d.driver_id == desc.driver_id) {
            return Err(DeviceManagerError::AlreadyRegistered(desc.driver_id));
        }
        drivers.push(desc);
        Ok(())
    }

    /// 注销驱动
    pub fn unregister_driver(&self, driver_id: u64) -> Result<(), DeviceManagerError> {
        let mut drivers = self.drivers.lock();
        let len_before = drivers.len();
        drivers.retain(|d| d.driver_id != driver_id);
        if drivers.len() == len_before {
            Err(DeviceManagerError::DriverNotFound(driver_id))
        } else {
            Ok(())
        }
    }

    /// 查找设备的最佳匹配驱动
    ///
    /// 根据设备类和厂商 ID 匹配，返回优先级最高的驱动。
    pub fn find_best_driver(&self, device: &DeviceDescriptor) -> Option<DriverDescriptor> {
        let drivers = self.drivers.lock();
        let mut matched: Vec<&DriverDescriptor> = drivers
            .iter()
            .filter(|d| {
                // 检查设备类是否匹配
                let class_match = d.supported_classes.contains(&device.device_class)
                    || d.supported_classes.contains(&DeviceClass::Unknown);
                // 检查厂商 ID 是否匹配（空列表表示匹配所有厂商）
                let vendor_match = d.supported_vendor_ids.is_empty()
                    || d.supported_vendor_ids.contains(&device.vendor_id);
                class_match && vendor_match
            })
            .collect();

        if matched.is_empty() {
            return None;
        }

        // 按优先级排序（数值越小优先级越高）
        matched.sort_by_key(|d| d.priority);
        Some(matched[0].clone())
    }

    /// 查找设备的所有匹配驱动
    pub fn find_all_drivers(&self, device: &DeviceDescriptor) -> Vec<DriverDescriptor> {
        let drivers = self.drivers.lock();
        drivers
            .iter()
            .filter(|d| {
                let class_match = d.supported_classes.contains(&device.device_class)
                    || d.supported_classes.contains(&DeviceClass::Unknown);
                let vendor_match = d.supported_vendor_ids.is_empty()
                    || d.supported_vendor_ids.contains(&device.vendor_id);
                class_match && vendor_match
            })
            .cloned()
            .collect()
    }

    /// 列出所有驱动
    pub fn list_drivers(&self) -> Vec<DriverDescriptor> {
        let drivers = self.drivers.lock();
        drivers.clone()
    }

    /// 获取驱动总数
    pub fn driver_count(&self) -> usize {
        let drivers = self.drivers.lock();
        drivers.len()
    }
}

/// 全局驱动匹配器实例
pub static DRIVER_MATCHER: spin::Lazy<Mutex<DriverMatcher>> = spin::Lazy::new(|| {
    Mutex::new(DriverMatcher::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用驱动描述
    fn make_driver(driver_id: u64, name: &str, classes: Vec<DeviceClass>, priority: u32) -> DriverDescriptor {
        DriverDescriptor {
            driver_id,
            name: String::from(name),
            supported_classes: classes,
            supported_vendor_ids: Vec::new(),
            priority,
            is_loaded: true,
        }
    }

    /// 创建带厂商 ID 的驱动描述
    fn make_driver_with_vendor(
        driver_id: u64,
        name: &str,
        classes: Vec<DeviceClass>,
        vendor_ids: Vec<u32>,
        priority: u32,
    ) -> DriverDescriptor {
        DriverDescriptor {
            driver_id,
            name: String::from(name),
            supported_classes: classes,
            supported_vendor_ids: vendor_ids,
            priority,
            is_loaded: true,
        }
    }

    /// 创建测试用设备描述
    fn make_device(name: &str, class: DeviceClass, vendor_id: u32) -> DeviceDescriptor {
        DeviceDescriptor {
            device_id: 0,
            name: String::from(name),
            device_class: class,
            status: DeviceStatus::Connected,
            vendor_id,
            product_id: 0,
            driver_id: None,
            parent_id: None,
            capabilities: Vec::new(),
        }
    }

    // === 测试: 创建驱动匹配器 ===
    #[test]
    fn test_new() {
        let matcher = DriverMatcher::new();
        assert_eq!(matcher.driver_count(), 0);
    }

    // === 测试: 注册驱动 ===
    #[test]
    fn test_register_driver() {
        let matcher = DriverMatcher::new();
        let driver = make_driver(1, "blk_driver", alloc::vec![DeviceClass::Block], 10);
        assert!(matcher.register_driver(driver).is_ok());
        assert_eq!(matcher.driver_count(), 1);

        // 重复注册应失败
        let driver2 = make_driver(1, "blk_driver2", alloc::vec![DeviceClass::Block], 20);
        assert!(matcher.register_driver(driver2).is_err());
    }

    // === 测试: 注销驱动 ===
    #[test]
    fn test_unregister_driver() {
        let matcher = DriverMatcher::new();
        matcher.register_driver(make_driver(1, "drv1", alloc::vec![DeviceClass::Block], 10)).unwrap();
        assert_eq!(matcher.driver_count(), 1);

        assert!(matcher.unregister_driver(1).is_ok());
        assert_eq!(matcher.driver_count(), 0);

        // 注销不存在的驱动
        assert!(matcher.unregister_driver(999).is_err());
    }

    // === 测试: 查找最佳驱动 ===
    #[test]
    fn test_find_best_driver() {
        let matcher = DriverMatcher::new();
        matcher.register_driver(make_driver(1, "blk_low", alloc::vec![DeviceClass::Block], 20)).unwrap();
        matcher.register_driver(make_driver(2, "blk_high", alloc::vec![DeviceClass::Block], 5)).unwrap();
        matcher.register_driver(make_driver(3, "net_drv", alloc::vec![DeviceClass::Network], 10)).unwrap();

        let device = make_device("my_blk", DeviceClass::Block, 1);
        let best = matcher.find_best_driver(&device).unwrap();
        assert_eq!(best.driver_id, 2); // 优先级 5 最高
        assert_eq!(best.name, "blk_high");
    }

    // === 测试: 查找所有匹配驱动 ===
    #[test]
    fn test_find_all_drivers() {
        let matcher = DriverMatcher::new();
        matcher.register_driver(make_driver(1, "blk1", alloc::vec![DeviceClass::Block], 10)).unwrap();
        matcher.register_driver(make_driver(2, "blk2", alloc::vec![DeviceClass::Block], 20)).unwrap();
        matcher.register_driver(make_driver(3, "net1", alloc::vec![DeviceClass::Network], 10)).unwrap();

        let device = make_device("my_blk", DeviceClass::Block, 1);
        let all = matcher.find_all_drivers(&device);
        assert_eq!(all.len(), 2);
    }

    // === 测试: 按厂商 ID 匹配 ===
    #[test]
    fn test_vendor_match() {
        let matcher = DriverMatcher::new();
        // 驱动 1 只支持厂商 0x1234
        matcher.register_driver(make_driver_with_vendor(
            1,
            "vendor_drv",
            alloc::vec![DeviceClass::Block],
            alloc::vec![0x1234],
            10,
        )).unwrap();
        // 驱动 2 支持所有厂商
        matcher.register_driver(make_driver(
            2,
            "generic_drv",
            alloc::vec![DeviceClass::Block],
            20,
        )).unwrap();

        // 匹配厂商 0x1234 的设备
        let device1 = make_device("dev1", DeviceClass::Block, 0x1234);
        let all1 = matcher.find_all_drivers(&device1);
        assert_eq!(all1.len(), 2);

        // 匹配其他厂商的设备
        let device2 = make_device("dev2", DeviceClass::Block, 0x5678);
        let all2 = matcher.find_all_drivers(&device2);
        assert_eq!(all2.len(), 1);
        assert_eq!(all2[0].driver_id, 2);
    }

    // === 测试: 列出所有驱动 ===
    #[test]
    fn test_list_drivers() {
        let matcher = DriverMatcher::new();
        matcher.register_driver(make_driver(1, "drv1", alloc::vec![DeviceClass::Block], 10)).unwrap();
        matcher.register_driver(make_driver(2, "drv2", alloc::vec![DeviceClass::Network], 20)).unwrap();

        let drivers = matcher.list_drivers();
        assert_eq!(drivers.len(), 2);
    }

    // === 测试: 无匹配驱动 ===
    #[test]
    fn test_no_match() {
        let matcher = DriverMatcher::new();
        matcher.register_driver(make_driver(1, "blk_drv", alloc::vec![DeviceClass::Block], 10)).unwrap();

        let device = make_device("audio_dev", DeviceClass::Audio, 1);
        assert!(matcher.find_best_driver(&device).is_none());
        assert!(matcher.find_all_drivers(&device).is_empty());
    }
}
