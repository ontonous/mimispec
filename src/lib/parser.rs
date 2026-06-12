use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    pending_rules: Vec<RuleDef>,
    errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            pending_rules: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn emit_error(&mut self, err: ParseError) {
        self.errors.push(err);
    }

    fn take_errors(&mut self) -> Vec<ParseError> {
        std::mem::take(&mut self.errors)
    }

    pub fn parse_file(&mut self) -> ParseResult {
        let mut errors = Vec::new();
        self.skip_newlines();

        let mut imports = Vec::new();
        // 解析文件顶部的 @import 指令
        while self.check(&TokenKind::Import) {
            self.advance();
            match self.expect(
                TokenKind::String(String::new()),
                "string literal (import path)",
            ) {
                Ok(tok) => {
                    let path = match &tok.kind {
                        TokenKind::String(s) => s.clone(),
                        _ => unreachable!(),
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
            // 跳过可能残留的 Dedent（来自之前失败 block 的尾部）
            while self.check(&TokenKind::Dedent) {
                self.advance();
            }
            if self.is_at_end() {
                break;
            }

            // 跳过 fragment 后面的 newline，然后收集前置 rule
            self.skip_newlines();
            let rule_errors = self.consume_pending_rules();
            errors.extend(rule_errors);

            // 跳过 newline，检测空行阻断（lexer 下无空行=2 个 newline，有空行>=3 个）
            let newline_count = self.skip_newlines_and_count();
            if newline_count >= 3 {
                global_rules.extend(self.take_pending_rules());
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

        // 文件末尾剩余的 pending_rules 变为全局约束
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

    /// 解析单个 Fragment（v0.3 核心入口）
    fn parse_fragment(&mut self) -> Result<Fragment, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Module) => Ok(Fragment::Module {
                module: self.parse_module()?,
            }),
            Some(TokenKind::Type) => Ok(Fragment::TypeDef {
                typedef: self.parse_type_def()?,
            }),
            Some(TokenKind::Flow) => Ok(Fragment::Flow {
                flow: self.parse_flow()?,
            }),
            Some(TokenKind::Func) => Ok(Fragment::Func {
                func: self.parse_func()?,
            }),
            Some(TokenKind::Ui) => Ok(Fragment::Ui {
                ui: self.parse_ui()?,
            }),
            Some(TokenKind::Steps) => self.parse_steps_fragment(),
            Some(TokenKind::Stack) | Some(TokenKind::Parallel) => Ok(Fragment::UiNode {
                node: self.parse_ui_node()?,
            }),
            Some(TokenKind::String(_)) => Ok(Fragment::UiNode {
                node: self.parse_ui_node()?,
            }),
            Some(TokenKind::Ellipsis) => {
                let _keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
                Ok(Fragment::Placeholder)
            }
            Some(TokenKind::If)
            | Some(TokenKind::For)
            | Some(TokenKind::While)
            | Some(TokenKind::Parasteps) => Ok(Fragment::Steps {
                steps: vec![self.parse_step()?],
            }),
            _ => {
                // 尝试作为裸表达式解析（v0.3: Expr Fragment）
                let save = self.pos;
                if let Ok(expr) = self.parse_expr(0) {
                    if self.line_will_end() {
                        return Ok(Fragment::Expr { expr });
                    }
                }
                self.pos = save;
                // 回退为单步 Steps fragment 解析（裸表达式或动作）
                Ok(Fragment::Steps {
                    steps: vec![self.parse_step()?],
                })
            }
        }
    }

    /// 解析独立的 steps: 块（v0.3 新增）
    fn parse_steps_fragment(&mut self) -> Result<Fragment, ParseError> {
        let _keyword_commitment = self.expect_kw(TokenKind::Steps, "`steps`")?;
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_block(|p| p.parse_step())?;
        Ok(Fragment::Steps { steps })
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if !matches!(tok.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        &self.tokens[self.pos.saturating_sub(1)]
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Eof) | None)
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    /// 消费连续的 rule 语句，收集到 pending_rules
    /// 返回解析过程中遇到的错误（不会 panic）。
    fn consume_pending_rules(&mut self) -> Vec<ParseError> {
        let mut errors = Vec::new();
        while self.check(&TokenKind::Rule) {
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

    /// 将 pending_rules 取出并清空
    fn take_pending_rules(&mut self) -> Vec<RuleDef> {
        std::mem::take(&mut self.pending_rules)
    }

    /// 将 pending_rules 附着到 fragment
    fn attach_rules_to_fragment(&mut self, fragment: &mut Fragment) {
        let rules = self.take_pending_rules();
        if rules.is_empty() {
            return;
        }
        match fragment {
            Fragment::Module { module } => module.rules.extend(rules),
            Fragment::TypeDef { typedef } => typedef.rules.extend(rules),
            Fragment::Flow { flow } => flow.rules.extend(rules),
            Fragment::Func { func } => func.rules.extend(rules),
            Fragment::Ui { .. } => { /* UiDef 暂不支持 rules */ }
            _ => { /* Steps/Expr/UiNode/Desc/Placeholder 不支持 rule 附着 */ }
        }
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<&Token, ParseError> {
        if let Some(tok) = self.peek() {
            if std::mem::discriminant(&tok.kind) == std::mem::discriminant(&kind) {
                return Ok(self.advance());
            }
        }
        let (found, line, col) = match self.peek() {
            Some(t) => (t.kind.to_string(), t.line, t.col),
            None => ("EOF".into(), 0, 0),
        };
        Err(ParseError::UnexpectedToken {
            found,
            expected: what.into(),
            line,
            col,
        })
    }

    fn commitment(&mut self) -> Result<Commitment, ParseError> {
        // 先读锁定后缀
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

        // 无锁定，再读不确定后缀
        if self.matches(&TokenKind::QuestionQuestion) {
            if self.check(&TokenKind::Dollar) || self.check(&TokenKind::DollarDollar) {
                let t = self.peek().unwrap();
                return Err(ParseError::UnexpectedToken {
                    found: t.kind.to_string(),
                    expected: "锁后缀必须在不确定后缀之前（`?$` / `?$$` 等顺序非法）".into(),
                    line: t.line,
                    col: t.col,
                });
            }
            return Ok(Commitment::QuestionQuestion);
        }

        if self.matches(&TokenKind::Question) {
            if self.check(&TokenKind::Dollar) || self.check(&TokenKind::DollarDollar) {
                let t = self.peek().unwrap();
                return Err(ParseError::UnexpectedToken {
                    found: t.kind.to_string(),
                    expected: "锁后缀必须在不确定后缀之前（`?$` / `?$$` 等顺序非法）".into(),
                    line: t.line,
                    col: t.col,
                });
            }
            return Ok(Commitment::Question);
        }

        Ok(Commitment::None)
    }

    fn expect_kw(&mut self, kind: TokenKind, what: &str) -> Result<Commitment, ParseError> {
        self.expect(kind, what)?;
        Ok(self.commitment()?)
    }

    fn fuzzy_ident(&mut self) -> Result<Ident, ParseError> {
        let tok = self.peek().ok_or(ParseError::UnexpectedEof)?.clone();
        let name = if let Some(kw) = tok.kind.as_keyword_str() {
            self.advance();
            kw.to_string()
        } else if let TokenKind::Ident(s) = &tok.kind {
            self.advance();
            s.clone()
        } else {
            return Err(ParseError::UnexpectedToken {
                found: tok.kind.to_string(),
                expected: "identifier".into(),
                line: tok.line,
                col: tok.col,
            });
        };
        let commitment = self.commitment()?;
        Ok(Ident { name, commitment })
    }

    fn fuzzy_string(&mut self) -> Result<FString, ParseError> {
        let tok = self.expect(TokenKind::String(String::new()), "string literal")?;
        let value = match &tok.kind {
            TokenKind::String(s) => s.clone(),
            _ => unreachable!(),
        };
        let commitment = self.commitment()?;
        Ok(FString { value, commitment })
    }

    fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn skip_newlines_and_count(&mut self) -> usize {
        let mut count = 0;
        while self.check(&TokenKind::Newline) {
            self.advance();
            count += 1;
        }
        count
    }

    /// 同步：跳过当前 import 的剩余部分，停在下一个 import 或 fragment 开头
    fn synchronize_past_import(&mut self) {
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

    /// 同步：跳到下一个可能的 fragment 开头（或 Dedent/EOF）
    fn synchronize_to_fragment_start(&mut self) {
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

    /// 同步：从 block 内错误位置跳到下一个同级 item 或当前 block 结束。
    ///
    /// 与 `synchronize_to_fragment_start` 不同，本方法维护一个缩进深度计数器：
    /// 遇到 `Indent` 时 depth++，遇到 `Dedent` 时 depth--；
    /// 当 depth 回到 0 并遇到 `Dedent`，或者直接遇到下一个 fragment/item 开头时停止。
    /// 这能避免错误恢复把残缺实体内部的内容（例如 `func Broken(:` 后面的
    /// `steps: do something`）误当成 module 级别的 fragment 解析，从而让后续
    /// 同级实体（如 `func Good`）继续保留在当前 block 中。
    fn synchronize_past_nested_block(&mut self) {
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

    /// 同步：从 block 内错误位置跳到下一个同级 item 或当前 block 结束。
    ///
    /// 与 `synchronize_past_nested_block` 不同，本方法在 depth==0 时遇到 newline
    /// 就会停止，从而让 parse_block 能在下一行继续尝试解析。这适合 steps/fields
    /// 等 block 场景：一行出错只跳过当前行，不吞掉后续合法 item。
    fn synchronize_to_next_item_in_block(&mut self) {
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

    fn parse_block<T>(
        &mut self,
        mut parse_item: impl FnMut(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;
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
        Ok(items)
    }

    // ── module / items ────────────────────────────────────────────────────────

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Module, "`module`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        let mut desc = None;
        let mut rules = Vec::new();
        let mut items = Vec::new();

        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }

            // 收集 block 内的前置 rule
            while self.check(&TokenKind::Rule) {
                rules.push(self.parse_rule_def()?);
            }
            self.skip_newlines();

            if self.check(&TokenKind::Desc) {
                let d = self.parse_desc_entity()?;
                if desc.is_none() {
                    desc = Some(d);
                } else {
                    // 额外的 desc 作为 Fragment::Desc
                    items.push(Fragment::Steps {
                        steps: vec![Step::Desc { content: d }],
                    });
                }
            } else {
                match self.parse_fragment() {
                    Ok(fragment) => {
                        items.push(fragment);
                    }
                    Err(e) => {
                        self.emit_error(e);
                        let before = self.pos;
                        self.synchronize_past_nested_block();
                        // 若同步点未前进且未到 EOF，强制前进一步，避免死循环
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

        Ok(Module {
            name,
            desc,
            rules,
            items,
            keyword_commitment,
        })
    }

    /// 解析 desc "..." 作为独立实体
    fn parse_desc_entity(&mut self) -> Result<Desc, ParseError> {
        self.expect(TokenKind::Desc, "`desc`")?;
        let need_commitment = self.commitment()?;
        let content = self.fuzzy_string()?;
        Ok(Desc {
            need_commitment,
            content,
        })
    }

    // ── type ──────────────────────────────────────────────────────────────────

    fn parse_type_def(&mut self) -> Result<TypeDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Type, "`type`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        // 单行占位或 inline enum
        if self.check(&TokenKind::Ellipsis) {
            self.advance();
            return Ok(TypeDef {
                name,
                desc: None,
                rules: Vec::new(),
                body: TypeBody::Record { fields: vec![] },
                keyword_commitment,
            });
        }

        if self.is_inline_enum() {
            return Ok(TypeDef {
                name,
                desc: None,
                rules: Vec::new(),
                body: TypeBody::Enum {
                    variants: self.parse_variant_list()?,
                },
                keyword_commitment,
            });
        }

        // 记录 block
        self.skip_newlines();
        self.expect(TokenKind::Indent, "indented block")?;

        let mut desc = None;
        let mut fields = Vec::new();
        let mut pending_field_rules: Vec<RuleDef> = Vec::new();

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }

            if self.check(&TokenKind::Rule) {
                // 收集连续 rule；rule 必须在后续某次迭代中附着给 field，
                // 因此消费完后 continue，避免把 rule 误解析为 field 名。
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
                // 额外的 desc 在 type body 中暂时没有独立载体，保持忽略
            } else if self.check(&TokenKind::Ellipsis) {
                self.advance();
            } else {
                // field 定义：接收 pending 的字段级 rule
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

        // rules 字段由 attach_rules_to_fragment 填充外部前置 rule
        Ok(TypeDef {
            name,
            desc,
            rules: Vec::new(),
            body: TypeBody::Record { fields },
            keyword_commitment,
        })
    }

    fn is_inline_enum(&mut self) -> bool {
        // lookahead: ident | ... before newline
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

    // ── rule ──────────────────────────────────────────────────────────────────

    /// 解析 rule "string"（无标签）
    fn parse_rule_def(&mut self) -> Result<RuleDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Rule, "`rule`")?;
        let content = self.fuzzy_string()?;
        let desc = Desc {
            need_commitment: Commitment::None,
            content,
        };
        Ok(RuleDef {
            desc,
            keyword_commitment,
        })
    }

    // ── flow ──────────────────────────────────────────────────────────────────

    fn parse_flow(&mut self) -> Result<FlowDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Flow, "`flow`")?;
        let name = self.fuzzy_ident()?;
        self.expect(TokenKind::Colon, "`:`")?;

        // v0.3: 支持 flow Lifecycle: ... 作为单行占位
        if self.check(&TokenKind::Ellipsis) {
            self.advance();
            return Ok(FlowDef {
                name,
                desc: None,
                rules: Vec::new(),
                entries: vec![],
                keyword_commitment,
            });
        }

        let entries = self.parse_block(|p| p.parse_flow_entry())?;

        Ok(FlowDef {
            name,
            desc: None,
            rules: Vec::new(),
            entries,
            keyword_commitment,
        })
    }

    fn parse_flow_entry(&mut self) -> Result<FlowEntry, ParseError> {
        let state = self.fuzzy_ident()?;
        if self.check(&TokenKind::To) {
            let to_keyword_commitment = self.expect_kw(TokenKind::To, "`to`")?;
            let arm = self.parse_flow_arm_after_to_with_commitment(to_keyword_commitment)?;
            Ok(FlowEntry {
                state,
                rules: Vec::new(),
                arms: vec![arm],
            })
        } else {
            self.expect(TokenKind::Colon, "`:` or `to`")?;
            let arms = self.parse_block(|p| p.parse_flow_arm_in_block())?;
            Ok(FlowEntry {
                state,
                rules: Vec::new(),
                arms,
            })
        }
    }

    fn parse_flow_arm_in_block(&mut self) -> Result<FlowArm, ParseError> {
        let to_keyword_commitment = self.expect_kw(TokenKind::To, "`to`")?;
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
                return Err(ParseError::UnexpectedToken {
                    found,
                    expected: "`requires` or `desc`".into(),
                    line,
                    col,
                });
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

    fn line_will_end(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof)
        )
    }

    // ── func ──────────────────────────────────────────────────────────────────

    fn parse_func(&mut self) -> Result<FuncDef, ParseError> {
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
        // v0.3: 支持 func Pay(order): ... 作为单行函数体占位
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
                steps: vec![],
                keyword_commitment,
                requires_keyword_commitment: Commitment::None,
                ensures_keyword_commitment: Commitment::None,
                with_keyword_commitment,
                steps_keyword_commitment: Commitment::None,
            });
        }
        self.expect(TokenKind::Indent, "indented block")?;

        let mut desc = None;
        let mut rules = Vec::new();
        let mut requires = None;
        let mut requires_keyword_commitment = Commitment::None;
        let mut ensures = None;
        let mut ensures_keyword_commitment = Commitment::None;
        let mut steps = Vec::new();
        let mut steps_keyword_commitment = Commitment::None;

        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            if self.check(&TokenKind::Ellipsis) {
                // v0.3: block 内的 ... 占位符，消费掉并继续
                self.advance();
                continue;
            }
            // 收集前置 rule
            while self.check(&TokenKind::Rule) {
                rules.push(self.parse_rule_def()?);
            }
            self.skip_newlines();
            if self.check(&TokenKind::Desc) {
                let d = self.parse_desc_entity()?;
                if desc.is_none() {
                    desc = Some(d);
                } else {
                    // 额外的 desc 作为 Desc step
                    steps.push(Step::Desc { content: d });
                }
            } else if self.check(&TokenKind::Requires) {
                requires_keyword_commitment = self.expect_kw(TokenKind::Requires, "`requires`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                requires = Some(self.parse_condition()?);
            } else if self.check(&TokenKind::Ensures) {
                ensures_keyword_commitment = self.expect_kw(TokenKind::Ensures, "`ensures`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                ensures = Some(self.parse_condition()?);
            } else if self.check(&TokenKind::Steps) {
                steps_keyword_commitment = self.expect_kw(TokenKind::Steps, "`steps`")?;
                self.expect(TokenKind::Colon, "`:`")?;
                steps = self.parse_block(|p| p.parse_step())?;
            } else {
                // 尝试作为裸 step 解析
                let save = self.pos;
                match self.parse_step() {
                    Ok(step) => steps.push(step),
                    Err(e) => {
                        self.pos = save;
                        self.emit_error(e);
                        let before = self.pos;
                        self.synchronize_past_nested_block();
                        // 若同步点未前进且未到 EOF，强制前进一步，避免死循环
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

        Ok(FuncDef {
            name,
            desc,
            rules,
            params,
            capabilities,
            requires,
            ensures,
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

    fn parse_capabilities(&mut self) -> Result<Vec<Capability>, ParseError> {
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

    // ── condition / expression ────────────────────────────────────────────────

    fn parse_condition(&mut self) -> Result<Condition, ParseError> {
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

    fn parse_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_primary()?;
        loop {
            let (op, prec, right_assoc) = match self.peek_kind() {
                Some(TokenKind::Or) => (BinOp::Or, 1u8, false),
                Some(TokenKind::And) => (BinOp::And, 2u8, false),
                Some(TokenKind::In) => (BinOp::In, 4u8, false),
                Some(TokenKind::EqEq) => (BinOp::Cmp(CompareOp::Eq), 5u8, false),
                Some(TokenKind::NotEq) => (BinOp::Cmp(CompareOp::Ne), 5u8, false),
                Some(TokenKind::Lt) => (BinOp::Cmp(CompareOp::Lt), 5u8, false),
                Some(TokenKind::Gt) => (BinOp::Cmp(CompareOp::Gt), 5u8, false),
                Some(TokenKind::Le) => (BinOp::Cmp(CompareOp::Le), 5u8, false),
                Some(TokenKind::Ge) => (BinOp::Cmp(CompareOp::Ge), 5u8, false),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            let keyword_commitment = match op {
                BinOp::Or => self.expect_kw(TokenKind::Or, "`or`")?,
                BinOp::And => self.expect_kw(TokenKind::And, "`and`")?,
                BinOp::In => self.expect_kw(TokenKind::In, "`in`")?,
                BinOp::Cmp(_) => {
                    self.advance();
                    Commitment::None
                }
            };
            let next_min = if right_assoc { prec } else { prec + 1 };
            let rhs = self.parse_expr(next_min)?;
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
                },
            };
        }
        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        if self.check(&TokenKind::Not) {
            let keyword_commitment = self.expect_kw(TokenKind::Not, "`not`")?;
            let inner = self.parse_expr(4)?;
            return Ok(Expr::Not {
                expr: Box::new(inner),
                keyword_commitment,
            });
        }
        match self.peek_kind() {
            Some(TokenKind::Ident(_)) => {
                let id = self.fuzzy_ident()?;
                let mut expr = Expr::Ident { value: id };
                expr = self.parse_postfix(expr)?;
                Ok(expr)
            }
            Some(TokenKind::String(_)) => {
                let s = self.fuzzy_string()?;
                Ok(Expr::String { value: s })
            }
            Some(TokenKind::Number(_)) => {
                let n = self.advance();
                let value = match &n.kind {
                    TokenKind::Number(s) => s.clone(),
                    _ => unreachable!(),
                };
                Ok(Expr::Number { value })
            }
            Some(TokenKind::True) => {
                self.advance();
                let keyword_commitment = self.commitment()?;
                Ok(Expr::Bool {
                    value: true,
                    keyword_commitment,
                })
            }
            Some(TokenKind::False) => {
                self.advance();
                let keyword_commitment = self.commitment()?;
                Ok(Expr::Bool {
                    value: false,
                    keyword_commitment,
                })
            }
            Some(TokenKind::LParen) => {
                self.advance();
                let inner = self.parse_expr(0)?;
                self.expect(TokenKind::RParen, "`)`")?;
                Ok(inner)
            }
            Some(TokenKind::LBracket) => {
                self.advance();
                let mut items = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    items.push(self.parse_expr(0)?);
                    while self.matches(&TokenKind::Comma) {
                        items.push(self.parse_expr(0)?);
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
                Err(ParseError::UnexpectedToken {
                    found,
                    expected: "expression".into(),
                    line,
                    col,
                })
            }
        }
    }

    fn parse_postfix(&mut self, expr: Expr) -> Result<Expr, ParseError> {
        let mut expr = expr;
        loop {
            if self.matches(&TokenKind::Dot) {
                let field = self.fuzzy_ident()?;
                expr = Expr::Index {
                    object: Box::new(expr),
                    field,
                };
            } else if self.check(&TokenKind::LParen) {
                // only treat as call if lhs is an ident or another call/index
                let save = self.pos;
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    args.push(self.parse_expr(0)?);
                    while self.matches(&TokenKind::Comma) {
                        args.push(self.parse_expr(0)?);
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
                    break;
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    // ── steps ─────────────────────────────────────────────────────────────────

    fn parse_step(&mut self) -> Result<Step, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::If) => self.parse_if_step(),
            Some(TokenKind::For) => self.parse_for_step(),
            Some(TokenKind::While) => self.parse_while_step(),
            Some(TokenKind::Parasteps) => self.parse_parasteps_step(),
            Some(TokenKind::Error) => self.parse_error_step(),
            Some(TokenKind::Ellipsis) => self.parse_placeholder_step(),
            Some(TokenKind::Desc) => self.parse_desc_step(),
            _ => self.parse_action_step(),
        }
    }

    fn parse_desc_step(&mut self) -> Result<Step, ParseError> {
        let content = self.parse_desc_entity()?;
        Ok(Step::Desc { content })
    }

    fn parse_placeholder_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Ellipsis, "`...`")?;
        Ok(Step::Placeholder { keyword_commitment })
    }

    fn parse_if_step(&mut self) -> Result<Step, ParseError> {
        let if_keyword_commitment = self.expect_kw(TokenKind::If, "`if`")?;
        let cond = self.parse_condition()?;
        self.expect(TokenKind::Colon, "`:`")?;
        let then_branch = self.parse_block(|p| p.parse_step())?;
        let mut else_branch = None;
        let mut else_keyword_commitment = Commitment::None;
        self.skip_newlines();
        if self.check(&TokenKind::Else) {
            else_keyword_commitment = self.expect_kw(TokenKind::Else, "`else`")?;
            self.expect(TokenKind::Colon, "`:`")?;
            else_branch = Some(self.parse_block(|p| p.parse_step())?);
        }
        Ok(Step::If {
            step: IfStep {
                cond,
                then_branch,
                else_branch,
                if_keyword_commitment,
                else_keyword_commitment,
            },
        })
    }

    fn parse_for_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::For, "`for`")?;
        let var = self.fuzzy_ident()?;
        self.expect(TokenKind::In, "`in`")?;
        let iterable = self.parse_atoms_until(&[
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;
        self.expect(TokenKind::Colon, "`:`")?;
        let body = self.parse_block(|p| p.parse_step())?;
        Ok(Step::For {
            step: ForStep {
                var,
                iterable,
                body,
                keyword_commitment,
            },
        })
    }

    fn parse_while_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::While, "`while`")?;
        let cond = self.parse_condition()?;
        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let body = self.parse_block(|p| p.parse_step())?;
        Ok(Step::While {
            step: WhileStep {
                cond,
                desc,
                body,
                keyword_commitment,
            },
        })
    }

    fn parse_parasteps_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Parasteps, "`parasteps`")?;
        let description = if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            Some(self.fuzzy_string()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_block(|p| p.parse_step())?;
        Ok(Step::Parasteps {
            step: ParastepsStep {
                description,
                steps,
                keyword_commitment,
            },
        })
    }

    fn parse_error_step(&mut self) -> Result<Step, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Error, "`error`")?;
        let message = self.fuzzy_string()?;
        let to = if self.matches(&TokenKind::To) {
            Some(ToTarget {
                target: self.fuzzy_ident()?,
            })
        } else {
            None
        };
        Ok(Step::Error {
            step: ErrorStep {
                message,
                to,
                keyword_commitment,
            },
        })
    }

    fn parse_action_step(&mut self) -> Result<Step, ParseError> {
        let atoms = self.parse_atoms_until(&[
            TokenKind::Desc,
            TokenKind::To,
            TokenKind::On,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;

        // 尝试识别赋值：target = simple_value
        if let Some(eq_idx) = atoms
            .iter()
            .position(|a| matches!(a, Atom::Symbol { value } if value == "="))
        {
            let lhs = &atoms[..eq_idx];
            let rhs = &atoms[eq_idx + 1..];
            if rhs
                .iter()
                .any(|a| matches!(a, Atom::Symbol { value } if value == "="))
            {
                return Err(ParseError::UnexpectedToken {
                    found: "=".into(),
                    expected: "single assignment per step".into(),
                    line: 0,
                    col: 0,
                });
            }
            let target = Self::parse_target_from_atoms(lhs)?;
            let value = Self::parse_simple_value_from_atoms(rhs)?;

            let desc = if self.matches(&TokenKind::Desc) {
                Some(self.parse_desc_after_keyword()?)
            } else {
                None
            };

            let to = if self.matches(&TokenKind::To) {
                Some(ToTarget {
                    target: self.fuzzy_ident()?,
                })
            } else {
                None
            };

            if self.check(&TokenKind::Newline) {
                self.advance();
            }

            let mut on_blocks = Vec::new();
            loop {
                self.skip_newlines();
                if self.check(&TokenKind::On) {
                    on_blocks.push(self.parse_on_block()?);
                } else {
                    break;
                }
            }

            return Ok(Step::Assign {
                step: AssignStep {
                    target,
                    value,
                    desc,
                    to,
                    on_blocks,
                },
            });
        }

        let desc = if self.matches(&TokenKind::Desc) {
            Some(self.parse_desc_after_keyword()?)
        } else {
            None
        };

        let to = if self.matches(&TokenKind::To) {
            Some(ToTarget {
                target: self.fuzzy_ident()?,
            })
        } else {
            None
        };

        if self.check(&TokenKind::Newline) {
            self.advance();
        }

        let mut on_blocks = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::On) {
                on_blocks.push(self.parse_on_block()?);
            } else {
                break;
            }
        }

        Ok(Step::Action {
            step: ActionStep {
                label: atoms,
                desc,
                to,
                on_blocks,
            },
        })
    }

    fn parse_desc_after_keyword(&mut self) -> Result<Desc, ParseError> {
        let need_commitment = self.commitment()?;
        let content = self.fuzzy_string()?;
        Ok(Desc {
            need_commitment,
            content,
        })
    }

    fn parse_on_block(&mut self) -> Result<OnBlock, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::On, "`on`")?;
        let condition = self.parse_atoms_until(&[
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ])?;
        self.expect(TokenKind::Colon, "`:`")?;
        let steps = self.parse_block(|p| p.parse_step())?;
        Ok(OnBlock {
            condition,
            steps,
            keyword_commitment,
        })
    }

    // ── assignment helpers ──────────────────────────────────────────────────

    fn parse_target_from_atoms(atoms: &[Atom]) -> Result<Expr, ParseError> {
        if atoms.is_empty() {
            return Err(ParseError::UnexpectedToken {
                found: "empty".into(),
                expected: "assignment target".into(),
                line: 0,
                col: 0,
            });
        }
        let mut iter = atoms.iter();
        let first = iter.next().unwrap();
        let Atom::Ident { value: first_ident } = first else {
            return Err(ParseError::UnexpectedToken {
                found: "non-identifier target".into(),
                expected: "identifier".into(),
                line: 0,
                col: 0,
            });
        };
        let mut expr = Expr::Ident {
            value: first_ident.clone(),
        };
        while let Some(atom) = iter.next() {
            let Atom::Symbol { value: dot } = atom else {
                return Err(ParseError::UnexpectedToken {
                    found: "unexpected token in assignment target".into(),
                    expected: "`.`".into(),
                    line: 0,
                    col: 0,
                });
            };
            if dot != "." {
                return Err(ParseError::UnexpectedToken {
                    found: dot.clone(),
                    expected: "`.`".into(),
                    line: 0,
                    col: 0,
                });
            }
            let Some(next) = iter.next() else {
                return Err(ParseError::UnexpectedToken {
                    found: "EOF".into(),
                    expected: "field name".into(),
                    line: 0,
                    col: 0,
                });
            };
            let Atom::Ident { value: field } = next else {
                return Err(ParseError::UnexpectedToken {
                    found: "non-identifier".into(),
                    expected: "field name".into(),
                    line: 0,
                    col: 0,
                });
            };
            expr = Expr::Index {
                object: Box::new(expr),
                field: field.clone(),
            };
        }
        Ok(expr)
    }

    fn parse_simple_value_from_atoms(atoms: &[Atom]) -> Result<SimpleValue, ParseError> {
        if atoms.is_empty() {
            return Err(ParseError::UnexpectedToken {
                found: "empty".into(),
                expected: "simple value".into(),
                line: 0,
                col: 0,
            });
        }
        if atoms.len() > 1 {
            return Err(ParseError::UnexpectedToken {
                found: "compound expression".into(),
                expected: "simple value (identifier, literal, or list literal)".into(),
                line: 0,
                col: 0,
            });
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
            Atom::Symbol { value } => Err(ParseError::UnexpectedToken {
                found: value.clone(),
                expected: "simple value".into(),
                line: 0,
                col: 0,
            }),
        }
    }

    fn parse_atom_list_literal(&mut self) -> Result<Atom, ParseError> {
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

    // ── atom sequence helper ──────────────────────────────────────────────────

    fn parse_atoms_until(&mut self, stop: &[TokenKind]) -> Result<Vec<Atom>, ParseError> {
        let mut atoms = Vec::new();
        let mut depth_paren = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;
        loop {
            if let Some(tok) = self.peek() {
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
                    // 顶层 `[` 表示列表字面量，解析为 Atom::List
                    if matches!(tok.kind, TokenKind::LBracket) {
                        atoms.push(self.parse_atom_list_literal()?);
                        continue;
                    }
                }
                match &tok.kind {
                    TokenKind::LParen => depth_paren += 1,
                    TokenKind::RParen => {
                        if depth_paren > 0 {
                            depth_paren -= 1;
                        }
                    }
                    TokenKind::LBracket => depth_bracket += 1,
                    TokenKind::RBracket => {
                        if depth_bracket > 0 {
                            depth_bracket -= 1;
                        }
                    }
                    TokenKind::Lt => depth_angle += 1,
                    TokenKind::Gt => {
                        if depth_angle > 0 {
                            depth_angle -= 1;
                        }
                    }
                    _ => {}
                }
                let atom = self.atom_from_token()?;
                atoms.push(atom);
            } else {
                break;
            }
        }
        Ok(atoms)
    }

    fn atom_from_token(&mut self) -> Result<Atom, ParseError> {
        let tok = self.advance().clone();
        // 特殊处理 ...，不作为普通 keyword 解析
        if matches!(tok.kind, TokenKind::Ellipsis) {
            let commitment = self.commitment()?;
            return Ok(Atom::Ident {
                value: Ident {
                    name: "...".into(),
                    commitment,
                },
            });
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
            _ => Err(ParseError::UnexpectedToken {
                found: tok.kind.to_string(),
                expected: "atom".into(),
                line: tok.line,
                col: tok.col,
            }),
        }
    }
}

#[derive(Clone, Copy)]
enum BinOp {
    Or,
    And,
    In,
    Cmp(CompareOp),
}

// ── ui ──────────────────────────────────────────────────────────────────────

impl Parser {
    fn parse_ui(&mut self) -> Result<UiDef, ParseError> {
        let keyword_commitment = self.expect_kw(TokenKind::Ui, "`ui`")?;
        let name = self.fuzzy_ident()?;
        let mut binds = None;
        if self.check(&TokenKind::Binds) {
            self.advance();
            binds = Some(self.fuzzy_ident()?);
        }
        self.expect(TokenKind::Colon, "`:`")?;
        // v0.3: 支持 ui View: ... 作为单行占位
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

    fn parse_ui_node(&mut self) -> Result<UiNode, ParseError> {
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
        // 导航: to Ident
        if self.check(&TokenKind::To) {
            self.advance();
            let target = self.fuzzy_ident()?;
            return Ok(Action::Navigate { target });
        }

        // 自然语言描述: "..."
        if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            let text = self.fuzzy_string()?;
            return Ok(Action::Natural { text });
        }

        // 表达式（可能是函数调用或赋值左侧）
        let expr = self.parse_expr(0)?;

        // 赋值: Expr = Expr
        if self.matches(&TokenKind::Assign) {
            let value = self.parse_expr(0)?;
            return Ok(Action::Assign {
                target: expr,
                value,
            });
        }

        // 函数调用或其他表达式
        Ok(Action::Call { expr })
    }
}
