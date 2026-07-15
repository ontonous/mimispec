# Migrating from MimiSpec 0.2.1 to 0.3.x

> Status: draft for the 0.3.x development series on `main`.
>
> Published crates.io release remains `0.2.1` until an explicit 0.3.0 package
> release is cut.

## 1. Compatibility promise

- Valid `0.2.1` `.mms` source remains parseable.
- Existing semantic AST JSON field names for core Fragments stay compatible.
- New APIs are additive opt-ins:
  - `parse_lossless`
  - `collaboration::*`
  - `diagnostics::*`
  - `ide::*`
  - `session::*`
  - `materialize::*`
  - `profile::*`
  - `workflow::*`

## 2. What changes for library users

### 2.1 Keep using `parse` for 0.2.1 workflows

```rust
let result = mimispec::parse(source);
// still correct for syntax validation and semantic AST access
```

### 2.2 Opt into lossless collaboration state

```rust
let lossless = mimispec::parse_lossless(source);
let report = mimispec::diagnostics::analyze_document(&lossless.document, &lossless.errors);
let board = mimispec::workflow::build_workflow_board(&lossless.document, "release-1", &[]);
```

### 2.3 Commitment semantics

The nine suffix values are unchanged as text. Their normative meaning is now
documented:

| Suffix | Meaning |
|--------|---------|
| none | open content |
| `?` | content review |
| `??` | content delegation |
| `$` | ordinary lock (commit-ready) |
| `$?` | lock review (content protected) |
| `$??` | lock assessment delegated (content protected) |
| `$$` | strong lock (commit-ready) |
| `$$?` | strong-lock review |
| `$$??` | strong-lock assessment delegated |

Use the semantic helpers instead of ad-hoc suffix string checks:

```rust
use mimispec::ast::Commitment;
assert!(Commitment::Locked.is_commit_ready());
assert!(Commitment::LockedQuestion.protects_content());
```

### 2.4 Renderer attachment behavior

Fragment-level rules now render as **front preludes** so parse-render-parse
keeps attachment semantics:

```mimispec
rule "audit"
func Pay:
    steps:
        charge payment
```

rather than placing the rule only inside the function body.

## 3. CLI additions

```bash
mimispec diagnose file.mms
mimispec materialize file.mms --scope payments-v1
mimispec profile file.mms --target mimi|generic|rust|typescript
mimispec workflow file.mms --scope payments-v1
```

These commands are development-facing on `main`. They are not part of the
published `0.2.1` CLI contract until a 0.3 release is tagged.

## 4. Editor / OSE integration path

1. Parse with `parse_lossless` or open a `session::DocumentSession`.
2. Read decision/delegation queues from diagnostics or workflow boards.
3. Validate AI edits with `collaboration::validate_ai_document_patch`.
4. Plan materialization only from commit-ready slots.
5. Probe target profiles before generation; never silently drop unsupported
   locked intent.

## 5. Non-goals that remain true

- Locked does **not** mean verified, tested, or deployed.
- AI cannot finalize `$$` or unlock strong locks.
- Natural-language `rule` text is not automatically a formal proof.
- Mimi is an optional first-party target, not the host language of MimiSpec.

## 6. Suggested migration checklist

- [ ] Keep existing `parse` call sites working.
- [ ] Add lossless/diagnose only where collaboration UI needs queues.
- [ ] Replace local suffix interpretation with `Commitment` helpers.
- [ ] Re-run golden corpus / project fixtures under `cargo test --lib`.
- [ ] If you render AST back to source, re-check rule attachment fingerprints.
