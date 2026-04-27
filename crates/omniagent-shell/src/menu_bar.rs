//! 菜单栏组件
//!
//! 实现桌面环境顶部的菜单栏，支持下拉菜单、菜单项交互和快捷键显示。

/// 菜单项标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MenuItemId(pub u64);

/// 菜单项类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItemKind {
    /// 普通菜单项
    Normal,
    /// 分隔线
    Separator,
    /// 复选框
    Checkbox { checked: bool },
    /// 单选按钮
    Radio { selected: bool, group: String },
}

/// 菜单项
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// 菜单项 ID
    pub id: MenuItemId,
    /// 显示标签
    pub label: String,
    /// 菜单项类型
    pub kind: MenuItemKind,
    /// 是否启用
    pub enabled: bool,
    /// 快捷键标签
    pub shortcut_label: Option<String>,
    /// 子菜单项
    pub children: Vec<MenuItem>,
}

/// 菜单栏菜单（下拉菜单）
#[derive(Debug, Clone)]
pub struct MenuBarMenu {
    /// 菜单标题
    pub title: String,
    /// 菜单项列表
    pub items: Vec<MenuItem>,
}

/// 菜单栏
///
/// 桌面环境顶部的菜单栏，包含多个下拉菜单。
pub struct MenuBar {
    /// 菜单列表
    pub menus: Vec<MenuBarMenu>,
    /// 菜单栏高度
    pub height: u32,
    /// 当前激活的菜单索引
    pub active_menu: Option<usize>,
}

impl MenuBar {
    /// 创建新的菜单栏
    pub fn new() -> Self {
        MenuBar {
            menus: Vec::new(),
            height: 28,
            active_menu: None,
        }
    }

    /// 添加菜单
    pub fn add_menu(&mut self, menu: MenuBarMenu) {
        self.menus.push(menu);
    }

    /// 处理点击事件
    ///
    /// 根据点击的 x 坐标确定是否点击了某个菜单标题。
    pub fn handle_click(&mut self, x: f32) -> MenuAction {
        // 假设每个菜单标题宽度为 80 像素
        let menu_width = 80.0;
        let mut offset = 0.0;

        for (i, menu) in self.menus.iter().enumerate() {
            let title_width = (menu.title.len() as f32 * 10.0 + 20.0).max(menu_width);
            if x >= offset && x < offset + title_width {
                if self.active_menu == Some(i) {
                    // 再次点击同一菜单，关闭
                    self.active_menu = None;
                    return MenuAction::None;
                }
                self.active_menu = Some(i);
                return MenuAction::SubmenuOpened(i);
            }
            offset += title_width;
        }

        // 点击菜单栏空白区域，关闭菜单
        self.active_menu = None;
        MenuAction::None
    }

    /// 处理悬停事件
    ///
    /// 当菜单已打开时，悬停到其他菜单标题会切换激活菜单。
    pub fn handle_hover(&mut self, x: f32) -> bool {
        if self.active_menu.is_none() {
            return false;
        }

        let menu_width = 80.0;
        let mut offset = 0.0;

        for (i, menu) in self.menus.iter().enumerate() {
            let title_width = (menu.title.len() as f32 * 10.0 + 20.0).max(menu_width);
            if x >= offset && x < offset + title_width {
                if self.active_menu != Some(i) {
                    self.active_menu = Some(i);
                    return true;
                }
                return false;
            }
            offset += title_width;
        }

        false
    }

    /// 获取菜单数量
    pub fn menu_count(&self) -> usize {
        self.menus.len()
    }

    /// 计算菜单栏总宽度
    pub fn total_width(&self) -> u32 {
        let menu_width = 80.0;
        let mut total = 0.0;
        for menu in &self.menus {
            let title_width = (menu.title.len() as f32 * 10.0 + 20.0).max(menu_width);
            total += title_width;
        }
        total as u32
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

/// 菜单操作结果
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// 菜单项被点击
    ItemClicked(MenuItemId),
    /// 子菜单被打开
    SubmenuOpened(usize),
    /// 无操作
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_menu(title: &str, items: Vec<MenuItem>) -> MenuBarMenu {
        MenuBarMenu {
            title: title.to_string(),
            items,
        }
    }

    fn make_item(id: u64, label: &str) -> MenuItem {
        MenuItem {
            id: MenuItemId(id),
            label: label.to_string(),
            kind: MenuItemKind::Normal,
            enabled: true,
            shortcut_label: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_menubar_new() {
        let bar = MenuBar::new();
        assert_eq!(bar.height, 28);
        assert!(bar.menus.is_empty());
        assert!(bar.active_menu.is_none());
    }

    #[test]
    fn test_menubar_add_menu() {
        let mut bar = MenuBar::new();
        bar.add_menu(make_menu("文件", vec![
            make_item(1, "新建"),
            make_item(2, "打开"),
        ]));
        bar.add_menu(make_menu("编辑", vec![
            make_item(3, "撤销"),
            make_item(4, "重做"),
        ]));
        assert_eq!(bar.menu_count(), 2);
        assert_eq!(bar.menus[0].title, "文件");
        assert_eq!(bar.menus[1].title, "编辑");
    }

    #[test]
    fn test_menubar_click() {
        let mut bar = MenuBar::new();
        bar.add_menu(make_menu("文件", vec![make_item(1, "新建")]));
        bar.add_menu(make_menu("编辑", vec![make_item(2, "撤销")]));

        // 点击第一个菜单
        let action = bar.handle_click(40.0);
        assert_eq!(action, MenuAction::SubmenuOpened(0));
        assert_eq!(bar.active_menu, Some(0));

        // 再次点击同一菜单，关闭
        let action = bar.handle_click(40.0);
        assert_eq!(action, MenuAction::None);
        assert!(bar.active_menu.is_none());
    }

    #[test]
    fn test_menubar_click_switch() {
        let mut bar = MenuBar::new();
        bar.add_menu(make_menu("文件", vec![make_item(1, "新建")]));
        bar.add_menu(make_menu("编辑", vec![make_item(2, "撤销")]));

        bar.handle_click(40.0);
        assert_eq!(bar.active_menu, Some(0));

        // 点击第二个菜单
        let action = bar.handle_click(120.0);
        assert_eq!(action, MenuAction::SubmenuOpened(1));
        assert_eq!(bar.active_menu, Some(1));
    }

    #[test]
    fn test_menubar_click_empty_area() {
        let mut bar = MenuBar::new();
        bar.add_menu(make_menu("文件", vec![make_item(1, "新建")]));
        bar.active_menu = Some(0);

        // 点击空白区域
        let action = bar.handle_click(500.0);
        assert_eq!(action, MenuAction::None);
        assert!(bar.active_menu.is_none());
    }

    #[test]
    fn test_menubar_hover() {
        let mut bar = MenuBar::new();
        bar.add_menu(make_menu("文件", vec![make_item(1, "新建")]));
        bar.add_menu(make_menu("编辑", vec![make_item(2, "撤销")]));

        // 没有激活菜单时悬停不生效
        assert!(!bar.handle_hover(120.0));

        // 激活菜单后悬停
        bar.active_menu = Some(0);
        let switched = bar.handle_hover(120.0);
        assert!(switched);
        assert_eq!(bar.active_menu, Some(1));
    }

    #[test]
    fn test_menubar_hover_same_menu() {
        let mut bar = MenuBar::new();
        bar.add_menu(make_menu("文件", vec![make_item(1, "新建")]));
        bar.active_menu = Some(0);

        // 悬停在已激活的菜单上不切换
        let switched = bar.handle_hover(40.0);
        assert!(!switched);
        assert_eq!(bar.active_menu, Some(0));
    }

    #[test]
    fn test_menubar_total_width() {
        let mut bar = MenuBar::new();
        assert_eq!(bar.total_width(), 0);
        bar.add_menu(make_menu("文件", vec![]));
        // "文件" 2 个字符 * 10 + 20 = 40，但最小 80
        assert_eq!(bar.total_width(), 80);
    }
}
