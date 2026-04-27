//! 主题管理系统
//!
//! 提供桌面环境的主题定义、颜色管理和主题切换功能。

use crate::error::DesktopError;

/// 颜色（ARGB8888 格式）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Color(pub u32);

impl Color {
    /// 从 ARGB 分量创建颜色
    pub fn from_argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    /// 从 RGB 分量创建颜色（完全不透明）
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Color::from_argb(0xFF, r, g, b)
    }

    /// 获取 Alpha 通道
    pub fn alpha(&self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }

    /// 获取红色通道
    pub fn red(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// 获取绿色通道
    pub fn green(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// 获取蓝色通道
    pub fn blue(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// 线性插值两个颜色
    /// t 为 0.0 时返回 self，t 为 1.0 时返回 other
    pub fn lerp(&self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let a = (self.alpha() as f32 + (other.alpha() as f32 - self.alpha() as f32) * t) as u8;
        let r = (self.red() as f32 + (other.red() as f32 - self.red() as f32) * t) as u8;
        let g = (self.green() as f32 + (other.green() as f32 - self.green() as f32) * t) as u8;
        let b = (self.blue() as f32 + (other.blue() as f32 - self.blue() as f32) * t) as u8;
        Color::from_argb(a, r, g, b)
    }
}

/// 主题颜色配置
#[derive(Debug, Clone)]
pub struct ThemeColors {
    /// 背景色
    pub background: Color,
    /// 前景色
    pub foreground: Color,
    /// 强调色
    pub accent: Color,
    /// 表面色
    pub surface: Color,
    /// 边框色
    pub border: Color,
    /// 错误色
    pub error: Color,
    /// 主要文本色
    pub text_primary: Color,
    /// 次要文本色
    pub text_secondary: Color,
}

/// 主题字体配置
#[derive(Debug, Clone)]
pub struct ThemeFonts {
    /// 字体族名称
    pub family: String,
    /// 默认字号
    pub default_size: f32,
    /// 标题字号
    pub title_size: f32,
}

/// 阴影配置
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// 阴影颜色
    pub color: Color,
    /// 模糊半径
    pub blur_radius: f32,
    /// X 偏移
    pub offset_x: f32,
    /// Y 偏移
    pub offset_y: f32,
}

/// 动画配置
#[derive(Debug, Clone)]
pub struct AnimationConfig {
    /// 动画持续时间（毫秒）
    pub duration_ms: u32,
    /// 缓动函数名称
    pub easing: String,
}

/// 桌面主题
#[derive(Debug, Clone)]
pub struct DesktopTheme {
    /// 主题名称
    pub name: String,
    /// 颜色配置
    pub colors: ThemeColors,
    /// 圆角半径
    pub corner_radius: f32,
    /// 默认字体大小
    pub font_size: f32,
}

impl Default for DesktopTheme {
    fn default() -> Self {
        Self {
            name: "Aqua Dark".into(),
            colors: ThemeColors {
                background: Color::from_argb(0xFF, 0x1E, 0x1E, 0x2E),
                foreground: Color::from_rgb(0xFF, 0xFF, 0xFF),
                accent: Color::from_rgb(0x00, 0x7A, 0xFF),
                surface: Color::from_argb(0xFF, 0x2A, 0x2A, 0x3C),
                border: Color::from_argb(0xFF, 0x44, 0x44, 0x5A),
                error: Color::from_rgb(0xFF, 0x44, 0x44),
                text_primary: Color::from_rgb(0xFF, 0xFF, 0xFF),
                text_secondary: Color::from_argb(0xFF, 0xAA, 0xAA, 0xBB),
            },
            corner_radius: 12.0,
            font_size: 14.0,
        }
    }
}

/// 主题管理器
///
/// 管理多个桌面主题，支持切换和查询。
pub struct ThemeManager {
    /// 当前激活的主题
    current: DesktopTheme,
    /// 所有可用主题
    themes: Vec<DesktopTheme>,
}

impl ThemeManager {
    /// 创建新的主题管理器，使用默认主题
    pub fn new() -> Self {
        let default = DesktopTheme::default();
        ThemeManager {
            current: default.clone(),
            themes: vec![default],
        }
    }

    /// 获取当前主题的不可变引用
    pub fn current(&self) -> &DesktopTheme {
        &self.current
    }

    /// 按名称切换主题
    pub fn set_theme(&mut self, name: &str) -> Result<(), DesktopError> {
        if let Some(theme) = self.themes.iter().find(|t| t.name == name) {
            self.current = theme.clone();
            Ok(())
        } else {
            Err(DesktopError::ThemeNotFound(name.to_string()))
        }
    }

    /// 添加新主题
    pub fn add_theme(&mut self, theme: DesktopTheme) {
        // 如果已存在同名主题，先移除
        self.themes.retain(|t| t.name != theme.name);
        self.themes.push(theme);
    }

    /// 列出所有可用主题名称
    pub fn list_themes(&self) -> Vec<&str> {
        self.themes.iter().map(|t| t.name.as_str()).collect()
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_argb() {
        let c = Color::from_argb(0xAB, 0x12, 0x34, 0x56);
        assert_eq!(c.alpha(), 0xAB);
        assert_eq!(c.red(), 0x12);
        assert_eq!(c.green(), 0x34);
        assert_eq!(c.blue(), 0x56);
    }

    #[test]
    fn test_color_from_rgb() {
        let c = Color::from_rgb(0x10, 0x20, 0x30);
        assert_eq!(c.alpha(), 0xFF);
        assert_eq!(c.red(), 0x10);
        assert_eq!(c.green(), 0x20);
        assert_eq!(c.blue(), 0x30);
    }

    #[test]
    fn test_color_lerp_t0() {
        let a = Color::from_rgb(0, 0, 0);
        let b = Color::from_rgb(100, 200, 255);
        let result = a.lerp(b, 0.0);
        assert_eq!(result, a);
    }

    #[test]
    fn test_color_lerp_t1() {
        let a = Color::from_rgb(0, 0, 0);
        let b = Color::from_rgb(100, 200, 255);
        let result = a.lerp(b, 1.0);
        assert_eq!(result, b);
    }

    #[test]
    fn test_color_lerp_mid() {
        let a = Color::from_argb(0, 0, 0, 0);
        let b = Color::from_argb(200, 100, 200, 255);
        let result = a.lerp(b, 0.5);
        assert_eq!(result.alpha(), 100);
        assert_eq!(result.red(), 50);
        assert_eq!(result.green(), 100);
        assert_eq!(result.blue(), 127); // 255/2 = 127.5 截断为 127
    }

    #[test]
    fn test_color_lerp_clamp() {
        let a = Color::from_rgb(0, 0, 0);
        let b = Color::from_rgb(100, 100, 100);
        // t 超出范围应被钳制
        assert_eq!(a.lerp(b, 2.0), b);
        assert_eq!(a.lerp(b, -1.0), a);
    }

    #[test]
    fn test_theme_default() {
        let theme = DesktopTheme::default();
        assert_eq!(theme.name, "Aqua Dark");
        assert!((theme.corner_radius - 12.0).abs() < f32::EPSILON);
        assert!((theme.font_size - 14.0).abs() < f32::EPSILON);
        // 验证背景色
        assert_eq!(theme.colors.background, Color::from_argb(0xFF, 0x1E, 0x1E, 0x2E));
        // 验证强调色
        assert_eq!(theme.colors.accent, Color::from_rgb(0x00, 0x7A, 0xFF));
    }

    #[test]
    fn test_theme_manager_new() {
        let mgr = ThemeManager::new();
        assert_eq!(mgr.current().name, "Aqua Dark");
        assert_eq!(mgr.list_themes().len(), 1);
    }

    #[test]
    fn test_theme_manager_add_theme() {
        let mut mgr = ThemeManager::new();
        let light = DesktopTheme {
            name: "Aqua Light".into(),
            colors: ThemeColors {
                background: Color::from_rgb(0xF0, 0xF0, 0xF0),
                foreground: Color::from_rgb(0, 0, 0),
                accent: Color::from_rgb(0x00, 0x7A, 0xFF),
                surface: Color::from_rgb(0xFF, 0xFF, 0xFF),
                border: Color::from_rgb(0xCC, 0xCC, 0xCC),
                error: Color::from_rgb(0xFF, 0x00, 0x00),
                text_primary: Color::from_rgb(0, 0, 0),
                text_secondary: Color::from_rgb(0x66, 0x66, 0x66),
            },
            corner_radius: 8.0,
            font_size: 13.0,
        };
        mgr.add_theme(light);
        assert_eq!(mgr.list_themes().len(), 2);
        assert!(mgr.list_themes().contains(&"Aqua Light"));
    }

    #[test]
    fn test_theme_manager_set_theme() {
        let mut mgr = ThemeManager::new();
        let light = DesktopTheme {
            name: "Aqua Light".into(),
            ..DesktopTheme::default()
        };
        mgr.add_theme(light);
        mgr.set_theme("Aqua Light").unwrap();
        assert_eq!(mgr.current().name, "Aqua Light");
    }

    #[test]
    fn test_theme_manager_set_theme_not_found() {
        let mut mgr = ThemeManager::new();
        let result = mgr.set_theme("NonExistent");
        assert!(matches!(result, Err(DesktopError::ThemeNotFound(_))));
    }

    #[test]
    fn test_theme_manager_replace_theme() {
        let mut mgr = ThemeManager::new();
        // 添加同名主题应替换
        let new_dark = DesktopTheme {
            name: "Aqua Dark".into(),
            corner_radius: 20.0,
            ..DesktopTheme::default()
        };
        mgr.add_theme(new_dark);
        assert_eq!(mgr.list_themes().len(), 1);
        assert!((mgr.current().corner_radius - 12.0).abs() < f32::EPSILON); // 当前未切换
        mgr.set_theme("Aqua Dark").unwrap();
        assert!((mgr.current().corner_radius - 20.0).abs() < f32::EPSILON);
    }
}
