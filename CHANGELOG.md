# Changelog

## [Unreleased] - 0.3.x development

### Added
- Added orthogonal commitment semantics for lock intent, review intent, review
  target, content protection, delegation, and commit readiness.
- Added a shared Human/AI transition validator with explicit Human
  authorization and content-preserving AI lock challenge requirements.
- Added exhaustive nine-state semantic and actor transition matrix tests.
- Added Flow entry and Flow arm rule attachment parsing.
- Added an opt-in `parse_lossless()` API that keeps the exact source beside the
  semantic AST, including comments, whitespace, mixed newline kinds, physical
  paragraph breaks, line/column conversion, explicit commitment suffix spans,
  and source-derived rule group attachment candidates.
- Added parser-proven commitment slots, including zero-width insertion spans for
  uncommitted slots, and revision-local Fragment source nodes with header, core,
  and movable spans that include attached rule preludes.
- Added parser-authoritative rule occurrence records with unique revision-local
  IDs, exact spans, attached/environment/recovery decisions, target anchors,
  scope anchors, and Fragment target IDs when available.
- Extended lossless source nodes to nested `Field`, `FlowEntry`, `FlowArm`, and
  `Step` targets so attached rules and comments can resolve beyond top-level
  Fragments.
- Added line-comment attachment classification (`trailing` / `leading` /
  `free`) with optional target node IDs on the lossless document.

### Fixed
- Preserved commitment suffixes when rendering boolean values, `not`, `and`,
  `or`, `in`, comparison operators, Flow `>>>`, and Flow `requires` slots.
- Fixed the single-line Flow renderer placing the `>>>` suffix on the target
  identifier instead of the transition slot.
- Fixed capabilities consuming and rendering the same commitment suffix twice.
- Made the CI stress job execute the actual stress tests and added a rustfmt
  gate.

### Documentation
- Clarified that the current released implementation is `0.2.1`; the existing
  advanced specification is a staged `0.3.x` design target, not a released
  `1.0.0-rc` grammar.
- Added the `0.3.x` development roadmap.
- Added the normative commitment state-machine design, including the rule that
  `?`/`??` after `$`/`$$` review the lock decision rather than the content.
- Defined content-preserving AI lock challenges (`$ -> $?`, `$$ -> $$?`) and
  separated commitment from implementation evidence.
- Clarified that MimiSpec and Mimi are independent languages. Mimi is an
  optional first-party native materialization target; Mimi's historical
  `mms {}` super-comment is not the production integration path.

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
- Stress test suite (1000 items)
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
