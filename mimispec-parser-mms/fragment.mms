// FILE: fragment.mms
// MimiSpec 解析器实现描述 — Fragment 顶层分发

@import "types.mms"
@import "type.mms"
@import "func.mms"
@import "parser-core.mms"

module FragmentDispatch:

    desc "顶层 fragment 分发 按 token 选择子解析器"
    rule "任何语法子树都是合法的顶层 Fragment"

    func ParseFragment():
        desc "主分发器 按 peek kind 路由"
        steps:
            match peek kind
            kw_module runs parse module
            kw_type runs parse typedef
            kw_flow runs parse flowdef
            kw_func runs parse funcdef
            kw_ui runs parse uidef
            kw_steps runs parse steps fragment
            kw_stack or kw_parallel or string runs parse ui node
            kw_ellipsis returns placeholder
            kw_if kw_for kw_while kw_parasteps wraps step into fragment

    func ParseModule():
        desc "解析 module 声明 含 items"
        steps:
            expect kw_module keyword
            parse fuzzy ident as name
            expect colon
            parse indented block
            first description becomes module description
            math blocks stored as module invariants
            rule defs collected as pending
            other items parsed via parse_fragment

    func ParseFunc():
        desc "解析 func 定义 含参数能力契约和步骤"
        steps:
            expect kw_func keyword
            parse fuzzy ident as name
            expect lparen
            parse comma separated params
            expect rparen
            parse optional with capabilities
            expect colon
            handle ellipsis placeholder
            parse indented body
            first description becomes function description
            kw_requires parses pre condition
            kw_ensures parses post condition
            kw_math parses math block
            kw_steps parses step list

    func ParseTypeDef():
        desc "解析 type 定义 枚举或记录"
        steps:
            expect kw_type keyword
            parse fuzzy ident as name
            expect colon
            handle ellipsis placeholder
            detect inline enum via pipe presence
            detect block enum via indent plus bare identifiers
            otherwise parse record block with fields
            description stored as type description
            mathematical invariants stored as type constraints

    func ParseFlowDef():
        desc "解析 flow 状态机"
        steps:
            expect kw_flow keyword
            parse fuzzy ident as name
            expect colon
            handle ellipsis placeholder
            parse indented block of entries
            each state name becomes an entry
            inline arrow used when single transition exists
            block arms used when multiple transitions exist
            each arm carries optional requires and description
