//! 轻量 LaTeX 渲染器
//!
//! 将 MimiSpec 的 `Expr` / `MathBlock` 转换为 LaTeX 数学字符串，
//! 供前端通过 MathJax / KaTeX 等库进行图形化渲染。
//!
//! 设计原则：
//! - 只生成纯 LaTeX 源码，不引入外部渲染依赖。
//! - 保留运算符优先级与必要括号。
//! - 对 `x[i, j]` 等下标表达式渲染为 `x_{i,j}`，对 `a / b` 渲染为 `\frac{a}{b}`。

use crate::ast::*;
use crate::render_util::{expr_prec, paren_if};

/// 渲染单个表达式为 LaTeX。
pub fn render_expr(expr: &Expr) -> String {
    render_expr_prec(expr, 0)
}

/// 渲染 math 块内的一条语句。
pub fn render_math_statement(stmt: &MathStatement) -> String {
    match stmt {
        MathStatement::Define { target, value } => {
            format!("{} = {}", render_expr(target), render_expr(value))
        }
        MathStatement::Expr { expr } => render_expr(expr),
    }
}

/// 渲染整个 math 块为多行 LaTeX（语句间用 `\\` 分隔）。
pub fn render_math_block(math: &MathBlock) -> String {
    math.statements
        .iter()
        .map(render_math_statement)
        .collect::<Vec<_>>()
        .join(" \\\\\n")
}

/// 渲染文件中所有 math 块为轻量 LaTeX 字符串。
pub fn render_file_latex(file: &File) -> String {
    let mut blocks = Vec::new();
    collect_latex_fragments(&file.fragments, &mut blocks);
    blocks.join(" \\\\\n")
}

fn collect_latex_fragments(fragments: &[Fragment], out: &mut Vec<String>) {
    for fragment in fragments {
        match fragment {
            Fragment::Module { module } => {
                if let Some(math) = &module.math {
                    out.push(render_math_block(math));
                }
                collect_latex_fragments(&module.items, out);
            }
            Fragment::TypeDef { typedef } => {
                if let Some(math) = &typedef.math {
                    out.push(render_math_block(math));
                }
            }
            Fragment::Func { func } => {
                if let Some(math) = &func.math {
                    out.push(render_math_block(math));
                }
            }
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
        Expr::Neg { expr } => format!("-{}", render_expr_prec(expr, my_prec)),
        Expr::BitNot { expr } => format!("\\sim {}", render_expr_prec(expr, my_prec)),

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
        Expr::Compare { left, op, right } => {
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
    s.replace('\\', "\\textbackslash{}")
        .replace('&', "\\&")
        .replace('%', "\\%")
        .replace('$', "\\$")
        .replace('#', "\\#")
        .replace('_', "\\_")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('~', "\\textasciitilde{}")
        .replace('^', "\\textasciicircum{}")
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
        let math = func.math.as_ref().unwrap();
        let latex = render_math_block(math);
        assert!(latex.contains("\\frac{a + b}{c}"));
        assert!(latex.contains("mlm\\_loss"));
    }
}
