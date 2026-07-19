//! Lightweight LaTeX renderer for MimiSpec math expressions.
//!
//! Converts `Expr` and `MathBlock` AST nodes into LaTeX math strings suitable
//! for rendering with MathJax, KaTeX, or similar libraries.
//!
//! Design principles:
//! - Pure LaTeX output — no external rendering dependencies.
//! - Preserves operator precedence with parentheses.
//! - Subscripts render as `x_{i,j}`, division as `\frac{a}{b}`.

use crate::ast::*;
use crate::render_util::{expr_prec, paren_if};

/// Render a single expression as a LaTeX math string.
///
/// # Example
///
/// ```rust
/// use mimispec::parse;
/// use mimispec::latex::render_expr;
///
/// let result = parse("a / b");
/// // result.file.fragments[0] is Fragment::Expr { expr }
/// // render_expr(&expr) == "\\frac{a}{b}"
/// ```
pub fn render_expr(expr: &Expr) -> String {
    render_expr_prec(expr, 0)
}

/// Render a single math statement (define or expression) as LaTeX.
pub fn render_math_statement(stmt: &MathStatement) -> String {
    match stmt {
        MathStatement::Define { target, value } => {
            format!("{} = {}", render_expr(target), render_expr(value))
        }
        MathStatement::Expr { expr } => render_expr(expr),
    }
}

/// Render an entire math block as multi-line LaTeX (statements separated by `\\`).
pub fn render_math_block(math: &MathBlock) -> String {
    math.statements
        .iter()
        .map(render_math_statement)
        .collect::<Vec<_>>()
        .join(" \\\\\n")
}

/// Collect and render all math blocks across an entire file's fragments as LaTeX.
///
/// Recursively traverses `Module` fragments to find all nested `math:` blocks.
/// Each math block's statements are joined with `\\`, and blocks are separated
/// by `\\` as well.
pub fn render_file_latex(file: &File) -> String {
    let mut blocks = Vec::new();
    collect_latex_fragments(&file.fragments, &mut blocks);
    blocks.join(" \\\\\n")
}

fn collect_latex_fragments(fragments: &[Fragment], out: &mut Vec<String>) {
    for fragment in fragments {
        match fragment {
            Fragment::Math { math } => out.push(render_math_block(math)),
            Fragment::Module { module } => {
                collect_latex_fragments(&module.items, out);
            }
            Fragment::TypeDef { typedef } => {
                collect_latex_fragments(typedef.items(), out);
            }
            Fragment::Func { func } => {
                collect_latex_fragments(&func.items, out);
            }
            Fragment::Flow { flow } => collect_latex_fragments(&flow.items, out),
            Fragment::FlowEntry { entry } => collect_latex_fragments(&entry.items, out),
            Fragment::FlowArm { arm } => collect_latex_fragments(&arm.items, out),
            Fragment::Steps { items, .. } => collect_latex_fragments(items, out),
            _ => {}
        }
    }
}

fn render_expr_prec(expr: &Expr, parent_prec: u8) -> String {
    let my_prec = expr_prec(expr);
    let s = match expr {
        Expr::Ident { value } => render_ident(value),
        Expr::String { value } => render_fstring(value),
        Expr::Number { value } => value.clone(),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::List { items } => {
            let inner = items.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("\\left[{}\\right]", inner)
        }
        Expr::Placeholder { .. } => "\\ldots".into(),

        Expr::Not { expr, .. } => format!("\\neg {}", render_expr_prec(expr, my_prec)),
        Expr::Neg { expr, .. } => format!("-{}", render_expr_prec(expr, my_prec)),
        Expr::BitNot { expr, .. } => format!("\\sim {}", render_expr_prec(expr, my_prec)),

        Expr::And { left, right, .. } => format!(
            "{} \\land {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Or { left, right, .. } => format!(
            "{} \\lor {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::In { left, right, .. } => format!(
            "{} \\in {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Compare {
            left, op, right, ..
        } => {
            let op_s = match op {
                CompareOp::Eq => "=",
                CompareOp::Ne => "\\neq",
                CompareOp::Lt => "<",
                CompareOp::Gt => ">",
                CompareOp::Le => "\\leq",
                CompareOp::Ge => "\\geq",
            };
            format!(
                "{} {} {}",
                render_expr_prec(left, my_prec),
                op_s,
                render_expr_prec(right, my_prec)
            )
        }

        Expr::BitAnd { left, right } => format!(
            "{} \\land {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::BitOr { left, right } => format!(
            "{} \\lor {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::BitXor { left, right } => format!(
            "{} \\oplus {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Shl { left, right } => format!(
            "{} \\ll {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Shr { left, right } => format!(
            "{} \\gg {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),

        Expr::Add { left, right } => format!(
            "{} + {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Sub { left, right } => format!(
            "{} - {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Mul { left, right } => format!(
            "{} \\cdot {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Div { left, right } => {
            // \frac 自带分组，对子表达式使用顶层优先级渲染。
            format!(
                "\\frac{{{}}}{{{}}}",
                render_expr_prec(left, 0),
                render_expr_prec(right, 0)
            )
        }
        Expr::Pow { left, right } => format!(
            "{}^{{{}}}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, 0)
        ),
        Expr::MatMul { left, right } => format!(
            "{} \\times {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),

        Expr::Index { object, field } => {
            format!("{}.{}", render_expr_prec(object, 12), render_ident(field))
        }
        Expr::Subscript { object, indices } => {
            let inner = indices
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}_{{{}}}", render_expr_prec(object, 12), inner)
        }
        Expr::Call { callee, args } => {
            let inner = args.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", render_expr_prec(callee, 12), inner)
        }
    };
    paren_if(my_prec < parent_prec, s)
}

fn render_ident(ident: &Ident) -> String {
    // LaTeX 中把下划线转义，避免被当作下标标记；commitment 后缀不渲染。
    escape_latex(&ident.name)
}

fn render_fstring(s: &FString) -> String {
    format!("\\text{{{}}}", escape_latex(&s.value))
}

fn escape_latex(s: &str) -> String {
    // 单遍字符扫描，避免链式 replace 的顺序依赖和二次转义问题
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' => out.push_str("\\&"),
            '%' => out.push_str("\\%"),
            '$' => out.push_str("\\$"),
            '#' => out.push_str("\\#"),
            '_' => out.push_str("\\_"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn latex(src: &str) -> String {
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Expr { expr } = &result.file.fragments[0] else {
            panic!("expected expr fragment")
        };
        render_expr(expr)
    }

    #[test]
    fn latex_subscript() {
        assert_eq!(latex("x[i]"), "x_{i}");
        assert_eq!(latex("x[i, j]"), "x_{i, j}");
        assert_eq!(latex("tensor[-1, -2]"), "tensor_{-1, -2}");
    }

    #[test]
    fn latex_fraction_and_power() {
        assert_eq!(latex("a / b"), "\\frac{a}{b}");
        assert_eq!(latex("a ** b"), "a^{b}");
        assert_eq!(latex("(a + b) ** 2"), "(a + b)^{2}");
    }

    #[test]
    fn latex_matrix_ops() {
        assert_eq!(latex("A @ B"), "A \\times B");
        assert_eq!(latex("A * B"), "A \\cdot B");
    }

    #[test]
    fn latex_comparison_logic() {
        assert_eq!(latex("x == y"), "x = y");
        assert_eq!(latex("x <= y"), "x \\leq y");
        assert_eq!(latex("a and b"), "a \\land b");
        assert_eq!(latex("a or b"), "a \\lor b");
        assert_eq!(latex("not a"), "\\neg a");
    }

    #[test]
    fn latex_parentheses() {
        assert_eq!(latex("(a + b) * c"), "(a + b) \\cdot c");
        assert_eq!(latex("a * (b + c)"), "a \\cdot (b + c)");
    }

    #[test]
    fn latex_math_block() {
        let src = r#"
module M:
    func F():
        math:
            total_loss = mlm_loss + nsp_loss
            y = (a + b) / c
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Func { func } = &module.items[0] else {
            panic!("expected func")
        };
        let math = func.math_blocks()[0];
        let latex = render_math_block(math);
        assert!(latex.contains("\\frac{a + b}{c}"));
        assert!(latex.contains("mlm\\_loss"));
    }
}
