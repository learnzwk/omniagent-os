# OmniAgent OS 分布式服务规范 (Distributed Service Specification)

> **模块编号**: `omniagent-distributed` | **版本**: v0.3.0-draft | **状态**: 设计阶段

---

## 1. 概述 (Purpose)

分布式服务是 OmniAgent OS 的跨设备协作基础设施，灵感来源于 HarmonyOS 的分布式能力。提供设备发现、软总线通信、任务迁移、资源池化和状态同步五大核心能力，使 Agent 能够无缝地在多个设备间迁移、协作和共享资源。

### 1.1 设计目标

| 目标 | 描述 |
|------|------|
| 透明通信 | 软总线抽象屏蔽底层网络细节，跨设备调用如同本地调用 |
| 快速发现 | mDNS + 自定义能力广播，2 秒内完成设备发现 |
| 无感迁移 | Agent 检查点与恢复，3 秒内完成跨设备迁移 |
| 资源聚合 | 跨设备 CPU/GPU/内存/模型资源统一池化管理 |
| 一致性保证 | CRDT + 向量时钟实现最终一致性与因果有序 |
| 安全通信 | 双向 TLS 认证，端到端加密通道 |

### 1.2 架构总览

```
┌──────────────────────────────────────────────────────────┐
│  discover() / connect() / migrate() / allocate() / sync()│
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│  ┌────────────┐ ┌────────────┐ ┌──────────────────────┐  │
│  │  Device    │ │  Soft Bus  │ │  Task Migration      │  │
│  │  Discovery │ │  (虚拟总线) │ │  (检查点/恢复)       │  │
│  └─────┬──────┘ └─────┬──────┘ └──────────┬───────────┘  │
│  ┌─────▼──────────────▼───────────────────▼───────────┐  │
│  │          Resource Pool (跨设备资源聚合)              │  │
│  └──────────────────────┬────────────────────────────┘  │
│  ┌──────────────────────▼────────────────────────────┐  │
│  │   State Sync (CRDT + Vector Clock)                 │  │
│  └──────────────────────┬────────────────────────────┘  │
│  ┌──────────────────────▼────────────────────────────┐  │
│  │   Network Transport (TCP/TLS + QUIC fallback)      │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 2. 接口定义 (Interfaces)

### 2.1 核心特征

```rust
/// 设备发现特征
#[async_trait]
pub trait DeviceDiscovery: Send + Sync {
    async fn discover(&self) -> Result<Vec<DiscoveredDevice>, DistributedError>;
    async fn watch(&self) -> Result<DeviceEventStream, DistributedError>;
    async fn broadcast_capability(&self, cap: DeviceCapability) -> Result<(), DistributedError>;
    async fn stop_broadcast(&self) -> Result<(), DistributedError>;
}

/// 软总线特征（虚拟设备总线）
#[async_trait]
pub trait SoftBus: Send + Sync {
    async fn connect(&self, device: &DeviceId) -> Result<Channel, DistributedError>;
    async fn disconnect(&self, channel_id: &ChannelId) -> Result<(), DistributedError>;
    async fn send(&self, channel_id: &ChannelId, message: BusMessage) -> Result<(), DistributedError>;
    async fn receive(&self, channel_id: &ChannelId) -> Result<BusMessage, DistributedError>;
    async fn active_channels(&self) -> Result<Vec<ChannelInfo>, DistributedError>;
}

/// 任务迁移特征
#[async_trait]
pub trait TaskMigration: Send + Sync {
    async fn migrate(&self, agent_id: &AgentId, target: &DeviceId, opts: MigrationOptions)
        -> Result<MigrationStatus, DistributedError>;
    async fn checkpoint(&self, agent_id: &AgentId) -> Result<AgentCheckpoint, DistributedError>;
    async fn restore(&self, checkpoint: AgentCheckpoint, target: &DeviceId) -> Result<AgentId, DistributedError>;
    async fn migration_status(&self, id: &MigrationId) -> Result<MigrationStatus, DistributedError>;
}

/// 资源池特征
#[async_trait]
pub trait ResourcePool: Send + Sync {
    async fn allocate(&self, request: ResourceRequest) -> Result<ResourceAllocation, DistributedError>;
    async fn release(&self, allocation_id: &AllocationId) -> Result<(), DistributedError>;
    async fn query_resources(&self) -> Result<GlobalResourceView, DistributedError>;
    async fn register_local_resources(&self, resources: LocalResources) -> Result<(), DistributedError>;
}

/// 状态同步特征
#[async_trait]
pub trait StateSync: Send + Sync {
    async fn sync(&self, state: SyncState, target: &DeviceId) -> Result<SyncAck, DistributedError>;
    async fn subscribe(&self, key: &str, source: &DeviceId) -> Result<StateSubscription, DistributedError>;
    async fn resolve_conflict(&self, conflict: StateConflict) -> Result<ResolvedState, DistributedError>;
}
```

### 2.2 分布式管理器主接口

```rust
pub struct DistributedManager {
    discovery: Arc<dyn DeviceDiscovery>, soft_bus: Arc<dyn SoftBus>,
    migration: Arc<dyn TaskMigration>, resource_pool: Arc<dyn ResourcePool>,
    state_sync: Arc<dyn StateSync>, transport: Arc<dyn NetworkTransport>,
    local_device: LocalDeviceInfo, device_registry: Arc<DeviceRegistry>,
}

impl DistributedManager {
    pub async fn new(config: DistributedConfig) -> Result<Self, DistributedError> {
        let local = LocalDeviceInfo::detect()?;
        let transport = Arc::new(create_transport(&config.transport)?);
        let registry = Arc::new(DeviceRegistry::new());
        Ok(Self {
            discovery: Arc::new(MdnsDiscovery::new(local.clone(), registry.clone(), config.discovery)?),
            soft_bus: Arc::new(SoftBusImpl::new(transport.clone(), registry.clone(), config.soft_bus)),
            migration: Arc::new(TaskMigrator::new(Arc::new(SoftBusImpl::new(transport.clone(), registry.clone(), config.soft_bus.clone())), transport.clone(), config.migration)),
            resource_pool: Arc::new(GlobalResourcePool::new(local.clone(), registry.clone(), transport.clone())),
            state_sync: Arc::new(CrdtStateSync::new(local.device_id.clone(), transport.clone(), config.sync)),
            transport, local_device: local, device_registry: registry,
        })
    }

    pub async fn discover(&self) -> Result<Vec<DiscoveredDevice>, DistributedError> {
        Ok(self.discovery.discover().await?.into_iter()
            .filter(|d| d.id != self.local_device.device_id).collect())
    }

    pub async fn connect(&self, device_id: &DeviceId) -> Result<Channel, DistributedError> {
        self.soft_bus.connect(device_id).await
    }

    pub async fn migrate(&self, agent_id: &AgentId, target: &DeviceId) -> Result<MigrationStatus, DistributedError> {
        let target_device = self.device_registry.get(target).await
            .ok_or(DistributedError::DeviceNotFound(target.clone()))?;
        let required = self.get_agent_requirements(agent_id).await?;
        if !target_device.capabilities.satisfies(&required) {
            return Err(DistributedError::InsufficientCapability { device: target.clone(), required, available: target_device.capabilities });
        }
        self.migration.migrate(agent_id, target, MigrationOptions::default()).await
    }

    pub async fn allocate(&self, request: ResourceRequest) -> Result<ResourceAllocation, DistributedError> {
        self.resource_pool.allocate(request).await
    }

    pub async fn sync(&self, state: SyncState, target: &DeviceId) -> Result<SyncAck, DistributedError> {
        self.state_sync.sync(state, target).await
    }
}
```

---

## 3. 数据结构 (Data Structures)

### 3.1 设备相关类型

```rust
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceId { pub uuid: Uuid, pub fingerprint: [u8; 32] }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub id: DeviceId, pub name: String, pub device_type: DeviceType,
    pub capabilities: DeviceCapabilities, pub addresses: Vec<SocketAddr>,
    pub last_seen: Instant, pub trust_level: TrustLevel, pub latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType { Phone, Tablet, Desktop, Laptop, IoT, Server, Wearable, Vehicle }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub cpu_cores: u32, pub cpu_available: f32,
    pub memory_total: u64, pub memory_available: u64,
    pub gpu: Option<GpuInfo>, pub available_models: Vec<ModelInfo>,
    pub services: Vec<String>, pub battery: Option<BatteryInfo>,
    pub network_bandwidth: u64,
}

impl DeviceCapabilities {
    pub fn satisfies(&self, req: &ResourceRequirement) -> bool {
        self.cpu_cores >= req.min_cpu_cores && self.memory_available >= req.min_memory
            && self.cpu_available >= req.min_cpu_available
            && req.gpu_required.map_or(true, |_| self.gpu.is_some())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo { pub name: String, pub vram_total: u64, pub vram_available: u64, pub compute_capability: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo { pub name: String, pub version: String, pub size_bytes: u64, pub precision: Precision }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precision { FP32, FP16, INT8, INT4 }

#[derive(Debug, Clone)]
pub enum DeviceEvent { Discovered(DiscoveredDevice), Lost(DeviceId), Updated(DiscoveredDevice) }
```

### 3.2 软总线相关类型

```rust
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelId(Uuid);

#[derive(Debug, Clone)]
pub struct Channel {
    pub id: ChannelId, pub local_device: DeviceId, pub remote_device: DeviceId,
    pub channel_type: ChannelType, pub state: ChannelState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelType { Control, Data, Stream }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelState { Connecting, Active, Suspended, Closed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusMessage {
    pub message_id: Uuid, pub source: DeviceId, pub target: DeviceId,
    pub payload: MessagePayload, pub timestamp: SystemTime,
    pub priority: MessagePriority, pub requires_ack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    RpcRequest { method: String, params: serde_json::Value, request_id: Uuid },
    RpcResponse { request_id: Uuid, result: serde_json::Value, is_error: bool },
    Event { event_type: String, data: serde_json::Value },
    Data { data_id: Uuid, chunk_index: u32, total_chunks: u32, data: Vec<u8> },
    Migration { migration_type: MigrationMessageType, data: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePriority { Low, Normal, High, Critical }
```

### 3.3 任务迁移相关类型

```rust
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationId(Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOptions {
    pub migrate_memory: bool, pub migrate_auth: bool,
    pub timeout: Duration, pub strategy: MigrationStrategy,
}

impl Default for MigrationOptions {
    fn default() -> Self { Self { migrate_memory: true, migrate_auth: true, timeout: Duration::from_secs(10), strategy: MigrationStrategy::CheckpointRestore } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStrategy { CheckpointRestore, LiveMigration, Phased }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCheckpoint {
    pub checkpoint_id: Uuid, pub agent_id: AgentId, pub created_at: SystemTime,
    pub state_snapshot: Vec<u8>, pub memory_snapshot: Option<Vec<u8>>,
    pub checksum: [u8; 32], pub compressed_size: u64, pub original_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub migration_id: MigrationId, pub agent_id: AgentId,
    pub source: DeviceId, pub target: DeviceId, pub phase: MigrationPhase,
    pub progress: f32, pub started_at: SystemTime, pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase { Preparing, Checkpointing, Transferring, Restoring, Verifying, Completed, Failed, Cancelled }
```

### 3.4 资源池相关类型

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    pub request_id: Uuid, pub agent_id: AgentId,
    pub requirement: ResourceRequirement, pub priority: ResourcePriority,
    pub duration: Duration, pub preferred_device: Option<DeviceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirement {
    pub min_cpu_cores: u32, pub min_memory: u64, pub min_cpu_available: f32,
    pub gpu_required: Option<bool>, pub min_gpu_vram: Option<u64>,
    pub required_models: Vec<String>, pub max_latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub allocation_id: AllocationId, pub device_id: DeviceId, pub agent_id: AgentId,
    pub allocated_resources: AllocatedResources, pub valid_until: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocatedResources { pub cpu_cores: u32, pub memory_bytes: u64, pub gpu: Option<AllocatedGpu> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocatedGpu { pub device_name: String, pub vram_bytes: u64 }
```

### 3.5 状态同步相关类型 (CRDT + 向量时钟)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState { pub key: String, pub value: CrdtValue, pub vector_clock: VectorClock, pub source: DeviceId }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrdtValue {
    GCounter(GCounter), PNCounter(PNCounter), GSet(GSet), ORSet(ORSet),
    LWWRegister(LWWRegister),
}

/// G-Counter（增长计数器）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCounter { pub counts: HashMap<DeviceId, u64> }

impl GCounter {
    pub fn increment(&mut self, device_id: &DeviceId) { *self.counts.entry(device_id.clone()).or_insert(0) += 1; }
    pub fn value(&self) -> u64 { self.counts.values().sum() }
    pub fn merge(&mut self, other: &GCounter) {
        for (id, count) in &other.counts { let e = self.counts.entry(id.clone()).or_insert(0); *e = (*e).max(*count); }
    }
}

/// LWW-Register（最后写入者胜出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister { pub value: serde_json::Value, pub timestamp: SystemTime, pub device_id: DeviceId }

impl LWWRegister {
    pub fn merge(&mut self, other: &LWWRegister) {
        if other.timestamp > self.timestamp { self.value = other.value.clone(); self.timestamp = other.timestamp; self.device_id = other.device_id.clone(); }
    }
}

/// 向量时钟
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VectorClock { pub entries: HashMap<DeviceId, u64> }

impl VectorClock {
    pub fn increment(&mut self, device_id: &DeviceId) { *self.entries.entry(device_id.clone()).or_insert(0) += 1; }
    pub fn merge(&mut self, other: &VectorClock) {
        for (id, ts) in &other.entries { let e = self.entries.entry(id.clone()).or_insert(0); *e = (*e).max(*ts); }
    }
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut at_least_one_less = false;
        for (id, &ts) in &self.entries {
            let other_ts = other.entries.get(id).copied().unwrap_or(0);
            if ts > other_ts { return false; }
            if ts < other_ts { at_least_one_less = true; }
        }
        at_least_one_less
    }
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self)
    }
}
```

---

## 4. 设备发现 (Device Discovery)

```rust
/// 基于 mDNS 的设备发现
pub struct MdnsDiscovery {
    local_device: LocalDeviceInfo, registry: Arc<DeviceRegistry>,
    config: DiscoveryConfig, event_sender: broadcast::Sender<DeviceEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub service_type: String, pub broadcast_interval: Duration,
    pub device_timeout: Duration, pub discovery_timeout: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self { Self { service_type: "_omniagent._tcp".into(), broadcast_interval: Duration::from_secs(5), device_timeout: Duration::from_secs(30), discovery_timeout: Duration::from_secs(2) } }
}

#[async_trait]
impl DeviceDiscovery for MdnsDiscovery {
    async fn discover(&self) -> Result<Vec<DiscoveredDevice>, DistributedError> {
        let registry = self.registry.get_all().await;
        let now = Instant::now();
        Ok(registry.into_iter().filter(|d| now.duration_since(d.last_seen) < self.config.device_timeout).collect())
    }

    async fn watch(&self) -> Result<DeviceEventStream, DistributedError> {
        Ok(DeviceEventStream { receiver: self.event_sender.subscribe() })
    }

    async fn broadcast_capability(&self, _cap: DeviceCapability) -> Result<(), DistributedError> { Ok(()) }
    async fn stop_broadcast(&self) -> Result<(), DistributedError> { Ok(()) }
}
```

---

## 5. 软总线 (Soft Bus)

```rust
pub struct SoftBusImpl {
    transport: Arc<dyn NetworkTransport>, registry: Arc<DeviceRegistry>,
    channels: DashMap<ChannelId, ChannelState>, config: SoftBusConfig,
}

#[async_trait]
impl SoftBus for SoftBusImpl {
    async fn connect(&self, device: &DeviceId) -> Result<Channel, DistributedError> {
        let info = self.registry.get(device).await.ok_or(DistributedError::DeviceNotFound(device.clone()))?;
        let addr = info.addresses.first().ok_or(DistributedError::DeviceUnreachable(device.clone()))?;
        let connection = self.transport.connect(addr).await?;
        // TLS 握手
        let handshake = BusHandshake { local_device: self.registry.local_device_id().await, protocol_version: "0.3.0".into() };
        self.transport.send(connection, &handshake).await?;
        let response: BusHandshakeAck = self.transport.receive(connection).await?;
        if !response.accepted { return Err(DistributedError::HandshakeRejected(response.reason)); }
        let channel = Channel { id: ChannelId::new(), local_device: self.registry.local_device_id().await, remote_device: device.clone(), channel_type: ChannelType::Control, state: ChannelState::Active };
        self.channels.insert(channel.id.clone(), ChannelState::Active);
        self.start_message_loop(channel.id.clone(), connection).await?;
        Ok(channel)
    }

    async fn send(&self, channel_id: &ChannelId, message: BusMessage) -> Result<(), DistributedError> {
        let state = self.channels.get(channel_id).ok_or(DistributedError::ChannelNotFound(channel_id.clone()))?;
        if *state != ChannelState::Active { return Err(DistributedError::ChannelNotActive(channel_id.clone())); }
        let data = postcard::to_allocvec(&message).map_err(|e| DistributedError::Serialization(e.to_string()))?;
        self.transport.send_raw(channel_id, &data).await
    }

    async fn receive(&self, channel_id: &ChannelId) -> Result<BusMessage, DistributedError> {
        let data = self.transport.receive_raw(channel_id).await?;
        postcard::from_bytes(&data).map_err(|e| DistributedError::Deserialization(e.to_string()))
    }

    async fn disconnect(&self, channel_id: &ChannelId) -> Result<(), DistributedError> { self.channels.remove(channel_id); Ok(()) }
    async fn active_channels(&self) -> Result<Vec<ChannelInfo>, DistributedError> {
        Ok(self.channels.iter().map(|e| ChannelInfo { id: e.key().clone(), state: *e.value(), ..Default::default() }).collect())
    }
}
```

---

## 6. 任务迁移 (Task Migration)

```rust
pub struct TaskMigrator {
    soft_bus: Arc<dyn SoftBus>, transport: Arc<dyn NetworkTransport>,
    config: MigrationConfig, active_migrations: DashMap<MigrationId, MigrationState>,
}

#[async_trait]
impl TaskMigration for TaskMigrator {
    async fn migrate(&self, agent_id: &AgentId, target: &DeviceId, opts: MigrationOptions)
        -> Result<MigrationStatus, DistributedError> {
        let mid = MigrationId::new();
        // 阶段 1: 准备
        self.update_phase(&mid, MigrationPhase::Preparing, 0.0);
        // 阶段 2: 创建检查点
        self.update_phase(&mid, MigrationPhase::Checkpointing, 0.1);
        let cp = self.checkpoint(agent_id).await?;
        // 阶段 3: 传输
        self.update_phase(&mid, MigrationPhase::Transferring, 0.3);
        let channel = self.soft_bus.connect(target).await?;
        let chunks = cp.serialize()?.chunks(self.config.chunk_size).collect::<Vec<_>>();
        for (i, chunk) in chunks.iter().enumerate() {
            self.soft_bus.send(&channel.id, BusMessage::migration_data(channel.local_device.clone(), target.clone(), i as u32, chunks.len() as u32, chunk.to_vec())).await?;
            self.update_phase(&mid, MigrationPhase::Transferring, 0.3 + (i as f32 / chunks.len() as f32) * 0.4);
        }
        // 阶段 4: 恢复
        self.update_phase(&mid, MigrationPhase::Restoring, 0.7);
        self.soft_bus.send(&channel.id, BusMessage::migration_command(channel.local_device.clone(), target.clone(), MigrationMessageType::RestoreStart)).await?;
        let ack = tokio::time::timeout(opts.timeout, self.wait_for_restore_ack(&channel.id)).await
            .map_err(|_| DistributedError::MigrationTimeout)??;
        // 阶段 5: 验证
        self.update_phase(&mid, MigrationPhase::Verifying, 0.9);
        if self.verify_migration(agent_id, target, &ack).await? {
            self.update_phase(&mid, MigrationPhase::Completed, 1.0);
        } else { return Err(DistributedError::MigrationVerificationFailed); }
        self.get_status(&mid).await
    }

    async fn checkpoint(&self, agent_id: &AgentId) -> Result<AgentCheckpoint, DistributedError> {
        let kernel = KernelInterface::get();
        let snapshot = kernel.snapshot_agent(agent_id).await?;
        let original_size = snapshot.len() as u64;
        let compressed = self.compress(&snapshot)?;
        let checksum = blake3::hash(&compressed);
        Ok(AgentCheckpoint { checkpoint_id: Uuid::new_v4().into(), agent_id: agent_id.clone(), created_at: SystemTime::now(), state_snapshot: compressed, checksum: checksum.into(), compressed_size: compressed.len() as u64, original_size, ..Default::default() })
    }

    async fn restore(&self, cp: AgentCheckpoint, target: &DeviceId) -> Result<AgentId, DistributedError> {
        let computed = blake3::hash(&cp.state_snapshot);
        if computed.as_bytes() != &cp.checksum { return Err(DistributedError::CheckpointCorrupted); }
        let state = self.decompress(&cp.state_snapshot)?;
        let channel = self.soft_bus.connect(target).await?;
        self.soft_bus.send(&channel.id, BusMessage::restore_request(channel.local_device.clone(), target.clone(), &cp, &state)).await?;
        let resp = self.wait_for_restore_response(&channel.id).await?;
        Ok(resp.restored_agent_id)
    }

    async fn migration_status(&self, id: &MigrationId) -> Result<MigrationStatus, DistributedError> { self.get_status(id).await }
}
```

---

## 7. 资源池 (Resource Pool)

```rust
pub struct GlobalResourcePool {
    local_device: LocalDeviceInfo, registry: Arc<DeviceRegistry>,
    transport: Arc<dyn NetworkTransport>, allocations: DashMap<AllocationId, ResourceAllocation>,
}

impl GlobalResourcePool {
    /// 智能评分选择最优设备
    fn find_best_device(&self, req: &ResourceRequirement, preferred: Option<&DeviceId>) -> Option<DeviceId> {
        let devices = self.registry.get_all_cached();
        if let Some(pid) = preferred {
            if let Some(d) = devices.iter().find(|d| &d.id == pid) { if d.capabilities.satisfies(req) { return Some(pid.clone()); } }
        }
        let mut candidates: Vec<_> = devices.iter().filter(|d| d.capabilities.satisfies(req)).collect();
        candidates.sort_by(|a, b| self.score_device(&b.capabilities, req).partial_cmp(&self.score_device(&a.capabilities, req)).unwrap_or(Ordering::Equal));
        candidates.first().map(|d| d.id.clone())
    }

    fn score_device(&self, caps: &DeviceCapabilities, req: &ResourceRequirement) -> f32 {
        let mut s = (caps.cpu_available - req.min_cpu_available) as f32 * 10.0;
        s += ((caps.memory_available - req.min_memory) as f64 / req.min_memory as f64) as f32 * 5.0;
        if req.gpu_required.unwrap_or(false) && caps.gpu.is_some() { s += 50.0; }
        if let Some(bat) = &caps.battery { if !bat.charging && bat.level < 0.2 { s -= 30.0; } }
        s
    }
}

#[async_trait]
impl ResourcePool for GlobalResourcePool {
    async fn allocate(&self, request: ResourceRequest) -> Result<ResourceAllocation, DistributedError> {
        let device_id = self.find_best_device(&request.requirement, request.preferred_device.as_ref())
            .ok_or(DistributedError::NoSuitableDevice)?;
        let msg = BusMessage::resource_allocate_request(&self.local_device.device_id, &device_id, &request);
        let resp = self.transport.request_response(msg, Duration::from_secs(5)).await?;
        let allocation: ResourceAllocation = serde_json::from_value(match resp.payload {
            MessagePayload::RpcResponse { result, is_error: false, .. } => result,
            _ => return Err(DistributedError::UnexpectedResponse),
        }).map_err(|e| DistributedError::Serialization(e.to_string()))?;
        self.allocations.insert(allocation.allocation_id.clone(), allocation.clone());
        Ok(allocation)
    }

    async fn release(&self, id: &AllocationId) -> Result<(), DistributedError> {
        if let Some(a) = self.allocations.remove(id) {
            self.transport.send_best_effort(BusMessage::resource_release(&self.local_device.device_id, &a.device_id, id)).await?;
        }
        Ok(())
    }

    async fn query_resources(&self) -> Result<GlobalResourceView, DistributedError> {
        let devices = self.registry.get_all().await;
        Ok(GlobalResourceView { devices: devices.iter().map(|d| DeviceResourceView { device_id: d.id.clone(), capabilities: d.capabilities.clone(), ..Default::default() }).collect(), ..Default::default() })
    }

    async fn register_local_resources(&self, _r: LocalResources) -> Result<(), DistributedError> { Ok(()) }
}
```

---

## 8. 状态同步 (CRDT + Vector Clock)

```rust
pub struct CrdtStateSync {
    local_device: DeviceId, transport: Arc<dyn NetworkTransport>,
    config: SyncConfig, crdt_store: DashMap<String, CrdtValue>,
    vector_clocks: DashMap<String, VectorClock>,
}

#[async_trait]
impl StateSync for CrdtStateSync {
    async fn sync(&self, state: SyncState, target: &DeviceId) -> Result<SyncAck, DistributedError> {
        self.merge_local(&state.key, &state.value, &state.vector_clock).await;
        self.transport.send_best_effort(BusMessage::sync_request(&self.local_device, target, &state)).await?;
        Ok(SyncAck { accepted: true, vector_clock: self.vector_clocks.get(&state.key).cloned().unwrap_or_default(), timestamp: SystemTime::now() })
    }

    async fn subscribe(&self, key: &str, source: &DeviceId) -> Result<StateSubscription, DistributedError> {
        let (tx, rx) = mpsc::channel(100);
        let id = Uuid::new_v4(); self.subscriptions.insert(id, tx);
        Ok(StateSubscription { subscription_id: id.into(), key: key.into(), source: source.clone(), receiver: rx })
    }

    async fn resolve_conflict(&self, conflict: StateConflict) -> Result<ResolvedState, DistributedError> {
        let merged = match (&conflict.local_value, &conflict.remote_value) {
            (CrdtValue::GCounter(l), CrdtValue::GCounter(r)) => { let mut m = l.clone(); m.merge(r); CrdtValue::GCounter(m) }
            (CrdtValue::LWWRegister(l), CrdtValue::LWWRegister(r)) => { let mut m = l.clone(); m.merge(r); CrdtValue::LWWRegister(m) }
            _ => return Err(DistributedError::ConflictResolutionFailed),
        };
        let mut clock = conflict.local_clock; clock.merge(&conflict.remote_clock);
        Ok(ResolvedState { key: conflict.key, value: merged, vector_clock: clock, resolution_strategy: ConflictResolution::CrdtAutoMerge })
    }
}

impl CrdtStateSync {
    async fn merge_local(&self, key: &str, value: &CrdtValue, clock: &VectorClock) {
        self.crdt_store.entry(key.to_string()).and_modify(|e| match (e, value) {
            (CrdtValue::GCounter(e), CrdtValue::GCounter(v)) => e.merge(v),
            (CrdtValue::LWWRegister(e), CrdtValue::LWWRegister(v)) => e.merge(v),
            _ => *e = value.clone(),
        }).or_insert_with(|| value.clone());
        self.vector_clocks.entry(key.to_string()).and_modify(|c| c.merge(clock)).or_insert_with(|| clock.clone());
    }
}
```

---

## 9. 网络传输 (TCP/TLS + QUIC)

```rust
#[async_trait]
pub trait NetworkTransport: Send + Sync {
    async fn connect(&self, addr: &SocketAddr) -> Result<ConnectionHandle, DistributedError>;
    async fn send_raw(&self, channel_id: &ChannelId, data: &[u8]) -> Result<(), DistributedError>;
    async fn receive_raw(&self, channel_id: &ChannelId) -> Result<Vec<u8>, DistributedError>;
    async fn request_response(&self, msg: BusMessage, timeout: Duration) -> Result<BusMessage, DistributedError>;
    async fn send_best_effort(&self, msg: BusMessage) -> Result<(), DistributedError>;
}

/// 双协议传输层（TCP/TLS 主 + QUIC 备选）
pub struct DualProtocolTransport { tls: TlsTransport, quic: QuicTransport }

impl DualProtocolTransport {
    pub async fn connect(&self, addr: &SocketAddr) -> Result<ConnectionHandle, DistributedError> {
        match self.tls.connect(addr).await { Ok(c) => Ok(c), Err(_) => {
            tracing::info!("TCP/TLS 失败，降级到 QUIC: {}", addr);
            self.quic.connect(addr).await
        }}
    }
}
```

---

## 10. 错误处理 (Error Handling)

```rust
#[derive(Debug, thiserror::Error)]
pub enum DistributedError {
    #[error("设备未找到: {0}")] DeviceNotFound(DeviceId),
    #[error("设备不可达: {0}")] DeviceUnreachable(DeviceId),
    #[error("通道未找到: {0}")] ChannelNotFound(ChannelId),
    #[error("连接已关闭")] ConnectionClosed,
    #[error("TLS 错误: {0}")] TlsError(String),
    #[error("QUIC 错误: {0}")] QuicError(String),
    #[error("设备能力不足")] InsufficientCapability { device: DeviceId, required: ResourceRequirement, available: DeviceCapabilities },
    #[error("无合适设备")] NoSuitableDevice,
    #[error("迁移超时")] MigrationTimeout,
    #[error("迁移验证失败")] MigrationVerificationFailed,
    #[error("检查点损坏")] CheckpointCorrupted,
    #[error("冲突解决失败")] ConflictResolutionFailed,
    #[error("序列化错误: {0}")] Serialization(String),
    #[error("认证失败: {0}")] AuthenticationFailed(String),
}
```

---

## 11. 安全设计 (Security)

### 11.1 双向 TLS 认证

```rust
pub struct DeviceAuthenticator { ca_cert: CertificateDer, trusted: RwLock<HashSet<DeviceId>> }

impl DeviceAuthenticator {
    pub fn verify_device(&self, cert: &CertificateDer) -> Result<DeviceId, DistributedError> {
        let parsed = x509_parser::parse_x509_certificate(cert).map_err(|e| DistributedError::AuthenticationFailed(e.to_string()))?;
        self.verify_ca_signature(&parsed.1)?;
        let now = SystemTime::now();
        if now < parsed.1.validity.not_before || now > parsed.1.validity.not_after {
            return Err(DistributedError::AuthenticationFailed("证书过期".into()));
        }
        self.extract_device_id(&parsed.1)
    }

    pub fn create_server_config(&self, cert: &DeviceCertificate) -> Result<ServerConfig, DistributedError> {
        ServerConfig::builder().with_safe_defaults()
            .with_single_cert(cert.cert_chain.clone(), cert.private_key.clone_key())
            .map(|b| { b.with_client_cert_verifier(Arc::new(DeviceCertVerifier { ca: self.ca_cert.clone() })); b })
            .map_err(|e| DistributedError::TlsError(e.to_string()))
    }
}
```

### 11.2 端到端加密 (ECDH 密钥交换)

```rust
pub struct MessageEncryption { session_keys: DashMap<DeviceId, SessionKey>, local_device: DeviceId }

impl MessageEncryption {
    pub async fn establish_session(&self, device_id: &DeviceId, public_key: &[u8]) -> Result<(), DistributedError> {
        let local_key = EcdhKey::generate();
        let shared = local_key.compute_shared_secret(public_key)?;
        let session_key = hkdf_sha256(&shared, b"omniagent-session", &device_id.uuid.to_bytes());
        self.session_keys.insert(device_id.clone(), SessionKey { key: session_key, expires_at: Instant::now() + Duration::from_secs(3600) });
        Ok(())
    }

    pub fn encrypt_message(&self, device_id: &DeviceId, plaintext: &[u8]) -> Result<Vec<u8>, DistributedError> {
        let session = self.session_keys.get(device_id).ok_or(DistributedError::AuthenticationFailed("无会话密钥".into()))?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&session.key));
        let ct = cipher.encrypt(&nonce, plaintext).map_err(|e| DistributedError::EncryptionError(e.to_string()))?;
        let mut out = nonce.to_vec(); out.extend_from_slice(&ct); Ok(out)
    }
}
```

---

## 12. 性能约束 (Performance Constraints)

| 操作 | 目标延迟 | 最大延迟 | 备注 |
|------|---------|---------|------|
| 设备发现 (discover) | < 1s | < 2s | mDNS 广播 |
| 设备连接 (connect) | < 500ms | < 1s | 含 TLS 握手 |
| 消息发送 (控制) | < 1ms | < 5ms | > 10,000 msg/s |
| Agent 迁移 (migrate) | < 2s | < 3s | 含检查点和恢复 |
| 资源分配 (allocate) | < 200ms | < 500ms | 含设备协商 |
| 状态同步 (sync) | < 50ms | < 100ms | CRDT 合并 |
| 向量时钟合并 | < 10μs | < 50μs | 内存操作 |

### 一致性模型

```
一致性级别: 最终一致性 + 因果有序

Device A: ──w(x=1)──w(y=2)──
                 │ 因果关系
                 ▼
Device B: ──────r(x=1)──w(z=3)──r(y=2)──

保证: 因果有序 + 最终收敛 + CRDT 无冲突合并 + 反熵修复
```

---

## 13. 测试用例 (Test Cases)

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_device_discovery() {
        let registry = Arc::new(DeviceRegistry::new());
        let discovery = MdnsDiscovery::new(LocalDeviceInfo::test_new("a"), registry.clone(), DiscoveryConfig::default()).unwrap();
        registry.register(DiscoveredDevice::test_new("b", DeviceType::Tablet)).await;
        registry.register(DiscoveredDevice::test_new("c", DeviceType::Desktop)).await;
        let devices = discovery.discover().await.unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn test_device_timeout() {
        let registry = Arc::new(DeviceRegistry::new());
        let discovery = MdnsDiscovery::new(LocalDeviceInfo::test_new("a"), registry.clone(),
            DiscoveryConfig { device_timeout: Duration::from_millis(100), ..Default::default() }).unwrap();
        let mut d = DiscoveredDevice::test_new("b", DeviceType::Phone); d.last_seen = Instant::now() - Duration::from_secs(60);
        registry.register(d).await;
        assert!(discovery.discover().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_soft_bus_connect_send() {
        let registry = Arc::new(DeviceRegistry::new());
        let bus = Arc::new(SoftBusImpl::new(Arc::new(MockTransport::new()), registry.clone(), SoftBusConfig::default()));
        registry.register(DiscoveredDevice::test_new("remote", DeviceType::Desktop)).await;
        let ch = bus.connect(&DeviceId::test_new("remote")).await.unwrap();
        assert_eq!(ch.state, ChannelState::Active);
        bus.send(&ch.id, BusMessage::heartbeat(DeviceId::test_new("local"), DeviceId::test_new("remote"))).await.unwrap();
    }

    #[tokio::test]
    async fn test_agent_migration() {
        let migrator = create_test_migrator().await;
        let status = migrator.migrate(&AgentId::test_new("agent"), &DeviceId::test_new("target"), MigrationOptions::default()).await.unwrap();
        assert_eq!(status.phase, MigrationPhase::Completed);
        assert!((status.progress - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_checkpoint_integrity() {
        let migrator = create_test_migrator().await;
        let cp = migrator.checkpoint(&AgentId::test_new("agent")).await.unwrap();
        let computed = blake3::hash(&cp.state_snapshot);
        assert_eq!(computed.as_bytes(), &cp.checksum);
        assert!(cp.original_size as f64 / cp.compressed_size as f64 > 1.0);
    }

    #[tokio::test]
    async fn test_migration_insufficient_capability() {
        let mgr = create_test_distributed_manager().await;
        let weak = DiscoveredDevice { capabilities: DeviceCapabilities { cpu_cores: 1, memory_total: 256*1024*1024, memory_available: 128*1024*1024, cpu_available: 0.5, gpu: None, ..Default::default() }, ..Default::default() };
        assert!(matches!(mgr.migrate(&AgentId::test_new("heavy"), &weak.id).await, Err(DistributedError::InsufficientCapability { .. })));
    }

    #[test]
    fn test_gcounter_merge() {
        let da = DeviceId::test_new("a"); let db = DeviceId::test_new("b");
        let mut ca = GCounter::new(); let mut cb = GCounter::new();
        ca.increment(&da); ca.increment(&da); ca.increment(&da);
        cb.increment(&db); cb.increment(&db);
        assert_eq!(ca.value(), 3); assert_eq!(cb.value(), 2);
        ca.merge(&cb); assert_eq!(ca.value(), 5);
    }

    #[test]
    fn test_lww_register_merge() {
        let da = DeviceId::test_new("a"); let db = DeviceId::test_new("b");
        let mut ra = LWWRegister::new(json!("A"), da);
        std::thread::sleep(Duration::from_millis(10));
        let mut rb = LWWRegister::new(json!("B"), db);
        ra.merge(&rb); assert_eq!(ra.value, json!("B"));  // 更新者胜出
    }

    #[test]
    fn test_vector_clock_causality() {
        let da = DeviceId::test_new("a"); let db = DeviceId::test_new("b");
        let mut ca = VectorClock::default(); let mut cb = VectorClock::default();
        ca.increment(&da); cb.merge(&ca); cb.increment(&db);
        let earlier = VectorClock { entries: [(da.clone(), 1)].into_iter().collect() };
        let later = VectorClock { entries: [(da.clone(), 1), (db.clone(), 1)].into_iter().collect() };
        assert!(earlier.happens_before(&later));
        assert!(!earlier.is_concurrent(&later));
    }

    #[tokio::test]
    async fn test_resource_allocation() {
        let pool = create_test_resource_pool().await;
        register_test_devices(&pool).await;
        let req = ResourceRequest { requirement: ResourceRequirement { min_cpu_cores: 4, min_memory: 1024*1024*1024, gpu_required: Some(true), ..Default::default() }, ..Default::default() };
        let alloc = pool.allocate(req).await.unwrap();
        assert!(alloc.allocated_resources.cpu_cores >= 4);
        assert!(alloc.allocated_resources.gpu.is_some());
    }
}
```

---

## 14. 配置参考

```toml
[distributed]
device_name = "omniagent-device"
device_type = "desktop"

[distributed.discovery]
service_type = "_omniagent._tcp"
broadcast_interval = "5s"
device_timeout = "30s"

[distributed.soft_bus]
max_channels = 100
heartbeat_interval = "10s"
max_message_size = 67108864

[distributed.migration]
max_concurrent_migrations = 3
compression = "zstd"
chunk_size = 1048576

[distributed.transport]
primary_protocol = "tcp_tls"
fallback_protocol = "quic"
connect_timeout = "5s"

[distributed.transport.tls]
ca_cert_path = "/etc/omniagent/certs/ca.pem"
cert_path = "/etc/omniagent/certs/device.pem"
key_path = "/etc/omniagent/certs/device.key"
verify_peer = true

[distributed.transport.quic]
max_idle_timeout = "30s"
enable_0rtt = true

[distributed.sync]
sync_interval = "1s"
max_sync_batch = 100
default_resolution = "crdt_auto_merge"
anti_entropy_interval = "60s"
```

---

> **文档版本**: v0.3.0-draft | **最后更新**: 2026-04-25 | **作者**: OmniAgent OS 分布式架构团队
