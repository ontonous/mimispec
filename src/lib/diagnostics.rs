use serde::Serialize;

use std::collections::HashMap;

use crate::ast::{Commitment, EventName, Fragment, ReviewIntent, ReviewTarget, Step};
use crate::collaboration::collect_semantic_slot_snapshots;
use crate::error::ParseError;
use crate::lossless::{
    ByteSpan, CommitmentFootprintKind, CommitmentSlotId, LosslessDocument, RuleAttachment,
    SourceNodeId,
};

/// Stable diagnostic class for intent-oriented guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticClass {
    Syntax,
    Attachment,
    Collaboration,
    Decision,
    Delegation,
    IntentConflict,
    IntentGap,
    TargetGap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticCode(pub &'static str);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntentDiagnostic {
    pub code: DiagnosticCode,
    pub class: DiagnosticClass,
    pub severity: Severity,
    pub message: String,
    pub span: Option<ByteSpan>,
    pub help: Option<String>,
    pub related_nodes: Vec<SourceNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueItem {
    pub slot: CommitmentSlotId,
    pub node: SourceNodeId,
    pub state: Commitment,
    pub anchor: String,
    pub footprint: CommitmentFootprintKind,
    pub header: String,
    pub span: ByteSpan,
    pub review_target: ReviewTarget,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CommitmentSummary {
    pub total_slots: usize,
    pub open: usize,
    pub content_review: usize,
    pub content_delegated: usize,
    pub locked: usize,
    pub lock_review: usize,
    pub lock_delegated: usize,
    pub strong_locked: usize,
    pub strong_lock_review: usize,
    pub strong_lock_delegated: usize,
    pub commit_ready: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentDiagnostics {
    pub summary: CommitmentSummary,
    pub decision_queue: Vec<QueueItem>,
    pub delegation_queue: Vec<QueueItem>,
    pub diagnostics: Vec<IntentDiagnostic>,
}

/// Build decision/delegation queues and first-wave intent diagnostics.
pub fn analyze_document(document: &LosslessDocument, errors: &[ParseError]) -> DocumentDiagnostics {
    let slots = collect_semantic_slot_snapshots(document);
    let mut summary = CommitmentSummary {
        total_slots: slots.len(),
        ..CommitmentSummary::default()
    };
    let mut decision_queue = Vec::new();
    let mut delegation_queue = Vec::new();
    let mut diagnostics = Vec::new();

    for error in errors {
        diagnostics.push(IntentDiagnostic {
            code: DiagnosticCode("E-SYNTAX"),
            class: DiagnosticClass::Syntax,
            severity: Severity::Error,
            message: error.to_string(),
            span: None,
            help: Some(
                "Fix the syntax error before collaboration transitions can be trusted.".into(),
            ),
            related_nodes: Vec::new(),
        });
    }

    for slot in &slots {
        match slot.state {
            Commitment::None => summary.open += 1,
            Commitment::Question => summary.content_review += 1,
            Commitment::QuestionQuestion => summary.content_delegated += 1,
            Commitment::Locked => summary.locked += 1,
            Commitment::LockedQuestion => summary.lock_review += 1,
            Commitment::LockedQuestionQuestion => summary.lock_delegated += 1,
            Commitment::StrongLocked => summary.strong_locked += 1,
            Commitment::StrongLockedQuestion => summary.strong_lock_review += 1,
            Commitment::StrongLockedQuestionQuestion => summary.strong_lock_delegated += 1,
        }
        if slot.state.is_commit_ready() {
            summary.commit_ready += 1;
        }

        let span = if slot.suffix_span.is_empty() {
            slot.anchor_span
        } else {
            ByteSpan {
                start: slot.anchor_span.start,
                end: slot.suffix_span.end,
            }
        };
        let item = QueueItem {
            slot: slot.slot,
            node: slot.node,
            state: slot.state,
            anchor: slot.anchor.clone(),
            footprint: slot.footprint,
            header: slot.header.clone(),
            span,
            review_target: slot.state.review_target(),
        };

        match slot.state.review_intent() {
            ReviewIntent::Review => {
                if !queue_contains(&decision_queue, &item) {
                    decision_queue.push(item.clone());
                    diagnostics.push(IntentDiagnostic {
                        code: DiagnosticCode("I-DECISION"),
                        class: DiagnosticClass::Decision,
                        severity: Severity::Info,
                        message: format!(
                            "Human decision needed for {:?} slot `{}` ({})",
                            slot.footprint, slot.anchor, slot.state
                        ),
                        span: Some(span),
                        help: Some(decision_help(slot.state).into()),
                        related_nodes: vec![slot.node],
                    });
                }
            }
            ReviewIntent::Delegate => {
                if !queue_contains(&delegation_queue, &item) {
                    delegation_queue.push(item.clone());
                    diagnostics.push(IntentDiagnostic {
                        code: DiagnosticCode("I-DELEGATION"),
                        class: DiagnosticClass::Delegation,
                        severity: Severity::Info,
                        message: format!(
                            "AI work delegated for {:?} slot `{}` ({})",
                            slot.footprint, slot.anchor, slot.state
                        ),
                        span: Some(span),
                        help: Some(delegation_help(slot.state).into()),
                        related_nodes: vec![slot.node],
                    });
                }
            }
            ReviewIntent::None => {}
        }
    }

    for rule in document.rules() {
        match rule.attachment {
            RuleAttachment::DroppedByRecovery => {
                diagnostics.push(IntentDiagnostic {
                    code: DiagnosticCode("W-ATTACHMENT-DROPPED"),
                    class: DiagnosticClass::Attachment,
                    severity: Severity::Warning,
                    message: "Rule was dropped during error recovery and is not attached".into(),
                    span: Some(rule.span),
                    help: Some(
                        "Repair surrounding syntax so the rule can attach to its intended target."
                            .into(),
                    ),
                    related_nodes: rule.target.into_iter().collect(),
                });
            }
            RuleAttachment::Pending => {
                diagnostics.push(IntentDiagnostic {
                    code: DiagnosticCode("W-ATTACHMENT-PENDING"),
                    class: DiagnosticClass::Attachment,
                    severity: Severity::Warning,
                    message: "Rule attachment is still pending".into(),
                    span: Some(rule.span),
                    help: Some(
                        "Complete the following entity or insert a blank line to make the rule environmental."
                            .into(),
                    ),
                    related_nodes: Vec::new(),
                });
            }
            RuleAttachment::Attached | RuleAttachment::Environment => {}
        }
    }

    diagnostics.extend(detect_rule_commitment_conflicts(document));
    diagnostics.extend(detect_intent_gaps(document));

    DocumentDiagnostics {
        summary,
        decision_queue,
        delegation_queue,
        diagnostics,
    }
}

fn detect_rule_commitment_conflicts(document: &LosslessDocument) -> Vec<IntentDiagnostic> {
    let mut by_text: HashMap<String, Vec<(ByteSpan, Commitment, Option<SourceNodeId>)>> =
        HashMap::new();
    for rule in document.rules() {
        let text = document
            .text(rule.span)
            .unwrap_or_default()
            .trim()
            .to_string();
        // Extract the quoted content after `rule` + optional suffix for comparison.
        let content = text
            .find('"')
            .map(|idx| text[idx..].to_string())
            .unwrap_or(text.clone());
        let commitment = parse_rule_commitment(&text);
        by_text
            .entry(content)
            .or_default()
            .push((rule.span, commitment, rule.target));
    }

    let mut out = Vec::new();
    for (content, occurrences) in by_text {
        if occurrences.len() < 2 {
            continue;
        }
        let mut states: Vec<Commitment> = occurrences.iter().map(|(_, c, _)| *c).collect();
        states.sort_by_key(|c| format!("{c:?}"));
        states.dedup();
        if states.len() < 2 {
            continue;
        }
        let related: Vec<SourceNodeId> = occurrences.iter().filter_map(|(_, _, t)| *t).collect();
        out.push(IntentDiagnostic {
            code: DiagnosticCode("W-INTENT-CONFLICT"),
            class: DiagnosticClass::IntentConflict,
            severity: Severity::Warning,
            message: format!(
                "Rule content {content} appears with conflicting commitment states: {}",
                states
                    .iter()
                    .map(|s| {
                        let text = s.to_string();
                        if text.is_empty() {
                            "none".into()
                        } else {
                            text
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            span: Some(occurrences[0].0),
            help: Some(
                "Keep one commitment policy for the same rule content, or rewrite the texts so they are intentionally distinct."
                    .into(),
            ),
            related_nodes: related,
        });
    }
    out
}

fn parse_rule_commitment(rule_text: &str) -> Commitment {
    let after_rule = rule_text
        .trim_start()
        .strip_prefix("rule")
        .unwrap_or(rule_text)
        .trim_start();
    let token = after_rule
        .split(|ch: char| ch.is_whitespace() || ch == '"')
        .next()
        .unwrap_or("");
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

fn detect_intent_gaps(document: &LosslessDocument) -> Vec<IntentDiagnostic> {
    let mut out = Vec::new();
    for fragment in &document.semantic().fragments {
        match fragment {
            Fragment::Flow { flow } => {
                let entries = flow.entries();
                if entries.is_empty() {
                    continue;
                }
                let has_failure_hint = entries.iter().any(|entry| {
                    text_has_failure_hint(&entry.state.name)
                        || entry.arms().into_iter().any(|arm| {
                            text_has_failure_hint(&arm.to.name)
                                || arm.event.as_ref().is_some_and(|event| match &event.name {
                                    EventName::Ident { value } => {
                                        text_has_failure_hint(&value.name)
                                    }
                                    EventName::Natural { text } => {
                                        text_has_failure_hint(&text.value)
                                    }
                                })
                        })
                });
                if !has_failure_hint {
                    let flow_name = flow.name.as_ref().map_or("<anonymous>", |name| &name.name);
                    if let Some(node) = document.nodes().iter().find(|node| {
                        node.kind == crate::lossless::SourceNodeKind::Flow
                            && document
                                .text(node.spans.header)
                                .is_some_and(|text| text.contains(flow_name))
                    }) {
                        out.push(IntentDiagnostic {
                            code: DiagnosticCode("H-INTENT-GAP"),
                            class: DiagnosticClass::IntentGap,
                            severity: Severity::Hint,
                            message: format!(
                                "Flow `{}` has no obvious failure/cancel path",
                                flow_name
                            ),
                            span: Some(node.spans.header),
                            help: Some(
                                "Consider describing Fault/Error/Cancel arms so recovery intent is explicit."
                                    .into(),
                            ),
                            related_nodes: vec![node.id],
                        });
                    }
                }
            }
            Fragment::Func { func } => {
                let steps = func.step_refs();
                if steps.is_empty() {
                    continue;
                }
                let has_error_step = steps.iter().any(|step| step_has_error(step));
                if !has_error_step {
                    if let Some(node) = document.nodes().iter().find(|node| {
                        node.kind == crate::lossless::SourceNodeKind::Func
                            && document
                                .text(node.spans.header)
                                .is_some_and(|text| text.contains(&func.name.name))
                    }) {
                        out.push(IntentDiagnostic {
                            code: DiagnosticCode("H-INTENT-GAP"),
                            class: DiagnosticClass::IntentGap,
                            severity: Severity::Hint,
                            message: format!(
                                "Func `{}` steps do not mention an error path",
                                func.name.name
                            ),
                            span: Some(node.spans.header),
                            help: Some(
                                "Add an `error` step or `on` compensation block if failure recovery matters."
                                    .into(),
                            ),
                            related_nodes: vec![node.id],
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn text_has_failure_hint(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "fault", "error", "fail", "cancel", "reject", "abort", "timeout",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
        || ["失败", "错误", "取消", "拒绝", "终止", "超时"]
            .iter()
            .any(|keyword| text.contains(keyword))
}

fn step_items_have_error(items: &[Fragment]) -> bool {
    items.iter().any(|item| match item {
        Fragment::Step { step } => step_has_error(step),
        Fragment::Steps { items, .. } => step_items_have_error(items),
        _ => false,
    })
}

fn step_has_error(step: &Step) -> bool {
    match step {
        Step::Error { .. } => true,
        Step::Action { step } => !step.on_blocks.is_empty(),
        Step::Assign { step } => !step.on_blocks.is_empty(),
        Step::If { step } => {
            step_items_have_error(&step.then_branch)
                || step
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| step_items_have_error(branch))
        }
        Step::For { step } => step_items_have_error(&step.body),
        Step::While { step } => step_items_have_error(&step.body),
        Step::Parasteps { step } => step_items_have_error(&step.steps),
        Step::Desc { .. } | Step::Placeholder { .. } => false,
    }
}

fn queue_contains(queue: &[QueueItem], item: &QueueItem) -> bool {
    queue.iter().any(|existing| existing.slot == item.slot)
}

fn decision_help(state: Commitment) -> &'static str {
    match state {
        Commitment::Question => "Review the content and either lock it, revise it, or keep asking.",
        Commitment::LockedQuestion => {
            "Content is protected. Decide whether the ordinary lock is mature."
        }
        Commitment::StrongLockedQuestion => {
            "Content is protected. Decide whether the strong lock is justified."
        }
        _ => "Human decision is required.",
    }
}

fn delegation_help(state: Commitment) -> &'static str {
    match state {
        Commitment::QuestionQuestion => "AI should draft or refine this unlocked content.",
        Commitment::LockedQuestionQuestion => {
            "AI should assess whether the ordinary lock is ready without editing protected content."
        }
        Commitment::StrongLockedQuestionQuestion => {
            "AI should assess strong-lock readiness without editing protected content."
        }
        _ => "AI work is delegated for this slot.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_lossless;

    #[test]
    fn builds_decision_and_delegation_queues() {
        let source = r#"desc?? "家庭记账应用"
rule "本地优先"
func Pay$?:
    steps:
        charge payment

type Status?: Active | Paid
"#;
        let result = parse_lossless(source);
        let report = analyze_document(&result.document, &result.errors);
        assert!(report.errors_empty_for_test(&result.errors));

        assert!(
            report.decision_queue.len() >= 2,
            "decision queue: {:?}",
            report.decision_queue
        );
        assert!(
            !report.delegation_queue.is_empty(),
            "delegation queue: {:?}",
            report.delegation_queue
        );
        assert!(report
            .decision_queue
            .iter()
            .any(|item| item.state == Commitment::LockedQuestion));
        assert!(report
            .decision_queue
            .iter()
            .any(|item| item.state == Commitment::Question));
        assert!(report
            .delegation_queue
            .iter()
            .any(|item| item.state == Commitment::QuestionQuestion));

        assert_eq!(report.summary.commit_ready, 0);
        assert!(report.summary.lock_review >= 1);
        assert!(report.summary.content_delegated >= 1);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.class == DiagnosticClass::Decision));
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.class == DiagnosticClass::Delegation));
    }

    #[test]
    fn reports_dropped_rule_attachment() {
        let source = r#"rule "protected"
func Broken(:
type Good: ...
"#;
        let result = parse_lossless(source);
        let report = analyze_document(&result.document, &result.errors);
        assert!(report.diagnostics.iter().any(|d| {
            d.class == DiagnosticClass::Attachment && d.code.0 == "W-ATTACHMENT-DROPPED"
        }));
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.class == DiagnosticClass::Syntax));
    }

    #[test]
    fn detects_rule_commitment_conflict_and_flow_gap() {
        let source = r#"rule$ "支付必须幂等"
func Pay:
    steps:
        charge payment

rule "支付必须幂等"
func Refund:
    steps:
        refund payment

flow Checkout:
    Pending >>> Paid:
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let report = analyze_document(&result.document, &result.errors);
        assert!(report.diagnostics.iter().any(|d| {
            d.class == DiagnosticClass::IntentConflict && d.code.0 == "W-INTENT-CONFLICT"
        }));
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.class == DiagnosticClass::IntentGap && d.code.0 == "H-INTENT-GAP"));
    }

    #[test]
    fn flow_failure_event_suppresses_missing_failure_hint() {
        let source = r#"flow Sync:
    Uploading:
        on UploadFailed >>> RetryPending:
    RetryPending:
        on RetryDue >>> Uploading:
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let report = analyze_document(&result.document, &result.errors);
        assert!(
            !report.diagnostics.iter().any(|diagnostic| {
                diagnostic.class == DiagnosticClass::IntentGap
                    && diagnostic.message.contains("failure/cancel path")
            }),
            "failure-labelled event must count as an explicit failure path: {:?}",
            report.diagnostics
        );
    }

    impl DocumentDiagnostics {
        fn errors_empty_for_test(&self, errors: &[ParseError]) -> bool {
            errors.is_empty()
        }
    }
}
