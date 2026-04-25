# 贡献指南

> 感谢你对 OmniAgent OS 项目的关注！本指南说明了参与项目贡献的流程、规范和要求。

## 代码风格

### rustfmt 配置

```toml
# rustfmt.toml
edition = "2024"
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
reorder_imports = true
fn_single_line = false
control_brace_style = "AlwaysSameLine"
match_arm_blocks = true
trailing_semicolon = true
trailing_comma = "Vertical"
use_field_init_shorthand = true
use_try_shorthand = true
force_explicit_abi = true
```

```bash
make fmt                    # 格式化所有代码
cargo fmt --all -- --check  # CI 检查（不修改文件）
```

### Clippy 规则

```toml
# .clippy.toml
warn-lint = ["clippy::all", "clippy::pedantic", "clippy::nursery", "clippy::cargo"]
deny-lint = [
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::todo",
    "clippy::indexing_slicing",
    "clippy::arithmetic_side_effects",
    "clippy::std_instead_of_core",
    "clippy::missing_safety_doc",
]
```

```bash
make clippy                 # 运行检查
cargo clippy --fix --allow-dirty  # 自动修复
```

### 代码风格示例

```rust
// 命名：snake_case (函数/模块), PascalCase (结构体/枚举), SCREAMING_SNAKE_CASE (常量)
pub const PAGE_SIZE: usize = 4096;

/// 分配一个物理页帧。
///
/// # Safety
/// 调用者必须确保使用完毕后调用 `deallocate_page_frame` 释放。
pub unsafe fn allocate_page_frame() -> Option<PageFrame> {
    FRAME_ALLOCATOR.lock().allocate()
}
```

---

## 提交信息规范

### Conventional Commits

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### 提交类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `feat` | 新功能 | `feat(ipc): add async message passing` |
| `fix` | 修复 Bug | `fix(mm): resolve page table corruption` |
| `docs` | 文档变更 | `docs(guides): add driver guide` |
| `style` | 代码风格 | `style(kernel): apply rustfmt` |
| `refactor` | 重构 | `refactor(scheduler): extract run queue` |
| `perf` | 性能优化 | `perf(ipc): use shared memory` |
| `test` | 测试 | `test(syscall): add write tests` |
| `build` | 构建系统 | `build(ci): add aarch64 build` |
| `ci` | CI 配置 | `ci(workflow): add cargo cache` |
| `chore` | 杂项 | `chore(deps): update bitflags` |

### 常用作用域

`kernel`, `mm`, `ipc`, `scheduler`, `syscall`, `driver`, `agent`, `arch/x86_64`, `ci`, `docs`, `tools`

### 提交信息示例

```
feat(ipc): add zero-copy message passing for large payloads

Implement shared memory-based message passing for messages larger than
4KB, improving IPC throughput by approximately 3x.

Closes #123
```

```bash
# 使用 commitizen 生成提交信息
cargo install cz
cz commit
```

---

## Pull Request 流程

### 分支命名

```
<type>/<issue-number>-<short-description>

# 示例
feat/123-async-ipc
fix/456-page-table-bug
docs/789-driver-guide
```

### PR 描述模板

```markdown
## 变更类型
- [ ] feat / fix / docs / refactor / perf / test / chore

## 变更描述
<!-- 简要描述 -->

## 关联 Issue
Closes #

## 测试
- [ ] 单元测试通过
- [ ] 集成测试通过
- [ ] QEMU 启动测试通过

## 检查清单
- [ ] cargo fmt --check 通过
- [ ] cargo clippy 无警告
- [ ] 所有测试通过
- [ ] 文档已更新
- [ ] 提交信息符合 Conventional Commits
```

### PR 审查清单

**代码质量**：逻辑正确、边界处理、错误处理、unsafe 有 SAFETY 文档
**代码风格**：rustfmt 通过、clippy 无警告、命名规范
**测试**：新功能有测试、Bug 有回归测试、覆盖关键路径
**文档**：公共 API 有文档注释、指南已更新、CHANGELOG 已更新
**安全**：无漏洞、输入验证充分、能力声明最小化

---

## 测试要求

| 测试类型 | 位置 | 运行方式 | 覆盖率 |
|---------|------|---------|--------|
| 单元测试 | `#[cfg(test)] mod tests` | `cargo test --lib` | >= 80% |
| 集成测试 | `tests/` 目录 | `cargo test` | 关键路径 100% |
| 内核测试 | QEMU 启动测试 | `cargo bootimage --test` | 关键功能 |
| 文档测试 | `/// ``` ` 代码块 | `cargo test --doc` | 所有示例 |

### 测试编写规范

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_frame_allocation() {
        let allocator = setup_test_allocator();
        let frame = allocator.allocate();
        assert!(frame.is_some());
        assert_eq!(frame.unwrap().size(), PAGE_SIZE);
    }

    #[test]
    fn test_allocation_exhaustion() {
        let allocator = setup_test_allocator_with_limit(1);
        assert!(allocator.allocate().is_some());
        assert!(allocator.allocate().is_none());
    }

    use proptest::prelude::*;
    proptest! {
        #[test]
        fn test_roundtrip_serialization(
            data in proptest::collection::vec(proptest::num::u8::ANY, 0..1024)
        ) {
            let serialized = serialize(&data).unwrap();
            prop_assert_eq!(data, deserialize(&serialized).unwrap());
        }
    }
}
```

覆盖率检查：`cargo install cargo-tarpaulin && cargo tarpaulin --out Html --output-dir target/coverage`

---

## 文档要求

### 文档注释规范

```rust
/// IPC 通道 - Agent 间通信的双向管道。
///
/// # 示例
///
/// ```
/// let port = Port::create("my-service")?;
/// let (msg, reply) = port.receive()?;
/// ```
///
/// # 错误
///
/// - `ChannelError::NotFound`: 目标服务不存在
/// - `ChannelError::Timeout`: 请求超时
pub struct Channel { /* ... */ }
```

### 文档更新要求

每次 PR 必须包含：
1. 新增/修改的公共 API 有文档注释
2. 影响开发者工作流时更新对应指南
3. 在 `CHANGELOG.md` 的 `[Unreleased]` 部分添加条目

### CHANGELOG 格式

```markdown
## [Unreleased]
### Added
### Changed
### Deprecated
### Removed
### Fixed
### Security
```

---

## 架构决策记录 (ADR)

### ADR 模板

```markdown
# ADR-XXX: [决策标题]

## 状态
[提议 | 已接受 | 已废弃]

## 背景
[问题描述和动机]

## 决策
[做出的决策]

## 理由
[为什么选择此方案]

## 后果
### 正面 / 负面 / 风险

## 替代方案
### 方案 A / 方案 B

## 参考
- [相关链接]
```

### ADR 示例

```markdown
# ADR-001: 用户态设备驱动模型

## 状态
已接受

## 背景
传统内核态驱动存在稳定性、调试困难、安全隔离不足的问题。

## 决策
所有设备驱动作为 Agent 进程运行在用户空间，内核仅负责中断路由。

## 理由
1. 安全性：驱动崩溃不影响内核
2. 可调试性：使用标准用户态工具
3. 一致性：与 Agent 模型统一

## 后果
- 正面：稳定性提升、开发门槛降低
- 负面：IPC 开销约 5-15%、中断延迟增加
- 风险：高性能设备需特殊优化（缓解：共享内存快速路径）
```

---

## Issue 报告

### Bug 报告模板

```markdown
## Bug 描述
[简洁描述]

## 复现步骤
1. [步骤 1]
2. [步骤 2]

## 期望行为 / 实际行为

## 环境信息
- OS: [版本]
- Rust: [rustc --version]
- QEMU: [版本]
- Commit: [git rev-parse HEAD]

## 日志输出
```
[粘贴日志]
```
```

### 功能请求模板

```markdown
## 功能描述
[描述]

## 动机
[为什么需要]

## 建议实现方式
[如有]
```

---

## 代码审查指南

### 审查维度

1. **正确性**：逻辑、边界条件、并发安全、错误处理
2. **安全性**：缓冲区溢出、输入验证、能力声明、信息泄露
3. **性能**：内存分配、数据结构选择、死锁风险
4. **可维护性**：可读性、命名、注释、项目约定
5. **测试**：充分性、边界覆盖、测试质量

### 审查流程

```bash
git fetch origin pull/123/head:pr-123 && git checkout pr-123
make clean && make test && make clippy && make run
git diff main...pr-123
```

---

## 发布流程

版本号规范 (SemVer)：`MAJOR.MINOR.PATCH`

```bash
git checkout -b release/v0.2.0
make clean && make test && make clippy && make run
git add -A && git commit -m "chore(release): prepare v0.2.0"
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin release/v0.2.0 && git push origin v0.2.0
gh release create v0.2.0 --title "v0.2.0" --notes-file CHANGELOG.md \
    target/bootimage-omniagent-kernel.bin
```

---

## 社区指南

### 社区指南

| 渠道 | 用途 |
|------|------|
| GitHub Discussions | 一般讨论、问答 |
| GitHub Issues | Bug 报告、功能请求 |
| Discord | 实时交流 |

**行为准则**：尊重他人、建设性反馈、包容所有贡献者、协作解决分歧、聚焦技术讨论。

**获取帮助**：查阅文档 -> GitHub Discussions -> Discord -> 标记 `@omniagent-os/maintainers`

**成为维护者**：持续贡献 3 个月以上，提交 10+ 合并 PR，展示架构理解，积极参与审查。
