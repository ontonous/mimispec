// FILE: parser-core.mms
// MimiSpec 解析器实现描述 — Parser 核心

@import "types.mms"

module ParserCore:

    desc "Parser 核心结构 token 导航 rule 管理和错误恢复"

    func Peek():
        desc "查看当前 token 但不消费"

    func Advance():
        desc "消费当前 token 并前进"

    func Check(kind):
        desc "检查当前 token 是否匹配给定类型"

    func Matches(kind):
        desc "若匹配则消费返回 true"

    func Expect(kind, what):
        desc "期望当前 token 指定类型否则报错"

    func Commitment():
        desc "解析意图后缀"
        steps:
            check dollardollar suffix
            check dollar suffix
            check questionquestion suffix
            check question suffix
            return none absent

    func FuzzyIdent():
        desc "消费标识符或关键字加 commitment"

    func FuzzyString():
        desc "消费字符串字面量加 commitment"

    func ParseDescAfterKeyword():
        desc "解析关键字后面的 desc 实体"
        steps:
            parse commitment suffix
            parse fuzzy string content
            return Desc struct

    func ConsumePendingRules():
        desc "收集连续 rule 定义到 pending 列表"
        steps:
            scan ahead counting newlines
            break "if" 3 or more blank lines separate
            break "if" next token is not rule keyword
            parse rule def and push to pending stack

    func AttachRulesToFragment(fragment):
        desc "将 pending rules 附着到最近 fragment"
        steps:
            match fragment kind
            module gets module rules
            typedef gets typedef rules
            flow gets flow rules
            func gets func rules

    func SynchronizeToFragmentStart():
        desc "错误恢复 同步到下一个 fragment 开头"
        steps:
            advance over unexpected tokens
            stop at any fragment start keyword

    func SynchronizePastNestedBlock():
        desc "跳过嵌套块直到返回上层"
        steps:
            track indent depth
            advance over indent inc depth
            advance over dedent dec depth
            return when depth reaches zero

    func ParseBlock(parseItem):
        desc "解析缩进块 含错误恢复"
        steps:
            skip newlines
            expect indent token
            loop skip newlines and parse item
            break upon dedent or eof
            emit error when parse failure occurs
            synchronize and continue to next item
            consume dedent
            return items list

    func ParseAtomsUntil(stop):
        desc "解析原子序列直到遇到停止条件"
        steps:
            loop over tokens
            cease upon stop tokens
            break upon newline dedent eof
            handle bracket list literals
            track paren bracket angle depth
            convert each token to atom

    func AtomFromToken():
        desc "将单个 token 转换为 atom"
        steps:
            ellipsis becomes ident atom
            keywords become ident atoms
            ident string number become typed atoms
            punctuation becomes symbol atoms
            unknown tokens trigger error

    func ParseAtomListLiteral():
        desc "解析方括号列表字面量"
        steps:
            expect lbracket
            parse comma separated atom groups
            expect rbracket
            return atom list
