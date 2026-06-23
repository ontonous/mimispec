# 贡献指南

## 开发环境

- Rust 2021 edition
- 使用 `cargo` 管理依赖

## 常用命令

```bash
cargo build              # 编译
cargo test --lib         # 运行单元测试
cargo test               # 运行全部测试
cargo clippy             # lint 检查（必须零警告）
cargo test --release -- --ignored  # 压力测试
```

## 代码规范

- 所有 `cargo clippy` 警告视为错误
- 表达式解析器使用 Pratt（precedence climbing）算法
- 缩进和布局语法由 `lexer.rs` 处理（indent/dedent token）
- 测试分布在 `src/lib/mod.rs` 的 `mod tests`、`edge_case_tests`、`stress_tests`、`fuzzy_tests` 中

## 分支命名

| 前缀 | 用途 |
|------|------|
| `feat/*` | 新功能 |
| `fix/*` | Bug 修复 |
| `docs/*` | 文档 |
| `refactor/*` | 重构 |

## 提交信息格式

```
类型: 简短描述（50 字以内）

可选详细说明，每行不超过 72 字。
```

类型：`feat`、`fix`、`refactor`、`docs`、`test`、`chore`。

## PR 流程

1. 从 `main` 创建功能分支
2. 确保 `cargo clippy` 和 `cargo test --lib` 通过
3. 创建 PR，填写 PR 模板
4. 等待 CI 通过后合并
