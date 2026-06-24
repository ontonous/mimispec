# 贡献指南 / Contribution Guide

首先感谢您考虑贡献 MimiSpec！🎉

First of all, thank you for considering contributing to MimiSpec!

## 行为准则 / Code of Conduct

本项目采用 [Contributor Covenant v2.0](CODE_OF_CONDUCT.md)。参与即表示您同意遵守其条款。
This project adheres to the [Contributor Covenant v2.0](CODE_OF_CONDUCT.md). By participating, you agree to uphold its terms.

---

## 开发环境 / Development Environment

- Rust 2021 edition (latest stable toolchain)
- 使用 `cargo` 管理依赖 / Managed via `cargo`
- 推荐使用 [rustup](https://rustup.rs/) 管理工具链

```bash
# 安装 Rust / Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

## 常用命令 / Common Commands

```bash
cargo build              # 编译 / Build
cargo test --lib         # 单元测试 / Unit tests (77)
cargo test               # 全量测试 / All tests (incl. bin)
cargo clippy             # lint 检查（必须零警告）/ Lint (zero warnings required)
cargo test --release -- --ignored  # 压力测试 / Stress tests (1000 items)
```

## 代码规范 / Code Conventions

- 所有 `cargo clippy` 警告视为错误 / All clippy warnings are errors
- 表达式解析器使用 Pratt（precedence climbing）算法
- 缩进和布局语法由 `lexer.rs` 处理（indent/dedent token）
- 测试分布在 `src/lib/mod.rs` 的 `mod tests`、`edge_case_tests`、`stress_tests`、`fuzzy_tests` 中
- 中英文双语注释：公共 API、复杂逻辑、AST 类型需同时有中英文注释
- 保持文件行数合理，`parser/` 下每个子解析器一个文件

## 工作流 / Workflow

### 修复 Bug / Bug Fix

1. 编写重现测试 / Write a failing test
2. 修复代码 / Fix
3. 测试通过 / Test passes
4. 补充回归测试 / Add regression tests
5. 全量测试 + clippy / Full test + clippy
6. COMMIT

### 新增功能 / Feature

1. 在 `mod tests` 中添加测试 / Add parse/roundtrip tests
2. 实现功能 / Implement
3. `cargo clippy` 零警告 / Zero warnings
4. `cargo test --lib` 通过 / Pass
5. COMMIT

## 分支命名 / Branch Naming

| Prefix | Purpose |
|--------|---------|
| `feat/*` | New feature / 新功能 |
| `fix/*` | Bug fix / Bug 修复 |
| `docs/*` | Documentation / 文档 |
| `refactor/*` | Refactor / 重构 |
| `chore/*` | Maintenance / 维护 |

## 提交信息格式 / Commit Format

```
<type>: <简短描述 / short description>

type: feat / fix / refactor / docs / test / chore
```

推荐节奏 / Recommended rhythm:
```
COMMIT A: test: 补充 XXX 的解析/往返测试 / add parse/roundtrip tests for XXX
COMMIT B: fix: 修复 XXX 解析错误 / fix XXX parse error
COMMIT C: docs: 同步更新规范文档 / sync docs
```

## PR 流程 / PR Workflow

1. 从 `main` 创建功能分支 / Create feature branch from `main`
2. 确保 `cargo clippy` 和 `cargo test --lib` 通过 / Ensure both pass
3. 创建 PR，填写 [PR 模板](.github/PULL_REQUEST_TEMPLATE.md) / Create PR with template
4. 等待 CI 通过后合并 / Wait for CI to pass before merge
5. 如有文档同步义务，同步更新 `docs/` 和 `AGENTS.md`

## 文档同步义务 / Doc Sync Obligations

| 修改内容 / Changed | 必须更新 / Must update |
|---------------------|------------------------|
| 语言特性 / Language features | `AGENTS.md`, `docs/specification.md` |
| 公共 API / Public API | `AGENTS.md`, `README.md` |
| 测试套件 / Test suites | `AGENTS.md` §6 |
| CI/CD | `AGENTS.md` §1 CI 门禁 |

## 问题报告 / Issue Reporting

请使用 [GitHub Issues](https://github.com/ontonous/mimispec/issues) 提交 bug 报告或功能请求。
Please use [GitHub Issues](https://github.com/ontonous/mimispec/issues) for bug reports and feature requests.

报告 bug 时请提供：
- MimiSpec 版本号
- 重现步骤
- 期望行为与实际行为
- 相关代码片段

---

再次感谢您的贡献！❤️  
Thank you for contributing!
