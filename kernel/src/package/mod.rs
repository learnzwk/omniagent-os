//! 包管理器模块
//!
//! 提供 OmniAgent OS 内核的包管理功能，包括：
//! - 包清单（manifest）定义与版本管理
//! - 包注册表（registry）管理
//! - 依赖解析器（resolver）

pub mod error;
pub mod manifest;
pub mod registry;
pub mod resolver;
