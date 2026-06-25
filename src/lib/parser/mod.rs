use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::lexer::{Token, TokenKind};

pub mod expr;
pub mod flow;
pub mod fragment;
pub mod func;
pub mod module;
pub mod rule;
pub mod step;
pub mod r#type;
pub mod ui;

pub struct Parser {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) pending_rules: Vec<RuleDef>,
    pub(super) errors: Vec<ParseError>,
    block_depth: u32,
}

const MAX_BLOCK_DEPTH: u32 = 256;

#[derive(Clone, Copy)]
pub(super) enum BinOp {
    Or,
    And,
    BitOr,
    BitXor,
    BitAnd,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    MatMul,
    Pow,
    In,
    Cmp(CompareOp),
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            pending_rules: Vec::new(),
            errors: Vec::new(),
            block_depth: 0,
        }
    }

    pub(super) fn enter_block(&mut self) -> Result<(), ParseError> {
        if self.block_depth > MAX_BLOCK_DEPTH {
            let (line, col) = self.current_pos();
            return Err(ParseError::internal(
                "block nesting too deep".into(),
                line,
                col,
            ));
        }
        self.block_depth += 1;
        Ok(())
    }

    pub(super) fn leave_block(&mut self) {
        self.block_depth = self.block_depth.saturating_sub(1);
    }

    pub(super) fn emit_error(&mut self, err: ParseError) {
        self.errors.push(err);
    }

    pub(crate) fn take_errors(&mut self) -> Vec<ParseError> {
        std::mem::take(&mut self.errors)
    }

    pub fn parse_file(&mut self) -> ParseResult {
        let mut errors = Vec::new();
        self.skip_newlines();

        let mut imports = Vec::new();
        while self.check(&TokenKind::Import) {
            self.advance();
            match self.expect_string() {
                Ok(tok) => {
                    let path = match &tok.kind {
                        TokenKind::String(s) => s.clone(),
                        _ => {
                            errors.push(ParseError::internal(
                                "expected string after @import".into(),
                                tok.line,
                                tok.col,
                            ));
                            self.synchronize_past_import();
                            continue;
                        }
                    };
                    imports.push(path);
                }
                Err(e) => {
                    errors.push(e);
                    self.synchronize_past_import();
                }
            }
            self.skip_newlines();
        }

        let mut fragments = Vec::new();
        let mut global_rules = Vec::new();

        while !self.is_at_end() {
            while self.check(&TokenKind::Dedent) {
                self.advance();
            }
            if self.is_at_end() {
                break;
            }

            self.skip_newlines();
            let mut rule_errors = self.consume_pending_rules();
            errors.extend(rule_errors);

            let mut newline_count = self.skip_newlines_and_count();
            if newline_count >= 3 {
                global_rules.extend(self.take_pending_rules());
            }

            while self.check(&TokenKind::Rule) {
                rule_errors = self.consume_pending_rules();
                errors.extend(rule_errors);
                newline_count = self.skip_newlines_and_count();
                if newline_count >= 3 {
                    global_rules.extend(self.take_pending_rules());
                }
            }

            if self.is_at_end() {
                break;
            }

            match self.parse_fragment() {
                Ok(mut f) => {
                    self.attach_rules_to_fragment(&mut f);
                    fragments.push(f);
                }
                Err(e) => {
                    errors.push(e);
                    self.pending_rules.clear();
                    self.synchronize_to_fragment_start();
                }
            }
        }

        global_rules.extend(self.take_pending_rules());

        let mut all_errors = self.take_errors();
        all_errors.extend(errors);

        ParseResult {
            file: File {
                imports,
                rules: global_rules,
                fragments,
            },
            errors: all_errors,
        }
    }

    // ── token navigation ─────────────────────────────────────────────────────

    pub(super) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    pub(super) fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    pub(super) fn advance(&mut self) -> &Token {
        if let Some(tok) = self.tokens.get(self.pos) {
            if !matches!(tok.kind, TokenKind::Eof) {
                self.pos += 1;
            }
            tok
        } else {
            self.tokens.last().unwrap_or_else(|| {
                panic!("Parser: advance() called on empty token list — this is a bug")
            })
        }
    }

    pub(super) fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Eof) | None)
    }

    pub(super) fn check(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    pub(super) fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(super) fn expect(&mut self, kind: TokenKind, what: &str) -> Result<&Token, ParseError> {
        if let Some(tok) = self.peek() {
            if tok.kind.same_kind(&kind) {
                return Ok(self.advance());
            }
        }
        let (found, line, col) = match self.peek() {
            Some(t) => (t.kind.to_string(), t.line, t.col),
            None => ("EOF".into(), 0, 0),
        };
        Err(ParseError::unexpected_token(found, what.into(), line, col))
    }

    // ── commitment / identifiers ─────────────────────────────────────────────

    pub(super) fn commitment(&mut self) -> Result<Commitment, ParseError> {
        if self.matches(&TokenKind::DollarDollar) {
            if self.matches(&TokenKind::QuestionQuestion) {
                return Ok(Commitment::StrongLockedQuestionQuestion);
            }
            if self.matches(&TokenKind::Question) {
                return Ok(Commitment::StrongLockedQuestion);
            }
            return Ok(Commitment::StrongLocked);
        }

        if self.matches(&TokenKind::Dollar) {
            if self.matches(&TokenKind::QuestionQuestion) {
                return Ok(Commitment::LockedQuestionQuestion);
            }
            if self.matches(&TokenKind::Question) {
                return Ok(Commitment::LockedQuestion);
            }
            return Ok(Commitment::Locked);
        }

        if self.matches(&TokenKind::QuestionQuestion) {
            if self.check(&TokenKind::Dollar) || self.check(&TokenKind::DollarDollar) {
                let t = self.peek().unwrap();
                return Err(ParseError::unexpected_token(
                    t.kind.to_string(),
                    "lock suffix must come before uncertainty suffix (`?$` / `?$$` ordering is invalid)".into(),
                    t.line,
                    t.col,
                ));
            }
            return Ok(Commitment::QuestionQuestion);
        }

        if self.matches(&TokenKind::Question) {
            if self.check(&TokenKind::Dollar) || self.check(&TokenKind::DollarDollar) {
                let t = self.peek().unwrap();
                return Err(ParseError::unexpected_token(
                    t.kind.to_string(),
                    "lock suffix must come before uncertainty suffix (`?$` / `?$$` ordering is invalid)".into(),
                    t.line,
                    t.col,
                ));
            }
            return Ok(Commitment::Question);
        }

        Ok(Commitment::None)
    }

    pub(super) fn expect_kw(
        &mut self,
        kind: TokenKind,
        what: &str,
    ) -> Result<Commitment, ParseError> {
        self.expect(kind, what)?;
        self.commitment()
    }

    pub(super) fn expect_string(&mut self) -> Result<&Token, ParseError> {
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::String(_)) {
                return Ok(self.advance());
            }
        }
        let (found, line, col) = match self.peek() {
            Some(t) => (t.kind.to_string(), t.line, t.col),
            None => ("EOF".into(), 0, 0),
        };
        Err(ParseError::unexpected_token(found, "string literal".into(), line, col))
    }

    pub(super) fn fuzzy_ident(&mut self) -> Result<Ident, ParseError> {
        let tok_opt = self.peek().cloned();
        let tok = tok_opt.ok_or_else(|| {
            let (l, c) = self.current_pos();
            ParseError::unexpected_eof(l, c)
        })?;
        let name = if let Some(kw) = tok.kind.as_keyword_str() {
            self.advance();
            kw.to_string()
        } else if let TokenKind::Ident(s) = &tok.kind {
            self.advance();
            s.clone()
        } else {
            return Err(ParseError::unexpected_token(
                tok.kind.to_string(),
                "identifier".into(),
                tok.line,
                tok.col,
            ));
        };
        let commitment = self.commitment()?;
        Ok(Ident { name, commitment })
    }

    /// Return the (line, col) of the current token, or (0, 0) at EOF.
    pub(super) fn current_pos(&self) -> (usize, usize) {
        match self.peek() {
            Some(t) => (t.line, t.col),
            None => (0, 0),
        }
    }

    pub(super) fn fuzzy_string(&mut self) -> Result<FString, ParseError> {
        let tok = self.expect_string()?;
        let TokenKind::String(value) = &tok.kind else {
            unreachable!("expect_string() guaranteed String token")
        };
        let value = value.clone();
        let commitment = self.commitment()?;
        Ok(FString { value, commitment })
    }

    // ── newline handling ─────────────────────────────────────────────────────

    pub(crate) fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    pub(super) fn skip_newlines_and_count(&mut self) -> usize {
        let mut count = 0;
        while self.check(&TokenKind::Newline) {
            self.advance();
            count += 1;
        }
        count
    }

    pub(super) fn peek_newline_count(&self) -> usize {
        let mut count = 0;
        let mut lookahead = self.pos;
        while matches!(
            self.tokens.get(lookahead).map(|t| &t.kind),
            Some(TokenKind::Newline)
        ) {
            lookahead += 1;
            count += 1;
        }
        count
    }

    pub(super) fn line_will_end(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof)
        )
    }

    // ── synchronization / error recovery ─────────────────────────────────────

    pub(super) fn synchronize_past_import(&mut self) {
        while !self.is_at_end() {
            match self.peek_kind() {
                Some(TokenKind::Newline)
                | Some(TokenKind::Import)
                | Some(TokenKind::Module)
                | Some(TokenKind::Type)
                | Some(TokenKind::Rule)
                | Some(TokenKind::Flow)
                | Some(TokenKind::Func)
                | Some(TokenKind::Ui)
                | Some(TokenKind::Steps)
                | Some(TokenKind::Ellipsis)
                | Some(TokenKind::Dedent) => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    pub(super) fn synchronize_to_fragment_start(&mut self) {
        while !self.is_at_end() {
            match self.peek_kind() {
                Some(TokenKind::Module)
                | Some(TokenKind::Type)
                | Some(TokenKind::Rule)
                | Some(TokenKind::Flow)
                | Some(TokenKind::Func)
                | Some(TokenKind::Ui)
                | Some(TokenKind::Steps)
                | Some(TokenKind::Import)
                | Some(TokenKind::Ellipsis)
                | Some(TokenKind::If)
                | Some(TokenKind::For)
                | Some(TokenKind::While)
                | Some(TokenKind::Parasteps)
                | Some(TokenKind::Error)
                | Some(TokenKind::Stack)
                | Some(TokenKind::Parallel)
                | Some(TokenKind::Dedent) => return,
                Some(TokenKind::String(_)) => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    pub(super) fn synchronize_past_nested_block(&mut self) {
        let mut depth: usize = 0;
        while !self.is_at_end() {
            let kind = self.peek_kind();
            match kind {
                Some(TokenKind::Indent) => {
                    depth += 1;
                    self.advance();
                }
                Some(TokenKind::Dedent) => {
                    self.advance();
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                }
                Some(TokenKind::Module)
                | Some(TokenKind::Type)
                | Some(TokenKind::Rule)
                | Some(TokenKind::Flow)
                | Some(TokenKind::Func)
                | Some(TokenKind::Ui)
                | Some(TokenKind::Steps)
                | Some(TokenKind::Import)
                | Some(TokenKind::Ellipsis)
                | Some(TokenKind::If)
                | Some(TokenKind::For)
                | Some(TokenKind::While)
                | Some(TokenKind::Parasteps)
                | Some(TokenKind::Error)
                | Some(TokenKind::Stack)
                | Some(TokenKind::Parallel)
                | Some(TokenKind::String(_)) => {
                    if depth == 0 {
                        return;
                    }
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    pub(super) fn synchronize_to_next_item_in_block(&mut self) {
        let mut depth: usize = 0;
        while !self.is_at_end() {
            let kind = self.peek_kind();
            match kind {
                Some(TokenKind::Indent) => {
                    depth += 1;
                    self.advance();
                }
                Some(TokenKind::Dedent) => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    self.advance();
                }
                Some(TokenKind::Newline) if depth == 0 => {
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Helper: run a sync function, and if the position didn't advance,
    /// force-advance one token to avoid infinite loops.
    pub(super) fn try_sync<F: FnOnce(&mut Self)>(&mut self, sync: F) {
        let before = self.pos;
        sync(self);
        if self.pos == before && !self.is_at_end() {
            self.advance();
        }
    }

    // ── block parsing ────────────────────────────────────────────────────────

    pub(super) fn parse_block<T>(
        &mut self,
        mut parse_item: impl FnMut(&mut Self) -> Result<T, ParseError>,
    ) -> Vec<T> {
        self.skip_newlines();
        if self.expect(TokenKind::Indent, "indented block").is_err() {
            return Vec::new();
        }
        if self.enter_block().is_err() {
            return Vec::new();
        }
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            match parse_item(self) {
                Ok(item) => items.push(item),
                Err(e) => {
                    self.emit_error(e);
                    self.synchronize_to_next_item_in_block();
                    if self.check(&TokenKind::Dedent) || self.is_at_end() {
                        break;
                    }
                }
            }
        }
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        self.leave_block();
        items
    }

    // ── rule management ──────────────────────────────────────────────────────

    pub(super) fn consume_pending_rules(&mut self) -> Vec<ParseError> {
        let mut errors = Vec::new();
        loop {
            let newline_count = self.peek_newline_count();

            if newline_count >= 3 {
                break;
            }

            // Advance past newlines
            for _ in 0..newline_count {
                self.advance();
            }

            if !self.check(&TokenKind::Rule) {
                break;
            }

            match self.parse_rule_def() {
                Ok(rule) => self.pending_rules.push(rule),
                Err(e) => {
                    errors.push(e);
                    self.synchronize_to_fragment_start();
                    break;
                }
            }
        }
        errors
    }

    pub(super) fn take_pending_rules(&mut self) -> Vec<RuleDef> {
        std::mem::take(&mut self.pending_rules)
    }

    pub(super) fn attach_rules_to_fragment(&mut self, fragment: &mut Fragment) {
        let rules = self.take_pending_rules();
        Self::attach_rules_to_fragment_from(rules, fragment);
    }

    pub(super) fn attach_rules_to_fragment_from(rules: Vec<RuleDef>, fragment: &mut Fragment) {
        if rules.is_empty() {
            return;
        }
        match fragment {
            Fragment::Module { module } => module.rules.extend(rules),
            Fragment::TypeDef { typedef } => typedef.rules.extend(rules),
            Fragment::Flow { flow } => flow.rules.extend(rules),
            Fragment::Func { func } => func.rules.extend(rules),
            Fragment::Ui { ui } => ui.rules.extend(rules),
            _ => { /* Steps/Expr/UiNode/Desc/Placeholder 不支持 rule 附着 */ }
        }
    }

    // ── desc after keyword ───────────────────────────────────────────────────

    pub(super) fn parse_desc_after_keyword(&mut self) -> Result<Desc, ParseError> {
        let need_commitment = self.commitment()?;
        let content = self.fuzzy_string()?;
        Ok(Desc {
            need_commitment,
            content,
        })
    }

    // ── atom sequence helpers ────────────────────────────────────────────────

    pub(super) fn parse_atoms_until(
        &mut self,
        stop: &[TokenKind],
    ) -> Result<Vec<Atom>, ParseError> {
        let mut atoms = Vec::new();
        let mut depth_paren = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;
        while let Some(tok) = self.peek() {
            if depth_paren == 0 && depth_bracket == 0 && depth_angle == 0 {
                if stop
                    .iter()
                    .any(|k| std::mem::discriminant(k) == std::mem::discriminant(&tok.kind))
                {
                    break;
                }
                if matches!(
                    tok.kind,
                    TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof
                ) {
                    break;
                }
                if matches!(tok.kind, TokenKind::LBracket) {
                    atoms.push(self.parse_atom_list_literal()?);
                    continue;
                }
            }
            match &tok.kind {
                TokenKind::LParen => depth_paren += 1,
                TokenKind::RParen => {
                    depth_paren = depth_paren.saturating_sub(1);
                }
                TokenKind::LBracket => depth_bracket += 1,
                TokenKind::RBracket => {
                    depth_bracket = depth_bracket.saturating_sub(1);
                }
                TokenKind::Lt => depth_angle += 1,
                TokenKind::Gt => {
                    depth_angle = depth_angle.saturating_sub(1);
                }
                _ => {}
            }
            let atom = self.atom_from_token()?;
            atoms.push(atom);
        }
        Ok(atoms)
    }

    pub(super) fn atom_from_token(&mut self) -> Result<Atom, ParseError> {
        let tok = self.advance().clone();
        if matches!(tok.kind, TokenKind::Ellipsis) {
            let commitment = self.commitment()?;
            return Ok(Atom::Ellipsis { commitment });
        }
        if let Some(kw) = tok.kind.as_keyword_str() {
            let commitment = self.commitment()?;
            return Ok(Atom::Ident {
                value: Ident {
                    name: kw.into(),
                    commitment,
                },
            });
        }
        match &tok.kind {
            TokenKind::Ident(s) => {
                let commitment = self.commitment()?;
                Ok(Atom::Ident {
                    value: Ident {
                        name: s.clone(),
                        commitment,
                    },
                })
            }
            TokenKind::String(s) => {
                let commitment = self.commitment()?;
                Ok(Atom::String {
                    value: FString {
                        value: s.clone(),
                        commitment,
                    },
                })
            }
            TokenKind::Number(s) => Ok(Atom::Number { value: s.clone() }),
            TokenKind::Colon => Ok(Atom::Symbol { value: ":".into() }),
            TokenKind::Comma => Ok(Atom::Symbol { value: ",".into() }),
            TokenKind::Pipe => Ok(Atom::Symbol { value: "|".into() }),
            TokenKind::LParen => Ok(Atom::Symbol { value: "(".into() }),
            TokenKind::RParen => Ok(Atom::Symbol { value: ")".into() }),
            TokenKind::LBracket => Ok(Atom::Symbol { value: "[".into() }),
            TokenKind::RBracket => Ok(Atom::Symbol { value: "]".into() }),
            TokenKind::Assign => Ok(Atom::Symbol { value: "=".into() }),
            TokenKind::Dot => Ok(Atom::Symbol { value: ".".into() }),
            TokenKind::EqEq => Ok(Atom::Symbol { value: "==".into() }),
            TokenKind::NotEq => Ok(Atom::Symbol { value: "!=".into() }),
            TokenKind::Lt => Ok(Atom::Symbol { value: "<".into() }),
            TokenKind::Gt => Ok(Atom::Symbol { value: ">".into() }),
            TokenKind::Le => Ok(Atom::Symbol { value: "<=".into() }),
            TokenKind::Ge => Ok(Atom::Symbol { value: ">=".into() }),
            TokenKind::Question => Ok(Atom::Symbol { value: "?".into() }),
            TokenKind::QuestionQuestion => Ok(Atom::Symbol { value: "??".into() }),
            TokenKind::Dollar => Ok(Atom::Symbol { value: "$".into() }),
            TokenKind::DollarDollar => Ok(Atom::Symbol { value: "$$".into() }),
            _ => Err(ParseError::unexpected_token(
                tok.kind.to_string(),
                "atom".into(),
                tok.line,
                tok.col,
            )),
        }
    }

    pub(super) fn parse_atom_list_literal(&mut self) -> Result<Atom, ParseError> {
        self.expect(TokenKind::LBracket, "`[`")?;
        let mut items = Vec::new();
        if !self.check(&TokenKind::RBracket) {
            loop {
                let item_atoms =
                    self.parse_atoms_until(&[TokenKind::Comma, TokenKind::RBracket])?;
                items.push(item_atoms);
                if self.matches(&TokenKind::Comma) {
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RBracket, "`]`")?;
        Ok(Atom::List { items })
    }

    // ── assignment helpers ──────────────────────────────────────────────────

    pub(super) fn parse_target_from_atoms(
        atoms: &[Atom],
        line: usize,
        col: usize,
    ) -> Result<Expr, ParseError> {
        if atoms.is_empty() {
            return Err(ParseError::unexpected_token(
                "empty".into(),
                "assignment target".into(),
                line,
                col,
            ));
        }
        let mut iter = atoms.iter();
        let Some(first) = iter.next() else {
            return Err(ParseError::unexpected_token(
                "empty".into(),
                "assignment target".into(),
                line,
                col,
            ));
        };
        let Atom::Ident { value: first_ident } = first else {
            return Err(ParseError::unexpected_token(
                "non-identifier target".into(),
                "identifier".into(),
                line,
                col,
            ));
        };
        let mut expr = Expr::Ident {
            value: first_ident.clone(),
        };
        while let Some(atom) = iter.next() {
            let Atom::Symbol { value: dot } = atom else {
                return Err(ParseError::unexpected_token(
                    "unexpected token in assignment target".into(),
                    "`.`".into(),
                    line,
                    col,
                ));
            };
            if dot != "." {
                return Err(ParseError::unexpected_token(
                    dot.clone(),
                    "`.`".into(),
                    line,
                    col,
                ));
            }
            let Some(next) = iter.next() else {
                return Err(ParseError::unexpected_token(
                    "EOF".into(),
                    "field name".into(),
                    line,
                    col,
                ));
            };
            let Atom::Ident { value: field } = next else {
                return Err(ParseError::unexpected_token(
                    "non-identifier".into(),
                    "field name".into(),
                    line,
                    col,
                ));
            };
            expr = Expr::Index {
                object: Box::new(expr),
                field: field.clone(),
            };
        }
        Ok(expr)
    }

    pub(super) fn parse_simple_value_from_atoms(
        atoms: &[Atom],
        line: usize,
        col: usize,
    ) -> Result<SimpleValue, ParseError> {
        if atoms.is_empty() {
            return Err(ParseError::unexpected_token(
                "empty".into(),
                "simple value".into(),
                line,
                col,
            ));
        }
        if atoms.len() > 1 {
            return Err(ParseError::unexpected_token(
                "compound expression".into(),
                "simple value (identifier, literal, or list literal)".into(),
                line,
                col,
            ));
        }
        match &atoms[0] {
            Atom::Ident { value: ident } => {
                if ident.name == "true" {
                    Ok(SimpleValue::Bool {
                        value: true,
                        keyword_commitment: ident.commitment,
                    })
                } else if ident.name == "false" {
                    Ok(SimpleValue::Bool {
                        value: false,
                        keyword_commitment: ident.commitment,
                    })
                } else {
                    Ok(SimpleValue::Ident {
                        value: ident.clone(),
                    })
                }
            }
            Atom::String { value: s } => Ok(SimpleValue::String { value: s.clone() }),
            Atom::Number { value: n } => Ok(SimpleValue::Number { value: n.clone() }),
            Atom::List { items } => Ok(SimpleValue::List {
                items: items.clone(),
            }),
            Atom::Ellipsis { commitment } => Ok(SimpleValue::Placeholder {
                commitment: *commitment,
            }),
            Atom::Symbol { value } => Err(ParseError::unexpected_token(
                value.clone(),
                "simple value".into(),
                line,
                col,
            )),
        }
    }
}
