//! Agent 栏和 Spotlight 搜索组件
//!
//! Agent 栏显示当前运行的 AI Agent 状态信息。
//! Spotlight 提供全局搜索功能，可快速查找 Agent、应用、文件和命令。

/// Agent 栏条目
#[derive(Debug, Clone)]
pub struct AgentBarEntry {
    /// Agent 句柄
    pub agent_handle: u64,
    /// Agent 名称
    pub name: String,
    /// Agent 状态（"Running"、"Idle"、"Blocked"）
    pub state: String,
    /// CPU 使用率
    pub cpu_usage: f32,
    /// 内存使用量（字节）
    pub memory_used: u64,
}

/// Agent 栏位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentBarPosition {
    /// 右侧
    Right,
    /// 左侧
    Left,
    /// 底部
    Bottom,
}

/// Agent 栏
///
/// 显示当前运行的 AI Agent 列表及其状态信息。
pub struct AgentBar {
    /// Agent 栏位置
    pub position: AgentBarPosition,
    /// Agent 条目列表
    pub entries: Vec<AgentBarEntry>,
    /// Agent 栏宽度
    pub width: u32,
    /// 最大显示条目数
    pub max_entries: usize,
    /// 是否展开
    pub expanded: bool,
}

impl AgentBar {
    /// 创建新的 Agent 栏
    pub fn new(position: AgentBarPosition) -> Self {
        AgentBar {
            position,
            entries: Vec::new(),
            width: 250,
            max_entries: 10,
            expanded: false,
        }
    }

    /// 添加 Agent 条目
    pub fn add_agent(&mut self, entry: AgentBarEntry) {
        // 如果已存在相同句柄的 Agent，先移除
        self.entries.retain(|e| e.agent_handle != entry.agent_handle);
        self.entries.push(entry);
    }

    /// 移除 Agent 条目
    pub fn remove_agent(&mut self, handle: u64) {
        self.entries.retain(|e| e.agent_handle != handle);
    }

    /// 更新 Agent 状态信息
    pub fn update_agent(&mut self, handle: u64, state: String, cpu: f32, mem: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.agent_handle == handle) {
            entry.state = state;
            entry.cpu_usage = cpu;
            entry.memory_used = mem;
        }
    }

    /// 搜索 Agent（按名称模糊匹配）
    pub fn search(&self, query: &str) -> Vec<&AgentBarEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// 按 CPU 使用率排序（降序）
    pub fn sort_by_cpu(&mut self) {
        self.entries.sort_by(|a, b| {
            b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// 按内存使用量排序（降序）
    pub fn sort_by_memory(&mut self) {
        self.entries.sort_by(|a, b| b.memory_used.cmp(&a.memory_used));
    }

    /// 获取条目数量
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Spotlight 搜索结果类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotlightResultKind {
    /// Agent
    Agent,
    /// 应用程序
    Application,
    /// 文件
    File,
    /// 命令
    Command,
}

/// Spotlight 搜索结果
#[derive(Debug, Clone)]
pub struct SpotlightResult {
    /// 结果类型
    pub kind: SpotlightResultKind,
    /// 标题
    pub title: String,
    /// 副标题
    pub subtitle: String,
    /// 相关度评分
    pub score: f32,
}

/// Spotlight 状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotlightState {
    /// 隐藏
    Hidden,
    /// 可见（输入中）
    Visible,
    /// 显示搜索结果
    ShowingResults,
}

/// Spotlight 搜索
///
/// 全局搜索界面，支持快速查找 Agent、应用、文件和命令。
pub struct Spotlight {
    /// 当前状态
    pub state: SpotlightState,
    /// 搜索结果列表
    pub results: Vec<SpotlightResult>,
    /// 当前选中的结果索引
    pub selected_index: usize,
    /// 搜索查询字符串
    pub query: String,
}

impl Spotlight {
    /// 创建新的 Spotlight 实例
    pub fn new() -> Self {
        Spotlight {
            state: SpotlightState::Hidden,
            results: Vec::new(),
            selected_index: 0,
            query: String::new(),
        }
    }

    /// 打开 Spotlight
    pub fn open(&mut self) {
        self.state = SpotlightState::Visible;
        self.results.clear();
        self.selected_index = 0;
        self.query.clear();
    }

    /// 关闭 Spotlight
    pub fn close(&mut self) {
        self.state = SpotlightState::Hidden;
        self.results.clear();
        self.selected_index = 0;
        self.query.clear();
    }

    /// 执行搜索
    ///
    /// 在给定的 Agent 列表中搜索匹配项。
    pub fn search(&mut self, query: &str, agents: &[AgentBarEntry]) {
        self.query = query.to_string();

        if query.is_empty() {
            self.results.clear();
            self.state = SpotlightState::Visible;
            return;
        }

        let query_lower = query.to_lowercase();
        self.results.clear();

        // 搜索 Agent
        for agent in agents {
            if agent.name.to_lowercase().contains(&query_lower) {
                self.results.push(SpotlightResult {
                    kind: SpotlightResultKind::Agent,
                    title: agent.name.clone(),
                    subtitle: format!("Agent - {}", agent.state),
                    score: 1.0, // 简化评分
                });
            }
        }

        // 如果有结果，切换到显示结果状态
        if self.results.is_empty() {
            self.state = SpotlightState::Visible;
        } else {
            self.state = SpotlightState::ShowingResults;
            self.selected_index = 0;
        }
    }

    /// 选择下一个结果
    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    /// 选择上一个结果
    pub fn select_prev(&mut self) {
        if !self.results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// 确认选择
    ///
    /// 返回当前选中的结果（如果有）。
    pub fn confirm(&mut self) -> Option<SpotlightResult> {
        if self.results.is_empty() {
            return None;
        }
        let result = self.results.get(self.selected_index).cloned();
        self.close();
        result
    }

    /// 获取结果数量
    pub fn result_count(&self) -> usize {
        self.results.len()
    }
}

impl Default for Spotlight {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(handle: u64, name: &str, state: &str, cpu: f32, mem: u64) -> AgentBarEntry {
        AgentBarEntry {
            agent_handle: handle,
            name: name.to_string(),
            state: state.to_string(),
            cpu_usage: cpu,
            memory_used: mem,
        }
    }

    // === AgentBar 测试 ===

    #[test]
    fn test_agent_bar_new() {
        let bar = AgentBar::new(AgentBarPosition::Right);
        assert_eq!(bar.position, AgentBarPosition::Right);
        assert_eq!(bar.width, 250);
        assert_eq!(bar.max_entries, 10);
        assert!(bar.entries.is_empty());
        assert!(!bar.expanded);
    }

    #[test]
    fn test_agent_bar_add_remove() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "Agent-1", "Running", 10.0, 1024));
        bar.add_agent(make_agent(2, "Agent-2", "Idle", 5.0, 512));
        assert_eq!(bar.entry_count(), 2);

        bar.remove_agent(1);
        assert_eq!(bar.entry_count(), 1);
        assert_eq!(bar.entries[0].name, "Agent-2");
    }

    #[test]
    fn test_agent_bar_add_replace() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "Agent-1", "Running", 10.0, 1024));
        bar.add_agent(make_agent(1, "Agent-1-Updated", "Idle", 20.0, 2048));
        assert_eq!(bar.entry_count(), 1);
        assert_eq!(bar.entries[0].name, "Agent-1-Updated");
    }

    #[test]
    fn test_agent_bar_update() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "Agent-1", "Running", 10.0, 1024));
        bar.update_agent(1, "Blocked".to_string(), 50.0, 4096);
        assert_eq!(bar.entries[0].state, "Blocked");
        assert!((bar.entries[0].cpu_usage - 50.0).abs() < f32::EPSILON);
        assert_eq!(bar.entries[0].memory_used, 4096);
    }

    #[test]
    fn test_agent_bar_search() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "CodeAssistant", "Running", 10.0, 1024));
        bar.add_agent(make_agent(2, "FileHelper", "Idle", 5.0, 512));
        bar.add_agent(make_agent(3, "CodeReview", "Running", 20.0, 2048));

        let results = bar.search("Code");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_agent_bar_search_empty() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "Agent-1", "Running", 10.0, 1024));

        let results = bar.search("NotFound");
        assert!(results.is_empty());
    }

    #[test]
    fn test_agent_bar_sort_by_cpu() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "Agent-1", "Running", 10.0, 1024));
        bar.add_agent(make_agent(2, "Agent-2", "Running", 50.0, 2048));
        bar.add_agent(make_agent(3, "Agent-3", "Running", 30.0, 512));

        bar.sort_by_cpu();
        assert!((bar.entries[0].cpu_usage - 50.0).abs() < f32::EPSILON);
        assert!((bar.entries[1].cpu_usage - 30.0).abs() < f32::EPSILON);
        assert!((bar.entries[2].cpu_usage - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_agent_bar_sort_by_memory() {
        let mut bar = AgentBar::new(AgentBarPosition::Right);
        bar.add_agent(make_agent(1, "Agent-1", "Running", 10.0, 1024));
        bar.add_agent(make_agent(2, "Agent-2", "Running", 50.0, 4096));
        bar.add_agent(make_agent(3, "Agent-3", "Running", 30.0, 2048));

        bar.sort_by_memory();
        assert_eq!(bar.entries[0].memory_used, 4096);
        assert_eq!(bar.entries[1].memory_used, 2048);
        assert_eq!(bar.entries[2].memory_used, 1024);
    }

    // === Spotlight 测试 ===

    #[test]
    fn test_spotlight_new() {
        let sl = Spotlight::new();
        assert_eq!(sl.state, SpotlightState::Hidden);
        assert!(sl.results.is_empty());
        assert_eq!(sl.selected_index, 0);
        assert!(sl.query.is_empty());
    }

    #[test]
    fn test_spotlight_open_close() {
        let mut sl = Spotlight::new();
        sl.open();
        assert_eq!(sl.state, SpotlightState::Visible);
        assert!(sl.query.is_empty());

        sl.close();
        assert_eq!(sl.state, SpotlightState::Hidden);
        assert!(sl.results.is_empty());
    }

    #[test]
    fn test_spotlight_search() {
        let mut sl = Spotlight::new();
        let agents = vec![
            make_agent(1, "CodeAssistant", "Running", 10.0, 1024),
            make_agent(2, "FileHelper", "Idle", 5.0, 512),
        ];

        sl.open();
        sl.search("Code", &agents);
        assert_eq!(sl.state, SpotlightState::ShowingResults);
        assert_eq!(sl.result_count(), 1);
        assert_eq!(sl.results[0].title, "CodeAssistant");
        assert_eq!(sl.results[0].kind, SpotlightResultKind::Agent);
    }

    #[test]
    fn test_spotlight_search_empty_query() {
        let mut sl = Spotlight::new();
        let agents = vec![make_agent(1, "Agent-1", "Running", 10.0, 1024)];

        sl.open();
        sl.search("", &agents);
        assert_eq!(sl.result_count(), 0);
        assert_eq!(sl.state, SpotlightState::Visible);
    }

    #[test]
    fn test_spotlight_search_no_results() {
        let mut sl = Spotlight::new();
        let agents = vec![make_agent(1, "Agent-1", "Running", 10.0, 1024)];

        sl.open();
        sl.search("NotFound", &agents);
        assert_eq!(sl.result_count(), 0);
        assert_eq!(sl.state, SpotlightState::Visible);
    }

    #[test]
    fn test_spotlight_select() {
        let mut sl = Spotlight::new();
        let agents = vec![
            make_agent(1, "Agent-1", "Running", 10.0, 1024),
            make_agent(2, "Agent-2", "Idle", 5.0, 512),
            make_agent(3, "Agent-3", "Blocked", 20.0, 2048),
        ];

        sl.open();
        sl.search("Agent", &agents);
        assert_eq!(sl.selected_index, 0);

        sl.select_next();
        assert_eq!(sl.selected_index, 1);

        sl.select_next();
        assert_eq!(sl.selected_index, 2);

        // 循环到开头
        sl.select_next();
        assert_eq!(sl.selected_index, 0);

        sl.select_prev();
        assert_eq!(sl.selected_index, 2);
    }

    #[test]
    fn test_spotlight_select_empty() {
        let mut sl = Spotlight::new();
        // 无结果时选择不应 panic
        sl.select_next();
        sl.select_prev();
        assert_eq!(sl.selected_index, 0);
    }

    #[test]
    fn test_spotlight_confirm() {
        let mut sl = Spotlight::new();
        let agents = vec![
            make_agent(1, "Agent-1", "Running", 10.0, 1024),
            make_agent(2, "Agent-2", "Idle", 5.0, 512),
        ];

        sl.open();
        sl.search("Agent", &agents);
        sl.select_next(); // 选中第二个

        let result = sl.confirm();
        assert!(result.is_some());
        assert_eq!(result.unwrap().title, "Agent-2");
        // 确认后应关闭
        assert_eq!(sl.state, SpotlightState::Hidden);
    }

    #[test]
    fn test_spotlight_confirm_empty() {
        let mut sl = Spotlight::new();
        let result = sl.confirm();
        assert!(result.is_none());
    }
}
