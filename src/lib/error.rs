use thiserror::Error;

use crate::ast::File;

/// 解析结果：即使出错也返回尽可能完整的 AST + 所有错误
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file: File,
    pub errors: Vec<ParseError>,
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("unexpected token {found:?} at line {line}, col {col}; expected {expected}")]
    UnexpectedToken {
        found: String,
        expected: String,
        line: usize,
        col: usize,
    },
    #[error("indentation error at line {line}: {message}")]
    IndentError { line: usize, message: String },
    #[error("invalid escape at line {line}, col {col}: {message}")]
    InvalidEscape {
        line: usize,
        col: usize,
        message: String,
    },
    #[error("unterminated string at line {line}, col {col}")]
    UnterminatedString { line: usize, col: usize },
}

impl ParseError {
    pub fn line(&self) -> usize {
        match self {
            ParseError::UnexpectedEof => 0,
            ParseError::UnexpectedToken { line, .. } => *line,
            ParseError::IndentError { line, .. } => *line,
            ParseError::InvalidEscape { line, .. } => *line,
            ParseError::UnterminatedString { line, .. } => *line,
        }
    }

    pub fn col(&self) -> usize {
        match self {
            ParseError::UnexpectedEof => 0,
            ParseError::UnexpectedToken { col, .. } => *col,
            ParseError::IndentError { .. } => 0,
            ParseError::InvalidEscape { col, .. } => *col,
            ParseError::UnterminatedString { col, .. } => *col,
        }
    }
}
