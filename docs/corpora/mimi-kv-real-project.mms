// Real-project transcription: MIMI projects/mimi-kv/src/main.mimi + client.mimi.
// The implementation is observation input, not authority for commitment.

desc? "一个通过 TCP 提供命令协议、并可将数据保存为 JSON 的嵌入式键值服务"
desc? "同一工程包含交互式命令行客户端，用于发送请求并显示人类可读响应"

rule? "服务默认监听 6380 端口，但部署时应允许明确覆盖"
rule? "一个连接中的无效命令或存储失败必须返回错误，不能让服务进程崩溃"
rule? "COUNT 必须始终等于当前实际键数量"
rule? "QUIT 只结束当前客户端会话，不关闭整个服务"
rule? "当前源码按连接串行处理；是否需要并发客户端与背压策略仍需人类确认"

type? CommandKind:
    Ping | Set | Get | Delete | Exists | Keys | Count | Save | Load | Clear | Quit

type? CommandRequest:
    command: CommandKind
    key: Text
    value: Text

type? CommandResponse:
    status: Text
    payload: Text

type? StoreSnapshot:
    entries: KeyValueMap
    key_count: Integer

flow? ClientConnection:
    Accepted:
        on RequestReceived >>> Processing: desc? "每个连接读取一条不超过接收缓冲区的请求"
        on EmptyRequest >>> Closing: desc? "空请求不产生存储副作用"
    Processing:
        on ResponseSent >>> Closing:
        on CommandRejected >>> Closing: desc? "错误响应仍必须关闭客户端文件描述符"
    Closing:
        on DescriptorClosed >>> Finished:

flow? PersistenceLifecycle:
    MemoryOnly:
        on SaveRequested >>> Saving:
        on LoadRequested >>> Loading:
    Saving:
        on WriteSucceeded >>> Persisted:
        on WriteFailed >>> MemoryOnly: desc? "写入失败必须保留内存数据并返回可见错误"
    Loading:
        on ValidJsonRead >>> Persisted:
        on FileMissing >>> MemoryOnly: desc? "缺失文件不能清空当前内存数据"
        on InvalidJsonRead >>> MemoryOnly: desc? "无效 JSON 不能替换当前内存数据"
    Persisted:
        on StoreChanged >>> MemoryOnly:

module? KeyValueServer:
    desc? "服务拥有内存 map、键计数、监听 socket 和 JSON 数据文件"

    rule? "SET 新键时计数加一，覆盖已有键时计数不变"
    rule? "DEL 仅在键存在时删除并减少计数"
    func? ApplyMutation(request, store):
        requires?: request.command in [Set, Delete, Clear]
        ensures?: result.key_count >= 0
        ensures?: result.key_count == result.entries.length
        steps:
            validate command arguments
            apply one mutation to the map
            update key count from the resulting map
            return response and updated store >>> done

    rule? "GET、EXISTS、KEYS 和 COUNT 不得修改存储"
    func? ExecuteQuery(request, store):
        requires?: request.command in [Get, Exists, Keys, Count]
        ensures?: store.unchanged == true
        steps:
            inspect requested key or collection
            encode protocol response
            return response >>> done

    func? SaveStore(store, path):
        requires?: path.is_configured == true
        ensures?: success == true or failure.visible == true
        steps:
            serialize map as JSON
            write configured data file
            return save result >>> done

    func? LoadStore(current_store, path):
        ensures?: loaded.valid == true or current_store.preserved == true
        steps:
            check whether data file exists
            read file
            validate nonempty JSON
            replace store only after validation
            recompute key count
            return load result >>> done

    func? Serve():
        ensures?: listener.closed_on_terminal_failure == true
        steps:
            load valid persisted data when available
            create TCP listener socket
            accept client connection
            parse one command
            route command to query mutation or persistence operation
            send protocol response
            close client descriptor
            continue accepting connections

module? KeyValueClient:
    desc? "客户端连接指定主机和端口，运行 REPL，并将协议响应格式化后显示"

    func? SendCommand(connection, command):
        requires?: connection.open == true
        ensures?: response.received == true or connection.failure_visible == true
        steps:
            append line terminator
            send command
            receive bounded response
            return response >>> done

    func? RunInteractiveClient(host, port):
        steps:
            connect to server
            read user command
            send command unless local exit was requested
            display formatted response
            send Quit before local exit
            close connection

rule? "协议当前依赖换行分帧和单次 recv；粘包、拆包、超长值与多命令连接的语义尚未明确"
rule? "SAVE 的原子替换、崩溃一致性和多进程写入策略尚未明确"
