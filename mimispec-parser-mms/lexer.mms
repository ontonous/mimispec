// FILE: lexer.mms
// MimiSpec 解析器实现描述 — 词法分析器

@import "types.mms"

module LexerImpl:

    desc "Lexer 将源字符串转换为 Token 流"
    rule "缩进必须是 4 的倍数"

    type LexerState:
        chars: string
        line: usize
        col: usize
        pending: Token
        indentstack: usize
        atlinestart: bool
        sawblankline: bool

    func Tokenize(input):
        desc "消费所有 token 直到 EOF"
        steps:
            call nexttoken repeatedly
            accumulate each token into result list
            break when eof received
            return result list

    func NextToken():
        desc "获取下一个 token 按字符分发"
        steps:
            check pending token queue
            handle layout at line start
            skip inline whitespace
            dispatch by next character
            handle punctuation like colon comma pipe
            handle operators like plus minus star
            handle special tokens like dollar question

    func HandleLineStart():
        desc "处理行首缩进空行和注释"
        steps:
            count leading spaces
            advance past empty lines
            skip comment lines
            validate spaces as multiple of 4
            compare with indent stack
            emit indent dedent or newline

    func IdentOrKeyword(line, col):
        desc "识别标识符或关键字"
        steps:
            consume alphanumeric and underscore chars
            match against keyword lookup table
            return keyword token or ident token

    func StringToken(line, col):
        desc "解析双引号字符串"
        steps:
            consume opening quote
            loop until closing quote
            handle escape sequences
            reject if newline before close
            return string token

    func NumberToken(line, col):
        desc "解析数字字面量"
        steps:
            consume digits
            detect decimal point and trailing digits
            detect scientific notation e or E
            return number token

    func RecognizeMultiCharGreater():
        desc "识别大于号运算符系列"
        steps:
            single greater is gt
            greater equal is ge
            double greater is shr
            triple greater is arrow

    func RecognizeMultiCharOther():
        desc "识别各类多字符运算符"
        steps:
            star star is power
            equal equal is eqeq
            bang equal is noteq
            less equal is le
            double less is shl
            triple dot is ellipsis
            at import is import keyword
            double question is questionquestion
            double dollar is dollardollar
