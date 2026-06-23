// FILE: ui.mms
// MimiSpec 解析器参考实现 — UI 视图解析

@import "types.mms"

module UIParser:

    desc "解析 ui 声明: 视图树 stack parallel 叶子 事件绑定"
    desc "对应 src/lib/parser/ui.rs"

    func ParseUI():
        desc "解析 ui 视图定义"
        steps:
            expect kw_ui plus commitment
            parse fuzzy ident as view name
            desc "check binds keyword then parse model name"
            expect colon
            handle ellipsis placeholder
            desc "parse indented ui root node"
            return UiDef

    func ParseUiRoot():
        desc "解析 UI 根节点, 必须为 stack 或 parallel"
        steps:
            expect indent
            desc "kw_stack -> parse stack node"
            desc "kw_parallel -> parse parallel node"
            desc "otherwise -> error"

    func ParseUiNode():
        desc "UI 节点分发器"
        steps:
            skip newlines
            desc "kw_stack -> parse stack node"
            desc "kw_parallel -> parse parallel node"
            desc "kw_error -> parse error node"
            desc "string literal -> parse leaf node"
            desc "otherwise -> error"

    func ParseStackNode():
        desc "解析 stack 容器, 纵向堆叠"
        steps:
            expect kw_stack plus commitment
            desc "parse optional string label"
            expect colon
            parse indented child ui nodes
            return stack node

    func ParseParallelNode():
        desc "解析 parallel 容器, 横向排列"
        steps:
            expect kw_parallel plus commitment
            desc "parse optional string label"
            expect colon
            parse indented child ui nodes
            return parallel node

    func ParseUiLeaf():
        desc "解析叶子: 字符串内容加可选注解"
        steps:
            parse fuzzy string as content
            desc "loop until line end"
            desc "kw_desc -> parse desc annotation"
            desc "kw_requires -> parse condition guard"
            desc "kw_with -> parse capability list"
            desc "kw_on -> parse event binding then break"
            return leaf node

    func ParseErrorNode():
        desc "解析 UI 错误节点"
        steps:
            expect kw_error plus commitment
            parse fuzzy string as message
            desc "parse optional desc annotation"
            return error node

    func ParseOnBinding():
        desc "解析 on 事件绑定"
        steps:
            desc "event name as ident or natural string"
            expect colon
            desc "parse comma separated action list"
            return OnBinding

    func ParseActionExpr():
        desc "解析逗号分隔的动作列表"
        steps:
            parse first action
            desc "while comma follows then parse next action"
            return ActionExpr

    func ParseAction():
        desc "解析单个动作"
        steps:
            desc "kw_arrow -> navigate action"
            desc "string literal -> natural action"
            desc "parse expr check assign -> assign action"
            desc "otherwise -> call action"
