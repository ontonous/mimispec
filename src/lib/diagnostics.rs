use serde::Serialize;

use std::collections::{HashMap, HashSet};

use crate::ast::{Atom, Commitment, EventName, Fragment, ReviewIntent, ReviewTarget, Step};
use crate::collaboration::collect_semantic_slot_snapshots;
use crate::error::ParseError;
use crate::lossless::{
    ByteSpan, CommitmentFootprintKind, CommitmentSlotId, LosslessDocument, RuleAttachment,
    SourceNodeId, SourceNodeKind,
};

/// Frozen CLI collaboration-report envelope version for the 0.3 line.
pub const COLLABORATION_REPORT_SCHEMA_VERSION: &str = "mimispec.collaboration/0.3";

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

/// Explicit source edit for conservative syntax recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyntaxQuickFix {
    pub title: String,
    pub span: ByteSpan,
    pub replacement: String,
}

/// Hierarchical queue view derived from the compatible flat queues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueTree {
    pub root: QueueScopeNode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueScopeNode {
    pub scope_path: Vec<String>,
    pub header: String,
    pub node: Option<SourceNodeId>,
    pub span: Option<ByteSpan>,
    pub decision_count: usize,
    pub delegation_count: usize,
    pub children: Vec<QueueScopeNode>,
    pub items: Vec<QueueItem>,
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
    pub queue_tree: QueueTree,
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
    if errors.is_empty() {
        diagnostics.extend(detect_intent_gaps(document));
    }

    let queue_tree = build_queue_tree(document, &decision_queue, &delegation_queue);

    DocumentDiagnostics {
        summary,
        decision_queue,
        delegation_queue,
        queue_tree,
        diagnostics,
    }
}

/// Return only semantics-preserving, line-local recovery edits.
///
/// Lines containing transitions, assignments, or a structural colon receive
/// parser guidance but no automatic edit because quoting them could change
/// control-flow meaning.
pub fn syntax_quick_fixes(source: &str, errors: &[ParseError]) -> Vec<SyntaxQuickFix> {
    let error_lines = errors
        .iter()
        .filter(|error| {
            error.code == crate::error::ErrorCode::E0010
                && error.help.as_deref().is_some_and(|help| {
                    help.contains("quote the whole action label")
                        && help.contains("natural language")
                })
                && error
                    .suggestion
                    .as_deref()
                    .is_some_and(|suggestion| suggestion.starts_with("desc \""))
        })
        .map(|error| error.line)
        .collect::<HashSet<_>>();
    let mut fixes = Vec::new();
    for (index, (offset, line)) in physical_lines(source).into_iter().enumerate() {
        let line_number = index + 1;
        if !error_lines.contains(&line_number) {
            continue;
        }
        let leading = line.len() - line.trim_start().len();
        let content = line.trim();
        let unsafe_structure = content.is_empty()
            || content.contains(">>>")
            || content.contains('=')
            || content.contains(':');
        if unsafe_structure {
            continue;
        }
        let escaped = escape_mimispec_string(content);
        let span = ByteSpan::new(offset + leading, offset + line.trim_end().len());
        fixes.push(SyntaxQuickFix {
            title: "Quote the whole action label".into(),
            span,
            replacement: format!("\"{escaped}\""),
        });
        fixes.push(SyntaxQuickFix {
            title: "Convert to a natural-language desc step".into(),
            span,
            replacement: format!("desc \"{escaped}\""),
        });
    }
    fixes
}

fn physical_lines(source: &str) -> Vec<(usize, &str)> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        let mut end = start;
        while end < bytes.len() && !matches!(bytes[end], b'\r' | b'\n') {
            end += 1;
        }
        lines.push((start, &source[start..end]));
        if end == bytes.len() {
            break;
        }
        start = if bytes[end] == b'\r' && bytes.get(end + 1) == Some(&b'\n') {
            end + 2
        } else {
            end + 1
        };
    }
    lines
}

fn escape_mimispec_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn build_queue_tree(
    document: &LosslessDocument,
    decisions: &[QueueItem],
    delegations: &[QueueItem],
) -> QueueTree {
    let mut items_by_scope = HashMap::<Option<SourceNodeId>, Vec<QueueItem>>::new();
    for item in decisions.iter().chain(delegations) {
        let scope = document
            .scope_path(item.node)
            .and_then(|path| path.last().copied());
        items_by_scope.entry(scope).or_default().push(item.clone());
    }
    for items in items_by_scope.values_mut() {
        items.sort_by_key(|item| (item.span.start, item.span.end, item.slot.0));
    }

    let mut scope_children = HashMap::<Option<SourceNodeId>, Vec<SourceNodeId>>::new();
    for node in document
        .nodes()
        .iter()
        .filter(|node| node.kind.is_scope_container())
    {
        let path = document.scope_path(node.id).unwrap_or_default();
        let parent = path.iter().rev().nth(1).copied();
        scope_children.entry(parent).or_default().push(node.id);
    }
    for children in scope_children.values_mut() {
        children.sort_by_key(|id| {
            let node = document.node(*id).expect("indexed scope node");
            (node.spans.core.start, node.spans.core.end, node.id.0)
        });
    }

    let root = build_queue_scope_node(document, None, &scope_children, &items_by_scope)
        .expect("document queue root is always retained");
    QueueTree { root }
}

fn build_queue_scope_node(
    document: &LosslessDocument,
    scope: Option<SourceNodeId>,
    scope_children: &HashMap<Option<SourceNodeId>, Vec<SourceNodeId>>,
    items_by_scope: &HashMap<Option<SourceNodeId>, Vec<QueueItem>>,
) -> Option<QueueScopeNode> {
    let mut children = scope_children
        .get(&scope)
        .into_iter()
        .flatten()
        .filter_map(|child| {
            build_queue_scope_node(document, Some(*child), scope_children, items_by_scope)
        })
        .collect::<Vec<_>>();
    children.sort_by_key(|child| child.span.map_or(0, |span| span.start));
    let items = items_by_scope.get(&scope).cloned().unwrap_or_default();
    let own_decisions = items
        .iter()
        .filter(|item| item.state.review_intent() == ReviewIntent::Review)
        .count();
    let own_delegations = items.len() - own_decisions;
    let decision_count = own_decisions
        + children
            .iter()
            .map(|child| child.decision_count)
            .sum::<usize>();
    let delegation_count = own_delegations
        + children
            .iter()
            .map(|child| child.delegation_count)
            .sum::<usize>();
    if scope.is_some() && decision_count == 0 && delegation_count == 0 {
        return None;
    }

    let (header, span, scope_path) = if let Some(id) = scope {
        let node = document.node(id)?;
        let path = document
            .scope_path(id)
            .unwrap_or_default()
            .iter()
            .filter_map(|part| document.node(*part))
            .map(|part| {
                document
                    .text(part.spans.header)
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .collect();
        (
            document
                .text(node.spans.header)
                .unwrap_or_default()
                .trim()
                .to_string(),
            Some(node.spans.header),
            path,
        )
    } else {
        ("<document>".into(), None, Vec::new())
    };
    Some(QueueScopeNode {
        scope_path,
        header,
        node: scope,
        span,
        decision_count,
        delegation_count,
        children,
        items,
    })
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
    let mut index = SemanticNodeIndex::new(document);
    let mut out = Vec::new();
    visit_context_items(
        &document.semantic().fragments,
        document,
        &mut index,
        &mut out,
    );
    out
}

/// Source-order semantic/source correspondence. It intentionally avoids names:
/// repeated names and anonymous flows are matched by parser-recorded kind/order.
struct SemanticNodeIndex {
    nodes: HashMap<SourceNodeKind, Vec<SourceNodeId>>,
    cursor: HashMap<SourceNodeKind, usize>,
}

impl SemanticNodeIndex {
    fn new(document: &LosslessDocument) -> Self {
        let mut nodes = HashMap::<SourceNodeKind, Vec<SourceNodeId>>::new();
        for kind in [
            SourceNodeKind::Module,
            SourceNodeKind::TypeDef,
            SourceNodeKind::Flow,
            SourceNodeKind::Func,
            SourceNodeKind::Steps,
        ] {
            nodes.insert(
                kind,
                document.nodes_of_kind(kind).map(|node| node.id).collect(),
            );
        }
        Self {
            nodes,
            cursor: HashMap::new(),
        }
    }

    fn take(&mut self, kind: SourceNodeKind) -> Option<SourceNodeId> {
        let cursor = self.cursor.entry(kind).or_default();
        let result = self
            .nodes
            .get(&kind)
            .and_then(|nodes| nodes.get(*cursor))
            .copied();
        *cursor += usize::from(result.is_some());
        result
    }
}

fn visit_context_items(
    items: &[Fragment],
    document: &LosslessDocument,
    index: &mut SemanticNodeIndex,
    out: &mut Vec<IntentDiagnostic>,
) {
    for fragment in items {
        match fragment {
            Fragment::Module { module } => {
                let _ = index.take(SourceNodeKind::Module);
                visit_context_items(&module.items, document, index, out);
            }
            Fragment::Flow { flow } => {
                let node = index.take(SourceNodeKind::Flow);
                detect_flow_gap(flow, node, document, out);
                visit_context_items(&flow.items, document, index, out);
            }
            Fragment::Func { func } => {
                let node = index.take(SourceNodeKind::Func);
                detect_func_gap(func, node, document, out);
                visit_context_items(&func.items, document, index, out);
            }
            Fragment::TypeDef { typedef } => {
                let _ = index.take(SourceNodeKind::TypeDef);
                visit_context_items(typedef.items(), document, index, out);
            }
            Fragment::Steps { items, .. } => {
                let _ = index.take(SourceNodeKind::Steps);
                visit_context_items(items, document, index, out);
            }
            Fragment::FlowEntry { entry } => {
                visit_context_items(&entry.items, document, index, out)
            }
            Fragment::FlowArm { arm } => visit_context_items(&arm.items, document, index, out),
            _ => {}
        }
    }
}

fn detect_flow_gap(
    flow: &crate::ast::FlowDef,
    node: Option<SourceNodeId>,
    document: &LosslessDocument,
    out: &mut Vec<IntentDiagnostic>,
) {
    let entries = flow.entries();
    if entries.is_empty() {
        return;
    }
    let has_failure_hint = entries.iter().any(|entry| {
        text_has_failure_hint(&entry.state.name)
            || entry.arms().into_iter().any(|arm| {
                text_has_failure_hint(&arm.to.name)
                    || arm.event.as_ref().is_some_and(|event| match &event.name {
                        EventName::Ident { value } => text_has_failure_hint(&value.name),
                        EventName::Natural { text } => text_has_failure_hint(&text.value),
                    })
            })
    });
    if has_failure_hint {
        return;
    }
    let Some(node) = node.and_then(|id| document.node(id)) else {
        return;
    };
    let flow_name = flow.name.as_ref().map_or("<anonymous>", |name| &name.name);
    out.push(IntentDiagnostic {
        code: DiagnosticCode("H-INTENT-GAP"),
        class: DiagnosticClass::IntentGap,
        severity: Severity::Hint,
        message: format!("Flow `{flow_name}` has no obvious failure/cancel path"),
        span: Some(node.spans.header),
        help: Some(
            "Consider describing Fault/Error/Cancel arms so recovery intent is explicit.".into(),
        ),
        related_nodes: vec![node.id],
    });
}

fn detect_func_gap(
    func: &crate::ast::FuncDef,
    node: Option<SourceNodeId>,
    document: &LosslessDocument,
    out: &mut Vec<IntentDiagnostic>,
) {
    let steps = func.step_refs();
    if steps.is_empty() || !steps.iter().any(|step| step_has_boundary_action(step)) {
        return;
    }
    let has_error_step = steps.iter().any(|step| step_has_error(step));
    let node = node.and_then(|id| document.node(id));
    let has_failure_text = node.is_some_and(|node| {
        text_has_failure_hint(document.text(node.spans.core).unwrap_or_default())
    });
    if has_error_step || has_failure_text {
        return;
    }
    let Some(node) = node else {
        return;
    };
    out.push(IntentDiagnostic {
        code: DiagnosticCode("H-INTENT-GAP"),
        class: DiagnosticClass::IntentGap,
        severity: Severity::Hint,
        message: format!(
            "Boundary-facing Func `{}` does not mention an error path",
            func.name.name
        ),
        span: Some(node.spans.header),
        help: Some(
            "Add an `error` step or `on` compensation block if failure recovery matters.".into(),
        ),
        related_nodes: vec![node.id],
    });
}

fn text_has_failure_hint(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "fault",
        "error",
        "fail",
        "cancel",
        "reject",
        "abort",
        "timeout",
        "kick",
        "invalid",
        "unavailable",
        "success condition",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
        || [
            "失败",
            "错误",
            "取消",
            "拒绝",
            "终止",
            "超时",
            "踢出",
            "无效",
            "不可用",
            "成功条件",
        ]
        .iter()
        .any(|keyword| text.contains(keyword))
}

fn text_has_boundary_action(text: &str) -> bool {
    let lower = text.to_lowercase();
    let strong = [
        "connect", "send", "receive", "persist", "socket", "file", "external", "network", "listen",
        "bind", "upload", "download",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
        || [
            "网络",
            "连接",
            "发送",
            "接收",
            "持久化",
            "文件",
            "套接字",
            "外部",
            "监听",
            "上传",
            "下载",
        ]
        .iter()
        .any(|keyword| text.contains(keyword));
    let local = lower.contains("local") || text.contains("本地");
    let basic_io = ["read", "write", "save", "load"]
        .iter()
        .any(|keyword| lower.contains(keyword))
        || ["读取", "写入", "保存", "加载"]
            .iter()
            .any(|keyword| text.contains(keyword));
    strong || (basic_io && !local)
}

fn step_has_boundary_action(step: &Step) -> bool {
    match step {
        Step::Action { step } => {
            atoms_have_boundary_action(&step.label)
                || step
                    .desc
                    .as_ref()
                    .is_some_and(|desc| text_has_boundary_action(&desc.content.value))
                || step
                    .on_blocks
                    .iter()
                    .any(|block| step_items_have_boundary_action(&block.steps))
        }
        Step::Assign { step } => step
            .on_blocks
            .iter()
            .any(|block| step_items_have_boundary_action(&block.steps)),
        Step::If { step } => {
            step_items_have_boundary_action(&step.then_branch)
                || step
                    .else_branch
                    .as_ref()
                    .is_some_and(|items| step_items_have_boundary_action(items))
        }
        Step::For { step } => step_items_have_boundary_action(&step.body),
        Step::While { step } => step_items_have_boundary_action(&step.body),
        Step::Parasteps { step } => step_items_have_boundary_action(&step.steps),
        Step::Desc { content } => text_has_boundary_action(&content.content.value),
        Step::Error { .. } | Step::Placeholder { .. } => false,
    }
}

fn step_items_have_boundary_action(items: &[Fragment]) -> bool {
    items.iter().any(|item| match item {
        Fragment::Step { step } => step_has_boundary_action(step),
        Fragment::Steps { items, .. } => step_items_have_boundary_action(items),
        _ => false,
    })
}

fn atoms_have_boundary_action(atoms: &[Atom]) -> bool {
    let mut text = String::new();
    append_atom_text(atoms, &mut text);
    text_has_boundary_action(&text)
}

fn append_atom_text(atoms: &[Atom], out: &mut String) {
    for atom in atoms {
        if !out.is_empty() {
            out.push(' ');
        }
        match atom {
            Atom::Ident { value } => out.push_str(&value.name),
            Atom::String { value } => out.push_str(&value.value),
            Atom::Number { value } | Atom::Symbol { value } => out.push_str(value),
            Atom::List { items } => {
                for item in items {
                    append_atom_text(item, out);
                }
            }
            Atom::Ellipsis { .. } => out.push_str("..."),
        }
    }
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

    #[test]
    fn kicked_transition_suppresses_missing_failure_hint() {
        let source = r#"flow Membership:
    Member:
        on Kicked >>> Outside:
    Outside:
        on Join >>> Member:
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let report = analyze_document(&result.document, &result.errors);
        assert!(
            !report.diagnostics.iter().any(|diagnostic| {
                diagnostic.class == DiagnosticClass::IntentGap
                    && diagnostic.message.contains("failure/cancel path")
            }),
            "forced removal is an explicit failure path: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn action_syntax_guidance_and_quick_fixes_are_conservative() {
        for action in [
            "error.visible",
            "operate on data",
            "non-empty",
            "bind and listen on",
        ] {
            let source = format!("func Work:\n    steps:\n        {action}\n");
            let parsed = parse_lossless(&source);
            assert!(
                !parsed.errors.is_empty(),
                "case unexpectedly parsed: {action}"
            );
            assert!(parsed.errors.iter().any(|error| {
                error.code == crate::error::ErrorCode::E0010
                    && error
                        .help
                        .as_deref()
                        .is_some_and(|help| help.contains("quote"))
                    && error
                        .suggestion
                        .as_deref()
                        .is_some_and(|fix| fix.starts_with("desc \""))
            }));
            let fixes = syntax_quick_fixes(&source, &parsed.errors);
            assert_eq!(fixes.len(), 2, "fixes for {action}: {fixes:?}");
            for fix in fixes {
                let mut candidate = source.clone();
                candidate.replace_range(fix.span.as_range(), &fix.replacement);
                let repaired = parse_lossless(&candidate);
                assert!(
                    repaired.errors.is_empty(),
                    "quick fix did not parse for {action}: {:?}\n{candidate}",
                    repaired.errors
                );
                let rendered = crate::render::render_file(repaired.document.semantic());
                assert!(crate::parse(&rendered).errors.is_empty(), "{rendered}");
            }
        }

        let structured = r#"func Work:
    steps:
        connect server
        on timeout:
            error "connection failed"
"#;
        let parsed = parse_lossless(structured);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(syntax_quick_fixes(structured, &parsed.errors).is_empty());

        let unsafe_line = "func Work:\n    steps:\n        non-empty >>> Ready\n";
        let parsed = parse_lossless(unsafe_line);
        assert!(!parsed.errors.is_empty());
        assert!(syntax_quick_fixes(unsafe_line, &parsed.errors).is_empty());

        let structural = "func Broken(\n";
        let parsed = parse_lossless(structural);
        assert!(parsed
            .errors
            .iter()
            .any(|error| error.code == crate::error::ErrorCode::E0010));
        assert!(syntax_quick_fixes(structural, &parsed.errors).is_empty());

        let lone_cr = "func Work:\r    steps:\r        non-empty\r";
        let parsed = parse_lossless(lone_cr);
        let fixes = syntax_quick_fixes(lone_cr, &parsed.errors);
        assert_eq!(fixes.len(), 2, "{:?}", parsed.errors);
        assert!(fixes.iter().all(|fix| fix.span.start > 20));
    }

    #[test]
    fn queue_tree_contains_every_real_project_slot_once_in_source_order() {
        let source = include_str!("../../docs/corpora/mimichat-real-project.mms");
        let parsed = parse_lossless(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let report = analyze_document(&parsed.document, &parsed.errors);
        assert_eq!(report.decision_queue.len(), 73);
        assert!(report.delegation_queue.is_empty());
        assert_eq!(report.queue_tree.root.decision_count, 73);
        assert_eq!(report.queue_tree.root.delegation_count, 0);

        fn collect(node: &QueueScopeNode, slots: &mut Vec<(u32, u32)>) {
            slots.extend(node.items.iter().map(|item| (item.span.start, item.slot.0)));
            for child in &node.children {
                collect(child, slots);
            }
        }
        let mut tree_slots = Vec::new();
        collect(&report.queue_tree.root, &mut tree_slots);
        assert_eq!(tree_slots.len(), 73);
        let unique = tree_slots
            .iter()
            .map(|(_, slot)| *slot)
            .collect::<HashSet<_>>();
        assert_eq!(unique.len(), 73);
        let flat = report
            .decision_queue
            .iter()
            .map(|item| item.slot.0)
            .collect::<HashSet<_>>();
        assert_eq!(unique, flat);
        assert!(report
            .queue_tree
            .root
            .children
            .iter()
            .any(|scope| scope.header.starts_with("module")));
    }

    #[test]
    fn nested_boundary_diagnostics_use_source_order_and_skip_partial_documents() {
        let source = r#"module First:
    func Sync:
        steps:
            read file

module Second:
    func Sync:
        steps:
            read file
            on timeout:
                error "read failed"
"#;
        let parsed = parse_lossless(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let report = analyze_document(&parsed.document, &parsed.errors);
        let gaps = report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.class == DiagnosticClass::IntentGap
                    && diagnostic.message.contains("Func `Sync`")
            })
            .collect::<Vec<_>>();
        assert_eq!(gaps.len(), 1, "{:?}", report.diagnostics);
        let first_header = parsed
            .document
            .text(
                parsed
                    .document
                    .node(gaps[0].related_nodes[0])
                    .unwrap()
                    .spans
                    .header,
            )
            .unwrap();
        assert_eq!(first_header.trim(), "func Sync:");
        assert!(parsed
            .document
            .scope_path(gaps[0].related_nodes[0])
            .is_some());

        let partial = parse_lossless("func Broken(:\n    steps:\n        read file\n");
        assert!(!partial.errors.is_empty());
        let report = analyze_document(&partial.document, &partial.errors);
        assert!(!report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.class == DiagnosticClass::IntentGap));
    }

    impl DocumentDiagnostics {
        fn errors_empty_for_test(&self, errors: &[ParseError]) -> bool {
            errors.is_empty()
        }
    }
}
