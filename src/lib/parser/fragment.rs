use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind, RecordedSlotKind};

impl Parser {
    pub(crate) fn parse_fragment(&mut self) -> Result<Fragment, ParseError> {
        let start = self.pos;
        let fragment = match self.peek_kind() {
            Some(TokenKind::Desc) => Ok(Fragment::Desc {
                desc: self.parse_desc_entity()?,
            }),
            Some(TokenKind::Requires) => Ok(Fragment::Clause {
                clause: self.parse_clause(ClauseKind::Requires)?,
            }),
            Some(TokenKind::Ensures) => Ok(Fragment::Clause {
                clause: self.parse_clause(ClauseKind::Ensures)?,
            }),
            Some(TokenKind::Module) => Ok(Fragment::Module {
                module: self.parse_module()?,
            }),
            Some(TokenKind::Type) => Ok(Fragment::TypeDef {
                typedef: self.parse_type_def()?,
            }),
            Some(TokenKind::Flow) => Ok(Fragment::Flow {
                flow: self.parse_flow()?,
            }),
            Some(TokenKind::Func) => Ok(Fragment::Func {
                func: self.parse_func()?,
            }),
            Some(TokenKind::Ui) => Ok(Fragment::Ui {
                ui: self.parse_ui()?,
            }),
            Some(TokenKind::Steps) => self.parse_steps_fragment(),
            Some(TokenKind::Math) => Ok(Fragment::Math {
                math: self.parse_math_block()?,
            }),
            Some(TokenKind::Stack) | Some(TokenKind::Parallel) => Ok(Fragment::UiNode {
                node: self.parse_ui_node()?,
            }),
            Some(TokenKind::String(_)) => Ok(Fragment::UiNode {
                node: self.parse_ui_node()?,
            }),
            Some(TokenKind::Ellipsis) => self.parse_placeholder_item(),
            Some(TokenKind::If)
            | Some(TokenKind::For)
            | Some(TokenKind::While)
            | Some(TokenKind::Parasteps) => {
                let step = self.parse_step()?;
                Ok(Fragment::Steps {
                    keyword_commitment: Self::step_keyword_commitment(&step),
                    items: vec![Fragment::Step { step }],
                })
            }
            _ => {
                let save = self.pos;
                let source_checkpoint = self.source_checkpoint();
                if let Ok(expr) = self.parse_expr(0) {
                    if self.line_will_end() {
                        return Ok(Fragment::Expr { expr });
                    }
                }
                self.pos = save;
                self.restore_source_checkpoint(source_checkpoint);
                let step = self.parse_step()?;
                Ok(Fragment::Steps {
                    keyword_commitment: Self::step_keyword_commitment(&step),
                    items: vec![Fragment::Step { step }],
                })
            }
        }?;
        let kind = match &fragment {
            Fragment::Desc { .. } => RecordedNodeKind::Desc,
            Fragment::Clause { .. } => RecordedNodeKind::Clause,
            Fragment::Module { .. } => RecordedNodeKind::Module,
            Fragment::TypeDef { .. } => RecordedNodeKind::TypeDef,
            Fragment::Flow { .. } => RecordedNodeKind::Flow,
            Fragment::Func { .. } => RecordedNodeKind::Func,
            Fragment::Ui { .. } => RecordedNodeKind::Ui,
            Fragment::Steps { .. } => RecordedNodeKind::Steps,
            Fragment::Expr { .. } => RecordedNodeKind::Expr,
            Fragment::UiNode { .. } => RecordedNodeKind::UiNode,
            Fragment::Math { .. } => RecordedNodeKind::Math,
            Fragment::Placeholder { .. } => RecordedNodeKind::Placeholder,
            Fragment::Rule { .. }
            | Fragment::Step { .. }
            | Fragment::Field { .. }
            | Fragment::Variants { .. }
            | Fragment::FlowEntry { .. }
            | Fragment::FlowArm { .. } => {
                unreachable!("body-only item dispatched as a root fragment")
            }
        };
        if !matches!(
            fragment,
            Fragment::Desc { .. }
                | Fragment::Clause { .. }
                | Fragment::UiNode { .. }
                | Fragment::Math { .. }
                | Fragment::Placeholder { .. }
        ) {
            self.record_source_node(start..self.pos, kind);
        }
        Ok(fragment)
    }

    pub(super) fn parse_steps_fragment(&mut self) -> Result<Fragment, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Steps, "`steps`")?;
        self.expect(TokenKind::Colon, "`:`")?;
        let items = self.parse_step_block(scope_token);

        Ok(Fragment::Steps {
            keyword_commitment,
            items,
        })
    }

    fn step_keyword_commitment(step: &Step) -> Commitment {
        match step {
            Step::If { step } => step.if_keyword_commitment,
            Step::For { step } => step.keyword_commitment,
            Step::While { step } => step.keyword_commitment,
            Step::Parasteps { step } => step.keyword_commitment,
            _ => Commitment::None,
        }
    }

    pub(super) fn parse_desc_entity(&mut self) -> Result<Desc, ParseError> {
        let start = self.pos;
        self.expect(TokenKind::Desc, "`desc`")?;
        let need_commitment = self.commitment_after_previous(RecordedSlotKind::Keyword)?;
        let content = self.fuzzy_string()?;
        let desc = Desc {
            need_commitment,
            content,
        };
        self.record_source_node(start..self.pos, RecordedNodeKind::Desc);
        Ok(desc)
    }

    pub(super) fn parse_desc_step_entity(&mut self) -> Result<Desc, ParseError> {
        self.expect(TokenKind::Desc, "`desc`")?;
        let need_commitment = self.commitment_after_previous(RecordedSlotKind::Keyword)?;
        let content = self.fuzzy_string()?;
        Ok(Desc {
            need_commitment,
            content,
        })
    }

    pub(super) fn parse_placeholder_item(&mut self) -> Result<Fragment, ParseError> {
        let start = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
        self.record_source_node(start..self.pos, RecordedNodeKind::Placeholder);
        Ok(Fragment::Placeholder { keyword_commitment })
    }

    pub(super) fn parse_clause(&mut self, clause_kind: ClauseKind) -> Result<Clause, ParseError> {
        self.parse_clause_with_flow_tail(clause_kind, false)
    }

    pub(super) fn parse_flow_tail_clause(
        &mut self,
        clause_kind: ClauseKind,
    ) -> Result<Clause, ParseError> {
        self.parse_clause_with_flow_tail(clause_kind, true)
    }

    fn parse_clause_with_flow_tail(
        &mut self,
        clause_kind: ClauseKind,
        allow_following_flow_tail: bool,
    ) -> Result<Clause, ParseError> {
        let start = self.pos;
        let token = match clause_kind {
            ClauseKind::Requires => TokenKind::Requires,
            ClauseKind::Ensures => TokenKind::Ensures,
        };
        let label = match clause_kind {
            ClauseKind::Requires => "`requires`",
            ClauseKind::Ensures => "`ensures`",
        };
        let keyword_commitment = self.expect_kw(token, label)?;
        self.expect(TokenKind::Colon, "`:`")?;
        let condition = self.parse_condition()?;
        let followed_by_flow_tail = allow_following_flow_tail
            && matches!(
                self.peek_kind(),
                Some(TokenKind::Requires | TokenKind::Ensures | TokenKind::Desc)
            );
        if !self.line_will_end() && !followed_by_flow_tail {
            let (found, line, col) = match self.peek() {
                Some(token) => (token.kind.to_string(), token.line, token.col),
                None => ("EOF".into(), 0, 0),
            };
            return Err(ParseError::unexpected_token(
                found,
                format!("newline after {} condition", label),
                line,
                col,
            ));
        }
        let clause = Clause {
            clause_kind,
            condition,
            keyword_commitment,
        };
        self.record_source_node(start..self.pos, RecordedNodeKind::Clause);
        Ok(clause)
    }
}
