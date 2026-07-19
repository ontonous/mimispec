use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::ast::Commitment;
use crate::collaboration::{
    collect_semantic_slot_snapshots, validate_transition, Actor, HumanAuthorization,
    TransitionEffects, TransitionRequest,
};
use crate::render::render_file;

pub const CONFORMANCE_SCHEMA_VERSION: &str = "mimispec.conformance/0.3";

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    parse_cases: Vec<ParseCase>,
    transition_matrix: String,
    lsp_transcript: String,
}

#[derive(Debug, Deserialize)]
struct ParseCase {
    name: String,
    source: String,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct TransitionMatrix {
    schema_version: String,
    cases: Vec<TransitionCase>,
}

#[derive(Debug, Deserialize)]
struct TransitionCase {
    name: String,
    actor: String,
    from: String,
    to: String,
    #[serde(default)]
    effects: Effects,
    #[serde(default)]
    authorization: Authorization,
    challenge_reason: Option<String>,
    allowed: bool,
    violation: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Effects {
    #[serde(default)]
    content_changed: bool,
    #[serde(default)]
    structure_changed: bool,
    #[serde(default)]
    attachment_changed: bool,
}

#[derive(Debug, Default, Deserialize)]
struct Authorization {
    #[serde(default)]
    modify_protected: bool,
    #[serde(default)]
    unlock_strong_lock: bool,
}

#[derive(Debug, Deserialize)]
struct LspTranscript {
    schema_version: String,
    messages: Vec<Value>,
    expectations: Vec<LspExpectation>,
}

#[derive(Debug, Deserialize)]
struct LspExpectation {
    #[serde(default)]
    id: Option<Value>,
    method: Option<String>,
    pointer: String,
    equals: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceFailure {
    pub case: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceReport {
    pub schema_version: &'static str,
    pub manifest: String,
    pub total: usize,
    pub passed: usize,
    pub failures: Vec<ConformanceFailure>,
}

impl ConformanceReport {
    pub fn success(&self) -> bool {
        self.failures.is_empty()
    }
}

pub fn check_manifest(path: &Path) -> Result<ConformanceReport, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid manifest {}: {error}", path.display()))?;
    if manifest.schema_version != CONFORMANCE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported conformance schema `{}`",
            manifest.schema_version
        ));
    }
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut failures = Vec::new();
    let mut total = 0;

    for case in &manifest.parse_cases {
        total += 1;
        if let Err(message) = check_parse_case(root, case) {
            failures.push(ConformanceFailure {
                case: case.name.clone(),
                message,
            });
        }
    }

    let transition_path = root.join(&manifest.transition_matrix);
    let transition_bytes = fs::read(&transition_path)
        .map_err(|error| format!("{}: {error}", transition_path.display()))?;
    let matrix: TransitionMatrix = serde_json::from_slice(&transition_bytes)
        .map_err(|error| format!("invalid {}: {error}", transition_path.display()))?;
    if matrix.schema_version != CONFORMANCE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported transition schema `{}`",
            matrix.schema_version
        ));
    }
    for case in &matrix.cases {
        total += 1;
        if let Err(message) = check_transition_case(case) {
            failures.push(ConformanceFailure {
                case: format!("transition/{}", case.name),
                message,
            });
        }
    }

    total += 1;
    let transcript_path = root.join(&manifest.lsp_transcript);
    if let Err(message) = check_lsp_transcript(&transcript_path) {
        failures.push(ConformanceFailure {
            case: "lsp/transcript".into(),
            message,
        });
    }

    Ok(ConformanceReport {
        schema_version: CONFORMANCE_SCHEMA_VERSION,
        manifest: path.display().to_string(),
        total,
        passed: total - failures.len(),
        failures,
    })
}

fn check_parse_case(root: &Path, case: &ParseCase) -> Result<(), String> {
    let source_path = root.join(&case.source);
    let expected_path = root.join(&case.expected);
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let expected_bytes = fs::read(&expected_path)
        .map_err(|error| format!("{}: {error}", expected_path.display()))?;
    let expected: Value = serde_json::from_slice(&expected_bytes)
        .map_err(|error| format!("invalid {}: {error}", expected_path.display()))?;
    let actual = conformance_parse_value(&source);
    compare_subset(&expected, &actual, "$".into())
}

pub fn conformance_parse_value(source: &str) -> Value {
    let semantic = crate::parse(source);
    let lossless = crate::parse_lossless(source);
    let slots = collect_semantic_slot_snapshots(&lossless.document)
        .into_iter()
        .map(|slot| {
            json!({
                "anchor": slot.anchor,
                "state": slot.state.to_string(),
                "footprint": format!("{:?}", slot.footprint).to_lowercase(),
                "anchor_span": slot.anchor_span,
                "suffix_span": slot.suffix_span
            })
        })
        .collect::<Vec<_>>();
    let explicit_slots = slots
        .iter()
        .filter(|slot| slot.get("state").and_then(Value::as_str) != Some(""))
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "schema_version": CONFORMANCE_SCHEMA_VERSION,
        "status": format!("{:?}", semantic.status).to_lowercase(),
        "ast": serde_json::to_value(&semantic.file).unwrap_or(Value::Null),
        "error_codes": semantic.errors.iter().map(|error| error.code.to_string()).collect::<Vec<_>>(),
        "render": render_file(&semantic.file),
        "lossless": {
            "source_sha256": sha256(source.as_bytes()),
            "source_exact": lossless.document.render_lossless() == source,
            "rule_attachments": lossless.document.rule_attachment_fingerprint(),
            "slot_count": slots.len(),
            "all_spans_valid": lossless.document.commitment_slots().iter().all(|slot| {
                slot.anchor_span.end <= source.len() as u32
                    && slot.suffix_span.end <= source.len() as u32
                    && slot.anchor_span.start <= slot.anchor_span.end
                    && slot.suffix_span.start <= slot.suffix_span.end
            }),
            "explicit_slots": explicit_slots,
            "commitment_slots": slots
        }
    })
}

fn check_transition_case(case: &TransitionCase) -> Result<(), String> {
    let actor = match case.actor.as_str() {
        "human" => Actor::Human,
        "ai" => Actor::Ai,
        other => return Err(format!("unknown actor `{other}`")),
    };
    let from = parse_commitment(&case.from)?;
    let to = parse_commitment(&case.to)?;
    let result = validate_transition(&TransitionRequest {
        actor,
        from,
        to,
        effects: TransitionEffects {
            content_changed: case.effects.content_changed,
            structure_changed: case.effects.structure_changed,
            attachment_changed: case.effects.attachment_changed,
        },
        authorization: HumanAuthorization {
            modify_protected: case.authorization.modify_protected,
            unlock_strong_lock: case.authorization.unlock_strong_lock,
        },
        challenge_reason: case.challenge_reason.as_deref(),
    });
    if result.is_ok() != case.allowed {
        return Err(format!(
            "expected allowed={}, got {:?}",
            case.allowed, result
        ));
    }
    if let (Err(actual), Some(expected)) = (result, case.violation.as_deref()) {
        let actual = match actual {
            crate::collaboration::TransitionViolation::AiTransitionForbidden => {
                "ai_transition_forbidden"
            }
            crate::collaboration::TransitionViolation::ProtectedContentChanged => {
                "protected_content_changed"
            }
            crate::collaboration::TransitionViolation::ProtectedStructureChanged => {
                "protected_structure_changed"
            }
            crate::collaboration::TransitionViolation::ProtectedAttachmentChanged => {
                "protected_attachment_changed"
            }
            crate::collaboration::TransitionViolation::ChallengeReasonRequired => {
                "challenge_reason_required"
            }
            crate::collaboration::TransitionViolation::HumanAuthorizationRequired => {
                "human_authorization_required"
            }
            crate::collaboration::TransitionViolation::StrongUnlockAuthorizationRequired => {
                "strong_unlock_authorization_required"
            }
        };
        if actual != expected.to_ascii_lowercase() {
            return Err(format!("expected violation {expected}, got {actual}"));
        }
    }
    Ok(())
}

fn check_lsp_transcript(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let transcript: LspTranscript = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    if transcript.schema_version != CONFORMANCE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported LSP transcript schema `{}`",
            transcript.schema_version
        ));
    }
    let responses = crate::lsp::run_json_transcript(&transcript.messages)
        .map_err(|error| format!("LSP transcript failed: {error}"))?;
    for expectation in transcript.expectations {
        let response = responses.iter().find(|response| {
            expectation
                .id
                .as_ref()
                .is_some_and(|id| response.get("id") == Some(id))
                || expectation.method.as_deref().is_some_and(|method| {
                    response.get("method").and_then(Value::as_str) == Some(method)
                })
        });
        let Some(response) = response else {
            return Err(format!(
                "missing response for expectation `{}`",
                expectation.pointer
            ));
        };
        let actual = response
            .pointer(&expectation.pointer)
            .unwrap_or(&Value::Null);
        if actual != &expectation.equals {
            return Err(format!(
                "expectation {}: expected {}, got {}",
                expectation.pointer, expectation.equals, actual
            ));
        }
    }
    Ok(())
}

fn compare_subset(expected: &Value, actual: &Value, path: String) -> Result<(), String> {
    match expected {
        Value::Object(expected) => {
            let Some(actual) = actual.as_object() else {
                return Err(format!("{path}: expected object, got {actual}"));
            };
            for (key, expected) in expected {
                let next = format!("{path}/{key}");
                let actual = actual
                    .get(key)
                    .ok_or_else(|| format!("{next}: missing key"))?;
                compare_subset(expected, actual, next)?;
            }
            Ok(())
        }
        Value::Array(expected) => {
            let Some(actual) = actual.as_array() else {
                return Err(format!("{path}: expected array, got {actual}"));
            };
            if expected.len() != actual.len() {
                return Err(format!(
                    "{path}: expected {} entries, got {}",
                    expected.len(),
                    actual.len()
                ));
            }
            for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
                compare_subset(expected, actual, format!("{path}/{index}"))?;
            }
            Ok(())
        }
        _ if expected == actual => Ok(()),
        _ => Err(format!("{path}: expected {expected}, got {actual}")),
    }
}

fn parse_commitment(value: &str) -> Result<Commitment, String> {
    match value {
        "none" | "" => Ok(Commitment::None),
        "?" => Ok(Commitment::Question),
        "??" => Ok(Commitment::QuestionQuestion),
        "$" => Ok(Commitment::Locked),
        "$?" => Ok(Commitment::LockedQuestion),
        "$??" => Ok(Commitment::LockedQuestionQuestion),
        "$$" => Ok(Commitment::StrongLocked),
        "$$?" => Ok(Commitment::StrongLockedQuestion),
        "$$??" => Ok(Commitment::StrongLockedQuestionQuestion),
        other => Err(format!("unknown commitment `{other}`")),
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn bundled_conformance_suite_passes() {
        let manifest =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/0.3/manifest.json");
        let report = check_manifest(&manifest).expect("conformance suite must load");
        assert!(report.success(), "{:?}", report.failures);
    }
}
