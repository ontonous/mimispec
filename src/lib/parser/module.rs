use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedRuleDecision};

impl Parser {
    pub(super) fn parse_module(&mut self) -> Result<Module, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Module, "`module`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

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
            match self.parse_fragment() {
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
                    self.try_sync(|parser| parser.synchronize_past_nested_block());
                }
            }
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        self.leave_block();

        Ok(Module {
            name,
            items,
            keyword_commitment,
        })
    }
}
