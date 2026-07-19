use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind, RecordedRuleDecision};

impl Parser {
    pub(super) fn parse_step(&mut self) -> Result<Step, ParseError> {
        let start = self.pos;
        let step = match self.peek_kind() {
            Some(TokenKind::If) => self.parse_if_step(),
            Some(TokenKind::For) => self.parse_for_step(),
            Some(TokenKind::While) => self.parse_while_step(),
            Some(TokenKind::Parasteps) => self.parse_parasteps_step(),
            Some(TokenKind::Error) => self.parse_error_step(),
            Some(TokenKind::Ellipsis) => self.parse_placeholder_step(),
            Some(TokenKind::Desc) => self.parse_desc_step(),
            _ => self.parse_action_step(),
        }?;
        self.record_source_node(start..self.pos, RecordedNodeKind::Step);
        Ok(step)
    }

    fn parse_desc_step(&mut self) -> Result<Step, ParseError> {
        let content = self.parse_desc_step_entity()?;
        Ok(Step::Desc { content })
    }

    fn parse_placeholder_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
        Ok(Step::Placeholder { keyword_commitment })
    }

    fn parse_if_step(&mut self) -> Result<Step, ParseError> {
        let then_scope_token = self.pos;
        let if_keyword_commitment = self.expect_kw(TokenKind::If, "`if`")?;
        let cond = self.parse_condition()?;
        self.expect(TokenKind::Colon, "`:`")?;
        let then_branch = self.parse_step_block(then_scope_token);
        let mut else_branch = None;
        let mut else_keyword_commitment = Commitment::None;
        self.skip_newlines();
        if self.check(&TokenKind::Else) {
            let else_scope_token = self.pos;
            else_keyword_commitment = self.expect_kw(TokenKind::Else, "`else`")?;
            self.expect(TokenKind::Colon, "`:`")?;
            else_branch = Some(self.parse_step_block(else_scope_token));
        }
        Ok(Step::If {
            step: IfStep {
                cond,
                then_branch,
                else_branch,
                if_keyword_commitment,
                else_keyword_commitment,
            },
        })
    }

    fn parse_for_step(&mut self) -> Result<Step, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::For, "`for`")?;
        let var = self.fuzzy_ident()?;
        self.expect(TokenKind::In, "`in`")?;
        let iterable = self.parse_atoms_until(&[
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;
        self.expect(TokenKind::Colon, "`:`")?;
        let body = self.parse_step_block(scope_token);
        Ok(Step::For {
            step: ForStep {
                var,
                iterable,
                body,
                keyword_commitment,
            },
        })
    }

    fn parse_while_step(&mut self) -> Result<Step, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::While, "`while`")?;
        let cond = self.parse_condition()?;
        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let body = self.parse_step_block(scope_token);
        Ok(Step::While {
            step: WhileStep {
                cond,
                desc,
                body,
                keyword_commitment,
            },
        })
    }

    fn parse_parasteps_step(&mut self) -> Result<Step, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Parasteps, "`parasteps`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_step_block(scope_token);
        Ok(Step::Parasteps {
            step: ParastepsStep {
                description,
                steps,
                keyword_commitment,
            },
        })
    }

    fn parse_error_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Error, "`error`")?;
        let message = self.fuzzy_string()?;
        let to = if self.matches(&TokenKind::Arrow) {
            Some(ToTarget {
                target: self.fuzzy_ident()?,
            })
        } else {
            None
        };
        Ok(Step::Error {
            step: ErrorStep {
                message,
                to,
                keyword_commitment,
            },
        })
    }

    fn parse_action_step(&mut self) -> Result<Step, ParseError> {
        let atoms = self.parse_atoms_until(&[
            TokenKind::Desc,
            TokenKind::Arrow,
            TokenKind::On,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;

        if let Some(eq_idx) = atoms
            .iter()
            .position(|a| matches!(a, Atom::Symbol { value } if value == "="))
        {
            let lhs = &atoms[..eq_idx];
            let rhs = &atoms[eq_idx + 1..];
            let (err_line, err_col) = match self.peek() {
                Some(t) => (t.line, t.col),
                None => (0, 0),
            };
            if rhs
                .iter()
                .any(|a| matches!(a, Atom::Symbol { value } if value == "="))
            {
                return Err(ParseError::unexpected_token(
                    "=".into(),
                    "single assignment per step".into(),
                    err_line,
                    err_col,
                ));
            }
            let target = Self::parse_target_from_atoms(lhs, err_line, err_col)?;
            let value = Self::parse_simple_value_from_atoms(rhs, err_line, err_col)?;

            let desc = if self.matches(&TokenKind::Desc) {
                Some(self.parse_desc_after_keyword()?)
            } else {
                None
            };

            let to = if self.matches(&TokenKind::Arrow) {
                Some(ToTarget {
                    target: self.fuzzy_ident()?,
                })
            } else {
                None
            };

            let mut on_blocks = Vec::new();
            loop {
                self.skip_newlines();
                if self.check(&TokenKind::On) {
                    on_blocks.push(self.parse_on_block()?);
                } else {
                    break;
                }
            }

            return Ok(Step::Assign {
                step: AssignStep {
                    target,
                    value,
                    desc,
                    to,
                    on_blocks,
                },
            });
        }

        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };

        let to = if self.matches(&TokenKind::Arrow) {
            Some(ToTarget {
                target: self.fuzzy_ident()?,
            })
        } else {
            None
        };

        let mut on_blocks = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::On) {
                on_blocks.push(self.parse_on_block()?);
            } else {
                break;
            }
        }

        Ok(Step::Action {
            step: ActionStep {
                label: atoms,
                desc,
                to,
                on_blocks,
            },
        })
    }

    fn parse_on_block(&mut self) -> Result<OnBlock, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::On, "`on`")?;
        let condition = self.parse_atoms_until(&[
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_step_block(scope_token);
        Ok(OnBlock {
            condition,
            steps,
            keyword_commitment,
        })
    }

    /// Parse any nested step scope using the same ordered-item and rule
    /// attachment semantics as a standalone `steps:` block.
    pub(super) fn parse_step_block(&mut self, scope_token: usize) -> Vec<Fragment> {
        self.skip_newlines();
        if let Err(error) = self.expect(TokenKind::Indent, "indented block") {
            self.emit_error(error);
            return Vec::new();
        }
        if let Err(error) = self.enter_block() {
            self.emit_error(error);
            return Vec::new();
        }

        let mut items = Vec::new();
        let mut pending_rules: Vec<(usize, Option<usize>)> = Vec::new();
        loop {
            let newline_count = self.skip_newlines_and_count();
            if newline_count >= 3 {
                self.resolve_semantic_rules(
                    &mut items,
                    &mut pending_rules,
                    RuleAttachment::Environment,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
            }

            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                self.resolve_semantic_rules(
                    &mut items,
                    &mut pending_rules,
                    RuleAttachment::Environment,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                break;
            }

            if self.check(&TokenKind::Rule) {
                match self.parse_rule_def() {
                    Ok((rule, record_id)) => {
                        let index = items.len();
                        items.push(Fragment::Rule { rule });
                        pending_rules.push((index, record_id));
                    }
                    Err(error) => {
                        self.emit_error(error);
                        self.try_sync(|parser| parser.synchronize_to_next_item_in_block());
                    }
                }
                continue;
            }

            let target_token = self.pos;
            match self.parse_step() {
                Ok(step) => {
                    let target_index = items.len();
                    self.resolve_semantic_rules(
                        &mut items,
                        &mut pending_rules,
                        RuleAttachment::Attached { target_index },
                        RecordedRuleDecision::Attached { target_token },
                    );
                    items.push(Fragment::Step { step });
                }
                Err(error) => {
                    self.resolve_semantic_rules(
                        &mut items,
                        &mut pending_rules,
                        RuleAttachment::UnresolvedByRecovery,
                        RecordedRuleDecision::DroppedByRecovery,
                    );
                    self.emit_error(error);
                    self.try_sync(|parser| parser.synchronize_to_next_item_in_block());
                }
            }
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        self.leave_block();
        items
    }
}
