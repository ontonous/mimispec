use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

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

        // Multi-line enum (方案A): check if indented block contains enum variants
        self.skip_newlines();
        if self.check(&TokenKind::Indent) {
            let save = self.pos;
            self.advance(); // consume Indent for scanning
            let is_block_enum = self.peek_block_is_enum();
            self.pos = save; // restore back to Indent

            if is_block_enum {
                self.advance(); // consume Indent
                let variants = self.parse_block_enum_variants()?;
                return Ok(TypeDef {
                    name,
                    desc: None,
                    rules: Vec::new(),
                    math: None,
                    body: TypeBody::Enum { variants },
                    keyword_commitment,
                });
            }
            // Fall through to record parsing, but keep Indent unconsumed
        }

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.enter_block()?;

        let mut desc = None;
        let mut math = None;
        let mut rules: Vec<RuleDef> = Vec::new();
        let mut fields = Vec::new();
        let mut pending_field_rules: Vec<RuleDef> = Vec::new();

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }

            if self.check(&TokenKind::Rule) {
                let mut had_error = false;
                while self.check(&TokenKind::Rule) && !had_error {
                    match self.parse_rule_def() {
                        Ok(rule) => {
                            let newline_count = self.skip_newlines_and_count();
                            if newline_count >= 3 && fields.is_empty() {
                                rules.push(rule);
                            } else {
                                pending_field_rules.push(rule);
                            }
                        }
                        Err(e) => {
                            self.emit_error(e);
                            had_error = true;
                        }
                    }
                }
                if had_error {
                    self.synchronize_to_next_item_in_block();
                    if self.check(&TokenKind::Dedent) || self.is_at_end() {
                        break;
                    }
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
        self.leave_block();

        Ok(TypeDef {
            name,
            desc,
            rules,
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
                    depth = depth.saturating_sub(1);
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

    /// Pre-scan the indented block (past Indent) to determine if it's a multi-line enum
    /// or a record. Returns true if the block content looks like enum variants.
    fn peek_block_is_enum(&mut self) -> bool {
        let mut scan = self.pos;
        // Skip newlines
        while let Some(TokenKind::Newline) = self.tokens.get(scan).map(|t| &t.kind) {
            scan += 1;
        }

        match self.tokens.get(scan).map(|t| &t.kind) {
            // Leading `|` on a line → enum block
            Some(TokenKind::Pipe) => true,
            // desc / rule / math / ... → record block
            Some(TokenKind::Desc)
            | Some(TokenKind::Rule)
            | Some(TokenKind::Math)
            | Some(TokenKind::Ellipsis) => false,
            // Identifier or keyword variant name → check what follows
            Some(_kind) => {
                scan += 1;
                // Skip commitment
                while let Some(tok) = self.tokens.get(scan) {
                    match &tok.kind {
                        TokenKind::Question
                        | TokenKind::QuestionQuestion
                        | TokenKind::Dollar
                        | TokenKind::DollarDollar => {
                            scan += 1;
                        }
                        _ => break,
                    }
                }
                // After the ident + commitment: `:` means record field
                match self.tokens.get(scan).map(|t| &t.kind) {
                    Some(TokenKind::Colon) => false,
                    // bare identifier or `|` continuation → enum variant
                    _ => true,
                }
            }
            _ => false,
        }
    }

    /// Parse variants inside a multi-line enum block.
    /// Each line may start with optional `|`, and inline `A | B` per line is allowed.
    fn parse_block_enum_variants(&mut self) -> Result<Vec<Ident>, ParseError> {
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            // Optional leading `|`
            if self.check(&TokenKind::Pipe) {
                self.advance();
            }
            variants.push(self.fuzzy_ident()?);
            // Allow inline `|` on the same line for compactness
            while self.matches(&TokenKind::Pipe) {
                variants.push(self.fuzzy_ident()?);
            }
        }
        if self.check(&TokenKind::Dedent) {
            self.advance();
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
