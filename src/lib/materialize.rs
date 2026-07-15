use serde::Serialize;

use crate::ast::Commitment;
use crate::collaboration::{collect_slot_snapshots, protected_hashes, ContentHash};
use crate::lossless::{ByteSpan, LosslessDocument, SourceNodeId, SourceNodeKind};

/// How a materialization slot entered the current plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    HumanLocked,
    HumanStrongLocked,
    TargetDerived,
    ImplementationChoice,
    Unresolved,
    GeneratedTest,
}

/// One locked (or intentionally open) slot considered for materialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaterializationSlot {
    pub node: SourceNodeId,
    pub kind: SourceNodeKind,
    pub header: String,
    pub state: Commitment,
    pub provenance: Provenance,
    pub commit_ready: bool,
    pub content_hash: ContentHash,
    pub span: ByteSpan,
}

/// Explicit selection of slots that may be materialized in the current release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitSelection {
    pub release_scope: String,
    pub slots: Vec<MaterializationSlot>,
}

/// Evidence that a selected slot was checked, generated, tested, or verified.
///
/// Evidence never changes commitment. `$`/`$$` remain human intent only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceRecord {
    pub slot: SourceNodeId,
    pub kind: EvidenceKind,
    pub status: EvidenceStatus,
    pub summary: String,
    pub artifact: Option<String>,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Parsed,
    TypeChecked,
    Tested,
    Verified,
    Built,
    DriftChecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Passed,
    Failed,
    Unknown,
    Skipped,
}

/// A materialization plan for one release scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaterializationPlan {
    pub selection: CommitSelection,
    pub excluded_unlocked: Vec<MaterializationSlot>,
    pub evidence: Vec<EvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftFinding {
    pub slot: SourceNodeId,
    pub expected_hash: ContentHash,
    pub observed_hash: ContentHash,
    pub message: String,
}

/// Build a commit selection from a lossless document.
///
/// Only commit-ready slots (`$` / `$$`) are included by default. Explicit open
/// extension slots remain excluded unless `include_unresolved` is true, and even
/// then they are marked `Unresolved` rather than confirmed.
pub fn select_commit_ready(
    document: &LosslessDocument,
    release_scope: impl Into<String>,
    include_unresolved: bool,
) -> CommitSelection {
    let mut slots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for snapshot in collect_slot_snapshots(document) {
        let commit_ready = snapshot.state.is_commit_ready();
        if !commit_ready && !include_unresolved {
            continue;
        }
        // Prefer top-level Fragment kinds for materialization selection/display.
        if !snapshot.kind.is_top_level_fragment() {
            continue;
        }
        let identity = format!(
            "{:?}:{}:{:?}",
            snapshot.kind,
            strip_suffixes(&snapshot.header),
            snapshot.state
        );
        if !seen.insert(identity) {
            continue;
        }
        let provenance = if commit_ready {
            if snapshot.state.is_strong_locked() {
                Provenance::HumanStrongLocked
            } else {
                Provenance::HumanLocked
            }
        } else {
            Provenance::Unresolved
        };
        let span = document
            .node(snapshot.node)
            .map(|node| node.spans.full)
            .unwrap_or(ByteSpan::new(0, 0));
        slots.push(MaterializationSlot {
            node: snapshot.node,
            kind: snapshot.kind,
            header: snapshot.header,
            state: snapshot.state,
            provenance,
            commit_ready,
            content_hash: snapshot.hashes.content,
            span,
        });
    }
    CommitSelection {
        release_scope: release_scope.into(),
        slots,
    }
}

/// Create a materialization plan that never promotes unlocked intent as confirmed.
pub fn plan_materialization(
    document: &LosslessDocument,
    release_scope: impl Into<String>,
) -> MaterializationPlan {
    let release_scope = release_scope.into();
    let selected = select_commit_ready(document, release_scope.clone(), false);
    let all = select_commit_ready(document, release_scope, true);
    let excluded_unlocked = all
        .slots
        .into_iter()
        .filter(|slot| !slot.commit_ready)
        .collect();
    let evidence = selected
        .slots
        .iter()
        .map(|slot| EvidenceRecord {
            slot: slot.node,
            kind: EvidenceKind::Parsed,
            status: EvidenceStatus::Passed,
            summary: format!("selected {} as commit-ready", slot.header.trim()),
            artifact: None,
            content_hash: slot.content_hash,
        })
        .collect();
    MaterializationPlan {
        selection: selected,
        excluded_unlocked,
        evidence,
    }
}

/// Attach a target-derived or implementation-choice slot without auto-locking.
pub fn annotate_target_derived(
    mut slot: MaterializationSlot,
    choice: Provenance,
) -> MaterializationSlot {
    match choice {
        Provenance::TargetDerived
        | Provenance::ImplementationChoice
        | Provenance::GeneratedTest => {
            slot.provenance = choice;
            // Never upgrade commitment through materialization.
            slot.commit_ready = slot.state.is_commit_ready();
        }
        Provenance::HumanLocked | Provenance::HumanStrongLocked | Provenance::Unresolved => {}
    }
    slot
}

/// Detect drift when a later document no longer matches selected content hashes.
pub fn detect_drift(baseline: &CommitSelection, current: &LosslessDocument) -> Vec<DriftFinding> {
    let mut findings = Vec::new();
    for slot in &baseline.slots {
        if !slot.commit_ready {
            continue;
        }
        let Some(current_hashes) = protected_hashes(current, slot.node).or_else(|| {
            // Node IDs are revision-local; fall back to kind+header identity.
            collect_slot_snapshots(current)
                .into_iter()
                .find(|candidate| {
                    candidate.kind == slot.kind
                        && strip_suffixes(&candidate.header) == strip_suffixes(&slot.header)
                })
                .map(|candidate| candidate.hashes)
        }) else {
            findings.push(DriftFinding {
                slot: slot.node,
                expected_hash: slot.content_hash,
                observed_hash: ContentHash(0),
                message: format!(
                    "locked slot `{}` missing from current document",
                    slot.header.trim()
                ),
            });
            continue;
        };
        if current_hashes.content != slot.content_hash {
            findings.push(DriftFinding {
                slot: slot.node,
                expected_hash: slot.content_hash,
                observed_hash: current_hashes.content,
                message: format!(
                    "locked slot `{}` content drifted from selected hash",
                    slot.header.trim()
                ),
            });
        }
    }
    findings
}

/// Reject plans that would emit unlocked intent as confirmed target behavior.
pub fn validate_plan(plan: &MaterializationPlan) -> Result<(), String> {
    for slot in &plan.selection.slots {
        if !slot.commit_ready {
            return Err(format!(
                "unlocked slot `{}` cannot be confirmed materialization",
                slot.header.trim()
            ));
        }
        if matches!(slot.provenance, Provenance::Unresolved) {
            return Err(format!(
                "unresolved slot `{}` cannot be confirmed materialization",
                slot.header.trim()
            ));
        }
    }
    Ok(())
}

fn strip_suffixes(header: &str) -> String {
    header
        .split_inclusive(|ch: char| {
            ch.is_whitespace() || matches!(ch, ':' | '(' | ')' | '[' | ']' | ',' | '|')
        })
        .map(|part| {
            let (token, sep) = match part.chars().last() {
                Some(ch)
                    if ch.is_whitespace()
                        || matches!(ch, ':' | '(' | ')' | '[' | ']' | ',' | '|') =>
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
    use crate::parse_lossless;

    #[test]
    fn selects_only_commit_ready_slots_by_default() {
        let source = r#"desc?? "app"
func Pay$:
    steps:
        charge payment

type Status$$: Active | Paid
"#;
        let doc = parse_lossless(source).document;
        let selection = select_commit_ready(&doc, "payments-v1", false);
        assert!(selection.slots.iter().all(|slot| slot.commit_ready));
        assert!(selection.slots.iter().any(|slot| {
            slot.state == Commitment::Locked && slot.provenance == Provenance::HumanLocked
        }));
        assert!(selection.slots.iter().any(|slot| {
            slot.state == Commitment::StrongLocked
                && slot.provenance == Provenance::HumanStrongLocked
        }));
        assert!(!selection
            .slots
            .iter()
            .any(|slot| slot.state == Commitment::QuestionQuestion));
    }

    #[test]
    fn plan_excludes_unlocked_and_records_parse_evidence() {
        let source = r#"func Pay$:
    steps:
        charge payment

func Draft??:
    steps:
        todo
"#;
        let doc = parse_lossless(source).document;
        let plan = plan_materialization(&doc, "core");
        assert!(validate_plan(&plan).is_ok());
        assert!(plan.selection.slots.iter().all(|slot| slot.commit_ready));
        assert!(plan
            .excluded_unlocked
            .iter()
            .any(|slot| slot.provenance == Provenance::Unresolved));
        assert_eq!(plan.evidence.len(), plan.selection.slots.len());
        assert!(
            plan.evidence
                .iter()
                .all(|item| item.kind == EvidenceKind::Parsed
                    && item.status == EvidenceStatus::Passed)
        );
    }

    #[test]
    fn drift_detection_flags_changed_locked_content() {
        let before = parse_lossless(
            r#"func Pay$:
    steps:
        charge payment
"#,
        )
        .document;
        let selection = select_commit_ready(&before, "pay", false);
        let after = parse_lossless(
            r#"func Pay$:
    steps:
        charge twice
"#,
        )
        .document;
        let findings = detect_drift(&selection, &after);
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|finding| finding.expected_hash != finding.observed_hash));
    }

    #[test]
    fn target_derived_annotation_does_not_auto_lock() {
        let source = r#"func Pay??:
    steps:
        charge payment
"#;
        let doc = parse_lossless(source).document;
        let mut slot = select_commit_ready(&doc, "pay", true)
            .slots
            .into_iter()
            .next()
            .unwrap();
        assert!(!slot.commit_ready);
        slot = annotate_target_derived(slot, Provenance::TargetDerived);
        assert_eq!(slot.provenance, Provenance::TargetDerived);
        assert!(!slot.commit_ready);
        assert!(!slot.state.is_commit_ready());
    }
}
