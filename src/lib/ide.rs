use serde::Serialize;

use crate::ast::Commitment;
use crate::collaboration::{validate_transition, Actor, TransitionEffects, TransitionRequest};
use crate::diagnostics::{analyze_document, DocumentDiagnostics, QueueItem, QueueTree};
use crate::error::ParseError;
use crate::lossless::{
    ByteSpan, ColumnEncoding, CommitmentAnchorKind, CommitmentSlotSyntax, LosslessDocument,
    SourceNodeId, SourcePosition,
};

/// Semantic token kinds for commitment and rule attachment highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTokenKind {
    ContextDesc,
    ContextClause,
    ContextModule,
    ContextType,
    ContextFlow,
    ContextFunc,
    ContextUi,
    ContextSteps,
    ContextField,
    ContextFlowEntry,
    ContextFlowArm,
    ContextStep,
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
    pub slot: Option<crate::lossless::CommitmentSlotId>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_tree: Option<QueueTree>,
    pub diagnostics: DocumentDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationKind {
    RuleAttachmentTarget,
    AttachedRule,
    FlowSource,
    FlowEvent,
    FlowTransition,
    FlowTarget,
    FlowGuard,
    FlowTargetDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NavigationTarget {
    pub kind: NavigationKind,
    pub span: ByteSpan,
    pub node: Option<SourceNodeId>,
    pub label: String,
}

/// Build a library-level IDE snapshot from a lossless document.
pub fn ide_snapshot(document: &LosslessDocument, errors: &[ParseError]) -> IdeSnapshot {
    let diagnostics = analyze_document(document, errors);
    IdeSnapshot {
        semantic_tokens: semantic_tokens(document),
        decision_queue: diagnostics.decision_queue.clone(),
        delegation_queue: diagnostics.delegation_queue.clone(),
        queue_tree: Some(diagnostics.queue_tree.clone()),
        diagnostics,
    }
}

pub fn semantic_tokens(document: &LosslessDocument) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();
    for node in document.nodes() {
        let kind = match node.kind {
            crate::lossless::SourceNodeKind::Desc => SemanticTokenKind::ContextDesc,
            crate::lossless::SourceNodeKind::Clause => SemanticTokenKind::ContextClause,
            crate::lossless::SourceNodeKind::Module => SemanticTokenKind::ContextModule,
            crate::lossless::SourceNodeKind::TypeDef => SemanticTokenKind::ContextType,
            crate::lossless::SourceNodeKind::Flow => SemanticTokenKind::ContextFlow,
            crate::lossless::SourceNodeKind::Func => SemanticTokenKind::ContextFunc,
            crate::lossless::SourceNodeKind::Ui => SemanticTokenKind::ContextUi,
            crate::lossless::SourceNodeKind::Steps => SemanticTokenKind::ContextSteps,
            crate::lossless::SourceNodeKind::Field => SemanticTokenKind::ContextField,
            crate::lossless::SourceNodeKind::FlowEntry => SemanticTokenKind::ContextFlowEntry,
            crate::lossless::SourceNodeKind::FlowArm => SemanticTokenKind::ContextFlowArm,
            crate::lossless::SourceNodeKind::Step => SemanticTokenKind::ContextStep,
            crate::lossless::SourceNodeKind::Rule
            | crate::lossless::SourceNodeKind::Expr
            | crate::lossless::SourceNodeKind::UiNode
            | crate::lossless::SourceNodeKind::Placeholder
            | crate::lossless::SourceNodeKind::Math => continue,
        };
        let header = document.text(node.spans.header).unwrap_or("");
        let raw = header
            .split(|character: char| character.is_whitespace() || character == ':')
            .next()
            .unwrap_or("");
        let base = raw.trim_end_matches('?').trim_end_matches('$');
        let token_span = if base.is_empty() {
            node.spans.header
        } else {
            ByteSpan::new(
                node.spans.header.start as usize,
                node.spans.header.start as usize + base.len(),
            )
        };
        tokens.push(SemanticToken {
            span: token_span,
            kind,
            label: base.to_string(),
        });
    }
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

/// Return target-neutral navigation edges at one byte offset.
pub fn navigation_at(document: &LosslessDocument, offset: u32) -> Vec<NavigationTarget> {
    let mut targets = Vec::new();

    if let Some(rule) = document
        .rules()
        .iter()
        .find(|rule| contains(rule.span, offset))
    {
        if let Some(target) = rule.target.and_then(|id| document.node(id)) {
            targets.push(NavigationTarget {
                kind: NavigationKind::RuleAttachmentTarget,
                span: target.spans.header,
                node: Some(target.id),
                label: document.text(target.spans.header).unwrap_or("").to_string(),
            });
        }
        return targets;
    }

    if let Some(node) = most_specific_node(document, offset) {
        for rule in document
            .rules()
            .iter()
            .filter(|rule| rule.target == Some(node.id))
        {
            targets.push(NavigationTarget {
                kind: NavigationKind::AttachedRule,
                span: rule.span,
                node: Some(node.id),
                label: document.text(rule.span).unwrap_or("rule").to_string(),
            });
        }
    }

    let Some(arm) = document
        .nodes()
        .iter()
        .filter(|node| node.kind == crate::lossless::SourceNodeKind::FlowArm)
        .filter(|node| contains_or_end(node.spans.core, offset))
        .min_by_key(|node| node.spans.core.len())
    else {
        return targets;
    };

    if let Some(source) = document
        .nodes()
        .iter()
        .filter(|node| node.kind == crate::lossless::SourceNodeKind::FlowEntry)
        .filter(|node| {
            node.spans.core.start <= arm.spans.core.start
                && node.spans.core.end >= arm.spans.core.end
        })
        .min_by_key(|node| node.spans.core.len())
    {
        targets.push(NavigationTarget {
            kind: NavigationKind::FlowSource,
            span: source.spans.header,
            node: Some(source.id),
            label: document.text(source.spans.header).unwrap_or("").to_string(),
        });
    }

    for slot in document
        .commitment_slots()
        .iter()
        .filter(|slot| slot.owner == Some(arm.id) && slot.semantic_slot)
    {
        let kind = match slot.footprint {
            crate::lossless::CommitmentFootprintKind::Event => NavigationKind::FlowEvent,
            crate::lossless::CommitmentFootprintKind::Transition => NavigationKind::FlowTransition,
            crate::lossless::CommitmentFootprintKind::NameOrReference
                if is_flow_target_slot(document, arm, slot.anchor_span) =>
            {
                NavigationKind::FlowTarget
            }
            _ => continue,
        };
        targets.push(NavigationTarget {
            kind,
            span: slot.full_span,
            node: Some(arm.id),
            label: format!(
                "{}{}",
                document.text(slot.anchor_span).unwrap_or(""),
                document.text(slot.suffix_span).unwrap_or("")
            ),
        });
        if kind == NavigationKind::FlowTarget {
            let name = document.text(slot.anchor_span).unwrap_or("");
            if let Some(definition) = find_flow_state_definition(document, arm, name) {
                targets.push(NavigationTarget {
                    kind: NavigationKind::FlowTargetDefinition,
                    span: definition.spans.header,
                    node: Some(definition.id),
                    label: document
                        .text(definition.spans.header)
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
    }

    for clause in document
        .nodes()
        .iter()
        .filter(|node| node.kind == crate::lossless::SourceNodeKind::Clause)
        .filter(|node| {
            node.spans.core.start >= arm.spans.core.start
                && node.spans.core.end <= arm.spans.core.end
        })
    {
        targets.push(NavigationTarget {
            kind: NavigationKind::FlowGuard,
            span: clause.spans.core,
            node: Some(clause.id),
            label: document.text(clause.spans.core).unwrap_or("").to_string(),
        });
    }

    targets.sort_by_key(|target| (target.span.start, target.span.end));
    targets.dedup_by_key(|target| (target.kind, target.span.start, target.span.end));
    targets
}

pub fn navigation_at_position(
    document: &LosslessDocument,
    position: SourcePosition,
    encoding: ColumnEncoding,
) -> Vec<NavigationTarget> {
    document
        .line_index()
        .offset(document.source(), position, encoding)
        .map_or_else(Vec::new, |offset| navigation_at(document, offset))
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

/// Suggest actor-aware code actions for the node's entity-kind slot.
pub fn code_actions_for_node(document: &LosslessDocument, node: SourceNodeId) -> Vec<CodeAction> {
    if document.node(node).is_none() {
        return Vec::new();
    }
    let semantic_slots = crate::collaboration::collect_semantic_slot_snapshots(document);
    let entity_slot = semantic_slots.into_iter().find(|slot| {
        slot.node == node && slot.footprint == crate::lossless::CommitmentFootprintKind::EntityKind
    });
    let slot = entity_slot.as_ref().map(|slot| slot.slot);
    let from = entity_slot.map_or(Commitment::None, |slot| slot.state);
    code_actions_for_state(node, slot, from)
}

/// Suggest actor-aware actions for one exact parser-proven suffix slot.
pub fn code_actions_for_slot(
    document: &LosslessDocument,
    slot: crate::lossless::CommitmentSlotId,
) -> Vec<CodeAction> {
    let Some(snapshot) = crate::collaboration::collect_semantic_slot_snapshots(document)
        .into_iter()
        .find(|snapshot| snapshot.slot == slot)
    else {
        return Vec::new();
    };
    code_actions_for_state(snapshot.node, Some(slot), snapshot.state)
}

fn code_actions_for_state(
    node: SourceNodeId,
    slot: Option<crate::lossless::CommitmentSlotId>,
    from: Commitment,
) -> Vec<CodeAction> {
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
            slot,
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
        slot,
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
    let (effective, inherited_from) = effective_protection(document, slot);
    HoverInfo {
        span: slot.full_span,
        title: format!("Commitment {suffix}"),
        body: format!(
            "anchor=`{anchor}`\nslot={slot_kind}\nfootprint={:?}\nowner={:?}\nstate={}\nlock={:?}\neffective_lock={:?}\ninherited_from={}\nreview={:?}\ntarget={:?}\nconfirmed={}\ncommit_ready={}",
            slot.footprint,
            slot.owner,
            slot.value,
            slot.value.lock_intent(),
            effective,
            inherited_from,
            slot.value.review_intent(),
            slot.value.review_target(),
            slot.value.is_confirmed(),
            slot.value.is_commit_ready()
        ),
        commitment: Some(slot.value),
    }
}

fn effective_protection(
    document: &LosslessDocument,
    slot: &CommitmentSlotSyntax,
) -> (crate::ast::LockIntent, String) {
    let own = slot.value.lock_intent();
    let Some(owner) = slot.owner.and_then(|id| document.node(id)) else {
        return (own, "none".into());
    };
    let mut effective = own;
    let mut inherited = "none".to_string();
    for candidate in document.commitment_slots().iter().filter(|candidate| {
        candidate.semantic_slot
            && candidate.id != slot.id
            && candidate.value.protects_content()
            && candidate
                .owner
                .and_then(|id| document.node(id))
                .is_some_and(|node| {
                    node.spans.core.start <= owner.spans.core.start
                        && node.spans.core.end >= owner.spans.core.end
                        && node.spans.core != owner.spans.core
                })
    }) {
        let lock = candidate.value.lock_intent();
        if lock == crate::ast::LockIntent::StrongLocked
            || (lock == crate::ast::LockIntent::Locked && effective == crate::ast::LockIntent::Open)
        {
            effective = lock;
            inherited = candidate
                .owner
                .and_then(|id| document.node(id))
                .and_then(|node| document.text(node.spans.header))
                .unwrap_or("ancestor")
                .trim()
                .to_string();
        }
    }
    (effective, inherited)
}

fn most_specific_node(
    document: &LosslessDocument,
    offset: u32,
) -> Option<&crate::lossless::SourceNode> {
    document
        .nodes()
        .iter()
        .filter(|node| contains_or_end(node.spans.core, offset))
        .min_by_key(|node| node.spans.core.len())
}

fn contains_or_end(span: ByteSpan, offset: u32) -> bool {
    span.start <= offset && offset <= span.end
}

fn is_flow_target_slot(
    document: &LosslessDocument,
    arm: &crate::lossless::SourceNode,
    anchor: ByteSpan,
) -> bool {
    let header = document.text(arm.spans.header).unwrap_or("");
    header
        .find(">>>")
        .is_some_and(|index| anchor.start as usize >= arm.spans.header.start as usize + index + 3)
}

fn find_flow_state_definition<'a>(
    document: &'a LosslessDocument,
    arm: &crate::lossless::SourceNode,
    name: &str,
) -> Option<&'a crate::lossless::SourceNode> {
    let flow = document
        .nodes()
        .iter()
        .filter(|node| node.kind == crate::lossless::SourceNodeKind::Flow)
        .filter(|node| {
            node.spans.core.start <= arm.spans.core.start
                && node.spans.core.end >= arm.spans.core.end
        })
        .min_by_key(|node| node.spans.core.len())?;
    document.nodes().iter().find(|node| {
        node.kind == crate::lossless::SourceNodeKind::FlowEntry
            && node.spans.core.start >= flow.spans.core.start
            && node.spans.core.end <= flow.spans.core.end
            && document
                .text(node.spans.header)
                .and_then(|header| header.split_whitespace().next())
                .is_some_and(|token| token.trim_end_matches(':') == name)
    })
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
    fn hover_reports_inherited_strong_protection() {
        let source = "module$$ App:\n    func Build?: ...\n";
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let offset = source.find("Build?").unwrap() as u32;
        let hover = hover_at(&result.document, offset).expect("hover");
        assert!(hover.body.contains("effective_lock=StrongLocked"));
        assert!(hover.body.contains("inherited_from=module$$ App:"));
    }

    #[test]
    fn navigation_links_rules_and_flow_slots() {
        let source = r#"rule "audit"
flow Checkout:
    Pending:
        on Capture >>> Paid: requires: payment.ready
    Paid:
        on Refund >>> Refunded:
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let rule_offset = source.find("rule").unwrap() as u32;
        assert!(navigation_at(&result.document, rule_offset)
            .iter()
            .any(|target| target.kind == NavigationKind::RuleAttachmentTarget));

        let arm_offset = source.find("Capture").unwrap() as u32;
        let navigation = navigation_at(&result.document, arm_offset);
        assert!(navigation
            .iter()
            .any(|target| target.kind == NavigationKind::FlowEvent));
        assert!(navigation
            .iter()
            .any(|target| target.kind == NavigationKind::FlowTargetDefinition));
        assert!(navigation
            .iter()
            .any(|target| target.kind == NavigationKind::FlowGuard));
    }

    #[test]
    fn code_actions_use_transition_validator() {
        let source = "func$ Pay:\n    steps:\n        charge payment\n";
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
    fn code_actions_target_independent_slots_on_the_same_header() {
        let result = parse_lossless("func?? Pay$: ...\n");
        let slots = crate::collaboration::collect_semantic_slot_snapshots(&result.document);
        let keyword = slots.iter().find(|slot| slot.anchor == "func").unwrap();
        let name = slots.iter().find(|slot| slot.anchor == "Pay").unwrap();
        assert_eq!(keyword.state, Commitment::QuestionQuestion);
        assert_eq!(name.state, Commitment::Locked);

        let keyword_actions = code_actions_for_slot(&result.document, keyword.slot);
        assert!(keyword_actions
            .iter()
            .any(|action| { action.kind == CodeActionKind::MarkContentReview && action.allowed }));
        let name_actions = code_actions_for_slot(&result.document, name.slot);
        assert!(name_actions.iter().any(|action| {
            action.kind == CodeActionKind::ChallengeOrdinaryLock && action.allowed
        }));
        assert!(name_actions
            .iter()
            .all(|action| action.slot == Some(name.slot)));
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
