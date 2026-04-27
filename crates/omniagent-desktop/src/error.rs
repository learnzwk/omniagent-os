//! 桌面错误类型

/// 桌面层错误枚举
#[derive(Debug, Clone)]
pub enum DesktopError {
    /// 窗口未找到
    WindowNotFound(u64),
    /// 无效的窗口 ID
    InvalidWindowId,
    /// 渲染失败
    RenderFailed { reason: &'static str },
    /// 动画错误
    AnimationError { reason: &'static str },
    /// 主题未找到
    ThemeNotFound(String),
    /// 无效的渲染表面
    InvalidSurface,
    /// 缓冲区太小
    BufferTooSmall,
}

impl std::fmt::Display for DesktopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DesktopError::WindowNotFound(id) => write!(f, "窗口未找到: {}", id),
            DesktopError::InvalidWindowId => write!(f, "无效的窗口 ID"),
            DesktopError::RenderFailed { reason } => write!(f, "渲染失败: {}", reason),
            DesktopError::AnimationError { reason } => write!(f, "动画错误: {}", reason),
            DesktopError::ThemeNotFound(name) => write!(f, "主题未找到: {}", name),
            DesktopError::InvalidSurface => write!(f, "无效的渲染表面"),
            DesktopError::BufferTooSmall => write!(f, "缓冲区太小"),
        }
    }
}

impl std::error::Error for DesktopError {}
