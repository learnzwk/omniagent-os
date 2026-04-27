//! 块设备抽象接口
//!
//! 定义块设备的 trait、错误类型和设备信息结构。

// ============================================================================
// BlockError: 块设备错误类型
// ============================================================================

/// 块设备错误类型
///
/// 封装块设备操作中可能出现的各种错误情况。
#[derive(Debug, Clone)]
pub enum BlockError {
    /// I/O 错误
    IoError {
        /// 错误原因描述
        reason: &'static str,
    },
    /// 无效的逻辑块地址 (LBA)
    InvalidLba(u64),
    /// 缓冲区大小不匹配
    InvalidBufferSize {
        /// 期望的缓冲区大小
        expected: usize,
        /// 实际的缓冲区大小
        actual: usize,
    },
    /// 设备忙
    DeviceBusy,
    /// 设备未找到
    DeviceNotFound,
    /// 介质错误
    MediaError,
}

// ============================================================================
// BlockDevice: 块设备 trait
// ============================================================================

/// 块设备 trait
///
/// 定义所有块设备必须实现的接口，包括读写、刷新、查询等操作。
/// 所有块设备必须是 Send + Sync 的，以支持在多线程环境中使用。
pub trait BlockDevice: Send + Sync {
    /// 从指定 LBA 读取块数据
    ///
    /// # 参数
    /// - `start_lba`: 起始逻辑块地址
    /// - `buf`: 输出缓冲区，长度必须为块大小的整数倍
    ///
    /// # 错误
    /// - `InvalidLba`: LBA 超出设备容量
    /// - `InvalidBufferSize`: 缓冲区大小不是块大小的整数倍
    fn read_blocks(&self, start_lba: u64, buf: &mut [u8]) -> Result<(), BlockError>;

    /// 向指定 LBA 写入块数据
    ///
    /// # 参数
    /// - `start_lba`: 起始逻辑块地址
    /// - `buf`: 输入缓冲区，长度必须为块大小的整数倍
    ///
    /// # 错误
    /// - `InvalidLba`: LBA 超出设备容量
    /// - `InvalidBufferSize`: 缓冲区大小不是块大小的整数倍
    fn write_blocks(&self, start_lba: u64, buf: &[u8]) -> Result<(), BlockError>;

    /// 刷新设备缓存（将脏数据写入持久存储）
    fn flush(&self) -> Result<(), BlockError>;

    /// 获取块大小（字节）
    fn block_size(&self) -> usize;

    /// 获取设备容量（以块为单位）
    fn capacity(&self) -> u64;

    /// 获取设备名称
    fn name(&self) -> &str;

    /// 检查设备是否为可移动设备
    fn is_removable(&self) -> bool;
}

// ============================================================================
// BlockDeviceInfo: 块设备信息
// ============================================================================

/// 块设备信息
///
/// 描述块设备的基本属性，用于设备列表和查询。
#[derive(Debug, Clone)]
pub struct BlockDeviceInfo {
    /// 设备名称
    pub name: alloc::string::String,
    /// 块大小（字节）
    pub block_size: usize,
    /// 设备容量（以块为单位）
    pub capacity: u64,
    /// 是否为可移动设备
    pub is_removable: bool,
}
