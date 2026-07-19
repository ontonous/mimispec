use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ast::{Commitment, LockIntent};
use crate::collaboration::{
    build_lock_challenge, challenge_is_duplicate, collect_semantic_slot_snapshots,
    validate_document_patch, Actor, DocumentPatchRequest, DocumentPatchViolation,
    HumanAuthorization, LockChallenge, TransitionViolation,
};
use crate::diagnostics::{analyze_document, DocumentDiagnostics};
use crate::error::ParseError;
use crate::ide::{
    code_actions_for_node, hover_at, semantic_tokens, CodeAction, HoverInfo, IdeSnapshot,
    SemanticToken,
};
use crate::lossless::{
    ColumnEncoding, CommitmentSlotId, LineIndex, LosslessDocument, SourceNodeId, SourcePosition,
};
#[cfg(feature = "experimental-targets")]
use crate::materialize::{plan_materialization, MaterializationPlan};
#[cfg(feature = "experimental-targets")]
use crate::profile::{analyze_generic_profile, analyze_mimi_profile, ProfileAnalysis};

pub const LANGUAGE_SERVICE_SCHEMA_VERSION: &str = "mimispec.ls/0.3";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationMode {
    #[default]
    Advisory,
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct RevisionHash(pub String);

impl RevisionHash {
    pub fn from_source(source: &str) -> Self {
        let digest = Sha256::digest(source.as_bytes());
        Self(format!("{digest:x}"))
    }
}

#[derive(Debug)]
struct RevisionPayload {
    hash: RevisionHash,
    document: LosslessDocument,
    errors: Vec<ParseError>,
    diagnostics: DocumentDiagnostics,
}

#[derive(Debug, Clone)]
struct DocumentRevision {
    version: u64,
    payload: Arc<RevisionPayload>,
}

impl DocumentRevision {
    fn parse(version: u64, source: String) -> Self {
        let hash = RevisionHash::from_source(&source);
        Self::parse_hashed(version, source, hash)
    }

    fn parse_hashed(version: u64, source: String, hash: RevisionHash) -> Self {
        let parsed = crate::parse_lossless(&source);
        let diagnostics = analyze_document(&parsed.document, &parsed.errors);
        Self {
            version,
            payload: Arc::new(RevisionPayload {
                hash,
                document: parsed.document,
                errors: parsed.errors,
                diagnostics,
            }),
        }
    }

    fn is_complete(&self) -> bool {
        self.payload.errors.is_empty()
    }

    fn with_version(&self, version: u64) -> Self {
        Self {
            version,
            payload: Arc::clone(&self.payload),
        }
    }

    fn hash(&self) -> &RevisionHash {
        &self.payload.hash
    }

    fn source(&self) -> &str {
        self.payload.document.source()
    }

    fn document(&self) -> &LosslessDocument {
        &self.payload.document
    }

    fn errors(&self) -> &[ParseError] {
        &self.payload.errors
    }

    fn diagnostics(&self) -> &DocumentDiagnostics {
        &self.payload.diagnostics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionTextEdit {
    pub range: Option<TextRange>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionViolation {
    pub code: String,
    pub message: String,
}

impl SessionViolation {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionUnlockToken {
    pub token: String,
    pub uri: String,
    pub authoritative_version: u64,
    pub slot: CommitmentSlotId,
    pub from: Commitment,
    pub used: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentEditRequest {
    pub base_version: u64,
    pub actor: Actor,
    pub edits: Vec<SessionTextEdit>,
    pub authorization: HumanAuthorization,
    pub unlock_tokens: Vec<String>,
    pub challenge_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentEditResponse {
    pub schema_version: &'static str,
    pub accepted: bool,
    pub authoritative_version: u64,
    pub candidate_hash: Option<RevisionHash>,
    pub transaction_id: Option<String>,
    pub violations: Vec<SessionViolation>,
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    id: String,
    candidate: DocumentRevision,
    challenges: Vec<LockChallenge>,
    reserved_unlock_tokens: Vec<String>,
}

/// In-memory target-neutral language-service document state.
///
/// The observed revision follows editor text. The authoritative revision is
/// the last actor-declared revision accepted by collaboration policy. In
/// advisory mode undeclared changes are usable but explicitly untrusted; in
/// strict mode they cannot replace the authoritative revision.
#[derive(Debug, Clone)]
pub struct DocumentSession {
    uri: String,
    mode: CollaborationMode,
    observed: DocumentRevision,
    authoritative: DocumentRevision,
    authoritative_trusted: bool,
    pending: Option<PendingTransaction>,
    violations: Vec<SessionViolation>,
    unlock_tokens: HashMap<String, SessionUnlockToken>,
    lock_challenges: Vec<LockChallenge>,
    next_nonce: u64,
    #[cfg(test)]
    observed_parse_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionSnapshot {
    pub schema_version: &'static str,
    pub uri: String,
    /// Compatibility alias for `observed_version`.
    pub version: u64,
    pub observed_version: u64,
    pub authoritative_version: u64,
    pub observed_hash: RevisionHash,
    pub authoritative_hash: RevisionHash,
    pub authoritative_trusted: bool,
    pub divergent: bool,
    pub mode: CollaborationMode,
    pub error_count: usize,
    pub violations: Vec<SessionViolation>,
    pub lock_challenges: Vec<LockChallenge>,
    pub ide: IdeSnapshot,
}

impl DocumentSession {
    pub fn open(uri: impl Into<String>, source: impl Into<String>) -> Self {
        let uri = uri.into();
        let revision = DocumentRevision::parse(1, source.into());
        Self {
            uri,
            mode: CollaborationMode::Advisory,
            observed: revision.clone(),
            authoritative: revision,
            authoritative_trusted: true,
            pending: None,
            violations: Vec::new(),
            unlock_tokens: HashMap::new(),
            lock_challenges: Vec::new(),
            next_nonce: 1,
            #[cfg(test)]
            observed_parse_count: 1,
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn mode(&self) -> CollaborationMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: CollaborationMode) {
        self.mode = mode;
        if mode == CollaborationMode::Strict && !self.authoritative_trusted {
            if self.observed.hash() != self.authoritative.hash() {
                self.push_violation(
                    "C-DOCUMENT-DIVERGED",
                    "strict mode requires a Human to adopt or restore the current observed revision",
                );
            } else {
                self.push_violation(
                    "C-DOCUMENT-DIVERGED",
                    "observed revision is not yet trusted; call adopt_observed or restore to confirm",
                );
            }
        }
    }

    pub fn version(&self) -> u64 {
        self.observed.version
    }

    pub fn authoritative_version(&self) -> u64 {
        self.authoritative.version
    }

    pub fn source(&self) -> &str {
        self.observed.source()
    }

    pub fn authoritative_source(&self) -> &str {
        self.authoritative.source()
    }

    pub fn document(&self) -> &LosslessDocument {
        self.observed.document()
    }

    pub fn authoritative_document(&self) -> &LosslessDocument {
        self.authoritative.document()
    }

    pub fn errors(&self) -> &[ParseError] {
        self.observed.errors()
    }

    pub fn diagnostics(&self) -> &DocumentDiagnostics {
        self.observed.diagnostics()
    }

    pub fn violations(&self) -> &[SessionViolation] {
        &self.violations
    }

    pub fn lock_challenges(&self) -> &[LockChallenge] {
        &self.lock_challenges
    }

    pub fn is_divergent(&self) -> bool {
        self.observed.hash() != self.authoritative.hash() || !self.authoritative_trusted
    }

    /// Compatibility full-text observation. It is deliberately undeclared and
    /// therefore produces advisory collaboration diagnostics.
    pub fn update_full(&mut self, source: impl Into<String>) {
        self.observe_full(source.into());
    }

    pub fn observe_full(&mut self, source: String) {
        let next_version = self.observed.version.saturating_add(1);
        let observed_hash = RevisionHash::from_source(&source);

        if let Some(pending) = self.pending.take() {
            if pending.candidate.hash() == &observed_hash {
                for token_id in &pending.reserved_unlock_tokens {
                    if let Some(token) = self.unlock_tokens.get_mut(token_id) {
                        token.used = true;
                    }
                }
                let accepted = pending.candidate.with_version(next_version);
                self.observed = accepted.clone();
                self.authoritative = accepted;
                self.authoritative_trusted = true;
                for challenge in pending.challenges {
                    if !challenge_is_duplicate(&self.lock_challenges, &challenge) {
                        self.lock_challenges.push(challenge);
                    }
                }
                self.violations.clear();
                return;
            }
            // Any other observed revision invalidates the candidate's ranges.
            // Drop it atomically; reserved unlock tokens remain unused.
        }

        if self.mode == CollaborationMode::Strict && &observed_hash == self.authoritative.hash() {
            self.observed = self.authoritative.with_version(next_version);
            self.authoritative_trusted = true;
            self.violations.clear();
            return;
        }

        #[cfg(test)]
        {
            self.observed_parse_count += 1;
        }
        let candidate = DocumentRevision::parse_hashed(next_version, source, observed_hash);
        self.violations = advisory_patch_violations(self.observed.document(), candidate.document());
        self.observed = candidate.clone();
        if self.mode == CollaborationMode::Advisory {
            self.authoritative = candidate;
            self.authoritative_trusted = false;
        } else {
            self.push_violation(
                "C-DOCUMENT-DIVERGED",
                "strict mode observed an edit that was not produced by an accepted transaction",
            );
        }
    }

    pub fn observe_edits(&mut self, edits: &[SessionTextEdit]) -> Result<(), SessionViolation> {
        let mut source = self.observed.source().to_string();
        let mut line_index = self.observed.document().line_index().clone();
        for edit in edits {
            if let Err(violation) = apply_sequential_text_edit(&mut source, &mut line_index, edit) {
                self.push_violation(&violation.code, &violation.message);
                return Err(violation);
            }
        }
        self.observe_full(source);
        Ok(())
    }

    pub(crate) fn record_invalid_edit(&mut self, message: &str) {
        self.push_violation("C-INVALID-EDIT", message);
    }

    pub fn issue_unlock_token(
        &mut self,
        base_version: u64,
        actor: Actor,
        slot: CommitmentSlotId,
    ) -> Result<SessionUnlockToken, SessionViolation> {
        if actor != Actor::Human {
            return Err(SessionViolation::new(
                "C-ACTOR-REQUIRED",
                "only a Human actor may issue a strong-lock unlock token",
            ));
        }
        if base_version != self.authoritative.version {
            return Err(SessionViolation::new(
                "C-STALE-REVISION",
                format!(
                    "base version {base_version} does not match authoritative version {}",
                    self.authoritative.version
                ),
            ));
        }
        let Some(snapshot) = collect_semantic_slot_snapshots(self.authoritative.document())
            .into_iter()
            .find(|snapshot| snapshot.slot == slot)
        else {
            return Err(SessionViolation::new(
                "C-INVALID-EDIT",
                "unknown commitment slot for this authoritative revision",
            ));
        };
        if snapshot.state.lock_intent() != LockIntent::StrongLocked {
            return Err(SessionViolation::new(
                "C-STRONG-UNLOCK-REQUIRED",
                "unlock tokens are issued only for strong-locked slots",
            ));
        }
        let material = format!(
            "{}|{}|{}|{}|{}",
            self.uri, self.authoritative.version, slot.0, snapshot.state, self.next_nonce
        );
        self.next_nonce = self.next_nonce.saturating_add(1);
        let token = RevisionHash::from_source(&material).0;
        let issued = SessionUnlockToken {
            token: token.clone(),
            uri: self.uri.clone(),
            authoritative_version: self.authoritative.version,
            slot,
            from: snapshot.state,
            used: false,
        };
        self.unlock_tokens.insert(token, issued.clone());
        Ok(issued)
    }

    pub fn prepare_edit(&mut self, request: DocumentEditRequest) -> DocumentEditResponse {
        self.prepare_edit_inner(request, false)
    }

    fn prepare_edit_inner(
        &mut self,
        request: DocumentEditRequest,
        allow_divergence: bool,
    ) -> DocumentEditResponse {
        let mut violations = Vec::new();
        if self.pending.is_some() {
            violations.push(SessionViolation::new(
                "C-INVALID-EDIT",
                "another transaction is pending; apply it or observe another revision before preparing a replacement",
            ));
        }
        if request.base_version != self.authoritative.version {
            violations.push(SessionViolation::new(
                "C-STALE-REVISION",
                format!(
                    "base version {} does not match authoritative version {}",
                    request.base_version, self.authoritative.version
                ),
            ));
        }
        if self.mode == CollaborationMode::Strict && self.is_divergent() && !allow_divergence {
            violations.push(SessionViolation::new(
                "C-DOCUMENT-DIVERGED",
                "restore or adopt the observed revision before preparing a strict edit",
            ));
        }
        if !violations.is_empty() {
            return self.rejected_edit(violations);
        }

        let candidate_source = match apply_text_edits(&self.authoritative, &request.edits) {
            Ok(source) => source,
            Err(violation) => return self.rejected_edit(vec![violation]),
        };
        let candidate = DocumentRevision::parse(
            self.authoritative.version.saturating_add(1),
            candidate_source,
        );
        if !candidate.is_complete() {
            return self.rejected_edit(vec![SessionViolation::new(
                "C-PARTIAL-CANDIDATE",
                "actor-declared edits must produce a Complete document",
            )]);
        }

        let strong_unlocks =
            strong_unlock_slots(self.authoritative.document(), candidate.document());
        let supplied: HashSet<&str> = request.unlock_tokens.iter().map(String::as_str).collect();
        let unlock_authorized = strong_unlocks.iter().all(|(slot, from)| {
            self.unlock_tokens.values().any(|token| {
                supplied.contains(token.token.as_str())
                    && !token.used
                    && token.uri == self.uri
                    && token.authoritative_version == self.authoritative.version
                    && token.slot == *slot
                    && token.from == *from
            })
        });
        if !strong_unlocks.is_empty() && !unlock_authorized {
            return self.rejected_edit(vec![SessionViolation::new(
                "C-STRONG-UNLOCK-REQUIRED",
                "every weakened strong-lock slot requires a matching unused Human token",
            )]);
        }

        let mut authorization = request.authorization;
        authorization.unlock_strong_lock = unlock_authorized && !strong_unlocks.is_empty();
        let mut new_challenges = Vec::new();
        for result in validate_document_patch(
            self.authoritative.document(),
            candidate.document(),
            &DocumentPatchRequest {
                actor: Some(request.actor),
                authorization,
                challenge_reason: request.challenge_reason.as_deref(),
            },
        ) {
            match result {
                Err(violation) => violations.push(patch_violation(violation)),
                Ok(diff)
                    if request.actor == Actor::Ai
                        && matches!(
                            (diff.from, diff.to),
                            (Commitment::Locked, Commitment::LockedQuestion)
                                | (Commitment::StrongLocked, Commitment::StrongLockedQuestion)
                        ) =>
                {
                    if let Ok(challenge) = build_lock_challenge(
                        self.authoritative.document(),
                        diff.before.slot,
                        diff.from,
                        diff.to,
                        request.challenge_reason.as_deref().unwrap_or(""),
                        Vec::new(),
                        vec!["review lock readiness".into()],
                    ) {
                        new_challenges.push(challenge);
                    }
                }
                Ok(_) => {}
            }
        }
        dedup_violations(&mut violations);
        if !violations.is_empty() {
            return self.rejected_edit(violations);
        }

        let reserved_unlock_tokens = strong_unlocks
            .iter()
            .filter_map(|(slot, from)| {
                self.unlock_tokens.values().find(|token| {
                    supplied.contains(token.token.as_str())
                        && !token.used
                        && token.uri == self.uri
                        && token.authoritative_version == self.authoritative.version
                        && token.slot == *slot
                        && token.from == *from
                })
            })
            .map(|token| token.token.clone())
            .collect::<Vec<_>>();

        let transaction_id = RevisionHash::from_source(&format!(
            "{}|{}|{}|edit|{}",
            self.uri,
            self.authoritative.version,
            candidate.hash().0,
            self.next_nonce
        ))
        .0;
        self.next_nonce = self.next_nonce.saturating_add(1);
        self.pending = Some(PendingTransaction {
            id: transaction_id.clone(),
            candidate: candidate.clone(),
            challenges: new_challenges,
            reserved_unlock_tokens,
        });
        DocumentEditResponse {
            schema_version: LANGUAGE_SERVICE_SCHEMA_VERSION,
            accepted: true,
            authoritative_version: self.authoritative.version,
            candidate_hash: Some(candidate.hash().clone()),
            transaction_id: Some(transaction_id),
            violations: Vec::new(),
        }
    }

    pub fn pending_transaction_id(&self) -> Option<&str> {
        self.pending.as_ref().map(|pending| pending.id.as_str())
    }

    pub fn adopt_observed(
        &mut self,
        base_version: u64,
        actor: Actor,
        authorization: HumanAuthorization,
        unlock_tokens: &[String],
    ) -> Result<(), Vec<SessionViolation>> {
        if actor != Actor::Human {
            return Err(vec![SessionViolation::new(
                "C-ACTOR-REQUIRED",
                "only a Human actor may adopt an observed revision",
            )]);
        }
        let request = DocumentEditRequest {
            base_version,
            actor,
            edits: vec![SessionTextEdit {
                range: None,
                text: self.observed.source().to_string(),
            }],
            authorization,
            unlock_tokens: unlock_tokens.to_vec(),
            challenge_reason: None,
        };
        // Adoption may resolve divergence, but it still passes the protected
        // content and strong-lock authorization checks.
        let response = self.prepare_edit_inner(request, true);
        if !response.accepted {
            return Err(response.violations);
        }
        let observed_source = self.observed.source().to_string();
        self.observe_full(observed_source);
        Ok(())
    }

    pub fn prepare_restore(&mut self, base_version: u64, actor: Actor) -> DocumentEditResponse {
        if actor != Actor::Human {
            return self.rejected_edit(vec![SessionViolation::new(
                "C-ACTOR-REQUIRED",
                "only a Human actor may restore the authoritative revision",
            )]);
        }
        if self.pending.is_some() {
            return self.rejected_edit(vec![SessionViolation::new(
                "C-INVALID-EDIT",
                "another transaction is pending; apply it or observe another revision before preparing a restore",
            )]);
        }
        if base_version != self.authoritative.version {
            return self.rejected_edit(vec![SessionViolation::new(
                "C-STALE-REVISION",
                format!(
                    "base version {base_version} does not match authoritative version {}",
                    self.authoritative.version
                ),
            )]);
        }

        let candidate = self
            .authoritative
            .with_version(self.authoritative.version.saturating_add(1));
        let transaction_id = RevisionHash::from_source(&format!(
            "{}|{}|{}|restore|{}",
            self.uri,
            self.authoritative.version,
            candidate.hash().0,
            self.next_nonce
        ))
        .0;
        self.next_nonce = self.next_nonce.saturating_add(1);
        self.pending = Some(PendingTransaction {
            id: transaction_id.clone(),
            candidate: candidate.clone(),
            challenges: Vec::new(),
            reserved_unlock_tokens: Vec::new(),
        });
        DocumentEditResponse {
            schema_version: LANGUAGE_SERVICE_SCHEMA_VERSION,
            accepted: true,
            authoritative_version: self.authoritative.version,
            candidate_hash: Some(candidate.hash().clone()),
            transaction_id: Some(transaction_id),
            violations: Vec::new(),
        }
    }

    fn rejected_edit(&mut self, violations: Vec<SessionViolation>) -> DocumentEditResponse {
        DocumentEditResponse {
            schema_version: LANGUAGE_SERVICE_SCHEMA_VERSION,
            accepted: false,
            authoritative_version: self.authoritative.version,
            candidate_hash: None,
            transaction_id: None,
            violations,
        }
    }

    fn push_violation(&mut self, code: &str, message: &str) {
        self.violations.push(SessionViolation::new(code, message));
        dedup_violations(&mut self.violations);
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            schema_version: LANGUAGE_SERVICE_SCHEMA_VERSION,
            uri: self.uri.clone(),
            version: self.observed.version,
            observed_version: self.observed.version,
            authoritative_version: self.authoritative.version,
            observed_hash: self.observed.hash().clone(),
            authoritative_hash: self.authoritative.hash().clone(),
            authoritative_trusted: self.authoritative_trusted,
            divergent: self.is_divergent(),
            mode: self.mode,
            error_count: self.observed.errors().len(),
            violations: self.violations.clone(),
            lock_challenges: self.lock_challenges.clone(),
            ide: crate::ide::ide_snapshot_from_diagnostics(
                self.observed.document(),
                self.observed.diagnostics(),
            ),
        }
    }

    pub fn semantic_tokens(&self) -> Vec<SemanticToken> {
        semantic_tokens(self.observed.document())
    }

    pub fn hover(&self, offset: u32) -> Option<HoverInfo> {
        hover_at(self.observed.document(), offset)
    }

    pub fn code_actions(&self, node: SourceNodeId) -> Vec<CodeAction> {
        if self.mode == CollaborationMode::Strict && self.is_divergent() {
            return Vec::new();
        }
        code_actions_for_node(self.authoritative.document(), node)
    }

    #[cfg(feature = "experimental-targets")]
    pub fn materialize(&self, release_scope: &str) -> MaterializationPlan {
        plan_materialization(self.authoritative.document(), release_scope)
    }

    #[cfg(feature = "experimental-targets")]
    pub fn profile(&self, target: &str, release_scope: &str) -> Option<ProfileAnalysis> {
        match target {
            "mimi" => Some(analyze_mimi_profile(
                self.authoritative.document(),
                release_scope,
            )),
            "generic" => Some(analyze_generic_profile(
                self.authoritative.document(),
                release_scope,
            )),
            _ => None,
        }
    }
}

fn apply_sequential_text_edit(
    source: &mut String,
    line_index: &mut LineIndex,
    edit: &SessionTextEdit,
) -> Result<(), SessionViolation> {
    let Some(range) = edit.range else {
        *source = edit.text.clone();
        *line_index = LineIndex::new(source);
        return Ok(());
    };
    let start = line_index
        .offset(
            source,
            SourcePosition {
                line: range.start.line,
                column: range.start.character,
            },
            ColumnEncoding::Utf16,
        )
        .ok_or_else(|| SessionViolation::new("C-INVALID-EDIT", "invalid UTF-16 start position"))?;
    let end = line_index
        .offset(
            source,
            SourcePosition {
                line: range.end.line,
                column: range.end.character,
            },
            ColumnEncoding::Utf16,
        )
        .ok_or_else(|| SessionViolation::new("C-INVALID-EDIT", "invalid UTF-16 end position"))?;
    if start > end {
        return Err(SessionViolation::new(
            "C-INVALID-EDIT",
            "edit range start is after its end",
        ));
    }
    line_index.apply_edit(start, end, &edit.text);
    source.replace_range(start as usize..end as usize, &edit.text);
    Ok(())
}

fn advisory_patch_violations(
    before: &LosslessDocument,
    after: &LosslessDocument,
) -> Vec<SessionViolation> {
    let mut violations = Vec::new();
    for result in validate_document_patch(
        before,
        after,
        &DocumentPatchRequest {
            actor: None,
            authorization: HumanAuthorization::default(),
            challenge_reason: None,
        },
    ) {
        if let Err(violation) = result {
            violations.push(patch_violation(violation));
        }
    }
    for result in validate_document_patch(
        before,
        after,
        &DocumentPatchRequest {
            actor: Some(Actor::Human),
            authorization: HumanAuthorization::default(),
            challenge_reason: None,
        },
    ) {
        if let Err(violation) = result {
            violations.push(patch_violation(violation));
        }
    }
    for result in validate_document_patch(
        before,
        after,
        &DocumentPatchRequest {
            actor: Some(Actor::Ai),
            authorization: HumanAuthorization::default(),
            challenge_reason: None,
        },
    ) {
        if let Err(violation) = result {
            violations.push(patch_violation(violation));
        }
    }
    dedup_violations(&mut violations);
    violations
}

fn patch_violation(violation: DocumentPatchViolation) -> SessionViolation {
    match violation {
        DocumentPatchViolation::ActorDeclarationRequired => SessionViolation::new(
            "C-ACTOR-REQUIRED",
            "the observed edit did not declare Human or AI authority",
        ),
        DocumentPatchViolation::Transition(transition) => match transition {
            TransitionViolation::AiTransitionForbidden => SessionViolation::new(
                "C-AI-TRANSITION-FORBIDDEN",
                "the AI transition is not permitted",
            ),
            TransitionViolation::ProtectedContentChanged => {
                SessionViolation::new("C-PROTECTED-CONTENT", "protected content changed")
            }
            TransitionViolation::ProtectedStructureChanged => {
                SessionViolation::new("C-PROTECTED-STRUCTURE", "protected structure changed")
            }
            TransitionViolation::ProtectedAttachmentChanged => SessionViolation::new(
                "C-PROTECTED-ATTACHMENT",
                "protected rule attachment changed",
            ),
            TransitionViolation::ChallengeReasonRequired => SessionViolation::new(
                "C-CHALLENGE-REASON-REQUIRED",
                "a lock challenge requires a non-empty reason",
            ),
            TransitionViolation::HumanAuthorizationRequired => SessionViolation::new(
                "C-HUMAN-AUTHORIZATION-REQUIRED",
                "protected content requires explicit Human authorization",
            ),
            TransitionViolation::StrongUnlockAuthorizationRequired => SessionViolation::new(
                "C-STRONG-UNLOCK-REQUIRED",
                "weakening a strong lock requires a matching Human unlock token",
            ),
        },
    }
}

fn dedup_violations(violations: &mut Vec<SessionViolation>) {
    let mut seen = HashSet::new();
    violations.retain(|violation| seen.insert((violation.code.clone(), violation.message.clone())));
}

fn apply_text_edits(
    revision: &DocumentRevision,
    edits: &[SessionTextEdit],
) -> Result<String, SessionViolation> {
    if edits.is_empty() {
        return Ok(revision.source().to_string());
    }
    if edits.len() == 1 && edits[0].range.is_none() {
        return Ok(edits[0].text.clone());
    }
    if edits.iter().any(|edit| edit.range.is_none()) {
        return Err(SessionViolation::new(
            "C-INVALID-EDIT",
            "a full-document replacement cannot be combined with ranged edits",
        ));
    }

    let mut byte_edits = Vec::with_capacity(edits.len());
    for edit in edits {
        let range = edit.range.expect("checked above");
        let start = revision
            .document()
            .line_index()
            .offset(
                revision.source(),
                SourcePosition {
                    line: range.start.line,
                    column: range.start.character,
                },
                ColumnEncoding::Utf16,
            )
            .ok_or_else(|| {
                SessionViolation::new("C-INVALID-EDIT", "invalid UTF-16 start position")
            })?;
        let end = revision
            .document()
            .line_index()
            .offset(
                revision.source(),
                SourcePosition {
                    line: range.end.line,
                    column: range.end.character,
                },
                ColumnEncoding::Utf16,
            )
            .ok_or_else(|| {
                SessionViolation::new("C-INVALID-EDIT", "invalid UTF-16 end position")
            })?;
        if start > end {
            return Err(SessionViolation::new(
                "C-INVALID-EDIT",
                "edit range start is after its end",
            ));
        }
        byte_edits.push((start as usize, end as usize, edit.text.as_str()));
    }
    byte_edits.sort_by_key(|(start, end, _)| (*start, *end));
    if byte_edits.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(SessionViolation::new(
            "C-INVALID-EDIT",
            "overlapping edits are not allowed",
        ));
    }

    let mut source = revision.source().to_string();
    for (start, end, text) in byte_edits.into_iter().rev() {
        source.replace_range(start..end, text);
    }
    Ok(source)
}

fn strong_unlock_slots(
    before: &LosslessDocument,
    after: &LosslessDocument,
) -> Vec<(CommitmentSlotId, Commitment)> {
    let after_slots = collect_semantic_slot_snapshots(after);
    let mut matched_after = HashSet::new();
    collect_semantic_slot_snapshots(before)
        .into_iter()
        .filter(|before| before.state.lock_intent() == LockIntent::StrongLocked)
        .filter_map(|before| {
            let after = after_slots.iter().enumerate().find(|(index, after)| {
                !matched_after.contains(index)
                    && after.position == before.position
                    && after.kind == before.kind
                    && after.anchor_kind == before.anchor_kind
                    && after.footprint == before.footprint
                    && after.owner_slot_index == before.owner_slot_index
            });
            if let Some((index, _)) = after {
                matched_after.insert(index);
            }
            let after = after.map(|(_, after)| after);
            if after.is_none_or(|after| after.state.lock_intent() != LockIntent::StrongLocked) {
                Some((before.slot, before.state))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_update_bumps_version_and_reports_undeclared_actor() {
        let mut session = DocumentSession::open("file:///demo.mms", "desc?? \"app\"\n");
        assert!(Arc::ptr_eq(
            &session.observed.payload,
            &session.authoritative.payload
        ));
        assert_eq!(session.version(), 1);
        assert!(!session.diagnostics().delegation_queue.is_empty());

        session.update_full("func Pay$:\n    steps:\n        charge payment\n");
        assert_eq!(session.version(), 2);
        assert!(session.errors().is_empty());
        assert!(session
            .violations()
            .iter()
            .any(|violation| violation.code == "C-ACTOR-REQUIRED"));
        let snapshot = session.snapshot();
        assert!(!snapshot.authoritative_trusted);
        assert!(snapshot
            .ide
            .semantic_tokens
            .iter()
            .any(|token| matches!(token.kind, crate::ide::SemanticTokenKind::CommitmentLocked)));
    }

    #[test]
    fn strict_mode_keeps_unapproved_change_out_of_authoritative_revision() {
        let source = "func Pay$:\n    steps:\n        charge payment\n";
        let mut session = DocumentSession::open("file:///pay.mms", source);
        session.set_mode(CollaborationMode::Strict);
        session.observe_full(source.replace("charge", "refund"));
        assert_ne!(session.source(), session.authoritative_source());
        assert!(session.is_divergent());
        assert!(session
            .violations()
            .iter()
            .any(|violation| violation.code == "C-DOCUMENT-DIVERGED"));
    }

    #[test]
    fn actor_declared_edit_is_confirmed_by_matching_observation() {
        let source = "desc?? \"draft\"\n";
        let mut session = DocumentSession::open("file:///draft.mms", source);
        let response = session.prepare_edit(DocumentEditRequest {
            base_version: 1,
            actor: Actor::Ai,
            edits: vec![SessionTextEdit {
                range: None,
                text: "desc? \"proposal\"\n".into(),
            }],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: None,
        });
        assert!(response.accepted, "{:?}", response.violations);
        assert!(session.pending_transaction_id().is_some());
        let parses_before_confirmation = session.observed_parse_count;
        session.observe_full("desc? \"proposal\"\n".into());
        assert_eq!(session.observed_parse_count, parses_before_confirmation);
        assert_eq!(session.authoritative_source(), "desc? \"proposal\"\n");
        assert!(Arc::ptr_eq(
            &session.observed.payload,
            &session.authoritative.payload
        ));
        assert!(!session.is_divergent());
    }

    #[test]
    fn ai_cannot_self_delegate_a_fresh_slot() {
        let mut session = DocumentSession::open("file:///fresh.mms", "");
        let review = session.prepare_edit(DocumentEditRequest {
            base_version: 1,
            actor: Actor::Ai,
            edits: vec![SessionTextEdit {
                range: None,
                text: "desc? \"new intent\"\n".into(),
            }],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: None,
        });
        assert!(review.accepted, "{:?}", review.violations);

        let mut session = DocumentSession::open("file:///delegated.mms", "");
        let delegated = session.prepare_edit(DocumentEditRequest {
            base_version: 1,
            actor: Actor::Ai,
            edits: vec![SessionTextEdit {
                range: None,
                text: "desc?? \"new intent\"\n".into(),
            }],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: None,
        });
        assert!(!delegated.accepted);
        assert!(delegated
            .violations
            .iter()
            .any(|violation| violation.code == "C-AI-TRANSITION-FORBIDDEN"));
    }

    #[test]
    fn strong_unlock_token_is_bound_and_single_use() {
        let source = "func$$ Pay:\n    steps:\n        charge payment\n";
        let mut session = DocumentSession::open("file:///pay.mms", source);
        let slot = collect_semantic_slot_snapshots(session.authoritative_document())
            .into_iter()
            .find(|slot| slot.state == Commitment::StrongLocked)
            .unwrap()
            .slot;
        let token = session.issue_unlock_token(1, Actor::Human, slot).unwrap();
        let request = || DocumentEditRequest {
            base_version: 1,
            actor: Actor::Human,
            edits: vec![SessionTextEdit {
                range: None,
                text: "func$ Pay:\n    steps:\n        charge payment\n".into(),
            }],
            authorization: HumanAuthorization {
                modify_protected: true,
                unlock_strong_lock: false,
            },
            unlock_tokens: vec![token.token.clone()],
            challenge_reason: None,
        };
        session.set_mode(CollaborationMode::Strict);
        assert!(session.prepare_edit(request()).accepted);
        assert!(!session.unlock_tokens.get(&token.token).unwrap().used);
        let blocked = session.prepare_edit(request());
        assert!(!blocked.accepted);
        assert!(blocked
            .violations
            .iter()
            .any(|violation| violation.message.contains("transaction is pending")));

        // Observing a different revision cancels the candidate without spending
        // its reserved token.
        session.observe_full(source.into());
        assert!(session.pending_transaction_id().is_none());
        assert!(!session.unlock_tokens.get(&token.token).unwrap().used);
        let accepted = session.prepare_edit(request());
        assert!(accepted.accepted, "{:?}", accepted.violations);
        session.observe_full("func$ Pay:\n    steps:\n        charge payment\n".into());
        assert!(session.pending_transaction_id().is_none());
        assert!(session.unlock_tokens.get(&token.token).unwrap().used);
    }

    #[test]
    fn ai_cannot_delete_an_identical_locked_node_from_another_scope() {
        for suffix in ["$", "$$"] {
            let source = format!(
                "module A:\n    func{suffix} Same: ...\nmodule B:\n    func{suffix} Same: ...\n"
            );
            let candidate = format!("module A:\n    func{suffix} Same: ...\n");
            let mut session = DocumentSession::open("file:///cross-scope.mms", source);
            let response = session.prepare_edit(DocumentEditRequest {
                base_version: 1,
                actor: Actor::Ai,
                edits: vec![SessionTextEdit {
                    range: None,
                    text: candidate,
                }],
                authorization: HumanAuthorization::default(),
                unlock_tokens: Vec::new(),
                challenge_reason: None,
            });
            assert!(!response.accepted, "{suffix}: {response:?}");
            assert!(response.transaction_id.is_none());

            if suffix == "$$" {
                let surviving_slot =
                    collect_semantic_slot_snapshots(session.authoritative_document())
                        .into_iter()
                        .find(|slot| slot.state == Commitment::StrongLocked)
                        .unwrap()
                        .slot;
                let wrong_token = session
                    .issue_unlock_token(1, Actor::Human, surviving_slot)
                    .unwrap();
                let human = session.prepare_edit(DocumentEditRequest {
                    base_version: 1,
                    actor: Actor::Human,
                    edits: vec![SessionTextEdit {
                        range: None,
                        text: format!("module A:\n    func{suffix} Same: ...\n"),
                    }],
                    authorization: HumanAuthorization {
                        modify_protected: true,
                        unlock_strong_lock: false,
                    },
                    unlock_tokens: vec![wrong_token.token],
                    challenge_reason: None,
                });
                assert!(!human.accepted, "wrong slot token: {human:?}");
            }
        }
    }

    #[test]
    fn rejected_prepare_preserves_the_existing_pending_candidate() {
        let mut session = DocumentSession::open("file:///pending.mms", "desc?? \"draft\"\n");
        let accepted = session.prepare_edit(DocumentEditRequest {
            base_version: 1,
            actor: Actor::Ai,
            edits: vec![SessionTextEdit {
                range: None,
                text: "desc? \"proposal\"\n".into(),
            }],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: None,
        });
        assert!(accepted.accepted);
        let pending = session.pending_transaction_id().unwrap().to_string();

        let rejected = session.prepare_restore(99, Actor::Human);
        assert!(!rejected.accepted);
        assert_eq!(session.pending_transaction_id(), Some(pending.as_str()));

        session.observe_full("desc? \"proposal\"\n".into());
        assert_eq!(session.authoritative_source(), "desc? \"proposal\"\n");
    }

    #[test]
    fn accepted_ai_lock_challenge_enters_snapshot_after_observation() {
        let source = "func$ Pay:\n    steps:\n        charge payment\n";
        let mut session = DocumentSession::open("file:///challenge.mms", source);
        let response = session.prepare_edit(DocumentEditRequest {
            base_version: 1,
            actor: Actor::Ai,
            edits: vec![SessionTextEdit {
                range: None,
                text: "func$? Pay:\n    steps:\n        charge payment\n".into(),
            }],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: Some("failure behavior is incomplete".into()),
        });
        assert!(response.accepted, "{:?}", response.violations);
        session.observe_full("func$? Pay:\n    steps:\n        charge payment\n".into());
        assert_eq!(session.lock_challenges().len(), 1);
        assert_eq!(
            session.lock_challenges()[0].reason,
            "failure behavior is incomplete"
        );
    }

    #[test]
    fn utf16_ranged_edit_supports_cjk_and_crlf() {
        let source = "desc \"家庭账本\"\r\n";
        let mut session = DocumentSession::open("file:///cjk.mms", source);
        let response = session.prepare_edit(DocumentEditRequest {
            base_version: 1,
            actor: Actor::Human,
            edits: vec![SessionTextEdit {
                range: Some(TextRange {
                    start: TextPosition {
                        line: 0,
                        character: 6,
                    },
                    end: TextPosition {
                        line: 0,
                        character: 10,
                    },
                }),
                text: "共同账本".into(),
            }],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: None,
        });
        assert!(response.accepted, "{:?}", response.violations);
    }

    #[test]
    fn invalid_observed_range_is_retained_as_a_collaboration_diagnostic() {
        let mut session = DocumentSession::open("file:///range.mms", "desc \"家庭账本\"\n");
        let result = session.observe_edits(&[SessionTextEdit {
            range: Some(TextRange {
                start: TextPosition {
                    line: 0,
                    character: 100,
                },
                end: TextPosition {
                    line: 0,
                    character: 101,
                },
            }),
            text: "x".into(),
        }]);
        assert!(result.is_err());
        assert!(session
            .violations()
            .iter()
            .any(|violation| violation.code == "C-INVALID-EDIT"));
    }

    #[test]
    fn batched_utf16_changes_are_sequential_but_parse_only_once() {
        let mut session = DocumentSession::open("file:///批量.mms", "desc \"甲乙\"\n");
        let before = session.observed_parse_count;
        session
            .observe_edits(&[
                SessionTextEdit {
                    range: Some(TextRange {
                        start: TextPosition {
                            line: 0,
                            character: 6,
                        },
                        end: TextPosition {
                            line: 0,
                            character: 7,
                        },
                    }),
                    text: "家庭".into(),
                },
                // This range is relative to the result of the previous edit,
                // exactly as LSP contentChanges requires.
                SessionTextEdit {
                    range: Some(TextRange {
                        start: TextPosition {
                            line: 0,
                            character: 8,
                        },
                        end: TextPosition {
                            line: 0,
                            character: 9,
                        },
                    }),
                    text: "账本".into(),
                },
            ])
            .unwrap();
        assert_eq!(session.source(), "desc \"家庭账本\"\n");
        assert_eq!(session.observed_parse_count, before + 1);
        assert!(session.errors().is_empty());
    }

    #[test]
    fn human_can_adopt_a_strict_divergence_with_the_authoritative_base() {
        let source = "desc \"draft\"\n";
        let mut session = DocumentSession::open("file:///adopt.mms", source);
        session.set_mode(CollaborationMode::Strict);
        session.observe_full("desc \"accepted\"\n".into());
        assert!(session.is_divergent());

        session
            .adopt_observed(1, Actor::Human, HumanAuthorization::default(), &[])
            .expect("Human adoption should resolve strict divergence");
        assert_eq!(session.authoritative_source(), "desc \"accepted\"\n");
        assert!(!session.is_divergent());
    }

    #[test]
    fn adopting_an_external_strong_unlock_requires_the_bound_token() {
        let source = "func$$ Pay:\n    steps:\n        charge payment\n";
        let mut session = DocumentSession::open("file:///adopt-lock.mms", source);
        let slot = collect_semantic_slot_snapshots(session.authoritative_document())
            .into_iter()
            .find(|slot| slot.state == Commitment::StrongLocked)
            .unwrap()
            .slot;
        let token = session.issue_unlock_token(1, Actor::Human, slot).unwrap();
        session.set_mode(CollaborationMode::Strict);
        session.observe_full("func$ Pay:\n    steps:\n        charge payment\n".into());

        let authorization = HumanAuthorization {
            modify_protected: true,
            unlock_strong_lock: false,
        };
        let rejected = session
            .adopt_observed(1, Actor::Human, authorization, &[])
            .unwrap_err();
        assert!(rejected
            .iter()
            .any(|violation| violation.code == "C-STRONG-UNLOCK-REQUIRED"));

        session
            .adopt_observed(1, Actor::Human, authorization, &[token.token])
            .expect("the bound single-use token should authorize adoption");
        assert!(!session.is_divergent());
    }

    #[test]
    fn restore_requires_human_and_waits_for_the_observed_confirmation() {
        let source = "desc \"trusted\"\n";
        let mut session = DocumentSession::open("file:///restore.mms", source);
        session.set_mode(CollaborationMode::Strict);
        session.observe_full("desc \"external\"\n".into());

        let rejected = session.prepare_restore(1, Actor::Ai);
        assert!(!rejected.accepted);
        assert_eq!(rejected.violations[0].code, "C-ACTOR-REQUIRED");

        let accepted = session.prepare_restore(1, Actor::Human);
        assert!(accepted.accepted);
        assert_eq!(session.source(), "desc \"external\"\n");
        assert!(session.pending_transaction_id().is_some());

        let parses_before_confirmation = session.observed_parse_count;
        session.observe_full(source.into());
        assert_eq!(session.observed_parse_count, parses_before_confirmation);
        assert!(Arc::ptr_eq(
            &session.observed.payload,
            &session.authoritative.payload
        ));
        assert_eq!(session.source(), session.authoritative_source());
        assert!(!session.is_divergent());
    }

    #[cfg(feature = "experimental-targets")]
    #[test]
    fn session_profile_and_materialize_use_authoritative_revision() {
        let session = DocumentSession::open(
            "mem://pay.mms",
            "func Pay$:\n    steps:\n        charge payment\n",
        );
        let plan = session.materialize("v1");
        assert!(!plan.selection.slots.is_empty());
        let analysis = session.profile("mimi", "v1").unwrap();
        assert_eq!(analysis.profile.name, "mimi");
    }
}
