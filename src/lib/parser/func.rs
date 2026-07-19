use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedRuleDecision};

impl Parser {
    pub(super) fn parse_func(&mut self) -> Result<FuncDef, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Func, "`func`")?;
        let name = self.fuzzy_ident()?;
        let params = if self.check(&TokenKind::LParen) {
            self.advance();
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen, "`)`")?;
            params
        } else {
            Vec::new()
        };

        let mut capabilities = Vec::new();
        let mut with_keyword_commitment = Commitment::None;
        if self.check(&TokenKind::With) {
            with_keyword_commitment = self.expect_kw(TokenKind::With, "`with`")?;
            capabilities = self.parse_capabilities()?;
        }

        self.expect(TokenKind::Colon, "`:`")?;
        self.skip_newlines();

        if self.check(&TokenKind::Ellipsis) {
            let placeholder = self.parse_placeholder_item()?;
            if !self.line_will_end() {
                let (found, line, col) = match self.peek() {
                    Some(token) => (token.kind.to_string(), token.line, token.col),
                    None => ("EOF".into(), 0, 0),
                };
                return Err(ParseError::unexpected_token(
                    found,
                    "end of line after `...`".into(),
                    line,
                    col,
                ));
            }
            return Ok(FuncDef {
                name,
                params,
                capabilities,
                items: vec![placeholder],
                keyword_commitment,
                with_keyword_commitment,
            });
        }

        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

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
            let parsed = if self.check(&TokenKind::Desc) {
                self.parse_desc_entity().map(|desc| Fragment::Desc { desc })
            } else if self.check(&TokenKind::Requires) {
                self.parse_clause(ClauseKind::Requires)
                    .map(|clause| Fragment::Clause { clause })
            } else if self.check(&TokenKind::Ensures) {
                self.parse_clause(ClauseKind::Ensures)
                    .map(|clause| Fragment::Clause { clause })
            } else if self.check(&TokenKind::Math) {
                self.parse_math_block().map(|math| Fragment::Math { math })
            } else if self.check(&TokenKind::Steps) {
                self.parse_steps_fragment()
            } else if self.check(&TokenKind::Ellipsis) {
                self.parse_placeholder_item()
            } else {
                self.parse_step().map(|step| Fragment::Step { step })
            };

            match parsed {
                Ok(item) => {
                    let target_index = items.len();
                    self.resolve_semantic_rules(
                        &mut items,
                        &mut pending_rules,
                        RuleAttachment::Attached { target_index },
                        RecordedRuleDecision::Attached { target_token },
                    );
                    items.push(item);
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

        Ok(FuncDef {
            name,
            params,
            capabilities,
            items,
            keyword_commitment,
            with_keyword_commitment,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let name = self.fuzzy_ident()?;
            let mut type_hint = Vec::new();
            if self.matches(&TokenKind::Colon) {
                type_hint = self.parse_atoms_until(&[
                    TokenKind::Comma,
                    TokenKind::RParen,
                    TokenKind::Newline,
                    TokenKind::Eof,
                ])?;
            }
            params.push(Param { name, type_hint });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    pub(super) fn parse_capabilities(&mut self) -> Result<Vec<Capability>, ParseError> {
        let mut capabilities = Vec::new();
        loop {
            let name = self.fuzzy_ident()?;
            let commitment = name.commitment;
            capabilities.push(Capability { name, commitment });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(capabilities)
    }
}
