//! Shell 解释器
//!
//! 实现 Shell 解释器，包括命令执行、管道处理、
//! 命令历史记录、别名支持和环境变量管理。

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use spin::Mutex;

use crate::shell::builtin::{BuiltinRegistry, CommandResult};
use crate::shell::command::{Command, CommandParser, Pipeline};
use crate::shell::error::ShellError;

// ============================================================================
// 命令历史记录
// ============================================================================

/// 命令历史记录管理器
///
/// 保存最近 100 条命令历史。
const MAX_HISTORY_SIZE: usize = 100;

pub struct CommandHistory {
    entries: Vec<String>,
}

impl CommandHistory {
    /// 创建新的命令历史
    pub fn new() -> Self {
        CommandHistory {
            entries: Vec::new(),
        }
    }

    /// 添加命令到历史
    pub fn push(&mut self, command: &str) {
        let cmd_str = String::from(command);
        // 如果与最后一条相同则不重复添加
        if self.entries.last().map(|s| s.as_str()) == Some(command) {
            return;
        }
        if self.entries.len() >= MAX_HISTORY_SIZE {
            self.entries.remove(0);
        }
        self.entries.push(cmd_str);
    }

    /// 获取所有历史记录
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// 获取最近 N 条记录
    pub fn last_n(&self, n: usize) -> Vec<String> {
        let start = if self.entries.len() > n {
            self.entries.len() - n
        } else {
            0
        };
        self.entries[start..].to_vec()
    }

    /// 获取历史记录数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 历史记录是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 清空历史记录
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ============================================================================
// 别名管理
// ============================================================================

/// 别名管理器
pub struct AliasManager {
    aliases: BTreeMap<String, String>,
}

impl AliasManager {
    /// 创建新的别名管理器
    pub fn new() -> Self {
        AliasManager {
            aliases: BTreeMap::new(),
        }
    }

    /// 设置别名
    pub fn set(&mut self, name: &str, value: &str) {
        self.aliases.insert(String::from(name), String::from(value));
    }

    /// 获取别名
    pub fn get(&self, name: &str) -> Option<&String> {
        self.aliases.get(name)
    }

    /// 移除别名
    pub fn remove(&mut self, name: &str) -> bool {
        self.aliases.remove(name).is_some()
    }

    /// 列出所有别名
    pub fn list(&self) -> Vec<(String, String)> {
        self.aliases
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// 检查是否存在别名
    pub fn contains(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// 展开别名（替换命令名称）
    pub fn expand(&self, input: &str) -> String {
        let trimmed = input.trim();
        // 获取第一个单词（命令名）
        let cmd_name_end = trimmed
            .find(|c: char| c.is_whitespace())
            .unwrap_or(trimmed.len());
        let cmd_name = &trimmed[..cmd_name_end];
        let rest = &trimmed[cmd_name_end..];

        if let Some(alias_value) = self.aliases.get(cmd_name) {
            format!("{}{}", alias_value, rest)
        } else {
            String::from(trimmed)
        }
    }

    /// 清空所有别名
    pub fn clear(&mut self) {
        self.aliases.clear();
    }
}

// ============================================================================
// 环境变量管理
// ============================================================================

/// 环境变量管理器
pub struct Environment {
    vars: BTreeMap<String, String>,
}

impl Environment {
    /// 创建新的环境变量管理器
    pub fn new() -> Self {
        let mut vars = BTreeMap::new();
        // 设置默认环境变量
        vars.insert(String::from("PATH"), String::from("/bin:/usr/bin"));
        vars.insert(String::from("HOME"), String::from("/root"));
        vars.insert(String::from("SHELL"), String::from("/bin/sh"));
        vars.insert(String::from("TERM"), String::from("vt100"));
        vars.insert(String::from("LANG"), String::from("zh_CN"));
        Environment { vars }
    }

    /// 设置环境变量
    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(String::from(key), String::from(value));
    }

    /// 获取环境变量
    pub fn get(&self, key: &str) -> Option<&String> {
        self.vars.get(key)
    }

    /// 移除环境变量
    pub fn remove(&mut self, key: &str) -> bool {
        self.vars.remove(key).is_some()
    }

    /// 列出所有环境变量
    pub fn list(&self) -> Vec<(String, String)> {
        self.vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// 检查环境变量是否存在
    pub fn contains(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// 清空所有环境变量
    pub fn clear(&mut self) {
        self.vars.clear();
    }
}

// ============================================================================
// Shell 解释器
// ============================================================================

/// Shell 解释器
///
/// 提供命令执行、管道处理、历史记录、别名和环境变量管理。
pub struct ShellInterpreter {
    /// 内置命令注册表
    builtin_registry: BuiltinRegistry,
    /// 命令历史
    history: CommandHistory,
    /// 别名管理
    aliases: AliasManager,
    /// 环境变量
    env: Environment,
}

impl ShellInterpreter {
    /// 创建新的 Shell 解释器
    pub fn new() -> Self {
        ShellInterpreter {
            builtin_registry: BuiltinRegistry::new(),
            history: CommandHistory::new(),
            aliases: AliasManager::new(),
            env: Environment::new(),
        }
    }

    /// 执行命令字符串
    ///
    /// 解析输入字符串，展开别名，然后执行。
    pub fn execute(&mut self, input: &str) -> Result<CommandResult, ShellError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(CommandResult::ok(""));
        }

        // 展开别名
        let expanded = self.aliases.expand(trimmed);

        // 解析管道
        let pipeline = CommandParser::parse_pipeline(&expanded)?;

        if pipeline.is_empty() {
            return Ok(CommandResult::ok(""));
        }

        // 记录历史
        self.history.push(trimmed);

        // 执行管道
        self.execute_pipeline(&pipeline)
    }

    /// 执行管道
    pub fn execute_pipeline(&self, pipeline: &Pipeline) -> Result<CommandResult, ShellError> {
        if pipeline.is_empty() {
            return Ok(CommandResult::ok(""));
        }

        let mut last_output = String::new();

        for (i, cmd) in pipeline.commands.iter().enumerate() {
            let result = self.execute_command(cmd)?;

            if !result.success {
                return Ok(result);
            }

            if i < pipeline.commands.len() - 1 {
                // 管道中间命令的输出传递给下一个命令
                last_output = result.output;
            } else {
                // 最后一个命令的结果
                return Ok(result);
            }
        }

        Ok(CommandResult::ok(&last_output))
    }

    /// 执行单个命令
    pub fn execute_command(&self, cmd: &Command) -> Result<CommandResult, ShellError> {
        // 查找内置命令
        if let Some(builtin) = self.builtin_registry.find(&cmd.name) {
            return builtin.execute(cmd);
        }

        Err(ShellError::CommandNotFound(cmd.name.clone()))
    }

    /// 获取命令历史
    pub fn history(&self) -> &[String] {
        self.history.entries()
    }

    /// 获取最近 N 条历史
    pub fn history_last(&self, n: usize) -> Vec<String> {
        self.history.last_n(n)
    }

    /// 设置别名
    pub fn set_alias(&mut self, name: &str, value: &str) {
        self.aliases.set(name, value);
    }

    /// 获取别名
    pub fn get_alias(&self, name: &str) -> Option<&String> {
        self.aliases.get(name)
    }

    /// 移除别名
    pub fn remove_alias(&mut self, name: &str) -> bool {
        self.aliases.remove(name)
    }

    /// 列出所有别名
    pub fn list_aliases(&self) -> Vec<(String, String)> {
        self.aliases.list()
    }

    /// 设置环境变量
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.set(key, value);
    }

    /// 获取环境变量
    pub fn get_env(&self, key: &str) -> Option<&String> {
        self.env.get(key)
    }

    /// 移除环境变量
    pub fn remove_env(&mut self, key: &str) -> bool {
        self.env.remove(key)
    }

    /// 列出所有环境变量
    pub fn list_env(&self) -> Vec<(String, String)> {
        self.env.list()
    }

    /// 获取内置命令注册表的引用
    pub fn builtin_registry(&self) -> &BuiltinRegistry {
        &self.builtin_registry
    }

    /// 清空历史记录
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 清空别名
    pub fn clear_aliases(&mut self) {
        self.aliases.clear();
    }

    /// 清空环境变量
    pub fn clear_env_vars(&mut self) {
        self.env.clear();
    }
}

/// 全局 Shell 解释器
pub static SHELL_INTERPRETER: spin::Lazy<Mutex<ShellInterpreter>> = spin::Lazy::new(|| {
    Mutex::new(ShellInterpreter::new())
});

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_help() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("help").unwrap();
        assert!(result.success);
        assert!(result.output.contains("help"));
    }

    #[test]
    fn test_execute_version() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("version").unwrap();
        assert!(result.success);
        assert!(result.output.contains("OmniAgent OS"));
    }

    #[test]
    fn test_execute_echo() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("echo hello world").unwrap();
        assert!(result.success);
        assert_eq!(result.output, "hello world");
    }

    #[test]
    fn test_execute_empty() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("").unwrap();
        assert!(result.success);
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_execute_whitespace() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("   ").unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_execute_unknown_command() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("nonexistent_cmd");
        assert!(result.is_err());
        match result.unwrap_err() {
            ShellError::CommandNotFound(name) => {
                assert_eq!(name, "nonexistent_cmd");
            }
            _ => panic!("Expected CommandNotFound"),
        }
    }

    #[test]
    fn test_execute_pipeline() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("echo test | echo").unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_command_history() {
        let mut interp = ShellInterpreter::new();
        interp.execute("echo a").unwrap();
        interp.execute("echo b").unwrap();
        interp.execute("echo c").unwrap();

        assert_eq!(interp.history().len(), 3);
        assert_eq!(interp.history()[0], "echo a");
        assert_eq!(interp.history()[1], "echo b");
        assert_eq!(interp.history()[2], "echo c");
    }

    #[test]
    fn test_command_history_no_duplicates() {
        let mut interp = ShellInterpreter::new();
        interp.execute("echo a").unwrap();
        interp.execute("echo a").unwrap();
        assert_eq!(interp.history().len(), 1);
    }

    #[test]
    fn test_command_history_empty_input() {
        let mut interp = ShellInterpreter::new();
        interp.execute("").unwrap();
        assert!(interp.history().is_empty());
    }

    #[test]
    fn test_command_history_last_n() {
        let mut interp = ShellInterpreter::new();
        interp.execute("echo 1").unwrap();
        interp.execute("echo 2").unwrap();
        interp.execute("echo 3").unwrap();
        interp.execute("echo 4").unwrap();
        interp.execute("echo 5").unwrap();

        let last_3 = interp.history_last(3);
        assert_eq!(last_3.len(), 3);
        assert_eq!(last_3[0], "echo 3");
        assert_eq!(last_3[1], "echo 4");
        assert_eq!(last_3[2], "echo 5");
    }

    #[test]
    fn test_alias_set_and_expand() {
        let mut interp = ShellInterpreter::new();
        interp.set_alias("ll", "ls -la");

        let result = interp.execute("ll").unwrap();
        assert!(result.success);
        assert!(result.output.contains("config.toml"));
    }

    #[test]
    fn test_alias_with_args() {
        let mut interp = ShellInterpreter::new();
        interp.set_alias("c", "cat");

        let result = interp.execute("c test.txt").unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.txt"));
    }

    #[test]
    fn test_alias_remove() {
        let mut interp = ShellInterpreter::new();
        interp.set_alias("test_alias", "echo hi");
        assert!(interp.get_alias("test_alias").is_some());
        interp.remove_alias("test_alias");
        assert!(interp.get_alias("test_alias").is_none());
    }

    #[test]
    fn test_alias_list() {
        let mut interp = ShellInterpreter::new();
        interp.set_alias("a1", "echo 1");
        interp.set_alias("a2", "echo 2");

        let aliases = interp.list_aliases();
        assert_eq!(aliases.len(), 2);
    }

    #[test]
    fn test_env_get_default() {
        let interp = ShellInterpreter::new();
        assert!(interp.get_env("PATH").is_some());
        assert!(interp.get_env("HOME").is_some());
        assert!(interp.get_env("SHELL").is_some());
    }

    #[test]
    fn test_env_set_and_get() {
        let mut interp = ShellInterpreter::new();
        interp.set_env("TEST_VAR", "test_value");
        assert_eq!(interp.get_env("TEST_VAR").unwrap(), "test_value");
    }

    #[test]
    fn test_env_remove() {
        let mut interp = ShellInterpreter::new();
        interp.set_env("REMOVE_ME", "value");
        assert!(interp.remove_env("REMOVE_ME"));
        assert!(interp.get_env("REMOVE_ME").is_none());
    }

    #[test]
    fn test_env_list() {
        let interp = ShellInterpreter::new();
        let vars = interp.list_env();
        assert!(!vars.is_empty());
        // 检查默认变量存在
        let keys: Vec<&String> = vars.iter().map(|(k, _)| k).collect();
        assert!(keys.iter().any(|k| *k == "PATH"));
        assert!(keys.iter().any(|k| *k == "HOME"));
    }

    #[test]
    fn test_clear_history() {
        let mut interp = ShellInterpreter::new();
        interp.execute("echo a").unwrap();
        interp.execute("echo b").unwrap();
        assert_eq!(interp.history().len(), 2);
        interp.clear_history();
        assert!(interp.history().is_empty());
    }

    #[test]
    fn test_execute_pipeline_failure() {
        let mut interp = ShellInterpreter::new();
        // 管道中有一个不存在的命令
        let result = interp.execute("nonexistent_cmd | echo");
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_pipeline_multiple() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("echo first | echo second | echo third");
        assert!(result.is_ok());
    }

    #[test]
    fn test_builtin_registry_access() {
        let interp = ShellInterpreter::new();
        assert!(interp.builtin_registry().find("help").is_some());
        assert!(interp.builtin_registry().find("info").is_some());
        assert_eq!(interp.builtin_registry().count(), 8);
    }

    #[test]
    fn test_execute_info() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("info").unwrap();
        assert!(result.success);
        assert!(result.output.contains("OmniAgent OS"));
    }

    #[test]
    fn test_execute_ps() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("ps").unwrap();
        assert!(result.success);
        assert!(result.output.contains("PID"));
    }

    #[test]
    fn test_execute_cat_no_args() {
        let mut interp = ShellInterpreter::new();
        let result = interp.execute("cat");
        assert!(result.is_err());
    }
}
