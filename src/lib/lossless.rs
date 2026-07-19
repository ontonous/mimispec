use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};
use std::ops::Range;
use std::sync::Arc;

use serde::Serialize;

use crate::ast::{Commitment, File};
use crate::error::{ParseError, ParseStatus};
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
        assert!(
            start <= end,
            "ByteSpan::new: start ({start}) must not exceed end ({end})"
        );
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
    pub id: CommitmentSlotId,
    pub owner: Option<SourceNodeId>,
    pub anchor_kind: CommitmentAnchorKind,
    pub footprint: CommitmentFootprintKind,
    pub anchor_span: ByteSpan,
    pub suffix_span: ByteSpan,
    pub full_span: ByteSpan,
    pub value: Commitment,
    pub adjacent: bool,
    pub semantic_slot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct CommitmentSlotId(pub u32);

/// Semantic field governed by a commitment anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentFootprintKind {
    EntityKind,
    NameOrReference,
    Value,
    Clause,
    Event,
    Transition,
    ExpressionOperator,
    Placeholder,
    Unknown,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceNodeKind {
    Rule,
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
    Desc,
    Clause,
    Math,
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
    pub(crate) fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(newline_end_offsets(source));
        Self { starts }
    }

    /// Update line starts after one byte-range edit without reparsing syntax.
    pub(crate) fn apply_edit(&mut self, start: u32, end: u32, replacement: &str) {
        debug_assert!(start <= end);
        let removed = end - start;
        let inserted = u32::try_from(replacement.len()).expect("document too large");
        let delta = i64::from(inserted) - i64::from(removed);
        let mut next = self
            .starts
            .iter()
            .copied()
            .filter(|line_start| *line_start <= start)
            .collect::<Vec<_>>();
        next.extend(
            newline_end_offsets(replacement)
                .into_iter()
                .map(|offset| start.checked_add(offset).expect("document too large")),
        );
        next.extend(
            self.starts
                .iter()
                .copied()
                .filter(|line_start| *line_start > end)
                .map(|line_start| {
                    u32::try_from(i64::from(line_start) + delta).expect("valid shifted line start")
                }),
        );
        next.sort_unstable();
        next.dedup();
        self.starts = next;
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

fn newline_end_offsets(source: &str) -> Vec<u32> {
    let bytes = source.as_bytes();
    let mut offsets = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                index += 2;
                offsets.push(u32::try_from(index).expect("document too large"));
            }
            b'\r' | b'\n' => {
                index += 1;
                offsets.push(u32::try_from(index).expect("document too large"));
            }
            _ => index += 1,
        }
    }
    offsets
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
    structure: DocumentStructureIndex,
}

/// Revision-local structural lookup tables built once with a lossless document.
///
/// The index is deliberately not serialized: all IDs and relationships are
/// derived from parser-recorded spans and must be rebuilt for every revision.
#[derive(Debug, Clone, Default)]
struct DocumentStructureIndex {
    parents: Vec<Option<SourceNodeId>>,
    children: Vec<Vec<SourceNodeId>>,
    nodes_by_kind: HashMap<SourceNodeKind, Vec<SourceNodeId>>,
    slots_by_owner: Vec<Vec<CommitmentSlotId>>,
    rules_by_target: Vec<Vec<RuleOccurrenceId>>,
    scope_paths: Vec<Vec<SourceNodeId>>,
    topology: Vec<String>,
    positions: Vec<String>,
}

impl DocumentStructureIndex {
    fn build(
        source: &str,
        nodes: &[SourceNode],
        slots: &[CommitmentSlotSyntax],
        rules: &[RuleOccurrence],
    ) -> Self {
        let mut parents = vec![None; nodes.len()];
        let mut ordered = nodes.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|node| {
            (
                node.spans.core.start,
                Reverse(node.spans.core.end),
                node.id.0,
            )
        });

        let mut active = Vec::<&SourceNode>::new();
        for node in ordered {
            active.retain(|candidate| candidate.spans.core.end > node.spans.core.start);
            let parent = active
                .iter()
                .copied()
                .filter(|candidate| {
                    candidate.spans.core.start <= node.spans.core.start
                        && candidate.spans.core.end >= node.spans.core.end
                        && (candidate.spans.core.start < node.spans.core.start
                            || candidate.spans.core.end > node.spans.core.end)
                })
                .min_by_key(|candidate| (candidate.spans.core.len(), candidate.id.0))
                .map(|candidate| candidate.id);
            if let Some(entry) = parents.get_mut(node.id.0 as usize) {
                *entry = parent;
            }
            active.push(node);
        }

        let mut children = vec![Vec::new(); nodes.len()];
        let mut roots = Vec::new();
        for node in nodes {
            match parents.get(node.id.0 as usize).copied().flatten() {
                Some(parent) => {
                    if let Some(entry) = children.get_mut(parent.0 as usize) {
                        entry.push(node.id);
                    }
                }
                None => roots.push(node.id),
            }
        }
        let source_order = |id: &SourceNodeId| {
            let node = &nodes[id.0 as usize];
            (node.spans.core.start, node.spans.core.end, node.id.0)
        };
        roots.sort_by_key(source_order);
        for item in &mut children {
            item.sort_by_key(source_order);
        }

        let mut nodes_by_kind = HashMap::<SourceNodeKind, Vec<SourceNodeId>>::new();
        for node in nodes {
            nodes_by_kind.entry(node.kind).or_default().push(node.id);
        }
        for item in nodes_by_kind.values_mut() {
            item.sort_by_key(source_order);
        }

        let mut slots_by_owner = vec![Vec::new(); nodes.len()];
        for slot in slots.iter().filter(|slot| slot.semantic_slot) {
            if let Some(owner) = slot.owner {
                if let Some(entry) = slots_by_owner.get_mut(owner.0 as usize) {
                    entry.push(slot.id);
                }
            }
        }
        for item in &mut slots_by_owner {
            item.sort_by_key(|id| {
                let slot = &slots[id.0 as usize];
                (slot.anchor_span.start, slot.suffix_span.start, slot.id.0)
            });
        }

        let mut rules_by_target = vec![Vec::new(); nodes.len()];
        for rule in rules {
            if let Some(target) = rule.target {
                if let Some(entry) = rules_by_target.get_mut(target.0 as usize) {
                    entry.push(rule.id);
                }
            }
        }
        for item in &mut rules_by_target {
            item.sort_by_key(|id| {
                let rule = &rules[id.0 as usize];
                (rule.span.start, rule.span.end, rule.id.0)
            });
        }

        let structural_identity = |id: SourceNodeId| {
            let node = &nodes[id.0 as usize];
            let stable_anchor = slots_by_owner
                .get(id.0 as usize)
                .into_iter()
                .flatten()
                .filter_map(|slot| slots.get(slot.0 as usize))
                .find(|slot| slot.footprint == CommitmentFootprintKind::NameOrReference)
                .and_then(|slot| source.get(slot.anchor_span.as_range()))
                .map(str::trim)
                .filter(|anchor| !anchor.is_empty())
                .unwrap_or("");
            format!("{:?}:{stable_anchor}", node.kind)
        };
        let mut topology = vec![String::new(); nodes.len()];
        for node in nodes {
            topology[node.id.0 as usize] = children[node.id.0 as usize]
                .iter()
                .map(|child| structural_identity(*child))
                .collect::<Vec<_>>()
                .join("|");
        }

        let mut positions = vec![String::new(); nodes.len()];
        for (ordinal, id) in roots.iter().enumerate() {
            positions[id.0 as usize] = format!("document/{ordinal}");
        }
        for parent in nodes {
            for (ordinal, child) in children[parent.id.0 as usize].iter().enumerate() {
                positions[child.0 as usize] = format!("{:?}/{ordinal}", parent.kind);
            }
        }

        let mut scope_paths = vec![Vec::new(); nodes.len()];
        let mut traversal = roots.into_iter().rev().collect::<Vec<_>>();
        while let Some(id) = traversal.pop() {
            let node = &nodes[id.0 as usize];
            let mut path = parents[id.0 as usize]
                .and_then(|parent| scope_paths.get(parent.0 as usize).cloned())
                .unwrap_or_default();
            if node.kind.is_scope_container() {
                path.push(id);
            }
            scope_paths[id.0 as usize] = path;
            if let Some(nested) = children.get(id.0 as usize) {
                traversal.extend(nested.iter().rev().copied());
            }
        }

        Self {
            parents,
            children,
            nodes_by_kind,
            slots_by_owner,
            rules_by_target,
            scope_paths,
            topology,
            positions,
        }
    }
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

    pub fn commitment_slot(&self, id: CommitmentSlotId) -> Option<&CommitmentSlotSyntax> {
        self.commitment_slots
            .get(id.0 as usize)
            .filter(|slot| slot.id == id)
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

    /// Immediate structural parent in this immutable document revision.
    pub fn parent_node(&self, id: SourceNodeId) -> Option<&SourceNode> {
        let parent = self
            .structure
            .parents
            .get(id.0 as usize)
            .copied()
            .flatten()?;
        self.node(parent)
    }

    /// Immediate structural children in source order for this revision.
    pub fn child_nodes(&self, id: SourceNodeId) -> impl Iterator<Item = &SourceNode> {
        self.structure
            .children
            .get(id.0 as usize)
            .into_iter()
            .flatten()
            .filter_map(|child| self.node(*child))
    }

    /// Parser-proven semantic commitment slots owned by a node, in source order.
    pub fn commitment_slots_for_owner(
        &self,
        id: SourceNodeId,
    ) -> impl Iterator<Item = &CommitmentSlotSyntax> {
        self.structure
            .slots_by_owner
            .get(id.0 as usize)
            .into_iter()
            .flatten()
            .filter_map(|slot| self.commitment_slot(*slot))
    }

    /// Container path from the outermost scope through the node's nearest scope.
    ///
    /// The node itself is included when it is a scope container. Returned IDs
    /// are revision-local and must not be persisted across edits.
    pub fn scope_path(&self, id: SourceNodeId) -> Option<&[SourceNodeId]> {
        self.node(id)?;
        self.structure
            .scope_paths
            .get(id.0 as usize)
            .map(Vec::as_slice)
    }

    pub(crate) fn nodes_of_kind(&self, kind: SourceNodeKind) -> impl Iterator<Item = &SourceNode> {
        self.structure
            .nodes_by_kind
            .get(&kind)
            .into_iter()
            .flatten()
            .filter_map(|id| self.node(*id))
    }

    pub(crate) fn rules_for_target(
        &self,
        id: SourceNodeId,
    ) -> impl Iterator<Item = &RuleOccurrence> {
        self.structure
            .rules_by_target
            .get(id.0 as usize)
            .into_iter()
            .flatten()
            .filter_map(|rule| self.rule(*rule))
    }

    pub(crate) fn structural_topology(&self, id: SourceNodeId) -> Option<&str> {
        self.node(id)?;
        self.structure
            .topology
            .get(id.0 as usize)
            .map(String::as_str)
    }

    pub(crate) fn structural_position(&self, id: SourceNodeId) -> Option<&str> {
        self.node(id)?;
        self.structure
            .positions
            .get(id.0 as usize)
            .map(String::as_str)
    }

    pub fn movable_span(&self, id: SourceNodeId) -> Option<ByteSpan> {
        self.node(id).map(|node| node.spans.full)
    }

    /// Top-level Fragment kinds that structured edit may relocate as a unit.
    pub fn is_movable_fragment(&self, id: SourceNodeId) -> bool {
        self.node(id)
            .is_some_and(|node| node.kind.is_top_level_fragment())
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

    /// Byte range that should travel with a structured move, including the
    /// attached rule prelude and one trailing newline when present.
    pub fn movable_chunk_span(&self, id: SourceNodeId) -> Option<ByteSpan> {
        let full = self.movable_span(id)?;
        let source = self.source();
        let mut end = full.end as usize;
        if source[end..].starts_with("\r\n") {
            end += 2;
        } else if source.as_bytes().get(end) == Some(&b'\n')
            || source.as_bytes().get(end) == Some(&b'\r')
        {
            end += 1;
        }
        Some(ByteSpan::new(full.start as usize, end))
    }

    /// Relocate a top-level Fragment, carrying its attached rule prelude text.
    ///
    /// Returns a new source string. The current document remains immutable.
    pub fn move_fragment(
        &self,
        id: SourceNodeId,
        destination: MoveDestination,
    ) -> Result<String, MoveError> {
        if !self.is_movable_fragment(id) {
            return Err(MoveError::NotMovableFragment);
        }
        let chunk = self.movable_chunk_span(id).ok_or(MoveError::UnknownNode)?;
        let insert_at = self.resolve_destination(destination, id, chunk)?;
        Ok(splice_move(self.source(), chunk, insert_at))
    }

    /// Relocate a top-level Fragment and reparse the resulting revision.
    pub fn move_fragment_reparse(
        &self,
        id: SourceNodeId,
        destination: MoveDestination,
    ) -> Result<LosslessParseResult, MoveError> {
        let source = self.move_fragment(id, destination)?;
        Ok(crate::parse_lossless(&source))
    }

    /// Stable attachment fingerprint for formatter and structured-edit checks.
    ///
    /// Each entry is `(rule text, attachment kind, target kind or scope kind)`.
    pub fn rule_attachment_fingerprint(&self) -> Vec<(String, RuleAttachment, Option<String>)> {
        self.rules
            .iter()
            .map(|rule| {
                let text = self.text(rule.span).unwrap_or_default().trim().to_string();
                let target = match rule.attachment {
                    RuleAttachment::Attached => rule
                        .target
                        .and_then(|id| self.node(id))
                        .map(|node| format!("{:?}", node.kind))
                        .or_else(|| {
                            rule.target_anchor
                                .and_then(|span| self.text(span))
                                .map(|text| format!("anchor:{text}"))
                        }),
                    RuleAttachment::Environment => rule
                        .scope_anchor
                        .and_then(|span| self.text(span))
                        .map(|text| format!("scope:{text}"))
                        .or(Some("scope:file".into())),
                    RuleAttachment::DroppedByRecovery => Some("dropped".into()),
                    RuleAttachment::Pending => Some("pending".into()),
                };
                (text, rule.attachment, target)
            })
            .collect()
    }

    /// Commitment suffix fingerprint: `(anchor text, suffix value, slot kind)`.
    pub fn commitment_fingerprint(&self) -> Vec<(String, Commitment, String)> {
        self.commitment_slots
            .iter()
            .filter(|slot| slot.semantic_slot && slot.value != Commitment::None)
            .map(|slot| {
                let anchor = self.text(slot.anchor_span).unwrap_or_default().to_string();
                (anchor, slot.value, format!("{:?}", slot.anchor_kind))
            })
            .collect()
    }

    fn resolve_destination(
        &self,
        destination: MoveDestination,
        moving: SourceNodeId,
        chunk: ByteSpan,
    ) -> Result<u32, MoveError> {
        let insert_at = match destination {
            MoveDestination::ByteOffset(offset) => {
                if offset as usize > self.source().len()
                    || !self.source().is_char_boundary(offset as usize)
                {
                    return Err(MoveError::InvalidDestination);
                }
                offset
            }
            MoveDestination::Before(target) => {
                if target == moving {
                    return Err(MoveError::InvalidDestination);
                }
                if !self.is_movable_fragment(target) {
                    return Err(MoveError::NotMovableFragment);
                }
                self.movable_span(target)
                    .ok_or(MoveError::UnknownNode)?
                    .start
            }
            MoveDestination::After(target) => {
                if target == moving {
                    return Err(MoveError::InvalidDestination);
                }
                if !self.is_movable_fragment(target) {
                    return Err(MoveError::NotMovableFragment);
                }
                self.movable_chunk_span(target)
                    .ok_or(MoveError::UnknownNode)?
                    .end
            }
        };

        // Inserting inside the removed chunk is undefined.
        if insert_at > chunk.start && insert_at < chunk.end {
            return Err(MoveError::InvalidDestination);
        }
        Ok(insert_at)
    }
}

/// Where a structured Fragment move should land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDestination {
    Before(SourceNodeId),
    After(SourceNodeId),
    ByteOffset(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    UnknownNode,
    NotMovableFragment,
    InvalidDestination,
}

impl SourceNodeKind {
    pub fn is_top_level_fragment(self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::TypeDef
                | Self::Flow
                | Self::Func
                | Self::Ui
                | Self::Steps
                | Self::Expr
                | Self::UiNode
                | Self::Placeholder
        )
    }

    pub fn is_scope_container(self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::TypeDef
                | Self::Flow
                | Self::Func
                | Self::Ui
                | Self::Steps
                | Self::UiNode
        )
    }
}

fn splice_move(source: &str, chunk: ByteSpan, insert_at: u32) -> String {
    let start = chunk.start as usize;
    let end = chunk.end as usize;
    let insert_at = insert_at as usize;
    let moved = &source[start..end];
    let without = format!("{}{}", &source[..start], &source[end..]);
    let adjusted = if insert_at <= start {
        insert_at
    } else {
        insert_at - (end - start)
    };
    let mut result = String::with_capacity(source.len());
    result.push_str(&without[..adjusted]);
    // Keep a blank-line separator when inserting between non-empty neighbors.
    if adjusted > 0 && !moved.starts_with(['\n', '\r']) {
        let before = &without[..adjusted];
        if !before.ends_with('\n') && !before.ends_with('\r') {
            result.push('\n');
        }
    }
    result.push_str(moved);
    if adjusted < without.len() {
        let after = &without[adjusted..];
        if !moved.ends_with('\n') && !moved.ends_with('\r') && !after.starts_with(['\n', '\r']) {
            result.push('\n');
        }
        result.push_str(after);
    }
    result
}

#[derive(Debug, Clone)]
pub struct LosslessParseResult {
    pub document: LosslessDocument,
    pub errors: Vec<ParseError>,
    pub status: ParseStatus,
}

impl LosslessParseResult {
    pub fn is_partial(&self) -> bool {
        self.status == ParseStatus::Partial
    }
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
    let paragraph_breaks = scan_paragraph_breaks(&source, &pieces);
    let rule_groups = scan_rule_groups(&source);
    let token_spans = map_token_spans(&source, &pieces, &line_index, tokens);
    let nodes = map_recorded_nodes(&source, tokens, &token_spans, recorded_nodes, &rule_groups);
    let commitment_slots = if recorded_commitments.is_empty() {
        scan_commitment_slots(&source, &pieces)
    } else {
        map_recorded_commitments(&source, &token_spans, recorded_commitments, &nodes)
    };
    let rules = map_recorded_rules(&token_spans, recorded_rules, &nodes);
    let comments = attach_comments(&source, &pieces, &nodes, &paragraph_breaks);
    let structure = DocumentStructureIndex::build(&source, &nodes, &commitment_slots, &rules);
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
        structure,
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
            id: CommitmentSlotId(u32::try_from(slots.len()).expect("too many commitment slots")),
            owner: None,
            anchor_kind,
            footprint: CommitmentFootprintKind::Unknown,
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
    token_spans: &[Option<ByteSpan>],
    recorded: &[RecordedCommitmentSlot],
    nodes: &[SourceNode],
) -> Vec<CommitmentSlotSyntax> {
    let anchor_spans = recorded
        .iter()
        .map(|slot| token_spans.get(slot.anchor_token).copied().flatten())
        .collect::<Vec<_>>();
    let owner_offsets = anchor_spans
        .iter()
        .map(|span| span.map(|span| span.start))
        .collect::<Vec<_>>();
    let owners = most_specific_nodes_at(nodes, &owner_offsets);

    recorded
        .iter()
        .enumerate()
        .filter_map(|(id, slot)| {
            let anchor_span = anchor_spans.get(id).copied().flatten()?;
            let suffix_span = if slot.suffix_tokens.is_empty() {
                ByteSpan {
                    start: anchor_span.end,
                    end: anchor_span.end,
                }
            } else {
                let first = token_spans
                    .get(slot.suffix_tokens.start)
                    .copied()
                    .flatten()?;
                let last = token_spans
                    .get(slot.suffix_tokens.end.checked_sub(1)?)
                    .copied()
                    .flatten()?;
                ByteSpan {
                    start: first.start,
                    end: last.end,
                }
            };
            let owner = owners.get(id).copied().flatten();
            let anchor_kind = match slot.kind {
                RecordedSlotKind::Keyword => CommitmentAnchorKind::Keyword,
                RecordedSlotKind::Identifier => CommitmentAnchorKind::Identifier,
                RecordedSlotKind::String => CommitmentAnchorKind::String,
                RecordedSlotKind::Value => CommitmentAnchorKind::Value,
            };
            Some(CommitmentSlotSyntax {
                id: CommitmentSlotId(u32::try_from(id).ok()?),
                owner,
                anchor_kind,
                footprint: classify_commitment_footprint(
                    source,
                    nodes,
                    owner,
                    anchor_kind,
                    anchor_span,
                ),
                anchor_span,
                suffix_span,
                full_span: ByteSpan {
                    start: anchor_span.start,
                    end: suffix_span.end,
                },
                value: slot.value,
                adjacent: anchor_span.end == suffix_span.start,
                semantic_slot: true,
            })
        })
        .collect()
}

fn classify_commitment_footprint(
    source: &str,
    nodes: &[SourceNode],
    owner: Option<SourceNodeId>,
    anchor_kind: CommitmentAnchorKind,
    anchor_span: ByteSpan,
) -> CommitmentFootprintKind {
    let anchor = source.get(anchor_span.as_range()).unwrap_or_default();
    let owner = owner.and_then(|id| nodes.get(id.0 as usize));

    if let Some(owner) = owner {
        if owner.kind == SourceNodeKind::FlowArm {
            let header = source
                .get(owner.spans.header.as_range())
                .unwrap_or_default();
            let arrow = header
                .find(">>>")
                .map(|index| owner.spans.header.start as usize + index);
            if arrow.is_some_and(|arrow| (anchor_span.start as usize) < arrow)
                && (anchor == "on"
                    || matches!(
                        anchor_kind,
                        CommitmentAnchorKind::Identifier | CommitmentAnchorKind::String
                    ))
            {
                return CommitmentFootprintKind::Event;
            }
        }
    }

    if matches!(anchor, "...") {
        return CommitmentFootprintKind::Placeholder;
    }
    if matches!(anchor, "true" | "false") {
        return CommitmentFootprintKind::Value;
    }
    if matches!(
        anchor_kind,
        CommitmentAnchorKind::String | CommitmentAnchorKind::Value
    ) {
        return CommitmentFootprintKind::Value;
    }
    if anchor_kind == CommitmentAnchorKind::Identifier {
        return CommitmentFootprintKind::NameOrReference;
    }

    if matches!(anchor, "requires" | "ensures") {
        return CommitmentFootprintKind::Clause;
    }
    if anchor == "on" && owner.is_some_and(|node| node.kind == SourceNodeKind::FlowArm) {
        return CommitmentFootprintKind::Event;
    }
    if anchor == ">>>" {
        return CommitmentFootprintKind::Transition;
    }
    if matches!(
        anchor,
        "and"
            | "or"
            | "not"
            | "in"
            | "=="
            | "!="
            | "<"
            | ">"
            | "<="
            | ">="
            | "+"
            | "-"
            | "*"
            | "/"
            | "**"
            | "@"
            | "&"
            | "|"
            | "^"
            | "~"
            | "<<"
            | ">>"
    ) {
        return CommitmentFootprintKind::ExpressionOperator;
    }
    CommitmentFootprintKind::EntityKind
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

fn map_token_spans(
    source: &str,
    pieces: &[SourcePiece],
    line_index: &LineIndex,
    tokens: &[Token],
) -> Vec<Option<ByteSpan>> {
    tokens
        .iter()
        .map(|token| {
            let offset = token_offset(source, line_index, token)?;
            let index = pieces.partition_point(|piece| piece.span.end as usize <= offset);
            pieces.get(index).and_then(|piece| {
                (piece.span.start as usize <= offset
                    && offset < piece.span.end as usize
                    && !piece.kind.is_trivia())
                .then_some(piece.span)
            })
        })
        .collect()
}

fn most_specific_nodes_at(
    nodes: &[SourceNode],
    offsets: &[Option<u32>],
) -> Vec<Option<SourceNodeId>> {
    let mut ordered_nodes = nodes.iter().collect::<Vec<_>>();
    ordered_nodes.sort_by_key(|node| (node.spans.header.start, node.id.0));

    let mut queries = offsets
        .iter()
        .enumerate()
        .filter_map(|(index, offset)| offset.map(|offset| (offset, index)))
        .collect::<Vec<_>>();
    queries.sort_by_key(|(offset, index)| (*offset, *index));

    let mut owners = vec![None; offsets.len()];
    let mut active = BTreeSet::<(u32, u32)>::new();
    let mut endings = BinaryHeap::<Reverse<(u32, u32, u32)>>::new();
    let mut next_node = 0usize;

    for (offset, query_index) in queries {
        while next_node < ordered_nodes.len()
            && ordered_nodes[next_node].spans.header.start <= offset
        {
            let node = ordered_nodes[next_node];
            if node.spans.core.end > offset {
                active.insert((node.spans.core.len(), node.id.0));
                endings.push(Reverse((
                    node.spans.core.end,
                    node.spans.core.len(),
                    node.id.0,
                )));
            }
            next_node += 1;
        }
        while endings
            .peek()
            .is_some_and(|Reverse((end, _, _))| *end <= offset)
        {
            let Reverse((_, len, id)) = endings.pop().expect("peeked ending must exist");
            active.remove(&(len, id));
        }
        owners[query_index] = active.first().map(|(_, id)| SourceNodeId(*id));
    }

    owners
}

fn map_recorded_nodes(
    source: &str,
    tokens: &[Token],
    token_spans: &[Option<ByteSpan>],
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
                token_spans.get(token_index).copied().flatten()
            });
            let concrete = concrete.collect::<Vec<_>>();
            let first = concrete.first()?;
            let last = concrete.last()?;
            let core = ByteSpan {
                start: first.start,
                end: last.end,
            };
            let header_end = source[first.start as usize..]
                .find(['\r', '\n'])
                .map_or(source.len(), |relative| first.start as usize + relative);
            let header = ByteSpan::new(first.start as usize, header_end);
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
        RecordedNodeKind::Rule => SourceNodeKind::Rule,
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
        RecordedNodeKind::Desc => SourceNodeKind::Desc,
        RecordedNodeKind::Clause => SourceNodeKind::Clause,
        RecordedNodeKind::Math => SourceNodeKind::Math,
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
    token_spans: &[Option<ByteSpan>],
    recorded: &[RecordedRuleOccurrence],
    nodes: &[SourceNode],
) -> Vec<RuleOccurrence> {
    recorded
        .iter()
        .enumerate()
        .filter_map(|(id, rule)| {
            let span = token_range_span(token_spans, rule.tokens.clone())?;
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
            let target_anchor =
                target_token.and_then(|token| token_anchor_span(token_spans, token));
            let scope_anchor = scope_token.and_then(|token| token_anchor_span(token_spans, token));
            let target = target_anchor.and_then(|anchor| {
                nodes
                    .iter()
                    .filter(|node| node.spans.header.start == anchor.start)
                    .max_by_key(|node| node.id.0)
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

fn token_range_span(token_spans: &[Option<ByteSpan>], range: Range<usize>) -> Option<ByteSpan> {
    let mut spans = range.filter_map(|token_index| token_anchor_span(token_spans, token_index));
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

fn token_anchor_span(token_spans: &[Option<ByteSpan>], token_index: usize) -> Option<ByteSpan> {
    token_spans.get(token_index).copied().flatten()
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
    let range = span.start as usize..span.end as usize;
    if range.end > source.len() {
        return "";
    }
    &source[range]
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
    fn semantic_commitment_slots_have_stable_ids_owners_and_footprints() {
        let source = r#"rule$ "audit"?
func?? Pay$:
    requires$: ready
    steps:
        desc? "charge"??

flow:
    Pending:
        on? Capture$ >>>$? Paid$: desc "done"
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        for (index, slot) in result.document.commitment_slots().iter().enumerate() {
            assert_eq!(slot.id, CommitmentSlotId(index as u32));
            assert!(slot.owner.is_some(), "owner missing for {slot:?}");
            assert_ne!(slot.footprint, CommitmentFootprintKind::Unknown);
            assert_eq!(result.document.commitment_slot(slot.id), Some(slot));
        }
        assert!(result.document.nodes().iter().any(|node| {
            node.kind == SourceNodeKind::Rule
                && result.document.text(node.spans.header) == Some("rule$ \"audit\"?")
        }));
    }

    #[test]
    fn batched_slot_owner_lookup_matches_exhaustive_containment() {
        let source = r#"module App:
    func Pay(order):
        requires: order.ready
        steps:
            if order.ready:
                charge payment
            else:
                reject payment
    flow:
        Pending:
            on Approved >>> Paid: desc "settled"
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let mut offsets = result
            .document
            .commitment_slots()
            .iter()
            .map(|slot| Some(slot.anchor_span.start))
            .collect::<Vec<_>>();
        offsets.extend(result.document.nodes().iter().flat_map(|node| {
            [
                Some(node.spans.header.start),
                node.spans.core.end.checked_sub(1),
            ]
        }));
        offsets.push(None);

        let batched = most_specific_nodes_at(result.document.nodes(), &offsets);
        for (offset, actual) in offsets.into_iter().zip(batched) {
            let expected =
                offset.and_then(|offset| most_specific_node_at(result.document.nodes(), offset));
            assert_eq!(actual, expected, "offset {offset:?}");
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
    fn line_index_treats_lf_crlf_and_lone_cr_as_single_line_breaks() {
        for source in ["a\nb", "a\r\nb", "a\rb"] {
            let index = LineIndex::new(source);
            let b = source.rfind('b').unwrap() as u32;
            assert_eq!(
                index.position(source, b, ColumnEncoding::Utf16),
                Some(SourcePosition { line: 1, column: 0 }),
                "{source:?}"
            );
            assert_eq!(
                index.offset(
                    source,
                    SourcePosition { line: 1, column: 0 },
                    ColumnEncoding::Utf16
                ),
                Some(b),
                "{source:?}"
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

    #[test]
    fn move_fragment_carries_attached_rule_prelude() {
        let source = r#"rule "must audit"
func Pay:
    steps:
        charge payment

type Status: Active | Paid
"#;
        let original = parse_lossless(source);
        assert!(original.errors.is_empty(), "{:?}", original.errors);
        let func = original
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::Func)
            .unwrap()
            .id;
        let typedef = original
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::TypeDef)
            .unwrap()
            .id;

        let moved = original
            .document
            .move_fragment_reparse(func, MoveDestination::After(typedef))
            .expect("move should succeed");
        assert!(moved.errors.is_empty(), "{:?}", moved.errors);

        let text = moved.document.render_lossless();
        assert!(
            text.find("type Status").unwrap() < text.find("rule \"must audit\"").unwrap(),
            "func+rule should follow typedef:\n{text}"
        );
        assert!(
            text.find("rule \"must audit\"").unwrap() < text.find("func Pay").unwrap(),
            "rule prelude must stay in front of func:\n{text}"
        );

        let pay_rule = moved
            .document
            .rules()
            .iter()
            .find(|rule| moved.document.text(rule.span) == Some("rule \"must audit\""))
            .unwrap();
        assert_eq!(pay_rule.attachment, RuleAttachment::Attached);
        let target = moved.document.node(pay_rule.target.unwrap()).unwrap();
        assert_eq!(target.kind, SourceNodeKind::Func);
    }

    #[test]
    fn move_rejects_nested_and_self_destination() {
        let source = r#"type User:
    name: String

func Pay: ...
"#;
        let original = parse_lossless(source);
        let field = original
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::Field)
            .unwrap()
            .id;
        let func = original
            .document
            .nodes()
            .iter()
            .find(|node| node.kind == SourceNodeKind::Func)
            .unwrap()
            .id;
        assert_eq!(
            original
                .document
                .move_fragment(field, MoveDestination::After(func)),
            Err(MoveError::NotMovableFragment)
        );
        assert_eq!(
            original
                .document
                .move_fragment(func, MoveDestination::Before(func)),
            Err(MoveError::InvalidDestination)
        );
    }

    #[test]
    fn semantic_render_preserves_rule_attachment_and_suffixes() {
        let source = r#"rule "audit payments"
func Pay$:
    steps:
        charge payment

rule "environment only"

type Status?: Active | Paid
"#;
        let before = parse_lossless(source);
        assert!(before.errors.is_empty(), "{:?}", before.errors);
        let mut fingerprint = before.document.rule_attachment_fingerprint();
        fingerprint.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(format!("{:?}", left.1).cmp(&format!("{:?}", right.1)))
        });
        let mut commitments = before.document.commitment_fingerprint();
        commitments.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(format!("{:?}", left.1).cmp(&format!("{:?}", right.1)))
                .then(left.2.cmp(&right.2))
        });

        let rendered = crate::render::render_file(before.document.semantic());
        let after = parse_lossless(&rendered);
        assert!(after.errors.is_empty(), "{:?}", after.errors);

        let mut after_fingerprint = after.document.rule_attachment_fingerprint();
        after_fingerprint.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(format!("{:?}", left.1).cmp(&format!("{:?}", right.1)))
        });
        let mut after_commitments = after.document.commitment_fingerprint();
        after_commitments.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(format!("{:?}", left.1).cmp(&format!("{:?}", right.1)))
                .then(left.2.cmp(&right.2))
        });

        assert_eq!(
            after_fingerprint, fingerprint,
            "formatter changed rule attachment\nbefore:\n{source}\nafter:\n{rendered}"
        );
        assert_eq!(
            after_commitments, commitments,
            "formatter changed commitment suffixes\nbefore:\n{source}\nafter:\n{rendered}"
        );

        // Environment rules stay detached; attached rules stay attached.
        assert!(after.document.rules().iter().any(|rule| {
            after.document.text(rule.span) == Some("rule \"audit payments\"")
                && rule.attachment == RuleAttachment::Attached
        }));
        assert!(after.document.rules().iter().any(|rule| {
            after.document.text(rule.span) == Some("rule \"environment only\"")
                && rule.attachment == RuleAttachment::Environment
        }));
    }

    #[test]
    fn structural_index_exposes_revision_local_hierarchy_slots_and_rules() {
        let source = r#"module App:
    rule "must report failure"
    func Load?:
        steps:
            desc?? "读取文件"
"#;
        let parsed = parse_lossless(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let document = &parsed.document;
        let module = document
            .nodes_of_kind(SourceNodeKind::Module)
            .next()
            .expect("module node")
            .id;
        let func = document
            .nodes_of_kind(SourceNodeKind::Func)
            .next()
            .expect("func node")
            .id;
        let desc = document
            .commitment_slots()
            .iter()
            .find(|slot| slot.semantic_slot && slot.value == Commitment::QuestionQuestion)
            .and_then(|slot| slot.owner)
            .expect("delegated desc owner");

        assert_eq!(document.parent_node(func).map(|node| node.id), Some(module));
        assert_eq!(document.parent_node(desc).map(|node| node.id), Some(func));
        assert!(document.child_nodes(module).any(|node| node.id == func));
        assert_eq!(document.scope_path(func), Some([module, func].as_slice()));
        assert_eq!(document.scope_path(desc), Some([module, func].as_slice()));
        assert!(document
            .structural_topology(module)
            .is_some_and(|topology| topology.contains("Func:Load")));
        assert!(document
            .structural_position(func)
            .is_some_and(|position| position.starts_with("Module/")));

        let func_slots = document
            .commitment_slots_for_owner(func)
            .collect::<Vec<_>>();
        assert!(func_slots
            .iter()
            .any(|slot| slot.value == Commitment::Question));
        let desc_slots = document
            .commitment_slots_for_owner(desc)
            .collect::<Vec<_>>();
        assert!(desc_slots
            .iter()
            .any(|slot| slot.value == Commitment::QuestionQuestion));

        let attached = document.rules_for_target(func).collect::<Vec<_>>();
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0].attachment, RuleAttachment::Attached);
    }
}
