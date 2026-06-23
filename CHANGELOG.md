# Changelog

## [0.1.0] - 2026-06-23

### Added
- MimiSpec parser CLI — parses `.mms` files into AST, with `--ast`, `--json`, `--render`, `--latex` output modes
- Pratt (precedence climbing) expression parser supporting arithmetic, bitwise, comparison, and logical operators
- Indentation-based block syntax with full indent/dedent token handling
- Fragment-based architecture: `Module`, `TypeDef`, `Flow`, `Func`, `Ui`, `Steps`, `Expr`, `UiNode`, `Placeholder`
- `math:` blocks with tensor/linear algebra operations, scientific notation, and multi-dimensional subscript
- Error recovery at multiple levels: import, fragment boundary, nested block, block item
- Structured error system with `ErrorCode` enum and source-context diagnostic formatting
- Commitment system: `$`/`$$` lock and `?`/`??` uncertainty suffixes on identifiers, keywords, and strings
- `rule` constraint system with blank-line-delimited attachment chains
- `>>>` transition operator for state machine flows and step transitions
- Multi-line enum support (Scheme A, `|`-prefixed lines)
- `parasteps` for parallel step blocks
- AST → MimiSpec source round-trip renderer
- Lightweight LaTeX renderer for math expressions (MathJax/KaTeX compatible)
- `@import` cross-file directive
- `on` compensation blocks and `error` termination
- UI view definition with `stack`, `parallel`, leaf nodes, and `on` event bindings
- Fuzzy matching helpers (Levenshtein edit distance, suggestion finding)
- Stress test suite (1000 items, ignored by default)
- Complete reference documentation: `docs/specification.md`, `docs/advanced-usage.md`
- Mimi Standard Library API reference: `docs/stdlib-api.md`

### Changed
- `to` keyword replaced with `>>>` transition operator
- `rule` changed from independent Fragment to constraint modifier (front-attached)
- `desc` changed from modifier to independent entity
- Parser split into 9 sub-modules: `expr`, `flow`, `fragment`, `func`, `module`, `rule`, `step`, `type`, `ui`
- Error system restructured with `ErrorCode` enum and structured diagnostics
- `File` struct now includes `rules: Vec<RuleDef>` (global unattached rules)
- License changed to Apache 2.0 with copyright notice

### Fixed
- String escape sequences properly handled (`\n`, `\t`, `\r`, `\\`, `\"`)
- Unterminated string error pinned to opening quote line
- Subscript indices support multi-dimensional access (`x[i, j]`)
- All `cargo clippy` warnings eliminated
- Error recovery no longer hangs on invalid tokens
- `Action::Navigate` renderer emits `>>>` instead of `to`

### Removed
- `Fragment::Rule` variant (rule is no longer a standalone Fragment)
