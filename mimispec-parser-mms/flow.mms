// FILE: flow.mms
// MimiSpec 解析器参考实现 — flow 状态机解析

@import "types.mms"

module FlowParser:

    desc "解析 flow 状态机: 状态 arrow 转移 守卫条件"
    desc "对应 src/lib/parser/flow.rs"

    rule "单出转移可内联一行"
    rule "多出转移必须用缩进块"

    func ParseFlow():
        desc "解析完整 flow 定义, 被 ParseFragment 调用"
        steps:
            parse fuzzy ident as flow name
            expect colon
            handle ellipsis placeholder
            parse indented block of entries
            return FlowDef

    func ParseFlowEntry():
        desc "解析单个状态及其转移臂"
        steps:
            parse fuzzy ident as state name
            desc "check inline arrow form"
            desc "inline means parse single arm without nesting"
            desc "otherwise expect colon then parse indented arms"

    func ParseFlowArmInBlock():
        desc "解析缩进块中的转移臂"
        steps:
            expect arrow plus commitment
            parse fuzzy ident as target
            desc "parse optional requires condition"
            desc "parse optional desc"
            return FlowArm

    func ParseFlowArmAfterToWithCommitment():
        desc "解析内联转移臂"
        steps:
            expect colon
            desc "parse optional requires"
            desc "parse optional desc"
            return FlowArm
