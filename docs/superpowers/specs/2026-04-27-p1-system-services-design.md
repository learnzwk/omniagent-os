# OmniAgent OS 系统完善设计文档 — P1 系统服务集成

> **文档版本**: v1.0.0
> **日期**: 2026-04-27
> **状态**: 待审阅
> **范围**: 文件系统内核集成、网络栈内核集成、libagent syscall 封装、安全能力桥接、块设备驱动框架

---

## 0. 执行摘要

P1 阶段将现有的独立 crate（omniagent-fs、omniagent-net、omniagent-security）集成到内核 syscall 层，使 Agent 能够通过系统调用执行文件 I/O、网络通信和安全操作。同时完善 libagent 的用户态 API，提供真正的 syscall 封装。

**依赖关系：**
```
块设备驱动框架 ──→ 文件系统内核集成
                         │
libagent syscall 封装 ←──┤──→ 网络栈内核集成
                         │
安全能力桥接 ←────────────┘
```

---

## 1. 文件系统内核集成

### 1.1 设计动机

`omniagent-fs` crate 已实现完整的内存文件系统（VFS + AgentFS），但完全独立于内核。Agent 无法通过 syscall 执行 open/read/write/close 等操作。需要将文件系统接入内核 syscall 分发层。

### 1.2 设计方案

**架构：** 内核 VFS 层 + omniagent-fs 作为后端实现

```
用户态 Agent
    │
    │ syscall (open/read/write/close/stat)
    ▼
┌─────────────────────────┐
│  Syscall Dispatch Layer │  ← dispatch.rs 中实现
│  SYS_OPEN (2)           │
│  SYS_READ (0)           │
│  SYS_WRITE (1)          │
│  SYS_CLOSE (3)          │
│  SYS_STAT (4)           │
│  SYS_FSTAT (5)          │
│  SYS_MKDIR (83)         │
│  SYS_UNLINK (87)        │
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│  Kernel VFS Layer       │  ← kernel/src/fs/ 新建
│  ├── fd_table.rs        │  文件描述符表（per-task）
│  ├── vfs.rs             │  VFS 统一接口
│  └── mount.rs           │  挂载管理
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│  omniagent-fs crate     │  ← 已有实现
│  ├── VirtualFileSystem  │
│  ├── AgentFileSystem    │
│  └── INode              │
└─────────────────────────┘
```

### 1.3 文件描述符表

```rust
/// 文件描述符表（每个任务一份）
pub struct FileDescriptorTable {
    /// 文件描述符数组
    fds: [Option<FdEntry>; MAX_FDS],
    /// 下一个可用的 fd 编号
    next_fd: AtomicU32,
    /// 已打开的 fd 数量
    open_count: AtomicU32,
}

const MAX_FDS: usize = 1024;

/// 文件描述符条目
pub struct FdEntry {
    /// 文件描述符编号
    pub fd: u32,
    /// 引用的 inode
    pub inode: Arc<dyn VfsInode>,
    /// 打开标志
    pub flags: OpenFlags,
    /// 当前偏移量
    pub offset: AtomicU64,
    /// 文件类型
    pub file_type: FileType,
}

/// VFS Inode trait（桥接 omniagent-fs 的 INode）
pub trait VfsInode: Send + Sync {
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError>;
    fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, FsError>;
    fn stat(&self) -> Result<FileStat, FsError>;
    fn as_any(&self) -> &dyn core::any::Any;
}

/// 文件状态信息（与 Linux stat 兼容）
#[repr(C)]
pub struct FileStat {
    pub st_dev: u64,      // 设备号
    pub st_ino: u64,      // inode 号
    pub st_mode: u32,     // 文件模式
    pub st_nlink: u32,    // 硬链接数
    pub st_uid: u32,      // 所有者 UID
    pub st_gid: u32,      // 所有者 GID
    pub st_size: u64,     // 文件大小
    pub st_blksize: u32,  // 块大小
    pub st_blocks: u64,   // 块数量
    pub st_atime: u64,    // 访问时间
    pub st_mtime: u64,    // 修改时间
    pub st_ctime: u64,    // 创建时间
}
```

### 1.4 VFS 挂载管理

```rust
/// 挂载点
pub struct MountPoint {
    /// 挂载路径（如 "/sys", "/agent"）
    pub path: PathBuf,
    /// 挂载的文件系统实例
    pub fs: Arc<dyn FileSystem>,
    /// 挂载标志
    pub flags: MountFlags,
}

/// 文件系统 trait
pub trait FileSystem: Send + Sync {
    fn root_inode(&self) -> Result<Arc<dyn VfsInode>, FsError>;
    fn name(&self) -> &str;
}

/// VFS 管理器
pub struct VfsManager {
    /// 挂载点列表
    mounts: SpinLock<Vec<MountPoint>>,
    /// 全局根文件系统
    root_fs: Option<Arc<dyn FileSystem>>,
}

impl VfsManager {
    /// 挂载文件系统
    pub fn mount(&self, path: &str, fs: Arc<dyn FileSystem>) -> Result<(), FsError>;

    /// 卸载文件系统
    pub fn unmount(&self, path: &str) -> Result<(), FsError>;

    /// 解析路径到 inode
    pub fn resolve_path(&self, path: &str) -> Result<Arc<dyn VfsInode>, FsError>;

    /// 创建目录
    pub fn mkdir(&self, path: &str) -> Result<(), FsError>;

    /// 删除文件/目录
    pub fn remove(&self, path: &str) -> Result<(), FsError>;

    /// 列出目录内容
    pub fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
}
```

### 1.5 Syscall 实现

在 `dispatch.rs` 中实现以下 syscall：

```rust
// SYS_OPEN (2)
fn sys_open(path: *const u8, flags: i32, mode: u32) -> Result<i32, SyscallError> {
    let path_str = copy_path_from_user(path)?;
    let inode = VFS.resolve_path(&path_str)?;
    let fd = current_task().fd_table.open(inode, OpenFlags::from_bits(flags))?;
    Ok(fd as i32)
}

// SYS_READ (0)
fn sys_read(fd: i32, buf: *mut u8, count: usize) -> Result<isize, SyscallError> {
    let entry = current_task().fd_table.get(fd as u32)?;
    let mut user_buf = UserBuffer::new(buf, count)?;
    let n = entry.inode.read(entry.offset.load(Relaxed), &mut user_buf)?;
    entry.offset.fetch_add(n as u64, Relaxed);
    Ok(n as isize)
}

// SYS_WRITE (1)
fn sys_write(fd: i32, buf: *const u8, count: usize) -> Result<isize, SyscallError> {
    let entry = current_task().fd_table.get(fd as u32)?;
    let user_buf = UserBuffer::new(buf, count)?;
    let n = entry.inode.write(entry.offset.load(Relaxed), &user_buf)?;
    entry.offset.fetch_add(n as u64, Relaxed);
    Ok(n as isize)
}

// SYS_CLOSE (3)
fn sys_close(fd: i32) -> Result<i32, SyscallError> {
    current_task().fd_table.close(fd as u32)?;
    Ok(0)
}

// SYS_STAT (4) / SYS_FSTAT (5)
fn sys_fstat(fd: i32, stat_buf: *mut FileStat) -> Result<i32, SyscallError> {
    let entry = current_task().fd_table.get(fd as u32)?;
    let stat = entry.inode.stat()?;
    copy_to_user(stat_buf, &stat)?;
    Ok(0)
}

// SYS_MKDIR (83)
fn sys_mkdir(path: *const u8, mode: u32) -> Result<i32, SyscallError> {
    let path_str = copy_path_from_user(path)?;
    VFS.mkdir(&path_str)?;
    Ok(0)
}

// SYS_UNLINK (87)
fn sys_unlink(path: *const u8) -> Result<i32, SyscallError> {
    let path_str = copy_path_from_user(path)?;
    VFS.remove(&path_str)?;
    Ok(0)
}
```

### 1.6 测试策略

```
TDD 测试用例：
1. test_fd_table_open_close — 打开和关闭文件描述符
2. test_fd_table_read_write — 读写操作
3. test_fd_table_seek — 偏移量管理
4. test_fd_table_max_fds — 超过最大 fd 数
5. test_fd_table_duplicate_fd — 复制文件描述符
6. test_vfs_mount_unmount — 挂载和卸载
7. test_vfs_resolve_path — 路径解析
8. test_vfs_nested_path — 嵌套路径解析 (/sys/agent/file)
9. test_vfs_mkdir — 创建目录
10. test_vfs_remove — 删除文件
11. test_vfs_list_dir — 列出目录
12. test_syscall_open_read_write_close — syscall 完整流程
13. test_syscall_stat — stat 系统调用
14. test_syscall_invalid_fd — 无效 fd 错误处理
15. test_agent_filesystem_isolation — Agent 文件系统隔离
```

### 1.7 文件结构

```
kernel/src/
├── fs/
│   ├── mod.rs          # 新建：文件系统模块声明
│   ├── fd_table.rs     # 新建：文件描述符表
│   ├── vfs.rs          # 新建：VFS 管理器
│   ├── mount.rs        # 新建：挂载管理
│   ├── inode.rs        # 新建：VFS Inode trait
│   └── path.rs         # 新建：路径解析工具
└── syscall/
    └── dispatch.rs     # 修改：实现文件系统 syscall
```

---

## 2. 网络栈内核集成

### 2.1 设计动机

`omniagent-net` crate 已实现 TCP 状态机、UDP 套接字、DNS 缓存等，但完全独立于内核。Agent 无法通过 syscall 创建 socket 或进行网络通信。

### 2.2 设计方案

**架构：** 内核网络层 + omniagent-net 作为协议实现

```
用户态 Agent
    │
    │ syscall (socket/bind/connect/send/recv/close)
    ▼
┌─────────────────────────┐
│  Syscall Dispatch Layer │
│  SYS_SOCKET (41)        │
│  SYS_BIND (49)          │
│  SYS_CONNECT (42)       │
│  SYS_SEND (44)          │
│  SYS_RECV (45)          │
│  SYS_SENDTO (44)        │
│  SYS_RECVFROM (45)      │
│  SYS_SHUTDOWN (48)      │
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│  Kernel Network Layer   │  ← kernel/src/net/ 新建
│  ├── socket_table.rs    │  Socket 表（per-task）
│  ├── net_manager.rs     │  网络管理器
│  └── protocol.rs        │  协议分发
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│  omniagent-net crate    │  ← 已有实现
│  ├── TcpConnection      │
│  ├── UdpSocket          │
│  ├── NetworkManager     │
│  └── DnsCache           │
└─────────────────────────┘
```

### 2.3 Socket 表

```rust
/// Socket 描述符表（每个任务一份）
pub struct SocketTable {
    /// Socket 数组
    sockets: [Option<SocketEntry>; MAX_SOCKETS],
    /// 下一个可用编号
    next_fd: AtomicU32,
}

const MAX_SOCKETS: usize = 256;

/// Socket 条目
pub struct SocketEntry {
    /// Socket 编号
    pub fd: u32,
    /// Socket 类型
    pub domain: SocketDomain,
    /// 协议类型
    pub socket_type: SocketType,
    /// 协议实现
    pub protocol: Box<dyn ProtocolSocket>,
    /// 状态
    pub state: SocketState,
    /// 绑定的本地地址
    pub local_addr: Option<SocketAddr>,
    /// 远端地址
    pub remote_addr: Option<SocketAddr>,
}

/// 协议 Socket trait
pub trait ProtocolSocket: Send + Sync {
    fn bind(&mut self, addr: SocketAddr) -> Result<(), NetError>;
    fn connect(&mut self, addr: SocketAddr) -> Result<(), NetError>;
    fn send(&mut self, data: &[u8]) -> Result<usize, NetError>;
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError>;
    fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> Result<usize, NetError>;
    fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError>;
    fn listen(&mut self, backlog: u32) -> Result<(), NetError>;
    fn accept(&mut self) -> Result<(Box<dyn ProtocolSocket>, SocketAddr), NetError>;
    fn shutdown(&mut self, how: ShutdownHow) -> Result<(), NetError>;
    fn close(&mut self) -> Result<(), NetError>;
}

/// Socket 状态
pub enum SocketState {
    Created,
    Bound,
    Listening,
    Connected,
    Closed,
}
```

### 2.4 Syscall 实现

```rust
// SYS_SOCKET (41)
fn sys_socket(domain: i32, socket_type: i32, protocol: i32) -> Result<i32, SyscallError> {
    let domain = SocketDomain::from_raw(domain)?;
    let stype = SocketType::from_raw(socket_type)?;
    let entry = SocketEntry::new(domain, stype, protocol)?;
    let fd = current_task().socket_table.add(entry)?;
    Ok(fd as i32)
}

// SYS_BIND (49)
fn sys_bind(sockfd: i32, addr: *const SockAddr, addrlen: u32) -> Result<i32, SyscallError> {
    let entry = current_task().socket_table.get(sockfd as u32)?;
    let addr = parse_sockaddr(addr, addrlen)?;
    entry.protocol.lock().bind(addr)?;
    entry.local_addr = Some(addr);
    Ok(0)
}

// SYS_CONNECT (42)
fn sys_connect(sockfd: i32, addr: *const SockAddr, addrlen: u32) -> Result<i32, SyscallError> {
    let entry = current_task().socket_table.get(sockfd as u32)?;
    let addr = parse_sockaddr(addr, addrlen)?;
    entry.protocol.lock().connect(addr)?;
    entry.remote_addr = Some(addr);
    Ok(0)
}

// SYS_SEND (44) / SYS_SENDTO
fn sys_send(sockfd: i32, buf: *const u8, len: usize, flags: i32) -> Result<isize, SyscallError> {
    let entry = current_task().socket_table.get(sockfd as u32)?;
    let user_buf = UserBuffer::new(buf, len)?;
    let n = entry.protocol.lock().send(&user_buf)?;
    Ok(n as isize)
}

// SYS_RECV (45) / SYS_RECVFROM
fn sys_recv(sockfd: i32, buf: *mut u8, len: usize, flags: i32) -> Result<isize, SyscallError> {
    let entry = current_task().socket_table.get(sockfd as u32)?;
    let mut user_buf = UserBuffer::new(buf, len)?;
    let n = entry.protocol.lock().recv(&mut user_buf)?;
    Ok(n as isize)
}

// SYS_SHUTDOWN (48)
fn sys_shutdown(sockfd: i32, how: i32) -> Result<i32, SyscallError> {
    let entry = current_task().socket_table.get(sockfd as u32)?;
    let how = ShutdownHow::from_raw(how)?;
    entry.protocol.lock().shutdown(how)?;
    Ok(0)
}
```

### 2.5 测试策略

```
TDD 测试用例：
1. test_socket_table_create — 创建 socket
2. test_socket_table_close — 关闭 socket
3. test_socket_table_max — 超过最大 socket 数
4. test_socket_bind — 绑定地址
5. test_socket_connect — 连接
6. test_socket_send_recv — 发送接收
7. test_socket_sendto_recvfrom — 无连接发送接收
8. test_socket_shutdown — 关闭连接
9. test_tcp_state_machine — TCP 状态转换
10. test_udp_send_recv — UDP 通信
11. test_dns_cache — DNS 缓存
12. test_syscall_socket_lifecycle — socket 生命周期
13. test_syscall_invalid_socket — 无效 socket 错误
14. test_network_manager_interface — 网络接口管理
15. test_concurrent_sockets — 并发 socket 操作
```

### 2.6 文件结构

```
kernel/src/
├── net/
│   ├── mod.rs          # 新建：网络模块声明
│   ├── socket_table.rs # 新建：Socket 表
│   ├── net_manager.rs  # 新建：网络管理器
│   ├── protocol.rs     # 新建：协议 trait + TCP/UDP 适配器
│   └── sockaddr.rs     # 新建：Socket 地址解析
└── syscall/
    └── dispatch.rs     # 修改：实现网络 syscall
```

---

## 3. libagent Syscall 封装

### 3.1 设计动机

当前 `libagent` 的 `raw_syscall` 函数全部返回 `E_NOTSUP`。需要实现真正的 x86_64 syscall 内联汇编，使 Agent 能够与内核通信。

### 3.2 设计方案

**关键决策：**
1. 使用 `core::arch::asm!` 内联汇编实现 syscall
2. 每个公共 API 函数调用 `raw_syscall` 并处理返回值
3. 保持零外部依赖
4. 提供安全的 Rust 包装器

### 3.3 Syscall ABI

```rust
/// x86_64 Linux syscall ABI:
/// - syscall number: rax
/// - args: rdi, rsi, rdx, r10, r8, r9
/// - return value: rax
/// - clobbers: rcx, r11

/// 原始 syscall 调用（内联汇编）
///
/// # Safety
/// - 所有指针参数必须指向有效的用户态内存
/// - 调用者必须确保不会破坏内核不变量
#[inline(always)]
pub unsafe fn raw_syscall(
    number: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        inoutlate("rax") number => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8")  arg5,
        in("r9")  arg6,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    ret
}

/// 检查 syscall 返回值是否为错误
/// Linux x86_64 约定：负值表示错误（-errno）
pub fn check_syscall_result(ret: i64) -> Result<u64, SyscallErrno> {
    if ret < 0 {
        Err(SyscallErrno::from_code(-ret as u32))
    } else {
        Ok(ret as u64)
    }
}
```

### 3.4 Agent Syscall 封装

```rust
impl AgentRuntime {
    /// 创建 Agent
    pub fn spawn(config: &AgentConfig) -> Result<AgentHandle, RuntimeError> {
        let spec = config.build();
        let spec_ptr = &spec as *const AgentSpec as u64;
        let spec_len = core::mem::size_of::<AgentSpec>() as u64;

        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_SPAWN,  // 512
                spec_ptr,
                spec_len,
                0, 0, 0, 0,
            )
        };

        match check_syscall_result(ret as i64) {
            Ok(handle) => Ok(AgentHandle::from_raw(handle)),
            Err(e) => Err(RuntimeError::SyscallFailed(e)),
        }
    }

    /// 终止 Agent
    pub fn kill(handle: AgentHandle, signal: Signal) -> Result<(), RuntimeError> {
        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_KILL,  // 513
                handle.as_raw(),
                signal as u64,
                0, 0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 查询 Agent 信息
    pub fn query(handle: AgentHandle) -> Result<AgentInfoView, RuntimeError> {
        let mut info = AgentInfo::zeroed();
        let info_ptr = &mut info as *mut AgentInfo as u64;
        let info_len = core::mem::size_of::<AgentInfo>() as u64;

        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_QUERY,  // 514
                handle.as_raw(),
                info_ptr,
                info_len,
                0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(AgentInfoView::from(info))
    }

    /// 发送消息
    pub fn send(
        from: AgentHandle,
        to: AgentHandle,
        msg: &AgentMessage,
    ) -> Result<(), RuntimeError> {
        let header = msg.header();
        let header_ptr = &header as *const AgentMsgHeader as u64;
        let payload_ptr = msg.payload_ptr() as u64;
        let payload_len = msg.payload_len() as u64;

        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_MSG,  // 515
                from.as_raw(),
                to.as_raw(),
                header_ptr,
                payload_ptr,
                payload_len,
                0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 订阅事件
    pub fn subscribe(
        handle: AgentHandle,
        topic: &str,
    ) -> Result<(), RuntimeError> {
        let topic_bytes = topic.as_bytes();
        let topic_ptr = topic_bytes.as_ptr() as u64;
        let topic_len = topic_bytes.len() as u64;

        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_SUBSCRIBE,  // 517
                handle.as_raw(),
                topic_ptr,
                topic_len,
                0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 授予能力
    pub fn grant_capability(
        from: AgentHandle,
        to: AgentHandle,
        capability: u64,
    ) -> Result<(), RuntimeError> {
        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_CAP_GRANT,  // 520
                from.as_raw(),
                to.as_raw(),
                capability,
                0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 撤销能力
    pub fn revoke_capability(
        from: AgentHandle,
        to: AgentHandle,
        capability: u64,
    ) -> Result<(), RuntimeError> {
        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_CAP_REVOKE,  // 521
                from.as_raw(),
                to.as_raw(),
                capability,
                0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 设置资源配额
    pub fn set_quota(
        handle: AgentHandle,
        quota: &ResourceQuota,
    ) -> Result<(), RuntimeError> {
        let quota_ptr = quota as *const ResourceQuota as u64;
        let quota_len = core::mem::size_of::<ResourceQuota>() as u64;

        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_SET_QUOTA,  // 525
                handle.as_raw(),
                quota_ptr,
                quota_len,
                0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 获取资源配额
    pub fn get_quota(handle: AgentHandle) -> Result<ResourceQuota, RuntimeError> {
        let mut quota = ResourceQuota::default();
        let quota_ptr = &mut quota as *mut ResourceQuota as u64;

        let ret = unsafe {
            raw_syscall(
                SYS_AGENT_GET_QUOTA,  // 526
                handle.as_raw(),
                quota_ptr,
                core::mem::size_of::<ResourceQuota>() as u64,
                0, 0, 0,
            )
        };
        check_syscall_result(ret as i64)?;
        Ok(quota)
    }
}
```

### 3.5 POSIX Syscall 封装

```rust
/// 文件操作
pub mod fs {
    /// 打开文件
    pub fn open(path: &str, flags: i32, mode: u32) -> Result<i32, RuntimeError> {
        let path_bytes = path.as_bytes();
        let ret = unsafe {
            raw_syscall(2, path_bytes.as_ptr() as u64, flags as u64, mode as u64, 0, 0, 0)
        };
        check_syscall_result(ret as i64).map(|v| v as i32)
    }

    /// 读取文件
    pub fn read(fd: i32, buf: &mut [u8]) -> Result<isize, RuntimeError> {
        let ret = unsafe {
            raw_syscall(0, fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0)
        };
        check_syscall_result(ret as i64).map(|v| v as isize)
    }

    /// 写入文件
    pub fn write(fd: i32, buf: &[u8]) -> Result<isize, RuntimeError> {
        let ret = unsafe {
            raw_syscall(1, fd as u64, buf.as_ptr() as u64, buf.len() as u64, 0, 0, 0)
        };
        check_syscall_result(ret as i64).map(|v| v as isize)
    }

    /// 关闭文件
    pub fn close(fd: i32) -> Result<(), RuntimeError> {
        let ret = unsafe { raw_syscall(3, fd as u64, 0, 0, 0, 0, 0) };
        check_syscall_result(ret as i64)?;
        Ok(())
    }
}

/// 网络操作
pub mod net {
    /// 创建 socket
    pub fn socket(domain: i32, socket_type: i32, protocol: i32) -> Result<i32, RuntimeError> {
        let ret = unsafe {
            raw_syscall(41, domain as u64, socket_type as u64, protocol as u64, 0, 0, 0)
        };
        check_syscall_result(ret as i64).map(|v| v as i32)
    }

    /// 绑定地址
    pub fn bind(sockfd: i32, addr: &SockAddr) -> Result<(), RuntimeError> {
        let ret = unsafe {
            raw_syscall(49, sockfd as u64, addr as *const _ as u64, core::mem::size_of::<SockAddr>() as u64, 0, 0, 0)
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 连接
    pub fn connect(sockfd: i32, addr: &SockAddr) -> Result<(), RuntimeError> {
        let ret = unsafe {
            raw_syscall(42, sockfd as u64, addr as *const _ as u64, core::mem::size_of::<SockAddr>() as u64, 0, 0, 0)
        };
        check_syscall_result(ret as i64)?;
        Ok(())
    }

    /// 发送数据
    pub fn send(sockfd: i32, buf: &[u8], flags: i32) -> Result<isize, RuntimeError> {
        let ret = unsafe {
            raw_syscall(44, sockfd as u64, buf.as_ptr() as u64, buf.len() as u64, flags as u64, 0, 0)
        };
        check_syscall_result(ret as i64).map(|v| v as isize)
    }

    /// 接收数据
    pub fn recv(sockfd: i32, buf: &mut [u8], flags: i32) -> Result<isize, RuntimeError> {
        let ret = unsafe {
            raw_syscall(45, sockfd as u64, buf.as_mut_ptr() as u64, buf.len() as u64, flags as u64, 0, 0)
        };
        check_syscall_result(ret as i64).map(|v| v as isize)
    }
}
```

### 3.6 测试策略

```
TDD 测试用例：
1. test_raw_syscall_returns_value — syscall 返回值正确
2. test_check_syscall_result_ok — 正值返回 Ok
3. test_check_syscall_result_err — 负值返回 Err
4. test_syscall_errno_from_code — errno 转换正确
5. test_agent_spawn_config_build — AgentConfig 构建正确
6. test_agent_spawn_syscall_args — spawn syscall 参数正确
7. test_agent_kill_syscall_args — kill syscall 参数正确
8. test_agent_query_syscall_args — query syscall 参数正确
9. test_agent_send_syscall_args — send syscall 参数正确
10. test_fs_open_syscall_args — open syscall 参数正确
11. test_fs_read_syscall_args — read syscall 参数正确
12. test_fs_write_syscall_args — write syscall 参数正确
13. test_net_socket_syscall_args — socket syscall 参数正确
14. test_net_connect_syscall_args — connect syscall 参数正确
15. test_capability_grant_revoke_args — 能力 syscall 参数正确
```

### 3.7 文件结构

```
crates/libagent/
└── src/
    └── lib.rs         # 修改：替换 syscall 桩为实际内联汇编
```

---

## 4. 安全能力桥接

### 4.1 设计动机

`omniagent-security` crate 定义了 21 种 `Capability` 枚举，而 `libagent` 使用 128 位 `CapBitmap`。两者语义对应但无代码桥接。需要在内核 syscall 层实现 CapBitmap ↔ Capability 的转换。

### 4.2 设计方案

```rust
/// 能力桥接模块
///
/// 将 libagent 的 CapBitmap（128 位位图）与 omniagent-security 的 Capability 枚举互转
pub mod capability_bridge {

use omniagent_security::capability::Capability;
use crate::CapBitmap;

/// CapBitmap 位分配：
/// - bits 0-20:  基本 Capability（对应 Capability 枚举 0-20）
/// - bits 21-63: 保留
/// - bits 64-127: 扩展 Capability

/// 将 Capability 枚举转换为 CapBitmap 位索引
pub fn capability_to_bit(cap: &Capability) -> Option<u32> {
    match cap {
        // 基本能力 (0-20)
        Capability::ReadFiles      => Some(0),
        Capability::WriteFiles     => Some(1),
        Capability::Execute        => Some(2),
        Capability::NetworkAccess  => Some(3),
        Capability::CreateAgent    => Some(4),
        Capability::KillAgent      => Some(5),
        Capability::SendMessage    => Some(6),
        Capability::ReceiveMessage => Some(7),
        Capability::Subscribe      => Some(8),
        Capability::Publish        => Some(9),
        Capability::ManageQuota    => Some(10),
        // 扩展能力 (11-20)
        Capability::ManageSystem   => Some(11),
        Capability::AccessHardware => Some(12),
        Capability::ManageDrivers  => Some(13),
        Capability::ManageMemory   => Some(14),
        Capability::ManageNetwork  => Some(15),
        Capability::ManageSecurity => Some(16),
        Capability::ManageUsers    => Some(17),
        Capability::ManagePolicies => Some(18),
        Capability::AuditAccess    => Some(19),
        Capability::AdminAccess    => Some(20),
        _ => None,  // 未知能力
    }
}

/// 将 CapBitmap 转换为 CapabilitySet
pub fn bitmap_to_capabilities(bitmap: &CapBitmap) -> CapabilitySet {
    let mut set = CapabilitySet::new();
    for cap in Capability::all() {
        if let Some(bit) = capability_to_bit(&cap) {
            if bitmap.test(bit) {
                set.insert(cap);
            }
        }
    }
    set
}

/// 将 CapabilitySet 转换为 CapBitmap
pub fn capabilities_to_bitmap(set: &CapabilitySet) -> CapBitmap {
    let mut bitmap = CapBitmap::new();
    for cap in set.iter() {
        if let Some(bit) = capability_to_bit(&cap) {
            bitmap.set(bit);
        }
    }
    bitmap
}

/// 检查 CapBitmap 是否包含指定能力
pub fn has_capability(bitmap: &CapBitmap, cap: &Capability) -> bool {
    match capability_to_bit(cap) {
        Some(bit) => bitmap.test(bit),
        None => false,
    }
}

/// 授予能力
pub fn grant_capability(bitmap: &mut CapBitmap, cap: &Capability) -> bool {
    match capability_to_bit(cap) {
        Some(bit) => { bitmap.set(bit); true }
        None => false,
    }
}

/// 撤销能力
pub fn revoke_capability(bitmap: &mut CapBitmap, cap: &Capability) -> bool {
    match capability_to_bit(cap) {
        Some(bit) => { bitmap.clear(bit); true }
        None => false,
    }
}
}
```

### 4.3 内核侧能力检查

```rust
/// 在 syscall 分发层检查能力
fn check_capability(agent_handle: u64, required: Capability) -> Result<(), SyscallError> {
    let pool = AGENT_POOL.lock();
    let acb = pool.get(AgentHandle::from_raw(agent_handle))
        .ok_or(SyscallError::E_INVAL)?;

    if !capability_bridge::has_capability(&acb.capabilities, &required) {
        return Err(SyscallError::E_PERM);
    }
    Ok(())
}

// 示例：在文件操作中检查能力
fn sys_open(path: *const u8, flags: i32, mode: u32) -> Result<i32, SyscallError> {
    let current = current_agent_handle();
    if flags & O_WRONLY != 0 || flags & O_RDWR != 0 {
        check_capability(current, Capability::WriteFiles)?;
    } else {
        check_capability(current, Capability::ReadFiles)?;
    }
    // ... 实际 open 逻辑
}
```

### 4.4 测试策略

```
TDD 测试用例：
1. test_capability_to_bit — 所有 21 种能力映射正确
2. test_bitmap_to_capabilities — 位图转能力集
3. test_capabilities_to_bitmap — 能力集转位图
4. test_has_capability — 能力检查
5. test_grant_capability — 授予能力
6. test_revoke_capability — 撤销能力
7. test_roundtrip_conversion — 双向转换一致性
8. test_empty_bitmap — 空位图
9. test_full_bitmap — 全部位设置
10. test_unknown_capability — 未知能力处理
```

### 4.5 文件结构

```
kernel/src/
├── security/
│   ├── mod.rs              # 新建：安全模块声明
│   └── capability_bridge.rs # 新建：能力桥接
└── syscall/
    └── dispatch.rs         # 修改：添加能力检查
```

---

## 5. 块设备驱动框架

### 5.1 设计动机

当前文件系统是纯内存实现，数据不持久化。需要块设备驱动框架来支持磁盘 I/O，为文件系统提供持久化后端。

### 5.2 设计方案

**架构：** 块设备接口 → 驱动实现 → 物理设备

```
┌─────────────────────────┐
│  VFS / 文件系统          │
└────────┬────────────────┘
         │ block_read / block_write
         ▼
┌─────────────────────────┐
│  Block Device Interface │  ← kernel/src/drivers/block/
│  trait BlockDevice      │
│  ├── read_block()       │
│  ├── write_block()      │
│  └── flush()            │
└────────┬────────────────┘
         │
    ┌────┼────────┐
    │    │        │
    ▼    ▼        ▼
┌──────┐ ┌──────┐ ┌──────────┐
│ VirtIO│ │ AHCI │ │ RAM Disk │
│ Block│ │ SATA │ │ (测试用)  │
└──────┘ └──────┘ └──────────┘
```

### 5.3 核心数据结构

```rust
/// 块设备 trait
pub trait BlockDevice: Send + Sync {
    /// 读取一个或多个块
    fn read_blocks(&self, start_lba: u64, buf: &mut [u8]) -> Result<(), BlockError>;

    /// 写入一个或多个块
    fn write_blocks(&self, start_lba: u64, buf: &[u8]) -> Result<(), BlockError>;

    /// 刷新缓存
    fn flush(&self) -> Result<(), BlockError>;

    /// 获取块大小（字节）
    fn block_size(&self) -> usize;

    /// 获取设备容量（块数）
    fn capacity(&self) -> u64;

    /// 设备名称
    fn name(&self) -> &str;

    /// 是否可移除
    fn is_removable(&self) -> bool;
}

/// 块设备错误
pub enum BlockError {
    IoError { reason: &'static str },
    InvalidLba(u64),
    InvalidBufferSize { expected: usize, actual: usize },
    DeviceBusy,
    DeviceNotFound,
    MediaError,
    Timeout,
}

/// 块设备管理器
pub struct BlockDeviceManager {
    /// 已注册的块设备
    devices: SpinLock<Vec<Arc<dyn BlockDevice>>>,
    /// 按名称索引
    name_index: SpinLock<BTreeMap<&'static str, Arc<dyn BlockDevice>>>,
}

impl BlockDeviceManager {
    /// 注册块设备
    pub fn register(&self, device: Arc<dyn BlockDevice>) -> Result<(), BlockError>;

    /// 注销块设备
    pub fn unregister(&self, name: &str) -> Result<(), BlockError>;

    /// 按名称获取设备
    pub fn get(&self, name: &str) -> Option<Arc<dyn BlockDevice>>;

    /// 列出所有设备
    pub fn list(&self) -> Vec<BlockDeviceInfo>;
}

/// 块设备信息
pub struct BlockDeviceInfo {
    pub name: &'static str,
    pub block_size: usize,
    pub capacity: u64,
    pub is_removable: bool,
}

/// RAM Disk（用于测试和开发）
pub struct RamDisk {
    /// 存储数据
    data: SpinLock<Vec<u8>>,
    /// 块大小
    block_size: usize,
    /// 设备名称
    name: &'static str,
}
```

### 5.4 请求队列

```rust
/// 块 I/O 请求
pub struct BlockRequest {
    /// 请求类型
    pub kind: BlockRequestKind,
    /// 起始 LBA
    pub start_lba: u64,
    /// 数据缓冲区
    pub buffer: Vec<u8>,
    /// 完成回调
    pub completion: Option<Box<dyn FnOnce(Result<(), BlockError>) + Send>>,
}

pub enum BlockRequestKind {
    Read,
    Write,
    Flush,
}

/// 块请求队列
pub struct BlockRequestQueue {
    /// 待处理请求
    pending: SpinLock<VecDeque<BlockRequest>>,
    /// 最大队列深度
    max_depth: usize,
    /// 当前队列深度
    depth: AtomicUsize,
}
```

### 5.5 测试策略

```
TDD 测试用例：
1. test_ram_disk_create — 创建 RAM Disk
2. test_ram_disk_read_write — 读写操作
3. test_ram_disk_capacity — 容量正确
4. test_ram_disk_flush — 刷新操作
5. test_block_device_manager_register — 注册设备
6. test_block_device_manager_get — 获取设备
7. test_block_device_manager_unregister — 注销设备
8. test_block_device_manager_list — 列出设备
9. test_block_request_queue — 请求队列
10. test_block_request_queue_full — 队列满处理
11. test_multi_block_read_write — 多块读写
12. test_invalid_lba — 无效 LBA 错误
13. test_buffer_size_mismatch — 缓冲区大小不匹配
14. test_concurrent_block_access — 并发访问
15. test_block_device_info — 设备信息
```

### 5.6 文件结构

```
kernel/src/drivers/
├── mod.rs              # 修改：添加 block 模块
└── block/
    ├── mod.rs          # 新建：块设备模块声明
    ├── device.rs       # 新建：BlockDevice trait
    ├── manager.rs      # 新建：BlockDeviceManager
    ├── ramdisk.rs      # 新建：RAM Disk 实现
    └── request.rs      # 新建：请求队列
```

---

## 6. 跨模块集成

### 6.1 P1 模块依赖图

```
                    ┌──────────────────┐
                    │  libagent (用户态) │
                    │  syscall 封装     │
                    └────────┬─────────┘
                             │ syscall ABI
                             ▼
┌─────────────────────────────────────────────────┐
│                  内核 Syscall 层                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │ FS syscall│  │ Net syscall│ │ Agent syscall│  │
│  └────┬─────┘  └────┬─────┘  └──────┬───────┘  │
│       │              │               │          │
│  ┌────▼─────┐  ┌────▼─────┐  ┌──────▼───────┐  │
│  │ VFS 层   │  │ 网络层   │  │ 能力检查     │  │
│  └────┬─────┘  └────┬─────┘  └──────┬───────┘  │
│       │              │               │          │
│  ┌────▼─────┐  ┌────▼─────┐  ┌──────▼───────┐  │
│  │fd_table  │  │socket_tbl│  │capability    │  │
│  │mount     │  │protocol  │  │bridge        │  │
│  └────┬─────┘  └────┬─────┘  └──────────────┘  │
└───────┼──────────────┼──────────────────────────┘
        │              │
        ▼              ▼
┌──────────────┐ ┌──────────────┐
│ omniagent-fs │ │ omniagent-net│
│ (VFS 后端)   │ │ (协议实现)   │
└──────────────┘ └──────────────┘

┌──────────────────────────────────────────────────┐
│  块设备驱动框架                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │
│  │ BlockDevice│  │RamDisk  │  │RequestQueue  │   │
│  │ trait     │  │         │  │              │   │
│  └──────────┘  └──────────┘  └──────────────┘   │
└──────────────────────────────────────────────────┘
```

### 6.2 内核 lib.rs 更新

```rust
pub mod fs;        // 新增：文件系统
pub mod net;       // 新增：网络
pub mod security;  // 新增：安全
```

### 6.3 Cargo.toml 更新

```toml
# kernel/Cargo.toml 新增依赖
[dependencies]
omniagent-fs = { path = "../crates/omniagent-fs" }
omniagent-net = { path = "../crates/omniagent-net" }
omniagent-security = { path = "../crates/omniagent-security" }
```

---

## 7. 成功标准

| 标准 | 验证方法 |
|------|---------|
| Agent 可通过 syscall 读写文件 | 集成测试：spawn → open → write → read → close |
| Agent 可通过 syscall 网络通信 | 集成测试：socket → bind → connect → send → recv |
| libagent syscall 封装可编译 | `cargo build -p libagent` |
| 能力检查生效 | 测试：无能力 Agent 执行受限操作返回 E_PERM |
| 块设备可读写 | 单元测试：RAM Disk 读写 |
| 所有测试通过 | `cargo test --workspace` 全绿 |
