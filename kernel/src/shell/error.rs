//! Shell 错误类型
//!
//! 定义 Shell 执行过程中所有可能的错误类型。

use core::fmt;

/// Shell 错误枚举
#[derive(Debug, Clone)]
pub enum ShellError {
    /// 命令未找到
    CommandNotFound(alloc::string::String),
    /// 参数不足
    InsufficientArguments {
        command: alloc::string::String,
        expected: usize,
        found: usize,
    },
    /// 无效参数
    InvalidArgument {
        command: alloc::string::String,
        argument: alloc::string::String,
        reason: alloc::string::String,
    },
    /// 语法错误
    SyntaxError(alloc::string::String),
    /// 执行失败
    ExecutionFailed(alloc::string::String),
    /// 管道错误
    PipeError(alloc::string::String),
    /// 重定向错误
    RedirectError(alloc::string::String),
    /// 环境变量未找到
    EnvVarNotFound(alloc::string::String),
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::CommandNotFound(cmd) => write!(f, "命令未找到: {}", cmd),
            ShellError::InsufficientArguments {
                command,
                expected,
                found,
            } => write!(
                f,
                "参数不足: '{}' 需要 {} 个参数, 提供 {} 个",
                command, expected, found
            ),
            ShellError::InvalidArgument {
                command,
                argument,
                reason,
            } => write!(f, "无效参数: '{}' 的 '{}' - {}", command, argument, reason),
            ShellError::SyntaxError(msg) => write!(f, "语法错误: {}", msg),
            ShellError::ExecutionFailed(msg) => write!(f, "执行失败: {}", msg),
            ShellError::PipeError(msg) => write!(f, "管道错误: {}", msg),
            ShellError::RedirectError(msg) => write!(f, "重定向错误: {}", msg),
            ShellError::EnvVarNotFound(name) => write!(f, "环境变量未找到: {}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_not_found_display() {
        let err = ShellError::CommandNotFound(alloc::string::String::from("foobar"));
        assert_eq!(format!("{}", err), "命令未找到: foobar");
    }

    #[test]
    fn test_insufficient_arguments_display() {
        let err = ShellError::InsufficientArguments {
            command: alloc::string::String::from("cp"),
            expected: 2,
            found: 1,
        };
        assert_eq!(
            format!("{}", err),
            "参数不足: 'cp' 需要 2 个参数, 提供 1 个"
        );
    }

    #[test]
    fn test_invalid_argument_display() {
        let err = ShellError::InvalidArgument {
            command: alloc::string::String::from("ls"),
            argument: alloc::string::String::from("-z"),
            reason: alloc::string::String::from("未知选项"),
        };
        assert_eq!(
            format!("{}", err),
            "无效参数: 'ls' 的 '-z' - 未知选项"
        );
    }

    #[test]
    fn test_syntax_error_display() {
        let err = ShellError::SyntaxError(alloc::string::String::from("未闭合的引号"));
        assert_eq!(format!("{}", err), "语法错误: 未闭合的引号");
    }

    #[test]
    fn test_execution_failed_display() {
        let err = ShellError::ExecutionFailed(alloc::string::String::from("权限被拒绝"));
        assert_eq!(format!("{}", err), "执行失败: 权限被拒绝");
    }

    #[test]
    fn test_pipe_error_display() {
        let err = ShellError::PipeError(alloc::string::String::from("管道破裂"));
        assert_eq!(format!("{}", err), "管道错误: 管道破裂");
    }

    #[test]
    fn test_redirect_error_display() {
        let err = ShellError::RedirectError(alloc::string::String::from("无法打开文件"));
        assert_eq!(format!("{}", err), "重定向错误: 无法打开文件");
    }

    #[test]
    fn test_env_var_not_found_display() {
        let err = ShellError::EnvVarNotFound(alloc::string::String::from("HOME"));
        assert_eq!(format!("{}", err), "环境变量未找到: HOME");
    }

    #[test]
    fn test_error_clone() {
        let err = ShellError::CommandNotFound(alloc::string::String::from("clone-test"));
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));
    }

    #[test]
    fn test_error_debug() {
        let err = ShellError::SyntaxError(alloc::string::String::from("test"));
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("SyntaxError"));
    }
}
