//! 命令表示与解析
//!
//! 定义命令结构体、管道结构和命令解析器。

use alloc::string::String;
use alloc::vec::Vec;

// ============================================================================
// 命令结构
// ============================================================================

/// 命令结构
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// 命令名称
    pub name: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 标准输入重定向
    pub stdin_redir: Option<String>,
    /// 标准输出重定向
    pub stdout_redir: Option<String>,
    /// 标准错误重定向
    pub stderr_redir: Option<String>,
    /// 是否后台运行
    pub background: bool,
}

impl Command {
    /// 创建新命令
    pub fn new(name: &str) -> Self {
        Command {
            name: String::from(name),
            args: Vec::new(),
            stdin_redir: None,
            stdout_redir: None,
            stderr_redir: None,
            background: false,
        }
    }

    /// 添加参数
    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(String::from(arg));
        self
    }
}

// ============================================================================
// 管道结构
// ============================================================================

/// 管道结构
///
/// 表示由管道符 | 连接的多个命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    /// 管道中的命令列表
    pub commands: Vec<Command>,
}

impl Pipeline {
    /// 创建新的管道
    pub fn new() -> Self {
        Pipeline {
            commands: Vec::new(),
        }
    }

    /// 添加命令到管道
    pub fn add_command(mut self, cmd: Command) -> Self {
        self.commands.push(cmd);
        self
    }

    /// 获取管道中的命令数量
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// 管道是否为空
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

// ============================================================================
// 命令解析器
// ============================================================================

/// 命令解析器
///
/// 将输入字符串解析为 Command 或 Pipeline。
pub struct CommandParser;

impl CommandParser {
    /// 解析输入字符串为管道
    ///
    /// 支持管道符 | 分隔多个命令。
    pub fn parse_pipeline(input: &str) -> Result<Pipeline, crate::shell::error::ShellError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(Pipeline::new());
        }

        let mut pipeline = Pipeline::new();
        let segments: Vec<&str> = input.split('|').collect();

        for segment in segments {
            let segment = segment.trim();
            if segment.is_empty() {
                return Err(crate::shell::error::ShellError::SyntaxError(
                    String::from("管道符后缺少命令"),
                ));
            }
            let cmd = Self::parse(segment)?;
            pipeline.commands.push(cmd);
        }

        Ok(pipeline)
    }

    /// 解析输入字符串为单个命令
    ///
    /// 支持功能:
    /// - 空格分割参数
    /// - 双引号字符串
    /// - 单引号字符串
    /// - 输入/输出重定向 (<, >, 2>)
    /// - 后台运行 (&)
    pub fn parse(input: &str) -> Result<Command, crate::shell::error::ShellError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(crate::shell::error::ShellError::SyntaxError(
                String::from("空命令"),
            ));
        }

        let tokens = Self::tokenize(input)?;
        if tokens.is_empty() {
            return Err(crate::shell::error::ShellError::SyntaxError(
                String::from("空命令"),
            ));
        }

        let mut cmd = Command::new(&tokens[0]);
        let mut i = 1;

        while i < tokens.len() {
            let token = &tokens[i];

            match token.as_str() {
                "<" => {
                    i += 1;
                    if i >= tokens.len() {
                        return Err(crate::shell::error::ShellError::SyntaxError(
                            String::from("输入重定向缺少文件名"),
                        ));
                    }
                    cmd.stdin_redir = Some(tokens[i].clone());
                }
                ">" => {
                    i += 1;
                    if i >= tokens.len() {
                        return Err(crate::shell::error::ShellError::SyntaxError(
                            String::from("输出重定向缺少文件名"),
                        ));
                    }
                    cmd.stdout_redir = Some(tokens[i].clone());
                }
                "2>" => {
                    i += 1;
                    if i >= tokens.len() {
                        return Err(crate::shell::error::ShellError::SyntaxError(
                            String::from("错误重定向缺少文件名"),
                        ));
                    }
                    cmd.stderr_redir = Some(tokens[i].clone());
                }
                "&" => {
                    cmd.background = true;
                }
                _ => {
                    cmd.args.push(token.clone());
                }
            }
            i += 1;
        }

        Ok(cmd)
    }

    /// 将输入字符串分割为 token 列表
    ///
    /// 支持引号字符串和转义字符。
    fn tokenize(input: &str) -> Result<Vec<String>, crate::shell::error::ShellError> {
        let mut tokens: Vec<String> = Vec::new();
        let mut current: String = String::new();
        let mut chars = input.chars().peekable();
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        while let Some(&ch) = chars.peek() {
            match ch {
                ' ' | '\t' => {
                    if in_single_quote || in_double_quote {
                        current.push(ch);
                    } else if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    chars.next();
                }
                '"' => {
                    if in_double_quote {
                        in_double_quote = false;
                    } else if !in_single_quote {
                        in_double_quote = true;
                    } else {
                        current.push(ch);
                    }
                    chars.next();
                }
                '\'' => {
                    if in_single_quote {
                        in_single_quote = false;
                    } else if !in_double_quote {
                        in_single_quote = true;
                    } else {
                        current.push(ch);
                    }
                    chars.next();
                }
                _ => {
                    current.push(ch);
                    chars.next();
                }
            }
        }

        // 未闭合的引号
        if in_single_quote || in_double_quote {
            return Err(crate::shell::error::ShellError::SyntaxError(
                String::from("未闭合的引号"),
            ));
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        Ok(tokens)
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_new() {
        let cmd = Command::new("ls");
        assert_eq!(cmd.name, "ls");
        assert!(cmd.args.is_empty());
        assert!(!cmd.background);
    }

    #[test]
    fn test_command_with_args() {
        let cmd = Command::new("echo").arg("hello").arg("world");
        assert_eq!(cmd.args.len(), 2);
        assert_eq!(cmd.args[0], "hello");
        assert_eq!(cmd.args[1], "world");
    }

    #[test]
    fn test_parse_simple() {
        let cmd = CommandParser::parse("ls -la").unwrap();
        assert_eq!(cmd.name, "ls");
        assert_eq!(cmd.args, vec!["-la"]);
    }

    #[test]
    fn test_parse_multiple_args() {
        let cmd = CommandParser::parse("cp src.txt dest.txt").unwrap();
        assert_eq!(cmd.name, "cp");
        assert_eq!(cmd.args, vec!["src.txt", "dest.txt"]);
    }

    #[test]
    fn test_parse_quoted_string() {
        let cmd = CommandParser::parse("echo \"hello world\"").unwrap();
        assert_eq!(cmd.name, "echo");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(cmd.args[0], "hello world");
    }

    #[test]
    fn test_parse_single_quoted_string() {
        let cmd = CommandParser::parse("echo 'foo bar'").unwrap();
        assert_eq!(cmd.name, "echo");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(cmd.args[0], "foo bar");
    }

    #[test]
    fn test_parse_redirect_stdout() {
        let cmd = CommandParser::parse("ls > output.txt").unwrap();
        assert_eq!(cmd.name, "ls");
        assert_eq!(cmd.stdout_redir, Some(String::from("output.txt")));
    }

    #[test]
    fn test_parse_redirect_stdin() {
        let cmd = CommandParser::parse("sort < input.txt").unwrap();
        assert_eq!(cmd.name, "sort");
        assert_eq!(cmd.stdin_redir, Some(String::from("input.txt")));
    }

    #[test]
    fn test_parse_redirect_stderr() {
        let cmd = CommandParser::parse("cmd 2> error.log").unwrap();
        assert_eq!(cmd.name, "cmd");
        assert_eq!(cmd.stderr_redir, Some(String::from("error.log")));
    }

    #[test]
    fn test_parse_background() {
        let cmd = CommandParser::parse("sleep 10 &").unwrap();
        assert_eq!(cmd.name, "sleep");
        assert_eq!(cmd.args, vec!["10"]);
        assert!(cmd.background);
    }

    #[test]
    fn test_parse_empty() {
        assert!(CommandParser::parse("").is_err());
        assert!(CommandParser::parse("   ").is_err());
    }

    #[test]
    fn test_parse_unclosed_quote() {
        assert!(CommandParser::parse("echo \"hello").is_err());
        assert!(CommandParser::parse("echo 'hello").is_err());
    }

    #[test]
    fn test_parse_pipeline() {
        let pipeline = CommandParser::parse_pipeline("ls | grep foo").unwrap();
        assert_eq!(pipeline.len(), 2);
        assert_eq!(pipeline.commands[0].name, "ls");
        assert_eq!(pipeline.commands[1].name, "grep");
        assert_eq!(pipeline.commands[1].args, vec!["foo"]);
    }

    #[test]
    fn test_parse_pipeline_multiple() {
        let pipeline = CommandParser::parse_pipeline("cat file | sort | uniq").unwrap();
        assert_eq!(pipeline.len(), 3);
        assert_eq!(pipeline.commands[0].name, "cat");
        assert_eq!(pipeline.commands[1].name, "sort");
        assert_eq!(pipeline.commands[2].name, "uniq");
    }

    #[test]
    fn test_parse_pipeline_empty() {
        let pipeline = CommandParser::parse_pipeline("").unwrap();
        assert!(pipeline.is_empty());
    }

    #[test]
    fn test_parse_pipeline_empty_segment() {
        assert!(CommandParser::parse_pipeline("ls | ").is_err());
    }

    #[test]
    fn test_pipeline_new() {
        let pipeline = Pipeline::new();
        assert!(pipeline.is_empty());
        assert_eq!(pipeline.len(), 0);
    }

    #[test]
    fn test_pipeline_add_command() {
        let pipeline = Pipeline::new()
            .add_command(Command::new("ls"))
            .add_command(Command::new("grep").arg("foo"));
        assert_eq!(pipeline.len(), 2);
        assert_eq!(pipeline.commands[1].args[0], "foo");
    }

    #[test]
    fn test_command_equality() {
        let cmd1 = Command::new("ls").arg("-la");
        let cmd2 = Command::new("ls").arg("-la");
        let cmd3 = Command::new("ls").arg("-l");
        assert_eq!(cmd1, cmd2);
        assert_ne!(cmd1, cmd3);
    }
}
