<div align="center">

# 🧩 MimiSpec

**A high-density intent description language for human-AI collaboration**

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
echo "func Hello: steps:\n    say hi" | mimispec - --ast  # stdin
mimispec *.mms --json                     # multiple files
```

### Library Usage

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
│       ├── mod.rs               # Public API (parse, tokenize)
│       ├── ast.rs               # AST types
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
| [Syntax Specification](docs/specification.md) | Full language reference (1329 lines) |
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
A: `.mms` (MimiSpec) is the **intent design layer** — progressive, human-readable, fragment-friendly. `.mimi` (Mimi) is the **production compile target** — contract-verified, LLVM-compiled, with structured concurrency and linear capabilities.

**Q: Is this ready for production?**  
A: The parser (v0.1) is fully functional — published on [crates.io](https://crates.io/crates/mimispec) with 77 unit tests passing, error recovery, and complete AST rendering. CLI binary installable via `cargo install mimispec`. Production tooling (cross-file linking, LSP, Mimi compilation) is on the roadmap.

**Q: How do I contribute?**  
A: See [CONTRIBUTING.md](CONTRIBUTING.md). All contributions — code, docs, issues — are welcome.

---

## 🔒 Security

Please report security vulnerabilities to **ontonous@gmail.com**.  
See [SECURITY.md](SECURITY.md) for details.

---

## 📄 License

Apache 2.0 © 2026 ontonous. See [LICENSE](LICENSE).
