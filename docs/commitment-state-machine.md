# MimiSpec Commitment State Machine

> Status: normative `0.3.0-dev` Core contract; semantic-slot identities,
> footprints and the actor/patch validator are implemented on the unreleased
> main development line and are ready for development-snapshot evaluation
>
> Current published implementation and Cargo version: `0.2.1`; this document
> does not claim an RC or stable 0.3 release

## 1. Purpose

MimiSpec commitment suffixes are not decorative confidence labels. They are the
control surface through which humans and AI continuously evolve a document.

The suffix system answers two different questions:

1. What is the current content decision state?
2. Is the proposed lock mature enough to apply or retain?

The position of `?`/`??` determines which question is being asked.

## 2. Composition Rule

Let `A` be the content of a suffix-bearing slot.

Without a preceding lock suffix:

```text
A?  = Question(A)
A?? = Delegate(A)
```

After a lock suffix:

```text
A$?    = Question(Lock(A))
A$??   = Delegate(Lock(A))
A$$?   = Question(StrongLock(A))
A$$??  = Delegate(StrongLock(A))
```

Therefore `A$?` does not mean that A's content is uncertain. It means the human
believes A may be ready for an ordinary lock but asks AI to judge whether that
lock is truly ready or whether A still needs work.

Likewise, `A$$?` means the human believes A may be ready for a strong lock but
asks AI to judge whether strong locking is justified.

The fixed order, lock before question, follows directly from this composition.
Forms such as `A?$` would lock uncertain content rather than question the lock
decision and are invalid.

## 3. The Nine Surface States

| Suffix | State Name | Meaning |
|--------|------------|---------|
| none | `Open` | Current collaborative draft; AI may propose or make permitted edits |
| `?` | `ContentReview` | Content has a direction but needs options or review |
| `??` | `ContentDelegated` | Content decision is delegated to AI |
| `$` | `Locked` | Content and ordinary-lock decision are confirmed |
| `$?` | `LockReview` | Content is protected; ordinary-lock readiness needs review |
| `$??` | `LockDelegated` | Content is protected; ordinary-lock readiness is delegated to AI |
| `$$` | `StrongLocked` | Content and strong-lock decision are human-confirmed |
| `$$?` | `StrongLockReview` | Content is protected; strong-lock readiness needs review |
| `$$??` | `StrongLockDelegated` | Content is protected; strong-lock readiness assessment is delegated to AI |

All states containing `$` protect their current content. A trailing `?` or `??`
opens discussion about the lock, not permission to modify the protected content.

## 4. Slot Granularity

The state machine applies to every suffix-bearing semantic slot, not only to a
whole Fragment.

```mimispec
func?? Pay$:
    desc "处理支付"?
```

This contains three independent states:

- `func??`: the entity form is delegated; AI may suggest a function, flow, or
  decomposition;
- `Pay$`: the name is locked;
- `"处理支付"?`: the description content needs review.

Keyword, identifier, and value suffixes must retain separate source spans and
must not be moved between slots by formatting.

### 4.1 Semantic footprint and inheritance

A suffix anchor and its semantic footprint are related but not identical. The
parser/lossless layer must describe which semantic fields a commitment governs;
consumers must not infer the footprint by scanning a header string.

| Anchor | Default semantic footprint |
|--------|----------------------------|
| keyword | entity existence/kind; unsuffixed fields in the same atomic declaration or header; for a container, its existing child-list topology |
| identifier | that name/reference and its semantic identity |
| value | that literal or natural-language value |
| clause | one `requires` / `ensures` condition and its clause kind; repeated clauses never share one slot |
| event | one Flow edge's optional event identity, separate from target and guard |
| attachment | the rule-to-target or comment-to-target relation, not the rule text itself |

The default is refined by these precedence rules:

1. An explicit suffix on a more specific identifier, value, child, or
   attachment slot overrides an inherited state for that footprint.
2. Ordinary `$` on an atomic declaration covers its unsuffixed declaration
   fields. On a container it covers the header and child-list topology, but it
   does not recursively confirm unsuffixed descendant content.
3. Strong `$$` covers the existing subtree and effectively confirms/protects
   unsuffixed descendants.
4. Explicit `?`, `??`, `$?`, `$??`, `$$?`, or `$$??` on a descendant remains
   unconfirmed even inside a strong-locked ancestor.
5. An environment rule outside a locked subtree does not inherit the target's
   commitment merely because the rule is applicable to it.

For example:

```mimispec
rule$ "支付必须幂等"       // atomic rule text inherits the ordinary lock

func$ Pay:
    desc "支付步骤仍可演进" // ordinary container lock does not confirm this body content

module$$ Payment:
    desc "现有边界已强锁"   // unsuffixed descendant is effectively strong-confirmed
    desc?? "未来渠道扩展"   // explicit delegation remains open
```

An inherited/effective commitment is a semantic judgement with an explicit
ancestor or atomic-declaration basis. It is not serialized as if the author had
typed another suffix. `Commitment::is_commit_ready()` may remain as a
compatibility API name, but its normative meaning is only “directly confirmed”.

The unreleased main implementation exposes each parser-proven anchor as a
revision-local `CommitmentSlotId` with its `owner`, exact anchor/suffix spans and
`CommitmentFootprintKind`. `collect_semantic_slot_snapshots()` is the
authoritative collaboration view. The older node summary API is compatibility
only: consumers must not use a node-wide “strongest suffix” to authorize an
edit when `func?? Pay$` contains two independent states.

## 5. Current Meaning Versus Collaboration State

A question suffix changes collaboration state, not the current interpretation
of already written content.

For example:

```mimispec
rule$? "支付必须幂等"
func Pay:
```

The idempotency rule remains active and attached to `Pay`. `$?` means its lock
readiness is under review; it does not suspend the rule.

This preserves a usable current document at every stage.

## 6. Content Protection

The following states are content-protected:

```text
$
$?
$??
$$
$$?
$$??
```

For AI edits:

```text
content_before == content_after
```

must hold for a suffix-only lock review transition. Protected content includes
the slot's text, identity, attachment target, and structural position unless an
explicitly open descendant slot authorizes local elaboration.

## 7. Human and AI Transition Rights

### 7.1 AI-Permitted Transitions

| From | To | Conditions |
|------|----|------------|
| `??` | `?` | AI supplies candidates; human review remains |
| `??` | none | Delegation permits AI to form the current draft |
| `?` | `?` | AI revises or expands candidates |
| none | `?` | AI identifies unresolved content in an unlocked slot |
| `$` | `$?` | Content and structure unchanged; challenge reason required |
| `$$` | `$$?` | Content and structure unchanged; challenge reason required |
| `$??` | `$?` | AI completes ordinary-lock assessment and requests review |
| `$$??` | `$$?` | AI completes strong-lock assessment and requests human confirmation |

AI may recommend other transitions, but recommendations do not change the
source state until a human authorizes them.

A newly created semantic slot is validated as a transition from `none` too.
Therefore AI may create a fresh `?` review slot, but it may not create a fresh
`??` delegation or any lock-family state merely because that slot had no prior
revision identity.

### 7.2 AI-Forbidden Transitions

AI must not perform:

```text
$   -> none, ?, or ??
$$  -> $, none, ?, or ??
$?  -> none, ?, or ??
$$? -> $, $?, none, ?, or ??
any state -> $$
```

AI also must not combine `$ -> $?` or `$$ -> $$?` with content, name,
attachment, order, or location changes.

### 7.3 Human Transitions

Humans may authorize any transition. The following operations must be explicit
in collaboration tooling:

- removing `$$`;
- reducing `$$` to `$`;
- accepting a strong-lock candidate as `$$`;
- allowing protected content to change;
- rejecting or accepting an AI lock challenge.

### 7.4 Tooling Execution

Tooling is not an independent authorization actor. A formatter, CLI, IDE,
migration tool, or other automated client must execute as either Human or AI
and carry the corresponding authorization context. Tooling may analyze and
recommend transitions without authorization, but it may not create a third
permission path around the Human/AI matrix.

## 8. Lock Challenge

AI may challenge a confirmed lock without changing content:

```text
$  -> $?
$$ -> $$?
```

Every challenge must produce a record containing at least:

```text
LockChallenge {
    slot_id
    original_state
    challenged_state
    content_hash
    reason
    basis
    affected_slots
    suggested_actions
}
```

The transition is valid only when the protected content hash and protected
structure hash are unchanged.

An identical challenge rejected by a human must not be repeated until new
information changes the challenge fingerprint. `basis` is explanatory data for
human review, not a Core Evidence object.

## 9. Ordinary and Strong Lock

### Ordinary Lock `$`

An ordinary lock confirms the current slot and prevents AI from changing its
identity, name, node kind, position, order, container structure, or attached
rule prelude. Explicit `?` or `??` child slots may continue to evolve within
the locked boundary according to their own state.

### Strong Lock `$$`

A strong lock protects the node and its existing structural subtree, including
descendant slots without a suffix. Only explicit `?` or `??` child slots remain
open. Adding, deleting, moving, or reordering child slots is a protected
structural edit. Strong lock requires human confirmation to enter and explicit
human action to weaken or remove.

### Explicit Open Slots

A strong-locked container may intentionally contain delegated extension slots:

```mimispec
module$$ Payment:
    rule$$ "支付必须幂等"
    desc?? "未来支付渠道扩展"
```

AI may elaborate the explicit `desc??` slot but cannot alter its strong-locked
parent or siblings. This preserves safe extensibility inside frozen structure.

## 10. Rule Attachment and Commitment

Rule attachment and commitment are independent dimensions, but an attached
rule prelude belongs to the target entity's protected structural boundary.
An attachment keeps its own identity and hash even when its protection or
confirmation basis is inherited from a locked atomic rule or target boundary.

Attachment never moves the Rule out of its scope's ordered item sequence. The
target's `attached_rules()` view is derived from an explicit relation; it is
not a second authoritative copy of the Rule. This prevents a suffix-only edit
or formatter from changing narrative order while preserving only tree shape.

```mimispec
rule$$? "支付必须幂等"
func Pay:
```

The rule is attached to `Pay` according to paragraph semantics. Its content is
protected, and the strong-lock decision is under review. The attachment remains
active unless a human-authorized structural edit changes the paragraph.

A paragraph-separated, targetless, or scope-terminal rule has an Environment
attachment to its current scope. That attachment is equally explicit and must
not be rewritten as dangling or silently rebound to a later entity.

For malformed source, `UnresolvedByRecovery` may be recorded only together
with a parser diagnostic and partial-document status. It is not a valid third
attachment state and does not receive commitment semantics as if lowering had
succeeded.

Only a physical blank line creates the ParagraphBreak used here. A comment-only
line is trivia and does not by itself change the attachment relation.

Formatters and structured editors must not change rule attachment as a side
effect of suffix transitions.

## 11. Confirmed States

Only final lock states mean that the current intent decision is confirmed:

```text
Locked       ($)
StrongLocked ($$)
```

Review and delegated lock states are not confirmed:

```text
$?
$??
$$?
$$??
```

Confirmed means only that the human intent decision is closed at the stated lock
strength. It does not claim that code exists, tests pass, a theorem is proven,
or a product is ready. Those are external facts and are outside this state
machine.

`$?`, `$??`, `$$?`, and `$$??` protect content but remain unconfirmed because
the lock decision is still under review. A confirmed parent cannot hide an
explicitly unconfirmed child slot.

The existing API name `is_commit_ready()` may be retained during migration, but
documentation and new APIs should prefer `is_confirmed()`.

## 12. Commitment and External Facts

Commitment answers only:

> Has the human confirmed this intent and its lock state?

Parser results, implementation state, tests, runtime observations and formal
verification are external facts. They may motivate a lock challenge, but they
are not suffix states and cannot mutate commitment automatically. MimiSpec Core
does not define an Evidence, materialization or release protocol in 0.3.

## 13. Document-Level Evolution

A `.mms` document is not in one global commitment state. It is a forest of slot
state machines:

```text
project goal       $$
payment rule       $?
refund rule        ?
database choice    ??
Pay name           $
Pay steps          none
```

Tools should derive work queues from this distribution:

- Decision Queue: `?`, `$?`, `$$?`;
- Delegation Queue: `??`, `$??`, `$$??`;
- Confirmed Intent view: `$`, `$$`;
- Challenge Queue: AI-originated `$ -> $?` and `$$ -> $$?`.

Flat queues are the compatibility view. Tools may derive a source-ordered
scope tree, but every exact semantic slot must occur once and parent summaries
must not rewrite child state. A Human batch operation may resolve current
review/delegation slots to `none`, `?`, or `$` only after validating the whole
suffix-only candidate atomically. AI batches, `$$` targets, strong-lock
weakening, stale/unknown IDs, and partial success are forbidden.

The document evolves through local transitions. A product-level Draft, Review,
or Release label is only a summary, not the language state itself.

## 14. Serialization Compatibility

The existing nine-value serialized `Commitment` representation may remain
stable in 0.3.x. Implementations should expose semantic decomposition:

```text
LockIntent: None | Ordinary | Strong
ReviewIntent: None | Question | Delegate
ReviewTarget: Content | Lock
```

`ReviewTarget` is derived as follows:

```text
lock == None  => Content
lock != None  => Lock
```

This prevents consumers from interpreting `$?` as locked uncertain content.

## 15. Required Invariants

1. Trailing `?`/`??` targets content only when no lock suffix is present.
2. Every state containing `$` protects current content.
3. AI lock challenges preserve content and structure.
4. AI cannot finalize or remove a strong lock.
5. Question states do not suspend current document meaning.
6. Commitment remains separate from every external implementation or
   verification fact.
7. Formatter and round-trip operations preserve suffix slot and rule attachment.
8. Every repeated description and clause owns an independent suffix slot.
9. A Flow event slot is distinct from source state, target state and guard.
10. No external consumer can redefine, infer or automatically mutate a suffix.
11. Attachment is a relation over one ordered item sequence, never duplicated
    ownership of Rule content.
