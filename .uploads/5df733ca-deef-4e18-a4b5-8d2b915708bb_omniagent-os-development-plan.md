# OmniAgent OS — 新操作系统开发计划 v2.0

## 一、项目定位

开发一个**纯微内核架构的 Agent-Native 操作系统**，核心系统使用 **Rust**（主）+ **C**（性能关键路径）实现，桌面 UI 借鉴 **macOS 桌面美学** + **HarmonyOS PC 模式交互理念**。不要求与任何现有操作系统完全兼容，仅吸收其设计优势。

### 核心原则
- **内核 Rust 优先**: 全部内核代码使用 Rust，仅启动汇编和极少数性能关键路径用 C/ASM
- **UI 美学驱动**: 桌面环境以 macOS 视觉质感为基底，融入 HarmonyOS PC 模式的分布式交互
- **Agent 一等公民**: Agent 不是应用，是系统级服务，通过微内核 syscall 直接管理
- **全模态交互**: Agent 同时具备语音/文本/图像/视频四种模态的输入输出能力
- **自动化执行**: Agent 可自动分解、编排、执行用户指令的复杂操作序列
- **持续学习进化**: Agent 通过主动学习+被动学习双轨机制实现自我改进和知识扩展
- **文档先行**: 每个模块开发前必须完成技术规格文档

---

## 二、系统架构

### 2.1 微内核架构全景

```
┌───────────────────────────────────────────────────────────────────────┐
│                    Desktop Environment (桌面环境)                      │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  Aqua Shell (Rust+GPU)                                         │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │  │
│  │  │  Dock    │ │  MenuBar │ │ WindowMgr│ │ Notification Ctr │  │  │
│  │  │  (停靠栏) │ │  (菜单栏) │ │ (窗口管理)│ │  (通知中心)       │  │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  │  │
│  │  ┌──────────────────────────────────────────────────────────┐  │  │
│  │  │  Agent Bar (Agent 助手栏 — 系统级 Agent 交互入口)          │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ │
│  │Agent Runtime │ │   Memory     │ │    RAG       │ │ Distributed  │ │  Automation  │ │  Multimodal  │ │   Learning   │ │
│  │  Service     │ │   Service    │ │  Service     │ │   Service    │ │   Engine     │ │   Service    │ │   Service    │ │
│  │ (Agent运行时) │ │ (量化记忆)    │ │ (检索增强)    │ │ (分布式协同)   │ │ (自动化引擎)  │ │ (全模态交互)  │ │ (高级学习)    │ │
│  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ │
│         │                │                │                │                │                │                │          │
│  ┌──────┴────────────────┴────────────────┴────────────────┴────────────────┴────────────────┴──────────────┐  │
│  │              IPC Message Router (进程间通信路由)                                                             │  │
│  └──────────────────────────┬─────────────────────────────────────────────────────────────────────────────┘  │
│                              │                                         │
│  ┌──────────────────────────┴─────────────────────────────────────┐  │
│  │            Authorization Manager (授权管理器)                     │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │  │
│  │  │ One-Time Auth│  │Permanent Auth│  │   Policy Engine      │  │  │
│  │  │ (一次性授权)   │  │ (永久授权)    │  │   (策略引擎)          │  │  │
│  │  └──────────────┘  └──────────────┘  └──────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
├───────────────────────────────────────────────────────────────────────┤
│                    Microkernel (微内核 — 纯 Rust)                      │
│                                                                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │Scheduler │ │   IPC    │ │ Memory   │ │  Agent   │ │Interrupt │  │
│  │ (调度器)  │ │(进程通信) │ │Manager   │ │ Syscall  │ │ Handler  │  │
│  │          │ │          │ │(内存管理)  │ │(Agent系统│ │(中断处理) │  │
│  │          │ │          │ │          │ │  调用)    │ │          │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                             │
│  │  Device  │ │ Security │ │  Timer   │                             │
│  │  Driver  │ │ Enclave  │ │ (定时器)  │                             │
│  │ Framework│ │(安全飞地)  │ │          │                             │
│  └──────────┘ └──────────┘ └──────────┘                             │
├───────────────────────────────────────────────────────────────────────┤
│                    HAL — Hardware Abstraction Layer                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │x86_64    │ │ aarch64  │ │   GPU    │ │  NVMe    │ │  NIC     │  │
│  │(Intel/AMD)│ │(ARM/RISC)│ │ (Vulkan) │ │  Driver  │ │  Driver  │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
└───────────────────────────────────────────────────────────────────────┘
```

### 2.2 借鉴各 OS 优势的设计决策

| 来源 OS | 借鉴什么 | 不借鉴什么 | 在 OmniAgent OS 中的实现 |
|---------|---------|-----------|-------------------------|
| **macOS** | 视觉设计语言(磨砂玻璃/圆角/阴影)、全局菜单栏、Dock、Spotlight搜索、Keychain安全、触控板手势 | 闭源、Apple Silicon锁定、文件系统限制 | **Aqua Shell**: GPU加速合成、毛玻璃效果、全局Dock+MenuBar、Agent Spotlight |
| **HarmonyOS** | 分布式软总线、设备虚拟化、跨设备任务流转、PC模式自适应布局 | 华为生态绑定、微内核不纯粹 | **DistributedService**: 设备发现→能力协商→Agent迁移→状态同步 |
| **Linux** | 内核模块化、包管理生态、CLI强大、社区驱动、驱动覆盖广 | 桌面碎片化、ABI不稳定 | **pkg+agent**: 统一包管理器、Agent包格式、Bash兼容Shell |
| **Windows** | 窗口管理( snapped/虚拟桌面)、WDDM显示驱动模型、企业组策略(GPO) | 注册表、DLL地狱、更新机制 | **WindowMgr**: 吸附布局+虚拟桌面、策略引擎(类GPO) |
| **Android** | 电池感知调度(wakelocks)、Intent通信机制、权限模型(安装时+运行时) | Java虚拟机、碎片化、后台滥用 | **PowerMgr**: 电池感知Agent调度、Agent Intent通信、双层权限模型 |

---

## 三、桌面 UI 设计规格 — Aqua Shell

### 3.1 设计理念

**"macOS 的优雅 + HarmonyOS 的智慧 + Agent 的无限可能"**

以 macOS 视觉语言为基底（毛玻璃、圆角、阴影、精致排版），融入 HarmonyOS PC 模式的自适应布局和分布式能力，再加上 Agent-Native 的独创交互范式。

### 3.2 桌面布局

```
┌─────────────────────────────────────────────────────────────────┐
│  [Apple/Logo]  File  Edit  View  Go  Window  Help    [Agent ▼] │  ← 全局菜单栏 (Menu Bar)
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│    ┌──────────┐   ┌──────────┐   ┌──────────────────────────┐  │
│    │          │   │          │   │                          │  │
│    │  App 1   │   │  App 2   │   │     Desktop Widget      │  │
│    │          │   │          │   │   (Agent Status Card)    │  │
│    └──────────┘   └──────────┘   └──────────────────────────┘  │
│                                                                  │
│    ┌──────────────────────────────────────────────────────────┐ │
│    │                    Desktop                                │ │
│    │  📁 Projects    📁 Documents    📁 Downloads              │ │
│    │  🖥️ Terminal    🤖 Agent Hub    ⚙️ Settings               │ │
│    └──────────────────────────────────────────────────────────┘ │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│  [🤖Agent] [📁Files] [🖥️Terminal] [🌐Browser] [🎵Music] ...  │  ← Dock
│                                                                  │
│  ┌─ Agent Bar ─────────────────────────────────────────────────┐│
│  │  🤖 How can I help?  [________________________] [Send]    ││  ← Agent 助手栏
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

### 3.3 UI 组件技术规格

| 组件 | 实现语言 | 渲染方式 | 设计参考 |
|------|---------|---------|---------|
| **Menu Bar** | Rust | GPU 合成 (Vulkan) | macOS 全局菜单栏 |
| **Dock** | Rust | GPU 合成 + 动画 | macOS Dock (弹跳/缩放) |
| **Window Manager** | Rust | Vulkan 合成 | macOS 窗口管理 + Windows Snap |
| **Agent Bar** | Rust | GPU 合成 | 独创：系统级 Agent 交互栏 |
| **Agent Spotlight** | Rust | 全屏覆盖层 | macOS Spotlight → Agent 增强版 |
| **Notification Center** | Rust | GPU 合成 | macOS 通知中心 |
| **Control Center** | Rust | 面板式 | macOS Control Center + HarmonyOS |
| **File Manager** | Rust + TS | 列表/图标/分栏 | macOS Finder 分栏视图 |
| **Terminal** | Rust | GPU 文本渲染 | iTerm2 + Agent 辅助 |
| **Settings** | Rust + TS | 分组列表 | macOS System Preferences |

### 3.4 视觉设计语言

```
Aqua Design Token:
  ├── Colors
  │   ├── Window Background:  rgba(30, 30, 30, 0.85)   # 深色毛玻璃
  │   ├── Window Border:      rgba(255, 255, 255, 0.12) # 微光边框
  │   ├── Accent Color:       #007AFF                    # 系统蓝
  │   ├── Agent Accent:       #AF52DE                    # Agent 紫
  │   ├── Text Primary:       #FFFFFF
  │   ├── Text Secondary:     rgba(255, 255, 255, 0.60)
  │   └── Danger:             #FF3B30
  ├── Typography
  │   ├── System Font:        "Inter" / "SF Pro" fallback
  │   ├── Title:              28px Semibold
  │   ├── Heading:            20px Medium
  │   ├── Body:               14px Regular
  │   └── Caption:            12px Regular
  ├── Spacing
  │   ├── Base Unit:          8px
  │   ├── Padding Small:      8px
  │   ├── Padding Medium:     16px
  │   └── Padding Large:      24px
  ├── Corner Radius
  │   ├── Window:             10px
  │   ├── Button:             6px
  │   ├── Card:               12px
  │   └── Dock Icon:          16px
  ├── Blur
  │   ├── Window Backdrop:    40px Gaussian
  │   ├── Menu Backdrop:      30px Gaussian
  │   └── Dock Backdrop:      25px Gaussian
  └── Animation
      ├── Window Open:        200ms ease-out
      ├── Dock Bounce:        600ms spring
      └── Transition:         150ms ease-in-out
```

---

## 四、核心模块技术规格 (Rust 实现)

### 4.1 微内核 (`os/kernel/`)

所有代码使用 **Rust** 编写，仅 `arch/x86_64/boot.asm` 和 `arch/aarch64/boot.asm` 使用汇编。

| 模块 | 源文件 | 关键类型/接口 | 实现要点 |
|------|--------|-------------|---------|
| 入口 | `main.rs` | `kernel_main(multiboot_info) -> !` | 从 GRUB/Multiboot2 接管，初始化各子系统 |
| 调度器 | `scheduler.rs` | `struct Scheduler`, `fn schedule()`, `fn add_task()`, `fn block()`, `fn yield()` | CFS 变体 + Agent 优先级类（realtime/high/normal/idle/agent） |
| IPC | `ipc.rs` | `struct Channel`, `fn send()`, `fn recv()`, `fn call()` | 同步 RPC + 异步消息队列，零拷贝 fast-path |
| 内存 | `memory.rs` | `struct PageTable`, `fn alloc_pages()`, `fn map()`, `fn protect()` | 4级页表，Agent 隔离地址空间，共享内存区 |
| Agent Syscall | `agent_syscall.rs` | `SYS_AGENT_SPAWN`, `SYS_AGENT_KILL`, `SYS_AGENT_QUERY`, `SYS_AGENT_MSG` | Agent 系统调用号 512+，用户态→内核态 syscall 指令 |
| 中断 | `interrupt.rs` | `struct IDT`, `fn register_handler()`, `fn ack()` | x86_64 IDT / aarch64 异常向量表 |
| 设备驱动 | `device.rs` | `trait Driver`, `fn register()`, `fn ioctl()` | 统一驱动框架，用户态驱动支持 |
| 安全飞地 | `enclave.rs` | `struct Enclave`, `fn create()`, `fn seal()`, `fn unseal()` | 软件TEE（SGX后备），密钥隔离 |
| 定时器 | `timer.rs` | `struct Timer`, `fn set_oneshot()`, `fn set_periodic()` | HPET/Local APIC Timer，高精度定时 |

### 4.2 Agent Runtime Service (`os/services/agent_runtime/`)

使用 **Rust** 实现，通过 IPC 与内核通信。

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| Agent 生成器 | `spawner.rs` | `fn spawn(spec: AgentSpec) -> Result<AgentId>` | 通过 IPC 调用内核 `SYS_AGENT_SPAWN` |
| 专家工厂 | `expert_factory.rs` | `fn create_expert(domain, tools, knowledge) -> ExpertAgent` | 基于用户指令动态组装 Agent |
| Agent 池 | `pool.rs` | `fn register()`, `fn dispatch(task)`, `fn recycle()` | 工作窃取调度，负载均衡 |
| Agent 通信 | `comm.rs` | `fn broadcast()`, `fn send_direct()`, `fn subscribe()` | 发布/订阅 + 直接消息 |
| Agent 进化 | `evolution.rs` | `fn evaluate()`, `fn mutate()`, `fn select()` | 遗传算法 + 性能评估 |
| Agent 知识共享 | `knowledge.rs` | `fn share()`, `fn acquire()` | Agent 间知识传递协议 |

### 4.3 授权管理器 (`os/services/authorization/`)

使用 **Rust** 实现。

| 模块 | 源文件 | 关键接口 | 说明 |
|------|--------|---------|------|
| 一次性授权 | `one_time.rs` | `fn request_once(resource, scope) -> Token` | Token 一次性使用后销毁 |
| 永久授权 | `permanent.rs` | `fn grant_permanent(agent_id, resource, level) -> GrantId` | 持久授权，可撤销 |
| 策略引擎 | `policy.rs` | `fn evaluate(agent_id, resource, action) -> Decision` | PBAC + RBAC 混合策略 |
| 授权存储 | `store.rs` | `fn store()`, `fn revoke()`, `fn audit()` | 持久化 + 审计日志 |
| 授权 UI | `consent_ui.rs` | `fn prompt_user(request) -> Approval` | 桌面弹窗/CLI确认/语音确认 |

### 4.4 量化记忆服务 (`os/services/memory/`)

核心量化引擎使用 **Rust** 实现，嵌入模型推理使用 **C** (ONNX Runtime) 优化。

| 模块 | 源文件 | 关键接口 | 说明 |
|------|--------|---------|------|
| 量化器 | `quantizer.rs` | `fn compress(data) -> QuantizedData`, `fn decompress() -> MemoryData` | INT8/INT4 量化，4x+ 压缩率 |
| 记忆存储 | `store.rs` | `fn store(key, memory)`, `fn recall(query, top_k)` | 三层记忆(工作/情景/语义) |
| 本地嵌入 | `embedding.rs` | `fn encode(text) -> Vec<f32>` | 调用 ONNX Runtime 本地推理 |
| 向量索引 | `index.rs` | `fn build_index()`, `fn search(query_vec)` | HNSW 图索引，纯 Rust |
| 睡眠整理 | `dream.rs` | `fn consolidate()`, `fn forget(threshold)` | 后台整理短期→长期记忆 |

### 4.5 自动化引擎服务 (`os/services/automation/`)

Agent 的**综合辅助能力核心**——支持用户通过自然语言/语音/手势发出指令，由 Agent 自动分解、编排、执行复杂操作序列。

#### 4.5.1 任务自动化引擎

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 指令解析器 | `instruction_parser.rs` | `fn parse(natural_input) -> InstructionGraph` | 将自然语言/结构化输入解析为可执行指令图(DAG) |
| 任务分解器 | `task_decomposer.rs` | `fn decompose(goal) -> Vec<SubTask>` | 递归分解复杂目标为原子子任务，支持依赖分析 |
| 顺序执行器 | `sequential_executor.rs` | `fn execute(dag: InstructionGraph) -> ExecutionResult` | 严格按拓扑序执行子任务，支持回滚/重试/跳过 |
| 条件分支器 | `condition_router.rs` | `fn evaluate_branch(condition, context) -> BranchId` | if/else/switch 分支逻辑，运行时条件评估 |
| 循环控制器 | `loop_controller.rs` | `fn should_continue(iteration, context) -> bool` | for/while/until 循环，含最大迭代保护 |
| 错误恢复器 | `error_recovery.rs` | `fn recover(error, strategy) -> RecoveryAction` | 重试/跳过/回滚/降级/人工介入 五级恢复策略 |
| 任务模板库 | `template_library.rs` | `fn load_template(name) -> InstructionGraph`, `fn save_template()` | 预定义常用操作模板（文件批量处理/数据处理/部署等） |

**指令执行状态机**:
```
IDLE → PARSING → DECOMPOSED → EXECUTING → [SUB_TASK_*] → COMPLETED
                    ↓              ↓              ↓
                  FAILED      PAUSED(等待授权)  RECOVERING
```

#### 4.5.2 工作流自动化引擎

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 工作流定义 | `workflow_def.rs` | `struct Workflow`, `fn from_yaml()`, `fn from_dsl()` | YAML/DSL 声明式工作流定义，支持变量/参数 |
| 工作流引擎 | `workflow_engine.rs` | `fn start(workflow, input) -> RunId`, `fn pause()`, `fn resume()` | 长时运行工作流引擎，持久化状态，支持暂停/恢复 |
| 并行调度器 | `parallel_scheduler.rs` | `fn schedule_parallel(tasks) -> JoinSet` | 并行执行无依赖子任务，DAG 感知，资源约束 |
| 事件触发器 | `event_trigger.rs` | `fn register_trigger(event, workflow)`, `fn fire(event)` | 事件驱动工作流启动（文件变更/定时/Agent通知/设备事件） |
| 定时调度器 | `cron_scheduler.rs` | `fn schedule_cron(expr, workflow)`, `fn cancel(job_id)` | Cron 表达式调度，持久化到存储，系统重启恢复 |
| 工作流监控 | `workflow_monitor.rs` | `fn get_status(run_id) -> WorkflowStatus`, `fn get_metrics()` | 实时进度/耗时/成功率监控，告警阈值 |

**工作流执行模型**:
```
用户/事件触发 → 解析工作流定义 → 构建执行DAG
  → 并行调度无依赖节点 → 顺序执行有依赖节点
  → 收集结果 → 处理错误 → 完成或暂停等待
```

#### 4.5.3 复杂顺序操作处理器

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 操作链编排 | `chain_orchestrator.rs` | `fn chain(operations) -> OperationChain`, `fn execute_chain()` | 链式操作：每步输出作为下步输入，支持中间结果缓存 |
| 上下文传递器 | `context_pass.rs` | `fn pass_context(step_result) -> NextStepInput` | 操作间上下文自动传递，变量绑定，类型安全转换 |
| 断点续执行 | `checkpoint_resume.rs` | `fn save_checkpoint()`, `fn resume_from(checkpoint_id)` | 任意步骤可保存检查点，支持断点续执行 |
| 操作沙箱 | `operation_sandbox.rs` | `fn sandbox_exec(op) -> Result` | 每个操作在隔离沙箱中执行，限制资源/网络/文件访问 |
| 操作市场 | `operation_market.rs` | `fn publish_operation()`, `fn install_operation(id)` | 可复用操作单元发布/安装/组合 |

### 4.6 全模态交互服务 (`os/services/multimodal/`)

Agent 具备**全模态能力**——同时支持语音、文本、图像、视频四种模态的输入/输出/理解/生成，实现真正的多模态融合交互。

#### 4.6.1 语音模态

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 语音识别 (ASR) | `asr.rs` | `fn transcribe(audio_stream) -> Text`, `fn stream_transcribe() -> AsyncIter<Text>` | 本地 Whisper-large-v3 量化模型，60min 长音频，50+ 语言，流式输出 |
| 语音合成 (TTS) | `tts.rs` | `fn synthesize(text, voice_id) -> AudioStream`, `fn stream_synthesize()` | 本地 VITS/Edge-TTS 模型，9+ 语言，多音色，流式输出 < 200ms 首字延迟 |
| 声纹识别 | `voiceprint.rs` | `fn enroll(user_id, samples)`, `fn identify(audio) -> UserId` | 用户声纹注册/识别，用于授权确认 |
| 语音活动检测 | `vad.rs` | `fn detect_voice(audio_chunk) -> bool`, `fn segment(audio) -> Vec<SpeechSegment>` | 实时 VAD，区分语音/静音/噪声，分段传给 ASR |
| 远场拾音 | `far_field.rs` | `fn beamform(mic_array) -> FocusedAudio`, `fn denoise()` | 麦克风阵列波束成形 + 降噪（可选硬件支持） |

**语音管线**:
```
Mic → VAD → 远场降噪 → ASR(流式) → 语义理解 → Agent处理
  → TTS(流式) → Speaker
```

#### 4.6.2 文本模态

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 自然语言理解 | `nlu.rs` | `fn understand(text) -> Intent`, `fn extract_entities() -> Vec<Entity>` | 意图识别 + 实体抽取，本地小模型 + LLM 后备 |
| 自然语言生成 | `nlg.rs` | `fn generate(context, intent) -> Text`, `fn stream_generate()` | LLM 驱动文本生成，流式输出 |
| 文本摘要 | `summarizer.rs` | `fn summarize(text, ratio) -> Summary` | 抽取式+生成式摘要，支持长文档分段摘要 |
| 翻译引擎 | `translator.rs` | `fn translate(text, src_lang, tgt_lang) -> TranslatedText` | 本地 NLLB/Opus-MT 模型，50+ 语言互译 |
| 代码理解 | `code_understand.rs` | `fn analyze_code(source) -> CodeAnalysis` | 代码语法/语义分析，AST 解析，依赖追踪 |

#### 4.6.3 图像模态

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 图像理解 (VQA) | `image_understand.rs` | `fn describe(image) -> Description`, `fn answer_question(image, question) -> Answer` | 本地 VLM 模型（LLaVA/InternVL 量化），图像描述/问答 |
| OCR 识别 | `ocr.rs` | `fn recognize_text(image) -> Vec<TextRegion>`, `fn recognize_table() -> Table` | PaddleOCR/TrOCR 量化模型，多语言文字识别，表格结构化 |
| 图像生成 | `image_generate.rs` | `fn generate(prompt, style) -> Image`, `fn edit(image, mask, prompt) -> Image` | 本地 Stable Diffusion 量化模型，文生图/图生图/局部编辑 |
| 图像分割 | `segmentation.rs` | `fn segment(image) -> Vec<Segment>`, `fn detect_objects() -> Vec<Detection>` | SAM/YOLO 量化模型，目标检测+语义分割 |
| 屏幕理解 | `screen_understand.rs` | `fn capture_screen() -> Screenshot`, `fn understand_ui(screenshot) -> UIElementTree` | 屏幕截图 + UI 元素识别，Agent 辅助操作基础 |

**图像管线**:
```
摄像头/截图/文件 → 预处理 → [OCR/VQA/分割/检测] → 结构化结果 → Agent 理解
Agent 指令 → 图像生成/编辑 → 输出图像/显示
```

#### 4.6.4 视频模态

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 视频理解 | `video_understand.rs` | `fn analyze_video(video) -> VideoAnalysis`, `fn answer_question(video, question) -> Answer` | 关键帧抽取 + VLM 逐帧分析，视频问答/摘要 |
| 视频摘要 | `video_summary.rs` | `fn summarize(video) -> KeyMoments`, `fn generate_timeline() -> Timeline` | 自动关键帧检测，生成视频时间线摘要 |
| 视频生成 | `video_generate.rs` | `fn generate(prompt, duration) -> Video`, `fn interpolate(frames) -> Video` | 本地视频生成模型（AnimateDiff/CogVideo 量化），帧插值 |
| 实时视频流 | `video_stream.rs` | `fn process_stream(stream) -> AsyncIter<FrameResult>` | 实时视频流处理：人脸检测/手势识别/物体追踪 |
| 视频编辑 | `video_edit.rs` | `fn trim(video, start, end)`, `fn merge(clips)`, `fn add_subtitles()` | 基础视频编辑操作，Agent 可组合使用 |

**视频管线**:
```
摄像头/文件 → 解码 → 关键帧抽取 → [VQA/OCR/检测] → 时序聚合 → Agent 理解
Agent 指令 → 视频生成/编辑 → 编码输出
```

#### 4.6.5 多模态融合层

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 模态路由器 | `modal_router.rs` | `fn route(input) -> ModalType`, `fn dispatch(input) -> ModalHandler` | 自动检测输入模态，路由到对应处理器 |
| 跨模态对齐 | `cross_modal_align.rs` | `fn align(text, image) -> AlignmentScore`, `fn fuse(features) -> FusedRepresentation` | 文本-图像-音频-视频 特征空间对齐，多模态融合表示 |
| 统一对话接口 | `unified_dialog.rs` | `fn send(message: MultiModalMessage) -> MultiModalResponse`, `fn stream_send()` | 统一多模态消息格式，Agent 单一入口处理所有模态 |
| 模态转换器 | `modal_converter.rs` | `fn text_to_speech()`, `fn speech_to_text()`, `fn image_to_text()`, `fn text_to_image()` | 模态间互转（文本→语音、图像→描述等） |

**统一多模态消息格式**:
```rust
enum ModalContent {
    Text(String),
    Audio(AudioStream),
    Image(ImageData),
    Video(VideoStream),
    Multi(Vec<ModalContent>),  // 混合模态
}

struct MultiModalMessage {
    content: ModalContent,
    metadata: MessageMetadata,
    context: ConversationContext,
}
```

### 4.7 高级学习服务 (`os/services/learning/`)

Agent 具备**持续自我改进能力**——通过主动学习和被动学习双轨机制，实现知识的自动扩展、技能的自主进化、性能的持续优化。

#### 4.7.1 主动学习引擎

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 好奇心驱动探索 | `curiosity_engine.rs` | `fn identify_knowledge_gap() -> Vec<Topic>`, `fn propose_exploration(gap) -> ExplorationPlan` | 检测知识空白区，主动发起信息搜集/实验/推理 |
| 自主知识获取 | `knowledge_seeker.rs` | `fn search(query) -> Knowledge`, `fn validate(knowledge) -> Confidence` | 主动搜索网络/文档/代码库，交叉验证知识可靠性 |
| 自主实验 | `self_experiment.rs` | `fn design_experiment(hypothesis) -> Experiment`, `fn run(experiment) -> Result` | 设计-A/B测试-分析循环，验证假设/优化参数 |
| 技能自创 | `skill_creator.rs` | `fn analyze_need(task_pattern) -> SkillSpec`, `fn create_skill(spec) -> Skill` | 分析反复出现的任务模式，自动创建新技能并注册 |
| 主动提问 | `active_questioning.rs` | `fn formulate_question(gap) -> Question`, `fn integrate_answer(answer)` | 当知识不足时，主动向用户/其他Agent提问获取知识 |

**主动学习循环**:
```
知识图谱扫描 → 检测空白 → 生成探索计划
  → [搜索/实验/提问] → 获取新知识 → 验证可靠性
  → 整合到知识库 → 更新技能/策略 → 触发下一轮
```

#### 4.7.2 被动学习引擎

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 行为观察器 | `behavior_observer.rs` | `fn observe(action, context, outcome)`, `fn get_patterns() -> Vec<Pattern>` | 被动观察用户操作/Agent执行，提取行为模式 |
| 反馈吸收器 | `feedback_absorber.rs` | `fn absorb_feedback(feedback, context)`, `fn adjust_behavior()` | 吸收用户显式反馈（点赞/纠正/评分），调整行为权重 |
| 错误学习器 | `error_learner.rs` | `fn record_error(error, context)`, `fn generate_prevention() -> PreventionRule` | 记录错误及上下文，生成预防规则避免重犯 |
| 模式提取器 | `pattern_extractor.rs` | `fn extract_patterns(history) -> Vec<Pattern>`, `fn generalize(pattern) -> Rule` | 从历史交互记录中提取通用模式/规则 |
| 偏好学习器 | `preference_learner.rs` | `fn learn_preference(user_action)`, `fn predict_preference(context) -> Preference` | 逐步学习用户偏好（交互风格/输出格式/工作习惯） |

**被动学习循环**:
```
用户操作/Agent执行 → 观察 → 记录
  → 定期分析模式 → 提取规则/偏好
  → 验证(AB测试) → 整合到行为模型
```

#### 4.7.3 知识扩展与进化

| 模块 | 源文件 | 关键接口 | 实现要点 |
|------|--------|---------|---------|
| 知识图谱 | `knowledge_graph.rs` | `fn add_node(entity)`, `fn add_relation(from, to, type)`, `fn query(pattern) -> SubGraph` | 本地知识图谱存储与推理，支持增量更新 |
| 知识融合器 | `knowledge_fusion.rs` | `fn fuse(source_a, source_b) -> FusedKnowledge`, `fn resolve_conflict()` | 多源知识融合，冲突检测与解决 |
| 技能进化器 | `skill_evolver.rs` | `fn evolve(skill, feedback) -> EvolvedSkill`, `fn rank_skills() -> Vec<SkillRank>` | 技能参数优化/组合变异/选择保留，遗传算法驱动 |
| 策略优化器 | `strategy_optimizer.rs` | `fn optimize(strategy, metrics) -> OptimizedStrategy` | 基于累积指标自动优化执行策略（延迟/成本/质量平衡） |
| 迁移学习器 | `transfer_learner.rs` | `fn transfer(source_domain, target_domain) -> TransferResult` | 跨领域知识迁移，将一个领域的经验应用到新领域 |
| 遗忘管理器 | `forgetting_mgr.rs` | `fn evaluate_retention(knowledge) -> RetentionScore`, `fn prune(threshold)` | 知识遗忘曲线管理，过时/低价值知识自动清理 |

**学习效果度量**:
```rust
struct LearningMetrics {
    knowledge_nodes_added: u64,
    knowledge_nodes_pruned: u64,
    skills_created: u64,
    skills_evolved: u64,
    error_rate_reduction: f32,
    user_satisfaction_trend: f32,
    task_completion_improvement: f32,
    response_latency_improvement: f32,
}
```

### 4.8 分布式服务 (`os/services/distributed/`) — 借鉴 HarmonyOS

使用 **Rust** 实现。

| 模块 | 源文件 | 关键接口 | 说明 |
|------|--------|---------|------|
| 设备发现 | `discovery.rs` | `fn discover() -> DeviceList`, `fn publish()` | mDNS + 自定义能力广播 |
| 软总线 | `soft_bus.rs` | `fn connect(device) -> Channel`, `fn transfer(task, target)` | 虚拟设备总线，透明跨设备通信 |
| 任务迁移 | `migration.rs` | `fn migrate(agent_id, target) -> Status` | Agent Checkpoint + Restore |
| 资源池 | `resource.rs` | `fn aggregate() -> GlobalPool`, `fn allocate()` | 全局资源视图 |
| 状态同步 | `sync.rs` | `fn sync_state()`, `fn resolve_conflict()` | CRDT + 向量时钟 |

### 4.9 Aqua Shell 桌面环境 (`os/ui/`)

使用 **Rust** 实现，Vulkan GPU 渲染。

| 模块 | 源文件 | 关键接口 | 说明 |
|------|--------|---------|------|
| 合成器 | `compositor.rs` | `fn compose()`, `fn present()` | Vulkan 渲染循环，毛玻璃效果 |
| 窗口管理器 | `window_mgr.rs` | `fn create_window()`, `fn resize()`, `fn snap()` | 浮动/平铺/全屏/分屏 |
| Dock | `dock.rs` | `fn add_icon()`, `fn bounce()`, `fn context_menu()` | 弹跳动画、右键菜单 |
| 菜单栏 | `menubar.rs` | `fn set_app_menu()`, `fn show_dropdown()` | 全局菜单栏 |
| Agent 助手栏 | `agent_bar.rs` | `fn show()`, `fn process_input()`, `fn stream_response()` | 系统级 Agent 交互 |
| Agent Spotlight | `spotlight.rs` | `fn search()`, `fn agent_action()` | Cmd+Space 全局搜索+Agent执行 |
| 通知中心 | `notification.rs` | `fn push()`, `fn dismiss()`, `fn group()` | 通知分组与操作 |
| 触控板手势 | `gesture.rs` | `fn on_swipe()`, `fn on_pinch()`, `fn on_rotate()` | macOS 风格多点触控 |
| 主题引擎 | `theme.rs` | `fn apply_theme()`, `fn get_design_tokens()` | 设计令牌系统 |

---

## 五、技术文档体系

### 5.1 文档层级

| 层级 | 文档类型 | 路径 | 受众 |
|------|---------|------|------|
| L1 | 架构规格 | `docs/architecture/` | 全体开发者 |
| L2 | 模块规格 | `docs/modules/` | 模块开发者 |
| L3 | API 参考 | `docs/api/` | 应用开发者 |
| L4 | 开发指南 | `docs/guides/` | 新加入开发者 |
| L5 | 测试规格 | `docs/testing/` | QA |
| L6 | 安全规格 | `docs/security/` | 安全工程师 |

### 5.2 必须文档清单

```
docs/
├── architecture/
│   ├── system-overview.md          # 系统全景架构
│   ├── microkernel-design.md      # 微内核设计规格
│   ├── ipc-protocol.md            # IPC 协议规格
│   ├── memory-model.md            # 内存模型规格
│   ├── agent-syscall-abi.md       # Agent 系统调用 ABI
│   ├── security-model.md          # 安全模型规格
│   └── boot-process.md            # 启动流程规格
├── modules/
│   ├── scheduler-spec.md          # 调度器规格
│   ├── ipc-spec.md                # IPC 规格
│   ├── memory-manager-spec.md     # 内存管理规格
│   ├── agent-runtime-spec.md      # Agent Runtime 规格
│   ├── authorization-spec.md      # 授权管理规格
│   ├── quantized-memory-spec.md   # 量化记忆规格
│   ├── distributed-spec.md        # 分布式规格
│   ├── automation-spec.md         # 自动化引擎规格
│   ├── multimodal-spec.md         # 全模态交互规格
│   ├── learning-spec.md           # 高级学习规格
│   └── aqua-shell-spec.md         # Aqua Shell 规格
├── api/
│   ├── kernel-syscalls.md         # 内核系统调用参考
│   ├── agent-api.md               # Agent API 参考
│   ├── ipc-api.md                 # IPC API 参考
│   ├── automation-api.md          # 自动化引擎 API 参考
│   ├── multimodal-api.md          # 全模态交互 API 参考
│   ├── learning-api.md            # 高级学习 API 参考
│   └── ui-framework-api.md        # UI 框架 API 参考
├── guides/
│   ├── getting-started.md         # 快速开始
│   ├── build-toolchain.md         # 构建工具链配置
│   ├── debugging-with-qemu.md     # QEMU 调试指南
│   ├── writing-device-drivers.md  # 编写设备驱动
│   ├── creating-agent-packages.md # 创建 Agent 包
│   └── contributing.md            # 贡献指南
├── testing/
│   ├── test-strategy.md           # 测试策略
│   ├── kernel-testing.md          # 内核测试方法
│   └── performance-benchmarks.md  # 性能基准规格
└── security/
    ├── threat-model.md            # 威胁模型
    ├── authorization-design.md    # 授权设计
    ├── enclave-spec.md            # 安全飞地规格
    └── audit-logging.md           # 审计日志规格
```

### 5.3 文档标准

每份技术文档必须包含：
1. **目的与范围** — 模块解决什么问题
2. **接口契约** — 所有公开 API 签名 + 前置/后置条件
3. **数据结构** — 关键数据结构的内存布局
4. **状态机** — 复杂状态转换的 Mermaid 图
5. **错误处理** — 所有可能的错误码与恢复策略
6. **性能约束** — 延迟/吞吐/内存目标
7. **安全考量** — 攻击面分析与防护措施
8. **测试用例** — 最少测试用例清单

---

## 六、开发里程碑

### Phase 0: 基础设施与文档 (3 周)
- [ ] 创建项目仓库与目录结构
- [ ] 初始化 Rust workspace (`Cargo.toml`)
- [ ] 配置 QEMU 测试环境
- [ ] 编写 L1 架构规格文档（全部6份）
- [ ] 编写 Phase 1 涉及模块的 L2 规格
- [ ] 搭建 CI 流水线骨架

### Phase 1: 微内核核心 (8 周)
- [ ] `arch/x86_64/boot.asm` — 多核启动
- [ ] `main.rs` — 内核入口与初始化序列
- [ ] `memory.rs` — 物理内存分配器 + 4级页表
- [ ] `interrupt.rs` — IDT + 中断分发
- [ ] `scheduler.rs` — CFS 变体调度器
- [ ] `ipc.rs` — 同步消息传递
- [ ] `timer.rs` — HPET 定时器
- [ ] **验收**: QEMU 启动到 shell 提示符，IPC 延迟 < 1μs

### Phase 2: Agent Syscall + 授权 (6 周)
- [ ] `agent_syscall.rs` — Agent 系统调用实现
- [ ] 用户态 libagent Rust 库
- [ ] `one_time.rs` — 一次性授权
- [ ] `permanent.rs` — 永久授权
- [ ] `policy.rs` — 策略引擎
- [ ] `store.rs` — 授权持久化
- [ ] **验收**: Agent 可 spawn/kill，授权链路完整

### Phase 3: Agent Runtime 服务 (8 周)
- [ ] `spawner.rs` — Agent 生成器
- [ ] `expert_factory.rs` — 专家工厂
- [ ] `pool.rs` — Agent 池
- [ ] `comm.rs` — Agent 通信
- [ ] `evolution.rs` — Agent 进化引擎
- [ ] `knowledge.rs` — 知识共享
- [ ] **验收**: 100+ 并发 Agent，进化引擎自主运行

### Phase 4: 量化记忆 + 分布式 + 自动化 (10 周)
- [ ] `quantizer.rs` — INT8/INT4 量化
- [ ] `store.rs` — 三层记忆
- [ ] `embedding.rs` — 本地嵌入 (ONNX)
- [ ] `index.rs` — HNSW 向量索引
- [ ] `dream.rs` — 睡眠整理
- [ ] `discovery.rs` — 设备发现
- [ ] `soft_bus.rs` — 分布式软总线
- [ ] `migration.rs` — Agent 迁移
- [ ] `instruction_parser.rs` — 指令解析器
- [ ] `task_decomposer.rs` — 任务分解器
- [ ] `sequential_executor.rs` — 顺序执行器
- [ ] `workflow_engine.rs` — 工作流引擎
- [ ] `chain_orchestrator.rs` — 操作链编排
- [ ] `checkpoint_resume.rs` — 断点续执行
- [ ] **验收**: 离线记忆存取，量化率 4x+，2 设备迁移 < 3s，工作流端到端可执行

### Phase 5: 全模态交互服务 (8 周)
- [ ] `asr.rs` — 语音识别 (Whisper 量化)
- [ ] `tts.rs` — 语音合成 (VITS/Edge-TTS)
- [ ] `vad.rs` — 语音活动检测
- [ ] `nlu.rs` + `nlg.rs` — 文本理解与生成
- [ ] `image_understand.rs` — 图像理解 (VLM 量化)
- [ ] `ocr.rs` — OCR 识别
- [ ] `image_generate.rs` — 图像生成 (SD 量化)
- [ ] `video_understand.rs` — 视频理解
- [ ] `modal_router.rs` — 模态路由器
- [ ] `unified_dialog.rs` — 统一对话接口
- [ ] `cross_modal_align.rs` — 跨模态对齐
- [ ] **验收**: 语音/文本/图像/视频四模态输入输出可用，统一对话接口工作

### Phase 6: 高级学习服务 (6 周)
- [ ] `curiosity_engine.rs` — 好奇心驱动探索
- [ ] `knowledge_seeker.rs` — 自主知识获取
- [ ] `skill_creator.rs` — 技能自创
- [ ] `behavior_observer.rs` — 行为观察器
- [ ] `feedback_absorber.rs` — 反馈吸收器
- [ ] `error_learner.rs` — 错误学习器
- [ ] `preference_learner.rs` — 偏好学习器
- [ ] `knowledge_graph.rs` — 知识图谱
- [ ] `skill_evolver.rs` — 技能进化器
- [ ] `forgetting_mgr.rs` — 遗忘管理器
- [ ] **验收**: 主动学习可检测知识空白并自主填补，被动学习可从用户行为提取偏好

### Phase 7: Aqua Shell 桌面环境 (8 周)
- [ ] `compositor.rs` — Vulkan GPU 合成
- [ ] `window_mgr.rs` — 窗口管理
- [ ] `dock.rs` — Dock 栏
- [ ] `menubar.rs` — 全局菜单栏
- [ ] `agent_bar.rs` — Agent 助手栏
- [ ] `spotlight.rs` — Agent Spotlight
- [ ] `notification.rs` — 通知中心
- [ ] `theme.rs` — 主题引擎(设计令牌)
- [ ] **验收**: GUI 可渲染，Dock/Menu 可交互，Agent Bar 可对话

### Phase 8: 安全加固 + 优化 (4 周)
- [ ] `enclave.rs` — 安全飞地完善
- [ ] 形式化验证调度器和 IPC 关键路径
- [ ] 性能调优 (IPC/调度/内存)
- [ ] 安全审计
- [ ] **验收**: 无已知安全漏洞，IPC < 1μs，启动 < 3s

### Phase 9: 集成测试 + 发布 (4 周)
- [ ] 端到端系统测试
- [ ] 压力测试 (1000+ Agent)
- [ ] 文档完整性审查
- [ ] 发布 QEMU 镜像
- [ ] **验收**: 全部测试通过

**总计: ~55 周 (约13个月)**

---

## 七、目录结构

```
OmniAgent-OS/
├── os/
│   ├── kernel/                          # 微内核 (Rust)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── scheduler.rs
│   │   │   ├── ipc.rs
│   │   │   ├── memory.rs
│   │   │   ├── agent_syscall.rs
│   │   │   ├── interrupt.rs
│   │   │   ├── device.rs
│   │   │   ├── enclave.rs
│   │   │   ├── timer.rs
│   │   │   └── lib.rs
│   │   └── arch/
│   │       ├── x86_64/
│   │       │   ├── boot.asm
│   │       │   ├── idt.rs
│   │       │   ├── paging.rs
│   │       │   └── mod.rs
│   │       └── aarch64/
│   │           ├── boot.asm
│   │           ├── exceptions.rs
│   │           ├── paging.rs
│   │           └── mod.rs
│   ├── services/                        # 用户态系统服务 (Rust)
│   │   ├── agent_runtime/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── spawner.rs
│   │   │       ├── expert_factory.rs
│   │   │       ├── pool.rs
│   │   │       ├── comm.rs
│   │   │       ├── evolution.rs
│   │   │       └── knowledge.rs
│   │   ├── authorization/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── one_time.rs
│   │   │       ├── permanent.rs
│   │   │       ├── policy.rs
│   │   │       ├── store.rs
│   │   │       └── consent_ui.rs
│   │   ├── memory/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── quantizer.rs
│   │   │       ├── store.rs
│   │   │       ├── embedding.rs    # C FFI → ONNX Runtime
│   │   │       ├── index.rs
│   │   │       └── dream.rs
│   │   └── distributed/
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── discovery.rs
│   │           ├── soft_bus.rs
│   │           ├── migration.rs
│   │           ├── resource.rs
│   │           └── sync.rs
│   ├── services/automation/                   # 自动化引擎服务 (Rust)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── instruction_parser.rs
│   │       ├── task_decomposer.rs
│   │       ├── sequential_executor.rs
│   │       ├── condition_router.rs
│   │       ├── loop_controller.rs
│   │       ├── error_recovery.rs
│   │       ├── template_library.rs
│   │       ├── workflow_def.rs
│   │       ├── workflow_engine.rs
│   │       ├── parallel_scheduler.rs
│   │       ├── event_trigger.rs
│   │       ├── cron_scheduler.rs
│   │       ├── workflow_monitor.rs
│   │       ├── chain_orchestrator.rs
│   │       ├── context_pass.rs
│   │       ├── checkpoint_resume.rs
│   │       ├── operation_sandbox.rs
│   │       └── operation_market.rs
│   ├── services/multimodal/                   # 全模态交互服务 (Rust)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── asr.rs
│   │       ├── tts.rs
│   │       ├── voiceprint.rs
│   │       ├── vad.rs
│   │       ├── far_field.rs
│   │       ├── nlu.rs
│   │       ├── nlg.rs
│   │       ├── summarizer.rs
│   │       ├── translator.rs
│   │       ├── code_understand.rs
│   │       ├── image_understand.rs
│   │       ├── ocr.rs
│   │       ├── image_generate.rs
│   │       ├── segmentation.rs
│   │       ├── screen_understand.rs
│   │       ├── video_understand.rs
│   │       ├── video_summary.rs
│   │       ├── video_generate.rs
│   │       ├── video_stream.rs
│   │       ├── video_edit.rs
│   │       ├── modal_router.rs
│   │       ├── cross_modal_align.rs
│   │       ├── unified_dialog.rs
│   │       └── modal_converter.rs
│   ├── services/learning/                     # 高级学习服务 (Rust)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── curiosity_engine.rs
│   │       ├── knowledge_seeker.rs
│   │       ├── self_experiment.rs
│   │       ├── skill_creator.rs
│   │       ├── active_questioning.rs
│   │       ├── behavior_observer.rs
│   │       ├── feedback_absorber.rs
│   │       ├── error_learner.rs
│   │       ├── pattern_extractor.rs
│   │       ├── preference_learner.rs
│   │       ├── knowledge_graph.rs
│   │       ├── knowledge_fusion.rs
│   │       ├── skill_evolver.rs
│   │       ├── strategy_optimizer.rs
│   │       ├── transfer_learner.rs
│   │       └── forgetting_mgr.rs
│   ├── ui/                              # Aqua Shell (Rust + Vulkan)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── compositor.rs
│   │       ├── window_mgr.rs
│   │       ├── dock.rs
│   │       ├── menubar.rs
│   │       ├── agent_bar.rs
│   │       ├── spotlight.rs
│   │       ├── notification.rs
│   │       ├── gesture.rs
│   │       ├── theme.rs
│   │       └── render/
│   │           ├── vulkan_ctx.rs
│   │           ├── blur.rs
│   │           └── text.rs
│   ├── hal/                             # HAL (Rust)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── cpu.rs
│   │       ├── gpu.rs
│   │       ├── nvme.rs
│   │       ├── nic.rs
│   │       └── serial.rs
│   └── lib/                             # 用户态库
│       ├── libagent/                    # Agent 开发库 (Rust)
│       ├── libui/                       # UI 框架库 (Rust)
│       └── libc/                        # 最小 C 兼容层
├── tools/                               # 开发工具
│   ├── bootloader/
│   ├── qemu-config/
│   ├── debugger/
│   └── profiler/
├── tests/                               # 测试
│   ├── kernel/
│   ├── services/
│   ├── ui/
│   └── benchmarks/
├── docs/                                # 技术文档 (上述文档树)
├── Cargo.toml                           # Rust workspace root
├── Makefile                             # 统一构建
└── rust-toolchain.toml                  # Rust 工具链配置
```

---

## 八、质量保证与测试

### 8.1 测试层次

| 层级 | 类型 | 工具 | 覆盖目标 |
|------|------|------|---------|
| L1 | 内核单元测试 | `#[test]` + `cargo test` | ≥ 90% |
| L2 | 内核集成测试 | QEMU + 自定义 harness | ≥ 85% |
| L3 | 服务单元测试 | `cargo test` | ≥ 85% |
| L4 | 服务集成测试 | QEMU + IPC 测试 | ≥ 80% |
| L5 | UI 渲染测试 | Vulkan 验证层 + 截图对比 | ≥ 70% |
| L6 | 端到端测试 | QEMU 全系统自动化 | 关键路径 100% |
| L7 | 性能基准 | 基准套件 (criterion) | 每次 CI 运行 |
| L8 | 安全测试 | 模糊测试 (cargo-fuzz) | 持续运行 |

### 8.2 性能指标

| 指标 | 目标 |
|------|------|
| IPC 延迟 (同核) | < 1μs |
| Agent spawn 延迟 | < 10ms |
| 授权评估延迟 | < 100μs |
| 窗口合成帧率 | ≥ 60fps |
| 系统启动时间 | < 3s (QEMU) |
| 内存量化压缩率 | ≥ 4x |
| Agent 迁移时间 | < 3s |
| Spotlight 搜索响应 | < 50ms |
| ASR 实时转录延迟 | < 300ms |
| TTS 首字延迟 | < 200ms |
| 图像理解延迟 | < 2s |
| 视频关键帧分析 | < 500ms/frame |
| 工作流启动延迟 | < 100ms |
| 任务分解复杂目标 | < 5s |
| 学习反馈整合 | < 1s |
| 知识图谱查询 | < 10ms |

---

## 九、风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Rust 嵌入式/OS 生态不够成熟 | 中 | 高 | 复用 `no_std` crate 生态，自建缺失组件 |
| IPC 性能不达标 | 中 | 高 | 零拷贝共享内存 + fast-path 优化 |
| Vulkan 合成器开发复杂 | 高 | 中 | 优先实现软件渲染后备，渐进式 GPU 加速 |
| Agent 并发上限 | 中 | 高 | 早期压测，Agent 轻量化（协程模型） |
| 量化记忆精度损失 | 中 | 中 | 多级量化 + 精度评估框架 |
| 分布式一致性 | 高 | 高 | CRDT + 向量时钟，最终一致性模型 |
| 文档与代码不同步 | 中 | 中 | CI 强制检查：每个 PR 必须包含文档变更 |

---

## 十、从 Phase 0 开始的执行步骤

### Step 1: 创建项目骨架
1. `mkdir -p OmniAgent-OS && cd OmniAgent-OS`
2. 初始化 Cargo workspace
3. 创建 `os/kernel/Cargo.toml` (no_std)
4. 创建 `Makefile` (build/run-qemu/test)
5. 配置 QEMU + GRUB 启动

### Step 2: 编写架构文档 (Phase 0)
1. `docs/architecture/system-overview.md`
2. `docs/architecture/microkernel-design.md`
3. `docs/architecture/ipc-protocol.md`
4. `docs/architecture/memory-model.md`
5. `docs/architecture/agent-syscall-abi.md`
6. `docs/architecture/security-model.md`
7. `docs/architecture/boot-process.md`

### Step 3: x86_64 最小内核启动 (Phase 1)
1. `arch/x86_64/boot.asm` — Multiboot2 入口
2. `main.rs` — VGA 文字模式输出 "OmniAgent OS"
3. `memory.rs` — 最简物理内存分配器 (bump allocator)
4. `interrupt.rs` — 基本 IDT 设置
5. QEMU 启动验证

### Step 4: 调度器 + IPC
1. `scheduler.rs` — 轮转调度 → CFS
2. `ipc.rs` — 同步消息传递
3. 用户态 shell 进程
4. IPC 基准测试

### Step 5: Agent Syscall
1. 定义 syscall ABI (syscall 号 512+)
2. 实现 `SYS_AGENT_SPAWN/KILL/QUERY/MSG`
3. `libagent` 用户态库
4. Agent 基本生命周期测试

### Step 6: 授权管理器
1. 一次性授权实现
2. 永久授权实现
3. 策略引擎
4. CLI 授权确认 UI

### Step 7: Agent Runtime 服务
1. Agent 生成器 + 专家工厂
2. Agent 池调度
3. Agent 通信
4. Agent 进化引擎

### Step 8: 量化记忆
1. INT8/INT4 量化器
2. 三层记忆存储
3. ONNX Runtime FFI 嵌入
4. HNSW 向量索引
5. 睡眠整理引擎

### Step 9: 自动化引擎
1. 指令解析器 + 任务分解器
2. 顺序执行器 + 错误恢复器
3. 工作流定义 + 工作流引擎
4. 并行调度 + 事件触发 + Cron 调度
5. 操作链编排 + 断点续执行

### Step 10: 全模态交互
1. 语音管线 (ASR + TTS + VAD)
2. 文本管线 (NLU + NLG + 翻译)
3. 图像管线 (VQA + OCR + 生成)
4. 视频管线 (理解 + 摘要 + 生成)
5. 多模态融合 (路由器 + 对齐 + 统一对话)

### Step 11: 高级学习
1. 主动学习 (好奇心引擎 + 知识获取 + 技能自创)
2. 被动学习 (行为观察 + 反馈吸收 + 错误学习)
3. 知识进化 (知识图谱 + 技能进化 + 遗忘管理)

### Step 12: 分布式能力
1. 设备发现 (mDNS)
2. 软总线 (TCP/TLS)
3. Agent 迁移 (Checkpoint/Restore)
4. 状态同步 (CRDT)

### Step 13: Aqua Shell 桌面
1. Vulkan 上下文初始化
2. 合成器渲染循环
3. 窗口管理器
4. Dock + MenuBar
5. Agent Bar + Spotlight
6. 主题引擎 (设计令牌)
