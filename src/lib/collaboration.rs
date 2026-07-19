use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::ast::{Commitment, LockIntent};
use crate::lossless::{
    ByteSpan, CommitmentAnchorKind, CommitmentFootprintKind, CommitmentSlotId,
    CommitmentSlotSyntax, LosslessDocument, SourceNode, SourceNodeId, SourceNodeKind,
};

/// 请求实际代表的授权主体。Tooling 必须代理其中一个主体，不能自行获得权限。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Human,
    Ai,
}

/// Stable hex digest of protected content/structure bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ContentHash(pub u64);

impl ContentHash {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self(hasher.finish())
    }

    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

/// Protected hashes for one source node revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProtectedHashes {
    pub node: SourceNodeId,
    pub content: ContentHash,
    pub structure: ContentHash,
    pub full: ContentHash,
}

/// AI lock challenge record produced when `$ -> $?` or `$$ -> $$?`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockChallenge {
    pub slot_id: CommitmentSlotId,
    pub owner_node: SourceNodeId,
    pub original_state: Commitment,
    pub challenged_state: Commitment,
    pub content_hash: ContentHash,
    pub structure_hash: ContentHash,
    pub reason: String,
    pub evidence: Vec<String>,
    pub affected_targets: Vec<SourceNodeId>,
    pub suggested_actions: Vec<String>,
}

impl LockChallenge {
    /// Fingerprint used for challenge deduplication.
    pub fn fingerprint(&self) -> ContentHash {
        let mut material = String::new();
        material.push_str(&self.slot_id.0.to_string());
        material.push('|');
        material.push_str(&format!("{:?}", self.original_state));
        material.push('|');
        material.push_str(&format!("{:?}", self.challenged_state));
        material.push('|');
        material.push_str(&self.content_hash.hex());
        material.push('|');
        material.push_str(&self.structure_hash.hex());
        material.push('|');
        material.push_str(self.reason.trim());
        material.push('|');
        let mut evidence = self.evidence.clone();
        evidence.sort();
        for item in evidence {
            material.push_str(item.trim());
            material.push(';');
        }
        ContentHash::from_bytes(material.as_bytes())
    }
}

/// Compute protected hashes for a node from the lossless document revision.
pub fn protected_hashes(
    document: &LosslessDocument,
    node: SourceNodeId,
) -> Option<ProtectedHashes> {
    let source_node = document.node(node)?;
    let content = document.text(source_node.spans.core)?;
    let structure = document.text(source_node.spans.header)?;
    let full = document.text(source_node.spans.full)?;
    Some(ProtectedHashes {
        node,
        content: ContentHash::from_bytes(content.as_bytes()),
        structure: ContentHash::from_bytes(structure.as_bytes()),
        full: ContentHash::from_bytes(full.as_bytes()),
    })
}

/// Hash an arbitrary protected span (for suffix-only slots without full nodes).
pub fn hash_span(document: &LosslessDocument, span: ByteSpan) -> Option<ContentHash> {
    document
        .text(span)
        .map(|text| ContentHash::from_bytes(text.as_bytes()))
}

/// Compare two protected-hash snapshots and fill transition effects.
pub fn effects_from_hashes(before: &ProtectedHashes, after: &ProtectedHashes) -> TransitionEffects {
    TransitionEffects {
        content_changed: before.content != after.content,
        structure_changed: before.structure != after.structure,
        attachment_changed: before.full != after.full
            && before.content == after.content
            && before.structure == after.structure,
    }
}

/// Effects for patch validation: commitment suffix-only edits do not count as
/// protected content/structure changes.
pub fn effects_for_slot_patch(before: &SlotSnapshot, after: &SlotSnapshot) -> TransitionEffects {
    let before_core = strip_all_commitment_suffixes(&before.core);
    let after_core = strip_all_commitment_suffixes(&after.core);
    let before_header = strip_all_commitment_suffixes(&before.header);
    let after_header = strip_all_commitment_suffixes(&after.header);
    let before_full = strip_all_commitment_suffixes(&before.full);
    let after_full = strip_all_commitment_suffixes(&after.full);
    TransitionEffects {
        content_changed: before_core != after_core,
        structure_changed: before_header != after_header,
        attachment_changed: before_full != after_full
            && before_core == after_core
            && before_header == after_header,
    }
}

fn strip_all_commitment_suffixes(text: &str) -> String {
    text.split_inclusive(|ch: char| {
        ch.is_whitespace() || matches!(ch, ':' | '(' | ')' | '[' | ']' | ',' | '|' | '\n' | '\r')
    })
    .map(|part| {
        let (token, sep) = match part.chars().last() {
            Some(ch)
                if ch.is_whitespace()
                    || matches!(ch, ':' | '(' | ')' | '[' | ']' | ',' | '|' | '\n' | '\r') =>
            {
                (&part[..part.len() - ch.len_utf8()], ch.to_string())
            }
            _ => (part, String::new()),
        };
        format!("{}{}", strip_commitment_suffix(token), sep)
    })
    .collect()
}

/// Build a lock challenge after a validated AI `$/$` challenge transition.
pub fn build_lock_challenge(
    document: &LosslessDocument,
    slot_id: CommitmentSlotId,
    original_state: Commitment,
    challenged_state: Commitment,
    reason: &str,
    evidence: Vec<String>,
    suggested_actions: Vec<String>,
) -> Result<LockChallenge, TransitionViolation> {
    let is_challenge = matches!(
        (original_state, challenged_state),
        (Commitment::Locked, Commitment::LockedQuestion)
            | (Commitment::StrongLocked, Commitment::StrongLockedQuestion)
    );
    if !is_challenge {
        return Err(TransitionViolation::AiTransitionForbidden);
    }
    if reason.trim().is_empty() {
        return Err(TransitionViolation::ChallengeReasonRequired);
    }
    let snapshot = collect_semantic_slot_snapshots(document)
        .into_iter()
        .find(|slot| slot.slot == slot_id)
        .ok_or(TransitionViolation::ProtectedContentChanged)?;
    if snapshot.state != original_state {
        return Err(TransitionViolation::AiTransitionForbidden);
    }
    let structure_hash = ContentHash::from_bytes(
        format!(
            "{}|{}|{}",
            snapshot.topology, snapshot.attachment, snapshot.position
        )
        .as_bytes(),
    );
    Ok(LockChallenge {
        slot_id,
        owner_node: snapshot.node,
        original_state,
        challenged_state,
        content_hash: snapshot.protected_hash,
        structure_hash,
        reason: reason.trim().to_string(),
        evidence,
        affected_targets: vec![snapshot.node],
        suggested_actions,
    })
}

/// Reject repeated identical challenges until evidence changes.
pub fn challenge_is_duplicate(existing: &[LockChallenge], challenge: &LockChallenge) -> bool {
    let fingerprint = challenge.fingerprint();
    existing
        .iter()
        .any(|prior| prior.fingerprint() == fingerprint)
}

/// One commitment slot observed on a lossless document revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlotSnapshot {
    pub node: SourceNodeId,
    pub kind: crate::lossless::SourceNodeKind,
    pub state: Commitment,
    pub header: String,
    pub core: String,
    pub full: String,
    pub hashes: ProtectedHashes,
}

/// One parser-proven semantic commitment slot.
///
/// Unlike [`SlotSnapshot`], this never folds several suffix anchors into one
/// node-level state and never discovers state by scanning header text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticSlotSnapshot {
    pub slot: CommitmentSlotId,
    pub node: SourceNodeId,
    pub kind: SourceNodeKind,
    pub anchor_kind: CommitmentAnchorKind,
    pub footprint: CommitmentFootprintKind,
    pub owner_slot_index: u32,
    pub state: Commitment,
    pub anchor: String,
    pub header: String,
    pub protected_text: String,
    pub topology: String,
    pub attachment: String,
    pub position: String,
    pub anchor_span: ByteSpan,
    pub suffix_span: ByteSpan,
    pub protected_hash: ContentHash,
}

/// Collect each semantic suffix slot independently from parser-proven lossless
/// metadata. Zero-width slots are retained so IDEs can address insertion
/// positions without inventing a node-wide state.
pub fn collect_semantic_slot_snapshots(document: &LosslessDocument) -> Vec<SemanticSlotSnapshot> {
    let attachments = AttachmentIndex::new(document);
    let mut explicit_open_footprints = document
        .commitment_slots()
        .iter()
        .filter(|slot| {
            slot.semantic_slot
                && matches!(
                    slot.value,
                    Commitment::Question | Commitment::QuestionQuestion
                )
        })
        .filter_map(|slot| footprint_span(document, slot))
        .collect::<Vec<_>>();
    explicit_open_footprints.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    let mut owner_counts = HashMap::<SourceNodeId, u32>::new();
    let mut snapshots = Vec::new();
    for slot in document
        .commitment_slots()
        .iter()
        .filter(|slot| slot.semantic_slot)
    {
        let Some(node_id) = slot.owner else {
            continue;
        };
        let Some(node) = document.node(node_id) else {
            continue;
        };
        let owner_slot_index = owner_counts.entry(node_id).or_default();
        let index = *owner_slot_index;
        *owner_slot_index += 1;
        let anchor = document
            .text(slot.anchor_span)
            .unwrap_or_default()
            .to_string();
        let header = document
            .text(node.spans.header)
            .unwrap_or_default()
            .to_string();
        let protected_text =
            protected_text_for_slot(document, slot, node, &explicit_open_footprints);
        let topology = document
            .structural_topology(node_id)
            .unwrap_or_default()
            .to_string();
        let attachment = attachments.signature(node_id).to_string();
        let position = document
            .structural_position(node_id)
            .unwrap_or_default()
            .to_string();
        snapshots.push(SemanticSlotSnapshot {
            slot: slot.id,
            node: node_id,
            kind: node.kind,
            anchor_kind: slot.anchor_kind,
            footprint: slot.footprint,
            owner_slot_index: index,
            state: slot.value,
            anchor,
            header,
            protected_hash: ContentHash::from_bytes(protected_text.as_bytes()),
            protected_text,
            topology,
            attachment,
            position,
            anchor_span: slot.anchor_span,
            suffix_span: slot.suffix_span,
        });
    }
    snapshots
}

fn protected_text_for_slot(
    document: &LosslessDocument,
    slot: &CommitmentSlotSyntax,
    node: &SourceNode,
    explicit_open_footprints: &[ByteSpan],
) -> String {
    if slot.state_is_strong_lock() {
        return masked_strong_subtree(document, node, explicit_open_footprints);
    }

    let span = match slot.footprint {
        CommitmentFootprintKind::Clause => node.spans.core,
        CommitmentFootprintKind::Event => event_footprint_span(document, slot, node),
        CommitmentFootprintKind::Transition => transition_footprint_span(document, node),
        CommitmentFootprintKind::EntityKind
            if node.kind.is_scope_container() || document.child_nodes(node.id).next().is_some() =>
        {
            node.spans.header
        }
        CommitmentFootprintKind::EntityKind => node.spans.core,
        CommitmentFootprintKind::Placeholder => node.spans.core,
        CommitmentFootprintKind::NameOrReference
        | CommitmentFootprintKind::Value
        | CommitmentFootprintKind::ExpressionOperator
        | CommitmentFootprintKind::Unknown => slot.anchor_span,
    };
    strip_all_commitment_suffixes(document.text(span).unwrap_or_default())
}

trait CommitmentSlotExt {
    fn state_is_strong_lock(&self) -> bool;
}

impl CommitmentSlotExt for CommitmentSlotSyntax {
    fn state_is_strong_lock(&self) -> bool {
        self.value.lock_intent() == LockIntent::StrongLocked
    }
}

fn event_footprint_span(
    document: &LosslessDocument,
    slot: &CommitmentSlotSyntax,
    node: &SourceNode,
) -> ByteSpan {
    let header = document.text(node.spans.header).unwrap_or_default();
    let arrow = header
        .find(">>>")
        .map(|offset| node.spans.header.start as usize + offset)
        .unwrap_or(slot.anchor_span.end as usize);
    ByteSpan::new(
        slot.anchor_span.start as usize,
        arrow.max(slot.anchor_span.end as usize),
    )
}

fn transition_footprint_span(document: &LosslessDocument, node: &SourceNode) -> ByteSpan {
    let header = document.text(node.spans.header).unwrap_or_default();
    let start = header
        .find(">>>")
        .map(|offset| node.spans.header.start as usize + offset)
        .unwrap_or(node.spans.header.start as usize);
    let tail = &document.source()[start..node.spans.header.end as usize];
    let end = tail
        .find(':')
        .map(|offset| start + offset)
        .unwrap_or(node.spans.header.end as usize);
    ByteSpan::new(start, end)
}

fn masked_strong_subtree(
    document: &LosslessDocument,
    node: &SourceNode,
    explicit_open_footprints: &[ByteSpan],
) -> String {
    let base = node.spans.core;
    let first = explicit_open_footprints.partition_point(|span| span.start < base.start);
    let mut masks = explicit_open_footprints[first..]
        .iter()
        .copied()
        .take_while(|span| span.start <= base.end)
        .filter(|span| span.start >= base.start && span.end <= base.end)
        .collect::<Vec<_>>();
    masks.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    let mut merged: Vec<ByteSpan> = Vec::new();
    for span in masks {
        if let Some(last) = merged.last_mut() {
            if span.start <= last.end {
                last.end = last.end.max(span.end);
                continue;
            }
        }
        merged.push(span);
    }

    let mut normalized = String::new();
    let mut cursor = base.start as usize;
    for (index, span) in merged.iter().enumerate() {
        normalized.push_str(&strip_all_commitment_suffixes(
            &document.source()[cursor..span.start as usize],
        ));
        normalized.push_str(&format!("<explicit-open:{index}>"));
        cursor = span.end as usize;
    }
    normalized.push_str(&strip_all_commitment_suffixes(
        &document.source()[cursor..base.end as usize],
    ));
    normalized
}

fn footprint_span(document: &LosslessDocument, slot: &CommitmentSlotSyntax) -> Option<ByteSpan> {
    let node = document.node(slot.owner?)?;
    Some(match slot.footprint {
        CommitmentFootprintKind::EntityKind
        | CommitmentFootprintKind::Clause
        | CommitmentFootprintKind::Placeholder => node.spans.core,
        CommitmentFootprintKind::Event => event_footprint_span(document, slot, node),
        CommitmentFootprintKind::Transition => transition_footprint_span(document, node),
        CommitmentFootprintKind::NameOrReference
        | CommitmentFootprintKind::Value
        | CommitmentFootprintKind::ExpressionOperator
        | CommitmentFootprintKind::Unknown => slot.anchor_span,
    })
}

/// Attachment signatures derived once per semantic snapshot. Parent/children,
/// topology and position come directly from `LosslessDocument`'s shared index.
struct AttachmentIndex {
    attachment: HashMap<SourceNodeId, String>,
}

impl AttachmentIndex {
    fn new(document: &LosslessDocument) -> Self {
        let attachment = document
            .nodes()
            .iter()
            .filter_map(|node| {
                let parts = document
                    .rules_for_target(node.id)
                    .map(|rule| {
                        strip_all_commitment_suffixes(document.text(rule.span).unwrap_or_default())
                    })
                    .collect::<Vec<_>>();
                (!parts.is_empty()).then_some((node.id, parts.join("|")))
            })
            .collect();

        Self { attachment }
    }

    fn signature(&self, node: SourceNodeId) -> &str {
        self.attachment.get(&node).map(String::as_str).unwrap_or("")
    }
}

/// Compatibility node summary derived from parser-proven semantic slots.
///
/// New collaboration code must use [`collect_semantic_slot_snapshots`]. This
/// view intentionally folds a node's independent slots only for older
/// node-oriented consumers; it never scans source text to discover state.
pub fn collect_slot_snapshots(document: &LosslessDocument) -> Vec<SlotSnapshot> {
    let semantic_slots = collect_semantic_slot_snapshots(document);
    document
        .nodes()
        .iter()
        .filter_map(|node| {
            let header = document.text(node.spans.header)?.to_string();
            let core = document.text(node.spans.core)?.to_string();
            let full = document.text(node.spans.full)?.to_string();
            let state = semantic_slots
                .iter()
                .filter(|slot| slot.node == node.id)
                .map(|slot| slot.state)
                .max_by_key(|state| commitment_rank(*state))
                .unwrap_or(Commitment::None);
            let hashes = protected_hashes(document, node.id)?;
            Some(SlotSnapshot {
                node: node.id,
                kind: node.kind,
                state,
                header,
                core,
                full,
                hashes,
            })
        })
        .collect()
}

fn commitment_rank(state: Commitment) -> u8 {
    match state {
        Commitment::None => 0,
        Commitment::Question => 1,
        Commitment::QuestionQuestion => 2,
        Commitment::Locked => 3,
        Commitment::LockedQuestion => 4,
        Commitment::LockedQuestionQuestion => 5,
        Commitment::StrongLocked => 6,
        Commitment::StrongLockedQuestion => 7,
        Commitment::StrongLockedQuestionQuestion => 8,
    }
}

/// Structured comparison between two document revisions for one logical slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchSlotDiff {
    pub before: SemanticSlotSnapshot,
    pub after: Option<SemanticSlotSnapshot>,
    pub effects: TransitionEffects,
    pub from: Commitment,
    pub to: Commitment,
}

/// Authority and authorization attached to a whole-document edit request.
///
/// `actor == None` is used only for observed editor changes in advisory mode;
/// it can never authorize a protected transition.
#[derive(Debug, Clone, Copy)]
pub struct DocumentPatchRequest<'a> {
    pub actor: Option<Actor>,
    pub authorization: HumanAuthorization,
    pub challenge_reason: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentPatchViolation {
    ActorDeclarationRequired,
    Transition(TransitionViolation),
}

/// Validate a document revision for Human, AI, or an undeclared observer.
///
/// The comparison is slot-precise and uses the same parser-proven identity as
/// the AI compatibility validator. Callers must validate any session-scoped
/// unlock token before setting `authorization.unlock_strong_lock`.
pub fn validate_document_patch(
    before: &LosslessDocument,
    after: &LosslessDocument,
    request: &DocumentPatchRequest<'_>,
) -> Vec<Result<PatchSlotDiff, DocumentPatchViolation>> {
    let before_slots = collect_semantic_slot_snapshots(before);
    let after_slots = collect_semantic_slot_snapshots(after);
    let mut results = Vec::new();
    let mut matched_after = std::collections::HashSet::new();

    for prior in &before_slots {
        let identity = semantic_slot_identity(prior);
        let matched = after_slots
            .iter()
            .enumerate()
            .find(|(_, candidate)| semantic_slot_identity(candidate) == identity);
        let Some((after_index, next)) = matched else {
            if prior.state.protects_content() {
                let violation = match request.actor {
                    None => DocumentPatchViolation::ActorDeclarationRequired,
                    Some(Actor::Human) if !request.authorization.modify_protected => {
                        DocumentPatchViolation::Transition(
                            TransitionViolation::HumanAuthorizationRequired,
                        )
                    }
                    Some(Actor::Ai) => DocumentPatchViolation::Transition(
                        TransitionViolation::ProtectedContentChanged,
                    ),
                    Some(Actor::Human) => continue,
                };
                results.push(Err(violation));
            }
            continue;
        };
        matched_after.insert(after_index);

        let effects = effects_for_semantic_slot_patch(prior, next);
        if prior.state == next.state && effects.is_unchanged() {
            continue;
        }
        // An explicitly open slot has no commitment transition to validate.
        // Container protection, when present, is enforced by the protected
        // parent slot's footprint in the same document comparison.
        if prior.state == Commitment::None
            && next.state == Commitment::None
            && request.actor.is_some()
        {
            continue;
        }
        let Some(actor) = request.actor else {
            results.push(Err(DocumentPatchViolation::ActorDeclarationRequired));
            continue;
        };
        let transition = TransitionRequest {
            actor,
            from: prior.state,
            to: next.state,
            effects,
            authorization: request.authorization,
            challenge_reason: request.challenge_reason,
        };
        match validate_transition(&transition) {
            Ok(()) => results.push(Ok(PatchSlotDiff {
                before: prior.clone(),
                after: Some(next.clone()),
                effects,
                from: prior.state,
                to: next.state,
            })),
            Err(violation) => results.push(Err(DocumentPatchViolation::Transition(violation))),
        }
    }

    for (index, slot) in after_slots.iter().enumerate() {
        if matched_after.contains(&index) || slot.state == Commitment::None {
            continue;
        }
        match request.actor {
            None => results.push(Err(DocumentPatchViolation::ActorDeclarationRequired)),
            Some(Actor::Ai) => {
                let transition = TransitionRequest {
                    actor: Actor::Ai,
                    from: Commitment::None,
                    to: slot.state,
                    effects: TransitionEffects::default(),
                    authorization: HumanAuthorization::default(),
                    challenge_reason: request.challenge_reason,
                };
                if let Err(violation) = validate_transition(&transition) {
                    results.push(Err(DocumentPatchViolation::Transition(violation)));
                }
            }
            Some(Actor::Human) => {}
        }
    }

    if results.is_empty() && before.source() != after.source() && request.actor.is_none() {
        results.push(Err(DocumentPatchViolation::ActorDeclarationRequired));
    }
    results
}

/// Validate an AI patch by comparing before/after lossless documents.
///
/// Matching uses parser-proven owner position, footprint and slot ordinal.
/// Independent suffixes on one header are validated independently; no
/// "strongest header suffix" folding is permitted.
pub fn validate_ai_document_patch(
    before: &LosslessDocument,
    after: &LosslessDocument,
    challenge_reason: Option<&str>,
) -> Vec<Result<PatchSlotDiff, TransitionViolation>> {
    let before_slots = collect_semantic_slot_snapshots(before);
    let after_slots = collect_semantic_slot_snapshots(after);
    let mut results = Vec::new();
    let mut matched_after = std::collections::HashSet::new();

    for prior in &before_slots {
        if prior.state == Commitment::None {
            continue;
        }
        let identity = semantic_slot_identity(prior);
        let matched = after_slots
            .iter()
            .enumerate()
            .find(|(_, candidate)| semantic_slot_identity(candidate) == identity);
        let Some((after_index, next)) = matched else {
            if prior.state.protects_content() {
                results.push(Err(TransitionViolation::ProtectedContentChanged));
            } else {
                // AI cannot silently delete non-protected slots either:
                // the transition must be validated through the matrix.
                let implicit = TransitionRequest {
                    actor: Actor::Ai,
                    from: prior.state,
                    to: Commitment::None,
                    effects: TransitionEffects {
                        content_changed: true,
                        ..TransitionEffects::default()
                    },
                    authorization: HumanAuthorization::default(),
                    challenge_reason: None,
                };
                if let Err(violation) = validate_transition(&implicit) {
                    results.push(Err(violation));
                }
            }
            continue;
        };
        matched_after.insert(after_index);

        let effects = effects_for_semantic_slot_patch(prior, next);
        if prior.state == next.state && effects.is_unchanged() {
            continue;
        }
        let request = TransitionRequest {
            actor: Actor::Ai,
            from: prior.state,
            to: next.state,
            effects,
            authorization: HumanAuthorization::default(),
            challenge_reason,
        };
        match validate_transition(&request) {
            Ok(()) => results.push(Ok(PatchSlotDiff {
                before: prior.clone(),
                after: Some(next.clone()),
                effects,
                from: prior.state,
                to: next.state,
            })),
            Err(violation) => results.push(Err(violation)),
        }
    }

    // Fresh slots still start at `None` and must pass the same matrix. In
    // particular, AI may open a review with `?`, but may not self-delegate
    // with `??` or create any lock-family state.
    for (index, slot) in after_slots.iter().enumerate() {
        if matched_after.contains(&index) || slot.state == Commitment::None {
            continue;
        }
        if let Err(violation) = validate_transition(&TransitionRequest {
            actor: Actor::Ai,
            from: Commitment::None,
            to: slot.state,
            effects: TransitionEffects::default(),
            authorization: HumanAuthorization::default(),
            challenge_reason,
        }) {
            results.push(Err(violation));
        }
    }
    results
}

fn semantic_slot_identity(slot: &SemanticSlotSnapshot) -> String {
    format!(
        "{}:{:?}:{:?}:{:?}:{}",
        slot.position, slot.kind, slot.anchor_kind, slot.footprint, slot.owner_slot_index
    )
}

fn effects_for_semantic_slot_patch(
    before: &SemanticSlotSnapshot,
    after: &SemanticSlotSnapshot,
) -> TransitionEffects {
    let protects_structure = before.state.is_strong_locked()
        || matches!(
            before.footprint,
            CommitmentFootprintKind::EntityKind | CommitmentFootprintKind::Clause
        );
    TransitionEffects {
        content_changed: before.protected_text != after.protected_text,
        structure_changed: before.position != after.position
            || (protects_structure && before.topology != after.topology),
        attachment_changed: protects_structure && before.attachment != after.attachment,
    }
}

fn strip_commitment_suffix(token: &str) -> &str {
    for suffix in ["$$??", "$??", "$$?", "$?", "$$", "$", "??", "?"] {
        if let Some(stripped) = token.strip_suffix(suffix) {
            if !stripped.is_empty() {
                return stripped;
            }
        }
    }
    token
}

/// Human 对受保护内容和强锁解除的显式授权。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct HumanAuthorization {
    pub modify_protected: bool,
    pub unlock_strong_lock: bool,
}

/// Explicit human-issued token required to weaken or remove a strong lock.
///
/// Tooling must obtain this from a Human actor session; AI cannot mint it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnlockToken {
    pub slot: CommitmentSlotId,
    pub from: Commitment,
    pub nonce: u64,
}

impl UnlockToken {
    pub fn issue(
        slot: CommitmentSlotId,
        from: Commitment,
        nonce: u64,
    ) -> Result<Self, TransitionViolation> {
        if from.lock_intent() != LockIntent::StrongLocked {
            return Err(TransitionViolation::StrongUnlockAuthorizationRequired);
        }
        Ok(Self { slot, from, nonce })
    }

    pub fn authorizes(&self, slot: CommitmentSlotId, from: Commitment) -> bool {
        self.slot == slot && self.from == from && from.lock_intent() == LockIntent::StrongLocked
    }
}

/// Parent/child lock propagation decision for nested slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PropagationViolation {
    StrongParentBlocksChildEdit,
    OrdinaryParentBlocksStructure,
    ExplicitOpenChildOnly,
}

/// Decide whether an AI edit to a child slot is allowed under a parent lock.
///
/// - Ordinary `$` parent: protects parent identity/structure; explicit `?`/`??`
///   children may still evolve their own content.
/// - Strong `$$` parent: protects the whole structural subtree. Only children
///   that already carry explicit open review/delegation (`?`/`??`) may evolve,
///   and only without structural boundary changes on the parent.
pub fn validate_child_under_parent(
    parent: Commitment,
    child_from: Commitment,
    child_to: Commitment,
    child_effects: TransitionEffects,
    actor: Actor,
) -> Result<(), PropagationViolation> {
    if actor == Actor::Human {
        return Ok(());
    }

    match parent.lock_intent() {
        LockIntent::Open => Ok(()),
        LockIntent::Locked => {
            // Ordinary lock: child content may evolve only when the child itself
            // is not protected, or via allowed AI transitions on that child.
            if child_from.protects_content()
                && child_effects.changes_protected_boundary()
                && validate_transition(&TransitionRequest {
                    actor: Actor::Ai,
                    from: child_from,
                    to: child_to,
                    effects: child_effects,
                    authorization: HumanAuthorization::default(),
                    challenge_reason: Some("nested"),
                })
                .is_err()
            {
                return Err(PropagationViolation::OrdinaryParentBlocksStructure);
            }
            Ok(())
        }
        LockIntent::StrongLocked => {
            // Strong parent: only explicit open children (`?`/`??` without `$`)
            // remain editable by AI.
            let child_is_explicit_open = matches!(
                child_from,
                Commitment::Question | Commitment::QuestionQuestion
            );
            if !child_is_explicit_open {
                if child_from != child_to || child_effects.changes_protected_boundary() {
                    return Err(PropagationViolation::StrongParentBlocksChildEdit);
                }
                return Ok(());
            }
            if child_effects.structure_changed || child_effects.attachment_changed {
                return Err(PropagationViolation::ExplicitOpenChildOnly);
            }
            // Open child content may change; state transitions still go through
            // the normal AI matrix.
            if child_from != child_to
                && validate_transition(&TransitionRequest {
                    actor: Actor::Ai,
                    from: child_from,
                    to: child_to,
                    effects: child_effects,
                    authorization: HumanAuthorization::default(),
                    challenge_reason: None,
                })
                .is_err()
            {
                return Err(PropagationViolation::ExplicitOpenChildOnly);
            }
            Ok(())
        }
    }
}

/// Validate a human unlock of a strong-locked slot using an unlock token.
pub fn validate_strong_unlock(
    token: &UnlockToken,
    slot: CommitmentSlotId,
    from: Commitment,
    to: Commitment,
) -> TransitionDecision {
    if from.lock_intent() != LockIntent::StrongLocked {
        return Err(TransitionViolation::StrongUnlockAuthorizationRequired);
    }
    if to.lock_intent() == LockIntent::StrongLocked {
        // Still strong-locked family; no unlock token required for in-family
        // review transitions performed by Human.
        return Ok(());
    }
    if !token.authorizes(slot, from) {
        return Err(TransitionViolation::StrongUnlockAuthorizationRequired);
    }
    Ok(())
}

/// 由调用方描述的转移影响。0.3.2 的 patch validator 将负责从真实 patch 证明这些值。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TransitionEffects {
    pub content_changed: bool,
    pub structure_changed: bool,
    pub attachment_changed: bool,
}

impl TransitionEffects {
    pub fn is_unchanged(self) -> bool {
        !self.content_changed && !self.structure_changed && !self.attachment_changed
    }

    pub fn changes_protected_boundary(self) -> bool {
        self.content_changed || self.structure_changed || self.attachment_changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionRequest<'a> {
    pub actor: Actor,
    pub from: Commitment,
    pub to: Commitment,
    pub effects: TransitionEffects,
    pub authorization: HumanAuthorization,
    pub challenge_reason: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionViolation {
    AiTransitionForbidden,
    ProtectedContentChanged,
    ProtectedStructureChanged,
    ProtectedAttachmentChanged,
    ChallengeReasonRequired,
    HumanAuthorizationRequired,
    StrongUnlockAuthorizationRequired,
}

pub type TransitionDecision = Result<(), TransitionViolation>;

/// 验证单个 commitment slot 的 Actor 转移。
///
/// 该 API 验证 0.3.0 的语义请求。它不验证源码 patch 是否诚实声明了 effects；
/// span、hash 和结构化 patch 的证明属于 0.3.1-0.3.2。
pub fn validate_transition(request: &TransitionRequest<'_>) -> TransitionDecision {
    match request.actor {
        Actor::Human => validate_human_transition(request),
        Actor::Ai => validate_ai_transition(request),
    }
}

fn validate_human_transition(request: &TransitionRequest<'_>) -> TransitionDecision {
    if request.from.lock_intent() == LockIntent::StrongLocked
        && request.to.lock_intent() != LockIntent::StrongLocked
        && !request.authorization.unlock_strong_lock
    {
        return Err(TransitionViolation::StrongUnlockAuthorizationRequired);
    }

    if request.from.protects_content()
        && request.effects.changes_protected_boundary()
        && !request.authorization.modify_protected
    {
        return Err(TransitionViolation::HumanAuthorizationRequired);
    }

    Ok(())
}

fn validate_ai_transition(request: &TransitionRequest<'_>) -> TransitionDecision {
    if !ai_transition_allowed(request.from, request.to) {
        return Err(TransitionViolation::AiTransitionForbidden);
    }

    if request.from.protects_content() {
        if request.effects.content_changed {
            return Err(TransitionViolation::ProtectedContentChanged);
        }
        if request.effects.structure_changed {
            return Err(TransitionViolation::ProtectedStructureChanged);
        }
        if request.effects.attachment_changed {
            return Err(TransitionViolation::ProtectedAttachmentChanged);
        }
    }

    if matches!(
        (request.from, request.to),
        (Commitment::Locked, Commitment::LockedQuestion)
            | (Commitment::StrongLocked, Commitment::StrongLockedQuestion)
    ) && request
        .challenge_reason
        .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err(TransitionViolation::ChallengeReasonRequired);
    }

    Ok(())
}

fn ai_transition_allowed(from: Commitment, to: Commitment) -> bool {
    matches!(
        (from, to),
        (Commitment::None, Commitment::Question)
            | (Commitment::Question, Commitment::Question)
            | (Commitment::QuestionQuestion, Commitment::Question)
            | (Commitment::QuestionQuestion, Commitment::None)
            | (Commitment::Locked, Commitment::LockedQuestion)
            | (Commitment::StrongLocked, Commitment::StrongLockedQuestion)
            | (
                Commitment::LockedQuestionQuestion,
                Commitment::LockedQuestion
            )
            | (
                Commitment::StrongLockedQuestionQuestion,
                Commitment::StrongLockedQuestion
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ReviewIntent, ReviewTarget};

    const STATES: [Commitment; 9] = [
        Commitment::None,
        Commitment::Question,
        Commitment::QuestionQuestion,
        Commitment::Locked,
        Commitment::LockedQuestion,
        Commitment::LockedQuestionQuestion,
        Commitment::StrongLocked,
        Commitment::StrongLockedQuestion,
        Commitment::StrongLockedQuestionQuestion,
    ];

    fn request(actor: Actor, from: Commitment, to: Commitment) -> TransitionRequest<'static> {
        TransitionRequest {
            actor,
            from,
            to,
            effects: TransitionEffects::default(),
            authorization: HumanAuthorization::default(),
            challenge_reason: Some("new evidence conflicts with the lock"),
        }
    }

    #[test]
    fn commitment_semantics_cover_all_nine_states() {
        let expected = [
            (
                LockIntent::Open,
                ReviewIntent::None,
                ReviewTarget::Content,
                false,
            ),
            (
                LockIntent::Open,
                ReviewIntent::Review,
                ReviewTarget::Content,
                false,
            ),
            (
                LockIntent::Open,
                ReviewIntent::Delegate,
                ReviewTarget::Content,
                false,
            ),
            (
                LockIntent::Locked,
                ReviewIntent::None,
                ReviewTarget::Lock,
                true,
            ),
            (
                LockIntent::Locked,
                ReviewIntent::Review,
                ReviewTarget::Lock,
                true,
            ),
            (
                LockIntent::Locked,
                ReviewIntent::Delegate,
                ReviewTarget::Lock,
                true,
            ),
            (
                LockIntent::StrongLocked,
                ReviewIntent::None,
                ReviewTarget::StrongLock,
                true,
            ),
            (
                LockIntent::StrongLocked,
                ReviewIntent::Review,
                ReviewTarget::StrongLock,
                true,
            ),
            (
                LockIntent::StrongLocked,
                ReviewIntent::Delegate,
                ReviewTarget::StrongLock,
                true,
            ),
        ];

        for (state, (lock, review, target, protected)) in STATES.into_iter().zip(expected) {
            assert_eq!(state.lock_intent(), lock);
            assert_eq!(state.review_intent(), review);
            assert_eq!(state.review_target(), target);
            assert_eq!(state.protects_content(), protected);
            assert_eq!(state.is_delegated(), review == ReviewIntent::Delegate);
            assert_eq!(
                state.requires_human_decision(),
                review == ReviewIntent::Review
            );
        }

        assert!(Commitment::Locked.is_commit_ready());
        assert!(Commitment::StrongLocked.is_commit_ready());
        for state in STATES {
            if !matches!(state, Commitment::Locked | Commitment::StrongLocked) {
                assert!(!state.is_commit_ready(), "{state:?}");
            }
        }
    }

    #[test]
    fn ai_transition_matrix_is_exhaustive() {
        for from in STATES {
            for to in STATES {
                let decision = validate_transition(&request(Actor::Ai, from, to));
                assert_eq!(
                    decision.is_ok(),
                    ai_transition_allowed(from, to),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn human_transition_matrix_allows_state_changes_with_required_unlock() {
        for from in STATES {
            for to in STATES {
                let mut request = request(Actor::Human, from, to);
                request.authorization.unlock_strong_lock = true;
                assert!(validate_transition(&request).is_ok(), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn ai_lock_challenge_requires_reason_and_preserves_boundary() {
        let mut request = request(Actor::Ai, Commitment::Locked, Commitment::LockedQuestion);
        request.challenge_reason = None;
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ChallengeReasonRequired)
        );

        request.challenge_reason = Some("evidence");
        request.effects.content_changed = true;
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ProtectedContentChanged)
        );
        request.effects = TransitionEffects {
            structure_changed: true,
            ..TransitionEffects::default()
        };
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ProtectedStructureChanged)
        );
        request.effects = TransitionEffects {
            attachment_changed: true,
            ..TransitionEffects::default()
        };
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ProtectedAttachmentChanged)
        );
    }

    #[test]
    fn human_protected_edits_and_strong_unlock_require_authorization() {
        let mut protected_edit = request(Actor::Human, Commitment::Locked, Commitment::Locked);
        protected_edit.effects.content_changed = true;
        assert_eq!(
            validate_transition(&protected_edit),
            Err(TransitionViolation::HumanAuthorizationRequired)
        );
        protected_edit.authorization.modify_protected = true;
        assert!(validate_transition(&protected_edit).is_ok());

        let mut unlock = request(Actor::Human, Commitment::StrongLocked, Commitment::Locked);
        assert_eq!(
            validate_transition(&unlock),
            Err(TransitionViolation::StrongUnlockAuthorizationRequired)
        );
        unlock.authorization.unlock_strong_lock = true;
        assert!(validate_transition(&unlock).is_ok());
    }

    #[test]
    fn protected_hashes_detect_content_and_structure_changes() {
        let source = r#"func Pay$:
    steps:
        charge payment
"#;
        let doc = crate::parse_lossless(source).document;
        let func = doc
            .nodes()
            .iter()
            .find(|node| node.kind == crate::lossless::SourceNodeKind::Func)
            .unwrap()
            .id;
        let before = protected_hashes(&doc, func).unwrap();

        let renamed = source.replace("Pay$", "Refund$");
        let after_doc = crate::parse_lossless(&renamed).document;
        let after_func = after_doc
            .nodes()
            .iter()
            .find(|node| node.kind == crate::lossless::SourceNodeKind::Func)
            .unwrap()
            .id;
        let after = protected_hashes(&after_doc, after_func).unwrap();
        let effects = effects_from_hashes(&before, &after);
        assert!(effects.content_changed || effects.structure_changed);

        let same = protected_hashes(&doc, func).unwrap();
        let unchanged = effects_from_hashes(&before, &same);
        assert!(unchanged.is_unchanged());
    }

    #[test]
    fn lock_challenge_requires_reason_and_deduplicates() {
        let source = r#"func Pay$:
    steps:
        charge payment
"#;
        let doc = crate::parse_lossless(source).document;
        let func = doc
            .nodes()
            .iter()
            .find(|node| node.kind == crate::lossless::SourceNodeKind::Func)
            .unwrap()
            .id;
        let slot = collect_semantic_slot_snapshots(&doc)
            .into_iter()
            .find(|slot| slot.node == func && slot.state == Commitment::Locked)
            .unwrap()
            .slot;

        assert_eq!(
            build_lock_challenge(
                &doc,
                slot,
                Commitment::Locked,
                Commitment::LockedQuestion,
                "   ",
                vec![],
                vec![],
            ),
            Err(TransitionViolation::ChallengeReasonRequired)
        );

        let challenge = build_lock_challenge(
            &doc,
            slot,
            Commitment::Locked,
            Commitment::LockedQuestion,
            "missing refund path",
            vec!["audit log gap".into()],
            vec!["add refund step".into()],
        )
        .unwrap();
        assert_eq!(challenge.original_state, Commitment::Locked);
        assert_eq!(challenge.challenged_state, Commitment::LockedQuestion);
        assert!(!challenge.content_hash.hex().is_empty());

        let repeated = challenge.clone();
        assert!(challenge_is_duplicate(&[challenge], &repeated));

        let mut with_new_evidence = repeated;
        with_new_evidence.evidence.push("new counterexample".into());
        assert!(!challenge_is_duplicate(
            &[build_lock_challenge(
                &doc,
                slot,
                Commitment::Locked,
                Commitment::LockedQuestion,
                "missing refund path",
                vec!["audit log gap".into()],
                vec!["add refund step".into()],
            )
            .unwrap()],
            &with_new_evidence
        ));
    }

    #[test]
    fn ai_patch_accepts_suffix_only_lock_challenge() {
        let before = crate::parse_lossless(
            r#"func Pay$:
    steps:
        charge payment
"#,
        )
        .document;
        let after = crate::parse_lossless(
            r#"func Pay$?:
    steps:
        charge payment
"#,
        )
        .document;
        let results = validate_ai_document_patch(&before, &after, Some("refund path missing"));
        assert!(results.iter().all(|result| result.is_ok()), "{results:?}");
        assert!(results.iter().any(|result| {
            matches!(
                result,
                Ok(diff)
                    if diff.from == Commitment::Locked
                        && diff.to == Commitment::LockedQuestion
                        && diff.effects.is_unchanged()
            )
        }));
    }

    #[test]
    fn ai_patch_rejects_lock_bypass_content_edit() {
        let before = crate::parse_lossless(
            r#"func$$ Pay:
    steps:
        charge payment
"#,
        )
        .document;
        let after = crate::parse_lossless(
            r#"func$$ Pay:
    steps:
        charge twice
"#,
        )
        .document;
        let results = validate_ai_document_patch(&before, &after, None);
        assert!(results.iter().any(|result| {
            matches!(
                result,
                Err(TransitionViolation::ProtectedContentChanged)
                    | Err(TransitionViolation::ProtectedStructureChanged)
                    | Err(TransitionViolation::AiTransitionForbidden)
            )
        }));
    }

    #[test]
    fn ai_patch_rejects_unlock_without_authorization() {
        let before = crate::parse_lossless(
            r#"func Pay$$:
    steps:
        charge payment
"#,
        )
        .document;
        let after = crate::parse_lossless(
            r#"func Pay:
    steps:
        charge payment
"#,
        )
        .document;
        let results = validate_ai_document_patch(&before, &after, None);
        assert!(results
            .iter()
            .any(|result| { matches!(result, Err(TransitionViolation::AiTransitionForbidden)) }));
    }

    #[test]
    fn semantic_slots_do_not_fold_keyword_name_and_value_states() {
        let document =
            crate::parse_lossless("func?? Pay$:\n    desc \"payment intent\"?\n").document;
        let slots = collect_semantic_slot_snapshots(&document);
        let func = document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::Func)
            .unwrap();
        let func_slots = slots
            .iter()
            .filter(|slot| slot.node == func.id)
            .collect::<Vec<_>>();
        assert!(func_slots.iter().any(|slot| {
            slot.anchor == "func"
                && slot.footprint == CommitmentFootprintKind::EntityKind
                && slot.state == Commitment::QuestionQuestion
        }));
        assert!(func_slots.iter().any(|slot| {
            slot.anchor == "Pay"
                && slot.footprint == CommitmentFootprintKind::NameOrReference
                && slot.state == Commitment::Locked
        }));
        assert!(slots.iter().any(|slot| {
            slot.anchor == "\"payment intent\""
                && slot.footprint == CommitmentFootprintKind::Value
                && slot.state == Commitment::Question
        }));
    }

    #[test]
    fn structure_index_matches_exhaustive_parent_selection() {
        let result = crate::parse_lossless(
            r#"module App:
    rule "audit every attempt"
    func Pay(order):
        requires: order.ready
        steps:
            if order.ready:
                charge payment
            else:
                reject payment
    flow:
        Pending:
            on Approved >>> Paid: desc "settled"
"#,
        );
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        for node in result.document.nodes() {
            let exhaustive = result
                .document
                .nodes()
                .iter()
                .filter(|candidate| {
                    candidate.id != node.id
                        && candidate.spans.core.start <= node.spans.core.start
                        && candidate.spans.core.end >= node.spans.core.end
                        && (candidate.spans.core.start < node.spans.core.start
                            || candidate.spans.core.end > node.spans.core.end)
                })
                .min_by_key(|candidate| candidate.spans.core.len())
                .map(|candidate| candidate.id);
            assert_eq!(
                result.document.parent_node(node.id).map(|parent| parent.id),
                exhaustive
            );
        }
    }

    #[test]
    fn ai_patch_respects_ordinary_and_strong_container_footprints() {
        let ordinary_before =
            crate::parse_lossless("module$ App:\n    desc \"draft one\"\n").document;
        let ordinary_after =
            crate::parse_lossless("module$ App:\n    desc \"draft two\"\n").document;
        let ordinary = validate_ai_document_patch(&ordinary_before, &ordinary_after, None);
        assert!(ordinary.iter().all(Result::is_ok), "{ordinary:?}");

        let strong_before =
            crate::parse_lossless("module$$ App:\n    desc \"locked one\"\n").document;
        let strong_after =
            crate::parse_lossless("module$$ App:\n    desc \"locked two\"\n").document;
        let strong = validate_ai_document_patch(&strong_before, &strong_after, None);
        assert!(strong.iter().any(|result| result.is_err()), "{strong:?}");

        let open_before =
            crate::parse_lossless("module$$ App:\n    desc?? \"delegated one\"\n").document;
        let open_after =
            crate::parse_lossless("module$$ App:\n    desc? \"delegated two\"\n").document;
        let explicit_open = validate_ai_document_patch(&open_before, &open_after, None);
        assert!(explicit_open.iter().all(Result::is_ok), "{explicit_open:?}");
    }

    #[test]
    fn locked_container_rejects_same_kind_child_reordering() {
        let ordinary_before =
            crate::parse_lossless("module$ M:\n    func A: ...\n    func B: ...\n").document;
        let ordinary_after =
            crate::parse_lossless("module$ M:\n    func B: ...\n    func A: ...\n").document;
        let ordinary = validate_ai_document_patch(&ordinary_before, &ordinary_after, None);
        assert!(ordinary.iter().any(Result::is_err), "{ordinary:?}");
        let ordinary_session = validate_document_patch(
            &ordinary_before,
            &ordinary_after,
            &DocumentPatchRequest {
                actor: Some(Actor::Ai),
                authorization: HumanAuthorization::default(),
                challenge_reason: None,
            },
        );
        assert!(
            ordinary_session.iter().any(Result::is_err),
            "{ordinary_session:?}"
        );

        let strong_before =
            crate::parse_lossless("module$$ M:\n    func? A: ...\n    func? B: ...\n").document;
        let strong_after =
            crate::parse_lossless("module$$ M:\n    func? B: ...\n    func? A: ...\n").document;
        let strong = validate_ai_document_patch(&strong_before, &strong_after, None);
        assert!(strong.iter().any(Result::is_err), "{strong:?}");
    }

    #[test]
    fn ai_patch_cannot_bypass_independent_name_or_rule_value_locks() {
        let before = crate::parse_lossless("func?? Pay$: ...\nrule \"must audit\"$\n").document;
        let after =
            crate::parse_lossless("func? Refund$: ...\nrule \"audit optional\"$\n").document;
        let results = validate_ai_document_patch(&before, &after, None);
        assert!(
            results.iter().filter(|result| result.is_err()).count() >= 2,
            "{results:?}"
        );
    }

    #[test]
    fn ai_patch_cannot_create_a_fresh_strong_lock() {
        let before = crate::parse_lossless("desc \"draft\"\n").document;
        let after = crate::parse_lossless("desc$$ \"draft\"\n").document;
        let results = validate_ai_document_patch(&before, &after, None);
        assert!(results
            .iter()
            .any(|result| matches!(result, Err(TransitionViolation::AiTransitionForbidden))));
    }

    #[test]
    fn ai_patch_validates_the_state_of_every_fresh_slot() {
        let before = crate::parse_lossless("").document;
        let review = crate::parse_lossless("desc? \"new intent\"\n").document;
        let delegated = crate::parse_lossless("desc?? \"new intent\"\n").document;

        let allowed = validate_ai_document_patch(&before, &review, None);
        assert!(allowed.iter().all(Result::is_ok), "{allowed:?}");

        let forbidden = validate_ai_document_patch(&before, &delegated, None);
        assert!(forbidden
            .iter()
            .any(|result| matches!(result, Err(TransitionViolation::AiTransitionForbidden))));

        let request = DocumentPatchRequest {
            actor: Some(Actor::Ai),
            authorization: HumanAuthorization::default(),
            challenge_reason: None,
        };
        let forbidden = validate_document_patch(&before, &delegated, &request);
        assert!(forbidden.iter().any(|result| matches!(
            result,
            Err(DocumentPatchViolation::Transition(
                TransitionViolation::AiTransitionForbidden
            ))
        )));
    }

    #[test]
    fn unlock_token_is_required_to_weaken_strong_lock() {
        let source = r#"func Pay$$:
    steps:
        charge payment
"#;
        let doc = crate::parse_lossless(source).document;
        let func = doc
            .nodes()
            .iter()
            .find(|node| node.kind == crate::lossless::SourceNodeKind::Func)
            .unwrap()
            .id;
        let slot = collect_semantic_slot_snapshots(&doc)
            .into_iter()
            .find(|slot| slot.node == func && slot.state == Commitment::StrongLocked)
            .unwrap()
            .slot;

        assert!(UnlockToken::issue(slot, Commitment::Locked, 1).is_err());
        let token = UnlockToken::issue(slot, Commitment::StrongLocked, 7).unwrap();
        assert!(
            validate_strong_unlock(&token, slot, Commitment::StrongLocked, Commitment::Locked)
                .is_ok()
        );
        assert_eq!(
            validate_strong_unlock(
                &token,
                slot,
                Commitment::StrongLockedQuestion,
                Commitment::Locked
            ),
            Err(TransitionViolation::StrongUnlockAuthorizationRequired)
        );
        assert_eq!(
            validate_strong_unlock(
                &UnlockToken {
                    slot: CommitmentSlotId(999),
                    from: Commitment::StrongLocked,
                    nonce: 1
                },
                slot,
                Commitment::StrongLocked,
                Commitment::None
            ),
            Err(TransitionViolation::StrongUnlockAuthorizationRequired)
        );
    }

    #[test]
    fn parent_child_lock_propagation_matrix() {
        assert_eq!(
            validate_child_under_parent(
                Commitment::StrongLocked,
                Commitment::None,
                Commitment::Question,
                TransitionEffects {
                    content_changed: true,
                    ..TransitionEffects::default()
                },
                Actor::Ai,
            ),
            Err(PropagationViolation::StrongParentBlocksChildEdit)
        );

        assert!(validate_child_under_parent(
            Commitment::StrongLocked,
            Commitment::QuestionQuestion,
            Commitment::Question,
            TransitionEffects {
                content_changed: true,
                ..TransitionEffects::default()
            },
            Actor::Ai,
        )
        .is_ok());

        assert_eq!(
            validate_child_under_parent(
                Commitment::StrongLocked,
                Commitment::Question,
                Commitment::Question,
                TransitionEffects {
                    structure_changed: true,
                    ..TransitionEffects::default()
                },
                Actor::Ai,
            ),
            Err(PropagationViolation::ExplicitOpenChildOnly)
        );

        assert!(validate_child_under_parent(
            Commitment::StrongLocked,
            Commitment::None,
            Commitment::Question,
            TransitionEffects {
                content_changed: true,
                ..TransitionEffects::default()
            },
            Actor::Human,
        )
        .is_ok());

        assert!(validate_child_under_parent(
            Commitment::Locked,
            Commitment::None,
            Commitment::Question,
            TransitionEffects {
                content_changed: true,
                ..TransitionEffects::default()
            },
            Actor::Ai,
        )
        .is_ok());
    }

    #[test]
    fn generalized_document_patch_requires_actor_for_observed_changes() {
        let before = crate::parse_lossless("desc \"draft\"\n");
        let after = crate::parse_lossless("desc \"changed\"\n");
        let results = validate_document_patch(
            &before.document,
            &after.document,
            &DocumentPatchRequest {
                actor: None,
                authorization: HumanAuthorization::default(),
                challenge_reason: None,
            },
        );
        assert!(results.iter().any(|result| {
            matches!(
                result,
                Err(DocumentPatchViolation::ActorDeclarationRequired)
            )
        }));
    }

    #[test]
    fn generalized_human_patch_requires_explicit_protected_authorization() {
        let before = crate::parse_lossless("func$ Pay:\n    steps:\n        charge payment\n");
        let after = crate::parse_lossless("func$ Refund:\n    steps:\n        charge payment\n");
        let denied = validate_document_patch(
            &before.document,
            &after.document,
            &DocumentPatchRequest {
                actor: Some(Actor::Human),
                authorization: HumanAuthorization::default(),
                challenge_reason: None,
            },
        );
        assert!(denied.iter().any(|result| {
            matches!(
                result,
                Err(DocumentPatchViolation::Transition(
                    TransitionViolation::HumanAuthorizationRequired
                ))
            )
        }));
        let allowed = validate_document_patch(
            &before.document,
            &after.document,
            &DocumentPatchRequest {
                actor: Some(Actor::Human),
                authorization: HumanAuthorization {
                    modify_protected: true,
                    unlock_strong_lock: false,
                },
                challenge_reason: None,
            },
        );
        assert!(allowed.iter().all(Result::is_ok), "{allowed:?}");
    }
}
