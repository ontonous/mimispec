//! 渲染器共享工具
//!
//! `render` 与 `latex` 模块都需要表达式优先级和括号处理逻辑，
//! 集中在此处避免重复。

use crate::ast::Expr;

/// 返回表达式节点的优先级（数值越小优先级越低）。
pub fn expr_prec(expr: &Expr) -> u8 {
    match expr {
        Expr::Or { .. } => 1,
        Expr::And { .. } => 2,
        Expr::In { .. } | Expr::Compare { .. } => 3,
        Expr::BitOr { .. } => 4,
        Expr::BitXor { .. } => 5,
        Expr::BitAnd { .. } => 6,
        Expr::Shl { .. } | Expr::Shr { .. } => 7,
        Expr::Add { .. } | Expr::Sub { .. } => 8,
        Expr::Mul { .. } | Expr::Div { .. } | Expr::MatMul { .. } => 9,
        Expr::Pow { .. } => 10,
        Expr::Not { .. } | Expr::Neg { .. } | Expr::BitNot { .. } => 11,
        _ => 12,
    }
}

/// 根据是否需要括号包裹子表达式。
pub fn paren_if(needed: bool, s: String) -> String {
    if needed {
        format!("({})", s)
    } else {
        s
    }
}
