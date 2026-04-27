//! 优先级系统
//!
//! 定义任务优先级类、调度信息（SchedInfo）以及 CFS 风格的
//! 虚拟运行时间（vruntime）计算逻辑。
//!
//! 优先级从低到高：Idle < Normal < Agent < High < Realtime
//! 权重越大，vruntime 增长越慢，获得的 CPU 时间越多。

/// 优先级类
///
/// 定义了五个优先级等级，对应不同的调度权重和时间片。
/// 数值越大优先级越高。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PriorityClass {
    /// 空闲优先级 — 仅在无其他可运行任务时调度
    Idle     = 0,
    /// 普通优先级 — 默认的用户任务优先级
    Normal   = 1,
    /// Agent 优先级 — AI Agent 任务的专用优先级
    Agent    = 2,
    /// 高优先级 — 需要快速响应的系统任务
    High     = 3,
    /// 实时优先级 — 具有最高调度权重
    Realtime = 4,
}

impl PriorityClass {
    /// 获取优先级对应的调度权重
    ///
    /// 权重值越大，vruntime 增长越慢，获得的 CPU 时间越多。
    /// 基于 Linux CFS 的 nice 值到权重映射。
    pub fn weight(&self) -> u32 {
        match self {
            PriorityClass::Idle     => 3,
            PriorityClass::Normal   => 1024,
            PriorityClass::Agent    => 1536,
            PriorityClass::High     => 2048,
            PriorityClass::Realtime => 4096,
        }
    }

    /// 获取优先级对应的基础时间片（纳秒）
    ///
    /// 高优先级任务获得更长的时间片，减少调度开销。
    pub fn base_time_slice_ns(&self) -> u64 {
        match self {
            PriorityClass::Idle     => 1_000_000,     // 1ms
            PriorityClass::Normal   => 5_000_000,     // 5ms
            PriorityClass::Agent    => 8_000_000,     // 8ms
            PriorityClass::High     => 10_000_000,    // 10ms
            PriorityClass::Realtime => 20_000_000,    // 20ms
        }
    }

    /// 从 u8 值创建优先级类
    ///
    /// 有效范围 0-4，超出范围返回 None。
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(PriorityClass::Idle),
            1 => Some(PriorityClass::Normal),
            2 => Some(PriorityClass::Agent),
            3 => Some(PriorityClass::High),
            4 => Some(PriorityClass::Realtime),
            _ => None,
        }
    }
}

/// 调度信息
///
/// 每个任务关联一个 SchedInfo，记录 CFS 调度所需的
/// 虚拟运行时间、实际运行时间、权重和时间片等数据。
#[derive(Debug, Clone)]
pub struct SchedInfo {
    /// 虚拟运行时间（纳秒）— CFS 调度的核心指标
    pub vruntime: u64,
    /// 实际运行时间（纳秒）
    pub runtime: u64,
    /// 优先级类
    pub priority: PriorityClass,
    /// 调度权重
    pub weight: u32,
    /// 剩余时间片（纳秒）
    pub time_slice_remain: u64,
    /// 上次调度时的 tick 计数
    pub last_sched_tick: u64,
}

impl SchedInfo {
    /// 创建新的调度信息
    ///
    /// 初始化 vruntime 为 0，时间片为优先级对应的基础值。
    pub fn new(priority: PriorityClass) -> Self {
        let weight = priority.weight();
        SchedInfo {
            vruntime: 0,
            runtime: 0,
            priority,
            weight,
            time_slice_remain: priority.base_time_slice_ns(),
            last_sched_tick: 0,
        }
    }

    /// 更新虚拟运行时间
    ///
    /// 根据实际运行时间和任务权重计算 vruntime 增量。
    /// 公式：delta_vruntime = delta_runtime * 1024 / weight
    /// 权重越大（优先级越高），vruntime 增长越慢。
    pub fn update_vruntime(&mut self, delta_runtime_ns: u64) {
        if delta_runtime_ns == 0 || self.weight == 0 {
            return;
        }
        // 使用 1024 作为基准权重进行归一化
        let delta_vruntime = (delta_runtime_ns * 1024) / self.weight as u64;
        self.vruntime += delta_vruntime;
        self.runtime += delta_runtime_ns;
    }

    /// 计算分配的时间片
    ///
    /// 基于 CFS 公式：time_slice = sched_period * weight / total_weight
    /// 保证高权重任务获得更大的时间片。
    pub fn calc_time_slice(&self, total_weight: u64, sched_period_ns: u64) -> u64 {
        if total_weight == 0 {
            return self.priority.base_time_slice_ns();
        }
        let slice = (sched_period_ns * self.weight as u64) / total_weight;
        // 确保至少 1ms 的时间片
        if slice < 1_000_000 {
            1_000_000
        } else {
            slice
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 1：优先级类排序 — Idle < Normal < Agent < High < Realtime
    #[test]
    fn test_priority_class_ordering() {
        assert!(PriorityClass::Idle < PriorityClass::Normal);
        assert!(PriorityClass::Normal < PriorityClass::Agent);
        assert!(PriorityClass::Agent < PriorityClass::High);
        assert!(PriorityClass::High < PriorityClass::Realtime);

        // 验证完整的排序链
        let priorities = [
            PriorityClass::Idle,
            PriorityClass::Normal,
            PriorityClass::Agent,
            PriorityClass::High,
            PriorityClass::Realtime,
        ];
        for i in 0..priorities.len() - 1 {
            assert!(priorities[i] < priorities[i + 1]);
        }
    }

    /// 测试 2：优先级权重值验证
    #[test]
    fn test_priority_weights() {
        assert_eq!(PriorityClass::Idle.weight(), 3);
        assert_eq!(PriorityClass::Normal.weight(), 1024);
        assert_eq!(PriorityClass::Agent.weight(), 1536);
        assert_eq!(PriorityClass::High.weight(), 2048);
        assert_eq!(PriorityClass::Realtime.weight(), 4096);
    }

    /// 测试 3：vruntime 更新计算正确性
    #[test]
    fn test_vruntime_update() {
        let mut info = SchedInfo::new(PriorityClass::Normal);
        assert_eq!(info.vruntime, 0);

        // Normal 权重 = 1024，delta_vruntime = delta_runtime * 1024 / 1024 = delta_runtime
        info.update_vruntime(10_000_000);
        assert_eq!(info.vruntime, 10_000_000);
        assert_eq!(info.runtime, 10_000_000);

        // 再运行 5ms
        info.update_vruntime(5_000_000);
        assert_eq!(info.vruntime, 15_000_000);
        assert_eq!(info.runtime, 15_000_000);
    }

    /// 测试 4：Agent 的 vruntime 增长比 Normal 慢
    #[test]
    fn test_vruntime_agent_slower() {
        let mut agent_info = SchedInfo::new(PriorityClass::Agent);
        let mut normal_info = SchedInfo::new(PriorityClass::Normal);

        // 两个任务运行相同的实际时间
        let runtime = 10_000_000; // 10ms
        agent_info.update_vruntime(runtime);
        normal_info.update_vruntime(runtime);

        // Agent 权重 1536 > Normal 权重 1024
        // Agent vruntime = 10_000_000 * 1024 / 1536 = 6_666_666
        // Normal vruntime = 10_000_000 * 1024 / 1024 = 10_000_000
        assert!(agent_info.vruntime < normal_info.vruntime,
            "Agent vruntime ({}) 应小于 Normal vruntime ({})",
            agent_info.vruntime, normal_info.vruntime);

        // 验证具体数值
        assert_eq!(normal_info.vruntime, 10_000_000);
        assert_eq!(agent_info.vruntime, 6_666_666);
    }

    /// 测试 5：时间片计算
    #[test]
    fn test_time_slice_calculation() {
        let info = SchedInfo::new(PriorityClass::Normal);

        // 总权重 = 1024（只有一个 Normal 任务），调度周期 = 6ms
        // time_slice = 6_000_000 * 1024 / 1024 = 6_000_000
        let slice = info.calc_time_slice(1024, 6_000_000);
        assert_eq!(slice, 6_000_000);

        // 总权重 = 2048（Normal + Agent），调度周期 = 6ms
        // time_slice = 6_000_000 * 1024 / 2048 = 3_000_000
        let slice = info.calc_time_slice(2048, 6_000_000);
        assert_eq!(slice, 3_000_000);

        // 总权重 = 0 时返回基础时间片
        let slice = info.calc_time_slice(0, 6_000_000);
        assert_eq!(slice, PriorityClass::Normal.base_time_slice_ns());
    }

    /// 测试 6：u8 转换优先级类
    #[test]
    fn test_priority_from_u8() {
        assert_eq!(PriorityClass::from_u8(0), Some(PriorityClass::Idle));
        assert_eq!(PriorityClass::from_u8(1), Some(PriorityClass::Normal));
        assert_eq!(PriorityClass::from_u8(2), Some(PriorityClass::Agent));
        assert_eq!(PriorityClass::from_u8(3), Some(PriorityClass::High));
        assert_eq!(PriorityClass::from_u8(4), Some(PriorityClass::Realtime));
        assert_eq!(PriorityClass::from_u8(5), None);
        assert_eq!(PriorityClass::from_u8(255), None);
    }
}
