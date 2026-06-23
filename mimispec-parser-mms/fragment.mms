// FILE: fragment.mms
// MimiSpec 解析器参考实现 — Fragment 顶层分发

@import "types.mms"
@import "parser-core.mms"

module FragmentDispatch:

    desc "顶层 fragment 分发, 按首 token 路由到子解析器"
    desc "对应 src/lib/parser/fragment.rs"

    rule "任何语法子树都是合法的顶层 Fragment"

    func ParseFragment():
        desc "按 peek kind 路由到子解析器"
        steps:
            desc "kw_module -> parse module"
            desc "kw_type -> parse type def"
            desc "kw_flow -> parse flow def"
            desc "kw_func -> parse func def"
            desc "kw_ui -> parse ui def"
            desc "kw_steps -> parse steps fragment block"
            desc "kw_stack kw_parallel or string -> parse ui node"
            desc "kw_ellipsis -> return placeholder fragment"
            desc "control flow keywords -> wrap single step"
            desc "other -> try expr, fallback to action step"

    func ParseStepsFragment():
        desc "解析独立 steps 块作为顶层 Fragment"
        steps:
            expect kw_steps plus commitment
            expect colon
            parse indented block of steps
            return steps fragment

    func ParseDescEntity():
        desc "解析 desc 关键字后的描述实体"
        steps:
            expect kw_desc
            parse commitment
            parse fuzzy string
            return Desc

    func StepKeywordCommitment(step):
        desc "从步骤中提取关键字 commitment 用于顶层包装"

    func ParseModule():
        desc "解析 module 声明, 可嵌套, 含 items 列表"
        steps:
            expect kw_module plus commitment
            parse fuzzy ident as name
            expect colon
            handle ellipsis placeholder
            expect indent
            desc "first desc becomes module description"
            desc "rule attaches as pending to next entity"
            desc "math becomes module math block"
            desc "other items -> inner fragment recursively"
            consume dedent
            return module fragment

    func ParseTypeDef():
        desc "解析 type: 枚举 inline/block 或 record"
        steps:
            expect kw_type plus commitment
            parse fuzzy ident as name
            expect colon
            handle ellipsis placeholder
            desc "detect inline enum by pipe on same line"
            desc "detect block enum by indent plus bare idents"
            desc "otherwise parse record block with fields"
            return typedef fragment

    func ParseFunc():
        desc "解析 func: 参数能力契约步骤"
        steps:
            expect kw_func plus commitment
            parse fuzzy ident as name
            desc "parse optional parenthesized param list"
            desc "parse optional with capability list"
            expect colon
            handle ellipsis placeholder
            expect indent
            desc "first desc -> func description"
            desc "later desc -> Desc step"
            desc "requires -> precondition, ensures -> postcondition"
            desc "math -> math block"
            desc "steps -> step block"
            desc "any step -> inline in steps block"
            return func fragment

    func ParseFlowDef():
        desc "解析 flow 状态机"
        steps:
            expect kw_flow plus commitment
            parse fuzzy ident as name
            expect colon
            handle ellipsis placeholder
            expect indent
            desc "each entry: state name plus arms"
            desc "single arm -> inline arrow form"
            desc "multiple arms -> indented arm block"
            return flow fragment
