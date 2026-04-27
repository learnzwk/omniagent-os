//! 内置命令
//!
//! 定义 Shell 内置命令的 trait 和具体实现。

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::shell::command::Command;
use crate::shell::error::ShellError;

// ============================================================================
// 内置命令 trait
// ============================================================================

/// 内置命令执行结果
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// 输出内容
    pub output: String,
    /// 是否成功
    pub success: bool,
    /// 退出码
    pub exit_code: i32,
}

impl CommandResult {
    /// 创建成功结果
    pub fn ok(output: &str) -> Self {
        CommandResult {
            output: String::from(output),
            success: true,
            exit_code: 0,
        }
    }

    /// 创建失败结果
    pub fn err(output: &str) -> Self {
        CommandResult {
            output: String::from(output),
            success: false,
            exit_code: 1,
        }
    }
}

/// 内置命令 trait
pub trait BuiltinCommand: Send + Sync {
    /// 返回命令名称
    fn name(&self) -> &str;

    /// 返回命令描述
    fn description(&self) -> &str;

    /// 执行命令
    fn execute(&self, cmd: &Command) -> Result<CommandResult, ShellError>;
}

// ============================================================================
// help 命令
// ============================================================================

/// help 内置命令
pub struct HelpCommand;

impl BuiltinCommand for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "显示帮助信息"
    }

    fn execute(&self, _cmd: &Command) -> Result<CommandResult, ShellError> {
        let output = String::from(
            "OmniAgent OS Shell - 可用命令:\n\
             help    - 显示帮助信息\n\
             version - 显示内核版本\n\
             ps      - 列出进程/任务\n\
             ls      - 列出文件/目录\n\
             cat     - 显示文件内容\n\
             echo    - 输出文本\n\
             clear   - 清屏\n\
             info    - 显示系统信息",
        );
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// version 命令
// ============================================================================

/// version 内置命令
pub struct VersionCommand;

impl BuiltinCommand for VersionCommand {
    fn name(&self) -> &str {
        "version"
    }

    fn description(&self) -> &str {
        "显示内核版本"
    }

    fn execute(&self, _cmd: &Command) -> Result<CommandResult, ShellError> {
        let output = format!("OmniAgent OS v{}", crate::KERNEL_VERSION);
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// ps 命令
// ============================================================================

/// ps 内置命令
pub struct PsCommand;

impl BuiltinCommand for PsCommand {
    fn name(&self) -> &str {
        "ps"
    }

    fn description(&self) -> &str {
        "列出进程/任务"
    }

    fn execute(&self, _cmd: &Command) -> Result<CommandResult, ShellError> {
        let output = String::from(
            "PID    STATE    NAME\n\
             0      Running  kernel\n\
             1      Running  shell\n\
             2      Sleeping idle",
        );
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// ls 命令
// ============================================================================

/// ls 内置命令
pub struct LsCommand;

impl BuiltinCommand for LsCommand {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        "列出文件/目录"
    }

    fn execute(&self, cmd: &Command) -> Result<CommandResult, ShellError> {
        let path = if cmd.args.is_empty() {
            "/"
        } else {
            &cmd.args[0]
        };

        let output = format!(
            "drwxr-xr-x  2 root root 4096 {} .\n\
             drwxr-xr-x  2 root root 4096 {} ..\n\
             -rw-r--r--  1 root root  512 {} config.toml\n\
             -rw-r--r--  1 root root  256 {} readme.txt",
            path, path, path, path
        );
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// cat 命令
// ============================================================================

/// cat 内置命令
pub struct CatCommand;

impl BuiltinCommand for CatCommand {
    fn name(&self) -> &str {
        "cat"
    }

    fn description(&self) -> &str {
        "显示文件内容"
    }

    fn execute(&self, cmd: &Command) -> Result<CommandResult, ShellError> {
        if cmd.args.is_empty() {
            return Err(ShellError::InsufficientArguments {
                command: String::from("cat"),
                expected: 1,
                found: 0,
            });
        }

        let filename = &cmd.args[0];
        let output = format!("{}: 文件内容 (模拟)", filename);
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// echo 命令
// ============================================================================

/// echo 内置命令
pub struct EchoCommand;

impl BuiltinCommand for EchoCommand {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "输出文本"
    }

    fn execute(&self, cmd: &Command) -> Result<CommandResult, ShellError> {
        let output = cmd.args.join(" ");
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// clear 命令
// ============================================================================

/// clear 内置命令
pub struct ClearCommand;

impl BuiltinCommand for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "清屏"
    }

    fn execute(&self, _cmd: &Command) -> Result<CommandResult, ShellError> {
        Ok(CommandResult::ok("\x1b[2J\x1b[H"))
    }
}

// ============================================================================
// info 命令
// ============================================================================

/// info 内置命令
pub struct InfoCommand;

impl BuiltinCommand for InfoCommand {
    fn name(&self) -> &str {
        "info"
    }

    fn description(&self) -> &str {
        "显示系统信息"
    }

    fn execute(&self, _cmd: &Command) -> Result<CommandResult, ShellError> {
        let output = format!(
            "系统信息:\n\
             内核: {} v{}\n\
             架构: x86_64\n\
             状态: 运行中",
            crate::KERNEL_NAME,
            crate::KERNEL_VERSION
        );
        Ok(CommandResult::ok(&output))
    }
}

// ============================================================================
// 内置命令注册表
// ============================================================================

/// 内置命令注册表
///
/// 管理所有可用的内置命令。
pub struct BuiltinRegistry {
    /// 命令映射表
    commands: Vec<Box<dyn BuiltinCommand>>,
}

impl BuiltinRegistry {
    /// 创建新的内置命令注册表，注册所有默认内置命令
    pub fn new() -> Self {
        let commands: Vec<Box<dyn BuiltinCommand>> = vec![
            Box::new(HelpCommand),
            Box::new(VersionCommand),
            Box::new(PsCommand),
            Box::new(LsCommand),
            Box::new(CatCommand),
            Box::new(EchoCommand),
            Box::new(ClearCommand),
            Box::new(InfoCommand),
        ];
        BuiltinRegistry { commands }
    }

    /// 查找内置命令
    pub fn find(&self, name: &str) -> Option<&dyn BuiltinCommand> {
        self.commands
            .iter()
            .find(|cmd| cmd.name() == name)
            .map(|cmd| cmd.as_ref())
    }

    /// 列出所有内置命令名称
    pub fn list_names(&self) -> Vec<String> {
        self.commands.iter().map(|cmd| String::from(cmd.name())).collect()
    }

    /// 获取命令数量
    pub fn count(&self) -> usize {
        self.commands.len()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_command_name() {
        let cmd = HelpCommand;
        assert_eq!(cmd.name(), "help");
    }

    #[test]
    fn test_help_command_description() {
        let cmd = HelpCommand;
        assert_eq!(cmd.description(), "显示帮助信息");
    }

    #[test]
    fn test_help_command_execute() {
        let cmd = HelpCommand;
        let result = cmd.execute(&Command::new("help")).unwrap();
        assert!(result.success);
        assert!(result.output.contains("help"));
        assert!(result.output.contains("version"));
    }

    #[test]
    fn test_version_command_execute() {
        let cmd = VersionCommand;
        let result = cmd.execute(&Command::new("version")).unwrap();
        assert!(result.success);
        assert!(result.output.contains("OmniAgent OS"));
        assert!(result.output.contains(crate::KERNEL_VERSION));
    }

    #[test]
    fn test_ps_command_execute() {
        let cmd = PsCommand;
        let result = cmd.execute(&Command::new("ps")).unwrap();
        assert!(result.success);
        assert!(result.output.contains("PID"));
        assert!(result.output.contains("kernel"));
    }

    #[test]
    fn test_ls_command_execute() {
        let cmd = LsCommand;
        let result = cmd.execute(&Command::new("ls")).unwrap();
        assert!(result.success);
        assert!(result.output.contains("config.toml"));
    }

    #[test]
    fn test_ls_command_with_path() {
        let cmd = LsCommand;
        let cmd_input = Command::new("ls").arg("/tmp");
        let result = cmd.execute(&cmd_input).unwrap();
        assert!(result.success);
        assert!(result.output.contains("/tmp"));
    }

    #[test]
    fn test_cat_command_no_args() {
        let cmd = CatCommand;
        let result = cmd.execute(&Command::new("cat"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ShellError::InsufficientArguments { command, expected, found } => {
                assert_eq!(command, "cat");
                assert_eq!(expected, 1);
                assert_eq!(found, 0);
            }
            _ => panic!("Expected InsufficientArguments"),
        }
    }

    #[test]
    fn test_cat_command_with_file() {
        let cmd = CatCommand;
        let cmd_input = Command::new("cat").arg("test.txt");
        let result = cmd.execute(&cmd_input).unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.txt"));
    }

    #[test]
    fn test_echo_command() {
        let cmd = EchoCommand;
        let cmd_input = Command::new("echo").arg("hello").arg("world");
        let result = cmd.execute(&cmd_input).unwrap();
        assert!(result.success);
        assert_eq!(result.output, "hello world");
    }

    #[test]
    fn test_echo_command_no_args() {
        let cmd = EchoCommand;
        let result = cmd.execute(&Command::new("echo")).unwrap();
        assert!(result.success);
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_clear_command() {
        let cmd = ClearCommand;
        let result = cmd.execute(&Command::new("clear")).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_info_command() {
        let cmd = InfoCommand;
        let result = cmd.execute(&Command::new("info")).unwrap();
        assert!(result.success);
        assert!(result.output.contains(crate::KERNEL_NAME));
    }

    #[test]
    fn test_builtin_registry_new() {
        let registry = BuiltinRegistry::new();
        assert_eq!(registry.count(), 8);
    }

    #[test]
    fn test_builtin_registry_find() {
        let registry = BuiltinRegistry::new();
        assert!(registry.find("help").is_some());
        assert!(registry.find("echo").is_some());
        assert!(registry.find("nonexistent").is_none());
    }

    #[test]
    fn test_builtin_registry_list_names() {
        let registry = BuiltinRegistry::new();
        let names = registry.list_names();
        assert_eq!(names.len(), 8);
        assert!(names.contains(&String::from("help")));
        assert!(names.contains(&String::from("version")));
    }

    #[test]
    fn test_command_result_ok() {
        let result = CommandResult::ok("test output");
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output, "test output");
    }

    #[test]
    fn test_command_result_err() {
        let result = CommandResult::err("error msg");
        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
        assert_eq!(result.output, "error msg");
    }
}
