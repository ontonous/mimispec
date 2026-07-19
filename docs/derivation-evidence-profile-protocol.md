# Deferred MimiSpec Derivation, Evidence, and Profile Research

> Status: **deferred, non-normative research draft**. It is not part of the
> MimiSpec 0.3 Core language, parser contract, roadmap, or freeze gate.
>
> Current released version: `0.2.1`.
>
> The first-wave `materialize` and `profile` APIs on `main` are prototypes. They
> do not yet implement the complete contract in this document.

The formal 0.3 Core sources are `docs/0.3.x-design-zh.md`,
`docs/specification.md`, `docs/commitment-state-machine.md`, and
`docs/roadmap-0.3.x.md`. This file may be reconsidered only after the 0.3 Core
semantics and canonical parser are stable; it cannot redefine `desc`, `rule`,
Flow, Context, clauses, or commitment.

## 1. Purpose

This protocol defines how a MimiSpec project may derive analyses, candidates,
or confirmed target artifacts from intent without collapsing four different
facts into one status:

1. what the source currently says;
2. what a human has confirmed;
3. what a target or workflow derived;
4. what evidence currently supports or challenges a claim.

It is deliberately target-neutral. Mimi, Rust, TypeScript, an external service,
and a human procedure all use the same identity, routing, provenance, and
evidence rules. A target may add deeper domain-specific contracts, but it may
not redefine MimiSpec commitment.

This document does not add MimiSpec surface syntax. Durable IDs and protocol
records may initially live in a sidecar or project index so that `desc`, `rule`,
and `??` remain the five-minute entry point.

## 2. Normative Boundaries

The words **must**, **must not**, **should**, and **may** state requirements for
the planned protocol. They do not claim that the published `0.2.1` release or
the current first-wave implementation already satisfies them.

The protocol preserves these boundaries:

- The lossless source and semantic AST remain the intent authority.
- The commitment state machine remains the collaboration authority.
- A derivation run cannot edit source or grant commitment.
- A Profile reports interpretation and capability; it does not own source
  meaning.
- Evidence records observations about explicit claims. Evidence does not
  confirm or unlock intent.
- Production artifacts do not become a competing intent authority.

## 3. Four Orthogonal Planes

| Plane | Authoritative objects | May change | Must not change implicitly |
|-------|-----------------------|------------|----------------------------|
| Intent | source revision, semantic nodes, slots, rule attachments | authored source through a validated edit | commitment, artifact status, evidence outcome |
| Collaboration | slot state, transition, authorization, challenge | suffix state through Human/AI rules | source content outside the authorized edit |
| Derivation | request, route plan, run, artifact, provenance graph | candidate or projection outputs | source or commitment |
| Evidence | claim, observation, validity event, policy judgement | claim support and release judgement | intent or derivation history |

Cross-plane effects are proposals or derived judgements, never hidden writes.
For example, a failed runtime observation may propose a lock challenge, but it
cannot perform `$ -> $?` by itself.

## 4. Identity Model

### 4.1 Identity classes

The protocol uses opaque, namespaced IDs. IDs are not headers, paths, byte
offsets, or hashes of mutable content.

```text
ProjectId       identity namespace and policy boundary
DocumentId      one logical source document across path changes
RevisionId      one immutable snapshot of a resolved document set
SourceNodeId    parser-local node locator inside one RevisionId
IntentId        project-stable identity of one authored intent entity
SlotId          project-stable identity of one semantic slot of an IntentId
RoutePlanId     immutable routing decision set
DerivationRunId one derivation execution
ArtifactId      one immutable candidate or projection artifact
ClaimId         one versioned predicate about one explicit subject
EvidenceId      one immutable observation record
PolicyId        one immutable derivation, evidence, or release policy
```

`SourceNodeId` remains valuable for exact navigation inside a parsed revision.
It must always be paired with `RevisionId` when it crosses an API boundary.

### 4.2 Stable and occurrence references

Stable identity and current source location are separate:

```text
SlotRef {
    project_id
    intent_id
    slot_id
}

OccurrenceRef {
    revision_id
    document_id
    source_node_id
    slot_kind
    span
    source_hash
}

SlotBinding {
    slot_ref
    occurrence_ref
    binding_basis
}
```

`slot_kind` includes at least `Keyword`, `Identifier`, `Value`, `Attachment`,
and `Structure`. A keyword suffix and a name suffix in the same header have
different `SlotId` values and different commitment snapshots.

An attached `rule` is itself an intent. Its attachment relation is a separate
stable slot that points to the target `IntentId`; changing the relation is not
equivalent to editing the rule text. The attachment may derive its confirmation
basis from a locked atomic rule declaration or the target's locked structural
boundary, but it remains separately identified and hashed so reattachment
cannot hide inside a content match.

Each binding also carries a parser-proven descriptor:

```text
SemanticSlotDescriptor {
    slot_ref
    anchor_kind
    explicit_commitment
    semantic_footprint
    parent_boundary
    inheritance_kind
}
```

`semantic_footprint` identifies the fields governed by that suffix anchor. The
keyword of an atomic `rule`, for example, may govern its unsuffixed text, while
an ordinary-locked container governs its header and child-list topology but
does not recursively confirm unsuffixed descendant content. A strong-locked
container does cover existing unsuffixed descendants. A more specific explicit
suffix always overrides inheritance for its own footprint.

The normative footprint and precedence rules are defined in
[`commitment-state-machine.md`](commitment-state-machine.md). Profile and
derivation code consumes the descriptor; it must not recreate these rules from
token text.

### 4.3 Revision identity

A `RevisionId` identifies the complete input snapshot used by a protocol
operation, including resolved imports. Its manifest must record:

- every `DocumentId` and exact content digest;
- resolver configuration and import targets;
- MimiSpec semantic/lossless model version;
- digest algorithm and canonicalization version.

Changing an imported document therefore creates a new project revision even
when the root file bytes are unchanged.

### 4.4 Lineage events

Cross-revision identity is maintained by explicit lineage records:

```text
LineageEvent {
    event_id
    from: [SlotRef or IntentId]
    to: [SlotRef or IntentId]
    kind
    basis
    asserted_by
    confidence
    from_revision
    to_revision
}
```

Required `kind` values are:

- `Preserve` for an exact one-to-one continuation;
- `Rename` and `Move` for one-to-one structural edits;
- `Split` and `Merge` for cardinality-changing edits;
- `Copy` for intentional duplication;
- `Delete` and `Restore` for tombstone lifecycle;
- `ImportRedirect` for a logical document moving behind a different import.

The identity rules are fail-closed:

- A validated rename or move keeps the existing `IntentId` and affected
  `SlotId` values.
- A copy receives new IDs and a `copied-from` edge. It does not inherit lock
  authorization or Evidence automatically.
- A split or merge retires the old entity IDs, creates new entity IDs, and
  keeps explicit lineage edges. One ID must not silently mean several things.
- A deleted ID remains as a tombstone so old provenance stays resolvable.
- Import path similarity is not proof of `DocumentId` continuity.

An authorized edit may preserve an `IntentId` even though its content hash
changes; identity continuity and Evidence validity are different questions. A
semantic replacement creates a new ID and a `supersedes` edge. If two branches
change the same stable ID incompatibly, merge must report an identity/content
conflict instead of picking the closest text. Evidence remains bound to the
branch revision until the merge lineage is resolved.

Heuristic matching may create a lineage proposal with a score and explanation.
It must not rebind protected content, confirmed projections, or Evidence until
the proposal is accepted by an authorized actor or an exact structured-edit
record proves continuity. Ambiguity produces an orphaned reference and a
diagnostic, never a `kind + header` fallback binding.

## 5. Slot Snapshots and Influence Closure

### 5.1 Slot snapshot

Every derivation selection captures an immutable snapshot:

```text
SlotSnapshot {
    slot_ref
    occurrence_ref
    explicit_commitment
    semantic_footprint
    effective_protection
    effective_confirmation_basis
    content_hash
    structure_hash
    attachment_hash
    semantic_model_version
}
```

`effective_protection` and `effective_confirmation_basis` are derived from the
commitment state machine, semantic footprint, and parent boundaries. The basis
is one of `Direct`, `AtomicBoundary`, `StrongAncestor`, or `None` and names the
exact governing slot. It is not serialized as a suffix the author did not type.

An explicit `?`, `??`, `$?`, `$??`, `$$?`, or `$$??` has precedence over an
inherited confirmation basis for its footprint. Protection and confirmation
remain separate: an ordinary container may protect child-list topology while
leaving an unsuffixed child's content open.

### 5.2 Influence closure

A target artifact rarely depends on one header alone. For a selected output,
the engine must compute a conservative `InfluenceClosure` containing every
source slot that can affect the claimed behavior, including:

- selected keyword, identifier, value, and structure slots;
- attached rule preludes and paragraph attachment relations;
- referenced types, functions, flows, and imported intent;
- applicable environment rules;
- target-derived decisions that affect externally visible behavior.

Unknown dependency analysis expands the closure or blocks confirmed
projection. It must not be treated as absence of a dependency.

An output can be classified as a confirmed projection only when every authored
slot in its influence closure is directly commit-ready or has an unambiguous
effective confirmation basis. If any influencing slot is open, under content
review, delegated, or under lock review, that output is a candidate even when
another slot in the same node has `$` or `$$`.

This rule prohibits selecting a node by the strongest suffix found in its
header.

### 5.3 Release scope

Confirmed projection uses an immutable, authorized scope rather than a free
form label:

```text
ReleaseScope {
    scope_id
    project_id
    base_revision_id
    entries [{ unit_ref, requiredness, rationale }]
    dependency_policy_id
    authorized_by
}
```

`requiredness` is `Required`, `Optional`, or `Excluded`. Exclusion is an
auditable human/project-policy decision, not a Profile decision. A Profile may
report that a unit is unsupported, but it may not remove that unit from scope.

Influence and dependency closure may add required units to the effective scope.
If an excluded or non-confirmed unit affects a required artifact, the engine
must expand the scope, keep the artifact as a candidate, or block projection.
It must not hide the dependency to preserve readiness.

This permits local releases without requiring the whole document to be locked,
while preventing a caller from obtaining a green result by selecting only easy
slots.

## 6. Derivation Modes

### 6.1 Common request envelope

Every operation begins with an immutable request:

```text
DerivationRequest {
    request_id
    mode
    project_id
    base_revision_id
    scope_id
    selected_slot_snapshots
    route_plan_id
    derivation_policy_id
    requested_by
    requested_at
}
```

The request digest is included in every result. A Profile receives only this
snapshot and declared project inputs; undeclared ambient state must be reported
as an external dependency.

### 6.2 `Explore`

`Explore` may consume policy-selected intent in any collaboration state. It may
produce analyses, questions, alternatives, prototypes, counterexamples, and
test suggestions.

Every output influenced by non-confirmed intent has class `Candidate`. A
candidate:

- must name its source revision and influence closure;
- must not claim to be human-confirmed behavior;
- must not satisfy a confirmed-projection or release gate;
- must not add or weaken `$`/`$$`;
- may propose a source patch, but applying that patch requires the normal actor
  and lock validation path.

A candidate is never promoted in place. To become a confirmed projection, the
relevant intent must first obtain a direct or inherited final-lock confirmation
basis, and a new `ConfirmedProjection` request must run against the current
revision. This prevents candidate laundering.

### 6.3 `ConfirmedProjection`

`ConfirmedProjection` maps a selected, confirmed influence closure into target
artifacts. It requires:

1. exact current slot bindings with no ambiguous lineage;
2. a direct `$`/`$$` or valid atomic/strong-lock confirmation basis for every
   authored slot in the closure;
3. stable rule attachment and dependency closure;
4. an explicit, closed route plan;
5. current Profile capability declarations;
6. revalidation of hashes immediately before result acceptance.

`$?`, `$??`, `$$?`, and `$$??` remain protected, but they are not confirmed
projection inputs because the lock decision is unresolved. Their explicit
state also overrides an ancestor's inherited confirmation for that footprint.

A run may finish as `Complete`, `Partial`, `Blocked`, `Failed`, or `Cancelled`.
`Partial` output remains visible and traceable, but it does not claim complete
projection of the requested scope.

### 6.4 Artifact classification

Artifact class is computed per artifact, not once per run:

```text
CandidateArtifact
ConfirmedProjectionArtifact
AuxiliaryArtifact
```

`ConfirmedProjectionArtifact` describes the authority of its intent inputs. It
does not claim that the artifact is correct, complete, tested, or release-ready.

An `AuxiliaryArtifact` includes logs, generated tests, build metadata, and
diagnostic reports. Its existence says nothing about the truth of a domain
claim.

If a Profile mixes a non-confirmed input into an otherwise confirmed output,
the affected artifact is downgraded to `CandidateArtifact`, and the route plan
receives a blocking mixed-input diagnostic.

### 6.5 Result envelope

Every run returns an immutable envelope:

```text
DerivationResult {
    run_id
    request_digest
    status
    executor_contract_digests
    artifacts
    source_proposals
    provenance_edges
    introduced_claims
    evidence_records
    residual_units
    diagnostics
    completed_at
}
```

Artifacts and referenced details are content-addressed. A `source_proposal` is
not an applied edit. Non-deterministic executors must record the model/tool
identity, configuration digest, seed when available, and undeclared variability
as a reproducibility limitation.

## 7. Provenance Graph

### 7.1 Separate dimensions

Provenance is not a single status enum. At minimum, records keep these
dimensions independently:

```text
Principal {
    kind: Human | AI | Tool | External
    identity
    version
}

DerivationKind {
    DirectProjection
    TargetDerived
    ImplementationChoice
    GeneratedTest
    Exploratory
}
```

- `authored_by` describes who authored source intent.
- `produced_by` describes who or what produced an artifact or observation.
- `derivation_kind` describes the role of a derived object.
- `confirmation_basis` references the exact source slot snapshots and
  authorized commitment transitions; it is not copied into a provenance enum.

### 7.2 Required edges

The graph supports at least:

```text
derived-from
projects
implements
tests
observes
supports
refutes
supersedes
copied-from
```

Every artifact must have a path to its complete influence closure. Every
target-derived or implementation-choice object must have a path to the source
intent that motivated it and to the Profile that introduced it.

Derivation edges form an acyclic graph within a run. `supersedes` may connect
runs but must not form a cycle. Missing or cyclic provenance blocks confirmed
projection acceptance.

### 7.3 Target-derived obligations

A Profile may discover obligations not authored in MMS, such as a recovery
state, wire compatibility choice, or generated test. These remain
`TargetDerived` or `ImplementationChoice`.

If an obligation changes externally visible behavior or selects between
meaningfully different human outcomes, it becomes a clarification proposal. It
must not be silently treated as authored intent. Mechanical target choices may
remain derived when the Profile declares their scope and alternatives.

## 8. Claims

### 8.1 Claim definition

Evidence attaches to a versioned claim, not directly to a node:

```text
Claim {
    claim_id
    predicate { namespace, name, semantic_version }
    subject_ref
    parameters
    scope
    introduced_by
    required_by
}
```

The `subject_ref` may name a source revision, slot, route plan, derivation run,
artifact, deployment, environment, or external process.

The predicate must be specific enough to prevent evidence inflation. For
example:

- `mimispec.parser.accepts(source_revision)`;
- `target.build.succeeds(artifact, toolchain, platform)`;
- `test.scenario.passes(artifact, scenario, environment)`;
- `formal.property.holds(artifact, property, assumptions)`;
- `runtime.behavior.observed(deployment, scenario, window)`.

`mimispec.parser.accepts` does not imply that a domain rule holds. Artifact
generation does not imply that the artifact implements its source intent.

A natural-language `rule` is authored intent, not automatically a formal
claim. A Profile may propose one or more testable claim interpretations with
their assumptions. Human confirmation is required when choosing an
interpretation would narrow or change the rule's meaning.

### 8.2 Claim stability

A `ClaimId` identifies one predicate, subject, parameter set, and scope. Editing
any of these creates a new claim revision or a new `ClaimId`; old Evidence is
not silently reused.

## 9. Evidence Records and Validity

### 9.1 Immutable observation

```text
EvidenceRecord {
    evidence_id
    claim_id
    subject_ref
    producer
    method { namespace, name, version }
    outcome
    scope
    assumptions
    input_digests
    artifact_digests
    dependency_digests
    environment
    observed_at
    details_ref
}
```

Required outcomes include:

```text
Passed
Failed
Counterexample
Unknown
Timeout
ToolError
NotApplicable
```

`Unknown`, `Timeout`, `ToolError`, and missing Evidence never become `Passed`.
`NotApplicable` is accepted only when the governing policy allows it and an
authorized rationale names the exact scope.

Evidence is append-only. Correcting a bad record creates a revocation event and
a replacement record; it does not rewrite history.

### 9.2 Validity is separate from outcome

A previously observed `Passed` outcome may later be unusable. Current validity
is derived from the observation and later events:

```text
Current
Stale
Revoked
Orphaned
Corrupt
```

- `Stale`: a declared input, dependency, environment, semantic version, tool
  version, or policy-sensitive condition changed.
- `Revoked`: the producer or an authorized auditor invalidated the record.
- `Orphaned`: the claim or subject can no longer be bound unambiguously.
- `Corrupt`: integrity verification failed.

Stale or orphaned Evidence remains in history and remains visible to review. It
does not satisfy a current release policy.

### 9.3 Invalidation graph

Evidence dependency digests form an invalidation graph. A change propagates to
all observations that declared the changed object as an input. Tools must not
infer that unrelated Evidence is stale merely because it shares a nearby node.

At minimum, invalidation checks cover:

- source and attachment hashes;
- artifact and generated-input hashes;
- Profile identity and capability-contract version;
- parser or semantic model version when it affects the claim;
- test, verifier, compiler, and runtime versions declared by the method;
- external environment and policy versions required by the claim.

## 10. Evidence Policy and Claim Judgement

A release policy names required claims and how evidence is admitted:

```text
EvidencePolicy {
    policy_id
    required_claims
    accepted_methods
    trusted_producers
    minimum_tool_versions
    environment_constraints
    freshness_rules
    quorum_rules
    contradiction_rules
}
```

Policy evaluation returns one of:

```text
Satisfied
Violated
Inconclusive
Missing
NotApplicable
```

- `Satisfied` requires current, admissible support under the exact policy.
- `Violated` retains an admissible failure or counterexample.
- `Inconclusive` covers conflicting evidence, unknown, timeout, or tool error.
- `Missing` means the required observation does not exist or is not admissible.
- `NotApplicable` is explicit and policy-authorized, never a default.

Unless a policy explicitly defines a stronger safe rule, admissible
counterevidence prevents `Satisfied`. Only `Satisfied`, or an explicitly
permitted `NotApplicable`, passes a required release claim.

Policy judgement is reproducible output bound to `PolicyId`; it is not stored
as an intrinsic property of the claim or Evidence.

## 11. Composite Profile Routing

### 11.1 Profile contract

Every executor publishes a versioned contract:

```text
ProfileContract {
    profile_id
    protocol_version
    implementation_version
    capability_schema_version
    capability_digest
    configuration_digest
    declared_external_dependencies
}
```

The contract describes supported intent/claim categories, possible derived
obligations, artifact classes, evidence methods, security boundary, and known
limitations. A boolean Fragment capability matrix may be a discovery summary,
but it is not sufficient for per-unit routing.

Changing any contract field that can affect interpretation or output produces
a new digest and invalidates route approval until re-analysis.

### 11.2 Route plan, unit, and assignment

Routing operates on intent or claim units, not only Fragment kinds:

```text
RoutePlan {
    route_plan_id
    project_id
    revision_id
    scope_id
    composer_policy_id
    profile_contract_digests
    assignments
    residual_units
    introduced_obligations
    approved_by
}

RouteAssignment {
    unit_ref
    executor { profile_id | human_procedure_id | external_system_id }
    responsibility
    support
    source_coverage
    derived_obligations
    expected_artifacts
    expected_claims
}
```

`responsibility` is one of:

- `ExclusiveAuthority` for exactly one committer of a fact or state;
- `SharedContribution` for deliberately combined outputs;
- `Advisory` for analysis without ownership;
- `ObservationOnly` for an evidence producer.

`support` is one of:

- `Supported`: the executor accepts the full declared unit;
- `Partial`: the executor accepts named parts and emits explicit residuals;
- `Unsupported`: the executor cannot accept the unit;
- `NotApplicable`: the unit is outside this executor by explicit rationale;
- `NeedsClarification`: meaning or target choice is insufficient to route.

Support describes capability and planned responsibility, not implementation or
verification success.

`NotApplicable` is local to one executor and provides no coverage for a
required route unit. Another executor, an explicit external owner, or an
authorized scope exclusion is still required.

### 11.3 Residual intent

Every `Partial`, `Unsupported`, or `NeedsClarification` result creates an
explicit residual unit. A residual is classified as:

```text
BlockingUnassigned
AssignedExternal
DeferredOutOfScope
```

`AssignedExternal` names a human procedure or external owner and its expected
claims. `DeferredOutOfScope` requires explicit scope authorization; it cannot
be used for a required unit already included in the release scope.

No Profile, including Mimi, receives unassigned residual intent by default.

### 11.4 Closure and conflicts

Route planning iterates to a fixed point because Profiles may introduce
target-derived obligations. The plan is closed only when every required source
unit and every blocking derived obligation is assigned or explicitly rejected
for the current scope.

The composer reports at least:

- required coverage gaps;
- `Partial` support without enumerated residuals;
- two `ExclusiveAuthority` owners for the same fact;
- incompatible shared representations or version requirements;
- cycles between target-derived obligations;
- derived decisions that require human clarification;
- units silently omitted by an executor;
- capability-contract drift after route approval.

Overlap is legal only when responsibility and reconciliation are explicit. Two
Profiles may test the same rule, but two Profiles may not both become the
exclusive business commit authority.

## 12. Three Readiness Judgements

Readiness is always parameterized. There is no timeless `document.ready`
boolean.

### 12.1 Confirmation readiness

```text
DirectCommitReady(slot_snapshot)
    = explicit commitment is exactly $ or $$

ConfirmationReady(slot_snapshot, revision)
    = DirectCommitReady
      or an unambiguous AtomicBoundary / StrongAncestor basis
      with no more-specific explicit unresolved suffix
```

This answers only whether the human confirmation state for that slot is closed.
The effective case records the exact governing slot and footprint; it does not
pretend that the author typed a suffix on the descendant. Strong lock is not
more implemented or more verified than ordinary lock.

### 12.2 Projection readiness

```text
ProjectionReady(scope, revision, route_plan, derivation_policy)
    = required influence closure is known
    + every authored slot in that closure is ConfirmationReady
    + rule attachment and import resolution are exact
    + lineage is unambiguous
    + route-plan fixed point is closed
    + no blocking residual or clarification remains
    + Profile capability contracts match the approved plan
```

This authorizes starting or accepting a `ConfirmedProjection` run. It does not
claim that any artifact has been generated successfully.

### 12.3 Release readiness

```text
ReleaseReady(scope, revision, run, route_plan, evidence_policy)
    = ProjectionReady for the run's exact request
    + run completed for every required route
    + required artifacts have complete current provenance
    + every required claim passes the evidence policy
    + no blocking collaboration, target, drift, or integrity diagnostic
```

Readiness is non-monotonic across external change. Evidence can become stale,
an import can drift, or a Profile capability contract can change, making
`ReleaseReady` false while source commitment remains `$` or `$$`. The correct
response is a diagnostic or challenge proposal, not automatic unlock.

## 13. Transaction and Concurrency Rules

Derivation is an optimistic transaction over an immutable revision:

1. capture `RevisionId`, slot snapshots, policies, and route-plan digest;
2. validate mode and readiness before dispatch;
3. run Profiles against the captured request;
4. ingest artifacts, provenance, claims, and Evidence as one auditable bundle;
5. revalidate revision, bindings, hashes, Profile identities, and policy IDs;
6. accept the result, or retain it as stale/candidate output with diagnostics.

If source or imports change during the run, the output is not silently rebound
to the new revision. A new run, or an explicit lineage-aware revalidation,
must occur.

Profiles must return structured source proposals instead of writing `.mms`
directly. Applying a proposal is a separate actor-authorized patch operation.

## 14. Fail-Closed Diagnostics

The protocol must expose machine-readable diagnostics for at least:

```text
D-IDENTITY-AMBIGUOUS
D-INFLUENCE-OPEN
D-MIXED-INPUT
D-PROVENANCE-INCOMPLETE
D-EVIDENCE-STALE
D-EVIDENCE-ORPHANED
D-CLAIM-INCONCLUSIVE
D-ROUTE-RESIDUAL
D-ROUTE-AUTHORITY-CONFLICT
D-PROFILE-DRIFT
D-REVISION-DRIFT
```

Each diagnostic carries stable object references, severity, blocking status,
reason, and possible actions. A missing mapping, unknown result, or unsupported
unit must never be converted into success merely to keep a pipeline moving.

## 15. Versioned Serialization

Protocol JSON uses an explicit schema version independent of the MimiSpec
language version. It must:

- encode IDs as opaque strings;
- pair local IDs and spans with `RevisionId`;
- name digest algorithm and canonicalization version;
- use immutable records plus explicit supersession/revocation events;
- preserve unknown non-critical fields where possible;
- reject an unsupported schema major version;
- distinguish absent, unknown, not applicable, and failed;
- keep deterministic ordering for signed or hashed manifests.

The collaboration report schema may embed summaries of these records, but a
summary must not become the authoritative project ledger.

## 16. Current Prototype Migration Map

The first-wave APIs demonstrate useful behavior but remain provisional:

| Current prototype | Required protocol replacement |
|-------------------|-------------------------------|
| `MaterializationSlot.node: SourceNodeId` | `SlotRef` plus revision-bound `OccurrenceRef` |
| strongest top-level header suffix | parser-proven slot snapshots and influence closure |
| `commit_ready: bool` | direct/effective `ConfirmationReady` judgement with its exact basis |
| single `Provenance` enum | independent principals, derivation kind, confirmation basis, and graph edges |
| node-scoped `EvidenceRecord` | `ClaimId` plus subject-, method-, dependency-, and validity-aware observation |
| implicit parser-passed evidence | explicit `mimispec.parser.accepts` claim only |
| `kind + header` drift fallback | lineage proposal; ambiguity becomes orphaned |
| Fragment capability booleans | versioned capability contract plus per-unit support |
| one Profile analysis | composite `RoutePlan` with residuals and authority checks |
| free-form release-scope string | immutable authorized `ReleaseScope` plus dependency closure |
| one materialization plan | mode-specific request, run, route plan, artifacts, claims, and evidence bundle |

Compatibility work should add the new model beside the prototype, migrate CLI
JSON behind a new schema version, and remove ambiguous fallbacks before calling
the API stable.

## 17. Required Conformance Scenarios

An implementation is not conforming until it tests at least these cases:

| Scenario | Required result |
|----------|-----------------|
| Open intent produces a prototype | candidate artifact; no confirmation or release credit |
| `rule$` has an unsuffixed atomic text value | value has an atomic-boundary confirmation basis |
| Ordinary-locked container has an unsuffixed body child | child content remains open; topology remains protected |
| Strong-locked container has an unsuffixed body child | child has a strong-ancestor confirmation basis |
| Explicit `??` child appears under `$$` | explicit delegation overrides inheritance and blocks affected projection |
| `$` name with `??` body influences one artifact | artifact is candidate or projection is blocked |
| Caller excludes an attached required rule | closure restores the rule or projection blocks |
| Candidate later becomes locked | new confirmed-projection run; no in-place promotion |
| Node is renamed through a validated structured edit | stable ID preserved; occurrence changes |
| Node is copied | new ID; old lock and Evidence do not transfer automatically |
| Heuristic matches two renamed nodes | ambiguous/orphaned; no automatic Evidence rebind |
| Rule paragraph attachment changes | attachment hash drifts and projection blocks |
| Parser accepts a document | only parser-acceptance claim may pass |
| Verifier returns `Unknown` or times out | claim remains inconclusive |
| Old passing test has changed dependencies | Evidence is stale and release readiness regresses |
| Profile reports `Partial` without residuals | Profile conformance fails |
| Two Profiles claim exclusive business authority | route plan blocks |
| Profile introduces an unrouted behavioral choice | clarification/residual blocks fixed-point closure |
| Root source stays unchanged but an import changes | new revision; old projection cannot release |
| Source changes while a Profile is running | result retained as stale/candidate; no silent rebind |

These scenarios are protocol tests, not proof that a particular target can
satisfy the authored domain intent.

## 18. Deferred Surface Decisions

The following remain deliberately open until sidecar/project-index experience
exists across multiple Profiles:

- whether authors need an optional textual `IntentId` syntax;
- how project identity is shared across repository splits and forks;
- which claim predicate namespaces become standard library contracts;
- which evidence policies, if any, ship as defaults;
- how much lineage can be inferred safely from plain-text edits.

None of these open decisions permits an implementation to use a path, header,
or revision-local node number as durable identity.
