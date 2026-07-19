use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedRuleDecision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedTypeShape {
    Enum,
    Record,
}

impl Parser {
    pub(super) fn parse_type_def(&mut self) -> Result<TypeDef, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Type, "`type`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        if self.check(&TokenKind::Ellipsis) {
            let placeholder = self.parse_placeholder_item()?;
            return Ok(TypeDef {
                name,
                body: TypeBody::Record {
                    items: vec![placeholder],
                },
                keyword_commitment,
            });
        }

        if self.is_inline_enum() {
            let variants = self.parse_variant_line()?;
            return Ok(TypeDef {
                name,
                body: TypeBody::Enum {
                    inline: true,
                    items: vec![Fragment::Variants { variants }],
                },
                keyword_commitment,
            });
        }

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        let mut items = Vec::new();
        let mut pending_rules: Vec<(usize, Option<usize>)> = Vec::new();
        let mut shape = None;

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
                self.parse_desc_entity()
                    .map(|desc| (Fragment::Desc { desc }, None))
            } else if self.check(&TokenKind::Math) {
                self.parse_math_block()
                    .map(|math| (Fragment::Math { math }, None))
            } else if self.check(&TokenKind::Ellipsis) {
                self.parse_placeholder_item()
                    .map(|placeholder| (placeholder, None))
            } else if self.type_line_is_field() {
                self.parse_field()
                    .map(|field| (Fragment::Field { field }, Some(ParsedTypeShape::Record)))
            } else {
                self.parse_variant_line()
                    .map(|variants| (Fragment::Variants { variants }, Some(ParsedTypeShape::Enum)))
            };

            match parsed {
                Ok((item, item_shape)) => {
                    if let Some(item_shape) = item_shape {
                        if let Some(existing) = shape {
                            if existing != item_shape {
                                let (line, col) = self.current_pos();
                                self.emit_error(ParseError::unexpected_token(
                                    "mixed enum variant and record field".into(),
                                    "a type body containing only variants or only fields".into(),
                                    line,
                                    col,
                                ));
                            }
                        } else {
                            shape = Some(item_shape);
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

        let body = match shape.unwrap_or(ParsedTypeShape::Record) {
            ParsedTypeShape::Enum => TypeBody::Enum {
                inline: false,
                items,
            },
            ParsedTypeShape::Record => TypeBody::Record { items },
        };
        Ok(TypeDef {
            name,
            body,
            keyword_commitment,
        })
    }

    fn is_inline_enum(&mut self) -> bool {
        let save = self.pos;
        let mut depth = 0usize;
        let mut found_pipe = false;
        while let Some(token) = self.tokens.get(self.pos) {
            match &token.kind {
                TokenKind::Newline | TokenKind::Eof => break,
                TokenKind::Pipe if depth == 0 => found_pipe = true,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::Lt => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::Gt => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            self.pos += 1;
        }
        self.pos = save;
        found_pipe
    }

    fn type_line_is_field(&self) -> bool {
        let mut index = self.pos;
        if matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Pipe)
        ) {
            return false;
        }
        if self.tokens.get(index).is_none() {
            return false;
        }
        index += 1;
        while matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(
                TokenKind::Question
                    | TokenKind::QuestionQuestion
                    | TokenKind::Dollar
                    | TokenKind::DollarDollar
            )
        ) {
            index += 1;
        }
        matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Colon)
        )
    }

    fn parse_variant_line(&mut self) -> Result<Vec<Ident>, ParseError> {
        self.matches(&TokenKind::Pipe);
        let mut variants = vec![self.fuzzy_ident()?];
        while self.matches(&TokenKind::Pipe) {
            variants.push(self.fuzzy_ident()?);
        }
        if !self.line_will_end() {
            let (found, line, col) = match self.peek() {
                Some(token) => (token.kind.to_string(), token.line, token.col),
                None => ("EOF".into(), 0, 0),
            };
            return Err(ParseError::unexpected_token(
                found,
                "`|` or end of enum variant line".into(),
                line,
                col,
            ));
        }
        Ok(variants)
    }

    fn parse_field(&mut self) -> Result<Field, ParseError> {
        let start = self.pos;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;
        let type_hint =
            self.parse_atoms_until(&[TokenKind::Newline, TokenKind::Dedent, TokenKind::Eof])?;
        self.record_source_node(start..self.pos, crate::parser::RecordedNodeKind::Field);
        Ok(Field { name, type_hint })
    }
}
