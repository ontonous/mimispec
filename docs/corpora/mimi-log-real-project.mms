// Real-project transcription: MIMI projects/mimi-log/src/main.mimi.
// Source behavior is observed evidence; known implementation defects are not locked intent.

desc? "一个从 Apache Combined、Nginx 和 JSON Lines 文件读取记录，再进行过滤、聚合、分位数统计和多格式输出的命令行日志分析器"
desc? "源码还包含 partial read、byte read、JSON line read 和逐行 callback 的 pipeline 演示"

rule? "一条输入日志在每个过滤阶段最多保留一次，不得因匹配而复制"
rule? "格式检测失败和单行解析失败必须可区分，不能把损坏记录静默伪造成正常 INFO"
rule? "统计必须基于过滤后的真实记录数，并对空集合有明确结果"
rule? "CSV 和 JSON 输出必须对用户输入字段正确转义，生成结果必须能被标准消费者解析"
rule? "文件读取失败、无效正则和无效 CLI 参数必须产生非零退出码与可操作诊断"

type? LogFormat:
    Apache | Nginx | JsonLines | Unknown

type? LogLevel:
    Debug | Info | Warn | Error | Fatal | Unknown

type? LogEntry:
    timestamp: Text
    level: LogLevel
    source: Text
    message: Text
    method: Text
    path: Text
    status: Integer
    size: Integer
    latency: Decimal
    format: LogFormat
    raw: Text

type? LogStatistics:
    total: Integer
    info_count: Integer
    warn_count: Integer
    error_count: Integer
    fatal_count: Integer
    error_rate: Decimal
    average_latency: Decimal
    p50_latency: Decimal
    p95_latency: Decimal
    p99_latency: Decimal
    maximum_latency: Decimal
    status_2xx: Integer
    status_3xx: Integer
    status_4xx: Integer
    status_5xx: Integer

type? AnalyzerOptions:
    input_path: Path
    minimum_level: LogLevel
    maximum_level: LogLevel
    raw_pattern: Text
    source_filter: Text
    method_filter: Text
    path_pattern: Text
    minimum_status: Integer
    maximum_status: Integer
    output_format: Text
    head_count: Integer
    tail_count: Integer

flow? LogAnalysisPipeline:
    Waiting:
        on ArgumentsAccepted >>> Reading:
        on HelpRequested >>> Completed:
        on InvalidArguments >>> Failed:
    Reading:
        on FileRead >>> Parsing:
        on ReadFailed >>> Failed: desc? "读取失败应包含文件路径"
    Parsing:
        on LinesParsed >>> Filtering:
        on MalformedLine >>> Parsing: desc? "跳过、保留 Unknown 或终止的策略尚待确认"
    Filtering:
        on FiltersApplied >>> Aggregating:
        on InvalidPattern >>> Failed:
    Aggregating:
        on StatisticsRequested >>> Rendering:
        on EntriesRequested >>> Rendering:
    Rendering:
        on OutputRendered >>> Completed:
        on SerializationFailed >>> Failed:

flow? LineClassification:
    Raw:
        on EmptyLine >>> Ignored:
        on ValidJson >>> JsonDetected:
        on ApacheSignature >>> ApacheDetected:
        on NginxSignature >>> NginxDetected:
        on UnknownSignature >>> UnknownDetected:
    JsonDetected:
        on Parsed >>> Ready:
        on MissingField >>> Ready: desc? "字段缺失时的默认值必须明确"
    ApacheDetected:
        on Parsed >>> Ready:
        on MalformedLine >>> Rejected:
    NginxDetected:
        on Parsed >>> Ready:
        on MalformedLine >>> Rejected:
    UnknownDetected:
        on PreserveRaw >>> Ready:

module? LogParsing:
    desc? "根据行内标记自动检测格式，并将多种输入归一化为 LogEntry"

    rule? "Apache 和 Nginx 解析不得在字段缺失时越界访问拆分结果"
    rule? "未知 level 不应静默等同 INFO，其过滤排序需要显式政策"
    func? DetectFormat(line):
        ensures?: result in [Apache, Nginx, JsonLines, Unknown]
        steps:
            trim the input line
            recognize valid JSON
            inspect timestamp quote and method signatures
            return detected format >>> done

    func? ParseApache(line):
        ensures?: success == true or failure.visible == true
        steps:
            extract host timestamp and request text
            validate method and path fields
            parse status and response size
            derive level from status
            return normalized entry >>> done

    func? ParseNginx(line):
        ensures?: success == true or failure.visible == true
        steps:
            extract host request status and size
            parse configured latency field when present
            derive level from status
            return normalized entry >>> done

    func? ParseJsonLine(line):
        ensures?: success == true or failure.visible == true
        steps:
            validate JSON object
            read text integer and decimal fields
            derive missing level and message fields
            return normalized entry >>> done

    func? ParseAuto(line):
        ensures?: result.raw == line
        steps:
            detect one input format
            invoke exactly one matching parser
            preserve unknown raw input with explicit status
            return one normalized entry >>> done

module? LogFiltering:
    desc? "按 level 范围、HTTP status、原始行正则、source、method、path 和 format 顺序过滤"

    rule? "过滤器按 CLI 声明顺序组合，每一步只能删除或保留记录"
    rule? "minimum_status 和 maximum_status 同时给定时必须验证下界不大于上界"
    func? FilterByStatus(entries, minimum, maximum):
        ensures?: result.length <= entries.length
        ensures?: every_result_matches_range == true
        steps:
            inspect each status once
            append a matching entry once
            preserve original order
            return filtered entries >>> done

    func? ApplyFilters(entries, options):
        ensures?: result.order_preserved == true
        steps:
            apply requested level bounds
            apply raw source method and path filters
            apply requested status bounds
            return filtered entries >>> done

module? LogStatistics:
    desc? "对过滤后记录计数，计算错误率、latency 分布和 HTTP status 分桶"

    rule? "error_rate 等于 (ERROR 数量 + FATAL 数量) 除以总记录数"
    rule? "latency 平均值的分母只包含有有效 latency 的记录"
    rule? "分位数输入必须已排序，索引、插值和空集合策略必须稳定"
    func? Percentile(sorted_values, percentile):
        requires?: percentile >= 0.0 and percentile <= 1.0
        ensures?: result.within_observed_range == true or sorted_values.empty == true
        steps:
            handle an empty sample
            compute the selected sample position
            clamp position to available values
            return percentile value >>> done

    func? ComputeStatistics(entries):
        ensures?: result.total == entries.length
        ensures?: result.error_rate >= 0.0 and result.error_rate <= 1.0
        steps:
            count each level
            collect positive latency samples
            sort latency samples
            compute average and percentiles
            count each HTTP status bucket
            return statistics >>> done

module? LogSerialization:
    desc? "把记录输出为人类可读文本、CSV 或 JSON，把统计输出为文本、CSV metric 表或 JSON"

    rule? "CSV 字段中的逗号、引号和换行符必须按选定方言转义"
    rule? "JSON 字符串必须转义引号、反斜杠和控制字符"
    func? EntriesToCsv(entries):
        ensures?: result.parseable_as_csv == true
        steps:
            emit one stable header row
            escape every selected field
            emit one row per entry
            return CSV text >>> done

    func? EntriesToJson(entries):
        ensures?: result.parseable_as_json == true
        steps:
            encode each entry as a JSON object
            preserve numeric fields as numbers
            join objects in one array
            return JSON text >>> done

    func? StatisticsToJson(statistics):
        ensures?: result.parseable_as_json == true
        steps:
            encode count rate and latency fields
            return JSON object >>> done

module? LogCommandLine:
    desc? "解析一个输入文件、可选过滤、聚合分组、head 或 tail 范围和 pipeline 演示标志"

    rule? "未知选项、缺失选项值、多个输入路径和不支持的输出格式必须被拒绝"
    rule? "head 与 tail 同时给定时的组合语义必须明确，不得因赋值顺序偶然覆盖"
    func? ParseArguments(arguments):
        ensures?: success == true or failure.visible == true
        steps:
            require exactly one input path
            validate option names and required values
            validate level status count and output ranges
            return analyzer options >>> done

    func? RunAnalysis(options):
        ensures?: exit_code.explained == true
        ensures?: success == true or failure.visible == true
        steps:
            run requested pipeline demonstrations
            read the complete log file
            parse nonempty lines
            apply filters in declaration order
            compute requested statistics or selected entries
            serialize the requested output format
            return exit code >>> done

rule? "当前 FilterByStatus 对每条匹配记录 push 了两次，这是实现缺陷，不应被当作业务意图"
rule? "当前 error_rate 表达式只用 total 除 fatal 计数，与总错误率含义不一致"
rule? "ParseJsonFieldFloat 和 ParseNginxLatency 当前固定返回 0.0，latency 统计尚不是可验证的已实现能力"
rule? "DetectFormat 只返回 JSON、Apache 或 Unknown，声称支持的 Nginx 分支当前无法被自动选中"
rule? "Apache 和 Nginx 解析直接索引 split 结果，损坏行的拒绝、降级和计数策略尚未实现"
rule? "CSV 和 JSON 当前通过字符串拼接生成且未转义字段，含逗号、引号、换行或控制字符的输入会破坏输出"
rule? "源码头部声称支持 time range filtering，但 CLI 和过滤器中没有对应时间范围功能"
rule? "当前 CLI 静默忽略未知选项和缺失值，多个非选项参数则由最后一个覆盖输入路径"
rule? "head 和 tail 同时启用时 tail 会基于原始 filtered 列表重建输出，实际覆盖了之前的 head 结果"
