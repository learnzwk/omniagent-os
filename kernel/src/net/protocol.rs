//! 网络协议 Socket 定义与实现

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::net::address::SocketAddr;
use crate::net::error::NetError;

/// Socket 状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocketState {
    /// 已创建
    Created = 0,
    /// 已绑定
    Bound = 1,
    /// 正在监听
    Listening = 2,
    /// 已连接
    Connected = 3,
    /// 已关闭
    Closed = 4,
}

/// 关闭方式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownHow {
    /// 关闭读端
    Read = 0,
    /// 关闭写端
    Write = 1,
    /// 关闭读写两端
    Both = 2,
}

/// Socket 域（地址族）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDomain {
    /// IPv4 Internet 协议
    Inet = 2,
    /// IPv6 Internet 协议
    Inet6 = 10,
    /// Unix 本地协议
    Unix = 1,
}

/// Socket 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// 流式 Socket（TCP）
    Stream = 1,
    /// 数据报 Socket（UDP）
    Datagram = 2,
    /// 原始 Socket
    Raw = 3,
}

/// 协议 Socket trait
pub trait ProtocolSocket: Send + Sync {
    /// 绑定到指定地址
    fn bind(&mut self, addr: SocketAddr) -> Result<(), NetError>;
    /// 连接到指定地址
    fn connect(&mut self, addr: SocketAddr) -> Result<(), NetError>;
    /// 发送数据
    fn send(&mut self, data: &[u8]) -> Result<usize, NetError>;
    /// 接收数据
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError>;
    /// 发送数据到指定地址
    fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> Result<usize, NetError>;
    /// 从指定地址接收数据
    fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError>;
    /// 开始监听
    fn listen(&mut self, backlog: u32) -> Result<(), NetError>;
    /// 接受新连接
    fn accept(&mut self) -> Result<(Box<dyn ProtocolSocket>, SocketAddr), NetError>;
    /// 关闭连接
    fn shutdown(&mut self, how: ShutdownHow) -> Result<(), NetError>;
    /// 关闭 Socket
    fn close(&mut self) -> Result<(), NetError>;
    /// 获取本地地址
    fn local_addr(&self) -> Option<SocketAddr>;
    /// 获取远程地址
    fn remote_addr(&self) -> Option<SocketAddr>;
    /// 获取当前状态
    fn state(&self) -> SocketState;
}

/// TCP Socket 模拟实现
pub struct TcpSocket {
    /// 当前状态
    state: SocketState,
    /// 本地地址
    local_addr: Option<SocketAddr>,
    /// 远程地址
    remote_addr: Option<SocketAddr>,
    /// 接收缓冲区
    recv_buffer: Vec<u8>,
    /// 发送缓冲区
    send_buffer: Vec<u8>,
    /// 是否已连接
    connected: bool,
}

impl TcpSocket {
    /// 创建新的 TCP Socket
    pub fn new() -> Self {
        TcpSocket {
            state: SocketState::Created,
            local_addr: None,
            remote_addr: None,
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
            connected: false,
        }
    }
}

impl ProtocolSocket for TcpSocket {
    /// 绑定到指定地址，状态变为 Bound
    fn bind(&mut self, addr: SocketAddr) -> Result<(), NetError> {
        if self.state != SocketState::Created {
            return Err(NetError::AlreadyConnected);
        }
        self.local_addr = Some(addr);
        self.state = SocketState::Bound;
        Ok(())
    }

    /// 连接到指定地址，状态变为 Connected
    fn connect(&mut self, addr: SocketAddr) -> Result<(), NetError> {
        if self.connected {
            return Err(NetError::AlreadyConnected);
        }
        self.remote_addr = Some(addr);
        self.connected = true;
        self.state = SocketState::Connected;
        Ok(())
    }

    /// 将数据写入发送缓冲区
    fn send(&mut self, data: &[u8]) -> Result<usize, NetError> {
        if !self.connected {
            return Err(NetError::NotConnected);
        }
        let len = data.len();
        self.send_buffer.extend_from_slice(data);
        Ok(len)
    }

    /// 从接收缓冲区读取数据
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        if !self.connected {
            return Err(NetError::NotConnected);
        }
        if self.recv_buffer.is_empty() {
            return Err(NetError::WouldBlock);
        }
        let len = buf.len().min(self.recv_buffer.len());
        buf[..len].copy_from_slice(&self.recv_buffer[..len]);
        self.recv_buffer.drain(..len);
        Ok(len)
    }

    /// TCP Socket 不支持 send_to
    fn send_to(&mut self, _data: &[u8], _addr: SocketAddr) -> Result<usize, NetError> {
        Err(NetError::ProtocolError {
            reason: "TCP Socket 不支持 send_to".to_string(),
        })
    }

    /// TCP Socket 不支持 recv_from
    fn recv_from(&mut self, _buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        Err(NetError::ProtocolError {
            reason: "TCP Socket 不支持 recv_from".to_string(),
        })
    }

    /// 开始监听，状态变为 Listening
    fn listen(&mut self, _backlog: u32) -> Result<(), NetError> {
        if self.state != SocketState::Bound {
            return Err(NetError::NotConnected);
        }
        self.state = SocketState::Listening;
        Ok(())
    }

    /// 接受新连接（模拟实现）
    fn accept(&mut self) -> Result<(Box<dyn ProtocolSocket>, SocketAddr), NetError> {
        if self.state != SocketState::Listening {
            return Err(NetError::NotConnected);
        }
        let new_socket = TcpSocket::new();
        Ok((Box::new(new_socket), self.local_addr.unwrap()))
    }

    /// 关闭连接
    fn shutdown(&mut self, _how: ShutdownHow) -> Result<(), NetError> {
        self.connected = false;
        self.send_buffer.clear();
        self.recv_buffer.clear();
        Ok(())
    }

    /// 关闭 Socket，状态变为 Closed
    fn close(&mut self) -> Result<(), NetError> {
        self.state = SocketState::Closed;
        self.connected = false;
        self.send_buffer.clear();
        self.recv_buffer.clear();
        Ok(())
    }

    /// 获取本地地址
    fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// 获取远程地址
    fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// 获取当前状态
    fn state(&self) -> SocketState {
        self.state
    }
}

/// UDP Socket 模拟实现
pub struct UdpSocket {
    /// 当前状态
    state: SocketState,
    /// 本地地址
    local_addr: Option<SocketAddr>,
    /// 接收缓冲区（存储数据包和来源地址）
    recv_buffer: Vec<(Vec<u8>, SocketAddr)>,
}

impl UdpSocket {
    /// 创建新的 UDP Socket
    pub fn new() -> Self {
        UdpSocket {
            state: SocketState::Created,
            local_addr: None,
            recv_buffer: Vec::new(),
        }
    }
}

impl ProtocolSocket for UdpSocket {
    /// 绑定到指定地址
    fn bind(&mut self, addr: SocketAddr) -> Result<(), NetError> {
        if self.state != SocketState::Created {
            return Err(NetError::AlreadyConnected);
        }
        self.local_addr = Some(addr);
        self.state = SocketState::Bound;
        Ok(())
    }

    /// UDP Socket 不支持 connect
    fn connect(&mut self, _addr: SocketAddr) -> Result<(), NetError> {
        Err(NetError::ProtocolError {
            reason: "UDP Socket 不支持 connect".to_string(),
        })
    }

    /// UDP Socket 不支持 send（无连接）
    fn send(&mut self, _data: &[u8]) -> Result<usize, NetError> {
        Err(NetError::NotConnected)
    }

    /// UDP Socket 不支持 recv（无连接）
    fn recv(&mut self, _buf: &mut [u8]) -> Result<usize, NetError> {
        Err(NetError::NotConnected)
    }

    /// 发送数据到指定地址（模拟：将数据存入自己的接收缓冲区）
    fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> Result<usize, NetError> {
        let len = data.len();
        self.recv_buffer.push((data.to_vec(), addr));
        Ok(len)
    }

    /// 从接收缓冲区读取数据
    fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        if self.recv_buffer.is_empty() {
            return Err(NetError::WouldBlock);
        }
        let (data, addr) = self.recv_buffer.remove(0);
        let len = buf.len().min(data.len());
        buf[..len].copy_from_slice(&data[..len]);
        Ok((len, addr))
    }

    /// UDP Socket 不支持 listen
    fn listen(&mut self, _backlog: u32) -> Result<(), NetError> {
        Err(NetError::ProtocolError {
            reason: "UDP Socket 不支持 listen".to_string(),
        })
    }

    /// UDP Socket 不支持 accept
    fn accept(&mut self) -> Result<(Box<dyn ProtocolSocket>, SocketAddr), NetError> {
        Err(NetError::ProtocolError {
            reason: "UDP Socket 不支持 accept".to_string(),
        })
    }

    /// UDP Socket 不支持 shutdown
    fn shutdown(&mut self, _how: ShutdownHow) -> Result<(), NetError> {
        Err(NetError::ProtocolError {
            reason: "UDP Socket 不支持 shutdown".to_string(),
        })
    }

    /// 关闭 Socket
    fn close(&mut self) -> Result<(), NetError> {
        self.state = SocketState::Closed;
        self.recv_buffer.clear();
        Ok(())
    }

    /// 获取本地地址
    fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// 获取远程地址（UDP 无连接，返回 None）
    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }

    /// 获取当前状态
    fn state(&self) -> SocketState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::address::Ipv4Addr;

    #[test]
    fn test_tcp_new() {
        let socket = TcpSocket::new();
        assert_eq!(socket.state(), SocketState::Created);
        assert_eq!(socket.local_addr(), None);
        assert_eq!(socket.remote_addr(), None);
    }

    #[test]
    fn test_tcp_bind() {
        let mut socket = TcpSocket::new();
        let addr = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 8080);
        socket.bind(addr).unwrap();
        assert_eq!(socket.state(), SocketState::Bound);
        assert_eq!(socket.local_addr(), Some(addr));
    }

    #[test]
    fn test_tcp_connect() {
        let mut socket = TcpSocket::new();
        let local = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 8080);
        let remote = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 1), 80);
        socket.bind(local).unwrap();
        socket.connect(remote).unwrap();
        assert_eq!(socket.state(), SocketState::Connected);
        assert_eq!(socket.remote_addr(), Some(remote));
    }

    #[test]
    fn test_tcp_send_recv() {
        let mut socket = TcpSocket::new();
        let local = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 8080);
        let remote = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 1), 80);
        socket.bind(local).unwrap();
        socket.connect(remote).unwrap();

        // 模拟：将数据放入接收缓冲区
        let test_data = b"Hello, World!";
        socket.recv_buffer.extend_from_slice(test_data);

        let mut buf = [0u8; 64];
        let n = socket.recv(&mut buf).unwrap();
        assert_eq!(n, test_data.len());
        assert_eq!(&buf[..n], test_data);

        // 测试发送
        let send_data = b"Response";
        let sent = socket.send(send_data).unwrap();
        assert_eq!(sent, send_data.len());
        assert_eq!(socket.send_buffer.len(), send_data.len());
    }

    #[test]
    fn test_tcp_listen() {
        let mut socket = TcpSocket::new();
        let addr = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 80);
        socket.bind(addr).unwrap();
        socket.listen(128).unwrap();
        assert_eq!(socket.state(), SocketState::Listening);
    }

    #[test]
    fn test_tcp_shutdown() {
        let mut socket = TcpSocket::new();
        let local = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 8080);
        let remote = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 1), 80);
        socket.bind(local).unwrap();
        socket.connect(remote).unwrap();

        // 发送一些数据
        socket.send(b"data").unwrap();
        socket.recv_buffer.extend_from_slice(b"response");

        socket.shutdown(ShutdownHow::Both).unwrap();
        // 关闭后发送缓冲区和接收缓冲区应被清空
        assert!(socket.send_buffer.is_empty());
        assert!(socket.recv_buffer.is_empty());
    }

    #[test]
    fn test_tcp_close() {
        let mut socket = TcpSocket::new();
        let addr = SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1), 8080);
        socket.bind(addr).unwrap();
        socket.close().unwrap();
        assert_eq!(socket.state(), SocketState::Closed);
    }

    #[test]
    fn test_udp_new() {
        let socket = UdpSocket::new();
        assert_eq!(socket.state(), SocketState::Created);
        assert_eq!(socket.local_addr(), None);
    }

    #[test]
    fn test_udp_send_to_recv_from() {
        let mut socket = UdpSocket::new();
        let target = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 1), 1234);
        let data = b"UDP packet data";

        // 发送数据到目标地址（模拟）
        let sent = socket.send_to(data, target).unwrap();
        assert_eq!(sent, data.len());

        // 接收数据
        let mut buf = [0u8; 64];
        let (n, from) = socket.recv_from(&mut buf).unwrap();
        assert_eq!(n, data.len());
        assert_eq!(&buf[..n], data);
        assert_eq!(from, target);
    }

    #[test]
    fn test_socket_state_transitions() {
        let mut socket = TcpSocket::new();

        // Created -> Bound
        assert_eq!(socket.state(), SocketState::Created);
        let addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
        socket.bind(addr).unwrap();
        assert_eq!(socket.state(), SocketState::Bound);

        // Bound -> Listening
        socket.listen(10).unwrap();
        assert_eq!(socket.state(), SocketState::Listening);

        // Listening -> Closed
        socket.close().unwrap();
        assert_eq!(socket.state(), SocketState::Closed);

        // 测试另一条路径：Created -> Bound -> Connected
        let mut socket2 = TcpSocket::new();
        assert_eq!(socket2.state(), SocketState::Created);
        let addr2 = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1), 9090);
        socket2.bind(addr2).unwrap();
        assert_eq!(socket2.state(), SocketState::Bound);
        let remote = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 1), 80);
        socket2.connect(remote).unwrap();
        assert_eq!(socket2.state(), SocketState::Connected);

        // Connected -> Closed
        socket2.close().unwrap();
        assert_eq!(socket2.state(), SocketState::Closed);
    }
}
