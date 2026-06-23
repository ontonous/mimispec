// FILE: step.mms
// MimiSpec 解析器参考实现 — 步骤 step 解析

@import "types.mms"

module StepParser:

    desc "解析 steps 块内各种步骤类型, 对应 src/lib/parser/step.rs"

    rule "else 匹配同缩进层级最近的 if"
    rule "on 块附着到前一个步骤, 支持多个 on 分支"
    rule "步骤标签中的关键字冲突可用双引号转义"

    func ParseStep():
        desc "按 peek kind 路由到对应步骤解析器"
        steps:
            desc "kw_if -> parse if step"
            desc "kw_for -> parse for step"
            desc "kw_while -> parse while step"
            desc "kw_parasteps -> parse parasteps step"
            desc "kw_error -> parse error step"
            desc "kw_ellipsis -> parse placeholder step"
            desc "kw_desc -> parse desc step"
            desc "other -> parse action or assign step"

    func ParseIfStep():
        desc "解析 if else 条件分支"
        steps:
            expect kw_if plus commitment
            parse condition
            expect colon
            parse indented then branch
            desc "check kw_else then parse else branch"
            return IfStep

    func ParseForStep():
        desc "解析 for 循环"
        steps:
            expect kw_for plus commitment
            parse fuzzy ident as loop variable
            expect kw_in
            desc "parse iterable atoms until colon"
            expect colon
            parse indented loop body
            return ForStep

    func ParseWhileStep():
        desc "解析 while 循环"
        steps:
            expect kw_while plus commitment
            parse condition
            desc "parse optional desc annotation"
            expect colon
            parse indented loop body
            return WhileStep

    func ParseParastepsStep():
        desc "解析并行步骤组, 全部完成后继续"
        steps:
            expect kw_parasteps plus commitment
            desc "parse optional string label"
            expect colon
            parse indented step list
            return ParastepsStep

    func ParseErrorStep():
        desc "解析 error 终止步骤"
        steps:
            expect kw_error plus commitment
            parse fuzzy string as message
            desc "parse optional arrow target"
            return ErrorStep

    func ParseActionStep():
        desc "解析动作步骤或赋值步骤"
        steps:
            desc "parse atom sequence until desc/arrow/on/newline"
            desc "scan atoms for equals sign"
            desc "if equals found: parse assign step"
            desc "otherwise: parse action step"
            desc "both paths also parse desc arrow on blocks"

    func ParsePlaceholderStep():
        desc "解析 ellipsis 占位步骤"
        steps:
            expect kw_ellipsis plus commitment
            return Placeholder step

    func ParseDescStep():
        desc "解析 desc 作为独立步骤实体"
        steps:
            call parse_desc_entity
            return Desc step

    func ParseOnBlock():
        desc "解析 on 补偿或错误处理块"
        steps:
            expect kw_on plus commitment
            desc "parse condition atoms until colon"
            expect colon
            parse indented step list
            return OnBlock
