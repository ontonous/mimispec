// FILE: parser-core.mms
// MimiSpec 解析器参考实现 — Parser 核心

@import "types.mms"

module ParserCore:

    desc "Parser 核心: token 导航 rule 管理 错误恢复 atom 解析"
    desc "对应 src/lib/parser/mod.rs"

    rule "空行大于等于 3 阻断 rule 附着链, rule 变为全局"

    func Peek():
        desc "查看当前 token 不消费"

    func PeekKind():
        desc "查看当前 token 类型"

    func Advance():
        desc "消费当前 token 并前进, EOF 不自增"

    func IsAtEnd():
        desc "是否到达 token 流末尾"

    func Check(kind):
        desc "检查当前 token 是否匹配指定类型"

    func Matches(kind):
        desc "若匹配则消费并返回 true"

    func Expect(kind, what):
        desc "期望当前 token 为指定类型, 否则报错"

    func ExpectKw(kind, what):
        desc "匹配关键字后解析 commitment 后缀"

    func Commitment():
        desc "解析 $ $$ ? ?? 后缀, 顺序先锁定后不确定"
        steps:
            desc "check dollardollar then optional ?? or ?"
            desc "check dollar then optional ?? or ?"
            desc "check questionquestion, reject if dollar follows"
            desc "check question, reject if dollar follows"
            return none absent

    func FuzzyIdent():
        desc "消费标识符或关键字并解析 commitment"

    func FuzzyString():
        desc "消费字符串字面量并解析 commitment"

    func ExpectString():
        desc "期望下一个 token 是字符串字面量"

    func CurrentPos():
        desc "返回当前 token 的行列号"

    func SkipNewlines():
        desc "跳过连续换行 token"

    func SkipNewlinesAndCount():
        desc "跳过并计数连续换行 token"

    func LineWillEnd():
        desc "判断当前 token 是否为换行或反缩进或EOF"

    func ConsumePendingRules():
        desc "扫描并消费连续 rule, 追加到 pending 列表"
        steps:
            desc "look ahead counting newlines"
            desc "skip 1-2 newlines and consume rule tokens"
            desc "3 or more newlines break the chain"
            desc "comment lines are transparent"

    func TakePendingRules():
        desc "取出并清空 pending 规则列表"

    func AttachRulesToFragment(fragment):
        desc "将 pending rules 附着到相邻 fragment"
        steps:
            desc "module -> module level rules"
            desc "typedef -> typedef level rules"
            desc "flow -> flow level rules"
            desc "func -> func level rules"
            desc "other fragment types discard rules"

    func SynchronizePastImport():
        desc "跳过错误 import 后的 token 直到合法起始点"

    func SynchronizeToFragmentStart():
        desc "跳过 token 直到遇到 fragment 起始关键字"

    func SynchronizePastNestedBlock():
        desc "跳过嵌套块直到返回, 追踪 indent/dedent"

    func SynchronizeToNextItemInBlock():
        desc "跳过错 item 到同层下一个, 停在换行"

    func ParseBlock(parseItem):
        desc "解析缩进块, 含逐 item 错误恢复"
        steps:
            skip newlines
            expect indent
            desc "repeatedly parse items until dedent"
            skip newlines
            desc "break on dedent or eof"
            desc "parse item with error recovery"
            desc "synchronize to next item and continue"
            consume dedent
            return items

    func ParseAtomsUntil(stop):
        desc "解析原子序列直到停止, 追踪括号深度"

    func AtomFromToken():
        desc "单个 token 转换为 Atom"

    func ParseAtomListLiteral():
        desc "解析方括号列表字面量"
        steps:
            expect lbracket
            desc "parse comma separated atom groups"
            expect rbracket
            return Atom list

    func ParseTargetFromAtoms(atoms):
        desc "atom 序列转换为赋值目标表达式"

    func ParseSimpleValueFromAtoms(atoms):
        desc "单个 atom 转换为赋值右侧简单值"
