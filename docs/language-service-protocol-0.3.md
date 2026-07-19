# MimiSpec Language Service Protocol 0.3

> Wire schema: `mimispec.ls/0.3`. Transport in 0.3.0 is LSP 3.17 over stdio
> only. Rust `ide`, `session`, and `lsp` implementation types are not frozen.

## Document state

Each open URI owns three revision layers:

- **observed**: the editor text most recently reported by `didChange`;
- **authoritative**: the last actor-declared revision admitted by collaboration
  validation;
- **pending**: one accepted edit candidate waiting for the client to apply its
  returned `WorkspaceEdit`.

Revision and protected-region authority uses SHA-256. The legacy 64-bit
`ContentHash` remains a compatibility helper and is not a transaction digest.

Workspace setting `mimispec.collaborationMode` is `advisory` by default:

- `advisory` accepts observed edits but emits `C-ACTOR-REQUIRED` and any lock
  violations; the revision is explicitly untrusted until an actor-declared
  request is confirmed;
- `strict` keeps unsolicited text only as observed state. It cannot replace
  the authoritative revision. Code actions are disabled while the two diverge.

Switching an untrusted advisory document to strict requires a Human actor to
call `mimispec/adoptObservedRevision` or
`mimispec/restoreAuthoritativeRevision`. Neither operation implicitly trusts
an omitted actor or a stale `base_version`.

## Standard LSP surface

`mimispec lsp --stdio` implements initialize/shutdown/exit, incremental UTF-16
text synchronization, publishDiagnostics, semanticTokens/full, hover,
definition, references, and codeAction. Incremental synchronization currently
reparses the whole document; it is not advertised as an incremental parser.

Semantic tokens cover Context Item kind, all nine commitment states, and rule
attachment. Hover includes local and inherited effective protection.
Definition/references navigate rule attachments and Flow state targets.

## Frozen custom methods

| Method | Purpose |
|---|---|
| `mimispec/documentSnapshot` | Observed/authoritative versions, queues, confirmed slots, challenges and diagnostics |
| `mimispec/applyDocumentEdit` | Validate an actor-declared UTF-16 edit batch and return a transaction plus `WorkspaceEdit` |
| `mimispec/issueUnlockToken` | Human-only, revision- and slot-bound strong-lock token |
| `mimispec/adoptObservedRevision` | Human-authorized promotion of current observed text |
| `mimispec/restoreAuthoritativeRevision` | Human-only transaction returning a full-document edit that restores trusted text |
| `mimispec/slotNavigation` | Target-neutral rule and Flow slot navigation |

`applyDocumentEdit` requires `uri`, `base_version`, `actor`, `edits`,
`authorization`, `unlock_tokens`, and optional `challenge_reason`. Its response
contains `accepted`, authoritative version, candidate hash, transaction ID,
violations, and, when accepted, a `workspace_edit`.

An accepted transaction becomes authoritative only after a subsequent
`didChange` produces the exact candidate SHA-256. A candidate with parser
errors is rejected as `C-PARTIAL-CANDIDATE`.

The frozen request fields are:

| Method | Required fields |
|---|---|
| `documentSnapshot` | `uri` |
| `applyDocumentEdit` | `uri`, `base_version`, `actor`, `edits`, `authorization`, `unlock_tokens` |
| `issueUnlockToken` | `uri`, `base_version`, `actor: "human"`, `slot` |
| `adoptObservedRevision` | `uri`, `base_version`, `actor: "human"`, `authorization`, `unlock_tokens` |
| `restoreAuthoritativeRevision` | `uri`, `base_version`, `actor: "human"` |
| `slotNavigation` | `uri`, UTF-16 `position` |

Custom operation responses carry `schema_version`, `accepted`, the current
`authoritative_version`, and `violations`. Edit/restore responses additionally
carry `candidate_hash`, `transaction_id`, and an accepted `workspace_edit`.
Unlock-token responses carry the bound token object. Adoption validates
protected edits and strong-lock tokens before promotion; it is not a bypass
for the collaboration state machine.

## Frozen collaboration codes

```text
C-ACTOR-REQUIRED
C-STALE-REVISION
C-DOCUMENT-DIVERGED
C-INVALID-EDIT
C-PARTIAL-CANDIDATE
C-AI-TRANSITION-FORBIDDEN
C-PROTECTED-CONTENT
C-PROTECTED-STRUCTURE
C-PROTECTED-ATTACHMENT
C-CHALLENGE-REASON-REQUIRED
C-HUMAN-AUTHORIZATION-REQUIRED
C-STRONG-UNLOCK-REQUIRED
```

Actor declarations are a local collaboration trust contract, not operating
system authentication. Strict mode cannot prevent an external process from
changing a file; it prevents that change from silently replacing the language
server's authoritative revision.
