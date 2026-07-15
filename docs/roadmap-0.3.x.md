# MimiSpec 0.3.x Development Roadmap

> Status: planning
>
> Current released version: `0.2.1`
>
> Scope: `0.3.0` through the final `0.3.x` stabilization release

## 1. Series Goal

MimiSpec 0.3.x turns the existing commitment suffixes from parseable metadata
into a documented and enforceable human-AI collaboration protocol.

The series does not turn MimiSpec into a programming language. The five-minute
entry point remains unchanged:

```mimispec
desc?? "我想做一个帮助家庭记录日常开销的应用"

rule "老人也可以轻松使用"
rule "财务数据默认只保存在本地"
```

The defining model of 0.3.x is:

```text
natural-language intent
    + Fragment/slot structure
    + paragraph-scoped constraints
    + commitment state transitions
    = an evolving human-AI specification
```

Mimi remains the native and first-party production target. MimiSpec Core stays
target-language independent so the same `.mms` can constrain Rust, TypeScript,
Python, and other implementation languages through target profiles.

## 2. Non-Negotiable Invariants

Every 0.3.x release must preserve these invariants.

1. Five-minute usability: a non-programmer can begin with `desc`, `rule`, and
   `??` without learning modules, types, functions, or formal contracts.
2. Fragment-first legality: an individual Fragment or meaningful partial tree
   remains a valid file.
3. Natural language is first-class intent, not a degraded fallback.
4. Blank lines retain their paragraph meaning for `rule` attachment.
5. Without a lock suffix, `?`/`??` describe the content. After `$`/`$$`, they
   describe whether the lock itself is ready.
6. `$?`, `$??`, `$$?`, and `$$??` continue to protect their content.
7. AI may challenge `$` as `$?`, and `$$` as `$$?`, only when content is
   unchanged and a reason is supplied.
8. AI cannot remove or weaken `$`/`$$`; strong lock confirmation and unlock are
   human-only operations.
9. Lock state and implementation evidence remain separate concepts.
10. A target profile must report unsupported intent instead of silently
    dropping it.

## 3. Version Structure

The series is deliberately ordered from language semantics to tooling. Later
releases must not invent workflow behavior before earlier releases freeze its
meaning.

| Version | Theme | Primary Result |
|---------|-------|----------------|
| `0.3.0` | Semantic foundation | Freeze the commitment state machine and paragraph constraint semantics |
| `0.3.1` | Lossless document model | Preserve suffix locations, paragraph boundaries, attachments, and source spans |
| `0.3.2` | Transition and patch validation | Enforce actor permissions and content-preserving lock challenges |
| `0.3.3` | Intent diagnostics | Report collaboration states, attachment risks, conflicts, and unresolved decisions |
| `0.3.4` | IDE protocol | Expose semantic tokens, code actions, decision queues, and incremental document state |
| `0.3.5` | Materialization core | Define commit-ready selection, provenance, partial materialization, and evidence records |
| `0.3.6` | Native Mimi profile | Add first-party Mimi gap analysis, generation planning, validation, and evidence ingestion |
| `0.3.7` | Generic target profiles | Stabilize a language-neutral profile API and capability reporting |
| `0.3.8` | OSE workflow integration | Drive collaboration by slot states instead of a single document phase |
| `0.3.9` | Compatibility and release candidate | Migration, corpus freeze, performance, security, and API stabilization |

Patch releases may be inserted when a milestone needs stabilization. The
semantic dependency order must remain intact.

## 4. v0.3.0: Semantic Foundation

### Goal

Publish the normative meaning of the nine suffix combinations and make it the
single source of truth for parser, IDE, OSE, and AI agents.

### Deliverables

- `docs/commitment-state-machine.md` as the normative collaboration model.
- Correct definitions for `$?`, `$??`, `$$?`, and `$$??`.
- Actor transition matrix for Human and AI; tooling must proxy one of those
  actors with the corresponding authorization context.
- Definition of content-preserving lock challenge:
  - `$ -> $?`
  - `$$ -> $$?`
- Definition of keyword, identifier, and value suffix slots.
- Definition of ordinary lock versus strong lock propagation.
- Definition of commit-ready state: only `$` and `$$` without `?`.
- Explicit separation of commitment from implementation evidence.
- Specification version labels corrected to distinguish released and draft
  behavior.

### Implementation

- Keep the serialized nine-value `Commitment` representation compatible.
- Add semantic decomposition APIs without breaking current JSON:

```rust
commitment.lock_intent()
commitment.review_intent()
commitment.review_target()
commitment.protects_content()
commitment.is_commit_ready()
```

- Correct `has_question_question()` so `$??` and `$$??` are represented by the
  public semantic API instead of being treated as plain `??` only.
- Add exhaustive table-driven tests for all nine states.

### Acceptance

- Every suffix has one normative interpretation across specification, README,
  CLI JSON, OSE prompts, and editor hover text.
- All 9 x actor transition cases are tested.
- No existing valid `0.2.1` source becomes syntactically invalid.
- `cargo test`, release stress tests, clippy, and round-trip tests pass.

## 5. v0.3.1: Lossless Document Model

> Implementation status: the first opt-in source layer is now under development.
> `parse_lossless()` preserves exact source pieces, comments, whitespace,
> LF/CRLF/CR, physical blank-line paragraph breaks, explicit suffix occurrence
> spans, parser-proven suffix slots including zero-width insertion ranges,
> source-derived rule group candidates, and revision-local source nodes with
> movable spans that carry attached rule preludes. Nested Field, FlowEntry,
> FlowArm, and Step nodes now receive revision-local IDs so rule targets and
> comments can resolve beyond top-level Fragments. Parser-authoritative rule
> occurrences expose unique IDs, exact spans, attachment decisions, target
> anchors, scope anchors, and nested target IDs when available. Line comments
> are classified as trailing, leading, or free with optional target node IDs.
> Structured Fragment moves (`move_fragment` / `move_fragment_reparse`) carry
> attached rule preludes, and formatter attachment fingerprints check that
> semantic render does not reattach or drop rules.

### Goal

Represent the document semantics needed by collaboration tools without relying
on newline-count heuristics or lossy rendering.

### Deliverables

- Explicit `BlankLine` or `ParagraphBreak` representation in the token or
  lossless syntax layer.
- Stable rule attachment information.
- Source spans for:
  - keyword commitment;
  - identifier commitment;
  - string/value commitment;
  - attached rule groups;
  - paragraph boundaries.
- Lossless syntax tree or source map alongside the semantic AST.
- Formatter guarantee that rule attachment and suffix targets do not change.

### Compatibility

The existing semantic AST remains available. New lossless structures are added
as an opt-in API for IDE and collaboration clients.

### Acceptance

- Parse-render-parse preserves semantic AST and rule attachment.
- Formatting cannot attach or detach a rule.
- Moving a Fragment through the structured edit API carries its attached rule
  prelude.
- Comments and blank lines survive lossless round trips.

## 6. v0.3.2: Transition and Patch Validation

> Implementation status: early foundation is landing. Protected node hashes,
> `LockChallenge` construction, identical-challenge deduplication, and
> before/after AI document patch validation (`validate_ai_document_patch`) are
> available on top of the 0.3.0 transition validator. Human-only `UnlockToken`
> issuance and parent/child lock propagation checks are also available. Richer
> multi-slot patch provenance remains in progress.

### Goal

Make the suffix state machine enforceable rather than prompt-only convention.

### Deliverables

- Stable slot identity within a document revision.
- Content hashes for protected slots and subtrees.
- `TransitionRequest` and `TransitionDecision` APIs.
- Actor-aware transition validation.
- Structured edit validation for AI patches.
- Strong-lock explicit unlock tokens or user-authorized operations.
- `LockChallenge` record with reason, evidence, affected targets, and content
  hash.

### Hard Rules

- `$ -> $?` and `$$ -> $$?` are allowed for AI only if protected content is
  byte-for-byte or semantically unchanged according to the selected edit mode.
- A suffix-only challenge cannot move, rename, reorder, or reattach the node.
- AI cannot transition into final `$$` or out of any strong-lock state.
- Human decisions remain auditable but do not require Git to function.

### Acceptance

- Malicious or accidental lock-bypass patches are rejected.
- Valid suffix-only challenges are accepted and explained.
- Parent/child lock propagation has exhaustive tests.
- Challenge deduplication prevents repeated identical objections without new
  evidence.

## 7. v0.3.3: Intent Diagnostics

> Implementation status: first wave is available via `diagnostics::analyze_document`
> and `mimispec diagnose` / `--diagnostics`. It builds Decision/Delegation queues,
> a commitment-state summary, syntax/attachment diagnostics, rule-commitment
> conflict heuristics, Flow/Func intent-gap hints, and stable codes such as
> `I-DECISION` / `W-INTENT-CONFLICT` / `H-INTENT-GAP`. Versioned JSON schema and
> deeper semantic conflict analysis remain later work.

### Goal

Shift diagnostics from parser-only errors toward useful specification guidance.

### Diagnostic Classes

1. Syntax: source cannot be parsed.
2. Attachment: a `rule` is attached differently than likely intended.
3. Collaboration: an edit violates a lock or actor transition rule.
4. Decision: `?`, `$?`, or `$$?` is awaiting human review.
5. Delegation: `??`, `$??`, or `$$??` is awaiting AI work.
6. Intent conflict: rules, flows, steps, or UI behavior contradict each other.
7. Intent gap: an important success, failure, permission, or state case is not
   described.
8. Target gap: a selected implementation profile cannot satisfy an intent.

### Deliverables

- Stable diagnostic codes and JSON schema.
- Human-readable fixes and machine-readable code actions.
- Commitment-state summaries per document and project.
- Better fuzzy suggestions wired into real diagnostics.

### Acceptance

- Diagnostics preserve code, severity, span, help, suggestion, and related
  Fragment IDs.
- Syntax-valid but unresolved documents produce guidance, not false errors.
- The five-minute workflow never requires advanced syntax to clear a warning.

## 8. v0.3.4: IDE Protocol

> Implementation status: library-level protocol helpers are landing in
> `ide::{semantic_tokens, hover_at, code_actions_for_node, ide_snapshot}` and
> `session::DocumentSession` for versioned full-text document state. They reuse
> the same transition validator and diagnostics queues as Core. A long-running
> LSP server and editor wiring remain later work.

### Goal

Provide the language services required by OSE, VS Code, and Monaco without
embedding product-specific policy in the parser.

### Deliverables

- LSP or equivalent long-running language service.
- Incremental document updates based on the existing cache.
- Semantic tokens for suffix states and rule attachment.
- Hover explanations for the exact suffix slot.
- Code actions:
  - ask AI for content candidates;
  - mark content as `?` or `??`;
  - propose `$?` or `$$?`;
  - accept lock as `$` or `$$`;
  - challenge a lock without changing content;
  - show rule scope.
- APIs for Decision Queue and Delegation Queue.

### Acceptance

- Editor behavior uses the same transition validator as the library.
- No editor can bypass strong-lock policy through raw structured edits.
- Rule scope and lock scope are visually inspectable.

## 9. v0.3.5: Materialization Core

> Implementation status: core library types and planning APIs are landing in
> `materialize::{select_commit_ready, plan_materialization, detect_drift,
> validate_plan, EvidenceRecord}` plus `mimispec materialize`. Target adapters
> and profile-specific generation remain later work.

### Goal

Define how confirmed intent becomes target artifacts without coupling Core to a
specific programming language.

### Deliverables

- `CommitSelection`: which locked slots are in the current materialization
  scope.
- Partial materialization rules.
- Provenance categories:
  - human locked;
  - human strong-locked;
  - target-derived;
  - implementation choice;
  - unresolved;
  - generated test.
- `EvidenceRecord` schema.
- Drift detection between locked intent and generated artifacts.
- Release scope separate from whole-document lock percentage.

### Acceptance

- Unlocked intent never appears as confirmed target behavior.
- Target-derived scaffolding is distinguishable from authored requirements.
- Code changes can propose new MMS candidates but cannot auto-lock them.

## 10. v0.3.6: Native Mimi Profile

> Implementation status: capability probing and gap reporting land in
> `profile::{analyze_mimi_profile, analyze_generic_profile}` and
> `mimispec profile`. Actual `.mimi` generation remains an external adapter.

### Goal

Make Mimi the first-party, deepest target while keeping MimiSpec Core generic.

### Profile Responsibilities

- Map modules, type hints, functions, conditions, steps, flows, and capabilities
  into Mimi implementation candidates.
- Use Mimi's Flow, Fault, recovery, capability, protocol, session, and contract
  model to identify missing decisions.
- Ask natural-language clarification questions rather than requiring users to
  write Mimi-level mechanics.
- Generate a materialization plan and `.mimi` source through a separate target
  tool or adapter.
- Ingest evidence from Mimi check, verify, run, build, and tests.

### Boundary

Mimi-specific mechanics remain profile output. They do not become mandatory
MimiSpec surface syntax.

The profile is an external bridge between two independent toolchains. It must
not be implemented by routing through Mimi's `mms {}` super-comment block.

### Acceptance

- Every generated Mimi construct records its source MMS slots or its
  target-derived origin.
- Unsupported or ambiguous intent is reported before generation.
- Generated source passes the configured Mimi gates or returns structured
  evidence explaining why it does not.

## 11. v0.3.7: Generic Target Profile API

### Goal

Open materialization to other programming languages without reducing MimiSpec
to a lowest-common-denominator model.

### Deliverables

- Stable target profile trait/protocol.
- Capability declaration for structure, contracts, behavior, concurrency,
  resource safety, formal verification, runtime evidence, and round-trip sync.
- Conformance tests.
- At least one non-Mimi reference profile, preferably TypeScript or Rust.

### Acceptance

- A profile must explicitly report partial support.
- The same locked rule can yield different target obligations while preserving
  its original natural-language meaning.

## 12. v0.3.8: OSE Workflow Integration

### Goal

Replace the single linear Draft/Review/Lock model with a state-driven queue over
all slots while retaining simple product-level milestones.

### Deliverables

- Decision Queue for `?`, `$?`, and `$$?`.
- Delegation Queue for `??`, `$??`, and `$$??`.
- Lock Challenge review.
- Semantic diff that separates content changes from state transitions.
- Materialization and evidence view.
- Release-scope readiness instead of mandatory whole-document closure.

### Acceptance

- OSE schedules work from slot states.
- An AI lock challenge appears as a review task, not an unauthorized content
  edit.
- Human rejection of a challenge is remembered until new evidence appears.

## 13. v0.3.9: Stabilization

### Goal

Freeze the 0.3 semantic and API surface before considering 0.4.

### Deliverables

- `0.2.1 -> 0.3.x` migration guide.
- Golden corpus covering beginner, advanced, malformed, multilingual, and
  large-project inputs.
- JSON schema versioning.
- Performance and memory baselines.
- Security review of resolver, patch validation, lock bypass, and profile
  execution boundaries.
- API deprecation report.
- Documentation consistency audit.

### Acceptance

- Existing 0.2.1 corpus parses with documented compatibility behavior.
- No known content-changing AI path bypasses `$` or `$$`.
- All public examples are verified by CI.
- Specification, tutorial, README, CLI, editor, and OSE use the same suffix
  semantics.

## 14. Features Explicitly Deferred Beyond 0.3.x

- Adding target-specific MIMI Flow/Fault/Session syntax to MimiSpec Core.
- Treating natural-language rules as automatically proven formulas.
- Requiring a fully locked document before any local materialization.
- Automatic locking of AI-generated content.
- Making production code a competing source of truth.
- Freezing a `1.0` grammar before the collaboration protocol is validated in
  real OSE workflows.

## 15. Testing Strategy

0.3.x adds four invariant layers to the existing parser tests.

| Layer | Invariant |
|-------|-----------|
| S1 Syntax | Valid 0.2.1 source remains parseable |
| S2 Document | Round trips preserve suffix targets and rule attachment |
| S3 Collaboration | Actor transitions and protected-content hashes are enforced |
| S4 Materialization | Only selected locked intent is emitted as confirmed behavior |

Required test families:

- exhaustive nine-state suffix tables;
- all Human/AI transition pairs;
- blank-line and comment attachment corpus;
- formatter semantic-equivalence property tests;
- parent/child lock propagation;
- adversarial patch and lock-bypass tests;
- profile capability and unsupported-intent tests;
- beginner five-minute examples;
- multilingual natural-language content;
- OSE end-to-end state evolution.

## 16. Documentation Plan

The following documents are normative or required before the corresponding
release:

| Document | Purpose | Target |
|----------|---------|--------|
| `commitment-state-machine.md` | Normative suffix and actor-transition semantics | `0.3.0` |
| `specification.md` | Surface grammar and current/draft status | every release |
| `migration-0.2-to-0.3.md` | Compatibility and API migration | `0.3.9` |
| `target-profile-api.md` | Generic target integration contract | `0.3.7` |
| `mimi-profile.md` | First-party Mimi mapping and evidence model | `0.3.6` |
| `ose-integration.md` | Decision/delegation/materialization workflow | `0.3.8` |

Every implemented feature must update the specification, README, changelog,
tests, and editor-facing JSON schema in the same release.
