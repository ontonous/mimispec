use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_step(&mut self) -> Result<Step, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::If) => self.parse_if_step(),
            Some(TokenKind::For) => self.parse_for_step(),
            Some(TokenKind::While) => self.parse_while_step(),
            Some(TokenKind::Parasteps) => self.parse_parasteps_step(),
            Some(TokenKind::Error) => self.parse_error_step(),
            Some(TokenKind::Ellipsis) => self.parse_placeholder_step(),
            Some(TokenKind::Desc) => self.parse_desc_step(),
            _ => self.parse_action_step(),
        }
    }

    fn parse_desc_step(&mut self) -> Result<Step, ParseError> {
        let content = self.parse_desc_entity()?;
        Ok(Step::Desc { content })
    }

    fn parse_placeholder_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
        Ok(Step::Placeholder { keyword_commitment })
    }

    fn parse_if_step(&mut self) -> Result<Step, ParseError> {
        let if_keyword_commitment = self.expect_kw(TokenKind::If, "`if`")?;
        let cond = self.parse_condition()?;
        self.expect(TokenKind::Colon, "`:`")?;
        let then_branch = self.parse_block(|p| p.parse_step())?;
        let mut else_branch = None;
        let mut else_keyword_commitment = Commitment::None;
        self.skip_newlines();
        if self.check(&TokenKind::Else) {
            else_keyword_commitment = self.expect_kw(TokenKind::Else, "`else`")?;
            self.expect(TokenKind::Colon, "`:`")?;
            else_branch = Some(self.parse_block(|p| p.parse_step())?);
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
        let body = self.parse_block(|p| p.parse_step())?;
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
        let keyword_commitment = self.expect_kw(TokenKind::While, "`while`")?;
        let cond = self.parse_condition()?;
        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let body = self.parse_block(|p| p.parse_step())?;
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
        let keyword_commitment = self.expect_kw(TokenKind::Parasteps, "`parasteps`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_block(|p| p.parse_step())?;
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
                return Err(ParseError::UnexpectedToken {
                    found: "=".into(),
                    expected: "single assignment per step".into(),
                    line: err_line,
                    col: err_col,
                });
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

            if self.check(&TokenKind::Newline) {
                self.advance();
            }

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

        if self.check(&TokenKind::Newline) {
            self.advance();
        }

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
        let keyword_commitment = self.expect_kw(TokenKind::On, "`on`")?;
        let condition = self.parse_atoms_until(&[
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_block(|p| p.parse_step())?;
        Ok(OnBlock {
            condition,
            steps,
            keyword_commitment,
        })
    }
}
