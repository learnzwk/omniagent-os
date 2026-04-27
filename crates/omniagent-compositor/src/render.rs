//! 合成器渲染器模块
//!
//! 包含合成器错误类型、渲染层、渲染操作、渲染队列和合成器渲染器。

use crate::config::{FpsCounter, FrameContext, Swapchain, VulkanConfig};
use std::fmt;

// ============================================================================
// CompositorError - 合成器错误
// ============================================================================

/// 合成器错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositorError {
    /// 初始化失败
    InitializationFailed(String),
    /// 交换链创建失败
    SwapchainCreationFailed(String),
    /// 渲染失败
    RenderFailed(String),
    /// 表面丢失
    SurfaceLost,
    /// 设备丢失
    DeviceLost,
    /// 内存不足
    OutOfMemory,
    /// 无效配置
    InvalidConfig(String),
    /// 着色器编译失败
    ShaderCompilationFailed(String),
    /// 管线创建失败
    PipelineCreationFailed(String),
}

impl fmt::Display for CompositorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitializationFailed(msg) => write!(f, "初始化失败: {}", msg),
            Self::SwapchainCreationFailed(msg) => write!(f, "交换链创建失败: {}", msg),
            Self::RenderFailed(msg) => write!(f, "渲染失败: {}", msg),
            Self::SurfaceLost => write!(f, "表面已丢失"),
            Self::DeviceLost => write!(f, "设备已丢失"),
            Self::OutOfMemory => write!(f, "GPU 内存不足"),
            Self::InvalidConfig(msg) => write!(f, "无效配置: {}", msg),
            Self::ShaderCompilationFailed(msg) => write!(f, "着色器编译失败: {}", msg),
            Self::PipelineCreationFailed(msg) => write!(f, "管线创建失败: {}", msg),
        }
    }
}

impl std::error::Error for CompositorError {}

// ============================================================================
// RenderLayer - 渲染层
// ============================================================================

/// 渲染层，定义绘制顺序（数值越小越先绘制）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RenderLayer {
    /// 背景层
    Background = 0,
    /// 窗口层
    Windows = 1,
    /// 覆盖层
    Overlays = 2,
    /// 通知层
    Notifications = 3,
    /// 光标层（最顶层）
    Cursor = 4,
}

// ============================================================================
// RenderOp - 渲染操作
// ============================================================================

/// 渲染操作指令
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOp {
    /// 清除画布，填充指定颜色
    Clear {
        /// 清除颜色 (RGBA)
        color: [f32; 4],
    },
    /// 绘制矩形
    DrawRect {
        /// X 坐标
        x: f32,
        /// Y 坐标
        y: f32,
        /// 宽度
        w: f32,
        /// 高度
        h: f32,
        /// 填充颜色 (RGBA)
        color: [f32; 4],
        /// 圆角半径
        border_radius: f32,
    },
    /// 绘制文本
    DrawText {
        /// X 坐标
        x: f32,
        /// Y 坐标
        y: f32,
        /// 文本内容
        text: String,
        /// 字体大小
        font_size: f32,
        /// 文本颜色 (RGBA)
        color: [f32; 4],
    },
    /// 绘制图像
    DrawImage {
        /// X 坐标
        x: f32,
        /// Y 坐标
        y: f32,
        /// 宽度
        w: f32,
        /// 高度
        h: f32,
        /// 纹理 ID
        texture_id: u64,
    },
    /// 绘制圆角矩形
    DrawRoundedRect {
        /// X 坐标
        x: f32,
        /// Y 坐标
        y: f32,
        /// 宽度
        w: f32,
        /// 高度
        h: f32,
        /// 填充颜色 (RGBA)
        color: [f32; 4],
        /// 圆角半径
        radius: f32,
    },
    /// 设置裁剪区域
    Clip {
        /// 裁剪区域 X 坐标
        x: f32,
        /// 裁剪区域 Y 坐标
        y: f32,
        /// 裁剪区域宽度
        w: f32,
        /// 裁剪区域高度
        h: f32,
    },
    /// 弹出裁剪区域
    PopClip,
    /// 设置变换矩阵
    Transform {
        /// 平移 (X, Y)
        translate: (f32, f32),
        /// 缩放 (X, Y)
        scale: (f32, f32),
        /// 旋转角度（弧度）
        rotation: f32,
    },
    /// 弹出变换矩阵
    PopTransform,
    /// 绘制阴影
    Shadow {
        /// 阴影 X 偏移
        x: f32,
        /// 阴影 Y 偏移
        y: f32,
        /// 阴影宽度
        w: f32,
        /// 阴影高度
        h: f32,
        /// 模糊半径
        blur: f32,
        /// 阴影颜色 (RGBA)
        color: [f32; 4],
    },
}

// ============================================================================
// RenderQueue - 渲染队列
// ============================================================================

/// 渲染队列，按层组织渲染操作
pub struct RenderQueue {
    /// 渲染操作列表
    ops: Vec<RenderOp>,
    /// 所属渲染层
    layer: RenderLayer,
}

impl RenderQueue {
    /// 创建指定层的渲染队列
    pub fn new(layer: RenderLayer) -> Self {
        Self {
            ops: Vec::new(),
            layer,
        }
    }

    /// 向队列末尾添加渲染操作
    pub fn push(&mut self, op: RenderOp) {
        self.ops.push(op);
    }

    /// 清空队列中的所有操作
    pub fn clear(&mut self) {
        self.ops.clear();
    }

    /// 获取操作列表的不可变引用
    pub fn ops(&self) -> &[RenderOp] {
        &self.ops
    }

    /// 获取所属渲染层
    pub fn layer(&self) -> RenderLayer {
        self.layer
    }

    /// 获取队列中的操作数量
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// 队列是否为空
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

// ============================================================================
// CompositorRenderer - 合成器渲染器
// ============================================================================

/// 合成器渲染器（框架接口）
///
/// 提供合成器的生命周期管理和帧渲染接口。
/// 实际的 Vulkan 调用将在后续集成时实现。
pub struct CompositorRenderer {
    /// Vulkan 配置
    config: VulkanConfig,
    /// 交换链（初始化后可用）
    swapchain: Option<Swapchain>,
    /// FPS 计数器
    fps_counter: FpsCounter,
    /// 当前帧编号
    frame_number: u64,
    /// 是否已初始化
    is_initialized: bool,
    /// 上一帧时间戳（毫秒）
    last_frame_time: f64,
    /// 总时间（毫秒）
    total_time: f64,
}

impl CompositorRenderer {
    /// 创建新的合成器渲染器
    pub fn new(config: VulkanConfig) -> Self {
        Self {
            config,
            swapchain: None,
            fps_counter: FpsCounter::new(120),
            frame_number: 0,
            is_initialized: false,
            last_frame_time: 0.0,
            total_time: 0.0,
        }
    }

    /// 初始化 Vulkan（桩实现）
    ///
    /// 在实际集成中，此处将创建 Vulkan 实例、物理设备、逻辑设备、交换链等。
    pub fn initialize(&mut self) -> Result<(), CompositorError> {
        // 验证配置
        let (w, h) = self.config.surface_size;
        if w == 0 || h == 0 {
            return Err(CompositorError::InvalidConfig(
                "表面尺寸不能为零".to_string(),
            ));
        }
        if self.config.max_frames_in_flight == 0 {
            return Err(CompositorError::InvalidConfig(
                "最大帧中飞行数必须大于零".to_string(),
            ));
        }

        // 桩实现：创建模拟交换链
        let image_count = self.config.max_frames_in_flight + 1;
        self.swapchain = Some(Swapchain::new(
            w,
            h,
            self.config.surface_format,
            image_count,
        ));

        self.is_initialized = true;
        Ok(())
    }

    /// 渲染一帧
    ///
    /// # 参数
    /// - `queues`: 各渲染层的渲染队列
    ///
    /// # 返回
    /// 当前帧的上下文信息
    pub fn render_frame(&mut self, queues: &[RenderQueue]) -> Result<FrameContext, CompositorError> {
        if !self.is_initialized {
            return Err(CompositorError::InitializationFailed(
                "渲染器未初始化".to_string(),
            ));
        }

        // 计算时间
        let current_time = self.total_time;
        let delta = if self.last_frame_time > 0.0 {
            16.667 // 桩实现：模拟 60 FPS
        } else {
            0.0
        };
        self.last_frame_time = current_time;
        self.total_time += delta;

        // 记录帧时间
        self.fps_counter.record_frame(self.total_time);

        // 推进交换链
        if let Some(ref mut swapchain) = self.swapchain {
            swapchain.advance();
        }

        // 桩实现：遍历渲染队列（实际将提交到 Vulkan 命令缓冲区）
        let _total_ops: usize = queues.iter().map(|q| q.len()).sum();

        // 构建帧上下文
        let ctx = FrameContext {
            frame_number: self.frame_number,
            swapchain_index: self
                .swapchain
                .as_ref()
                .map(|sc| sc.current_image_index)
                .unwrap_or(0),
            delta_time_ms: delta as f32,
            total_time_ms: self.total_time,
        };

        self.frame_number += 1;
        Ok(ctx)
    }

    /// 重新创建交换链（通常在窗口大小变化时调用）
    pub fn recreate_swapchain(&mut self, width: u32, height: u32) -> Result<(), CompositorError> {
        if !self.is_initialized {
            return Err(CompositorError::InitializationFailed(
                "渲染器未初始化，无法重建交换链".to_string(),
            ));
        }
        if width == 0 || height == 0 {
            return Err(CompositorError::InvalidConfig(
                "交换链尺寸不能为零".to_string(),
            ));
        }

        let image_count = self.config.max_frames_in_flight + 1;
        self.swapchain = Some(Swapchain::new(
            width,
            height,
            self.config.surface_format,
            image_count,
        ));
        Ok(())
    }

    /// 获取当前 FPS
    pub fn fps(&self) -> f64 {
        self.fps_counter.fps()
    }

    /// 获取 Vulkan 配置的不可变引用
    pub fn config(&self) -> &VulkanConfig {
        &self.config
    }

    /// 是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// 获取帧编号
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    /// 关闭渲染器，释放资源
    pub fn shutdown(&mut self) {
        self.swapchain = None;
        self.is_initialized = false;
        self.frame_number = 0;
        self.fps_counter.reset();
        self.last_frame_time = 0.0;
        self.total_time = 0.0;
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- CompositorError 测试 ----

    #[test]
    fn test_compositor_error_display() {
        let err = CompositorError::SurfaceLost;
        assert_eq!(format!("{}", err), "表面已丢失");

        let err = CompositorError::DeviceLost;
        assert_eq!(format!("{}", err), "设备已丢失");

        let err = CompositorError::OutOfMemory;
        assert_eq!(format!("{}", err), "GPU 内存不足");

        let err = CompositorError::InitializationFailed("测试错误".to_string());
        assert_eq!(format!("{}", err), "初始化失败: 测试错误");
    }

    // ---- RenderLayer 测试 ----

    #[test]
    fn test_render_layer_ordering() {
        assert!(RenderLayer::Background < RenderLayer::Windows);
        assert!(RenderLayer::Windows < RenderLayer::Overlays);
        assert!(RenderLayer::Overlays < RenderLayer::Notifications);
        assert!(RenderLayer::Notifications < RenderLayer::Cursor);
    }

    // ---- RenderOp 测试 ----

    #[test]
    fn test_render_op_clear() {
        let op = RenderOp::Clear {
            color: [0.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(
            op,
            RenderOp::Clear {
                color: [0.0, 0.0, 0.0, 1.0]
            }
        );
    }

    #[test]
    fn test_render_op_draw_rect() {
        let op = RenderOp::DrawRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
            color: [1.0, 0.0, 0.0, 1.0],
            border_radius: 5.0,
        };
        if let RenderOp::DrawRect {
            x, y, w, h, color, border_radius
        } = &op
        {
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 20.0);
            assert_eq!(*w, 100.0);
            assert_eq!(*h, 50.0);
            assert_eq!(*color, [1.0, 0.0, 0.0, 1.0]);
            assert_eq!(*border_radius, 5.0);
        } else {
            panic!("应为 DrawRect 操作");
        }
    }

    #[test]
    fn test_render_op_draw_text() {
        let op = RenderOp::DrawText {
            x: 5.0,
            y: 10.0,
            text: "Hello".to_string(),
            font_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
        };
        if let RenderOp::DrawText { text, .. } = &op {
            assert_eq!(text, "Hello");
        } else {
            panic!("应为 DrawText 操作");
        }
    }

    // ---- RenderQueue 测试 ----

    #[test]
    fn test_render_queue_new() {
        let queue = RenderQueue::new(RenderLayer::Windows);
        assert_eq!(queue.layer(), RenderLayer::Windows);
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_render_queue_push() {
        let mut queue = RenderQueue::new(RenderLayer::Background);
        queue.push(RenderOp::Clear {
            color: [0.0, 0.0, 0.0, 1.0],
        });
        queue.push(RenderOp::DrawRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
            color: [1.0, 0.0, 0.0, 1.0],
            border_radius: 0.0,
        });
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());
    }

    #[test]
    fn test_render_queue_clear() {
        let mut queue = RenderQueue::new(RenderLayer::Overlays);
        queue.push(RenderOp::Clear {
            color: [0.0, 0.0, 0.0, 1.0],
        });
        queue.push(RenderOp::PopClip);
        assert_eq!(queue.len(), 2);
        queue.clear();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_render_queue_ops() {
        let mut queue = RenderQueue::new(RenderLayer::Cursor);
        queue.push(RenderOp::PopClip);
        queue.push(RenderOp::PopTransform);
        let ops = queue.ops();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0], RenderOp::PopClip);
        assert_eq!(ops[1], RenderOp::PopTransform);
    }

    #[test]
    fn test_render_queue_layer() {
        let bg = RenderQueue::new(RenderLayer::Background);
        let cursor = RenderQueue::new(RenderLayer::Cursor);
        assert_eq!(bg.layer(), RenderLayer::Background);
        assert_eq!(cursor.layer(), RenderLayer::Cursor);
    }

    // ---- CompositorRenderer 测试 ----

    #[test]
    fn test_compositor_renderer_new() {
        let renderer = CompositorRenderer::new(VulkanConfig::default());
        assert!(!renderer.is_initialized());
        assert_eq!(renderer.frame_number(), 0);
        assert_eq!(renderer.fps(), 0.0);
        assert_eq!(renderer.config().application_name, "OmniAgent Compositor");
    }

    #[test]
    fn test_compositor_renderer_initialize() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        assert!(!renderer.is_initialized());
        let result = renderer.initialize();
        assert!(result.is_ok());
        assert!(renderer.is_initialized());
    }

    #[test]
    fn test_compositor_renderer_initialize_invalid_size() {
        let mut renderer = CompositorRenderer::new(VulkanConfig {
            surface_size: (0, 1080),
            ..VulkanConfig::default()
        });
        let result = renderer.initialize();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            CompositorError::InvalidConfig("表面尺寸不能为零".to_string())
        );
    }

    #[test]
    fn test_compositor_renderer_initialize_zero_flight_frames() {
        let mut renderer = CompositorRenderer::new(VulkanConfig {
            max_frames_in_flight: 0,
            ..VulkanConfig::default()
        });
        let result = renderer.initialize();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            CompositorError::InvalidConfig("最大帧中飞行数必须大于零".to_string())
        );
    }

    #[test]
    fn test_compositor_renderer_render_frame() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        renderer.initialize().unwrap();

        let bg_queue = RenderQueue::new(RenderLayer::Background);
        let win_queue = RenderQueue::new(RenderLayer::Windows);
        let queues = vec![bg_queue, win_queue];

        let ctx = renderer.render_frame(&queues).unwrap();
        assert_eq!(ctx.frame_number, 0);
        assert_eq!(renderer.frame_number(), 1);
    }

    #[test]
    fn test_compositor_renderer_render_frame_without_init() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        let queues = vec![RenderQueue::new(RenderLayer::Background)];
        let result = renderer.render_frame(&queues);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            CompositorError::InitializationFailed("渲染器未初始化".to_string())
        );
    }

    #[test]
    fn test_compositor_renderer_multiple_frames() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        renderer.initialize().unwrap();

        let queues = vec![RenderQueue::new(RenderLayer::Background)];

        for i in 0..5 {
            let ctx = renderer.render_frame(&queues).unwrap();
            assert_eq!(ctx.frame_number, i as u64);
        }
        assert_eq!(renderer.frame_number(), 5);
    }

    #[test]
    fn test_compositor_renderer_recreate_swapchain() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        renderer.initialize().unwrap();
        let result = renderer.recreate_swapchain(2560, 1440);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compositor_renderer_recreate_swapchain_invalid() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        renderer.initialize().unwrap();
        let result = renderer.recreate_swapchain(0, 1440);
        assert!(result.is_err());
    }

    #[test]
    fn test_compositor_renderer_shutdown() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        renderer.initialize().unwrap();
        assert!(renderer.is_initialized());

        renderer.shutdown();
        assert!(!renderer.is_initialized());
        assert_eq!(renderer.frame_number(), 0);
        assert_eq!(renderer.fps(), 0.0);
    }

    #[test]
    fn test_compositor_renderer_shutdown_and_render() {
        let mut renderer = CompositorRenderer::new(VulkanConfig::default());
        renderer.initialize().unwrap();
        renderer.shutdown();

        let queues = vec![RenderQueue::new(RenderLayer::Background)];
        let result = renderer.render_frame(&queues);
        assert!(result.is_err());
    }
}
