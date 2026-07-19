# Changelog

## [Unreleased] - 0.3.x development

### Added
- Added the frozen `mimispec.ls/0.3` target-neutral wire protocol and
  `mimispec lsp --stdio`, implementing LSP 3.17 document sync, diagnostics,
  semantic tokens, effective-protection hover, rule/Flow navigation, code
  actions, queues, confirmed intent, and lock-challenge snapshots.
- Added advisory/strict workspace collaboration modes with observed,
  authoritative and pending revisions, SHA-256 revision hashes, stale-version
  checks, actor-declared UTF-16 edits, and server-bound single-use strong-lock
  tokens.
- Added generalized Human/AI/undeclared `validate_document_patch`; retained
  `validate_ai_document_patch` as the compatibility entry point.
- Added `mimispec.conformance/0.3` parse/lossless/transition/LSP fixtures and
  `mimispec conformance check`; the canonical parser remains the only parser
  implementation.
- Added a machine-readable 5-author/25-document usability gate and
  `mimispec usability check`. Release automation intentionally fails until
  independent trial evidence satisfies it.
- Added a real child-process stdio LSP lifecycle test, Linux release-blocking
  CI, experimental macOS/Windows smoke jobs, VS Code language-client CI, and
  Cargo package smoke checks.
- Added the anonymous Document Context and one cross-kind ordered item model for
  root, module, type, Flow, func, steps, nested step blocks, UI bodies, and UI
  layout children.
- Added first-class root `Desc` and `Clause` items, repeatable ordered
  `requires`/`ensures`, and explicit Attached/Environment rule relations.
- Added anonymous Flow, optional `on Event` labels, multiple inline Flow tails,
  and independent event/transition/target commitment slots.
- Added explicit `ParseStatus::Complete` / `Partial` on semantic and lossless
  parse results, including recovery-only `UnresolvedByRecovery` attachment.
- Added `mimispec.ast/0.3` semantic JSON, `mimispec.parse/0.3` CLI envelopes,
  and `docs/schemas/parse-output-v0.3.schema.json`.
- Added parser-proven `CommitmentSlotId`, semantic owner/footprint metadata,
  exact slot code actions, and slot-level AI patch validation that does not
  collapse keyword/name/value/event suffixes into one header state.
- Added strong-lock subtree validation with explicit `?`/`??` descendant
  footprints remaining editable through allowed AI transitions.
- Added public Context queries for descriptions, clauses, rules, environment
  rules, attached rules, and recursively ordered items.
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
- Extended lossless source nodes to `Rule`, `Desc`, `Clause`, `Math`, nested
  `Field`, `FlowEntry`, `FlowArm`, `Step`, `UiNode`, and `Placeholder` targets so
  slots, attached rules, and comments resolve beyond top-level Fragments.
- Added line-comment attachment classification (`trailing` / `leading` /
  `free`) with optional target node IDs on the lossless document.
- Added structured Fragment moves that relocate a top-level node while carrying
  its attached rule prelude, plus attachment/commitment fingerprints used to
  assert formatter stability.
- Added protected content/structure hashes, `LockChallenge` records, and
  challenge-fingerprint deduplication for AI lock challenges.
- Added `validate_ai_document_patch` for before/after lossless revision checks
  that accept suffix-only lock challenges and reject lock-bypass content edits.
- Added human-only `UnlockToken` for weakening strong locks and parent/child
  lock propagation checks for nested AI edits.
- Added first-wave intent diagnostics with decision/delegation queues,
  commitment summaries, syntax/attachment guidance, and stable diagnostic codes.
- Added intent-conflict and intent-gap heuristics, plus `mimispec diagnose`
  (and `--diagnostics`) for human/JSON collaboration reports.
- Added library-level IDE protocol helpers: semantic tokens, commitment hover,
  actor-aware code actions, and an `ide_snapshot` combining queues/diagnostics.
- Added materialization core types: `CommitSelection`, provenance categories,
  `EvidenceRecord`, materialization plans, drift detection, and
  `mimispec materialize` for commit-ready slot planning.
- Added first-party/generic target profile analysis with capability matrices,
  unsupported-intent gap reporting, and `mimispec profile --target mimi|generic`.
- Added `DocumentSession` for full-text incremental collaboration document state
  with versioned snapshots, hover/tokens/code actions, and plan/profile helpers.
- Stabilized the generic `TargetProfile` trait with built-in mimi/generic/rust/
  typescript profiles, conformance checks, and CLI target selection.
- Added OSE workflow board APIs (`workflow::build_workflow_board`, semantic
  diff, rejected-challenge filtering) and `mimispec workflow`.
- Added draft `docs/migration-0.2-to-0.3.md` and
  `docs/schemas/collaboration-report.schema.json` for 0.3.x stabilization.
- Added the M5 property and fuzz test family (`src/lib/mod.rs::property_tests`)
  with a seed-deterministic LCG and seven invariants: idempotent render,
  render determinism, AST JSON schema versioning, lossless no-panic on
  arbitrary byte input, error-status consistency, lossless/semantic parser
  equivalence, and tokenize-then-parse equivalence with `parse()`. Gated in CI
  as the `Property & Fuzz Tests` job.
- Added the M5 multilingual acceptance family
  (`src/lib/mod.rs::multilingual_tests`) covering byte-exact CJK round-trip,
  emoji and punctuation preservation in rules, NFC/NFD non-normalization of
  combining marks, Unicode-scalar column math on CJK input, and a seven-script
  corpus (Simplified/Traditional Chinese, Japanese, Korean, Arabic, Cyrillic,
  emoji+Latin).
- Added `examples/perf_baseline.rs` and a deterministic slot-linearity stress
  guard (`stress_tests::stress_slot_count_scales_linearly_with_module_size`).
  Published the M5 performance budget for 500/1000/2000-func modules in
  `docs/roadmap-0.3.x.md` §10.
- Added `docs/api-stability-0.3.md` freezing the Tier 1 public API (parser
  entry points, result types, AST shape, `AST_SCHEMA_VERSION`, `ErrorCode`
  assignments) for the remainder of 0.3.x. Tier 3 experimental modules
  (materialize, profile, workflow, session, ide, diagnostics) are explicitly
  excluded from the freeze.
- Added the M5 cross-domain acceptance corpus under `docs/corpora/`:
  `plain-product-intent`, `state-transitions`, `failure-and-recovery`,
  `resource-ownership`, `ordered-communication`, `external-boundaries`, and
  `multilingual`, plus the cohesive `real-world-family-ledger` usability
  fixture. Each is gated by `corpus_acceptance_tests` in
  `src/lib/mod.rs`, which asserts clean parse, Complete status, round-trip
  AST equivalence, and 0.3 schema version on every corpus.
- Added real-project `mimi-kv` and `mimichat` MMS transcriptions plus an
  end-to-end usability report covering parser/lossless results, review-queue
  scaling, authoring friction, provenance, and deferred Mimi Profile gaps.
- Added `docs/0.3-usability-report.md`, recording the real-world family-ledger
  trial, its automated contract, and the remaining 0.3.4/release gaps.
- Extended `docs/migration-0.2-to-0.3.md` with a "Tooling and Tests" section
  pointing migrants at the property/fuzz gate, perf baseline, corpus, and
  API-stability documents.
- Added a revision-local `LosslessDocument` structure index and Tier-2
  `parent_node`, `child_nodes`, `commitment_slots_for_owner`, and `scope_path`
  queries; collaboration, diagnostics, queues, and materialization reuse it.
- Added contextual Action E0010 guidance and Tier-3 `SyntaxQuickFix` values.
  LSP quick fixes use standard `WorkspaceEdit` and avoid automatic rewrites of
  transitions, assignments, and structural blocks.
- Added compatible hierarchical `QueueTree` snapshots, grouped CLI diagnose
  output with `--flat-queues`, a VS Code queue tree, and Human-only atomic
  `mimispec/prepareQueueBatch` transactions for exact current suffix slots.
- Added optional finite-capacity LRU behavior to `ImportCache` via
  `with_capacity`, plus `len`, `is_empty`, and `clear`; the default remains
  unlimited and mtime-aware.
- Added experimental Core-external `mimispec.provenance/0.1` sidecars,
  SHA-256/`SlotLocator` drift checks, path confinement under `--source-root`,
  and real `mimi-kv`/`mimichat` provenance fixtures. No target command is run
  and provenance never changes commitment.
- Refactored experimental Materialize/Profile/Evidence selection to exact
  `CommitmentSlotId + SlotLocator` values. Only exact `$`/`$$` slots are
  selected; open residual slots on the same node remain explicit.

### Fixed
- Fixed `DocumentSession::observe_edits` reparsing after every LSP change; a
  batch now applies sequential UTF-16 changes to text/LineIndex and parses once.
- Fixed intent-gap diagnostics skipping nested Contexts and matching duplicate
  names by substring. Complete documents now use source-order kind/scope
  correspondence and conservative boundary-action hints; Partial documents
  skip heuristics.
- Fixed the Flow intent-gap heuristic treating a `Kicked`/`踢出` transition as
  if no forced-removal failure path existed.
- Fixed experimental materialization drift checks reusing revision-local node
  IDs after Fragment reorder and comparing unrelated locked slots.
- Fixed frozen language-service custom requests silently defaulting omitted
  `base_version`, `authorization`, and `unlock_tokens`; malformed or missing
  required fields now return `C-INVALID-EDIT` before policy evaluation.
- Fixed document-level patch validation allowing AI to self-delegate a newly
  created semantic slot with `??`; every fresh non-empty state now passes the
  same `none -> state` transition matrix as an existing slot.
- Fixed multiple direct descriptions and repeated clauses being ignored,
  reclassified, or silently overwritten.
- Fixed root clauses degrading into Action Steps.
- Fixed comment-only lines acting like paragraph breaks.
- Fixed normalization rendering rebinding an Environment rule chain when it is
  immediately followed by a separate attached prelude.
- Fixed Flow/UI placeholder commitment suffixes being lost during rendering or
  parsing.
- Fixed `parse_fragment` ignoring unrelated trailing Context Units and reporting
  their diagnostic at `(0, 0)`.
- Preserved commitment suffixes when rendering boolean values, `not`, `and`,
  `or`, `in`, comparison operators, Flow `>>>`, and Flow `requires` slots.
- Fixed the single-line Flow renderer placing the `>>>` suffix on the target
  identifier instead of the transition slot.
- Fixed capabilities consuming and rendering the same commitment suffix twice.
- Replaced repeated token-to-source and per-slot hierarchy scans with
  revision-local indexes, and added a 1000-function lossless semantic-slot
  stress regression.
- Made the CI stress job execute the actual stress tests and added a rustfmt
  gate.
- Fixed the property/fuzz gate failing to compile because a closure parameter
  used unsupported `impl Trait` syntax and a `Result` called a nonexistent
  `expect_or_else` method; the gate is also clean under `-D warnings`.
- Fixed the intent-gap heuristic reporting a missing Flow failure path when
  failure or cancellation is expressed by an event label such as
  `UploadFailed`; English and Chinese event labels now participate in the hint.
- Fixed Context Item semantic tokens consuming whole headers and hiding
  overlapping commitment suffix tokens.

### Changed

- Reclassified the never-published 0.3.0-0.3.5 labels as internal M0-M5
  milestones. The only planned public release is consolidated `0.3.0`, with
  `0.3.0-rc.1` blocked on the independent usability gate.
- Upgraded the VS Code extension development version to 0.5.0 and made the
  long-lived server primary, retaining reduced 0.2.1 CLI validation fallback.

### Documentation
- Clarified that the current released implementation is `0.2.1`; the existing
  advanced specification is a staged `0.3.x` design target, not a released
  `1.0.0-rc` grammar.
- Added the `0.3.x` development roadmap.
- Added the normative commitment state-machine design, including the rule that
  `?`/`??` after `$`/`$$` review the lock decision rather than the content.
- Defined content-preserving AI lock challenges (`$ -> $?`, `$$ -> $$?`) and
  separated commitment from implementation evidence.
- Clarified that MimiSpec Core is independent of every external wrapper or
  target; external callers may pass MMS text to the canonical parser without
  changing its syntax or semantics.
- Replaced the 0.3 design center with a formal Core-language baseline covering
  anonymous Document Context, non-delegating `desc`, environment `rule`,
  repeatable clauses, event-labelled/open-world Flow, lossless source, and
  commitment protection.
- Defined one authoritative ordered item sequence per semantic scope, explicit
  Attached/Environment rule relations, uniform prelude targets, and physical
  blank lines as the only ParagraphBreak source.
- Deferred Materialization, Profile, Evidence, Release Scope, and OSE workflow
  protocols beyond the 0.3 Core language freeze.

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
