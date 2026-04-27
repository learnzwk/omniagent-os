//! 渲染配置和帧管理模块
//!
//! 包含 Vulkan 配置、交换链描述、帧上下文和 FPS 计数器。

// ============================================================================
// SurfaceFormat - 表面格式
// ============================================================================

/// 表面像素格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SurfaceFormat {
    /// BGRA 8位无符号归一化
    BGRA8Unorm = 0,
    /// RGBA 8位无符号归一化
    RGBA8Unorm = 1,
    /// BGR 8位无符号归一化
    BGR8Unorm = 2,
    /// RGB 8位无符号归一化
    RGB8Unorm = 3,
}

// ============================================================================
// SampleCount - 采样数
// ============================================================================

/// 多重采样抗锯齿 (MSAA) 采样数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SampleCount {
    /// 无 MSAA
    X1 = 0,
    /// 2倍采样
    X2 = 1,
    /// 4倍采样
    X4 = 2,
    /// 8倍采样
    X8 = 3,
}

// ============================================================================
// VulkanConfig - Vulkan 配置
// ============================================================================

/// Vulkan 实例和设备配置
#[derive(Debug, Clone)]
pub struct VulkanConfig {
    /// 应用程序名称
    pub application_name: String,
    /// 引擎名称
    pub engine_name: String,
    /// 版本号 (主版本, 次版本, 补丁)
    pub version: (u32, u32, u32),
    /// 首选 GPU 索引，None 表示自动选择
    pub preferred_gpu_index: Option<usize>,
    /// 是否启用验证层
    pub enable_validation: bool,
    /// 是否启用垂直同步
    pub enable_vsync: bool,
    /// 表面尺寸 (宽度, 高度)
    pub surface_size: (u32, u32),
    /// 表面格式
    pub surface_format: SurfaceFormat,
    /// MSAA 采样数
    pub sample_count: SampleCount,
    /// 最大帧中飞行数
    pub max_frames_in_flight: u32,
}

impl VulkanConfig {
    /// 创建默认 Vulkan 配置
    pub fn default_config() -> Self {
        Self {
            application_name: "OmniAgent Compositor".to_string(),
            engine_name: "OmniAgent".to_string(),
            version: (0, 1, 0),
            preferred_gpu_index: None,
            enable_validation: false,
            enable_vsync: true,
            surface_size: (1920, 1080),
            surface_format: SurfaceFormat::BGRA8Unorm,
            sample_count: SampleCount::X1,
            max_frames_in_flight: 2,
        }
    }
}

impl Default for VulkanConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

// ============================================================================
// Swapchain - 交换链
// ============================================================================

/// 交换链状态描述
#[derive(Debug, Clone)]
pub struct Swapchain {
    /// 交换链宽度
    pub width: u32,
    /// 交换链高度
    pub height: u32,
    /// 表面格式
    pub format: SurfaceFormat,
    /// 交换链图像数量
    pub image_count: u32,
    /// 当前图像索引
    pub current_image_index: u32,
    /// 交换链是否处于次优状态
    pub is_suboptimal: bool,
}

impl Swapchain {
    /// 创建新的交换链描述
    pub fn new(width: u32, height: u32, format: SurfaceFormat, image_count: u32) -> Self {
        Self {
            width,
            height,
            format,
            image_count,
            current_image_index: 0,
            is_suboptimal: false,
        }
    }

    /// 推进到下一帧图像
    pub fn advance(&mut self) {
        self.current_image_index = (self.current_image_index + 1) % self.image_count;
    }
}

// ============================================================================
// FrameContext - 帧上下文
// ============================================================================

/// 单帧渲染上下文，包含帧号和时间信息
#[derive(Debug, Clone)]
pub struct FrameContext {
    /// 帧编号（从 0 开始递增）
    pub frame_number: u64,
    /// 当前交换链图像索引
    pub swapchain_index: u32,
    /// 距上一帧的间隔时间（毫秒）
    pub delta_time_ms: f32,
    /// 从启动到当前的总时间（毫秒）
    pub total_time_ms: f64,
}

// ============================================================================
// FpsCounter - FPS 计数器
// ============================================================================

/// FPS 计数器，基于滑动窗口统计帧率
pub struct FpsCounter {
    /// 历史帧时间记录（毫秒）
    frame_times: Vec<f64>,
    /// 最大采样数
    max_samples: usize,
}

impl FpsCounter {
    /// 创建新的 FPS 计数器
    ///
    /// # 参数
    /// - `max_samples`: 滑动窗口大小，用于计算平均帧率
    pub fn new(max_samples: usize) -> Self {
        Self {
            frame_times: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    /// 记录一帧的时间
    ///
    /// # 参数
    /// - `time_ms`: 帧时间戳（毫秒）
    pub fn record_frame(&mut self, time_ms: f64) {
        if self.frame_times.len() >= self.max_samples {
            self.frame_times.remove(0);
        }
        self.frame_times.push(time_ms);
    }

    /// 计算当前平均 FPS
    pub fn fps(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        let first = self.frame_times.first().unwrap();
        let last = self.frame_times.last().unwrap();
        let elapsed = last - first;
        if elapsed <= 0.0 {
            return 0.0;
        }
        (self.frame_times.len() - 1) as f64 / (elapsed / 1000.0)
    }

    /// 计算平均帧时间（毫秒）
    pub fn frame_time_ms(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        let first = self.frame_times.first().unwrap();
        let last = self.frame_times.last().unwrap();
        let elapsed = last - first;
        if elapsed <= 0.0 {
            return 0.0;
        }
        elapsed / (self.frame_times.len() - 1) as f64
    }

    /// 获取最小帧时间（毫秒）
    pub fn min_frame_time(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        let mut min_delta = f64::MAX;
        for i in 1..self.frame_times.len() {
            let delta = self.frame_times[i] - self.frame_times[i - 1];
            if delta < min_delta {
                min_delta = delta;
            }
        }
        if min_delta == f64::MAX {
            0.0
        } else {
            min_delta
        }
    }

    /// 获取最大帧时间（毫秒）
    pub fn max_frame_time(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        let mut max_delta = 0.0;
        for i in 1..self.frame_times.len() {
            let delta = self.frame_times[i] - self.frame_times[i - 1];
            if delta > max_delta {
                max_delta = delta;
            }
        }
        max_delta
    }

    /// 重置计数器
    pub fn reset(&mut self) {
        self.frame_times.clear();
    }

    /// 获取已记录的帧数
    pub fn frame_count(&self) -> usize {
        self.frame_times.len()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- VulkanConfig 测试 ----

    #[test]
    fn test_vulkan_config_default() {
        let config = VulkanConfig::default();
        assert_eq!(config.application_name, "OmniAgent Compositor");
        assert_eq!(config.engine_name, "OmniAgent");
        assert_eq!(config.version, (0, 1, 0));
        assert!(config.preferred_gpu_index.is_none());
        assert!(!config.enable_validation);
        assert!(config.enable_vsync);
        assert_eq!(config.surface_size, (1920, 1080));
        assert_eq!(config.surface_format, SurfaceFormat::BGRA8Unorm);
        assert_eq!(config.sample_count, SampleCount::X1);
        assert_eq!(config.max_frames_in_flight, 2);
    }

    #[test]
    fn test_vulkan_config_custom() {
        let config = VulkanConfig {
            application_name: "Test App".to_string(),
            engine_name: "Test Engine".to_string(),
            version: (1, 2, 3),
            preferred_gpu_index: Some(0),
            enable_validation: true,
            enable_vsync: false,
            surface_size: (2560, 1440),
            surface_format: SurfaceFormat::RGBA8Unorm,
            sample_count: SampleCount::X4,
            max_frames_in_flight: 3,
        };
        assert_eq!(config.application_name, "Test App");
        assert_eq!(config.version, (1, 2, 3));
        assert_eq!(config.preferred_gpu_index, Some(0));
        assert!(config.enable_validation);
        assert!(!config.enable_vsync);
        assert_eq!(config.surface_size, (2560, 1440));
        assert_eq!(config.surface_format, SurfaceFormat::RGBA8Unorm);
        assert_eq!(config.sample_count, SampleCount::X4);
        assert_eq!(config.max_frames_in_flight, 3);
    }

    #[test]
    fn test_vulkan_config_clone() {
        let config = VulkanConfig::default();
        let cloned = config.clone();
        assert_eq!(config.application_name, cloned.application_name);
        assert_eq!(config.version, cloned.version);
    }

    // ---- SurfaceFormat 测试 ----

    #[test]
    fn test_surface_format_values() {
        assert_eq!(SurfaceFormat::BGRA8Unorm as u8, 0);
        assert_eq!(SurfaceFormat::RGBA8Unorm as u8, 1);
        assert_eq!(SurfaceFormat::BGR8Unorm as u8, 2);
        assert_eq!(SurfaceFormat::RGB8Unorm as u8, 3);
    }

    // ---- SampleCount 测试 ----

    #[test]
    fn test_sample_count_values() {
        assert_eq!(SampleCount::X1 as u8, 0);
        assert_eq!(SampleCount::X2 as u8, 1);
        assert_eq!(SampleCount::X4 as u8, 2);
        assert_eq!(SampleCount::X8 as u8, 3);
    }

    // ---- Swapchain 测试 ----

    #[test]
    fn test_swapchain_new() {
        let sc = Swapchain::new(1920, 1080, SurfaceFormat::BGRA8Unorm, 3);
        assert_eq!(sc.width, 1920);
        assert_eq!(sc.height, 1080);
        assert_eq!(sc.format, SurfaceFormat::BGRA8Unorm);
        assert_eq!(sc.image_count, 3);
        assert_eq!(sc.current_image_index, 0);
        assert!(!sc.is_suboptimal);
    }

    #[test]
    fn test_swapchain_advance() {
        let mut sc = Swapchain::new(800, 600, SurfaceFormat::RGBA8Unorm, 3);
        assert_eq!(sc.current_image_index, 0);
        sc.advance();
        assert_eq!(sc.current_image_index, 1);
        sc.advance();
        assert_eq!(sc.current_image_index, 2);
        sc.advance();
        // 应循环回到 0
        assert_eq!(sc.current_image_index, 0);
    }

    // ---- FpsCounter 测试 ----

    #[test]
    fn test_fps_counter_new() {
        let counter = FpsCounter::new(60);
        assert_eq!(counter.max_samples, 60);
        assert_eq!(counter.frame_count(), 0);
    }

    #[test]
    fn test_fps_counter_record() {
        let mut counter = FpsCounter::new(10);
        counter.record_frame(0.0);
        counter.record_frame(16.67);
        counter.record_frame(33.33);
        assert_eq!(counter.frame_count(), 3);
    }

    #[test]
    fn test_fps_counter_fps() {
        let mut counter = FpsCounter::new(100);
        // 模拟 60 FPS 的帧时间
        for i in 0..61 {
            counter.record_frame(i as f64 * 16.667);
        }
        let fps = counter.fps();
        // 应接近 60 FPS
        assert!(fps > 55.0 && fps < 65.0, "FPS 应接近 60，实际: {}", fps);
    }

    #[test]
    fn test_fps_counter_frame_time() {
        let mut counter = FpsCounter::new(100);
        for i in 0..61 {
            counter.record_frame(i as f64 * 16.667);
        }
        let ft = counter.frame_time_ms();
        assert!(ft > 15.0 && ft < 18.0, "帧时间应接近 16.67ms，实际: {}", ft);
    }

    #[test]
    fn test_fps_counter_min_max() {
        let mut counter = FpsCounter::new(100);
        // 不均匀帧时间
        counter.record_frame(0.0);
        counter.record_frame(10.0);   // 快帧
        counter.record_frame(30.0);   // 慢帧 (delta=20)
        counter.record_frame(40.0);   // 快帧 (delta=10)
        counter.record_frame(60.0);   // 慢帧 (delta=20)

        let min_ft = counter.min_frame_time();
        let max_ft = counter.max_frame_time();
        assert_eq!(min_ft, 10.0);
        assert_eq!(max_ft, 20.0);
    }

    #[test]
    fn test_fps_counter_insufficient_data() {
        let counter = FpsCounter::new(100);
        assert_eq!(counter.fps(), 0.0);
        assert_eq!(counter.frame_time_ms(), 0.0);
        assert_eq!(counter.min_frame_time(), 0.0);
        assert_eq!(counter.max_frame_time(), 0.0);
    }

    #[test]
    fn test_fps_counter_single_frame() {
        let mut counter = FpsCounter::new(100);
        counter.record_frame(100.0);
        // 只有一帧数据，无法计算 FPS
        assert_eq!(counter.fps(), 0.0);
    }

    #[test]
    fn test_fps_counter_reset() {
        let mut counter = FpsCounter::new(100);
        counter.record_frame(0.0);
        counter.record_frame(16.67);
        counter.reset();
        assert_eq!(counter.frame_count(), 0);
        assert_eq!(counter.fps(), 0.0);
    }

    #[test]
    fn test_fps_counter_sliding_window() {
        let mut counter = FpsCounter::new(3);
        counter.record_frame(0.0);
        counter.record_frame(16.67);
        counter.record_frame(33.33);
        counter.record_frame(50.0);  // 滑出第一帧
        assert_eq!(counter.frame_count(), 3);
    }
}
