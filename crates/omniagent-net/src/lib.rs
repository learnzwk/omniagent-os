//! OmniAgent 网络栈
//!
//! 提供虚拟网络功能，包括 IP 地址、MAC 地址、TCP/UDP 套接字、
//! 网络接口管理和 DNS 解析等功能。

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt;

// === 网络基础类型 ===

/// IP 地址 (支持 IPv4 和 IPv6)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IpAddress {
    /// IPv4 地址
    V4([u8; 4]),
    /// IPv6 地址
    V6([u8; 16]),
}

impl IpAddress {
    /// 创建 IPv4 地址
    pub fn v4(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddress::V4([a, b, c, d])
    }

    /// 创建 IPv4 回环地址
    pub fn v4_loopback() -> Self {
        IpAddress::V4([127, 0, 0, 1])
    }

    /// 检查是否是回环地址
    pub fn is_loopback(&self) -> bool {
        match self {
            IpAddress::V4(addr) => addr[0] == 127,
            IpAddress::V6(addr) => addr[0..15] == [0; 15] && addr[15] == 1,
        }
    }

    /// 检查是否是私有地址
    pub fn is_private(&self) -> bool {
        match self {
            IpAddress::V4(addr) => {
                // 10.0.0.0/8
                if addr[0] == 10 {
                    return true;
                }
                // 172.16.0.0/12
                if addr[0] == 172 && (addr[1] >= 16 && addr[1] <= 31) {
                    return true;
                }
                // 192.168.0.0/16
                if addr[0] == 192 && addr[1] == 168 {
                    return true;
                }
                false
            }
            IpAddress::V6(addr) => {
                // fc00::/7 (唯一本地地址)
                (addr[0] & 0xfe) == 0xfc
            }
        }
    }

    /// 转换为字符串
    pub fn to_string(&self) -> String {
        match self {
            IpAddress::V4(addr) => format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
            IpAddress::V6(addr) => {
                let mut s = String::new();
                for i in 0..16 {
                    if i > 0 && i % 2 == 0 {
                        s.push(':');
                    }
                    if i % 2 == 0 {
                        s.push_str(&format!("{:x}", (addr[i] as u16) << 8 | addr[i + 1] as u16));
                    }
                }
                s
            }
        }
    }
}

impl fmt::Display for IpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddress::V4(addr) => write!(f, "{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
            IpAddress::V6(addr) => {
                write!(f, "[")?;
                for i in 0..16 {
                    if i > 0 && i % 2 == 0 {
                        write!(f, ":")?;
                    }
                    if i % 2 == 0 {
                        write!(f, "{:x}", (addr[i] as u16) << 8 | addr[i + 1] as u16)?;
                    }
                }
                write!(f, "]")
            }
        }
    }
}

/// IPv4 地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    /// 回环地址 127.0.0.1
    pub const LOOPBACK: Ipv4Addr = Ipv4Addr([127, 0, 0, 1]);
    /// 任意地址 0.0.0.0
    pub const ANY: Ipv4Addr = Ipv4Addr([0, 0, 0, 0]);

    /// 创建新的 IPv4 地址
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr([a, b, c, d])
    }

    /// 转换为 u32 (大端序)
    pub fn to_u32(&self) -> u32 {
        ((self.0[0] as u32) << 24)
            | ((self.0[1] as u32) << 16)
            | ((self.0[2] as u32) << 8)
            | (self.0[3] as u32)
    }

    /// 从 u32 创建 (大端序)
    pub fn from_u32(addr: u32) -> Self {
        Ipv4Addr([
            ((addr >> 24) & 0xFF) as u8,
            ((addr >> 16) & 0xFF) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ])
    }

    /// 检查是否是回环地址
    pub fn is_loopback(&self) -> bool {
        self.0[0] == 127
    }

    /// 检查是否是私有地址
    pub fn is_private(&self) -> bool {
        // 10.0.0.0/8
        if self.0[0] == 10 {
            return true;
        }
        // 172.16.0.0/12
        if self.0[0] == 172 && (self.0[1] >= 16 && self.0[1] <= 31) {
            return true;
        }
        // 192.168.0.0/16
        if self.0[0] == 192 && self.0[1] == 168 {
            return true;
        }
        false
    }

    /// 检查是否是链路本地地址 (169.254.0.0/16)
    pub fn is_link_local(&self) -> bool {
        self.0[0] == 169 && self.0[1] == 254
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

/// MAC 地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    /// 广播地址 FF:FF:FF:FF:FF:FF
    pub const BROADCAST: MacAddr = MacAddr([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

    /// 创建新的 MAC 地址
    pub const fn new(bytes: [u8; 6]) -> Self {
        MacAddr(bytes)
    }

    /// 检查是否是广播地址
    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    }

    /// 检查是否是多播地址 (第一字节最低位为 1)
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0x01) != 0
    }

    /// 转换为字符串 (格式: XX:XX:XX:XX:XX:XX)
    pub fn to_string(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }

    /// 从字节切片创建 MAC 地址
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut arr = [0u8; 6];
        let len = core::cmp::min(bytes.len(), 6);
        arr[..len].copy_from_slice(&bytes[..len]);
        MacAddr(arr)
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

/// 端口号
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Port(pub u16);

impl Port {
    /// HTTP 端口
    pub const HTTP: Port = Port(80);
    /// HTTPS 端口
    pub const HTTPS: Port = Port(443);
    /// SSH 端口
    pub const SSH: Port = Port(22);
    /// DNS 端口
    pub const DNS: Port = Port(53);
    /// DHCP 客户端端口
    pub const DHCP_CLIENT: Port = Port(68);
    /// DHCP 服务器端口
    pub const DHCP_SERVER: Port = Port(67);

    /// 检查是否是临时端口 (>= 49152)
    pub fn is_ephemeral(&self) -> bool {
        self.0 >= 49152
    }

    /// 检查是否是知名端口 (< 1024)
    pub fn is_well_known(&self) -> bool {
        self.0 < 1024
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Socket 地址
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SocketAddr {
    /// IPv4 地址
    V4 { ip: Ipv4Addr, port: Port },
    /// IPv6 地址
    V6 { ip: [u8; 16], port: Port },
}

impl SocketAddr {
    /// 创建 IPv4 Socket 地址
    pub fn v4(ip: Ipv4Addr, port: Port) -> Self {
        SocketAddr::V4 { ip, port }
    }

    /// 获取端口号
    pub fn port(&self) -> Port {
        match self {
            SocketAddr::V4 { port, .. } => *port,
            SocketAddr::V6 { port, .. } => *port,
        }
    }
}

impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketAddr::V4 { ip, port } => write!(f, "{}:{}", ip, port),
            SocketAddr::V6 { ip, port } => {
                write!(f, "[")?;
                for i in 0..16 {
                    if i > 0 && i % 2 == 0 {
                        write!(f, ":")?;
                    }
                    if i % 2 == 0 {
                        write!(f, "{:x}", (ip[i] as u16) << 8 | ip[i + 1] as u16)?;
                    }
                }
                write!(f, "]:{}", port)
            }
        }
    }
}

// === TCP ===

/// TCP 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TcpState {
    /// 关闭
    Closed = 0,
    /// 监听
    Listen = 1,
    /// SYN 已发送
    SynSent = 2,
    /// SYN 已接收
    SynReceived = 3,
    /// 已建立连接
    Established = 4,
    /// FIN 等待 1
    FinWait1 = 5,
    /// FIN 等待 2
    FinWait2 = 6,
    /// 关闭等待
    CloseWait = 7,
    /// 关闭中
    Closing = 8,
    /// 最后 ACK
    LastAck = 9,
    /// 时间等待
    TimeWait = 10,
}

impl fmt::Display for TcpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TcpState::Closed => write!(f, "CLOSED"),
            TcpState::Listen => write!(f, "LISTEN"),
            TcpState::SynSent => write!(f, "SYN_SENT"),
            TcpState::SynReceived => write!(f, "SYN_RECEIVED"),
            TcpState::Established => write!(f, "ESTABLISHED"),
            TcpState::FinWait1 => write!(f, "FIN_WAIT_1"),
            TcpState::FinWait2 => write!(f, "FIN_WAIT_2"),
            TcpState::CloseWait => write!(f, "CLOSE_WAIT"),
            TcpState::Closing => write!(f, "CLOSING"),
            TcpState::LastAck => write!(f, "LAST_ACK"),
            TcpState::TimeWait => write!(f, "TIME_WAIT"),
        }
    }
}

/// TCP 连接
pub struct TcpConnection {
    /// 本地地址
    pub local_addr: SocketAddr,
    /// 远程地址
    pub remote_addr: SocketAddr,
    /// 当前状态
    pub state: TcpState,
    /// 发送窗口大小
    pub send_window: u32,
    /// 接收窗口大小
    pub recv_window: u32,
    /// 已发送但未确认的字节数
    pub send_unack: u32,
    /// 下一个期望接收的序列号
    pub recv_next: u32,
    /// 创建时间
    pub created_at: u64,
    /// 最后活动时间
    pub last_activity: u64,
}

impl TcpConnection {
    /// 创建新的 TCP 连接
    pub fn new(local: SocketAddr, remote: SocketAddr) -> Self {
        TcpConnection {
            local_addr: local,
            remote_addr: remote,
            state: TcpState::Closed,
            send_window: 65535,
            recv_window: 65535,
            send_unack: 0,
            recv_next: 0,
            created_at: 0,
            last_activity: 0,
        }
    }

    /// TCP 状态转换 (简化版状态机)
    pub fn transition(&mut self, new_state: TcpState) -> Result<(), NetError> {
        // 验证状态转换是否合法
        let valid = match self.state {
            TcpState::Closed => matches!(new_state, TcpState::Listen | TcpState::SynSent),
            TcpState::Listen => matches!(new_state, TcpState::SynReceived | TcpState::Closed),
            TcpState::SynSent => matches!(new_state, TcpState::Established | TcpState::SynReceived | TcpState::Closed),
            TcpState::SynReceived => matches!(new_state, TcpState::Established | TcpState::FinWait1 | TcpState::Closed),
            TcpState::Established => matches!(new_state, TcpState::FinWait1 | TcpState::CloseWait | TcpState::Closed),
            TcpState::FinWait1 => matches!(new_state, TcpState::FinWait2 | TcpState::Closing | TcpState::TimeWait | TcpState::Closed),
            TcpState::FinWait2 => matches!(new_state, TcpState::TimeWait | TcpState::Closed),
            TcpState::CloseWait => matches!(new_state, TcpState::LastAck | TcpState::Closed),
            TcpState::Closing => matches!(new_state, TcpState::TimeWait | TcpState::Closed),
            TcpState::LastAck => matches!(new_state, TcpState::Closed),
            TcpState::TimeWait => matches!(new_state, TcpState::Closed),
        };

        if !valid {
            return Err(NetError::ProtocolError(format!(
                "无效的 TCP 状态转换: {:?} -> {:?}",
                self.state, new_state
            )));
        }

        self.state = new_state;
        self.last_activity = 0;
        Ok(())
    }

    /// 检查连接是否已建立
    pub fn is_established(&self) -> bool {
        self.state == TcpState::Established
    }
}

// === UDP ===

/// UDP 套接字
pub struct UdpSocket {
    /// 本地地址
    pub local_addr: SocketAddr,
    /// 远程地址 (connect 后设置)
    pub remote_addr: Option<SocketAddr>,
    /// 是否已绑定
    pub bound: bool,
    /// 创建时间
    pub created_at: u64,
}

impl UdpSocket {
    /// 创建新的 UDP 套接字
    pub fn new() -> Self {
        UdpSocket {
            local_addr: SocketAddr::v4(Ipv4Addr::ANY, Port(0)),
            remote_addr: None,
            bound: false,
            created_at: 0,
        }
    }

    /// 绑定到指定地址
    pub fn bind(&mut self, addr: SocketAddr) -> Result<(), NetError> {
        self.local_addr = addr;
        self.bound = true;
        Ok(())
    }

    /// 连接到指定远程地址
    pub fn connect(&mut self, addr: SocketAddr) -> Result<(), NetError> {
        if !self.bound {
            return Err(NetError::NotConnected);
        }
        self.remote_addr = Some(addr);
        Ok(())
    }

    /// 发送数据到指定地址
    pub fn send_to(&self, _addr: SocketAddr, data: &[u8]) -> Result<usize, NetError> {
        if !self.bound {
            return Err(NetError::NotConnected);
        }
        // 简化实现：直接返回发送的字节数
        Ok(data.len())
    }

    /// 接收数据
    pub fn recv_from(&self, _buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        if !self.bound {
            return Err(NetError::NotConnected);
        }
        // 简化实现：返回 0 字节和远程地址
        let remote = self.remote_addr.clone().unwrap_or_else(|| {
            SocketAddr::v4(Ipv4Addr::ANY, Port(0))
        });
        Ok((0, remote))
    }
}

// === 网络接口 ===

/// 网络接口状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InterfaceState {
    /// 接口已关闭
    Down = 0,
    /// 接口已启动
    Up = 1,
    /// 接口测试中
    Testing = 2,
}

impl fmt::Display for InterfaceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterfaceState::Down => write!(f, "DOWN"),
            InterfaceState::Up => write!(f, "UP"),
            InterfaceState::Testing => write!(f, "TESTING"),
        }
    }
}

/// 网络接口
pub struct NetworkInterface {
    /// 接口名称
    pub name: String,
    /// 接口索引
    pub index: u32,
    /// MAC 地址
    pub mac_addr: MacAddr,
    /// IP 地址
    pub ip_addr: Option<Ipv4Addr>,
    /// 子网掩码
    pub netmask: Option<Ipv4Addr>,
    /// 默认网关
    pub gateway: Option<Ipv4Addr>,
    /// 接口状态
    pub state: InterfaceState,
    /// 最大传输单元
    pub mtu: u16,
    /// 接收的数据包数
    pub rx_packets: u64,
    /// 发送的数据包数
    pub tx_packets: u64,
    /// 接收的字节数
    pub rx_bytes: u64,
    /// 发送的字节数
    pub tx_bytes: u64,
}

impl NetworkInterface {
    /// 创建新的网络接口
    pub fn new(name: &str, index: u32, mac: MacAddr) -> Self {
        NetworkInterface {
            name: name.to_string(),
            index,
            mac_addr: mac,
            ip_addr: None,
            netmask: None,
            gateway: None,
            state: InterfaceState::Down,
            mtu: 1500,
            rx_packets: 0,
            tx_packets: 0,
            rx_bytes: 0,
            tx_bytes: 0,
        }
    }

    /// 设置 IP 地址和子网掩码
    pub fn set_ip(&mut self, ip: Ipv4Addr, netmask: Ipv4Addr) {
        self.ip_addr = Some(ip);
        self.netmask = Some(netmask);
    }

    /// 设置默认网关
    pub fn set_gateway(&mut self, gateway: Ipv4Addr) {
        self.gateway = Some(gateway);
    }

    /// 检查接口是否已启动
    pub fn is_up(&self) -> bool {
        self.state == InterfaceState::Up
    }

    /// 记录接收的数据
    pub fn record_rx(&mut self, bytes: usize) {
        self.rx_packets += 1;
        self.rx_bytes += bytes as u64;
    }

    /// 记录发送的数据
    pub fn record_tx(&mut self, bytes: usize) {
        self.tx_packets += 1;
        self.tx_bytes += bytes as u64;
    }
}

// === 网络管理器 ===

/// 网络错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// 地址已使用
    AddressInUse(SocketAddr),
    /// 地址不可用
    AddressNotAvailable,
    /// 连接被拒绝
    ConnectionRefused,
    /// 连接被重置
    ConnectionReset,
    /// 连接超时
    ConnectionTimedOut,
    /// 主机未找到
    HostNotFound(String),
    /// 网络不可达
    NetworkUnreachable,
    /// 权限不足
    PermissionDenied,
    /// 无效地址
    InvalidAddress(String),
    /// 接口未找到
    InterfaceNotFound(String),
    /// 接口已关闭
    InterfaceDown(String),
    /// 端口已使用
    PortInUse(u16),
    /// 缓冲区太小
    BufferTooSmall,
    /// 未连接
    NotConnected,
    /// 已连接
    AlreadyConnected,
    /// 协议错误
    ProtocolError(String),
    /// 超时
    Timeout(u64),
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::AddressInUse(addr) => write!(f, "地址已使用: {}", addr),
            NetError::AddressNotAvailable => write!(f, "地址不可用"),
            NetError::ConnectionRefused => write!(f, "连接被拒绝"),
            NetError::ConnectionReset => write!(f, "连接被重置"),
            NetError::ConnectionTimedOut => write!(f, "连接超时"),
            NetError::HostNotFound(host) => write!(f, "主机未找到: {}", host),
            NetError::NetworkUnreachable => write!(f, "网络不可达"),
            NetError::PermissionDenied => write!(f, "权限不足"),
            NetError::InvalidAddress(addr) => write!(f, "无效地址: {}", addr),
            NetError::InterfaceNotFound(name) => write!(f, "接口未找到: {}", name),
            NetError::InterfaceDown(name) => write!(f, "接口已关闭: {}", name),
            NetError::PortInUse(port) => write!(f, "端口已使用: {}", port),
            NetError::BufferTooSmall => write!(f, "缓冲区太小"),
            NetError::NotConnected => write!(f, "未连接"),
            NetError::AlreadyConnected => write!(f, "已连接"),
            NetError::ProtocolError(msg) => write!(f, "协议错误: {}", msg),
            NetError::Timeout(ms) => write!(f, "超时: {}ms", ms),
        }
    }
}

#[cfg(test)]
impl std::error::Error for NetError {}

/// 网络统计信息
#[derive(Debug, Clone)]
pub struct NetStats {
    /// 接口数量
    pub interface_count: usize,
    /// TCP 连接数
    pub tcp_connections: usize,
    /// UDP 套接字数
    pub udp_sockets: usize,
    /// 总接收字节数
    pub total_rx_bytes: u64,
    /// 总发送字节数
    pub total_tx_bytes: u64,
    /// DNS 缓存大小
    pub dns_cache_size: usize,
}

/// 网络管理器
pub struct NetworkManager {
    /// 网络接口表
    interfaces: BTreeMap<String, NetworkInterface>,
    /// TCP 连接表
    tcp_connections: BTreeMap<u64, TcpConnection>,
    /// UDP 套接字表
    udp_sockets: BTreeMap<u64, UdpSocket>,
    /// DNS 缓存
    dns_cache: BTreeMap<String, Ipv4Addr>,
    /// 下一个连接 ID
    next_conn_id: u64,
}

impl NetworkManager {
    /// 创建新的网络管理器
    pub fn new() -> Self {
        NetworkManager {
            interfaces: BTreeMap::new(),
            tcp_connections: BTreeMap::new(),
            udp_sockets: BTreeMap::new(),
            dns_cache: BTreeMap::new(),
            next_conn_id: 1,
        }
    }

    /// 分配连接 ID
    fn alloc_conn_id(&mut self) -> u64 {
        let id = self.next_conn_id;
        self.next_conn_id += 1;
        id
    }

    /// 添加网络接口
    pub fn add_interface(&mut self, iface: NetworkInterface) {
        self.interfaces.insert(iface.name.clone(), iface);
    }

    /// 移除网络接口
    pub fn remove_interface(&mut self, name: &str) -> Result<(), NetError> {
        self.interfaces.remove(name)
            .ok_or_else(|| NetError::InterfaceNotFound(name.to_string()))?;
        Ok(())
    }

    /// 获取网络接口
    pub fn get_interface(&self, name: &str) -> Option<&NetworkInterface> {
        self.interfaces.get(name)
    }

    /// 获取可变网络接口
    pub fn get_interface_mut(&mut self, name: &str) -> Option<&mut NetworkInterface> {
        self.interfaces.get_mut(name)
    }

    /// 列出所有网络接口
    pub fn list_interfaces(&self) -> Vec<&NetworkInterface> {
        self.interfaces.values().collect()
    }

    /// 启动网络接口
    pub fn bring_up_interface(&mut self, name: &str) -> Result<(), NetError> {
        let iface = self.interfaces.get_mut(name)
            .ok_or_else(|| NetError::InterfaceNotFound(name.to_string()))?;
        iface.state = InterfaceState::Up;
        Ok(())
    }

    /// 关闭网络接口
    pub fn bring_down_interface(&mut self, name: &str) -> Result<(), NetError> {
        let iface = self.interfaces.get_mut(name)
            .ok_or_else(|| NetError::InterfaceNotFound(name.to_string()))?;
        iface.state = InterfaceState::Down;
        Ok(())
    }

    /// 建立 TCP 连接
    pub fn tcp_connect(&mut self, local: SocketAddr, remote: SocketAddr) -> Result<u64, NetError> {
        let conn_id = self.alloc_conn_id();
        let mut conn = TcpConnection::new(local, remote);

        // 模拟 TCP 三次握手
        conn.transition(TcpState::SynSent)?;
        conn.transition(TcpState::Established)?;

        self.tcp_connections.insert(conn_id, conn);
        Ok(conn_id)
    }

    /// 通过 TCP 连接发送数据
    pub fn tcp_send(&mut self, conn_id: u64, data: &[u8]) -> Result<usize, NetError> {
        let conn = self.tcp_connections.get_mut(&conn_id)
            .ok_or(NetError::NotConnected)?;

        if !conn.is_established() {
            return Err(NetError::NotConnected);
        }

        // 简化实现：记录发送的数据量
        conn.send_unack += data.len() as u32;
        conn.last_activity = 0;

        // 更新接口统计
        for iface in self.interfaces.values_mut() {
            if iface.is_up() {
                iface.record_tx(data.len());
                break;
            }
        }

        Ok(data.len())
    }

    /// 通过 TCP 连接接收数据
    pub fn tcp_recv(&mut self, conn_id: u64, _buf: &mut [u8]) -> Result<usize, NetError> {
        let conn = self.tcp_connections.get_mut(&conn_id)
            .ok_or(NetError::NotConnected)?;

        if !conn.is_established() {
            return Err(NetError::NotConnected);
        }

        // 简化实现：返回 0 字节
        conn.last_activity = 0;
        Ok(0)
    }

    /// 关闭 TCP 连接
    pub fn tcp_close(&mut self, conn_id: u64) -> Result<(), NetError> {
        let conn = self.tcp_connections.get_mut(&conn_id)
            .ok_or(NetError::NotConnected)?;

        // 模拟 TCP 四次挥手
        conn.transition(TcpState::FinWait1)?;
        conn.transition(TcpState::FinWait2)?;
        conn.transition(TcpState::TimeWait)?;
        conn.transition(TcpState::Closed)?;

        self.tcp_connections.remove(&conn_id);
        Ok(())
    }

    /// 获取 TCP 连接状态
    pub fn tcp_status(&self, conn_id: u64) -> Result<TcpState, NetError> {
        let conn = self.tcp_connections.get(&conn_id)
            .ok_or(NetError::NotConnected)?;
        Ok(conn.state)
    }

    /// 绑定 UDP 套接字
    pub fn udp_bind(&mut self, addr: SocketAddr) -> Result<u64, NetError> {
        // 检查端口是否已被使用
        let port = addr.port().0;
        for socket in self.udp_sockets.values() {
            if socket.bound && socket.local_addr.port().0 == port {
                return Err(NetError::PortInUse(port));
            }
        }

        let sock_id = self.alloc_conn_id();
        let mut socket = UdpSocket::new();
        socket.bind(addr)?;
        self.udp_sockets.insert(sock_id, socket);
        Ok(sock_id)
    }

    /// 通过 UDP 发送数据
    pub fn udp_send(&mut self, sock_id: u64, addr: SocketAddr, data: &[u8]) -> Result<usize, NetError> {
        let socket = self.udp_sockets.get(&sock_id)
            .ok_or(NetError::NotConnected)?;

        let sent = socket.send_to(addr, data)?;

        // 更新接口统计
        for iface in self.interfaces.values_mut() {
            if iface.is_up() {
                iface.record_tx(data.len());
                break;
            }
        }

        Ok(sent)
    }

    /// 关闭 UDP 套接字
    pub fn udp_close(&mut self, sock_id: u64) -> Result<(), NetError> {
        self.udp_sockets.remove(&sock_id)
            .ok_or(NetError::NotConnected)?;
        Ok(())
    }

    /// DNS 解析 (带缓存)
    pub fn dns_resolve(&mut self, hostname: &str) -> Result<Ipv4Addr, NetError> {
        // 先查缓存
        if let Some(&addr) = self.dns_cache.get(hostname) {
            return Ok(addr);
        }

        // 简化实现：对于 localhost 返回回环地址
        if hostname == "localhost" {
            let addr = Ipv4Addr::LOOPBACK;
            self.dns_cache.insert(hostname.to_string(), addr);
            return Ok(addr);
        }

        Err(NetError::HostNotFound(hostname.to_string()))
    }

    /// 添加 DNS 缓存条目
    pub fn dns_cache_add(&mut self, hostname: &str, addr: Ipv4Addr) {
        self.dns_cache.insert(hostname.to_string(), addr);
    }

    /// 获取网络统计信息
    pub fn stats(&self) -> NetStats {
        let total_rx_bytes: u64 = self.interfaces.values().map(|i| i.rx_bytes).sum();
        let total_tx_bytes: u64 = self.interfaces.values().map(|i| i.tx_bytes).sum();

        NetStats {
            interface_count: self.interfaces.len(),
            tcp_connections: self.tcp_connections.len(),
            udp_sockets: self.udp_sockets.len(),
            total_rx_bytes,
            total_tx_bytes,
            dns_cache_size: self.dns_cache.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Ipv4Addr 测试 ===

    #[test]
    fn test_ipv4_addr_new() {
        let addr = Ipv4Addr::new(192, 168, 1, 1);
        assert_eq!(addr.0, [192, 168, 1, 1]);
    }

    #[test]
    fn test_ipv4_addr_to_u32() {
        let addr = Ipv4Addr::new(192, 168, 1, 1);
        assert_eq!(addr.to_u32(), 0xC0A80101);

        let addr = Ipv4Addr::new(0, 0, 0, 1);
        assert_eq!(addr.to_u32(), 1);
    }

    #[test]
    fn test_ipv4_addr_from_u32() {
        let addr = Ipv4Addr::from_u32(0xC0A80101);
        assert_eq!(addr.0, [192, 168, 1, 1]);

        let addr = Ipv4Addr::from_u32(0x7F000001);
        assert_eq!(addr.0, [127, 0, 0, 1]);
    }

    #[test]
    fn test_ipv4_addr_roundtrip() {
        let original = Ipv4Addr::new(10, 0, 0, 1);
        let restored = Ipv4Addr::from_u32(original.to_u32());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_ipv4_addr_is_loopback() {
        assert!(Ipv4Addr::LOOPBACK.is_loopback());
        assert!(Ipv4Addr::new(127, 255, 255, 255).is_loopback());
        assert!(!Ipv4Addr::new(128, 0, 0, 1).is_loopback());
        assert!(!Ipv4Addr::ANY.is_loopback());
    }

    #[test]
    fn test_ipv4_addr_is_private() {
        // 10.0.0.0/8
        assert!(Ipv4Addr::new(10, 0, 0, 1).is_private());
        assert!(Ipv4Addr::new(10, 255, 255, 255).is_private());

        // 172.16.0.0/12
        assert!(Ipv4Addr::new(172, 16, 0, 1).is_private());
        assert!(Ipv4Addr::new(172, 31, 255, 255).is_private());
        assert!(!Ipv4Addr::new(172, 15, 0, 1).is_private());
        assert!(!Ipv4Addr::new(172, 32, 0, 1).is_private());

        // 192.168.0.0/16
        assert!(Ipv4Addr::new(192, 168, 0, 1).is_private());
        assert!(Ipv4Addr::new(192, 168, 255, 255).is_private());

        // 公网地址
        assert!(!Ipv4Addr::new(8, 8, 8, 8).is_private());
        assert!(!Ipv4Addr::new(1, 1, 1, 1).is_private());
    }

    #[test]
    fn test_ipv4_addr_is_link_local() {
        assert!(Ipv4Addr::new(169, 254, 0, 1).is_link_local());
        assert!(Ipv4Addr::new(169, 254, 255, 255).is_link_local());
        assert!(!Ipv4Addr::new(169, 255, 0, 1).is_link_local());
        assert!(!Ipv4Addr::new(192, 168, 1, 1).is_link_local());
    }

    #[test]
    fn test_ipv4_addr_display() {
        assert_eq!(format!("{}", Ipv4Addr::LOOPBACK), "127.0.0.1");
        assert_eq!(format!("{}", Ipv4Addr::ANY), "0.0.0.0");
        assert_eq!(format!("{}", Ipv4Addr::new(192, 168, 1, 1)), "192.168.1.1");
    }

    // === MacAddr 测试 ===

    #[test]
    fn test_mac_addr_new() {
        let mac = MacAddr::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(mac.0, [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    }

    #[test]
    fn test_mac_addr_broadcast() {
        assert!(MacAddr::BROADCAST.is_broadcast());
        assert!(!MacAddr::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]).is_broadcast());
    }

    #[test]
    fn test_mac_addr_multicast() {
        // 多播地址第一字节最低位为 1
        assert!(MacAddr::new([0x01, 0x00, 0x5E, 0x00, 0x00, 0x01]).is_multicast());
        assert!(MacAddr::new([0x33, 0x33, 0x00, 0x00, 0x00, 0x01]).is_multicast());
        assert!(!MacAddr::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]).is_multicast());
        // 广播也是多播
        assert!(MacAddr::BROADCAST.is_multicast());
    }

    #[test]
    fn test_mac_addr_to_string() {
        let mac = MacAddr::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(mac.to_string(), "00:11:22:33:44:55");
    }

    #[test]
    fn test_mac_addr_from_bytes() {
        let bytes: &[u8] = &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let mac = MacAddr::from_bytes(bytes);
        assert_eq!(mac.0, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_mac_addr_from_bytes_short() {
        let bytes: &[u8] = &[0xAA, 0xBB];
        let mac = MacAddr::from_bytes(bytes);
        assert_eq!(mac.0, [0xAA, 0xBB, 0x00, 0x00, 0x00, 0x00]);
    }

    // === Port 测试 ===

    #[test]
    fn test_port_constants() {
        assert_eq!(Port::HTTP.0, 80);
        assert_eq!(Port::HTTPS.0, 443);
        assert_eq!(Port::SSH.0, 22);
        assert_eq!(Port::DNS.0, 53);
        assert_eq!(Port::DHCP_CLIENT.0, 68);
        assert_eq!(Port::DHCP_SERVER.0, 67);
    }

    #[test]
    fn test_port_is_ephemeral() {
        assert!(Port(49152).is_ephemeral());
        assert!(Port(65535).is_ephemeral());
        assert!(!Port(49151).is_ephemeral());
        assert!(!Port(80).is_ephemeral());
    }

    #[test]
    fn test_port_is_well_known() {
        assert!(Port(80).is_well_known());
        assert!(Port(443).is_well_known());
        assert!(Port(0).is_well_known());
        assert!(Port(1023).is_well_known());
        assert!(!Port(1024).is_well_known());
        assert!(!Port(8080).is_well_known());
    }

    // === SocketAddr 测试 ===

    #[test]
    fn test_socket_addr_v4() {
        let addr = SocketAddr::v4(Ipv4Addr::new(192, 168, 1, 1), Port(8080));
        match addr {
            SocketAddr::V4 { ip, port } => {
                assert_eq!(ip, Ipv4Addr::new(192, 168, 1, 1));
                assert_eq!(port, Port(8080));
            }
            SocketAddr::V6 { .. } => panic!("期望 V4 地址"),
        }
    }

    #[test]
    fn test_socket_addr_port() {
        let addr = SocketAddr::v4(Ipv4Addr::LOOPBACK, Port::HTTP);
        assert_eq!(addr.port(), Port::HTTP);
    }

    #[test]
    fn test_socket_addr_display() {
        let addr = SocketAddr::v4(Ipv4Addr::new(192, 168, 1, 1), Port(8080));
        assert_eq!(format!("{}", addr), "192.168.1.1:8080");
    }

    // === IpAddress 测试 ===

    #[test]
    fn test_ip_address_v4() {
        let ip = IpAddress::v4(192, 168, 1, 1);
        assert_eq!(ip, IpAddress::V4([192, 168, 1, 1]));
    }

    #[test]
    fn test_ip_address_loopback() {
        assert!(IpAddress::v4_loopback().is_loopback());
        assert!(!IpAddress::v4(192, 168, 1, 1).is_loopback());
    }

    #[test]
    fn test_ip_address_private() {
        assert!(IpAddress::v4(10, 0, 0, 1).is_private());
        assert!(IpAddress::v4(192, 168, 1, 1).is_private());
        assert!(!IpAddress::v4(8, 8, 8, 8).is_private());
    }

    #[test]
    fn test_ip_address_to_string() {
        let ip = IpAddress::v4(192, 168, 1, 1);
        assert_eq!(ip.to_string(), "192.168.1.1");
    }

    // === TcpState 测试 ===

    #[test]
    fn test_tcp_state_values() {
        assert_eq!(TcpState::Closed as u8, 0);
        assert_eq!(TcpState::Listen as u8, 1);
        assert_eq!(TcpState::SynSent as u8, 2);
        assert_eq!(TcpState::SynReceived as u8, 3);
        assert_eq!(TcpState::Established as u8, 4);
        assert_eq!(TcpState::FinWait1 as u8, 5);
        assert_eq!(TcpState::FinWait2 as u8, 6);
        assert_eq!(TcpState::CloseWait as u8, 7);
        assert_eq!(TcpState::Closing as u8, 8);
        assert_eq!(TcpState::LastAck as u8, 9);
        assert_eq!(TcpState::TimeWait as u8, 10);
    }

    #[test]
    fn test_tcp_state_display() {
        assert_eq!(format!("{}", TcpState::Closed), "CLOSED");
        assert_eq!(format!("{}", TcpState::Established), "ESTABLISHED");
        assert_eq!(format!("{}", TcpState::TimeWait), "TIME_WAIT");
    }

    // === TcpConnection 测试 ===

    #[test]
    fn test_tcp_connection_new() {
        let local = SocketAddr::v4(Ipv4Addr::ANY, Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let conn = TcpConnection::new(local, remote);

        assert_eq!(conn.state, TcpState::Closed);
        assert_eq!(conn.send_window, 65535);
        assert_eq!(conn.recv_window, 65535);
        assert!(!conn.is_established());
    }

    #[test]
    fn test_tcp_connection_transition_valid() {
        let local = SocketAddr::v4(Ipv4Addr::ANY, Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let mut conn = TcpConnection::new(local, remote);

        // 完整的三次握手
        assert!(conn.transition(TcpState::SynSent).is_ok());
        assert_eq!(conn.state, TcpState::SynSent);
        assert!(conn.transition(TcpState::Established).is_ok());
        assert_eq!(conn.state, TcpState::Established);
        assert!(conn.is_established());
    }

    #[test]
    fn test_tcp_connection_transition_invalid() {
        let local = SocketAddr::v4(Ipv4Addr::ANY, Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let mut conn = TcpConnection::new(local, remote);

        // 从 Closed 不能直接跳到 Established
        let result = conn.transition(TcpState::Established);
        assert!(result.is_err());
        assert!(matches!(result, Err(NetError::ProtocolError(_))));
    }

    #[test]
    fn test_tcp_connection_full_lifecycle() {
        let local = SocketAddr::v4(Ipv4Addr::ANY, Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let mut conn = TcpConnection::new(local, remote);

        // 三次握手
        conn.transition(TcpState::SynSent).unwrap();
        conn.transition(TcpState::Established).unwrap();
        assert!(conn.is_established());

        // 四次挥手
        conn.transition(TcpState::FinWait1).unwrap();
        conn.transition(TcpState::FinWait2).unwrap();
        conn.transition(TcpState::TimeWait).unwrap();
        conn.transition(TcpState::Closed).unwrap();
        assert_eq!(conn.state, TcpState::Closed);
    }

    #[test]
    fn test_tcp_connection_listen_established() {
        let local = SocketAddr::v4(Ipv4Addr::ANY, Port(80));
        let remote = SocketAddr::v4(Ipv4Addr::new(10, 0, 0, 1), Port(54321));
        let mut conn = TcpConnection::new(local, remote);

        // 服务端：Listen -> SynReceived -> Established
        conn.transition(TcpState::Listen).unwrap();
        conn.transition(TcpState::SynReceived).unwrap();
        conn.transition(TcpState::Established).unwrap();
        assert!(conn.is_established());
    }

    // === UdpSocket 测试 ===

    #[test]
    fn test_udp_socket_new() {
        let socket = UdpSocket::new();
        assert!(!socket.bound);
        assert!(socket.remote_addr.is_none());
    }

    #[test]
    fn test_udp_socket_bind() {
        let mut socket = UdpSocket::new();
        let addr = SocketAddr::v4(Ipv4Addr::ANY, Port(8080));
        socket.bind(addr).unwrap();
        assert!(socket.bound);
    }

    #[test]
    fn test_udp_socket_connect() {
        let mut socket = UdpSocket::new();
        let bind_addr = SocketAddr::v4(Ipv4Addr::ANY, Port(8080));
        socket.bind(bind_addr).unwrap();

        let remote = SocketAddr::v4(Ipv4Addr::new(8, 8, 8, 8), Port::DNS);
        socket.connect(remote).unwrap();
        assert!(socket.remote_addr.is_some());
    }

    #[test]
    fn test_udp_socket_connect_without_bind() {
        let mut socket = UdpSocket::new();
        let remote = SocketAddr::v4(Ipv4Addr::new(8, 8, 8, 8), Port::DNS);
        let result = socket.connect(remote);
        assert!(matches!(result, Err(NetError::NotConnected)));
    }

    #[test]
    fn test_udp_socket_send_to() {
        let mut socket = UdpSocket::new();
        socket.bind(SocketAddr::v4(Ipv4Addr::ANY, Port(8080))).unwrap();

        let dest = SocketAddr::v4(Ipv4Addr::new(8, 8, 8, 8), Port::DNS);
        let sent = socket.send_to(dest, b"hello").unwrap();
        assert_eq!(sent, 5);
    }

    #[test]
    fn test_udp_socket_send_to_unbound() {
        let socket = UdpSocket::new();
        let dest = SocketAddr::v4(Ipv4Addr::new(8, 8, 8, 8), Port::DNS);
        let result = socket.send_to(dest, b"hello");
        assert!(matches!(result, Err(NetError::NotConnected)));
    }

    #[test]
    fn test_udp_socket_recv_from() {
        let mut socket = UdpSocket::new();
        socket.bind(SocketAddr::v4(Ipv4Addr::ANY, Port(8080))).unwrap();

        let mut buf = [0u8; 64];
        let (read, _addr) = socket.recv_from(&mut buf).unwrap();
        assert_eq!(read, 0);
    }

    // === NetworkInterface 测试 ===

    #[test]
    fn test_network_interface_new() {
        let iface = NetworkInterface::new("eth0", 0, MacAddr::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]));
        assert_eq!(iface.name, "eth0");
        assert_eq!(iface.index, 0);
        assert_eq!(iface.state, InterfaceState::Down);
        assert_eq!(iface.mtu, 1500);
        assert_eq!(iface.rx_packets, 0);
        assert_eq!(iface.tx_packets, 0);
        assert!(iface.ip_addr.is_none());
        assert!(iface.gateway.is_none());
    }

    #[test]
    fn test_network_interface_set_ip() {
        let mut iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        iface.set_ip(Ipv4Addr::new(192, 168, 1, 100), Ipv4Addr::new(255, 255, 255, 0));

        assert_eq!(iface.ip_addr, Some(Ipv4Addr::new(192, 168, 1, 100)));
        assert_eq!(iface.netmask, Some(Ipv4Addr::new(255, 255, 255, 0)));
    }

    #[test]
    fn test_network_interface_set_gateway() {
        let mut iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        iface.set_gateway(Ipv4Addr::new(192, 168, 1, 1));

        assert_eq!(iface.gateway, Some(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn test_network_interface_is_up() {
        let mut iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        assert!(!iface.is_up());

        iface.state = InterfaceState::Up;
        assert!(iface.is_up());
    }

    #[test]
    fn test_network_interface_record_rx() {
        let mut iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        iface.record_rx(100);
        assert_eq!(iface.rx_packets, 1);
        assert_eq!(iface.rx_bytes, 100);

        iface.record_rx(200);
        assert_eq!(iface.rx_packets, 2);
        assert_eq!(iface.rx_bytes, 300);
    }

    #[test]
    fn test_network_interface_record_tx() {
        let mut iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        iface.record_tx(50);
        assert_eq!(iface.tx_packets, 1);
        assert_eq!(iface.tx_bytes, 50);

        iface.record_tx(150);
        assert_eq!(iface.tx_packets, 2);
        assert_eq!(iface.tx_bytes, 200);
    }

    // === InterfaceState 测试 ===

    #[test]
    fn test_interface_state_values() {
        assert_eq!(InterfaceState::Down as u8, 0);
        assert_eq!(InterfaceState::Up as u8, 1);
        assert_eq!(InterfaceState::Testing as u8, 2);
    }

    #[test]
    fn test_interface_state_display() {
        assert_eq!(format!("{}", InterfaceState::Down), "DOWN");
        assert_eq!(format!("{}", InterfaceState::Up), "UP");
        assert_eq!(format!("{}", InterfaceState::Testing), "TESTING");
    }

    // === NetworkManager 测试 ===

    #[test]
    fn test_network_manager_new() {
        let mgr = NetworkManager::new();
        assert_eq!(mgr.interfaces.len(), 0);
        assert_eq!(mgr.tcp_connections.len(), 0);
        assert_eq!(mgr.udp_sockets.len(), 0);
    }

    #[test]
    fn test_network_manager_add_interface() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]));
        mgr.add_interface(iface);

        assert!(mgr.get_interface("eth0").is_some());
        assert_eq!(mgr.list_interfaces().len(), 1);
    }

    #[test]
    fn test_network_manager_remove_interface() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        mgr.add_interface(iface);

        mgr.remove_interface("eth0").unwrap();
        assert!(mgr.get_interface("eth0").is_none());
    }

    #[test]
    fn test_network_manager_remove_nonexistent_interface() {
        let mut mgr = NetworkManager::new();
        let result = mgr.remove_interface("nonexistent");
        assert!(matches!(result, Err(NetError::InterfaceNotFound(_))));
    }

    #[test]
    fn test_network_manager_bring_up_down() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        mgr.add_interface(iface);

        mgr.bring_up_interface("eth0").unwrap();
        assert!(mgr.get_interface("eth0").unwrap().is_up());

        mgr.bring_down_interface("eth0").unwrap();
        assert!(!mgr.get_interface("eth0").unwrap().is_up());
    }

    #[test]
    fn test_network_manager_tcp_connect() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        mgr.add_interface(iface);
        mgr.bring_up_interface("eth0").unwrap();

        let local = SocketAddr::v4(Ipv4Addr::new(192, 168, 1, 100), Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);

        let conn_id = mgr.tcp_connect(local, remote).unwrap();
        assert!(conn_id > 0);

        let status = mgr.tcp_status(conn_id).unwrap();
        assert_eq!(status, TcpState::Established);
    }

    #[test]
    fn test_network_manager_tcp_send_recv() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        mgr.add_interface(iface);
        mgr.bring_up_interface("eth0").unwrap();

        let local = SocketAddr::v4(Ipv4Addr::new(192, 168, 1, 100), Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let conn_id = mgr.tcp_connect(local, remote).unwrap();

        let sent = mgr.tcp_send(conn_id, b"GET / HTTP/1.1").unwrap();
        assert_eq!(sent, 14);

        let mut buf = [0u8; 64];
        let recv = mgr.tcp_recv(conn_id, &mut buf).unwrap();
        assert_eq!(recv, 0); // 简化实现返回 0
    }

    #[test]
    fn test_network_manager_tcp_close() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        mgr.add_interface(iface);
        mgr.bring_up_interface("eth0").unwrap();

        let local = SocketAddr::v4(Ipv4Addr::ANY, Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let conn_id = mgr.tcp_connect(local, remote).unwrap();

        mgr.tcp_close(conn_id).unwrap();

        let result = mgr.tcp_status(conn_id);
        assert!(matches!(result, Err(NetError::NotConnected)));
    }

    #[test]
    fn test_network_manager_udp_bind() {
        let mut mgr = NetworkManager::new();
        let addr = SocketAddr::v4(Ipv4Addr::ANY, Port(8080));
        let sock_id = mgr.udp_bind(addr).unwrap();
        assert!(sock_id > 0);
    }

    #[test]
    fn test_network_manager_udp_bind_port_in_use() {
        let mut mgr = NetworkManager::new();
        let addr = SocketAddr::v4(Ipv4Addr::ANY, Port(8080));
        mgr.udp_bind(addr).unwrap();

        let addr2 = SocketAddr::v4(Ipv4Addr::new(127, 0, 0, 1), Port(8080));
        let result = mgr.udp_bind(addr2);
        assert!(matches!(result, Err(NetError::PortInUse(8080))));
    }

    #[test]
    fn test_network_manager_udp_send_close() {
        let mut mgr = NetworkManager::new();
        let iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        mgr.add_interface(iface);
        mgr.bring_up_interface("eth0").unwrap();

        let addr = SocketAddr::v4(Ipv4Addr::ANY, Port(8080));
        let sock_id = mgr.udp_bind(addr).unwrap();

        let dest = SocketAddr::v4(Ipv4Addr::new(8, 8, 8, 8), Port::DNS);
        let sent = mgr.udp_send(sock_id, dest, b"test").unwrap();
        assert_eq!(sent, 4);

        mgr.udp_close(sock_id).unwrap();

        let result = mgr.udp_close(sock_id);
        assert!(matches!(result, Err(NetError::NotConnected)));
    }

    #[test]
    fn test_network_manager_dns_resolve() {
        let mut mgr = NetworkManager::new();

        // localhost 应该能解析
        let addr = mgr.dns_resolve("localhost").unwrap();
        assert_eq!(addr, Ipv4Addr::LOOPBACK);

        // 第二次应该从缓存获取
        let addr2 = mgr.dns_resolve("localhost").unwrap();
        assert_eq!(addr2, Ipv4Addr::LOOPBACK);
    }

    #[test]
    fn test_network_manager_dns_resolve_not_found() {
        let mut mgr = NetworkManager::new();
        let result = mgr.dns_resolve("nonexistent.example.com");
        assert!(matches!(result, Err(NetError::HostNotFound(_))));
    }

    #[test]
    fn test_network_manager_dns_cache_add() {
        let mut mgr = NetworkManager::new();

        mgr.dns_cache_add("example.com", Ipv4Addr::new(93, 184, 216, 34));
        let addr = mgr.dns_resolve("example.com").unwrap();
        assert_eq!(addr, Ipv4Addr::new(93, 184, 216, 34));
    }

    #[test]
    fn test_network_manager_stats() {
        let mut mgr = NetworkManager::new();

        let mut iface = NetworkInterface::new("eth0", 0, MacAddr::BROADCAST);
        iface.set_ip(Ipv4Addr::new(192, 168, 1, 100), Ipv4Addr::new(255, 255, 255, 0));
        iface.state = InterfaceState::Up;
        mgr.add_interface(iface);

        let local = SocketAddr::v4(Ipv4Addr::new(192, 168, 1, 100), Port(12345));
        let remote = SocketAddr::v4(Ipv4Addr::new(93, 184, 216, 34), Port::HTTP);
        let conn_id = mgr.tcp_connect(local, remote).unwrap();
        mgr.tcp_send(conn_id, b"hello").unwrap();

        mgr.udp_bind(SocketAddr::v4(Ipv4Addr::ANY, Port(8080))).unwrap();
        mgr.dns_cache_add("example.com", Ipv4Addr::new(93, 184, 216, 34));

        let stats = mgr.stats();
        assert_eq!(stats.interface_count, 1);
        assert_eq!(stats.tcp_connections, 1);
        assert_eq!(stats.udp_sockets, 1);
        assert_eq!(stats.total_tx_bytes, 5); // "hello" = 5 bytes
        assert_eq!(stats.dns_cache_size, 1); // example.com
    }

    // === NetError 测试 ===

    #[test]
    fn test_net_error_display() {
        assert_eq!(
            format!("{}", NetError::ConnectionRefused),
            "连接被拒绝"
        );
        assert_eq!(
            format!("{}", NetError::NetworkUnreachable),
            "网络不可达"
        );
        assert_eq!(
            format!("{}", NetError::HostNotFound("example.com".into())),
            "主机未找到: example.com"
        );
        assert_eq!(
            format!("{}", NetError::PortInUse(8080)),
            "端口已使用: 8080"
        );
        assert_eq!(
            format!("{}", NetError::Timeout(5000)),
            "超时: 5000ms"
        );
        assert_eq!(
            format!("{}", NetError::InterfaceNotFound("eth0".into())),
            "接口未找到: eth0"
        );
    }

    #[test]
    fn test_net_error_equality() {
        assert_eq!(NetError::ConnectionRefused, NetError::ConnectionRefused);
        assert_eq!(NetError::PortInUse(80), NetError::PortInUse(80));
        assert_ne!(NetError::PortInUse(80), NetError::PortInUse(443));
        assert_eq!(
            NetError::HostNotFound("a".into()),
            NetError::HostNotFound("a".into())
        );
        assert_ne!(
            NetError::HostNotFound("a".into()),
            NetError::HostNotFound("b".into())
        );
    }
}
