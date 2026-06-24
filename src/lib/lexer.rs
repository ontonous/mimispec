use crate::error::ParseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Keywords
    Module,
    Type,
    Rule,
    Flow,
    Func,
    Ui,
    Parallel,
    Stack,
    Binds,
    Requires,
    Ensures,
    Steps,
    Math,
    Parasteps,
    If,
    Else,
    For,
    While,
    Desc,
    On,
    With,
    Error,
    And,
    Or,
    Not,
    In,
    Done,
    Exit,
    True,
    False,
    Import,   // v0.3 新增：@import 跨文件指令
    Ellipsis, // v0.3 新增：... 占位符
    At,       // `@` 矩阵乘法运算符

    // Literals
    Ident(String),
    String(String),
    Number(String),

    // Punctuation / operators
    Colon,
    Comma,
    Pipe, // `|` used as enum separator and bitwise OR in math
    LParen,
    RParen,
    LBracket,
    RBracket,
    Assign,
    Dot,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,

    // Math / arithmetic / bitwise operators
    Plus,   // `+`
    Minus,  // `-`
    Star,   // `*`
    Slash,  // `/`
    Power,  // `**`
    BitAnd, // `&`
    BitXor, // `^`
    BitNot, // `~`
    Shl,    // `<<`
    Shr,    // `>>`
    Arrow,  // `>>>` transition operator (v0.4+)

    // Fuzzy
    Question,
    QuestionQuestion,

    // Lock
    Dollar,
    DollarDollar,

    // Layout
    Indent,
    Dedent,
    Newline,
    Eof,
}

impl TokenKind {
    /// 判断两个 token 是否属于同一类别（忽略 String/Ident/Number 内携带的具体值）。
    pub fn same_kind(&self, other: &TokenKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// 若该 token 是关键字，返回其关键字文本（不含 `?`/`??` 后缀）。
    pub fn as_keyword_str(&self) -> Option<&'static str> {
        match self {
            TokenKind::Module => Some("module"),
            TokenKind::Type => Some("type"),
            TokenKind::Rule => Some("rule"),
            TokenKind::Flow => Some("flow"),
            TokenKind::Func => Some("func"),
            TokenKind::Ui => Some("ui"),
            TokenKind::Parallel => Some("parallel"),
            TokenKind::Stack => Some("stack"),
            TokenKind::Binds => Some("binds"),
            TokenKind::Parasteps => Some("parasteps"),
            TokenKind::Requires => Some("requires"),
            TokenKind::Ensures => Some("ensures"),
            TokenKind::Steps => Some("steps"),
            TokenKind::Math => Some("math"),
            TokenKind::If => Some("if"),
            TokenKind::Else => Some("else"),
            TokenKind::For => Some("for"),
            TokenKind::While => Some("while"),
            TokenKind::Desc => Some("desc"),
            TokenKind::On => Some("on"),
            TokenKind::With => Some("with"),
            TokenKind::Error => Some("error"),
            TokenKind::And => Some("and"),
            TokenKind::Or => Some("or"),
            TokenKind::Not => Some("not"),
            TokenKind::In => Some("in"),
            TokenKind::Done => Some("done"),
            TokenKind::Exit => Some("exit"),
            TokenKind::True => Some("true"),
            TokenKind::False => Some("false"),
            TokenKind::Import => Some("import"),
            _ => None,
        }
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TokenKind::Module => "module".into(),
            TokenKind::Type => "type".into(),
            TokenKind::Rule => "rule".into(),
            TokenKind::Flow => "flow".into(),
            TokenKind::Func => "func".into(),
            TokenKind::Ui => "ui".into(),
            TokenKind::Parallel => "parallel".into(),
            TokenKind::Stack => "stack".into(),
            TokenKind::Binds => "binds".into(),
            TokenKind::Parasteps => "parasteps".into(),
            TokenKind::Requires => "requires".into(),
            TokenKind::Ensures => "ensures".into(),
            TokenKind::Steps => "steps".into(),
            TokenKind::Math => "math".into(),
            TokenKind::If => "if".into(),
            TokenKind::Else => "else".into(),
            TokenKind::For => "for".into(),
            TokenKind::While => "while".into(),
            TokenKind::Arrow => ">>>".into(),
            TokenKind::Desc => "desc".into(),
            TokenKind::On => "on".into(),
            TokenKind::With => "with".into(),
            TokenKind::Error => "error".into(),
            TokenKind::And => "and".into(),
            TokenKind::Or => "or".into(),
            TokenKind::Not => "not".into(),
            TokenKind::In => "in".into(),
            TokenKind::Done => "done".into(),
            TokenKind::Exit => "exit".into(),
            TokenKind::True => "true".into(),
            TokenKind::False => "false".into(),
            TokenKind::Import => "@import".into(),
            TokenKind::Ellipsis => "...".into(),
            TokenKind::At => "@".into(),
            TokenKind::Ident(n) => format!("identifier `{}`", n),
            TokenKind::String(_) => "string literal".into(),
            TokenKind::Number(_) => "number".into(),
            TokenKind::Colon => ":".into(),
            TokenKind::Comma => ",".into(),
            TokenKind::Pipe => "|".into(),
            TokenKind::LParen => "(".into(),
            TokenKind::RParen => ")".into(),
            TokenKind::LBracket => "[".into(),
            TokenKind::RBracket => "]".into(),
            TokenKind::Assign => "=".into(),
            TokenKind::Dot => ".".into(),
            TokenKind::EqEq => "==".into(),
            TokenKind::NotEq => "!=".into(),
            TokenKind::Lt => "<".into(),
            TokenKind::Gt => ">".into(),
            TokenKind::Le => "<=".into(),
            TokenKind::Ge => ">=".into(),
            TokenKind::Plus => "+".into(),
            TokenKind::Minus => "-".into(),
            TokenKind::Star => "*".into(),
            TokenKind::Slash => "/".into(),
            TokenKind::Power => "**".into(),
            TokenKind::BitAnd => "&".into(),
            TokenKind::BitXor => "^".into(),
            TokenKind::BitNot => "~".into(),
            TokenKind::Shl => "<<".into(),
            TokenKind::Shr => ">>".into(),
            TokenKind::Question => "?".into(),
            TokenKind::QuestionQuestion => "??".into(),
            TokenKind::Dollar => "$".into(),
            TokenKind::DollarDollar => "$$".into(),
            TokenKind::Indent => "INDENT".into(),
            TokenKind::Dedent => "DEDENT".into(),
            TokenKind::Newline => "NEWLINE".into(),
            TokenKind::Eof => "EOF".into(),
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
    pending: Vec<Token>,
    indent_stack: Vec<usize>,
    at_line_start: bool,
    /// Tracks number of blank lines since the last content line.
    /// Used by the parser to detect blank-line breaks between fragments.
    blank_line_count: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            line: 1,
            col: 1,
            pending: Vec::new(),
            indent_stack: vec![0],
            at_line_start: true,
            blank_line_count: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = matches!(&tok.kind, &TokenKind::Eof);
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
            self.at_line_start = true;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn skip_whitespace_inline(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        if let Some(t) = self.pending.pop() {
            return Ok(t);
        }

        // Layout handling at line start
        if self.at_line_start {
            self.at_line_start = false;
            let start_line = self.line;
            let start_col = self.col;

            // skip blank / comment lines and compute indent of next content line
            loop {
                let spaces = self.count_leading_spaces();
                match self.peek() {
                    None => {
                        return Ok(self.flush_eof());
                    }
                    Some('\n') => {
                        self.bump(); // empty line
                        self.blank_line_count = self.blank_line_count.saturating_add(1).min(100);
                        continue;
                    }
                    Some('/') if self.peek_second() == Some('/') => {
                        self.skip_comment();
                        continue;
                    }
                    _ => {
                        if !spaces.is_multiple_of(4) {
                            return Err(ParseError::indent_error(
                                start_line, start_col,
                                format!("indentation must be a multiple of 4 spaces, found {}", spaces),
                            ));
                        }
                        let current = *self.indent_stack.last().unwrap_or(&0);
                        if spaces > current {
                            self.blank_line_count = 0;
                            self.indent_stack.push(spaces);
                            let mut t = Token {
                                kind: TokenKind::Newline,
                                line: start_line,
                                col: start_col,
                            };
                            self.pending.push(Token {
                                kind: TokenKind::Indent,
                                line: start_line,
                                col: start_col,
                            });
                            // Emit Newline before Indent so parser can consume
                            std::mem::swap(&mut t, self.pending.last_mut().unwrap());
                            self.at_line_start = false;
                            return Ok(t);
                        } else if spaces < current {
                            self.blank_line_count = 0;
                            self.at_line_start = false;
                            return self.emit_dedents(spaces);
                        } else {
                            self.at_line_start = false;
                            let t = Token {
                                kind: TokenKind::Newline,
                                line: start_line,
                                col: start_col,
                            };
                            // Push one extra Newline per blank line encountered
                            for _ in 0..self.blank_line_count {
                                self.pending.push(t.clone());
                            }
                            self.blank_line_count = 0;
                            return Ok(t);
                        }
                    }
                }
            }
        }

        self.skip_whitespace_inline();

        let line = self.line;
        let col = self.col;

        match self.peek() {
            None => Ok(Token {
                kind: TokenKind::Eof,
                line,
                col,
            }),
            Some('\n') => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Newline,
                    line,
                    col,
                })
            }
            Some('/') if self.peek_second() == Some('/') => {
                self.skip_comment();
                self.next_token()
            }
            Some('"') => self.string_token(line, col),
            Some(c) if c.is_ascii_digit() => self.number_token(line, col),
            Some(c) if is_ident_start(c) => self.ident_or_keyword(line, col),
            Some(':') => {
                self.bump();
                Ok(Token::new(TokenKind::Colon, line, col))
            }
            Some(',') => {
                self.bump();
                Ok(Token::new(TokenKind::Comma, line, col))
            }
            Some('|') => {
                self.bump();
                Ok(Token::new(TokenKind::Pipe, line, col))
            }
            Some('&') => {
                self.bump();
                Ok(Token::new(TokenKind::BitAnd, line, col))
            }
            Some('^') => {
                self.bump();
                Ok(Token::new(TokenKind::BitXor, line, col))
            }
            Some('~') => {
                self.bump();
                Ok(Token::new(TokenKind::BitNot, line, col))
            }
            Some('+') => {
                self.bump();
                Ok(Token::new(TokenKind::Plus, line, col))
            }
            Some('-') => {
                self.bump();
                Ok(Token::new(TokenKind::Minus, line, col))
            }
            Some('*') => {
                self.bump();
                if self.peek() == Some('*') {
                    self.bump();
                    Ok(Token::new(TokenKind::Power, line, col))
                } else {
                    Ok(Token::new(TokenKind::Star, line, col))
                }
            }
            Some('/') => {
                self.bump();
                Ok(Token::new(TokenKind::Slash, line, col))
            }
            Some('(') => {
                self.bump();
                Ok(Token::new(TokenKind::LParen, line, col))
            }
            Some(')') => {
                self.bump();
                Ok(Token::new(TokenKind::RParen, line, col))
            }
            Some('[') => {
                self.bump();
                Ok(Token::new(TokenKind::LBracket, line, col))
            }
            Some(']') => {
                self.bump();
                Ok(Token::new(TokenKind::RBracket, line, col))
            }
            Some('=') => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::new(TokenKind::EqEq, line, col))
                } else {
                    Ok(Token::new(TokenKind::Assign, line, col))
                }
            }
            Some('!') => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::new(TokenKind::NotEq, line, col))
                } else {
                    Err(ParseError::unexpected_token(
                    "!".into(),
                    "`!=`".into(),
                    line,
                    col,
                ))
                }
            }
            Some('<') => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::new(TokenKind::Le, line, col))
                } else if self.peek() == Some('<') {
                    self.bump();
                    Ok(Token::new(TokenKind::Shl, line, col))
                } else {
                    Ok(Token::new(TokenKind::Lt, line, col))
                }
            }
            Some('>') => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::new(TokenKind::Ge, line, col))
                } else if self.peek() == Some('>') {
                    self.bump();
                    if self.peek() == Some('>') {
                        self.bump();
                        Ok(Token::new(TokenKind::Arrow, line, col))
                    } else {
                        Ok(Token::new(TokenKind::Shr, line, col))
                    }
                } else {
                    Ok(Token::new(TokenKind::Gt, line, col))
                }
            }
            Some('.') => {
                self.bump();
                if self.peek() == Some('.') {
                    self.bump();
                    if self.peek() == Some('.') {
                        self.bump();
                        Ok(Token::new(TokenKind::Ellipsis, line, col))
                    } else {
                        Err(ParseError::unexpected_token(
                            "..".into(),
                            "`...`".into(),
                            line,
                            col,
                        ))
                    }
                } else {
                    Ok(Token::new(TokenKind::Dot, line, col))
                }
            }
            Some('@') => {
                self.bump();
                let ahead = self.peek_string(7);
                if ahead.starts_with("import")
                    && (ahead.len() == 6 || !is_ident_continue(ahead.chars().nth(6).unwrap()))
                {
                    for _ in 0..6 {
                        self.bump();
                    }
                    Ok(Token::new(TokenKind::Import, line, col))
                } else {
                    Ok(Token::new(TokenKind::At, line, col))
                }
            }
            Some('?') => {
                self.bump();
                if self.peek() == Some('?') {
                    self.bump();
                    Ok(Token::new(TokenKind::QuestionQuestion, line, col))
                } else {
                    Ok(Token::new(TokenKind::Question, line, col))
                }
            }
            Some('$') => {
                self.bump();
                if self.peek() == Some('$') {
                    self.bump();
                    Ok(Token::new(TokenKind::DollarDollar, line, col))
                } else {
                    Ok(Token::new(TokenKind::Dollar, line, col))
                }
            }
            Some(c) => Err(ParseError::unexpected_token(
                c.to_string(),
                "valid token".into(),
                line,
                col,
            )),
        }
    }

    fn count_leading_spaces(&mut self) -> usize {
        let mut count = 0;
        while let Some(c) = self.peek() {
            if c == ' ' {
                count += 1;
                self.bump();
            } else if c == '\t' {
                count += 4;
                self.bump();
            } else {
                break;
            }
        }
        count
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.bump();
        }
    }

    fn peek_second(&mut self) -> Option<char> {
        let mut it = self.chars.clone();
        it.next()?;
        it.next()
    }

    fn peek_string(&self, n: usize) -> String {
        self.chars.clone().take(n).collect()
    }

    fn emit_dedents(&mut self, target: usize) -> Result<Token, ParseError> {
        let current = *self.indent_stack.last().unwrap_or(&0);
        if target > current {
            return Err(ParseError::indent_error(
                self.line, self.col,
                format!("dedent to {} exceeds current indent {}", target, current),
            ));
        }
        // emit DEDENT for each popped level, then a Newline, then continue
        let line = self.line;
        let col = self.col;
        while *self.indent_stack.last().unwrap_or(&0) > target {
            self.indent_stack.pop();
            self.pending.push(Token::new(TokenKind::Dedent, line, col));
        }
        self.pending.push(Token::new(TokenKind::Newline, line, col));
        // return first pending token
        if let Some(t) = self.pending.pop() {
            Ok(t)
        } else {
            self.next_token()
        }
    }

    fn flush_eof(&mut self) -> Token {
        let line = self.line;
        let col = self.col;
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.pending.push(Token::new(TokenKind::Dedent, line, col));
        }
        self.pending.push(Token::new(TokenKind::Eof, line, col));
        self.pending.pop().unwrap_or_else(|| Token::new(TokenKind::Eof, line, col))
    }

    fn string_token(&mut self, line: usize, col: usize) -> Result<Token, ParseError> {
        // consume opening "
        self.bump();
        let mut value = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(ParseError::unterminated_string(line, col));
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\n') => {
                    // 字符串不允许隐式跨行；未闭合的引号应立即报错
                    return Err(ParseError::unterminated_string(line, col));
                }
                Some('\\') => {
                    self.bump();
                    match self.bump() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('r') => value.push('\r'),
                        Some('\\') => value.push('\\'),
                        Some('"') => value.push('"'),
                        Some(c) => {
                            return Err(ParseError::invalid_escape(
                            self.line,
                            self.col,
                            format!("\\{}", c),
                        ))
                        }
                        None => return Err(ParseError::unterminated_string(line, col)),
                    }
                }
                Some(c) => {
                    self.bump();
                    value.push(c);
                }
            }
        }
        Ok(Token::new(TokenKind::String(value), line, col))
    }

    fn number_token(&mut self, line: usize, col: usize) -> Result<Token, ParseError> {
        let mut value = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                value.push(c);
                self.bump();
            } else {
                break;
            }
        }
        // 支持小数：123.45
        if self.peek() == Some('.') && self.peek_second().is_some_and(|c| c.is_ascii_digit()) {
            value.push('.');
            self.bump();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    value.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }
        // 支持科学计数法：1e-4, 1.5e-3, 1E+6
        if matches!(self.peek(), Some('e') | Some('E')) {
            let lookahead = self.peek_string(4);
            let chars: Vec<char> = lookahead.chars().collect();
            let valid_exp = chars.len() >= 2
                && (chars[1].is_ascii_digit() || matches!(chars[1], '+' | '-'))
                && (chars[1].is_ascii_digit()
                    || (matches!(chars[1], '+' | '-')
                        && chars.get(2).is_some_and(|c| c.is_ascii_digit())));
            if valid_exp {
                value.push(self.bump().unwrap()); // e/E
                if matches!(self.peek(), Some('+') | Some('-')) {
                    value.push(self.bump().unwrap());
                }
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        value.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(Token::new(TokenKind::Number(value), line, col))
    }

    fn ident_or_keyword(&mut self, line: usize, col: usize) -> Result<Token, ParseError> {
        let mut value = String::new();
        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                value.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let kind = match value.as_str() {
            "module" => TokenKind::Module,
            "type" => TokenKind::Type,
            "rule" => TokenKind::Rule,
            "flow" => TokenKind::Flow,
            "func" => TokenKind::Func,
            "ui" => TokenKind::Ui,
            "parallel" => TokenKind::Parallel,
            "stack" => TokenKind::Stack,
            "binds" => TokenKind::Binds,
            "parasteps" => TokenKind::Parasteps,
            "math" => TokenKind::Math,
            "requires" => TokenKind::Requires,
            "ensures" => TokenKind::Ensures,
            "steps" => TokenKind::Steps,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "desc" => TokenKind::Desc,
            "on" => TokenKind::On,
            "with" => TokenKind::With,
            "error" => TokenKind::Error,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "in" => TokenKind::In,
            "done" => TokenKind::Done,
            "exit" => TokenKind::Exit,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(value),
        };
        Ok(Token::new(kind, line, col))
    }
}

impl Token {
    fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Self { kind, line, col }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
