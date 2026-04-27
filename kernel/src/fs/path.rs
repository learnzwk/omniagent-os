//! 路径解析工具
//!
//! 提供路径规范化、分割、连接等操作。
//! 兼容 no_std 环境，使用 alloc 进行堆分配。

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// 路径解析工具结构体
///
/// 提供静态方法进行路径操作，不持有任何状态。
pub struct Path;

impl Path {
    /// 规范化路径（处理 . 和 ..）
    ///
    /// 将路径中的 `.` 和 `..` 组件解析为规范形式。
    /// 移除多余的 `/`，处理连续的分隔符。
    ///
    /// # 示例
    /// ```
    /// assert_eq!(Path::normalize("/a/b/../c"), "/a/c");
    /// assert_eq!(Path::normalize("/a/./b"), "/a/b");
    /// ```
    pub fn normalize(path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }

        let is_absolute = path.starts_with('/');
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let mut stack: Vec<&str> = Vec::new();

        for component in &components {
            match *component {
                "." => {
                    // 当前目录，跳过
                }
                ".." => {
                    // 上级目录，弹出栈顶
                    if !stack.is_empty() {
                        stack.pop();
                    }
                }
                _ => {
                    stack.push(component);
                }
            }
        }

        let mut result = String::new();
        if is_absolute {
            result.push('/');
        }
        for (i, component) in stack.iter().enumerate() {
            if i > 0 {
                result.push('/');
            }
            result.push_str(component);
        }

        // 如果原始路径以 / 结尾且结果非空，也加上 /
        if path.ends_with('/') && !result.ends_with('/') && !result.is_empty() {
            result.push('/');
        }

        result
    }

    /// 分割路径为组件
    ///
    /// 将路径按 `/` 分割为各个组件，过滤空字符串。
    ///
    /// # 示例
    /// ```
    /// let comps = Path::components("/a/b/c");
    /// assert_eq!(comps, vec!["a", "b", "c"]);
    /// ```
    pub fn components(path: &str) -> Vec<&str> {
        path.split('/')
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// 获取父路径
    ///
    /// 返回路径的父目录部分。如果路径没有父目录，返回 None。
    ///
    /// # 示例
    /// ```
    /// assert_eq!(Path::parent("/a/b/c"), Some("/a/b"));
    /// assert_eq!(Path::parent("/a"), Some("/"));
    /// assert_eq!(Path::parent("/"), None);
    /// ```
    pub fn parent(path: &str) -> Option<&str> {
        if path.is_empty() || path == "/" {
            return None;
        }

        // 去掉末尾的 /
        let trimmed = path.trim_end_matches('/');

        // 找到最后一个 /
        match trimmed.rfind('/') {
            None => None,
            Some(0) => Some("/"),
            Some(pos) => Some(&trimmed[..pos]),
        }
    }

    /// 获取文件名
    ///
    /// 返回路径中最后一级的文件名或目录名。
    ///
    /// # 示例
    /// ```
    /// assert_eq!(Path::filename("/a/b/c.txt"), Some("c.txt"));
    /// assert_eq!(Path::filename("/a/"), Some("a"));
    /// assert_eq!(Path::filename("/"), None);
    /// ```
    pub fn filename(path: &str) -> Option<&str> {
        if path.is_empty() || path == "/" {
            return None;
        }

        // 去掉末尾的 /
        let trimmed = path.trim_end_matches('/');

        // 找到最后一个 /
        match trimmed.rfind('/') {
            None => Some(trimmed),
            Some(pos) => {
                let name = &trimmed[pos + 1..];
                if name.is_empty() {
                    None
                } else {
                    Some(name)
                }
            }
        }
    }

    /// 检查路径是否是绝对路径
    ///
    /// 绝对路径以 `/` 开头。
    pub fn is_absolute(path: &str) -> bool {
        path.starts_with('/')
    }

    /// 连接两个路径
    ///
    /// 将 `relative` 路径连接到 `base` 路径后面。
    /// 如果 `relative` 是绝对路径，则直接返回 `relative`。
    ///
    /// # 示例
    /// ```
    /// assert_eq!(Path::join("/a/b", "c"), "/a/b/c");
    /// assert_eq!(Path::join("/a/b", "/c"), "/c");
    /// ```
    pub fn join(base: &str, relative: &str) -> String {
        if relative.is_empty() {
            return base.to_string();
        }

        if Path::is_absolute(relative) {
            return relative.to_string();
        }

        if base.is_empty() {
            return relative.to_string();
        }

        let base_trimmed = base.trim_end_matches('/');
        let mut result = String::from(base_trimmed);
        result.push('/');
        result.push_str(relative);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_normalize() {
        // 基本规范化
        assert_eq!(Path::normalize("/a/b/c"), "/a/b/c");
        assert_eq!(Path::normalize("a/b/c"), "a/b/c");

        // 处理 ..
        assert_eq!(Path::normalize("/a/b/../c"), "/a/c");
        assert_eq!(Path::normalize("/a/b/c/../../d"), "/a/d");
        assert_eq!(Path::normalize("/a/../b/../c"), "/c");

        // 处理 .
        assert_eq!(Path::normalize("/a/./b"), "/a/b");
        assert_eq!(Path::normalize("/a/./b/./c"), "/a/b/c");

        // 处理连续的 /
        assert_eq!(Path::normalize("/a//b"), "/a/b");
        assert_eq!(Path::normalize("/a///b/c"), "/a/b/c");

        // 根目录
        assert_eq!(Path::normalize("/"), "/");
        assert_eq!(Path::normalize("/.."), "/");

        // 空路径
        assert_eq!(Path::normalize(""), "");

        // 混合情况
        assert_eq!(Path::normalize("/a/b/./c/../d"), "/a/b/d");
        assert_eq!(Path::normalize("../a"), "a");
    }

    #[test]
    fn test_path_components() {
        assert_eq!(Path::components("/a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(Path::components("a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(Path::components("/a//b"), vec!["a", "b"]);
        assert_eq!(Path::components("/"), Vec::<&str>::new());
        assert_eq!(Path::components(""), Vec::<&str>::new());
        assert_eq!(Path::components("a"), vec!["a"]);
    }

    #[test]
    fn test_path_parent() {
        assert_eq!(Path::parent("/a/b/c"), Some("/a/b"));
        assert_eq!(Path::parent("/a/b"), Some("/a"));
        assert_eq!(Path::parent("/a"), Some("/"));
        assert_eq!(Path::parent("/"), None);
        assert_eq!(Path::parent(""), None);
        assert_eq!(Path::parent("a/b"), Some("a"));
        assert_eq!(Path::parent("a"), None);
        assert_eq!(Path::parent("/a/"), Some("/"));
    }

    #[test]
    fn test_path_filename() {
        assert_eq!(Path::filename("/a/b/c.txt"), Some("c.txt"));
        assert_eq!(Path::filename("/a/b"), Some("b"));
        assert_eq!(Path::filename("/a/"), Some("a"));
        assert_eq!(Path::filename("/"), None);
        assert_eq!(Path::filename(""), None);
        assert_eq!(Path::filename("a.txt"), Some("a.txt"));
        assert_eq!(Path::filename("a/b/c"), Some("c"));
    }

    #[test]
    fn test_path_is_absolute() {
        assert!(Path::is_absolute("/"));
        assert!(Path::is_absolute("/a/b"));
        assert!(Path::is_absolute("/a"));
        assert!(!Path::is_absolute("a/b"));
        assert!(!Path::is_absolute("a"));
        assert!(!Path::is_absolute(""));
    }

    #[test]
    fn test_path_join() {
        assert_eq!(Path::join("/a/b", "c"), "/a/b/c");
        assert_eq!(Path::join("/a/b", "c/d"), "/a/b/c/d");
        assert_eq!(Path::join("/a/b", "/c"), "/c");
        assert_eq!(Path::join("/a", "b"), "/a/b");
        assert_eq!(Path::join("/", "a"), "/a");
        assert_eq!(Path::join("/a/", "b"), "/a/b");
        assert_eq!(Path::join("", "a"), "a");
        assert_eq!(Path::join("/a/b", ""), "/a/b");
    }
}
