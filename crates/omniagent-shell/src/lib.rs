//! Aqua Shell 桌面环境框架
//!
//! 本 crate 实现了 OmniAgent OS 的桌面环境核心框架，包括：
//! - 窗口系统（Window, WindowManager）
//! - UI 组件系统（Label, Button, Container）
//! - Shell 核心（AquaShell, Theme, ShellConfig）

mod error;
mod color;
mod rect;
mod window;
mod window_manager;
mod component;
mod shell;

pub use error::ShellError;
pub use color::Color;
pub use rect::Rect;
pub use window::{WindowId, WindowState, WindowType, WindowProperties, Window};
pub use window_manager::WindowManager;
pub use component::{
    ComponentId, UIEvent, MouseButton, KeyCode, Modifiers,
    UIComponent, Label, Button, Container, Layout,
};
pub use shell::{ShellConfig, DockPosition, Theme, AquaShell};
