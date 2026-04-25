# OmniAgent OS 安全飞地规范

> **文档版本**: v1.0.0
> **最后更新**: 2026-04-25
> **文档状态**: 正式发布
> **责任团队**: 安全工程与内核架构组

---

## 1. 概述

### 1.1 设计目标

OmniAgent OS 安全飞地 (Security Enclave) 提供软件级别的可信执行环境 (Software TEE)，用于保护最敏感的数据和操作。即使操作系统内核或用户空间服务被攻陷，飞地内的数据仍保持机密性和完整性。

| 安全属性 | 描述 | 保证级别 |
|---------|------|---------|
| **机密性** | 飞地内数据对外部不可见 | 强保证 |
| **完整性** | 飞地内代码和数据不可被篡改 | 强保证 |
| **隔离性** | 飞地与外部环境完全隔离 | 强保证 |
| **可认证性** | 飞地状态可被本地和远程验证 | 强保证 |
| **防重放** | 飞地操作不可被重放 | 强保证 |

### 1.2 架构定位

```
┌─────────────────────────────────────────────────────────────┐
│                    硬件层                                    │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  CPU + MMU (页表隔离) + 可选 SGX/TDX 硬件 TEE        │  │
│  └───────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    内核层                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ 安全飞地管理器 │  │ 内存隔离引擎  │  │ 密封存储驱动     │  │
│  │ (Enclave Mgr) │  │ (Mem Isolate)│  │ (Sealed Storage) │  │
│  └──────┬───────┘  └──────┬───────┘  └───────┬──────────┘  │
│         │                 │                   │              │
├─────────┼─────────────────┼───────────────────┼──────────────┤
│         │     安全飞地运行时 (Enclave Runtime) │              │
│  ┌──────┴─────────────────┴───────────────────┴──────────┐  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐    │  │
│  │  │ 密钥管理  │  │ 认证模块  │  │ 加密/解密引擎     │    │  │
│  │  │ (Key Mgr)│  │ (Attest) │  │ (Crypto Engine)  │    │  │
│  │  └──────────┘  └──────────┘  └──────────────────┘    │  │
│  └──────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    用户空间服务                               │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Agent 服务│  │ 授权服务  │  │ 云 API   │  │ 桌面服务  │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 软件 TEE 设计

### 2.1 隔离机制

```rust
/// 安全飞地配置
pub struct EnclaveConfig {
    /// 飞地唯一标识
    pub id: EnclaveId,
    /// 飞地名称
    pub name: String,
    /// 飞地代码哈希 (用于完整性验证)
    pub code_hash: [u8; 32],
    /// 飞地内存大小 (字节)
    pub memory_size: usize,
    /// 飞地线程数
    pub thread_count: u32,
    /// 是否允许调试
    pub allow_debug: bool,
    /// 安全属性
    pub security_flags: EnclaveSecurityFlags,
}

#[derive(Debug, Clone)]
pub struct EnclaveSecurityFlags {
    /// 启用内存加密
    pub memory_encryption: bool,
    /// 启用密封存储
    pub sealed_storage: bool,
    /// 启用本地认证
    pub local_attestation: bool,
    /// 启用远程认证
    pub remote_attestation: bool,
    /// 禁止外部内存访问
    pub no_external_memory_access: bool,
    /// 启用栈保护
    pub stack_canary: bool,
}
```

### 2.2 内存隔离实现

```rust
/// 飞地内存布局
pub struct EnclaveMemoryLayout {
    /// 飞地基地址
    pub base: VirtualAddress,
    /// 飞地大小
    pub size: usize,
    /// 代码段
    pub code_region: MemoryRegion,
    /// 数据段
    pub data_region: MemoryRegion,
    /// 堆区域
    pub heap_region: MemoryRegion,
    /// 栈区域
    pub stack_region: MemoryRegion,
    /// 线程控制块区域
    pub tcs_region: MemoryRegion,
}

/// 内存区域
pub struct MemoryRegion {
    pub start: VirtualAddress,
    pub size: usize,
    pub permissions: PageFlags,
    pub encrypted: bool,
}

impl EnclaveMemoryLayout {
    /// 创建飞地内存布局
    pub fn new(base: VirtualAddress, size: usize) -> Self {
        let page_size = 4096;
        let total_pages = size / page_size;

        // 内存布局分配:
        // [代码段 30%] [数据段 20%] [堆 30%] [栈 10%] [TCB 10%]
        let code_pages = (total_pages as f64 * 0.30) as usize;
        let data_pages = (total_pages as f64 * 0.20) as usize;
        let heap_pages = (total_pages as f64 * 0.30) as usize;
        let stack_pages = (total_pages as f64 * 0.10) as usize;
        let tcs_pages = total_pages - code_pages - data_pages - heap_pages - stack_pages;

        let mut offset = 0;

        let code_region = MemoryRegion {
            start: base + offset,
            size: code_pages * page_size,
            permissions: PageFlags::READ | PageFlags::EXECUTE,
            encrypted: true,
        };
        offset += code_pages * page_size;

        let data_region = MemoryRegion {
            start: base + offset,
            size: data_pages * page_size,
            permissions: PageFlags::READ | PageFlags::WRITE,
            encrypted: true,
        };
        offset += data_pages * page_size;

        let heap_region = MemoryRegion {
            start: base + offset,
            size: heap_pages * page_size,
            permissions: PageFlags::READ | PageFlags::WRITE,
            encrypted: true,
        };
        offset += heap_pages * page_size;

        let stack_region = MemoryRegion {
            start: base + offset,
            size: stack_pages * page_size,
            permissions: PageFlags::READ | PageFlags::WRITE,
            encrypted: true,
        };
        offset += stack_pages * page_size;

        let tcs_region = MemoryRegion {
            start: base + offset,
            size: tcs_pages * page_size,
            permissions: PageFlags::READ | PageFlags::WRITE,
            encrypted: true,
        };

        Self {
            base,
            size,
            code_region,
            data_region,
            heap_region,
            stack_region,
            tcs_region,
        }
    }
}
```

### 2.3 受控入口/出口

```rust
/// 飞地入口点定义
pub struct EnclaveEntryPoint {
    /// 函数编号
    pub function_id: u32,
    /// 函数名称
    pub name: String,
    /// 参数类型
    pub param_types: Vec<ParamType>,
    /// 返回类型
    pub return_type: ParamType,
    /// 是否需要认证
    pub requires_attestation: bool,
}

/// 飞地调用
pub struct EnclaveCall {
    /// 目标飞地 ID
    pub enclave_id: EnclaveId,
    /// 函数编号
    pub function_id: u32,
    /// 输入缓冲区 (在飞地外，需要拷贝)
    pub input_buffer: Vec<u8>,
    /// 输出缓冲区
    pub output_buffer: Vec<u8>,
}

/// 飞地管理器 - 处理飞地调用
impl EnclaveManager {
    /// 进入飞地执行函数
    pub fn enter_enclave(&self, call: EnclaveCall) -> Result<EnclaveResult, EnclaveError> {
        // 1. 验证飞地存在且状态正常
        let enclave = self.get_enclave(&call.enclave_id)?;

        // 2. 验证调用者权限
        self.verify_caller_permission(&call)?;

        // 3. 验证输入缓冲区大小
        self.validate_input_size(&call)?;

        // 4. 保存当前上下文
        let saved_context = self.save_context();

        // 5. 切换到飞地页表
        self.switch_page_table(enclave.page_table);

        // 6. 拷贝输入数据到飞地内存
        let input_ptr = enclave.allocate_input_buffer(call.input_buffer.len());
        unsafe {
            core::ptr::copy_nonoverlapping(
                call.input_buffer.as_ptr(),
                input_ptr as *mut u8,
                call.input_buffer.len(),
            );
        }

        // 7. 执行飞地函数
        let result = unsafe {
            enclave.execute(call.function_id, input_ptr, call.input_buffer.len())
        };

        // 8. 拷贝输出数据
        let output = if result.output_len > 0 {
            let output_slice = unsafe {
                core::slice::from_raw_parts(result.output_ptr, result.output_len)
            };
            output_slice.to_vec()
        } else {
            Vec::new()
        };

        // 9. 恢复上下文
        self.restore_context(saved_context);

        // 10. 清理飞地临时缓冲区
        unsafe { enclave.free_input_buffer(input_ptr); }

        Ok(EnclaveResult {
            status: result.status,
            output,
        })
    }
}
```

---

## 3. 飞地生命周期

### 3.1 生命周期状态机

```
                    ┌──────────┐
                    │  Created  │
                    └─────┬────┘
                          │ initialize()
                          ▼
                    ┌──────────┐
              ┌────│ Running   │────┐
              │    └─────┬────┘    │
              │          │ seal()  │ pause()
              │          ▼         ▼
              │    ┌──────────┐ ┌──────────┐
              │    │  Sealed   │ │ Paused   │
              │    └─────┬────┘ └────┬─────┘
              │          │ unseal()   │ resume()
              │          ▼            │
              │    ┌──────────┐       │
              └───→│ Running  │←──────┘
                   └─────┬────┘
                         │ destroy()
                         ▼
                   ┌──────────┐
                   │ Destroyed │
                   └──────────┘
```

### 3.2 生命周期实现

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum EnclaveState {
    /// 已创建，未初始化
    Created,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 已密封 (持久化)
    Sealed { seal_time: Timestamp },
    /// 已销毁
    Destroyed,
}

pub struct Enclave {
    pub id: EnclaveId,
    pub config: EnclaveConfig,
    pub state: EnclaveState,
    pub page_table: PageTable,
    pub memory_layout: EnclaveMemoryLayout,
    pub metrics: EnclaveMetrics,
}

impl Enclave {
    /// 初始化飞地
    pub fn initialize(&mut self) -> Result<(), EnclaveError> {
        ensure!(self.state == EnclaveState::Created, "飞地已初始化");

        // 1. 分配飞地内存
        let memory = allocate_enclave_memory(self.config.memory_size)?;

        // 2. 设置飞地页表
        self.page_table = create_enclave_page_table(&memory)?;

        // 3. 加载飞地代码
        self.load_enclave_code()?;

        // 4. 验证代码完整性
        self.verify_code_integrity()?;

        // 5. 初始化飞地运行时
        self.init_runtime()?;

        // 6. 生成飞地身份密钥对
        self.generate_identity_key()?;

        self.state = EnclaveState::Running;
        Ok(())
    }

    /// 密封飞地 (持久化状态)
    pub fn seal(&mut self) -> Result<SealedData, EnclaveError> {
        ensure!(self.state == EnclaveState::Running, "飞地不在运行状态");

        // 1. 暂停飞地执行
        self.pause_execution()?;

        // 2. 收集飞地状态
        let state_data = self.serialize_state()?;

        // 3. 加密状态数据
        let encrypted = self.encrypt_state(&state_data)?;

        // 4. 计算完整性哈希
        let hash = blake3::hash(&encrypted);

        // 5. 创建密封数据
        let sealed = SealedData {
            enclave_id: self.id.clone(),
            encrypted_data: encrypted,
            integrity_hash: hash.into(),
            seal_time: Timestamp::now(),
            version: 1,
        };

        self.state = EnclaveState::Sealed {
            seal_time: sealed.seal_time,
        };

        Ok(sealed)
    }

    /// 解密封飞地
    pub fn unseal(&mut self, sealed: &SealedData) -> Result<(), EnclaveError> {
        ensure!(self.state == EnclaveState::Sealed { .. }, "飞地未密封");

        // 1. 验证完整性
        let hash = blake3::hash(&sealed.encrypted_data);
        ensure!(hash.as_bytes() == sealed.integrity_hash.as_slice(),
            "密封数据完整性验证失败");

        // 2. 解密状态数据
        let state_data = self.decrypt_state(&sealed.encrypted_data)?;

        // 3. 恢复飞地状态
        self.deserialize_state(&state_data)?;

        // 4. 验证代码完整性 (防止代码被替换)
        self.verify_code_integrity()?;

        self.state = EnclaveState::Running;
        Ok(())
    }

    /// 销毁飞地
    pub fn destroy(&mut self) -> Result<(), EnclaveError> {
        // 1. 安全擦除飞地内存
        self.secure_wipe_memory()?;

        // 2. 释放页表
        self.page_table.release()?;

        // 3. 释放物理内存
        self.release_memory()?;

        // 4. 撤销飞地身份密钥
        self.revoke_identity_key()?;

        self.state = EnclaveState::Destroyed;

        // 审计记录
        audit_log::record(AuditEvent::EnclaveDestroyed {
            enclave_id: self.id.clone(),
            timestamp: Timestamp::now(),
        });

        Ok(())
    }
}
```

---

## 4. 密封存储

### 4.1 密封数据格式

```rust
/// 密封数据 - 加密的持久化存储
pub struct SealedData {
    /// 飞地 ID
    pub enclave_id: EnclaveId,
    /// 加密后的数据
    pub encrypted_data: Vec<u8>,
    /// 完整性哈希 (BLAKE3)
    pub integrity_hash: [u8; 32],
    /// 加密算法标识
    pub algorithm: CipherAlgorithm,
    /// 密封时间
    pub seal_time: Timestamp,
    /// 数据版本 (用于密钥轮换)
    pub version: u32,
    /// 关联数据 (AEAD additional data)
    pub associated_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum CipherAlgorithm {
    /// AES-256-GCM
    Aes256Gcm,
    /// ChaCha20-Poly1305
    ChaCha20Poly1305,
}
```

### 4.2 密封存储实现

```rust
/// 密封存储管理器
pub struct SealedStorage {
    /// 存储后端
    backend: Box<dyn SealedStorageBackend>,
    /// 加密引擎
    crypto: CryptoEngine,
}

impl SealedStorage {
    /// 密封数据并存储
    pub fn seal_and_store(
        &self,
        enclave_id: &EnclaveId,
        data: &[u8],
        associated_data: &[u8],
    ) -> Result<SealedDataHandle, SealedError> {
        // 1. 派生加密密钥
        let key = self.derive_seal_key(enclave_id)?;

        // 2. 加密数据 (AEAD)
        let nonce = self.crypto.generate_nonce();
        let encrypted = self.crypto.aead_encrypt(
            &key,
            data,
            associated_data,
            &nonce,
        )?;

        // 3. 计算完整性哈希
        let integrity_hash = blake3::hash(&encrypted);

        // 4. 创建密封数据
        let sealed = SealedData {
            enclave_id: enclave_id.clone(),
            encrypted_data: encrypted,
            integrity_hash: integrity_hash.into(),
            algorithm: CipherAlgorithm::Aes256Gcm,
            seal_time: Timestamp::now(),
            version: 1,
            associated_data: associated_data.to_vec(),
        };

        // 5. 存储到后端
        let handle = self.backend.store(&sealed)?;

        Ok(handle)
    }

    /// 从存储中解密封数据
    pub fn load_and_unseal(
        &self,
        handle: &SealedDataHandle,
        associated_data: &[u8],
    ) -> Result<Vec<u8>, SealedError> {
        // 1. 从后端加载密封数据
        let sealed = self.backend.load(handle)?;

        // 2. 验证完整性
        let hash = blake3::hash(&sealed.encrypted_data);
        if hash.as_bytes() != sealed.integrity_hash.as_slice() {
            return Err(SealedError::IntegrityViolation);
        }

        // 3. 派生解密密钥
        let key = self.derive_seal_key(&sealed.enclave_id)?;

        // 4. 解密数据
        let nonce = self.crypto.extract_nonce(&sealed.encrypted_data)?;
        let decrypted = self.crypto.aead_decrypt(
            &key,
            &sealed.encrypted_data,
            associated_data,
            &nonce,
        )?;

        Ok(decrypted)
    }
}
```

---

## 5. 密钥管理

### 5.1 密钥层次结构

```
┌─────────────────────────────────────────┐
│         根密钥 (Root Key)                │  ← 硬件派生 / 安全飞地生成
│         永不导出，仅存在于飞地内存中       │
└──────────────┬──────────────────────────┘
               │ HKDF 派生
    ┌──────────┼──────────┬──────────────┐
    ▼          ▼          ▼              ▼
┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐
│ 密封密钥│ │ 认证密钥│ │ API密钥│ │ 会话密钥  │
│ (Seal) │ │ (Attest)│ │ (API) │ │ (Session)│
└────────┘ └────────┘ └────────┘ └──────────┘
```

### 5.2 密钥派生

```rust
/// 密钥管理器 - 仅在飞地内运行
pub struct KeyManager {
    /// 根密钥 (永不导出)
    root_key: [u8; 32],
    /// 密钥版本 (用于密钥轮换)
    key_version: u32,
    /// 密钥缓存
    derived_keys: HashMap<KeyPurpose, DerivedKey>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum KeyPurpose {
    SealStorage,
    LocalAttestation,
    RemoteAttestation,
    ApiEncryption,
    SessionEncryption,
    IdentitySigning,
}

impl KeyManager {
    /// 从硬件安全模块派生根密钥
    pub fn derive_root_key() -> Result<Self, KeyError> {
        // 使用 CPU 提供的随机数 + 飞地唯一标识派生
        let unique_id = EnclaveIdentity::current().unique_id();
        let hardware_random = read_hardware_random_32()?;

        // HKDF 派生根密钥
        let root_key = hkdf_sha256(
            hardware_random.as_slice(),
            unique_id.as_slice(),
            b"omniagent-enclave-root-key",
        );

        Ok(Self {
            root_key,
            key_version: 1,
            derived_keys: HashMap::new(),
        })
    }

    /// 派生子密钥
    pub fn derive_key(&mut self, purpose: KeyPurpose) -> Result<&DerivedKey, KeyError> {
        if let Some(key) = self.derived_keys.get(&purpose) {
            return Ok(key);
        }

        let context = format!("omniagent:{}:v{}", purpose.as_str(), self.key_version);
        let derived = hkdf_sha256(
            &self.root_key,
            b"",
            context.as_bytes(),
        );

        let key = DerivedKey {
            purpose: purpose.clone(),
            material: derived,
            created_at: Timestamp::now(),
            version: self.key_version,
        };

        self.derived_keys.insert(purpose, key);
        Ok(self.derived_keys.get(&purpose).unwrap())
    }

    /// 密钥轮换
    pub fn rotate_keys(&mut self) -> Result<(), KeyError> {
        // 1. 生成新的根密钥
        let hardware_random = read_hardware_random_32()?;
        let new_root = hkdf_sha256(
            hardware_random.as_slice(),
            &self.root_key, // 旧根密钥作为盐
            b"omniagent-enclave-root-key-rotation",
        );

        // 2. 清除所有派生密钥
        self.secure_wipe_derived_keys();

        // 3. 更新根密钥
        self.root_key = new_root;
        self.key_version += 1;

        // 4. 重新密封所有存储数据
        self.reseal_all_data()?;

        // 审计记录
        audit_log::record(AuditEvent::KeyRotation {
            new_version: self.key_version,
            timestamp: Timestamp::now(),
        });

        Ok(())
    }

    /// 密钥包装 (用于安全传输)
    pub fn wrap_key(&self, key: &[u8], recipient_public_key: &[u8]) -> Result<Vec<u8>, KeyError> {
        // 使用 ECIES 或类似方案包装密钥
        let ephemeral_key = self.generate_ephemeral_key()?;
        let shared_secret = ecdh_shared_secret(&ephemeral_key, recipient_public_key)?;

        let wrapped = aead_encrypt(
            &hkdf_sha256(&shared_secret, b"", b"key-wrapping"),
            key,
            b"key-wrap",
        )?;

        Ok([ephemeral_key.to_bytes().as_slice(), &wrapped].concat())
    }
}
```

---

## 6. 认证机制

### 6.1 本地认证 (Enclave-to-Enclave)

```rust
/// 本地认证 - 验证同一平台上的两个飞地
pub struct LocalAttestation {
    key_manager: KeyManager,
}

impl LocalAttestation {
    /// 发起本地认证
    pub fn initiate(
        &self,
        target_enclave: EnclaveId,
    ) -> Result<LocalAttestationSession, AttestationError> {
        // 1. 生成挑战随机数
        let challenge = generate_random_32()?;

        // 2. 创建认证报告
        let report = AttestationReport {
            reporter_enclave: EnclaveIdentity::current(),
            target_enclave,
            challenge: challenge.clone(),
            report_data: self.generate_report_data()?,
            timestamp: Timestamp::now(),
        };

        // 3. 使用飞地身份密钥签名
        let signing_key = self.key_manager.derive_key(KeyPurpose::LocalAttestation)?;
        let signature = sign(signing_key.material(), &report.serialize()?)?;

        Ok(LocalAttestationSession {
            report,
            signature,
            state: AttestationState::ChallengeSent,
        })
    }

    /// 验证本地认证报告
    pub fn verify_report(
        &self,
        report: &AttestationReport,
        signature: &[u8],
    ) -> Result<bool, AttestationError> {
        // 1. 获取目标飞地的身份公钥
        let public_key = self.get_enclave_public_key(&report.reporter_enclave)?;

        // 2. 验证签名
        let valid = verify_signature(public_key, &report.serialize()?, signature)?;

        // 3. 验证飞地身份 (代码哈希、配置等)
        let expected_identity = self.get_expected_identity(&report.reporter_enclave)?;
        if report.reporter_enclave.code_hash != expected_identity.code_hash {
            return Ok(false);
        }

        // 4. 验证时间戳 (防止重放)
        let age = Timestamp::now().duration_since(report.timestamp);
        if age > Duration::from_secs(60) {
            return Ok(false);
        }

        Ok(valid)
    }
}
```

### 6.2 远程认证

```rust
/// 远程认证 - 向远程验证者证明飞地身份
pub struct RemoteAttestation {
    key_manager: KeyManager,
    quote_provider: Box<dyn QuoteProvider>,
}

impl RemoteAttestation {
    /// 生成远程认证引用 (Quote)
    pub fn generate_quote(
        &self,
        user_data: &[u8],
    ) -> Result<AttestationQuote, AttestationError> {
        // 1. 生成飞地报告
        let report = self.generate_enclave_report(user_data)?;

        // 2. 通过引用提供者签名报告
        let quote = self.quote_provider.sign_report(&report)?;

        Ok(quote)
    }

    /// 验证远程认证引用
    pub fn verify_quote(
        &self,
        quote: &AttestationQuote,
        expected_code_hash: &[u8; 32],
    ) -> Result<AttestationVerificationResult, AttestationError> {
        // 1. 验证引用签名 (使用 Intel/AMD 提供的验证密钥)
        let report = self.quote_provider.verify_signature(quote)?;

        // 2. 验证飞地代码哈希
        if report.code_hash != *expected_code_hash {
            return Ok(AttestationVerificationResult::InvalidCode);
        }

        // 3. 验证飞地配置
        if !report.security_flags.memory_encryption {
            return Ok(AttestationVerificationResult::InsufficientSecurity);
        }

        // 4. 验证时间戳
        let age = Timestamp::now().duration_since(report.timestamp);
        if age > Duration::from_secs(300) { // 5 分钟有效期
            return Ok(AttestationVerificationResult::Expired);
        }

        Ok(AttestationVerificationResult::Trusted)
    }
}
```

---

## 7. 飞地 API

### 7.1 公共接口

```rust
/// 飞地公共 API
pub trait EnclaveApi {
    /// 创建飞地
    fn create(config: EnclaveConfig) -> Result<EnclaveHandle, EnclaveError>;

    /// 初始化飞地
    fn initialize(handle: &EnclaveHandle) -> Result<(), EnclaveError>;

    /// 调用飞地函数
    fn call(
        handle: &EnclaveHandle,
        function_id: u32,
        input: &[u8],
    ) -> Result<Vec<u8>, EnclaveError>;

    /// 密封飞地状态
    fn seal(handle: &EnclaveHandle) -> Result<SealedData, EnclaveError>;

    /// 解密封飞地
    fn unseal(
        handle: &EnclaveHandle,
        sealed: &SealedData,
    ) -> Result<(), EnclaveError>;

    /// 销毁飞地
    fn destroy(handle: EnclaveHandle) -> Result<(), EnclaveError>;

    /// 本地认证
    fn attest_local(
        source: &EnclaveHandle,
        target: &EnclaveHandle,
    ) -> Result<AttestationResult, AttestationError>;

    /// 远程认证
    fn attest_remote(
        handle: &EnclaveHandle,
        user_data: &[u8],
    ) -> Result<AttestationQuote, AttestationError>;
}
```

### 7.2 内部函数 ID

| 函数 ID | 名称 | 描述 | 参数 |
|---------|------|------|------|
| 0x0001 | `seal_data` | 密封数据 | (data, associated_data) |
| 0x0002 | `unseal_data` | 解密封数据 | (sealed_data) |
| 0x0003 | `generate_key` | 生成密钥 | (purpose) |
| 0x0004 | `sign_data` | 签名数据 | (data, key_id) |
| 0x0005 | `verify_signature` | 验证签名 | (data, signature, public_key) |
| 0x0006 | `encrypt_data` | 加密数据 | (data, key_id) |
| 0x0007 | `decrypt_data` | 解密数据 | (encrypted_data, key_id) |
| 0x0008 | `derive_key` | 派生密钥 | (purpose, context) |
| 0x0009 | `rotate_keys` | 密钥轮换 | () |
| 0x000A | `get_identity` | 获取飞地身份 | () |
| 0x000B | `attest` | 本地认证 | (target_enclave_id) |
| 0x000C | `destroy_sensitive` | 安全擦除 | (key_id) |

---

## 8. 使用场景

### 8.1 Agent 密钥存储

```rust
/// Agent 在飞地中安全存储密钥
pub struct AgentKeyStore {
    enclave: EnclaveHandle,
}

impl AgentKeyStore {
    /// 存储 Agent 的 API 密钥
    pub fn store_api_key(
        &self,
        agent_id: &AgentId,
        provider: &str,
        api_key: &str,
    ) -> Result<(), EnclaveError> {
        let data = ApiKeyData {
            agent_id: agent_id.clone(),
            provider: provider.to_string(),
            api_key: api_key.to_string(),
            stored_at: Timestamp::now(),
        };

        let serialized = serde_json::to_vec(&data)
            .map_err(|_| EnclaveError::SerializationFailed)?;

        // 在飞地内加密并密封
        let encrypted = self.enclave.call(
            0x0006, // encrypt_data
            &serialized,
        )?;

        self.enclave.call(
            0x0001, // seal_data
            &encrypted,
        )?;

        Ok(())
    }

    /// 获取 Agent 的 API 密钥 (仅在飞地内解密)
    pub fn get_api_key(
        &self,
        agent_id: &AgentId,
        provider: &str,
    ) -> Result<String, EnclaveError> {
        // 密钥仅在飞地内存中解密，从不暴露给外部
        let request = KeyRetrievalRequest {
            agent_id: agent_id.clone(),
            provider: provider.to_string(),
        };

        let serialized = serde_json::to_vec(&request)
            .map_err(|_| EnclaveError::SerializationFailed)?;

        // 飞地内部解密后，通过安全通道返回
        let result = self.enclave.call(0x0007, &serialized)?;
        let key_data: ApiKeyData = serde_json::from_slice(&result)
            .map_err(|_| EnclaveError::DeserializationFailed)?;

        Ok(key_data.api_key)
    }
}
```

### 8.2 授权秘密管理

```rust
/// 授权服务使用飞地保护授权决策密钥
pub struct AuthSecretStore {
    enclave: EnclaveHandle,
}

impl AuthSecretStore {
    /// 签署授权令牌 (在飞地内完成)
    pub fn sign_auth_token(&self, token: &AuthToken) -> Result<Vec<u8>, EnclaveError> {
        let token_data = serde_json::to_vec(token)
            .map_err(|_| EnclaveError::SerializationFailed)?;

        // 在飞地内签名，私钥永不离开飞地
        let signature = self.enclave.call(0x0004, &token_data)?;
        Ok(signature)
    }

    /// 验证授权令牌签名
    pub fn verify_auth_token(
        &self,
        token: &AuthToken,
        signature: &[u8],
    ) -> Result<bool, EnclaveError> {
        let token_data = serde_json::to_vec(token)
            .map_err(|_| EnclaveError::SerializationFailed)?;

        let public_key = self.get_signing_public_key()?;
        let verify_input = [token_data.as_slice(), public_key.as_slice()].concat();

        let result = self.enclave.call(0x0005, &verify_input)?;
        Ok(result.first() == Some(&1))
    }
}
```

---

## 9. 性能指标

### 9.1 性能目标

| 操作 | 目标延迟 | 测量条件 |
|------|---------|---------|
| 飞地调用 (无数据) | < 10 us | 同核 |
| 飞地调用 (4KB 数据) | < 50 us | 同核 |
| 密封操作 (1KB) | < 100 us | AES-256-GCM |
| 解密封操作 (1KB) | < 100 us | AES-256-GCM |
| 密封操作 (1MB) | < 5 ms | AES-256-GCM |
| 密钥派生 | < 20 us | HKDF-SHA256 |
| 签名 (Ed25519) | < 50 us | 32 字节消息 |
| 本地认证 | < 500 us | 含签名验证 |
| 远程认证 | < 2 ms | 含引用生成 |

### 9.2 性能优化策略

| 优化 | 描述 | 预期收益 |
|------|------|---------|
| **批量调用** | 合并多个飞地调用 | 减少 50% 上下文切换 |
| **共享内存通道** | 大数据通过共享内存传递 | 减少 80% 拷贝开销 |
| **密钥缓存** | 缓存派生密钥 | 消除重复派生开销 |
| **异步密封** | 后台线程执行密封 | 零阻塞调用 |
| **硬件加速** | 使用 AES-NI 指令集 | 加密速度提升 10x |

---

## 10. 安全属性验证

### 10.1 机密性保证

```rust
#[cfg(test)]
mod security_tests {
    /// 验证飞地内存对外部不可读
    #[test]
    fn test_memory_confidentiality() {
        let enclave = create_test_enclave();
        let secret = b"top_secret_data";

        // 写入秘密到飞地内存
        enclave.write_internal(secret);

        // 尝试从外部读取飞地内存
        let external_read = try_read_external_memory(enclave.memory_range());

        // 外部读取应返回全零或错误
        assert!(external_read.is_err() || external_read.unwrap().iter().all(|&b| b == 0));
    }

    /// 验证密封数据不可被篡改
    #[test]
    fn test_sealed_data_integrity() {
        let enclave = create_test_enclave();
        let data = b"important_data";

        let sealed = enclave.seal(data).unwrap();

        // 篡改密封数据
        let mut tampered = sealed.clone();
        tampered.encrypted_data[0] ^= 0xFF;

        // 解密封应失败
        let result = enclave.unseal(&tampered);
        assert!(result.is_err());
    }
}
```

### 10.2 防重放保护

```rust
/// 防重放机制
pub struct ReplayProtection {
    /// 已使用的 nonce 集合
    used_nonces: HashSet<[u8; 32]>,
    /// nonce 过期时间
    nonce_ttl: Duration,
}

impl ReplayProtection {
    /// 验证 nonce 未被使用过
    pub fn verify_nonce(&mut self, nonce: &[u8; 32]) -> Result<(), ReplayError> {
        if self.used_nonces.contains(nonce) {
            return Err(ReplayError::NonceReused);
        }

        self.used_nonces.insert(*nonce);

        // 定期清理过期 nonce
        if self.used_nonces.len() > 10000 {
            self.cleanup_expired();
        }

        Ok(())
    }
}
```

---

## 11. 硬件 TEE 回退

### 11.1 SGX/TDX 支持

```rust
/// 硬件 TEE 检测与回退
pub enum TeeBackend {
    /// Intel SGX
    Sgx(SgxEnclave),
    /// Intel TDX
    Tdx(TdxEnclave),
    /// AMD SEV-SNP
    SevSnp(SevEnclave),
    /// 软件 TEE (回退方案)
    Software(SoftwareEnclave),
}

impl TeeBackend {
    /// 自动检测并选择最佳 TEE 后端
    pub fn auto_detect() -> Self {
        if sgx_is_available() {
            info!("检测到 Intel SGX，使用硬件 TEE");
            return TeeBackend::Sgx(SgxEnclave::new());
        }

        if tdx_is_available() {
            info!("检测到 Intel TDX，使用硬件 TEE");
            return TeeBackend::Tdx(TdxEnclave::new());
        }

        if sev_snp_is_available() {
            info!("检测到 AMD SEV-SNP，使用硬件 TEE");
            return TeeBackend::SevSnp(SevEnclave::new());
        }

        warn!("未检测到硬件 TEE，使用软件 TEE");
        TeeBackend::Software(SoftwareEnclave::new())
    }
}

/// 硬件 vs 软件 TEE 对比
pub fn tee_comparison() -> &'static str {
    "
    | 特性          | SGX/TDX/SEV  | 软件 TEE        |
    |--------------|-------------|-----------------|
    | 内存加密      | 硬件级       | 页表隔离         |
    | 侧信道防护    | 硬件级       | 软件缓解         |
    | 远程认证      | 硬件签名     | 自签名           |
    | 启动速度      | 较慢         | 快               |
    | 内存开销      | EPC 限制     | 无限制           |
    | 可移植性      | 特定硬件     | 任意平台         |
    | 安全级别      | 更高         | 较高             |
    "
}
```
