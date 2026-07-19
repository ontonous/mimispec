use serde::Serialize;

use crate::ast::Commitment;
use crate::collaboration::{
    challenge_is_duplicate, collect_semantic_slot_snapshots, ContentHash, LockChallenge,
};
use crate::diagnostics::{analyze_document, QueueItem};
use crate::lossless::LosslessDocument;
use crate::materialize::{plan_materialization, EvidenceRecord, MaterializationPlan};
use crate::provenance::{resolve_locator_identity, slot_locator};

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
    pub open_challenges: usize,
    pub evidence_ready: bool,
    pub ready: bool,
    pub blockers: Vec<String>,
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
    build_workflow_board_with_evidence(document, release_scope, challenges, &[])
}

/// Build a workflow board with externally produced, slot-exact evidence.
///
/// Evidence is observational: it does not alter commitment. Readiness only
/// consumes records bound to the selected slot locator and protected hash.
pub fn build_workflow_board_with_evidence(
    document: &LosslessDocument,
    release_scope: &str,
    challenges: &[LockChallenge],
    evidence: &[EvidenceRecord],
) -> WorkflowBoard {
    let diagnostics = analyze_document(document, &[]);
    let mut plan = plan_materialization(document, release_scope);
    plan.evidence.extend_from_slice(evidence);

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

    let readiness = release_readiness(
        &plan,
        decision.len(),
        delegation.len(),
        lock_challenges.len(),
    );

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
    open_challenges: usize,
) -> ReleaseReadiness {
    let selected = plan.selection.slots.len();
    let excluded = plan.excluded_unlocked.len();
    let evidence_ready = selected > 0
        && plan.selection.slots.iter().all(|slot| {
            let has_test = plan.evidence.iter().any(|evidence| {
                evidence.slot == slot.slot
                    && evidence.locator == slot.locator
                    && evidence.content_hash == slot.content_hash
                    && evidence.kind == crate::materialize::EvidenceKind::Tested
                    && evidence.status == crate::materialize::EvidenceStatus::Passed
            });
            let has_build = plan.evidence.iter().any(|evidence| {
                evidence.slot == slot.slot
                    && evidence.locator == slot.locator
                    && evidence.content_hash == slot.content_hash
                    && evidence.kind == crate::materialize::EvidenceKind::Built
                    && evidence.status == crate::materialize::EvidenceStatus::Passed
            });
            has_test && has_build
        });
    let mut blockers = Vec::new();
    if selected == 0 {
        blockers.push("no confirmed slots are selected".into());
    }
    if open_decisions > 0 {
        blockers.push(format!("{open_decisions} Human decision(s) remain open"));
    }
    if open_delegations > 0 {
        blockers.push(format!("{open_delegations} AI delegation(s) remain open"));
    }
    if open_challenges > 0 {
        blockers.push(format!("{open_challenges} lock challenge(s) remain open"));
    }
    if !evidence_ready {
        blockers.push("every selected slot requires passed Tested and Built evidence".into());
    }
    let ready = blockers.is_empty();
    let summary = if ready {
        format!("Release evidence is complete for {selected} selected slot(s).")
    } else {
        format!(
            "Not release-ready: {}. {excluded} unlocked slot(s) remain outside scope.",
            blockers.join("; ")
        )
    };
    ReleaseReadiness {
        selected_commit_ready: selected,
        excluded_unlocked: excluded,
        open_decisions,
        open_delegations,
        open_challenges,
        evidence_ready,
        ready,
        blockers,
        summary,
    }
}

/// Compare two document revisions and classify content vs state transitions.
pub fn semantic_diff(
    before: &LosslessDocument,
    after: &LosslessDocument,
) -> Vec<SemanticDiffEntry> {
    let before_slots = collect_semantic_slot_snapshots(before);
    let mut entries = Vec::new();

    for prior in &before_slots {
        let locator = slot_locator(before, prior);
        let matches = resolve_locator_identity(after, &locator);
        let [next] = matches.as_slice() else {
            continue;
        };
        let content_changed = prior.protected_text != next.protected_text;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collaboration::{
        build_lock_challenge, collect_semantic_slot_snapshots, Actor, TransitionEffects,
    };
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

        let independent_before = parse_lossless("func?? Pay$: ...\n").document;
        let independent_after = parse_lossless("func? Pay$: ...\n").document;
        let independent = semantic_diff(&independent_before, &independent_after);
        assert!(independent.iter().any(|entry| {
            entry.from == Commitment::QuestionQuestion
                && entry.to == Commitment::Question
                && entry.state_changed
        }));
    }

    #[test]
    fn commitment_without_external_evidence_is_never_release_ready() {
        let doc = parse_lossless("func$ Pay: ...\ndesc?? \"implement later\"\n").document;
        let board = build_workflow_board(&doc, "payments", &[]);
        assert!(!board.readiness.ready);
        assert!(!board.readiness.evidence_ready);
        assert_eq!(board.readiness.open_delegations, 1);
        assert!(board
            .readiness
            .blockers
            .iter()
            .any(|blocker| blocker.contains("Tested and Built")));
    }

    #[test]
    fn external_evidence_must_match_the_exact_selected_slot_and_hash() {
        let doc = parse_lossless("func$ Pay: ...\n").document;
        let plan = plan_materialization(&doc, "payments");
        let selected = plan.selection.slots.first().unwrap();
        let evidence = [
            EvidenceRecord {
                slot: selected.slot,
                locator: selected.locator.clone(),
                kind: crate::materialize::EvidenceKind::Tested,
                status: crate::materialize::EvidenceStatus::Passed,
                summary: "integration tests passed".into(),
                artifact: Some("test-report.json".into()),
                content_hash: selected.content_hash,
            },
            EvidenceRecord {
                slot: selected.slot,
                locator: selected.locator.clone(),
                kind: crate::materialize::EvidenceKind::Built,
                status: crate::materialize::EvidenceStatus::Passed,
                summary: "release artifact built".into(),
                artifact: Some("artifact.tar".into()),
                content_hash: selected.content_hash,
            },
        ];
        let ready = build_workflow_board_with_evidence(&doc, "payments", &[], &evidence);
        assert!(ready.readiness.ready, "{:?}", ready.readiness.blockers);

        let mut stale = evidence;
        stale[0].content_hash = ContentHash(0);
        let rejected = build_workflow_board_with_evidence(&doc, "payments", &[], &stale);
        assert!(!rejected.readiness.ready);
        assert!(!rejected.readiness.evidence_ready);
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
        let slot = collect_semantic_slot_snapshots(&doc)
            .into_iter()
            .find(|slot| slot.node == func && slot.state == Commitment::Locked)
            .unwrap()
            .slot;
        let challenge = build_lock_challenge(
            &doc,
            slot,
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
