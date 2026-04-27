//! 能力桥接模块：将 CapBitmap (128位) 与 Capability 枚举互转
//!
//! 本模块实现了内核态的能力管理系统，提供：
//! - CapBitmap: 128 位能力位图，用于高效存储和查询能力集合
//! - KernelCapability: 能力枚举，定义所有支持的系统能力
//! - 桥接函数: 在位图和枚举之间进行转换

// ============================================================================
// CapBitmap: 128 位能力位图
// ============================================================================

/// CapBitmap: 128 位能力位图
///
/// 使用两个 u64 存储 128 个能力位，支持高效的位操作。
/// bits[0] 存储低 64 位（能力 0-63），bits[1] 存储高 64 位（能力 64-127）。
#[derive(Debug, Clone, Default)]
pub struct CapBitmap {
    bits: [u64; 2],
}

impl CapBitmap {
    /// 创建一个全零的能力位图
    pub fn new() -> Self {
        Self { bits: [0, 0] }
    }

    /// 设置指定位置的能力位
    ///
    /// # 参数
    /// - `bit`: 能力位索引（0-127）
    pub fn set(&mut self, bit: u32) {
        if bit < 128 {
            let word = (bit / 64) as usize;
            let offset = bit % 64;
            self.bits[word] |= 1 << offset;
        }
    }

    /// 清除指定位置的能力位
    ///
    /// # 参数
    /// - `bit`: 能力位索引（0-127）
    pub fn clear(&mut self, bit: u32) {
        if bit < 128 {
            let word = (bit / 64) as usize;
            let offset = bit % 64;
            self.bits[word] &= !(1 << offset);
        }
    }

    /// 检查指定位置的能力位是否已设置
    ///
    /// # 参数
    /// - `bit`: 能力位索引（0-127）
    ///
    /// # 返回
    /// 如果该位已设置返回 true，否则返回 false
    pub fn test(&self, bit: u32) -> bool {
        if bit < 128 {
            let word = (bit / 64) as usize;
            let offset = bit % 64;
            (self.bits[word] & (1 << offset)) != 0
        } else {
            false
        }
    }

    /// 检查位图是否为空（所有位均为 0）
    pub fn is_empty(&self) -> bool {
        self.bits[0] == 0 && self.bits[1] == 0
    }

    /// 计算位图中已设置位的数量
    pub fn count_ones(&self) -> u32 {
        self.bits[0].count_ones() + self.bits[1].count_ones()
    }
}

// ============================================================================
// KernelCapability: 能力枚举
// ============================================================================

/// 能力枚举（与 omniagent-security 的 Capability 对应）
///
/// 定义内核支持的所有系统能力，每个能力对应一个唯一的位索引。
/// 使用 u8 表示，最大支持 255 种能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KernelCapability {
    /// 读取文件
    ReadFiles = 0,
    /// 写入文件
    WriteFiles = 1,
    /// 执行程序
    Execute = 2,
    /// 网络访问
    NetworkAccess = 3,
    /// 创建 Agent
    CreateAgent = 4,
    /// 终止 Agent
    KillAgent = 5,
    /// 发送消息
    SendMessage = 6,
    /// 接收消息
    ReceiveMessage = 7,
    /// 订阅事件
    Subscribe = 8,
    /// 发布事件
    Publish = 9,
    /// 管理配额
    ManageQuota = 10,
    /// 管理系统
    ManageSystem = 11,
    /// 访问硬件
    AccessHardware = 12,
    /// 管理驱动
    ManageDrivers = 13,
    /// 管理内存
    ManageMemory = 14,
    /// 管理网络
    ManageNetwork = 15,
    /// 管理安全
    ManageSecurity = 16,
    /// 管理用户
    ManageUsers = 17,
    /// 管理策略
    ManagePolicies = 18,
    /// 审计访问
    AuditAccess = 19,
    /// 管理员访问
    AdminAccess = 20,
}

impl KernelCapability {
    /// 获取所有已知能力的列表
    ///
    /// 返回包含所有 KernelCapability 变体的向量
    pub fn all() -> alloc::vec::Vec<KernelCapability> {
        alloc::vec![
            KernelCapability::ReadFiles,
            KernelCapability::WriteFiles,
            KernelCapability::Execute,
            KernelCapability::NetworkAccess,
            KernelCapability::CreateAgent,
            KernelCapability::KillAgent,
            KernelCapability::SendMessage,
            KernelCapability::ReceiveMessage,
            KernelCapability::Subscribe,
            KernelCapability::Publish,
            KernelCapability::ManageQuota,
            KernelCapability::ManageSystem,
            KernelCapability::AccessHardware,
            KernelCapability::ManageDrivers,
            KernelCapability::ManageMemory,
            KernelCapability::ManageNetwork,
            KernelCapability::ManageSecurity,
            KernelCapability::ManageUsers,
            KernelCapability::ManagePolicies,
            KernelCapability::AuditAccess,
            KernelCapability::AdminAccess,
        ]
    }

    /// 从 u8 值创建 KernelCapability
    ///
    /// 如果值对应有效的能力变体，返回 Some，否则返回 None
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(KernelCapability::ReadFiles),
            1 => Some(KernelCapability::WriteFiles),
            2 => Some(KernelCapability::Execute),
            3 => Some(KernelCapability::NetworkAccess),
            4 => Some(KernelCapability::CreateAgent),
            5 => Some(KernelCapability::KillAgent),
            6 => Some(KernelCapability::SendMessage),
            7 => Some(KernelCapability::ReceiveMessage),
            8 => Some(KernelCapability::Subscribe),
            9 => Some(KernelCapability::Publish),
            10 => Some(KernelCapability::ManageQuota),
            11 => Some(KernelCapability::ManageSystem),
            12 => Some(KernelCapability::AccessHardware),
            13 => Some(KernelCapability::ManageDrivers),
            14 => Some(KernelCapability::ManageMemory),
            15 => Some(KernelCapability::ManageNetwork),
            16 => Some(KernelCapability::ManageSecurity),
            17 => Some(KernelCapability::ManageUsers),
            18 => Some(KernelCapability::ManagePolicies),
            19 => Some(KernelCapability::AuditAccess),
            20 => Some(KernelCapability::AdminAccess),
            _ => None,
        }
    }

    /// 获取能力对应的位索引
    ///
    /// 返回该能力在 CapBitmap 中的位位置
    pub fn bit_index(&self) -> u32 {
        *self as u32
    }
}

// ============================================================================
// 桥接函数
// ============================================================================

/// 检查位图中是否包含指定能力
///
/// # 参数
/// - `bitmap`: 能力位图
/// - `cap`: 要检查的能力
///
/// # 返回
/// 如果位图中该能力的位已设置，返回 true
pub fn has_capability(bitmap: &CapBitmap, cap: KernelCapability) -> bool {
    bitmap.test(cap.bit_index())
}

/// 授予指定能力
///
/// 在位图中设置对应的能力位。
///
/// # 参数
/// - `bitmap`: 能力位图
/// - `cap`: 要授予的能力
///
/// # 返回
/// 如果该能力之前未设置，返回 true（表示新授予）
pub fn grant_capability(bitmap: &mut CapBitmap, cap: KernelCapability) -> bool {
    let already_set = bitmap.test(cap.bit_index());
    bitmap.set(cap.bit_index());
    !already_set
}

/// 撤销指定能力
///
/// 在位图中清除对应的能力位。
///
/// # 参数
/// - `bitmap`: 能力位图
/// - `cap`: 要撤销的能力
///
/// # 返回
/// 如果该能力之前已设置，返回 true（表示成功撤销）
pub fn revoke_capability(bitmap: &mut CapBitmap, cap: KernelCapability) -> bool {
    let was_set = bitmap.test(cap.bit_index());
    bitmap.clear(cap.bit_index());
    was_set
}

/// 将能力枚举列表转换为能力位图
///
/// # 参数
/// - `caps`: 能力枚举切片
///
/// # 返回
/// 包含所有指定能力的位图
pub fn capabilities_to_bitmap(caps: &[KernelCapability]) -> CapBitmap {
    let mut bitmap = CapBitmap::new();
    for &cap in caps {
        bitmap.set(cap.bit_index());
    }
    bitmap
}

/// 将能力位图转换为能力枚举列表
///
/// # 参数
/// - `bitmap`: 能力位图
///
/// # 返回
/// 位图中所有已设置位对应的能力枚举列表
pub fn bitmap_to_capabilities(bitmap: &CapBitmap) -> alloc::vec::Vec<KernelCapability> {
    let mut caps = alloc::vec::Vec::new();
    for cap in KernelCapability::all() {
        if bitmap.test(cap.bit_index()) {
            caps.push(cap);
        }
    }
    caps
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === 测试: CapBitmap set/test/clear 操作 ===
    #[test]
    fn test_cap_bitmap_set_test_clear() {
        let mut bitmap = CapBitmap::new();
        assert!(!bitmap.test(0));
        assert!(!bitmap.test(64));

        bitmap.set(0);
        assert!(bitmap.test(0));
        assert!(!bitmap.test(1));

        bitmap.set(64);
        assert!(bitmap.test(64));
        assert!(!bitmap.test(65));

        bitmap.clear(0);
        assert!(!bitmap.test(0));
        assert!(bitmap.test(64));

        // 超出范围的操作不应 panic
        bitmap.set(128);
        assert!(!bitmap.test(128));
        bitmap.clear(200);
    }

    // === 测试: CapBitmap::new 创建空位图 ===
    #[test]
    fn test_cap_bitmap_new_empty() {
        let bitmap = CapBitmap::new();
        assert!(bitmap.is_empty());
        assert_eq!(bitmap.count_ones(), 0);
        assert!(!bitmap.test(0));
        assert!(!bitmap.test(127));
    }

    // === 测试: CapBitmap::count_ones ===
    #[test]
    fn test_cap_bitmap_count_ones() {
        let mut bitmap = CapBitmap::new();
        assert_eq!(bitmap.count_ones(), 0);

        bitmap.set(0);
        bitmap.set(1);
        bitmap.set(63);
        assert_eq!(bitmap.count_ones(), 3);

        bitmap.set(64);
        bitmap.set(127);
        assert_eq!(bitmap.count_ones(), 5);

        bitmap.clear(1);
        assert_eq!(bitmap.count_ones(), 4);
    }

    // === 测试: KernelCapability::bit_index ===
    #[test]
    fn test_capability_bit_index() {
        assert_eq!(KernelCapability::ReadFiles.bit_index(), 0);
        assert_eq!(KernelCapability::WriteFiles.bit_index(), 1);
        assert_eq!(KernelCapability::Execute.bit_index(), 2);
        assert_eq!(KernelCapability::NetworkAccess.bit_index(), 3);
        assert_eq!(KernelCapability::AdminAccess.bit_index(), 20);
    }

    // === 测试: KernelCapability::from_u8 ===
    #[test]
    fn test_capability_from_u8() {
        assert_eq!(KernelCapability::from_u8(0), Some(KernelCapability::ReadFiles));
        assert_eq!(KernelCapability::from_u8(20), Some(KernelCapability::AdminAccess));
        assert_eq!(KernelCapability::from_u8(21), None);
        assert_eq!(KernelCapability::from_u8(255), None);
    }

    // === 测试: KernelCapability::all ===
    #[test]
    fn test_capability_all() {
        let all = KernelCapability::all();
        assert_eq!(all.len(), 21);
        assert!(all.contains(&KernelCapability::ReadFiles));
        assert!(all.contains(&KernelCapability::AdminAccess));
    }

    // === 测试: has_capability ===
    #[test]
    fn test_has_capability() {
        let mut bitmap = CapBitmap::new();
        assert!(!has_capability(&bitmap, KernelCapability::ReadFiles));

        bitmap.set(KernelCapability::NetworkAccess.bit_index());
        assert!(has_capability(&bitmap, KernelCapability::NetworkAccess));
        assert!(!has_capability(&bitmap, KernelCapability::ReadFiles));
    }

    // === 测试: grant_capability / revoke_capability ===
    #[test]
    fn test_grant_revoke_capability() {
        let mut bitmap = CapBitmap::new();

        // 授予新能力应返回 true
        assert!(grant_capability(&mut bitmap, KernelCapability::ReadFiles));
        // 重复授予应返回 false
        assert!(!grant_capability(&mut bitmap, KernelCapability::ReadFiles));

        // 撤销已设置的能力应返回 true
        assert!(revoke_capability(&mut bitmap, KernelCapability::ReadFiles));
        // 撤销未设置的能力应返回 false
        assert!(!revoke_capability(&mut bitmap, KernelCapability::ReadFiles));
    }

    // === 测试: capabilities_to_bitmap 往返转换 ===
    #[test]
    fn test_capabilities_to_bitmap_roundtrip() {
        let caps = [
            KernelCapability::ReadFiles,
            KernelCapability::NetworkAccess,
            KernelCapability::AdminAccess,
        ];
        let bitmap = capabilities_to_bitmap(&caps);
        assert!(bitmap.test(KernelCapability::ReadFiles.bit_index()));
        assert!(bitmap.test(KernelCapability::NetworkAccess.bit_index()));
        assert!(bitmap.test(KernelCapability::AdminAccess.bit_index()));
        assert!(!bitmap.test(KernelCapability::WriteFiles.bit_index()));
        assert_eq!(bitmap.count_ones(), 3);
    }

    // === 测试: bitmap_to_capabilities ===
    #[test]
    fn test_bitmap_to_capabilities() {
        let mut bitmap = CapBitmap::new();
        bitmap.set(KernelCapability::Execute.bit_index());
        bitmap.set(KernelCapability::KillAgent.bit_index());

        let caps = bitmap_to_capabilities(&bitmap);
        assert_eq!(caps.len(), 2);
        assert!(caps.contains(&KernelCapability::Execute));
        assert!(caps.contains(&KernelCapability::KillAgent));

        // 空位图应返回空列表
        let empty_bitmap = CapBitmap::new();
        let empty_caps = bitmap_to_capabilities(&empty_bitmap);
        assert!(empty_caps.is_empty());
    }
}
