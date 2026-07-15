use std::ops::Range;
use std::sync::Arc;

use serde::Serialize;

use crate::ast::{Commitment, File};
use crate::error::ParseError;
use crate::lexer::{Token, TokenKind};
use crate::parser::{
    RecordedCommitmentSlot, RecordedNodeKind, RecordedRuleDecision, RecordedRuleOccurrence,
    RecordedSlotKind, RecordedSourceNode,
};

/// Half-open UTF-8 byte range within one immutable document revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ByteSpan {
    pub start: u32,
    pub end: u32,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: u32::try_from(start).expect("MimiSpec documents must be smaller than 4 GiB"),
            end: u32::try_from(end).expect("MimiSpec documents must be smaller than 4 GiB"),
        }
    }

    pub fn as_range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NewlineKind {
    Lf,
    CrLf,
    Cr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePieceKind {
    Word,
    Number,
    String,
    Symbol,
    Whitespace,
    LineComment,
    Newline(NewlineKind),
    Unknown,
}

impl SourcePieceKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::Newline(_)
        )
    }
}

/// One exact, non-overlapping piece of the original source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourcePiece {
    pub kind: SourcePieceKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentAnchorKind {
    Keyword,
    Identifier,
    String,
    Value,
}

/// An explicit suffix occurrence found in source text.
///
/// This first lossless layer records source syntax. `semantic_slot` remains false
/// until parser recording proves that the suffix was consumed as commitment rather
/// than as a free-form action symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommitmentSlotSyntax {
    pub anchor_kind: CommitmentAnchorKind,
    pub anchor_span: ByteSpan,
    pub suffix_span: ByteSpan,
    pub full_span: ByteSpan,
    pub value: Commitment,
    pub adjacent: bool,
    pub semantic_slot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BlankLine {
    pub span: ByteSpan,
    pub newline: NewlineKind,
}

/// A physical paragraph boundary caused by one or more empty lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParagraphBreak {
    pub span: ByteSpan,
    pub blank_lines: Vec<BlankLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAttachmentSyntaxKind {
    AttachedCandidate,
    Environment,
}

/// Source-derived rule group. Stable semantic node IDs are added by parser
/// recording; this layer still provides exact group and candidate target spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleGroupSyntax {
    pub span: ByteSpan,
    pub rule_spans: Vec<ByteSpan>,
    pub indentation: u32,
    pub attachment: RuleAttachmentSyntaxKind,
    pub target_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct SourceNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceNodeKind {
    Module,
    TypeDef,
    Flow,
    Func,
    Ui,
    Steps,
    Expr,
    UiNode,
    Placeholder,
    Field,
    FlowEntry,
    FlowArm,
    Step,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct CommentId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentAttachment {
    /// Comment sits on the same physical line after non-trivia content.
    Trailing,
    /// Full-line comment immediately precedes a node without a blank line between.
    Leading,
    /// No stable structural target in the current revision.
    Free,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommentOccurrence {
    pub id: CommentId,
    pub span: ByteSpan,
    pub attachment: CommentAttachment,
    pub target: Option<SourceNodeId>,
    pub target_anchor: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceNodeSpans {
    /// The node's syntax tokens, excluding trailing layout tokens.
    pub core: ByteSpan,
    /// The first physical header line, excluding its newline.
    pub header: ByteSpan,
    /// Core plus an attached rule prelude when one is present.
    pub full: ByteSpan,
}

/// A revision-local semantic Fragment origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceNode {
    pub id: SourceNodeId,
    pub kind: SourceNodeKind,
    pub spans: SourceNodeSpans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct RuleOccurrenceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAttachment {
    Attached,
    Environment,
    DroppedByRecovery,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuleOccurrence {
    pub id: RuleOccurrenceId,
    pub span: ByteSpan,
    pub attachment: RuleAttachment,
    pub target: Option<SourceNodeId>,
    pub target_anchor: Option<ByteSpan>,
    pub scope_anchor: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnEncoding {
    Utf8Bytes,
    UnicodeScalar,
    Utf16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourcePosition {
    /// Zero-based line.
    pub line: u32,
    /// Zero-based column in the requested encoding.
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(u32::try_from(index + 1).expect("document too large"));
            }
        }
        Self { starts }
    }

    pub fn position(
        &self,
        source: &str,
        offset: u32,
        encoding: ColumnEncoding,
    ) -> Option<SourcePosition> {
        let offset = offset as usize;
        if offset > source.len() || !source.is_char_boundary(offset) {
            return None;
        }
        let line = self
            .starts
            .partition_point(|start| *start as usize <= offset)
            .saturating_sub(1);
        let line_start = self.starts[line] as usize;
        let prefix = &source[line_start..offset];
        let column = match encoding {
            ColumnEncoding::Utf8Bytes => prefix.len(),
            ColumnEncoding::UnicodeScalar => prefix.chars().count(),
            ColumnEncoding::Utf16 => prefix.encode_utf16().count(),
        };
        Some(SourcePosition {
            line: u32::try_from(line).ok()?,
            column: u32::try_from(column).ok()?,
        })
    }

    pub fn offset(
        &self,
        source: &str,
        position: SourcePosition,
        encoding: ColumnEncoding,
    ) -> Option<u32> {
        let line_start = *self.starts.get(position.line as usize)? as usize;
        let line_end = self
            .starts
            .get(position.line as usize + 1)
            .map_or(source.len(), |start| *start as usize);
        let line = &source[line_start..line_end];
        let target = position.column as usize;
        let relative = match encoding {
            ColumnEncoding::Utf8Bytes => {
                (target <= line.len() && line.is_char_boundary(target)).then_some(target)?
            }
            ColumnEncoding::UnicodeScalar => offset_by_units(line, target, |_| 1)?,
            ColumnEncoding::Utf16 => offset_by_units(line, target, char::len_utf16)?,
        };
        u32::try_from(line_start + relative).ok()
    }
}

fn offset_by_units(source: &str, target: usize, units: impl Fn(char) -> usize) -> Option<usize> {
    if target == 0 {
        return Some(0);
    }
    let mut consumed = 0;
    for (index, ch) in source.char_indices() {
        consumed += units(ch);
        if consumed == target {
            return Some(index + ch.len_utf8());
        }
        if consumed > target {
            return None;
        }
    }
    (consumed == target).then_some(source.len())
}

/// Opt-in document layer that keeps the original source beside the semantic AST.
#[derive(Debug, Clone)]
pub struct LosslessDocument {
    source: Arc<str>,
    semantic: File,
    pieces: Vec<SourcePiece>,
    commitment_slots: Vec<CommitmentSlotSyntax>,
    paragraph_breaks: Vec<ParagraphBreak>,
    rule_groups: Vec<RuleGroupSyntax>,
    nodes: Vec<SourceNode>,
    rules: Vec<RuleOccurrence>,
    comments: Vec<CommentOccurrence>,
    line_index: LineIndex,
}

impl LosslessDocument {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn semantic(&self) -> &File {
        &self.semantic
    }

    pub fn pieces(&self) -> &[SourcePiece] {
        &self.pieces
    }

    pub fn commitment_slots(&self) -> &[CommitmentSlotSyntax] {
        &self.commitment_slots
    }

    pub fn paragraph_breaks(&self) -> &[ParagraphBreak] {
        &self.paragraph_breaks
    }

    pub fn rule_groups(&self) -> &[RuleGroupSyntax] {
        &self.rule_groups
    }

    pub fn nodes(&self) -> &[SourceNode] {
        &self.nodes
    }

    pub fn rules(&self) -> &[RuleOccurrence] {
        &self.rules
    }

    pub fn rule(&self, id: RuleOccurrenceId) -> Option<&RuleOccurrence> {
        self.rules.get(id.0 as usize).filter(|rule| rule.id == id)
    }

    pub fn comments(&self) -> &[CommentOccurrence] {
        &self.comments
    }

    pub fn comment(&self, id: CommentId) -> Option<&CommentOccurrence> {
        self.comments
            .get(id.0 as usize)
            .filter(|comment| comment.id == id)
    }

    pub fn node(&self, id: SourceNodeId) -> Option<&SourceNode> {
        self.nodes.get(id.0 as usize).filter(|node| node.id == id)
    }

    pub fn movable_span(&self, id: SourceNodeId) -> Option<ByteSpan> {
        self.node(id).map(|node| node.spans.full)
    }

    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    pub fn text(&self, span: ByteSpan) -> Option<&str> {
        self.source.get(span.as_range())
    }

    /// Exact rendering of the current immutable revision.
    pub fn render_lossless(&self) -> &str {
        &self.source
    }
}

#[derive(Debug, Clone)]
pub struct LosslessParseResult {
    pub document: LosslessDocument,
    pub errors: Vec<ParseError>,
}

pub(crate) fn build_document(
    source: Arc<str>,
    semantic: File,
    tokens: &[Token],
    recorded_commitments: &[RecordedCommitmentSlot],
    recorded_nodes: &[RecordedSourceNode],
    recorded_rules: &[RecordedRuleOccurrence],
) -> LosslessDocument {
    assert!(u32::try_from(source.len()).is_ok(), "document too large");
    let pieces = scan_source(&source);
    let line_index = LineIndex::new(&source);
    let commitment_slots = if recorded_commitments.is_empty() {
        scan_commitment_slots(&source, &pieces)
    } else {
        map_recorded_commitments(&source, &pieces, &line_index, tokens, recorded_commitments)
    };
    let paragraph_breaks = scan_paragraph_breaks(&source, &pieces);
    let rule_groups = scan_rule_groups(&source);
    let nodes = map_recorded_nodes(
        &source,
        &pieces,
        &line_index,
        tokens,
        recorded_nodes,
        &rule_groups,
    );
    let rules = map_recorded_rules(
        &source,
        &pieces,
        &line_index,
        tokens,
        recorded_rules,
        &nodes,
    );
    let comments = attach_comments(&source, &pieces, &nodes, &paragraph_breaks);
    LosslessDocument {
        source,
        semantic,
        pieces,
        commitment_slots,
        paragraph_breaks,
        rule_groups,
        nodes,
        rules,
        comments,
        line_index,
    }
}

fn scan_source(source: &str) -> Vec<SourcePiece> {
    let mut pieces = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        let rest = &source[offset..];
        let bytes = rest.as_bytes();
        let (kind, len) = if rest.starts_with("\r\n") {
            (SourcePieceKind::Newline(NewlineKind::CrLf), 2)
        } else if bytes[0] == b'\n' {
            (SourcePieceKind::Newline(NewlineKind::Lf), 1)
        } else if bytes[0] == b'\r' {
            (SourcePieceKind::Newline(NewlineKind::Cr), 1)
        } else if rest.starts_with("//") {
            let len = rest.find(['\r', '\n']).unwrap_or(rest.len());
            (SourcePieceKind::LineComment, len)
        } else if bytes[0] == b' ' || bytes[0] == b'\t' {
            let len = rest
                .bytes()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count();
            (SourcePieceKind::Whitespace, len)
        } else if bytes[0] == b'"' {
            let mut escaped = false;
            let mut end = 1;
            for (relative, ch) in rest[1..].char_indices() {
                end = 1 + relative + ch.len_utf8();
                if matches!(ch, '\r' | '\n') {
                    break;
                }
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    break;
                }
            }
            (SourcePieceKind::String, end)
        } else {
            let ch = rest.chars().next().expect("non-empty source remainder");
            if is_word_start(ch) {
                let len = rest
                    .char_indices()
                    .take_while(|(_, current)| is_word_continue(*current))
                    .map(|(index, current)| index + current.len_utf8())
                    .last()
                    .unwrap_or(ch.len_utf8());
                (SourcePieceKind::Word, len)
            } else if ch.is_ascii_digit() {
                let len = scan_number_len(rest);
                (SourcePieceKind::Number, len)
            } else if let Some(symbol) = longest_symbol(rest) {
                (SourcePieceKind::Symbol, symbol.len())
            } else {
                (SourcePieceKind::Unknown, ch.len_utf8())
            }
        };
        let end = offset + len.max(1);
        pieces.push(SourcePiece {
            kind,
            span: ByteSpan::new(offset, end),
        });
        offset = end;
    }
    pieces
}

fn scan_number_len(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if bytes.get(index) == Some(&b'.') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let exponent = index;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let digits = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if digits == index {
            index = exponent;
        }
    }
    index
}

fn longest_symbol(source: &str) -> Option<&'static str> {
    const SYMBOLS: [&str; 29] = [
        ">>>", "...", "??", "$$", "**", "!=", "<=", ">=", "<<", ">>", "==", ":", ",", "|", "(",
        ")", "[", "]", "=", ".", "+", "-", "*", "/", "&", "^", "~", "?", "$",
    ];
    SYMBOLS
        .into_iter()
        .find(|symbol| source.starts_with(symbol))
}

fn scan_commitment_slots(source: &str, pieces: &[SourcePiece]) -> Vec<CommitmentSlotSyntax> {
    let mut slots = Vec::new();
    let mut index = 0;
    while index < pieces.len() {
        let Some((value, consumed)) = suffix_at(source, pieces, index) else {
            index += 1;
            continue;
        };
        let Some(anchor_index) = previous_anchor(pieces, index) else {
            index += consumed;
            continue;
        };
        let anchor = pieces[anchor_index];
        let anchor_kind = match anchor.kind {
            SourcePieceKind::Word => CommitmentAnchorKind::Identifier,
            SourcePieceKind::String => CommitmentAnchorKind::String,
            SourcePieceKind::Symbol => CommitmentAnchorKind::Keyword,
            _ => {
                index += consumed;
                continue;
            }
        };
        let suffix_end = pieces[index + consumed - 1].span.end;
        slots.push(CommitmentSlotSyntax {
            anchor_kind,
            anchor_span: anchor.span,
            suffix_span: ByteSpan {
                start: pieces[index].span.start,
                end: suffix_end,
            },
            full_span: ByteSpan {
                start: anchor.span.start,
                end: suffix_end,
            },
            value,
            adjacent: anchor.span.end == pieces[index].span.start,
            semantic_slot: false,
        });
        index += consumed;
    }
    slots
}

fn map_recorded_commitments(
    source: &str,
    pieces: &[SourcePiece],
    line_index: &LineIndex,
    tokens: &[Token],
    recorded: &[RecordedCommitmentSlot],
) -> Vec<CommitmentSlotSyntax> {
    recorded
        .iter()
        .filter_map(|slot| {
            let anchor_token = tokens.get(slot.anchor_token)?;
            let anchor_offset = token_offset(source, line_index, anchor_token)?;
            let anchor_piece = pieces.iter().find(|piece| {
                piece.span.start as usize <= anchor_offset
                    && anchor_offset < piece.span.end as usize
                    && !piece.kind.is_trivia()
            })?;
            let suffix_span = if slot.suffix_tokens.is_empty() {
                ByteSpan {
                    start: anchor_piece.span.end,
                    end: anchor_piece.span.end,
                }
            } else {
                let first = tokens.get(slot.suffix_tokens.start)?;
                let last = tokens.get(slot.suffix_tokens.end.checked_sub(1)?)?;
                let first_offset = token_offset(source, line_index, first)?;
                let last_offset = token_offset(source, line_index, last)?;
                let first_piece = pieces.iter().find(|piece| {
                    piece.span.start as usize <= first_offset
                        && first_offset < piece.span.end as usize
                })?;
                let last_piece = pieces.iter().find(|piece| {
                    piece.span.start as usize <= last_offset
                        && last_offset < piece.span.end as usize
                })?;
                ByteSpan {
                    start: first_piece.span.start,
                    end: last_piece.span.end,
                }
            };
            Some(CommitmentSlotSyntax {
                anchor_kind: match slot.kind {
                    RecordedSlotKind::Keyword => CommitmentAnchorKind::Keyword,
                    RecordedSlotKind::Identifier => CommitmentAnchorKind::Identifier,
                    RecordedSlotKind::String => CommitmentAnchorKind::String,
                    RecordedSlotKind::Value => CommitmentAnchorKind::Value,
                },
                anchor_span: anchor_piece.span,
                suffix_span,
                full_span: ByteSpan {
                    start: anchor_piece.span.start,
                    end: suffix_span.end,
                },
                value: slot.value,
                adjacent: anchor_piece.span.end == suffix_span.start,
                semantic_slot: true,
            })
        })
        .collect()
}

fn token_offset(source: &str, line_index: &LineIndex, token: &Token) -> Option<usize> {
    let position = SourcePosition {
        line: u32::try_from(token.line.checked_sub(1)?).ok()?,
        column: u32::try_from(token.col.checked_sub(1)?).ok()?,
    };
    line_index
        .offset(source, position, ColumnEncoding::UnicodeScalar)
        .map(|offset| offset as usize)
}

fn map_recorded_nodes(
    source: &str,
    pieces: &[SourcePiece],
    line_index: &LineIndex,
    tokens: &[Token],
    recorded: &[RecordedSourceNode],
    rule_groups: &[RuleGroupSyntax],
) -> Vec<SourceNode> {
    recorded
        .iter()
        .enumerate()
        .filter_map(|(id, node)| {
            let concrete = node.tokens.clone().filter_map(|token_index| {
                let token = tokens.get(token_index)?;
                if matches!(
                    token.kind,
                    TokenKind::Indent | TokenKind::Dedent | TokenKind::Newline | TokenKind::Eof
                ) {
                    return None;
                }
                let offset = token_offset(source, line_index, token)?;
                pieces.iter().find(|piece| {
                    piece.span.start as usize <= offset
                        && offset < piece.span.end as usize
                        && !piece.kind.is_trivia()
                })
            });
            let concrete = concrete.collect::<Vec<_>>();
            let first = concrete.first()?;
            let last = concrete.last()?;
            let core = ByteSpan {
                start: first.span.start,
                end: last.span.end,
            };
            let header_end = source[first.span.start as usize..]
                .find(['\r', '\n'])
                .map_or(source.len(), |relative| {
                    first.span.start as usize + relative
                });
            let header = ByteSpan::new(first.span.start as usize, header_end);
            let prelude = rule_groups.iter().find(|group| {
                group.attachment == RuleAttachmentSyntaxKind::AttachedCandidate
                    && group
                        .target_span
                        .is_some_and(|target| target.start == header.start)
            });
            let full = ByteSpan {
                start: prelude.map_or(core.start, |group| group.span.start),
                end: core.end,
            };
            Some(SourceNode {
                id: SourceNodeId(u32::try_from(id).ok()?),
                kind: map_node_kind(node.kind),
                spans: SourceNodeSpans { core, header, full },
            })
        })
        .collect()
}

fn map_node_kind(kind: RecordedNodeKind) -> SourceNodeKind {
    match kind {
        RecordedNodeKind::Module => SourceNodeKind::Module,
        RecordedNodeKind::TypeDef => SourceNodeKind::TypeDef,
        RecordedNodeKind::Flow => SourceNodeKind::Flow,
        RecordedNodeKind::Func => SourceNodeKind::Func,
        RecordedNodeKind::Ui => SourceNodeKind::Ui,
        RecordedNodeKind::Steps => SourceNodeKind::Steps,
        RecordedNodeKind::Expr => SourceNodeKind::Expr,
        RecordedNodeKind::UiNode => SourceNodeKind::UiNode,
        RecordedNodeKind::Placeholder => SourceNodeKind::Placeholder,
        RecordedNodeKind::Field => SourceNodeKind::Field,
        RecordedNodeKind::FlowEntry => SourceNodeKind::FlowEntry,
        RecordedNodeKind::FlowArm => SourceNodeKind::FlowArm,
        RecordedNodeKind::Step => SourceNodeKind::Step,
    }
}

/// Attach line comments using a stable, source-local policy:
///
/// 1. Same physical line after content → `Trailing` of the most specific node
///    whose core covers the last non-trivia piece on that line.
/// 2. Full-line comment with no blank paragraph break before the next node
///    header → `Leading` of the nearest following node.
/// 3. Otherwise → `Free` (still preserved with an exact span).
fn attach_comments(
    source: &str,
    pieces: &[SourcePiece],
    nodes: &[SourceNode],
    paragraph_breaks: &[ParagraphBreak],
) -> Vec<CommentOccurrence> {
    let mut comments = Vec::new();
    let mut id = 0u32;
    for (index, piece) in pieces.iter().enumerate() {
        if piece.kind != SourcePieceKind::LineComment {
            continue;
        }
        let line_start = line_start_offset(source, piece.span.start as usize);
        let has_prior_content = pieces[..index].iter().any(|prior| {
            (prior.span.end as usize) > line_start
                && (prior.span.start as usize) < (piece.span.start as usize)
                && !prior.kind.is_trivia()
        });

        let (attachment, target, target_anchor) = if has_prior_content {
            let anchor = pieces[..index]
                .iter()
                .rev()
                .find(|prior| {
                    (prior.span.end as usize) > line_start
                        && (prior.span.start as usize) < (piece.span.start as usize)
                        && !prior.kind.is_trivia()
                })
                .map(|prior| prior.span);
            let target = anchor.and_then(|anchor| most_specific_node_at(nodes, anchor.start));
            (CommentAttachment::Trailing, target, anchor)
        } else {
            let next_node = nodes
                .iter()
                .filter(|node| node.spans.header.start >= piece.span.end)
                .min_by_key(|node| (node.spans.header.start, node.spans.core.len()));
            match next_node {
                Some(node)
                    if !paragraph_breaks.iter().any(|break_| {
                        break_.span.start >= piece.span.end
                            && break_.span.end <= node.spans.header.start
                    }) =>
                {
                    (
                        CommentAttachment::Leading,
                        Some(node.id),
                        Some(node.spans.header),
                    )
                }
                _ => (CommentAttachment::Free, None, None),
            }
        };

        comments.push(CommentOccurrence {
            id: CommentId(id),
            span: piece.span,
            attachment,
            target,
            target_anchor,
        });
        id += 1;
    }
    comments
}

fn most_specific_node_at(nodes: &[SourceNode], offset: u32) -> Option<SourceNodeId> {
    nodes
        .iter()
        .filter(|node| node.spans.header.start <= offset && offset < node.spans.core.end)
        .min_by_key(|node| node.spans.core.len())
        .map(|node| node.id)
}

fn line_start_offset(source: &str, offset: usize) -> usize {
    source[..offset]
        .rfind(['\r', '\n'])
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn map_recorded_rules(
    source: &str,
    pieces: &[SourcePiece],
    line_index: &LineIndex,
    tokens: &[Token],
    recorded: &[RecordedRuleOccurrence],
    nodes: &[SourceNode],
) -> Vec<RuleOccurrence> {
    recorded
        .iter()
        .enumerate()
        .filter_map(|(id, rule)| {
            let span = token_range_span(source, pieces, line_index, tokens, rule.tokens.clone())?;
            let (attachment, target_token, scope_token) = match rule.decision {
                RecordedRuleDecision::Pending => (RuleAttachment::Pending, None, None),
                RecordedRuleDecision::Attached { target_token } => {
                    (RuleAttachment::Attached, Some(target_token), None)
                }
                RecordedRuleDecision::Environment { scope_token } => {
                    (RuleAttachment::Environment, None, scope_token)
                }
                RecordedRuleDecision::DroppedByRecovery => {
                    (RuleAttachment::DroppedByRecovery, None, None)
                }
            };
            let target_anchor = target_token
                .and_then(|token| token_anchor_span(source, pieces, line_index, tokens, token));
            let scope_anchor = scope_token
                .and_then(|token| token_anchor_span(source, pieces, line_index, tokens, token));
            let target = target_anchor.and_then(|anchor| {
                nodes
                    .iter()
                    .find(|node| node.spans.header.start == anchor.start)
                    .map(|node| node.id)
            });
            Some(RuleOccurrence {
                id: RuleOccurrenceId(u32::try_from(id).ok()?),
                span,
                attachment,
                target,
                target_anchor,
                scope_anchor,
            })
        })
        .collect()
}

fn token_range_span(
    source: &str,
    pieces: &[SourcePiece],
    line_index: &LineIndex,
    tokens: &[Token],
    range: Range<usize>,
) -> Option<ByteSpan> {
    let mut spans = range.filter_map(|token_index| {
        token_anchor_span(source, pieces, line_index, tokens, token_index)
    });
    let first = spans.next()?;
    let mut end = first.end;
    for span in spans {
        end = span.end;
    }
    Some(ByteSpan {
        start: first.start,
        end,
    })
}

fn token_anchor_span(
    source: &str,
    pieces: &[SourcePiece],
    line_index: &LineIndex,
    tokens: &[Token],
    token_index: usize,
) -> Option<ByteSpan> {
    let token = tokens.get(token_index)?;
    let offset = token_offset(source, line_index, token)?;
    pieces
        .iter()
        .find(|piece| {
            piece.span.start as usize <= offset
                && offset < piece.span.end as usize
                && !piece.kind.is_trivia()
        })
        .map(|piece| piece.span)
}

fn suffix_at(source: &str, pieces: &[SourcePiece], index: usize) -> Option<(Commitment, usize)> {
    if pieces.get(index)?.kind != SourcePieceKind::Symbol {
        return None;
    }
    let first = document_text(source, pieces[index].span);
    let second = pieces.get(index + 1).and_then(|piece| {
        (piece.kind == SourcePieceKind::Symbol).then(|| document_text(source, piece.span))
    });
    match (first, second) {
        ("$$", Some("??")) => Some((Commitment::StrongLockedQuestionQuestion, 2)),
        ("$$", Some("?")) => Some((Commitment::StrongLockedQuestion, 2)),
        ("$", Some("??")) => Some((Commitment::LockedQuestionQuestion, 2)),
        ("$", Some("?")) => Some((Commitment::LockedQuestion, 2)),
        ("$$", _) => Some((Commitment::StrongLocked, 1)),
        ("$", _) => Some((Commitment::Locked, 1)),
        ("??", _) => Some((Commitment::QuestionQuestion, 1)),
        ("?", _) => Some((Commitment::Question, 1)),
        _ => None,
    }
}

fn previous_anchor(pieces: &[SourcePiece], suffix_index: usize) -> Option<usize> {
    let mut index = suffix_index.checked_sub(1)?;
    loop {
        match pieces[index].kind {
            SourcePieceKind::Whitespace => {}
            SourcePieceKind::Newline(_) | SourcePieceKind::LineComment => return None,
            _ => return Some(index),
        }
        index = index.checked_sub(1)?;
    }
}

fn scan_paragraph_breaks(source: &str, pieces: &[SourcePiece]) -> Vec<ParagraphBreak> {
    let mut breaks = Vec::new();
    let mut line_start = 0usize;
    let mut line_has_content = false;
    let mut blank_run: Vec<BlankLine> = Vec::new();

    for piece in pieces {
        match piece.kind {
            SourcePieceKind::Whitespace => {}
            SourcePieceKind::Newline(kind) => {
                if !line_has_content {
                    blank_run.push(BlankLine {
                        span: ByteSpan {
                            start: u32::try_from(line_start).expect("document too large"),
                            end: piece.span.end,
                        },
                        newline: kind,
                    });
                } else if !blank_run.is_empty() {
                    push_paragraph_break(&mut breaks, std::mem::take(&mut blank_run));
                }
                line_start = piece.span.end as usize;
                line_has_content = false;
            }
            SourcePieceKind::LineComment => line_has_content = true,
            _ => line_has_content = true,
        }
    }
    if !blank_run.is_empty() {
        push_paragraph_break(&mut breaks, blank_run);
    }
    debug_assert!(breaks.iter().all(|boundary| source
        .is_char_boundary(boundary.span.start as usize)
        && source.is_char_boundary(boundary.span.end as usize)));
    breaks
}

fn push_paragraph_break(breaks: &mut Vec<ParagraphBreak>, blank_lines: Vec<BlankLine>) {
    let span = ByteSpan {
        start: blank_lines[0].span.start,
        end: blank_lines[blank_lines.len() - 1].span.end,
    };
    breaks.push(ParagraphBreak { span, blank_lines });
}

#[derive(Debug, Clone, Copy)]
struct PhysicalLine {
    content_span: ByteSpan,
    indentation: u32,
    blank: bool,
}

fn scan_rule_groups(source: &str) -> Vec<RuleGroupSyntax> {
    let lines = physical_lines(source);
    let mut groups = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if !is_rule_line(source, line) {
            index += 1;
            continue;
        }

        let indentation = line.indentation;
        let mut rule_spans = vec![line.content_span];
        let start = line.content_span.start;
        let mut end = line.content_span.end;
        index += 1;

        while index < lines.len()
            && !lines[index].blank
            && lines[index].indentation == indentation
            && is_rule_line(source, lines[index])
        {
            rule_spans.push(lines[index].content_span);
            end = lines[index].content_span.end;
            index += 1;
        }

        let has_paragraph_break = lines.get(index).is_some_and(|line| line.blank);
        let target = if has_paragraph_break {
            None
        } else {
            lines.get(index).copied().filter(|candidate| {
                candidate.indentation == indentation
                    && is_attachable_target_line(source, *candidate)
            })
        };
        groups.push(RuleGroupSyntax {
            span: ByteSpan { start, end },
            rule_spans,
            indentation,
            attachment: if target.is_some() {
                RuleAttachmentSyntaxKind::AttachedCandidate
            } else {
                RuleAttachmentSyntaxKind::Environment
            },
            target_span: target.map(|line| line.content_span),
        });
    }
    groups
}

fn physical_lines(source: &str) -> Vec<PhysicalLine> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let rest = &source[start..];
        let newline = rest.find(['\r', '\n']);
        let content_end = newline.map_or(source.len(), |relative| start + relative);
        let line_end = match newline {
            Some(relative)
                if rest.as_bytes()[relative] == b'\r'
                    && rest.as_bytes().get(relative + 1) == Some(&b'\n') =>
            {
                content_end + 2
            }
            Some(_) => content_end + 1,
            None => content_end,
        };
        let content = &source[start..content_end];
        let indentation_bytes = content
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let indentation = content[..indentation_bytes]
            .bytes()
            .map(|byte| if byte == b'\t' { 4 } else { 1 })
            .sum();
        let trimmed = content[indentation_bytes..].trim_end_matches([' ', '\t']);
        lines.push(PhysicalLine {
            content_span: ByteSpan::new(start + indentation_bytes, content_end),
            indentation,
            blank: trimmed.is_empty(),
        });
        if line_end == start {
            break;
        }
        start = line_end;
    }
    if source.is_empty() {
        lines.push(PhysicalLine {
            content_span: ByteSpan::new(0, 0),
            indentation: 0,
            blank: true,
        });
    }
    lines
}

fn is_rule_line(source: &str, line: PhysicalLine) -> bool {
    let text = document_text(source, line.content_span);
    text.strip_prefix("rule")
        .is_some_and(|rest| rest.starts_with([' ', '\t', '?', '$']) || rest.starts_with('"'))
}

fn is_attachable_target_line(source: &str, line: PhysicalLine) -> bool {
    let text = document_text(source, line.content_span);
    let first = text
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '?' | '$' | ':'))
        .next()
        .unwrap_or_default();
    matches!(first, "module" | "type" | "flow" | "func" | "ui")
        || text.starts_with(">>>")
        || looks_like_field_or_flow_entry(text)
}

fn looks_like_field_or_flow_entry(text: &str) -> bool {
    let Some(colon) = text.find(':') else {
        return text.contains(">>>");
    };
    !text[..colon].trim().is_empty()
}

fn document_text(source: &str, span: ByteSpan) -> &str {
    &source[span.as_range()]
}

fn is_word_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_word_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_lossless;

    #[test]
    fn pieces_exactly_partition_mixed_source() {
        let source = "module 应用$:\r\n\t// comment\r\n\r\n    desc?? \"a\\n值\"$?  \n";
        let result = parse_lossless(source);
        let reconstructed = result
            .document
            .pieces()
            .iter()
            .map(|piece| result.document.text(piece.span).unwrap())
            .collect::<String>();
        assert_eq!(reconstructed, source);
        assert_eq!(result.document.render_lossless(), source);
        for windows in result.document.pieces().windows(2) {
            assert_eq!(windows[0].span.end, windows[1].span.start);
        }
    }

    #[test]
    fn preserves_newline_kinds_and_exact_blank_lines() {
        let source = "a\r\n\r\n\n\r\nb\r\r";
        let result = parse_lossless(source);
        let newlines = result
            .document
            .pieces()
            .iter()
            .filter_map(|piece| match piece.kind {
                SourcePieceKind::Newline(kind) => Some(kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            newlines,
            vec![
                NewlineKind::CrLf,
                NewlineKind::CrLf,
                NewlineKind::Lf,
                NewlineKind::CrLf,
                NewlineKind::Cr,
                NewlineKind::Cr,
            ]
        );
        assert_eq!(result.document.paragraph_breaks()[0].blank_lines.len(), 3);
    }

    #[test]
    fn preserves_more_blank_lines_than_the_legacy_lexer_cap() {
        let source = "a\n\n\n\n\n\n\nb\n";
        let result = parse_lossless(source);
        assert_eq!(result.document.paragraph_breaks()[0].blank_lines.len(), 6);
        assert_eq!(result.document.render_lossless(), source);
    }

    #[test]
    fn preserves_source_after_lexer_errors() {
        let source = "desc \"unterminated\nfunc Later: ...\n";
        let result = parse_lossless(source);
        assert!(!result.errors.is_empty());
        assert_eq!(result.document.render_lossless(), source);
        assert_eq!(
            result.document.semantic().fragments.len(),
            0,
            "lexer-level errors still produce an empty semantic AST"
        );
    }

    #[test]
    fn finds_all_explicit_commitment_suffixes() {
        let source = "func?? Pay$:\n    desc$? \"处理\"$$??\n    requires: ready and$ true\n";
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let values = result
            .document
            .commitment_slots()
            .iter()
            .filter(|slot| slot.value != Commitment::None)
            .map(|slot| slot.value)
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                Commitment::QuestionQuestion,
                Commitment::Locked,
                Commitment::LockedQuestion,
                Commitment::StrongLockedQuestionQuestion,
                Commitment::Locked,
            ]
        );
        assert!(result
            .document
            .commitment_slots()
            .iter()
            .filter(|slot| slot.value != Commitment::None)
            .all(|slot| slot.adjacent && slot.semantic_slot));
    }

    #[test]
    fn records_zero_width_slots_for_uncommitted_semantic_anchors() {
        let source =
            "func Pay:\n    requires: ready and true\n    steps:\n        enabled = false\n";
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let empty_slots = result
            .document
            .commitment_slots()
            .iter()
            .filter(|slot| slot.value == Commitment::None)
            .collect::<Vec<_>>();
        assert!(!empty_slots.is_empty());
        assert!(empty_slots
            .iter()
            .all(|slot| slot.semantic_slot && slot.suffix_span.is_empty()));
        for slot in empty_slots {
            assert_eq!(slot.anchor_span.end, slot.suffix_span.start);
        }
    }

    #[test]
    fn fragment_nodes_include_attached_rule_preludes_in_movable_spans() {
        let source = r#"rule "must audit"
func Pay:
    steps:
        charge payment

type Status: Active | Paid
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let fragments: Vec<_> = result
            .document
            .nodes()
            .iter()
            .filter(|node| matches!(node.kind, SourceNodeKind::Func | SourceNodeKind::TypeDef))
            .collect();
        assert_eq!(fragments.len(), 2);

        let func = fragments
            .iter()
            .find(|node| node.kind == SourceNodeKind::Func)
            .unwrap();
        assert_eq!(result.document.text(func.spans.header), Some("func Pay:"));
        assert_eq!(
            result.document.text(func.spans.full).unwrap(),
            "rule \"must audit\"\nfunc Pay:\n    steps:\n        charge payment"
        );
        assert_eq!(result.document.movable_span(func.id), Some(func.spans.full));

        let typedef = fragments
            .iter()
            .find(|node| node.kind == SourceNodeKind::TypeDef)
            .unwrap();
        assert_eq!(typedef.spans.full, typedef.spans.core);

        assert!(result
            .document
            .nodes()
            .iter()
            .any(|node| node.kind == SourceNodeKind::Step));
    }

    #[test]
    fn parser_records_duplicate_rules_by_occurrence_not_content() {
        let source = r#"rule "same"

rule "same"
func Pay: ...
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.document.rules().len(), 2);

        let environment = &result.document.rules()[0];
        assert_eq!(environment.attachment, RuleAttachment::Environment);
        assert!(environment.target.is_none());
        assert_eq!(
            result.document.text(environment.span),
            Some("rule \"same\"")
        );

        let attached = &result.document.rules()[1];
        assert_eq!(attached.attachment, RuleAttachment::Attached);
        let target = result.document.node(attached.target.unwrap()).unwrap();
        assert_eq!(target.kind, SourceNodeKind::Func);
        assert_eq!(result.document.text(attached.span), Some("rule \"same\""));
        assert_eq!(
            result.document.text(attached.target_anchor.unwrap()),
            Some("func")
        );
    }

    #[test]
    fn parser_records_nested_flow_rule_targets() {
        let source = r#"flow Checkout:
    rule "entry"
    Pending:
        rule "arm"
        >>> Paid:
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.document.rules().len(), 2);
        assert!(result
            .document
            .rules()
            .iter()
            .all(|rule| rule.attachment == RuleAttachment::Attached));
        assert_eq!(
            result
                .document
                .text(result.document.rules()[0].target_anchor.unwrap()),
            Some("Pending")
        );
        assert_eq!(
            result
                .document
                .text(result.document.rules()[1].target_anchor.unwrap()),
            Some(">>>")
        );

        let entry = result
            .document
            .node(result.document.rules()[0].target.unwrap())
            .unwrap();
        assert_eq!(entry.kind, SourceNodeKind::FlowEntry);
        assert_eq!(result.document.text(entry.spans.header), Some("Pending:"));

        let arm = result
            .document
            .node(result.document.rules()[1].target.unwrap())
            .unwrap();
        assert_eq!(arm.kind, SourceNodeKind::FlowArm);
        assert_eq!(result.document.text(arm.spans.header), Some(">>> Paid:"));

        assert!(result
            .document
            .nodes()
            .iter()
            .any(|node| node.kind == SourceNodeKind::Flow));
        assert!(result
            .document
            .nodes()
            .iter()
            .any(|node| node.kind == SourceNodeKind::FlowEntry));
        assert!(result
            .document
            .nodes()
            .iter()
            .any(|node| node.kind == SourceNodeKind::FlowArm));
    }

    #[test]
    fn parser_keeps_rules_dropped_during_recovery() {
        let source = r#"rule "protected"
func Broken(:
type Good: ...
"#;
        let result = parse_lossless(source);
        assert!(!result.errors.is_empty());
        assert_eq!(result.document.rules().len(), 1);
        let rule = &result.document.rules()[0];
        assert_eq!(rule.attachment, RuleAttachment::DroppedByRecovery);
        assert_eq!(result.document.text(rule.span), Some("rule \"protected\""));
        assert!(result.document.semantic().fragments.iter().any(
            |fragment| matches!(fragment, crate::ast::Fragment::TypeDef { typedef } if typedef.name.name == "Good")
        ));
    }

    #[test]
    fn parser_records_field_attachment_and_module_environment_scope() {
        let source = r#"module App:
    rule "module environment"

    type User:
        rule "field"
        name: String
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.document.rules().len(), 2);

        let module_rule = &result.document.rules()[0];
        assert_eq!(module_rule.attachment, RuleAttachment::Environment);
        assert_eq!(
            result.document.text(module_rule.scope_anchor.unwrap()),
            Some("module")
        );

        let field_rule = &result.document.rules()[1];
        assert_eq!(field_rule.attachment, RuleAttachment::Attached);
        assert_eq!(
            result.document.text(field_rule.target_anchor.unwrap()),
            Some("name")
        );
        let field = result.document.node(field_rule.target.unwrap()).unwrap();
        assert_eq!(field.kind, SourceNodeKind::Field);
        assert_eq!(
            result.document.text(field.spans.header),
            Some("name: String")
        );
        assert!(result
            .document
            .nodes()
            .iter()
            .any(|node| node.kind == SourceNodeKind::Field));
    }

    #[test]
    fn comment_attachment_policy_covers_leading_trailing_and_free() {
        let source = r#"// free before blank

// leading func
func Pay: // trailing func
    steps:
        // leading step
        charge payment
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let comments = result.document.comments();
        assert_eq!(comments.len(), 4);

        assert_eq!(comments[0].attachment, CommentAttachment::Free);
        assert!(comments[0].target.is_none());
        assert_eq!(
            result.document.text(comments[0].span),
            Some("// free before blank")
        );

        assert_eq!(comments[1].attachment, CommentAttachment::Leading);
        let func = result.document.node(comments[1].target.unwrap()).unwrap();
        assert_eq!(func.kind, SourceNodeKind::Func);
        assert_eq!(
            result.document.text(comments[1].span),
            Some("// leading func")
        );

        assert_eq!(comments[2].attachment, CommentAttachment::Trailing);
        assert_eq!(comments[2].target, Some(func.id));
        assert_eq!(
            result.document.text(comments[2].span),
            Some("// trailing func")
        );

        assert_eq!(comments[3].attachment, CommentAttachment::Leading);
        let step = result.document.node(comments[3].target.unwrap()).unwrap();
        assert_eq!(step.kind, SourceNodeKind::Step);
        assert_eq!(
            result.document.text(comments[3].span),
            Some("// leading step")
        );
        assert_eq!(
            result.document.text(step.spans.header),
            Some("charge payment")
        );
    }

    #[test]
    fn parser_records_nested_step_nodes() {
        let source = r#"func Pay:
    steps:
        charge payment
        if ready:
            confirm
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let steps: Vec<_> = result
            .document
            .nodes()
            .iter()
            .filter(|node| node.kind == SourceNodeKind::Step)
            .collect();
        assert!(
            steps.len() >= 3,
            "expected action + if + nested confirm steps, got {}",
            steps.len()
        );
        assert!(steps
            .iter()
            .any(|node| { result.document.text(node.spans.header) == Some("charge payment") }));
        assert!(steps
            .iter()
            .any(|node| result.document.text(node.spans.header) == Some("if ready:")));
        assert!(steps
            .iter()
            .any(|node| result.document.text(node.spans.header) == Some("confirm")));
    }

    #[test]
    fn nested_field_and_flow_nodes_carry_movable_core_spans() {
        let source = r#"type User:
    rule "field"
    name: String

flow Checkout:
    rule "entry"
    Pending >>> Paid:
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let field = result
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::Field)
            .unwrap();
        assert_eq!(
            result.document.text(field.spans.full).unwrap(),
            "rule \"field\"\n    name: String"
        );

        let entry = result
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::FlowEntry)
            .unwrap();
        assert_eq!(
            result.document.text(entry.spans.full).unwrap(),
            "rule \"entry\"\n    Pending >>> Paid:"
        );

        let arm = result
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::FlowArm)
            .unwrap();
        assert_eq!(result.document.text(arm.spans.header), Some(">>> Paid:"));
    }

    #[test]
    fn line_index_supports_utf8_scalar_and_utf16_columns() {
        let source = "a😀中\nnext";
        let result = parse_lossless(source);
        let offset = "a😀".len() as u32;
        assert_eq!(
            result
                .document
                .line_index()
                .position(source, offset, ColumnEncoding::UnicodeScalar),
            Some(SourcePosition { line: 0, column: 2 })
        );
        assert_eq!(
            result
                .document
                .line_index()
                .position(source, offset, ColumnEncoding::Utf16),
            Some(SourcePosition { line: 0, column: 3 })
        );
        assert_eq!(
            result
                .document
                .line_index()
                .position(source, offset, ColumnEncoding::Utf8Bytes),
            Some(SourcePosition { line: 0, column: 5 })
        );
        for encoding in [
            ColumnEncoding::Utf8Bytes,
            ColumnEncoding::UnicodeScalar,
            ColumnEncoding::Utf16,
        ] {
            let position = result
                .document
                .line_index()
                .position(source, offset, encoding)
                .unwrap();
            assert_eq!(
                result
                    .document
                    .line_index()
                    .offset(source, position, encoding),
                Some(offset)
            );
        }
    }

    #[test]
    fn rule_groups_keep_exact_spans_and_attachment_candidates() {
        let source = r#"rule "first"
rule$ "second"
func Pay: ...

rule "environment"

steps:
    do work

flow Checkout:
    Pending:
        rule "arm"
        >>> Paid:
"#;
        let result = parse_lossless(source);
        let groups = result.document.rule_groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].rule_spans.len(), 2);
        assert_eq!(
            groups[0].attachment,
            RuleAttachmentSyntaxKind::AttachedCandidate
        );
        assert_eq!(
            result.document.text(groups[0].target_span.unwrap()),
            Some("func Pay: ...")
        );
        assert_eq!(groups[1].attachment, RuleAttachmentSyntaxKind::Environment);
        assert_eq!(
            groups[2].attachment,
            RuleAttachmentSyntaxKind::AttachedCandidate
        );
        assert_eq!(
            result.document.text(groups[2].target_span.unwrap()),
            Some(">>> Paid:")
        );
    }
}
