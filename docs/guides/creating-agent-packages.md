# Agent 包开发指南

> 本指南介绍如何在 OmniAgent OS 中开发、打包和发布 Agent 包。

## 什么是 Agent 包

Agent 包是可分发、可安装的应用单元：

```
my-agent/
├── Agent.toml              # 包清单（必需）
├── Cargo.toml              # Rust 项目配置
├── src/main.rs             # Agent 入口
├── resources/              # 资源文件（可选）
└── tests/                  # 测试文件
```

| 要素 | 说明 |
|------|------|
| **清单** | `Agent.toml`，定义元数据和能力声明 |
| **代码** | Rust 二进制，实现 Agent 逻辑 |
| **资源** | 配置文件、模板、知识库等 |
| **能力** | Agent 被授予的系统权限 |

---

## 包清单格式

```toml
[package]
name = "com.example.file-manager"    # 反向域名格式
version = "1.2.0"                     # 语义化版本
description = "智能文件管理 Agent"
authors = ["Zhang Wei <zhangwei@example.com>"]
license = "MIT"

[agent]
display_name = "文件管家"
type = "service"           # service | tool | daemon | ui
binary = "file-manager"
autostart = true
priority = 50              # 0-255，越小越先启动

[capabilities.ipc]
allowed_ports = ["file-manager.*"]
allowed_connect = ["fsd", "logd", "shell"]
max_connections = 64

[capabilities.filesystem]
allowed_paths = ["/data", "/tmp"]
read_only = false
max_file_size = 104857600  # 100MB

[capabilities.resources]
max_memory = 52428800      # 50MB
cpu_quota = 25
max_processes = 4

[dependencies]
fsd = ">=1.0.0"
logd = ">=0.5.0"

[resources]
knowledge_base = "resources/knowledge/file_types.json"
```

---

## AgentSpec 配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    pub display_name: String,
    pub agent_type: AgentType,       // Service | Tool | Daemon | OneShot
    pub priority: u8,
    pub resources: ResourceSpec,
    pub security_label: SecurityLabel,
    pub restart_policy: RestartPolicy,  // Never | Always | OnFailure { max_retries }
    pub health_check: Option<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub memory_limit: u64, pub cpu_quota: u8,
    pub max_open_files: u32, pub max_ipc_connections: u32, pub disk_quota: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityLabel {
    pub confidentiality: ConfidentialityLevel,  // Public/Internal/Confidential/Secret
    pub integrity: IntegrityLevel,              // Low/Medium/High/Critical
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub check_type: HealthCheckType,  // IpcPing | PortReachable | CustomCommand
    pub interval_ms: u64, pub timeout_ms: u64, pub failure_threshold: u32,
}
```

---

## libagent API 使用

```rust
use libagent::{AgentBuilder, Context, Message, Response};

fn main() {
    AgentBuilder::new("com.example.my-agent")
        .version("1.0.0")
        .handler(handle_message)
        .on_start(|ctx| {
            ctx.log_info("Agent started");
            ctx.register_port("my-service").expect("Port registration failed");
        })
        .on_stop(|ctx| ctx.log_info("Agent shutting down"))
        .build()
        .run();
}

fn handle_message(ctx: &mut Context, msg: Message) -> Response {
    match msg.command() {
        "ping" => Response::ok().data("pong"),
        "status" => Response::ok().data(json!({
            "uptime": ctx.uptime_ms(), "memory_used": ctx.memory_used(),
        })),
        "shutdown" => { ctx.request_shutdown(); Response::ok() }
        _ => Response::error("unknown command"),
    }
}
```

### Context API

```rust
impl Context {
    pub fn log_info(&self, message: &str);
    pub fn log_warn(&self, message: &str);
    pub fn log_error(&self, message: &str);
    pub fn register_port(&mut self, name: &str) -> Result<Port>;
    pub fn connect(&mut self, target: &str) -> Result<Channel>;
    pub fn send(&self, target: &str, msg: &Message) -> Result<()>;
    pub fn request(&self, target: &str, msg: &Message) -> Result<Message>;
    pub fn uptime_ms(&self) -> u64;
    pub fn memory_used(&self) -> u64;
    pub fn request_shutdown(&self);
    pub fn kv_get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    pub fn kv_set(&self, key: &str, value: &[u8]) -> Result<()>;
    pub fn schedule(&self, delay_ms: u64, callback: fn(&mut Context)) -> TimerId;
}
```

---

## 示例：Hello World Agent

```toml
# Agent.toml
[package]
name = "com.example.hello"
version = "0.1.0"
description = "Hello World Agent 示例"

[agent]
display_name = "Hello Agent"
type = "service"
binary = "hello-agent"
autostart = false
priority = 100

[capabilities.ipc]
allowed_ports = ["hello.*"]
allowed_connect = ["shell"]

[capabilities.resources]
max_memory = 8388608
cpu_quota = 10
```

```rust
// main.rs
use libagent::{AgentBuilder, Context, Message, Response};

fn main() {
    AgentBuilder::new("com.example.hello")
        .version("0.1.0")
        .handler(handle_message)
        .on_start(|ctx| {
            ctx.log_info("Hello World Agent is running!");
            ctx.register_port("hello.greeting").unwrap();
        })
        .build()
        .run();
}

fn handle_message(ctx: &mut Context, msg: Message) -> Response {
    match msg.command() {
        "greet" => {
            let name: String = msg.data().unwrap_or_else(|_| "World".to_string());
            ctx.log_info(&format!("Hello, {}!", name));
            Response::ok().data(format!("Hello, {}!", name))
        }
        "info" => Response::ok().data(json!({
            "name": "Hello Agent", "version": "0.1.0", "uptime_ms": ctx.uptime_ms()
        })),
        _ => Response::error(format!("unknown command: {}", msg.command())),
    }
}
```

构建与打包：`cargo build --release --target x86_64-unknown-none && ./tools/agent-pack/build.sh agents/hello-agent`

---

## 示例：文件管理 Agent

```rust
use libagent::{AgentBuilder, Context, Message, Response};
use serde::{Deserialize, Serialize};

fn main() {
    AgentBuilder::new("com.example.file-manager")
        .version("1.0.0")
        .handler(handle_message)
        .on_start(|ctx| { ctx.log_info("File Manager started"); ctx.register_port("file-manager").unwrap(); })
        .build().run();
}

fn handle_message(_ctx: &mut Context, msg: Message) -> Response {
    match msg.command() {
        "list" => {
            let path: String = msg.data().unwrap_or_else(|_| "/".to_string());
            match list_directory(&path) {
                Ok(entries) => Response::ok().data(entries),
                Err(e) => Response::error(format!("Failed: {}", e)),
            }
        }
        "search" => {
            let query: SearchQuery = msg.data().unwrap();
            search_files(&query).map_or_else(
                |e| Response::error(format!("Search failed: {}", e)),
                |r| Response::ok().data(r),
            )
        }
        "batch_move" => {
            let ops: Vec<MoveOp> = msg.data().unwrap();
            let results: Vec<Result<String, String>> = ops.into_iter()
                .map(|op| move_file(&op.source, &op.dest).map_err(|e| e.to_string())).collect();
            Response::ok().data(results)
        }
        _ => Response::error("unknown command"),
    }
}

#[derive(Serialize, Deserialize)]
struct SearchQuery { pattern: String, path: String, recursive: bool, max_results: usize }
#[derive(Serialize, Deserialize)]
struct MoveOp { source: String, dest: String }
#[derive(Serialize)]
struct FileInfo { name: String, path: String, size: u64, is_directory: bool }
fn list_directory(path: &str) -> Result<Vec<FileInfo>, String> { todo!("Via FSD IPC") }
fn search_files(query: &SearchQuery) -> Result<Vec<FileInfo>, String> { todo!() }
fn move_file(src: &str, dst: &str) -> Result<String, String> { todo!() }
```

---

## 示例：数据分析 Agent

```rust
use libagent::{AgentBuilder, Context, Message, Response};
use serde::{Deserialize, Serialize};

fn main() {
    AgentBuilder::new("com.example.data-analyzer")
        .version("1.0.0").handler(handle_message).build().run();
}

fn handle_message(ctx: &mut Context, msg: Message) -> Response {
    match msg.command() {
        "analyze" => {
            let req: AnalysisRequest = match msg.data() {
                Ok(r) => r, Err(e) => return Response::error(format!("Invalid: {}", e)),
            };
            match perform_analysis(&req) {
                Ok(result) => { ctx.log_info(&format!("Analyzed {} rows", result.rows_processed));
                    Response::ok().data(result) }
                Err(e) => Response::error(format!("Failed: {}", e)),
            }
        }
        "summarize" => {
            let data: Vec<f64> = match msg.data() {
                Ok(d) => d, Err(e) => return Response::error(format!("Invalid: {}", e)),
            };
            Response::ok().data(DataSummary::from_data(&data))
        }
        _ => Response::error("unknown command"),
    }
}

#[derive(Serialize)]
struct DataSummary {
    count: usize, mean: f64, median: f64, std_dev: f64,
    min: f64, max: f64, q1: f64, q3: f64,
}

impl DataSummary {
    fn from_data(data: &[f64]) -> Self {
        if data.is_empty() { return Self { count: 0, mean: 0.0, median: 0.0, std_dev: 0.0,
            min: 0.0, max: 0.0, q1: 0.0, q3: 0.0 }; }
        let n = data.len();
        let mean = data.iter().sum::<f64>() / n as f64;
        let var = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Self { count: n, mean, median: sorted[n/2], std_dev: var.sqrt(),
            min: sorted[0], max: sorted[n-1], q1: sorted[n/4], q3: sorted[n*3/4] }
    }
}

#[derive(Deserialize)]
struct AnalysisRequest { data_source: String, columns: Vec<String>, operations: Vec<String> }
#[derive(Serialize)]
struct AnalysisResult { rows_processed: usize, results: Vec<serde_json::Value> }
fn perform_analysis(_req: &AnalysisRequest) -> Result<AnalysisResult, String> { todo!() }
```

---

## Agent 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_data_summary_empty() { assert_eq!(DataSummary::from_data(&[]).count, 0); }
    #[test]
    fn test_data_summary_basic() {
        let s = DataSummary::from_data(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(s.mean, 3.0); assert_eq!(s.min, 1.0);
    }
}
```

集成测试使用 `libagent::testing::{TestContext, TestMessage}` 模拟消息交互。

---

## 发布到运营市场

```bash
./tools/agent-pack/build.sh agents/my-agent
./tools/agent-pack/verify.sh target/packages/my-agent-1.0.0.oap
./tools/agent-pack/publish.sh target/packages/my-agent-1.0.0.oap --marketplace production
```

发布前检查：Agent.toml 格式正确、能力声明最小化、测试通过、文档完整、无敏感信息。

---

## Agent 安全考虑

### 最小权限原则

```toml
# 错误：过多权限
[capabilities.ipc]
allowed_connect = ["*"]
# 正确：仅声明必要权限
[capabilities.ipc]
allowed_connect = ["fsd", "logd"]
```

### 输入验证

对所有外部输入进行严格校验，防止路径穿越、注入等攻击。

---

## 最佳实践

1. **单一职责**：每个 Agent 只做一件事
2. **无状态优先**：减少内部状态，通过 IPC 通信
3. **优雅降级**：依赖不可用时提供降级行为
4. **幂等操作**：重复调用不应产生副作用
5. **超时处理**：所有 IPC 调用设置超时
