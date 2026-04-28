//! Shell 错误类型

use crate::window::WindowId;
use crate::window::WindowState;

/// Shell 错误枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellError {
    /// 窗口未找到
    WindowNotFound(WindowId),
    /// 无效的窗口状态
    InvalidWindowState(WindowState),
    /// 无效的边界
    InvalidBounds(String),
    /// 组件未找到
    ComponentNotFound(String),
    /// 事件未被处理
    EventNotHandled,
    /// Shell 未初始化
    ShellNotInitialized,
    /// Dock 错误
    DockError(String),
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellError::WindowNotFound(id) => write!(f, "窗口未找到: {}", id),
            ShellError::InvalidWindowState(state) => write!(f, "无效的窗口状态: {}", state),
            ShellError::InvalidBounds(s) => write!(f, "无效的边界: {}", s),
            ShellError::ComponentNotFound(s) => write!(f, "组件未找到: {}", s),
            ShellError::EventNotHandled => write!(f, "事件未被处理"),
            ShellError::ShellNotInitialized => write!(f, "Shell 未初始化"),
            ShellError::DockError(s) => write!(f, "Dock 错误: {}", s),
        }
    }
}

impl std::error::Error for ShellError {}
