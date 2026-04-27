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
}
