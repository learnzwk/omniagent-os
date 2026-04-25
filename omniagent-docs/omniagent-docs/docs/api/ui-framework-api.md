# UI 框架 API 参考

> **模块名称**: `ui-framework-api`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

UI 框架 API 提供 OmniAgent OS 中 Aqua Shell 桌面环境的编程接口。涵盖窗口管理、布局控制、Dock 栏、菜单栏、Agent Bar、Spotlight、通知、主题、手势和渲染等所有桌面组件。此 API 供应用开发者构建原生桌面应用，并支持 Agent 驱动的动态界面生成。

### 1.2 架构概览

```
┌──────────────────────────────────────────────────────────┐
│                   UI Framework API                       │
├──────────┬──────────┬──────────┬────────────────────────┤
│ Window   │ Layout   │  Dock    │  Menu Bar              │
│ Manager  │ Engine   │          │                        │
├──────────┼──────────┼──────────┼────────────────────────┤
│ Agent    │Spotlight │ Notif.   │  Theme Engine          │
│ Bar      │          │ Center   │                        │
├──────────┼──────────┼──────────┼────────────────────────┤
│ Gesture  │Rendering │ Event    │  Accessibility         │
│ Handler  │ Pipeline │ System   │                        │
├──────────┴──────────┴──────────┴────────────────────────┤
│              Aqua Shell Compositor (Vulkan)              │
└──────────────────────────────────────────────────────────┘
```

---

## 2. 窗口管理 API

### 2.1 窗口创建与控制

```rust
use std::collections::HashMap;
use std::time::Duration;

/// 窗口标识符
pub type WindowId = u64;

/// 窗口属性
#[derive(Debug, Clone)]
pub struct WindowAttributes {
    /// 窗口标题
    pub title: String,
    /// 应用 ID
    pub app_id: String,
    /// 初始位置
    pub position: (i32, i32),
    /// 初始大小
    pub size: (u32, u32),
    /// 最小大小
    pub min_size: Option<(u32, u32)>,
    /// 最大大小
    pub max_size: Option<(u32, u32)>,
    /// 是否可调整大小
    pub resizable: bool,
    /// 窗口类型
    pub window_type: WindowType,
    /// 是否显示标题栏
    pub decorated: bool,
    /// 是否始终在最前
    pub always_on_top: bool,
    /// 初始不透明度
    pub opacity: f32,
    /// 窗口图标
    pub icon: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    Application,
    Dialog,
    Tooltip,
    Menu,
    Dock,
    Notification,
    AgentPanel,
    Splash,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: String::new(),
            app_id: String::new(),
            position: (100, 100),
            size: (800, 600),
            min_size: None,
            max_size: None,
            resizable: true,
            window_type: WindowType::Application,
            decorated: true,
            always_on_top: false,
            opacity: 1.0,
            icon: None,
        }
    }
}

/// 几何提示
#[derive(Debug, Clone, Copy)]
pub struct GeometryHints {
    pub min_width: u32,
    pub min_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub base_width: u32,
    pub base_height: u32,
    pub width_increment: u32,
    pub height_increment: u32,
    pub min_aspect_ratio: Option<(u32, u32)>,
    pub max_aspect_ratio: Option<(u32, u32)>,
}

/// 窗口事件
#[derive(Debug, Clone)]
pub enum WindowEvent {
    Created(WindowId),
    Destroyed(WindowId),
    Focused(WindowId),
    Unfocused(WindowId),
    Resized(WindowId, (u32, u32)),
    Moved(WindowId, (i32, i32)),
    Minimized(WindowId),
    Maximized(WindowId),
    Fullscreened(WindowId),
    Restored(WindowId),
    Closed(WindowId),
    StateChanged(WindowId, WindowState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

/// 窗口管理 trait
pub trait WindowManager: Send + Sync {
    /// 创建窗口
    fn create_window(&self, attrs: WindowAttributes) -> Result<WindowId, UiError>;

    /// 设置窗口标题
    fn set_title(&self, window_id: WindowId, title: &str) -> Result<(), UiError>;

    /// 调整窗口大小
    fn resize(&self, window_id: WindowId, width: u32, height: u32) -> Result<(), UiError>;

    /// 移动窗口
    fn move_window(&self, window_id: WindowId, x: i32, y: i32) -> Result<(), UiError>;

    /// 关闭窗口
    fn close(&self, window_id: WindowId) -> Result<(), UiError>;

    /// 设置几何提示
    fn set_geometry_hints(&self, window_id: WindowId, hints: GeometryHints) -> Result<(), UiError>;

    /// 设置窗口不透明度
    fn set_opacity(&self, window_id: WindowId, opacity: f32) -> Result<(), UiError>;

    /// 设置窗口图标
    fn set_icon(&self, window_id: WindowId, icon_data: &[u8]) -> Result<(), UiError>;

    /// 聚焦窗口
    fn focus(&self, window_id: WindowId) -> Result<(), UiError>;

    /// 最小化窗口
    fn minimize(&self, window_id: WindowId) -> Result<(), UiError>;

    /// 最大化窗口
    fn maximize(&self, window_id: WindowId) -> Result<(), UiError>;

    /// 获取窗口属性
    fn get_attributes(&self, window_id: WindowId) -> Result<WindowAttributes, UiError>;

    /// 注册窗口事件回调
    fn on_window_event(&self, callback: Box<dyn Fn(WindowEvent) + Send + Sync>);
}
```

---

## 3. 布局 API

### 3.1 布局控制

```rust
/// 布局模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Floating,
    Tiled,
    Split,
}

/// 分屏方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// 布局管理 trait
pub trait LayoutEngine: Send + Sync {
    /// 设置布局模式
    fn set_layout(&self, mode: LayoutMode) -> Result<(), UiError>;

    /// 设置分屏比例
    fn set_split_ratio(&self, ratio: f32) -> Result<(), UiError>;

    /// 设置分屏方向
    fn set_split_direction(&self, direction: SplitDirection) -> Result<(), UiError>;

    /// 切换全屏
    fn toggle_fullscreen(&self, window_id: WindowId) -> Result<(), UiError>;

    /// 窗口吸附到指定区域
    fn snap_to_zone(&self, window_id: WindowId, zone: SnapZone) -> Result<(), UiError>;

    /// 获取当前布局模式
    fn get_layout_mode(&self) -> LayoutMode;

    /// 获取分屏比例
    fn get_split_ratio(&self) -> f32;
}

/// 吸附区域
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapZone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}
```

---

## 4. Dock API

### 4.1 Dock 栏管理

```rust
/// Dock 项属性
#[derive(Debug, Clone)]
pub struct DockItemAttributes {
    /// 应用 ID
    pub app_id: String,
    /// 显示标签
    pub label: String,
    /// 图标数据（PNG/SVG）
    pub icon_data: Vec<u8>,
    /// 图标格式
    pub icon_format: IconFormat,
    /// 是否固定
    pub pinned: bool,
    /// 上下文菜单项
    pub context_menu_items: Vec<ContextMenuItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconFormat {
    Png,
    Svg,
    Rgba,
}

/// 上下文菜单项
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub separator_after: bool,
    pub action: ContextAction,
}

#[derive(Debug, Clone)]
pub enum ContextAction {
    Launch,
    Quit,
    NewWindow,
    TogglePin,
    Custom { action_id: String, handler: String },
    Submenu(Vec<ContextMenuItem>),
}

/// Dock 位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPosition {
    Bottom,
    Left,
    Right,
}

/// Dock 管理 trait
pub trait DockManager: Send + Sync {
    /// 添加 Dock 项
    fn add_dock_item(&self, item: DockItemAttributes) -> Result<String, UiError>;

    /// 移除 Dock 项
    fn remove_dock_item(&self, item_id: &str) -> Result<(), UiError>;

    /// 设置 Dock 图标
    fn set_dock_icon(&self, item_id: &str, icon_data: &[u8], format: IconFormat) -> Result<(), UiError>;

    /// 触发弹跳动画
    fn bounce_dock_item(&self, item_id: &str) -> Result<(), UiError>;

    /// 设置 Dock 位置
    fn set_dock_position(&self, position: DockPosition) -> Result<(), UiError>;

    /// 设置 Dock 图标大小
    fn set_icon_size(&self, size: u32) -> Result<(), UiError>;

    /// 设置 Dock 自动隐藏
    fn set_auto_hide(&self, enabled: bool) -> Result<(), UiError>;

    /// 获取所有 Dock 项
    fn get_dock_items(&self) -> Result<Vec<DockItemAttributes>, UiError>;
}
```

---

## 5. 菜单栏 API

### 5.1 全局菜单栏

```rust
/// 菜单项
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
    pub icon: Option<String>,
    pub enabled: bool,
    pub checked: bool,
    pub submenu: Option<Vec<MenuItem>>,
    pub action: Option<MenuAction>,
}

#[derive(Debug, Clone)]
pub enum MenuAction {
    /// 回调函数 ID
    Callback(String),
    /// 系统动作
    System(SystemAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemAction {
    About,
    Preferences,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    CloseWindow,
    Minimize,
    Zoom,
    Fullscreen,
}

/// 菜单栏管理 trait
pub trait MenuBarManager: Send + Sync {
    /// 设置应用菜单
    fn set_app_menu(&self, app_id: &str, menu_items: Vec<MenuItem>) -> Result<(), UiError>;

    /// 添加菜单项
    fn add_menu_item(&self, app_id: &str, menu_id: &str, item: MenuItem) -> Result<(), UiError>;

    /// 显示下拉菜单
    fn show_dropdown(&self, menu_id: &str, position: (i32, i32)) -> Result<(), UiError>;

    /// 隐藏下拉菜单
    fn hide_dropdown(&self) -> Result<(), UiError>;

    /// 更新菜单项状态
    fn update_menu_item(&self, app_id: &str, item_id: &str, enabled: Option<bool>, checked: Option<bool>) -> Result<(), UiError>;

    /// 添加系统托盘项
    fn add_tray_item(&self, icon_data: &[u8], tooltip: &str, menu_items: Vec<MenuItem>) -> Result<String, UiError>;

    /// 移除系统托盘项
    fn remove_tray_item(&self, tray_id: &str) -> Result<(), UiError>;
}
```

---

## 6. Agent Bar API

### 6.1 Agent 交互栏

```rust
/// Agent Bar 配置
#[derive(Debug, Clone)]
pub struct AgentBarConfig {
    /// 初始位置
    pub position: AgentBarPosition,
    /// 宽度
    pub width: u32,
    /// 最大高度
    pub max_height: u32,
    /// 占位符文本
    pub placeholder: String,
    /// 是否支持多行输入
    pub multiline: bool,
    /// 主题
    pub theme: AgentBarTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentBarPosition {
    Bottom,
    Right,
    Floating,
}

#[derive(Debug, Clone)]
pub struct AgentBarTheme {
    pub background_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub border_radius: f32,
    pub blur_amount: f32,
}

impl Default for AgentBarConfig {
    fn default() -> Self {
        Self {
            position: AgentBarPosition::Bottom,
            width: 600,
            max_height: 400,
            placeholder: "Ask Agent...".to_string(),
            multiline: false,
            theme: AgentBarTheme {
                background_color: "#1e1e2e".to_string(),
                text_color: "#cdd6f4".to_string(),
                accent_color: "#89b4fa".to_string(),
                border_radius: 12.0,
                blur_amount: 20.0,
            },
        }
    }
}

/// 流式响应回调
pub type StreamCallback = Box<dyn Fn(StreamChunk) + Send + Sync>;

/// 流式响应片段
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub chunk_id: String,
    pub content: String,
    pub is_final: bool,
    pub agent_id: Option<String>,
}

/// Agent Bar 管理 trait
pub trait AgentBarManager: Send + Sync {
    /// 显示 Agent Bar
    fn show_agent_bar(&self, config: AgentBarConfig) -> Result<(), UiError>;

    /// 隐藏 Agent Bar
    fn hide_agent_bar(&self) -> Result<(), UiError>;

    /// 处理用户输入
    fn process_agent_input(&self, input: &str) -> Result<String, UiError>;

    /// 设置流式响应回调
    fn stream_agent_response(&self, callback: StreamCallback) -> Result<(), UiError>;

    /// 追加流式响应片段
    fn append_stream_chunk(&self, chunk: &str) -> Result<(), UiError>;

    /// 完成流式响应
    fn complete_stream(&self) -> Result<(), UiError>;

    /// 设置激活的 Agent
    fn set_active_agent(&self, agent_id: Option<&str>) -> Result<(), UiError>;

    /// 获取 Agent Bar 可见性
    fn is_visible(&self) -> bool;
}
```

---

## 7. Spotlight API

### 7.1 全局搜索

```rust
/// Spotlight 搜索结果
#[derive(Debug, Clone)]
pub struct SpotlightResult {
    pub result_id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: Option<Vec<u8>>,
    pub category: SpotlightCategory,
    pub score: f64,
    pub action: SpotlightAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotlightCategory {
    Application,
    File,
    AgentAction,
    SystemSetting,
    WebSearch,
    Calculator,
    Bookmark,
    Contact,
}

#[derive(Debug, Clone)]
pub enum SpotlightAction {
    LaunchApp(String),
    OpenFile(String),
    ExecuteAgentAction(String),
    OpenSetting(String),
    CopyToClipboard(String),
    OpenUrl(String),
    Custom { action_id: String, data: String },
}

/// Spotlight 管理 trait
pub trait SpotlightManager: Send + Sync {
    /// 显示 Spotlight
    fn show_spotlight(&self) -> Result<(), UiError>;

    /// 隐藏 Spotlight
    fn hide_spotlight(&self) -> Result<(), UiError>;

    /// 执行搜索
    fn spotlight_search(&self, query: &str) -> Result<Vec<SpotlightResult>, UiError>;

    /// 执行 Spotlight 动作
    fn spotlight_action(&self, action: &SpotlightAction) -> Result<(), UiError>;

    /// 注册搜索提供者
    fn register_provider(&self, provider: Box<dyn SpotlightProvider + Send + Sync>) -> Result<(), UiError>;
}

/// 搜索提供者 trait
pub trait SpotlightProvider: Send + Sync {
    fn search(&self, query: &str, limit: usize) -> Vec<SpotlightResult>;
    fn name(&self) -> &str;
    fn priority(&self) -> u32;
}
```

---

## 8. 通知 API

### 8.1 通知管理

```rust
/// 通知属性
#[derive(Debug, Clone)]
pub struct NotificationAttributes {
    pub title: String,
    pub body: String,
    pub icon: Option<Vec<u8>>,
    pub urgency: NotificationUrgency,
    pub actions: Vec<NotificationAction>,
    pub auto_dismiss: Option<Duration>,
    pub group_id: Option<String>,
    pub app_id: String,
    pub sound: Option<String>,
    pub persistent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub id: String,
    pub label: String,
}

/// 通知事件回调
pub type NotificationCallback = Box<dyn Fn(NotificationEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub enum NotificationEvent {
    Dismissed(String),
    ActionClicked(String, String),
    TimedOut(String),
}

/// 通知管理 trait
pub trait NotificationManager: Send + Sync {
    /// 推送通知
    fn push_notification(&self, attrs: NotificationAttributes) -> Result<String, UiError>;

    /// 关闭通知
    fn dismiss_notification(&self, notification_id: &str) -> Result<(), UiError>;

    /// 分组通知
    fn group_notifications(&self, group_id: &str) -> Result<Vec<String>, UiError>;

    /// 注册通知事件回调
    fn on_notification_event(&self, callback: NotificationCallback) -> Result<(), UiError>;

    /// 获取所有通知
    fn get_notifications(&self) -> Result<Vec<NotificationAttributes>, UiError>;

    /// 清除所有通知
    fn clear_all(&self) -> Result<(), UiError>;
}
```

---

## 9. 主题 API

### 9.1 主题管理

```rust
/// 设计令牌
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTokens {
    pub colors: ColorTokens,
    pub typography: TypographyTokens,
    pub spacing: SpacingTokens,
    pub corner_radius: CornerTokens,
    pub blur: BlurTokens,
    pub animation: AnimationTokens,
    pub shadows: ShadowTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorTokens {
    pub primary: String,
    pub primary_variant: String,
    pub secondary: String,
    pub secondary_variant: String,
    pub background: String,
    pub surface: String,
    pub surface_variant: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub text_disabled: String,
    pub accent: String,
    pub error: String,
    pub warning: String,
    pub success: String,
    pub info: String,
    pub border: String,
    pub divider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyTokens {
    pub font_family: String,
    pub font_family_mono: String,
    pub font_size_xs: f32,
    pub font_size_sm: f32,
    pub font_size_md: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,
    pub font_size_xxl: f32,
    pub font_weight_normal: u32,
    pub font_weight_medium: u32,
    pub font_weight_bold: u32,
    pub line_height: f32,
    pub letter_spacing: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingTokens {
    pub space_xs: f32,
    pub space_sm: f32,
    pub space_md: f32,
    pub space_lg: f32,
    pub space_xl: f32,
    pub space_xxl: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CornerTokens {
    pub none: f32,
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub extra_large: f32,
    pub full: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlurTokens {
    pub none: f32,
    pub light: f32,
    pub medium: f32,
    pub heavy: f32,
    pub extra_heavy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationTokens {
    pub duration_fast: f32,
    pub duration_normal: f32,
    pub duration_slow: f32,
    pub easing_default: String,
    pub easing_decelerate: String,
    pub easing_spring_stiffness: f32,
    pub easing_spring_damping: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTokens {
    pub small: String,
    pub medium: String,
    pub large: String,
    pub extra_large: String,
}

/// 主题管理 trait
pub trait ThemeManager: Send + Sync {
    /// 应用主题
    fn apply_theme(&self, theme_name: &str) -> Result<(), UiError>;

    /// 获取当前设计令牌
    fn get_design_tokens(&self) -> Result<DesignTokens, UiError>;

    /// 设置强调色
    fn set_accent_color(&self, color: &str) -> Result<(), UiError>;

    /// 切换深色/浅色模式
    fn toggle_dark_mode(&self) -> Result<bool, UiError>;

    /// 加载自定义主题
    fn load_custom_theme(&self, theme_data: &str) -> Result<(), UiError>;

    /// 注册主题变更监听器
    fn on_theme_changed(&self, callback: Box<dyn Fn(DesignTokens) + Send + Sync>) -> Result<(), UiError>;
}
```

---

## 10. 手势 API

### 10.1 手势处理

```rust
/// 手势类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureType {
    Swipe,
    Pinch,
    Rotate,
    EdgeSwipe,
    Tap,
    LongPress,
    TwoFingerTap,
}

/// 手势方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureDirection {
    Up,
    Down,
    Left,
    Right,
}

/// 手势事件
#[derive(Debug, Clone)]
pub struct GestureEvent {
    pub gesture_type: GestureType,
    pub direction: Option<GestureDirection>,
    pub finger_count: u32,
    pub delta_x: f32,
    pub delta_y: f32,
    pub scale: f32,
    pub rotation: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub timestamp: std::time::Instant,
    pub phase: GesturePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    Began,
    Changed,
    Ended,
    Cancelled,
}

/// 手势回调类型
type GestureCallback = Box<dyn Fn(GestureEvent) -> GestureResponse + Send + Sync>;

/// 手势响应
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureResponse {
    /// 接受手势，阻止传播
    Accepted,
    /// 忽略手势，继续传播
    Ignored,
}

/// 手势管理 trait
pub trait GestureManager: Send + Sync {
    /// 注册手势处理器
    fn register_gesture_handler(
        &self,
        gesture_type: GestureType,
        callback: GestureCallback,
    ) -> Result<String, UiError>;

    /// 注册滑动回调
    fn on_swipe(&self, direction: GestureDirection, callback: GestureCallback) -> Result<String, UiError>;

    /// 注册捏合回调
    fn on_pinch(&self, callback: GestureCallback) -> Result<String, UiError>;

    /// 注册旋转回调
    fn on_rotate(&self, callback: GestureCallback) -> Result<String, UiError>;

    /// 移除手势处理器
    fn remove_gesture_handler(&self, handler_id: &str) -> Result<(), UiError>;

    /// 设置手势识别阈值
    fn set_threshold(&self, gesture_type: GestureType, threshold: f32) -> Result<(), UiError>;
}
```

---

## 11. 渲染 API

### 11.1 自定义渲染

```rust
/// 帧缓冲区
#[derive(Debug, Clone)]
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8888,
    Bgra8888,
    Rgb565,
    A8,
}

/// 渲染上下文
#[derive(Debug, Clone)]
pub struct RenderContext {
    pub frame_number: u64,
    pub delta_time: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub dpi_scale: f32,
}

/// 渲染命令
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// 绘制矩形
    DrawRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: String,
        corner_radius: f32,
    },
    /// 绘制文本
    DrawText {
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        color: String,
        max_width: Option<f32>,
    },
    /// 绘制图像
    DrawImage {
        image_data: Vec<u8>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        opacity: f32,
    },
    /// 应用模糊效果
    ApplyBlur {
        region: (f32, f32, f32, f32),
        radius: f32,
    },
    /// 裁剪区域
    Clip {
        region: (f32, f32, f32, f32),
        corner_radius: f32,
    },
    /// 变换
    Transform {
        translate_x: f32,
        translate_y: f32,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
    },
}

/// 渲染回调类型
type RenderCallback = Box<dyn Fn(&RenderContext, &mut Vec<RenderCommand>) + Send + Sync>;

/// 渲染管理 trait
pub trait RenderManager: Send + Sync {
    /// 注册渲染回调
    fn register_render_callback(&self, window_id: WindowId, callback: RenderCallback) -> Result<(), UiError>;

    /// 获取帧缓冲区
    fn get_framebuffer(&self, window_id: WindowId) -> Result<Framebuffer, UiError>;

    /// 提交渲染帧
    fn present(&self, window_id: WindowId) -> Result<(), UiError>;

    /// 设置垂直同步
    fn set_vsync(&self, enabled: bool) -> Result<(), UiError>;

    /// 获取当前帧率
    fn get_fps(&self) -> f64;

    /// 设置渲染质量
    fn set_render_quality(&self, quality: RenderQuality) -> Result<(), UiError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderQuality {
    Low,
    Medium,
    High,
    Ultra,
}
```

---

## 12. 错误处理

```rust
/// UI 框架错误类型
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("窗口不存在: {0}")]
    WindowNotFound(WindowId),

    #[error("窗口创建失败: {0}")]
    WindowCreationFailed(String),

    #[error("无效的窗口状态: {0}")]
    InvalidWindowState(String),

    #[error("布局切换失败: {0}")]
    LayoutSwitchFailed(String),

    #[error("Dock 项不存在: {0}")]
    DockItemNotFound(String),

    #[error("菜单不存在: {0}")]
    MenuNotFound(String),

    #[error("通知推送失败: {0}")]
    NotificationFailed(String),

    #[error("主题不存在: {0}")]
    ThemeNotFound(String),

    #[error("主题解析失败: {0}")]
    ThemeParseError(String),

    #[error("渲染失败: {0}")]
    RenderFailed(String),

    #[error("手势注册失败: {0}")]
    GestureRegistrationFailed(String),

    #[error("Spotlight 搜索失败: {0}")]
    SpotlightSearchFailed(String),

    #[error("Agent Bar 错误: {0}")]
    AgentBarError(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("超时: {0}")]
    Timeout(String),

    #[error("无效参数: {0}")]
    InvalidParameter(String),
}
```

---

## 13. 使用示例

### 13.1 创建窗口

```rust
use ui_framework_api::*;

async fn window_example() -> Result<(), Box<dyn std::error::Error>> {
    let wm = WindowManagerImpl::new();

    // 创建窗口
    let attrs = WindowAttributes {
        title: "OmniAgent 文本编辑器".to_string(),
        app_id: "com.omniagent.editor".to_string(),
        position: (200, 200),
        size: (1200, 800),
        min_size: Some((400, 300)),
        resizable: true,
        window_type: WindowType::Application,
        decorated: true,
        ..Default::default()
    };

    let window_id = wm.create_window(attrs)?;
    println!("窗口已创建: {}", window_id);

    // 设置几何提示
    let hints = GeometryHints {
        min_width: 400,
        min_height: 300,
        max_width: 3840,
        max_height: 2160,
        base_width: 800,
        base_height: 600,
        width_increment: 1,
        height_increment: 1,
        min_aspect_ratio: Some((4, 3)),
        max_aspect_ratio: Some((16, 9)),
    };
    wm.set_geometry_hints(window_id, hints)?;

    // 注册事件回调
    wm.on_window_event(Box::new(|event| {
        match event {
            WindowEvent::Closed(id) => println!("窗口已关闭: {}", id),
            WindowEvent::Resized(id, (w, h)) => println!("窗口 {} 调整为 {}x{}", id, w, h),
            _ => {}
        }
    }));

    Ok(())
}
```

### 13.2 Agent Bar 集成

```rust
async fn agent_bar_example() -> Result<(), Box<dyn std::error::Error>> {
    let abm = AgentBarManagerImpl::new();

    // 显示 Agent Bar
    let config = AgentBarConfig {
        placeholder: "问我任何问题...".to_string(),
        width: 700,
        ..Default::default()
    };
    abm.show_agent_bar(config)?;

    // 设置流式响应回调
    abm.stream_agent_response(Box::new(|chunk| {
        if chunk.is_final {
            println!("响应完成");
        } else {
            print!("{}", chunk.content);
        }
    }))?;

    // 处理用户输入
    let response_id = abm.process_agent_input("帮我总结今天的日程")?;
    println!("响应 ID: {}", response_id);

    Ok(())
}
```

### 13.3 主题切换

```rust
async fn theme_example() -> Result<(), Box<dyn std::error::Error>> {
    let tm = ThemeManagerImpl::new();

    // 应用内置主题
    tm.apply_theme("catppuccin-mocha")?;

    // 获取设计令牌
    let tokens = tm.get_design_tokens()?;
    println!("主色调: {}", tokens.colors.primary);
    println!("圆角: {}", tokens.corner_radius.medium);

    // 设置强调色
    tm.set_accent_color("#89b4fa")?;

    // 切换深色模式
    let is_dark = tm.toggle_dark_mode()?;
    println!("深色模式: {}", is_dark);

    // 监听主题变更
    tm.on_theme_changed(Box::new(|tokens| {
        println!("主题已变更，主色调: {}", tokens.colors.primary);
    }))?;

    Ok(())
}
```

### 13.4 自定义渲染

```rust
async fn render_example() -> Result<(), Box<dyn std::error::Error>> {
    let rm = RenderManagerImpl::new();

    // 注册渲染回调
    rm.register_render_callback(window_id, Box::new(|ctx, commands| {
        // 清除背景
        commands.push(RenderCommand::DrawRect {
            x: 0.0,
            y: 0.0,
            width: ctx.viewport_width as f32,
            height: ctx.viewport_height as f32,
            color: "#1e1e2e".to_string(),
            corner_radius: 0.0,
        });

        // 绘制标题
        commands.push(RenderCommand::DrawText {
            text: "Hello, OmniAgent OS!".to_string(),
            x: 50.0,
            y: 50.0,
            font_size: 24.0,
            color: "#cdd6f4".to_string(),
            max_width: None,
        });

        // 绘制圆角矩形卡片
        commands.push(RenderCommand::DrawRect {
            x: 50.0,
            y: 100.0,
            width: 300.0,
            height: 200.0,
            color: "#313244".to_string(),
            corner_radius: 12.0,
        });
    }))?;

    // 提交渲染
    rm.present(window_id)?;

    // 查询帧率
    println!("当前帧率: {:.1} FPS", rm.get_fps());

    Ok(())
}
```

---

## 14. 性能约束

| 操作 | 延迟目标 | 吞吐量目标 | 说明 |
|------|---------|-----------|------|
| create_window | <50ms | 50/s | 含资源分配 |
| set_title | <1ms | 10000/s | 属性更新 |
| resize | <1ms | 5000/s | 布局重计算 |
| move_window | <1ms | 5000/s | 位置更新 |
| close | <10ms | 200/s | 含资源释放 |
| set_layout | <5ms | 200/s | 布局切换 |
| toggle_fullscreen | <16ms | 60/s | 含动画 |
| add_dock_item | <5ms | 100/s | Dock 更新 |
| bounce_dock_item | <1ms | 1000/s | 动画触发 |
| show_agent_bar | <16ms | 60/s | 含动画 |
| process_agent_input | <5ms | 100/s | 输入处理 |
| spotlight_search | <50ms | 20/s | 含索引查询 |
| push_notification | <5ms | 200/s | 通知显示 |
| apply_theme | <100ms | 10/s | 令牌替换 |
| set_accent_color | <16ms | 60/s | 即时生效 |
| register_gesture_handler | <1ms | 1000/s | 注册回调 |
| register_render_callback | <1ms | 1000/s | 注册回调 |
| present | <16ms | 60/s | 帧提交 |

---

## 15. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_attributes_default() {
        let attrs = WindowAttributes::default();
        assert_eq!(attrs.title, "");
        assert_eq!(attrs.size, (800, 600));
        assert!(attrs.resizable);
        assert!(!attrs.always_on_top);
        assert!((attrs.opacity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_window_types() {
        let types = [
            WindowType::Application,
            WindowType::Dialog,
            WindowType::Tooltip,
            WindowType::Menu,
            WindowType::Dock,
            WindowType::Notification,
            WindowType::AgentPanel,
            WindowType::Splash,
        ];
        assert_eq!(types.len(), 8);
    }

    #[test]
    fn test_layout_modes() {
        let modes = [LayoutMode::Floating, LayoutMode::Tiled, LayoutMode::Split];
        assert_eq!(modes.len(), 3);
    }

    #[test]
    fn test_snap_zones() {
        let zones = [
            SnapZone::Left, SnapZone::Right, SnapZone::Top, SnapZone::Bottom,
            SnapZone::TopLeft, SnapZone::TopRight, SnapZone::BottomLeft, SnapZone::BottomRight,
            SnapZone::Center,
        ];
        assert_eq!(zones.len(), 9);
    }

    #[test]
    fn test_dock_positions() {
        let positions = [DockPosition::Bottom, DockPosition::Left, DockPosition::Right];
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn test_agent_bar_config_default() {
        let config = AgentBarConfig::default();
        assert_eq!(config.width, 600);
        assert_eq!(config.position, AgentBarPosition::Bottom);
        assert!(!config.multiline);
    }

    #[test]
    fn test_notification_urgency() {
        let urgencies = [NotificationUrgency::Low, NotificationUrgency::Normal, NotificationUrgency::Critical];
        assert_eq!(urgencies.len(), 3);
    }

    #[test]
    fn test_spotlight_categories() {
        let categories = [
            SpotlightCategory::Application,
            SpotlightCategory::File,
            SpotlightCategory::AgentAction,
            SpotlightCategory::SystemSetting,
            SpotlightCategory::WebSearch,
            SpotlightCategory::Calculator,
            SpotlightCategory::Bookmark,
            SpotlightCategory::Contact,
        ];
        assert_eq!(categories.len(), 8);
    }

    #[test]
    fn test_gesture_types() {
        let types = [
            GestureType::Swipe, GestureType::Pinch, GestureType::Rotate,
            GestureType::EdgeSwipe, GestureType::Tap, GestureType::LongPress, GestureType::TwoFingerTap,
        ];
        assert_eq!(types.len(), 7);
    }

    #[test]
    fn test_gesture_phases() {
        let phases = [GesturePhase::Began, GesturePhase::Changed, GesturePhase::Ended, GesturePhase::Cancelled];
        assert_eq!(phases.len(), 4);
    }

    #[test]
    fn test_render_quality() {
        let qualities = [RenderQuality::Low, RenderQuality::Medium, RenderQuality::High, RenderQuality::Ultra];
        assert_eq!(qualities.len(), 4);
    }

    #[test]
    fn test_pixel_formats() {
        let formats = [PixelFormat::Rgba8888, PixelFormat::Bgra8888, PixelFormat::Rgb565, PixelFormat::A8];
        assert_eq!(formats.len(), 4);
    }

    #[test]
    fn test_geometry_hints() {
        let hints = GeometryHints {
            min_width: 400,
            min_height: 300,
            max_width: 3840,
            max_height: 2160,
            base_width: 800,
            base_height: 600,
            width_increment: 1,
            height_increment: 1,
            min_aspect_ratio: Some((4, 3)),
            max_aspect_ratio: None,
        };
        assert_eq!(hints.min_width, 400);
        assert!(hints.min_aspect_ratio.is_some());
    }

    #[test]
    fn test_stream_chunk() {
        let chunk = StreamChunk {
            chunk_id: "c-1".to_string(),
            content: "Hello".to_string(),
            is_final: false,
            agent_id: Some("agent-1".to_string()),
        };
        assert_eq!(chunk.content, "Hello");
        assert!(!chunk.is_final);
    }

    #[test]
    fn test_system_actions() {
        let actions = [
            SystemAction::About, SystemAction::Preferences, SystemAction::Hide,
            SystemAction::HideOthers, SystemAction::ShowAll, SystemAction::Quit,
            SystemAction::CloseWindow, SystemAction::Minimize, SystemAction::Zoom, SystemAction::Fullscreen,
        ];
        assert_eq!(actions.len(), 10);
    }

    #[test]
    fn test_icon_formats() {
        let formats = [IconFormat::Png, IconFormat::Svg, IconFormat::Rgba];
        assert_eq!(formats.len(), 3);
    }
}
```

---

*本文档为 OmniAgent OS UI 框架 API 参考，版本 0.1.0。*
