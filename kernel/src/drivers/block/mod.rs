//! 块设备驱动框架
//!
//! 提供块设备的抽象接口、设备管理和内存磁盘实现。

pub mod device;
pub mod manager;
pub mod ramdisk;

pub use device::{BlockDevice, BlockError, BlockDeviceInfo};
pub use manager::BlockDeviceManager;
pub use manager::BLOCK_MANAGER;
pub use ramdisk::RamDisk;
