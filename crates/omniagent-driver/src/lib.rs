#![no_std]

#[cfg(test)]
extern crate std;

use core::fmt;

/// 驱动 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DriverId(pub u32);

impl DriverId {
    pub const INVALID: DriverId = DriverId(0);
    pub fn new(id: u32) -> Self { DriverId(id) }
    pub fn is_valid(&self) -> bool { self.0 != 0 }
}

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeviceType {
    Block = 0,
    Network = 1,
    Display = 2,
    Input = 3,
    Audio = 4,
    Serial = 5,
    Usb = 6,
    Virtio = 7,
    Custom = 255,
}

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: &'static str,
    pub device_type: DeviceType,
    pub vendor_id: u16,
    pub device_id: u16,
    pub irq: Option<u32>,
    pub mmio_base: Option<u64>,
    pub mmio_size: Option<usize>,
}

/// 驱动状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DriverState {
    Uninitialized = 0,
    Initializing = 1,
    Running = 2,
    Suspended = 3,
    Error = 4,
    Removed = 5,
}

/// 驱动错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverError {
    NotInitialized,
    AlreadyInitialized,
    NotFound,
    Busy,
    Timeout,
    InvalidArgument,
    IoError,
    NoMemory,
    Unsupported,
    PermissionDenied,
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "driver not initialized"),
            Self::AlreadyInitialized => write!(f, "driver already initialized"),
            Self::NotFound => write!(f, "device not found"),
            Self::Busy => write!(f, "device busy"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::IoError => write!(f, "I/O error"),
            Self::NoMemory => write!(f, "out of memory"),
            Self::Unsupported => write!(f, "operation unsupported"),
            Self::PermissionDenied => write!(f, "permission denied"),
        }
    }
}

/// 设备驱动 trait (对象安全)
pub trait DeviceDriver: Send + Sync {
    /// 返回驱动 ID
    fn id(&self) -> DriverId;

    /// 返回设备信息
    fn device_info(&self) -> &DeviceInfo;

    /// 初始化驱动
    fn init(&mut self) -> Result<(), DriverError>;

    /// 获取当前状态
    fn state(&self) -> DriverState;

    /// 暂停驱动
    fn suspend(&mut self) -> Result<(), DriverError> {
        Err(DriverError::Unsupported)
    }

    /// 恢复驱动
    fn resume(&mut self) -> Result<(), DriverError> {
        Err(DriverError::Unsupported)
    }

    /// 卸载驱动
    fn deinit(&mut self) -> Result<(), DriverError>;

    /// 处理中断
    fn handle_irq(&mut self, _irq: u32) -> Result<(), DriverError> {
        Ok(())
    }
}

/// 可读设备 trait
pub trait ReadableDevice: DeviceDriver {
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, DriverError>;
}

/// 可写设备 trait
pub trait WritableDevice: DeviceDriver {
    fn write(&mut self, offset: u64, data: &[u8]) -> Result<usize, DriverError>;
}

/// 块设备 trait
pub trait BlockDevice: DeviceDriver {
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), DriverError>;
    fn write_block(&mut self, block_id: u64, data: &[u8]) -> Result<(), DriverError>;
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDevice {
        info: DeviceInfo,
        state: DriverState,
    }

    impl MockDevice {
        fn new() -> Self {
            Self {
                info: DeviceInfo {
                    name: "mock-device",
                    device_type: DeviceType::Block,
                    vendor_id: 0x1234,
                    device_id: 0x5678,
                    irq: Some(4),
                    mmio_base: Some(0xFE000000),
                    mmio_size: Some(0x1000),
                },
                state: DriverState::Uninitialized,
            }
        }
    }

    impl DeviceDriver for MockDevice {
        fn id(&self) -> DriverId { DriverId::new(1) }
        fn device_info(&self) -> &DeviceInfo { &self.info }
        fn init(&mut self) -> Result<(), DriverError> {
            self.state = DriverState::Running;
            Ok(())
        }
        fn state(&self) -> DriverState { self.state }
        fn deinit(&mut self) -> Result<(), DriverError> {
            self.state = DriverState::Removed;
            Ok(())
        }
    }

    #[test]
    fn test_driver_id() {
        let id = DriverId::INVALID;
        assert!(!id.is_valid());
        let id = DriverId::new(42);
        assert!(id.is_valid());
    }

    #[test]
    fn test_device_type_values() {
        assert_eq!(DeviceType::Block as u8, 0);
        assert_eq!(DeviceType::Network as u8, 1);
        assert_eq!(DeviceType::Virtio as u8, 7);
    }

    #[test]
    fn test_device_info() {
        let info = DeviceInfo {
            name: "test",
            device_type: DeviceType::Serial,
            vendor_id: 0, device_id: 0,
            irq: None, mmio_base: None, mmio_size: None,
        };
        assert_eq!(info.name, "test");
        assert_eq!(info.device_type, DeviceType::Serial);
    }

    #[test]
    fn test_driver_state_transitions() {
        let mut dev = MockDevice::new();
        assert_eq!(dev.state(), DriverState::Uninitialized);
        dev.init().unwrap();
        assert_eq!(dev.state(), DriverState::Running);
        dev.deinit().unwrap();
        assert_eq!(dev.state(), DriverState::Removed);
    }

    #[test]
    fn test_driver_error_display() {
        use std::string::ToString;
        let err = DriverError::NotFound;
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_device_driver_is_object_safe() {
        fn takes_driver(_: &dyn DeviceDriver) {}
        let dev = MockDevice::new();
        takes_driver(&dev);
    }

    #[test]
    fn test_driver_trait_default_impls() {
        let mut dev = MockDevice::new();
        dev.init().unwrap();
        // suspend and resume have default impls returning Unsupported
        assert_eq!(dev.suspend(), Err(DriverError::Unsupported));
        assert_eq!(dev.resume(), Err(DriverError::Unsupported));
        // handle_irq has default impl returning Ok
        assert_eq!(dev.handle_irq(4), Ok(()));
    }

    #[test]
    fn test_driver_state_equality() {
        assert_eq!(DriverState::Running, DriverState::Running);
        assert_ne!(DriverState::Running, DriverState::Error);
    }

    #[test]
    fn test_device_info_with_mmio() {
        let info = DeviceInfo {
            name: "virtio-blk",
            device_type: DeviceType::Virtio,
            vendor_id: 0x1AF4, device_id: 0x1001,
            irq: Some(5), mmio_base: Some(0xFE000000), mmio_size: Some(0x1000),
        };
        assert_eq!(info.vendor_id, 0x1AF4);
        assert_eq!(info.mmio_base, Some(0xFE000000));
    }

    #[test]
    fn test_driver_id_hash() {
        fn hash_id(id: DriverId) -> u64 {
            let mut h = 0u64;
            // Simple hash for testing
            h ^= id.0 as u64;
            h
        }
        let id1 = DriverId::new(42);
        let id2 = DriverId::new(42);
        assert_eq!(hash_id(id1), hash_id(id2));
    }
}
