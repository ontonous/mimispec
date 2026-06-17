use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_ui(&mut self) -> Result<UiDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Ui, "`ui`")?;
        let name = self.fuzzy_ident()?;
        let mut binds = None;
        if self.check(&TokenKind::Binds) {
            self.advance();
            binds = Some(self.fuzzy_ident()?);
        }
        self.expect(TokenKind::Colon, "`:`")?;
        if self.check(&TokenKind::Ellipsis) {
            self.advance();
            return Ok(UiDef {
                name,
                binds,
                root: UiNode::Stack {
                    stack: StackNode {
                        description: None,
                        children: vec![],
                        keyword_commitment: Commitment::None,
                    },
                },
                keyword_commitment,
            });
        }
        let root = self.parse_ui_root()?;
        Ok(UiDef {
            name,
            binds,
            root,
            keyword_commitment,
        })
    }

    fn parse_ui_root(&mut self) -> Result<UiNode, ParseError> {
        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
        self.skip_newlines();
        let node = match self.peek_kind() {
            Some(TokenKind::Stack) => self.parse_stack_node(),
            Some(TokenKind::Parallel) => self.parse_parallel_node(),
            _ => {
                let (found, line, col) = match self.peek() {
                    Some(t) => (t.kind.to_string(), t.line, t.col),
                    None => ("EOF".into(), 0, 0),
                };
                Err(ParseError::UnexpectedToken {
                    found,
                    expected: "`stack` or `parallel`".into(),
                    line,
                    col,
                })
            }
        }?;
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        Ok(node)
    }

    pub(super) fn parse_ui_node(&mut self) -> Result<UiNode, ParseError> {
        self.skip_newlines();
        match self.peek_kind() {
            Some(TokenKind::Stack) => self.parse_stack_node(),
            Some(TokenKind::Parallel) => self.parse_parallel_node(),
            Some(TokenKind::String(_)) => self.parse_ui_leaf(),
            Some(TokenKind::Error) => self.parse_error_node(),
            _ => {
                let (found, line, col) = match self.peek() {
                    Some(t) => (t.kind.to_string(), t.line, t.col),
                    None => ("EOF".into(), 0, 0),
                };
                Err(ParseError::UnexpectedToken {
                    found,
                    expected: "`stack`, `parallel`, `error` or string literal".into(),
                    line,
                    col,
                })
            }
        }
    }

    fn parse_stack_node(&mut self) -> Result<UiNode, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Stack, "`stack`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let children = self.parse_block(|p| p.parse_ui_node())?;
        Ok(UiNode::Stack {
            stack: StackNode {
                description,
                children,
                keyword_commitment,
            },
        })
    }

    fn parse_parallel_node(&mut self) -> Result<UiNode, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Parallel, "`parallel`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let children = self.parse_block(|p| p.parse_ui_node())?;
        Ok(UiNode::Parallel {
            parallel: StackNode {
                description,
                children,
                keyword_commitment,
            },
        })
    }

    fn parse_error_node(&mut self) -> Result<UiNode, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Error, "`error`")?;
        let message = self.fuzzy_string()?;
        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };
        Ok(UiNode::Error {
            error: UiErrorNode {
                message,
                desc,
                keyword_commitment,
            },
        })
    }

    fn parse_ui_leaf(&mut self) -> Result<UiNode, ParseError> {
        let content = self.fuzzy_string()?;
        let mut desc = None;
        let mut requires = None;
        let mut with = Vec::new();
        let mut on_binding = None;

        loop {
            if self.line_will_end() {
                break;
            }
            if self.matches(&TokenKind::Desc) {
                desc = Some(self.parse_desc_after_keyword()?);
            } else if self.check(&TokenKind::Requires) {
                let _requires_kw_commitment = self.expect_kw(TokenKind::Requires, "`requires`")?;
                requires = Some(self.parse_condition()?);
            } else if self.check(&TokenKind::With) {
                let _with_kw_commitment = self.expect_kw(TokenKind::With, "`with`")?;
                with = self.parse_capabilities()?;
            } else if self.check(&TokenKind::On) {
                on_binding = Some(self.parse_on_binding()?);
            } else {
                break;
            }
        }

        Ok(UiNode::Leaf {
            leaf: UiLeaf {
                content,
                desc,
                requires,
                with,
                on: on_binding,
            },
        })
    }

    fn parse_on_binding(&mut self) -> Result<OnBinding, ParseError> {
        self.expect_kw(TokenKind::On, "`on`")?;
        let event_name = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            EventName::Natural {
                text: self.fuzzy_string()?,
            }
        } else {
            EventName::Ident {
                value: self.fuzzy_ident()?,
            }
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let action = self.parse_action_expr()?;
        Ok(OnBinding { event_name, action })
    }

    fn parse_action_expr(&mut self) -> Result<ActionExpr, ParseError> {
        let mut actions = Vec::new();
        actions.push(self.parse_action()?);
        while self.matches(&TokenKind::Comma) {
            actions.push(self.parse_action()?);
        }
        Ok(ActionExpr { actions })
    }

    fn parse_action(&mut self) -> Result<Action, ParseError> {
        if self.check(&TokenKind::To) {
            self.advance();
            let target = self.fuzzy_ident()?;
            return Ok(Action::Navigate { target });
        }

        if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            let text = self.fuzzy_string()?;
            return Ok(Action::Natural { text });
        }

        let expr = self.parse_expr(0)?;

        if self.matches(&TokenKind::Assign) {
            let value = self.parse_expr(0)?;
            return Ok(Action::Assign {
                target: expr,
                value,
            });
        }

        Ok(Action::Call { expr })
    }
}
