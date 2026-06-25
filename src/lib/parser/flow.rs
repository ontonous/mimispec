use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_flow(&mut self) -> Result<FlowDef, ParseError> {
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

        let entries = self.parse_block(|p| p.parse_flow_entry());

        Ok(FlowDef {
            name,
            rules: Vec::new(),
            entries,
            keyword_commitment,
        })
    }

    fn parse_flow_entry(&mut self) -> Result<FlowEntry, ParseError> {
        let state = self.fuzzy_ident()?;
        if self.check(&TokenKind::Arrow) {
            let to_keyword_commitment = self.expect_kw(TokenKind::Arrow, "`>>>`")?;
            let arm = self.parse_flow_arm_after_to_with_commitment(to_keyword_commitment)?;
            Ok(FlowEntry {
                state,
                rules: Vec::new(),
                arms: vec![arm],
            })
        } else {
            self.expect(TokenKind::Colon, "`:` or `to`")?;
            let arms = self.parse_block(|p| p.parse_flow_arm_in_block());
            Ok(FlowEntry {
                state,
                rules: Vec::new(),
                arms,
            })
        }
    }

    fn parse_flow_arm_in_block(&mut self) -> Result<FlowArm, ParseError> {
        let to_keyword_commitment = self.expect_kw(TokenKind::Arrow, "`>>>`")?;
        self.parse_flow_arm_after_to_with_commitment(to_keyword_commitment)
    }

    fn parse_flow_arm_after_to_with_commitment(
        &mut self,
        to_keyword_commitment: Commitment,
    ) -> Result<FlowArm, ParseError> {
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
