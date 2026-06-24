<div align="center">

# 🧩 MimiSpec

**A high-density intent description language for human-AI collaboration**  
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

MimiSpec embeds a **progressive workflow** — from uncertainty to structured to fully locked — directly into the syntax. Every phase of design is a **syntactically valid `.mms` file**.

MimiSpec 将"不确定 → 部分结构化 → 完整锁定"的渐进式工作流嵌入语法本身。每个阶段都是**语法合法的 `.mms` 文件**。

```
// Phase 1: Raw intent, fully delegated to AI
module?? Shop:
    type?? Order:
        desc?? "订单数据，包含买家、商品、金额和状态"
    func?? Pay:
        steps:
            desc?? "检查余额"
            desc?? "扣款"

// Phase 5: Fully locked architecture
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

## ✨ Core Features / 核心特性

| Feature | Description |
|---------|-------------|
| 🧩 **Fragment Architecture** | Any syntax subtree is a valid top-level Fragment — `module`, `type`, `flow`, `func`, `ui`, `steps`, expressions, UI nodes |
| 📈 **Progressive Precision** | `desc` → `requires`/`ensures` → `math:` blocks, step by step |
| 🔒 **Commitment System** | `$`/`$$` for confirmed, `?`/`??` for uncertainty — 9 combinations |
| ⛓️ **Constraint Chains** | `rule` front-attachment from file-level to function-level |
| ➗ **Structured Math** | `math:` blocks with tensor ops, linear algebra, calculus |
| 🎯 **State Machine** | `flow` definitions with `>>>` transition operator |
| 🖼️ **UI Views** | `stack`/`parallel` layouts, `on` event bindings, Saga compensation |
| 🛡️ **Error Recovery** | Multi-level synchronization, never loses the AST |
| 🦀 **Pure Rust** | Zero runtime dependencies, single binary CLI |

---

## 🚀 Quick Start / 快速开始

### Install / 安装

```bash
# From crates.io (CLI tool)
cargo install mimispec

# Or as a library
cargo add mimispec

# Or build from source
git clone https://github.com/ontonous/mimispec.git
cd mimispec
cargo build --release
```

### CLI Usage / 命令行使用

```bash
mimispec path/to/file.mms --ast           # 输出 AST / dump AST
mimispec path/to/file.mms --json          # 输出 JSON / JSON output (for IDE)
mimispec path/to/file.mms --render        # 渲染回源码 / render back to source
mimispec path/to/file.mms --latex         # 渲染数学为 LaTeX / render math to LaTeX
echo "func Hello: steps:\n    say hi" | mimispec - --ast  # 标准输入 / stdin
mimispec *.mms --json                     # 多文件 / multiple files
```

### Library Usage / 作为库使用

```toml
[dependencies]
mimispec = "0.1"
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

## 📖 Syntax Preview / 语法预览

| Structure | Example |
|-----------|---------|
| Enum | `type Status: New \| Pending \| Paid` |
| Record | `type Order:\n    id: u64\n    status: Status` |
| Function | `func Pay(order):\n    requires: order.status == Pending\n    steps:\n        charge payment >>> done` |
| State Machine | `flow Lifecycle:\n    New >>> Pending:\n    Pending >>> Paid:\n    Paid >>> Done:` |
| UI View | `ui Panel binds Model:\n    stack:\n        "标题" on tap: DoSomething()` |
| Parallel | `parasteps "加载数据":\n    load users\n    load orders` |
| Math | `math:\n    scores = Q @ K.T / sqrt(d_k)` |

Full syntax: [docs/specification.md](docs/specification.md)

---

## 📁 Project Structure / 项目结构

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
├── mimispec-parser-mms/         # 参考解析器 (MimiSpec 自身编写) / reference parser in MimiSpec
├── editors/
│   ├── vscode/                  # VS Code 扩展 / VS Code extension
│   └── monaco/                  # Monaco 参考集成 / Monaco reference integration
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
└── AGENTS.md                    # AI 代理协作指南 / AI agent guide
```

---

## 📚 Documentation / 文档

| Document | Description |
|----------|-------------|
| [Syntax Specification](docs/specification.md) | Full language reference / 完整语言参考 (1329 lines) |
| [Advanced Usage](docs/advanced-usage.md) | Modular design, contracts, Saga, ML specs / 模块化、契约设计、Saga、ML 规格 |
| [Version Management](docs/version-management.md) | SemVer, branching model, CI/CD / 版本管理、分支模型、CI/CD |
| [Stdlib API](docs/stdlib-api.md) | Mimi runtime 16-module reference / Mimi 运行时 16 模块参考 |
| [Contribution Guide](CONTRIBUTING.md) | Dev environment & PR workflow / 开发环境与 PR 流程 |
| [Code of Conduct](CODE_OF_CONDUCT.md) | Community guidelines / 社区行为准则 |
| [Security Policy](SECURITY.md) | Vulnerability reporting / 安全漏洞报告 |

---

## 💻 Editor Support / 编辑器支持

### VS Code

Full extension with `.mms` syntax highlighting and CLI-driven live diagnostics:

```bash
cd editors/vscode
npm install && npm run compile
code --install-extension mimispec-vscode-*.vsix
```

### Monaco Editor

Reference implementation with Monarch tokenizer and completion provider:

```ts
import { registerMimiSpecLanguage } from './mimispecLanguage';
registerMimiSpecLanguage(monaco);
```

See [editors/monaco/](editors/monaco/) for details.

---

## 🎯 Design Philosophy / 设计哲学

> **From Scratch to Full** — 碎片是起点，聚合是过程，完整是结果。  
> Fragments are the starting point, aggregation is the process, completeness is the result.

| Principle | 原则 |
|-----------|------|
| Write `desc "..."` when uncertain, AI fills the details | 不确定时写 `desc "..."`，AI 负责填充细节 |
| Add `?` for ambiguity, `$` when locked | 模糊时加 `?`，锁定后加 `$` |
| Every fragment is a valid `.mms` file | 碎片阶段就是一个合法的 `.mms` 文件 |
| The parser never rejects for incompleteness | 解析器不因"不完整"而拒绝 |

---

## ❓ FAQ / 常见问题

**Q: How is MimiSpec different from TypeSpec / Smithy / OpenAPI?**  
A: MimiSpec targets **human-AI collaboration**, not just API contracts. Its progressive precision model (`desc` → structured → locked) and Fragment architecture are designed for iterative design workflows with AI partners.

**Q: Can I use MimiSpec without AI?**  
A: Yes. MimiSpec is a fully self-contained specification language. AI tooling is an optional layer.

**Q: What is the difference between `.mms` and `.mimi`?**  
A: `.mms` (MimiSpec) is the **intent design layer** — progressive, human-readable, fragment-friendly. `.mimi` (Mimi) is the **production compile target** — contract-verified, LLVM-compiled, with structured concurrency and linear capabilities.

**Q: Is this ready for production?**  
A: The parser (v0.1) is fully functional — published on [crates.io](https://crates.io/crates/mimispec) with 77 unit tests passing, error recovery, and complete AST rendering. CLI binary installable via `cargo install mimispec`. Production tooling (cross-file linking, LSP, Mimi compilation) is on the roadmap.

**Q: How do I contribute?**  
A: See [CONTRIBUTING.md](CONTRIBUTING.md). All contributions — code, docs, issues — are welcome.

---

## 🔒 Security / 安全

Please report security vulnerabilities to **ontonous@gmail.com**.  
See [SECURITY.md](SECURITY.md) for details.

---

## 📄 License / 许可证

Apache 2.0 © 2026 ontonous. See [LICENSE](LICENSE).
