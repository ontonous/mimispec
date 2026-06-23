// FILE: index.mms
// MimiSpec 解析器参考实现 — 主入口和公共 API

@import "types.mms"
@import "lexer.mms"
@import "parser-core.mms"
@import "fragment.mms"
@import "step.mms"
@import "flow.mms"
@import "expr.mms"
@import "ui.mms"

module MimispecParser:

    desc "MimiSpec .mms 解析器参考实现，对应 src/lib/mod.rs"

    rule "解析器总是尽可能解析，不因局部错误丢弃整体 AST"

    type ParseResult:
        result_file: File
        errors: ParseError

    func Parse(source):
        desc "解析完整 mms 字符串，返回 AST 和错误列表"
        steps:
            create lexer from source
            tokenize to token list
            desc "on lex error return empty file with error"
            create parser from tokens
            parse file
            return ParseResult

    func ParseFragment(source):
        desc "解析单个 Fragment，IDE 片段验证用"
        steps:
            create lexer from source
            tokenize to token list
            desc "on lex error return empty result"
            create parser from tokens
            skip leading newlines
            parse single fragment
            return ParseResult

    func Tokenize(source):
        desc "仅词法分析，返回 token 列表"
        steps:
            create lexer from source
            call tokenize method
            return token list or error

    func EditDistance(a, b):
        desc "计算 Levenshtein 编辑距离，O(m·n) 时间 O(n) 空间"

    func KnownNamesFromTokens(tokens):
        desc "收集 token 中所有标识符、字符串、数字名称用于模糊匹配"

    func FindSuggestion(target, known, maxDist):
        desc "在已知名称列表中查找编辑距离最小的候选项"
