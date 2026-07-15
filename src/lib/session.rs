use serde::Serialize;

use crate::diagnostics::{analyze_document, DocumentDiagnostics};
use crate::error::ParseError;
use crate::ide::{
    code_actions_for_node, hover_at, ide_snapshot, semantic_tokens, CodeAction, HoverInfo,
    IdeSnapshot, SemanticToken,
};
use crate::lossless::{LosslessDocument, SourceNodeId};
use crate::materialize::{plan_materialization, MaterializationPlan};
use crate::profile::{analyze_generic_profile, analyze_mimi_profile, ProfileAnalysis};

/// In-memory collaboration document session for IDE/OSE clients.
///
/// This is intentionally not a full LSP server. It keeps one revision of source,
/// lossless analysis, and derived queues so editors can apply full-text updates
/// without re-implementing Core policy.
#[derive(Debug, Clone)]
pub struct DocumentSession {
    uri: String,
    version: u64,
    source: String,
    document: LosslessDocument,
    errors: Vec<ParseError>,
    diagnostics: DocumentDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionSnapshot {
    pub uri: String,
    pub version: u64,
    pub error_count: usize,
    pub ide: IdeSnapshot,
}

impl DocumentSession {
    pub fn open(uri: impl Into<String>, source: impl Into<String>) -> Self {
        let uri = uri.into();
        let source = source.into();
        let parsed = crate::parse_lossless(&source);
        let diagnostics = analyze_document(&parsed.document, &parsed.errors);
        Self {
            uri,
            version: 1,
            source,
            document: parsed.document,
            errors: parsed.errors,
            diagnostics,
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn document(&self) -> &LosslessDocument {
        &self.document
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    pub fn diagnostics(&self) -> &DocumentDiagnostics {
        &self.diagnostics
    }

    /// Replace the full document text and recompute derived state.
    pub fn update_full(&mut self, source: impl Into<String>) {
        self.source = source.into();
        self.version = self.version.saturating_add(1);
        let parsed = crate::parse_lossless(&self.source);
        self.document = parsed.document;
        self.errors = parsed.errors;
        self.diagnostics = analyze_document(&self.document, &self.errors);
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            uri: self.uri.clone(),
            version: self.version,
            error_count: self.errors.len(),
            ide: ide_snapshot(&self.document, &self.errors),
        }
    }

    pub fn semantic_tokens(&self) -> Vec<SemanticToken> {
        semantic_tokens(&self.document)
    }

    pub fn hover(&self, offset: u32) -> Option<HoverInfo> {
        hover_at(&self.document, offset)
    }

    pub fn code_actions(&self, node: SourceNodeId) -> Vec<CodeAction> {
        code_actions_for_node(&self.document, node)
    }

    pub fn materialize(&self, release_scope: &str) -> MaterializationPlan {
        plan_materialization(&self.document, release_scope)
    }

    pub fn profile(&self, target: &str, release_scope: &str) -> Option<ProfileAnalysis> {
        match target {
            "mimi" => Some(analyze_mimi_profile(&self.document, release_scope)),
            "generic" => Some(analyze_generic_profile(&self.document, release_scope)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_update_bumps_version_and_queues() {
        let mut session = DocumentSession::open("file:///demo.mms", "desc?? \"app\"\n");
        assert_eq!(session.version(), 1);
        assert!(!session.diagnostics().delegation_queue.is_empty());

        session.update_full("func Pay$:\n    steps:\n        charge payment\n");
        assert_eq!(session.version(), 2);
        assert!(session.errors().is_empty());
        let snapshot = session.snapshot();
        assert_eq!(snapshot.version, 2);
        assert!(snapshot
            .ide
            .semantic_tokens
            .iter()
            .any(|token| matches!(token.kind, crate::ide::SemanticTokenKind::CommitmentLocked)));
    }

    #[test]
    fn session_profile_and_materialize_use_current_revision() {
        let session = DocumentSession::open(
            "mem://pay.mms",
            "func Pay$:\n    steps:\n        charge payment\n",
        );
        let plan = session.materialize("v1");
        assert!(!plan.selection.slots.is_empty());
        let analysis = session.profile("mimi", "v1").unwrap();
        assert_eq!(analysis.profile.name, "mimi");
        assert!(analysis
            .supported_slots
            .iter()
            .any(|slot| slot.header.contains("Pay")));
    }
}
