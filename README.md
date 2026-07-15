<div align="center">

# 🧩 MimiSpec

**A high-density intent description language for human-AI collaboration**

[![CI](https://github.com/ontonous/mimispec/actions/workflows/ci.yml/badge.svg)](https://github.com/ontonous/mimispec/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/mimispec)](https://crates.io/crates/mimispec)
[![Downloads](https://img.shields.io/crates/d/mimispec)](https://crates.io/crates/mimispec)
[![docs.rs](https://docs.rs/mimispec/badge.svg)](https://docs.rs/mimispec)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-blueviolet)](https://www.rust-lang.org)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](CONTRIBUTING.md)

</div>

---

MimiSpec embeds a **progressive workflow** — from uncertainty to structured to fully locked — directly into the syntax. Every phase of design is a **syntactically valid `.mms` file**.

```
// Phase 1: Raw intent, fully delegated to AI
module?? Shop:
    type?? Order:
        desc?? "Order data, including buyer, product, amount, and status"
    func?? Pay:
        steps:
            desc?? "Check balance"
            desc?? "Deduct payment"

// Phase 5: Fully locked architecture
module$ Shop:
    type$ OrderStatus: New | Pending | Paid | Shipped | Cancelled
    rule$ "Payment must be idempotent"
    func$ Pay(order, amount):
        requires$: order.status == Pending
        ensures$: order.status == Paid
        steps:
            check$ balance
            charge$ payment
            order.status$ = Paid >>> done
```

---

## ✨ Core Features

| Feature | Description |
|---------|-------------|
| 🧩 **Fragment Architecture** | Multiple meaningful local structures are valid top-level Fragments — `module`, `type`, `flow`, `func`, `ui`, `steps`, expressions, UI nodes |
| 📈 **Progressive Precision** | `desc` → `requires`/`ensures` → `math:` blocks, step by step |
| 🔒 **Commitment System** | `$`/`$$` for confirmed, `?`/`??` for uncertainty — 9 combinations |
| ⛓️ **Constraint Chains** | `rule` front-attachment from file-level to function-level |
| ➗ **Structured Math** | `math:` blocks with tensor ops, linear algebra, calculus |
| 🎯 **State Machine** | `flow` definitions with `>>>` transition operator |
| 🖼️ **UI Views** | `stack`/`parallel` layouts, `on` event bindings, Saga compensation |
| 🛡️ **Error Recovery** | Multi-level synchronization preserves successfully recovered nodes; callers must check diagnostics before treating a partial AST as complete |
| 🦀 **Pure Rust** | Zero runtime dependencies, single binary CLI |

---

## 🚀 Quick Start

### Install

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

### CLI Usage

```bash
mimispec path/to/file.mms --ast           # dump AST
mimispec path/to/file.mms --json          # JSON output (for IDE)
mimispec path/to/file.mms --render        # render back to source
mimispec path/to/file.mms --latex         # render math to LaTeX
mimispec diagnose path/to/file.mms        # decision/delegation queues + intent diagnostics
mimispec path/to/file.mms --diagnostics   # same as diagnose
echo "func Hello: steps:\n    say hi" | mimispec - --ast  # stdin
mimispec *.mms --json                     # multiple files
```

> Note: the current crates.io release is still `0.2.1`. Lossless parsing,
> collaboration validation, and `diagnose` are under development on `main` for
> the `0.3.x` series and are not yet a published release contract.

### Library Usage

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

// 0.3.x development APIs (available on main; not the published 0.2.1 crate):
// let lossless = mimispec::parse_lossless(source);
// let report = mimispec::diagnostics::analyze_document(&lossless.document, &lossless.errors);
```

---

## 📖 Syntax Preview

| Structure | Example |
|-----------|---------|
| Enum | `type Status: New \| Pending \| Paid` |
| Record | `type Order:\n    id: u64\n    status: Status` |
| Function | `func Pay(order):\n    requires: order.status == Pending\n    steps:\n        charge payment >>> done` |
| State Machine | `flow Lifecycle:\n    New >>> Pending:\n    Pending >>> Paid:\n    Paid >>> Done:` |
| UI View | `ui Panel binds Model:\n    stack:\n        "Title" on tap: DoSomething()` |
| Parallel | `parasteps "Load Data":\n    load users\n    load orders` |
| Math | `math:\n    scores = Q @ K.T / sqrt(d_k)` |

Full syntax: [docs/specification.md](docs/specification.md)

---

## 📁 Project Structure

```
mimispec/
├── src/
│   ├── main.rs                  # CLI entry
│   └── lib/
│       ├── mod.rs               # Public API (parse, parse_lossless, tokenize)
│       ├── ast.rs               # AST types
│       ├── collaboration.rs     # Actor transitions, patch validation (0.3.x)
│       ├── diagnostics.rs       # Decision/delegation queues (0.3.x)
│       ├── lossless.rs          # Opt-in source map layer (0.3.x)
│       ├── error.rs             # Error system
│       ├── lexer.rs             # Lexer (indent/dedent)
│       ├── parser/
│       │   ├── mod.rs           # Parser core
│       │   ├── expr.rs          # Pratt expression parser
│       │   ├── fragment.rs      # Fragment dispatch
│       │   ├── func.rs          # FuncDef parser
│       │   ├── module.rs        # Module parser
│       │   ├── flow.rs          # FlowDef parser
│       │   ├── step.rs          # Step parser
│       │   ├── type.rs          # TypeDef parser
│       │   ├── ui.rs            # UiDef parser
│       │   └── rule.rs          # RuleDef parser
│       ├── render.rs            # AST → source renderer
│       ├── render_util.rs       # Expression precedence utils
│       ├── format.rs            # Diagnostic formatter
│       └── latex.rs             # LaTeX math renderer
├── docs/
│   ├── specification.md         # Syntax specification
│   ├── roadmap-0.3.x.md         # 0.3.x development roadmap
│   ├── commitment-state-machine.md
│   ├── advanced-usage.md        # Advanced usage
│   ├── version-management.md    # Version management
│   └── stdlib-api.md            # Stdlib API reference
├── mimispec-parser-mms/         # Reference parser in MimiSpec
├── editors/
│   ├── vscode/                  # VS Code extension
│   └── monaco/                  # Monaco reference integration
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
└── AGENTS.md                    # AI agent guide
```

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Syntax Specification Draft](docs/specification.md) | Current syntax plus staged 0.3.x design; released implementation remains 0.2.1 |
| [0.3.x Roadmap](docs/roadmap-0.3.x.md) | Version plan from collaboration semantics to the native Mimi profile |
| [0.3.x Chinese Design Overview](docs/0.3.x-design-zh.md) | Chinese architecture baseline for states, paragraphs, materialization, evidence, and Mimi |
| [Commitment State Machine](docs/commitment-state-machine.md) | Normative `$`/`?` transitions, actor permissions, and lock challenges |
| [Advanced Usage](docs/advanced-usage.md) | Modular design, contracts, Saga, ML specs |
| [Version Management](docs/version-management.md) | SemVer, branching model, CI/CD |
| [Stdlib API](docs/stdlib-api.md) | Mimi runtime 16-module reference |
| [Contribution Guide](CONTRIBUTING.md) | Dev environment & PR workflow |
| [Code of Conduct](CODE_OF_CONDUCT.md) | Community guidelines |
| [Security Policy](SECURITY.md) | Vulnerability reporting |

---

## 💻 Editor Support

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

## 🎯 Design Philosophy

> **From Scratch to Full** — Fragments are the starting point, aggregation is the process, completeness is the result.

| Principle |
|-----------|
| Write `desc "..."` when uncertain, AI fills the details |
| Add `?` for ambiguity, `$` when locked |
| Every fragment is a valid `.mms` file |
| The parser never rejects for incompleteness |

---

## ❓ FAQ

**Q: How is MimiSpec different from TypeSpec / Smithy / OpenAPI?**  
A: MimiSpec targets **human-AI collaboration**, not just API contracts. Its progressive precision model (`desc` → structured → locked) and Fragment architecture are designed for iterative design workflows with AI partners.

**Q: Can I use MimiSpec without AI?**  
A: Yes. MimiSpec is a fully self-contained specification language. AI tooling is an optional layer.

**Q: What is the difference between `.mms` and `.mimi`?**  
A: MimiSpec and Mimi are separate languages with independent syntax, ASTs, toolchains, and release cycles. `.mms` is a progressive, natural-language-friendly intent format; `.mimi` is an independently usable Typestate/Flow systems language. Mimi is the first-party native materialization target, not a mandatory backend for every MMS. Mimi's `mms {}` block is a historical super-comment skipped by the production compiler pipeline, not a production-grade embedded MimiSpec system.

**Q: What is the current released version?**
A: The current release is `v0.2.1`. Parsing, cross-file resolution, symbol tables, incremental caching, and the CLI are available. The `0.3.x` series will implement the commitment collaboration protocol, IDE services, materialization evidence, and the native Mimi profile; forward-looking specification text is not yet released behavior.

**Q: How do I contribute?**  
A: See [CONTRIBUTING.md](CONTRIBUTING.md). All contributions — code, docs, issues — are welcome.

---

## 🔒 Security

Please report security vulnerabilities to **ontonous@gmail.com**.  
See [SECURITY.md](SECURITY.md) for details.

---

## 📄 License

Apache 2.0 © 2026 ontonous. See [LICENSE](LICENSE).
