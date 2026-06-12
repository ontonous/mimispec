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
    Parasteps,
    If,
    Else,
    For,
    While,
    To,
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

    // Literals
    Ident(String),
    String(String),
    Number(String),

    // Punctuation / operators
    Colon,
    Comma,
    Pipe,
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
            TokenKind::If => Some("if"),
            TokenKind::Else => Some("else"),
            TokenKind::For => Some("for"),
            TokenKind::While => Some("while"),
            TokenKind::To => Some("to"),
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
            TokenKind::If => "if".into(),
            TokenKind::Else => "else".into(),
            TokenKind::For => "for".into(),
            TokenKind::While => "while".into(),
            TokenKind::To => "to".into(),
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
    /// Tracks whether we have passed a blank line since the last content line.
    /// Used by the parser to detect blank-line breaks between fragments.
    saw_blank_line: bool,
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
            saw_blank_line: false,
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
                        // EOF: flush dedents and emit EOF
                        return self.flush_eof();
                    }
                    Some('\n') => {
                        self.bump(); // empty line
                        self.saw_blank_line = true;
                        continue;
                    }
                    Some('/') if self.peek_second() == Some('/') => {
                        self.skip_comment();
                        continue;
                    }
                    _ => {
                        if spaces % 4 != 0 {
                            return Err(ParseError::IndentError {
                                line: start_line,
                                message: format!(
                                    "indentation must be a multiple of 4 spaces, found {}",
                                    spaces
                                ),
                            });
                        }
                        let current = *self.indent_stack.last().unwrap_or(&0);
                        if spaces > current {
                            self.saw_blank_line = false;
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
                            self.saw_blank_line = false;
                            self.at_line_start = false;
                            return self.emit_dedents(spaces);
                        } else {
                            self.at_line_start = false;
                            let t = Token {
                                kind: TokenKind::Newline,
                                line: start_line,
                                col: start_col,
                            };
                            if self.saw_blank_line {
                                self.saw_blank_line = false;
                                self.pending.push(t.clone());
                            }
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
                    Err(ParseError::UnexpectedToken {
                        found: "!".into(),
                        expected: "`!=`".into(),
                        line,
                        col,
                    })
                }
            }
            Some('<') => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::new(TokenKind::Le, line, col))
                } else {
                    Ok(Token::new(TokenKind::Lt, line, col))
                }
            }
            Some('>') => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::new(TokenKind::Ge, line, col))
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
                        Err(ParseError::UnexpectedToken {
                            found: "..".into(),
                            expected: "`...`".into(),
                            line,
                            col,
                        })
                    }
                } else {
                    Ok(Token::new(TokenKind::Dot, line, col))
                }
            }
            Some('@') => {
                self.bump();
                let mut import_str = String::new();
                for _ in 0..6 {
                    if let Some(c) = self.peek() {
                        import_str.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                if import_str == "import" && !self.peek().map_or(false, is_ident_continue) {
                    Ok(Token::new(TokenKind::Import, line, col))
                } else {
                    Err(ParseError::UnexpectedToken {
                        found: format!("@{}", import_str),
                        expected: "`@import`".into(),
                        line,
                        col,
                    })
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
            Some(c) => Err(ParseError::UnexpectedToken {
                found: c.to_string(),
                expected: "valid token".into(),
                line,
                col,
            }),
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

    fn emit_dedents(&mut self, target: usize) -> Result<Token, ParseError> {
        let current = *self.indent_stack.last().unwrap_or(&0);
        if target > current {
            return Err(ParseError::IndentError {
                line: self.line,
                message: format!("dedent to {} exceeds current indent {}", target, current),
            });
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

    fn flush_eof(&mut self) -> Result<Token, ParseError> {
        let line = self.line;
        let col = self.col;
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.pending.push(Token::new(TokenKind::Dedent, line, col));
        }
        self.pending.push(Token::new(TokenKind::Eof, line, col));
        if let Some(t) = self.pending.pop() {
            Ok(t)
        } else {
            Ok(Token::new(TokenKind::Eof, line, col))
        }
    }

    fn string_token(&mut self, line: usize, col: usize) -> Result<Token, ParseError> {
        // consume opening "
        self.bump();
        let mut value = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(ParseError::UnterminatedString { line, col });
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\n') => {
                    // 字符串不允许隐式跨行；未闭合的引号应立即报错，
                    // 避免吞掉后续大量代码并在远离真实错误的位置报 unexpected token。
                    return Err(ParseError::UnterminatedString { line, col });
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
                            return Err(ParseError::InvalidEscape {
                                line: self.line,
                                col: self.col,
                                message: format!("\\{}", c),
                            })
                        }
                        None => return Err(ParseError::UnterminatedString { line, col }),
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
            "requires" => TokenKind::Requires,
            "ensures" => TokenKind::Ensures,
            "steps" => TokenKind::Steps,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "to" => TokenKind::To,
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
