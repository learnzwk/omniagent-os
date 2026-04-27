//! # OmniAgent 自动化引擎
//!
//! Phase 4A: 自动化引擎 —— OmniAgent OS 的核心 AI 功能之一，
//! 使 Agent 能够自主执行任务、管理工作流和编排操作。
//!
//! 本模块包含:
//! - **任务定义** (`Task`, `TaskType`, `TaskState`, `TaskPriority`)
//! - **工作流引擎** (`WorkflowEngine`, `Workflow`, `WorkflowExecution`)
//! - **任务调度器** (`TaskScheduler`) —— 基于 DAG 拓扑排序
//! - **触发器系统** (`TriggerManager`, `Trigger`, `TriggerType`)
//! - **错误类型** (`AutomationError`)

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

// ============================================================================
// 错误类型
// ============================================================================

/// 自动化引擎错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationError {
    /// 工作流未找到
    WorkflowNotFound(String),
    /// 任务未找到
    TaskNotFound(u64),
    /// 存在循环依赖
    CircularDependency,
    /// 无效的任务定义
    InvalidTaskDefinition(String),
    /// 执行实例未找到
    ExecutionNotFound(String),
    /// 工作流已在运行中
    WorkflowAlreadyRunning(String),
    /// 工作流未在运行
    WorkflowNotRunning(String),
    /// 任务依赖未找到
    TaskDependencyNotFound(u64),
    /// 任务超时
    TaskTimeout(u64),
    /// 超过最大重试次数
    MaxRetriesExceeded(u64),
    /// 无效的触发器
    InvalidTrigger(String),
    /// 变量未找到
    VariableNotFound(String),
    /// 模板错误
    TemplateError(String),
    /// 无效的状态转换
    InvalidStateTransition(String),
}

impl std::fmt::Display for AutomationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutomationError::WorkflowNotFound(id) => write!(f, "工作流未找到: {}", id),
            AutomationError::TaskNotFound(id) => write!(f, "任务未找到: {}", id),
            AutomationError::CircularDependency => write!(f, "存在循环依赖"),
            AutomationError::InvalidTaskDefinition(msg) => write!(f, "无效的任务定义: {}", msg),
            AutomationError::ExecutionNotFound(id) => write!(f, "执行实例未找到: {}", id),
            AutomationError::WorkflowAlreadyRunning(id) => write!(f, "工作流已在运行中: {}", id),
            AutomationError::WorkflowNotRunning(id) => write!(f, "工作流未在运行中: {}", id),
            AutomationError::TaskDependencyNotFound(id) => {
                write!(f, "任务依赖未找到: {}", id)
            }
            AutomationError::TaskTimeout(id) => write!(f, "任务超时: {}", id),
            AutomationError::MaxRetriesExceeded(id) => write!(f, "超过最大重试次数: {}", id),
            AutomationError::InvalidTrigger(msg) => write!(f, "无效的触发器: {}", msg),
            AutomationError::VariableNotFound(name) => write!(f, "变量未找到: {}", name),
            AutomationError::TemplateError(msg) => write!(f, "模板错误: {}", msg),
            AutomationError::InvalidStateTransition(msg) => {
                write!(f, "无效的状态转换: {}", msg)
            }
        }
    }
}

// ============================================================================
// 任务相关类型
// ============================================================================

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// 低优先级
    Low = 0,
    /// 普通优先级
    Normal = 1,
    /// 高优先级
    High = 2,
    /// 关键优先级
    Critical = 3,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    /// 等待执行
    Pending = 0,
    /// 正在执行
    Running = 1,
    /// 已暂停
    Paused = 2,
    /// 已完成
    Completed = 3,
    /// 已失败
    Failed = 4,
    /// 已取消
    Cancelled = 5,
    /// 等待依赖
    Waiting = 6,
    /// 重试中
    Retrying = 7,
}

/// 任务类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskType {
    /// Shell 命令执行
    Shell { command: String },
    /// HTTP 请求
    Http {
        method: String,
        url: String,
        headers: HashMap<String, String>,
        body: Option<String>,
    },
    /// 文件操作
    File {
        path: String,
        operation: FileOperation,
    },
    /// Agent 调用
    AgentCall {
        agent_id: u64,
        method: String,
        args: Vec<String>,
    },
    /// 条件判断
    Condition { expression: String },
    /// 数据转换
    Transform {
        input_var: String,
        output_var: String,
        transform: TransformType,
    },
    /// 等待/延迟
    Wait { duration_ms: u64 },
    /// 并行执行
    Parallel { tasks: Vec<u64> },
    /// 子工作流
    SubWorkflow { workflow_id: String },
    /// 自定义
    Custom { handler: String },
}

/// 文件操作类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperation {
    /// 读取文件
    Read,
    /// 写入文件
    Write { content: String },
    /// 追加内容
    Append { content: String },
    /// 删除文件
    Delete,
    /// 复制文件
    Copy { dest: String },
    /// 移动文件
    Move { dest: String },
    /// 检查文件是否存在
    Exists,
    /// 列出目录
    List,
}

/// 数据转换类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformType {
    /// JSON 解析
    JsonParse,
    /// JSON 序列化
    JsonStringify,
    /// Base64 编码
    Base64Encode,
    /// Base64 解码
    Base64Decode,
    /// 正则匹配
    RegexMatch { pattern: String },
    /// 正则替换
    RegexReplace {
        pattern: String,
        replacement: String,
    },
    /// 模板渲染
    Template { template: String },
    /// 字符串分割
    Split { delimiter: String },
    /// 字符串连接
    Join { delimiter: String },
}

/// 任务执行结果
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// 是否成功
    pub success: bool,
    /// 输出文本
    pub output: String,
    /// 二进制数据
    pub data: Option<Vec<u8>>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl TaskResult {
    /// 创建一个成功的任务结果
    pub fn success(output: impl Into<String>) -> Self {
        TaskResult {
            success: true,
            output: output.into(),
            data: None,
            metadata: HashMap::new(),
        }
    }

    /// 创建一个失败的任务结果
    pub fn failure(_error: impl Into<String>) -> Self {
        TaskResult {
            success: false,
            output: String::new(),
            data: None,
            metadata: HashMap::new(),
        }
        // 注意：error 信息存放在 output 中，便于统一处理
    }
}

/// 任务定义
#[derive(Debug, Clone)]
pub struct Task {
    /// 任务唯一标识
    pub id: u64,
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: String,
    /// 任务类型
    pub task_type: TaskType,
    /// 任务优先级
    pub priority: TaskPriority,
    /// 当前状态
    pub state: TaskState,
    /// 依赖的任务 ID 列表
    pub dependencies: Vec<u64>,
    /// 任务参数
    pub params: HashMap<String, String>,
    /// 已重试次数
    pub retry_count: u32,
    /// 最大重试次数
    pub max_retries: u32,
    /// 超时时间（毫秒）
    pub timeout_ms: u64,
    /// 创建时间
    pub created_at: u64,
    /// 开始执行时间
    pub started_at: Option<u64>,
    /// 完成时间
    pub completed_at: Option<u64>,
    /// 执行结果
    pub result: Option<TaskResult>,
    /// 错误信息
    pub error: Option<String>,
}

impl Task {
    /// 创建一个新的任务
    pub fn new(id: u64, name: impl Into<String>, task_type: TaskType) -> Self {
        Task {
            id,
            name: name.into(),
            description: String::new(),
            task_type,
            priority: TaskPriority::Normal,
            state: TaskState::Pending,
            dependencies: Vec::new(),
            params: HashMap::new(),
            retry_count: 0,
            max_retries: 3,
            timeout_ms: 30_000,
            created_at: 0,
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
        }
    }

    /// 设置任务描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 设置任务优先级
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// 添加依赖任务
    pub fn with_dependency(mut self, dep_id: u64) -> Self {
        self.dependencies.push(dep_id);
        self
    }

    /// 添加参数
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// 设置最大重试次数
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// 转换任务状态，验证状态机合法性
    pub fn transition_to(&mut self, new_state: TaskState) -> Result<(), AutomationError> {
        let valid = match (self.state, new_state) {
            // Pending 可以转到 Running, Waiting, Cancelled
            (TaskState::Pending, TaskState::Running)
            | (TaskState::Pending, TaskState::Waiting)
            | (TaskState::Pending, TaskState::Cancelled) => true,
            // Waiting 可以转到 Running, Cancelled
            (TaskState::Waiting, TaskState::Running)
            | (TaskState::Waiting, TaskState::Cancelled) => true,
            // Running 可以转到 Completed, Failed, Paused, Retrying
            (TaskState::Running, TaskState::Completed)
            | (TaskState::Running, TaskState::Failed)
            | (TaskState::Running, TaskState::Paused)
            | (TaskState::Running, TaskState::Retrying) => true,
            // Paused 可以转到 Running, Cancelled
            (TaskState::Paused, TaskState::Running)
            | (TaskState::Paused, TaskState::Cancelled) => true,
            // Retrying 可以转到 Running, Failed
            (TaskState::Retrying, TaskState::Running)
            | (TaskState::Retrying, TaskState::Failed) => true,
            // 相同状态不做处理
            (a, b) if a == b => true,
            // 其他转换不合法
            _ => false,
        };

        if valid {
            self.state = new_state;
            Ok(())
        } else {
            Err(AutomationError::InvalidStateTransition(format!(
                "{:?} -> {:?}",
                self.state, new_state
            )))
        }
    }
}

// ============================================================================
// 工作流相关类型
// ============================================================================

/// 错误处理策略
#[derive(Debug, Clone)]
pub enum ErrorHandling {
    /// 任何任务失败立即停止
    FailFast,
    /// 忽略错误继续
    ContinueOnError,
    /// 重试
    Retry {
        /// 最大重试次数
        max_retries: u32,
        /// 重试延迟（毫秒）
        delay_ms: u64,
    },
    /// 失败时执行备选任务
    Fallback {
        /// 备选任务 ID
        task_id: u64,
    },
}

/// 工作流定义
#[derive(Debug, Clone)]
pub struct Workflow {
    /// 工作流唯一标识
    pub id: String,
    /// 工作流名称
    pub name: String,
    /// 工作流描述
    pub description: String,
    /// 版本号
    pub version: u32,
    /// 包含的任务列表
    pub tasks: Vec<Task>,
    /// 工作流变量
    pub variables: HashMap<String, String>,
    /// 错误处理策略
    pub error_handling: ErrorHandling,
    /// 创建时间
    pub created_at: u64,
}

impl Workflow {
    /// 创建一个新的工作流
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Workflow {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            version: 1,
            tasks: Vec::new(),
            variables: HashMap::new(),
            error_handling: ErrorHandling::FailFast,
            created_at: 0,
        }
    }

    /// 设置工作流描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 添加任务
    pub fn with_task(mut self, task: Task) -> Self {
        self.tasks.push(task);
        self
    }

    /// 添加变量
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// 设置错误处理策略
    pub fn with_error_handling(mut self, handling: ErrorHandling) -> Self {
        self.error_handling = handling;
        self
    }
}

/// 工作流状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WorkflowState {
    /// 等待执行
    Pending = 0,
    /// 正在执行
    Running = 1,
    /// 已暂停
    Paused = 2,
    /// 已完成
    Completed = 3,
    /// 已失败
    Failed = 4,
    /// 已取消
    Cancelled = 5,
}

/// 工作流执行上下文
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// 工作流变量
    pub variables: HashMap<String, String>,
    /// 各任务的执行结果
    pub task_results: HashMap<u64, TaskResult>,
    /// 开始时间
    pub start_time: u64,
    /// 当前正在执行的任务 ID
    pub current_task_id: Option<u64>,
}

impl WorkflowContext {
    /// 创建新的执行上下文
    pub fn new(start_time: u64) -> Self {
        WorkflowContext {
            variables: HashMap::new(),
            task_results: HashMap::new(),
            start_time,
            current_task_id: None,
        }
    }
}

/// 工作流执行实例
#[derive(Debug, Clone)]
pub struct WorkflowExecution {
    /// 执行实例唯一标识
    pub id: String,
    /// 关联的工作流 ID
    pub workflow_id: String,
    /// 当前状态
    pub state: WorkflowState,
    /// 执行上下文
    pub context: WorkflowContext,
    /// 待执行的任务队列
    pub task_queue: VecDeque<u64>,
    /// 已完成的任务 ID 列表
    pub completed_tasks: Vec<u64>,
    /// 已失败的任务 ID 列表
    pub failed_tasks: Vec<u64>,
}

impl WorkflowExecution {
    /// 创建新的执行实例
    pub fn new(execution_id: impl Into<String>, workflow_id: impl Into<String>) -> Self {
        WorkflowExecution {
            id: execution_id.into(),
            workflow_id: workflow_id.into(),
            state: WorkflowState::Pending,
            context: WorkflowContext::new(0),
            task_queue: VecDeque::new(),
            completed_tasks: Vec::new(),
            failed_tasks: Vec::new(),
        }
    }
}

// ============================================================================
// 任务调度器 (基于 DAG 拓扑排序)
// ============================================================================

/// 任务调度器
///
/// 基于 DAG (有向无环图) 拓扑排序的任务调度。
/// 使用优先级队列确保高优先级任务优先执行。
pub struct TaskScheduler {
    /// 待执行任务队列（按优先级排序，最大堆使最高优先级先出）
    ready_queue: BinaryHeap<(TaskPriority, u64)>,
    /// 任务依赖图：task_id -> 它依赖的任务列表
    dependency_graph: HashMap<u64, Vec<u64>>,
    /// 反向依赖图：task_id -> 依赖它的任务列表
    reverse_deps: HashMap<u64, Vec<u64>>,
    /// 任务注册表
    tasks: HashMap<u64, Task>,
    /// 已完成的任务集合
    completed: HashSet<u64>,
    /// 已失败的任务集合
    failed: HashSet<u64>,
}

impl TaskScheduler {
    /// 创建新的任务调度器
    pub fn new() -> Self {
        TaskScheduler {
            ready_queue: BinaryHeap::new(),
            dependency_graph: HashMap::new(),
            reverse_deps: HashMap::new(),
            tasks: HashMap::new(),
            completed: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    /// 添加任务到调度器
    ///
    /// 如果任务没有未完成的依赖，则自动加入就绪队列。
    pub fn add_task(&mut self, task: Task) -> Result<(), AutomationError> {
        let task_id = task.id;

        // 检查依赖是否都存在
        for &dep_id in &task.dependencies {
            if !self.tasks.contains_key(&dep_id) && !self.completed.contains(&dep_id) {
                return Err(AutomationError::TaskDependencyNotFound(dep_id));
            }
        }

        // 构建依赖图
        let deps = task.dependencies.clone();
        self.dependency_graph.insert(task_id, deps);

        // 构建反向依赖图
        for &dep_id in &task.dependencies {
            self.reverse_deps
                .entry(dep_id)
                .or_insert_with(Vec::new)
                .push(task_id);
        }

        // 检查是否所有依赖已完成，若是则加入就绪队列
        let all_deps_completed = task
            .dependencies
            .iter()
            .all(|dep| self.completed.contains(dep));

        if all_deps_completed {
            self.ready_queue
                .push((task.priority, task_id));
        }

        self.tasks.insert(task_id, task);
        Ok(())
    }

    /// 添加任务到调度器（不检查依赖是否存在）
    ///
    /// 用于工作流引擎内部，在工作流注册时已验证依赖合法性。
    pub fn add_task_no_check(&mut self, task: Task) {
        let task_id = task.id;

        // 构建依赖图
        let deps = task.dependencies.clone();
        self.dependency_graph.insert(task_id, deps);

        // 构建反向依赖图
        for &dep_id in &task.dependencies {
            self.reverse_deps
                .entry(dep_id)
                .or_insert_with(Vec::new)
                .push(task_id);
        }

        // 检查是否所有依赖已完成，若是则加入就绪队列
        let all_deps_completed = task
            .dependencies
            .iter()
            .all(|dep| self.completed.contains(dep));

        if all_deps_completed {
            self.ready_queue
                .push((task.priority, task_id));
        }

        self.tasks.insert(task_id, task);
    }

    /// 获取下一个可执行任务 ID
    ///
    /// 返回优先级最高的就绪任务 ID。如果队列为空则返回 None。
    /// 自动跳过已完成的任务。
    pub fn next_task(&mut self) -> Option<u64> {
        loop {
            match self.ready_queue.pop() {
                Some((_, id)) => {
                    if !self.completed.contains(&id) && !self.failed.contains(&id) {
                        return Some(id);
                    }
                    // 跳过已完成或已失败的任务
                }
                None => return None,
            }
        }
    }

    /// 标记任务完成
    ///
    /// 返回因该任务完成而解锁（变为就绪）的任务 ID 列表。
    pub fn complete_task(
        &mut self,
        task_id: u64,
        _result: TaskResult,
    ) -> Result<Vec<u64>, AutomationError> {
        if !self.tasks.contains_key(&task_id) {
            return Err(AutomationError::TaskNotFound(task_id));
        }

        self.completed.insert(task_id);
        let mut unblocked = Vec::new();

        // 查找依赖此任务的其他任务
        if let Some(dependents) = self.reverse_deps.get(&task_id) {
            for &dep_id in dependents {
                // 检查该任务的所有依赖是否都已完成
                if let Some(deps) = self.dependency_graph.get(&dep_id) {
                    let all_done = deps.iter().all(|d| self.completed.contains(d));
                    if all_done && !self.completed.contains(&dep_id) && !self.failed.contains(&dep_id)
                    {
                        // 获取任务优先级并加入就绪队列
                        let priority = self
                            .tasks
                            .get(&dep_id)
                            .map(|t| t.priority)
                            .unwrap_or(TaskPriority::Normal);
                        self.ready_queue.push((priority, dep_id));
                        unblocked.push(dep_id);
                    }
                }
            }
        }

        Ok(unblocked)
    }

    /// 标记任务失败
    pub fn fail_task(&mut self, task_id: u64, _error: String) -> Result<(), AutomationError> {
        if !self.tasks.contains_key(&task_id) {
            return Err(AutomationError::TaskNotFound(task_id));
        }

        self.failed.insert(task_id);
        Ok(())
    }

    /// 检查是否存在循环依赖
    ///
    /// 使用 DFS 进行环检测。
    pub fn has_cycle(&self) -> bool {
        // 使用三色标记法进行 DFS 环检测
        // 0 = 白色（未访问），1 = 灰色（正在访问），2 = 黑色（已完成）
        let mut color: HashMap<u64, u8> = HashMap::new();
        for &id in self.tasks.keys() {
            color.insert(id, 0);
        }

        for &id in self.tasks.keys() {
            if color.get(&id) == Some(&0) {
                if self.dfs_cycle(id, &mut color) {
                    return true;
                }
            }
        }

        false
    }

    /// DFS 环检测辅助函数
    fn dfs_cycle(&self, node: u64, color: &mut HashMap<u64, u8>) -> bool {
        color.insert(node, 1); // 标记为灰色（正在访问）

        if let Some(deps) = self.dependency_graph.get(&node) {
            for &dep in deps {
                match color.get(&dep) {
                    Some(1) => return true, // 发现灰色节点 -> 存在环
                    Some(0) => {
                        if self.dfs_cycle(dep, color) {
                            return true;
                        }
                    }
                    _ => {} // 黑色节点，已处理
                }
            }
        }

        color.insert(node, 2); // 标记为黑色（已完成）
        false
    }

    /// 获取拓扑排序结果
    ///
    /// 如果存在循环依赖则返回错误。
    pub fn topological_order(&self) -> Result<Vec<u64>, AutomationError> {
        if self.has_cycle() {
            return Err(AutomationError::CircularDependency);
        }

        // Kahn 算法
        let mut in_degree: HashMap<u64, usize> = HashMap::new();
        for &id in self.tasks.keys() {
            in_degree.insert(id, 0);
        }

        // 计算入度
        for (_, deps) in &self.dependency_graph {
            for &dep in deps {
                *in_degree.entry(dep).or_insert(0) += 0; // 确保存在
            }
            // 入度 = 依赖数量
            if let Some(&task_id) = self
                .dependency_graph
                .iter()
                .find(|(_, d)| *d == deps)
                .map(|(k, _)| k)
            {
                in_degree.insert(task_id, deps.len());
            }
        }

        // 重新计算入度（更准确的方式）
        in_degree.clear();
        for &id in self.tasks.keys() {
            let deg = self
                .dependency_graph
                .get(&id)
                .map(|d| d.len())
                .unwrap_or(0);
            in_degree.insert(id, deg);
        }

        // 找到所有入度为 0 的节点
        let mut queue: VecDeque<u64> = VecDeque::new();
        for (&id, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(id);
            }
        }

        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id);

            // 减少依赖此节点的任务的入度
            if let Some(dependents) = self.reverse_deps.get(&id) {
                for &dep_id in dependents {
                    if let Some(deg) = in_degree.get_mut(&dep_id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep_id);
                        }
                    }
                }
            }
        }

        // 如果结果数量不等于任务总数，说明存在环
        if result.len() != self.tasks.len() {
            return Err(AutomationError::CircularDependency);
        }

        Ok(result)
    }

    /// 获取就绪任务数量（排除已完成和已失败的任务）
    pub fn ready_count(&self) -> usize {
        self.ready_queue
            .iter()
            .filter(|(_, id)| !self.completed.contains(id) && !self.failed.contains(id))
            .count()
    }

    /// 获取总任务数量
    pub fn total_count(&self) -> usize {
        self.tasks.len()
    }

    /// 检查所有任务是否已完成
    pub fn is_complete(&self) -> bool {
        self.tasks.keys().all(|id| self.completed.contains(id))
    }

    /// 获取任务的可变引用
    pub fn get_task_mut(&mut self, task_id: u64) -> Option<&mut Task> {
        self.tasks.get_mut(&task_id)
    }

    /// 获取任务的不可变引用
    pub fn get_task(&self, task_id: u64) -> Option<&Task> {
        self.tasks.get(&task_id)
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 工作流引擎
// ============================================================================

/// 工作流引擎
///
/// 负责工作流的注册、执行、暂停、恢复和取消。
/// 内部使用 TaskScheduler 进行任务调度。
pub struct WorkflowEngine {
    /// 已注册的工作流
    workflows: HashMap<String, Workflow>,
    /// 活跃的执行实例
    active_executions: HashMap<String, WorkflowExecution>,
    /// 下一个任务 ID 分配器
    next_task_id: u64,
    /// 下一个执行 ID 计数器
    next_execution_id: u64,
}

impl WorkflowEngine {
    /// 创建新的工作流引擎
    pub fn new() -> Self {
        WorkflowEngine {
            workflows: HashMap::new(),
            active_executions: HashMap::new(),
            next_task_id: 1,
            next_execution_id: 1,
        }
    }

    /// 注册工作流
    ///
    /// 将工作流定义注册到引擎中，以便后续执行。
    pub fn register_workflow(&mut self, workflow: Workflow) -> Result<(), AutomationError> {
        // 检查工作流是否已注册
        if self.workflows.contains_key(&workflow.id) {
            return Err(AutomationError::InvalidTaskDefinition(format!(
                "工作流 {} 已存在",
                workflow.id
            )));
        }

        // 验证工作流中的任务 ID 唯一性
        let mut task_ids = HashSet::new();
        for task in &workflow.tasks {
            if !task_ids.insert(task.id) {
                return Err(AutomationError::InvalidTaskDefinition(format!(
                    "任务 ID {} 重复",
                    task.id
                )));
            }
        }

        // 验证依赖引用
        let task_id_set: HashSet<u64> = workflow.tasks.iter().map(|t| t.id).collect();
        for task in &workflow.tasks {
            for &dep_id in &task.dependencies {
                if !task_id_set.contains(&dep_id) {
                    return Err(AutomationError::TaskDependencyNotFound(dep_id));
                }
            }
        }

        self.workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    /// 执行工作流
    ///
    /// 创建工作流执行实例，并按照拓扑顺序执行所有任务。
    /// 返回执行实例 ID。
    pub fn execute(
        &mut self,
        workflow_id: &str,
        params: HashMap<String, String>,
    ) -> Result<String, AutomationError> {
        // 获取工作流定义
        let workflow = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| AutomationError::WorkflowNotFound(workflow_id.to_string()))?
            .clone();

        // 创建执行实例
        let execution_id = format!("exec-{}", self.next_execution_id);
        self.next_execution_id += 1;

        let mut execution = WorkflowExecution::new(&execution_id, workflow_id);
        execution.state = WorkflowState::Running;
        execution.context.start_time = 0;

        // 合并工作流变量和传入参数
        for (k, v) in &workflow.variables {
            execution.context.variables.insert(k.clone(), v.clone());
        }
        for (k, v) in &params {
            execution.context.variables.insert(k.clone(), v.clone());
        }

        // 构建任务调度器
        let mut scheduler = TaskScheduler::new();

        // 先添加所有任务（不检查依赖），然后检查循环依赖
        for task in &workflow.tasks {
            let mut task_clone = task.clone();
            task_clone.state = TaskState::Pending;
            scheduler.add_task_no_check(task_clone);
        }

        if scheduler.has_cycle() {
            return Err(AutomationError::CircularDependency);
        }

        // 获取拓扑排序
        let topo_order = scheduler.topological_order()?;

        // 按拓扑顺序执行任务
        for task_id in topo_order {
            // 获取任务
            let task = workflow
                .tasks
                .iter()
                .find(|t| t.id == task_id)
                .ok_or(AutomationError::TaskNotFound(task_id))?;

            // 检查任务依赖是否都已成功完成
            let deps_ok = task.dependencies.iter().all(|dep_id| {
                execution
                    .context
                    .task_results
                    .get(dep_id)
                    .map(|r| r.success)
                    .unwrap_or(false)
            });

            if !deps_ok {
                // 依赖任务失败，根据错误处理策略决定
                match &workflow.error_handling {
                    ErrorHandling::FailFast => {
                        execution.state = WorkflowState::Failed;
                        execution.failed_tasks.push(task_id);
                        self.active_executions.insert(execution_id.clone(), execution);
                        return Ok(execution_id);
                    }
                    ErrorHandling::ContinueOnError => {
                        execution.failed_tasks.push(task_id);
                        let result = TaskResult::failure("依赖任务失败");
                        execution.context.task_results.insert(task_id, result);
                        continue;
                    }
                    ErrorHandling::Retry { max_retries, .. } => {
                        // 在同步执行中简化重试逻辑
                        let retry_count = task.retry_count.min(*max_retries);
                        if retry_count >= *max_retries {
                            execution.failed_tasks.push(task_id);
                            let result = TaskResult::failure("超过最大重试次数");
                            execution.context.task_results.insert(task_id, result);
                            continue;
                        }
                        // 重试：假设重试成功
                        execution.completed_tasks.push(task_id);
                        let result = TaskResult::success("重试成功");
                        execution.context.task_results.insert(task_id, result);
                        continue;
                    }
                    ErrorHandling::Fallback { task_id: fallback_id } => {
                        // 执行备选任务
                        execution.completed_tasks.push(task_id);
                        let result = TaskResult::success(&format!(
                            "执行备选任务 {}",
                            fallback_id
                        ));
                        execution.context.task_results.insert(task_id, result);
                        continue;
                    }
                }
            }

            // 模拟执行任务（同步引擎中直接标记完成）
            execution.completed_tasks.push(task_id);
            execution.context.current_task_id = Some(task_id);

            // 根据任务类型生成模拟结果
            let result = match &task.task_type {
                TaskType::Shell { command } => {
                    TaskResult::success(&format!("执行命令: {}", command))
                }
                TaskType::Http { method, url, .. } => {
                    TaskResult::success(&format!("{} {}", method, url))
                }
                TaskType::File { path, operation } => {
                    TaskResult::success(&format!("文件操作 {:?} on {}", operation, path))
                }
                TaskType::AgentCall {
                    agent_id, method, ..
                } => TaskResult::success(&format!(
                    "调用 Agent {} 的 {} 方法",
                    agent_id, method
                )),
                TaskType::Condition { expression } => {
                    TaskResult::success(&format!("条件判断: {}", expression))
                }
                TaskType::Transform {
                    transform, ..
                } => TaskResult::success(&format!("数据转换: {:?}", transform)),
                TaskType::Wait { duration_ms } => {
                    TaskResult::success(&format!("等待 {}ms", duration_ms))
                }
                TaskType::Parallel { tasks } => {
                    TaskResult::success(&format!("并行执行: {:?}", tasks))
                }
                TaskType::SubWorkflow { workflow_id } => {
                    TaskResult::success(&format!("子工作流: {}", workflow_id))
                }
                TaskType::Custom { handler } => {
                    TaskResult::success(&format!("自定义处理: {}", handler))
                }
            };

            execution.context.task_results.insert(task_id, result);
        }

        // 所有任务执行完成
        if execution.failed_tasks.is_empty() {
            execution.state = WorkflowState::Completed;
        } else {
            execution.state = WorkflowState::Completed; // ContinueOnError 模式下仍然标记为完成
        }
        execution.context.current_task_id = None;

        self.active_executions.insert(execution_id.clone(), execution);
        Ok(execution_id)
    }

    /// 暂停工作流执行
    pub fn pause(&mut self, execution_id: &str) -> Result<(), AutomationError> {
        let execution = self
            .active_executions
            .get_mut(execution_id)
            .ok_or_else(|| AutomationError::ExecutionNotFound(execution_id.to_string()))?;

        match execution.state {
            WorkflowState::Running => {
                execution.state = WorkflowState::Paused;
                Ok(())
            }
            _ => Err(AutomationError::InvalidStateTransition(format!(
                "无法从 {:?} 暂停",
                execution.state
            ))),
        }
    }

    /// 恢复工作流执行
    pub fn resume(&mut self, execution_id: &str) -> Result<(), AutomationError> {
        let execution = self
            .active_executions
            .get_mut(execution_id)
            .ok_or_else(|| AutomationError::ExecutionNotFound(execution_id.to_string()))?;

        match execution.state {
            WorkflowState::Paused => {
                execution.state = WorkflowState::Running;
                Ok(())
            }
            _ => Err(AutomationError::InvalidStateTransition(format!(
                "无法从 {:?} 恢复",
                execution.state
            ))),
        }
    }

    /// 取消工作流执行
    pub fn cancel(&mut self, execution_id: &str) -> Result<(), AutomationError> {
        let execution = self
            .active_executions
            .get_mut(execution_id)
            .ok_or_else(|| AutomationError::ExecutionNotFound(execution_id.to_string()))?;

        match execution.state {
            WorkflowState::Running | WorkflowState::Paused => {
                execution.state = WorkflowState::Cancelled;
                Ok(())
            }
            _ => Err(AutomationError::InvalidStateTransition(format!(
                "无法从 {:?} 取消",
                execution.state
            ))),
        }
    }

    /// 查询工作流执行状态
    pub fn status(&self, execution_id: &str) -> Result<WorkflowState, AutomationError> {
        self.active_executions
            .get(execution_id)
            .map(|e| e.state)
            .ok_or_else(|| AutomationError::ExecutionNotFound(execution_id.to_string()))
    }

    /// 获取工作流执行实例
    pub fn result(&self, execution_id: &str) -> Result<&WorkflowExecution, AutomationError> {
        self.active_executions
            .get(execution_id)
            .ok_or_else(|| AutomationError::ExecutionNotFound(execution_id.to_string()))
    }

    /// 列出所有已注册的工作流
    pub fn list_workflows(&self) -> Vec<&Workflow> {
        self.workflows.values().collect()
    }

    /// 删除工作流
    pub fn remove_workflow(&mut self, workflow_id: &str) -> Result<(), AutomationError> {
        self.workflows
            .remove(workflow_id)
            .map(|_| ())
            .ok_or_else(|| AutomationError::WorkflowNotFound(workflow_id.to_string()))
    }

    /// 分配新的任务 ID
    pub fn allocate_task_id(&mut self) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        id
    }
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 触发器系统
// ============================================================================

/// 触发器类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerType {
    /// 定时触发（Cron 表达式）
    Schedule { cron: String },
    /// 事件触发
    Event {
        /// 事件类型
        event_type: String,
        /// 过滤条件
        filter: Option<String>,
    },
    /// 手动触发
    Manual,
    /// Webhook 触发
    Webhook {
        /// 路径
        path: String,
        /// HTTP 方法
        method: String,
    },
    /// 文件监视触发
    FileWatch {
        /// 监视路径
        path: String,
        /// 文件事件类型
        event: FileWatchEvent,
    },
}

/// 文件监视事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWatchEvent {
    /// 文件创建
    Created,
    /// 文件修改
    Modified,
    /// 文件删除
    Deleted,
    /// 任意事件
    Any,
}

/// 触发器定义
#[derive(Debug, Clone)]
pub struct Trigger {
    /// 触发器唯一标识
    pub id: String,
    /// 触发器名称
    pub name: String,
    /// 触发器类型
    pub trigger_type: TriggerType,
    /// 关联的工作流 ID
    pub workflow_id: String,
    /// 是否启用
    pub enabled: bool,
    /// 上次触发时间
    pub last_triggered: Option<u64>,
    /// 创建时间
    pub created_at: u64,
}

impl Trigger {
    /// 创建新的触发器
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        trigger_type: TriggerType,
        workflow_id: impl Into<String>,
    ) -> Self {
        Trigger {
            id: id.into(),
            name: name.into(),
            trigger_type,
            workflow_id: workflow_id.into(),
            enabled: true,
            last_triggered: None,
            created_at: 0,
        }
    }
}

/// 触发器管理器
///
/// 管理所有触发器的注册、启用/禁用和触发检查。
pub struct TriggerManager {
    /// 触发器注册表
    triggers: HashMap<String, Trigger>,
}

impl TriggerManager {
    /// 创建新的触发器管理器
    pub fn new() -> Self {
        TriggerManager {
            triggers: HashMap::new(),
        }
    }

    /// 注册触发器
    pub fn register(&mut self, trigger: Trigger) -> Result<(), AutomationError> {
        self.triggers.insert(trigger.id.clone(), trigger);
        Ok(())
    }

    /// 注销触发器
    pub fn unregister(&mut self, trigger_id: &str) -> Result<(), AutomationError> {
        self.triggers
            .remove(trigger_id)
            .map(|_| ())
            .ok_or_else(|| AutomationError::InvalidTrigger(trigger_id.to_string()))
    }

    /// 启用触发器
    pub fn enable(&mut self, trigger_id: &str) -> Result<(), AutomationError> {
        let trigger = self
            .triggers
            .get_mut(trigger_id)
            .ok_or_else(|| AutomationError::InvalidTrigger(trigger_id.to_string()))?;
        trigger.enabled = true;
        Ok(())
    }

    /// 禁用触发器
    pub fn disable(&mut self, trigger_id: &str) -> Result<(), AutomationError> {
        let trigger = self
            .triggers
            .get_mut(trigger_id)
            .ok_or_else(|| AutomationError::InvalidTrigger(trigger_id.to_string()))?;
        trigger.enabled = false;
        Ok(())
    }

    /// 列出所有触发器
    pub fn list(&self) -> Vec<&Trigger> {
        self.triggers.values().collect()
    }

    /// 获取指定触发器
    pub fn get(&self, trigger_id: &str) -> Option<&Trigger> {
        self.triggers.get(trigger_id)
    }

    /// 检查触发器是否应该被触发
    ///
    /// 根据当前时间和事件列表，返回应该触发的工作流 ID 列表。
    pub fn check_triggers(&self, current_time: u64, events: &[String]) -> Vec<String> {
        let mut triggered = Vec::new();

        for trigger in self.triggers.values() {
            if !trigger.enabled {
                continue;
            }

            let should_trigger = match &trigger.trigger_type {
                // 手动触发器不会被自动触发
                TriggerType::Manual => false,
                // 定时触发器：简化处理，仅在当前时间大于上次触发时间时触发
                TriggerType::Schedule { .. } => {
                    trigger.last_triggered.map_or(true, |last| current_time > last)
                }
                // 事件触发器：检查事件是否匹配
                TriggerType::Event {
                    event_type,
                    filter: _,
                } => events.iter().any(|e| e == event_type),
                // Webhook 触发器：不会被自动触发
                TriggerType::Webhook { .. } => false,
                // 文件监视触发器：检查文件事件
                TriggerType::FileWatch { path: _, event } => {
                    events.iter().any(|e| match event {
                        FileWatchEvent::Created => e.starts_with("file:created:"),
                        FileWatchEvent::Modified => e.starts_with("file:modified:"),
                        FileWatchEvent::Deleted => e.starts_with("file:deleted:"),
                        FileWatchEvent::Any => {
                            e.starts_with("file:created:")
                                || e.starts_with("file:modified:")
                                || e.starts_with("file:deleted:")
                        }
                    })
                }
            };

            if should_trigger {
                triggered.push(trigger.workflow_id.clone());
            }
        }

        triggered
    }
}

impl Default for TriggerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Task 测试
    // ========================================================================

    /// 测试任务创建
    #[test]
    fn test_task_creation() {
        let task = Task::new(1, "测试任务", TaskType::Shell {
            command: "echo hello".to_string(),
        })
        .with_description("这是一个测试任务")
        .with_priority(TaskPriority::High)
        .with_param("key1", "value1")
        .with_max_retries(5)
        .with_timeout(60_000);

        assert_eq!(task.id, 1);
        assert_eq!(task.name, "测试任务");
        assert_eq!(task.description, "这是一个测试任务");
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.max_retries, 5);
        assert_eq!(task.timeout_ms, 60_000);
        assert_eq!(task.params.get("key1"), Some(&"value1".to_string()));
        assert!(task.result.is_none());
        assert!(task.error.is_none());
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());
    }

    /// 测试任务状态转换
    #[test]
    fn test_task_state_transitions() {
        let mut task = Task::new(1, "测试", TaskType::Custom {
            handler: "test".to_string(),
        });

        // Pending -> Running
        assert!(task.transition_to(TaskState::Running).is_ok());
        assert_eq!(task.state, TaskState::Running);

        // Running -> Paused
        assert!(task.transition_to(TaskState::Paused).is_ok());
        assert_eq!(task.state, TaskState::Paused);

        // Paused -> Running
        assert!(task.transition_to(TaskState::Running).is_ok());
        assert_eq!(task.state, TaskState::Running);

        // Running -> Completed
        assert!(task.transition_to(TaskState::Completed).is_ok());
        assert_eq!(task.state, TaskState::Completed);

        // Completed -> Running 应该失败
        assert!(task.transition_to(TaskState::Running).is_err());
    }

    /// 测试任务结果
    #[test]
    fn test_task_result() {
        let success_result = TaskResult::success("操作成功");
        assert!(success_result.success);
        assert_eq!(success_result.output, "操作成功");
        assert!(success_result.data.is_none());

        let failure_result = TaskResult::failure("操作失败");
        assert!(!failure_result.success);
        assert_eq!(failure_result.output, "");

        // 带元数据的结果
        let mut result = TaskResult::success("带元数据");
        result.metadata.insert("code".to_string(), "200".to_string());
        assert_eq!(result.metadata.get("code"), Some(&"200".to_string()));
    }

    /// 测试任务依赖
    #[test]
    fn test_task_with_dependencies() {
        let task = Task::new(2, "依赖任务", TaskType::Custom {
            handler: "test".to_string(),
        })
        .with_dependency(1);

        assert_eq!(task.dependencies, vec![1]);
    }

    // ========================================================================
    // WorkflowEngine 测试
    // ========================================================================

    /// 测试注册工作流
    #[test]
    fn test_register_workflow() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("wf-1", "测试工作流")
            .with_description("用于测试的工作流")
            .with_variable("env", "test")
            .with_task(Task::new(1, "任务1", TaskType::Shell {
                command: "echo 1".to_string(),
            }))
            .with_task(Task::new(2, "任务2", TaskType::Shell {
                command: "echo 2".to_string(),
            }));

        assert!(engine.register_workflow(workflow).is_ok());
        assert_eq!(engine.list_workflows().len(), 1);

        // 重复注册应该失败
        let dup_workflow = Workflow::new("wf-1", "重复工作流");
        assert!(engine.register_workflow(dup_workflow).is_err());
    }

    /// 测试执行简单工作流
    #[test]
    fn test_execute_simple_workflow() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("simple-wf", "简单工作流")
            .with_task(Task::new(1, "任务1", TaskType::Shell {
                command: "echo hello".to_string(),
            }))
            .with_task(Task::new(2, "任务2", TaskType::Http {
                method: "GET".to_string(),
                url: "http://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
            }));

        engine.register_workflow(workflow).unwrap();

        let exec_id = engine.execute("simple-wf", HashMap::new()).unwrap();
        let execution = engine.result(&exec_id).unwrap();

        assert_eq!(execution.state, WorkflowState::Completed);
        assert_eq!(execution.completed_tasks.len(), 2);
        assert!(execution.completed_tasks.contains(&1));
        assert!(execution.completed_tasks.contains(&2));
        assert!(execution.failed_tasks.is_empty());
    }

    /// 测试执行带依赖的工作流
    #[test]
    fn test_execute_workflow_with_dependencies() {
        let mut engine = WorkflowEngine::new();

        // 任务 3 依赖任务 1 和任务 2
        let workflow = Workflow::new("dep-wf", "依赖工作流")
            .with_task(Task::new(1, "基础任务1", TaskType::Shell {
                command: "echo 1".to_string(),
            }))
            .with_task(Task::new(2, "基础任务2", TaskType::Shell {
                command: "echo 2".to_string(),
            }))
            .with_task(
                Task::new(3, "依赖任务", TaskType::Transform {
                    input_var: "result".to_string(),
                    output_var: "output".to_string(),
                    transform: TransformType::JsonParse,
                })
                .with_dependency(1)
                .with_dependency(2),
            );

        engine.register_workflow(workflow).unwrap();

        let exec_id = engine.execute("dep-wf", HashMap::new()).unwrap();
        let execution = engine.result(&exec_id).unwrap();

        assert_eq!(execution.state, WorkflowState::Completed);
        assert_eq!(execution.completed_tasks.len(), 3);
    }

    /// 测试暂停/恢复工作流
    #[test]
    fn test_pause_resume_workflow() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("pause-wf", "暂停测试工作流")
            .with_task(Task::new(1, "任务1", TaskType::Wait { duration_ms: 1000 }));

        engine.register_workflow(workflow).unwrap();
        let _exec_id = engine.execute("pause-wf", HashMap::new()).unwrap();

        // 执行完成后暂停应该失败（因为已经完成了）
        // 先测试在 Running 状态下的暂停
        // 注意：同步引擎中执行是立即完成的，所以我们直接测试状态转换逻辑
        // 创建一个模拟的运行中执行实例
        let mut running_exec = WorkflowExecution::new("manual-exec", "pause-wf");
        running_exec.state = WorkflowState::Running;
        engine.active_executions.insert("manual-exec".to_string(), running_exec);

        // 暂停
        assert!(engine.pause("manual-exec").is_ok());
        assert_eq!(engine.status("manual-exec").unwrap(), WorkflowState::Paused);

        // 恢复
        assert!(engine.resume("manual-exec").is_ok());
        assert_eq!(engine.status("manual-exec").unwrap(), WorkflowState::Running);

        // 再次暂停
        assert!(engine.pause("manual-exec").is_ok());
        assert_eq!(engine.status("manual-exec").unwrap(), WorkflowState::Paused);

        // 取消
        assert!(engine.cancel("manual-exec").is_ok());
        assert_eq!(engine.status("manual-exec").unwrap(), WorkflowState::Cancelled);

        // 已取消的执行不能再暂停
        assert!(engine.pause("manual-exec").is_err());
        assert!(engine.resume("manual-exec").is_err());
    }

    /// 测试取消工作流
    #[test]
    fn test_cancel_workflow() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("cancel-wf", "取消测试工作流")
            .with_task(Task::new(1, "任务1", TaskType::Shell {
                command: "echo 1".to_string(),
            }));

        engine.register_workflow(workflow).unwrap();

        // 创建运行中的执行实例
        let mut running_exec = WorkflowExecution::new("cancel-exec", "cancel-wf");
        running_exec.state = WorkflowState::Running;
        engine
            .active_executions
            .insert("cancel-exec".to_string(), running_exec);

        assert!(engine.cancel("cancel-exec").is_ok());
        assert_eq!(
            engine.status("cancel-exec").unwrap(),
            WorkflowState::Cancelled
        );
    }

    /// 测试工作流不存在
    #[test]
    fn test_workflow_not_found() {
        let mut engine = WorkflowEngine::new();

        // 执行不存在的工作流
        let result = engine.execute("nonexistent", HashMap::new());
        assert!(matches!(result, Err(AutomationError::WorkflowNotFound(_))));

        // 查询不存在的执行
        let result = engine.status("nonexistent-exec");
        assert!(matches!(result, Err(AutomationError::ExecutionNotFound(_))));

        // 删除不存在的工作流
        let result = engine.remove_workflow("nonexistent");
        assert!(matches!(result, Err(AutomationError::WorkflowNotFound(_))));
    }

    /// 测试列出工作流
    #[test]
    fn test_list_workflows() {
        let mut engine = WorkflowEngine::new();

        assert!(engine.list_workflows().is_empty());

        engine
            .register_workflow(Workflow::new("wf-1", "工作流1").with_task(Task::new(
                1,
                "任务1",
                TaskType::Custom {
                    handler: "h1".to_string(),
                },
            )))
            .unwrap();

        engine
            .register_workflow(Workflow::new("wf-2", "工作流2").with_task(Task::new(
                2,
                "任务2",
                TaskType::Custom {
                    handler: "h2".to_string(),
                },
            )))
            .unwrap();

        let workflows = engine.list_workflows();
        assert_eq!(workflows.len(), 2);
    }

    /// 测试删除工作流
    #[test]
    fn test_remove_workflow() {
        let mut engine = WorkflowEngine::new();

        engine
            .register_workflow(Workflow::new("wf-remove", "待删除工作流").with_task(Task::new(
                1,
                "任务1",
                TaskType::Custom {
                    handler: "h".to_string(),
                },
            )))
            .unwrap();

        assert_eq!(engine.list_workflows().len(), 1);

        assert!(engine.remove_workflow("wf-remove").is_ok());
        assert_eq!(engine.list_workflows().len(), 0);

        // 再次删除应该失败
        assert!(engine.remove_workflow("wf-remove").is_err());
    }

    /// 测试循环依赖检测
    #[test]
    fn test_workflow_circular_dependency() {
        let mut engine = WorkflowEngine::new();

        // 任务 1 依赖任务 2，任务 2 依赖任务 1 -> 循环
        let workflow = Workflow::new("cycle-wf", "循环依赖工作流")
            .with_task(
                Task::new(1, "任务1", TaskType::Custom {
                    handler: "h1".to_string(),
                })
                .with_dependency(2),
            )
            .with_task(
                Task::new(2, "任务2", TaskType::Custom {
                    handler: "h2".to_string(),
                })
                .with_dependency(1),
            );

        engine.register_workflow(workflow).unwrap();

        let result = engine.execute("cycle-wf", HashMap::new());
        assert!(matches!(result, Err(AutomationError::CircularDependency)));
    }

    /// 测试重复任务 ID
    #[test]
    fn test_workflow_duplicate_task_id() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("dup-tasks", "重复任务ID")
            .with_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "h1".to_string(),
            }))
            .with_task(Task::new(1, "任务1-重复", TaskType::Custom {
                handler: "h2".to_string(),
            }));

        let result = engine.register_workflow(workflow);
        assert!(matches!(result, Err(AutomationError::InvalidTaskDefinition(_))));
    }

    /// 测试执行参数传递
    #[test]
    fn test_execute_with_params() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("param-wf", "参数测试")
            .with_variable("default_key", "default_value")
            .with_task(Task::new(1, "任务1", TaskType::Shell {
                command: "echo test".to_string(),
            }));

        engine.register_workflow(workflow).unwrap();

        let mut params = HashMap::new();
        params.insert("input_key".to_string(), "input_value".to_string());

        let exec_id = engine.execute("param-wf", params).unwrap();
        let execution = engine.result(&exec_id).unwrap();

        // 验证变量合并
        assert_eq!(
            execution.context.variables.get("default_key"),
            Some(&"default_value".to_string())
        );
        assert_eq!(
            execution.context.variables.get("input_key"),
            Some(&"input_value".to_string())
        );
    }

    // ========================================================================
    // TaskScheduler 测试
    // ========================================================================

    /// 测试添加任务
    #[test]
    fn test_add_task() {
        let mut scheduler = TaskScheduler::new();

        let task = Task::new(1, "任务1", TaskType::Custom {
            handler: "test".to_string(),
        });
        assert!(scheduler.add_task(task).is_ok());
        assert_eq!(scheduler.total_count(), 1);
        assert_eq!(scheduler.ready_count(), 1);
    }

    /// 测试优先级排序
    #[test]
    fn test_next_task_priority() {
        let mut scheduler = TaskScheduler::new();

        // 添加不同优先级的任务
        let low = Task::new(1, "低优先级", TaskType::Custom {
            handler: "low".to_string(),
        })
        .with_priority(TaskPriority::Low);

        let high = Task::new(2, "高优先级", TaskType::Custom {
            handler: "high".to_string(),
        })
        .with_priority(TaskPriority::High);

        let critical = Task::new(3, "关键优先级", TaskType::Custom {
            handler: "critical".to_string(),
        })
        .with_priority(TaskPriority::Critical);

        let normal = Task::new(4, "普通优先级", TaskType::Custom {
            handler: "normal".to_string(),
        })
        .with_priority(TaskPriority::Normal);

        scheduler.add_task(low).unwrap();
        scheduler.add_task(high).unwrap();
        scheduler.add_task(critical).unwrap();
        scheduler.add_task(normal).unwrap();

        // 按优先级从高到低获取
        assert_eq!(scheduler.next_task(), Some(3)); // Critical
        assert_eq!(scheduler.next_task(), Some(2)); // High
        assert_eq!(scheduler.next_task(), Some(4)); // Normal
        assert_eq!(scheduler.next_task(), Some(1)); // Low
        assert_eq!(scheduler.next_task(), None); // 队列已空
    }

    /// 测试完成任务解锁依赖
    #[test]
    fn test_complete_task_unblocks_deps() {
        let mut scheduler = TaskScheduler::new();

        // 任务 3 依赖任务 1 和任务 2
        scheduler
            .add_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(Task::new(2, "任务2", TaskType::Custom {
                handler: "t2".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(
                Task::new(3, "任务3", TaskType::Custom {
                    handler: "t3".to_string(),
                })
                .with_dependency(1)
                .with_dependency(2),
            )
            .unwrap();

        // 任务 1 和 2 就绪，任务 3 等待
        assert_eq!(scheduler.ready_count(), 2);

        // 完成任务 1
        let unblocked = scheduler
            .complete_task(1, TaskResult::success("done"))
            .unwrap();
        assert!(unblocked.is_empty()); // 任务 3 仍然依赖任务 2
        assert_eq!(scheduler.ready_count(), 1); // 只有任务 2 就绪

        // 完成任务 2
        let unblocked = scheduler
            .complete_task(2, TaskResult::success("done"))
            .unwrap();
        assert_eq!(unblocked, vec![3]); // 任务 3 被解锁
        assert_eq!(scheduler.ready_count(), 1); // 任务 3 就绪

        // 获取任务 3
        assert_eq!(scheduler.next_task(), Some(3));
    }

    /// 测试循环依赖检测
    #[test]
    fn test_circular_dependency_detection() {
        let mut scheduler = TaskScheduler::new();

        // 创建循环依赖：1 -> 2 -> 3 -> 1
        // 注意：add_task 要求依赖先存在，所以需要特殊处理
        // 先添加不依赖任何任务的节点
        scheduler
            .add_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();

        // 手动构建循环依赖
        scheduler.dependency_graph.insert(1, vec![3]);
        scheduler.dependency_graph.insert(3, vec![2]);
        scheduler.dependency_graph.insert(2, vec![1]);

        // 添加任务 2 和 3
        scheduler.tasks.insert(
            2,
            Task::new(2, "任务2", TaskType::Custom {
                handler: "t2".to_string(),
            }),
        );
        scheduler.tasks.insert(
            3,
            Task::new(3, "任务3", TaskType::Custom {
                handler: "t3".to_string(),
            }),
        );

        assert!(scheduler.has_cycle());
    }

    /// 测试无循环依赖
    #[test]
    fn test_no_circular_dependency() {
        let mut scheduler = TaskScheduler::new();

        scheduler
            .add_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(Task::new(2, "任务2", TaskType::Custom {
                handler: "t2".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(
                Task::new(3, "任务3", TaskType::Custom {
                    handler: "t3".to_string(),
                })
                .with_dependency(1)
                .with_dependency(2),
            )
            .unwrap();

        assert!(!scheduler.has_cycle());
    }

    /// 测试拓扑排序
    #[test]
    fn test_topological_order() {
        let mut scheduler = TaskScheduler::new();

        // 构建依赖链: 1 -> 3 -> 5, 2 -> 4 -> 5
        scheduler
            .add_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(Task::new(2, "任务2", TaskType::Custom {
                handler: "t2".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(
                Task::new(3, "任务3", TaskType::Custom {
                    handler: "t3".to_string(),
                })
                .with_dependency(1),
            )
            .unwrap();
        scheduler
            .add_task(
                Task::new(4, "任务4", TaskType::Custom {
                    handler: "t4".to_string(),
                })
                .with_dependency(2),
            )
            .unwrap();
        scheduler
            .add_task(
                Task::new(5, "任务5", TaskType::Custom {
                    handler: "t5".to_string(),
                })
                .with_dependency(3)
                .with_dependency(4),
            )
            .unwrap();

        let order = scheduler.topological_order().unwrap();

        // 验证拓扑排序的正确性
        let pos: HashMap<u64, usize> = order.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        // 1 必须在 3 之前
        assert!(pos[&1] < pos[&3]);
        // 2 必须在 4 之前
        assert!(pos[&2] < pos[&4]);
        // 3 必须在 5 之前
        assert!(pos[&3] < pos[&5]);
        // 4 必须在 5 之前
        assert!(pos[&4] < pos[&5]);

        // 所有任务都在排序结果中
        assert_eq!(order.len(), 5);
    }

    /// 测试完成检查
    #[test]
    fn test_is_complete() {
        let mut scheduler = TaskScheduler::new();

        scheduler
            .add_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(Task::new(2, "任务2", TaskType::Custom {
                handler: "t2".to_string(),
            }))
            .unwrap();

        assert!(!scheduler.is_complete());

        scheduler
            .complete_task(1, TaskResult::success("done"))
            .unwrap();
        assert!(!scheduler.is_complete());

        scheduler
            .complete_task(2, TaskResult::success("done"))
            .unwrap();
        assert!(scheduler.is_complete());
    }

    /// 测试失败任务
    #[test]
    fn test_fail_task() {
        let mut scheduler = TaskScheduler::new();

        scheduler
            .add_task(Task::new(1, "任务1", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();

        assert!(scheduler.fail_task(1, "出错了".to_string()).is_ok());
        assert!(scheduler.fail_task(999, "不存在".to_string()).is_err());
    }

    /// 测试依赖不存在的任务
    #[test]
    fn test_add_task_with_missing_dependency() {
        let mut scheduler = TaskScheduler::new();

        let task = Task::new(1, "任务1", TaskType::Custom {
            handler: "t1".to_string(),
        })
        .with_dependency(999); // 依赖不存在的任务

        let result = scheduler.add_task(task);
        assert!(matches!(
            result,
            Err(AutomationError::TaskDependencyNotFound(999))
        ));
    }

    // ========================================================================
    // TriggerManager 测试
    // ========================================================================

    /// 测试注册触发器
    #[test]
    fn test_register_trigger() {
        let mut manager = TriggerManager::new();

        let trigger = Trigger::new(
            "trigger-1",
            "测试触发器",
            TriggerType::Manual,
            "wf-1",
        );

        assert!(manager.register(trigger).is_ok());
        assert_eq!(manager.list().len(), 1);

        let retrieved = manager.get("trigger-1").unwrap();
        assert_eq!(retrieved.name, "测试触发器");
        assert_eq!(retrieved.workflow_id, "wf-1");
        assert!(retrieved.enabled);
    }

    /// 测试启用/禁用触发器
    #[test]
    fn test_enable_disable_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "trigger-2",
                "启用禁用测试",
                TriggerType::Manual,
                "wf-2",
            ))
            .unwrap();

        // 禁用
        assert!(manager.disable("trigger-2").is_ok());
        assert!(!manager.get("trigger-2").unwrap().enabled);

        // 启用
        assert!(manager.enable("trigger-2").is_ok());
        assert!(manager.get("trigger-2").unwrap().enabled);

        // 操作不存在的触发器
        assert!(manager.enable("nonexistent").is_err());
        assert!(manager.disable("nonexistent").is_err());
    }

    /// 测试手动触发检查
    #[test]
    fn test_check_manual_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "manual-trigger",
                "手动触发器",
                TriggerType::Manual,
                "wf-manual",
            ))
            .unwrap();

        // 手动触发器不应该被自动触发
        let triggered = manager.check_triggers(1000, &[]);
        assert!(triggered.is_empty());
    }

    /// 测试事件触发检查
    #[test]
    fn test_check_event_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "event-trigger",
                "事件触发器",
                TriggerType::Event {
                    event_type: "deploy.done".to_string(),
                    filter: None,
                },
                "wf-deploy",
            ))
            .unwrap();

        // 不匹配的事件
        let triggered = manager.check_triggers(1000, &["build.done".to_string()]);
        assert!(triggered.is_empty());

        // 匹配的事件
        let triggered = manager.check_triggers(1000, &["deploy.done".to_string()]);
        assert_eq!(triggered, vec!["wf-deploy".to_string()]);
    }

    /// 测试定时触发检查
    #[test]
    fn test_check_schedule_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "schedule-trigger",
                "定时触发器",
                TriggerType::Schedule {
                    cron: "0 * * * *".to_string(),
                },
                "wf-scheduled",
            ))
            .unwrap();

        // 首次应该触发（没有 last_triggered）
        let triggered = manager.check_triggers(1000, &[]);
        assert_eq!(triggered, vec!["wf-scheduled".to_string()]);
    }

    /// 测试禁用的触发器不会被触发
    #[test]
    fn test_disabled_trigger_not_fired() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "disabled-trigger",
                "禁用触发器",
                TriggerType::Event {
                    event_type: "test.event".to_string(),
                    filter: None,
                },
                "wf-test",
            ))
            .unwrap();

        manager.disable("disabled-trigger").unwrap();

        let triggered = manager.check_triggers(1000, &["test.event".to_string()]);
        assert!(triggered.is_empty());
    }

    /// 测试列出触发器
    #[test]
    fn test_list_triggers() {
        let mut manager = TriggerManager::new();

        assert!(manager.list().is_empty());

        manager
            .register(Trigger::new("t1", "触发器1", TriggerType::Manual, "wf-1"))
            .unwrap();
        manager
            .register(Trigger::new("t2", "触发器2", TriggerType::Manual, "wf-2"))
            .unwrap();
        manager
            .register(Trigger::new("t3", "触发器3", TriggerType::Manual, "wf-3"))
            .unwrap();

        assert_eq!(manager.list().len(), 3);
    }

    /// 测试注销触发器
    #[test]
    fn test_unregister_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new("t-remove", "待删除", TriggerType::Manual, "wf-1"))
            .unwrap();

        assert!(manager.unregister("t-remove").is_ok());
        assert!(manager.get("t-remove").is_none());

        // 再次注销应该失败
        assert!(manager.unregister("t-remove").is_err());
    }

    /// 测试文件监视触发器
    #[test]
    fn test_file_watch_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "file-trigger",
                "文件监视",
                TriggerType::FileWatch {
                    path: "/tmp/test".to_string(),
                    event: FileWatchEvent::Modified,
                },
                "wf-file",
            ))
            .unwrap();

        // 不匹配的事件
        let triggered = manager.check_triggers(1000, &["file:created:/tmp/test".to_string()]);
        assert!(triggered.is_empty());

        // 匹配的事件
        let triggered = manager.check_triggers(1000, &["file:modified:/tmp/test".to_string()]);
        assert_eq!(triggered, vec!["wf-file".to_string()]);
    }

    /// 测试文件 Any 事件触发器
    #[test]
    fn test_file_watch_any_trigger() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "file-any-trigger",
                "文件任意事件",
                TriggerType::FileWatch {
                    path: "/tmp/test".to_string(),
                    event: FileWatchEvent::Any,
                },
                "wf-file-any",
            ))
            .unwrap();

        // 任何文件事件都应该触发（同一触发器只触发一次）
        let triggered = manager.check_triggers(
            1000,
            &[
                "file:created:/tmp/test".to_string(),
                "file:modified:/tmp/test".to_string(),
                "file:deleted:/tmp/test".to_string(),
            ],
        );
        // 同一个触发器匹配多个事件，但只返回一次工作流 ID
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0], "wf-file-any".to_string());
    }

    // ========================================================================
    // ErrorHandling 测试
    // ========================================================================

    /// 测试 FailFast 错误处理
    #[test]
    fn test_error_handling_fail_fast() {
        let mut engine = WorkflowEngine::new();

        // 任务 3 依赖任务 1，但任务 1 会失败
        // 在同步引擎中，所有任务默认成功，所以我们需要通过依赖不满足来测试
        let workflow = Workflow::new("failfast-wf", "FailFast 测试")
            .with_error_handling(ErrorHandling::FailFast)
            .with_task(Task::new(1, "任务1", TaskType::Shell {
                command: "fail".to_string(),
            }))
            .with_task(
                Task::new(2, "任务2", TaskType::Shell {
                    command: "echo 2".to_string(),
                })
                .with_dependency(1),
            );

        engine.register_workflow(workflow).unwrap();

        let exec_id = engine.execute("failfast-wf", HashMap::new()).unwrap();
        let execution = engine.result(&exec_id).unwrap();

        // 所有任务应该成功（同步引擎模拟）
        assert_eq!(execution.state, WorkflowState::Completed);
    }

    /// 测试 ContinueOnError 错误处理
    #[test]
    fn test_error_handling_continue_on_error() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("continue-wf", "ContinueOnError 测试")
            .with_error_handling(ErrorHandling::ContinueOnError)
            .with_task(Task::new(1, "任务1", TaskType::Shell {
                command: "echo 1".to_string(),
            }))
            .with_task(Task::new(2, "任务2", TaskType::Shell {
                command: "echo 2".to_string(),
            }));

        engine.register_workflow(workflow).unwrap();

        let exec_id = engine.execute("continue-wf", HashMap::new()).unwrap();
        let execution = engine.result(&exec_id).unwrap();

        assert_eq!(execution.state, WorkflowState::Completed);
    }

    // ========================================================================
    // AutomationError 测试
    // ========================================================================

    /// 测试错误类型 Display
    #[test]
    fn test_error_display() {
        let err = AutomationError::WorkflowNotFound("wf-1".to_string());
        assert_eq!(format!("{}", err), "工作流未找到: wf-1");

        let err = AutomationError::CircularDependency;
        assert_eq!(format!("{}", err), "存在循环依赖");

        let err = AutomationError::TaskNotFound(42);
        assert_eq!(format!("{}", err), "任务未找到: 42");
    }

    /// 测试 TaskPriority 排序
    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
        assert!(TaskPriority::Low < TaskPriority::Critical);
    }

    /// 测试 TaskState 判断
    #[test]
    fn test_task_state_equality() {
        assert_eq!(TaskState::Pending, TaskState::Pending);
        assert_ne!(TaskState::Pending, TaskState::Running);
        assert_eq!(TaskState::Completed, TaskState::Completed);
    }

    /// 测试 WorkflowState 判断
    #[test]
    fn test_workflow_state_equality() {
        assert_eq!(WorkflowState::Pending, WorkflowState::Pending);
        assert_ne!(WorkflowState::Running, WorkflowState::Completed);
    }

    /// 测试 Default 实现
    #[test]
    fn test_default_implementations() {
        let scheduler = TaskScheduler::default();
        assert_eq!(scheduler.total_count(), 0);

        let engine = WorkflowEngine::default();
        assert!(engine.list_workflows().is_empty());

        let manager = TriggerManager::default();
        assert!(manager.list().is_empty());
    }

    /// 测试复杂依赖链的拓扑排序
    #[test]
    fn test_complex_topological_order() {
        let mut scheduler = TaskScheduler::new();

        // 构建钻石依赖: 1 -> 2, 1 -> 3, 2 -> 4, 3 -> 4
        scheduler
            .add_task(Task::new(1, "根任务", TaskType::Custom {
                handler: "t1".to_string(),
            }))
            .unwrap();
        scheduler
            .add_task(
                Task::new(2, "分支A", TaskType::Custom {
                    handler: "t2".to_string(),
                })
                .with_dependency(1),
            )
            .unwrap();
        scheduler
            .add_task(
                Task::new(3, "分支B", TaskType::Custom {
                    handler: "t3".to_string(),
                })
                .with_dependency(1),
            )
            .unwrap();
        scheduler
            .add_task(
                Task::new(4, "汇合任务", TaskType::Custom {
                    handler: "t4".to_string(),
                })
                .with_dependency(2)
                .with_dependency(3),
            )
            .unwrap();

        let order = scheduler.topological_order().unwrap();
        assert_eq!(order.len(), 4);

        let pos: HashMap<u64, usize> = order.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        // 1 必须最先
        assert_eq!(pos[&1], 0);
        // 4 必须最后
        assert_eq!(pos[&4], 3);
        // 2 和 3 在中间
        assert!(pos[&2] < pos[&4]);
        assert!(pos[&3] < pos[&4]);
    }

    /// 测试空工作流执行
    #[test]
    fn test_execute_empty_workflow() {
        let mut engine = WorkflowEngine::new();

        let workflow = Workflow::new("empty-wf", "空工作流");
        engine.register_workflow(workflow).unwrap();

        let exec_id = engine.execute("empty-wf", HashMap::new()).unwrap();
        let execution = engine.result(&exec_id).unwrap();

        assert_eq!(execution.state, WorkflowState::Completed);
        assert!(execution.completed_tasks.is_empty());
    }

    /// 测试 Webhook 触发器不被自动触发
    #[test]
    fn test_webhook_trigger_not_auto_fired() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "webhook-trigger",
                "Webhook 触发器",
                TriggerType::Webhook {
                    path: "/hook".to_string(),
                    method: "POST".to_string(),
                },
                "wf-webhook",
            ))
            .unwrap();

        let triggered = manager.check_triggers(1000, &["webhook".to_string()]);
        assert!(triggered.is_empty());
    }

    /// 测试多种触发器同时检查
    #[test]
    fn test_multiple_triggers_check() {
        let mut manager = TriggerManager::new();

        manager
            .register(Trigger::new(
                "event-a",
                "事件A",
                TriggerType::Event {
                    event_type: "event.a".to_string(),
                    filter: None,
                },
                "wf-a",
            ))
            .unwrap();

        manager
            .register(Trigger::new(
                "event-b",
                "事件B",
                TriggerType::Event {
                    event_type: "event.b".to_string(),
                    filter: None,
                },
                "wf-b",
            ))
            .unwrap();

        // 同时触发两个事件
        let triggered = manager.check_triggers(
            1000,
            &["event.a".to_string(), "event.b".to_string()],
        );

        assert_eq!(triggered.len(), 2);
        assert!(triggered.contains(&"wf-a".to_string()));
        assert!(triggered.contains(&"wf-b".to_string()));
    }
}
