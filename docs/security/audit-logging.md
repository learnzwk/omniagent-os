# OmniAgent OS 审计日志规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: 正式发布
> **责任团队**: 安全工程与运维组

---

## 1. 概述

### 1.1 设计目标

OmniAgent OS 审计日志系统提供全面、不可篡改的操作记录，用于安全监控、合规审计和事件取证。

| 目标 | 描述 |
|------|------|
| **完整性** | 日志条目不可被删除或篡改 |
| **不可抵赖** | 操作者无法否认其执行的操作 |
| **可追溯** | 任何操作都可以追溯到具体实体 |
| **实时性** | 安全事件可实时告警 |
| **高性能** | 日志记录开销 < 1ms |
| **隐私保护** | 敏感数据自动脱敏 |

### 1.2 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                     日志生产者                                │
│  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌───────┐ │
│  │ 微内核  │  │ 服务层  │  │ Agent  │  │ 用户   │  │ 桌面  │ │
│  └───┬────┘  └───┬────┘  └───┬────┘  └───┬────┘  └──┬────┘ │
│      │           │           │           │          │       │
├──────┼───────────┼───────────┼───────────┼──────────┼───────┤
│      ▼           ▼           ▼           ▼          ▼       │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              审计日志管道 (Audit Pipeline)              │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐    │   │
│  │  │ 事件收集  │→│ 脱敏过滤  │→│ 哈希链计算        │    │   │
│  │  │ (Collect)│  │ (Filter) │  │ (Hash Chain)     │    │   │
│  │  └──────────┘  └──────────┘  └──────────────────┘    │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐    │   │
│  │  │ 实时告警  │  │ 批量写入  │  │ Merkle 树更新     │    │   │
│  │  │ (Alert)  │  │ (Write)  │  │ (Merkle Tree)    │    │   │
│  │  └──────────┘  └──────────┘  └──────────────────┘    │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                   │
│                          ▼                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    日志存储                            │   │
│  │  ┌──────────────┐  ┌────────────────────────────┐   │   │
│  │  │ 追加写入文件  │  │ 循环缓冲区 (最近事件)       │   │   │
│  │  │ (Append-Only)│  │ (Circular Buffer)          │   │   │
│  │  └──────────────┘  └────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    日志消费者                          │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │   │
│  │  │ 查询 API  │  │ 告警引擎  │  │ 合规报告生成     │   │   │
│  │  └──────────┘  └──────────┘  └──────────────────┘   │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 日志格式

### 2.1 结构化 JSON 格式

每条审计日志采用结构化 JSON 格式，包含以下字段：

```json
{
    "log_id": "aud-20260425-00000001-ab12cd34",
    "timestamp": "2026-04-25T10:30:45.123456789+08:00",
    "sequence": 12345678,
    "event_type": "auth.token_used",
    "actor": {
        "id": "agent-42",
        "type": "agent",
        "name": "file-assistant",
        "capabilities": ["file_read", "ipc"]
    },
    "target": {
        "id": "/documents/report.md",
        "type": "file",
        "service": "file-service"
    },
    "action": "read",
    "result": "success",
    "metadata": {
        "bytes_read": 4096,
        "duration_us": 23,
        "token_id": "tok-abc123"
    },
    "security": {
        "prev_hash": "blake3:ef2d...a1b2",
        "this_hash": "blake3:c3d4...e5f6",
        "merkle_root": "blake3:789a...bcde"
    },
    "source": {
        "hostname": "omniagent-node-01",
        "process_id": 42,
        "thread_id": 7
    }
}
```

### 2.2 字段定义

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `log_id` | string | 是 | 日志唯一标识 (时间戳+随机后缀) |
| `timestamp` | string (ISO 8601) | 是 | 事件发生时间 (纳秒精度) |
| `sequence` | integer | 是 | 单调递增序列号 |
| `event_type` | string | 是 | 事件类型 (点分隔命名空间) |
| `actor` | object | 是 | 操作发起者 |
| `target` | object | 是 | 操作目标 |
| `action` | string | 是 | 执行的动作 |
| `result` | string | 是 | 操作结果 (success/failure/denied) |
| `metadata` | object | 否 | 事件附加信息 |
| `security` | object | 是 | 安全相关字段 (哈希链) |
| `source` | object | 是 | 日志来源信息 |

### 2.3 事件类型命名空间

```
auth.*              # 授权相关事件
  auth.token_created       # 令牌创建
  auth.token_used          # 令牌使用
  auth.token_revoked       # 令牌撤销
  auth.grant_created       # 授权创建
  auth.grant_revoked       # 授权撤销
  auth.policy_evaluated    # 策略评估
  auth.denied              # 授权拒绝

kernel.*            # 内核相关事件
  kernel.syscall           # 系统调用
  kernel.page_fault        # 页错误
  kernel.sched_switch      # 调度切换
  kernel.ipc_send          # IPC 发送
  kernel.ipc_recv          # IPC 接收
  kernel.interrupt         # 中断处理
  kernel.panic             # 内核崩溃

agent.*             # Agent 相关事件
  agent.spawned            # Agent 生成
  agent.terminated         # Agent 终止
  agent.message_sent       # 消息发送
  agent.message_received   # 消息接收
  agent.capability_used    # 能力使用
  agent.resource_exceeded  # 资源超限

service.*           # 服务相关事件
  service.started          # 服务启动
  service.stopped          # 服务停止
  service.crashed          # 服务崩溃
  service.config_changed   # 配置变更

security.*          # 安全相关事件
  security.anomaly_detected  # 异常检测
  security.breach_attempt    # 入侵尝试
  security.fuzz_crash        # 模糊测试崩溃
  security.integrity_violation  # 完整性违规

user.*              # 用户相关事件
  user.login               # 用户登录
  user.logout              # 用户注销
  user.consent_given       # 用户授权
  user.consent_denied      # 用户拒绝
  user.privilege_changed   # 权限变更

desktop.*           # 桌面相关事件
  desktop.window_created   # 窗口创建
  desktop.window_destroyed # 窗口销毁
  desktop.input_event      # 输入事件
  desktop.clipboard_access # 剪贴板访问
```

---

## 3. 防篡改哈希链

### 3.1 BLAKE3 哈希链

```rust
/// 审计日志条目
pub struct AuditEntry {
    /// 日志 ID
    pub log_id: String,
    /// 时间戳
    pub timestamp: Timestamp,
    /// 序列号
    pub sequence: u64,
    /// 事件类型
    pub event_type: String,
    /// 操作者
    pub actor: AuditActor,
    /// 目标
    pub target: AuditTarget,
    /// 动作
    pub action: String,
    /// 结果
    pub result: AuditResult,
    /// 元数据
    pub metadata: serde_json::Value,
    /// 前一条日志的哈希 (链式结构)
    pub prev_hash: [u8; 32],
    /// 本条日志的哈希
    pub this_hash: [u8; 32],
}

/// 哈希链管理器
pub struct HashChain {
    /// 当前链头哈希
    current_hash: [u8; 32],
    /// 链长度
    length: u64,
    /// 链根哈希 (创世哈希)
    genesis_hash: [u8; 32],
}

impl HashChain {
    /// 创建新的哈希链
    pub fn new() -> Self {
        let genesis_hash = blake3::hash(b"omniagent-audit-genesis").into();
        Self {
            current_hash: genesis_hash,
            length: 0,
            genesis_hash,
        }
    }

    /// 计算日志条目的哈希
    pub fn compute_hash(&self, entry: &AuditEntryData) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();

        // 包含前一条日志的哈希 (防止删除/插入)
        hasher.update(&self.current_hash);

        // 包含所有关键字段
        hasher.update(entry.timestamp.to_bytes());
        hasher.update(entry.sequence.to_le_bytes());
        hasher.update(entry.event_type.as_bytes());
        hasher.update(entry.actor.id.as_bytes());
        hasher.update(entry.target.id.as_bytes());
        hasher.update(entry.action.as_bytes());
        hasher.update(entry.result.as_bytes());

        // 包含元数据的哈希
        let metadata_json = serde_json::to_string(&entry.metadata)
            .unwrap_or_default();
        let metadata_hash = blake3::hash(metadata_json.as_bytes());
        hasher.update(metadata_hash.as_bytes());

        hasher.finalize().into()
    }

    /// 验证哈希链完整性
    pub fn verify_chain(&self, entries: &[AuditEntry]) -> ChainVerificationResult {
        let mut prev_hash = self.genesis_hash;

        for (i, entry) in entries.iter().enumerate() {
            // 验证前一条哈希链接
            if entry.prev_hash != prev_hash {
                return ChainVerificationResult::Broken {
                    position: i,
                    reason: "前一条哈希不匹配".to_string(),
                };
            }

            // 重新计算哈希并验证
            let computed = self.recompute_hash(entry);
            if computed != entry.this_hash {
                return ChainVerificationResult::Broken {
                    position: i,
                    reason: "条目哈希不匹配 (可能被篡改)".to_string(),
                };
            }

            prev_hash = entry.this_hash;
        }

        ChainVerificationResult::Valid {
            entries_verified: entries.len(),
        }
    }
}
```

### 3.2 Merkle 树验证

```rust
/// Merkle 树用于高效验证日志子集
pub struct AuditMerkleTree {
    /// 树节点
    nodes: Vec<[u8; 32]>,
    /// 叶子节点数量
    leaf_count: u64,
    /// 树深度
    depth: u32,
}

impl AuditMerkleTree {
    /// 添加日志条目到 Merkle 树
    pub fn append(&mut self, entry_hash: [u8; 32]) -> [u8; 32] {
        // 添加叶子节点
        self.nodes.push(entry_hash);
        self.leaf_count += 1;

        // 向上更新父节点
        let mut index = self.nodes.len() - 1;
        while index > 0 {
            let sibling = if index % 2 == 0 { index - 1 } else { index + 1 };
            let parent = (index - 1) / 2;

            let mut hasher = blake3::Hasher::new();
            hasher.update(&self.nodes[index]);
            if sibling < self.nodes.len() {
                hasher.update(&self.nodes[sibling]);
            } else {
                hasher.update(&self.nodes[index]); // 单节点自哈希
            }
            self.nodes[parent] = hasher.finalize().into();

            index = parent;
        }

        // 返回根哈希
        self.nodes[0]
    }

    /// 获取 Merkle 根
    pub fn root(&self) -> [u8; 32] {
        self.nodes[0]
    }

    /// 生成 Merkle 证明 (用于验证单个条目)
    pub fn generate_proof(&self, leaf_index: u64) -> MerkleProof {
        let mut proof = Vec::new();
        let mut index = leaf_index as usize;

        while index > 0 {
            let sibling = if index % 2 == 0 { index - 1 } else { index + 1 };
            let sibling_hash = if sibling < self.nodes.len() {
                Some(self.nodes[sibling])
            } else {
                None
            };
            proof.push((sibling_hash, index % 2 == 0));

            index = (index - 1) / 2;
        }

        MerkleProof {
            leaf_hash: self.nodes[leaf_index as usize],
            proof_path: proof,
            root_hash: self.nodes[0],
        }
    }

    /// 验证 Merkle 证明
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let mut current = proof.leaf_hash;

        for (sibling_hash, is_right) in &proof.proof_path {
            let mut hasher = blake3::Hasher::new();
            if *is_right {
                hasher.update(sibling_hash.as_ref().unwrap_or(&current));
                hasher.update(&current);
            } else {
                hasher.update(&current);
                hasher.update(sibling_hash.as_ref().unwrap_or(&current));
            }
            current = hasher.finalize().into();
        }

        current == proof.root_hash
    }
}
```

---

## 4. 日志收集

### 4.1 事件收集器

```rust
/// 审计事件收集器
pub struct AuditCollector {
    /// 日志管道发送端
    sender: crossbeam_channel::Sender<AuditEntryData>,
    /// 本地缓冲区 (异步写入)
    buffer: Vec<AuditEntryData>,
    /// 缓冲区大小阈值
    flush_threshold: usize,
    /// 脱敏规则
    redaction_rules: Vec<RedactionRule>,
}

impl AuditCollector {
    /// 记录审计事件 (异步，非阻塞)
    pub fn record(&self, event: AuditEvent) {
        let entry = self.event_to_entry(event);

        // 应用脱敏规则
        let redacted = self.apply_redaction(&entry);

        // 非阻塞发送到管道
        if self.sender.try_send(redacted).is_err() {
            // 管道满时记录到本地缓冲区
            self.buffer.push(redacted);
        }
    }

    /// 记录审计事件 (同步，确保写入)
    pub fn record_sync(&self, event: AuditEvent) -> Result<(), AuditError> {
        let entry = self.event_to_entry(event);
        let redacted = self.apply_redaction(&entry);
        self.sender.send(redacted)?;
        Ok(())
    }

    /// 将审计事件转换为日志条目
    fn event_to_entry(&self, event: AuditEvent) -> AuditEntryData {
        AuditEntryData {
            log_id: self.generate_log_id(),
            timestamp: Timestamp::now(),
            sequence: self.next_sequence(),
            event_type: event.event_type(),
            actor: event.actor(),
            target: event.target(),
            action: event.action(),
            result: event.result(),
            metadata: event.metadata(),
        }
    }
}
```

### 4.2 各层事件收集

| 来源 | 收集方式 | 事件类型 |
|------|---------|---------|
| **内核** | 系统调用钩子 + IPC 拦截 | `kernel.*`, `auth.*` |
| **服务** | 服务框架自动记录 | `service.*`, `auth.*` |
| **Agent** | Agent 运行时自动记录 | `agent.*`, `auth.*` |
| **用户** | 桌面管理器 + 输入事件 | `user.*`, `desktop.*` |
| **安全** | 安全模块主动记录 | `security.*` |

### 4.3 内核事件收集

```rust
/// 内核审计钩子 - 在系统调用路径中嵌入
pub fn syscall_audit_hook(
    process: &Process,
    syscall: &Syscall,
    result: &SyscallResult,
) {
    let event = AuditEvent::Syscall {
        actor: AuditActor {
            id: process.pid().to_string(),
            entity_type: EntityType::Process,
            name: process.name().to_string(),
        },
        action: syscall.name().to_string(),
        target: syscall.target().map(|t| AuditTarget {
            id: t.to_string(),
            target_type: syscall.target_type().to_string(),
        }),
        result: if result.is_ok() {
            AuditResult::Success
        } else {
            AuditResult::Failure(result.err().unwrap().to_string())
        },
        metadata: serde_json::json!({
            "syscall_id": syscall.id(),
            "duration_ns": syscall.duration().as_nanos(),
        }),
    };

    audit_collector().record(event);
}
```

---

## 5. 日志存储

### 5.1 追加写入文件

```rust
/// 追加写入日志存储
pub struct AppendOnlyLogStore {
    /// 日志文件句柄
    file: std::fs::File,
    /// 当前文件大小
    file_size: u64,
    /// 最大文件大小 (超过后轮转)
    max_file_size: u64,
    /// 当前文件编号
    file_index: u32,
    /// 存储目录
    directory: PathBuf,
    /// 写入锁
    write_lock: std::sync::Mutex<()>,
}

impl AppendOnlyLogStore {
    /// 追加写入日志条目
    pub fn append(&mut self, entry: &AuditEntry) -> Result<(), StorageError> {
        let _lock = self.write_lock.lock().unwrap();

        // 序列化为 JSON Lines 格式
        let json = serde_json::to_string(entry)?;
        let mut data = json.into_bytes();
        data.push(b'\n');

        // 原子写入
        self.file.write_all(&data)?;
        self.file.sync_all()?; // 确保持久化

        self.file_size += data.len() as u64;

        // 文件轮转
        if self.file_size >= self.max_file_size {
            self.rotate()?;
        }

        Ok(())
    }

    /// 文件轮转
    fn rotate(&mut self) -> Result<(), StorageError> {
        self.file_index += 1;
        let new_path = self.directory.join(format!(
            "audit-{}.{}.logl",
            Timestamp::now().format("%Y%m%d"),
            self.file_index
        ));

        self.file = std::fs::OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&new_path)?;
        self.file_size = 0;

        Ok(())
    }
}
```

### 5.2 循环缓冲区

```rust
/// 循环缓冲区 - 用于快速访问最近的日志事件
pub struct CircularAuditBuffer {
    /// 缓冲区
    buffer: Vec<AuditEntry>,
    /// 写入位置
    write_pos: usize,
    /// 缓冲区容量
    capacity: usize,
    /// 读取锁
    read_lock: RwLock<()>,
}

impl CircularAuditBuffer {
    /// 写入日志条目 (覆盖最旧的数据)
    pub fn push(&mut self, entry: AuditEntry) {
        self.buffer[self.write_pos] = entry;
        self.write_pos = (self.write_pos + 1) % self.capacity;
    }

    /// 读取最近的 N 条日志
    pub fn read_recent(&self, count: usize) -> Vec<&AuditEntry> {
        let count = count.min(self.capacity);
        let mut result = Vec::with_capacity(count);

        for i in 0..count {
            let pos = (self.write_pos + self.capacity - 1 - i) % self.capacity;
            result.push(&self.buffer[pos]);
        }

        result
    }

    /// 按时间范围查询
    pub fn query_by_time_range(
        &self,
        start: Timestamp,
        end: Timestamp,
    ) -> Vec<&AuditEntry> {
        self.buffer.iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }
}
```

---

## 6. 日志分析

### 6.1 查询 API

```rust
/// 审计日志查询 API
pub struct AuditQueryApi {
    store: AppendOnlyLogStore,
    buffer: CircularAuditBuffer,
    merkle_tree: AuditMerkleTree,
}

impl AuditQueryApi {
    /// 按事件类型查询
    pub fn query_by_event_type(
        &self,
        event_type: &str,
        limit: usize,
    ) -> Vec<AuditEntry> {
        // 优先从循环缓冲区查询 (最近事件)
        let recent: Vec<_> = self.buffer.buffer.iter()
            .filter(|e| e.event_type.starts_with(event_type))
            .take(limit)
            .cloned()
            .collect();

        if recent.len() >= limit {
            return recent;
        }

        // 从文件存储查询更多
        let file_results = self.store.search_by_event_type(
            event_type,
            limit - recent.len(),
        );

        let mut results = file_results;
        results.extend(recent);
        results
    }

    /// 按操作者查询
    pub fn query_by_actor(
        &self,
        actor_id: &str,
        limit: usize,
    ) -> Vec<AuditEntry> {
        self.buffer.buffer.iter()
            .filter(|e| e.actor.id == actor_id)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 按目标查询
    pub fn query_by_target(
        &self,
        target_id: &str,
        limit: usize,
    ) -> Vec<AuditEntry> {
        self.buffer.buffer.iter()
            .filter(|e| e.target.id == target_id)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 按时间范围查询
    pub fn query_by_time_range(
        &self,
        start: Timestamp,
        end: Timestamp,
    ) -> Vec<AuditEntry> {
        self.buffer.query_by_time_range(start, end)
            .into_iter()
            .cloned()
            .collect()
    }

    /// 组合查询
    pub fn query(&self, filter: AuditFilter) -> Vec<AuditEntry> {
        let mut results: Vec<_> = self.buffer.buffer.iter()
            .filter(|e| filter.matches(e))
            .cloned()
            .collect();

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        results.truncate(filter.limit.unwrap_or(100));
        results
    }
}

/// 查询过滤器
pub struct AuditFilter {
    pub event_type: Option<String>,
    pub actor_id: Option<String>,
    pub target_id: Option<String>,
    pub action: Option<String>,
    pub result: Option<String>,
    pub time_start: Option<Timestamp>,
    pub time_end: Option<Timestamp>,
    pub limit: Option<usize>,
}

impl AuditFilter {
    pub fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(ref et) = self.event_type {
            if !entry.event_type.starts_with(et) { return false; }
        }
        if let Some(ref aid) = self.actor_id {
            if entry.actor.id != *aid { return false; }
        }
        if let Some(ref tid) = self.target_id {
            if entry.target.id != *tid { return false; }
        }
        if let Some(ref act) = self.action {
            if entry.action != *act { return false; }
        }
        if let Some(ref res) = self.result {
            if entry.result.as_str() != res { return false; }
        }
        if let Some(start) = self.time_start {
            if entry.timestamp < start { return false; }
        }
        if let Some(end) = self.time_end {
            if entry.timestamp > end { return false; }
        }
        true
    }
}
```

### 6.2 实时告警

```rust
/// 告警规则
pub struct AlertRule {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 匹配条件
    pub condition: AlertCondition,
    /// 告警级别
    pub severity: AlertSeverity,
    /// 告警动作
    pub actions: Vec<AlertAction>,
    /// 冷却时间 (防止告警风暴)
    pub cooldown: Duration,
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    /// 事件类型匹配
    EventTypeMatches(String),
    /// 授权失败次数超过阈值
    AuthFailureThreshold { count: u32, window: Duration },
    /// 资源使用异常
    ResourceAnomaly { resource: String, threshold: f64 },
    /// 可疑操作模式
    SuspiciousPattern { pattern: String },
    /// IP 信誉检查
    IpReputation { min_score: u8 },
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Critical,  // 立即处理
    High,      // 1 小时内处理
    Medium,    // 24 小时内处理
    Low,       // 记录并监控
}

/// 告警引擎
pub struct AlertEngine {
    rules: Vec<AlertRule>,
    state: AlertState,
    notification: NotificationService,
}

impl AlertEngine {
    /// 处理审计事件
    pub fn process_event(&mut self, event: &AuditEntry) {
        for rule in &self.rules {
            if self.evaluate_condition(&rule.condition, event) {
                if !self.is_in_cooldown(&rule.id) {
                    self.trigger_alert(rule, event);
                    self.set_cooldown(&rule.id, rule.cooldown);
                }
            }
        }
    }

    /// 触发告警
    fn trigger_alert(&self, rule: &AlertRule, event: &AuditEntry) {
        let alert = Alert {
            id: generate_alert_id(),
            rule_id: rule.id.clone(),
            severity: rule.severity.clone(),
            event: event.clone(),
            timestamp: Timestamp::now(),
        };

        // 执行告警动作
        for action in &rule.actions {
            match action {
                AlertAction::NotifyAdmin => {
                    self.notification.send_admin_notification(&alert);
                }
                AlertAction::BlockActor => {
                    self.block_actor(&event.actor.id);
                }
                AlertAction::IncreaseLogging => {
                    self.increase_log_verbosity(&event.actor.id);
                }
                AlertAction::Webhook(url) => {
                    self.notification.send_webhook(url, &alert);
                }
            }
        }

        // 记录告警到审计日志
        audit_log::record(AuditEvent::AlertTriggered {
            alert_id: alert.id.clone(),
            rule_name: rule.name.clone(),
            severity: format!("{:?}", rule.severity),
            actor: event.actor.id.clone(),
        });
    }
}
```

### 6.3 历史分析

```rust
/// 审计日志分析器
pub struct AuditAnalyzer {
    query_api: AuditQueryApi,
}

impl AuditAnalyzer {
    /// 生成安全摘要报告
    pub fn generate_security_summary(
        &self,
        period: Duration,
    ) -> SecuritySummary {
        let end = Timestamp::now();
        let start = end - period;

        let events = self.query_api.query(AuditFilter {
            time_start: Some(start),
            time_end: Some(end),
            ..Default::default()
        });

        let total_events = events.len();
        let auth_failures = events.iter()
            .filter(|e| e.event_type.starts_with("auth.denied"))
            .count();
        let security_events = events.iter()
            .filter(|e| e.event_type.starts_with("security."))
            .count();
        let unique_actors = events.iter()
            .map(|e| &e.actor.id)
            .collect::<HashSet<_>>()
            .len();

        // 检测异常模式
        let anomalies = self.detect_anomalies(&events);

        SecuritySummary {
            period_start: start,
            period_end: end,
            total_events,
            auth_failures,
            security_events,
            unique_actors,
            anomalies,
            risk_score: self.calculate_risk_score(auth_failures, security_events, anomalies.len()),
        }
    }

    /// 异常检测
    fn detect_anomalies(&self, events: &[AuditEntry]) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // 检测高频授权失败
        let auth_failures_by_actor: HashMap<&str, usize> = events.iter()
            .filter(|e| e.event_type == "auth.denied")
            .map(|e| (e.actor.id.as_str(), 1))
            .fold(HashMap::new(), |mut acc, (k, v)| {
                *acc.entry(k).or_insert(0) += v;
                acc
            });

        for (actor, count) in auth_failures_by_actor {
            if count > 10 {
                anomalies.push(Anomaly {
                    anomaly_type: "高频授权失败".to_string(),
                    description: format!("实体 {} 在分析周期内有 {} 次授权失败", actor, count),
                    severity: if count > 50 { AlertSeverity::Critical } else { AlertSeverity::High },
                    actor: actor.to_string(),
                });
            }
        }

        // 检测异常时间活动
        let night_events: Vec<_> = events.iter()
            .filter(|e| e.timestamp.hour() >= 0 && e.timestamp.hour() < 6)
            .collect();

        if night_events.len() > 100 {
            anomalies.push(Anomaly {
                anomaly_type: "异常时间活动".to_string(),
                description: format!(
                    "凌晨时段 (0-6时) 检测到 {} 个事件", night_events.len()
                ),
                severity: AlertSeverity::Medium,
                actor: String::new(),
            });
        }

        anomalies
    }
}
```

---

## 7. 隐私保护

### 7.1 数据脱敏规则

```rust
/// 脱敏规则
pub struct RedactionRule {
    /// 规则名称
    pub name: String,
    /// 匹配模式 (正则表达式)
    pub pattern: Regex,
    /// 替换策略
    pub strategy: RedactionStrategy,
    /// 适用的事件类型
    pub event_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum RedactionStrategy {
    /// 完全替换为 [REDACTED]
    FullRedact,
    /// 部分遮蔽 (如 abc***xyz)
    PartialMask { prefix_len: usize, suffix_len: usize },
    /// 哈希替换
    HashReplace,
    /// 替换为固定值
    ReplaceWith(String),
}

/// 预定义脱敏规则
pub fn default_redaction_rules() -> Vec<RedactionRule> {
    vec![
        // API 密钥
        RedactionRule {
            name: "api_key".to_string(),
            pattern: Regex::new(r"(?i)api[_-]?key['\"]?\s*[:=]\s*['\"]?([a-zA-Z0-9_-]{20,})").unwrap(),
            strategy: RedactionStrategy::FullRedact,
            event_types: vec!["agent.*".to_string(), "service.*".to_string()],
        },
        // 密码
        RedactionRule {
            name: "password".to_string(),
            pattern: Regex::new(r"(?i)password['\"]?\s*[:=]\s*['\"]?(\S+)").unwrap(),
            strategy: RedactionStrategy::FullRedact,
            event_types: vec!["user.*".to_string(), "auth.*".to_string()],
        },
        // IP 地址 (部分遮蔽)
        RedactionRule {
            name: "ip_address".to_string(),
            pattern: Regex::new(r"\b(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\b").unwrap(),
            strategy: RedactionStrategy::PartialMask { prefix_len: 1, suffix_len: 1 },
            event_types: vec!["*".to_string()],
        },
        // 用户名
        RedactionRule {
            name: "username".to_string(),
            pattern: Regex::new(r"(?i)username['\"]?\s*[:=]\s*['\"]?(\S+)").unwrap(),
            strategy: RedactionStrategy::PartialMask { prefix_len: 1, suffix_len: 0 },
            event_types: vec!["user.*".to_string()],
        },
        // 文件路径中的用户目录
        RedactionRule {
            name: "home_path".to_string(),
            pattern: Regex::new(r"/home/([^/]+)/").unwrap(),
            strategy: RedactionStrategy::ReplaceWith("/home/[USER]/".to_string()),
            event_types: vec!["*".to_string()],
        },
    ]
}
```

### 7.2 保留策略

| 数据类别 | 保留期限 | 存储位置 | 加密 |
|---------|---------|---------|------|
| 安全事件 (Critical/High) | 7 年 | 追加写入文件 | AES-256 |
| 授权事件 | 3 年 | 追加写入文件 | AES-256 |
| 系统事件 | 1 年 | 追加写入文件 | AES-256 |
| 调试事件 | 30 天 | 追加写入文件 | 可选 |
| 循环缓冲区 | 最近 100K 条 | 内存 | 不加密 |

---

## 8. 性能优化

### 8.1 异步日志管道

```rust
/// 高性能异步日志管道
pub struct AsyncAuditPipeline {
    /// 事件接收通道
    receiver: crossbeam_channel::Receiver<AuditEntryData>,
    /// 写入线程
    writer_thread: std::thread::JoinHandle<()>,
    /// 批量写入缓冲区
    batch_buffer: Vec<AuditEntryData>,
    /// 批量大小
    batch_size: usize,
    /// 刷新间隔
    flush_interval: Duration,
}

impl AsyncAuditPipeline {
    /// 启动异步写入线程
    pub fn start(store: AppendOnlyLogStore, buffer: CircularAuditBuffer) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();

        let writer_thread = std::thread::Builder::new()
            .name("audit-writer".to_string())
            .spawn(move || {
                let mut batch = Vec::with_capacity(100);
                let mut last_flush = Instant::now();

                loop {
                    // 批量收集事件
                    while batch.len() < 100 {
                        match receiver.recv_timeout(Duration::from_millis(100)) {
                            Ok(entry) => batch.push(entry),
                            Err(crossbeam_channel::RecvTimeoutError::Timeout) => break,
                            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                        }
                    }

                    // 定时刷新
                    if !batch.is_empty() &&
                        last_flush.elapsed() >= Duration::from_secs(1) {
                        // 批量写入
                        for entry in &batch {
                            let _ = store.append(entry);
                            buffer.push(entry.clone());
                        }
                        batch.clear();
                        last_flush = Instant::now();
                    }
                }
            })
            .expect("无法启动审计写入线程");

        Self {
            receiver,
            writer_thread,
            batch_buffer: Vec::new(),
            batch_size: 100,
            flush_interval: Duration::from_secs(1),
        }
    }
}
```

### 8.2 性能指标

| 指标 | 目标值 | 测量条件 |
|------|--------|---------|
| 单条日志记录延迟 | < 1 us | 内存操作 (无 IO) |
| 批量写入吞吐 | > 100K 条/秒 | 100 条批量 |
| 查询延迟 (最近 100 条) | < 5 ms | 循环缓冲区 |
| 查询延迟 (时间范围) | < 100 ms | 1 天范围 |
| Merkle 树更新 | < 5 us | 单条追加 |
| 哈希链验证 (1K 条) | < 1 ms | 纯计算 |
| 日志存储空间 | < 500 字节/条 | 压缩后 |

---

## 9. 合规支持

### 9.1 导出格式

```rust
/// 日志导出格式
pub enum ExportFormat {
    /// JSON Lines
    JsonLines,
    /// CSV
    Csv,
    /// Syslog (RFC 5424)
    Syslog,
    /// CEF (Common Event Format)
    Cef,
    /// STIX (Structured Threat Information Expression)
    Stix,
}

impl AuditQueryApi {
    /// 导出审计日志
    pub fn export(
        &self,
        filter: &AuditFilter,
        format: ExportFormat,
    ) -> Result<Vec<u8>, ExportError> {
        let entries = self.query(filter.clone());

        match format {
            ExportFormat::JsonLines => {
                let lines: Vec<String> = entries.iter()
                    .map(|e| serde_json::to_string(e).unwrap())
                    .collect();
                Ok(lines.join("\n").into_bytes())
            }
            ExportFormat::Csv => {
                let mut csv = csv::Writer::from_vec();
                for entry in &entries {
                    csv.serialize(entry)?;
                }
                Ok(csv.into_inner())
            }
            ExportFormat::Syslog => {
                let lines: Vec<String> = entries.iter()
                    .map(|e| self.to_syslog_format(e))
                    .collect();
                Ok(lines.join("\n").into_bytes())
            }
            _ => Err(ExportError::UnsupportedFormat),
        }
    }
}
```

### 9.2 合规报告

| 合规标准 | 要求 | 支持情况 |
|---------|------|---------|
| **SOC 2** | 完整审计追踪 | 完全支持 |
| **ISO 27001** | 访问控制日志 | 完全支持 |
| **GDPR** | 数据处理记录 + 删除权 | 支持 (脱敏 + 保留策略) |
| **PCI DSS** | 10 年日志保留 | 支持 (可配置保留期) |
| **等保 2.0** | 三级审计要求 | 支持 |

---

## 10. 日志完整性验证

### 10.1 定期 Merkle 根检查

```rust
/// 完整性验证调度器
pub struct IntegrityVerifier {
    merkle_tree: AuditMerkleTree,
    store: AppendOnlyLogStore,
    check_interval: Duration,
    last_root: [u8; 32],
}

impl IntegrityVerifier {
    /// 执行完整性验证
    pub fn verify(&mut self) -> IntegrityReport {
        let current_root = self.merkle_tree.root();

        // 比较当前根与上次记录的根
        let root_changed = current_root != self.last_root;

        // 验证哈希链
        let recent_entries = self.store.read_recent(1000);
        let chain_result = self.merkle_tree.verify_chain(&recent_entries);

        // 随机抽样验证 Merkle 证明
        let mut proof_failures = 0;
        let sample_count = 10.min(recent_entries.len());
        for i in 0..sample_count {
            let proof = self.merkle_tree.generate_proof(i as u64);
            if !AuditMerkleTree::verify_proof(&proof) {
                proof_failures += 1;
            }
        }

        let is_valid = matches!(chain_result, ChainVerificationResult::Valid { .. })
            && proof_failures == 0;

        self.last_root = current_root;

        IntegrityReport {
            timestamp: Timestamp::now(),
            merkle_root: hex::encode(current_root),
            entries_verified: recent_entries.len(),
            chain_valid: matches!(chain_result, ChainVerificationResult::Valid { .. }),
            proofs_verified: sample_count,
            proof_failures,
            overall_valid: is_valid,
        }
    }
}
```

---

## 11. 事件响应支持

### 11.1 告警触发器

```yaml
# 配置文件: /etc/omniagent/audit/alerts.yml
alerts:
  - id: "alert-001"
    name: "暴力破解检测"
    condition:
      type: auth_failure_threshold
      count: 20
      window: 300s  # 5 分钟内
    severity: critical
    actions:
      - type: block_actor
      - type: notify_admin
      - type: increase_logging
    cooldown: 600s

  - id: "alert-002"
    name: "Agent 权限提升尝试"
    condition:
      type: event_type_matches
      pattern: "auth.denied"
    filter:
      actor_type: agent
      action: "privilege_escalation"
    severity: high
    actions:
      - type: notify_admin
      - type: block_actor
    cooldown: 300s

  - id: "alert-003"
    name: "异常数据访问"
    condition:
      type: suspicious_pattern
      pattern: "大量文件读取后网络传输"
    severity: high
    actions:
      - type: notify_admin
      - type: webhook
        url: "https://security.example.com/alerts"
    cooldown: 600s

  - id: "alert-004"
    name: "内核完整性异常"
    condition:
      type: event_type_matches
      pattern: "security.integrity_violation"
    severity: critical
    actions:
      - type: notify_admin
      - type: block_actor
      - type: increase_logging
    cooldown: 0s  # 不设冷却
```

### 11.2 取证分析支持

```rust
/// 取证分析器
pub struct ForensicAnalyzer {
    query_api: AuditQueryApi,
}

impl ForensicAnalyzer {
    /// 重建事件时间线
    pub fn build_timeline(
        &self,
        actor_id: &str,
        start: Timestamp,
        end: Timestamp,
    ) -> EventTimeline {
        let events = self.query_api.query(AuditFilter {
            actor_id: Some(actor_id.to_string()),
            time_start: Some(start),
            time_end: Some(end),
            ..Default::default()
        });

        let mut timeline = EventTimeline {
            actor: actor_id.to_string(),
            events: Vec::new(),
            summary: TimelineSummary::default(),
        };

        for event in events {
            timeline.summary.total_events += 1;
            match event.result.as_str() {
                "success" => timeline.summary.successful += 1,
                "failure" => timeline.summary.failed += 1,
                "denied" => timeline.summary.denied += 1,
                _ => {}
            }

            timeline.events.push(TimelineEvent {
                timestamp: event.timestamp,
                event_type: event.event_type,
                action: event.action,
                target: event.target.id,
                result: event.result,
                metadata: event.metadata,
            });
        }

        timeline.events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        timeline
    }

    /// 生成取证报告
    pub fn generate_forensic_report(
        &self,
        incident_id: &str,
        scope: ForensicScope,
    ) -> ForensicReport {
        ForensicReport {
            incident_id: incident_id.to_string(),
            generated_at: Timestamp::now(),
            scope: scope.clone(),
            timeline: self.build_timeline(
                &scope.actor_id,
                scope.time_start,
                scope.time_end,
            ),
            chain_integrity: self.verify_chain_integrity(&scope),
            merkle_proofs: self.generate_merkle_proofs(&scope),
        }
    }
}
```

---

## 附录 A: 日志文件格式

```
# 文件命名规则
audit-YYYYMMDD-NNNN.logl    # 日志文件 (JSON Lines)
audit-YYYYMMDD-NNNN.logl.gz # 压缩归档
merkle-root-YYYYMMDD.bin    # Merkle 根快照

# 文件目录结构
/var/log/omniagent/
├── audit/
│   ├── current/              # 当前活跃日志
│   │   ├── audit-20260425-0001.logl
│   │   └── audit-20260425-0002.logl
│   ├── archive/              # 归档日志
│   │   ├── audit-20260424-0001.logl.gz
│   │   └── audit-20260423-0001.logl.gz
│   └── integrity/            # 完整性数据
│       ├── merkle-root-20260425.bin
│       └── chain-verify-20260425.json
└── alerts/
    ├── alert-20260425-001.json
    └── alert-summary-20260425.json
```

## 附录 B: 配置参考

```yaml
# /etc/omniagent/audit/config.yml
audit:
  enabled: true

  pipeline:
    batch_size: 100
    flush_interval: 1s
    buffer_capacity: 100000

  storage:
    directory: /var/log/omniagent/audit
    max_file_size: 100MB
    compression: gzip
    compression_after_days: 7

  redaction:
    rules: default
    custom_rules: []

  retention:
    security_events: 7y
    auth_events: 3y
    system_events: 1y
    debug_events: 30d

  integrity:
    merkle_update_interval: 1s
    chain_verify_interval: 1h
    merkle_snapshot_interval: 24h

  alerts:
    enabled: true
    config_file: /etc/omniagent/audit/alerts.yml

  performance:
    max_overhead_ms: 1
    async_writers: 2
    writer_queue_size: 10000
```
