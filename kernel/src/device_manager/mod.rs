//! 设备管理框架模块
//!
//! 提供设备抽象层、热插拔和驱动匹配功能。
//! 包含设备管理和驱动匹配两个子模块。

pub mod device;
pub mod driver;

pub use device::{DeviceManagerError, DeviceClass, DeviceStatus, DeviceDescriptor, DeviceManager, DEVICE_MANAGER};
pub use driver::{DriverDescriptor, DriverMatcher, DRIVER_MATCHER};
