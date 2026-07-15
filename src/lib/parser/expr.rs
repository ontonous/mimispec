use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::{BinOp, Parser, RecordedSlotKind};

const EXPR_RECURSION_LIMIT: u32 = 256;

impl Parser {
    pub(super) fn parse_condition(&mut self) -> Result<Condition, ParseError> {
        if self.check(&TokenKind::Ellipsis) {
            let keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
            return Ok(Condition::Structured {
                expr: Expr::Placeholder { keyword_commitment },
            });
        }
        if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            return Ok(Condition::Natural {
                text: self.fuzzy_string()?,
            });
        }
        let expr = self.parse_expr(0)?;
        Ok(Condition::Structured { expr })
    }

    pub(super) fn parse_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        self.parse_expr_depth(min_prec, 0)
    }

    fn parse_expr_depth(&mut self, min_prec: u8, depth: u32) -> Result<Expr, ParseError> {
        if depth > EXPR_RECURSION_LIMIT {
            let (line, col) = self.current_pos();
            return Err(ParseError::unexpected_token(
                "expression too deeply nested".into(),
                "expression".into(),
                line,
                col,
            ));
        }
        let mut lhs = self.parse_primary_depth(depth)?;
        loop {
            let (op, prec, right_assoc) = match self.peek_kind() {
                Some(TokenKind::Or) => (BinOp::Or, 1u8, false),
                Some(TokenKind::And) => (BinOp::And, 2u8, false),
                Some(TokenKind::In) => (BinOp::In, 3u8, false),
                Some(TokenKind::EqEq) => (BinOp::Cmp(CompareOp::Eq), 3u8, false),
                Some(TokenKind::NotEq) => (BinOp::Cmp(CompareOp::Ne), 3u8, false),
                Some(TokenKind::Lt) => (BinOp::Cmp(CompareOp::Lt), 3u8, false),
                Some(TokenKind::Gt) => (BinOp::Cmp(CompareOp::Gt), 3u8, false),
                Some(TokenKind::Le) => (BinOp::Cmp(CompareOp::Le), 3u8, false),
                Some(TokenKind::Ge) => (BinOp::Cmp(CompareOp::Ge), 3u8, false),
                Some(TokenKind::Pipe) => (BinOp::BitOr, 4u8, false),
                Some(TokenKind::BitXor) => (BinOp::BitXor, 5u8, false),
                Some(TokenKind::BitAnd) => (BinOp::BitAnd, 6u8, false),
                Some(TokenKind::Shl) => (BinOp::Shl, 7u8, false),
                Some(TokenKind::Shr) => (BinOp::Shr, 7u8, false),
                Some(TokenKind::Plus) => (BinOp::Add, 8u8, false),
                Some(TokenKind::Minus) => (BinOp::Sub, 8u8, false),
                Some(TokenKind::Star) => (BinOp::Mul, 9u8, false),
                Some(TokenKind::Slash) => (BinOp::Div, 9u8, false),
                Some(TokenKind::Power) => (BinOp::Pow, 10u8, true),
                Some(TokenKind::At) => (BinOp::MatMul, 9u8, false),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            let keyword_commitment = match op {
                BinOp::Or => self.expect_kw(TokenKind::Or, "`or`")?,
                BinOp::And => self.expect_kw(TokenKind::And, "`and`")?,
                BinOp::In => self.expect_kw(TokenKind::In, "`in`")?,
                BinOp::Cmp(op) => self.expect_kw(compare_token(op), "comparison operator")?,
                _ => {
                    self.advance();
                    Commitment::None
                }
            };
            let next_min = if right_assoc { prec } else { prec + 1 };
            let rhs = self.parse_expr_depth(next_min, depth + 1)?;
            lhs = match op {
                BinOp::Or => Expr::Or {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                    keyword_commitment,
                },
                BinOp::And => Expr::And {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                    keyword_commitment,
                },
                BinOp::In => Expr::In {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                    keyword_commitment,
                },
                BinOp::Cmp(op) => Expr::Compare {
                    left: Box::new(lhs),
                    op,
                    right: Box::new(rhs),
                    keyword_commitment,
                },
                BinOp::BitOr => Expr::BitOr {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::BitXor => Expr::BitXor {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::BitAnd => Expr::BitAnd {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Shl => Expr::Shl {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Shr => Expr::Shr {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Add => Expr::Add {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Sub => Expr::Sub {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Mul => Expr::Mul {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Div => Expr::Div {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::Pow => Expr::Pow {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                BinOp::MatMul => Expr::MatMul {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
            };
        }
        Ok(lhs)
    }

    fn parse_primary_depth(&mut self, depth: u32) -> Result<Expr, ParseError> {
        if depth > EXPR_RECURSION_LIMIT {
            let (line, col) = self.current_pos();
            return Err(ParseError::unexpected_token(
                "expression too deeply nested".into(),
                "expression".into(),
                line,
                col,
            ));
        }
        if self.check(&TokenKind::Not) {
            let keyword_commitment = self.expect_kw(TokenKind::Not, "`not`")?;
            let inner = self.parse_expr_depth(12, depth + 1)?;
            return Ok(Expr::Not {
                expr: Box::new(inner),
                keyword_commitment,
            });
        }
        if self.check(&TokenKind::Minus) {
            let keyword_commitment = self.expect_kw(TokenKind::Minus, "`-`")?;
            let inner = self.parse_expr_depth(12, depth + 1)?;
            return Ok(Expr::Neg {
                expr: Box::new(inner),
                keyword_commitment,
            });
        }
        if self.check(&TokenKind::BitNot) {
            let keyword_commitment = self.expect_kw(TokenKind::BitNot, "`~`")?;
            let inner = self.parse_expr_depth(12, depth + 1)?;
            return Ok(Expr::BitNot {
                expr: Box::new(inner),
                keyword_commitment,
            });
        }
        match self.peek_kind() {
            Some(TokenKind::Ident(_)) => {
                let id = self.fuzzy_ident()?;
                let mut expr = Expr::Ident { value: id };
                expr = self.parse_postfix_depth(expr, depth)?;
                Ok(expr)
            }
            Some(TokenKind::String(_)) => {
                let s = self.fuzzy_string()?;
                Ok(Expr::String { value: s })
            }
            Some(TokenKind::Number(_)) => {
                let n = self.advance();
                let TokenKind::Number(value) = &n.kind else {
                    unreachable!("advance() guaranteed Number token");
                };
                let value = value.clone();
                Ok(Expr::Number { value })
            }
            Some(TokenKind::True) => {
                self.advance();
                let keyword_commitment = self.commitment_after_previous(RecordedSlotKind::Value)?;
                Ok(Expr::Bool {
                    value: true,
                    keyword_commitment,
                })
            }
            Some(TokenKind::False) => {
                self.advance();
                let keyword_commitment = self.commitment_after_previous(RecordedSlotKind::Value)?;
                Ok(Expr::Bool {
                    value: false,
                    keyword_commitment,
                })
            }
            Some(TokenKind::LParen) => {
                self.advance();
                let inner = self.parse_expr_depth(0, depth + 1)?;
                self.expect(TokenKind::RParen, "`)`")?;
                Ok(inner)
            }
            Some(TokenKind::LBracket) => {
                self.advance();
                let mut items = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    items.push(self.parse_expr_depth(0, depth + 1)?);
                    while self.matches(&TokenKind::Comma) {
                        items.push(self.parse_expr_depth(0, depth + 1)?);
                    }
                }
                self.expect(TokenKind::RBracket, "`]`")?;
                Ok(Expr::List { items })
            }
            _ => {
                let (found, line, col) = match self.peek() {
                    Some(t) => (t.kind.to_string(), t.line, t.col),
                    None => ("EOF".into(), 0, 0),
                };
                Err(ParseError::unexpected_token(
                    found,
                    "expression".into(),
                    line,
                    col,
                ))
            }
        }
    }

    fn parse_postfix_depth(&mut self, expr: Expr, depth: u32) -> Result<Expr, ParseError> {
        let mut expr = expr;
        loop {
            if self.matches(&TokenKind::Dot) {
                let field = self.fuzzy_ident()?;
                expr = Expr::Index {
                    object: Box::new(expr),
                    field,
                };
            } else if self.check(&TokenKind::LParen) {
                let save = self.pos;
                let source_checkpoint = self.source_checkpoint();
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    args.push(self.parse_expr_depth(0, depth + 1)?);
                    while self.matches(&TokenKind::Comma) {
                        args.push(self.parse_expr_depth(0, depth + 1)?);
                    }
                }
                if self.check(&TokenKind::RParen) {
                    self.advance();
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                    };
                } else {
                    self.pos = save;
                    self.restore_source_checkpoint(source_checkpoint);
                    break;
                }
            } else if self.check(&TokenKind::LBracket) {
                self.advance();
                let mut indices = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    indices.push(self.parse_expr_depth(0, depth + 1)?);
                    while self.matches(&TokenKind::Comma) {
                        indices.push(self.parse_expr_depth(0, depth + 1)?);
                    }
                }
                self.expect(TokenKind::RBracket, "`]`")?;
                expr = Expr::Subscript {
                    object: Box::new(expr),
                    indices,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    pub(super) fn parse_math_block(&mut self) -> Result<MathBlock, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Math, "`math`")?;
        self.expect(TokenKind::Colon, "`:`")?;
        let statements = self.parse_block(|p| p.parse_math_statement());
        Ok(MathBlock {
            statements,
            keyword_commitment,
        })
    }

    pub(super) fn parse_math_statement(&mut self) -> Result<MathStatement, ParseError> {
        let expr = self.parse_expr(0)?;
        let stmt = if self.check(&TokenKind::Assign) {
            self.advance();
            let value = self.parse_expr(0)?;
            MathStatement::Define {
                target: expr,
                value,
            }
        } else {
            MathStatement::Expr { expr }
        };
        if !self.line_will_end() {
            let (found, line, col) = match self.peek() {
                Some(t) => (t.kind.to_string(), t.line, t.col),
                None => ("EOF".into(), 0, 0),
            };
            return Err(ParseError::unexpected_token(
                found,
                "end of math statement".into(),
                line,
                col,
            ));
        }
        Ok(stmt)
    }
}

fn compare_token(op: CompareOp) -> TokenKind {
    match op {
        CompareOp::Eq => TokenKind::EqEq,
        CompareOp::Ne => TokenKind::NotEq,
        CompareOp::Lt => TokenKind::Lt,
        CompareOp::Gt => TokenKind::Gt,
        CompareOp::Le => TokenKind::Le,
        CompareOp::Ge => TokenKind::Ge,
    }
}
