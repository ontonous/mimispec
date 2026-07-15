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

        let mut desc = None;
        let mut rules = Vec::new();
        let mut math = None;
        let mut items = Vec::new();
        let mut pending_item_rules: Vec<RuleDef> = Vec::new();
        let mut pending_item_ids = Vec::new();

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                self.mark_rule_ids(
                    &pending_item_ids,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                rules.append(&mut pending_item_rules);
                pending_item_ids.clear();
                break;
            }

            if self.check(&TokenKind::Rule) {
                let rule_errors = self.consume_pending_rules();
                for e in rule_errors {
                    self.emit_error(e);
                }
                let (mut collected, mut collected_ids) = self.take_pending_rules_with_ids();
                let newline_count = self.skip_newlines_and_count();
                if newline_count >= 3 {
                    self.mark_rule_ids(
                        &collected_ids,
                        RecordedRuleDecision::Environment {
                            scope_token: Some(scope_token),
                        },
                    );
                    rules.append(&mut collected);
                } else {
                    pending_item_rules = collected;
                    pending_item_ids.append(&mut collected_ids);
                    continue;
                }
            }

            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                self.mark_rule_ids(
                    &pending_item_ids,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                rules.append(&mut pending_item_rules);
                pending_item_ids.clear();
                break;
            }

            if self.check(&TokenKind::Desc) {
                let d = self.parse_desc_entity()?;
                self.mark_rule_ids(
                    &pending_item_ids,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                rules.append(&mut pending_item_rules);
                pending_item_ids.clear();
                if desc.is_none() {
                    desc = Some(d);
                } else {
                    items.push(Fragment::Steps {
                        keyword_commitment: Commitment::None,
                        steps: vec![Step::Desc { content: d }],
                    });
                }
            } else if self.check(&TokenKind::Math) {
                self.mark_rule_ids(
                    &pending_item_ids,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                rules.append(&mut pending_item_rules);
                pending_item_ids.clear();
                math = Some(self.parse_math_block()?);
            } else {
                match self.parse_fragment() {
                    Ok(mut fragment) => {
                        if !pending_item_rules.is_empty() {
                            let target_token = self
                                .recorded_nodes
                                .last()
                                .map_or(self.pos, |node| node.tokens.start);
                            let attached = Self::fragment_accepts_rules(&fragment);
                            let unattached = Self::attach_rules_to_fragment_from(
                                std::mem::take(&mut pending_item_rules),
                                &mut fragment,
                            );
                            self.mark_rule_ids(
                                &pending_item_ids,
                                if attached {
                                    RecordedRuleDecision::Attached { target_token }
                                } else {
                                    RecordedRuleDecision::Environment {
                                        scope_token: Some(scope_token),
                                    }
                                },
                            );
                            pending_item_ids.clear();
                            rules.extend(unattached);
                        }
                        items.push(fragment);
                    }
                    Err(e) => {
                        self.mark_rule_ids(
                            &pending_item_ids,
                            RecordedRuleDecision::DroppedByRecovery,
                        );
                        pending_item_rules.clear();
                        pending_item_ids.clear();
                        self.emit_error(e);
                        self.try_sync(|p| p.synchronize_past_nested_block());
                        if self.check(&TokenKind::Dedent) || self.is_at_end() {
                            break;
                        }
                    }
                }
            }
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        self.leave_block();

        Ok(Module {
            name,
            desc,
            rules,
            math,
            items,
            keyword_commitment,
        })
    }
}
