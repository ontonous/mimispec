// FILE: lexer.mms
// MimiSpec 解析器参考实现 — 词法分析器

@import "types.mms"

module LexerImpl:

    desc "Lexer 将源字符串转换为 Token 流, 对应 src/lib/lexer.rs"
    rule "缩进必须是 4 的整数倍"
    rule "字符串不允许隐式跨行"

    type LexerState:
        text: string
        line: usize
        col: usize
        pending: Token
        indent_stack: usize
        at_line_start: bool
        saw_blank_line: bool

    func Tokenize(input):
        desc "消费所有 token 直到 EOF"
        steps:
            call next_token repeatedly
            accumulate token list
            stop at eof
            return result list

    func NextToken():
        desc "获取下一个 token, 按字符分发"
        steps:
            check pending queue
            handle layout at line start
            skip inline whitespace
            dispatch by character

    func HandleLineStart():
        desc "行首处理: 缩进 indent/dedent, 空行, 注释"
        steps:
            count leading spaces
            return eof if source exhausted
            desc "skip blank lines: sets saw_blank_line flag"
            desc "skip comment lines"
            desc "validate spaces as multiple of 4"
            desc "compare indent stack to decide emit action"
            desc "deeper -> emit newline plus indent token"
            desc "shallower -> emit dedents plus newline"
            desc "same depth -> emit newline"
            clear saw_blank_line after flush

    func DispatchByChar():
        desc "按首字符分发到子解析器"
        steps:
            desc "newline -> newline token"
            desc "slash slash -> skip comment and recurse"
            desc "double quote -> parse string token"
            desc "digit -> parse number token"
            desc "ident start -> parse ident or keyword"
            desc "colon comma pipe -> emit punctuation"
            desc "ampersand caret tilde -> emit bit op"
            desc "plus minus -> emit arithmetic op"
            desc "star -> star or power star-star"
            desc "slash lparen rparen -> emit symbol"
            desc "lbracket rbracket -> emit bracket"
            desc "equals -> assign or eqeq"
            desc "bang -> must be bang-equals else error"
            desc "less -> lt le or shl"
            desc "greater -> gt ge shr or arrow"
            desc "dot -> dot or ellipsis else error"
            desc "at-sign -> check at-import or emit at"
            desc "question -> question or questionquestion"
            desc "dollar -> dollar or dollardollar"
            desc "other -> unexpected token error"

    func IdentOrKeyword():
        desc "识别标识符或关键字"
        steps:
            consume alphanumeric and underscore chars
            match against keyword table
            return keyword token or Ident token

    func StringToken():
        desc "解析双引号字符串字面量"
        steps:
            consume opening quote
            desc "loop until closing quote"
            desc "backslash -> parse escape sequence"
            desc "newline -> unterminated string error"
            desc "any char -> append to value"
            return String token

    func NumberToken():
        desc "解析数字: 整数 小数 科学计数法"
        steps:
            consume digits
            desc "detect decimal point plus digits"
            desc "detect e/E plus optional sign plus digits"
            return Number token

    func CountLeadingSpaces():
        desc "计算行首空格数, tab 计为 4"

    func EmitDedents(target):
        desc "弹出缩进栈并发射 DEDENT 直到目标深度"

    func FlushEof():
        desc "文件末尾弹出所有缩进并发射 EOF"

    func SkipComment():
        desc "跳过双斜杠注释到行尾"
