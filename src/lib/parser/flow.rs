use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind, RecordedRuleDecision};

impl Parser {
    pub(super) fn parse_flow(&mut self) -> Result<FlowDef, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Flow, "`flow`")?;
        let name = if self.check(&TokenKind::Colon) {
            None
        } else {
            Some(self.fuzzy_ident()?)
        };
        self.expect(TokenKind::Colon, "`:`")?;

        if self.check(&TokenKind::Ellipsis) {
            let placeholder = self.parse_placeholder_item()?;
            return Ok(FlowDef {
                name,
                items: vec![placeholder],
                keyword_commitment,
            });
        }

        self.skip_newlines();
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
            } else if self.check(&TokenKind::Ellipsis) {
                self.parse_placeholder_item()
            } else {
                self.parse_flow_entry()
                    .map(|entry| Fragment::FlowEntry { entry })
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

        Ok(FlowDef {
            name,
            items,
            keyword_commitment,
        })
    }

    fn parse_flow_entry(&mut self) -> Result<FlowEntry, ParseError> {
        let start = self.pos;
        let state = self.fuzzy_ident()?;
        let items = if self.check(&TokenKind::Arrow) || self.check(&TokenKind::On) {
            vec![Fragment::FlowArm {
                arm: self.parse_flow_arm()?,
            }]
        } else {
            self.expect(TokenKind::Colon, "`:`")?;
            self.parse_flow_arm_block()?
        };
        self.record_source_node(start..self.pos, RecordedNodeKind::FlowEntry);
        Ok(FlowEntry { state, items })
    }

    fn parse_flow_arm_block(&mut self) -> Result<Vec<Fragment>, ParseError> {
        let scope_token = self.pos.saturating_sub(1);
        self.skip_newlines();
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
            match self.parse_flow_arm() {
                Ok(arm) => {
                    let target_index = items.len();
                    self.resolve_semantic_rules(
                        &mut items,
                        &mut pending_rules,
                        RuleAttachment::Attached { target_index },
                        RecordedRuleDecision::Attached { target_token },
                    );
                    items.push(Fragment::FlowArm { arm });
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
        Ok(items)
    }

    fn parse_flow_arm(&mut self) -> Result<FlowArm, ParseError> {
        let start = self.pos;
        let event = if self.check(&TokenKind::On) {
            let keyword_commitment = self.expect_kw(TokenKind::On, "`on`")?;
            let name = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
                EventName::Natural {
                    text: self.fuzzy_string()?,
                }
            } else {
                EventName::Ident {
                    value: self.fuzzy_ident()?,
                }
            };
            Some(FlowEvent {
                keyword_commitment,
                name,
            })
        } else {
            None
        };

        let to_keyword_commitment = self.expect_kw(TokenKind::Arrow, "`>>>`")?;
        let to = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        let mut items = Vec::new();
        while !self.line_will_end() {
            if self.check(&TokenKind::Requires) {
                let clause = self.parse_flow_tail_clause(ClauseKind::Requires)?;
                items.push(Fragment::Clause { clause });
            } else if self.check(&TokenKind::Ensures) {
                let clause = self.parse_flow_tail_clause(ClauseKind::Ensures)?;
                items.push(Fragment::Clause { clause });
            } else if self.check(&TokenKind::Desc) {
                let desc = self.parse_desc_entity()?;
                items.push(Fragment::Desc { desc });
            } else {
                let (found, line, col) = match self.peek() {
                    Some(token) => (token.kind.to_string(), token.line, token.col),
                    None => ("EOF".into(), 0, 0),
                };
                return Err(ParseError::unexpected_token(
                    found,
                    "`requires`, `ensures`, or `desc`".into(),
                    line,
                    col,
                ));
            }
        }
        self.record_source_node(start..self.pos, RecordedNodeKind::FlowArm);
        Ok(FlowArm {
            event,
            to,
            items,
            to_keyword_commitment,
        })
    }
}
