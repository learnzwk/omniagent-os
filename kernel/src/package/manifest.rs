//! 包清单定义
//!
//! 定义包标识、依赖关系、版本解析与匹配等功能。

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};

// ============================================================================
// 包标识
// ============================================================================

/// 包唯一标识符
#[derive(Debug, Clone)]
pub struct PackageId {
    /// 包名称
    pub name: String,
    /// 版本号 (语义化版本: major.minor.patch)
    pub version: String,
    /// 目标架构
    pub arch: String,
}

impl PartialEq for PackageId {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version && self.arch == other.arch
    }
}

impl Eq for PackageId {}

impl Hash for PackageId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.version.hash(state);
        self.arch.hash(state);
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{} ({})", self.name, self.version, self.arch)
    }
}

impl PackageId {
    /// 创建新的包标识符
    pub fn new(name: &str, version: &str, arch: &str) -> Self {
        PackageId {
            name: String::from(name),
            version: String::from(version),
            arch: String::from(arch),
        }
    }
}

// ============================================================================
// 依赖声明
// ============================================================================

/// 包依赖声明
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// 依赖的包名称
    pub name: String,
    /// 版本要求 (如 "^1.0.0", "~1.0.0", ">=1.0.0")
    pub version_req: String,
    /// 是否为可选依赖
    pub optional: bool,
}

impl Dependency {
    /// 创建新的依赖声明
    pub fn new(name: &str, version_req: &str) -> Self {
        Dependency {
            name: String::from(name),
            version_req: String::from(version_req),
            optional: false,
        }
    }

    /// 创建可选依赖
    pub fn optional(name: &str, version_req: &str) -> Self {
        Dependency {
            name: String::from(name),
            version_req: String::from(version_req),
            optional: true,
        }
    }
}

// ============================================================================
// 包清单
// ============================================================================

/// 包清单
#[derive(Debug, Clone)]
pub struct PackageManifest {
    /// 包标识符
    pub id: PackageId,
    /// 包描述
    pub description: String,
    /// 作者
    pub author: String,
    /// 许可证
    pub license: String,
    /// 依赖列表
    pub dependencies: Vec<Dependency>,
    /// 能力列表
    pub capabilities: Vec<String>,
    /// 支持的 Agent 类型
    pub agent_types: Vec<String>,
    /// 校验和
    pub checksum: String,
    /// 包大小（字节）
    pub size: u64,
}

impl PackageManifest {
    /// 创建新的包清单
    pub fn new(id: PackageId) -> Self {
        PackageManifest {
            id,
            description: String::new(),
            author: String::new(),
            license: String::new(),
            dependencies: Vec::new(),
            capabilities: Vec::new(),
            agent_types: Vec::new(),
            checksum: String::new(),
            size: 0,
        }
    }
}

// ============================================================================
// 版本解析与匹配
// ============================================================================

/// 解析后的语义化版本
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVersion {
    /// 主版本号
    pub major: u32,
    /// 次版本号
    pub minor: u32,
    /// 补丁版本号
    pub patch: u32,
}

impl SemVersion {
    /// 从字符串解析语义化版本
    ///
    /// 支持格式: "major.minor.patch"
    pub fn parse(version: &str) -> Option<SemVersion> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        let patch = parts[2].parse::<u32>().ok()?;
        Some(SemVersion {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for SemVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 版本匹配结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionMatchResult {
    /// 匹配成功
    Match,
    /// 不匹配
    NoMatch,
    /// 无法解析版本要求
    InvalidReq,
}

/// 检查版本是否满足版本要求
///
/// 支持的版本要求格式:
/// - "^1.0.0" - 兼容版本 (>=1.0.0, <2.0.0)
/// - "~1.0.0" - 近似版本 (>=1.0.0, <1.1.0)
/// - ">=1.0.0" - 大于等于
/// - "<=1.0.0" - 小于等于
/// - ">1.0.0" - 大于
/// - "<1.0.0" - 小于
/// - "=1.0.0" 或 "1.0.0" - 精确匹配
/// - ">=1.0.0, <2.0.0" - 范围匹配（逗号分隔）
pub fn version_matches(req: &str, ver: &str) -> VersionMatchResult {
    let version = match SemVersion::parse(ver) {
        Some(v) => v,
        None => return VersionMatchResult::InvalidReq,
    };

    // 支持逗号分隔的多重条件
    let conditions: Vec<&str> = req.split(',').map(|s| s.trim()).collect();

    for cond in &conditions {
        if !check_single_condition(cond, &version) {
            return VersionMatchResult::NoMatch;
        }
    }

    VersionMatchResult::Match
}

/// 检查单个版本条件
fn check_single_condition(cond: &str, version: &SemVersion) -> bool {
    let cond = cond.trim();

    if cond.starts_with('^') {
        // 兼容版本: ^1.0.0 => >=1.0.0, <2.0.0
        let req_ver = match SemVersion::parse(&cond[1..]) {
            Some(v) => v,
            None => return false,
        };
        version >= &req_ver && version.major == req_ver.major
    } else if cond.starts_with('~') {
        // 近似版本: ~1.0.0 => >=1.0.0, <1.1.0
        let req_ver = match SemVersion::parse(&cond[1..]) {
            Some(v) => v,
            None => return false,
        };
        version >= &req_ver && version.major == req_ver.major && version.minor == req_ver.minor
    } else if cond.starts_with(">=") {
        let req_ver = match SemVersion::parse(&cond[2..]) {
            Some(v) => v,
            None => return false,
        };
        version >= &req_ver
    } else if cond.starts_with("<=") {
        let req_ver = match SemVersion::parse(&cond[2..]) {
            Some(v) => v,
            None => return false,
        };
        version <= &req_ver
    } else if cond.starts_with('>') {
        let req_ver = match SemVersion::parse(&cond[1..]) {
            Some(v) => v,
            None => return false,
        };
        version > &req_ver
    } else if cond.starts_with('<') {
        let req_ver = match SemVersion::parse(&cond[1..]) {
            Some(v) => v,
            None => return false,
        };
        version < &req_ver
    } else if cond.starts_with('=') {
        let req_ver = match SemVersion::parse(&cond[1..]) {
            Some(v) => v,
            None => return false,
        };
        version == &req_ver
    } else {
        // 精确匹配
        let req_ver = match SemVersion::parse(cond) {
            Some(v) => v,
            None => return false,
        };
        version == &req_ver
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_id_new() {
        let id = PackageId::new("test-pkg", "1.0.0", "x86_64");
        assert_eq!(id.name, "test-pkg");
        assert_eq!(id.version, "1.0.0");
        assert_eq!(id.arch, "x86_64");
    }

    #[test]
    fn test_package_id_display() {
        let id = PackageId::new("my-pkg", "2.3.1", "aarch64");
        let display = format!("{}", id);
        assert_eq!(display, "my-pkg@2.3.1 (aarch64)");
    }

    #[test]
    fn test_package_id_equality() {
        let id1 = PackageId::new("pkg", "1.0.0", "x86_64");
        let id2 = PackageId::new("pkg", "1.0.0", "x86_64");
        let id3 = PackageId::new("pkg", "2.0.0", "x86_64");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_package_id_hash() {
        // 验证 PackageId 实现了 Hash trait（通过编译检查）
        // 使用 BTreeMap<PackageId, _> 需要 Ord，所以我们用另一种方式验证 Hash
        use core::hash::{Hash, Hasher};
        struct TestHasher(u64);
        impl Hasher for TestHasher {
            fn finish(&self) -> u64 { self.0 }
            fn write(&mut self, bytes: &[u8]) {
                for &b in bytes {
                    self.0 = self.0.wrapping_mul(31).wrapping_add(b as u64);
                }
            }
        }
        let id1 = PackageId::new("pkg", "1.0.0", "x86_64");
        let id2 = PackageId::new("pkg", "1.0.0", "x86_64");
        let mut h1 = TestHasher(0);
        let mut h2 = TestHasher(0);
        id1.hash(&mut h1);
        id2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_dependency_new() {
        let dep = Dependency::new("serde", "^1.0.0");
        assert_eq!(dep.name, "serde");
        assert_eq!(dep.version_req, "^1.0.0");
        assert!(!dep.optional);
    }

    #[test]
    fn test_dependency_optional() {
        let dep = Dependency::optional("tokio", "~1.0.0");
        assert!(dep.optional);
        assert_eq!(dep.name, "tokio");
    }

    #[test]
    fn test_manifest_new() {
        let id = PackageId::new("test", "1.0.0", "x86_64");
        let manifest = PackageManifest::new(id);
        assert_eq!(manifest.id.name, "test");
        assert!(manifest.dependencies.is_empty());
        assert_eq!(manifest.size, 0);
    }

    #[test]
    fn test_semver_parse_valid() {
        let v = SemVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_semver_parse_invalid() {
        assert!(SemVersion::parse("1.2").is_none());
        assert!(SemVersion::parse("abc").is_none());
        assert!(SemVersion::parse("").is_none());
        assert!(SemVersion::parse("1.2.3.4").is_none());
    }

    #[test]
    fn test_semver_display() {
        let v = SemVersion::parse("4.5.6").unwrap();
        assert_eq!(format!("{}", v), "4.5.6");
    }

    #[test]
    fn test_semver_ordering() {
        let v1 = SemVersion::parse("1.0.0").unwrap();
        let v2 = SemVersion::parse("2.0.0").unwrap();
        let v3 = SemVersion::parse("1.5.0").unwrap();
        assert!(v1 < v2);
        assert!(v1 < v3);
        assert!(v3 < v2);
    }

    #[test]
    fn test_version_matches_caret() {
        // ^1.0.0 => >=1.0.0, <2.0.0
        assert_eq!(version_matches("^1.0.0", "1.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("^1.0.0", "1.5.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("^1.0.0", "1.9.9"), VersionMatchResult::Match);
        assert_eq!(version_matches("^1.0.0", "2.0.0"), VersionMatchResult::NoMatch);
        assert_eq!(version_matches("^1.0.0", "0.9.0"), VersionMatchResult::NoMatch);
    }

    #[test]
    fn test_version_matches_tilde() {
        // ~1.0.0 => >=1.0.0, <1.1.0
        assert_eq!(version_matches("~1.0.0", "1.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("~1.0.0", "1.0.5"), VersionMatchResult::Match);
        assert_eq!(version_matches("~1.0.0", "1.1.0"), VersionMatchResult::NoMatch);
        assert_eq!(version_matches("~1.0.0", "0.9.0"), VersionMatchResult::NoMatch);
    }

    #[test]
    fn test_version_matches_gte() {
        assert_eq!(version_matches(">=1.0.0", "1.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches(">=1.0.0", "2.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches(">=1.0.0", "0.9.0"), VersionMatchResult::NoMatch);
    }

    #[test]
    fn test_version_matches_lte() {
        assert_eq!(version_matches("<=1.0.0", "1.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("<=1.0.0", "0.9.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("<=1.0.0", "1.0.1"), VersionMatchResult::NoMatch);
    }

    #[test]
    fn test_version_matches_gt_lt() {
        assert_eq!(version_matches(">1.0.0", "1.0.1"), VersionMatchResult::Match);
        assert_eq!(version_matches(">1.0.0", "1.0.0"), VersionMatchResult::NoMatch);
        assert_eq!(version_matches("<2.0.0", "1.9.9"), VersionMatchResult::Match);
        assert_eq!(version_matches("<2.0.0", "2.0.0"), VersionMatchResult::NoMatch);
    }

    #[test]
    fn test_version_matches_exact() {
        assert_eq!(version_matches("1.0.0", "1.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("1.0.0", "1.0.1"), VersionMatchResult::NoMatch);
        assert_eq!(version_matches("=1.0.0", "1.0.0"), VersionMatchResult::Match);
        assert_eq!(version_matches("=1.0.0", "2.0.0"), VersionMatchResult::NoMatch);
    }

    #[test]
    fn test_version_matches_range() {
        // >=1.0.0, <2.0.0
        assert_eq!(
            version_matches(">=1.0.0, <2.0.0", "1.5.0"),
            VersionMatchResult::Match
        );
        assert_eq!(
            version_matches(">=1.0.0, <2.0.0", "2.0.0"),
            VersionMatchResult::NoMatch
        );
        assert_eq!(
            version_matches(">=1.0.0, <2.0.0", "0.9.0"),
            VersionMatchResult::NoMatch
        );
    }

    #[test]
    fn test_version_matches_invalid() {
        assert_eq!(version_matches("^abc", "1.0.0"), VersionMatchResult::NoMatch);
        assert_eq!(version_matches("^1.0", "1.0.0"), VersionMatchResult::NoMatch);
    }
}
