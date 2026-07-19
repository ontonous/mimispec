use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{Parser, RecordedNodeKind};

impl Parser {
    pub(super) fn parse_rule_def(&mut self) -> Result<(RuleDef, Option<usize>), ParseError> {
        let start = self.pos;
        let keyword_commitment = self.expect_kw(TokenKind::Rule, "`rule`")?;
        let content = self.fuzzy_string()?;
        let desc = Desc {
            need_commitment: Commitment::None,
            content,
        };
        let rule = RuleDef {
            desc,
            keyword_commitment,
            attachment: RuleAttachment::Pending,
        };
        let id = self.record_rule_occurrence(start..self.pos);
        self.record_source_node(start..self.pos, RecordedNodeKind::Rule);
        Ok((rule, id))
    }
}
