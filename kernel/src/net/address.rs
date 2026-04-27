//! 网络地址类型定义

/// IPv4 地址结构体
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Ipv4Addr {
    /// 四个字节的地址
    pub octets: [u8; 4],
}

impl Ipv4Addr {
    /// 创建新的 IPv4 地址
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr {
            octets: [a, b, c, d],
        }
    }

    /// 返回本地回环地址 127.0.0.1
    pub fn localhost() -> Self {
        Ipv4Addr::new(127, 0, 0, 1)
    }

    /// 返回未指定地址 0.0.0.0
    pub fn unspecified() -> Self {
        Ipv4Addr::new(0, 0, 0, 0)
    }

    /// 从 u32 值创建 IPv4 地址（大端序）
    pub fn from_u32(val: u32) -> Self {
        Ipv4Addr {
            octets: [
                (val >> 24) as u8,
                (val >> 16) as u8,
                (val >> 8) as u8,
                val as u8,
            ],
        }
    }

    /// 将 IPv4 地址转换为 u32 值（大端序）
    pub fn to_u32(&self) -> u32 {
        ((self.octets[0] as u32) << 24)
            | ((self.octets[1] as u32) << 16)
            | ((self.octets[2] as u32) << 8)
            | (self.octets[3] as u32)
    }

    /// 检查是否为回环地址（127.0.0.0/8）
    pub fn is_loopback(&self) -> bool {
        self.octets[0] == 127
    }

    /// 检查是否为链路本地地址（169.254.0.0/16）
    pub fn is_link_local(&self) -> bool {
        self.octets[0] == 169 && self.octets[1] == 254
    }
}

/// Socket 地址（IP + 端口）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketAddr {
    /// IP 地址
    pub ip: Ipv4Addr,
    /// 端口号
    pub port: u16,
}

impl SocketAddr {
    /// 创建新的 Socket 地址
    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        SocketAddr { ip, port }
    }

    /// 创建本地回环 Socket 地址
    pub fn localhost(port: u16) -> Self {
        SocketAddr {
            ip: Ipv4Addr::localhost(),
            port,
        }
    }
}

/// MAC 地址结构体
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddr {
    /// 六个字节的 MAC 地址
    pub octets: [u8; 6],
}

impl MacAddr {
    /// 创建新的 MAC 地址
    pub fn new(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) -> Self {
        MacAddr {
            octets: [a, b, c, d, e, f],
        }
    }

    /// 返回广播地址 FF:FF:FF:FF:FF:FF
    pub fn broadcast() -> Self {
        MacAddr::new(0xff, 0xff, 0xff, 0xff, 0xff, 0xff)
    }

    /// 检查是否为广播地址
    pub fn is_broadcast(&self) -> bool {
        self.octets == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    }

    /// 检查是否为多播地址（第一字节最低位为 1）
    pub fn is_multicast(&self) -> bool {
        (self.octets[0] & 0x01) != 0
    }

    /// 将 MAC 地址转换为字符串表示（如 "AA:BB:CC:DD:EE:FF"）
    pub fn to_string(&self) -> alloc::string::String {
        use alloc::format;
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.octets[0],
            self.octets[1],
            self.octets[2],
            self.octets[3],
            self.octets[4],
            self.octets[5]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_new() {
        let addr = Ipv4Addr::new(192, 168, 1, 1);
        assert_eq!(addr.octets, [192, 168, 1, 1]);
    }

    #[test]
    fn test_ipv4_localhost() {
        let addr = Ipv4Addr::localhost();
        assert_eq!(addr.octets, [127, 0, 0, 1]);
    }

    #[test]
    fn test_ipv4_from_to_u32() {
        let addr = Ipv4Addr::new(192, 168, 1, 100);
        let val = addr.to_u32();
        assert_eq!(val, 0xC0A80164);

        let addr2 = Ipv4Addr::from_u32(val);
        assert_eq!(addr2, addr);
    }

    #[test]
    fn test_ipv4_is_loopback() {
        assert!(Ipv4Addr::new(127, 0, 0, 1).is_loopback());
        assert!(Ipv4Addr::new(127, 255, 255, 255).is_loopback());
        assert!(!Ipv4Addr::new(192, 168, 1, 1).is_loopback());
    }

    #[test]
    fn test_socket_addr_new() {
        let addr = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 8080);
        assert_eq!(addr.ip.octets, [192, 168, 1, 1]);
        assert_eq!(addr.port, 8080);
    }

    #[test]
    fn test_mac_addr_new() {
        let mac = MacAddr::new(0x00, 0x11, 0x22, 0x33, 0x44, 0x55);
        assert_eq!(mac.octets, [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    }

    #[test]
    fn test_mac_addr_broadcast() {
        let broadcast = MacAddr::broadcast();
        assert!(broadcast.is_broadcast());
        assert_eq!(broadcast.octets, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

        let normal = MacAddr::new(0x00, 0x11, 0x22, 0x33, 0x44, 0x55);
        assert!(!normal.is_broadcast());
    }
}
