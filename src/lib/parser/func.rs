use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_func(&mut self) -> Result<FuncDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Func, "`func`")?;
        let name = self.fuzzy_ident()?;
        let params = if self.check(&TokenKind::LParen) {
            self.advance();
            let p = self.parse_params()?;
            self.expect(TokenKind::RParen, "`)`")?;
            p
        } else {
            Vec::new()
        };

        let mut capabilities = Vec::new();
        let mut with_keyword_commitment = Commitment::None;
        if self.check(&TokenKind::With) {
            with_keyword_commitment = self.expect_kw(TokenKind::With, "`with`")?;
            capabilities = self.parse_capabilities()?;
        }

        self.expect(TokenKind::Colon, "`:`")?;

        self.skip_newlines();
        if self.check(&TokenKind::Ellipsis) {
            self.advance();
            return Ok(FuncDef {
                name,
                desc: None,
                rules: Vec::new(),
                params,
                capabilities,
                requires: None,
                ensures: None,
                math: None,
                steps: vec![],
                keyword_commitment,
                requires_keyword_commitment: Commitment::None,
                ensures_keyword_commitment: Commitment::None,
                with_keyword_commitment,
                steps_keyword_commitment: Commitment::None,
            });
        }
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        let mut desc = None;
        let mut rules = Vec::new();
        let mut requires = None;
        let mut requires_keyword_commitment = Commitment::None;
        let mut ensures = None;
        let mut ensures_keyword_commitment = Commitment::None;
        let mut math = None;
        let mut steps = Vec::new();
        let mut steps_keyword_commitment = Commitment::None;

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            if self.check(&TokenKind::Ellipsis) {
                self.advance();
                continue;
            }
            if self.check(&TokenKind::Rule) {
                let rule_errors = self.consume_pending_rules();
                for e in rule_errors {
                    self.emit_error(e);
                }
                let collected = std::mem::take(&mut self.pending_rules);
                rules.extend(collected);
                let newline_count = self.skip_newlines_and_count();
                if newline_count < 3 {
                    continue;
                }
            }
            self.skip_newlines();
            if self.check(&TokenKind::Desc) {
                let d = self.parse_desc_entity()?;
                if desc.is_none() {
                    desc = Some(d);
                } else {
                    steps.push(Step::Desc { content: d });
                }
            } else if self.check(&TokenKind::Requires) {
                requires_keyword_commitment = self.expect_kw(TokenKind::Requires, "`requires`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                requires = Some(self.parse_condition()?);
                if !self.line_will_end() {
                    let (found, line, col) = match self.peek() {
                        Some(t) => (t.kind.to_string(), t.line, t.col),
                        None => ("EOF".into(), 0, 0),
                    };
                    return Err(ParseError::unexpected_token(
                        found,
                        "newline after requires condition".into(),
                        line,
                        col,
                    ));
                }
            } else if self.check(&TokenKind::Ensures) {
                ensures_keyword_commitment = self.expect_kw(TokenKind::Ensures, "`ensures`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                ensures = Some(self.parse_condition()?);
                if !self.line_will_end() {
                    let (found, line, col) = match self.peek() {
                        Some(t) => (t.kind.to_string(), t.line, t.col),
                        None => ("EOF".into(), 0, 0),
                    };
                    return Err(ParseError::unexpected_token(
                        found,
                        "newline after ensures condition".into(),
                        line,
                        col,
                    ));
                }
            } else if self.check(&TokenKind::Math) {
                let keyword_commitment = self.expect_kw(TokenKind::Math, "`math`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                math = Some(MathBlock {
                    statements: self.parse_block(|p| p.parse_math_statement())?,
                    keyword_commitment,
                });
            } else if self.check(&TokenKind::Steps) {
                steps_keyword_commitment = self.expect_kw(TokenKind::Steps, "`steps`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                steps = self.parse_block(|p| p.parse_step())?;
            } else {
                let save = self.pos;
                match self.parse_step() {
                    Ok(step) => steps.push(step),
                    Err(e) => {
                        self.pos = save;
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

        Ok(FuncDef {
            name,
            desc,
            rules,
            params,
            capabilities,
            requires,
            ensures,
            math,
            steps,
            keyword_commitment,
            requires_keyword_commitment,
            ensures_keyword_commitment,
            with_keyword_commitment,
            steps_keyword_commitment,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let name = self.fuzzy_ident()?;
            let mut type_hint = Vec::new();
            if self.matches(&TokenKind::Colon) {
                type_hint = self.parse_atoms_until(&[
                    TokenKind::Comma,
                    TokenKind::RParen,
                    TokenKind::Newline,
                    TokenKind::Eof,
                ])?;
            }
            params.push(Param { name, type_hint });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    pub(super) fn parse_capabilities(&mut self) -> Result<Vec<Capability>, ParseError> {
        let mut caps = Vec::new();
        loop {
            let name = self.fuzzy_ident()?;
            let commitment = self.commitment()?;
            caps.push(Capability { name, commitment });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(caps)
    }
}
