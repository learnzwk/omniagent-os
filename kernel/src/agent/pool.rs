//! Agent 池管理
//!
//! 管理系统中所有 Agent 的生命周期。
//! AgentPool 是内核中 Agent 的中央注册表，负责分配句柄、
//! 创建/销毁 ACB、查询 Agent 信息等操作。

use super::control_block::AgentControlBlock;
use crate::syscall::abi::*;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

/// 最大 Agent 数量
#[cfg(not(test))]
const MAX_AGENTS: usize = 4096;
#[cfg(test)]
const MAX_AGENTS: usize = 256;

/// Agent 池
///
/// 使用固定大小数组存储所有 ACB，通过自旋锁保证线程安全。
/// 句柄从 1 开始递增分配，内部使用 (handle - 1) 作为数组索引。
pub struct AgentPool {
    /// Agent 数组 (每个槽位可以是空的或包含一个 ACB)
    agents: Mutex<[Option<AgentControlBlock>; MAX_AGENTS]>,
    /// 下一个可分配的句柄值
    next_handle: AtomicU64,
    /// 当前活跃 Agent 数量
    active_count: AtomicU32,
}

impl AgentPool {
    /// 创建新的空 Agent 池
    ///
    /// 使用 core::array::from_fn 安全初始化所有槽位为 None。
    pub fn new() -> Self {
        AgentPool {
            agents: Mutex::new(core::array::from_fn(|_| None)),
            next_handle: AtomicU64::new(1),
            active_count: AtomicU32::new(0),
        }
    }

    /// 初始化池 (确保所有槽位为 None)
    ///
    /// 在首次使用前调用，将所有槽位显式设为 None。
    /// 仅在 const new() 后需要调用一次。
    pub fn init(&self) {
        let mut agents = self.agents.lock();
        for slot in agents.iter_mut() {
            *slot = None;
        }
    }

    /// 创建新 Agent
    ///
    /// 从 AgentSpec 创建新的 ACB 并加入池中。
    /// 返回新分配的 AgentHandle。
    pub fn spawn(&self, spec: &AgentSpec, creator_pid: u64) -> Result<AgentHandle, SyscallError> {
        // 查找空闲槽位
        let slot = self.find_free_slot().ok_or(SyscallError::EAGAIN)?;

        // 分配全局唯一句柄 (单调递增，永不重复)
        let handle = self.alloc_handle();

        // 创建 ACB
        let acb = AgentControlBlock::new(handle, spec, creator_pid);

        // 插入到池中
        {
            let mut agents = self.agents.lock();
            agents[slot] = Some(acb);
        }

        // 增加活跃计数
        self.active_count.fetch_add(1, Ordering::Relaxed);

        Ok(handle)
    }

    /// 终止 Agent
    ///
    /// 将指定 Agent 的状态设置为 Terminating 并从池中移除。
    /// signal 参数: 0 = 强制终止, 1 = 优雅终止
    pub fn kill(&self, handle: AgentHandle, signal: u32) -> Result<(), SyscallError> {
        if !handle.is_valid() {
            return Err(SyscallError::EINVAL);
        }

        let mut agents = self.agents.lock();
        // 通过句柄线性搜索 ACB
        let idx = agents.iter().position(|acb| {
            acb.as_ref().map_or(false, |a| a.handle == handle)
        }).ok_or(SyscallError::ESRCH)?;

        let acb = &mut agents[idx];
        let current_state = acb.as_ref().unwrap().state();
        // 已终止的 Agent 不能再次终止
        if current_state == AgentState::Terminated {
            return Err(SyscallError::ESRCH);
        }

        // 根据信号类型处理
        match signal {
            0 => {
                // 强制终止: 直接设为 Terminating
                let _ = acb.as_mut().unwrap().set_state(AgentState::Terminating);
            }
            1 => {
                // 优雅终止: 从 Running/Waiting/Frozen -> Terminating
                match current_state {
                    AgentState::Running
                    | AgentState::Waiting
                    | AgentState::Frozen
                    | AgentState::Ready
                    | AgentState::Creating => {
                        let _ = acb.as_mut().unwrap().set_state(AgentState::Terminating);
                    }
                    AgentState::Terminating => {
                        // 已经在终止中
                        return Ok(());
                    }
                    AgentState::Terminated => {
                        return Err(SyscallError::ESRCH);
                    }
                    AgentState::Failed => {
                        // 已失败，直接移除
                    }
                    AgentState::Migrating => {
                        return Err(SyscallError::EAGENT_MIGRATING);
                    }
                }
            }
            _ => {
                return Err(SyscallError::EINVAL);
            }
        }

        // 从池中移除
        agents[idx] = None;
        self.active_count.fetch_sub(1, Ordering::Relaxed);
        Ok(())
    }

    /// 查询 Agent 信息
    ///
    /// 返回指定 Agent 的状态快照。
    pub fn query(&self, handle: AgentHandle) -> Result<AgentInfo, SyscallError> {
        if !handle.is_valid() {
            return Err(SyscallError::EINVAL);
        }

        let agents = self.agents.lock();
        let acb = agents.iter().find_map(|acb| {
            acb.as_ref().filter(|a| a.handle == handle)
        }).ok_or(SyscallError::ESRCH)?;
        Ok(acb.to_info())
    }

    /// 列出所有活跃 Agent 的句柄
    ///
    /// 将活跃 Agent 的句柄写入提供的缓冲区，返回实际写入的数量。
    pub fn list(&self, buf: &mut [AgentHandle]) -> usize {
        let agents = self.agents.lock();
        let mut count = 0;
        for acb_opt in agents.iter() {
            if count >= buf.len() {
                break;
            }
            if let Some(acb) = acb_opt {
                buf[count] = acb.handle;
                count += 1;
            }
        }
        count
    }

    /// 查找 ACB (不可变引用，返回克隆)
    ///
    /// 返回指定句柄对应的 ACB 的克隆副本。
    pub fn get(&self, handle: AgentHandle) -> Option<AgentControlBlock> {
        if !handle.is_valid() {
            return None;
        }
        let agents = self.agents.lock();
        agents.iter().find_map(|acb| {
            acb.as_ref().filter(|a| a.handle == handle).cloned()
        })
    }

    /// 活跃 Agent 数量
    pub fn active_count(&self) -> u32 {
        self.active_count.load(Ordering::Relaxed)
    }

    /// 分配新句柄
    ///
    /// 原子递增并返回唯一的句柄值。
    fn alloc_handle(&self) -> AgentHandle {
        let handle_val = self.next_handle.fetch_add(1, Ordering::Relaxed);
        AgentHandle(handle_val)
    }

    /// 查找空闲槽位
    ///
    /// 线性搜索第一个 None 槽位。
    fn find_free_slot(&self) -> Option<usize> {
        let agents = self.agents.lock();
        for (i, slot) in agents.iter().enumerate() {
            if slot.is_none() {
                return Some(i);
            }
        }
        None
    }

    /// 检查 Agent 是否存在
    pub fn exists(&self, handle: AgentHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }
        let agents = self.agents.lock();
        agents.iter().any(|acb| {
            acb.as_ref().map_or(false, |a| a.handle == handle)
        })
    }

    /// 获取池容量
    pub fn capacity(&self) -> usize {
        MAX_AGENTS
    }
}

/// 全局 Agent 池实例
static AGENT_POOL: spin::Lazy<Mutex<AgentPool>> = spin::Lazy::new(|| {
    Mutex::new(AgentPool::new())
});

/// 初始化全局 Agent 池
///
/// 创建并初始化 Agent 池，将所有槽位设为空。
/// 在内核启动的子系统初始化阶段调用。
pub fn init() {
    let pool = AGENT_POOL.lock();
    pool.init();
}

/// 获取全局 Agent 池的引用
///
/// 返回全局 Agent 池的锁守卫，用于执行 Agent 管理操作。
pub fn global_pool() -> &'static spin::Lazy<Mutex<AgentPool>> {
    &AGENT_POOL
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用 AgentSpec
    fn make_spec(name: &str) -> AgentSpec {
        let mut spec = AgentSpec::default();
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(63);
        spec.name[..len].copy_from_slice(&name_bytes[..len]);
        spec.agent_type = AgentType::Generic;
        spec
    }

    /// 创建已初始化的 Agent 池
    fn new_pool() -> AgentPool {
        let pool = AgentPool::new();
        pool.init();
        pool
    }

    #[test]
    fn test_pool_spawn_agent() {
        let pool = new_pool();
        let spec = make_spec("test_agent");
        let handle = pool.spawn(&spec, 1).unwrap();

        assert!(handle.is_valid());
        assert_eq!(handle.0, 1); // 第一个分配的句柄
        assert_eq!(pool.active_count(), 1);
        assert!(pool.exists(handle));
    }

    #[test]
    fn test_pool_spawn_multiple() {
        let pool = new_pool();

        let h1 = pool.spawn(&make_spec("agent_1"), 1).unwrap();
        let h2 = pool.spawn(&make_spec("agent_2"), 1).unwrap();
        let h3 = pool.spawn(&make_spec("agent_3"), 1).unwrap();

        // 句柄应该唯一且递增
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        assert!(h2.0 > h1.0);
        assert!(h3.0 > h2.0);

        assert_eq!(pool.active_count(), 3);
    }

    #[test]
    fn test_pool_kill_agent() {
        let pool = new_pool();
        let handle = pool.spawn(&make_spec("killable"), 1).unwrap();
        assert_eq!(pool.active_count(), 1);

        // 强制终止
        let result = pool.kill(handle, 0);
        assert!(result.is_ok());
        assert_eq!(pool.active_count(), 0);
        assert!(!pool.exists(handle));
    }

    #[test]
    fn test_pool_kill_agent_graceful() {
        let pool = new_pool();
        let handle = pool.spawn(&make_spec("graceful"), 1).unwrap();

        // 优雅终止
        let result = pool.kill(handle, 1);
        assert!(result.is_ok());
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_pool_query_agent() {
        let pool = new_pool();
        let spec = make_spec("query_me");
        let handle = pool.spawn(&spec, 42).unwrap();

        let info = pool.query(handle).unwrap();

        assert_eq!(info.handle, handle);
        assert_eq!(info.state, AgentState::Creating);
        assert_eq!(info.agent_type, AgentType::Generic);
        assert_eq!(info.creator_pid, 42);

        // 验证名称
        let name_str = core::str::from_utf8(&info.name).unwrap_or("");
        assert!(name_str.starts_with("query_me"));
    }

    #[test]
    fn test_pool_list_agents() {
        let pool = new_pool();

        // 空池
        let mut buf = [AgentHandle::INVALID; 16];
        assert_eq!(pool.list(&mut buf), 0);

        // 创建几个 Agent
        let h1 = pool.spawn(&make_spec("a1"), 1).unwrap();
        let h2 = pool.spawn(&make_spec("a2"), 1).unwrap();
        let h3 = pool.spawn(&make_spec("a3"), 1).unwrap();

        let mut buf = [AgentHandle::INVALID; 16];
        let count = pool.list(&mut buf);
        assert_eq!(count, 3);

        // 验证列出的句柄包含创建的
        let handles: Vec<AgentHandle> = buf[..count].to_vec();
        assert!(handles.contains(&h1));
        assert!(handles.contains(&h2));
        assert!(handles.contains(&h3));
    }

    #[test]
    fn test_pool_list_buffer_too_small() {
        let pool = new_pool();
        pool.spawn(&make_spec("a1"), 1).unwrap();
        pool.spawn(&make_spec("a2"), 1).unwrap();
        pool.spawn(&make_spec("a3"), 1).unwrap();

        // 缓冲区只能容纳 2 个
        let mut buf = [AgentHandle::INVALID; 2];
        let count = pool.list(&mut buf);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_pool_query_nonexistent() {
        let pool = new_pool();

        // 查询不存在的 Agent
        let result = pool.query(AgentHandle(999));
        assert_eq!(result, Err(SyscallError::ESRCH));

        // 查询无效句柄
        let result = pool.query(AgentHandle::INVALID);
        assert_eq!(result, Err(SyscallError::EINVAL));
    }

    #[test]
    fn test_pool_kill_nonexistent() {
        let pool = new_pool();

        // 终止不存在的 Agent
        let result = pool.kill(AgentHandle(999), 0);
        assert_eq!(result, Err(SyscallError::ESRCH));

        // 终止无效句柄
        let result = pool.kill(AgentHandle::INVALID, 0);
        assert_eq!(result, Err(SyscallError::EINVAL));
    }

    #[test]
    fn test_pool_max_agents() {
        let pool = new_pool();

        // 填满池
        for i in 0..MAX_AGENTS {
            let name = format!("agent_{:04}", i);
            let spec = make_spec(&name);
            let result = pool.spawn(&spec, 1);
            assert!(result.is_ok(), "创建 Agent {} 失败", i);
        }
        assert_eq!(pool.active_count(), MAX_AGENTS as u32);

        // 再创建一个应该失败
        let result = pool.spawn(&make_spec("overflow"), 1);
        assert_eq!(result, Err(SyscallError::EAGAIN));
    }

    #[test]
    fn test_pool_unique_handles() {
        let pool = new_pool();

        // 创建并销毁多个 Agent，验证句柄始终唯一
        let mut handles = Vec::new();
        for _ in 0..100 {
            let handle = pool.spawn(&make_spec("temp"), 1).unwrap();
            handles.push(handle);
        }

        // 验证所有句柄唯一
        for i in 0..handles.len() {
            for j in (i + 1)..handles.len() {
                assert_ne!(handles[i], handles[j], "发现重复句柄");
            }
        }

        // 销毁一半
        for i in 0..50 {
            pool.kill(handles[i], 0).unwrap();
        }

        // 再创建新的，句柄应该仍然唯一且不与之前的重复
        let mut new_handles = Vec::new();
        for _ in 0..50 {
            let handle = pool.spawn(&make_spec("new_temp"), 1).unwrap();
            new_handles.push(handle);
        }

        // 新句柄不应与任何旧句柄重复
        for new_h in &new_handles {
            for old_h in &handles {
                assert_ne!(new_h, old_h, "新句柄与旧句柄重复");
            }
        }
    }

    #[test]
    fn test_pool_get_agent() {
        let pool = new_pool();
        let spec = make_spec("gettable");
        let handle = pool.spawn(&spec, 1).unwrap();

        let acb = pool.get(handle);
        assert!(acb.is_some());
        let acb = acb.unwrap();
        assert_eq!(acb.handle, handle);
        assert_eq!(acb.creator_pid, 1);
    }

    #[test]
    fn test_pool_get_nonexistent() {
        let pool = new_pool();
        let acb = pool.get(AgentHandle(999));
        assert!(acb.is_none());
    }

    #[test]
    fn test_pool_capacity() {
        let pool = new_pool();
        assert_eq!(pool.capacity(), MAX_AGENTS);
    }

    #[test]
    fn test_pool_kill_invalid_signal() {
        let pool = new_pool();
        let handle = pool.spawn(&make_spec("signal_test"), 1).unwrap();

        // 无效信号值
        let result = pool.kill(handle, 99);
        assert_eq!(result, Err(SyscallError::EINVAL));
        // Agent 应该仍然存在
        assert!(pool.exists(handle));
    }

    #[test]
    fn test_pool_spawn_after_kill() {
        let pool = new_pool();

        // 创建 -> 终止 -> 再创建
        let h1 = pool.spawn(&make_spec("first"), 1).unwrap();
        pool.kill(h1, 0).unwrap();
        assert_eq!(pool.active_count(), 0);

        let h2 = pool.spawn(&make_spec("second"), 1).unwrap();
        assert!(h2.is_valid());
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn test_pool_initial_active_count() {
        let pool = new_pool();
        assert_eq!(pool.active_count(), 0);
    }

    /// 测试：模块级 init 函数
    #[test]
    fn test_pool_init() {
        // 调用模块级 init 函数
        init();

        // 验证全局池可访问
        let pool = AGENT_POOL.lock();
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.capacity(), MAX_AGENTS);
    }
}
