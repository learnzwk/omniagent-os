//! 窗口类型定义

use crate::color::Color;
use crate::rect::Rect;

/// 窗口 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WindowId({})", self.0)
    }
}

/// 窗口状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowState {
    /// 正常状态
    Normal = 0,
    /// 最小化
    Minimized = 1,
    /// 最大化
    Maximized = 2,
    /// 全屏
    Fullscreen = 3,
    /// 隐藏
    Hidden = 4,
}

impl std::fmt::Display for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowState::Normal => write!(f, "Normal"),
            WindowState::Minimized => write!(f, "Minimized"),
            WindowState::Maximized => write!(f, "Maximized"),
            WindowState::Fullscreen => write!(f, "Fullscreen"),
            WindowState::Hidden => write!(f, "Hidden"),
        }
    }
}

/// 窗口类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowType {
    /// 普通窗口
    Normal = 0,
    /// 对话框
    Dialog = 1,
    /// 弹出窗口
    Popup = 2,
    /// 工具提示
    Tooltip = 3,
    /// 菜单
    Menu = 4,
    /// Dock 栏
    Dock = 5,
    /// 通知
    Notification = 6,
}

/// 窗口属性
#[derive(Debug, Clone)]
pub struct WindowProperties {
    /// 窗口标题
    pub title: String,
    /// 窗口类型
    pub window_type: WindowType,
    /// 窗口边界
    pub bounds: Rect,
    /// 最小尺寸 (宽, 高)
    pub min_size: Option<(u32, u32)>,
    /// 最大尺寸 (宽, 高)
    pub max_size: Option<(u32, u32)>,
    /// 背景颜色
    pub background_color: Color,
    /// 不透明度 (0.0 - 1.0)
    pub opacity: f32,
    /// 是否可调整大小
    pub resizable: bool,
    /// 是否可移动
    pub movable: bool,
    /// 是否始终在最上层
    pub always_on_top: bool,
    /// 是否显示装饰（标题栏/边框）
    pub decorations: bool,
}

impl Default for WindowProperties {
    fn default() -> Self {
        WindowProperties {
            title: String::new(),
            window_type: WindowType::Normal,
            bounds: Rect::new(0, 0, 400, 300),
            min_size: None,
            max_size: None,
            background_color: Color::WHITE,
            opacity: 1.0,
            resizable: true,
            movable: true,
            always_on_top: false,
            decorations: true,
        }
    }
}

/// 窗口
pub struct Window {
    /// 窗口 ID
    pub id: WindowId,
    /// 窗口属性
    pub properties: WindowProperties,
    /// 窗口状态
    pub state: WindowState,
    /// Z 轴顺序
    pub z_order: u32,
    /// 是否聚焦
    pub focused: bool,
    /// 创建时间戳
    pub created_at: u64,
}

impl Window {
    /// 创建新窗口
    pub fn new(id: WindowId, props: WindowProperties) -> Self {
        Window {
            id,
            properties: props,
            state: WindowState::Normal,
            z_order: 0,
            focused: false,
            created_at: 0,
        }
    }

    /// 设置标题
    pub fn set_title(&mut self, title: &str) {
        self.properties.title = title.to_string();
    }

    /// 设置位置
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.properties.bounds.x = x;
        self.properties.bounds.y = y;
    }

    /// 设置大小
    pub fn set_size(&mut self, width: u32, height: u32) {
        self.properties.bounds.width = width;
        self.properties.bounds.height = height;
    }

    /// 设置窗口状态
    pub fn set_state(&mut self, state: WindowState) {
        self.state = state;
    }

    /// 获取可见区域
    /// 最小化或隐藏的窗口返回空矩形
    pub fn visible_bounds(&self) -> Rect {
        match self.state {
            WindowState::Minimized | WindowState::Hidden => Rect::new(0, 0, 0, 0),
            _ => self.properties.bounds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_new() {
        let props = WindowProperties::default();
        let win = Window::new(WindowId(1), props);
        assert_eq!(win.id, WindowId(1));
        assert_eq!(win.state, WindowState::Normal);
        assert!(!win.focused);
        assert_eq!(win.z_order, 0);
    }

    #[test]
    fn test_set_title() {
        let mut win = Window::new(WindowId(1), WindowProperties::default());
        win.set_title("测试窗口");
        assert_eq!(win.properties.title, "测试窗口");
    }

    #[test]
    fn test_set_position() {
        let mut win = Window::new(WindowId(1), WindowProperties::default());
        win.set_position(100, 200);
        assert_eq!(win.properties.bounds.x, 100);
        assert_eq!(win.properties.bounds.y, 200);
    }

    #[test]
    fn test_set_size() {
        let mut win = Window::new(WindowId(1), WindowProperties::default());
        win.set_size(800, 600);
        assert_eq!(win.properties.bounds.width, 800);
        assert_eq!(win.properties.bounds.height, 600);
    }

    #[test]
    fn test_set_state() {
        let mut win = Window::new(WindowId(1), WindowProperties::default());
        win.set_state(WindowState::Minimized);
        assert_eq!(win.state, WindowState::Minimized);
        win.set_state(WindowState::Maximized);
        assert_eq!(win.state, WindowState::Maximized);
    }

    #[test]
    fn test_visible_bounds_normal() {
        let mut props = WindowProperties::default();
        props.bounds = Rect::new(10, 20, 300, 400);
        let win = Window::new(WindowId(1), props);
        let vb = win.visible_bounds();
        assert_eq!(vb.x, 10);
        assert_eq!(vb.y, 20);
        assert_eq!(vb.width, 300);
        assert_eq!(vb.height, 400);
    }

    #[test]
    fn test_visible_bounds_minimized() {
        let win = Window::new(WindowId(1), WindowProperties::default());
        let mut win = win;
        win.set_state(WindowState::Minimized);
        let vb = win.visible_bounds();
        assert!(vb.is_empty());
    }

    #[test]
    fn test_visible_bounds_hidden() {
        let mut win = Window::new(WindowId(1), WindowProperties::default());
        win.set_state(WindowState::Hidden);
        let vb = win.visible_bounds();
        assert!(vb.is_empty());
    }

    #[test]
    fn test_default_properties() {
        let props = WindowProperties::default();
        assert!(props.title.is_empty());
        assert_eq!(props.window_type, WindowType::Normal);
        assert_eq!(props.bounds, Rect::new(0, 0, 400, 300));
        assert!(props.min_size.is_none());
        assert!(props.max_size.is_none());
        assert_eq!(props.background_color, Color::WHITE);
        assert!((props.opacity - 1.0).abs() < f32::EPSILON);
        assert!(props.resizable);
        assert!(props.movable);
        assert!(!props.always_on_top);
        assert!(props.decorations);
    }
}
