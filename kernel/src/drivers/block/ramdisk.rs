//! 内存磁盘 (RAM Disk) 实现
//!
//! RamDisk 是一个基于内存的块设备实现，所有数据存储在内存中的 Vec<u8> 中。
//! 适用于测试和临时存储场景。

use super::device::{BlockDevice, BlockError, BlockDeviceInfo};
use spin::Mutex;

// ============================================================================
// RamDisk: 内存磁盘
// ============================================================================

/// 内存磁盘
///
/// 基于内存的块设备实现，使用 Mutex 保护的 Vec<u8> 作为存储后端。
/// 支持基本的块读写操作，flush 为空操作。
pub struct RamDisk {
    /// 存储数据（受 Mutex 保护以支持并发访问）
    data: Mutex<alloc::vec::Vec<u8>>,
    /// 块大小（字节）
    block_size: usize,
    /// 设备名称
    disk_name: alloc::string::String,
}

impl RamDisk {
    /// 创建新的内存磁盘
    ///
    /// # 参数
    /// - `name`: 设备名称
    /// - `block_size`: 块大小（字节），必须为 2 的幂且大于 0
    /// - `capacity_blocks`: 容量（以块为单位）
    ///
    /// # Panics
    /// 如果 block_size 为 0 或 capacity_blocks 为 0
    pub fn new(name: &str, block_size: usize, capacity_blocks: u64) -> Self {
        assert!(block_size > 0, "块大小必须大于 0");
        assert!(capacity_blocks > 0, "容量必须大于 0");

        let total_size = (block_size as u64)
            .checked_mul(capacity_blocks)
            .expect("容量溢出") as usize;

        Self {
            data: Mutex::new(alloc::vec![0u8; total_size]),
            block_size,
            disk_name: alloc::string::String::from(name),
        }
    }
}

impl BlockDevice for RamDisk {
    fn read_blocks(&self, start_lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        // 验证缓冲区大小
        if buf.len() % self.block_size != 0 {
            return Err(BlockError::InvalidBufferSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }

        let num_blocks = buf.len() / self.block_size;
        let capacity = self.capacity();

        // 验证 LBA 范围
        if start_lba.checked_add(num_blocks as u64).map_or(true, |end| end > capacity) {
            return Err(BlockError::InvalidLba(start_lba));
        }

        let mut data = self.data.lock();
        let start_offset = (start_lba as usize) * self.block_size;
        let end_offset = start_offset + buf.len();

        buf.copy_from_slice(&data[start_offset..end_offset]);
        Ok(())
    }

    fn write_blocks(&self, start_lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        // 验证缓冲区大小
        if buf.len() % self.block_size != 0 {
            return Err(BlockError::InvalidBufferSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }

        let num_blocks = buf.len() / self.block_size;
        let capacity = self.capacity();

        // 验证 LBA 范围
        if start_lba.checked_add(num_blocks as u64).map_or(true, |end| end > capacity) {
            return Err(BlockError::InvalidLba(start_lba));
        }

        let mut data = self.data.lock();
        let start_offset = (start_lba as usize) * self.block_size;
        let end_offset = start_offset + buf.len();

        data[start_offset..end_offset].copy_from_slice(buf);
        Ok(())
    }

    fn flush(&self) -> Result<(), BlockError> {
        // 内存磁盘无需刷新，直接返回成功
        Ok(())
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn capacity(&self) -> u64 {
        let data = self.data.lock();
        (data.len() / self.block_size) as u64
    }

    fn name(&self) -> &str {
        &self.disk_name
    }

    fn is_removable(&self) -> bool {
        false
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    // === 测试: 创建 RamDisk ===
    #[test]
    fn test_ramdisk_create() {
        let disk = RamDisk::new("test-disk", 512, 100);
        assert_eq!(disk.name(), "test-disk");
        assert_eq!(disk.block_size(), 512);
        assert_eq!(disk.capacity(), 100);
        assert!(!disk.is_removable());
    }

    // === 测试: RamDisk 读写 ===
    #[test]
    fn test_ramdisk_read_write() {
        let disk = RamDisk::new("rw-test", 512, 10);

        // 写入数据
        let write_buf = [0xABu8; 512];
        disk.write_blocks(0, &write_buf).unwrap();

        // 读回数据
        let mut read_buf = [0u8; 512];
        disk.read_blocks(0, &mut read_buf).unwrap();

        assert_eq!(read_buf, write_buf);
    }

    // === 测试: RamDisk 容量 ===
    #[test]
    fn test_ramdisk_capacity() {
        let disk = RamDisk::new("cap-test", 1024, 50);
        assert_eq!(disk.capacity(), 50);
        assert_eq!(disk.block_size(), 1024);
    }

    // === 测试: RamDisk flush ===
    #[test]
    fn test_ramdisk_flush() {
        let disk = RamDisk::new("flush-test", 512, 10);
        assert!(disk.flush().is_ok());
    }

    // === 测试: RamDisk 无效 LBA ===
    #[test]
    fn test_ramdisk_invalid_lba() {
        let disk = RamDisk::new("lba-test", 512, 10);
        let mut buf = [0u8; 512];

        // LBA 超出范围
        let result = disk.read_blocks(10, &mut buf);
        assert!(matches!(result, Err(BlockError::InvalidLba(10))));

        // LBA 超出范围（写入）
        let write_buf = [0u8; 512];
        let result = disk.write_blocks(100, &write_buf);
        assert!(matches!(result, Err(BlockError::InvalidLba(100))));
    }

    // === 测试: RamDisk 缓冲区大小不匹配 ===
    #[test]
    fn test_ramdisk_buffer_mismatch() {
        let disk = RamDisk::new("buf-test", 512, 10);
        let mut buf = [0u8; 100]; // 不是 512 的倍数

        let result = disk.read_blocks(0, &mut buf);
        assert!(matches!(result, Err(BlockError::InvalidBufferSize { .. })));

        let result = disk.write_blocks(0, &buf);
        assert!(matches!(result, Err(BlockError::InvalidBufferSize { .. })));
    }

    // === 测试: 多块读写 ===
    #[test]
    fn test_multi_block_read_write() {
        let disk = RamDisk::new("multi-test", 512, 100);

        // 写入 3 个块
        let write_data = {
            let mut v = alloc::vec![0u8; 512 * 3];
            v[0] = 0xAA;
            v[512] = 0xBB;
            v[1024] = 0xCC;
            v
        };
        disk.write_blocks(5, &write_data).unwrap();

        // 读回并验证
        let mut read_data = alloc::vec![0u8; 512 * 3];
        disk.read_blocks(5, &mut read_data).unwrap();

        assert_eq!(read_data[0], 0xAA);
        assert_eq!(read_data[512], 0xBB);
        assert_eq!(read_data[1024], 0xCC);

        // 验证其他块未被修改
        let mut other_buf = [0u8; 512];
        disk.read_blocks(0, &mut other_buf).unwrap();
        assert_eq!(other_buf, [0u8; 512]);
    }

    // === 测试: BlockDeviceInfo ===
    #[test]
    fn test_block_device_info() {
        let disk = RamDisk::new("info-test", 4096, 200);
        let info = BlockDeviceInfo {
            name: alloc::string::String::from(disk.name()),
            block_size: disk.block_size(),
            capacity: disk.capacity(),
            is_removable: disk.is_removable(),
        };
        assert_eq!(info.name, "info-test");
        assert_eq!(info.block_size, 4096);
        assert_eq!(info.capacity, 200);
        assert!(!info.is_removable);
    }

    // === 测试: RamDisk 可作为 Arc<dyn BlockDevice> 使用 ===
    #[test]
    fn test_ramdisk_as_dyn_block_device() {
        let disk: Arc<dyn BlockDevice> = Arc::new(RamDisk::new("dyn-test", 512, 10));
        assert_eq!(disk.name(), "dyn-test");
        assert_eq!(disk.block_size(), 512);
        assert_eq!(disk.capacity(), 10);
    }
}
