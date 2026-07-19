//! Experimental Core-external provenance sidecar support.
//!
//! Provenance observes relationships between source artifacts and exact
//! MimiSpec semantic slots. It never changes commitment and never invokes a
//! target toolchain.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::collaboration::{collect_semantic_slot_snapshots, SemanticSlotSnapshot};
use crate::lossless::{CommitmentSlotId, LosslessDocument};

pub const PROVENANCE_SCHEMA_VERSION: &str = "mimispec.provenance/0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceManifest {
    pub schema_version: String,
    pub mms: ArtifactRevision,
    pub source: SourceArtifactRevision,
    #[serde(default)]
    pub links: Vec<ProvenanceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRevision {
    pub path: String,
    pub revision_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArtifactRevision {
    pub language: String,
    pub path: String,
    pub revision_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_span: Option<ExternalSourceSpan>,
    pub slot: SlotLocator,
    pub relation: ProvenanceRelation,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceRelation {
    ObservedFrom,
    InferredFrom,
    ConfirmedAgainst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSourceSpan {
    pub start: u64,
    pub end: u64,
}

/// Cross-revision locator for an exact semantic commitment slot.
///
/// No revision-local node ID is persisted. Ambiguous matches are errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlotLocator {
    pub scope_path: Vec<String>,
    pub node_kind: String,
    pub anchor_kind: String,
    pub footprint: String,
    pub owner_slot_ordinal: u32,
    pub protected_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvenanceCheckReport {
    pub schema_version: &'static str,
    pub valid: bool,
    pub checked_links: usize,
    pub findings: Vec<ProvenanceFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvenanceFinding {
    pub code: String,
    pub message: String,
    pub link_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocatorResolution {
    Unique(CommitmentSlotId),
    Missing,
    Ambiguous(Vec<CommitmentSlotId>),
}

pub fn slot_locator(document: &LosslessDocument, slot: &SemanticSlotSnapshot) -> SlotLocator {
    let scope_path = document
        .scope_path(slot.node)
        .unwrap_or_default()
        .iter()
        .filter_map(|id| document.node(*id))
        .map(|node| {
            format!(
                "{}:{}",
                kind_name(node.kind),
                normalize_header(document.text(node.spans.header).unwrap_or_default())
            )
        })
        .collect();
    SlotLocator {
        scope_path,
        node_kind: kind_name(slot.kind),
        anchor_kind: format!("{:?}", slot.anchor_kind).to_ascii_lowercase(),
        footprint: format!("{:?}", slot.footprint).to_ascii_lowercase(),
        owner_slot_ordinal: slot.owner_slot_index,
        protected_sha256: sha256_bytes(slot.protected_text.as_bytes()),
    }
}

pub fn resolve_slot_locator(
    document: &LosslessDocument,
    locator: &SlotLocator,
) -> LocatorResolution {
    let matches = collect_semantic_slot_snapshots(document)
        .into_iter()
        .filter(|slot| slot_locator_identity(document, slot, locator))
        .filter(|slot| sha256_bytes(slot.protected_text.as_bytes()) == locator.protected_sha256)
        .map(|slot| slot.slot)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => LocatorResolution::Missing,
        [slot] => LocatorResolution::Unique(*slot),
        _ => LocatorResolution::Ambiguous(matches),
    }
}

#[cfg(feature = "experimental-targets")]
pub(crate) fn resolve_locator_identity(
    document: &LosslessDocument,
    locator: &SlotLocator,
) -> Vec<SemanticSlotSnapshot> {
    collect_semantic_slot_snapshots(document)
        .into_iter()
        .filter(|slot| slot_locator_identity(document, slot, locator))
        .collect()
}

fn slot_locator_identity(
    document: &LosslessDocument,
    slot: &SemanticSlotSnapshot,
    locator: &SlotLocator,
) -> bool {
    let candidate = slot_locator(document, slot);
    candidate.scope_path == locator.scope_path
        && candidate.node_kind == locator.node_kind
        && candidate.anchor_kind == locator.anchor_kind
        && candidate.footprint == locator.footprint
        && candidate.owner_slot_ordinal == locator.owner_slot_ordinal
}

pub fn check_manifest_path(
    manifest_path: &Path,
    source_root: &Path,
) -> Result<ProvenanceCheckReport, String> {
    let manifest_text = fs::read_to_string(manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest: ProvenanceManifest = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("invalid provenance manifest: {error}"))?;
    check_manifest(&manifest, source_root)
}

pub fn check_manifest(
    manifest: &ProvenanceManifest,
    source_root: &Path,
) -> Result<ProvenanceCheckReport, String> {
    let root = source_root
        .canonicalize()
        .map_err(|error| format!("invalid source root {}: {error}", source_root.display()))?;
    let mut findings = Vec::new();
    if manifest.schema_version != PROVENANCE_SCHEMA_VERSION {
        findings.push(ProvenanceFinding {
            code: "P-SCHEMA".into(),
            message: format!(
                "expected schema {}, found {}",
                PROVENANCE_SCHEMA_VERSION, manifest.schema_version
            ),
            link_index: None,
        });
    }

    let mms_path = resolve_inside_root(&root, &manifest.mms.path)?;
    let source_path = resolve_inside_root(&root, &manifest.source.path)?;
    if mms_path.is_none() {
        findings.push(ProvenanceFinding {
            code: "P-MMS-MISSING".into(),
            message: "declared MMS artifact does not exist under source root".into(),
            link_index: None,
        });
    }
    if source_path.is_none() {
        findings.push(ProvenanceFinding {
            code: "P-SOURCE-MISSING".into(),
            message: "declared source artifact does not exist under source root".into(),
            link_index: None,
        });
    }
    let (Some(mms_path), Some(source_path)) = (mms_path, source_path) else {
        return Ok(ProvenanceCheckReport {
            schema_version: PROVENANCE_SCHEMA_VERSION,
            valid: false,
            checked_links: 0,
            findings,
        });
    };
    let mms_bytes = fs::read(&mms_path)
        .map_err(|error| format!("failed to read MMS {}: {error}", mms_path.display()))?;
    let source_bytes = fs::read(&source_path)
        .map_err(|error| format!("failed to read source {}: {error}", source_path.display()))?;
    let mms_hash = sha256_bytes(&mms_bytes);
    let source_hash = sha256_bytes(&source_bytes);
    if mms_hash != manifest.mms.revision_sha256 {
        findings.push(ProvenanceFinding {
            code: "P-MMS-DRIFT".into(),
            message: "MMS revision SHA-256 does not match the sidecar".into(),
            link_index: None,
        });
    }
    if source_hash != manifest.source.revision_sha256 {
        findings.push(ProvenanceFinding {
            code: "P-SOURCE-DRIFT".into(),
            message: "source revision SHA-256 does not match the sidecar".into(),
            link_index: None,
        });
    }

    let mms_text = std::str::from_utf8(&mms_bytes)
        .map_err(|error| format!("MMS source is not UTF-8: {error}"))?;
    let parsed = crate::parse_lossless(mms_text);
    if !parsed.errors.is_empty() {
        findings.push(ProvenanceFinding {
            code: "P-MMS-PARTIAL".into(),
            message: "MMS document is Partial; locators cannot be trusted".into(),
            link_index: None,
        });
    } else {
        for (index, link) in manifest.links.iter().enumerate() {
            match resolve_slot_locator(&parsed.document, &link.slot) {
                LocatorResolution::Unique(_) => {}
                LocatorResolution::Missing => findings.push(ProvenanceFinding {
                    code: "P-LOCATOR-MISSING".into(),
                    message: "slot locator no longer resolves at its protected hash".into(),
                    link_index: Some(index),
                }),
                LocatorResolution::Ambiguous(slots) => findings.push(ProvenanceFinding {
                    code: "P-LOCATOR-AMBIGUOUS".into(),
                    message: format!("slot locator resolves to {} slots", slots.len()),
                    link_index: Some(index),
                }),
            }
            if let Some(span) = link.source_span {
                if span.start > span.end || span.end > source_bytes.len() as u64 {
                    findings.push(ProvenanceFinding {
                        code: "P-SOURCE-SPAN".into(),
                        message: "source span is outside the declared source revision".into(),
                        link_index: Some(index),
                    });
                }
            }
        }
    }

    Ok(ProvenanceCheckReport {
        schema_version: PROVENANCE_SCHEMA_VERSION,
        valid: findings.is_empty(),
        checked_links: manifest.links.len(),
        findings,
    })
}

fn resolve_inside_root(root: &Path, relative: &str) -> Result<Option<PathBuf>, String> {
    let path = Path::new(relative);
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "path must be relative and may not escape source root: {relative}"
        ));
    }
    let resolved = match root.join(path).canonicalize() {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "unable to resolve source-root path {relative}: {error}"
            ))
        }
    };
    if !resolved.starts_with(root) {
        return Err(format!("path escapes source root: {relative}"));
    }
    Ok(Some(resolved))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn normalize_header(header: &str) -> String {
    header
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("$$??", "")
        .replace("$$?", "")
        .replace("$??", "")
        .replace("$?", "")
        .replace("$$", "")
        .replace("??", "")
        .replace(['$', '?'], "")
}

fn kind_name(kind: crate::lossless::SourceNodeKind) -> String {
    format!("{kind:?}").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mimispec-provenance-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn locator_survives_reorder_and_reports_ambiguity_without_node_fallback() {
        let before = crate::parse_lossless("func Pay$?: ...\nfunc Refund?: ...\n").document;
        let slot = collect_semantic_slot_snapshots(&before)
            .into_iter()
            .find(|slot| slot.state == crate::ast::Commitment::LockedQuestion)
            .unwrap();
        let locator = slot_locator(&before, &slot);
        let reordered = crate::parse_lossless("func Refund?: ...\nfunc Pay$?: ...\n").document;
        assert!(matches!(
            resolve_slot_locator(&reordered, &locator),
            LocatorResolution::Unique(_)
        ));

        let duplicate = crate::parse_lossless("func Pay$?: ...\nfunc Pay$?: ...\n").document;
        assert!(matches!(
            resolve_slot_locator(&duplicate, &locator),
            LocatorResolution::Ambiguous(_)
        ));
    }

    #[test]
    fn checker_detects_hash_drift_path_escape_and_preserves_commitment() {
        let root = temp_root("check");
        let mms = "func? Sync: ...\n";
        let source = "func Sync() {}\n";
        fs::write(root.join("intent.mms"), mms).unwrap();
        fs::write(root.join("source.mimi"), source).unwrap();
        let parsed = crate::parse_lossless(mms);
        let snapshot = collect_semantic_slot_snapshots(&parsed.document)
            .into_iter()
            .find(|slot| slot.state == crate::ast::Commitment::Question)
            .unwrap();
        let fingerprint = parsed.document.commitment_fingerprint();
        let manifest = ProvenanceManifest {
            schema_version: PROVENANCE_SCHEMA_VERSION.into(),
            mms: ArtifactRevision {
                path: "intent.mms".into(),
                revision_sha256: sha256_bytes(mms.as_bytes()),
            },
            source: SourceArtifactRevision {
                language: "mimi".into(),
                path: "source.mimi".into(),
                revision_sha256: sha256_bytes(source.as_bytes()),
            },
            links: vec![ProvenanceLink {
                source_symbol: Some("Sync".into()),
                source_span: Some(ExternalSourceSpan {
                    start: 0,
                    end: source.len() as u64,
                }),
                slot: slot_locator(&parsed.document, &snapshot),
                relation: ProvenanceRelation::ObservedFrom,
                note: "trace only".into(),
            }],
        };
        let report = check_manifest(&manifest, &root).unwrap();
        assert!(report.valid, "{:?}", report.findings);
        let after = crate::parse_lossless(mms);
        assert_eq!(after.document.commitment_fingerprint(), fingerprint);

        fs::write(root.join("source.mimi"), "changed\n").unwrap();
        let drift = check_manifest(&manifest, &root).unwrap();
        assert!(drift
            .findings
            .iter()
            .any(|finding| finding.code == "P-SOURCE-DRIFT"));

        fs::remove_file(root.join("source.mimi")).unwrap();
        let missing = check_manifest(&manifest, &root).unwrap();
        assert!(!missing.valid);
        assert_eq!(missing.checked_links, 0);
        assert!(missing
            .findings
            .iter()
            .any(|finding| finding.code == "P-SOURCE-MISSING"));

        let mut escaped = manifest.clone();
        escaped.source.path = "../outside.mimi".into();
        assert!(check_manifest(&escaped, &root).is_err());
    }

    #[test]
    fn real_project_sidecars_resolve_without_auto_locking() {
        for (manifest_text, mms) in [
            (
                include_str!("../../docs/corpora/mimi-kv-real-project.provenance.json"),
                include_str!("../../docs/corpora/mimi-kv-real-project.mms"),
            ),
            (
                include_str!("../../docs/corpora/mimichat-real-project.provenance.json"),
                include_str!("../../docs/corpora/mimichat-real-project.mms"),
            ),
        ] {
            let manifest: ProvenanceManifest = serde_json::from_str(manifest_text).unwrap();
            assert_eq!(manifest.schema_version, PROVENANCE_SCHEMA_VERSION);
            assert_eq!(manifest.mms.revision_sha256, sha256_bytes(mms.as_bytes()));
            let parsed = crate::parse_lossless(mms);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            for link in &manifest.links {
                assert!(matches!(
                    resolve_slot_locator(&parsed.document, &link.slot),
                    LocatorResolution::Unique(_)
                ));
            }
            assert!(parsed
                .document
                .commitment_slots()
                .iter()
                .all(|slot| !slot.value.is_commit_ready()));
        }
    }
}
