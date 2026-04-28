//! Dock 栏组件
//!
//! 实现 macOS 风格的 Dock 栏，支持应用图标管理、
//! 悬停放大效果和点击交互。

use crate::error::ShellError;

/// Dock 项目标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DockItemId(pub u64);

/// Dock 项目状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockItemState {
    /// 普通状态
    Normal,
    /// 运行中
    Running { pid: u64 },
    /// 有通知
    Notification { count: u32 },
    /// 启动中
    Launching,
}

/// Dock 项目
#[derive(Debug, Clone)]
pub struct DockItem {
    /// 项目 ID
    pub id: DockItemId,
    /// 应用名称
    pub app_name: String,
    /// 项目状态
    pub state: DockItemState,
    /// 图标索引
    pub icon_index: usize,
}

/// Dock 栏位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPosition {
    /// 底部
    Bottom,
    /// 左侧
    Left,
    /// 右侧
    Right,
}

/// Dock 栏
///
/// 桌面环境底部的应用快捷栏，支持图标管理、悬停放大和点击交互。
pub struct Dock {
    /// Dock 栏位置
    pub position: DockPosition,
    /// 项目列表
    pub items: Vec<DockItem>,
    /// 图标大小
    pub icon_size: u32,
    /// 图标间距
    pub spacing: u32,
    /// 放大倍率
    pub magnification: f32,
    /// 当前悬停的项目索引
    pub hovered_index: Option<usize>,
}

impl Dock {
    /// 创建新的 Dock 栏
    pub fn new(position: DockPosition) -> Self {
        Dock {
            position,
            items: Vec::new(),
            icon_size: 48,
            spacing: 8,
            magnification: 1.5,
            hovered_index: None,
        }
    }

    /// 添加项目
    pub fn add_item(&mut self, item: DockItem) -> Result<(), ShellError> {
        // 检查是否已存在相同 ID
        if self.items.iter().any(|i| i.id == item.id) {
            return Err(ShellError::DockError(format!("Dock 项目已存在: {:?}", item.id)));
        }
        self.items.push(item);
        Ok(())
    }

    /// 移除项目
    pub fn remove_item(&mut self, id: DockItemId) -> Result<(), ShellError> {
        let len_before = self.items.len();
        self.items.retain(|i| i.id != id);
        if self.items.len() < len_before {
            // 如果移除的是悬停项，清除悬停状态
            if let Some(idx) = self.hovered_index {
                if idx >= self.items.len() {
                    self.hovered_index = None;
                }
            }
            Ok(())
        } else {
            Err(ShellError::DockError(format!("Dock 项目未找到: {:?}", id)))
        }
    }

    /// 更新项目状态
    pub fn update_state(&mut self, id: DockItemId, state: DockItemState) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.state = state;
        }
    }

    /// 处理悬停事件
    ///
    /// 返回受影响的项目索引和对应的放大倍率列表。
    pub fn handle_hover(&mut self, x: f32, y: f32, dock_origin: (f32, f32)) -> Vec<(usize, f32)> {
        let mut result = Vec::new();
        let item_size = self.icon_size as f32 + self.spacing as f32;

        for (i, _item) in self.items.iter().enumerate() {
            let item_start = match self.position {
                DockPosition::Bottom => {
                    dock_origin.0 + i as f32 * item_size
                }
                DockPosition::Left | DockPosition::Right => {
                    dock_origin.1 + i as f32 * item_size
                }
            };

            let item_end = item_start + self.icon_size as f32;

            let is_hovered = match self.position {
                DockPosition::Bottom => {
                    x >= item_start && x <= item_end
                }
                DockPosition::Left | DockPosition::Right => {
                    y >= item_start && y <= item_end
                }
            };

            if is_hovered {
                self.hovered_index = Some(i);
                result.push((i, self.magnification));
            } else {
                result.push((i, 1.0));
            }
        }

        if result.iter().all(|(_, scale)| *scale == 1.0) {
            self.hovered_index = None;
        }

        result
    }

    /// 处理点击事件
    ///
    /// 返回被点击的项目 ID（如果有）。
    pub fn handle_click(&mut self, x: f32, y: f32, dock_origin: (f32, f32)) -> Option<DockItemId> {
        let item_size = self.icon_size as f32 + self.spacing as f32;

        for (i, item) in self.items.iter().enumerate() {
            let item_start = match self.position {
                DockPosition::Bottom => {
                    dock_origin.0 + i as f32 * item_size
                }
                DockPosition::Left | DockPosition::Right => {
                    dock_origin.1 + i as f32 * item_size
                }
            };

            let item_end = item_start + self.icon_size as f32;

            let is_clicked = match self.position {
                DockPosition::Bottom => {
                    x >= item_start && x <= item_end
                }
                DockPosition::Left | DockPosition::Right => {
                    y >= item_start && y <= item_end
                }
            };

            if is_clicked {
                return Some(item.id);
            }
        }

        None
    }

    /// 获取项目数量
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 计算总宽度（像素）
    pub fn total_width(&self) -> u32 {
        if self.items.is_empty() {
            return 0;
        }
        self.items.len() as u32 * self.icon_size
            + (self.items.len() as u32 - 1) * self.spacing
    }

    /// 计算 Dock 栏在屏幕上的边界 (x, y, width, height)
    pub fn bounds(&self, screen_size: (u32, u32)) -> (i32, i32, u32, u32) {
        let total_w = self.total_width();
        let dock_height = self.icon_size + 16; // 上下各 8px 内边距

        match self.position {
            DockPosition::Bottom => {
                let x = ((screen_size.0 as i32 - total_w as i32) / 2).max(0);
                let y = screen_size.1 as i32 - dock_height as i32;
                (x, y, total_w, dock_height)
            }
            DockPosition::Left => {
                let x = 0;
                let y = ((screen_size.1 as i32 - total_w as i32) / 2).max(0);
                (x, y, dock_height, total_w)
            }
            DockPosition::Right => {
                let x = screen_size.0 as i32 - dock_height as i32;
                let y = ((screen_size.1 as i32 - total_w as i32) / 2).max(0);
                (x, y, dock_height, total_w)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(id: u64, name: &str) -> DockItem {
        DockItem {
            id: DockItemId(id),
            app_name: name.to_string(),
            state: DockItemState::Normal,
            icon_index: 0,
        }
    }

    #[test]
    fn test_dock_new() {
        let dock = Dock::new(DockPosition::Bottom);
        assert_eq!(dock.position, DockPosition::Bottom);
        assert_eq!(dock.icon_size, 48);
        assert_eq!(dock.spacing, 8);
        assert!(dock.items.is_empty());
        assert!(dock.hovered_index.is_none());
    }

    #[test]
    fn test_dock_add_remove() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();
        dock.add_item(make_item(2, "Terminal")).unwrap();
        assert_eq!(dock.item_count(), 2);

        dock.remove_item(DockItemId(1)).unwrap();
        assert_eq!(dock.item_count(), 1);
        assert_eq!(dock.items[0].app_name, "Terminal");
    }

    #[test]
    fn test_dock_add_duplicate() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();
        let result = dock.add_item(make_item(1, "Finder"));
        assert!(result.is_err());
    }

    #[test]
    fn test_dock_remove_nonexistent() {
        let mut dock = Dock::new(DockPosition::Bottom);
        let result = dock.remove_item(DockItemId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_dock_update_state() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();
        dock.update_state(DockItemId(1), DockItemState::Running { pid: 1234 });
        assert_eq!(dock.items[0].state, DockItemState::Running { pid: 1234 });
    }

    #[test]
    fn test_dock_hover() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();
        dock.add_item(make_item(2, "Terminal")).unwrap();

        // 悬停在第一个项目上
        let result = dock.handle_hover(30.0, 0.0, (0.0, 0.0));
        assert_eq!(dock.hovered_index, Some(0));
        assert_eq!(result[0].1, dock.magnification);
        assert_eq!(result[1].1, 1.0);
    }

    #[test]
    fn test_dock_hover_none() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();

        // 悬停在项目之外
        let result = dock.handle_hover(200.0, 0.0, (0.0, 0.0));
        assert!(dock.hovered_index.is_none());
        assert_eq!(result[0].1, 1.0);
    }

    #[test]
    fn test_dock_click() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();
        dock.add_item(make_item(2, "Terminal")).unwrap();

        // 点击第二个项目
        let clicked = dock.handle_click(56.0, 0.0, (0.0, 0.0));
        assert_eq!(clicked, Some(DockItemId(2)));
    }

    #[test]
    fn test_dock_click_miss() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();

        let clicked = dock.handle_click(200.0, 0.0, (0.0, 0.0));
        assert!(clicked.is_none());
    }

    #[test]
    fn test_dock_bounds_bottom() {
        let mut dock = Dock::new(DockPosition::Bottom);
        dock.add_item(make_item(1, "Finder")).unwrap();
        dock.add_item(make_item(2, "Terminal")).unwrap();

        let (_x, y, w, h) = dock.bounds((1920, 1080));
        assert_eq!(w, 2 * 48 + 1 * 8); // 2 个图标 + 1 个间距
        assert_eq!(h, 48 + 16);
        assert!(y > 0); // 应在屏幕底部
    }

    #[test]
    fn test_dock_total_width() {
        let mut dock = Dock::new(DockPosition::Bottom);
        assert_eq!(dock.total_width(), 0);
        dock.add_item(make_item(1, "Finder")).unwrap();
        assert_eq!(dock.total_width(), 48);
        dock.add_item(make_item(2, "Terminal")).unwrap();
        assert_eq!(dock.total_width(), 48 * 2 + 8);
    }
}
