//! Agent 管理子系统
//!
//! 提供内核中 Agent 的完整生命周期管理，包括：
//! - Agent 控制块 (ACB): 每个 Agent 的运行时状态表示
//! - Agent 池: 管理所有 ACB 的创建、查询和销毁
//! - 通信管理: Agent 间消息路由和发布/订阅

pub mod control_block;
pub mod pool;
pub mod communication;
