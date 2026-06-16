use crate::ast::*;
use crate::error::ParseError;
use crate::parser::Parser;
use crate::lexer::TokenKind;

impl Parser {
    pub(super) fn parse_type_def(&mut self) -> Result<TypeDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Type, "`type`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        if self.check(&TokenKind::Ellipsis) {
            self.advance();
            return Ok(TypeDef {
                name,
                desc: None,
                rules: Vec::new(),
                math: None,
                body: TypeBody::Record { fields: vec![] },
                keyword_commitment,
            });
        }

        if self.is_inline_enum() {
            return Ok(TypeDef {
                name,
                desc: None,
                rules: Vec::new(),
                math: None,
                body: TypeBody::Enum {
                    variants: self.parse_variant_list()?,
                },
                keyword_commitment,
            });
        }

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;

        let mut desc = None;
        let mut math = None;
        let mut fields = Vec::new();
        let mut pending_field_rules: Vec<RuleDef> = Vec::new();

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }

            if self.check(&TokenKind::Rule) {
                while self.check(&TokenKind::Rule) {
                    pending_field_rules.push(self.parse_rule_def()?);
                }
                continue;
            }

            if self.check(&TokenKind::Desc) {
                let d = self.parse_desc_entity()?;
                if desc.is_none() {
                    desc = Some(d);
                }
            } else if self.check(&TokenKind::Math) {
                math = Some(self.parse_math_block()?);
            } else if self.check(&TokenKind::Ellipsis) {
                self.advance();
            } else {
                match self.parse_field() {
                    Ok(mut field) => {
                        field.rules = std::mem::take(&mut pending_field_rules);
                        fields.push(field);
                    }
                    Err(e) => {
                        self.emit_error(e);
                        self.synchronize_to_next_item_in_block();
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

        Ok(TypeDef {
            name,
            desc,
            rules: Vec::new(),
            math,
            body: TypeBody::Record { fields },
            keyword_commitment,
        })
    }

    fn is_inline_enum(&mut self) -> bool {
        let save = self.pos;
        let mut depth = 0usize;
        let mut found_pipe = false;
        while let Some(tok) = self.tokens.get(self.pos) {
            match &tok.kind {
                TokenKind::Newline | TokenKind::Eof => break,
                TokenKind::Pipe if depth == 0 => found_pipe = true,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::Lt => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::Gt => {
                    if depth > 0 {
                        depth -= 1;
                    }
                }
                _ => {}
            }
            self.pos += 1;
        }
        self.pos = save;
        found_pipe
    }

    fn parse_variant_list(&mut self) -> Result<Vec<Ident>, ParseError> {
        let mut variants = Vec::new();
        variants.push(self.fuzzy_ident()?);
        while self.matches(&TokenKind::Pipe) {
            variants.push(self.fuzzy_ident()?);
        }
        Ok(variants)
    }

    fn parse_field(&mut self) -> Result<Field, ParseError> {
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;
        let type_hint =
            self.parse_atoms_until(&[TokenKind::Newline, TokenKind::Dedent, TokenKind::Eof])?;
        Ok(Field {
            name,
            rules: Vec::new(),
            type_hint,
        })
    }
}
