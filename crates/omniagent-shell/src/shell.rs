//! Shell 核心（AquaShell、ShellConfig、Theme）

use crate::color::Color;
use crate::component::{Container, UIComponent, UIEvent};
use crate::window::{WindowId, WindowProperties};
use crate::window_manager::WindowManager;

/// Dock 栏位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DockPosition {
    /// 底部
    Bottom = 0,
    /// 左侧
    Left = 1,
    /// 右侧
    Right = 2,
    /// 顶部
    Top = 3,
}

/// Shell 配置
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// 屏幕宽度
    pub screen_width: u32,
    /// 屏幕高度
    pub screen_height: u32,
    /// 背景颜色
    pub background_color: Color,
    /// 主题
    pub theme: Theme,
    /// 字体名称
    pub font_name: String,
    /// 默认字体大小
    pub font_size: u16,
    /// Dock 栏位置
    pub dock_position: DockPosition,
    /// 是否启用动画
    pub animation_enabled: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            screen_width: 1920,
            screen_height: 1080,
            background_color: Color::rgb(30, 30, 30),
            theme: Theme::default(),
            font_name: "system-ui".to_string(),
            font_size: 14,
            dock_position: DockPosition::Bottom,
            animation_enabled: true,
        }
    }
}

/// 主题定义
///
/// 定义了桌面环境的颜色方案和视觉风格。
#[derive(Debug, Clone)]
pub struct Theme {
    /// 主题名称
    pub name: String,
    /// 主色调
    pub primary: Color,
    /// 次色调
    pub secondary: Color,
    /// 背景色
    pub background: Color,
    /// 表面色
    pub surface: Color,
    /// 文本色
    pub text: Color,
    /// 次要文本色
    pub text_secondary: Color,
    /// 强调色
    pub accent: Color,
    /// 错误色
    pub error: Color,
    /// 警告色
    pub warning: Color,
    /// 成功色
    pub success: Color,
    /// 边框色
    pub border: Color,
    /// 阴影色
    pub shadow: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            name: "Aqua Dark".to_string(),
            primary: Color::rgb(0, 122, 255),
            secondary: Color::rgb(88, 86, 214),
            background: Color::rgb(30, 30, 30),
            surface: Color::rgb(44, 44, 46),
            text: Color::rgb(255, 255, 255),
            text_secondary: Color::rgb(155, 155, 155),
            accent: Color::rgb(0, 199, 190),
            error: Color::rgb(255, 59, 48),
            warning: Color::rgb(255, 204, 0),
            success: Color::rgb(52, 199, 89),
            border: Color::rgb(63, 63, 70),
            shadow: Color::rgba(0, 0, 0, 64),
        }
    }
}

/// Aqua Shell
///
/// 桌面环境的核心实例，整合窗口管理器和 UI 组件系统。
pub struct AquaShell {
    /// Shell 配置
    config: ShellConfig,
    /// 窗口管理器
    window_manager: WindowManager,
    /// 根容器
    root_container: Container,
}

impl AquaShell {
    /// 创建新的 Aqua Shell 实例
    pub fn new(config: ShellConfig) -> Self {
        let wm = WindowManager::new(config.screen_width, config.screen_height);
        let root = Container::new("root");
        AquaShell {
            config,
            window_manager: wm,
            root_container: root,
        }
    }

    /// 创建窗口
    pub fn create_window(&mut self, props: WindowProperties) -> WindowId {
        self.window_manager.create_window(props)
    }

    /// 处理 UI 事件
    pub fn handle_event(&mut self, event: UIEvent) {
        // 将事件传递给根容器
        self.root_container.handle_event(&event);
    }

    /// 获取窗口管理器的不可变引用
    pub fn window_manager(&self) -> &WindowManager {
        &self.window_manager
    }

    /// 获取窗口管理器的可变引用
    pub fn window_manager_mut(&mut self) -> &mut WindowManager {
        &mut self.window_manager
    }

    /// 获取配置的不可变引用
    pub fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// 获取主题的不可变引用
    pub fn theme(&self) -> &Theme {
        &self.config.theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect::Rect;

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Aqua Dark");
        assert_eq!(theme.primary, Color::rgb(0, 122, 255));
        assert_eq!(theme.secondary, Color::rgb(88, 86, 214));
        assert_eq!(theme.background, Color::rgb(30, 30, 30));
        assert_eq!(theme.surface, Color::rgb(44, 44, 46));
        assert_eq!(theme.text, Color::rgb(255, 255, 255));
        assert_eq!(theme.text_secondary, Color::rgb(155, 155, 155));
        assert_eq!(theme.accent, Color::rgb(0, 199, 190));
        assert_eq!(theme.error, Color::rgb(255, 59, 48));
        assert_eq!(theme.warning, Color::rgb(255, 204, 0));
        assert_eq!(theme.success, Color::rgb(52, 199, 89));
        assert_eq!(theme.border, Color::rgb(63, 63, 70));
        assert_eq!(theme.shadow, Color::rgba(0, 0, 0, 64));
    }

    #[test]
    fn test_shell_config_default() {
        let config = ShellConfig::default();
        assert_eq!(config.screen_width, 1920);
        assert_eq!(config.screen_height, 1080);
        assert_eq!(config.dock_position, DockPosition::Bottom);
        assert!(config.animation_enabled);
        assert_eq!(config.font_name, "system-ui");
        assert_eq!(config.font_size, 14);
    }

    #[test]
    fn test_aqua_shell_new() {
        let shell = AquaShell::new(ShellConfig::default());
        assert_eq!(shell.config().screen_width, 1920);
        assert_eq!(shell.config().screen_height, 1080);
        assert_eq!(shell.window_manager().window_count(), 0);
    }

    #[test]
    fn test_aqua_shell_create_window() {
        let mut shell = AquaShell::new(ShellConfig::default());
        let id = shell.create_window(WindowProperties::default());
        assert_eq!(id, WindowId(1));
        assert_eq!(shell.window_manager().window_count(), 1);
    }

    #[test]
    fn test_aqua_shell_handle_event() {
        let mut shell = AquaShell::new(ShellConfig::default());
        // 处理事件不应 panic
        shell.handle_event(UIEvent::Click { x: 0, y: 0, button: crate::component::MouseButton::Left });
        shell.handle_event(UIEvent::KeyPress {
            key: crate::component::KeyCode(65),
            modifiers: crate::component::Modifiers(0),
        });
    }

    #[test]
    fn test_aqua_shell_theme() {
        let shell = AquaShell::new(ShellConfig::default());
        let theme = shell.theme();
        assert_eq!(theme.name, "Aqua Dark");
    }

    #[test]
    fn test_aqua_shell_custom_config() {
        let config = ShellConfig {
            screen_width: 1280,
            screen_height: 720,
            dock_position: DockPosition::Left,
            animation_enabled: false,
            ..ShellConfig::default()
        };
        let shell = AquaShell::new(config);
        assert_eq!(shell.config().screen_width, 1280);
        assert_eq!(shell.config().screen_height, 720);
        assert_eq!(shell.config().dock_position, DockPosition::Left);
        assert!(!shell.config().animation_enabled);
    }

    #[test]
    fn test_aqua_shell_window_manager_access() {
        let mut shell = AquaShell::new(ShellConfig::default());
        let id = shell.create_window(WindowProperties {
            bounds: Rect::new(10, 10, 200, 200),
            ..WindowProperties::default()
        });

        // 通过 window_manager 访问
        assert!(shell.window_manager().get_window(id).is_some());

        // 通过 window_manager_mut 修改
        shell.window_manager_mut().move_window(id, 50, 50).unwrap();
        assert_eq!(shell.window_manager().get_window(id).unwrap().properties.bounds.x, 50);
    }
}
