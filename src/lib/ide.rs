use serde::Serialize;

use crate::ast::Commitment;
use crate::collaboration::{validate_transition, Actor, TransitionEffects, TransitionRequest};
use crate::diagnostics::{analyze_document, DocumentDiagnostics, QueueItem};
use crate::error::ParseError;
use crate::lossless::{
    ByteSpan, ColumnEncoding, CommitmentAnchorKind, CommitmentSlotSyntax, LosslessDocument,
    SourceNodeId, SourcePosition,
};

/// Semantic token kinds for commitment and rule attachment highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTokenKind {
    CommitmentOpen,
    CommitmentContentReview,
    CommitmentContentDelegated,
    CommitmentLocked,
    CommitmentLockReview,
    CommitmentLockDelegated,
    CommitmentStrongLocked,
    CommitmentStrongLockReview,
    CommitmentStrongLockDelegated,
    RuleAttached,
    RuleEnvironment,
    RuleDropped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticToken {
    pub span: ByteSpan,
    pub kind: SemanticTokenKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoverInfo {
    pub span: ByteSpan,
    pub title: String,
    pub body: String,
    pub commitment: Option<Commitment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeActionKind {
    MarkContentReview,
    MarkContentDelegated,
    ProposeLockReview,
    ProposeStrongLockReview,
    AcceptOrdinaryLock,
    AcceptStrongLock,
    ChallengeOrdinaryLock,
    ChallengeStrongLock,
    ShowRuleScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeAction {
    pub kind: CodeActionKind,
    pub title: String,
    pub target: SourceNodeId,
    pub from: Commitment,
    pub to: Option<Commitment>,
    pub actor: Actor,
    pub allowed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdeSnapshot {
    pub semantic_tokens: Vec<SemanticToken>,
    pub decision_queue: Vec<QueueItem>,
    pub delegation_queue: Vec<QueueItem>,
    pub diagnostics: DocumentDiagnostics,
}

/// Build a library-level IDE snapshot from a lossless document.
pub fn ide_snapshot(document: &LosslessDocument, errors: &[ParseError]) -> IdeSnapshot {
    let diagnostics = analyze_document(document, errors);
    IdeSnapshot {
        semantic_tokens: semantic_tokens(document),
        decision_queue: diagnostics.decision_queue.clone(),
        delegation_queue: diagnostics.delegation_queue.clone(),
        diagnostics,
    }
}

pub fn semantic_tokens(document: &LosslessDocument) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();
    for slot in document.commitment_slots() {
        if !slot.semantic_slot {
            continue;
        }
        let span = if slot.suffix_span.is_empty() {
            slot.anchor_span
        } else {
            slot.suffix_span
        };
        tokens.push(SemanticToken {
            span,
            kind: commitment_token_kind(slot.value),
            label: format!(
                "{}{}",
                document.text(slot.anchor_span).unwrap_or(""),
                document.text(slot.suffix_span).unwrap_or("")
            ),
        });
    }
    for rule in document.rules() {
        let kind = match rule.attachment {
            crate::lossless::RuleAttachment::Attached => SemanticTokenKind::RuleAttached,
            crate::lossless::RuleAttachment::Environment => SemanticTokenKind::RuleEnvironment,
            crate::lossless::RuleAttachment::DroppedByRecovery
            | crate::lossless::RuleAttachment::Pending => SemanticTokenKind::RuleDropped,
        };
        tokens.push(SemanticToken {
            span: rule.span,
            kind,
            label: document.text(rule.span).unwrap_or("rule").to_string(),
        });
    }
    tokens.sort_by_key(|token| token.span.start);
    tokens
}

pub fn hover_at(document: &LosslessDocument, offset: u32) -> Option<HoverInfo> {
    if let Some(slot) = document.commitment_slots().iter().find(|slot| {
        slot.semantic_slot
            && (contains(slot.full_span, offset)
                || contains(slot.suffix_span, offset)
                || contains(slot.anchor_span, offset))
    }) {
        return Some(hover_for_slot(document, slot));
    }
    if let Some(rule) = document
        .rules()
        .iter()
        .find(|rule| contains(rule.span, offset))
    {
        let attachment = format!("{:?}", rule.attachment);
        let target = rule
            .target
            .and_then(|id| document.node(id))
            .map(|node| {
                format!(
                    "{:?} {}",
                    node.kind,
                    document.text(node.spans.header).unwrap_or("")
                )
            })
            .unwrap_or_else(|| "none".into());
        return Some(HoverInfo {
            span: rule.span,
            title: "Rule attachment".into(),
            body: format!("attachment={attachment}\ntarget={target}"),
            commitment: None,
        });
    }
    None
}

pub fn hover_at_position(
    document: &LosslessDocument,
    position: SourcePosition,
    encoding: ColumnEncoding,
) -> Option<HoverInfo> {
    let offset = document
        .line_index()
        .offset(document.source(), position, encoding)?;
    hover_at(document, offset)
}

/// Suggest actor-aware code actions for a node based on its header commitment.
pub fn code_actions_for_node(document: &LosslessDocument, node: SourceNodeId) -> Vec<CodeAction> {
    let Some(source_node) = document.node(node) else {
        return Vec::new();
    };
    let header = document.text(source_node.spans.header).unwrap_or("");
    let from = crate::collaboration::collect_slot_snapshots(document)
        .into_iter()
        .find(|slot| slot.node == node)
        .map(|slot| slot.state)
        .unwrap_or_else(|| header_commitment(header));

    let candidates = [
        (
            CodeActionKind::MarkContentReview,
            "Mark content for human review (?)",
            Actor::Ai,
            Commitment::Question,
            None,
        ),
        (
            CodeActionKind::MarkContentDelegated,
            "Delegate content to AI (??)",
            Actor::Human,
            Commitment::QuestionQuestion,
            None,
        ),
        (
            CodeActionKind::ProposeLockReview,
            "Challenge ordinary lock ($ -> $?)",
            Actor::Ai,
            Commitment::LockedQuestion,
            Some("challenge lock readiness"),
        ),
        (
            CodeActionKind::ProposeStrongLockReview,
            "Challenge strong lock ($$ -> $$?)",
            Actor::Ai,
            Commitment::StrongLockedQuestion,
            Some("challenge strong-lock readiness"),
        ),
        (
            CodeActionKind::AcceptOrdinaryLock,
            "Accept ordinary lock ($)",
            Actor::Human,
            Commitment::Locked,
            None,
        ),
        (
            CodeActionKind::AcceptStrongLock,
            "Accept strong lock ($$)",
            Actor::Human,
            Commitment::StrongLocked,
            None,
        ),
        (
            CodeActionKind::ChallengeOrdinaryLock,
            "Open ordinary lock challenge",
            Actor::Ai,
            Commitment::LockedQuestion,
            Some("new evidence conflicts with the lock"),
        ),
        (
            CodeActionKind::ChallengeStrongLock,
            "Open strong lock challenge",
            Actor::Ai,
            Commitment::StrongLockedQuestion,
            Some("new evidence conflicts with the strong lock"),
        ),
    ];

    let mut actions = Vec::new();
    for (kind, title, actor, to, reason) in candidates {
        if from == to {
            continue;
        }
        // Only surface transitions that match the current lock family / action intent.
        let relevant = match kind {
            CodeActionKind::MarkContentReview | CodeActionKind::MarkContentDelegated => {
                !from.protects_content()
            }
            CodeActionKind::ProposeLockReview | CodeActionKind::ChallengeOrdinaryLock => {
                from == Commitment::Locked
            }
            CodeActionKind::ProposeStrongLockReview | CodeActionKind::ChallengeStrongLock => {
                from == Commitment::StrongLocked
            }
            CodeActionKind::AcceptOrdinaryLock => {
                matches!(
                    from,
                    Commitment::None
                        | Commitment::Question
                        | Commitment::QuestionQuestion
                        | Commitment::LockedQuestion
                        | Commitment::LockedQuestionQuestion
                )
            }
            CodeActionKind::AcceptStrongLock => {
                matches!(
                    from,
                    Commitment::Locked
                        | Commitment::StrongLockedQuestion
                        | Commitment::StrongLockedQuestionQuestion
                )
            }
            CodeActionKind::ShowRuleScope => true,
        };
        if !relevant {
            continue;
        }
        let request = TransitionRequest {
            actor,
            from,
            to,
            effects: TransitionEffects::default(),
            authorization: crate::collaboration::HumanAuthorization {
                modify_protected: actor == Actor::Human,
                unlock_strong_lock: actor == Actor::Human,
            },
            challenge_reason: reason,
        };
        let decision = validate_transition(&request);
        actions.push(CodeAction {
            kind,
            title: title.into(),
            target: node,
            from,
            to: Some(to),
            actor,
            allowed: decision.is_ok(),
            reason: decision.err().map(|err| format!("{err:?}")),
        });
    }

    actions.push(CodeAction {
        kind: CodeActionKind::ShowRuleScope,
        title: "Show attached/environment rule scope".into(),
        target: node,
        from,
        to: None,
        actor: Actor::Human,
        allowed: true,
        reason: None,
    });
    actions
}

fn hover_for_slot(document: &LosslessDocument, slot: &CommitmentSlotSyntax) -> HoverInfo {
    let anchor = document.text(slot.anchor_span).unwrap_or("");
    let suffix = document.text(slot.suffix_span).unwrap_or("");
    let slot_kind = match slot.anchor_kind {
        CommitmentAnchorKind::Keyword => "keyword",
        CommitmentAnchorKind::Identifier => "identifier",
        CommitmentAnchorKind::String => "string",
        CommitmentAnchorKind::Value => "value",
    };
    HoverInfo {
        span: slot.full_span,
        title: format!("Commitment {suffix}"),
        body: format!(
            "anchor=`{anchor}`\nslot={slot_kind}\nstate={}\nlock={:?}\nreview={:?}\ntarget={:?}\ncommit_ready={}",
            slot.value,
            slot.value.lock_intent(),
            slot.value.review_intent(),
            slot.value.review_target(),
            slot.value.is_commit_ready()
        ),
        commitment: Some(slot.value),
    }
}

fn commitment_token_kind(state: Commitment) -> SemanticTokenKind {
    match state {
        Commitment::None => SemanticTokenKind::CommitmentOpen,
        Commitment::Question => SemanticTokenKind::CommitmentContentReview,
        Commitment::QuestionQuestion => SemanticTokenKind::CommitmentContentDelegated,
        Commitment::Locked => SemanticTokenKind::CommitmentLocked,
        Commitment::LockedQuestion => SemanticTokenKind::CommitmentLockReview,
        Commitment::LockedQuestionQuestion => SemanticTokenKind::CommitmentLockDelegated,
        Commitment::StrongLocked => SemanticTokenKind::CommitmentStrongLocked,
        Commitment::StrongLockedQuestion => SemanticTokenKind::CommitmentStrongLockReview,
        Commitment::StrongLockedQuestionQuestion => {
            SemanticTokenKind::CommitmentStrongLockDelegated
        }
    }
}

fn contains(span: ByteSpan, offset: u32) -> bool {
    if span.is_empty() {
        span.start == offset
    } else {
        span.start <= offset && offset < span.end
    }
}

fn header_commitment(header: &str) -> Commitment {
    let mut found = Commitment::None;
    for token in header.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, ':' | '(' | ')' | '[' | ']' | ',' | '|')
    }) {
        let state = trailing_commitment(token);
        if rank(state) > rank(found) {
            found = state;
        }
    }
    found
}

fn trailing_commitment(token: &str) -> Commitment {
    if token.ends_with("$$??") {
        Commitment::StrongLockedQuestionQuestion
    } else if token.ends_with("$??") {
        Commitment::LockedQuestionQuestion
    } else if token.ends_with("$$?") {
        Commitment::StrongLockedQuestion
    } else if token.ends_with("$?") {
        Commitment::LockedQuestion
    } else if token.ends_with("$$") {
        Commitment::StrongLocked
    } else if token.ends_with('$') {
        Commitment::Locked
    } else if token.ends_with("??") {
        Commitment::QuestionQuestion
    } else if token.ends_with('?') {
        Commitment::Question
    } else {
        Commitment::None
    }
}

fn rank(state: Commitment) -> u8 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_lossless;

    #[test]
    fn semantic_tokens_cover_commitment_and_rules() {
        let source = r#"rule "audit"
func Pay$:
    steps:
        charge payment
"#;
        let result = parse_lossless(source);
        let tokens = semantic_tokens(&result.document);
        assert!(tokens
            .iter()
            .any(|token| token.kind == SemanticTokenKind::CommitmentLocked));
        assert!(tokens
            .iter()
            .any(|token| token.kind == SemanticTokenKind::RuleAttached));
    }

    #[test]
    fn hover_explains_commitment_suffix() {
        let source = "func Pay$:\n    steps:\n        charge payment\n";
        let result = parse_lossless(source);
        let offset = source.find('$').unwrap() as u32;
        let hover = hover_at(&result.document, offset).expect("hover");
        assert!(hover.body.contains("commit_ready=true"));
        assert_eq!(hover.commitment, Some(Commitment::Locked));
    }

    #[test]
    fn code_actions_use_transition_validator() {
        let source = "func Pay$:\n    steps:\n        charge payment\n";
        let result = parse_lossless(source);
        let func = result
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == crate::lossless::SourceNodeKind::Func)
            .unwrap()
            .id;
        let actions = code_actions_for_node(&result.document, func);
        assert!(actions.iter().any(|action| {
            action.kind == CodeActionKind::ChallengeOrdinaryLock && action.allowed
        }));
        assert!(actions
            .iter()
            .any(|action| { action.kind == CodeActionKind::AcceptStrongLock && action.allowed }));
        // AI cannot unlock.
        assert!(!actions
            .iter()
            .any(|action| { action.kind == CodeActionKind::MarkContentReview && action.allowed }));
    }

    #[test]
    fn ide_snapshot_includes_queues() {
        let source = "desc?? \"app\"\nfunc Pay$?: ...\n";
        let result = parse_lossless(source);
        let snapshot = ide_snapshot(&result.document, &result.errors);
        assert!(!snapshot.delegation_queue.is_empty() || !snapshot.decision_queue.is_empty());
        assert!(!snapshot.semantic_tokens.is_empty());
    }
}
