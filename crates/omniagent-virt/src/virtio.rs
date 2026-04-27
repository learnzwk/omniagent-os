// Virtio 设备框架
//
// 定义 Virtio 设备类型、virtqueue、设备 trait 和具体设备实现

use crate::error::VirtError;

/// Virtio 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VirtioDeviceType {
    /// 无效设备
    Invalid = 0,
    /// 网络设备
    Net = 1,
    /// 块设备
    Block = 2,
    /// 控制台设备
    Console = 3,
    /// 熵源设备
    Entropy = 4,
    /// GPU 设备
    Gpu = 16,
    /// 输入设备
    Input = 18,
}

/// Virtio 设备特征位
#[derive(Debug, Clone)]
pub struct VirtioFeatures {
    /// 特征位图
    pub bits: u64,
}

impl VirtioFeatures {
    /// 创建空特征集
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    /// 检查是否设置了指定特征位
    pub fn has(&self, bit: u32) -> bool {
        if bit >= 64 {
            return false;
        }
        (self.bits & (1u64 << bit)) != 0
    }

    /// 设置指定特征位
    pub fn set(&mut self, bit: u32) {
        if bit < 64 {
            self.bits |= 1u64 << bit;
        }
    }

    /// 清除指定特征位
    pub fn clear(&mut self, bit: u32) {
        if bit < 64 {
            self.bits &= !(1u64 << bit);
        }
    }
}

impl Default for VirtioFeatures {
    fn default() -> Self {
        Self::new()
    }
}

/// Virtqueue 描述符
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtqDesc {
    /// 缓冲区地址
    pub addr: u64,
    /// 缓冲区长度
    pub len: u32,
    /// 标志位
    pub flags: u16,
    /// 下一个描述符索引
    pub next: u16,
}

/// Virtqueue 可用环
#[derive(Debug, Clone)]
pub struct VirtqAvailable {
    /// 标志位
    pub flags: u16,
    /// 可用索引
    pub idx: u16,
    /// 可用环
    pub ring: Vec<u16>,
    /// 已用事件索引
    pub used_event: u16,
}

/// Virtqueue 已用环元素
#[derive(Debug, Clone, Copy)]
pub struct VirtqUsedElem {
    /// 描述符索引
    pub id: u32,
    /// 写入长度
    pub len: u32,
}

/// Virtqueue 已用环
#[derive(Debug, Clone)]
pub struct VirtqUsed {
    /// 标志位
    pub flags: u16,
    /// 已用索引
    pub idx: u16,
    /// 已用环
    pub ring: Vec<VirtqUsedElem>,
    /// 可用事件索引
    pub avail_event: u16,
}

/// Virtqueue
pub struct VirtQueue {
    /// 队列索引
    pub index: u16,
    /// 队列大小
    pub queue_size: u16,
    /// 描述符表
    pub descriptors: Vec<VirtqDesc>,
    /// 可用环
    pub available: VirtqAvailable,
    /// 已用环
    pub used: VirtqUsed,
    /// 是否就绪
    pub ready: bool,
}

impl VirtQueue {
    /// 创建新的 Virtqueue
    pub fn new(index: u16, queue_size: u16) -> Self {
        Self {
            index,
            queue_size,
            descriptors: vec![VirtqDesc {
                addr: 0,
                len: 0,
                flags: 0,
                next: 0,
            }; queue_size as usize],
            available: VirtqAvailable {
                flags: 0,
                idx: 0,
                ring: vec![0; queue_size as usize],
                used_event: 0,
            },
            used: VirtqUsed {
                flags: 0,
                idx: 0,
                ring: Vec::new(),
                avail_event: 0,
            },
            ready: false,
        }
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.available.idx == 0
    }

    /// 获取可用缓冲区数量
    pub fn available_count(&self) -> u16 {
        self.available.idx
    }
}

/// Virtio 设备 trait
pub trait VirtioDevice: Send + Sync {
    /// 获取设备类型
    fn device_type(&self) -> VirtioDeviceType;
    /// 获取设备名称
    fn device_name(&self) -> &str;
    /// 获取设备特征位
    fn get_features(&self) -> u64;
    /// 设置设备特征位
    fn set_features(&mut self, features: u64);
    /// 获取队列数量
    fn queue_count(&self) -> u16;
    /// 激活设备
    fn activate(&mut self) -> Result<(), VirtError>;
    /// 处理队列通知
    fn handle_queue_notify(&mut self, queue_index: u16) -> Result<(), VirtError>;
    /// 处理配置空间读取
    fn handle_config_read(&self, offset: u64, size: u8) -> Result<u64, VirtError>;
    /// 处理配置空间写入
    fn handle_config_write(&mut self, offset: u64, size: u8, value: u64) -> Result<(), VirtError>;
    /// 重置设备
    fn reset(&mut self);
    /// 获取设备状态
    fn get_status(&self) -> u8;
    /// 设置设备状态
    fn set_status(&mut self, status: u8);
}

/// Virtio 块设备
pub struct VirtioBlockDevice {
    /// 设备名称
    pub name: String,
    /// 块大小
    pub block_size: u32,
    /// 容量（扇区数）
    pub capacity_sectors: u64,
    /// 是否只读
    pub readonly: bool,
    /// 设备状态
    pub status: u8,
    /// 设备特征位
    pub features: u64,
    /// 队列列表
    pub queues: Vec<VirtQueue>,
}

impl VirtioBlockDevice {
    /// 创建新的 Virtio 块设备
    pub fn new(name: &str, capacity_sectors: u64, block_size: u32) -> Self {
        let mut features = VirtioFeatures::new();
        features.set(0);  // VIRTIO_BLK_F_BARRIER
        features.set(1);  // VIRTIO_BLK_F_SIZE_MAX
        features.set(2);  // VIRTIO_BLK_F_SEG_MAX
        features.set(4);  // VIRTIO_BLK_F_RO（如果只读）
        features.set(5);  // VIRTIO_BLK_F_BLK_SIZE
        features.set(6);  // VIRTIO_BLK_F_FLUSH
        features.set(11); // VIRTIO_BLK_F_DISCARD
        features.set(13); // VIRTIO_BLK_F_WRITE_ZEROES

        Self {
            name: name.to_string(),
            block_size,
            capacity_sectors,
            readonly: false,
            status: 0,
            features: features.bits,
            queues: vec![VirtQueue::new(0, 256)],
        }
    }
}

impl VirtioDevice for VirtioBlockDevice {
    fn device_type(&self) -> VirtioDeviceType {
        VirtioDeviceType::Block
    }

    fn device_name(&self) -> &str {
        &self.name
    }

    fn get_features(&self) -> u64 {
        self.features
    }

    fn set_features(&mut self, features: u64) {
        self.features = features;
    }

    fn queue_count(&self) -> u16 {
        self.queues.len() as u16
    }

    fn activate(&mut self) -> Result<(), VirtError> {
        self.status = 4; // VIRTIO_CONFIG_S_DRIVER_OK
        for queue in &mut self.queues {
            queue.ready = true;
        }
        Ok(())
    }

    fn handle_queue_notify(&mut self, _queue_index: u16) -> Result<(), VirtError> {
        // 框架层面不实现实际 I/O，仅记录通知
        Ok(())
    }

    fn handle_config_read(&self, offset: u64, size: u8) -> Result<u64, VirtError> {
        match offset {
            0..=7 if size <= 8 => {
                // 容量（扇区数），低 64 位
                Ok(self.capacity_sectors)
            }
            8..=11 if size <= 4 => {
                // 块大小
                Ok(self.block_size as u64)
            }
            _ => Err(VirtError::DeviceError(format!(
                "块设备配置读取越界: offset={}, size={}",
                offset, size
            ))),
        }
    }

    fn handle_config_write(&mut self, _offset: u64, _size: u8, _value: u64) -> Result<(), VirtError> {
        // 块设备配置空间通常为只读
        Ok(())
    }

    fn reset(&mut self) {
        self.status = 0;
        for queue in &mut self.queues {
            queue.ready = false;
            queue.available.idx = 0;
            queue.used.idx = 0;
            queue.used.ring.clear();
        }
    }

    fn get_status(&self) -> u8 {
        self.status
    }

    fn set_status(&mut self, status: u8) {
        self.status = status;
    }
}

/// Virtio 网络设备
pub struct VirtioNetDevice {
    /// 设备名称
    pub name: String,
    /// MAC 地址
    pub mac_address: [u8; 6],
    /// 设备状态
    pub status: u8,
    /// 设备特征位
    pub features: u64,
    /// 队列列表
    pub queues: Vec<VirtQueue>,
    /// 接收包计数
    pub rx_packets: u64,
    /// 发送包计数
    pub tx_packets: u64,
}

impl VirtioNetDevice {
    /// 创建新的 Virtio 网络设备
    pub fn new(name: &str, mac_address: [u8; 6]) -> Self {
        let mut features = VirtioFeatures::new();
        features.set(0);  // VIRTIO_NET_F_CSUM
        features.set(1);  // VIRTIO_NET_F_GUEST_CSUM
        features.set(2);  // VIRTIO_NET_F_CTRL_GUEST_OFFLOADS
        features.set(5);  // VIRTIO_NET_F_MAC
        features.set(11); // VIRTIO_NET_F_MRG_RXBUF
        features.set(12); // VIRTIO_NET_F_STATUS

        Self {
            name: name.to_string(),
            mac_address,
            status: 0,
            features: features.bits,
            queues: vec![
                VirtQueue::new(0, 256), // 接收队列
                VirtQueue::new(1, 256), // 发送队列
            ],
            rx_packets: 0,
            tx_packets: 0,
        }
    }
}

impl VirtioDevice for VirtioNetDevice {
    fn device_type(&self) -> VirtioDeviceType {
        VirtioDeviceType::Net
    }

    fn device_name(&self) -> &str {
        &self.name
    }

    fn get_features(&self) -> u64 {
        self.features
    }

    fn set_features(&mut self, features: u64) {
        self.features = features;
    }

    fn queue_count(&self) -> u16 {
        self.queues.len() as u16
    }

    fn activate(&mut self) -> Result<(), VirtError> {
        self.status = 4; // VIRTIO_CONFIG_S_DRIVER_OK
        for queue in &mut self.queues {
            queue.ready = true;
        }
        Ok(())
    }

    fn handle_queue_notify(&mut self, queue_index: u16) -> Result<(), VirtError> {
        match queue_index {
            0 => self.rx_packets += 1,
            1 => self.tx_packets += 1,
            _ => {}
        }
        Ok(())
    }

    fn handle_config_read(&self, offset: u64, size: u8) -> Result<u64, VirtError> {
        match offset {
            0..=5 if size <= 6 => {
                // MAC 地址
                let mut val = 0u64;
                for i in 0..6 {
                    val |= (self.mac_address[i] as u64) << (i * 8);
                }
                Ok(val)
            }
            6 if size <= 2 => {
                // 状态
                Ok(1) // VIRTIO_NET_S_LINK_UP
            }
            _ => Err(VirtError::DeviceError(format!(
                "网络设备配置读取越界: offset={}, size={}",
                offset, size
            ))),
        }
    }

    fn handle_config_write(&mut self, _offset: u64, _size: u8, _value: u64) -> Result<(), VirtError> {
        // 网络设备配置空间通常为只读
        Ok(())
    }

    fn reset(&mut self) {
        self.status = 0;
        self.rx_packets = 0;
        self.tx_packets = 0;
        for queue in &mut self.queues {
            queue.ready = false;
            queue.available.idx = 0;
            queue.used.idx = 0;
            queue.used.ring.clear();
        }
    }

    fn get_status(&self) -> u8 {
        self.status
    }

    fn set_status(&mut self, status: u8) {
        self.status = status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtio_device_type_repr() {
        assert_eq!(VirtioDeviceType::Invalid as u32, 0);
        assert_eq!(VirtioDeviceType::Net as u32, 1);
        assert_eq!(VirtioDeviceType::Block as u32, 2);
        assert_eq!(VirtioDeviceType::Console as u32, 3);
        assert_eq!(VirtioDeviceType::Entropy as u32, 4);
        assert_eq!(VirtioDeviceType::Gpu as u32, 16);
        assert_eq!(VirtioDeviceType::Input as u32, 18);
    }

    #[test]
    fn test_virtio_features() {
        let mut features = VirtioFeatures::new();
        assert!(!features.has(0));
        assert!(!features.has(63));

        features.set(5);
        assert!(features.has(5));
        assert!(!features.has(4));

        features.clear(5);
        assert!(!features.has(5));

        // 超出范围的位
        features.set(64);
        assert!(!features.has(64));
        features.clear(64);
    }

    #[test]
    fn test_virtqueue_new() {
        let vq = VirtQueue::new(0, 256);
        assert_eq!(vq.index, 0);
        assert_eq!(vq.queue_size, 256);
        assert_eq!(vq.descriptors.len(), 256);
        assert!(!vq.ready);
        assert!(vq.is_empty());
        assert_eq!(vq.available_count(), 0);
    }

    #[test]
    fn test_virtqueue_is_empty() {
        let mut vq = VirtQueue::new(0, 16);
        assert!(vq.is_empty());

        vq.available.idx = 5;
        assert!(!vq.is_empty());
    }

    #[test]
    fn test_virtqueue_available_count() {
        let mut vq = VirtQueue::new(0, 16);
        assert_eq!(vq.available_count(), 0);

        vq.available.idx = 10;
        assert_eq!(vq.available_count(), 10);
    }

    #[test]
    fn test_virtio_block_device_new() {
        let device = VirtioBlockDevice::new("virtio-blk0", 1024 * 1024, 512);
        assert_eq!(device.name, "virtio-blk0");
        assert_eq!(device.capacity_sectors, 1024 * 1024);
        assert_eq!(device.block_size, 512);
        assert!(!device.readonly);
        assert_eq!(device.status, 0);
        assert_eq!(device.queue_count(), 1);
    }

    #[test]
    fn test_virtio_block_device_activate() {
        let mut device = VirtioBlockDevice::new("virtio-blk0", 1024, 512);
        assert!(device.activate().is_ok());
        assert_eq!(device.get_status(), 4);
        assert!(device.queues[0].ready);
    }

    #[test]
    fn test_virtio_block_device_reset() {
        let mut device = VirtioBlockDevice::new("virtio-blk0", 1024, 512);
        device.activate().unwrap();
        device.reset();
        assert_eq!(device.get_status(), 0);
        assert!(!device.queues[0].ready);
    }

    #[test]
    fn test_virtio_block_device_config_read() {
        let device = VirtioBlockDevice::new("virtio-blk0", 2048, 4096);

        // 读取容量
        let capacity = device.handle_config_read(0, 8).unwrap();
        assert_eq!(capacity, 2048);

        // 读取块大小
        let block_size = device.handle_config_read(8, 4).unwrap();
        assert_eq!(block_size, 4096);

        // 越界读取
        assert!(device.handle_config_read(100, 4).is_err());
    }

    #[test]
    fn test_virtio_block_device_features() {
        let mut device = VirtioBlockDevice::new("virtio-blk0", 1024, 512);
        let features = device.get_features();
        assert_ne!(features, 0);

        device.set_features(0);
        assert_eq!(device.get_features(), 0);
    }

    #[test]
    fn test_virtio_net_device_new() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = VirtioNetDevice::new("virtio-net0", mac);
        assert_eq!(device.name, "virtio-net0");
        assert_eq!(device.mac_address, mac);
        assert_eq!(device.status, 0);
        assert_eq!(device.queue_count(), 2);
        assert_eq!(device.rx_packets, 0);
        assert_eq!(device.tx_packets, 0);
    }

    #[test]
    fn test_virtio_net_device_activate() {
        let mac = [0x00; 6];
        let mut device = VirtioNetDevice::new("virtio-net0", mac);
        assert!(device.activate().is_ok());
        assert_eq!(device.get_status(), 4);
        assert!(device.queues[0].ready);
        assert!(device.queues[1].ready);
    }

    #[test]
    fn test_virtio_net_device_reset() {
        let mac = [0x00; 6];
        let mut device = VirtioNetDevice::new("virtio-net0", mac);
        device.activate().unwrap();
        device.handle_queue_notify(0).unwrap();
        device.handle_queue_notify(1).unwrap();
        assert_eq!(device.rx_packets, 1);
        assert_eq!(device.tx_packets, 1);

        device.reset();
        assert_eq!(device.get_status(), 0);
        assert_eq!(device.rx_packets, 0);
        assert_eq!(device.tx_packets, 0);
    }

    #[test]
    fn test_virtio_net_device_queue_notify() {
        let mac = [0x00; 6];
        let mut device = VirtioNetDevice::new("virtio-net0", mac);

        device.handle_queue_notify(0).unwrap();
        assert_eq!(device.rx_packets, 1);

        device.handle_queue_notify(1).unwrap();
        assert_eq!(device.tx_packets, 1);

        // 无效队列索引不产生效果
        device.handle_queue_notify(5).unwrap();
        assert_eq!(device.rx_packets, 1);
        assert_eq!(device.tx_packets, 1);
    }

    #[test]
    fn test_virtio_net_device_config_read() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = VirtioNetDevice::new("virtio-net0", mac);

        // 读取 MAC 地址
        let mac_val = device.handle_config_read(0, 6).unwrap();
        let mut expected = 0u64;
        for i in 0..6 {
            expected |= (mac[i] as u64) << (i * 8);
        }
        assert_eq!(mac_val, expected);

        // 读取状态
        let status = device.handle_config_read(6, 2).unwrap();
        assert_eq!(status, 1);

        // 越界读取
        assert!(device.handle_config_read(100, 4).is_err());
    }
}
