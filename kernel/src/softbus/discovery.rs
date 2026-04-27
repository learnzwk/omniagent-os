//! 设备发现模块
//! 模仿鸿蒙 DSoftBus 的设备发现机制，支持设备注册、注销、查询和自动发现

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use crate::softbus::error::SoftBusError;

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// 未知设备
    Unknown = 0,
    /// 传感器
    Sensor = 1,
    /// 可穿戴设备
    Wearable = 2,
    /// 手机
    Phone = 3,
    /// 平板
    Tablet = 4,
    /// 电视
    Tv = 5,
    /// 汽车
    Car = 6,
    /// 服务器
    Server = 7,
    /// Agent 节点
    Agent = 8,
}

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// 设备唯一标识
    pub device_id: u64,
    /// 设备名称
    pub device_name: String,
    /// 设备类型
    pub device_type: DeviceType,
    /// IP 地址
    pub ip_address: Option<[u8; 4]>,
    /// MAC 地址
    pub mac_address: Option<[u8; 6]>,
    /// 设备能力列表
    pub capabilities: Vec<String>,
    /// 是否在线
    pub is_online: bool,
    /// 最后活跃时间戳
    pub last_seen: u64,
}

/// 设备发现管理器
/// 负责设备的注册、注销、查询和自动发现
pub struct DeviceDiscovery {
    /// 已注册设备表
    devices: Mutex<BTreeMap<u64, DeviceInfo>>,
    /// 本地设备 ID
    local_device_id: u64,
    /// 下一个设备 ID（用于自动分配）
    next_device_id: AtomicU64,
    /// 时间戳计数器（用于 update_last_seen）
    timestamp_counter: AtomicU64,
}

impl DeviceDiscovery {
    /// 创建新的设备发现管理器
    ///
    /// # 参数
    /// - `local_device_id`: 本地设备的 ID
    pub fn new(local_device_id: u64) -> Self {
        Self {
            devices: Mutex::new(BTreeMap::new()),
            local_device_id,
            next_device_id: AtomicU64::new(local_device_id + 1),
            timestamp_counter: AtomicU64::new(10000),
        }
    }

    /// 注册一个新设备
    ///
    /// # 参数
    /// - `info`: 设备信息
    ///
    /// # 返回
    /// 成功返回 Ok(())，如果设备 ID 已存在则返回错误
    pub fn register_device(&self, info: DeviceInfo) -> Result<(), SoftBusError> {
        let mut devices = self.devices.lock();
        if devices.contains_key(&info.device_id) {
            return Err(SoftBusError::AlreadyConnected(info.device_id));
        }
        devices.insert(info.device_id, info);
        Ok(())
    }

    /// 注销设备
    ///
    /// # 参数
    /// - `device_id`: 要注销的设备 ID
    pub fn unregister_device(&self, device_id: u64) -> Result<(), SoftBusError> {
        let mut devices = self.devices.lock();
        if devices.remove(&device_id).is_some() {
            Ok(())
        } else {
            Err(SoftBusError::DeviceNotFound(device_id))
        }
    }

    /// 获取设备信息
    ///
    /// # 参数
    /// - `device_id`: 设备 ID
    ///
    /// # 返回
    /// 设备信息的克隆，如果不存在则返回 None
    pub fn get_device(&self, device_id: u64) -> Option<DeviceInfo> {
        let devices = self.devices.lock();
        devices.get(&device_id).cloned()
    }

    /// 列出所有已注册设备
    pub fn list_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.devices.lock();
        devices.values().cloned().collect()
    }

    /// 列出所有在线设备
    pub fn list_online_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.devices.lock();
        devices
            .values()
            .filter(|d| d.is_online)
            .cloned()
            .collect()
    }

    /// 按设备类型查找设备
    ///
    /// # 参数
    /// - `device_type`: 要查找的设备类型
    pub fn find_by_type(&self, device_type: DeviceType) -> Vec<DeviceInfo> {
        let devices = self.devices.lock();
        devices
            .values()
            .filter(|d| d.device_type == device_type)
            .cloned()
            .collect()
    }

    /// 按能力查找设备
    ///
    /// # 参数
    /// - `cap`: 要匹配的能力名称
    pub fn find_by_capability(&self, cap: &str) -> Vec<DeviceInfo> {
        let devices = self.devices.lock();
        devices
            .values()
            .filter(|d| d.capabilities.iter().any(|c| c == cap))
            .cloned()
            .collect()
    }

    /// 更新设备的最后活跃时间
    ///
    /// # 参数
    /// - `device_id`: 设备 ID
    pub fn update_last_seen(&self, device_id: u64) {
        let mut devices = self.devices.lock();
        if let Some(device) = devices.get_mut(&device_id) {
            // 使用原子计数器生成递增时间戳
            let new_ts = self.timestamp_counter.fetch_add(1, Ordering::SeqCst);
            device.last_seen = new_ts;
        }
    }

    /// 获取已注册设备总数
    pub fn device_count(&self) -> usize {
        let devices = self.devices.lock();
        devices.len()
    }

    /// 获取在线设备数量
    pub fn online_count(&self) -> usize {
        let devices = self.devices.lock();
        devices.values().filter(|d| d.is_online).count()
    }
}

/// 全局设备发现实例
pub static DISCOVERY: spin::Lazy<Mutex<DeviceDiscovery>> = spin::Lazy::new(|| {
    Mutex::new(DeviceDiscovery::new(1))
});

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用设备信息
    fn make_device(id: u64, name: &str, dtype: DeviceType, online: bool) -> DeviceInfo {
        DeviceInfo {
            device_id: id,
            device_name: String::from(name),
            device_type: dtype,
            ip_address: Some([192, 168, 1, (id % 256) as u8]),
            mac_address: Some([0x00, 0x11, 0x22, 0x33, 0x44, id as u8]),
            capabilities: vec![String::from("audio"), String::from("video")],
            is_online: online,
            last_seen: 100,
        }
    }

    #[test]
    fn test_register_device() {
        let discovery = DeviceDiscovery::new(1);
        let device = make_device(10, "传感器-A", DeviceType::Sensor, true);
        assert!(discovery.register_device(device).is_ok());
        assert_eq!(discovery.device_count(), 1);
    }

    #[test]
    fn test_unregister_device() {
        let discovery = DeviceDiscovery::new(1);
        let device = make_device(20, "手机-B", DeviceType::Phone, true);
        discovery.register_device(device).unwrap();
        assert_eq!(discovery.device_count(), 1);

        assert!(discovery.unregister_device(20).is_ok());
        assert_eq!(discovery.device_count(), 0);

        // 注销不存在的设备应返回错误
        let result = discovery.unregister_device(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_device() {
        let discovery = DeviceDiscovery::new(1);
        let device = make_device(30, "平板-C", DeviceType::Tablet, false);
        discovery.register_device(device.clone()).unwrap();

        let found = discovery.get_device(30);
        assert!(found.is_some());
        assert_eq!(found.unwrap().device_name, "平板-C");

        // 获取不存在的设备
        let not_found = discovery.get_device(999);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_list_devices() {
        let discovery = DeviceDiscovery::new(1);
        discovery.register_device(make_device(1, "设备1", DeviceType::Sensor, true)).unwrap();
        discovery.register_device(make_device(2, "设备2", DeviceType::Phone, false)).unwrap();
        discovery.register_device(make_device(3, "设备3", DeviceType::Agent, true)).unwrap();

        let all = discovery.list_devices();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_list_online() {
        let discovery = DeviceDiscovery::new(1);
        discovery.register_device(make_device(1, "在线设备", DeviceType::Sensor, true)).unwrap();
        discovery.register_device(make_device(2, "离线设备", DeviceType::Phone, false)).unwrap();
        discovery.register_device(make_device(3, "在线Agent", DeviceType::Agent, true)).unwrap();

        let online = discovery.list_online_devices();
        assert_eq!(online.len(), 2);
        assert!(online.iter().all(|d| d.is_online));
    }

    #[test]
    fn test_find_by_type() {
        let discovery = DeviceDiscovery::new(1);
        discovery.register_device(make_device(1, "传感器1", DeviceType::Sensor, true)).unwrap();
        discovery.register_device(make_device(2, "传感器2", DeviceType::Sensor, false)).unwrap();
        discovery.register_device(make_device(3, "手机", DeviceType::Phone, true)).unwrap();

        let sensors = discovery.find_by_type(DeviceType::Sensor);
        assert_eq!(sensors.len(), 2);

        let phones = discovery.find_by_type(DeviceType::Phone);
        assert_eq!(phones.len(), 1);

        let cars = discovery.find_by_type(DeviceType::Car);
        assert_eq!(cars.len(), 0);
    }

    #[test]
    fn test_find_by_capability() {
        let discovery = DeviceDiscovery::new(1);
        let mut device1 = make_device(1, "音频设备", DeviceType::Sensor, true);
        device1.capabilities = vec![String::from("audio")];
        let mut device2 = make_device(2, "视频设备", DeviceType::Phone, true);
        device2.capabilities = vec![String::from("video")];
        let mut device3 = make_device(3, "全能设备", DeviceType::Agent, true);
        device3.capabilities = vec![String::from("audio"), String::from("video")];

        discovery.register_device(device1).unwrap();
        discovery.register_device(device2).unwrap();
        discovery.register_device(device3).unwrap();

        let audio_devices = discovery.find_by_capability("audio");
        assert_eq!(audio_devices.len(), 2);

        let video_devices = discovery.find_by_capability("video");
        assert_eq!(video_devices.len(), 2);

        let other = discovery.find_by_capability("unknown");
        assert_eq!(other.len(), 0);
    }

    #[test]
    fn test_update_last_seen() {
        let discovery = DeviceDiscovery::new(1);
        let device = make_device(10, "设备", DeviceType::Sensor, true);
        discovery.register_device(device).unwrap();

        let before = discovery.get_device(10).unwrap().last_seen;
        discovery.update_last_seen(10);
        let after = discovery.get_device(10).unwrap().last_seen;
        assert!(after > before);
    }

    #[test]
    fn test_device_count() {
        let discovery = DeviceDiscovery::new(1);
        assert_eq!(discovery.device_count(), 0);
        assert_eq!(discovery.online_count(), 0);

        discovery.register_device(make_device(1, "在线", DeviceType::Sensor, true)).unwrap();
        discovery.register_device(make_device(2, "离线", DeviceType::Phone, false)).unwrap();

        assert_eq!(discovery.device_count(), 2);
        assert_eq!(discovery.online_count(), 1);
    }

    #[test]
    fn test_duplicate_register() {
        let discovery = DeviceDiscovery::new(1);
        let device = make_device(10, "重复设备", DeviceType::Sensor, true);
        assert!(discovery.register_device(device.clone()).is_ok());
        // 重复注册应返回错误
        let result = discovery.register_device(device);
        assert!(result.is_err());
        assert_eq!(discovery.device_count(), 1);
    }
}
