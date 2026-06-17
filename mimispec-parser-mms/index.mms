// FILE: index.mms
// MimiSpec 解析器实现描述 — 主入口

@import "types.mms"
@import "lexer.mms"
@import "parser-core.mms"
@import "fragment.mms"
@import "step.mms"
@import "flow.mms"
@import "expr.mms"
@import "ui.mms"

module MimispecParser:

    desc "MimiSpec (.mms) 解析器参考实现"

    rule "ParseResult 总是包含部分 AST 即使有错误"

    func Parse(source):
        desc "解析 MimiSpec 源字符串"
        steps:
            create lexer from source
            tokenize to token list
            create parser from tokens
            parse file
            return ParseResult

    func Tokenize(source):
        desc "词法分析入口"
        steps:
            create lexer from source
            call tokenize method
            return token list or error

    type ParseResult:
        file: File
        errors: ParseError
