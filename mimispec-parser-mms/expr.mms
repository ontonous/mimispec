// FILE: expr.mms
// MimiSpec 解析器实现描述 — Pratt Parser 表达式解析

@import "types.mms"

module ExprParser:

    desc "Pratt parser 实现 表达式条件和 math 块"

    func ParseCondition():
        desc "解析条件 占位符自然语言或结构化"
        steps:
            kw_ellipsis returns placeholder condition
            string literal returns natural condition
            otherwise parse structured expression

    func ParseExpr(minPrec):
        desc "Pratt 主循环 按优先级消费二元运算符"
        steps:
            parse primary expression
            loop check next token for binary op type
            compare operator precedence with minPrec
            consume operator and parse right operand
            build binary expression tree node
            return expression tree

    func ParsePrimary():
        desc "解析一元表达式和基础值"
        steps:
            kw_not parses not expression
            minus parses negation
            tilde parses bitwise not
            ident or keyword parses postfix chain
            string literal returns string expression
            number returns number expression
            kw_true or kw_false returns bool expression
            lparen parses parenthesized expression
            lbracket parses list literal

    func ParsePostfix(base):
        desc "解析后缀 点索引 函数调用 下标"
        steps:
            dot parses field access
            lparen parses function call with args
            lbracket parses subscript with indices
            repeat chain for nested postfix

    rule "operator precedence from 1 low to 10 high"

    func ParseMathBlock():
        desc "解析 math 块"
        steps:
            expect kw_math keyword
            expect colon
            expect indent
            loop parse math statement
            break dedent

    func ParseMathStatement():
        desc "解析 math 块内单条语句"
        steps:
            parse expression
            when assign token detected split target and value expr
            otherwise treat as pure expression
            return math statement struct
