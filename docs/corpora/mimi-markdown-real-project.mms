// Real-project transcription: MIMI projects/mimi-markdown/main.mimi.
// The implementation is observation input, not authority for commitment.

desc? "一个使用纯 Mimi 实现的 Markdown 到 HTML 转换器，包含块级解析、行内格式化、HTML 渲染和命令行入口"
desc? "源码声称兼容解释器和编译后端，并内置了覆盖标题、列表、代码块、引用和表格的输出型测试"

rule? "转换器必须区分受信任的 HTML 标记与来自 Markdown 文本的不受信任内容"
rule? "输入为空或只包含空白时应产生空文档，不应崩溃或伪造内容"
rule? "未识别的 Markdown 内容应以可见文本保留，不能静默丢弃"
rule? "文件读取、输出写入和输入格式失败都应向 CLI 用户可见"

type? MarkdownCursor:
    source: Text
    position: Integer
    length: Integer

type? MarkdownBlockKind:
    Heading | Paragraph | CodeFence | UnorderedList | OrderedList | BlockQuote | Table | HorizontalRule

type? MarkdownBlock:
    kind: MarkdownBlockKind
    content: Text
    level: Integer
    language: Text
    items: TextList
    ordered: Boolean

type? ConversionRequest:
    input_path: Path
    output_path: Path
    document_title: Text

flow? DocumentConversion:
    Waiting:
        on InputSelected >>> Reading:
    Reading:
        on FileRead >>> Parsing:
        on ReadFailed >>> Failed: desc? "读取失败保留路径和原因"
    Parsing:
        on BlocksParsed >>> Rendering:
        on MalformedInput >>> Failed: desc? "严格错误与宽容降级策略尚待确认"
    Rendering:
        on HtmlRendered >>> Writing:
        on UnsafeOutput >>> Failed:
    Writing:
        on FileWritten >>> Completed:
        on WriteFailed >>> Failed: desc? "不得把部分写入报告为完成"

flow? BlockRecognition:
    AtLineStart:
        on BlankLine >>> AtLineStart:
        on HeadingLine >>> BlockReady:
        on FenceLine >>> InCodeFence:
        on ListLine >>> InList:
        on QuoteLine >>> InQuote:
        on TableLine >>> InTable:
        on PlainLine >>> InParagraph:
        on InvalidBlock >>> Failed: desc? "严格拒绝与宽容保留策略尚待人类确认"
    InCodeFence:
        on ClosingFence >>> BlockReady:
        on EndOfInput >>> BlockReady: desc? "当前实现会接受未闭合 fence，是否应报错尚待确认"
    InList:
        on DifferentLine >>> BlockReady:
    InQuote:
        on DifferentLine >>> BlockReady:
    InTable:
        on DifferentLine >>> BlockReady:
    InParagraph:
        on BlankLine >>> BlockReady:
        on HeadingLine >>> BlockReady:
    BlockReady:
        on Continue >>> AtLineStart:
        on EndOfInput >>> Completed:

module? MarkdownParser:
    desc? "用不可变 cursor 和行扫描将源文本分解为有序块列表"

    rule? "cursor.position 必须在 0 到 source.length 之间单调前进"
    rule? "每次成功解析非空块后必须消费至少一个输入字符"
    func? ReadLine(cursor):
        ensures?: result.cursor.position >= cursor.position
        steps:
            locate the next newline
            preserve line text without the delimiter
            advance past one delimiter when present
            return line and cursor >>> done

    func? ParseBlocks(source):
        ensures?: result.order_preserved == true
        steps:
            skip blank lines
            classify the current line
            parse exactly one recognized block
            append block in source order
            continue until input ends
            return blocks >>> done

    func? ParseCodeFence(cursor):
        ensures?: result.content.order_preserved == true
        steps:
            capture fence marker and language label
            collect source lines until a matching fence
            preserve embedded newlines
            return code block and cursor >>> done

    func? ParseList(cursor, ordered):
        ensures?: result.items.order_preserved == true
        steps:
            recognize consecutive list item prefixes
            remove one item prefix
            preserve each item as inline source
            return list block and cursor >>> done

    func? ParseTable(cursor):
        ensures?: result.rows.order_preserved == true
        steps:
            recognize header and separator rows
            skip the separator row
            preserve remaining rows for rendering
            return table block and cursor >>> done

module? InlineFormatting:
    desc? "单次线性扫描粗体、斜体、行内代码、链接和删除线，当前不是递归 Markdown 解析器"

    rule? "找不到闭合标记时必须保留原始标记"
    rule? "普通文本、标记内容、链接文字和链接目标的转义边界必须明确"
    func? FindClosingMarker(text, start, marker):
        ensures?: result >= -1
        steps:
            scan from the requested position
            compare complete marker text
            return position or missing marker >>> done

    func? RenderInline(text):
        ensures?: source_order_preserved == true
        steps:
            scan one inline token at a time
            recognize paired formatting markers
            render link label and destination
            preserve unmatched marker text
            return inline HTML >>> done

module? HtmlRenderer:
    desc? "将解析块映射为 HTML 片段，并可包装为完整文档"

    rule? "代码块内容必须转义 HTML 特殊字符"
    rule? "标题级别只能在 1 到 6 之间"
    rule? "表格的第一个数据行作为表头，其余数据行作为单元格"
    func? EscapeHtml(text):
        ensures?: result.contains_unescaped_html_metacharacter == false
        steps:
            escape ampersand before other metacharacters
            escape angle brackets and quotes
            return escaped text >>> done

    func? RenderBlock(block):
        ensures?: recognized_block_rendered_once == true
        steps:
            select HTML element from block kind
            render inline content when applicable
            escape code content
            return block HTML >>> done

    func? RenderDocument(blocks, title):
        ensures?: result.has_utf8_metadata == true
        steps:
            render blocks in source order
            select the first level one heading as default title
            escape document title
            wrap body in one HTML document
            return complete HTML >>> done

module? MarkdownCommandLine:
    desc? "计划中的 CLI 应读取 Markdown 文件并写入派生的 HTML 路径；当前 main 只打印用法"

    rule? "默认输出路径用 .html 替换输入的最后一个扩展名"
    rule? "只有 md、markdown 和 mdown 扩展名默认被视为 Markdown"
    func? ConvertFile(request):
        ensures?: success == true or failure.visible == true
        steps:
            validate input extension
            read the requested file
            parse Markdown blocks
            render one complete HTML document
            write the requested output path
            return conversion result >>> done

    func? RunCli(arguments):
        ensures?: exit_code.explained == true
        steps:
            parse input and optional output paths
            show usage when arguments are incomplete
            invoke file conversion when requested
            report output path or failure
            return exit code >>> done

module? MarkdownAcceptance:
    desc? "源码内置输出型测试，但多数测试只 println 结果，没有自动断言"

    func? RunAcceptanceSuite():
        ensures?: every_case_has_machine_checkable_result == true
        steps:
            test cursor and line helpers
            test every supported block kind
            test inline formatting and escaping
            test empty malformed and nested inputs
            test path and title helpers
            return pass and failure counts >>> done

rule? "当前 RenderInline 把普通文本和强调内容直接插入 HTML，与代码块的转义策略不一致，需要人类确认安全契约"
rule? "链接 href、代码语言 class 和未来图片 src 的属性转义及允许的 URL scheme 尚未定义"
rule? "测试包含图片、嵌套强调和嵌套列表，但当前解析器没有对应的完整实现"
rule? "ParseParagraph 只在空行或标题前停止，无空行的列表、fence、引用和表格是否应分块尚待确认"
rule? "任意包含竖线的行都可能被识别为表格，列数不一致、转义竖线和对齐语义未表达"
rule? "未闭合代码 fence 当前被接受至文件末尾，严格拒绝、警告或宽容渲染的策略尚待选择"
rule? "当前 main 只打印用法并返回成功，还没有连接已实现的文件读取、解析和写入流程"
rule? "源码中 test_escape_html 出现重复函数头，应作为实现缺陷而不是意图被保留"
