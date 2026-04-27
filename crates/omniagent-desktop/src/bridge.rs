//! 桌面桥接接口
//!
//! 提供桌面窗口管理的桥接层，抽象窗口的添加、移除、更新等操作。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::error::DesktopError;
use crate::renderable::{RenderableWindow, WindowId};

/// 桌面桥接 trait
///
/// 定义桌面窗口管理的统一接口，所有桌面后端都需要实现此 trait。
pub trait DesktopBridge: Send + Sync {
    /// 添加窗口
    fn add_window(&self, window: &RenderableWindow) -> Result<WindowId, DesktopError>;
    /// 移除窗口
    fn remove_window(&self, id: WindowId) -> Result<(), DesktopError>;
    /// 更新窗口位置和大小
    fn update_window(&self, id: WindowId, x: i32, y: i32, width: u32, height: u32) -> Result<(), DesktopError>;
    /// 获取窗口
    fn get_window(&self, id: WindowId) -> Option<RenderableWindow>;
    /// 列出所有窗口
    fn list_windows(&self) -> Vec<RenderableWindow>;
    /// 获取窗口数量
    fn window_count(&self) -> usize;
}

/// 桌面桥接默认实现
///
/// 基于内存的窗口管理实现，用于测试和原型开发。
pub struct DesktopBridgeImpl {
    /// 窗口列表（使用 Mutex 保证线程安全）
    windows: Mutex<Vec<RenderableWindow>>,
    /// 下一个可用的窗口 ID
    next_id: AtomicU64,
}

impl DesktopBridgeImpl {
    /// 创建新的桌面桥接实例
    pub fn new() -> Self {
        DesktopBridgeImpl {
            windows: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }
}

impl Default for DesktopBridgeImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopBridge for DesktopBridgeImpl {
    fn add_window(&self, window: &RenderableWindow) -> Result<WindowId, DesktopError> {
        let id = WindowId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let mut win = window.clone();
        win.id = id;
        let mut windows = self.windows.lock().unwrap();
        windows.push(win);
        Ok(id)
    }

    fn remove_window(&self, id: WindowId) -> Result<(), DesktopError> {
        let mut windows = self.windows.lock().unwrap();
        let len_before = windows.len();
        windows.retain(|w| w.id != id);
        if windows.len() < len_before {
            Ok(())
        } else {
            Err(DesktopError::WindowNotFound(id.0))
        }
    }

    fn update_window(&self, id: WindowId, x: i32, y: i32, width: u32, height: u32) -> Result<(), DesktopError> {
        let mut windows = self.windows.lock().unwrap();
        let win = windows.iter_mut().find(|w| w.id == id)
            .ok_or(DesktopError::WindowNotFound(id.0))?;
        win.x = x;
        win.y = y;
        win.width = width;
        win.height = height;
        Ok(())
    }

    fn get_window(&self, id: WindowId) -> Option<RenderableWindow> {
        let windows = self.windows.lock().unwrap();
        windows.iter().find(|w| w.id == id).cloned()
    }

    fn list_windows(&self) -> Vec<RenderableWindow> {
        let windows = self.windows.lock().unwrap();
        windows.clone()
    }

    fn window_count(&self) -> usize {
        let windows = self.windows.lock().unwrap();
        windows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window(title: &str, x: i32, y: i32, w: u32, h: u32) -> RenderableWindow {
        RenderableWindow::new(WindowId(0), title, x, y, w, h)
    }

    #[test]
    fn test_add_remove_window() {
        let bridge = DesktopBridgeImpl::new();
        let win = make_window("窗口1", 10, 20, 100, 200);
        let id = bridge.add_window(&win).unwrap();
        assert_eq!(bridge.window_count(), 1);
        bridge.remove_window(id).unwrap();
        assert_eq!(bridge.window_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_window() {
        let bridge = DesktopBridgeImpl::new();
        let result = bridge.remove_window(WindowId(999));
        assert!(matches!(result, Err(DesktopError::WindowNotFound(999))));
    }

    #[test]
    fn test_update_window() {
        let bridge = DesktopBridgeImpl::new();
        let win = make_window("窗口1", 10, 20, 100, 200);
        let id = bridge.add_window(&win).unwrap();
        bridge.update_window(id, 50, 60, 300, 400).unwrap();
        let w = bridge.get_window(id).unwrap();
        assert_eq!(w.x, 50);
        assert_eq!(w.y, 60);
        assert_eq!(w.width, 300);
        assert_eq!(w.height, 400);
    }

    #[test]
    fn test_update_nonexistent_window() {
        let bridge = DesktopBridgeImpl::new();
        let result = bridge.update_window(WindowId(999), 0, 0, 100, 100);
        assert!(matches!(result, Err(DesktopError::WindowNotFound(999))));
    }

    #[test]
    fn test_list_windows() {
        let bridge = DesktopBridgeImpl::new();
        bridge.add_window(&make_window("窗口1", 0, 0, 100, 100)).unwrap();
        bridge.add_window(&make_window("窗口2", 10, 10, 200, 200)).unwrap();
        bridge.add_window(&make_window("窗口3", 20, 20, 300, 300)).unwrap();
        let list = bridge.list_windows();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].title, "窗口1");
        assert_eq!(list[1].title, "窗口2");
        assert_eq!(list[2].title, "窗口3");
    }

    #[test]
    fn test_get_window() {
        let bridge = DesktopBridgeImpl::new();
        let win = make_window("测试窗口", 10, 20, 100, 200);
        let id = bridge.add_window(&win).unwrap();
        let fetched = bridge.get_window(id).unwrap();
        assert_eq!(fetched.title, "测试窗口");
        assert_eq!(fetched.x, 10);
        assert_eq!(fetched.y, 20);
    }

    #[test]
    fn test_get_nonexistent_window() {
        let bridge = DesktopBridgeImpl::new();
        assert!(bridge.get_window(WindowId(999)).is_none());
    }

    #[test]
    fn test_window_count() {
        let bridge = DesktopBridgeImpl::new();
        assert_eq!(bridge.window_count(), 0);
        bridge.add_window(&make_window("窗口1", 0, 0, 100, 100)).unwrap();
        assert_eq!(bridge.window_count(), 1);
        bridge.add_window(&make_window("窗口2", 0, 0, 100, 100)).unwrap();
        assert_eq!(bridge.window_count(), 2);
    }

    #[test]
    fn test_add_window_auto_id() {
        let bridge = DesktopBridgeImpl::new();
        let id1 = bridge.add_window(&make_window("窗口1", 0, 0, 100, 100)).unwrap();
        let id2 = bridge.add_window(&make_window("窗口2", 0, 0, 100, 100)).unwrap();
        assert_eq!(id1, WindowId(1));
        assert_eq!(id2, WindowId(2));
    }
}
