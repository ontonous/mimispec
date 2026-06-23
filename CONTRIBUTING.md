# 贡献指南 / Contribution Guide

## 开发环境 / Development Environment

- Rust 2021 edition
- 使用 `cargo` 管理依赖 / Managed via `cargo`

## 常用命令 / Common Commands

```bash
cargo build              # 编译 / Build
cargo test --lib         # 单元测试 / Unit tests
cargo test               # 全量测试 / All tests
cargo clippy             # lint 检查（必须零警告）/ Lint (zero warnings required)
cargo test --release -- --ignored  # 压力测试 / Stress tests
```

## 代码规范 / Code Conventions

- 所有 `cargo clippy` 警告视为错误 / All clippy warnings are errors
- 表达式解析器使用 Pratt（precedence climbing）算法
- 缩进和布局语法由 `lexer.rs` 处理（indent/dedent token）
- 测试分布在 `src/lib/mod.rs` 的 `mod tests`、`edge_case_tests`、`stress_tests`、`fuzzy_tests` 中

## 分支命名 / Branch Naming

| 前缀 / Prefix | 用途 / Purpose |
|------|------|
| `feat/*` | 新功能 / New feature |
| `fix/*` | Bug 修复 / Bug fix |
| `docs/*` | 文档 / Documentation |
| `refactor/*` | 重构 / Refactor |

## 提交信息格式 / Commit Format

```
<type>: <简短描述 / short description (50 chars max)>

类型 / Types: feat / fix / refactor / docs / test / chore
```

## PR 流程 / PR Workflow

1. 从 `main` 创建功能分支 / Create feature branch from `main`
2. 确保 `cargo clippy` 和 `cargo test --lib` 通过 / Ensure both pass
3. 创建 PR，填写 PR 模板 / Create PR with template
4. 等待 CI 通过后合并 / Wait for CI to pass before merge
