//! OmniAgent OS 桌面集成层
//!
//! 本 crate 提供桌面环境的桥接层，包括：
//! - 可渲染窗口管理（RenderableWindow, PixelBuffer）
//! - 输入事件路由（InputRouter）
//! - 主题管理（ThemeManager）
//! - 桌面桥接接口（DesktopBridge）

pub mod bridge;
pub mod renderable;
pub mod input;
pub mod theme;
pub mod error;

pub use bridge::{DesktopBridge, DesktopBridgeImpl};
pub use renderable::{
    RenderableWindow, WindowSurface, PixelBuffer, RenderLayer,
    SurfaceUpdate, FrameToken, AffineTransform, WindowId,
};
pub use input::{InputRouter, InputTarget, MouseEvent, MouseEventKind, KeyEvent, KeyState};
pub use theme::{ThemeManager, DesktopTheme, ThemeColors, ThemeFonts, ShadowConfig, AnimationConfig};
pub use error::DesktopError;
