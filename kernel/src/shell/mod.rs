//! Shell 命令行模块
//!
//! 提供 OmniAgent OS 内核的命令行界面功能，包括：
//! - 命令解析（command）
//! - 内置命令（builtin）
//! - Shell 解释器（interpreter）

pub mod error;
pub mod command;
pub mod builtin;
pub mod interpreter;
