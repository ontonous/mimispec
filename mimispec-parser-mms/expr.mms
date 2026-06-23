// FILE: expr.mms
// MimiSpec 解析器参考实现 — Pratt Parser 表达式解析

@import "types.mms"

module ExprParser:

    desc "Pratt precedence climbing 表达式解析器, 对应 src/lib/parser/expr.rs"

    rule "运算符优先级 1 最低到 12 最高"
    rule "or 1, and 2, in 和比较 3, 位运算 4 5 6, 移位 7"
    rule "加减 8, 乘除 9, 幂 10 右结合, 一元 11, 基础值 12"

    func ParseCondition():
        desc "解析条件: 占位符自然语言或结构化表达式"
        steps:
            desc "kw_ellipsis -> placeholder condition"
            desc "string literal -> natural condition"
            desc "otherwise -> structured via parse_expr zero"

    func ParseExpr(minPrec):
        desc "Pratt 主循环, 按优先级消费二元运算符"
        steps:
            desc "parse lhs via parse_primary"
            desc "lookup binop from peek, break if none or low prec"
            desc "consume operator, parse rhs at correct precedence"
            desc "build binary expression node"
            return lhs

    func ParsePrimary():
        desc "解析一元表达式和基础值"
        steps:
            desc "kw_not -> not expr, minus -> neg, tilde -> bitnot"
            desc "ident -> ident plus postfix chain"
            desc "string -> string expr"
            desc "number -> number expr"
            desc "kw_true kw_false -> bool expr"
            desc "lparen -> parenthesized expr"
            desc "lbracket -> list literal"

    func ParsePostfix(base):
        desc "解析后缀: dot field, func call, bracket subscript"
        steps:
            desc "dot -> field access, build Index"
            desc "lparen -> call args, build Call"
            desc "lbracket -> multi-dim indices, build Subscript"
            desc "otherwise -> break"

    func ParseMathBlock():
        desc "解析 math 块, 含语句列表"
        steps:
            expect kw_math plus commitment
            expect colon
            parse indented block of math statements
            return MathBlock

    func ParseMathStatement():
        desc "解析 math 块内单条语句"
        steps:
            parse expression
            desc "check assign after expr: define or pure expr"
            desc "validate line_will_end else trailing token error"
            return MathStatement
