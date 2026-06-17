use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_rule_def(&mut self) -> Result<RuleDef, ParseError> {
        let line = self.peek().map(|t| t.line).unwrap_or(0);
        let keyword_commitment = self.expect_kw(TokenKind::Rule, "`rule`")?;
        let content = self.fuzzy_string()?;
        let desc = Desc {
            need_commitment: Commitment::None,
            content,
        };
        Ok(RuleDef {
            desc,
            keyword_commitment,
            line,
        })
    }
}
