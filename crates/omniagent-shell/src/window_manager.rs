//! 窗口管理器

use std::collections::HashMap;

use crate::error::ShellError;
use crate::rect::Rect;
use crate::window::{Window, WindowId, WindowProperties, WindowState};

/// 窗口管理器
///
/// 负责管理所有窗口的生命周期，包括创建、销毁、聚焦、
/// 最小化、最大化、移动和调整大小等操作。
pub struct WindowManager {
    /// 所有窗口的映射表
    windows: HashMap<WindowId, Window>,
    /// Z 轴顺序列表（从底到顶）
    z_order: Vec<WindowId>,
    /// 当前聚焦的窗口
    focused_window: Option<WindowId>,
    /// 下一个可用的窗口 ID
    next_id: u64,
    /// 屏幕尺寸 (宽, 高)
    screen_size: (u32, u32),
}

impl WindowManager {
    /// 创建新的窗口管理器
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        WindowManager {
            windows: HashMap::new(),
            z_order: Vec::new(),
            focused_window: None,
            next_id: 1,
            screen_size: (screen_width, screen_height),
        }
    }

    /// 创建窗口并返回窗口 ID
    pub fn create_window(&mut self, props: WindowProperties) -> WindowId {
        let id = WindowId(self.next_id);
        self.next_id += 1;

        let z = self.z_order.len() as u32;
        let mut window = Window::new(id, props);
        window.z_order = z;
        window.created_at = 0; // 实际环境中应使用时间戳

        self.z_order.push(id);
        self.windows.insert(id, window);

        // 自动聚焦新窗口
        let _ = self.focus_window(id);

        id
    }

    /// 销毁窗口
    pub fn destroy_window(&mut self, id: WindowId) -> Result<(), ShellError> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound(id));
        }

        self.windows.remove(&id);
        self.z_order.retain(|&wid| wid != id);

        // 如果销毁的是聚焦窗口，聚焦最上层的窗口
        if self.focused_window == Some(id) {
            self.focused_window = self.z_order.last().copied();
            if let Some(fid) = self.focused_window {
                if let Some(w) = self.windows.get_mut(&fid) {
                    w.focused = true;
                }
            }
        }

        Ok(())
    }

    /// 聚焦窗口（将其提升到最上层）
    pub fn focus_window(&mut self, id: WindowId) -> Result<(), ShellError> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound(id));
        }

        // 取消之前的聚焦
        if let Some(old_id) = self.focused_window {
            if let Some(w) = self.windows.get_mut(&old_id) {
                w.focused = false;
            }
        }

        // 将窗口移到 z_order 最顶层
        self.z_order.retain(|&wid| wid != id);
        self.z_order.push(id);

        // 更新所有窗口的 z_order 值
        for (i, &wid) in self.z_order.iter().enumerate() {
            if let Some(w) = self.windows.get_mut(&wid) {
                w.z_order = i as u32;
            }
        }

        // 设置新的聚焦窗口
        if let Some(w) = self.windows.get_mut(&id) {
            w.focused = true;
        }
        self.focused_window = Some(id);

        Ok(())
    }

    /// 最小化窗口
    pub fn minimize_window(&mut self, id: WindowId) -> Result<(), ShellError> {
        let window = self.windows.get_mut(&id)
            .ok_or(ShellError::WindowNotFound(id))?;
        window.state = WindowState::Minimized;
        Ok(())
    }

    /// 最大化窗口
    pub fn maximize_window(&mut self, id: WindowId) -> Result<(), ShellError> {
        let window = self.windows.get_mut(&id)
            .ok_or(ShellError::WindowNotFound(id))?;
        window.state = WindowState::Maximized;
        // 最大化时设置窗口边界为屏幕尺寸
        window.properties.bounds = Rect::new(0, 0, self.screen_size.0, self.screen_size.1);
        Ok(())
    }

    /// 恢复窗口到正常状态
    pub fn restore_window(&mut self, id: WindowId) -> Result<(), ShellError> {
        let window = self.windows.get_mut(&id)
            .ok_or(ShellError::WindowNotFound(id))?;
        window.state = WindowState::Normal;
        Ok(())
    }

    /// 移动窗口
    pub fn move_window(&mut self, id: WindowId, x: i32, y: i32) -> Result<(), ShellError> {
        let window = self.windows.get_mut(&id)
            .ok_or(ShellError::WindowNotFound(id))?;
        window.set_position(x, y);
        Ok(())
    }

    /// 调整窗口大小
    pub fn resize_window(&mut self, id: WindowId, width: u32, height: u32) -> Result<(), ShellError> {
        let window = self.windows.get_mut(&id)
            .ok_or(ShellError::WindowNotFound(id))?;

        // 检查最小尺寸限制
        if let Some((min_w, min_h)) = window.properties.min_size {
            if width < min_w || height < min_h {
                return Err(ShellError::InvalidBounds(
                    format!("尺寸 ({}, {}) 小于最小尺寸 ({}, {})", width, height, min_w, min_h)
                ));
            }
        }

        // 检查最大尺寸限制
        if let Some((max_w, max_h)) = window.properties.max_size {
            if width > max_w || height > max_h {
                return Err(ShellError::InvalidBounds(
                    format!("尺寸 ({}, {}) 超过最大尺寸 ({}, {})", width, height, max_w, max_h)
                ));
            }
        }

        window.set_size(width, height);
        Ok(())
    }

    /// 获取窗口的不可变引用
    pub fn get_window(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    /// 获取聚焦窗口的不可变引用
    pub fn focused_window(&self) -> Option<&Window> {
        self.focused_window.and_then(|id| self.windows.get(&id))
    }

    /// 列出所有窗口（按 z-order 从底到顶）
    pub fn list_windows(&self) -> Vec<&Window> {
        self.z_order
            .iter()
            .filter_map(|id| self.windows.get(id))
            .collect()
    }

    /// 获取窗口数量
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// 查找指定位置最上层的可见窗口
    pub fn window_at(&self, x: i32, y: i32) -> Option<WindowId> {
        // 从最上层开始查找
        for wid in self.z_order.iter().rev() {
            if let Some(w) = self.windows.get(wid) {
                if w.state != WindowState::Minimized && w.state != WindowState::Hidden {
                    if w.properties.bounds.contains(x, y) {
                        return Some(*wid);
                    }
                }
            }
        }
        None
    }

    /// 获取屏幕尺寸
    pub fn screen_size(&self) -> (u32, u32) {
        self.screen_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn default_props() -> WindowProperties {
        WindowProperties {
            bounds: Rect::new(10, 10, 100, 100),
            ..WindowProperties::default()
        }
    }

    #[test]
    fn test_create_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        assert_eq!(id, WindowId(1));
        assert_eq!(wm.window_count(), 1);
        assert!(wm.get_window(id).is_some());
    }

    #[test]
    fn test_create_multiple_windows() {
        let mut wm = WindowManager::new(1920, 1080);
        let id1 = wm.create_window(default_props());
        let id2 = wm.create_window(default_props());
        let id3 = wm.create_window(default_props());
        assert_eq!(id1, WindowId(1));
        assert_eq!(id2, WindowId(2));
        assert_eq!(id3, WindowId(3));
        assert_eq!(wm.window_count(), 3);
    }

    #[test]
    fn test_destroy_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        assert_eq!(wm.window_count(), 1);
        assert!(wm.destroy_window(id).is_ok());
        assert_eq!(wm.window_count(), 0);
    }

    #[test]
    fn test_destroy_nonexistent_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let result = wm.destroy_window(WindowId(999));
        assert_eq!(result, Err(ShellError::WindowNotFound(WindowId(999))));
    }

    #[test]
    fn test_focus_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id1 = wm.create_window(default_props());
        let id2 = wm.create_window(default_props());

        // 创建后 id2 应该是聚焦的（最后创建的）
        assert_eq!(wm.focused_window().map(|w| w.id), Some(id2));

        // 聚焦 id1
        wm.focus_window(id1).unwrap();
        assert_eq!(wm.focused_window().map(|w| w.id), Some(id1));

        // id1 应该在最上层
        let list = wm.list_windows();
        assert_eq!(list.last().unwrap().id, id1);
    }

    #[test]
    fn test_focus_nonexistent_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let result = wm.focus_window(WindowId(999));
        assert_eq!(result, Err(ShellError::WindowNotFound(WindowId(999))));
    }

    #[test]
    fn test_minimize_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        wm.minimize_window(id).unwrap();
        assert_eq!(wm.get_window(id).unwrap().state, WindowState::Minimized);
    }

    #[test]
    fn test_maximize_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        wm.maximize_window(id).unwrap();
        let win = wm.get_window(id).unwrap();
        assert_eq!(win.state, WindowState::Maximized);
        assert_eq!(win.properties.bounds, Rect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn test_restore_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        wm.minimize_window(id).unwrap();
        wm.restore_window(id).unwrap();
        assert_eq!(wm.get_window(id).unwrap().state, WindowState::Normal);
    }

    #[test]
    fn test_move_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        wm.move_window(id, 50, 60).unwrap();
        let win = wm.get_window(id).unwrap();
        assert_eq!(win.properties.bounds.x, 50);
        assert_eq!(win.properties.bounds.y, 60);
    }

    #[test]
    fn test_resize_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(default_props());
        wm.resize_window(id, 200, 300).unwrap();
        let win = wm.get_window(id).unwrap();
        assert_eq!(win.properties.bounds.width, 200);
        assert_eq!(win.properties.bounds.height, 300);
    }

    #[test]
    fn test_resize_window_min_size() {
        let mut wm = WindowManager::new(1920, 1080);
        let mut props = default_props();
        props.min_size = Some((100, 100));
        let id = wm.create_window(props);
        let result = wm.resize_window(id, 50, 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_window_max_size() {
        let mut wm = WindowManager::new(1920, 1080);
        let mut props = default_props();
        props.max_size = Some((200, 200));
        let id = wm.create_window(props);
        let result = wm.resize_window(id, 300, 300);
        assert!(result.is_err());
    }

    #[test]
    fn test_window_at() {
        let mut wm = WindowManager::new(1920, 1080);
        let id1 = wm.create_window(WindowProperties {
            bounds: Rect::new(0, 0, 100, 100),
            ..WindowProperties::default()
        });
        let id2 = wm.create_window(WindowProperties {
            bounds: Rect::new(50, 50, 100, 100),
            ..WindowProperties::default()
        });

        // 重叠区域应返回最上层的窗口
        assert_eq!(wm.window_at(75, 75), Some(id2));
        // 只在 id1 中的区域
        assert_eq!(wm.window_at(25, 25), Some(id1));
        // 不在任何窗口中
        assert_eq!(wm.window_at(200, 200), None);
    }

    #[test]
    fn test_window_at_minimized() {
        let mut wm = WindowManager::new(1920, 1080);
        let id = wm.create_window(WindowProperties {
            bounds: Rect::new(0, 0, 100, 100),
            ..WindowProperties::default()
        });
        wm.minimize_window(id).unwrap();
        assert_eq!(wm.window_at(50, 50), None);
    }

    #[test]
    fn test_list_windows_z_order() {
        let mut wm = WindowManager::new(1920, 1080);
        let id1 = wm.create_window(default_props());
        let id2 = wm.create_window(default_props());
        let id3 = wm.create_window(default_props());

        // 聚焦 id1 使其到最上层
        wm.focus_window(id1).unwrap();

        let list = wm.list_windows();
        assert_eq!(list.len(), 3);
        // 从底到顶: id2, id3, id1
        assert_eq!(list[0].id, id2);
        assert_eq!(list[1].id, id3);
        assert_eq!(list[2].id, id1);
    }

    #[test]
    fn test_screen_size() {
        let wm = WindowManager::new(1280, 720);
        assert_eq!(wm.screen_size(), (1280, 720));
    }

    #[test]
    fn test_destroy_focused_window() {
        let mut wm = WindowManager::new(1920, 1080);
        let id1 = wm.create_window(default_props());
        let id2 = wm.create_window(default_props());

        // id2 是聚焦的
        wm.destroy_window(id2).unwrap();
        // 应该聚焦 id1
        assert_eq!(wm.focused_window().map(|w| w.id), Some(id1));
    }

    #[test]
    fn test_destroy_all_windows() {
        let mut wm = WindowManager::new(1920, 1080);
        let id1 = wm.create_window(default_props());
        let id2 = wm.create_window(default_props());
        wm.destroy_window(id1).unwrap();
        wm.destroy_window(id2).unwrap();
        assert_eq!(wm.window_count(), 0);
        assert!(wm.focused_window().is_none());
    }
}
