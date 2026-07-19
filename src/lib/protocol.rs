//! Typed request values for the frozen `mimispec.ls/0.3` custom protocol.
//!
//! The LSP adapter may still shape standard JSON-RPC envelopes, but custom
//! request fields are decoded through these deny-unknown-fields DTOs so the
//! runtime and the checked-in wire schema cannot silently accept different
//! request shapes.

use serde::Deserialize;

use crate::collaboration::{Actor, HumanAuthorization};
use crate::session::{SessionTextEdit, TextPosition};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRequest {
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default, rename = "textDocument")]
    pub text_document: Option<TextDocumentIdentifier>,
}

impl SnapshotRequest {
    pub fn into_uri(self) -> Result<String, &'static str> {
        match (self.uri, self.text_document) {
            (Some(uri), None) => Ok(uri),
            (None, Some(document)) => Ok(document.uri),
            _ => Err("exactly one of uri or textDocument is required"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationRequest {
    pub modify_protected: bool,
}

impl From<AuthorizationRequest> for HumanAuthorization {
    fn from(value: AuthorizationRequest) -> Self {
        Self {
            modify_protected: value.modify_protected,
            unlock_strong_lock: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentEditRequest {
    pub uri: String,
    pub base_version: u64,
    pub actor: Actor,
    pub edits: Vec<SessionTextEdit>,
    pub authorization: AuthorizationRequest,
    pub unlock_tokens: Vec<String>,
    #[serde(default)]
    pub challenge_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueBatchRequest {
    pub uri: String,
    pub base_version: u64,
    pub actor: Actor,
    pub slot_ids: Vec<u32>,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlockTokenRequest {
    pub uri: String,
    pub base_version: u64,
    pub actor: Actor,
    pub slot: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptObservedRequest {
    pub uri: String,
    pub base_version: u64,
    pub actor: Actor,
    pub authorization: AuthorizationRequest,
    pub unlock_tokens: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreAuthoritativeRequest {
    pub uri: String,
    pub base_version: u64,
    pub actor: Actor,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotNavigationRequest {
    pub uri: String,
    pub position: TextPosition,
}

pub fn decode<T: for<'de> Deserialize<'de>>(value: &serde_json::Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frozen_requests_reject_unknown_fields_and_ambiguous_snapshot_uris() {
        let edit = json!({
            "uri": "file:///a.mms",
            "base_version": 1,
            "actor": "human",
            "edits": [{
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "text": "?"
            }],
            "authorization": { "modify_protected": false },
            "unlock_tokens": []
        });
        assert!(decode::<DocumentEditRequest>(&edit).is_ok());

        let mut unknown = edit;
        unknown["admin"] = json!(true);
        assert!(decode::<DocumentEditRequest>(&unknown).is_err());

        let both = decode::<SnapshotRequest>(&json!({
            "uri": "file:///a.mms",
            "textDocument": { "uri": "file:///a.mms" }
        }))
        .unwrap();
        assert!(both.into_uri().is_err());
    }
}
