//! 网络层模块
//!
//! 提供 IPv4/IPv6 地址、MAC 地址、TCP/UDP Socket 和 Socket 表管理功能。

pub mod error;
pub mod address;
pub mod protocol;
pub mod socket_table;

pub use error::NetError;
pub use address::{Ipv4Addr, SocketAddr, MacAddr};
pub use protocol::{ProtocolSocket, SocketState, ShutdownHow, SocketDomain, SocketType, TcpSocket, UdpSocket};
pub use socket_table::{SocketTable, SocketEntry};
