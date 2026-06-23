# MimiSpec

> **高信息密度的意图描述语言** — 从碎片到完整的渐进式规范。
> **A high-density intent description language** — from fragments to full specifications.

MimiSpec 是一门意图描述语言，专为**人-AI 协作设计**，将"不确定 → 部分结构化 → 完整锁定"的渐进式工作流嵌入语法本身。
MimiSpec is an **intent description language for human-AI collaboration**, embedding a progressive workflow — from uncertainty to structured to fully locked — directly into the syntax.

```
// 阶段 1 / Phase 1: 纯意图，完全委托 AI / Raw intent, fully delegated to AI
module?? Shop:
    type?? Order:
        desc?? "订单数据，包含买家、商品、金额和状态"
    func?? Pay:
        steps:
            desc?? "检查余额"
            desc?? "扣款"

// 阶段 5 / Phase 5: 完整锁定架构 / Fully locked architecture
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

每个阶段都是**语法合法的 `.mms` 文件**。
Every phase is a **syntactically valid `.mms` file**.

---

## 核心特性 / Core Features

- **Agent-Native Design** — `.mms` as persistent intent artifact: `?` for exploration, `$` for lock, `rule` for guardrails, agent acts autonomously inside the fences. See [VISION.md](VISION.md)
- **Fragment Architecture** — `module`, `type`, `flow`, `func`, `ui`, `steps`, expressions, UI nodes... any syntax subtree is a valid top-level Fragment
- **Progressive Precision** — `desc` (natural language) → `requires`/`ensures` → `math:` blocks, step by step
- **Built-in Commitment System** — `$`/`$$` marks confirmed designs, `?`/`??` marks uncertainty
- **Constraint Chains** — `rule` front-attachment from file-level to function-level
- **Structured Math** — `math:` blocks with tensor operations, linear algebra, calculus
- **State Machine** — `flow` definitions with `>>>` transition operator and `requires` guards
- **UI Views** — `stack`/`parallel` layouts, `on` event bindings, Saga compensation
- **Error Recovery** — multi-level synchronization, never loses the AST from local errors
- **Pure Rust** — zero runtime dependencies, single binary CLI

---

## 快速开始 / Quick Start

### 安装 / Install

```bash
git clone https://github.com/ontonous/mimispec.git
cd mimispec
cargo build --release
```

### 命令行使用 / CLI Usage

```bash
mimispec path/to/file.mms --ast           # 输出 AST / dump AST
mimispec path/to/file.mms --json          # 输出 JSON / JSON output (for IDE)
mimispec path/to/file.mms --render        # 渲染回源码 / render back to source
mimispec path/to/file.mms --latex         # 渲染数学为 LaTeX / render math to LaTeX
echo "func Hello: steps:\n    say hi" | mimispec - --ast  # 标准输入 / stdin
mimispec *.mms --json                     # 多文件 / multiple files
```

### 作为库使用 / Library Usage

```toml
[dependencies]
mimispec = { git = "https://github.com/ontonous/mimispec" }
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
```

---

## 语法预览 / Syntax Preview

| 结构 / Structure | 示例 / Example |
|------|------|
| Enum | `type Status: New \| Pending \| Paid` |
| Record | `type Order:\n    id: u64\n    status: Status` |
| Function | `func Pay(order):\n    requires: order.status == Pending\n    steps:\n        charge payment >>> done` |
| State Machine | `flow Lifecycle:\n    New >>> Pending:\n    Pending >>> Paid:\n    Paid >>> Done:` |
| UI View | `ui Panel binds Model:\n    stack:\n        "标题" on tap: DoSomething()` |
| Parallel | `parasteps "加载数据":\n    load users\n    load orders` |
| Math | `math:\n    scores = Q @ K.T / sqrt(d_k)` |

详见完整语法规范 / Full syntax: [docs/specification.md](docs/specification.md)

---

## 项目结构 / Project Structure

```
mimispec/
├── src/
│   ├── main.rs                  # CLI 入口 / CLI entry
│   └── lib/
│       ├── mod.rs               # 公共 API / public API (parse, tokenize)
│       ├── ast.rs               # AST 类型定义 / AST types
│       ├── error.rs             # 错误系统 / error system
│       ├── lexer.rs             # 词法分析器 / lexer (indent/dedent)
│       ├── parser/
│       │   ├── mod.rs           # 解析器核心 / parser core
│       │   ├── expr.rs          # Pratt 表达式解析器 / Pratt expression parser
│       │   ├── fragment.rs      # Fragment 分发 / fragment dispatch
│       │   ├── func.rs          # FuncDef 解析 / FuncDef parser
│       │   ├── module.rs        # Module 解析 / Module parser
│       │   ├── flow.rs          # FlowDef 解析 / FlowDef parser
│       │   ├── step.rs          # Step 解析 / Step parser
│       │   ├── type.rs          # TypeDef 解析 / TypeDef parser
│       │   ├── ui.rs            # UiDef 解析 / UiDef parser
│       │   └── rule.rs          # RuleDef 解析 / RuleDef parser
│       ├── render.rs            # AST → 源码渲染 / AST → source renderer
│       ├── render_util.rs       # 表达式优先级工具 / expression precedence
│       ├── format.rs            # 诊断格式化 / diagnostic formatter
│       └── latex.rs             # LaTeX 数学渲染 / LaTeX math renderer
├── docs/
│   ├── specification.md         # 语法规范 / syntax specification
│   ├── advanced-usage.md        # 高级用法 / advanced usage
│   ├── version-management.md    # 版本管理 / version management
│   └── stdlib-api.md            # 标准库参考 / stdlib API reference
├── mimispec-parser-mms/         # MimiSpec 写的参考解析器 / reference parser in MimiSpec
├── editors/vscode/              # VS Code 扩展 / VS Code extension
├── editors/monaco/              # Monaco 参考集成 / Monaco reference integration
├── VISION.md                    # Agent 愿景 / Agent vision
├── CHANGELOG.md
├── CONTRIBUTING.md
└── AGENTS.md                    # AI 代理协作指南 / AI agent guide
```

---

## 文档 / Documentation

| 文档 / Doc | 说明 / Description |
|------|------|
| [Agent 愿景 / Vision](VISION.md) | 通用 agent 的意图接口哲学 / The intent interface philosophy for universal agents |
| [语法规范](docs/specification.md) | 完整语言参考 / Complete language reference |
| [高级用法](docs/advanced-usage.md) | 模块化、契约设计、Saga、ML 规格 / Modular design, contracts, Saga, ML specs |
| [版本管理](docs/version-management.md) | SemVer、分支模型、CI/CD / Versioning, branching, CI/CD |
| [标准库 API](docs/stdlib-api.md) | Mimi 运行时 16 模块参考 / Mimi runtime 16-module reference |
| [贡献指南](CONTRIBUTING.md) | 开发环境与 PR 流程 / Dev environment & PR workflow |

---

## VS Code 扩展 / VS Code Extension

完整 VS Code 扩展，提供 `.mms` 语法高亮和 CLI 驱动的实时诊断：
A complete VS Code extension with `.mms` syntax highlighting and CLI-driven diagnostics:

```bash
cd editors/vscode
npm install
npm run compile
code --install-extension editors/vscode/mimispec-vscode-*.vsix
```

详见 [editors/vscode/](editors/vscode/)。

---

## 设计哲学 / Design Philosophy

> **From Scratch to Full** — 碎片是起点，聚合是过程，完整是结果。
> Fragments are the starting point, aggregation is the process, completeness is the result.

- 不确定时写 `desc "..."`，AI 负责填充细节 / Write `desc "..."` when uncertain, AI fills the details
- 模糊时加 `?`，锁定后加 `$` / Add `?` for ambiguity, `$` when locked
- 碎片阶段就是一个合法的 `.mms` 文件 / Every fragment is a valid `.mms` file
- 解析器不因"不完整"而拒绝 / The parser never rejects for incompleteness

---

## 许可证 / License

Apache 2.0 © 2026 ontonous
