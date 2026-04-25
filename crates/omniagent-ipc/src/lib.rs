#![cfg_attr(not(test), no_std)]

#[cfg(not(test))]
use core::fmt;

#[cfg(test)]
use std::fmt;

/// IPC 消息头大小 (固定 64 字节)
pub const MESSAGE_HEADER_SIZE: usize = 64;

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Invalid = 0,
    Request = 1,
    Response = 2,
    Notification = 3,
    Error = 4,
    StreamBegin = 5,
    StreamData = 6,
    StreamEnd = 7,
}

/// 消息标志位
#[derive(Debug, Clone, Copy)]
pub struct MessageFlags(u32);

impl MessageFlags {
    pub const NONE: u32 = 0;
    pub const URGENT: u32 = 1 << 0;
    pub const NO_REPLY: u32 = 1 << 1;
    pub const BROADCAST: u32 = 1 << 2;
    pub const ZERO_COPY: u32 = 1 << 3;
    pub const ENCRYPTED: u32 = 1 << 4;

    pub fn new() -> Self { Self(0) }
    pub fn bits(&self) -> u32 { self.0 }
    pub fn contains(&self, flag: u32) -> bool { (self.0 & flag) != 0 }
    pub fn set(&mut self, flag: u32) { self.0 |= flag; }
    pub fn clear(&mut self, flag: u32) { self.0 &= !flag; }
}

/// 消息优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum MessagePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// 消息头 (固定 64 字节)
#[derive(Clone, Copy)]
#[repr(C)]
pub struct MessageHeader {
    pub msg_type: MessageType,
    pub flags: MessageFlags,
    pub priority: MessagePriority,
    pub src_port: u32,
    pub dst_port: u32,
    pub msg_id: u64,
    pub reply_to: u64,
    pub payload_size: u32,
    pub payload_offset: u64,
    pub timestamp: u64,
    pub reserved: [u8; 0],
}

impl Default for MessageHeader {
    fn default() -> Self {
        Self {
            msg_type: MessageType::Invalid,
            flags: MessageFlags::new(),
            priority: MessagePriority::Normal,
            src_port: 0,
            dst_port: 0,
            msg_id: 0,
            reply_to: 0,
            payload_size: 0,
            payload_offset: 0,
            timestamp: 0,
            reserved: [],
        }
    }
}

impl MessageHeader {
    pub fn new() -> Self { Self::default() }

    pub fn request(src: u32, dst: u32) -> Self {
        Self {
            msg_type: MessageType::Request,
            priority: MessagePriority::Normal,
            src_port: src,
            dst_port: dst,
            ..Self::default()
        }
    }

    pub fn response(src: u32, dst: u32, reply_to: u64) -> Self {
        Self {
            msg_type: MessageType::Response,
            priority: MessagePriority::Normal,
            src_port: src,
            dst_port: dst,
            reply_to,
            ..Self::default()
        }
    }

    pub fn notification(src: u32, dst: u32) -> Self {
        Self {
            msg_type: MessageType::Notification,
            priority: MessagePriority::Normal,
            src_port: src,
            dst_port: dst,
            ..Self::default()
        }
    }

    pub fn set_payload_size(&mut self, size: u32) { self.payload_size = size; }
    pub fn set_msg_id(&mut self, id: u64) { self.msg_id = id; }
    pub fn set_urgent(&mut self) { self.flags.set(MessageFlags::URGENT); self.priority = MessagePriority::Urgent; }
}

/// 通道 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub u64);

impl ChannelId {
    pub const INVALID: ChannelId = ChannelId(0);
    pub fn new() -> Self { ChannelId(0) }
    pub fn is_valid(&self) -> bool { self.0 != 0 }
}

/// 端口 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(pub u32);

impl PortId {
    pub const INVALID: PortId = PortId(0);
    pub fn new() -> Self { PortId(0) }
    pub fn is_valid(&self) -> bool { self.0 != 0 }
}

/// IPC 错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    ChannelNotFound,
    ChannelClosed,
    ChannelFull,
    PermissionDenied,
    InvalidMessage,
    Timeout,
    PortNotFound,
    PortAlreadyBound,
    BufferTooSmall,
    SerializationFailed,
    DeserializationFailed,
    Backpressure,
    InvalidPort,
    ResourceExhausted,
    NotSupported,
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChannelNotFound => write!(f, "channel not found"),
            Self::ChannelClosed => write!(f, "channel closed"),
            Self::ChannelFull => write!(f, "channel full"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::InvalidMessage => write!(f, "invalid message"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::PortNotFound => write!(f, "port not found"),
            Self::PortAlreadyBound => write!(f, "port already bound"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::SerializationFailed => write!(f, "serialization failed"),
            Self::DeserializationFailed => write!(f, "deserialization failed"),
            Self::Backpressure => write!(f, "backpressure active"),
            Self::InvalidPort => write!(f, "invalid port"),
            Self::ResourceExhausted => write!(f, "resource exhausted"),
            Self::NotSupported => write!(f, "operation not supported"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header_size() {
        assert_eq!(core::mem::size_of::<MessageHeader>(), MESSAGE_HEADER_SIZE);
    }

    #[test]
    fn test_message_header_default() {
        let hdr = MessageHeader::default();
        assert_eq!(hdr.msg_type, MessageType::Invalid);
        assert_eq!(hdr.payload_size, 0);
    }

    #[test]
    fn test_message_header_request() {
        let hdr = MessageHeader::request(1, 2);
        assert_eq!(hdr.msg_type, MessageType::Request);
        assert_eq!(hdr.src_port, 1);
        assert_eq!(hdr.dst_port, 2);
    }

    #[test]
    fn test_message_header_response() {
        let hdr = MessageHeader::response(2, 1, 42);
        assert_eq!(hdr.msg_type, MessageType::Response);
        assert_eq!(hdr.reply_to, 42);
    }

    #[test]
    fn test_message_flags() {
        let mut flags = MessageFlags::new();
        assert!(!flags.contains(MessageFlags::URGENT));
        flags.set(MessageFlags::URGENT);
        assert!(flags.contains(MessageFlags::URGENT));
        flags.clear(MessageFlags::URGENT);
        assert!(!flags.contains(MessageFlags::URGENT));
    }

    #[test]
    fn test_message_priority_ordering() {
        assert!(MessagePriority::Urgent > MessagePriority::High);
        assert!(MessagePriority::High > MessagePriority::Normal);
        assert!(MessagePriority::Normal > MessagePriority::Low);
    }

    #[test]
    fn test_channel_id() {
        let id = ChannelId::INVALID;
        assert!(!id.is_valid());
        let id = ChannelId(42);
        assert!(id.is_valid());
    }

    #[test]
    fn test_port_id() {
        let id = PortId::INVALID;
        assert!(!id.is_valid());
        let id = PortId(100);
        assert!(id.is_valid());
    }

    #[test]
    fn test_ipc_error_display() {
        let err = IpcError::ChannelNotFound;
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_message_type_values() {
        assert_eq!(MessageType::Request as u8, 1);
        assert_eq!(MessageType::Response as u8, 2);
        assert_eq!(MessageType::Notification as u8, 3);
    }

    #[test]
    fn test_message_header_set_urgent() {
        let mut hdr = MessageHeader::request(1, 2);
        hdr.set_urgent();
        assert!(hdr.flags.contains(MessageFlags::URGENT));
        assert_eq!(hdr.priority, MessagePriority::Urgent);
    }

    #[test]
    fn test_message_header_set_payload() {
        let mut hdr = MessageHeader::request(1, 2);
        hdr.set_payload_size(1024);
        assert_eq!(hdr.payload_size, 1024);
    }
}

// IMPORTANT: If the test_message_header_size test fails (size != 64),
// adjust the reserved array size in MessageHeader to make it exactly 64 bytes.
// The struct layout must be #[repr(C)] for predictable sizing.
