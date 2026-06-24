use std::path::PathBuf;

use crate::ast::File;

/// Cross-file resolution error.
#[derive(Debug, Clone)]
pub enum ResolveError {
    /// Circular import detected.
    ImportCycle { chain: Vec<PathBuf> },
    /// File not found on disk.
    FileNotFound { path: PathBuf },
    /// File could not be read.
    IoError { path: PathBuf, message: String },
    /// File was parsed but with errors; partial AST available.
    ParseFailed {
        path: PathBuf,
        errors: Vec<ParseError>,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::ImportCycle { chain } => {
                let paths: Vec<_> = chain.iter().map(|p| p.display().to_string()).collect();
                write!(f, "import cycle detected: {}", paths.join(" → "))
            }
            ResolveError::FileNotFound { path } => {
                write!(f, "file not found: {}", path.display())
            }
            ResolveError::IoError { path, message } => {
                write!(f, "I/O error reading {}: {}", path.display(), message)
            }
            ResolveError::ParseFailed { path, errors } => {
                write!(
                    f,
                    "parse errors in {}: {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
        }
    }
}

/// Structured error code for categorizing and deduplicating parse errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // ── Lexer ──────────────────────────────────────────────────────────────
    /// Unexpected character (illegal token).
    E0001,
    /// Unexpected end of file.
    E0002,
    /// Indentation error (not a multiple of 4, or bad dedent).
    E0003,
    /// Invalid escape sequence in string literal.
    E0004,
    /// Unterminated string literal.
    E0005,
    // ── Parser: structure ──────────────────────────────────────────────────
    /// Token does not match what was expected (generic parser-level).
    E0010,
    /// Identifier is not defined (with optional suggestion).
    E0011,
    /// Binary operator not supported for the given operand types.
    E0012,
    /// Expression form is not supported in this context.
    E0013,
    /// Value is not callable (not a function).
    E0014,
    /// Subscript index is out of bounds.
    E0015,
    /// Operand types do not match for the operator.
    E0016,
    /// Expected an indented block after `:`.
    E0017,
    /// Expected a function body (`steps:` or `...`).
    E0018,
    /// Internal / unrecoverable parser state.
    E0701,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::E0001 => write!(f, "E0001"),
            ErrorCode::E0002 => write!(f, "E0002"),
            ErrorCode::E0003 => write!(f, "E0003"),
            ErrorCode::E0004 => write!(f, "E0004"),
            ErrorCode::E0005 => write!(f, "E0005"),
            ErrorCode::E0010 => write!(f, "E0010"),
            ErrorCode::E0011 => write!(f, "E0011"),
            ErrorCode::E0012 => write!(f, "E0012"),
            ErrorCode::E0013 => write!(f, "E0013"),
            ErrorCode::E0014 => write!(f, "E0014"),
            ErrorCode::E0015 => write!(f, "E0015"),
            ErrorCode::E0016 => write!(f, "E0016"),
            ErrorCode::E0017 => write!(f, "E0017"),
            ErrorCode::E0018 => write!(f, "E0018"),
            ErrorCode::E0701 => write!(f, "E0701"),
        }
    }
}

/// The result of parsing a MimiSpec source string.
///
/// Contains both the partially-parsed [`File`] AST and any errors encountered.
/// The AST is always as complete as possible — errors indicate specific locations
/// where parsing failed, but the parser continues recovering after each error.
///
/// [`File`]: crate::ast::File
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file: File,
    pub errors: Vec<ParseError>,
}

/// A single parse error with structured diagnostic information.
///
/// Each error includes:
/// - A unique [`ErrorCode`] for categorization
/// - Source location (1-indexed line and column)
/// - A human-readable message
/// - Optional `help` text with guidance on fixing the error
/// - Optional `suggestion` (e.g. "did you mean 'FOO'?")
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub code: ErrorCode,
    pub line: usize,
    pub col: usize,
    pub message: String,
    /// Optional hint for how to fix the error.
    pub help: Option<String>,
    /// Optional suggested replacement (e.g. "did you mean 'FOO'?").
    pub suggestion: Option<String>,
}

impl ParseError {
    /// Creates an error for an unexpected token.
    ///
    /// # Arguments
    ///
    /// * `found` — The actual token found (as a display string).
    /// * `expected` — A description of what was expected.
    /// * `line` — 1-indexed source line.
    /// * `col` — 1-indexed source column.
    pub fn unexpected_token(found: String, expected: String, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0010,
            line,
            col,
            message: format!("unexpected token {found:?} at line {line}, col {col}; expected {expected}"),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an error for an unexpected end of file.
    pub fn unexpected_eof(line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0002,
            line,
            col,
            message: "unexpected end of file".into(),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an indentation error (not a multiple of 4 spaces, or bad dedent).
    pub fn indent_error(line: usize, col: usize, message: String) -> Self {
        Self {
            code: ErrorCode::E0003,
            line,
            col,
            message: format!("indentation error at line {line}: {message}"),
            help: Some("indentation must be a multiple of 4 spaces".into()),
            suggestion: None,
        }
    }

    /// Creates an error for an invalid escape sequence in a string literal.
    pub fn invalid_escape(line: usize, col: usize, message: String) -> Self {
        Self {
            code: ErrorCode::E0004,
            line,
            col,
            message: format!("invalid escape at line {line}, col {col}: {message}"),
            help: Some("valid escapes are: \\n, \\t, \\r, \\\\, \\\"".into()),
            suggestion: None,
        }
    }

    /// Creates an error for an unterminated string literal.
    pub fn unterminated_string(line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0005,
            line,
            col,
            message: format!("unterminated string at line {line}, col {col}"),
            help: Some("string literals must be closed with a matching double-quote".into()),
            suggestion: None,
        }
    }

    /// Creates an error for an undefined variable, with an optional "did you mean" suggestion.
    pub fn undefined_variable(name: String, line: usize, col: usize, suggestion: Option<String>) -> Self {
        let msg = if let Some(ref s) = suggestion {
            format!("undefined variable '{name}' — did you mean '{s}'?")
        } else {
            format!("undefined variable '{name}'")
        };
        Self {
            code: ErrorCode::E0011,
            line,
            col,
            message: msg,
            help: Some("check the variable name for typos".into()),
            suggestion,
        }
    }

    /// Creates an error for an unsupported binary operator application.
    pub fn unsupported_bin_op(op: &str, left: &str, right: &str, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0012,
            line,
            col,
            message: format!("cannot apply '{op}' to {left} and {right}"),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an error for an expression form that is not supported in the current context.
    pub fn unsupported_expr(desc: &str, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0013,
            line,
            col,
            message: format!("unsupported expression: {desc}"),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an error for attempting to call a non-callable value.
    pub fn not_callable(value_desc: &str, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0014,
            line,
            col,
            message: format!("cannot call {value_desc}: value is not callable"),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an error for a subscript index that is out of bounds.
    pub fn index_out_of_bounds(index: usize, len: usize, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0015,
            line,
            col,
            message: format!("index out of bounds: index {index} is not valid for list of length {len}"),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an error for a type mismatch between expected and actual operand types.
    pub fn type_mismatch(expected: &str, got: &str, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0016,
            line,
            col,
            message: format!("type mismatch: expected {expected}, got {got}"),
            help: None,
            suggestion: None,
        }
    }

    /// Creates an error for a missing indented block after `:`.
    pub fn missing_indented_block(line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0017,
            line,
            col,
            message: "expected an indented block after ':'".into(),
            help: Some("indent the body by 4 spaces relative to the header".into()),
            suggestion: None,
        }
    }

    /// Creates an error for a function definition missing its body (`steps:` or `...`).
    pub fn missing_func_body(line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0018,
            line,
            col,
            message: "expected function body: 'steps:' or '...'".into(),
            help: Some("add a 'steps:' block or write '...' as a placeholder".into()),
            suggestion: None,
        }
    }

    /// Creates an error for an internal / unrecoverable parser state.
    ///
    /// These indicate a bug in the parser itself and should be reported.
    pub fn internal(msg: String, line: usize, col: usize) -> Self {
        Self {
            code: ErrorCode::E0701,
            line,
            col,
            message: format!("internal error: {msg}"),
            help: Some("this is a bug — please report it".into()),
            suggestion: None,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}] at line {}, col {}: {}", self.code, self.line, self.col, self.message)?;
        if let Some(ref help) = self.help {
            write!(f, "\n  help: {help}")?;
        }
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  suggestion: {suggestion}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}
