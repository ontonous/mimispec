// FILE: step.mms
// MimiSpec 解析器实现描述 — 步骤解析

@import "types.mms"

module StepParser:

    desc "解析 steps 块内各种步骤类型"
    rule "else 匹配同缩进层级最近的 if"
    rule "on 块附着到前一个步骤"

    func ParseStep():
        desc "步骤分发器 按 peek kind 路由"
        steps:
            match peek kind
            kw_if runs parse if step
            kw_for runs parse singular
            kw_while runs parse while step
            kw_parasteps runs parse parasteps step
            kw_error runs parse error step
            kw_ellipsis runs parse placeholder
            kw_desc runs parse description step
            default runs parse action or assign

    func ParseIfStep():
        desc "解析 if else 条件分支"
        steps:
            expect kw_if keyword
            parse condition
            expect colon
            parse indented then branch
            check kw_else and parse else branch

    func ParseForStep():
        desc "解析 singular 循环"
        steps:
            expect kw_for keyword
            parse fuzzy ident as variable
            expect kw_in keyword
            parse atom sequence as iterable
            expect colon
            parse indented loop body

    func ParseWhileStep():
        desc "解析 while 循环"
        steps:
            expect kw_while keyword
            parse condition
            parse optional description
            expect colon
            parse indented loop body

    func ParseParastepsStep():
        desc "解析并行步骤组"
        steps:
            expect kw_parasteps keyword
            parse optional string label
            expect colon
            parse indented step list

    func ParseErrorStep():
        desc "解析 error 终止步骤"
        steps:
            expect kw_error keyword
            parse fuzzy string as message
            parse optional arrow target via triple greater

    func ParseActionStep():
        desc "解析动作步骤或赋值步骤 含自然语言标签"
        steps:
            parse atom sequence until line end
            scan atom list assignment token
            when assign present split target and value
            parse optional description annotation
            parse optional arrow transition
            parse optional compensation blocks
            note keywords and arrow interfere with steps

    func ParseOnBlock():
        desc "解析 on 补偿和错误处理块"
        steps:
            expect kw_on keyword
            parse condition atom sequence
            expect colon
            parse indented step list
