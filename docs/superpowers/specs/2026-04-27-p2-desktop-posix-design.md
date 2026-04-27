# OmniAgent OS 系统完善设计文档 — P2 桌面与高级功能

> **文档版本**: v1.0.0
> **日期**: 2026-04-27
> **状态**: 待审阅
> **范围**: Shell ↔ Compositor 集成、Dock + Menu Bar 实现、Agent Bar + Spotlight、POSIX Syscall 完善

---

## 0. 执行摘要

P2 阶段完善桌面环境（Aqua Shell）与合成器（Vulkan Compositor）的集成，实现系统级 UI 组件（Dock、菜单栏、Agent Bar、Spotlight 搜索），并补全剩余的 POSIX syscall 实现。这些模块构成用户可见的操作系统体验层。

**依赖关系：**
```
POSIX Syscall 完善 ──→ Shell ↔ Compositor 集成
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
               Dock 实现  Menu Bar  Agent Bar
                                        │
                                        ▼
                                   Spotlight
```

---

## 1. Shell ↔ Compositor 集成

### 1.1 设计动机

当前 `omniagent-shell`（窗口管理器 + UI 组件）和 `omniagent-compositor`（渲染管线 + 动画）是两个完全独立的 crate，没有任何集成。需要建立桥接层，使窗口系统能够将 Window 提交给合成器渲染。

### 1.2 设计方案

**架构：** 创建 `omniagent-desktop` crate 作为集成层

```
┌──────────────────────────────────────────────────────┐
│  Aqua Shell (omniagent-shell)                        │
│  ├── WindowManager → 管理窗口生命周期、z-order、焦点   │
│  ├── UIComponent → 按钮、标签、容器等 UI 组件         │
│  └── AquaShell → 桌面环境配置                         │
└──────────────────────┬───────────────────────────────┘
                       │ DesktopBridge trait
                       ▼
┌──────────────────────────────────────────────────────┐
│  omniagent-desktop (新建集成层)                       │
│  ├── DesktopBridge → Shell ↔ Compositor 桥接         │
│  ├── RenderableWindow → 可渲染的窗口表示              │
│  ├── InputRouter → 输入事件路由                       │
│  └── ThemeManager → 统一主题管理                      │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│  Vulkan Compositor (omniagent-compositor)             │
│  ├── CompositorRenderer → 渲染管线                    │
│  ├── AnimationManager → 动画系统                      │
│  └── GpuResourceManager → GPU 资源管理                │
└──────────────────────────────────────────────────────┘
```

### 1.3 DesktopBridge

```rust
/// 桌面桥接 trait：连接 Shell 和 Compositor
pub trait DesktopBridge: Send + Sync {
    /// 将窗口添加到合成器
    fn add_window(&self, window: &RenderableWindow) -> Result<WindowId, DesktopError>;

    /// 从合成器移除窗口
    fn remove_window(&self, id: WindowId) -> Result<(), DesktopError>;

    /// 更新窗口属性（位置、大小、状态）
    fn update_window(&self, id: WindowId, props: &WindowProperties) -> Result<(), DesktopError>;

    /// 请求渲染一帧
    fn request_frame(&self) -> Result<FrameToken, DesktopError>;

    /// 提交渲染内容
    fn commit_frame(&self, token: FrameToken, surfaces: &[SurfaceUpdate]) -> Result<(), DesktopError>;

    /// 获取渲染统计
    fn render_stats(&self) -> RenderStats;
}

/// 可渲染的窗口表示
pub struct RenderableWindow {
    /// 窗口 ID
    pub id: WindowId,
    /// 窗口属性
    pub properties: WindowProperties,
    /// 窗口表面（像素缓冲区）
    pub surface: WindowSurface,
    /// 窗口层级
    pub layer: RenderLayer,
    /// 动画状态
    pub animation_state: Option<AnimationState>,
}

/// 窗口表面
pub struct WindowSurface {
    /// 像素缓冲区
    pub buffer: PixelBuffer,
    /// 表面格式
    pub format: SurfaceFormat,
    /// 是否需要重绘
    pub dirty: AtomicBool,
    /// 损坏区域（用于部分重绘）
    pub damage: SpinLock<Vec<Rect>>,
}

/// 像素缓冲区
pub struct PixelBuffer {
    /// 像素数据（ARGB8888）
    pub data: Vec<u32>,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
    /// 行跨度
    pub stride: u32,
}

/// 渲染层
pub enum RenderLayer {
    Background,  // 壁纸
    Desktop,     // 桌面图标
    Windows,     // 普通窗口
    Panels,      // Dock、菜单栏
    Overlay,     // Spotlight、通知
    Cursor,      // 鼠标光标
}

/// 表面更新
pub struct SurfaceUpdate {
    pub window_id: WindowId,
    pub damage: Vec<Rect>,
    pub buffer: PixelBuffer,
    pub opacity: f32,
    pub transform: AffineTransform,
}

/// 帧令牌
pub struct FrameToken(pub u64);

/// 仿射变换
pub struct AffineTransform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,  // 弧度
}
```

### 1.4 输入路由

```rust
/// 输入事件路由器
pub struct InputRouter {
    /// 焦点窗口
    focused_window: AtomicU64,  // WindowId
    /// 鼠标位置
    mouse_pos: AtomicCell<(f32, f32)>,
    /// 按键状态
    key_state: SpinLock<HashMap<KeyCode, KeyState>>,
}

impl InputRouter {
    /// 路由鼠标事件
    pub fn route_mouse_event(&self, event: MouseEvent) -> InputTarget {
        match event.kind {
            MouseEventKind::Move => self.handle_mouse_move(event),
            MouseEventKind::ButtonDown => self.handle_mouse_down(event),
            MouseEventKind::ButtonUp => self.handle_mouse_up(event),
            MouseEventKind::Scroll => self.handle_scroll(event),
        }
    }

    /// 路由键盘事件
    pub fn route_key_event(&self, event: KeyEvent) -> InputTarget {
        // 键盘事件发送到焦点窗口
        InputTarget::Window(self.focused_window.load(Ordering::Relaxed))
    }

    /// 处理鼠标移动（命中测试）
    fn handle_mouse_move(&self, event: MouseEvent) -> InputTarget {
        let (x, y) = (event.x, event.y);
        self.mouse_pos.store((x, y), Ordering::Relaxed);

        // 从顶层到底层进行命中测试
        // 1. 检查 Overlay 层
        // 2. 检查 Panels 层
        // 3. 检查 Windows 层（从 z-order 顶部开始）
        // 4. 检查 Desktop 层
        // 返回命中的窗口
        self.hit_test(x, y)
    }

    /// 命中测试
    fn hit_test(&self, x: f32, y: f32) -> InputTarget {
        // 遍历窗口 z-order（从高到低）
        // 检查 (x, y) 是否在窗口矩形内
        // 返回第一个命中的窗口
        InputTarget::Window(0)  // 简化
    }
}

/// 输入目标
pub enum InputTarget {
    Window(WindowId),
    Desktop,
    None,
}

/// 鼠标事件
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

pub enum MouseEventKind {
    Move,
    ButtonDown,
    ButtonUp,
    Scroll { delta_x: f32, delta_y: f32 },
}

/// 键盘事件
pub struct KeyEvent {
    pub key: KeyCode,
    pub state: KeyState,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

pub enum KeyState {
    Pressed,
    Released,
}
```

### 1.5 主题管理

```rust
/// 统一主题管理器
pub struct ThemeManager {
    /// 当前主题
    current: RwLock<DesktopTheme>,
    /// 已注册主题
    themes: RwLock<Vec<DesktopTheme>>,
}

/// 桌面主题（扩展 Shell 的 Theme）
pub struct DesktopTheme {
    /// 主题名称
    pub name: String,
    /// 基础颜色
    pub colors: ThemeColors,
    /// 字体配置
    pub fonts: ThemeFonts,
    /// 圆角半径
    pub corner_radius: f32,
    /// 阴影配置
    pub shadow: ShadowConfig,
    /// 动画配置
    pub animation: AnimationConfig,
}

pub struct ThemeColors {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub surface: Color,
    pub surface_variant: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
}

pub struct ThemeFonts {
    pub ui_font_name: String,
    pub ui_font_size: f32,
    pub mono_font_name: String,
    pub mono_font_size: f32,
    pub display_font_name: String,
    pub display_font_size: f32,
}

pub struct ShadowConfig {
    pub color: Color,
    pub blur_radius: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub spread: f32,
}

pub struct AnimationConfig {
    pub window_open_duration_ms: u32,
    pub window_close_duration_ms: u32,
    pub minimize_duration_ms: u32,
    pub dock_bounce_duration_ms: u32,
    pub default_easing: EasingFunction,
}

impl Default for DesktopTheme {
    fn default() -> Self {
        Self {
            name: "Aqua Dark".into(),
            colors: ThemeColors {
                background: Color::from_argb(0xFF, 0x1E, 0x1E, 0x2E),
                foreground: Color::from_argb(0xFF, 0xFF, 0xFF, 0xFF),
                accent: Color::from_argb(0xFF, 0x00, 0x7A, 0xFF),
                surface: Color::from_argb(0xFF, 0x2A, 0x2A, 0x3C),
                surface_variant: Color::from_argb(0xFF, 0x36, 0x36, 0x4A),
                border: Color::from_argb(0xFF, 0x44, 0x44, 0x5A),
                error: Color::from_argb(0xFF, 0xFF, 0x44, 0x44),
                warning: Color::from_argb(0xFF, 0xFF, 0xAA, 0x00),
                success: Color::from_argb(0xFF, 0x44, 0xFF, 0x44),
                text_primary: Color::from_argb(0xFF, 0xFF, 0xFF, 0xFF),
                text_secondary: Color::from_argb(0xFF, 0xAA, 0xAA, 0xBB),
                text_disabled: Color::from_argb(0xFF, 0x66, 0x66, 0x77),
            },
            fonts: ThemeFonts {
                ui_font_name: "SF Pro".into(),
                ui_font_size: 14.0,
                mono_font_name: "SF Mono".into(),
                mono_font_size: 13.0,
                display_font_name: "SF Pro Display".into(),
                display_font_size: 24.0,
            },
            corner_radius: 12.0,
            shadow: ShadowConfig {
                color: Color::from_argb(0x80, 0x00, 0x00, 0x00),
                blur_radius: 20.0,
                offset_x: 0.0,
                offset_y: 4.0,
                spread: 0.0,
            },
            animation: AnimationConfig {
                window_open_duration_ms: 250,
                window_close_duration_ms: 200,
                minimize_duration_ms: 300,
                dock_bounce_duration_ms: 500,
                default_easing: EasingFunction::EaseOutCubic,
            },
        }
    }
}
```

### 1.6 测试策略

```
TDD 测试用例：
1. test_desktop_bridge_add_window — 添加窗口
2. test_desktop_bridge_remove_window — 移除窗口
3. test_desktop_bridge_update_window — 更新窗口属性
4. test_desktop_bridge_request_commit_frame — 请求和提交帧
5. test_pixel_buffer_create — 创建像素缓冲区
6. test_pixel_buffer_set_pixel — 设置像素
7. test_pixel_buffer_get_pixel — 获取像素
8. test_renderable_window_properties — 可渲染窗口属性
9. test_input_router_mouse_move — 鼠标移动路由
10. test_input_router_key_event — 键盘事件路由
11. test_input_router_hit_test — 命中测试
12. test_theme_manager_default — 默认主题
13. test_theme_manager_switch — 切换主题
14. test_theme_colors_valid — 主题颜色有效
15. test_animation_config — 动画配置
16. test_surface_damage — 损坏区域跟踪
17. test_affine_transform — 仿射变换
18. test_render_layer_order — 渲染层级顺序
```

### 1.7 文件结构

```
crates/omniagent-desktop/
├── Cargo.toml
└── src/
    ├── lib.rs              # 模块声明 + 重导出
    ├── bridge.rs           # DesktopBridge trait + 默认实现
    ├── renderable.rs       # RenderableWindow, WindowSurface, PixelBuffer
    ├── input.rs            # InputRouter, MouseEvent, KeyEvent
    ├── theme.rs            # ThemeManager, DesktopTheme
    └── error.rs            # DesktopError
```

---

## 2. Dock + Menu Bar 实现

### 2.1 设计动机

Aqua Shell 规范中定义了 Dock（应用启动器）和 Menu Bar（全局菜单栏），但当前只有 `DockPosition` 枚举，没有实际实现。

### 2.2 Dock 实现

```rust
/// Dock 组件
pub struct Dock {
    /// Dock 位置
    pub position: DockPosition,
    /// Dock 项目列表
    pub items: Vec<DockItem>,
    /// Dock 大小
    pub size: DockSize,
    /// 当前悬停的项目
    pub hovered_item: AtomicUsize,  // usize::MAX 表示无
    /// 动画状态
    pub animation: DockAnimation,
    /// Dock 配置
    pub config: DockConfig,
}

/// Dock 位置
pub enum DockPosition {
    Bottom,
    Left,
    Right,
}

/// Dock 大小
pub struct DockSize {
    pub icon_size: u32,       // 图标大小（像素）
    pub padding: u32,         // 内边距
    pub spacing: u32,         // 项目间距
    pub magnification: f32,   // 放大倍率
}

/// Dock 项目
pub struct DockItem {
    /// 项目 ID
    pub id: DockItemId,
    /// 应用名称
    pub app_name: String,
    /// 图标
    pub icon: Option<Icon>,
    /// 应用状态
    pub state: DockItemState,
    /// 关联的 Agent 句柄
    pub agent_handle: Option<u64>,
    /// 点击回调
    pub on_click: Option<Box<dyn Fn() + Send>>,
}

/// Dock 项目状态
pub enum DockItemState {
    /// 正常（未运行）
    Normal,
    /// 运行中
    Running { pid: u64 },
    /// 有未读通知
    Notification { count: u32 },
    /// 正在启动
    Launching,
}

/// Dock 动画
pub struct DockAnimation {
    /// 放大动画
    pub magnification: Animation,
    /// 弹跳动画（应用启动时）
    pub bounce: Animation,
    /// 滑入动画
    pub slide_in: Animation,
}

/// Dock 配置
pub struct DockConfig {
    /// 自动隐藏
    pub auto_hide: bool,
    /// 隐藏延迟（毫秒）
    pub hide_delay_ms: u32,
    /// 显示延迟（毫秒）
    pub show_delay_ms: u32,
    /// 最大放大倍率
    pub max_magnification: f32,
    /// 放大范围（像素）
    pub magnification_range: u32,
    /// 是否显示运行指示器
    pub show_running_indicator: bool,
    /// 是否显示通知徽章
    pub show_notification_badge: bool,
}

impl Dock {
    /// 创建新的 Dock
    pub fn new(position: DockPosition) -> Self;

    /// 添加项目
    pub fn add_item(&mut self, item: DockItem) -> Result<(), DesktopError>;

    /// 移除项目
    pub fn remove_item(&mut self, id: DockItemId) -> Result<(), DesktopError>;

    /// 更新项目状态
    pub fn update_item_state(&mut self, id: DockItemId, state: DockItemState);

    /// 处理鼠标悬停（放大效果）
    pub fn handle_hover(&mut self, x: f32, y: f32) -> HoverEffect;

    /// 处理点击
    pub fn handle_click(&mut self, x: f32, y: f32) -> Option<DockItemId>;

    /// 渲染 Dock 到像素缓冲区
    pub fn render(&self, theme: &DesktopTheme) -> PixelBuffer;

    /// 获取 Dock 占据的矩形区域
    pub fn bounds(&self, screen_size: (u32, u32)) -> Rect;
}

/// 悬停效果
pub struct HoverEffect {
    /// 受影响的项目索引
    pub affected_items: Vec<(usize, f32)>,  // (index, scale_factor)
    /// Dock 是否需要重绘
    pub needs_redraw: bool,
}
```

### 2.3 Menu Bar 实现

```rust
/// 菜单栏
pub struct MenuBar {
    /// 菜单项列表
    pub menus: Vec<MenuBarMenu>,
    /// 当前激活的菜单（下拉展开）
    pub active_menu: AtomicUsize,  // usize::MAX 表示无
    /// 菜单栏高度
    pub height: u32,
    /// 菜单栏配置
    pub config: MenuBarConfig,
}

/// 菜单栏菜单
pub struct MenuBarMenu {
    /// 菜单标题
    pub title: String,
    /// 菜单项列表
    pub items: Vec<MenuItem>,
    /// 快捷键提示
    pub shortcut_hint: Option<String>,
}

/// 菜单项
pub struct MenuItem {
    /// 菜单项 ID
    pub id: MenuItemId,
    /// 显示文本
    pub label: String,
    /// 快捷键
    pub shortcut: Option<KeyBinding>,
    /// 菜单项类型
    pub kind: MenuItemKind,
    /// 是否启用
    pub enabled: bool,
    /// 子菜单
    pub submenu: Option<Vec<MenuItem>>,
    /// 点击回调
    pub on_click: Option<Box<dyn Fn() + Send>>,
}

/// 菜单项类型
pub enum MenuItemKind {
    /// 普通菜单项
    Normal,
    /// 分隔线
    Separator,
    /// 勾选菜单项
    Checkbox { checked: bool },
    /// 单选菜单项
    Radio { selected: bool, group: String },
    /// 子菜单入口
    Submenu,
}

/// 键绑定
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

/// 菜单栏配置
pub struct MenuBarConfig {
    /// 是否显示 Apple/系统菜单
    pub show_system_menu: bool,
    /// 字体大小
    pub font_size: f32,
    /// 菜单项内边距
    pub item_padding: (f32, f32),
    /// 下拉菜单最大高度
    pub dropdown_max_height: u32,
    /// 动画持续时间（毫秒）
    pub animation_duration_ms: u32,
}

impl MenuBar {
    /// 创建新的菜单栏
    pub fn new() -> Self;

    /// 添加菜单
    pub fn add_menu(&mut self, menu: MenuBarMenu);

    /// 处理鼠标点击
    pub fn handle_click(&mut self, x: f32, y: f32) -> MenuAction;

    /// 处理鼠标悬停
    pub fn handle_hover(&mut self, x: f32, y: f32) -> bool;

    /// 渲染菜单栏
    pub fn render(&self, theme: &DesktopTheme) -> PixelBuffer;

    /// 获取菜单栏占据的矩形区域
    pub fn bounds(&self, screen_width: u32) -> Rect;
}

/// 菜单操作结果
pub enum MenuAction {
    /// 点击了菜单项
    ItemClicked(MenuItemId),
    /// 展开了子菜单
    SubmenuOpened(usize),
    /// 无操作
    None,
}
```

### 2.4 测试策略

```
Dock 测试：
1. test_dock_create — 创建 Dock
2. test_dock_add_remove_item — 添加移除项目
3. test_dock_hover_effect — 悬停放大效果
4. test_dock_click — 点击选择
5. test_dock_bounds — Dock 区域计算
6. test_dock_render — 渲染输出
7. test_dock_magnification — 放大动画
8. test_dock_item_state_update — 状态更新
9. test_dock_auto_hide — 自动隐藏
10. test_dock_notification_badge — 通知徽章

Menu Bar 测试：
11. test_menubar_create — 创建菜单栏
12. test_menubar_add_menu — 添加菜单
13. test_menubar_click — 点击菜单项
14. test_menubar_hover — 悬停展开
15. test_menubar_bounds — 区域计算
16. test_menubar_render — 渲染输出
17. test_menu_item_shortcut — 快捷键
18. test_menu_item_checkbox — 勾选菜单项
19. test_menu_item_radio — 单选菜单项
20. test_menu_item_separator — 分隔线
```

### 2.5 文件结构

```
crates/omniagent-shell/src/
├── dock.rs            # 新建：Dock 组件
├── menu_bar.rs        # 新建：Menu Bar 组件
└── lib.rs             # 修改：添加模块导出
```

---

## 3. Agent Bar + Spotlight

### 3.1 设计动机

Aqua Shell 规范中定义了 Agent Bar（Agent 状态面板）和 Agent Spotlight（快速搜索），这是 OmniAgent OS 的独创交互方式，让用户能够直观地管理和搜索 Agent。

### 3.2 Agent Bar 实现

```rust
/// Agent Bar：显示 Agent 状态的面板
pub struct AgentBar {
    /// Agent 状态条目列表
    pub entries: Vec<AgentBarEntry>,
    /// 面板位置
    pub position: AgentBarPosition,
    /// 面板配置
    pub config: AgentBarConfig,
    /// 是否展开
    pub expanded: AtomicBool,
    /// 滚动偏移
    pub scroll_offset: AtomicU32,
}

/// Agent Bar 位置
pub enum AgentBarPosition {
    /// 屏幕右侧
    Right,
    /// 屏幕左侧
    Left,
    /// 屏幕底部（与 Dock 分离）
    Bottom,
}

/// Agent Bar 条目
pub struct AgentBarEntry {
    /// Agent 句柄
    pub agent_handle: u64,
    /// Agent 名称
    pub name: String,
    /// Agent 类型
    pub agent_type: AgentType,
    /// Agent 状态
    pub state: AgentState,
    /// CPU 使用率（百分比）
    pub cpu_usage: f32,
    /// 内存使用量
    pub memory_used: u64,
    /// 消息计数（发送/接收）
    pub msg_count: (u64, u64),
    /// 最后活跃时间
    pub last_active: u64,
    /// 图标
    pub icon: Option<Icon>,
    /// 点击回调
    pub on_click: Option<Box<dyn Fn() + Send>>,
}

/// Agent Bar 配置
pub struct AgentBarConfig {
    /// 面板宽度（像素）
    pub width: u32,
    /// 最大显示条目数
    pub max_entries: usize,
    /// 是否显示 CPU 使用率
    pub show_cpu_usage: bool,
    /// 是否显示内存使用量
    pub show_memory: bool,
    /// 是否显示消息计数
    pub show_msg_count: bool,
    /// 刷新间隔（毫秒）
    pub refresh_interval_ms: u32,
    /// 排序方式
    pub sort_by: AgentBarSortBy,
    /// 是否自动折叠
    pub auto_collapse: bool,
    /// 折叠超时（毫秒）
    pub collapse_timeout_ms: u32,
}

/// 排序方式
pub enum AgentBarSortBy {
    Name,
    CpuUsage,
    MemoryUsage,
    State,
    LastActive,
}

impl AgentBar {
    /// 创建新的 Agent Bar
    pub fn new(position: AgentBarPosition) -> Self;

    /// 更新 Agent 状态
    pub fn update_agent(&mut self, handle: u64, state: AgentState, cpu: f32, mem: u64);

    /// 添加 Agent
    pub fn add_agent(&mut self, entry: AgentBarEntry);

    /// 移除 Agent
    pub fn remove_agent(&mut self, handle: u64);

    /// 搜索 Agent
    pub fn search(&self, query: &str) -> Vec<&AgentBarEntry>;

    /// 排序
    pub fn sort(&mut self, by: AgentBarSortBy);

    /// 渲染 Agent Bar
    pub fn render(&self, theme: &DesktopTheme) -> PixelBuffer;

    /// 获取 Agent Bar 占据的矩形区域
    pub fn bounds(&self, screen_size: (u32, u32)) -> Rect;
}
```

### 3.3 Spotlight 实现

```rust
/// Agent Spotlight：快速搜索和启动
pub struct Spotlight {
    /// 搜索索引
    pub index: SpotlightIndex,
    /// 搜索结果
    pub results: Vec<SpotlightResult>,
    /// 当前选中的结果索引
    pub selected_index: AtomicUsize,
    /// Spotlight 状态
    pub state: SpotlightState,
    /// 配置
    pub config: SpotlightConfig,
    /// 搜索历史
    pub history: Vec<String>,
}

/// Spotlight 索引
pub struct SpotlightIndex {
    /// Agent 索引
    pub agents: Vec<SpotlightAgentEntry>,
    /// 应用索引
    pub applications: Vec<SpotlightAppEntry>,
    /// 文件索引
    pub files: Vec<SpotlightFileEntry>,
    /// 系统命令索引
    pub commands: Vec<SpotlightCommandEntry>,
}

/// Spotlight 结果
pub struct SpotlightResult {
    /// 结果类型
    pub kind: SpotlightResultKind,
    /// 结果标题
    pub title: String,
    /// 结果副标题
    pub subtitle: String,
    /// 结果图标
    pub icon: Option<Icon>,
    /// 关联数据
    pub data: SpotlightResultData,
    /// 相关度分数
    pub score: f32,
}

pub enum SpotlightResultKind {
    Agent,
    Application,
    File,
    Command,
    Setting,
}

pub enum SpotlightResultData {
    AgentHandle(u64),
    AppPath(String),
    FilePath(String),
    Command(String),
    SettingKey(String),
}

/// Spotlight 状态
pub enum SpotlightState {
    /// 隐藏
    Hidden,
    /// 显示中（搜索框聚焦）
    Visible,
    /// 显示结果
    ShowingResults,
}

/// Spotlight 配置
pub struct SpotlightConfig {
    /// 最大结果数
    pub max_results: usize,
    /// 搜索延迟（毫秒）
    pub search_delay_ms: u32,
    /// 是否包含文件搜索
    pub include_files: bool,
    /// 是否包含 Agent 搜索
    pub include_agents: bool,
    /// 是否包含命令搜索
    pub include_commands: bool,
    /// 快捷键
    pub hotkey: KeyBinding,
    /// 动画持续时间（毫秒）
    pub animation_duration_ms: u32,
    /// 模糊匹配阈值
    pub fuzzy_threshold: f32,
}

impl Spotlight {
    /// 创建新的 Spotlight
    pub fn new() -> Self;

    /// 打开 Spotlight
    pub fn open(&mut self);

    /// 关闭 Spotlight
    pub fn close(&mut self);

    /// 执行搜索
    pub fn search(&mut self, query: &str);

    /// 选择下一个结果
    pub fn select_next(&mut self);

    /// 选择上一个结果
    pub fn select_prev(&mut self);

    /// 确认选择
    pub fn confirm(&mut self) -> Option<SpotlightResult>;

    /// 处理键盘输入
    pub fn handle_key(&mut self, key: KeyEvent) -> SpotlightAction;

    /// 渲染 Spotlight 界面
    pub fn render(&self, theme: &DesktopTheme, screen_size: (u32, u32)) -> PixelBuffer;
}

/// Spotlight 操作结果
pub enum SpotlightAction {
    Opened,
    Closed,
    SearchUpdated,
    ResultSelected(SpotlightResult),
    None,
}
```

### 3.4 测试策略

```
Agent Bar 测试：
1. test_agent_bar_create — 创建 Agent Bar
2. test_agent_bar_add_remove — 添加移除 Agent
3. test_agent_bar_update — 更新 Agent 状态
4. test_agent_bar_search — 搜索 Agent
5. test_agent_bar_sort — 排序
6. test_agent_bar_render — 渲染
7. test_agent_bar_bounds — 区域计算
8. test_agent_bar_expand_collapse — 展开/折叠

Spotlight 测试：
9. test_spotlight_create — 创建 Spotlight
10. test_spotlight_open_close — 打开/关闭
11. test_spotlight_search_agent — 搜索 Agent
12. test_spotlight_search_file — 搜索文件
13. test_spotlight_search_command — 搜索命令
14. test_spotlight_fuzzy_match — 模糊匹配
15. test_spotlight_select — 选择结果
16. test_spotlight_confirm — 确认选择
17. test_spotlight_keyboard — 键盘操作
18. test_spotlight_render — 渲染
19. test_spotlight_history — 搜索历史
20. test_spotlight_score_ranking — 结果排序
```

### 3.5 文件结构

```
crates/omniagent-shell/src/
├── agent_bar.rs       # 新建：Agent Bar
├── spotlight.rs       # 新建：Spotlight 搜索
└── lib.rs             # 修改：添加模块导出
```

---

## 4. POSIX Syscall 完善

### 4.1 设计动机

当前内核的 30+ 个传统 POSIX syscall 全部返回 `E_NOTSUP`。需要实现最常用的 syscall，使标准库和应用程序能够正常运行。

### 4.2 实现优先级

**第一批（必须实现）：**

| Syscall | 编号 | 功能 | 复杂度 |
|---------|------|------|--------|
| `read` | 0 | 读文件 | 低（已有 fd_table） |
| `write` | 1 | 写文件 | 低 |
| `open` | 2 | 打开文件 | 低 |
| `close` | 3 | 关闭文件 | 低 |
| `stat` | 4 | 文件状态 | 中 |
| `fstat` | 5 | fd 状态 | 中 |
| `poll` | 7 | I/O 多路复用 | 中 |
| `mmap` | 9 | 内存映射 | 高 |
| `munmap` | 11 | 取消映射 | 中 |
| `brk` | 12 | 堆扩展 | 低 |
| `ioctl` | 16 | 设备控制 | 中 |
| `getpid` | 39 | 获取进程 ID | 低 |
| `clone` | 56 | 创建线程 | 高 |
| `fork` | 57 | 创建进程 | 高 |
| `execve` | 59 | 执行程序 | 高 |
| `exit` | 60 | 退出 | 低 |
| `wait4` | 61 | 等待子进程 | 中 |
| `fcntl` | 72 | 文件控制 | 中 |
| `getcwd` | 79 | 获取工作目录 | 低 |
| `chdir` | 80 | 切换目录 | 低 |
| `mkdir` | 83 | 创建目录 | 低 |
| `unlink` | 87 | 删除文件 | 低 |
| `getdents64` | 217 | 读目录 | 中 |
| `clock_gettime` | 228 | 获取时间 | 低 |
| `futex` | 202 | 快速用户空间锁 | 高 |

**第二批（后续实现）：**

| Syscall | 编号 | 功能 |
|---------|------|------|
| `lseek` | 8 | 文件偏移 |
| `mprotect` | 10 | 内存保护 |
| `writev` | 20 | 分散写 |
| `readv` | 19 | 分散读 |
| `madvise` | 28 | 内存建议 |
| `sigaction` | 13 | 信号处理 |
| `set_tid_address` | 96 | 设置 TID |
| `getrandom` | 318 | 随机数 |
| `rseq` | 334 | 可重启序列 |

### 4.3 关键 Syscall 实现

#### brk（堆扩展）

```rust
/// brk syscall — 扩展/收缩进程堆
///
/// 这是用户态堆分配器（如 glibc malloc）的基础
fn sys_brk(new_brk: u64) -> Result<u64, SyscallError> {
    let task = current_task();
    let current_brk = task.heap_brk.load(Relaxed);
    let heap_end = task.heap_end;

    if new_brk == 0 {
        return Ok(current_brk);
    }

    if new_brk < task.heap_start || new_brk > heap_end {
        return Ok(current_brk);  // 不变
    }

    task.heap_brk.store(new_brk, Relaxed);
    Ok(new_brk)
}
```

#### mmap（内存映射）

```rust
/// mmap syscall — 创建内存映射
fn sys_mmap(
    addr: u64,
    length: u64,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: u64,
) -> Result<u64, SyscallError> {
    let task = current_task();
    let prot = MmapProt::from_bits(prot)?;
    let flags = MmapFlags::from_bits(flags)?;

    // 查找空闲虚拟地址区域
    let vaddr = if addr != 0 && flags.contains(MmapFlags::MAP_FIXED) {
        VirtAddr(addr)
    } else {
        task.address_space.find_free_area(length as usize)?
    };

    // 根据映射类型处理
    if flags.contains(MmapFlags::MAP_ANONYMOUS) {
        // 匿名映射：分配物理页并映射
        let pages = (length + PAGE_SIZE - 1) / PAGE_SIZE;
        for i in 0..pages {
            let frame = FRAME_ALLOCATOR.allocate_frame()?;
            task.address_space.map_page(
                VirtAddr(vaddr.0 + i * PAGE_SIZE),
                frame,
                prot.to_page_table_flags(),
            )?;
        }
    } else {
        // 文件映射：从 fd 读取并映射
        let entry = task.fd_table.get(fd as u32)?;
        // ... 文件映射逻辑
    }

    Ok(vaddr.0)
}
```

#### clone（创建线程）

```rust
/// clone syscall — 创建新线程/进程
fn sys_clone(
    flags: u64,
    stack: u64,
    parent_tid: u64,
    child_tid: u64,
    tls: u64,
) -> Result<u64, SyscallError> {
    let task = current_task();
    let is_thread = flags & CLONE_THREAD != 0;
    let share_vm = flags & CLONE_VM != 0;
    let share_fs = flags & CLONE_FS != 0;
    let share_files = flags & CLONE_FILES != 0;
    let share_sighandler = flags & CLONE_SIGHAND != 0;

    // 创建新 TCB
    let new_task_id = SCHEDULER.lock().create_task(
        0,  // clone 不设置 entry（从 clone 恢复点继续）
        stack,
        task.sched_info.priority,
        false,  // 内核线程或用户线程
        None,
    )?;

    // 共享/复制资源
    if share_vm {
        // 共享地址空间（线程）
        new_tcb.address_space = task.address_space.clone();
    } else {
        // 复制地址空间（fork）
        new_tcb.address_space = Some(task.address_space.fork()?);
    }

    if share_files {
        new_tcb.fd_table = Arc::clone(&task.fd_table);
    } else {
        new_tcb.fd_table = Arc::new(task.fd_table.clone());
    }

    // 设置返回值
    // 子线程返回 0
    // 父线程返回子线程 TID

    Ok(new_task_id.0)
}
```

#### futex（快速用户空间锁）

```rust
/// futex syscall — 快速用户空间互斥锁
fn sys_futex(
    uaddr: u64,
    futex_op: i32,
    val: u64,
    timeout: u64,
    uaddr2: u64,
    val3: u64,
) -> Result<i32, SyscallError> {
    let op = FutexOp::from_raw(futex_op)?;

    match op {
        FutexOp::Wait => {
            // 原子检查 *uaddr == val
            // 如果相等，将当前任务加入等待队列并睡眠
            let ptr = uaddr as *const AtomicU32;
            let expected = val as u32;
            unsafe {
                if (*ptr).load(Ordering::SeqCst) == expected {
                    FUTEX_QUEUE.wait(uaddr, current_task_id());
                    scheduler::sleep(current_task_id(), uaddr);
                    Ok(0)
                } else {
                    Ok(-1)  // EAGAIN
                }
            }
        }
        FutexOp::Wake => {
            // 唤醒最多 val 个等待在 uaddr 上的任务
            let woken = FUTEX_QUEUE.wake(uaddr, val as usize);
            Ok(woken as i32)
        }
        FutexOp::CmpRequeue => {
            // 原子比较并重新排队
            // ...
        }
        _ => Err(SyscallError::E_INVAL),
    }
}

/// Futex 等待队列
pub struct FutexQueue {
    /// 等待队列：uaddr → Vec<TaskId>
    queues: SpinLock<HashMap<u64, Vec<TaskId>>>,
}
```

#### clock_gettime

```rust
/// clock_gettime syscall
fn sys_clock_gettime(clock_id: i32, tp: *mut TimeSpec) -> Result<i32, SyscallError> {
    let clock = ClockId::from_raw(clock_id)?;
    let timespec = match clock {
        ClockId::Monotonic => {
            let ns = time::timer::monotonic_ns();
            TimeSpec::from_nanos(ns)
        }
        ClockId::Realtime => {
            let ns = time::timer::realtime_ns();
            TimeSpec::from_nanos(ns)
        }
        _ => return Err(SyscallError::E_INVAL),
    };
    copy_to_user(tp, &timespec)?;
    Ok(0)
}

#[repr(C)]
pub struct TimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}
```

### 4.4 测试策略

```
TDD 测试用例：
1. test_sys_brk_expand — 扩展堆
2. test_sys_brk_shrink — 收缩堆
3. test_sys_brk_invalid — 无效地址
4. test_sys_mmap_anonymous — 匿名映射
5. test_sys_munmap — 取消映射
6. test_sys_mmap_fixed — 固定地址映射
7. test_sys_getpid — 获取进程 ID
8. test_sys_clone_thread — 创建线程
9. test_sys_futex_wait_wake — futex 等待唤醒
10. test_sys_futex_timeout — futex 超时
11. test_sys_clock_gettime_monotonic — 单调时钟
12. test_sys_clock_gettime_realtime — 实时时钟
13. test_sys_getcwd — 获取工作目录
14. test_sys_chdir — 切换目录
15. test_sys_mkdir — 创建目录
16. test_sys_unlink — 删除文件
17. test_sys_getdents64 — 读目录
18. test_sys_fcntl_dup — 复制 fd
19. test_sys_poll_timeout — poll 超时
20. test_sys_poll_readable — poll 可读
```

### 4.5 文件结构

```
kernel/src/syscall/
├── mod.rs              # 修改：添加 POSIX syscall 模块
├── dispatch.rs         # 修改：实现 POSIX syscall
├── posix.rs            # 新建：POSIX syscall 实现
├── futex.rs            # 新建：Futex 实现
├── mmap.rs             # 新建：mmap/munmap 实现
└── time.rs             # 新建：时间相关 syscall
```

---

## 5. 跨模块集成

### 5.1 P2 模块依赖图

```
┌──────────────────────────────────────────────────────────┐
│  Aqua Shell                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │  Dock    │  │ Menu Bar │  │ Agent Bar│  │Spotlight│ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬────┘ │
│       └──────────────┴──────────────┴──────────────┘    │
│                          │                               │
│                    omniagent-desktop                     │
│                    (DesktopBridge)                       │
│                          │                               │
│       ┌──────────────────┼──────────────────┐           │
│       │                  │                  │           │
│  omniagent-shell   omniagent-compositor  POSIX Syscall  │
│  (WindowManager)   (Renderer)          (brk/mmap/etc)   │
└──────────────────────────────────────────────────────────┘
```

### 5.2 Workspace Cargo.toml 更新

```toml
[workspace]
members = [
    # ... 现有成员
    "crates/omniagent-desktop",  # 新增
]
```

---

## 6. 成功标准

| 标准 | 验证方法 |
|------|---------|
| Shell ↔ Compositor 可通信 | 集成测试：创建窗口 → 提交渲染 → 获取帧 |
| Dock 可添加/移除/点击项目 | 单元测试 + 渲染验证 |
| Menu Bar 可展开/选择 | 单元测试 |
| Agent Bar 显示 Agent 状态 | 单元测试 + 状态更新 |
| Spotlight 可搜索/选择 | 单元测试 + 模糊匹配 |
| POSIX syscall 可用 | 单元测试 + syscall 返回正确值 |
| 所有测试通过 | `cargo test --workspace` 全绿 |
| 新增测试 ≥ 80 个 | 测试计数 |

---

## 7. 总结

P2 阶段完成后，OmniAgent OS 将具备：

1. **完整的桌面环境**：Dock + Menu Bar + Agent Bar + Spotlight
2. **Shell ↔ Compositor 集成**：窗口可被渲染
3. **POSIX 兼容**：常用 syscall 全部实现
4. **用户可见的进步**：从纯命令行到图形化桌面

预计新增代码量：~8000 行
预计新增测试：~80 个
