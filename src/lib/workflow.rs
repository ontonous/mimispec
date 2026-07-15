use serde::Serialize;

use crate::ast::Commitment;
use crate::collaboration::{
    challenge_is_duplicate, collect_slot_snapshots, ContentHash, LockChallenge,
};
use crate::diagnostics::{analyze_document, QueueItem};
use crate::lossless::LosslessDocument;
use crate::materialize::{plan_materialization, MaterializationPlan};

/// Work item kinds OSE can schedule from slot states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowTaskKind {
    Decision,
    Delegation,
    LockChallenge,
    Materialization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowActor {
    Human,
    Ai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowTask {
    pub kind: WorkflowTaskKind,
    pub actor: WorkflowActor,
    pub title: String,
    pub detail: String,
    pub state: Option<Commitment>,
    pub header: Option<String>,
    pub content_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowBoard {
    pub release_scope: String,
    pub decision: Vec<WorkflowTask>,
    pub delegation: Vec<WorkflowTask>,
    pub lock_challenges: Vec<WorkflowTask>,
    pub materialization: Vec<WorkflowTask>,
    pub readiness: ReleaseReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseReadiness {
    pub selected_commit_ready: usize,
    pub excluded_unlocked: usize,
    pub open_decisions: usize,
    pub open_delegations: usize,
    pub ready: bool,
    pub summary: String,
}

/// Semantic diff entry separating content edits from commitment transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticDiffEntry {
    pub header_before: String,
    pub header_after: String,
    pub content_changed: bool,
    pub state_changed: bool,
    pub from: Commitment,
    pub to: Commitment,
}

/// Build an OSE-facing workflow board from the current document revision.
pub fn build_workflow_board(
    document: &LosslessDocument,
    release_scope: &str,
    challenges: &[LockChallenge],
) -> WorkflowBoard {
    let diagnostics = analyze_document(document, &[]);
    let plan = plan_materialization(document, release_scope);

    let decision = diagnostics
        .decision_queue
        .iter()
        .map(|item| queue_task(WorkflowTaskKind::Decision, WorkflowActor::Human, item))
        .collect::<Vec<_>>();
    let delegation = diagnostics
        .delegation_queue
        .iter()
        .map(|item| queue_task(WorkflowTaskKind::Delegation, WorkflowActor::Ai, item))
        .collect::<Vec<_>>();

    let lock_challenges = challenges
        .iter()
        .map(|challenge| WorkflowTask {
            kind: WorkflowTaskKind::LockChallenge,
            actor: WorkflowActor::Human,
            title: format!(
                "Review AI lock challenge {} -> {}",
                challenge.original_state, challenge.challenged_state
            ),
            detail: format!(
                "reason: {}\nevidence: {}\nhash: {}",
                challenge.reason,
                challenge.evidence.join("; "),
                challenge.content_hash.hex()
            ),
            state: Some(challenge.challenged_state),
            header: None,
            content_hash: Some(challenge.content_hash),
        })
        .collect::<Vec<_>>();

    let materialization = plan
        .selection
        .slots
        .iter()
        .map(|slot| WorkflowTask {
            kind: WorkflowTaskKind::Materialization,
            actor: WorkflowActor::Ai,
            title: format!("Materialize {}", slot.header.trim()),
            detail: format!("provenance={:?} state={}", slot.provenance, slot.state),
            state: Some(slot.state),
            header: Some(slot.header.clone()),
            content_hash: Some(slot.content_hash),
        })
        .collect::<Vec<_>>();

    let readiness = release_readiness(&plan, decision.len(), delegation.len());

    WorkflowBoard {
        release_scope: release_scope.into(),
        decision,
        delegation,
        lock_challenges,
        materialization,
        readiness,
    }
}

fn queue_task(kind: WorkflowTaskKind, actor: WorkflowActor, item: &QueueItem) -> WorkflowTask {
    WorkflowTask {
        kind,
        actor,
        title: match kind {
            WorkflowTaskKind::Decision => format!("Decide {}", item.header.trim()),
            WorkflowTaskKind::Delegation => format!("Elaborate {}", item.header.trim()),
            _ => item.header.trim().to_string(),
        },
        detail: format!("state={} target={:?}", item.state, item.review_target),
        state: Some(item.state),
        header: Some(item.header.clone()),
        content_hash: None,
    }
}

fn release_readiness(
    plan: &MaterializationPlan,
    open_decisions: usize,
    open_delegations: usize,
) -> ReleaseReadiness {
    let selected = plan.selection.slots.len();
    let excluded = plan.excluded_unlocked.len();
    let ready = selected > 0 && open_decisions == 0;
    let summary = if ready {
        format!(
            "Release scope has {selected} commit-ready slot(s); {excluded} unlocked slot(s) remain outside scope."
        )
    } else if selected == 0 {
        "No commit-ready slots selected for this release scope.".into()
    } else {
        format!(
            "Release blocked by {open_decisions} decision(s); {open_delegations} delegation(s) may still proceed in parallel."
        )
    };
    ReleaseReadiness {
        selected_commit_ready: selected,
        excluded_unlocked: excluded,
        open_decisions,
        open_delegations,
        ready,
        summary,
    }
}

/// Compare two document revisions and classify content vs state transitions.
pub fn semantic_diff(
    before: &LosslessDocument,
    after: &LosslessDocument,
) -> Vec<SemanticDiffEntry> {
    let before_slots = collect_slot_snapshots(before);
    let after_slots = collect_slot_snapshots(after);
    let mut entries = Vec::new();

    for prior in &before_slots {
        if !prior.kind.is_top_level_fragment() {
            continue;
        }
        let identity = strip_header(&prior.header);
        let matched = after_slots.iter().find(|candidate| {
            candidate.kind == prior.kind && strip_header(&candidate.header) == identity
        });
        let Some(next) = matched else {
            continue;
        };
        let content_changed = strip_all_suffixes(&prior.core) != strip_all_suffixes(&next.core);
        let state_changed = prior.state != next.state;
        if content_changed || state_changed {
            entries.push(SemanticDiffEntry {
                header_before: prior.header.clone(),
                header_after: next.header.clone(),
                content_changed,
                state_changed,
                from: prior.state,
                to: next.state,
            });
        }
    }
    entries
}

/// Remember rejected challenges until evidence changes the fingerprint.
pub fn filter_active_challenges(
    existing: &[LockChallenge],
    rejected: &[LockChallenge],
) -> Vec<LockChallenge> {
    existing
        .iter()
        .filter(|challenge| !challenge_is_duplicate(rejected, challenge))
        .cloned()
        .collect()
}

fn strip_header(header: &str) -> String {
    strip_all_suffixes(header)
}

fn strip_all_suffixes(text: &str) -> String {
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
        let stripped = ["$$??", "$??", "$$?", "$?", "$$", "$", "??", "?"]
            .into_iter()
            .find_map(|suffix| token.strip_suffix(suffix))
            .unwrap_or(token);
        format!("{stripped}{sep}")
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collaboration::{build_lock_challenge, Actor, TransitionEffects};
    use crate::parse_lossless;

    #[test]
    fn board_schedules_decision_delegation_and_materialization() {
        let source = r#"desc?? "app"
func Pay$?:
    steps:
        charge payment

type Status$: Active | Paid
"#;
        let doc = parse_lossless(source).document;
        let board = build_workflow_board(&doc, "payments", &[]);
        assert!(!board.decision.is_empty());
        assert!(!board.delegation.is_empty());
        assert!(!board.materialization.is_empty());
        assert!(board
            .materialization
            .iter()
            .any(|task| task.title.contains("Status")));
        assert!(!board.readiness.ready); // decision still open
    }

    #[test]
    fn semantic_diff_separates_suffix_only_from_content() {
        let before = parse_lossless("func Pay$:\n    steps:\n        charge payment\n").document;
        let after_suffix =
            parse_lossless("func Pay$?:\n    steps:\n        charge payment\n").document;
        let after_content =
            parse_lossless("func Pay$:\n    steps:\n        charge twice\n").document;

        let suffix_diff = semantic_diff(&before, &after_suffix);
        assert!(suffix_diff.iter().any(|entry| {
            entry.state_changed && !entry.content_changed && entry.to == Commitment::LockedQuestion
        }));

        let content_diff = semantic_diff(&before, &after_content);
        assert!(content_diff
            .iter()
            .any(|entry| entry.content_changed && !entry.state_changed));
    }

    #[test]
    fn rejected_challenges_are_filtered_until_evidence_changes() {
        let doc = parse_lossless("func Pay$:\n    steps:\n        charge payment\n").document;
        let func = doc
            .nodes()
            .iter()
            .find(|node| node.kind == crate::lossless::SourceNodeKind::Func)
            .unwrap()
            .id;
        let challenge = build_lock_challenge(
            &doc,
            func,
            Commitment::Locked,
            Commitment::LockedQuestion,
            "missing refund",
            vec!["audit gap".into()],
            vec![],
        )
        .unwrap();
        let rejected = vec![challenge.clone()];
        let active = filter_active_challenges(std::slice::from_ref(&challenge), &rejected);
        assert!(active.is_empty());

        let mut renewed = challenge;
        renewed.evidence.push("new counterexample".into());
        let active = filter_active_challenges(std::slice::from_ref(&renewed), &rejected);
        assert_eq!(active.len(), 1);

        // Keep transition effects path exercised for AI challenge context.
        let _ = (Actor::Ai, TransitionEffects::default());
    }
}
