use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind, RecordedRuleDecision};

impl Parser {
    pub(super) fn parse_ui(&mut self) -> Result<UiDef, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Ui, "`ui`")?;
        let name = self.fuzzy_ident()?;
        let mut binds = None;
        let mut binds_keyword_commitment = Commitment::None;
        if self.check(&TokenKind::Binds) {
            binds_keyword_commitment = self.expect_kw(TokenKind::Binds, "`binds`")?;
            binds = Some(self.fuzzy_ident()?);
        }
        self.expect(TokenKind::Colon, "`:`")?;
        if self.check(&TokenKind::Ellipsis) {
            let placeholder = self.parse_placeholder_item()?;
            return Ok(UiDef {
                name,
                binds,
                binds_keyword_commitment,
                items: vec![placeholder],
                keyword_commitment,
            });
        }
        let items = self.parse_ui_item_block(scope_token, true)?;
        Ok(UiDef {
            name,
            binds,
            binds_keyword_commitment,
            items,
            keyword_commitment,
        })
    }

    fn parse_ui_item_block(
        &mut self,
        scope_token: usize,
        root_only: bool,
    ) -> Result<Vec<Fragment>, ParseError> {
        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        let mut items = Vec::new();
        let mut pending_rules: Vec<(usize, Option<usize>)> = Vec::new();
        let mut root_count = 0usize;
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
            let parsed = if self.check(&TokenKind::Ellipsis) {
                self.parse_placeholder_item()
            } else if root_only
                && !matches!(
                    self.peek_kind(),
                    Some(TokenKind::Stack | TokenKind::Parallel)
                )
            {
                let (found, line, col) = match self.peek() {
                    Some(token) => (token.kind.to_string(), token.line, token.col),
                    None => ("EOF".into(), 0, 0),
                };
                Err(ParseError::unexpected_token(
                    found,
                    "`stack`, `parallel`, or `...` as the UI root".into(),
                    line,
                    col,
                ))
            } else {
                self.parse_ui_node().map(|node| Fragment::UiNode { node })
            };

            match parsed {
                Ok(item) => {
                    if root_only {
                        root_count += 1;
                        if root_count > 1 {
                            let (line, col) = self
                                .tokens
                                .get(target_token)
                                .map_or((0, 0), |token| (token.line, token.col));
                            self.emit_error(ParseError::unexpected_token(
                                "additional UI root".into(),
                                "exactly one UI root item".into(),
                                line,
                                col,
                            ));
                        }
                    }
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
        Ok(items)
    }

    pub(super) fn parse_ui_node(&mut self) -> Result<UiNode, ParseError> {
        self.skip_newlines();
        let start = self.pos;
        let node = match self.peek_kind() {
            Some(TokenKind::Stack) => self.parse_stack_node(),
            Some(TokenKind::Parallel) => self.parse_parallel_node(),
            Some(TokenKind::String(_)) => self.parse_ui_leaf(),
            Some(TokenKind::Error) => self.parse_error_node(),
            _ => {
                let (found, line, col) = match self.peek() {
                    Some(t) => (t.kind.to_string(), t.line, t.col),
                    None => ("EOF".into(), 0, 0),
                };
                Err(ParseError::unexpected_token(
                    found,
                    "`stack`, `parallel`, `error` or string literal".into(),
                    line,
                    col,
                ))
            }
        }?;
        self.record_source_node(start..self.pos, RecordedNodeKind::UiNode);
        Ok(node)
    }

    fn parse_stack_node(&mut self) -> Result<UiNode, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Stack, "`stack`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let items = self.parse_ui_item_block(scope_token, false)?;
        Ok(UiNode::Stack {
            stack: StackNode {
                description,
                items,
                keyword_commitment,
            },
        })
    }

    fn parse_parallel_node(&mut self) -> Result<UiNode, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Parallel, "`parallel`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let items = self.parse_ui_item_block(scope_token, false)?;
        Ok(UiNode::Parallel {
            parallel: StackNode {
                description,
                items,
                keyword_commitment,
            },
        })
    }

    fn parse_error_node(&mut self) -> Result<UiNode, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Error, "`error`")?;
        let message = self.fuzzy_string()?;
        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };
        Ok(UiNode::Error {
            error: UiErrorNode {
                message,
                desc,
                keyword_commitment,
            },
        })
    }

    fn parse_ui_leaf(&mut self) -> Result<UiNode, ParseError> {
        let content = self.fuzzy_string()?;
        let mut desc = None;
        let mut requires = None;
        let mut requires_kw_commitment = Commitment::None;
        let mut with = Vec::new();
        let mut with_kw_commitment = Commitment::None;
        let mut on_binding = None;

        loop {
            if self.line_will_end() {
                break;
            }
            if self.matches(&TokenKind::Desc) {
                desc = Some(self.parse_desc_after_keyword()?);
            } else if self.check(&TokenKind::Requires) {
                requires_kw_commitment = self.expect_kw(TokenKind::Requires, "`requires`")?;
                requires = Some(self.parse_condition()?);
            } else if self.check(&TokenKind::With) {
                with_kw_commitment = self.expect_kw(TokenKind::With, "`with`")?;
                with = self.parse_capabilities()?;
            } else if self.check(&TokenKind::On) {
                on_binding = Some(self.parse_on_binding()?);
            } else {
                break;
            }
        }

        Ok(UiNode::Leaf {
            leaf: UiLeaf {
                content,
                desc,
                requires,
                requires_keyword_commitment: requires_kw_commitment,
                with,
                with_keyword_commitment: with_kw_commitment,
                on: on_binding,
            },
        })
    }

    fn parse_on_binding(&mut self) -> Result<OnBinding, ParseError> {
        self.expect_kw(TokenKind::On, "`on`")?;
        let event_name = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            EventName::Natural {
                text: self.fuzzy_string()?,
            }
        } else {
            EventName::Ident {
                value: self.fuzzy_ident()?,
            }
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let action = self.parse_action_expr()?;
        Ok(OnBinding { event_name, action })
    }

    fn parse_action_expr(&mut self) -> Result<ActionExpr, ParseError> {
        let mut actions = Vec::new();
        actions.push(self.parse_action()?);
        while self.matches(&TokenKind::Comma) {
            actions.push(self.parse_action()?);
        }
        Ok(ActionExpr { actions })
    }

    fn parse_action(&mut self) -> Result<Action, ParseError> {
        if self.check(&TokenKind::Arrow) {
            self.advance();
            let target = self.fuzzy_ident()?;
            return Ok(Action::Navigate { target });
        }

        if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            let text = self.fuzzy_string()?;
            return Ok(Action::Natural { text });
        }

        let expr = self.parse_expr(0)?;

        if self.matches(&TokenKind::Assign) {
            let value = self.parse_expr(0)?;
            return Ok(Action::Assign {
                target: expr,
                value,
            });
        }

        Ok(Action::Call { expr })
    }
}
