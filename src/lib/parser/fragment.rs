use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind, RecordedSlotKind};

impl Parser {
    pub(crate) fn parse_fragment(&mut self) -> Result<Fragment, ParseError> {
        let start = self.pos;
        let fragment = match self.peek_kind() {
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
            Some(TokenKind::Stack) | Some(TokenKind::Parallel) => Ok(Fragment::UiNode {
                node: self.parse_ui_node()?,
            }),
            Some(TokenKind::String(_)) => Ok(Fragment::UiNode {
                node: self.parse_ui_node()?,
            }),
            Some(TokenKind::Ellipsis) => {
                let keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
                Ok(Fragment::Placeholder { keyword_commitment })
            }
            Some(TokenKind::If)
            | Some(TokenKind::For)
            | Some(TokenKind::While)
            | Some(TokenKind::Parasteps) => {
                let step = self.parse_step()?;
                Ok(Fragment::Steps {
                    keyword_commitment: Self::step_keyword_commitment(&step),
                    steps: vec![step],
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
                    steps: vec![step],
                })
            }
        }?;
        let kind = match &fragment {
            Fragment::Module { .. } => RecordedNodeKind::Module,
            Fragment::TypeDef { .. } => RecordedNodeKind::TypeDef,
            Fragment::Flow { .. } => RecordedNodeKind::Flow,
            Fragment::Func { .. } => RecordedNodeKind::Func,
            Fragment::Ui { .. } => RecordedNodeKind::Ui,
            Fragment::Steps { .. } => RecordedNodeKind::Steps,
            Fragment::Expr { .. } => RecordedNodeKind::Expr,
            Fragment::UiNode { .. } => RecordedNodeKind::UiNode,
            Fragment::Placeholder { .. } => RecordedNodeKind::Placeholder,
        };
        self.record_source_node(start..self.pos, kind);
        Ok(fragment)
    }

    fn parse_steps_fragment(&mut self) -> Result<Fragment, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Steps, "`steps`")?;
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_block(|p| p.parse_step());
        Ok(Fragment::Steps {
            keyword_commitment,
            steps,
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
        self.expect(TokenKind::Desc, "`desc`")?;
        let need_commitment = self.commitment_after_previous(RecordedSlotKind::Keyword)?;
        let content = self.fuzzy_string()?;
        Ok(Desc {
            need_commitment,
            content,
        })
    }
}
