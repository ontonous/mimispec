use serde::Serialize;

use crate::ast::{Commitment, ReviewIntent, ReviewTarget};
use crate::collaboration::collect_slot_snapshots;
use crate::error::ParseError;
use crate::lossless::{ByteSpan, LosslessDocument, RuleAttachment, SourceNodeId};

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
    pub node: SourceNodeId,
    pub state: Commitment,
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
    let slots = collect_slot_snapshots(document);
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

        let node = document.node(slot.node);
        let span = node.map(|n| n.spans.header).unwrap_or(ByteSpan::new(0, 0));
        let item = QueueItem {
            node: slot.node,
            state: slot.state,
            header: slot.header.clone(),
            span,
            review_target: slot.state.review_target(),
        };

        match slot.state.review_intent() {
            ReviewIntent::Review => {
                decision_queue.push(item);
                diagnostics.push(IntentDiagnostic {
                    code: DiagnosticCode("I-DECISION"),
                    class: DiagnosticClass::Decision,
                    severity: Severity::Info,
                    message: format!(
                        "Human decision needed for {} ({})",
                        slot.header.trim(),
                        slot.state
                    ),
                    span: Some(span),
                    help: Some(decision_help(slot.state).into()),
                    related_nodes: vec![slot.node],
                });
            }
            ReviewIntent::Delegate => {
                delegation_queue.push(item);
                diagnostics.push(IntentDiagnostic {
                    code: DiagnosticCode("I-DELEGATION"),
                    class: DiagnosticClass::Delegation,
                    severity: Severity::Info,
                    message: format!(
                        "AI work delegated for {} ({})",
                        slot.header.trim(),
                        slot.state
                    ),
                    span: Some(span),
                    help: Some(delegation_help(slot.state).into()),
                    related_nodes: vec![slot.node],
                });
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

    DocumentDiagnostics {
        summary,
        decision_queue,
        delegation_queue,
        diagnostics,
    }
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

    impl DocumentDiagnostics {
        fn errors_empty_for_test(&self, errors: &[ParseError]) -> bool {
            errors.is_empty()
        }
    }
}
