use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind, RecordedRuleDecision};

impl Parser {
    pub(super) fn parse_flow(&mut self) -> Result<FlowDef, ParseError> {
        let scope_token = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Flow, "`flow`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        if self.check(&TokenKind::Ellipsis) {
            self.advance();
            return Ok(FlowDef {
                name,
                rules: Vec::new(),
                entries: vec![],
                keyword_commitment,
            });
        }

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        let mut rules = Vec::new();
        let mut entries = Vec::new();
        let mut pending_entry_rules = Vec::new();
        let mut pending_entry_ids = Vec::new();

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                self.mark_rule_ids(
                    &pending_entry_ids,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                rules.append(&mut pending_entry_rules);
                pending_entry_ids.clear();
                break;
            }

            if self.check(&TokenKind::Rule) {
                let rule_errors = self.consume_pending_rules();
                for error in rule_errors {
                    self.emit_error(error);
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
                    pending_entry_rules.append(&mut collected);
                    pending_entry_ids.append(&mut collected_ids);
                    continue;
                }
            }

            let target_token = self.pos;
            match self.parse_flow_entry() {
                Ok(mut entry) => {
                    entry.rules = std::mem::take(&mut pending_entry_rules);
                    self.mark_rule_ids(
                        &pending_entry_ids,
                        RecordedRuleDecision::Attached { target_token },
                    );
                    pending_entry_ids.clear();
                    entries.push(entry);
                }
                Err(error) => {
                    self.mark_rule_ids(&pending_entry_ids, RecordedRuleDecision::DroppedByRecovery);
                    pending_entry_rules.clear();
                    pending_entry_ids.clear();
                    self.emit_error(error);
                    self.synchronize_to_next_item_in_block();
                }
            }
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        self.leave_block();

        Ok(FlowDef {
            name,
            rules,
            entries,
            keyword_commitment,
        })
    }

    fn parse_flow_entry(&mut self) -> Result<FlowEntry, ParseError> {
        let start = self.pos;
        let state = self.fuzzy_ident()?;
        let entry = if self.check(&TokenKind::Arrow) {
            let to_keyword_commitment = self.expect_kw(TokenKind::Arrow, "`>>>`")?;
            let arm = self.parse_flow_arm_after_to_with_commitment(to_keyword_commitment)?;
            FlowEntry {
                state,
                rules: Vec::new(),
                arms: vec![arm],
            }
        } else {
            self.expect(TokenKind::Colon, "`:`")?;
            let arms = self.parse_flow_arms_block()?;
            FlowEntry {
                state,
                rules: Vec::new(),
                arms,
            }
        };
        self.record_source_node(start..self.pos, RecordedNodeKind::FlowEntry);
        Ok(entry)
    }

    fn parse_flow_arms_block(&mut self) -> Result<Vec<FlowArm>, ParseError> {
        let scope_token = self.pos.saturating_sub(1);
        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        let mut arms = Vec::new();
        let mut pending_arm_rules = Vec::new();
        let mut pending_arm_ids = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                self.mark_rule_ids(
                    &pending_arm_ids,
                    RecordedRuleDecision::Environment {
                        scope_token: Some(scope_token),
                    },
                );
                break;
            }

            if self.check(&TokenKind::Rule) {
                let rule_errors = self.consume_pending_rules();
                for error in rule_errors {
                    self.emit_error(error);
                }
                let (mut rules, mut ids) = self.take_pending_rules_with_ids();
                pending_arm_rules.append(&mut rules);
                pending_arm_ids.append(&mut ids);
                self.skip_newlines();
                continue;
            }

            let target_token = self.pos;
            match self.parse_flow_arm_in_block() {
                Ok(mut arm) => {
                    arm.rules = std::mem::take(&mut pending_arm_rules);
                    self.mark_rule_ids(
                        &pending_arm_ids,
                        RecordedRuleDecision::Attached { target_token },
                    );
                    pending_arm_ids.clear();
                    arms.push(arm);
                }
                Err(error) => {
                    self.mark_rule_ids(&pending_arm_ids, RecordedRuleDecision::DroppedByRecovery);
                    pending_arm_rules.clear();
                    pending_arm_ids.clear();
                    self.emit_error(error);
                    self.synchronize_to_next_item_in_block();
                }
            }
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        self.leave_block();
        Ok(arms)
    }

    fn parse_flow_arm_in_block(&mut self) -> Result<FlowArm, ParseError> {
        let to_keyword_commitment = self.expect_kw(TokenKind::Arrow, "`>>>`")?;
        self.parse_flow_arm_after_to_with_commitment(to_keyword_commitment)
    }

    fn parse_flow_arm_after_to_with_commitment(
        &mut self,
        to_keyword_commitment: Commitment,
    ) -> Result<FlowArm, ParseError> {
        // Anchor at the already-consumed `>>>` so rule targets and node headers share it.
        let start = self.pos.saturating_sub(1);
        let to = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;
        let mut requires = None;
        let mut requires_keyword_commitment = Commitment::None;
        let mut desc = None;
        loop {
            if self.line_will_end() {
                break;
            }
            if self.check(&TokenKind::Requires) {
                requires_keyword_commitment = self.expect_kw(TokenKind::Requires, "`requires`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                requires = Some(self.parse_condition()?);
            } else if self.matches(&TokenKind::Desc) {
                desc = Some(self.parse_desc_after_keyword()?);
            } else {
                let (found, line, col) = match self.peek() {
                    Some(t) => (t.kind.to_string(), t.line, t.col),
                    None => ("EOF".into(), 0, 0),
                };
                return Err(ParseError::unexpected_token(
                    found,
                    "`requires` or `desc`".into(),
                    line,
                    col,
                ));
            }
        }
        self.record_source_node(start..self.pos, RecordedNodeKind::FlowArm);
        Ok(FlowArm {
            to,
            requires,
            desc,
            rules: Vec::new(),
            to_keyword_commitment,
            requires_keyword_commitment,
        })
    }
}
