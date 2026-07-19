# Migrating from MimiSpec 0.2.1 to 0.3.x

> Status: migration guide for the implemented but unreleased main-line 0.3
> Core draft. Published behavior remains `0.2.1` until an explicit 0.3 release.

## 1. Compatibility Promise

- Valid 0.2.1 surface syntax remains parseable.
- Existing commitment suffix spellings remain valid.
- Existing named `module`, `type`, `flow`, `func`, `steps`, and `ui` forms
  remain valid.
- Existing unlabelled Flow edges remain valid.
- `parse(source)` remains the canonical complete-document entry point.

0.3 intentionally corrects semantic AST behavior where 0.2.1 can silently lose
or misclassify intent. AST JSON consumers must therefore opt into a versioned
0.3 schema rather than assuming exact 0.2 field shapes.

## 2. Corrected Language Semantics

### 2.1 Document Root

The document root becomes an anonymous Intent Context. These are complete,
valid documents:

```mimispec
desc "支付确认后才改变订单状态"
rule "结果未知时不得重复扣款"
```

```mimispec
requires: request.id != ""
ensures: accepted == true or original_state_preserved == true
```

Root `desc` and clauses are no longer wrapped or misclassified as Steps.

### 2.2 Description Lists

0.2.1 container ASTs commonly use `Option<Desc>` and may reinterpret or ignore
later descriptions. In 0.3 every direct description remains a description:

```mimispec
func Pay:
    desc "处理支付请求"
    desc "订单系统保持唯一提交权威"
```

Code reading `func.desc` must migrate to an ordered description collection or
the equivalent Context Item query.

### 2.3 Repeatable Clauses

0.2.1 stores one `requires` and one `ensures`; a later clause may overwrite an
earlier one. 0.3 stores all clauses:

```mimispec
func Pay(order, amount):
    requires: order.status == Pending
    requires: amount > 0
```

Library consumers migrate from:

```rust
func.requires: Option<Condition>
func.ensures: Option<Condition>
```

to ordered clause collections. Multiple same-kind clauses are conjunctive.

### 2.4 Rule Attachment

A same-paragraph rule prelude attaches to the following same-scope non-rule
semantic item. A rule separated by ParagraphBreak, lacking such a following
item, or occurring at scope end is an environment rule. Scope-terminal rules
are no longer described as dangling errors.

0.3 removes the 0.2.1 implementation allowlist: every same-scope non-rule
semantic item can receive a prelude, including Desc, Clause, Steps, Expr,
UiNode, and Placeholder items. A physical blank line is the explicit way to
keep a preceding rule at Environment scope.

A comment-only line no longer impersonates a blank line. It remains trivia in
the same paragraph unless a real blank line appears before or after it.

### 2.5 Flow

Existing syntax remains valid:

```mimispec
flow OrderLifecycle:
    Pending >>> Paid: desc "支付成功"
```

0.3 adds anonymous Flow and optional event labels:

```mimispec
flow:
    Pending:
        on CaptureConfirmed >>> Paid: desc "确认后提交"
```

Flow is open-world by default. Absence of an edge is not a prohibition unless
an explicit rule states closure.

## 3. Commitment Migration

The nine suffix values remain textually unchanged. Their normative meaning is:

| Suffix | Meaning |
|--------|---------|
| none | open content |
| `?` | content review |
| `??` | content delegation |
| `$` | confirmed ordinary lock |
| `$?` | ordinary-lock review; content protected |
| `$??` | ordinary-lock assessment delegated; content protected |
| `$$` | human-confirmed strong lock |
| `$$?` | strong-lock review; content protected |
| `$$??` | strong-lock assessment delegated; content protected |

`desc` without a suffix no longer implies delegation. Use `desc??` when the
description decision is delegated.

New code should prefer `is_confirmed()`. `is_commit_ready()` may remain a
compatibility alias but means only intent confirmation, never build or release
readiness.

## 4. Parser and AST Migration

The current unreleased main implementation uses:

- `File.fragments` evolving into one ordered Document Context body;
- Rule remaining an ordered body item with a separate Attached/Environment
  relation instead of being moved into a target-owned vector;
- first-class root Desc and Clause items;
- scope-specific ordered body-item enums with typed description/clause/rule
  queries;
- anonymous `FlowDef.name`;
- optional `FlowArm.event`;
- independent commitment/span slots for every repeated description, clause,
  event, target, guard, and attachment;
- versioned JSON output.

Concrete compatibility details:

- the Rust field `File.fragments` remains temporarily available, but now holds
  the authoritative cross-kind Context Item order and serializes as `items`;
- serialized `File` values carry `schema_version: "mimispec.ast/0.3"`;
- CLI parse envelopes carry `schema_version: "mimispec.parse/0.3"` and each
  result exposes `status` plus the compatibility `success`/`partial` booleans;
- `ParseResult` and `LosslessParseResult` expose `ParseStatus::Complete` or
  `ParseStatus::Partial`;
- module, type, Flow, func, steps, nested step blocks, UI bodies and UI layout
  children use ordered `Vec<Fragment>` item bodies;
- `UiDef::root()` and `StackNode::children()` are derived queries rather than
  independent authoritative fields;
- parser-proven commitment slots expose revision-local `CommitmentSlotId`,
  owner node and semantic footprint; collaboration consumers must not scan a
  header and choose its strongest suffix.

Parser consumers must not infer 0.3 structures by scanning rendered headers.
Use semantic fields and lossless slot metadata.

`parse_lossless(source)` is the source-preserving entry point. It must agree
with `parse(source)` about Context Item kind and rule attachment.

`parse_fragment(source)` remains as a compatibility name but validates one
Context Unit, including its rule prelude. Consumers that previously relied on
ignored trailing source must treat the new trailing-unit diagnostic as an
intent-loss fix.

## 5. Renderer and Lossless Source

The semantic renderer remains normalization-oriented. It must preserve semantic
AST, clause order, suffix targets, and rule attachment, but it does not promise
byte-for-byte restoration.

Use the 0.3 lossless document/edit API when comments, original blank lines,
exact indentation, or source spelling must remain unchanged.

## 6. Non-Goals

- This migration does not add target-language syntax.
- It does not define embedded-wrapper behavior in another language.
- It does not turn `rule`, `requires`, or `ensures` into automatically enforced
  target contracts.
- It does not add Materialization, Profile, Evidence, Release Scope, or OSE
  objects to the Core AST.
- Locked does not mean implemented, tested, verified, or deployed.

## 7. Tooling and Tests

The consolidated 0.3.0 M5 milestone provides a stabilization toolchain that
migration consumers can rely on before the independent RC trial completes:

- `cargo test --lib` — 225 default-Core tests, including the families below.
- `cargo test --all-features --lib` — 246 tests including the explicitly
  experimental target/provenance layers.
- `cargo test --release stress_tests` — large-file slot-linearity guard.
- `cargo test --release property_tests` — seven seed-deterministic
  property/fuzz invariants: idempotent render, render determinism, AST JSON
  schema versioning, lossless no-panic on arbitrary bytes, error-status
  consistency, lossless/semantic parser equivalence, and tokenize-then-parse
  equivalence. Failures print the seed.
- `cargo run --release --example perf_baseline` — measures parse/render/
  lossless timings for 500/1000/2000-func modules. Published budget numbers
  live in `docs/roadmap-0.3.x.md` §10.
- `docs/corpora/` — ten technical acceptance corpora
  (`corpus_acceptance_tests` in `src/lib/mod.rs`):
  - `plain-product-intent.mms` — five-minute entry, no named wrapper;
  - `state-transitions.mms` — anonymous/named Flow, event labels, open-world;
  - `failure-and-recovery.mms` — failure scope, persistent resource policy;
  - `resource-ownership.mms` — typed records, `with` capabilities, `in`;
  - `ordered-communication.mms` — ordered steps, `parasteps`;
  - `external-boundaries.mms` — external confirmation, idempotent propagation;
  - `multilingual.mms` — Simplified/Traditional Chinese, Japanese, Korean,
    Arabic, Cyrillic, emoji+Latin;
  - `real-world-family-ledger.mms` — cohesive product intent, data, Flow,
    functions, UI, lossless preservation, diagnostics, and JSON usability;
  - `mimi-kv-real-project.mms` — reverse transcription of a TCP/JSON key-value
    server and client from the sibling MIMI checkout;
  - `mimichat-real-project.mms` — reverse transcription of a 2,014-line
    Actor/concurrency/chat project, including unresolved production questions.
- `docs/api-stability-0.3.md` — the Tier 1/2/3 public API freeze. Tier 1
  (parser entry points, result types, AST shape, `AST_SCHEMA_VERSION`, and
  `ErrorCode` assignments) is frozen for the remainder of 0.3.x.
- `mimispec conformance check` — canonical MMS→JSON/lossless/transition/LSP
  fixtures under `mimispec.conformance/0.3`; no second parser is required.
- `mimispec lsp --stdio` — LSP 3.17 server with frozen
  `mimispec.ls/0.3` custom methods. The VS Code extension falls back to the
  released 0.2.1 parse process when the server command is unavailable.
- Experimental provenance is compiled with `experimental-provenance`;
  Materialize/Profile/Workflow require `experimental-targets`. Neither feature
  is enabled by default or required by the Core parser/LSP.

A 0.2.1 consumer migrating to 0.3 should run its own fixtures against
`cargo run --render` and compare against the corpus patterns above when
the 0.2.1 AST shape diverges.

## 8. Migration Checklist

- [ ] Keep 0.2.1 source fixtures parseable.
- [ ] Replace single-description assumptions with ordered queries.
- [ ] Replace optional single clauses with clause iteration.
- [ ] Ensure root clauses are not handled as Action Steps.
- [ ] Treat scope-terminal rules as environment rules.
- [ ] Replace target-owned rule vectors with ordered items plus attachment
      queries.
- [ ] Re-check rule-before-Steps/Expr/UiNode/Placeholder behavior.
- [ ] Re-check comment-only lines formerly treated as paragraph breaks.
- [ ] Do not infer Flow closure from missing edges or lock suffixes.
- [ ] Add support for anonymous/event-labelled Flow if consuming Flow AST.
- [ ] Use `is_confirmed()` semantics for `$` / `$$`.
- [ ] Version AST/diagnostic JSON consumers.
- [ ] Use lossless edits when paragraph or attachment preservation matters.
