//! 可渲染窗口和像素缓冲区
//!
//! 提供桌面环境中窗口渲染所需的核心类型，包括像素缓冲区、
//! 窗口表面、可渲染窗口和仿射变换。

/// 窗口标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

/// 渲染层级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLayer {
    /// 背景层
    Background = 0,
    /// 桌面层
    Desktop = 1,
    /// 窗口层
    Windows = 2,
    /// 面板层
    Panels = 3,
    /// 叠加层
    Overlay = 4,
    /// 光标层
    Cursor = 5,
}

/// 像素缓冲区（ARGB8888 格式）
#[derive(Debug, Clone)]
pub struct PixelBuffer {
    /// 像素数据（ARGB8888）
    pub data: Vec<u32>,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
    /// 行跨度（每行像素数，可能大于 width 以支持对齐）
    pub stride: u32,
}

impl PixelBuffer {
    /// 创建新的像素缓冲区
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width;
        let data = vec![0u32; (width * height) as usize];
        PixelBuffer {
            data,
            width,
            height,
            stride,
        }
    }

    /// 设置指定位置的像素颜色
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            let index = (y * self.stride + x) as usize;
            self.data[index] = color;
        }
    }

    /// 获取指定位置的像素颜色
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x < self.width && y < self.height {
            let index = (y * self.stride + x) as usize;
            self.data[index]
        } else {
            0
        }
    }

    /// 用指定颜色清空整个缓冲区
    pub fn clear(&mut self, color: u32) {
        for pixel in self.data.iter_mut() {
            *pixel = color;
        }
    }

    /// 填充矩形区域
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                let px = x + dx;
                let py = y + dy;
                if px < self.width && py < self.height {
                    let index = (py * self.stride + px) as usize;
                    self.data[index] = color;
                }
            }
        }
    }

    /// 获取缓冲区尺寸 (宽, 高)
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// 判断缓冲区是否为空（宽度或高度为 0）
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// 窗口渲染表面
#[derive(Debug, Clone)]
pub struct WindowSurface {
    /// 像素缓冲区
    pub buffer: PixelBuffer,
    /// 是否有脏区域需要重绘
    pub dirty: bool,
    /// 像素格式（ARGB8888 = 1）
    pub format: u32,
}

/// 可渲染窗口
#[derive(Debug, Clone)]
pub struct RenderableWindow {
    /// 窗口 ID
    pub id: WindowId,
    /// 窗口标题
    pub title: String,
    /// 窗口左上角 x 坐标
    pub x: i32,
    /// 窗口左上角 y 坐标
    pub y: i32,
    /// 窗口宽度
    pub width: u32,
    /// 窗口高度
    pub height: u32,
    /// 是否可见
    pub visible: bool,
    /// 渲染层级
    pub layer: RenderLayer,
    /// 不透明度 (0.0 - 1.0)
    pub opacity: f32,
}

impl RenderableWindow {
    /// 创建新的可渲染窗口
    pub fn new(id: WindowId, title: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        RenderableWindow {
            id,
            title: title.to_string(),
            x,
            y,
            width,
            height,
            visible: true,
            layer: RenderLayer::Windows,
            opacity: 1.0,
        }
    }

    /// 判断指定点是否在窗口范围内
    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }

    /// 获取窗口边界 (x, y, width, height)
    pub fn bounds(&self) -> (i32, i32, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }
}

/// 帧令牌，用于标识一帧渲染
#[derive(Debug, Clone)]
pub struct FrameToken(pub u64);

/// 表面更新信息
#[derive(Debug, Clone)]
pub struct SurfaceUpdate {
    /// 目标窗口 ID
    pub window_id: WindowId,
    /// 新的像素缓冲区
    pub buffer: PixelBuffer,
    /// 不透明度
    pub opacity: f32,
}

/// 仿射变换
#[derive(Debug, Clone)]
pub struct AffineTransform {
    /// X 轴平移
    pub translate_x: f32,
    /// Y 轴平移
    pub translate_y: f32,
    /// X 轴缩放
    pub scale_x: f32,
    /// Y 轴缩放
    pub scale_y: f32,
    /// 旋转角度（弧度）
    pub rotation: f32,
}

impl AffineTransform {
    /// 创建单位变换
    pub fn identity() -> Self {
        AffineTransform {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }

    /// 创建平移变换
    pub fn translate(x: f32, y: f32) -> Self {
        AffineTransform {
            translate_x: x,
            translate_y: y,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }

    /// 创建缩放变换
    pub fn scale(sx: f32, sy: f32) -> Self {
        AffineTransform {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: sx,
            scale_y: sy,
            rotation: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let buf = PixelBuffer::new(10, 20);
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 20);
        assert_eq!(buf.stride, 10);
        assert_eq!(buf.data.len(), 200);
    }

    #[test]
    fn test_set_get_pixel() {
        let mut buf = PixelBuffer::new(10, 10);
        buf.set_pixel(3, 4, 0xFFAABBCC);
        assert_eq!(buf.get_pixel(3, 4), 0xFFAABBCC);
        // 其他位置应为 0
        assert_eq!(buf.get_pixel(0, 0), 0);
    }

    #[test]
    fn test_set_pixel_out_of_bounds() {
        let mut buf = PixelBuffer::new(10, 10);
        // 越界设置不应 panic
        buf.set_pixel(10, 10, 0xFFFFFFFF);
        assert_eq!(buf.get_pixel(10, 10), 0);
    }

    #[test]
    fn test_clear() {
        let mut buf = PixelBuffer::new(5, 5);
        buf.set_pixel(0, 0, 0xFFFFFFFF);
        buf.clear(0x11223344);
        for i in 0..25 {
            assert_eq!(buf.data[i], 0x11223344);
        }
    }

    #[test]
    fn test_fill_rect() {
        let mut buf = PixelBuffer::new(10, 10);
        buf.fill_rect(2, 3, 4, 5, 0xFF0000FF);
        // 填充区域内的像素应为目标颜色
        assert_eq!(buf.get_pixel(2, 3), 0xFF0000FF);
        assert_eq!(buf.get_pixel(5, 7), 0xFF0000FF);
        // 填充区域外的像素应为 0
        assert_eq!(buf.get_pixel(0, 0), 0);
        assert_eq!(buf.get_pixel(1, 3), 0);
    }

    #[test]
    fn test_size() {
        let buf = PixelBuffer::new(100, 200);
        assert_eq!(buf.size(), (100, 200));
    }

    #[test]
    fn test_is_empty() {
        let buf = PixelBuffer::new(0, 10);
        assert!(buf.is_empty());
        let buf = PixelBuffer::new(10, 0);
        assert!(buf.is_empty());
        let buf = PixelBuffer::new(1, 1);
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_renderable_window_new() {
        let win = RenderableWindow::new(WindowId(1), "测试", 10, 20, 300, 400);
        assert_eq!(win.id, WindowId(1));
        assert_eq!(win.title, "测试");
        assert_eq!(win.x, 10);
        assert_eq!(win.y, 20);
        assert_eq!(win.width, 300);
        assert_eq!(win.height, 400);
        assert!(win.visible);
        assert_eq!(win.layer, RenderLayer::Windows);
        assert!((win.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_contains_point() {
        let win = RenderableWindow::new(WindowId(1), "测试", 10, 10, 100, 100);
        // 内部点
        assert!(win.contains_point(10, 10));
        assert!(win.contains_point(50, 50));
        assert!(win.contains_point(109, 109));
        // 外部点
        assert!(!win.contains_point(9, 10));
        assert!(!win.contains_point(10, 9));
        assert!(!win.contains_point(110, 10));
        assert!(!win.contains_point(0, 0));
    }

    #[test]
    fn test_bounds() {
        let win = RenderableWindow::new(WindowId(1), "测试", 10, 20, 300, 400);
        assert_eq!(win.bounds(), (10, 20, 300, 400));
    }

    #[test]
    fn test_affine_identity() {
        let t = AffineTransform::identity();
        assert!((t.translate_x - 0.0).abs() < f32::EPSILON);
        assert!((t.translate_y - 0.0).abs() < f32::EPSILON);
        assert!((t.scale_x - 1.0).abs() < f32::EPSILON);
        assert!((t.scale_y - 1.0).abs() < f32::EPSILON);
        assert!((t.rotation - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_affine_translate() {
        let t = AffineTransform::translate(10.0, 20.0);
        assert!((t.translate_x - 10.0).abs() < f32::EPSILON);
        assert!((t.translate_y - 20.0).abs() < f32::EPSILON);
        assert!((t.scale_x - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_affine_scale() {
        let t = AffineTransform::scale(2.0, 3.0);
        assert!((t.scale_x - 2.0).abs() < f32::EPSILON);
        assert!((t.scale_y - 3.0).abs() < f32::EPSILON);
        assert!((t.translate_x - 0.0).abs() < f32::EPSILON);
    }
}
