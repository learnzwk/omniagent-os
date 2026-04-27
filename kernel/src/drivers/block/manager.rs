//! 块设备管理器
//!
//! 提供块设备的注册、注销、查询和列表功能。
//! 使用全局静态实例 BLOCK_MANAGER 管理所有块设备。

use super::device::{BlockDevice, BlockError, BlockDeviceInfo};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

// ============================================================================
// BlockDeviceManager: 块设备管理器
// ============================================================================

/// 块设备管理器
///
/// 管理所有已注册的块设备，提供注册、注销、查询和列表功能。
/// 使用 Mutex 保护内部设备列表以支持并发访问。
pub struct BlockDeviceManager {
    /// 已注册的设备列表
    devices: Mutex<Vec<Arc<dyn BlockDevice>>>,
}

impl BlockDeviceManager {
    /// 创建新的块设备管理器
    pub fn new() -> Self {
        Self {
            devices: Mutex::new(Vec::new()),
        }
    }

    /// 注册新的块设备
    ///
    /// # 参数
    /// - `device`: 要注册的块设备（Arc 包装）
    ///
    /// # 错误
    /// - `DeviceBusy`: 同名设备已存在
    pub fn register(&self, device: Arc<dyn BlockDevice>) -> Result<(), BlockError> {
        let name = alloc::string::String::from(device.name());
        let mut devices = self.devices.lock();

        // 检查同名设备是否已存在
        for existing in devices.iter() {
            if existing.name() == name {
                return Err(BlockError::DeviceBusy);
            }
        }

        devices.push(device);
        Ok(())
    }

    /// 注销块设备
    ///
    /// # 参数
    /// - `name`: 要注销的设备名称
    ///
    /// # 错误
    /// - `DeviceNotFound`: 指定名称的设备不存在
    pub fn unregister(&self, name: &str) -> Result<(), BlockError> {
        let mut devices = self.devices.lock();
        let len_before = devices.len();

        devices.retain(|d| d.name() != name);

        if devices.len() == len_before {
            Err(BlockError::DeviceNotFound)
        } else {
            Ok(())
        }
    }

    /// 获取指定名称的块设备
    ///
    /// # 参数
    /// - `name`: 设备名称
    ///
    /// # 返回
    /// 如果找到设备，返回 Some(Arc<dyn BlockDevice>)
    pub fn get(&self, name: &str) -> Option<Arc<dyn BlockDevice>> {
        let devices = self.devices.lock();
        for device in devices.iter() {
            if device.name() == name {
                return Some(Arc::clone(device));
            }
        }
        None
    }

    /// 列出所有已注册的块设备信息
    ///
    /// # 返回
    /// 包含所有设备信息的向量
    pub fn list(&self) -> Vec<BlockDeviceInfo> {
        let devices = self.devices.lock();
        devices
            .iter()
            .map(|d| BlockDeviceInfo {
                name: alloc::string::String::from(d.name()),
                block_size: d.block_size(),
                capacity: d.capacity(),
                is_removable: d.is_removable(),
            })
            .collect()
    }

    /// 获取已注册设备数量
    pub fn count(&self) -> usize {
        let devices = self.devices.lock();
        devices.len()
    }
}

impl Default for BlockDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 全局块设备管理器
// ============================================================================

/// 全局块设备管理器
///
/// 使用 spin::Lazy 延迟初始化的全局块设备管理器实例。
pub static BLOCK_MANAGER: spin::Lazy<Mutex<BlockDeviceManager>> = spin::Lazy::new(|| {
    Mutex::new(BlockDeviceManager::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::block::RamDisk;

    // === 测试: 注册块设备 ===
    #[test]
    fn test_block_manager_register() {
        let manager = BlockDeviceManager::new();
        let disk = Arc::new(RamDisk::new("disk0", 512, 100));

        assert!(manager.register(disk).is_ok());
        assert_eq!(manager.count(), 1);
    }

    // === 测试: 获取块设备 ===
    #[test]
    fn test_block_manager_get() {
        let manager = BlockDeviceManager::new();
        let disk = Arc::new(RamDisk::new("disk0", 512, 100));
        manager.register(disk).unwrap();

        let found = manager.get("disk0");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "disk0");

        let not_found = manager.get("nonexistent");
        assert!(not_found.is_none());
    }

    // === 测试: 注销块设备 ===
    #[test]
    fn test_block_manager_unregister() {
        let manager = BlockDeviceManager::new();
        let disk = Arc::new(RamDisk::new("disk0", 512, 100));
        manager.register(disk).unwrap();

        assert!(manager.unregister("disk0").is_ok());
        assert_eq!(manager.count(), 0);
        assert!(manager.get("disk0").is_none());
    }

    // === 测试: 列出所有设备 ===
    #[test]
    fn test_block_manager_list() {
        let manager = BlockDeviceManager::new();
        manager.register(Arc::new(RamDisk::new("disk0", 512, 100))).unwrap();
        manager.register(Arc::new(RamDisk::new("disk1", 1024, 50))).unwrap();

        let list = manager.list();
        assert_eq!(list.len(), 2);

        let names: Vec<&str> = list.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"disk0"));
        assert!(names.contains(&"disk1"));
    }

    // === 测试: 设备计数 ===
    #[test]
    fn test_block_manager_count() {
        let manager = BlockDeviceManager::new();
        assert_eq!(manager.count(), 0);

        manager.register(Arc::new(RamDisk::new("d0", 512, 10))).unwrap();
        assert_eq!(manager.count(), 1);

        manager.register(Arc::new(RamDisk::new("d1", 512, 10))).unwrap();
        assert_eq!(manager.count(), 2);

        manager.unregister("d0").unwrap();
        assert_eq!(manager.count(), 1);
    }

    // === 测试: 重复名称注册 ===
    #[test]
    fn test_block_manager_duplicate_name() {
        let manager = BlockDeviceManager::new();
        manager.register(Arc::new(RamDisk::new("disk0", 512, 100))).unwrap();

        let result = manager.register(Arc::new(RamDisk::new("disk0", 1024, 50)));
        assert!(matches!(result, Err(BlockError::DeviceBusy)));
        assert_eq!(manager.count(), 1);
    }

    // === 测试: 注销不存在的设备 ===
    #[test]
    fn test_block_manager_not_found() {
        let manager = BlockDeviceManager::new();
        let result = manager.unregister("nonexistent");
        assert!(matches!(result, Err(BlockError::DeviceNotFound)));
    }

    // === 测试: 通过管理器获取设备进行读写 ===
    #[test]
    fn test_block_manager_read_write_via_get() {
        let manager = BlockDeviceManager::new();
        let disk = Arc::new(RamDisk::new("rw-disk", 512, 10));
        manager.register(disk).unwrap();

        let device = manager.get("rw-disk").unwrap();

        // 写入数据
        let write_buf = [0xCDu8; 512];
        device.write_blocks(0, &write_buf).unwrap();

        // 读回数据
        let mut read_buf = [0u8; 512];
        device.read_blocks(0, &mut read_buf).unwrap();

        assert_eq!(read_buf, write_buf);
    }
}
