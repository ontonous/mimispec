<div align="center">

# 🧩 MimiSpec

**一门高信息密度的意图描述语言，专为人-AI 协作设计**

[![CI](https://github.com/ontonous/mimispec/actions/workflows/ci.yml/badge.svg)](https://github.com/ontonous/mimispec/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/mimispec)](https://crates.io/crates/mimispec)
[![Downloads](https://img.shields.io/crates/d/mimispec)](https://crates.io/crates/mimispec)
[![docs.rs](https://img.shields.io/docsrs/mimispec)](https://docs.rs/mimispec)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-blueviolet)](https://www.rust-lang.org)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](CONTRIBUTING.md)

</div>

---

> 发布状态：crates.io、Cargo 与最新发布 tag 仍为 `0.2.1`。当前 main 的技术状态
> 已可作为 `0.3.0-dev` 开发快照候选，供源码或二进制试用：合并版 0.3 Core、
> stdio 语言服务器、规范符合性套件与 package 门禁均已落地。它还不是
> `0.3.0-rc.1`：5 名独立作者/25 份文档的试用门禁仍为空，任何版本变更、tag
> 或实际发布仍需单独明确执行。

MimiSpec 将"不确定 → 部分结构化 → 完整锁定"的渐进式工作流嵌入语法本身。每个阶段都是**语法合法的 `.mms` 文件**。

```
// 阶段 1：原始意图，完全交由 AI 处理
module?? Shop:
    type?? Order:
        desc?? "订单数据，包含买家、商品、金额和状态"
    func?? Pay:
        steps:
            desc?? "检查余额"
            desc?? "扣款"

// 阶段 5：完全锁定的架构
module$ Shop:
    type$ OrderStatus: New | Pending | Paid | Shipped | Cancelled
    rule$ "支付必须幂等"
    func$ Pay(order, amount):
        requires$: order.status == Pending
        ensures$: order.status == Paid
        steps:
            check$ balance
            charge$ payment
            order.status$ = Paid >>> done
```

---

## ✨ 核心特性

| 特性 | 说明 |
|------|------|
| 🧩 **匿名 Context + Fragment** | 根 `desc`/`rule`/clause 与普通 Fragment 共用一份有序 Context Item 模型 |
| 📈 **渐进式精确** | `desc` → `requires`/`ensures` → `math:` 代码块，逐步细化 |
| 🔒 **承诺系统** | `$`/`$$` 表示已确认，`?`/`??` 表示不确定 — 共 9 种组合 |
| ⛓️ **约束链** | `rule` 前置附着，从文件级到函数级 |
| ➗ **结构化数学** | `math:` 代码块支持张量运算、线性代数、微积分 |
| 🎯 **开放世界 Flow** | 命名/匿名 `flow`、可选 `on Event` 与 `>>>` 转移 |
| 🖼️ **UI 视图** | `stack`/`parallel` 布局，`on` 事件绑定，Saga 补偿 |
| 🛡️ **错误恢复** | 多级同步并保留成功恢复的节点；调用方必须检查诊断后才能把 partial AST 当作完整文档 |
| 🛰️ **0.3 语言服务** | 长驻 stdio LSP，支持 advisory/strict revision、语义 token、hover、导航和 actor 声明编辑 |
| 🦀 **纯 Rust** | 零运行时依赖，单二进制 CLI |

---

## 🚀 快速开始

### 安装

```bash
# 从 crates.io 安装（CLI 工具）
cargo install mimispec

# 或作为库使用
cargo add mimispec

# 或从源码构建
git clone https://github.com/ontonous/mimispec.git
cd mimispec
cargo build --release
```

### 命令行使用

```bash
mimispec path/to/file.mms --ast           # 输出 AST
mimispec path/to/file.mms --json          # 输出 JSON（供 IDE 使用）
mimispec path/to/file.mms --render        # 渲染回源码
mimispec path/to/file.mms --latex         # 渲染数学为 LaTeX
mimispec diagnose path/to/file.mms        # 按 scope 分组的队列 + 意图诊断
mimispec diagnose --flat-queues path/to/file.mms  # 兼容的平铺队列
mimispec --json diagnose path/to/file.mms # mimispec.collaboration/0.3 封装
mimispec path/to/file.mms --diagnostics   # 同上
mimispec lsp --stdio                       # 长驻 LSP 3.17 服务
mimispec conformance check                # 验证 mimispec.conformance/0.3
mimispec usability check                  # 查看独立 RC 试用进度
echo "func Hello: steps:\n    say hi" | mimispec - --ast  # 标准输入
mimispec *.mms --json                     # 多文件
```

实验性目标与 provenance 命令不进入默认 Core 构建；研究和试验时需显式启用：

```bash
cargo run --features experimental-provenance -- provenance check sidecar.json --source-root /project
cargo run --features experimental-targets -- materialize path/to/file.mms --scope payments-v1
cargo run --features experimental-targets -- profile path/to/file.mms --target mimi
cargo run --features experimental-targets -- workflow path/to/file.mms --scope payments-v1
```

> 说明：当前 crates.io 发布版本仍是 `0.2.1`。无损解析、协作校验与 `diagnose`
> 已在 main 的 `0.3.0-dev` 快照候选中实现，但尚不是已发布 crate 契约。仅存在于
> `main` 的
> 由 feature 显式启用的 `materialize`、`profile`、`provenance` 与
> `workflow` 是 0.3 Core 路线之外的临时研究接口。

### 作为库使用

```toml
[dependencies]
mimispec = "0.2.1"
```

```rust
use mimispec::parse;

let source = r#"
type Status: Active | Inactive
func Toggle(user):
    requires: user.status in [Active, Inactive]
    steps:
        user.status = Inactive >>> done
"#;

let result = parse(source);
if result.errors.is_empty() {
    println!("{} fragments", result.file.fragments.len());
} else {
    for err in &result.errors {
        eprintln!("{}", mimispec::format::format_diagnostic(err, source));
    }
}

// 0.3.0-dev API（在 main 可用；不是已发布的 0.2.1 crate 契约）：
// let lossless = mimispec::parse_lossless(source);
// let report = mimispec::diagnostics::analyze_document(&lossless.document, &lossless.errors);
```

---

## 📖 语法预览

| 结构 | 示例 |
|------|------|
| 枚举 | `type Status: New \| Pending \| Paid` |
| 记录 | `type Order:\n    id: u64\n    status: Status` |
| 函数 | `func Pay(order):\n    requires: order.status == Pending\n    steps:\n        charge payment >>> done` |
| 状态机 | `flow Lifecycle:\n    New >>> Pending:\n    Pending >>> Paid:\n    Paid >>> Done:` |
| UI 视图 | `ui Panel binds Model:\n    stack:\n        "标题" on tap: DoSomething()` |
| 并行步骤 | `parasteps "加载数据":\n    load users\n    load orders` |
| 数学 | `math:\n    scores = Q @ K.T / sqrt(d_k)` |

完整语法：[docs/specification.md](docs/specification.md)

---

## 📁 项目结构

```
mimispec/
├── src/
│   ├── main.rs                  # CLI 入口
│   └── lib/
│       ├── mod.rs               # 公共 API（parse、parse_lossless、tokenize）
│       ├── ast.rs               # AST 类型定义
│       ├── collaboration.rs     # Actor 转移与 patch 校验（0.3.x）
│       ├── diagnostics.rs       # 决策/委托队列诊断（0.3.x）
│       ├── lossless.rs          # 可选无损源码层（0.3.x）
│       ├── error.rs             # 错误系统
│       ├── lexer.rs             # 词法分析器（indent/dedent）
│       ├── parser/
│       │   ├── mod.rs           # 解析器核心
│       │   ├── expr.rs          # Pratt 表达式解析器
│       │   ├── fragment.rs      # Fragment 分发
│       │   ├── func.rs          # FuncDef 解析器
│       │   ├── module.rs        # Module 解析器
│       │   ├── flow.rs          # FlowDef 解析器
│       │   ├── step.rs          # Step 解析器
│       │   ├── type.rs          # TypeDef 解析器
│       │   ├── ui.rs            # UiDef 解析器
│       │   └── rule.rs          # RuleDef 解析器
│       ├── render.rs            # AST → 源码渲染器
│       ├── render_util.rs       # 表达式优先级工具
│       ├── format.rs            # 诊断格式化器
│       └── latex.rs             # LaTeX 数学渲染器
├── docs/
│   ├── specification.md         # 语法规范
│   ├── roadmap-0.3.x.md         # 0.3.x 开发路线
│   ├── commitment-state-machine.md
│   ├── migration-0.2-to-0.3.md  # 迁移草案
│   ├── schemas/                 # 版本化 parse JSON + collaboration schema 草案
│   ├── advanced-usage.md        # 高级用法
│   ├── version-management.md    # 版本管理
│   └── stdlib-api.md            # 标准库参考
├── mimispec-parser-mms/         # 参考解析器（MimiSpec 自身编写）
├── editors/
│   ├── vscode/                  # VS Code 扩展
│   └── monaco/                  # Monaco 参考集成
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
└── AGENTS.md                    # AI 代理协作指南
```

---

## 📚 文档

| 文档 | 说明 |
|------|------|
| [语法规范草案](docs/specification.md) | 已实现的 `0.3.0-dev` Core 契约；已发布包仍为 0.2.1 |
| [0.3.x 开发路线](docs/roadmap-0.3.x.md) | 已完成技术里程碑、dev 快照边界与剩余 RC 门禁 |
| [0.3 可用性报告](docs/0.3-usability-report.md) | 技术快照就绪度与尚未完成的独立作者门禁 |
| [真实工程转写报告](docs/0.3-real-project-transcription-report.md) | 四份非平凡 MIMI 工程转写及作者/审阅体验结论 |
| [0.3 语言服务协议](docs/language-service-protocol-0.3.md) | 冻结的 stdio LSP custom methods、协作模式与错误码 |
| [0.3.x 关键设计总纲](docs/0.3.x-design-zh.md) | Context、描述、规则、条款、Flow 与 commitment 的正式 Core 基线 |
| [后缀状态机](docs/commitment-state-machine.md) | `$`/`?` 状态流、AI/人类权限与锁定挑战规范 |
| [高级用法](docs/advanced-usage.md) | 模块化设计、契约设计、Saga、ML 规格 |
| [版本管理](docs/version-management.md) | SemVer、分支模型、CI/CD |
| [标准库 API](docs/stdlib-api.md) | Mimi 运行时 16 模块参考 |
| [贡献指南](CONTRIBUTING.md) | 开发环境与 PR 流程 |
| [行为准则](CODE_OF_CONDUCT.md) | 社区行为准则 |
| [安全策略](SECURITY.md) | 安全漏洞报告 |

---

## 💻 编辑器支持

### VS Code

完整的扩展，支持 `.mms` 语法高亮和基于 CLI 的实时诊断：

```bash
cd editors/vscode
npm install && npm run compile
code --install-extension mimispec-vscode-*.vsix
```

### Monaco Editor

参考实现，包含 Monarch tokenizer 和补全提供者：

```ts
import { registerMimiSpecLanguage } from './mimispecLanguage';
registerMimiSpecLanguage(monaco);
```

详见 [editors/monaco/](editors/monaco/)。

---

## 🎯 设计哲学

> **从碎片到完整** — 碎片是起点，聚合是过程，完整是结果。

| 原则 |
|------|
| 不确定时写 `desc "..."`，AI 负责填充细节 |
| 模糊时加 `?`，锁定后加 `$` |
| 每个碎片都是一个合法的 `.mms` 文件 |
| 解析器不因"不完整"而拒绝 |

---

## ❓ 常见问题

**Q: MimiSpec 与 TypeSpec / Smithy / OpenAPI 有何不同？**  
A: MimiSpec 面向**人-AI 协作**，而不仅仅是 API 契约。其渐进式精确模型（`desc` → 结构化 → 锁定）和 Fragment 架构专为与 AI 伙伴的迭代式设计工作流而设计。

**Q: 我可以不使用 AI 而使用 MimiSpec 吗？**  
A: 可以。MimiSpec 是一门完全自包含的规范语言。AI 工具是可选层。

**Q: `.mms` 和 `.mimi` 有什么区别？**  
A: MimiSpec 与 Mimi 是两门相对独立的语言，拥有独立语法、AST、工具链和发布周期。`.mms` 是渐进式、自然语言友好的意图文件；`.mimi` 是可独立编译运行的 Typestate/Flow 系统语言。外部语言或文档容器可以把 MMS 文本交给 canonical MimiSpec parser，但外部包裹不重新定义 MimiSpec 的语法和语义。

**Q: 当前发布版本是什么？**
A: 当前已发布版本是 `v0.2.1`。main 已实现合并版 0.3 Core、无损文档模型、可执行的 commitment 协议、诊断与 stdio 语言服务，技术上可以切出 `0.3.0-dev` 评估快照。它不是 RC 或稳定版；独立作者试用仍为零，继续阻断 `0.3.0-rc.1`。

**Q: 如何贡献？**  
A: 参见 [CONTRIBUTING.md](CONTRIBUTING.md)。欢迎所有贡献 — 代码、文档、问题。

---

## 🔒 安全

请将安全漏洞发送至 **ontonous@gmail.com**。  
详见 [SECURITY.md](SECURITY.md)。

---

## 📄 许可证

Apache 2.0 © 2026 ontonous。详见 [LICENSE](LICENSE)。
