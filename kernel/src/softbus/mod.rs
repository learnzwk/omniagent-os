//! 分布式软总线模块
//!
//! 模仿鸿蒙 DSoftBus（分布式软总线）思想，实现：
//! - 设备发现与自动组网
//! - 多协议连接管理
//! - 统一传输层
//!
//! 软总线是鸿蒙分布式能力的核心基础设施，为上层应用提供
//! 设备间无缝通信能力。

pub mod error;
pub mod discovery;
pub mod connection;
pub mod transport;

pub use error::SoftBusError;
pub use discovery::{DeviceInfo, DeviceType, DeviceDiscovery, DISCOVERY};
pub use connection::{ConnectionInfo, ConnectionState, ConnectionType, ConnectionManager, CONNECTION_MANAGER};
pub use transport::{TransportMessage, MessageType, TransportStats, TransportLayer, TRANSPORT};
