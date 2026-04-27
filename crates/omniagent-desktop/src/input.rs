//! 输入事件路由
//!
//! 提供鼠标和键盘事件的路由机制，将输入事件分发到正确的目标窗口。

use crate::renderable::{RenderableWindow, WindowId};

/// 鼠标按钮
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// 左键
    Left,
    /// 右键
    Right,
    /// 中键
    Middle,
    /// 后退键
    Back,
    /// 前进键
    Forward,
}

/// 鼠标事件类型
#[derive(Debug, Clone, Copy)]
pub enum MouseEventKind {
    /// 鼠标移动
    Move,
    /// 按钮按下
    ButtonDown,
    /// 按钮释放
    ButtonUp,
    /// 滚轮滚动
    Scroll { delta_x: f32, delta_y: f32 },
}

/// 鼠标事件
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// 事件类型
    pub kind: MouseEventKind,
    /// 鼠标 x 坐标
    pub x: f32,
    /// 鼠标 y 坐标
    pub y: f32,
    /// 按钮信息
    pub button: MouseButton,
}

/// 键码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Space, Enter, Escape, Tab, Backspace, Delete,
    Up, Down, Left, Right,
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    /// 未知键码
    Unknown(u16),
}

/// 按键状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// 按下
    Pressed,
    /// 释放
    Released,
}

/// 键盘事件
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// 键码
    pub key: KeyCode,
    /// 按键状态
    pub state: KeyState,
}

/// 输入目标
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputTarget {
    /// 窗口
    Window(WindowId),
    /// 桌面
    Desktop,
    /// 无目标
    None,
}

/// 输入路由器
///
/// 负责将输入事件路由到正确的目标窗口或桌面。
pub struct InputRouter {
    /// 当前聚焦的窗口
    focused_window: Option<WindowId>,
    /// 当前鼠标位置
    mouse_pos: (f32, f32),
}

impl InputRouter {
    /// 创建新的输入路由器
    pub fn new() -> Self {
        InputRouter {
            focused_window: None,
            mouse_pos: (0.0, 0.0),
        }
    }

    /// 设置聚焦窗口
    pub fn set_focus(&mut self, id: Option<WindowId>) {
        self.focused_window = id;
    }

    /// 获取当前聚焦的窗口
    pub fn focused_window(&self) -> Option<WindowId> {
        self.focused_window
    }

    /// 路由鼠标事件到目标窗口
    ///
    /// 根据鼠标位置进行命中测试，确定事件目标。
    pub fn route_mouse_event(&mut self, event: &MouseEvent, windows: &[RenderableWindow]) -> InputTarget {
        self.mouse_pos = (event.x, event.y);
        self.hit_test(event.x, event.y, windows)
    }

    /// 路由键盘事件到聚焦窗口
    pub fn route_key_event(&self, _event: &KeyEvent) -> InputTarget {
        match self.focused_window {
            Some(id) => InputTarget::Window(id),
            None => InputTarget::Desktop,
        }
    }

    /// 命中测试：根据坐标查找最上层的可见窗口
    ///
    /// 从窗口列表末尾（最上层）开始查找。
    pub fn hit_test(&self, x: f32, y: f32, windows: &[RenderableWindow]) -> InputTarget {
        // 从最上层开始查找（列表末尾是最上层）
        for win in windows.iter().rev() {
            if win.visible && win.contains_point(x as i32, y as i32) {
                return InputTarget::Window(win.id);
            }
        }
        InputTarget::Desktop
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window(id: u64, x: i32, y: i32, w: u32, h: u32) -> RenderableWindow {
        RenderableWindow::new(WindowId(id), "测试", x, y, w, h)
    }

    #[test]
    fn test_new() {
        let router = InputRouter::new();
        assert!(router.focused_window().is_none());
    }

    #[test]
    fn test_set_focus() {
        let mut router = InputRouter::new();
        assert!(router.focused_window().is_none());
        router.set_focus(Some(WindowId(1)));
        assert_eq!(router.focused_window(), Some(WindowId(1)));
        router.set_focus(None);
        assert!(router.focused_window().is_none());
    }

    #[test]
    fn test_hit_test_window() {
        let router = InputRouter::new();
        let windows = vec![
            make_window(1, 0, 0, 100, 100),
            make_window(2, 50, 50, 100, 100),
        ];
        // 在窗口 2 区域内（最上层）
        assert_eq!(
            router.hit_test(75.0, 75.0, &windows),
            InputTarget::Window(WindowId(2))
        );
        // 只在窗口 1 区域内
        assert_eq!(
            router.hit_test(25.0, 25.0, &windows),
            InputTarget::Window(WindowId(1))
        );
    }

    #[test]
    fn test_hit_test_desktop() {
        let router = InputRouter::new();
        let windows = vec![
            make_window(1, 0, 0, 100, 100),
        ];
        // 不在任何窗口内
        assert_eq!(
            router.hit_test(200.0, 200.0, &windows),
            InputTarget::Desktop
        );
    }

    #[test]
    fn test_hit_test_invisible_window() {
        let router = InputRouter::new();
        let mut win = make_window(1, 0, 0, 100, 100);
        win.visible = false;
        let windows = vec![win];
        assert_eq!(
            router.hit_test(50.0, 50.0, &windows),
            InputTarget::Desktop
        );
    }

    #[test]
    fn test_route_mouse() {
        let mut router = InputRouter::new();
        let windows = vec![make_window(1, 0, 0, 100, 100)];
        let event = MouseEvent {
            kind: MouseEventKind::Move,
            x: 50.0,
            y: 50.0,
            button: MouseButton::Left,
        };
        assert_eq!(
            router.route_mouse_event(&event, &windows),
            InputTarget::Window(WindowId(1))
        );
    }

    #[test]
    fn test_route_key_with_focus() {
        let mut router = InputRouter::new();
        router.set_focus(Some(WindowId(42)));
        let event = KeyEvent {
            key: KeyCode::A,
            state: KeyState::Pressed,
        };
        assert_eq!(
            router.route_key_event(&event),
            InputTarget::Window(WindowId(42))
        );
    }

    #[test]
    fn test_route_key_without_focus() {
        let router = InputRouter::new();
        let event = KeyEvent {
            key: KeyCode::A,
            state: KeyState::Pressed,
        };
        assert_eq!(
            router.route_key_event(&event),
            InputTarget::Desktop
        );
    }
}
