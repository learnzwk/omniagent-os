//! 纹理和缓冲区管理模块
//!
//! 包含纹理格式、纹理描述、纹理用法、缓冲区类型和 GPU 资源管理器。

use std::collections::HashMap;

use crate::render::CompositorError;

// ============================================================================
// TextureFormat - 纹理格式
// ============================================================================

/// 纹理像素格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureFormat {
    /// RGBA 8位
    RGBA8 = 0,
    /// BGRA 8位
    BGRA8 = 1,
    /// RGB 8位
    RGB8 = 2,
    /// 单通道 8位
    R8 = 3,
    /// 双通道 8位
    RG8 = 4,
    /// 16位无符号归一化深度
    D16Unorm = 5,
    /// 32位浮点深度
    D32Float = 6,
}

impl TextureFormat {
    /// 获取每个像素的字节数
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            Self::RGBA8 | Self::BGRA8 => 4,
            Self::RGB8 => 3,
            Self::R8 => 1,
            Self::RG8 => 2,
            Self::D16Unorm => 2,
            Self::D32Float => 4,
        }
    }
}

// ============================================================================
// TextureUsage - 纹理用法标志位
// ============================================================================

/// 纹理用法标志位（位掩码）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureUsage(u32);

impl TextureUsage {
    /// 可作为采样器使用
    pub const SAMPLED: TextureUsage = TextureUsage(1 << 0);
    /// 可作为渲染目标
    pub const RENDER_TARGET: TextureUsage = TextureUsage(1 << 1);
    /// 可作为传输源
    pub const TRANSFER_SRC: TextureUsage = TextureUsage(1 << 2);
    /// 可作为传输目标
    pub const TRANSFER_DST: TextureUsage = TextureUsage(1 << 3);

    /// 检查是否包含指定用法
    pub fn contains(&self, other: TextureUsage) -> bool {
        (self.0 & other.0) != 0
    }

    /// 合并两个用法标志
    pub fn union(&self, other: TextureUsage) -> TextureUsage {
        TextureUsage(self.0 | other.0)
    }

    /// 获取原始标志值
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// 检查是否为空（无任何用法）
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

// ============================================================================
// TextureDescriptor - 纹理描述
// ============================================================================

/// 纹理创建描述符
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    /// 纹理宽度
    pub width: u32,
    /// 纹理高度
    pub height: u32,
    /// 像素格式
    pub format: TextureFormat,
    /// 纹理用法
    pub usage: TextureUsage,
    /// MIP 层级数
    pub mip_levels: u32,
}

impl TextureDescriptor {
    /// 计算纹理所需内存大小（字节）
    ///
    /// 注意：此为简化计算，不考虑 MIP 链和内存对齐。
    pub fn memory_size(&self) -> usize {
        let bpp = self.format.bytes_per_pixel() as usize;
        let mut total = 0usize;
        let mut w = self.width as usize;
        let mut h = self.height as usize;
        for _ in 0..self.mip_levels {
            total += w * h * bpp;
            w = w.max(1) / 2;
            h = h.max(1) / 2;
        }
        total
    }
}

// ============================================================================
// Texture - 纹理
// ============================================================================

/// GPU 纹理资源
pub struct Texture {
    /// 纹理唯一 ID
    pub id: u64,
    /// 纹理描述
    pub descriptor: TextureDescriptor,
    /// 是否已分配 GPU 内存
    pub is_allocated: bool,
}

// ============================================================================
// BufferType - 缓冲区类型
// ============================================================================

/// GPU 缓冲区类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BufferType {
    /// 顶点缓冲区
    Vertex = 0,
    /// 索引缓冲区
    Index = 1,
    /// 统一缓冲区（Uniform Buffer）
    Uniform = 2,
    /// 存储缓冲区
    Storage = 3,
    /// 暂存缓冲区（用于数据传输）
    Staging = 4,
}

// ============================================================================
// Buffer - 缓冲区
// ============================================================================

/// GPU 缓冲区资源
pub struct Buffer {
    /// 缓冲区唯一 ID
    pub id: u64,
    /// 缓冲区大小（字节）
    pub size: usize,
    /// 缓冲区类型
    pub buffer_type: BufferType,
    /// 是否已分配 GPU 内存
    pub is_allocated: bool,
}

// ============================================================================
// GpuResourceManager - GPU 资源管理器
// ============================================================================

/// GPU 资源管理器
///
/// 管理纹理和缓冲区的创建、销毁和内存追踪。
pub struct GpuResourceManager {
    /// 纹理集合
    textures: HashMap<u64, Texture>,
    /// 缓冲区集合
    buffers: HashMap<u64, Buffer>,
    /// 下一个纹理 ID
    next_texture_id: u64,
    /// 下一个缓冲区 ID
    next_buffer_id: u64,
    /// 纹理总内存占用（字节）
    total_texture_memory: usize,
    /// 缓冲区总内存占用（字节）
    total_buffer_memory: usize,
    /// 最大内存限制（字节）
    max_memory: usize,
}

impl GpuResourceManager {
    /// 创建新的 GPU 资源管理器
    ///
    /// # 参数
    /// - `max_memory`: 最大内存限制（字节），0 表示无限制
    pub fn new(max_memory: usize) -> Self {
        Self {
            textures: HashMap::new(),
            buffers: HashMap::new(),
            next_texture_id: 1,
            next_buffer_id: 1,
            total_texture_memory: 0,
            total_buffer_memory: 0,
            max_memory,
        }
    }

    /// 创建纹理
    ///
    /// # 参数
    /// - `desc`: 纹理描述符
    ///
    /// # 返回
    /// 纹理 ID
    pub fn create_texture(&mut self, desc: TextureDescriptor) -> Result<u64, CompositorError> {
        let mem_size = desc.memory_size();

        // 检查内存限制
        if self.max_memory > 0 && self.total_texture_memory + self.total_buffer_memory + mem_size > self.max_memory
        {
            return Err(CompositorError::OutOfMemory);
        }

        let id = self.next_texture_id;
        self.next_texture_id += 1;

        let texture = Texture {
            id,
            descriptor: desc,
            is_allocated: true,
        };

        self.total_texture_memory += mem_size;
        self.textures.insert(id, texture);

        Ok(id)
    }

    /// 销毁纹理
    ///
    /// # 参数
    /// - `id`: 纹理 ID
    pub fn destroy_texture(&mut self, id: u64) -> Result<(), CompositorError> {
        if let Some(texture) = self.textures.remove(&id) {
            self.total_texture_memory -= texture.descriptor.memory_size();
            Ok(())
        } else {
            Err(CompositorError::InvalidConfig(format!(
                "纹理 ID {} 不存在",
                id
            )))
        }
    }

    /// 获取纹理的不可变引用
    pub fn get_texture(&self, id: u64) -> Option<&Texture> {
        self.textures.get(&id)
    }

    /// 创建缓冲区
    ///
    /// # 参数
    /// - `size`: 缓冲区大小（字节）
    /// - `buffer_type`: 缓冲区类型
    ///
    /// # 返回
    /// 缓冲区 ID
    pub fn create_buffer(
        &mut self,
        size: usize,
        buffer_type: BufferType,
    ) -> Result<u64, CompositorError> {
        // 检查内存限制
        if self.max_memory > 0
            && self.total_texture_memory + self.total_buffer_memory + size > self.max_memory
        {
            return Err(CompositorError::OutOfMemory);
        }

        let id = self.next_buffer_id;
        self.next_buffer_id += 1;

        let buffer = Buffer {
            id,
            size,
            buffer_type,
            is_allocated: true,
        };

        self.total_buffer_memory += size;
        self.buffers.insert(id, buffer);

        Ok(id)
    }

    /// 销毁缓冲区
    ///
    /// # 参数
    /// - `id`: 缓冲区 ID
    pub fn destroy_buffer(&mut self, id: u64) -> Result<(), CompositorError> {
        if let Some(buffer) = self.buffers.remove(&id) {
            self.total_buffer_memory -= buffer.size;
            Ok(())
        } else {
            Err(CompositorError::InvalidConfig(format!(
                "缓冲区 ID {} 不存在",
                id
            )))
        }
    }

    /// 获取缓冲区的不可变引用
    pub fn get_buffer(&self, id: u64) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    /// 获取总内存使用量（字节）
    pub fn memory_usage(&self) -> usize {
        self.total_texture_memory + self.total_buffer_memory
    }

    /// 获取可用内存（字节）
    pub fn memory_available(&self) -> usize {
        if self.max_memory == 0 {
            return usize::MAX;
        }
        self.max_memory.saturating_sub(self.memory_usage())
    }

    /// 获取纹理数量
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// 获取缓冲区数量
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TextureFormat 测试 ----

    #[test]
    fn test_texture_format_bytes_per_pixel() {
        assert_eq!(TextureFormat::RGBA8.bytes_per_pixel(), 4);
        assert_eq!(TextureFormat::BGRA8.bytes_per_pixel(), 4);
        assert_eq!(TextureFormat::RGB8.bytes_per_pixel(), 3);
        assert_eq!(TextureFormat::R8.bytes_per_pixel(), 1);
        assert_eq!(TextureFormat::RG8.bytes_per_pixel(), 2);
        assert_eq!(TextureFormat::D16Unorm.bytes_per_pixel(), 2);
        assert_eq!(TextureFormat::D32Float.bytes_per_pixel(), 4);
    }

    // ---- TextureUsage 测试 ----

    #[test]
    fn test_texture_usage_contains() {
        let usage = TextureUsage::SAMPLED.union(TextureUsage::RENDER_TARGET);
        assert!(usage.contains(TextureUsage::SAMPLED));
        assert!(usage.contains(TextureUsage::RENDER_TARGET));
        assert!(!usage.contains(TextureUsage::TRANSFER_SRC));
        assert!(!usage.contains(TextureUsage::TRANSFER_DST));
    }

    #[test]
    fn test_texture_usage_union() {
        let usage = TextureUsage::SAMPLED.union(TextureUsage::TRANSFER_SRC);
        assert!(usage.contains(TextureUsage::SAMPLED));
        assert!(usage.contains(TextureUsage::TRANSFER_SRC));
        assert_eq!(usage.bits(), 1 | 4);
    }

    #[test]
    fn test_texture_usage_union_multiple() {
        let usage = TextureUsage::SAMPLED
            .union(TextureUsage::RENDER_TARGET)
            .union(TextureUsage::TRANSFER_SRC)
            .union(TextureUsage::TRANSFER_DST);
        assert!(usage.contains(TextureUsage::SAMPLED));
        assert!(usage.contains(TextureUsage::RENDER_TARGET));
        assert!(usage.contains(TextureUsage::TRANSFER_SRC));
        assert!(usage.contains(TextureUsage::TRANSFER_DST));
        assert_eq!(usage.bits(), 0b1111);
    }

    #[test]
    fn test_texture_usage_empty() {
        let usage = TextureUsage(0);
        assert!(usage.is_empty());
        assert!(!usage.contains(TextureUsage::SAMPLED));
    }

    #[test]
    fn test_texture_usage_bits() {
        assert_eq!(TextureUsage::SAMPLED.bits(), 1);
        assert_eq!(TextureUsage::RENDER_TARGET.bits(), 2);
        assert_eq!(TextureUsage::TRANSFER_SRC.bits(), 4);
        assert_eq!(TextureUsage::TRANSFER_DST.bits(), 8);
    }

    // ---- TextureDescriptor 测试 ----

    #[test]
    fn test_texture_descriptor_memory_size() {
        let desc = TextureDescriptor {
            width: 256,
            height: 256,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 1,
        };
        // 256 * 256 * 4 = 262144
        assert_eq!(desc.memory_size(), 262144);
    }

    #[test]
    fn test_texture_descriptor_memory_size_mips() {
        let desc = TextureDescriptor {
            width: 256,
            height: 256,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 3,
        };
        // MIP 0: 256*256*4 = 262144
        // MIP 1: 128*128*4 = 65536
        // MIP 2: 64*64*4 = 16384
        // 总计: 344064
        assert_eq!(desc.memory_size(), 262144 + 65536 + 16384);
    }

    // ---- GpuResourceManager 测试 ----

    #[test]
    fn test_gpu_resource_manager_new() {
        let mgr = GpuResourceManager::new(1024 * 1024);
        assert_eq!(mgr.texture_count(), 0);
        assert_eq!(mgr.buffer_count(), 0);
        assert_eq!(mgr.memory_usage(), 0);
    }

    #[test]
    fn test_create_texture() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let desc = TextureDescriptor {
            width: 64,
            height: 64,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 1,
        };
        let id = mgr.create_texture(desc).unwrap();
        assert_eq!(id, 1);
        assert_eq!(mgr.texture_count(), 1);
        assert_eq!(mgr.memory_usage(), 64 * 64 * 4);
    }

    #[test]
    fn test_create_multiple_textures() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let desc = TextureDescriptor {
            width: 32,
            height: 32,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 1,
        };

        let id1 = mgr.create_texture(desc.clone()).unwrap();
        let id2 = mgr.create_texture(desc.clone()).unwrap();
        let id3 = mgr.create_texture(desc).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(mgr.texture_count(), 3);
    }

    #[test]
    fn test_destroy_texture() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let desc = TextureDescriptor {
            width: 64,
            height: 64,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 1,
        };
        let id = mgr.create_texture(desc).unwrap();
        assert_eq!(mgr.texture_count(), 1);

        mgr.destroy_texture(id).unwrap();
        assert_eq!(mgr.texture_count(), 0);
        assert_eq!(mgr.memory_usage(), 0);
    }

    #[test]
    fn test_destroy_nonexistent_texture() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let result = mgr.destroy_texture(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_texture() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let desc = TextureDescriptor {
            width: 128,
            height: 128,
            format: TextureFormat::BGRA8,
            usage: TextureUsage::RENDER_TARGET,
            mip_levels: 1,
        };
        let id = mgr.create_texture(desc).unwrap();

        let texture = mgr.get_texture(id).unwrap();
        assert_eq!(texture.id, id);
        assert_eq!(texture.descriptor.width, 128);
        assert_eq!(texture.descriptor.format, TextureFormat::BGRA8);
        assert!(texture.is_allocated);
    }

    #[test]
    fn test_get_nonexistent_texture() {
        let mgr = GpuResourceManager::new(1024 * 1024);
        assert!(mgr.get_texture(999).is_none());
    }

    #[test]
    fn test_create_buffer() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let id = mgr.create_buffer(1024, BufferType::Vertex).unwrap();
        assert_eq!(id, 1);
        assert_eq!(mgr.buffer_count(), 1);
        assert_eq!(mgr.memory_usage(), 1024);
    }

    #[test]
    fn test_create_multiple_buffers() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let id1 = mgr.create_buffer(256, BufferType::Vertex).unwrap();
        let id2 = mgr.create_buffer(512, BufferType::Index).unwrap();
        let id3 = mgr.create_buffer(128, BufferType::Uniform).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(mgr.buffer_count(), 3);
        assert_eq!(mgr.memory_usage(), 256 + 512 + 128);
    }

    #[test]
    fn test_destroy_buffer() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let id = mgr.create_buffer(2048, BufferType::Storage).unwrap();
        assert_eq!(mgr.buffer_count(), 1);

        mgr.destroy_buffer(id).unwrap();
        assert_eq!(mgr.buffer_count(), 0);
        assert_eq!(mgr.memory_usage(), 0);
    }

    #[test]
    fn test_destroy_nonexistent_buffer() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let result = mgr.destroy_buffer(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_buffer() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);
        let id = mgr.create_buffer(4096, BufferType::Uniform).unwrap();

        let buffer = mgr.get_buffer(id).unwrap();
        assert_eq!(buffer.id, id);
        assert_eq!(buffer.size, 4096);
        assert_eq!(buffer.buffer_type, BufferType::Uniform);
        assert!(buffer.is_allocated);
    }

    #[test]
    fn test_get_nonexistent_buffer() {
        let mgr = GpuResourceManager::new(1024 * 1024);
        assert!(mgr.get_buffer(999).is_none());
    }

    #[test]
    fn test_memory_tracking_mixed() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);

        // 创建纹理
        let tex_desc = TextureDescriptor {
            width: 32,
            height: 32,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 1,
        };
        let tex_id = mgr.create_texture(tex_desc).unwrap();
        // 32 * 32 * 4 = 4096

        // 创建缓冲区
        let buf_id = mgr.create_buffer(2048, BufferType::Vertex).unwrap();

        assert_eq!(mgr.memory_usage(), 4096 + 2048);
        assert_eq!(mgr.texture_count(), 1);
        assert_eq!(mgr.buffer_count(), 1);

        // 销毁纹理
        mgr.destroy_texture(tex_id).unwrap();
        assert_eq!(mgr.memory_usage(), 2048);

        // 销毁缓冲区
        mgr.destroy_buffer(buf_id).unwrap();
        assert_eq!(mgr.memory_usage(), 0);
    }

    #[test]
    fn test_memory_limit() {
        let mut mgr = GpuResourceManager::new(5000);

        // 创建纹理 (4096 字节)
        let tex_desc = TextureDescriptor {
            width: 32,
            height: 32,
            format: TextureFormat::RGBA8,
            usage: TextureUsage::SAMPLED,
            mip_levels: 1,
        };
        let result = mgr.create_texture(tex_desc);
        assert!(result.is_ok());

        // 尝试创建超过限制的缓冲区
        let result = mgr.create_buffer(2000, BufferType::Vertex);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CompositorError::OutOfMemory);
    }

    #[test]
    fn test_memory_available() {
        let mgr = GpuResourceManager::new(10000);
        assert_eq!(mgr.memory_available(), 10000);
    }

    #[test]
    fn test_memory_available_unlimited() {
        let mgr = GpuResourceManager::new(0);
        assert_eq!(mgr.memory_available(), usize::MAX);
    }

    #[test]
    fn test_buffer_types() {
        let mut mgr = GpuResourceManager::new(1024 * 1024);

        let vertex_id = mgr.create_buffer(100, BufferType::Vertex).unwrap();
        let index_id = mgr.create_buffer(200, BufferType::Index).unwrap();
        let uniform_id = mgr.create_buffer(300, BufferType::Uniform).unwrap();
        let storage_id = mgr.create_buffer(400, BufferType::Storage).unwrap();
        let staging_id = mgr.create_buffer(500, BufferType::Staging).unwrap();

        assert_eq!(mgr.get_buffer(vertex_id).unwrap().buffer_type, BufferType::Vertex);
        assert_eq!(mgr.get_buffer(index_id).unwrap().buffer_type, BufferType::Index);
        assert_eq!(mgr.get_buffer(uniform_id).unwrap().buffer_type, BufferType::Uniform);
        assert_eq!(mgr.get_buffer(storage_id).unwrap().buffer_type, BufferType::Storage);
        assert_eq!(mgr.get_buffer(staging_id).unwrap().buffer_type, BufferType::Staging);
    }
}
