# MimiSpec 0.3.0 Consolidated Development Roadmap

> Current released version: `0.2.1`
>
> Development snapshot status: the current main line is technically ready to
> be cut as `0.3.0-dev` for explicit source/binary evaluation. No `0.3.0-dev`
> Cargo version, tag, or artifact exists until a separately authorized release
> operation performs those changes.
>
> This roadmap covers the MimiSpec language, canonical parser, document model,
> collaboration semantics, diagnostics, and language-service protocol only.
>
> Main development status (2026-07-20): M0-M4 and the technical portion of M5
> are implemented in the working tree. Four real-project reverse transcriptions
> close the internal Action-recovery and queue-scalability P1s. The independent
> trial remains at 0/5 authors, 0/25 documents, 0/5 domains, and 0/4 five-minute
> successes, so Cargo and published release facts remain `0.2.1` and RC
> preparation remains blocked.

## 1. Series Goal

MimiSpec 0.3.x makes small, independent intent fragments reliable enough for
humans and AI to read, edit, and preserve without requiring a named wrapper or
target-language knowledge.

The series closes five language-level gaps:

1. an anonymous Document Context that accepts descriptions, rules, clauses,
   and ordinary Fragments;
2. `desc` semantics that do not imply delegation and never change by ordinal
   position;
3. repeatable `requires` / `ensures` clauses with no silent overwrite;
4. intent-level Flow events and explicit open-world semantics;
5. a lossless document and enforceable commitment state machine.

The normative Core design is
[`0.3.x-design-zh.md`](0.3.x-design-zh.md). The surface grammar is
[`specification.md`](specification.md), and suffix transitions are specified in
[`commitment-state-machine.md`](commitment-state-machine.md).

## 2. Scope Boundary

0.3.x includes:

- surface grammar and semantic AST;
- canonical parsing of a Document Context and individual Context Items;
- source-preserving document representation;
- rule attachment and paragraph semantics;
- commitment slots, actor transitions, lock challenges, and patch validation;
- versioned diagnostics and language-service data.

0.3.x does not include:

- target-language syntax or AST nodes;
- external wrapper syntax such as another language's embedded block;
- code generation or automatic implementation checking;
- Materialization, Target Profile, Evidence, Release Scope, or OSE workflow
  protocols;
- automatic formalization of natural-language rules.

External systems may call the canonical parser with MMS text, but they do not
change how that text is parsed.

## 3. Non-Negotiable Invariants

Every 0.3.x release must preserve these invariants:

1. Natural language is first-class.
2. A file containing only `desc` and/or `rule` is useful and valid.
3. No suffix is required for an ordinary draft.
4. `desc` without `?` or `??` does not delegate content to AI.
5. Repeated descriptions and clauses are never ignored or overwritten.
6. Reserved Context Items never silently degrade into Action Steps.
7. A trailing or paragraph-separated rule is an environment rule, not a
   dangling error.
8. Flow is open-world unless an explicit rule states closure.
9. Flow completeness is independent of `$` / `$$`.
10. All states containing `$` protect current content.
11. AI cannot enter, remove, or weaken `$$` without human authorization.
12. Formatting preserves suffix targets, clause order, and rule attachment.
13. Commitment means intent confirmation only.
14. Partial AST is never described as a complete document.
15. Valid 0.2.1 surface syntax remains parseable unless a separately approved
    migration documents a safety-critical exception.
16. Every semantic scope has one authoritative ordered item sequence; typed
    collections are derived queries, not competing storage.
17. Only a physical blank line creates a ParagraphBreak; a comment-only line
    cannot silently change rule attachment.

## 4. Milestone Structure and One Published Version

The historical `0.3.0`-`0.3.5` labels were never published. They are internal
milestones M0-M5, consolidated into one future public version: `0.3.0`.
`0.3.0-dev` names the optional development snapshot between technical M5 and
the independent-author gate; it is not an additional milestone or an RC.

| Milestone | Theme | Main status | Required outcome |
|---------|-------|-------------|------------------|
| `M0` | Core semantic reset | Implemented | Context root, corrected `desc`/`rule`, repeated clauses, Flow events/open-world semantics, frozen suffix meanings |
| `M1` | Lossless document | Implemented | ParagraphBreak, exact spans, comments, attachment decisions, stable revision-local slots |
| `M2` | Collaboration validation | Implemented | Actor transition matrix, semantic footprints, SHA-256 revisions, lock challenges, structured patch validation |
| `M3` | Parser and diagnostic contract | Implemented | Context Item API, versioned JSON, non-loss diagnostics, semantic queries |
| `M4` | Language-service protocol | Implemented, release-gated | advisory/strict session state, UTF-16 sync, stdio LSP, navigation, hover, semantic tokens, actor-declared edits |
| `M5` | Compatibility and stabilization | Technical gates implemented; external trial pending | conformance suite, corpus, performance, fuzzing, API/wire freeze, 5-author/25-document trial, release candidate |

The technical tree may be explicitly packaged as `0.3.0-dev` without claiming
independent usability validation. `0.3.0-rc.1` may be prepared only after M5's
external trial passes. The final `0.3.0` requires a 14-day RC observation
window with no unresolved P0/P1.

The dependency order is strict:

```text
Core semantics
-> lossless source and attachment
-> protected transitions
-> diagnostics and public parser contract
-> language-service protocol
-> stabilization
```

## 5. M0: Core Semantic Reset

### Goal

Freeze what MMS text means before expanding tooling.

### Deliverables

#### Anonymous Document Context

- `parse(source)` treats every file as an anonymous Intent Context.
- Root `desc`, `rule`, `requires`, and `ensures` are first-class Context Items.
- Context Items remain in one cross-kind source order.
- External callers do not need to synthesize `func`, `module`, or `flow` names.
- Root `requires` / `ensures` cannot enter generic Action fallback.

#### Description semantics

- `desc` is a natural-language intent description.
- Only `?` / `??` express review or delegation.
- All direct descriptions in a descriptive scope remain descriptions.
- `desc` inside `steps` remains a natural-language Step.
- AST and JSON retain every description independently.

#### Rule semantics

- A contiguous rule prelude attaches to the following same-scope non-rule
  semantic item.
- Paragraph-separated, targetless, and scope-terminal rules become environment
  rules.
- Every rule gets one explicit attachment decision.
- Valid documents have only Attached/Environment; recovery uncertainty is an
  explicitly diagnosed partial-document status, not a third valid attachment.
- Every same-scope non-rule semantic item is attachable; there is no hidden
  AST-kind allowlist.
- Comment-only lines preserve the chain; only a physical blank line ends it.

#### Repeatable clauses

- `requires` and `ensures` are ordered lists.
- Each clause has independent condition, commitment, span, and attached rules.
- Repeated clauses mean conjunction.
- Parser and renderer tests prove that no clause is lost.

#### Flow semantics

- `flow Name:` and anonymous `flow:` are both valid.
- `on Event >>> Target` is an optional event-labelled edge.
- Existing `>>> Target` syntax remains valid.
- Flow is open-world by default.
- Source, event, target, guard, description, and attachment remain separate
  semantic slots.

#### Commitment foundation

- Nine-state suffix table and lock-before-question composition.
- Keyword, identifier, value, clause, event, and attachment slots.
- `is_confirmed()` as the preferred semantic API.
- `is_commit_ready()` retained only as a compatibility alias if required.

### Acceptance

- A root Context with multiple descriptions, rules, requires, and ensures
  preserves their cross-kind order and round-trips without information loss.
- Two requires and two ensures remain four clauses in AST and JSON.
- A type containing three descriptions retains all three as descriptions.
- A root `requires:` is never emitted as an Action Step.
- Named, anonymous, labelled, and unlabelled Flows parse and render.
- Missing Flow edges never trigger an implicit closed-world diagnostic.
- All nine suffix states have normative table tests.
- Existing 0.2.1 grammar corpus remains parseable.

## 6. M1: Lossless Document Model

### Goal

Make paragraph, attachment, comments, and suffix position authoritative rather
than inferred from a lossy semantic AST.

### Deliverables

- Lossless token stream or concrete syntax tree.
- Explicit `BlankLine` / `ParagraphBreak` representation.
- One ordered semantic body sequence per scope plus explicit rule-attachment
  edges; typed item collections are query views.
- Exact spans for:
  - Context Items;
  - every description and clause;
  - rule text and rule attachment;
  - keyword, identifier, value, event, target, guard, and suffix slots;
  - nested Field, FlowEntry, FlowArm, and Step nodes.
- Deterministic semantic lowering with recorded attachment decisions.
- Structured moves that carry attached rule preludes.
- Formatter attachment fingerprint.
- Clear distinction between normalization render and lossless edit.

### Acceptance

- Parse-render-parse preserves semantic AST, clause order, and attachment.
- Lossless no-op edit preserves source bytes.
- Moving a Fragment with attached rules preserves its prelude.
- Adding/removing a blank line changes only the intended attachment decision.
- Comments cannot accidentally join or split rule chains.
- Formatter cannot move a suffix between keyword/name/value/event slots.

## 7. M2: Collaboration and Patch Validation

### Goal

Turn suffix meanings into enforceable Human/AI edit permissions.

### Deliverables

- Human and AI actor transition validator.
- Protected slot and subtree hashes.
- Ordinary-lock and strong-lock inheritance.
- Explicit open descendants inside strong-locked containers.
- `LockChallenge` with reason, basis, affected slots, and content-preservation
  proof.
- Human-authorized strong-lock weakening/unlock operation.
- Structured patch validator covering content, name, kind, order, attachment,
  clause position, and Flow event identity.
- Challenge replay suppression until relevant new information exists.

### Acceptance

- `$ -> $?` and `$$ -> $$?` preserve protected source and attachment.
- AI cannot perform `$$ -> $`, `$$ -> none`, or any transition into `$$`.
- AI cannot bypass a lock by moving a rule, clause, event, or Fragment.
- A strong-locked scope may still contain explicitly delegated descendants.
- Adversarial raw-text and structured-edit test suites agree.

## 8. M3: Parser and Diagnostic Contract

### Goal

Expose the Core model through stable library and JSON interfaces without
turning heuristic analysis into language semantics.

### Deliverables

- Public Context Item queries for descriptions, environment rules, clauses,
  and Fragments.
- Public `parse_lossless(source)` contract whose classification and attachment
  decisions agree with `parse(source)`.
- `parse_fragment(source)` defined as one Context Unit (optional rule prelude
  plus one item, or one terminal environment-rule chain) with no ignored tail.
- Versioned AST/diagnostic JSON schema.
- Diagnostic categories for:
  - invalid suffix order;
  - reserved Context Item misclassification;
  - attachment ambiguity or recovery loss;
  - repeated-clause conflicts;
  - malformed event-labelled Flow edges;
  - protected-content violations;
  - partial AST status.
- Source-map-safe recovery.
- Semantic diff that distinguishes content, state, and attachment changes.

### Acceptance

- CLI and library consumers receive the same AST shape and diagnostic codes.
- No diagnostic claims that a natural-language rule has been formally proven.
- Recovery never reports success while silently dropping a description or
  clause.
- JSON consumers can distinguish 0.2 and 0.3 shapes.
- `parse_fragment` retains its prelude and rejects an unrelated trailing unit.

## 9. M4: Language-Service Protocol

### Goal

Provide one target-neutral protocol for editors and AI clients to inspect and
edit the same language state.

### Deliverables

- Incremental Document Context state.
- Semantic tokens for Context Item kind and commitment state.
- Hover information for suffix target and effective protection.
- Navigation between rule and attachment target.
- Navigation among Flow source/event/target/guard slots.
- Human/AI actor-aware code actions.
- Decision, delegation, confirmed-intent, and lock-challenge views.
- Workspace-level `advisory`/`strict` collaboration modes, default advisory.
- SHA-256 observed/authoritative/pending revisions and single-use Human unlock
  tokens.
- LSP 3.17 over stdio with a frozen `mimispec.ls/0.3` custom wire contract.

### Acceptance

- Every edit request declares Human or AI authority.
- No language-service action can bypass the 0.3.2 validator.
- Multiple descriptions and clauses remain independently addressable.
- Protocol behavior is target-language neutral.
- A real stdio child-process transcript passes on Linux; Windows and macOS are
  experimental build/test targets.

## 10. M5: Compatibility and Stabilization

### Goal

Freeze the 0.3 language and public API only after real intent corpora exercise
the corrected semantics.

### Deliverables

- 0.2.1-to-0.3 semantic migration report. ✅ `docs/migration-0.2-to-0.3.md`
  covers syntax compatibility, corrected semantics, commitment migration,
  parser/AST shape changes, renderer/lossless behavior, a tooling-and-tests
  section, and a 13-item migration checklist.
- AST/JSON compatibility adapters where practical. ✅ `File.fragments` remains
  the Rust field name (legacy alias for the `items` JSON key); `success`/
  `partial` booleans ride alongside the versioned `status` field in CLI
  envelopes; `is_commit_ready()` remains as a compatibility alias for
  `is_confirmed()`.
- Corpus covering: ✅ Twelve technical acceptance corpora under
  `docs/corpora/`, each gated by `corpus_acceptance_tests` in
  `src/lib/mod.rs`:
  - plain-language product intent (`plain-product-intent.mms`);
  - state transitions and forbidden behavior (`state-transitions.mms`);
  - failure and recovery (`failure-and-recovery.mms`);
  - resource ownership and permissions (`resource-ownership.mms`);
  - ordered communication (`ordered-communication.mms`);
  - external boundaries (`external-boundaries.mms`);
  - multilingual descriptions (`multilingual.mms`);
  - cohesive real-world product usability (`real-world-family-ledger.mms`);
  - MIMI key-value server/client transcription (`mimi-kv-real-project.mms`);
  - 2,014-line MIMI Actor/chat transcription (`mimichat-real-project.mms`);
  - 1,009-line Markdown/HTML transcription (`mimi-markdown-real-project.mms`);
  - 755-line log-analysis pipeline transcription (`mimi-log-real-project.mms`).
  The four reverse transcriptions are technical fixtures, not independent-author
  trial evidence; findings are recorded in
  `docs/0.3-real-project-transcription-report.md`.
- Parser/formatter fuzzing and property tests. ✅ Implemented in
  `src/lib/mod.rs::property_tests` with a seed-deterministic LCG and seven
  invariants: idempotent render, render determinism, AST JSON versioning,
  lossless no-panic on arbitrary bytes, error-status consistency,
  lossless/semantic parser equivalence, and tokenize-then-parse equivalence.
  Gated in CI as the `Property & Fuzz Tests` job.
- Large-file performance and memory baseline. ✅ Measured on release builds
  with `examples/perf_baseline.rs`; deterministic slot-linearity guard added
  as `stress_tests::stress_slot_count_scales_linearly_with_module_size`. The
  baseline now also measures all four real MIMI transcriptions, QueueTree/IDE
  snapshot construction, and a 200-change sequential UTF-16 batch that parses
  only once.

  | Module size | Source bytes | parse | render | reparse | parse_lossless | slots |
  |-------------|-------------:|------:|-------:|--------:|---------------:|------:|
  | 500 funcs   | 74 KB       | 2.8 ms | 0.8 ms | 2.8 ms | 4.6 ms | 10,002 |
  | 1,000 funcs | 149 KB      | 3.4 ms | 1.4 ms | 4.1 ms | 9.3 ms | 20,002 |
  | 2,000 funcs | 299 KB      | 6.8 ms | 3.4 ms | 8.5 ms | 22.8 ms | 40,002 |

  Each func yields ~20 commitment slots; slot count is linear in module size
  within ±5%. Parsing is linear in source bytes. These numbers are the
  published M5 regression budget — a future commit that regresses them
  by more than 2× on the same hardware class is a release blocker.
- Public API and diagnostic-code freeze. ✅ Documented in
  [`api-stability-0.3.md`](api-stability-0.3.md). Tier 1 (parser entry
  points, result types, AST shape, `AST_SCHEMA_VERSION`, and `ErrorCode`
  assignments) is frozen for the remainder of 0.3.x. Tier 3 experimental
  modules (materialize, profile, workflow, session, ide, diagnostics) are
  explicitly excluded from the freeze.
- Language-neutral conformance suite. ✅ `mimispec conformance check` validates
  parse/AST goldens, lossless attachment/span facts, the commitment transition
  matrix, and an LSP transcript under `mimispec.conformance/0.3`.
- Internal real-project usability follow-ups. ✅ The four-project
  `mimi-kv`/`mimichat`/`mimi-markdown`/`mimi-log` trial now has contextual
  E0010 guidance, conservative standard-LSP quick fixes, hierarchical
  QueueTree presentation, VS Code tree navigation, and Human-only atomic queue
  batching. Flat queue fields and `--flat-queues` remain available for
  compatibility. This closes the internal technical P1s but does not
  substitute for independent authors.
- Independent usability gate. ⏳ The release workflow requires five independent
  authors, 25 final documents across five domains, four successful five-minute
  entries, exact lossless/semantic round-trip, and zero open P0/P1. The
  machine-readable manifest is currently `in_progress` with 0/5 authors,
  0/25 documents, 0/5 domains, and 0/4 five-minute successes. This
  intentionally blocks an RC, but not a clearly labelled development snapshot.

### Acceptance

- No known silent intent-loss bug remains.
- `cargo test`, clippy, fmt check, fuzz/property gates, and stress corpus pass.
- Parser performance regression is within the published budget.
- Migration documentation distinguishes syntax compatibility from corrected
  semantic interpretation.
- Five-minute entry still requires only `desc`, `rule`, and optionally `??`.

## 11. Features Deferred Beyond 0.3.x

The following may consume the stable 0.3 parser later, but do not participate
in the 0.3 language freeze:

- target profiles and target capability matrices;
- code generation or implementation synchronization;
- materialization and release planning;
- production Evidence/provenance ledgers and target adapters (the experimental
  Core-external `mimispec.provenance/0.1` hash/locator sidecar may be evaluated
  without entering the Core freeze);
- OSE product workflow;
- target-specific formal verification;
- target-language-specific Flow, Fault, Actor, Session, or FFI syntax.

No deferred feature may require reinterpreting `desc`, `rule`, Flow openness,
or commitment suffixes.

## 12. Required Test Families

| Family | Required property |
|--------|-------------------|
| Context | root desc/rule/requires/ensures classify correctly and retain cross-kind order |
| Description | repeated descriptions retain order and state |
| Clause | repeated clauses never overwrite and default to conjunction |
| Rule | every item kind is attachable; only physical blank lines split chains; environment attachment is stable |
| Flow | anonymous/named and labelled/unlabelled forms round-trip |
| Commitment | nine states, actor matrix, inheritance, challenge |
| Lossless | comments, blank lines, spans, no-op edit |
| Recovery | partial AST explicitly marked; no silent loss |
| Compatibility | 0.2.1 valid corpus still parses |
| Adversarial | formatter/patch cannot move or bypass protected slots |
| Multilingual | Unicode descriptions and rules preserve exact content. ✅ Covered by `src/lib/mod.rs::multilingual_tests` (CJK byte-exact round-trip, emoji/punctuation, NFC/NFD non-normalization, Unicode-scalar columns, seven-script corpus). |

## 13. Documentation Synchronization

Every normative language change must update together:

- `docs/specification.md`;
- `docs/0.3.x-design-zh.md`;
- `docs/commitment-state-machine.md` when suffix footprint changes;
- `docs/migration-0.2-to-0.3.md`;
- parser/AST tests and versioned JSON schema;
- README syntax summaries only after implementation is available;
- CHANGELOG only when behavior lands or a formal design decision must be
  recorded as draft.

Planning documents, local evaluation material, tutorials, editor UI, and
external target documents cannot be the sole normative source of Core meaning.
