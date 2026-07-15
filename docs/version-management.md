# MimiSpec 版本管理规范

## 1. 版本号规则

本项目遵循 **语义化版本 2.0.0** (SemVer)：

```
主版本.次版本.补丁 [-预发布号]
```

| 位 | 何时递增 | 示例 |
|---|----------|------|
| **主版本** | 破坏性 API 变更、AST 不兼容、语法不向下兼容 | `2.0.0` |
| **次版本** | 新增向后兼容的功能、语法扩展、新 Fragment 类型 | `1.3.0` |
| **补丁** | 向后兼容的 bug 修复、性能优化、文档改进 | `1.0.1` |
| **预发布号** | RC / beta / alpha | `1.0.0-rc.1` |

### 1.1 版本号判定标准

**主版本递增条件（任一触发）**：
- AST 类型定义（`ast.rs` 中的 enum / struct）发生不向下兼容的变更
- 解析器入口函数签名变更（`parse`、`parse_fragment`、`tokenize`）
- 关键字删除或语义反转
- CLI 选项删除或行为不兼容变更
- 缩进规则变更

**次版本递增条件（任一触发）**：
- 新增关键字或语法结构（不破坏现有解析）
- AST 新增枚举变体（`#[non_exhaustive]` 保护下安全）
- 新增 CLI 选项
- 新的 Fragment 类型
- 新增公共 API 函数

**补丁递增条件**：
- Bug 修复
- 文档更新
- 测试新增或改进
- 重构（不影响公共 API）
- 依赖更新

### 1.2 预发布与候选版本

| 后缀 | 含义 | 示例 |
|------|------|------|
| `-alpha.N` | 内部实验版，可能大幅变更 | `1.0.0-alpha.1` |
| `-beta.N` | 功能冻结，主要 bug 修复 | `1.0.0-beta.2` |
| `-rc.N` | 发布候选，仅关键修复 | `1.0.0-rc.3` |

预发布版本**优先级低于**正式版本：`1.0.0-alpha < 1.0.0-rc.1 < 1.0.0`。

---

## 2. 分支模型

采用 **Trunk-Based Development**（主干开发），保持极简分支策略：

```
main ───── feature/A ──── main
    \                  /
     └── feature/B ──┘
```

### 2.1 分支规则

| 分支 | 用途 | 来源 | 合并目标 |
|------|------|------|---------|
| `main` | 稳定发布版。始终可构建、可部署 | — | — |
| `fix/*` | Bug 修复 | `main` | `main` |
| `feat/*` | 新功能开发 | `main` | `main` |
| `docs/*` | 文档专用 | `main` | `main` |

- 功能分支应**短命**（存活不超过 1 周）
- 合并前必须通过 CI
- 禁止直接向 `main` 推送（通过 PR 合并）
- 分支命名示例：`fix/lexer-string-escape`、`feat/flow-multi-arm`、`docs/api-refactor`

### 2.2 发布分支（必要时）

当需要为旧版本提供补丁时，创建 `release/vN.x` 分支：

```
main ── v1.0.0 ── v1.0.1 ── v1.1.0 ── ...
        │                    ↑
        └── release/v1.x ────┘ (hotfix backport)
```

---

## 3. 发布流程

### 3.1 标准化发布步骤

```
1. 从 main 创建 release/vX.Y.Z 分支
2. 更新 CHANGELOG.md
3. 更新 Cargo.toml 中的 version 字段
4. 运行完整测试套件：cargo test --lib && cargo clippy
5. 运行压力测试：cargo test --release stress_tests
6. 创建 Git Tag：git tag -a vX.Y.Z -m "vX.Y.Z"
7. 发布到 GitHub Releases
8. 合并回 main（如有必要）
```

### 3.2 标签规范

```
v1.0.0          # 正式发布
v1.0.0-rc.1     # 候选发布
v1.0.0-beta.1   # Beta 版
```

标签使用带注释的标签（annotated tag），包含发布说明摘要。

### 3.3 预发布流程

```bash
# RC 示例
cargo bump 1.0.0-rc.1    # 更新 Cargo.toml
git tag -a v1.0.0-rc.1 -m "v1.0.0-rc.1: 语法规范冻结，修复已知解析错误"
git push origin v1.0.0-rc.1
```

---

## 4. 变更日志规范

`CHANGELOG.md` 遵循 [Keep a Changelog](https://keepachangelog.com/) 格式：

```markdown
## [1.2.0] - 2026-07-15

### Added
- 新增 `parasteps` 关键字支持并行步骤

### Changed
- `to` 操作符替换为 `>>>` 转移操作符

### Fixed
- 修复字符串转义序列中反斜杠处理
- 修复多层缩进块中错误恢复导致无限循环

### Removed
- 移除 `Fragment::Rule` 变体（rule 不再是独立 Fragment）
```

### 4.1 变更分类

| 类别 | 说明 |
|------|------|
| `Added` | 新功能、新语法、新 API |
| `Changed` | 现有功能变更、性能优化 |
| `Fixed` | Bug 修复 |
| `Removed` | 已弃用功能的移除 |
| `Deprecated` | 即将移除的功能标记 |
| `Security` | 安全修复 |

---

## 5. CI/CD 管道（建议方案）

当前项目**尚未配置 CI/CD**。以下是推荐的最低配置：

### 5.1 GitHub Actions 工作流

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo clippy -- -D warnings
      - run: cargo test --lib
      - run: cargo test --release stress_tests
```

### 5.2 CI 阶段

| 阶段 | 命令 | 超时 | 说明 |
|------|------|------|------|
| Lint | `cargo clippy -- -D warnings` | 5m | 零警告策略，将警告视为错误 |
| 单元测试 | `cargo test --lib` | 5m | 运行所有 75+ 个单元测试 |
| 集成测试 | `cargo test` | 5m | 含 bin 测试 |
| 压力测试 | `cargo test --release stress_tests` | 30m | 1000 个 items 的大文件测试 |
| 构建 | `cargo build --release` | 10m | 验证发布构建 |

### 5.3 自动发布（建议）

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ["v*"]
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - run: cargo test --lib
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          body_path: CHANGELOG.md
          files: target/release/mimispec
```

---

## 6. 当前管理状态 / Current Management Status

### 6.1 版本管理 / Version Management

| 评估项 | 当前状态 | 等级 |
|--------|----------|------|
| 版本号 | `Cargo.toml` 中定义 `0.1.0`，符合 SemVer | ✅ |
| Git Tag | **无任何 tag**，无法追溯发布历史 | ❌ |
| CHANGELOG | 完整记录 v0.1.0 变更，Keep a Changelog 格式 | ✅ |
| 发布流程 | **无**，靠手动操作 | ❌ |

### 6.2 分支管理 / Branch Management

| 评估项 | 当前状态 | 等级 |
|--------|----------|------|
| 分支策略 | Trunk-Based（`main` 单分支） | ⚠️ |
| PR 审查 | `.github/PULL_REQUEST_TEMPLATE.md` 已建立 | ✅ |
| 分支命名规范 | `feat/*`, `fix/*`, `docs/*`, `refactor/*`, `chore/*` | ✅ |

### 6.3 CI/CD 现状

| 评估项 | 当前状态 | 等级 |
|--------|----------|------|
| 持续集成 (CI) | `.github/workflows/ci.yml` 已配置（clippy + test + release build） | ✅ |
| 持续发布 (CD) | **不存在**。发布需手动构建、手动上传 | ❌ |
| PR 自动检查 | CI 自动触发 | ✅ |
| 自动发布 | **不存在**。tag 推送后无任何自动化 | ❌ |

### 6.4 文档与规范现状

| 评估项 | 当前状态 | 等级 |
|--------|----------|------|
| 版本管理规范 | 本文件已定义完整版本管理策略 | ✅ |
| 贡献指南 | `CONTRIBUTING.md` 已建立 | ✅ |
| 代码规范 | `AGENTS.md` + `CONTRIBUTING.md` 完整定义 | ✅ |
| 测试规范 | `AGENTS.md` §6 明确定义测试分类与工作流 | ✅ |
| 安全策略 | `SECURITY.md` 已建立 | ✅ |

### 6.5 优先级建议

| 优先级 | 事项 | 预估工作量 |
|--------|------|-----------|
| **P0** | 创建 `v0.1.0` Git Tag | 1 分钟 |
| **P1** | 添加自动发布 workflow（tag 触发） | 30 分钟 |
| **P3** | 设置 cargo-release / release-plz 自动化版本管理 | 1 小时 |

---

## 7. 工具推荐

| 工具 | 用途 | 安装 |
|------|------|------|
| [cargo-release](https://github.com/crate-ci/cargo-release) | 自动化版本发布 | `cargo install cargo-release` |
| [cargo-bump](https://crates.io/crates/cargo-bump) | 版本号递增 | `cargo install cargo-bump` |
| [release-plz](https://github.com/MarcoIeni/release-plz) | 基于 CI 的自动发布 | GitHub Action |
| [git-cliff](https://github.com/orhun/git-cliff) | 从 Git 日志生成 CHANGELOG | `cargo install git-cliff` |
