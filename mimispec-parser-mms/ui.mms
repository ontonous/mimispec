// FILE: ui.mms
// MimiSpec 解析器实现描述 — UI 视图解析

@import "types.mms"

module UIParser:

    desc "解析 ui 声明 视图树 容器 叶子 事件"

    func ParseUI():
        desc "解析 ui 声明 含 binds 和根节点"
        steps:
            expect kw_ui keyword
            parse fuzzy ident as view name
            check binds keyword and parse model name
            expect colon
            parse indented ui root node

    func ParseUiNode():
        desc "UI 节点分发器"
        steps:
            kw_stack runs parse stack container
            kw_parallel runs parse parallel container
            kw_error runs parse error node
            string literal runs parse leaf node

    func ParseStackNode():
        desc "解析 stack 容器 垂直布局"
        steps:
            expect kw_stack keyword
            parse optional string label
            expect colon
            parse indented children nodes

    func ParseParallelNode():
        desc "解析 parallel 容器 水平布局"
        steps:
            expect kw_parallel keyword
            parse optional string label
            expect colon
            parse indented children nodes

    func ParseUiLeaf():
        desc "解析叶子节点 内容加可选注解"
        steps:
            parse fuzzy string as content
            parse optional "desc" annotation
            parse optional requires guard
            parse optional with capabilities
            parse optional binding

    func ParseOnBinding():
        desc "解析事件绑定 事件名和动作"
        steps:
            expect kw_on keyword
            parse event name as ident or string
            expect colon
            parse comma separated action list

    func ParseAction():
        desc "解析单个动作"
        steps:
            arrow keyword runs navigate action
            string literal runs natural language action
            assign detected runs assign action
            otherwise runs function call action
