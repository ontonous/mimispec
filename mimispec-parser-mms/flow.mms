// FILE: flow.mms
// MimiSpec 解析器实现描述 — flow 状态机解析

@import "types.mms"

module FlowParser:

    desc "解析 flow 状态机 状态和转换臂"
    rule "单出转换可内联书写"

    func ParseFlowEntry():
        desc "解析单个状态及其转换"
        steps:
            parse fuzzy ident as state name
            check for arrow keyword and use inline form
            otherwise parse indented block of arms

    func ParseFlowArm():
        desc "解析转换臂 target requires desc"
        steps:
            expect arrow keyword
            parse fuzzy ident as target state
            parse optional requires condition
            parse optional description
            return flow arm structure

    func ParseFlowInlineArm():
        desc "解析单出转换的内联形式"
        steps:
            expect colon
            parse optional requires
            parse optional description
            return flow arm structure
