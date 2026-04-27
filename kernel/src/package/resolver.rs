//! 依赖解析器
//!
//! 实现包依赖的拓扑排序解析、循环依赖检测和版本冲突检测。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::package::error::PackageError;
use crate::package::manifest::{Dependency, PackageManifest, SemVersion, VersionMatchResult, version_matches};

// ============================================================================
// 解析结果
// ============================================================================

/// 依赖解析结果
#[derive(Debug, Clone)]
pub struct ResolveResult {
    /// 安装顺序列表（拓扑排序后的包名称）
    pub install_order: Vec<String>,
    /// 已解析的包版本映射
    pub resolved_versions: BTreeMap<String, String>,
}

// ============================================================================
// 依赖解析器
// ============================================================================

/// 依赖解析器
///
/// 根据已注册的包清单和依赖关系，计算正确的安装顺序。
pub struct DependencyResolver;

impl DependencyResolver {
    /// 解析依赖并返回安装顺序
    ///
    /// 使用 Kahn 算法进行拓扑排序，同时检测循环依赖。
    /// `available` 提供所有可用包的清单映射（包名 -> 清单）。
    /// `root_packages` 是需要安装的根包名称列表。
    pub fn resolve(
        available: &BTreeMap<String, PackageManifest>,
        root_packages: &[String],
    ) -> Result<ResolveResult, PackageError> {
        // 收集所有需要安装的包及其依赖
        let mut needed: BTreeMap<String, String> = BTreeMap::new(); // name -> version

        // BFS 收集所有依赖
        let mut queue: Vec<String> = root_packages.to_vec();
        let mut visited: Vec<String> = Vec::new();

        while !queue.is_empty() {
            let name = queue.remove(0);

            if visited.contains(&name) {
                continue;
            }
            visited.push(name.clone());

            // 查找包
            let manifest = available
                .get(&name)
                .ok_or_else(|| PackageError::DependencyNotFound(name.clone()))?;

            needed.insert(name.clone(), manifest.id.version.clone());

            // 添加依赖到队列
            for dep in &manifest.dependencies {
                if !dep.optional && !visited.contains(&dep.name) {
                    // 检查版本兼容性
                    if let Some(dep_manifest) = available.get(&dep.name) {
                        let match_result = version_matches(
                            &dep.version_req,
                            &dep_manifest.id.version,
                        );
                        if match_result == VersionMatchResult::NoMatch {
                            return Err(PackageError::VersionConflict {
                                package: name.clone(),
                                required: dep.version_req.clone(),
                                found: dep_manifest.id.version.clone(),
                            });
                        }
                    }
                    queue.push(dep.name.clone());
                }
            }
        }

        // 构建依赖图（邻接表）
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();

        for name in needed.keys() {
            graph.insert(name.clone(), Vec::new());
            in_degree.insert(name.clone(), 0);
        }

        for name in needed.keys() {
            if let Some(manifest) = available.get(name) {
                for dep in &manifest.dependencies {
                    if !dep.optional && needed.contains_key(&dep.name) {
                        graph.get_mut(&dep.name).unwrap().push(name.clone());
                        *in_degree.get_mut(name).unwrap() += 1;
                    }
                }
            }
        }

        // Kahn 算法拓扑排序
        let mut install_order: Vec<String> = Vec::new();
        let mut zero_degree: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        // 排序以确保确定性
        zero_degree.sort();

        while !zero_degree.is_empty() {
            let node = zero_degree.remove(0);
            install_order.push(node.clone());

            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            zero_degree.push(neighbor.clone());
                            zero_degree.sort();
                        }
                    }
                }
            }
        }

        // 检测循环依赖
        if install_order.len() != needed.len() {
            // 找出参与循环的包
            let mut cycle_packages: Vec<String> = needed
                .keys()
                .filter(|name| !install_order.contains(name))
                .cloned()
                .collect();
            cycle_packages.sort();
            let cycle_str = cycle_packages.join(" -> ");
            return Err(PackageError::CircularDependency(cycle_str));
        }

        Ok(ResolveResult {
            install_order,
            resolved_versions: needed,
        })
    }

    /// 检测循环依赖
    ///
    /// 使用 DFS 检测给定依赖图中是否存在循环。
    pub fn detect_cycles(
        available: &BTreeMap<String, PackageManifest>,
        package_name: &str,
    ) -> Result<(), PackageError> {
        let mut visited: Vec<String> = Vec::new();
        let mut stack: Vec<String> = Vec::new();
        let mut _has_cycle = false;

        Self::dfs_cycle(
            available,
            package_name,
            &mut visited,
            &mut stack,
            &mut _has_cycle,
        )
    }

    /// DFS 循环检测辅助函数
    fn dfs_cycle(
        available: &BTreeMap<String, PackageManifest>,
        current: &str,
        visited: &mut Vec<String>,
        stack: &mut Vec<String>,
        has_cycle: &mut bool,
    ) -> Result<(), PackageError> {
        visited.push(current.to_string());
        stack.push(current.to_string());

        if let Some(manifest) = available.get(current) {
            for dep in &manifest.dependencies {
                if !dep.optional {
                    if !visited.contains(&dep.name) {
                        Self::dfs_cycle(available, &dep.name, visited, stack, has_cycle)?;
                    } else if stack.contains(&dep.name) {
                        *has_cycle = true;
                        // 构建循环路径
                        let cycle_start = stack.iter().position(|x| x == &dep.name).unwrap();
                        let mut cycle_path: Vec<String> =
                            stack[cycle_start..].to_vec();
                        cycle_path.push(dep.name.clone());
                        return Err(PackageError::CircularDependency(cycle_path.join(" -> ")));
                    }
                }
            }
        }

        stack.pop();
        Ok(())
    }

    /// 检查版本冲突
    ///
    /// 检查给定包集合中是否存在版本冲突。
    pub fn check_version_conflicts(
        available: &BTreeMap<String, PackageManifest>,
        packages: &[String],
    ) -> Result<(), PackageError> {
        // 收集每个包被要求的版本约束
        let mut requirements: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        // (requirer, version_req)

        for pkg_name in packages {
            if let Some(manifest) = available.get(pkg_name) {
                for dep in &manifest.dependencies {
                    if !dep.optional {
                        requirements
                            .entry(dep.name.clone())
                            .or_insert_with(|| Vec::new())
                            .push((pkg_name.clone(), dep.version_req.clone()));
                    }
                }
            }
        }

        // 检查每个被依赖的包是否满足所有要求
        for (dep_name, reqs) in &requirements {
            if let Some(dep_manifest) = available.get(dep_name) {
                for (requirer, req) in reqs {
                    let result = version_matches(req, &dep_manifest.id.version);
                    if result == VersionMatchResult::NoMatch {
                        return Err(PackageError::VersionConflict {
                            package: requirer.clone(),
                            required: req.clone(),
                            found: dep_manifest.id.version.clone(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageId;

    /// 创建简单的包清单
    fn simple_manifest(name: &str, version: &str, deps: &[(&str, &str)]) -> PackageManifest {
        let id = PackageId::new(name, version, "x86_64");
        let dependencies = deps
            .iter()
            .map(|(n, v)| Dependency::new(n, v))
            .collect();
        PackageManifest {
            id,
            description: String::new(),
            author: String::new(),
            license: String::new(),
            dependencies,
            capabilities: Vec::new(),
            agent_types: Vec::new(),
            checksum: String::new(),
            size: 0,
        }
    }

    #[test]
    fn test_resolve_simple() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.install_order, vec!["A"]);
    }

    #[test]
    fn test_resolve_with_dependencies() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        // B 应该在 A 之前安装
        assert_eq!(res.install_order, vec!["B", "A"]);
    }

    #[test]
    fn test_resolve_chain() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[("C", "^1.0.0")]),
        );
        available.insert(
            String::from("C"),
            simple_manifest("C", "1.0.0", &[]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.install_order, vec!["C", "B", "A"]);
    }

    #[test]
    fn test_resolve_diamond_dependency() {
        // A -> B, A -> C, B -> D, C -> D
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0"), ("C", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[("D", "^1.0.0")]),
        );
        available.insert(
            String::from("C"),
            simple_manifest("C", "1.0.0", &[("D", "^1.0.0")]),
        );
        available.insert(
            String::from("D"),
            simple_manifest("D", "1.0.0", &[]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        // D 应该在 B 和 C 之前
        let d_pos = res.install_order.iter().position(|x| x == "D").unwrap();
        let b_pos = res.install_order.iter().position(|x| x == "B").unwrap();
        let c_pos = res.install_order.iter().position(|x| x == "C").unwrap();
        let a_pos = res.install_order.iter().position(|x| x == "A").unwrap();
        assert!(d_pos < b_pos);
        assert!(d_pos < c_pos);
        assert!(b_pos < a_pos);
        assert!(c_pos < a_pos);
    }

    #[test]
    fn test_resolve_circular_dependency() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[("A", "^1.0.0")]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PackageError::CircularDependency(msg) => {
                assert!(msg.contains("A") || msg.contains("B"));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn test_resolve_missing_dependency() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("missing", "^1.0.0")]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PackageError::DependencyNotFound(name) => {
                assert_eq!(name, "missing");
            }
            _ => panic!("Expected DependencyNotFound error"),
        }
    }

    #[test]
    fn test_resolve_version_conflict() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^2.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PackageError::VersionConflict { package, required, found } => {
                assert_eq!(package, "A");
                assert_eq!(required, "^2.0.0");
                assert_eq!(found, "1.0.0");
            }
            _ => panic!("Expected VersionConflict error"),
        }
    }

    #[test]
    fn test_detect_cycles_no_cycle() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[]),
        );

        assert!(DependencyResolver::detect_cycles(&available, "A").is_ok());
    }

    #[test]
    fn test_detect_cycles_with_cycle() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[("C", "^1.0.0")]),
        );
        available.insert(
            String::from("C"),
            simple_manifest("C", "1.0.0", &[("A", "^1.0.0")]),
        );

        assert!(DependencyResolver::detect_cycles(&available, "A").is_err());
    }

    #[test]
    fn test_check_version_conflicts_ok() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.5.0", &[]),
        );

        assert!(DependencyResolver::check_version_conflicts(
            &available,
            &[String::from("A")],
        ).is_ok());
    }

    #[test]
    fn test_check_version_conflicts_fail() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[("B", "^2.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[]),
        );

        assert!(DependencyResolver::check_version_conflicts(
            &available,
            &[String::from("A")],
        ).is_err());
    }

    #[test]
    fn test_resolve_multiple_roots() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "1.0.0", &[]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.0.0", &[]),
        );
        available.insert(
            String::from("C"),
            simple_manifest("C", "1.0.0", &[("A", "^1.0.0")]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("B"), String::from("C")],
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.install_order.len(), 3);
        // A 应该在 C 之前
        let a_pos = res.install_order.iter().position(|x| x == "A").unwrap();
        let c_pos = res.install_order.iter().position(|x| x == "C").unwrap();
        assert!(a_pos < c_pos);
    }

    #[test]
    fn test_resolve_optional_deps_ignored() {
        use crate::package::manifest::Dependency;

        let _id_a = PackageId::new("A", "1.0.0", "x86_64");
        let mut manifest_a = simple_manifest("A", "1.0.0", &[]);
        manifest_a.dependencies.push(Dependency::optional("missing-opt", "^1.0.0"));

        let mut available = BTreeMap::new();
        available.insert(String::from("A"), manifest_a);

        // 可选依赖缺失不应导致错误
        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_empty_roots() {
        let available = BTreeMap::new();
        let result = DependencyResolver::resolve(&available, &[]);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.install_order.is_empty());
    }

    #[test]
    fn test_resolved_versions() {
        let mut available = BTreeMap::new();
        available.insert(
            String::from("A"),
            simple_manifest("A", "2.0.0", &[("B", "^1.0.0")]),
        );
        available.insert(
            String::from("B"),
            simple_manifest("B", "1.5.0", &[]),
        );

        let result = DependencyResolver::resolve(
            &available,
            &[String::from("A")],
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.resolved_versions.get("A").unwrap(), "2.0.0");
        assert_eq!(res.resolved_versions.get("B").unwrap(), "1.5.0");
    }
}
