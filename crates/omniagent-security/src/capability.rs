//! 能力系统
//!
//! 基于能力的访问控制 (Capability-Based Access Control)，
//! 每个 Agent 持有一组能力，用于控制其对系统资源的访问权限。

use std::collections::HashSet;

/// 能力定义
///
/// 每个能力代表一种特定的系统操作权限。
/// 基础能力 (0-99) 用于核心系统功能，扩展能力 (100+) 用于设备与外设访问。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Capability {
    // 基础能力
    /// 创建普通 Agent
    SpawnAgent = 0,
    /// 创建系统级 Agent
    SpawnSystemAgent = 1,
    /// 网络访问
    Network = 2,
    /// 设备访问
    DeviceAccess = 3,
    /// 进程间通信
    Ipc = 4,
    /// 共享内存访问
    SharedMemory = 5,
    /// 文件系统访问
    Filesystem = 6,
    /// 管理员权限
    Admin = 7,
    /// 虚拟化权限
    Virtualization = 8,
    /// 安全飞地访问
    Enclave = 9,
    /// 创建飞地 Agent
    SpawnEnclaved = 10,

    // 扩展能力
    /// 屏幕访问
    ScreenAccess = 100,
    /// 音频访问
    AudioAccess = 101,
    /// 摄像头访问
    CameraAccess = 102,
    /// 麦克风访问
    MicrophoneAccess = 103,
    /// 剪贴板访问
    ClipboardAccess = 104,
    /// 通知访问
    NotificationAccess = 105,
    /// 位置信息访问
    LocationAccess = 106,
    /// 蓝牙访问
    BluetoothAccess = 107,
    /// USB 访问
    UsbAccess = 108,
    /// 打印访问
    PrintAccess = 109,
}

impl Capability {
    /// 获取所有基础能力的列表
    pub fn all_basic() -> Vec<Capability> {
        vec![
            Capability::SpawnAgent,
            Capability::SpawnSystemAgent,
            Capability::Network,
            Capability::DeviceAccess,
            Capability::Ipc,
            Capability::SharedMemory,
            Capability::Filesystem,
            Capability::Admin,
            Capability::Virtualization,
            Capability::Enclave,
            Capability::SpawnEnclaved,
        ]
    }

    /// 获取所有扩展能力的列表
    pub fn all_extended() -> Vec<Capability> {
        vec![
            Capability::ScreenAccess,
            Capability::AudioAccess,
            Capability::CameraAccess,
            Capability::MicrophoneAccess,
            Capability::ClipboardAccess,
            Capability::NotificationAccess,
            Capability::LocationAccess,
            Capability::BluetoothAccess,
            Capability::UsbAccess,
            Capability::PrintAccess,
        ]
    }

    /// 获取所有已知能力的列表
    pub fn all() -> Vec<Capability> {
        let mut caps = Self::all_basic();
        caps.extend(Self::all_extended());
        caps
    }
}

/// 能力集
///
/// 一组能力的集合，支持集合运算（并集、交集、包含检查等）。
#[derive(Debug, Clone)]
pub struct CapabilitySet {
    caps: HashSet<Capability>,
}

impl CapabilitySet {
    /// 创建一个空的能力集
    pub fn new() -> Self {
        CapabilitySet {
            caps: HashSet::new(),
        }
    }

    /// 创建一个空的能力集（别名）
    pub fn empty() -> Self {
        Self::new()
    }

    /// 创建一个包含所有已知能力的能力集
    pub fn all() -> Self {
        let mut set = CapabilitySet::new();
        for cap in Capability::all() {
            set.add(cap);
        }
        set
    }

    /// 添加一个能力
    pub fn add(&mut self, cap: Capability) {
        self.caps.insert(cap);
    }

    /// 移除一个能力
    pub fn remove(&mut self, cap: &Capability) {
        self.caps.remove(cap);
    }

    /// 检查是否包含指定能力
    pub fn has(&self, cap: Capability) -> bool {
        self.caps.contains(&cap)
    }

    /// 检查是否包含另一个能力集中的所有能力
    pub fn contains_all(&self, other: &CapabilitySet) -> bool {
        other.caps.is_subset(&self.caps)
    }

    /// 计算两个能力集的交集
    pub fn intersection(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            caps: self.caps.intersection(&other.caps).cloned().collect(),
        }
    }

    /// 计算两个能力集的并集
    pub fn union(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            caps: self.caps.union(&other.caps).cloned().collect(),
        }
    }

    /// 检查能力集是否为空
    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// 返回能力集中能力的数量
    pub fn count(&self) -> usize {
        self.caps.len()
    }

    /// 返回能力集的迭代器
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.caps.iter()
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_values() {
        // 测试基础能力的值
        assert_eq!(Capability::SpawnAgent as u16, 0);
        assert_eq!(Capability::Admin as u16, 7);
        assert_eq!(Capability::SpawnEnclaved as u16, 10);

        // 测试扩展能力的值
        assert_eq!(Capability::ScreenAccess as u16, 100);
        assert_eq!(Capability::PrintAccess as u16, 109);
    }

    #[test]
    fn test_capability_all() {
        let all = Capability::all();
        assert_eq!(all.len(), 21); // 11 基础 + 10 扩展
    }

    #[test]
    fn test_capability_set_new_and_empty() {
        let set1 = CapabilitySet::new();
        let set2 = CapabilitySet::empty();
        assert!(set1.is_empty());
        assert!(set2.is_empty());
        assert_eq!(set1.count(), 0);
    }

    #[test]
    fn test_capability_set_add_and_remove() {
        let mut set = CapabilitySet::new();
        assert!(!set.has(Capability::Network));

        set.add(Capability::Network);
        assert!(set.has(Capability::Network));
        assert_eq!(set.count(), 1);

        set.remove(&Capability::Network);
        assert!(!set.has(Capability::Network));
        assert_eq!(set.count(), 0);
    }

    #[test]
    fn test_capability_set_all() {
        let set = CapabilitySet::all();
        assert_eq!(set.count(), 21);
        assert!(set.has(Capability::Admin));
        assert!(set.has(Capability::CameraAccess));
    }

    #[test]
    fn test_capability_set_contains_all() {
        let mut full = CapabilitySet::new();
        full.add(Capability::Network);
        full.add(Capability::Filesystem);
        full.add(Capability::Ipc);

        let mut subset = CapabilitySet::new();
        subset.add(Capability::Network);
        subset.add(Capability::Filesystem);

        assert!(full.contains_all(&subset));
        assert!(!subset.contains_all(&full));
    }

    #[test]
    fn test_capability_set_intersection() {
        let mut set_a = CapabilitySet::new();
        set_a.add(Capability::Network);
        set_a.add(Capability::Filesystem);
        set_a.add(Capability::Ipc);

        let mut set_b = CapabilitySet::new();
        set_b.add(Capability::Filesystem);
        set_b.add(Capability::Admin);

        let intersection = set_a.intersection(&set_b);
        assert_eq!(intersection.count(), 1);
        assert!(intersection.has(Capability::Filesystem));
    }

    #[test]
    fn test_capability_set_union() {
        let mut set_a = CapabilitySet::new();
        set_a.add(Capability::Network);
        set_a.add(Capability::Filesystem);

        let mut set_b = CapabilitySet::new();
        set_b.add(Capability::Filesystem);
        set_b.add(Capability::Admin);

        let union = set_a.union(&set_b);
        assert_eq!(union.count(), 3);
        assert!(union.has(Capability::Network));
        assert!(union.has(Capability::Filesystem));
        assert!(union.has(Capability::Admin));
    }

    #[test]
    fn test_capability_set_iter() {
        let mut set = CapabilitySet::new();
        set.add(Capability::Network);
        set.add(Capability::Admin);

        let count = set.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_capability_set_default() {
        let set = CapabilitySet::default();
        assert!(set.is_empty());
    }
}
