//! Structured diagnostic formatting for CLI output.
//!
//! Provides [`format_diagnostic()`] which renders a [`crate::error::ParseError`]
//! together with the relevant source line and a caret underline pointing at
//! the error location.

use crate::error::ParseError;

/// Format a parse error with source-context display for CLI output.
///
/// Renders the error with the relevant source line and a caret underline
/// pointing at the error location.
///
/// # Format
///
/// ```text
/// error[E0010] at line 3, col 12: unexpected token 'foo'; expected 'bar'
///   help: did you mean 'baz'?
///    3 │ some preamble foo bar
///                  ^^^
/// ```
pub fn format_diagnostic(err: &ParseError, source: &str) -> String {
    let mut buf = String::new();

    // Error header
    buf.push_str(&format!(
        "error[{}] at line {}, col {}: {}\n",
        err.code, err.line, err.col, err.message
    ));

    // Help line
    if let Some(ref help) = err.help {
        buf.push_str("  help: ");
        buf.push_str(help);
        buf.push('\n');
    }

    // Suggestion line
    if let Some(ref suggestion) = err.suggestion {
        buf.push_str("  suggestion: ");
        buf.push_str(suggestion);
        buf.push('\n');
    }

    // Source context
    let (src_line, underline_col) = source_line_at(source, err.line, err.col);
    if !src_line.is_empty() {
        buf.push_str(&format!("  {:>4} │ {}\n", err.line, src_line));
        // Build underline: spaces up to (underline_col - 1), then ^
        // col is 1-indexed
        let pad = underline_col.saturating_sub(1);
        buf.push_str("        │ ");
        for _ in 0..pad {
            buf.push(' ');
        }
        buf.push_str("^\n");
    }

    buf
}

/// Return the source line (1-indexed) and the effective column for underlining.
/// Falls back gracefully if the line or column is out of range.
fn source_line_at(source: &str, line: usize, col: usize) -> (String, usize) {
    if source.is_empty() || line == 0 {
        return (String::new(), col);
    }

    // Precompute line start offsets for O(1) lookup
    let mut line_starts = Vec::new();
    line_starts.push(0);
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            line_starts.push(idx + 1);
        }
    }

    if line > line_starts.len() {
        return (String::new(), col);
    }

    let line_start = line_starts[line - 1];
    let rest = &source[line_start..];
    let line_end = rest.find('\n').map(|n| line_start + n).unwrap_or(source.len());
    let src_line = &source[line_start..line_end];
    // Clamp col to line length (1-indexed)
    let effective_col = col.min(src_line.chars().count()).max(1);
    (src_line.to_string(), effective_col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, ParseError};

    fn make_err() -> ParseError {
        ParseError {
            code: ErrorCode::E0010,
            line: 2,
            col: 10,
            message: "unexpected token 'x'; expected 'bar'".into(),
            help: None,
            suggestion: None,
        }
    }

    #[test]
    fn format_diagnostic_basic() {
        let src = "func Foo():\n    steps:\n        x = =\n";
        let err = make_err();
        let out = format_diagnostic(&err, src);
        assert!(out.contains("error[E0010]"));
        assert!(out.contains("at line 2, col 10"));
        assert!(out.contains("2 │"));
    }

    #[test]
    fn format_diagnostic_with_help() {
        let mut err = make_err();
        err.help = Some("use a single '=' for assignment".into());
        let src = "func Foo():\n    steps:\n        x = =\n";
        let out = format_diagnostic(&err, src);
        assert!(out.contains("help: use a single '=' for assignment"));
    }

    #[test]
    fn format_diagnostic_with_suggestion() {
        let mut err = make_err();
        err.suggestion = Some("bar".into());
        let src = "func Foo():\n    steps:\n        x = =\n";
        let out = format_diagnostic(&err, src);
        assert!(out.contains("suggestion: bar"));
    }

    #[test]
    fn format_diagnostic_empty_source() {
        let err = make_err();
        let out = format_diagnostic(&err, "");
        assert!(out.contains("error[E0010]"));
        assert!(!out.contains("│"));
    }

    #[test]
    fn format_diagnostic_col_clamped() {
        // col > line length should clamp to line length
        let err = ParseError {
            code: ErrorCode::E0010,
            line: 1,
            col: 999,
            message: "test".into(),
            help: None,
            suggestion: None,
        };
        let src = "abc\n";
        let out = format_diagnostic(&err, src);
        assert!(out.contains("1 │ abc"));
    }
}
