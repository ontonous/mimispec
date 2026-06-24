use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_module(&mut self) -> Result<Module, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Module, "`module`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        let mut desc = None;
        let mut rules = Vec::new();
        let mut math = None;
        let mut items = Vec::new();
        let mut pending_item_rules: Vec<RuleDef> = Vec::new();

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                rules.append(&mut pending_item_rules);
                break;
            }

            if self.check(&TokenKind::Rule) {
                let rule_errors = self.consume_pending_rules();
                for e in rule_errors {
                    self.emit_error(e);
                }
                let mut collected = std::mem::take(&mut self.pending_rules);
                let newline_count = self.skip_newlines_and_count();
                if newline_count >= 3 {
                    rules.append(&mut collected);
                } else {
                    pending_item_rules = collected;
                    continue;
                }
            }

            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                rules.append(&mut pending_item_rules);
                break;
            }

            if self.check(&TokenKind::Desc) {
                let d = self.parse_desc_entity()?;
                rules.append(&mut pending_item_rules);
                if desc.is_none() {
                    desc = Some(d);
                } else {
                    items.push(Fragment::Steps {
                        keyword_commitment: Commitment::None,
                        steps: vec![Step::Desc { content: d }],
                    });
                }
            } else if self.check(&TokenKind::Math) {
                rules.append(&mut pending_item_rules);
                math = Some(self.parse_math_block()?);
            } else {
                match self.parse_fragment() {
                    Ok(mut fragment) => {
                        if !pending_item_rules.is_empty() {
                            Self::attach_rules_to_fragment_from(
                                std::mem::take(&mut pending_item_rules),
                                &mut fragment,
                            );
                        }
                        items.push(fragment);
                    }
                    Err(e) => {
                        pending_item_rules.clear();
                        self.emit_error(e);
                        let before = self.pos;
                        self.synchronize_past_nested_block();
                        if self.pos == before && !self.is_at_end() {
                            self.advance();
                        }
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
