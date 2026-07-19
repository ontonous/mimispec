//! Target-neutral MimiSpec 0.3 language server over LSP 3.17 stdio.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};

use serde_json::{json, Map, Value};

use crate::ast::{Commitment, LockIntent, ReviewIntent};
use crate::collaboration::{collect_semantic_slot_snapshots, Actor, HumanAuthorization};
use crate::diagnostics::{syntax_quick_fixes, Severity};
use crate::ide::{navigation_at_position, NavigationKind, SemanticTokenKind};
use crate::lossless::{ByteSpan, ColumnEncoding, SourcePosition};
use crate::protocol;
use crate::session::{
    CollaborationMode, DocumentEditRequest, DocumentSession, SessionTextEdit, SessionViolation,
    TextPosition, TextRange, LANGUAGE_SERVICE_SCHEMA_VERSION,
};

const JSON_RPC_VERSION: &str = "2.0";
const LSP_VERSION: &str = "3.17";

const TOKEN_TYPES: &[&str] = &[
    "mimispec.desc",
    "mimispec.clause",
    "namespace",
    "type",
    "mimispec.flow",
    "function",
    "mimispec.ui",
    "mimispec.steps",
    "property",
    "enumMember",
    "event",
    "mimispec.step",
    "mimispec.open",
    "mimispec.contentReview",
    "mimispec.contentDelegated",
    "mimispec.locked",
    "mimispec.lockReview",
    "mimispec.lockDelegated",
    "mimispec.strongLocked",
    "mimispec.strongLockReview",
    "mimispec.strongLockDelegated",
    "mimispec.ruleAttached",
    "mimispec.ruleEnvironment",
    "mimispec.ruleDropped",
];

#[derive(Default)]
struct LanguageServer {
    sessions: HashMap<String, DocumentSession>,
    mode: CollaborationMode,
}

/// Run the long-lived language server on process stdio.
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    run_with_io(&mut reader, &mut writer)
}

pub fn run_with_io<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    let mut server = LanguageServer::default();
    while let Some(message) = read_message(reader)? {
        let is_exit = message.get("method").and_then(Value::as_str) == Some("exit");
        for response in server.handle(message) {
            write_message(writer, &response)?;
        }
        writer.flush()?;
        if is_exit {
            break;
        }
    }
    Ok(())
}

/// Execute a JSON message transcript through the exact stdio framing engine.
/// Used by the language-neutral conformance suite and protocol tests.
pub fn run_json_transcript(messages: &[Value]) -> io::Result<Vec<Value>> {
    let mut input = Vec::new();
    for message in messages {
        write_message(&mut input, message)?;
    }
    let mut output = Vec::new();
    run_with_io(
        &mut BufReader::new(std::io::Cursor::new(input)),
        &mut output,
    )?;
    let mut reader = BufReader::new(std::io::Cursor::new(output));
    let mut responses = Vec::new();
    while let Some(response) = read_message(&mut reader)? {
        responses.push(response);
    }
    Ok(responses)
}

impl LanguageServer {
    fn handle(&mut self, message: Value) -> Vec<Value> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                if let Some(mode) = params
                    .pointer("/initializationOptions/collaborationMode")
                    .and_then(Value::as_str)
                    .and_then(parse_mode)
                {
                    self.mode = mode;
                }
                vec![success(
                    id,
                    json!({
                        "capabilities": {
                            "positionEncoding": "utf-16",
                            "textDocumentSync": { "openClose": true, "change": 2 },
                            "hoverProvider": true,
                            "definitionProvider": true,
                            "referencesProvider": true,
                            "codeActionProvider": { "resolveProvider": true },
                            "semanticTokensProvider": {
                                "legend": { "tokenTypes": TOKEN_TYPES, "tokenModifiers": [] },
                                "full": true
                            },
                            "experimental": {
                                "schemaVersion": LANGUAGE_SERVICE_SCHEMA_VERSION,
                                "collaborationModes": ["advisory", "strict"]
                            }
                        },
                        "serverInfo": { "name": "mimispec", "version": env!("CARGO_PKG_VERSION") },
                        "mimispec": { "schemaVersion": LANGUAGE_SERVICE_SCHEMA_VERSION, "lspVersion": LSP_VERSION }
                    }),
                )]
            }
            "initialized" => Vec::new(),
            "shutdown" => {
                vec![success(id, Value::Null)]
            }
            "exit" => Vec::new(),
            "textDocument/didOpen" => self.did_open(params),
            "textDocument/didChange" => self.did_change(params),
            "textDocument/didClose" => self.did_close(params),
            "workspace/didChangeConfiguration" => {
                self.change_configuration(params);
                Vec::new()
            }
            "textDocument/hover" => vec![self.hover(id, params)],
            "textDocument/semanticTokens/full" => vec![self.semantic_tokens(id, params)],
            "textDocument/definition" => vec![self.definition(id, params)],
            "textDocument/references" => vec![self.references(id, params)],
            "textDocument/codeAction" => vec![self.code_actions(id, params)],
            "codeAction/resolve" => vec![self.resolve_code_action(id, params)],
            "mimispec/documentSnapshot" => vec![self.document_snapshot(id, params)],
            "mimispec/prepareQueueBatch" => vec![self.prepare_queue_batch(id, params)],
            "mimispec/applyDocumentEdit" => vec![self.apply_document_edit(id, params)],
            "mimispec/issueUnlockToken" => vec![self.issue_unlock_token(id, params)],
            "mimispec/adoptObservedRevision" => vec![self.adopt_observed(id, params)],
            "mimispec/restoreAuthoritativeRevision" => {
                vec![self.restore_authoritative(id, params)]
            }
            "mimispec/slotNavigation" => vec![self.slot_navigation(id, params)],
            "" if id.is_some() => vec![rpc_error(id, -32600, "invalid JSON-RPC request")],
            _ if id.is_some() => vec![rpc_error(id, -32601, "method not found")],
            _ => Vec::new(),
        }
    }

    fn did_open(&mut self, params: Value) -> Vec<Value> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let text = params
            .pointer("/textDocument/text")
            .and_then(Value::as_str)
            .unwrap_or("");
        let mut session = DocumentSession::open(uri, text);
        session.set_mode(self.mode);
        self.sessions.insert(uri.into(), session);
        self.publish_diagnostics(uri)
    }

    fn did_change(&mut self, params: Value) -> Vec<Value> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(session) = self.sessions.get_mut(uri) else {
            return Vec::new();
        };

        let Some(changes) = params.get("contentChanges").and_then(Value::as_array) else {
            session.record_invalid_edit("contentChanges is required and must be an array");
            return self.publish_diagnostics(uri);
        };
        if changes.is_empty() {
            session.record_invalid_edit("contentChanges must not be empty");
            return self.publish_diagnostics(uri);
        }
        let (edits, violations): (Vec<SessionTextEdit>, Vec<SessionViolation>) = changes
            .iter()
            .map(|value| match parse_text_edit(value) {
                Ok(edit) => (Some(edit), None),
                Err(msg) => (None, Some(SessionViolation::new("C-INVALID-EDIT", msg))),
            })
            .fold(
                (Vec::new(), Vec::new()),
                |(mut edits, mut violations), (edit, violation)| {
                    if let Some(edit) = edit {
                        edits.push(edit);
                    }
                    if let Some(violation) = violation {
                        violations.push(violation);
                    }
                    (edits, violations)
                },
            );

        if violations.is_empty() {
            let _ = session.observe_edits(&edits);
        } else {
            for violation in violations {
                session.record_invalid_edit(&violation.message);
            }
        }
        self.publish_diagnostics(uri)
    }

    fn did_close(&mut self, params: Value) -> Vec<Value> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        self.sessions.remove(uri);
        vec![notification(
            "textDocument/publishDiagnostics",
            json!({ "uri": uri, "diagnostics": [] }),
        )]
    }

    fn change_configuration(&mut self, params: Value) {
        let mode = params
            .pointer("/settings/mimispec/collaborationMode")
            .or_else(|| params.pointer("/settings/collaborationMode"))
            .and_then(Value::as_str)
            .and_then(parse_mode);
        if let Some(mode) = mode {
            self.mode = mode;
            for session in self.sessions.values_mut() {
                session.set_mode(mode);
            }
        }
    }

    fn hover(&self, id: Option<Value>, params: Value) -> Value {
        let Some((uri, position)) = text_document_position(&params) else {
            return success(id, Value::Null);
        };
        let Some(session) = self.sessions.get(uri) else {
            return success(id, Value::Null);
        };
        let Some(hover) = crate::ide::hover_at_position(
            session.document(),
            source_position(position),
            ColumnEncoding::Utf16,
        ) else {
            return success(id, Value::Null);
        };
        success(
            id,
            json!({
                "contents": { "kind": "markdown", "value": format!("**{}**\n\n```text\n{}\n```", hover.title, hover.body) },
                "range": span_range(session, hover.span)
            }),
        )
    }

    fn semantic_tokens(&self, id: Option<Value>, params: Value) -> Value {
        let Some(uri) = document_uri(&params) else {
            return success(id, json!({ "data": [] }));
        };
        let Some(session) = self.sessions.get(uri) else {
            return success(id, json!({ "data": [] }));
        };
        let mut tokens = session.semantic_tokens();
        tokens.sort_by_key(|token| {
            (
                token.span.start,
                std::cmp::Reverse(token_priority(token.kind)),
                token.span.end,
            )
        });
        let mut data = Vec::<u32>::new();
        let mut previous_line = 0;
        let mut previous_start = 0;
        let mut last_end = 0;
        for token in tokens {
            if token.span.start < last_end || token.span.is_empty() {
                continue;
            }
            let Some(start) = session.document().line_index().position(
                session.source(),
                token.span.start,
                ColumnEncoding::Utf16,
            ) else {
                continue;
            };
            let Some(end) = session.document().line_index().position(
                session.source(),
                token.span.end,
                ColumnEncoding::Utf16,
            ) else {
                continue;
            };
            if start.line != end.line || end.column <= start.column {
                continue;
            }
            let delta_line = start.line - previous_line;
            let delta_start = if delta_line == 0 {
                start.column - previous_start
            } else {
                start.column
            };
            data.extend([
                delta_line,
                delta_start,
                end.column - start.column,
                token_index(token.kind),
                0,
            ]);
            previous_line = start.line;
            previous_start = start.column;
            last_end = token.span.end;
        }
        success(id, json!({ "data": data }))
    }

    fn definition(&self, id: Option<Value>, params: Value) -> Value {
        let Some((uri, position)) = text_document_position(&params) else {
            return success(id, Value::Null);
        };
        let Some(session) = self.sessions.get(uri) else {
            return success(id, Value::Null);
        };
        let navigation = navigation_at_position(
            session.document(),
            source_position(position),
            ColumnEncoding::Utf16,
        );
        let target = navigation.iter().find(|target| {
            matches!(
                target.kind,
                NavigationKind::RuleAttachmentTarget | NavigationKind::FlowTargetDefinition
            )
        });
        success(
            id,
            target.map_or(
                Value::Null,
                |target| json!({ "uri": uri, "range": span_range(session, target.span) }),
            ),
        )
    }

    fn references(&self, id: Option<Value>, params: Value) -> Value {
        let Some((uri, position)) = text_document_position(&params) else {
            return success(id, json!([]));
        };
        let Some(session) = self.sessions.get(uri) else {
            return success(id, json!([]));
        };
        let locations = navigation_at_position(
            session.document(),
            source_position(position),
            ColumnEncoding::Utf16,
        )
        .into_iter()
        .map(|target| json!({ "uri": uri, "range": span_range(session, target.span) }))
        .collect::<Vec<_>>();
        success(id, Value::Array(locations))
    }

    fn code_actions(&self, id: Option<Value>, params: Value) -> Value {
        let Some((uri, position)) = text_document_position(&params) else {
            return success(id, json!([]));
        };
        let Some(session) = self.sessions.get(uri) else {
            return success(id, json!([]));
        };
        if session.mode() == CollaborationMode::Strict && session.is_divergent() {
            return success(id, json!([]));
        }
        let Some(offset) = session.document().line_index().offset(
            session.source(),
            source_position(position),
            ColumnEncoding::Utf16,
        ) else {
            return success(id, json!([]));
        };
        let mut actions = syntax_quick_fixes(session.source(), session.errors())
            .into_iter()
            .filter(|fix| fix.span.start <= offset && offset <= fix.span.end)
            .map(|fix| {
                json!({
                    "title": fix.title,
                    "kind": "quickfix",
                    "data": {
                        "schemaVersion": LANGUAGE_SERVICE_SCHEMA_VERSION,
                        "uri": uri,
                        "baseVersion": session.authoritative_version(),
                        "syntaxRecovery": true,
                        "span": { "start": fix.span.start, "end": fix.span.end },
                        "replacement": fix.replacement
                    }
                })
            })
            .collect::<Vec<_>>();
        if let Some(node) = session
            .document()
            .nodes()
            .iter()
            .filter(|node| node.spans.core.start <= offset && offset <= node.spans.core.end)
            .min_by_key(|node| node.spans.core.len())
        {
            actions.extend(session.code_actions(node.id).into_iter().filter_map(|action| {
                let to = action.to?;
                let mut value = json!({
                    "title": action.title,
                    "kind": "quickfix",
                    "data": {
                        "schemaVersion": LANGUAGE_SERVICE_SCHEMA_VERSION,
                        "uri": uri,
                        "baseVersion": session.authoritative_version(),
                        "target": action.target.0,
                        "slot": action.slot.map(|slot| slot.0),
                        "actor": format!("{:?}", action.actor).to_lowercase(),
                        "from": action.from.to_string(),
                        "to": to.to_string()
                    }
                });
                if !action.allowed {
                    value["disabled"] =
                        json!({ "reason": action.reason.unwrap_or_else(|| "not allowed".into()) });
                }
                Some(value)
            }));
        }
        success(id, Value::Array(actions))
    }

    fn resolve_code_action(&mut self, id: Option<Value>, mut action: Value) -> Value {
        if action.get("disabled").is_some() {
            return success(id, action);
        }
        let Some(data) = action.get("data").cloned() else {
            return success(id, action);
        };
        if data.get("schemaVersion").and_then(Value::as_str)
            != Some(LANGUAGE_SERVICE_SCHEMA_VERSION)
        {
            action["disabled"] = json!({ "reason": "unsupported MimiSpec code-action schema" });
            return success(id, action);
        }
        let Some(uri) = data.get("uri").and_then(Value::as_str) else {
            action["disabled"] = json!({ "reason": "missing document URI" });
            return success(id, action);
        };
        let Some(base_version) = data.get("baseVersion").and_then(Value::as_u64) else {
            action["disabled"] = json!({ "reason": "missing authoritative base version" });
            return success(id, action);
        };
        let Some(session) = self.sessions.get_mut(uri) else {
            action["disabled"] = json!({ "reason": "document is not open" });
            return success(id, action);
        };
        if base_version != session.authoritative_version() {
            action["disabled"] = json!({ "reason": "code action is stale" });
            return success(id, action);
        }

        let (edit, actor, challenge_reason) = if data.get("syntaxRecovery").and_then(Value::as_bool)
            == Some(true)
        {
            let span = data.get("span");
            let start = span
                .and_then(|span| span.get("start"))
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            let end = span
                .and_then(|span| span.get("end"))
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            let replacement = data.get("replacement").and_then(Value::as_str);
            let (Some(start), Some(end), Some(replacement)) = (start, end, replacement) else {
                action["disabled"] = json!({ "reason": "invalid syntax-recovery edit" });
                return success(id, action);
            };
            let Some(range) = text_range_for_span(
                session.authoritative_document(),
                session.authoritative_source(),
                ByteSpan { start, end },
            ) else {
                action["disabled"] = json!({ "reason": "syntax-recovery span is no longer valid" });
                return success(id, action);
            };
            (
                SessionTextEdit {
                    range: Some(range),
                    text: replacement.to_string(),
                },
                Actor::Human,
                None,
            )
        } else {
            let slot_id = data
                .get("slot")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            let to = data
                .get("to")
                .and_then(Value::as_str)
                .and_then(parse_commitment);
            let actor = data
                .get("actor")
                .and_then(Value::as_str)
                .and_then(parse_actor);
            let (Some(slot_id), Some(to), Some(actor)) = (slot_id, to, actor) else {
                action["disabled"] = json!({ "reason": "invalid commitment transition data" });
                return success(id, action);
            };
            let Some(slot) = session
                .authoritative_document()
                .commitment_slot(crate::lossless::CommitmentSlotId(slot_id))
                .filter(|slot| slot.semantic_slot)
            else {
                action["disabled"] = json!({ "reason": "commitment slot is stale" });
                return success(id, action);
            };
            let Some(range) = text_range_for_span(
                session.authoritative_document(),
                session.authoritative_source(),
                slot.suffix_span,
            ) else {
                action["disabled"] =
                    json!({ "reason": "commitment suffix is no longer addressable" });
                return success(id, action);
            };
            let reason = (actor == Actor::Ai)
                .then(|| "Code action requested a review of lock-readiness evidence.".to_string());
            (
                SessionTextEdit {
                    range: Some(range),
                    text: to.to_string(),
                },
                actor,
                reason,
            )
        };

        let response = session.prepare_edit(DocumentEditRequest {
            base_version,
            actor,
            edits: vec![edit.clone()],
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason,
        });
        if response.accepted {
            action["edit"] = workspace_edit(uri, session, &[edit]);
        } else {
            let reason = response
                .violations
                .iter()
                .map(|violation| violation.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            action["disabled"] = json!({ "reason": reason });
        }
        success(id, action)
    }

    fn document_snapshot(&self, id: Option<Value>, params: Value) -> Value {
        let uri = match protocol::decode::<protocol::SnapshotRequest>(&params)
            .and_then(|request| request.into_uri().map_err(str::to_string))
        {
            Ok(uri) => uri,
            Err(message) => return rpc_error(id, -32602, &message),
        };
        let Some(session) = self.sessions.get(&uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        let slots = collect_semantic_slot_snapshots(session.document());
        let confirmed = slots
            .iter()
            .filter(|slot| slot.state.is_confirmed())
            .collect::<Vec<_>>();
        success(
            id,
            json!({
                "schema_version": LANGUAGE_SERVICE_SCHEMA_VERSION,
                "session": session.snapshot(),
                "decision_queue": session.diagnostics().decision_queue,
                "delegation_queue": session.diagnostics().delegation_queue,
                "queue_tree": session.diagnostics().queue_tree,
                "confirmed_intent": confirmed,
                "lock_challenges": session.lock_challenges()
            }),
        )
    }

    fn prepare_queue_batch(&mut self, id: Option<Value>, params: Value) -> Value {
        let Some(uri) = document_uri(&params) else {
            return rpc_error(id, -32602, "missing document URI");
        };
        let Some(authoritative_version) = self
            .sessions
            .get(uri)
            .map(DocumentSession::authoritative_version)
        else {
            return rpc_error(id, -32002, "document is not open");
        };
        let Some(base_version) = parse_base_version(&params) else {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-INVALID-EDIT",
                    "base_version is required and must be a positive integer",
                ),
            );
        };
        if base_version != authoritative_version {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-STALE-REVISION",
                    "base_version does not match the current authoritative revision",
                ),
            );
        }
        if params.get("actor").and_then(Value::as_str) != Some("human") {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-ACTOR-REQUIRED",
                    "queue batches may only be prepared by actor: human",
                ),
            );
        }
        let decoded = match protocol::decode::<protocol::QueueBatchRequest>(&params) {
            Ok(decoded) => decoded,
            Err(message) => {
                return success(
                    id,
                    wire_rejection(authoritative_version, "C-INVALID-EDIT", &message),
                );
            }
        };
        if decoded.actor != Actor::Human {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-ACTOR-REQUIRED",
                    "queue batches may only be prepared by actor: human",
                ),
            );
        }
        let target_text = decoded.target;
        let replacement = match target_text.as_str() {
            "none" => "",
            "?" => "?",
            "$" => "$",
            _ => {
                return success(
                    id,
                    wire_rejection(
                        authoritative_version,
                        "C-INVALID-EDIT",
                        "target must be one of none, ?, or $",
                    ),
                );
            }
        };
        if decoded.slot_ids.is_empty() {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-INVALID-EDIT",
                    "slot_ids must not be empty",
                ),
            );
        }
        let mut slot_ids = Vec::with_capacity(decoded.slot_ids.len());
        let mut unique = std::collections::HashSet::new();
        for slot in decoded.slot_ids {
            if !unique.insert(slot) {
                return success(
                    id,
                    wire_rejection(
                        authoritative_version,
                        "C-INVALID-EDIT",
                        "slot_ids must not contain duplicates",
                    ),
                );
            }
            slot_ids.push(crate::lossless::CommitmentSlotId(slot));
        }

        let Some(session) = self.sessions.get_mut(uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        let document = session.authoritative_document();
        let mut edits = Vec::with_capacity(slot_ids.len());
        for slot_id in &slot_ids {
            let Some(slot) = document
                .commitment_slot(*slot_id)
                .filter(|slot| slot.semantic_slot)
            else {
                return success(
                    id,
                    wire_rejection(
                        authoritative_version,
                        "C-INVALID-EDIT",
                        "every slot_id must identify a current authoritative semantic slot",
                    ),
                );
            };
            if slot.value.review_intent() == ReviewIntent::None {
                return success(
                    id,
                    wire_rejection(
                        authoritative_version,
                        "C-INVALID-TRANSITION",
                        "queue batches accept only current review or delegation slots",
                    ),
                );
            }
            if slot.value.lock_intent() == LockIntent::StrongLocked {
                return success(
                    id,
                    wire_rejection(
                        authoritative_version,
                        "C-STRONG-UNLOCK-REQUIRED",
                        "strong-lock-family slots cannot participate in a queue batch",
                    ),
                );
            }
            let Some(range) =
                text_range_for_span(document, session.authoritative_source(), slot.suffix_span)
            else {
                return success(
                    id,
                    wire_rejection(
                        authoritative_version,
                        "C-INVALID-EDIT",
                        "slot suffix could not be represented as a UTF-16 range",
                    ),
                );
            };
            edits.push(SessionTextEdit {
                range: Some(range),
                text: replacement.into(),
            });
        }

        let response = session.prepare_edit(DocumentEditRequest {
            base_version,
            actor: Actor::Human,
            edits: edits.clone(),
            authorization: HumanAuthorization::default(),
            unlock_tokens: Vec::new(),
            challenge_reason: None,
        });
        let mut value = serde_json::to_value(&response).unwrap_or(Value::Null);
        if response.accepted {
            value["target"] = json!(target_text);
            value["slot_ids"] = json!(slot_ids.iter().map(|slot| slot.0).collect::<Vec<_>>());
            value["workspace_edit"] = workspace_edit(uri, session, &edits);
        }
        success(id, value)
    }

    fn apply_document_edit(&mut self, id: Option<Value>, params: Value) -> Value {
        let Some(uri) = document_uri(&params) else {
            return rpc_error(id, -32602, "missing document URI");
        };
        let Some(authoritative_version) = self
            .sessions
            .get(uri)
            .map(DocumentSession::authoritative_version)
        else {
            return rpc_error(id, -32002, "document is not open");
        };
        let Some(actor) = params
            .get("actor")
            .and_then(Value::as_str)
            .and_then(parse_actor)
        else {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-ACTOR-REQUIRED",
                    "actor must be human or ai",
                ),
            );
        };
        let Some(base_version) = parse_base_version(&params) else {
            return success(
                id,
                wire_rejection(
                    authoritative_version,
                    "C-INVALID-EDIT",
                    "base_version is required and must be a positive integer",
                ),
            );
        };
        let decoded = match protocol::decode::<protocol::DocumentEditRequest>(&params) {
            Ok(decoded) => decoded,
            Err(message) => {
                return success(
                    id,
                    wire_rejection(authoritative_version, "C-INVALID-EDIT", &message),
                );
            }
        };
        let edits = decoded.edits;
        let authorization = decoded.authorization.into();
        let unlock_tokens = decoded.unlock_tokens;
        let challenge_reason = decoded.challenge_reason;
        let Some(session) = self.sessions.get_mut(uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        if edits.iter().any(|edit| edit.range.is_none()) {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-INVALID-EDIT",
                    "edits must contain valid range and text fields",
                ),
            );
        }
        let response = session.prepare_edit(DocumentEditRequest {
            base_version,
            actor,
            edits: edits.clone(),
            authorization,
            unlock_tokens,
            challenge_reason,
        });
        let mut value = serde_json::to_value(&response).unwrap_or(Value::Null);
        if response.accepted {
            value["workspace_edit"] = workspace_edit(uri, session, &edits);
        }
        success(id, value)
    }

    fn issue_unlock_token(&mut self, id: Option<Value>, params: Value) -> Value {
        let Some(uri) = document_uri(&params) else {
            return rpc_error(id, -32602, "missing document URI");
        };
        let Some(session) = self.sessions.get_mut(uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        let actor = params
            .get("actor")
            .and_then(Value::as_str)
            .and_then(parse_actor);
        let base_version = parse_base_version(&params);
        let slot = params
            .get("slot")
            .and_then(Value::as_u64)
            .and_then(|slot| u32::try_from(slot).ok());
        let Some(actor) = actor else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-ACTOR-REQUIRED",
                    "actor must be human",
                ),
            );
        };
        let Some(base_version) = base_version else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-INVALID-EDIT",
                    "base_version is required",
                ),
            );
        };
        let Some(slot) = slot else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-INVALID-EDIT",
                    "slot is required",
                ),
            );
        };
        let _decoded = match protocol::decode::<protocol::UnlockTokenRequest>(&params) {
            Ok(decoded) => decoded,
            Err(message) => {
                return success(
                    id,
                    wire_rejection(session.authoritative_version(), "C-INVALID-EDIT", &message),
                );
            }
        };
        match session.issue_unlock_token(
            base_version,
            actor,
            crate::lossless::CommitmentSlotId(slot),
        ) {
            Ok(token) => success(
                id,
                json!({
                    "schema_version": LANGUAGE_SERVICE_SCHEMA_VERSION,
                    "accepted": true,
                    "authoritative_version": session.authoritative_version(),
                    "token": token,
                    "violations": []
                }),
            ),
            Err(violation) => success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    &violation.code,
                    &violation.message,
                ),
            ),
        }
    }

    fn adopt_observed(&mut self, id: Option<Value>, params: Value) -> Value {
        let Some(uri) = document_uri(&params) else {
            return rpc_error(id, -32602, "missing document URI");
        };
        let actor = params
            .get("actor")
            .and_then(Value::as_str)
            .and_then(parse_actor);
        let Some(session) = self.sessions.get_mut(uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        let Some(base_version) = parse_base_version(&params) else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-INVALID-EDIT",
                    "base_version is required and must be a positive integer",
                ),
            );
        };
        let Some(actor) = actor else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-ACTOR-REQUIRED",
                    "actor must be human",
                ),
            );
        };
        let decoded = match protocol::decode::<protocol::AdoptObservedRequest>(&params) {
            Ok(decoded) => decoded,
            Err(message) => {
                return success(
                    id,
                    wire_rejection(session.authoritative_version(), "C-INVALID-EDIT", &message),
                );
            }
        };
        let authorization = decoded.authorization.into();
        let tokens = decoded.unlock_tokens;
        match session.adopt_observed(base_version, actor, authorization, &tokens) {
            Ok(()) => success(
                id,
                json!({
                    "schema_version": LANGUAGE_SERVICE_SCHEMA_VERSION,
                    "accepted": true,
                    "authoritative_version": session.authoritative_version(),
                    "session": session.snapshot(),
                    "violations": []
                }),
            ),
            Err(violations) => success(
                id,
                json!({
                    "schema_version": LANGUAGE_SERVICE_SCHEMA_VERSION,
                    "accepted": false,
                    "authoritative_version": session.authoritative_version(),
                    "violations": violations
                }),
            ),
        }
    }

    fn restore_authoritative(&mut self, id: Option<Value>, params: Value) -> Value {
        let Some(uri) = document_uri(&params) else {
            return rpc_error(id, -32602, "missing document URI");
        };
        let Some(session) = self.sessions.get_mut(uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        let Some(base_version) = parse_base_version(&params) else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-INVALID-EDIT",
                    "base_version is required and must be a positive integer",
                ),
            );
        };
        let actor = params
            .get("actor")
            .and_then(Value::as_str)
            .and_then(parse_actor);
        let Some(actor) = actor else {
            return success(
                id,
                wire_rejection(
                    session.authoritative_version(),
                    "C-ACTOR-REQUIRED",
                    "actor must be human",
                ),
            );
        };
        let _decoded = match protocol::decode::<protocol::RestoreAuthoritativeRequest>(&params) {
            Ok(decoded) => decoded,
            Err(message) => {
                return success(
                    id,
                    wire_rejection(session.authoritative_version(), "C-INVALID-EDIT", &message),
                );
            }
        };
        let restore_text = session.authoritative_source().to_string();
        let response = session.prepare_restore(base_version, actor);
        let mut value = serde_json::to_value(&response).unwrap_or(Value::Null);
        if response.accepted {
            value["workspace_edit"] = workspace_edit(
                uri,
                session,
                &[SessionTextEdit {
                    range: None,
                    text: restore_text,
                }],
            );
        }
        success(id, value)
    }

    fn slot_navigation(&self, id: Option<Value>, params: Value) -> Value {
        let decoded = match protocol::decode::<protocol::SlotNavigationRequest>(&params) {
            Ok(decoded) => decoded,
            Err(message) => return rpc_error(id, -32602, &message),
        };
        let Some(session) = self.sessions.get(&decoded.uri) else {
            return rpc_error(id, -32002, "document is not open");
        };
        success(
            id,
            json!({
                "schema_version": LANGUAGE_SERVICE_SCHEMA_VERSION,
                "targets": navigation_at_position(session.document(), source_position(decoded.position), ColumnEncoding::Utf16)
            }),
        )
    }

    fn publish_diagnostics(&self, uri: &str) -> Vec<Value> {
        let Some(session) = self.sessions.get(uri) else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for error in session.errors() {
            diagnostics.push(json!({
                "range": parser_error_range(session, error.line, error.col),
                "severity": 1,
                "code": error.code.to_string(),
                "source": "mimispec",
                "message": error.message
            }));
        }
        for diagnostic in &session.diagnostics().diagnostics {
            diagnostics.push(json!({
                "range": diagnostic.span.map_or_else(zero_range, |span| span_range(session, span)),
                "severity": severity_number(diagnostic.severity),
                "code": diagnostic.code.0,
                "source": "mimispec",
                "message": diagnostic.message,
                "data": { "class": format!("{:?}", diagnostic.class).to_lowercase() }
            }));
        }
        for violation in session.violations() {
            diagnostics.push(json!({
                "range": zero_range(),
                "severity": if session.mode() == CollaborationMode::Strict { 1 } else { 2 },
                "code": violation.code,
                "source": "mimispec-collaboration",
                "message": violation.message
            }));
        }
        vec![notification(
            "textDocument/publishDiagnostics",
            json!({ "uri": uri, "diagnostics": diagnostics }),
        )]
    }
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line
            .trim()
            .strip_prefix("Content-Length:")
            .and_then(|value| value.trim().parse::<usize>().ok())
        {
            content_length = Some(value);
        }
    }
    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)
}

fn success(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": JSON_RPC_VERSION, "id": id.unwrap_or(Value::Null), "result": result })
}

fn rpc_error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": JSON_RPC_VERSION, "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": JSON_RPC_VERSION, "method": method, "params": params })
}

fn document_uri(params: &Value) -> Option<&str> {
    params
        .pointer("/textDocument/uri")
        .or_else(|| params.get("uri"))
        .and_then(Value::as_str)
}

fn text_document_position(params: &Value) -> Option<(&str, TextPosition)> {
    let line = params
        .pointer("/position/line")
        .or_else(|| params.pointer("/range/start/line"))?;
    let character = params
        .pointer("/position/character")
        .or_else(|| params.pointer("/range/start/character"))?;
    Some((
        document_uri(params)?,
        TextPosition {
            line: u32::try_from(line.as_u64()?).ok()?,
            character: u32::try_from(character.as_u64()?).ok()?,
        },
    ))
}

fn source_position(position: TextPosition) -> SourcePosition {
    SourcePosition {
        line: position.line,
        column: position.character,
    }
}

fn parse_text_edit(value: &Value) -> Result<SessionTextEdit, &'static str> {
    let text = value
        .get("text")
        .or_else(|| value.get("newText"))
        .and_then(Value::as_str)
        .ok_or("contentChange is missing 'text' or 'newText' field")?
        .to_string();
    let range = match value.get("range") {
        None => None,
        Some(range) if range.is_null() => None,
        Some(range) => {
            let start_line = range
                .pointer("/start/line")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("range.start.line is missing or invalid")?;
            let start_character = range
                .pointer("/start/character")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("range.start.character is missing or invalid")?;
            let end_line = range
                .pointer("/end/line")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("range.end.line is missing or invalid")?;
            let end_character = range
                .pointer("/end/character")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("range.end.character is missing or invalid")?;
            Some(TextRange {
                start: TextPosition {
                    line: start_line,
                    character: start_character,
                },
                end: TextPosition {
                    line: end_line,
                    character: end_character,
                },
            })
        }
    };
    Ok(SessionTextEdit { range, text })
}

fn parse_base_version(params: &Value) -> Option<u64> {
    params
        .get("base_version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
}

fn parse_actor(actor: &str) -> Option<Actor> {
    match actor.to_ascii_lowercase().as_str() {
        "human" => Some(Actor::Human),
        "ai" => Some(Actor::Ai),
        _ => None,
    }
}

fn parse_commitment(value: &str) -> Option<Commitment> {
    match value {
        "" | "none" => Some(Commitment::None),
        "?" => Some(Commitment::Question),
        "??" => Some(Commitment::QuestionQuestion),
        "$" => Some(Commitment::Locked),
        "$?" => Some(Commitment::LockedQuestion),
        "$??" => Some(Commitment::LockedQuestionQuestion),
        "$$" => Some(Commitment::StrongLocked),
        "$$?" => Some(Commitment::StrongLockedQuestion),
        "$$??" => Some(Commitment::StrongLockedQuestionQuestion),
        _ => None,
    }
}

fn parse_mode(mode: &str) -> Option<CollaborationMode> {
    match mode.to_ascii_lowercase().as_str() {
        "advisory" => Some(CollaborationMode::Advisory),
        "strict" => Some(CollaborationMode::Strict),
        _ => None,
    }
}

fn span_range(session: &DocumentSession, span: ByteSpan) -> Value {
    let start = session.document().line_index().position(
        session.source(),
        span.start,
        ColumnEncoding::Utf16,
    );
    let end =
        session
            .document()
            .line_index()
            .position(session.source(), span.end, ColumnEncoding::Utf16);
    match (start, end) {
        (Some(start), Some(end)) => json!({
            "start": { "line": start.line, "character": start.column },
            "end": { "line": end.line, "character": end.column }
        }),
        _ => zero_range(),
    }
}

fn text_range_for_span(
    document: &crate::lossless::LosslessDocument,
    source: &str,
    span: ByteSpan,
) -> Option<TextRange> {
    let start = document
        .line_index()
        .position(source, span.start, ColumnEncoding::Utf16)?;
    let end = document
        .line_index()
        .position(source, span.end, ColumnEncoding::Utf16)?;
    Some(TextRange {
        start: TextPosition {
            line: start.line,
            character: start.column,
        },
        end: TextPosition {
            line: end.line,
            character: end.column,
        },
    })
}

fn zero_range() -> Value {
    json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } })
}

fn parser_error_range(session: &DocumentSession, line: usize, column: usize) -> Value {
    let line_index = line.saturating_sub(1) as u32;
    let scalar_column = column.saturating_sub(1);
    let line_text = physical_line(session.source(), line_index as usize).unwrap_or("");
    let byte_offset = line_text
        .char_indices()
        .nth(scalar_column)
        .map_or(line_text.len(), |(offset, _)| offset);
    let utf16 = line_text[..byte_offset].encode_utf16().count() as u32;
    json!({
        "start": { "line": line_index, "character": utf16 },
        "end": { "line": line_index, "character": utf16.saturating_add(1) }
    })
}

fn physical_line(source: &str, target: usize) -> Option<&str> {
    let bytes = source.as_bytes();
    let mut start = 0usize;
    let mut line = 0usize;
    loop {
        if start > bytes.len() {
            return None;
        }
        let mut end = start;
        while end < bytes.len() && !matches!(bytes[end], b'\r' | b'\n') {
            end += 1;
        }
        if line == target {
            return Some(&source[start..end]);
        }
        if end == bytes.len() {
            return None;
        }
        start = if bytes[end] == b'\r' && bytes.get(end + 1) == Some(&b'\n') {
            end + 2
        } else {
            end + 1
        };
        line += 1;
    }
}

fn severity_number(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Info => 3,
        Severity::Hint => 4,
    }
}

fn token_index(kind: SemanticTokenKind) -> u32 {
    match kind {
        SemanticTokenKind::ContextDesc => 0,
        SemanticTokenKind::ContextClause => 1,
        SemanticTokenKind::ContextModule => 2,
        SemanticTokenKind::ContextType => 3,
        SemanticTokenKind::ContextFlow => 4,
        SemanticTokenKind::ContextFunc => 5,
        SemanticTokenKind::ContextUi => 6,
        SemanticTokenKind::ContextSteps => 7,
        SemanticTokenKind::ContextField => 8,
        SemanticTokenKind::ContextFlowEntry => 9,
        SemanticTokenKind::ContextFlowArm => 10,
        SemanticTokenKind::ContextStep => 11,
        SemanticTokenKind::CommitmentOpen => 12,
        SemanticTokenKind::CommitmentContentReview => 13,
        SemanticTokenKind::CommitmentContentDelegated => 14,
        SemanticTokenKind::CommitmentLocked => 15,
        SemanticTokenKind::CommitmentLockReview => 16,
        SemanticTokenKind::CommitmentLockDelegated => 17,
        SemanticTokenKind::CommitmentStrongLocked => 18,
        SemanticTokenKind::CommitmentStrongLockReview => 19,
        SemanticTokenKind::CommitmentStrongLockDelegated => 20,
        SemanticTokenKind::RuleAttached => 21,
        SemanticTokenKind::RuleEnvironment => 22,
        SemanticTokenKind::RuleDropped => 23,
    }
}

fn token_priority(kind: SemanticTokenKind) -> u8 {
    match kind {
        SemanticTokenKind::RuleAttached
        | SemanticTokenKind::RuleEnvironment
        | SemanticTokenKind::RuleDropped => 3,
        SemanticTokenKind::CommitmentOpen
        | SemanticTokenKind::CommitmentContentReview
        | SemanticTokenKind::CommitmentContentDelegated
        | SemanticTokenKind::CommitmentLocked
        | SemanticTokenKind::CommitmentLockReview
        | SemanticTokenKind::CommitmentLockDelegated
        | SemanticTokenKind::CommitmentStrongLocked
        | SemanticTokenKind::CommitmentStrongLockReview
        | SemanticTokenKind::CommitmentStrongLockDelegated => 2,
        _ => 1,
    }
}

fn workspace_edit(uri: &str, session: &DocumentSession, edits: &[SessionTextEdit]) -> Value {
    let edits = edits
        .iter()
        .map(|edit| {
            let range = edit.range.map_or_else(
                || {
                    let end = session.document().line_index().position(
                        session.source(),
                        session.source().len() as u32,
                        ColumnEncoding::Utf16,
                    );
                    end.map_or_else(zero_range, |end| {
                        json!({ "start": { "line": 0, "character": 0 }, "end": { "line": end.line, "character": end.column } })
                    })
                },
                |range| json!({
                    "start": { "line": range.start.line, "character": range.start.character },
                    "end": { "line": range.end.line, "character": range.end.character }
                }),
            );
            json!({ "range": range, "newText": edit.text })
        })
        .collect::<Vec<_>>();
    let mut changes = Map::new();
    changes.insert(uri.into(), Value::Array(edits));
    json!({ "changes": changes })
}

fn wire_rejection(authoritative_version: u64, code: &str, message: &str) -> Value {
    json!({
        "schema_version": LANGUAGE_SERVICE_SCHEMA_VERSION,
        "accepted": false,
        "authoritative_version": authoritative_version,
        "candidate_hash": null,
        "transaction_id": null,
        "violations": [{ "code": code, "message": message }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frame(value: Value) -> Vec<u8> {
        let body = serde_json::to_vec(&value).unwrap();
        let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        framed.extend(body);
        framed
    }

    fn decode_frames(bytes: &[u8]) -> Vec<Value> {
        let mut reader = BufReader::new(Cursor::new(bytes));
        let mut values = Vec::new();
        while let Some(value) = read_message(&mut reader).unwrap() {
            values.push(value);
        }
        values
    }

    #[test]
    fn stdio_transcript_initializes_opens_hovers_and_shuts_down() {
        let mut input = Vec::new();
        for message in [
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "initializationOptions": { "collaborationMode": "strict" } } }),
            json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
            json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": "file:///demo.mms", "version": 1, "languageId": "mimispec", "text": "func Pay$:\n    steps:\n        charge payment\n" } } }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover", "params": { "textDocument": { "uri": "file:///demo.mms" }, "position": { "line": 0, "character": 8 } } }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "mimispec/documentSnapshot", "params": { "textDocument": { "uri": "file:///demo.mms" } } }),
            json!({ "jsonrpc": "2.0", "id": 4, "method": "shutdown", "params": null }),
            json!({ "jsonrpc": "2.0", "method": "exit", "params": null }),
        ] {
            input.extend(frame(message));
        }
        let mut output = Vec::new();
        run_with_io(&mut BufReader::new(Cursor::new(input)), &mut output).unwrap();
        let messages = decode_frames(&output);
        assert!(messages.iter().any(|message| message["id"] == 1
            && message["result"]["capabilities"]["positionEncoding"] == "utf-16"));
        assert!(messages.iter().any(|message| message["id"] == 2
            && message["result"]["contents"]["value"]
                .as_str()
                .is_some_and(|value| value.contains("effective_lock"))));
        assert!(messages.iter().any(|message| message["id"] == 3
            && message["result"]["schema_version"] == LANGUAGE_SERVICE_SCHEMA_VERSION));
        assert!(messages
            .iter()
            .any(|message| { message["method"] == "textDocument/publishDiagnostics" }));
    }

    #[test]
    fn custom_edits_reject_malformed_ranges_and_restore_requires_human() {
        let uri = "file:///strict.mms";
        let responses = run_json_transcript(&[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "initializationOptions": { "collaborationMode": "strict" } } }),
            json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
            json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": uri, "version": 1, "languageId": "mimispec", "text": "desc \"trusted\"\n" } } }),
            json!({ "jsonrpc": "2.0", "method": "textDocument/didChange", "params": { "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": "desc \"external\"\n" }] } }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "mimispec/applyDocumentEdit", "params": { "uri": uri, "base_version": 1, "actor": "human", "edits": [{ "text": "desc \"bad\"\n" }], "authorization": { "modify_protected": false }, "unlock_tokens": [] } }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "mimispec/restoreAuthoritativeRevision", "params": { "uri": uri, "base_version": 1 } }),
            json!({ "jsonrpc": "2.0", "id": 4, "method": "mimispec/restoreAuthoritativeRevision", "params": { "uri": uri, "base_version": 1, "actor": "human" } }),
            json!({ "jsonrpc": "2.0", "id": 5, "method": "mimispec/adoptObservedRevision", "params": { "uri": uri, "base_version": 1, "authorization": { "modify_protected": false }, "unlock_tokens": [] } }),
            json!({ "jsonrpc": "2.0", "id": 6, "method": "shutdown", "params": null }),
            json!({ "jsonrpc": "2.0", "method": "exit", "params": null }),
        ])
        .unwrap();
        let response = |id| {
            responses
                .iter()
                .find(|message| message["id"] == id)
                .unwrap()
        };

        assert_eq!(
            response(2)["result"]["violations"][0]["code"],
            "C-INVALID-EDIT"
        );
        assert_eq!(
            response(3)["result"]["violations"][0]["code"],
            "C-ACTOR-REQUIRED"
        );
        assert_eq!(response(4)["result"]["accepted"], true);
        assert!(response(4)["result"]["workspace_edit"].is_object());
        assert_eq!(
            response(5)["result"]["violations"][0]["code"],
            "C-ACTOR-REQUIRED"
        );
    }

    #[test]
    fn custom_requests_reject_missing_frozen_fields() {
        let uri = "file:///required-fields.mms";
        let replacement = json!({ "range": null, "text": "desc \"updated\"\n" });
        let responses = run_json_transcript(&[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
            json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": uri, "version": 1, "languageId": "mimispec", "text": "desc \"draft\"\n" } } }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "mimispec/applyDocumentEdit", "params": { "uri": uri, "base_version": 1, "actor": "human", "edits": [replacement.clone()], "unlock_tokens": [] } }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "mimispec/applyDocumentEdit", "params": { "uri": uri, "base_version": 1, "actor": "human", "edits": [replacement.clone()], "authorization": { "modify_protected": false } } }),
            json!({ "jsonrpc": "2.0", "id": 4, "method": "mimispec/applyDocumentEdit", "params": { "uri": uri, "actor": "human", "edits": [replacement], "authorization": { "modify_protected": false }, "unlock_tokens": [] } }),
            json!({ "jsonrpc": "2.0", "id": 5, "method": "mimispec/adoptObservedRevision", "params": { "uri": uri, "base_version": 1, "actor": "human" } }),
            json!({ "jsonrpc": "2.0", "id": 6, "method": "mimispec/adoptObservedRevision", "params": { "uri": uri, "base_version": 1, "actor": "human", "authorization": { "modify_protected": false } } }),
            json!({ "jsonrpc": "2.0", "id": 7, "method": "mimispec/restoreAuthoritativeRevision", "params": { "uri": uri, "actor": "human" } }),
            json!({ "jsonrpc": "2.0", "id": 8, "method": "mimispec/issueUnlockToken", "params": { "uri": uri, "actor": "human", "slot": 0 } }),
            json!({ "jsonrpc": "2.0", "id": 9, "method": "mimispec/applyDocumentEdit", "params": { "uri": uri, "base_version": 1, "actor": "human", "edits": [{ "range": null, "text": "desc \"updated\"\n" }], "authorization": { "modify_protected": false }, "unlock_tokens": [], "admin": true } }),
            json!({ "jsonrpc": "2.0", "id": 10, "method": "shutdown", "params": null }),
            json!({ "jsonrpc": "2.0", "method": "exit", "params": null }),
        ])
        .unwrap();

        for id in 2..=9 {
            let response = responses
                .iter()
                .find(|message| message["id"] == id)
                .unwrap();
            assert_eq!(
                response["result"]["violations"][0]["code"], "C-INVALID-EDIT",
                "request {id} unexpectedly passed frozen-field validation: {response}"
            );
        }
    }

    #[test]
    fn syntax_code_actions_return_standard_workspace_edits() {
        let uri = "file:///action-recovery.mms";
        let mut server = LanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri,
                "version": 1,
                "languageId": "mimispec",
                "text": "func Work:\n    steps:\n        bind and listen on\n"
            }}
        }));
        let response = server.handle(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "textDocument/codeAction",
            "params": {
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": 2, "character": 10 },
                    "end": { "line": 2, "character": 10 }
                },
                "position": { "line": 2, "character": 10 },
                "context": { "diagnostics": [] }
            }
        }));
        let actions = response[0]["result"].as_array().unwrap();
        let syntax = actions
            .iter()
            .filter(|action| action.pointer("/data/syntaxRecovery") == Some(&Value::Bool(true)))
            .collect::<Vec<_>>();
        assert_eq!(syntax.len(), 2, "{actions:?}");
        assert!(syntax.iter().all(|action| action.get("edit").is_none()));

        let resolved = server.handle(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "codeAction/resolve",
            "params": syntax[0]
        }));
        assert!(resolved[0]["result"]["edit"]["changes"][uri]
            .as_array()
            .is_some_and(|edits| edits.len() == 1));
        assert!(server
            .sessions
            .get(uri)
            .unwrap()
            .pending_transaction_id()
            .is_some());
    }

    #[test]
    fn author_journey_recovers_syntax_reviews_queue_and_confirms_batch() {
        let uri = "file:///author-journey.mms";
        let source =
            "desc?? \"draft service\"\nfunc Work:\n    steps:\n        bind and listen on\n";
        let mut server = LanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "version": 1, "languageId": "mimispec", "text": source
            }}
        }));

        let listed = server.handle(json!({
            "jsonrpc": "2.0", "id": 1, "method": "textDocument/codeAction",
            "params": {
                "textDocument": { "uri": uri },
                "range": { "start": { "line": 3, "character": 10 }, "end": { "line": 3, "character": 10 } },
                "context": { "diagnostics": [] }
            }
        }));
        let action = listed[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action.pointer("/data/syntaxRecovery") == Some(&Value::Bool(true)))
            .unwrap()
            .clone();
        let resolved = server.handle(json!({
            "jsonrpc": "2.0", "id": 2, "method": "codeAction/resolve", "params": action
        }));
        let replacement = resolved[0]["result"]["edit"]["changes"][uri][0]["newText"]
            .as_str()
            .unwrap();
        let corrected = source.replacen("bind and listen on", replacement, 1);
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": corrected.clone() }]
            }
        }));

        let (base_version, delegated_slot) = {
            let session = server.sessions.get(uri).unwrap();
            assert!(session.errors().is_empty());
            assert!(session.pending_transaction_id().is_none());
            (
                session.authoritative_version(),
                session.diagnostics().delegation_queue[0].slot.0,
            )
        };
        let batched = server.handle(json!({
            "jsonrpc": "2.0", "id": 3, "method": "mimispec/prepareQueueBatch",
            "params": {
                "uri": uri, "base_version": base_version, "actor": "human",
                "slot_ids": [delegated_slot], "target": "?"
            }
        }));
        assert_eq!(batched[0]["result"]["accepted"], true, "{batched:?}");
        assert!(batched[0]["result"]["workspace_edit"]["changes"][uri].is_array());

        let reviewed = corrected.replacen("desc??", "desc?", 1);
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 3 },
                "contentChanges": [{ "text": reviewed }]
            }
        }));
        let snapshot = server.handle(json!({
            "jsonrpc": "2.0", "id": 4, "method": "mimispec/documentSnapshot",
            "params": { "uri": uri }
        }));
        assert_eq!(
            snapshot[0]["result"]["delegation_queue"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            snapshot[0]["result"]["decision_queue"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let session = server.sessions.get(uri).unwrap();
        assert_eq!(session.authoritative_version(), 3);
        assert!(session.pending_transaction_id().is_none());
    }

    #[test]
    fn actor_code_action_resolves_through_a_confirmable_transaction() {
        let uri = "file:///actor-action.mms";
        let mut server = LanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "version": 1, "languageId": "mimispec",
                "text": "desc?? \"delegated\"\n"
            }}
        }));
        let listed = server.handle(json!({
            "jsonrpc": "2.0", "id": 1, "method": "textDocument/codeAction",
            "params": {
                "textDocument": { "uri": uri },
                "range": { "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 2 } },
                "context": { "diagnostics": [] }
            }
        }));
        let action = listed[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action.pointer("/data/to") == Some(&json!("?")))
            .unwrap()
            .clone();
        let resolved = server.handle(json!({
            "jsonrpc": "2.0", "id": 2, "method": "codeAction/resolve", "params": action
        }));
        assert!(resolved[0]["result"]["edit"]["changes"][uri].is_array());
        assert!(server
            .sessions
            .get(uri)
            .unwrap()
            .pending_transaction_id()
            .is_some());

        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": "desc? \"delegated\"\n" }]
            }
        }));
        let session = server.sessions.get(uri).unwrap();
        assert_eq!(session.authoritative_source(), "desc? \"delegated\"\n");
        assert!(session.pending_transaction_id().is_none());
    }

    #[test]
    fn malformed_or_empty_did_change_preserves_pending_transaction() {
        let uri = "file:///malformed-change.mms";
        let mut server = LanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "version": 1, "languageId": "mimispec",
                "text": "desc?? \"delegated\"\n"
            }}
        }));
        let pending = {
            let session = server.sessions.get_mut(uri).unwrap();
            let response = session.prepare_edit(DocumentEditRequest {
                base_version: 1,
                actor: Actor::Ai,
                edits: vec![SessionTextEdit {
                    range: None,
                    text: "desc? \"delegated\"\n".into(),
                }],
                authorization: HumanAuthorization::default(),
                unlock_tokens: Vec::new(),
                challenge_reason: None,
            });
            assert!(response.accepted, "{:?}", response.violations);
            response.transaction_id.unwrap()
        };

        for params in [
            json!({ "textDocument": { "uri": uri, "version": 2 } }),
            json!({
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": []
            }),
        ] {
            server.handle(json!({
                "jsonrpc": "2.0", "method": "textDocument/didChange", "params": params
            }));
            let session = server.sessions.get(uri).unwrap();
            assert_eq!(session.version(), 1);
            assert_eq!(session.pending_transaction_id(), Some(pending.as_str()));
            assert!(session
                .violations()
                .iter()
                .any(|violation| violation.code == "C-INVALID-EDIT"));
        }
    }

    #[test]
    fn queue_batch_is_human_atomic_slot_precise_and_confirmable() {
        let uri = "file:///队列.mms";
        let source = "desc?? \"需要 AI 完善\"\nfunc? Work: ...\nrule$$? \"强锁审阅\"\n";
        let mut server = LanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "version": 1, "languageId": "mimispec", "text": source
            }}
        }));
        let session = server.sessions.get(uri).unwrap();
        let ordinary = session
            .diagnostics()
            .decision_queue
            .iter()
            .chain(&session.diagnostics().delegation_queue)
            .filter(|item| item.state.lock_intent() != LockIntent::StrongLocked)
            .map(|item| item.slot.0)
            .collect::<Vec<_>>();
        let strong = session
            .diagnostics()
            .decision_queue
            .iter()
            .find(|item| item.state.lock_intent() == LockIntent::StrongLocked)
            .unwrap()
            .slot
            .0;
        assert_eq!(ordinary.len(), 2);

        let request = |id: u64, actor: &str, base: u64, slots: Vec<u32>| {
            json!({
                "jsonrpc": "2.0", "id": id, "method": "mimispec/prepareQueueBatch",
                "params": {
                    "uri": uri, "base_version": base, "actor": actor,
                    "slot_ids": slots, "target": "$"
                }
            })
        };
        let stale = server.handle(request(1, "human", 99, ordinary.clone()));
        assert_eq!(
            stale[0]["result"]["violations"][0]["code"],
            "C-STALE-REVISION"
        );
        let ai = server.handle(request(2, "ai", 1, ordinary.clone()));
        assert_eq!(ai[0]["result"]["violations"][0]["code"], "C-ACTOR-REQUIRED");
        let duplicate = server.handle(request(3, "human", 1, vec![ordinary[0], ordinary[0]]));
        assert_eq!(duplicate[0]["result"]["accepted"], false);
        let unknown = server.handle(request(4, "human", 1, vec![u32::MAX]));
        assert_eq!(unknown[0]["result"]["accepted"], false);
        let mixed = server.handle(request(5, "human", 1, vec![ordinary[0], strong]));
        assert_eq!(
            mixed[0]["result"]["violations"][0]["code"],
            "C-STRONG-UNLOCK-REQUIRED"
        );
        assert!(server
            .sessions
            .get(uri)
            .unwrap()
            .pending_transaction_id()
            .is_none());

        let accepted = server.handle(request(6, "human", 1, ordinary.clone()));
        assert_eq!(accepted[0]["result"]["accepted"], true, "{accepted:?}");
        assert!(accepted[0]["result"]["transaction_id"].is_string());
        assert_eq!(
            accepted[0]["result"]["workspace_edit"]["changes"][uri]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let confirmed_source = "desc$ \"需要 AI 完善\"\nfunc$ Work: ...\nrule$$? \"强锁审阅\"\n";
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": confirmed_source }]
            }
        }));
        let session = server.sessions.get(uri).unwrap();
        assert_eq!(session.authoritative_version(), 2);
        assert!(session.pending_transaction_id().is_none());
        assert_eq!(session.authoritative_source(), confirmed_source);
    }
}
