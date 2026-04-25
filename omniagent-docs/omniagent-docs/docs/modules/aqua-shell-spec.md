# Aqua Shell 桌面环境规范

> **模块名称**: `aqua-shell`
> **版本**: 0.1.0
> **状态**: 设计阶段
> **最后更新**: 2026-04-25

---

## 1. 概述

### 1.1 目的

Aqua Shell 是 OmniAgent OS 的原生桌面环境，基于 Vulkan 渲染管线构建，提供高性能合成器、智能窗口管理、Agent 集成界面以及现代化的用户交互体验。作为 Agent-Native 操作系统的图形外壳，Aqua Shell 将 AI Agent 交互深度融入桌面体验，提供 Agent Bar、Agent Spotlight 等独创的交互范式。

### 1.2 设计目标

| 目标 | 描述 | 指标 |
|------|------|------|
| 高性能渲染 | Vulkan 硬件加速合成 | ≥60fps @ 4K |
| Agent 原生 | Agent 交互作为一等公民 | Spotlight 搜索 <50ms |
| 流畅动画 | 弹簧物理动画系统 | 窗口打开 200ms |
| 触控友好 | 手势支持 | 响应延迟 <16ms |
| 可扩展 | 插件化架构 | 主题热切换 |

### 1.3 架构总览

```
┌─────────────────────────────────────────────────────┐
│                   Aqua Shell                        │
├──────────┬──────────┬──────────┬────────────────────┤
│ Compositor│ Window   │  Dock    │  Menu Bar          │
│ (Vulkan)  │ Manager  │          │                    │
├──────────┼──────────┼──────────┼────────────────────┤
│ Agent Bar │Spotlight │ Notif.   │  Theme Engine      │
│           │          │ Center   │                    │
├──────────┴──────────┴──────────┴────────────────────┤
│              Input Handler (libinput)                │
├─────────────────────────────────────────────────────┤
│         Rendering Backend (ash / Smithay)           │
└─────────────────────────────────────────────────────┘
```

---

## 2. 合成器 (Compositor)

### 2.1 Vulkan 渲染循环

合成器基于 `ash` crate 实现 Vulkan 渲染，采用双缓冲交换链 (double-buffered swapchain) 并支持 vsync 垂直同步。

```rust
use ash::{vk, Entry, Instance, Device};
use std::time::Duration;

/// Vulkan 渲染器配置
pub struct VulkanConfig {
    /// 交换链图像数量（双缓冲 = 2）
    pub swapchain_image_count: u32,
    /// 是否启用 vsync
    pub vsync_enabled: bool,
    /// 渲染分辨率
    pub resolution: (u32, u32),
    /// 采样率（MSAA）
    pub sample_count: vk::SampleCountFlags,
    /// 预期帧率
    pub target_fps: u32,
}

impl Default for VulkanConfig {
    fn default() -> Self {
        Self {
            swapchain_image_count: 2,
            vsync_enabled: true,
            resolution: (3840, 2160),
            sample_count: vk::SampleCountFlags::TYPE_4,
            target_fps: 60,
        }
    }
}

/// 合成器渲染器
pub struct CompositorRenderer {
    entry: Entry,
    instance: Instance,
    device: Device,
    swapchain: vk::SwapchainKHR,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    /// 当前帧索引
    current_frame: usize,
    /// 帧率统计
    fps_counter: FpsCounter,
}

impl CompositorRenderer {
    /// 初始化 Vulkan 渲染器
    pub fn new(config: &VulkanConfig) -> Result<Self, CompositorError> {
        // 创建 Vulkan 实例、设备、交换链...
        todo!("Vulkan 初始化流程")
    }

    /// 渲染主循环
    pub fn render_loop<F>(&mut self, mut frame_callback: F) -> Result<(), CompositorError>
    where
        F: FnMut(&mut FrameContext) -> Result<(), CompositorError>,
    {
        loop {
            let frame_start = std::time::Instant::now();

            // 获取下一帧
            let (image_index, _) = self.acquire_next_image()?;

            // 构建帧上下文
            let mut ctx = FrameContext {
                image_index,
                command_buffer: self.command_buffers[image_index as usize],
                delta_time: frame_start.elapsed(),
            };

            // 执行用户回调（绘制所有图层）
            frame_callback(&mut ctx)?;

            // 提交并呈现
            self.submit_and_present(image_index)?;

            // 帧率控制
            self.fps_counter.tick();
            if self.config.vsync_enabled {
                self.wait_for_vsync();
            }
        }
    }

    /// 获取当前帧率
    pub fn current_fps(&self) -> f64 {
        self.fps_counter.fps()
    }
}

/// 帧渲染上下文
pub struct FrameContext {
    pub image_index: u32,
    pub command_buffer: vk::CommandBuffer,
    pub delta_time: Duration,
}

/// 帧率统计器
pub struct FpsCounter {
    frame_times: Vec<Duration>,
    last_update: std::time::Instant,
    cached_fps: f64,
}

impl FpsCounter {
    pub fn tick(&mut self) {
        let now = std::time::Instant::now();
        self.frame_times.push(now - self.last_update);
        self.last_update = now;
        // 保留最近 60 帧
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }
        self.cached_fps = self.calculate_fps();
    }

    fn calculate_fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let total: Duration = self.frame_times.iter().sum();
        self.frame_times.len() as f64 / total.as_secs_f64()
    }

    pub fn fps(&self) -> f64 {
        self.cached_fps
    }
}
```

### 2.2 交换链管理

```rust
/// 交换链配置
pub struct SwapchainConfig {
    pub width: u32,
    pub height: u32,
    pub format: vk::Format,
    pub color_space: vk::ColorSpaceKHR,
    pub present_mode: vk::PresentModeKHR,
    pub image_count: u32,
}

/// 交换链封装
pub struct Swapchain {
    pub handle: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
}

impl Swapchain {
    /// 创建交换链
    pub fn create(
        device: &Device,
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        config: &SwapchainConfig,
    ) -> Result<Self, CompositorError> {
        let present_mode = if config.present_mode == vk::PresentModeKHR::FIFO {
            // FIFO 模式保证 vsync
            vk::PresentModeKHR::FIFO
        } else {
            // 邮箱模式：低延迟，可能撕裂
            vk::PresentModeKHR::MAILBOX
        };

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(config.image_count)
            .image_format(config.format)
            .image_color_space(config.color_space)
            .image_extent(vk::Extent2D {
                width: config.width,
                height: config.height,
            })
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        let handle = unsafe {
            device
                .ext_swapchain()
                .create_swapchain(&create_info, None)?
        };

        let images = unsafe {
            device
                .ext_swapchain()
                .get_swapchain_images(handle)?
        };

        // 创建图像视图...
        Ok(Self {
            handle,
            images,
            image_views: Vec::new(),
            format: config.format,
            extent: vk::Extent2D {
                width: config.width,
                height: config.height,
            },
        })
    }
}
```

### 2.3 合成器错误处理

```rust
/// 合成器错误类型
#[derive(Debug, thiserror::Error)]
pub enum CompositorError {
    #[error("Vulkan 初始化失败: {0}")]
    VulkanInitFailed(String),

    #[error("交换链创建失败: {0}")]
    SwapchainCreationFailed(String),

    #[error("设备不支持所需的 Vulkan 特性")]
    InsufficientDeviceCapabilities,

    #[error("渲染管线创建失败: {0}")]
    PipelineCreationFailed(String),

    #[error("帧缓冲区获取超时")]
    FramebufferTimeout,

    #[error("着色器编译失败: {0}")]
    ShaderCompilationFailed(String),

    #[error("渲染帧失败: {0}")]
    RenderFrameFailed(String),
}
```

---

## 3. 窗口管理器 (Window Manager)

### 3.1 窗口数据结构

```rust
use std::collections::HashMap;

/// 窗口标识符
pub type WindowId = u64;

/// 窗口状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowState {
    /// 正常浮动
    Normal,
    /// 最小化
    Minimized,
    /// 最大化
    Maximized,
    /// 全屏
    Fullscreen,
    /// 平铺
    Tiled,
}

/// 窗口类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowType {
    /// 普通应用窗口
    Application,
    /// 对话框
    Dialog,
    /// 工具提示
    Tooltip,
    /// 菜单
    Menu,
    /// Dock
    Dock,
    /// 通知
    Notification,
    /// Agent 面板
    AgentPanel,
}

/// 窗口属性
#[derive(Debug, Clone)]
pub struct WindowProperties {
    pub id: WindowId,
    pub title: String,
    pub app_id: String,
    pub window_type: WindowType,
    pub geometry: Rect,
    /// 窗口状态
    pub state: WindowState,
    /// 是否可调整大小
    pub resizable: bool,
    /// 最小尺寸
    pub min_size: Size,
    /// 最大尺寸
    pub max_size: Option<Size>,
    /// 窗口层级（z-order）
    pub z_index: u32,
    /// 是否获得焦点
    pub focused: bool,
    /// 不透明度 (0.0 - 1.0)
    pub opacity: f32,
    /// 所属虚拟桌面
    pub virtual_desktop: u32,
}

/// 矩形区域
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// 尺寸
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// 位置
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

### 3.2 布局管理器

```rust
/// 布局类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    /// 浮动布局（传统桌面）
    Floating,
    /// 平铺布局
    Tiled,
    /// 全屏布局
    Fullscreen,
    /// 分屏布局
    Split,
}

/// 分屏方向
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// 吸附区域
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapZone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    /// 中心（最大化）
    Center,
}

/// 布局管理器 trait
pub trait LayoutManager: Send + Sync {
    /// 计算窗口布局
    fn arrange(&self, windows: &[&WindowProperties], screen: &Rect) -> Vec<Rect>;

    /// 处理窗口吸附
    fn snap_window(&self, window: &mut WindowProperties, zone: SnapZone, screen: &Rect);

    /// 获取布局类型
    fn layout_mode(&self) -> LayoutMode;
}

/// 浮动布局管理器
pub struct FloatingLayoutManager;

impl LayoutManager for FloatingLayoutManager {
    fn arrange(&self, windows: &[&WindowProperties], _screen: &Rect) -> Vec<Rect> {
        windows.iter().map(|w| w.geometry).collect()
    }

    fn snap_window(&self, window: &mut WindowProperties, zone: SnapZone, screen: &Rect) {
        let rect = match zone {
            SnapZone::Left => Rect {
                x: screen.x,
                y: screen.y,
                width: screen.width / 2,
                height: screen.height,
            },
            SnapZone::Right => Rect {
                x: screen.x + (screen.width / 2) as i32,
                y: screen.y,
                width: screen.width / 2,
                height: screen.height,
            },
            SnapZone::Center => *screen,
            _ => window.geometry,
        };
        window.geometry = rect;
    }

    fn layout_mode(&self) -> LayoutMode {
        LayoutMode::Floating
    }
}

/// 分屏布局管理器
pub struct SplitLayoutManager {
    pub direction: SplitDirection,
    pub ratio: f32, // 0.0 - 1.0，左侧/上方占比
}

impl LayoutManager for SplitLayoutManager {
    fn arrange(&self, windows: &[&WindowProperties], screen: &Rect) -> Vec<Rect> {
        if windows.is_empty() {
            return Vec::new();
        }

        match self.direction {
            SplitDirection::Horizontal => {
                let split_x = screen.x + (screen.width as f32 * self.ratio) as i32;
                windows.iter().enumerate().map(|(i, _)| {
                    if i % 2 == 0 {
                        Rect {
                            x: screen.x,
                            y: screen.y,
                            width: (split_x - screen.x) as u32,
                            height: screen.height,
                        }
                    } else {
                        Rect {
                            x: split_x,
                            y: screen.y,
                            width: (screen.x + screen.width as i32 - split_x) as u32,
                            height: screen.height,
                        }
                    }
                }).collect()
            }
            SplitDirection::Vertical => {
                let split_y = screen.y + (screen.height as f32 * self.ratio) as i32;
                windows.iter().enumerate().map(|(i, _)| {
                    if i % 2 == 0 {
                        Rect {
                            x: screen.x,
                            y: screen.y,
                            width: screen.width,
                            height: (split_y - screen.y) as u32,
                        }
                    } else {
                        Rect {
                            x: screen.x,
                            y: split_y,
                            width: screen.width,
                            height: (screen.y + screen.height as i32 - split_y) as u32,
                        }
                    }
                }).collect()
            }
        }
    }

    fn snap_window(&self, window: &mut WindowProperties, zone: SnapZone, screen: &Rect) {
        FloatingLayoutManager.snap_window(window, zone, screen)
    }

    fn layout_mode(&self) -> LayoutMode {
        LayoutMode::Split
    }
}
```

### 3.3 虚拟桌面

```rust
/// 虚拟桌面管理器
pub struct VirtualDesktopManager {
    /// 当前活动桌面索引
    current: u32,
    /// 桌面总数
    count: u32,
    /// 每个桌面上的窗口
    desktops: HashMap<u32, Vec<WindowId>>,
}

impl VirtualDesktopManager {
    pub fn new(count: u32) -> Self {
        let mut desktops = HashMap::new();
        for i in 0..count {
            desktops.insert(i, Vec::new());
        }
        Self {
            current: 0,
            count,
            desktops,
        }
    }

    /// 切换到指定桌面
    pub fn switch_to(&mut self, index: u32) -> Result<(), WmError> {
        if index >= self.count {
            return Err(WmError::InvalidDesktopIndex(index));
        }
        self.current = index;
        Ok(())
    }

    /// 将窗口移动到指定桌面
    pub fn move_window(&mut self, window_id: WindowId, desktop: u32) -> Result<(), WmError> {
        if desktop >= self.count {
            return Err(WmError::InvalidDesktopIndex(desktop));
        }
        // 从当前桌面移除
        for windows in self.desktops.values_mut() {
            windows.retain(|&id| id != window_id);
        }
        // 添加到目标桌面
        self.desktops
            .entry(desktop)
            .or_default()
            .push(window_id);
        Ok(())
    }

    /// 获取当前桌面的窗口列表
    pub fn current_windows(&self) -> &[WindowId] {
        self.desktops.get(&self.current).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WmError {
    #[error("无效的桌面索引: {0}")]
    InvalidDesktopIndex(u32),
    #[error("窗口不存在: {0}")]
    WindowNotFound(WindowId),
    #[error("布局切换失败: {0}")]
    LayoutSwitchFailed(String),
}
```

---

## 4. Dock 栏

### 4.1 Dock 数据结构

```rust
/// Dock 图标项
#[derive(Debug, Clone)]
pub struct DockItem {
    /// 唯一标识
    pub id: String,
    /// 应用 ID
    pub app_id: String,
    /// 显示名称
    pub label: String,
    /// 图标路径
    pub icon_path: String,
    /// 是否正在运行
    pub running: bool,
    /// 是否有未读通知
    pub has_notification: bool,
    /// 弹跳动画状态
    pub bounce_state: BounceState,
    /// 上下文菜单
    pub context_menu: Option<ContextMenu>,
}

/// 弹跳动画状态（弹簧物理）
#[derive(Debug, Clone)]
pub struct BounceState {
    /// 是否正在弹跳
    pub active: bool,
    /// 当前位移
    pub displacement: f32,
    /// 当前速度
    pub velocity: f32,
    /// 弹簧刚度系数
    pub stiffness: f32,
    /// 阻尼系数
    pub damping: f32,
    /// 弹跳次数
    pub bounce_count: u32,
}

/// 弹簧物理参数
impl BounceState {
    pub fn new() -> Self {
        Self {
            active: false,
            displacement: 0.0,
            velocity: -20.0, // 初始弹跳速度
            stiffness: 300.0,
            damping: 15.0,
            bounce_count: 0,
        }
    }

    /// 更新弹簧物理状态
    pub fn update(&mut self, dt: f64) {
        if !self.active {
            return;
        }
        // 弹簧力: F = -kx - cv
        let spring_force = -self.stiffness * self.displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) as f64;

        self.velocity += (acceleration * dt) as f32;
        self.displacement += self.velocity * dt as f32;

        // 检查弹跳是否结束
        if self.displacement.abs() < 0.1 && self.velocity.abs() < 0.5 {
            self.active = false;
            self.displacement = 0.0;
            self.velocity = 0.0;
        }
    }
}

/// 上下文菜单
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub items: Vec<MenuItem>,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub action: MenuAction,
    pub separator_after: bool,
}

#[derive(Debug, Clone)]
pub enum MenuAction {
    /// 启动应用
    Launch,
    /// 退出应用
    Quit,
    /// 打开新窗口
    NewWindow,
    /// 固定/取消固定
    TogglePin,
    /// 自定义动作
    Custom(String),
    /// 子菜单
    Submenu(Vec<MenuItem>),
}
```

### 4.2 Dock 管理器

```rust
/// Dock 管理器
pub struct DockManager {
    items: Vec<DockItem>,
    /// Dock 位置
    position: DockPosition,
    /// 图标大小
    icon_size: u32,
    /// 拖拽状态
    drag_state: Option<DragState>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockPosition {
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct DragState {
    pub item_id: String,
    pub source_index: usize,
    pub current_position: Point,
}

impl DockManager {
    pub fn new(position: DockPosition) -> Self {
        Self {
            items: Vec::new(),
            position,
            icon_size: 48,
            drag_state: None,
        }
    }

    /// 添加 Dock 项
    pub fn add_item(&mut self, item: DockItem) {
        self.items.push(item);
    }

    /// 移除 Dock 项
    pub fn remove_item(&mut self, id: &str) -> Option<DockItem> {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// 触发弹跳动画
    pub fn start_bounce(&mut self, item_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == item_id) {
            item.bounce_state = BounceState::new();
            item.bounce_state.active = true;
        }
    }

    /// 处理拖拽开始
    pub fn start_drag(&mut self, item_id: &str, position: Point) -> Result<(), DockError> {
        let index = self.items.iter().position(|i| i.id == item_id)
            .ok_or(DockError::ItemNotFound(item_id.to_string()))?;
        self.drag_state = Some(DragState {
            item_id: item_id.to_string(),
            source_index: index,
            current_position: position,
        });
        Ok(())
    }

    /// 处理拖拽结束（重新排序）
    pub fn end_drag(&mut self, target_index: usize) {
        if let Some(drag) = self.drag_state.take() {
            if drag.source_index != target_index {
                let item = self.items.remove(drag.source_index);
                let adjusted_index = if target_index > drag.source_index {
                    target_index - 1
                } else {
                    target_index
                };
                self.items.insert(adjusted_index.min(self.items.len()), item);
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DockError {
    #[error("Dock 项不存在: {0}")]
    ItemNotFound(String),
    #[error("拖拽操作无效")]
    InvalidDragOperation,
}
```

---

## 5. 菜单栏 (Menu Bar)

### 5.1 全局菜单栏

```rust
/// 全局菜单栏
pub struct MenuBar {
    /// 应用菜单区域
    app_menus: Vec<AppMenu>,
    /// 系统托盘区域
    system_tray: Vec<TrayItem>,
    /// Agent 菜单项
    agent_menu: AgentMenuEntry,
}

/// 应用菜单
#[derive(Debug, Clone)]
pub struct AppMenu {
    pub app_id: String,
    pub label: String,
    pub items: Vec<MenuItem>,
}

/// 系统托盘项
#[derive(Debug, Clone)]
pub struct TrayItem {
    pub id: String,
    pub icon: String,
    pub tooltip: String,
    pub menu: Option<ContextMenu>,
}

/// Agent 菜单入口
#[derive(Debug, Clone)]
pub struct AgentMenuEntry {
    pub label: String,
    pub icon: String,
    pub items: Vec<MenuItem>,
}

impl MenuBar {
    /// 设置当前应用菜单
    pub fn set_app_menu(&mut self, app_id: &str, menu: AppMenu) {
        // 移除旧菜单，添加新菜单
        self.app_menus.retain(|m| m.app_id != app_id);
        self.app_menus.push(menu);
    }

    /// 添加系统托盘项
    pub fn add_tray_item(&mut self, item: TrayItem) {
        self.system_tray.push(item);
    }

    /// 显示下拉菜单
    pub fn show_dropdown(&self, menu_id: &str, position: Point) -> DropdownMenu {
        // 查找菜单并构建下拉菜单
        DropdownMenu {
            menu_id: menu_id.to_string(),
            position,
            items: Vec::new(), // 从 app_menus 或 agent_menu 中查找
        }
    }
}

/// 下拉菜单
pub struct DropdownMenu {
    pub menu_id: String,
    pub position: Point,
    pub items: Vec<MenuItem>,
}
```

---

## 6. Agent Bar

### 6.1 Agent 交互栏

```rust
/// Agent Bar 状态
pub struct AgentBar {
    /// 是否可见
    pub visible: bool,
    /// 输入框内容
    pub input_text: String,
    /// 流式响应
    pub streaming_response: Option<StreamingResponse>,
    /// 当前对话历史
    pub conversation: Vec<AgentMessage>,
    /// 当前激活的 Agent
    pub active_agent: Option<String>,
}

/// 流式响应
#[derive(Debug, Clone)]
pub struct StreamingResponse {
    /// 已接收的文本片段
    pub chunks: Vec<String>,
    /// 是否完成
    pub completed: bool,
    /// 响应 ID
    pub response_id: String,
    /// 开始时间
    pub started_at: std::time::Instant,
}

/// Agent 消息
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: std::time::Instant,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageRole {
    User,
    Agent,
    System,
}

impl AgentBar {
    pub fn new() -> Self {
        Self {
            visible: false,
            input_text: String::new(),
            streaming_response: None,
            conversation: Vec::new(),
            active_agent: None,
        }
    }

    /// 显示 Agent Bar
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// 处理用户输入
    pub fn process_input(&mut self, text: String) -> Result<(), AgentBarError> {
        self.input_text = text.clone();
        let message = AgentMessage {
            role: MessageRole::User,
            content: text,
            timestamp: std::time::Instant::now(),
            agent_id: self.active_agent.clone(),
        };
        self.conversation.push(message);
        Ok(())
    }

    /// 追加流式响应片段
    pub fn append_stream_chunk(&mut self, chunk: &str) {
        if let Some(ref mut response) = self.streaming_response {
            response.chunks.push(chunk.to_string());
        } else {
            self.streaming_response = Some(StreamingResponse {
                chunks: vec![chunk.to_string()],
                completed: false,
                response_id: uuid::Uuid::new_v4().to_string(),
                started_at: std::time::Instant::now(),
            });
        }
    }

    /// 完成流式响应
    pub fn complete_stream(&mut self) {
        if let Some(ref mut response) = self.streaming_response {
            response.completed = true;
            let full_text: String = response.chunks.join("");
            self.conversation.push(AgentMessage {
                role: MessageRole::Agent,
                content: full_text,
                timestamp: std::time::Instant::now(),
                agent_id: self.active_agent.clone(),
            });
        }
        self.streaming_response = None;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentBarError {
    #[error("没有激活的 Agent")]
    NoActiveAgent,
    #[error("输入为空")]
    EmptyInput,
    #[error("Agent 通信失败: {0}")]
    AgentCommunicationFailed(String),
}
```

---

## 7. Agent Spotlight

### 7.1 全局搜索与 Agent 动作执行

```rust
/// Spotlight 搜索结果
#[derive(Debug, Clone)]
pub struct SpotlightResult {
    /// 结果标识
    pub id: String,
    /// 显示标题
    pub title: String,
    /// 副标题/描述
    pub subtitle: String,
    /// 图标
    pub icon: Option<String>,
    /// 结果类别
    pub category: SpotlightCategory,
    /// 匹配分数
    pub score: f32,
    /// 关联的动作
    pub action: SpotlightAction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpotlightCategory {
    Application,
    File,
    AgentAction,
    SystemSetting,
    WebSearch,
    Calculator,
}

#[derive(Debug, Clone)]
pub enum SpotlightAction {
    /// 启动应用
    LaunchApp(String),
    /// 打开文件
    OpenFile(String),
    /// 执行 Agent 动作
    ExecuteAgentAction(String),
    /// 打开设置
    OpenSetting(String),
    /// 复制文本到剪贴板
    CopyToClipboard(String),
}

/// Spotlight 管理器
pub struct SpotlightManager {
    /// 搜索提供者
    providers: Vec<Box<dyn SpotlightProvider>>,
    /// 搜索结果缓存
    cache: HashMap<String, Vec<SpotlightResult>>,
    /// 最大缓存条目
    max_cache_entries: usize,
}

/// 搜索提供者 trait
pub trait SpotlightProvider: Send + Sync {
    /// 搜索
    fn search(&self, query: &str, limit: usize) -> Vec<SpotlightResult>;
    /// 提供者名称
    fn name(&self) -> &str;
    /// 优先级
    fn priority(&self) -> u32;
}

impl SpotlightManager {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            cache: HashMap::new(),
            max_cache_entries: 1000,
        }
    }

    /// 注册搜索提供者
    pub fn register_provider(&mut self, provider: Box<dyn SpotlightProvider>) {
        self.providers.push(provider);
        // 按优先级排序
        self.providers.sort_by_key(|p| std::cmp::Reverse(p.priority()));
    }

    /// 执行搜索（必须在 50ms 内返回）
    pub fn search(&mut self, query: &str) -> Vec<SpotlightResult> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);

        // 检查缓存
        if let Some(cached) = self.cache.get(query) {
            return cached.clone();
        }

        let mut all_results: Vec<SpotlightResult> = self
            .providers
            .iter()
            .flat_map(|p| p.search(query, 10))
            .collect();

        // 按分数排序
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // 截取前 N 个结果
        all_results.truncate(10);

        // 缓存结果
        if self.cache.len() >= self.max_cache_entries {
            // 移除最早的缓存条目
            if let Some(first_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&first_key);
            }
        }
        self.cache.insert(query.to_string(), all_results.clone());

        // 性能检查
        if std::time::Instant::now() > deadline {
            log::warn!("Spotlight 搜索超时: query={}", query);
        }

        all_results
    }

    /// 执行 Spotlight 动作
    pub fn execute_action(&self, action: &SpotlightAction) -> Result<(), SpotlightError> {
        match action {
            SpotlightAction::LaunchApp(app_id) => {
                // 启动应用...
                Ok(())
            }
            SpotlightAction::OpenFile(path) => {
                // 打开文件...
                Ok(())
            }
            SpotlightAction::ExecuteAgentAction(action_id) => {
                // 通过 Agent API 执行动作
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SpotlightError {
    #[error("搜索超时")]
    SearchTimeout,
    #[error("动作执行失败: {0}")]
    ActionExecutionFailed(String),
}
```

---

## 8. 通知中心

### 8.1 通知系统

```rust
/// 通知
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: String,
    pub app_id: String,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub urgency: NotificationUrgency,
    pub actions: Vec<NotificationAction>,
    pub timestamp: std::time::Instant,
    pub expires_at: Option<std::time::Instant>,
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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

/// 通知中心
pub struct NotificationCenter {
    notifications: Vec<Notification>,
    /// 通知分组
    groups: HashMap<String, Vec<String>>,
    /// 最大通知数量
    max_notifications: usize,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            groups: HashMap::new(),
            max_notifications: 100,
        }
    }

    /// 推送通知
    pub fn push(&mut self, notification: Notification) -> Result<(), NotificationError> {
        if self.notifications.len() >= self.max_notifications {
            // 移除最旧的通知
            self.notifications.remove(0);
        }

        // 处理分组
        if let Some(ref group_id) = notification.group_id {
            self.groups
                .entry(group_id.clone())
                .or_default()
                .push(notification.id.clone());
        }

        self.notifications.push(notification);
        Ok(())
    }

    /// 关闭通知
    pub fn dismiss(&mut self, id: &str) {
        self.notifications.retain(|n| n.id != id);
        // 清理分组引用
        for group in self.groups.values_mut() {
            group.retain(|nid| nid != id);
        }
    }

    /// 获取分组通知
    pub fn get_group(&self, group_id: &str) -> Vec<&Notification> {
        self.groups
            .get(group_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.notifications.iter().find(|n| n.id == *id))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("通知数量已达上限")]
    NotificationLimitReached,
    #[error("无效的通知 ID: {0}")]
    InvalidNotificationId(String),
}
```

---

## 9. 手势支持

### 9.1 触控板手势

```rust
/// 手势类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureType {
    /// 滑动
    Swipe,
    /// 捏合缩放
    Pinch,
    /// 旋转
    Rotate,
    /// 边缘滑动
    EdgeSwipe,
}

/// 手势方向
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// 手指数量
    pub finger_count: u32,
    /// 累计位移（像素）
    pub delta_x: f32,
    pub delta_y: f32,
    /// 缩放因子（仅 Pinch）
    pub scale: f32,
    /// 旋转角度（仅 Rotate）
    pub rotation: f32,
    /// 时间戳
    pub timestamp: std::time::Instant,
}

/// 手势处理器 trait
pub trait GestureHandler: Send + Sync {
    /// 处理手势开始
    fn on_gesture_begin(&mut self, event: &GestureEvent);

    /// 处理手势更新
    fn on_gesture_update(&mut self, event: &GestureEvent);

    /// 处理手势结束
    fn on_gesture_end(&mut self, event: &GestureEvent);
}

/// 手势管理器
pub struct GestureManager {
    handlers: HashMap<GestureType, Vec<Box<dyn GestureHandler>>>,
    /// 最小识别阈值
    swipe_threshold: f32,
    pinch_threshold: f32,
    rotate_threshold: f32,
}

impl GestureManager {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            swipe_threshold: 50.0,
            pinch_threshold: 0.1,
            rotate_threshold: 5.0,
        }
    }

    /// 注册手势处理器
    pub fn register_handler(
        &mut self,
        gesture_type: GestureType,
        handler: Box<dyn GestureHandler>,
    ) {
        self.handlers
            .entry(gesture_type)
            .or_default()
            .push(handler);
    }

    /// 分发手势事件
    pub fn dispatch(&mut self, event: &GestureEvent) {
        if let Some(handlers) = self.handlers.get_mut(&event.gesture_type) {
            for handler in handlers.iter_mut() {
                handler.on_gesture_update(event);
            }
        }
    }
}
```

---

## 10. 主题引擎

### 10.1 设计令牌系统

```rust
/// 主题设计令牌
#[derive(Debug, Clone)]
pub struct DesignTokens {
    /// 颜色令牌
    pub colors: ColorTokens,
    /// 排版令牌
    pub typography: TypographyTokens,
    /// 间距令牌
    pub spacing: SpacingTokens,
    /// 圆角令牌
    pub corner_radius: CornerRadiusTokens,
    /// 模糊效果
    pub blur: BlurTokens,
    /// 动画令牌
    pub animation: AnimationTokens,
}

/// 颜色令牌
#[derive(Debug, Clone)]
pub struct ColorTokens {
    pub primary: Color,
    pub secondary: Color,
    pub background: Color,
    pub surface: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

/// 颜色（RGBA）
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 转换为十六进制字符串
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// 转换为 RGBA f32 元组
    pub fn to_rgba_f32(&self) -> (f32, f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a,
        )
    }
}

/// 排版令牌
#[derive(Debug, Clone)]
pub struct TypographyTokens {
    pub font_family: String,
    pub font_size_base: f32,
    pub font_size_large: f32,
    pub font_size_small: f32,
    pub font_weight_normal: u32,
    pub font_weight_bold: u32,
    pub line_height: f32,
    pub letter_spacing: f32,
}

/// 间距令牌
#[derive(Debug, Clone)]
pub struct SpacingTokens {
    pub xs: f32,   // 4px
    pub sm: f32,   // 8px
    pub md: f32,   // 16px
    pub lg: f32,   // 24px
    pub xl: f32,   // 32px
    pub xxl: f32,  // 48px
}

/// 圆角令牌
#[derive(Debug, Clone)]
pub struct CornerRadiusTokens {
    pub none: f32,
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub full: f32, // 圆形
}

/// 模糊令牌
#[derive(Debug, Clone)]
pub struct BlurTokens {
    pub none: f32,
    pub light: f32,
    pub medium: f32,
    pub heavy: f32,
}

/// 动画令牌
#[derive(Debug, Clone)]
pub struct AnimationTokens {
    pub duration_fast: Duration,
    pub duration_normal: Duration,
    pub duration_slow: Duration,
    pub easing_default: EasingFunction,
    pub easing_decelerate: EasingFunction,
    pub easing_spring: SpringConfig,
}

#[derive(Debug, Clone, Copy)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

#[derive(Debug, Clone, Copy)]
pub struct SpringConfig {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

/// 主题管理器
pub struct ThemeManager {
    current_theme: DesignTokens,
    themes: HashMap<String, DesignTokens>,
}

impl ThemeManager {
    /// 应用主题
    pub fn apply_theme(&mut self, name: &str) -> Result<(), ThemeError> {
        let theme = self.themes.get(name)
            .ok_or_else(|| ThemeError::ThemeNotFound(name.to_string()))?
            .clone();
        self.current_theme = theme;
        Ok(())
    }

    /// 设置强调色
    pub fn set_accent_color(&mut self, color: Color) {
        self.current_theme.colors.accent = color;
    }

    /// 获取当前设计令牌
    pub fn get_tokens(&self) -> &DesignTokens {
        &self.current_theme
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("主题不存在: {0}")]
    ThemeNotFound(String),
    #[error("主题解析失败: {0}")]
    ThemeParseError(String),
}
```

---

## 11. 文本渲染

### 11.1 cosmic-text + swash 集成

```rust
use cosmic_text::{Attrs, AttrsList, Buffer, FontSystem, Metrics, Shaping};

/// 文本渲染器
pub struct TextRenderer {
    font_system: FontSystem,
    metrics: Metrics,
}

impl TextRenderer {
    pub fn new(font_size: f32, line_height: f32) -> Self {
        let font_system = FontSystem::new();
        let metrics = Metrics::new(font_size, line_height);
        Self { font_system, metrics }
    }

    /// 加载字体
    pub fn load_font(&mut self, font_data: &[u8]) -> Result<(), TextRenderError> {
        self.font_system
            .load_font_data(cosmic_text::fontdb::Source::Binary(
                std::sync::Arc::new(font_data.to_vec()),
            ));
        Ok(())
    }

    /// 渲染文本到字形缓冲区
    pub fn shape_text(
        &mut self,
        text: &str,
        attrs: Attrs,
    ) -> Result<TextShapeResult, TextRenderError> {
        let mut buffer = Buffer::new(&mut self.font_system, self.metrics);
        buffer.set_size(None, None);
        buffer.set_text(text, AttrsList::new(attrs), Shaping::Advanced);
        buffer.shape_until_scroll();

        // 使用 swash 进行字形光栅化
        let glyphs = self.rasterize_glyphs(&buffer)?;

        Ok(TextShapeResult {
            width: buffer.size().0,
            height: buffer.size().1,
            glyphs,
            lines: buffer.lines.len(),
        })
    }

    /// 使用 swash 光栅化字形
    fn rasterize_glyphs(
        &mut self,
        buffer: &Buffer,
    ) -> Result<Vec<RasterizedGlyph>, TextRenderError> {
        let mut glyphs = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let glyph_id = glyph.glyph_id;
                let font_size = glyph.font_size;
                let x = glyph.x;
                let y = glyph.y;

                // swash 光栅化
                // (实际实现需要 swash::CacheKey 和 swash::Rasterizer)
                glyphs.push(RasterizedGlyph {
                    glyph_id,
                    x: x as i32,
                    y: y as i32,
                    width: 0,
                    height: 0,
                    bitmap: Vec::new(),
                });
            }
        }
        Ok(glyphs)
    }
}

/// 文本形状结果
pub struct TextShapeResult {
    pub width: f32,
    pub height: f32,
    pub glyphs: Vec<RasterizedGlyph>,
    pub lines: usize,
}

/// 光栅化字形
pub struct RasterizedGlyph {
    pub glyph_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum TextRenderError {
    #[error("字体加载失败: {0}")]
    FontLoadFailed(String),
    #[error("文本形状计算失败: {0}")]
    ShapingFailed(String),
    #[error("字形光栅化失败: {0}")]
    RasterizationFailed(String),
}
```

---

## 12. 输入处理

### 12.1 libinput 集成

```rust
/// 输入事件
#[derive(Debug, Clone)]
pub enum InputEvent {
    Keyboard(KeyboardEvent),
    Pointer(PointerEvent),
    Touch(TouchEvent),
    Gesture(GestureEvent),
}

#[derive(Debug, Clone)]
pub struct KeyboardEvent {
    pub keycode: u32,
    pub keysym: u32,
    pub modifiers: ModifierState,
    pub pressed: bool,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ModifierState {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub caps_lock: bool,
}

#[derive(Debug, Clone)]
pub struct PointerEvent {
    pub x: f64,
    pub y: f64,
    pub absolute: bool,
    pub button: Option<PointerButton>,
    pub button_state: PointerButtonState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerButtonState {
    Pressed,
    Released,
}

/// 键盘快捷键
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct KeyBinding {
    pub modifiers: ModifierState,
    pub keysym: u32,
    pub action: KeyAction,
}

#[derive(Debug, Clone)]
pub enum KeyAction {
    /// 显示 Spotlight (Cmd+Space)
    ShowSpotlight,
    /// 关闭窗口 (Cmd+W)
    CloseWindow,
    /// 退出应用 (Cmd+Q)
    QuitApp,
    /// 切换全屏 (Cmd+F)
    ToggleFullscreen,
    /// 显示 Agent Bar (Cmd+A)
    ShowAgentBar,
    /// 切换虚拟桌面 (Ctrl+Left/Right)
    SwitchDesktop(i32),
    /// 自定义动作
    Custom(String),
}

/// 输入管理器
pub struct InputManager {
    /// 键盘快捷键映射
    key_bindings: HashMap<(ModifierState, u32), KeyBinding>,
    /// 事件回调
    event_handlers: Vec<Box<dyn Fn(&InputEvent) + Send + Sync>>,
}

impl InputManager {
    pub fn new() -> Self {
        let mut manager = Self {
            key_bindings: HashMap::new(),
            event_handlers: Vec::new(),
        };
        manager.register_default_bindings();
        manager
    }

    /// 注册默认快捷键
    fn register_default_bindings(&mut self) {
        // Cmd+Space -> Spotlight
        self.register_binding(KeyBinding {
            modifiers: ModifierState {
                super_key: true, ..Default::default()
            },
            keysym: 0x0020, // Space
            action: KeyAction::ShowSpotlight,
        });
        // Cmd+Q -> 退出
        self.register_binding(KeyBinding {
            modifiers: ModifierState {
                super_key: true, ..Default::default()
            },
            keysym: 0x0071, // Q
            action: KeyAction::QuitApp,
        });
    }

    /// 注册快捷键
    pub fn register_binding(&mut self, binding: KeyBinding) {
        self.key_bindings
            .insert((binding.modifiers, binding.keysym), binding);
    }

    /// 处理输入事件
    pub fn handle_event(&self, event: &InputEvent) -> Option<KeyAction> {
        if let InputEvent::Keyboard(ref ke) = event {
            if ke.pressed {
                let mods = ke.modifiers;
                return self.key_bindings
                    .get(&(mods, ke.keysym))
                    .map(|b| b.action.clone());
            }
        }
        None
    }
}

impl Default for ModifierState {
    fn default() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
            caps_lock: false,
        }
    }
}
```

---

## 13. 开发路径

### 13.1 winit 原型 → Smithay 生产后端

```
阶段 1: winit 原型 (MVP)
├── 使用 winit 创建窗口和事件循环
├── ash 初始化 Vulkan 渲染
├── 基础窗口管理（浮动布局）
├── 简单 Dock 和菜单栏
└── Agent Bar 原型

阶段 2: 功能完善
├── Smithay Wayland 合成器后端
├── 完整布局管理（浮动/平铺/分屏）
├── 虚拟桌面
├── 手势支持
└── 主题引擎

阶段 3: 生产就绪
├── 性能优化（≥60fps @ 4K）
├── 无障碍支持
├── 多显示器支持
└── 安全沙箱
```

### 13.2 QEMU 兼容性

```rust
/// Vulkan 后端选择
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VulkanBackend {
    /// 硬件 GPU
    Hardware,
    /// lavapipe 软件 Vulkan（QEMU 环境）
    Lavapipe,
    /// 自动检测
    Auto,
}

/// 检测最佳 Vulkan 后端
pub fn detect_vulkan_backend() -> VulkanBackend {
    // 检查是否在 QEMU 环境中运行
    let is_qemu = std::fs::exists("/sys/class/drm/card0/device/uevent")
        .map(|exists| {
            if exists {
                std::fs::read_to_string("/sys/class/drm/card0/device/uevent")
                    .map(|s| s.contains("qemu") || s.contains("virtio"))
                    .unwrap_or(false)
            } else {
                false
            }
        })
        .unwrap_or(false);

    if is_qemu {
        log::info!("检测到 QEMU 环境，使用 lavapipe 软件 Vulkan");
        VulkanBackend::Lavapipe
    } else {
        VulkanBackend::Hardware
    }
}
```

---

## 14. 性能约束

| 组件 | 指标 | 目标值 | 测量方法 |
|------|------|--------|----------|
| 合成器 | 帧率 | ≥60fps | FpsCounter 连续 60 帧平均 |
| 合成器 | 帧时间 | ≤16.67ms | 单帧渲染耗时 |
| Spotlight | 搜索延迟 | <50ms | 从输入到结果返回 |
| 窗口动画 | 打开动画 | 200ms | 从触发到完成 |
| 窗口动画 | 关闭动画 | 150ms | 从触发到完成 |
| Dock 弹跳 | 弹簧动画 | 500ms | 自然衰减 |
| 主题切换 | 热切换延迟 | <100ms | 令牌替换到首帧渲染 |
| 文本渲染 | 首屏渲染 | <16ms | 1000 字符文本 |
| 手势响应 | 输入延迟 | <16ms | 事件到视觉反馈 |
| 通知显示 | 弹出动画 | 300ms | 从触发到完全显示 |

---

## 15. 测试用例

### 15.1 合成器测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_counter_accuracy() {
        let mut counter = FpsCounter::new();
        // 模拟 60fps
        for _ in 0..60 {
            counter.tick();
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        let fps = counter.fps();
        assert!(fps > 55.0 && fps < 65.0, "FPS 应在 55-65 之间，实际: {}", fps);
    }

    #[test]
    fn test_vulkan_config_default() {
        let config = VulkanConfig::default();
        assert_eq!(config.swapchain_image_count, 2);
        assert!(config.vsync_enabled);
        assert_eq!(config.target_fps, 60);
    }

    #[test]
    fn test_swapchain_present_mode_vsync() {
        let config = SwapchainConfig {
            present_mode: vk::PresentModeKHR::FIFO,
            ..Default::default()
        };
        assert_eq!(config.present_mode, vk::PresentModeKHR::FIFO);
    }
}
```

### 15.2 窗口管理器测试

```rust
#[test]
fn test_floating_layout_snap_left() {
    let manager = FloatingLayoutManager;
    let screen = Rect { x: 0, y: 0, width: 1920, height: 1080 };
    let mut window = WindowProperties {
        id: 1,
        geometry: Rect { x: 100, y: 100, width: 800, height: 600 },
        ..Default::default()
    };
    manager.snap_window(&mut window, SnapZone::Left, &screen);
    assert_eq!(window.geometry.width, 960);
    assert_eq!(window.geometry.x, 0);
}

#[test]
fn test_split_layout_horizontal() {
    let manager = SplitLayoutManager {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
    };
    let screen = Rect { x: 0, y: 0, width: 1920, height: 1080 };
    let w1 = WindowProperties { id: 1, ..Default::default() };
    let w2 = WindowProperties { id: 2, ..Default::default() };
    let rects = manager.arrange(&[&w1, &w2], &screen);
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].width, 960);
    assert_eq!(rects[1].x, 960);
}

#[test]
fn test_virtual_desktop_switch() {
    let mut vdm = VirtualDesktopManager::new(4);
    assert_eq!(vdm.current, 0);
    vdm.switch_to(2).unwrap();
    assert_eq!(vdm.current, 2);
    assert!(vdm.switch_to(5).is_err());
}

#[test]
fn test_virtual_desktop_move_window() {
    let mut vdm = VirtualDesktopManager::new(4);
    vdm.move_window(42, 1).unwrap();
    assert!(vdm.current_windows().is_empty());
    vdm.switch_to(1).unwrap();
    assert!(vdm.current_windows().contains(&42));
}
```

### 15.3 Dock 测试

```rust
#[test]
fn test_dock_bounce_physics() {
    let mut state = BounceState::new();
    state.active = true;
    let dt = 1.0 / 60.0;
    for _ in 0..300 {
        state.update(dt);
        if !state.active {
            break;
        }
    }
    assert!(!state.active, "弹跳应在有限时间内结束");
    assert!(state.displacement.abs() < 0.2, "最终位移应接近零");
}

#[test]
fn test_dock_add_remove() {
    let mut dock = DockManager::new(DockPosition::Bottom);
    let item = DockItem {
        id: "test-app".to_string(),
        app_id: "com.test.app".to_string(),
        label: "Test App".to_string(),
        icon_path: "/icons/test.svg".to_string(),
        running: false,
        has_notification: false,
        bounce_state: BounceState::new(),
        context_menu: None,
    };
    dock.add_item(item.clone());
    assert_eq!(dock.items.len(), 1);
    dock.remove_item("test-app");
    assert!(dock.items.is_empty());
}

#[test]
fn test_dock_drag_reorder() {
    let mut dock = DockManager::new(DockPosition::Bottom);
    for i in 0..3 {
        dock.add_item(DockItem {
            id: format!("item-{}", i),
            app_id: format!("com.app.{}", i),
            label: format!("App {}", i),
            icon_path: String::new(),
            running: false,
            has_notification: false,
            bounce_state: BounceState::new(),
            context_menu: None,
        });
    }
    dock.start_drag("item-0", Point { x: 0, y: 0 }).unwrap();
    dock.end_drag(2);
    assert_eq!(dock.items[2].id, "item-0");
}
```

### 15.4 Spotlight 测试

```rust
#[test]
fn test_spotlight_search_performance() {
    struct MockProvider;
    impl SpotlightProvider for MockProvider {
        fn search(&self, query: &str, limit: usize) -> Vec<SpotlightResult> {
            (0..limit).map(|i| SpotlightResult {
                id: format!("result-{}", i),
                title: format!("{} - Result {}", query, i),
                subtitle: String::new(),
                icon: None,
                category: SpotlightCategory::Application,
                score: 1.0 - (i as f32) * 0.1,
                action: SpotlightAction::LaunchApp(format!("app-{}", i)),
            }).collect()
        }
        fn name(&self) -> &str { "mock" }
        fn priority(&self) -> u32 { 0 }
    }

    let mut spotlight = SpotlightManager::new();
    spotlight.register_provider(Box::new(MockProvider));

    let start = std::time::Instant::now();
    let results = spotlight.search("test query");
    let elapsed = start.elapsed();

    assert!(results.len() <= 10);
    assert!(elapsed.as_millis() < 50, "搜索应在 50ms 内完成，实际: {}ms", elapsed.as_millis());
}
```

### 15.5 通知中心测试

```rust
#[test]
fn test_notification_push_and_dismiss() {
    let mut center = NotificationCenter::new();
    let notification = Notification {
        id: "notif-1".to_string(),
        app_id: "com.test.app".to_string(),
        title: "测试通知".to_string(),
        body: "这是一条测试通知".to_string(),
        icon: None,
        urgency: NotificationUrgency::Normal,
        actions: Vec::new(),
        timestamp: std::time::Instant::now(),
        expires_at: None,
        group_id: None,
    };
    center.push(notification).unwrap();
    assert_eq!(center.notifications.len(), 1);
    center.dismiss("notif-1");
    assert!(center.notifications.is_empty());
}

#[test]
fn test_notification_grouping() {
    let mut center = NotificationCenter::new();
    for i in 0..3 {
        center.push(Notification {
            id: format!("notif-{}", i),
            group_id: Some("email".to_string()),
            ..Default::default()
        }).unwrap();
    }
    let group = center.get_group("email");
    assert_eq!(group.len(), 3);
}
```

### 15.6 主题引擎测试

```rust
#[test]
fn test_color_conversion() {
    let color = Color::new(255, 128, 0, 1.0);
    assert_eq!(color.to_hex(), "#ff8000");
    let (r, g, b, a) = color.to_rgba_f32();
    assert!((r - 1.0).abs() < 0.01);
    assert!((g - 0.502).abs() < 0.01);
    assert!((b - 0.0).abs() < 0.01);
    assert!((a - 1.0).abs() < 0.01);
}

#[test]
fn test_theme_accent_color() {
    let mut manager = ThemeManager::new();
    let new_accent = Color::new(0, 122, 255, 1.0);
    manager.set_accent_color(new_accent);
    assert_eq!(manager.get_tokens().colors.accent, new_accent);
}
```

### 15.7 手势测试

```rust
#[test]
fn test_gesture_dispatch() {
    struct TestHandler {
        events: Vec<GestureEvent>,
    }
    impl GestureHandler for TestHandler {
        fn on_gesture_begin(&mut self, event: &GestureEvent) { self.events.push(event.clone()); }
        fn on_gesture_update(&mut self, event: &GestureEvent) { self.events.push(event.clone()); }
        fn on_gesture_end(&mut self, event: &GestureEvent) { self.events.push(event.clone()); }
    }

    let mut manager = GestureManager::new();
    manager.register_handler(GestureType::Swipe, Box::new(TestHandler { events: Vec::new() }));
    let event = GestureEvent {
        gesture_type: GestureType::Swipe,
        direction: Some(GestureDirection::Left),
        finger_count: 3,
        delta_x: -100.0,
        delta_y: 0.0,
        scale: 1.0,
        rotation: 0.0,
        timestamp: std::time::Instant::now(),
    };
    manager.dispatch(&event);
    assert_eq!(manager.handlers.get(&GestureType::Swipe).unwrap().len(), 1);
}
```

---

## 16. 依赖关系

| Crate | 版本 | 用途 |
|-------|------|------|
| `ash` | 0.38+ | Vulkan 绑定 |
| `winit` | 0.30+ | 原型窗口管理 |
| `smithay` | 0.8+ | Wayland 合成器 |
| `cosmic-text` | 0.12+ | 文本形状计算 |
| `swash` | 0.1+ | 字形光栅化 |
| `libinput` | 0.3+ | 输入设备处理 |
| `thiserror` | 1.0+ | 错误类型派生 |
| `uuid` | 1.0+ | 唯一标识生成 |
| `log` | 0.4+ | 日志记录 |

---

*本文档为 OmniAgent OS Aqua Shell 桌面环境的技术规范，版本 0.1.0。*
