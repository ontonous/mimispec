use thiserror::Error;

use crate::mimispec::ast::File;

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
