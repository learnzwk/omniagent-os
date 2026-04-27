//! UI 组件系统

use crate::color::Color;
use crate::rect::Rect;

/// 组件 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentId(String);

/// 鼠标按钮
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    /// 左键
    Left = 0,
    /// 右键
    Right = 1,
    /// 中键
    Middle = 2,
}

/// 键码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(pub u16);

/// 修饰键
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers(pub u8);

impl Modifiers {
    /// Shift 键
    pub const SHIFT: Modifiers = Modifiers(1 << 0);
    /// Ctrl 键
    pub const CTRL: Modifiers = Modifiers(1 << 1);
    /// Alt 键
    pub const ALT: Modifiers = Modifiers(1 << 2);
    /// Super 键（Windows/Meta）
    pub const SUPER: Modifiers = Modifiers(1 << 3);

    /// 是否按下 Shift
    pub fn has_shift(&self) -> bool {
        self.0 & Self::SHIFT.0 != 0
    }

    /// 是否按下 Ctrl
    pub fn has_ctrl(&self) -> bool {
        self.0 & Self::CTRL.0 != 0
    }

    /// 是否按下 Alt
    pub fn has_alt(&self) -> bool {
        self.0 & Self::ALT.0 != 0
    }
}

/// UI 事件类型
#[derive(Debug, Clone, PartialEq)]
pub enum UIEvent {
    /// 鼠标点击
    Click { x: i32, y: i32, button: MouseButton },
    /// 鼠标双击
    DoubleClick { x: i32, y: i32 },
    /// 鼠标移动
    MouseMove { x: i32, y: i32 },
    /// 键盘按下
    KeyPress { key: KeyCode, modifiers: Modifiers },
    /// 键盘释放
    KeyRelease { key: KeyCode, modifiers: Modifiers },
    /// 滚轮滚动
    Scroll { delta_x: f32, delta_y: f32 },
    /// 获得焦点
    FocusGained,
    /// 失去焦点
    FocusLost,
    /// 调整大小
    Resize { width: u32, height: u32 },
    /// 关闭
    Close,
}

/// UI 组件 trait
///
/// 所有 UI 组件都需要实现此 trait，提供基本的布局和事件处理能力。
pub trait UIComponent {
    /// 获取组件 ID
    fn id(&self) -> &ComponentId;

    /// 获取组件边界
    fn bounds(&self) -> Rect;

    /// 设置组件边界
    fn set_bounds(&mut self, bounds: Rect);

    /// 获取可见性
    fn visible(&self) -> bool;

    /// 设置可见性
    fn set_visible(&mut self, visible: bool);

    /// 处理事件，返回是否处理了该事件
    fn handle_event(&mut self, event: &UIEvent) -> bool;

    /// 获取子组件列表
    fn children(&self) -> &[Box<dyn UIComponent>];

    /// 添加子组件
    fn add_child(&mut self, child: Box<dyn UIComponent>);
}

/// 布局方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Layout {
    /// 无布局
    None = 0,
    /// 垂直布局
    Vertical = 1,
    /// 水平布局
    Horizontal = 2,
    /// 网格布局
    Grid { cols: u32, rows: u32 },
}

/// 标签组件
///
/// 用于显示文本信息。
pub struct Label {
    /// 组件 ID
    id: ComponentId,
    /// 组件边界
    bounds: Rect,
    /// 显示文本
    text: String,
    /// 文本颜色
    color: Color,
    /// 字体大小
    font_size: u16,
    /// 是否可见
    visible: bool,
    /// 子组件
    children: Vec<Box<dyn UIComponent>>,
}

impl Label {
    /// 创建新的标签组件
    pub fn new(id: &str, text: &str) -> Self {
        Label {
            id: ComponentId(id.to_string()),
            bounds: Rect::new(0, 0, 100, 30),
            text: text.to_string(),
            color: Color::BLACK,
            font_size: 14,
            visible: true,
            children: Vec::new(),
        }
    }

    /// 设置文本内容
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    /// 设置文本颜色
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    /// 设置字体大小
    pub fn set_font_size(&mut self, size: u16) {
        self.font_size = size;
    }

    /// 获取文本内容
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl UIComponent for Label {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, _event: &UIEvent) -> bool {
        // 标签不处理事件
        false
    }

    fn children(&self) -> &[Box<dyn UIComponent>] {
        &self.children
    }

    fn add_child(&mut self, child: Box<dyn UIComponent>) {
        self.children.push(child);
    }
}

/// 按钮组件
///
/// 可点击的交互组件，支持点击回调。
pub struct Button {
    /// 组件 ID
    id: ComponentId,
    /// 组件边界
    bounds: Rect,
    /// 按钮文本
    text: String,
    /// 背景颜色
    background: Color,
    /// 前景颜色（文本颜色）
    foreground: Color,
    /// 是否悬停
    hovered: bool,
    /// 是否按下
    pressed: bool,
    /// 是否启用
    enabled: bool,
    /// 是否可见
    visible: bool,
    /// 子组件
    children: Vec<Box<dyn UIComponent>>,
    /// 点击回调
    on_click: Option<Box<dyn Fn()>>,
}

impl Button {
    /// 创建新的按钮组件
    pub fn new(id: &str, text: &str) -> Self {
        Button {
            id: ComponentId(id.to_string()),
            bounds: Rect::new(0, 0, 120, 36),
            text: text.to_string(),
            background: Color::rgb(66, 133, 244),
            foreground: Color::WHITE,
            hovered: false,
            pressed: false,
            enabled: true,
            visible: true,
            children: Vec::new(),
            on_click: None,
        }
    }

    /// 设置点击回调
    pub fn set_on_click(&mut self, f: Box<dyn Fn()>) {
        self.on_click = Some(f);
    }

    /// 设置是否启用
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 获取按钮文本
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl UIComponent for Button {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &UIEvent) -> bool {
        if !self.enabled {
            return false;
        }

        match event {
            UIEvent::Click { x, y, button: MouseButton::Left } => {
                if self.bounds.contains(*x, *y) {
                    self.pressed = true;
                    // 触发点击回调
                    if let Some(ref callback) = self.on_click {
                        callback();
                    }
                    return true;
                }
                false
            }
            UIEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = self.bounds.contains(*x, *y);
                self.hovered != was_hovered
            }
            _ => false,
        }
    }

    fn children(&self) -> &[Box<dyn UIComponent>] {
        &self.children
    }

    fn add_child(&mut self, child: Box<dyn UIComponent>) {
        self.children.push(child);
    }
}

/// 容器组件
///
/// 用于容纳和管理其他 UI 组件的容器。
pub struct Container {
    /// 组件 ID
    id: ComponentId,
    /// 组件边界
    bounds: Rect,
    /// 背景颜色
    background: Option<Color>,
    /// 是否可见
    visible: bool,
    /// 子组件
    children: Vec<Box<dyn UIComponent>>,
    /// 布局方式
    layout: Layout,
}

impl Container {
    /// 创建新的容器组件
    pub fn new(id: &str) -> Self {
        Container {
            id: ComponentId(id.to_string()),
            bounds: Rect::new(0, 0, 400, 300),
            background: None,
            visible: true,
            children: Vec::new(),
            layout: Layout::None,
        }
    }

    /// 设置布局方式
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
    }

    /// 设置背景颜色
    pub fn set_background(&mut self, color: Color) {
        self.background = Some(color);
    }

    /// 获取布局方式
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// 获取子组件数量
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
}

impl UIComponent for Container {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &UIEvent) -> bool {
        // 容器将事件传递给子组件
        for child in self.children.iter_mut().rev() {
            if child.visible() && child.handle_event(event) {
                return true;
            }
        }
        false
    }

    fn children(&self) -> &[Box<dyn UIComponent>] {
        &self.children
    }

    fn add_child(&mut self, child: Box<dyn UIComponent>) {
        self.children.push(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn test_label_new() {
        let label = Label::new("label1", "Hello");
        assert_eq!(label.id(), &ComponentId("label1".to_string()));
        assert_eq!(label.text(), "Hello");
        assert!(label.visible());
    }

    #[test]
    fn test_label_set_text() {
        let mut label = Label::new("label1", "Hello");
        label.set_text("World");
        assert_eq!(label.text(), "World");
    }

    #[test]
    fn test_label_handle_event() {
        let mut label = Label::new("label1", "Hello");
        let event = UIEvent::Click { x: 0, y: 0, button: MouseButton::Left };
        assert!(!label.handle_event(&event));
    }

    #[test]
    fn test_label_bounds() {
        let mut label = Label::new("label1", "Hello");
        let new_bounds = Rect::new(10, 20, 200, 40);
        label.set_bounds(new_bounds);
        assert_eq!(label.bounds(), new_bounds);
    }

    #[test]
    fn test_label_visible() {
        let mut label = Label::new("label1", "Hello");
        label.set_visible(false);
        assert!(!label.visible());
    }

    #[test]
    fn test_label_add_child() {
        let mut label = Label::new("label1", "Hello");
        let child = Label::new("child1", "Child");
        label.add_child(Box::new(child));
        assert_eq!(label.children().len(), 1);
    }

    #[test]
    fn test_button_new() {
        let button = Button::new("btn1", "Click me");
        assert_eq!(button.id(), &ComponentId("btn1".to_string()));
        assert_eq!(button.text(), "Click me");
        assert!(button.is_enabled());
        assert!(button.visible());
    }

    #[test]
    fn test_button_set_on_click() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_clone = clicked.clone();

        let mut button = Button::new("btn1", "Click me");
        button.set_bounds(Rect::new(0, 0, 100, 50));
        button.set_on_click(Box::new(move || {
            clicked_clone.set(true);
        }));

        // 点击按钮区域
        let event = UIEvent::Click { x: 50, y: 25, button: MouseButton::Left };
        assert!(button.handle_event(&event));
        assert!(clicked.get());
    }

    #[test]
    fn test_button_click_outside() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_clone = clicked.clone();

        let mut button = Button::new("btn1", "Click me");
        button.set_bounds(Rect::new(0, 0, 100, 50));
        button.set_on_click(Box::new(move || {
            clicked_clone.set(true);
        }));

        // 点击按钮外部
        let event = UIEvent::Click { x: 200, y: 200, button: MouseButton::Left };
        assert!(!button.handle_event(&event));
        assert!(!clicked.get());
    }

    #[test]
    fn test_button_disabled() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_clone = clicked.clone();

        let mut button = Button::new("btn1", "Click me");
        button.set_bounds(Rect::new(0, 0, 100, 50));
        button.set_enabled(false);
        button.set_on_click(Box::new(move || {
            clicked_clone.set(true);
        }));

        let event = UIEvent::Click { x: 50, y: 25, button: MouseButton::Left };
        assert!(!button.handle_event(&event));
        assert!(!clicked.get());
    }

    #[test]
    fn test_button_hover() {
        let mut button = Button::new("btn1", "Click me");
        button.set_bounds(Rect::new(0, 0, 100, 50));

        let event = UIEvent::MouseMove { x: 50, y: 25 };
        assert!(button.handle_event(&event)); // 从非悬停变为悬停

        // 再次移动到同一位置不应再触发
        let event = UIEvent::MouseMove { x: 50, y: 25 };
        assert!(!button.handle_event(&event));
    }

    #[test]
    fn test_container_new() {
        let container = Container::new("container1");
        assert_eq!(container.id(), &ComponentId("container1".to_string()));
        assert!(container.visible());
        assert_eq!(container.child_count(), 0);
    }

    #[test]
    fn test_container_set_layout() {
        let mut container = Container::new("container1");
        container.set_layout(Layout::Vertical);
        assert_eq!(container.layout(), Layout::Vertical);

        container.set_layout(Layout::Grid { cols: 2, rows: 3 });
        assert_eq!(container.layout(), Layout::Grid { cols: 2, rows: 3 });
    }

    #[test]
    fn test_container_add_child() {
        let mut container = Container::new("container1");
        let label = Label::new("label1", "Hello");
        container.add_child(Box::new(label));
        assert_eq!(container.child_count(), 1);
        assert_eq!(container.children()[0].id(), &ComponentId("label1".to_string()));
    }

    #[test]
    fn test_container_set_background() {
        let mut container = Container::new("container1");
        container.set_background(Color::rgb(200, 200, 200));
    }

    #[test]
    fn test_container_event_propagation() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_clone = clicked.clone();

        let mut container = Container::new("container1");
        container.set_bounds(Rect::new(0, 0, 400, 300));

        let mut button = Button::new("btn1", "Click");
        button.set_bounds(Rect::new(10, 10, 100, 50));
        button.set_on_click(Box::new(move || {
            clicked_clone.set(true);
        }));
        container.add_child(Box::new(button));

        // 事件应传递给按钮
        let event = UIEvent::Click { x: 50, y: 30, button: MouseButton::Left };
        assert!(container.handle_event(&event));
        assert!(clicked.get());
    }

    #[test]
    fn test_modifiers() {
        let none = Modifiers(0);
        assert!(!none.has_shift());
        assert!(!none.has_ctrl());
        assert!(!none.has_alt());

        let shift = Modifiers::SHIFT;
        assert!(shift.has_shift());
        assert!(!shift.has_ctrl());

        let ctrl_alt = Modifiers(Modifiers::CTRL.0 | Modifiers::ALT.0);
        assert!(!ctrl_alt.has_shift());
        assert!(ctrl_alt.has_ctrl());
        assert!(ctrl_alt.has_alt());
    }

    #[test]
    fn test_modifiers_combined() {
        let all = Modifiers(Modifiers::SHIFT.0 | Modifiers::CTRL.0 | Modifiers::ALT.0 | Modifiers::SUPER.0);
        assert!(all.has_shift());
        assert!(all.has_ctrl());
        assert!(all.has_alt());
    }
}
