# MimiSpec 0.3.x Public API and Diagnostic-Code Stability

> Status: consolidated 0.3.0 M5 stabilization deliverable.
>
> This document freezes the public Rust API surface and diagnostic-code
> assignments that may not change in a breaking way within the 0.3.x series
> without an explicit migration note in `CHANGELOG.md` and a corresponding
> `docs/migration-*.md` entry.
>
> Everything outside this document is experimental and may evolve between
> minor 0.3.x releases.

## 1. Stability Tiers

### Tier 1 — Frozen for 0.3.x

These items are the canonical parser contract. A breaking change to any of
them is a 0.3.x release blocker and requires:

1. a `CHANGELOG.md` entry under `### Changed` (not `### Added`);
2. a migration note in `docs/migration-0.2-to-0.3.md` or a new
   `docs/migration-0.3.x-to-0.3.y.md`;
3. updated `docs/schemas/parse-output-v0.3.schema.json` if the JSON shape
   changes;
4. updated editor consumers (`editors/vscode/src/extension.ts`,
   `editors/monaco/mimispecCompletion.ts`) if the CLI JSON envelope changes.

#### Entry-point functions (`src/lib/mod.rs`)

| Function | Signature (frozen) | Notes |
|----------|--------------------|-------|
| `parse` | `fn parse(source: &str) -> ParseResult` | Semantic parse. Always returns a `File` (possibly empty on lexer error). |
| `parse_fragment` | `fn parse_fragment(source: &str) -> ParseResult` | Single-Context-Unit parse; rejects trailing input. |
| `parse_lossless` | `fn parse_lossless(source: &str) -> lossless::LosslessParseResult` | Source-preserving parse. |
| `tokenize` | `fn tokenize(source: &str) -> Result<Vec<lexer::Token>, error::ParseError>` | Lower-level lexer entry. |
| `parse_file` | `fn parse_file(path: &Path) -> ParseFileResult` | File + `@import` resolution. |

#### Result types (`src/lib/error.rs`, `src/lib/lossless.rs`)

| Type | Frozen shape |
|------|-------------|
| `ParseResult` | `{ file: File, errors: Vec<ParseError>, status: ParseStatus }` |
| `ParseStatus` | `Complete` or `Partial` (snake_case JSON) |
| `ParseError` | `{ code: ErrorCode, line: usize, col: usize, message: String, help: Option<String>, suggestion: Option<String> }` |
| `LosslessParseResult` | `{ document: lossless::Document, errors: Vec<ParseError>, status: ParseStatus }` |

#### AST types (`src/lib/ast.rs`)

| Item | Stability |
|------|-----------|
| `AST_SCHEMA_VERSION` | The `"mimispec.ast/0.3"` string is emitted on every serialized `File`. Bumping it requires a new `docs/schemas/` entry and a migration note. |
| `File` | Serialized as `{ "schema_version", "imports"?, "items" }` (imports omitted when empty). The `fragments` Rust field name is the legacy alias for `items`. |
| `Fragment` (a.k.a. `ContextItem`) | `#[non_exhaustive]` — new variants may be added in 0.3.x without a breaking-change note, but existing variant names and their field shapes are frozen. |
| `Commitment` | Nine-variant enum. The nine suffix states and their `is_locked()` / `is_strong_locked()` / `is_confirmed()` / `is_delegated()` / `has_question()` helpers are frozen. |
| `RuleAttachment` | `Pending`, `Attached { target_index }`, `Environment`, `UnresolvedByRecovery`. |

#### Diagnostic codes (`src/lib/error.rs::ErrorCode`)

The numeric assignment of these codes is frozen. A code's meaning may be
narrowed (more specific message) but not broadened (reused for a different
category). New codes must be appended; existing codes must not be renumbered.

| Code | Category | Currently emitted? |
|------|----------|-------------------|
| `E0001` | Lexer: unexpected character | Constructor defined; not yet emitted from the lexer hot path. |
| `E0002` | Lexer/parser: unexpected EOF | Emitted. |
| `E0003` | Lexer: indentation error | Emitted. |
| `E0004` | Lexer: invalid escape sequence | Emitted. |
| `E0005` | Lexer: unterminated string | Emitted. |
| `E0010` | Parser: unexpected token (generic) | Emitted. |
| `E0011` | Parser: undefined variable | Constructor defined; **not yet emitted** (no name-resolution pass wired in 0.3.x). |
| `E0012` | Parser: unsupported binary operator | Constructor defined; not yet emitted. |
| `E0013` | Parser: unsupported expression form | Constructor defined; not yet emitted. |
| `E0014` | Parser: value not callable | Constructor defined; not yet emitted. |
| `E0015` | Parser: subscript out of bounds | Constructor defined; not yet emitted. |
| `E0016` | Parser: operand type mismatch | Constructor defined; not yet emitted. |
| `E0017` | Parser: expected indented block | Emitted. |
| `E0018` | Parser: expected function body | Emitted. |
| `E0701` | Internal parser error | Reserved; emitted only on unrecoverable state. |

The CLI's legacy `E0000` string (used for file I/O errors in `src/main.rs`)
is **not** part of `ErrorCode` and is not frozen; it is expected to be
absorbed into a future `ResolveError` variant.

#### JSON schemas

| Schema file | Frozen version |
|-------------|---------------|
| `docs/schemas/parse-output-v0.3.schema.json` | `mimispec.parse/0.3` |
| `docs/schemas/collaboration-report.schema.json` | collaboration report envelope |
| `docs/schemas/language-service-v0.3.schema.json` | `mimispec.ls/0.3` custom LSP values |
| `docs/schemas/conformance-v0.3.schema.json` | `mimispec.conformance/0.3` manifest |

#### Frozen language-service wire contract

The custom method names, required request/response fields, and `C-*` codes in
`docs/language-service-protocol-0.3.md` are Tier 1. Standard LSP behavior is
LSP 3.17 over stdio. Adding optional fields is compatible; removing or
reinterpreting a field, method, enum value, or code requires a new wire schema
and migration note.

### Tier 2 — Stable shape, evolving internals

These modules expose public types whose *names and field shapes* are stable
for 0.3.x, but whose internal algorithms and helper methods may evolve:

- `resolver::Resolver`, `cache::ImportCache`, `symbol::SymbolTable`,
  `query::FileQuery` / `FragmentIter` — the cross-file resolution and query
  API. `ImportCache::new()` remains unlimited; additive `with_capacity`,
  `len`, `is_empty`, and `clear` provide optional LRU-bounded operation while
  preserving `get(&self)` and mtime validation.
- `lossless::Document` — the source-preserving document. Public methods
  listed in `src/lib/lossless.rs` are frozen; new methods may be added.
  `parent_node`, `child_nodes`, `commitment_slots_for_owner`, and `scope_path`
  expose its revision-local shared structure index. `SourceNodeId` values and
  returned scope paths must never be persisted across revisions.
- `collaboration::validate_document_patch` / `validate_ai_document_patch` and the
  `CommitmentSlotId` / `LockChallenge` types — the patch validator contract
  is frozen; internal heuristics may improve.

### Tier 3 — Experimental (not frozen)

These modules are first-version prototypes. Their public types may change
between minor 0.3.x releases without a migration note. They are excluded
from the 0.3 Core freeze:

- `materialize` — materialization plans, `CommitSelection`, `Evidence`.
- `profile` — `TargetProfile` trait and built-in profiles.
- `workflow` — OSE workflow board and semantic diff.
- `session` — observed/authoritative/pending `DocumentSession` internals. The
  Rust types remain experimental even though their `mimispec.ls/0.3` wire
  projection is frozen.
- `ide` and `lsp` — Rust helper/server types. Semantic behavior is covered by
  the frozen wire protocol, but the Rust representation may evolve.
- `diagnostics` — intent conflict/gap heuristics. Diagnostic *categories*
  are documented in `docs/0.3.x-design-zh.md`; their Rust types may evolve.
- `provenance` — Core-external `mimispec.provenance/0.1` sidecars and
  `SlotLocator`. This protocol remains experimental and cannot alter Core
  commitment or count as target verification.

## 2. What Counts as a Breaking Change

Within Tier 1, the following are breaking changes requiring a migration
note:

- Renaming or removing a public function, type, variant, field, or method.
- Changing a function signature (parameter types, return type, arity).
- Changing the serialized JSON shape of `File`, `ParseResult`,
  `LosslessParseResult`, or `ParseError`.
- Renumbering or reusing an `ErrorCode`.
- Changing `AST_SCHEMA_VERSION` or the CLI envelope schema version.
- Breaking a frozen `mimispec.ls/0.3` custom method or collaboration code.
- Changing the suffix-meaning table in `docs/commitment-state-machine.md`.

The following are **not** breaking changes within 0.3.x:

- Adding a new `Fragment` variant (the enum is `#[non_exhaustive]`).
- Adding a new public method to `lossless::Document` or `File`.
- Adding a new `ErrorCode` (appended, not reused).
- Narrowing an `ErrorCode`'s message to be more specific.
- Adding new Tier-3 experimental modules.

## 3. Verification

The frozen API surface is exercised by:

- `cargo test --lib` — all 239 tests, including `property_tests` and
  `multilingual_tests` which assert round-trip, JSON schema version, and
  Unicode-content invariants.
- `cargo test --release stress_tests` — large-file slot-linearity guard.
- `cargo test --release property_tests` — fuzz/property CI gate.
- `cargo test --test lsp_stdio` — real stdio process lifecycle.
- `cargo run -- conformance check` — frozen golden conformance suite.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check`.

A PR that changes any Tier-1 shape without the required migration artifacts
must fail review.
