// Real-project transcription: MIMI projects/mimichat/src/main.mimi.
// Actor/runtime details remain target-specific; this file captures reviewable intent.

desc? "一个支持多用户房间、私聊、昵称管理、消息历史和交互式客户端的 JSON-TCP 聊天系统"
desc? "服务为每个连接启动独立处理任务，并通过 Actor 串行化用户与房间状态"

rule? "昵称比较不区分大小写，且同一时刻必须唯一"
rule? "房间名称只能包含字母、数字、下划线和连字符，长度不超过 32"
rule? "普通聊天只广播给当前房间成员；私聊只发送给目标用户"
rule? "连接断开时必须注销用户、离开房间并通知仍在线的成员"
rule? "历史记录有明确上限，超过上限时保留最新消息"
rule? "JSON 字符串必须转义引号、反斜杠和控制字符"

type? ServerConfiguration:
    port: Integer
    max_clients: Integer
    history_file: Path
    max_history: Integer

type? User:
    nickname: Text
    connection: SocketHandle
    joined_at: Timestamp
    current_room: RoomName

type? Room:
    name: RoomName
    members: UserList
    topic: Text
    message_of_the_day: Text

type? ChatMessage:
    message_type: Text
    sender: Text
    room: Text
    text: Text
    target: Text
    timestamp: Timestamp

flow? UserSession:
    Connected:
        on ValidNickname >>> Registered:
        on NicknameCollision >>> Connected: desc? "客户端可以重新选择昵称"
        on InvalidNickname >>> Connected: desc? "错误必须说明昵称约束"
    Registered:
        on DefaultRoomJoined >>> Active:
        on Disconnect >>> Closed:
    Active:
        on JoinRoom >>> Active:
        on LeaveRoom >>> Active:
        on Rename >>> Active:
        on Kicked >>> Closed:
        on Quit >>> Closing:
        on NetworkFailure >>> Closing:
    Closing:
        on CleanupCompleted >>> Closed:

flow? RoomMembership:
    Outside:
        on JoinExistingRoom >>> Member:
        on CreateAndJoinRoom >>> Member:
    Member:
        on JoinOtherRoom >>> Member: desc? "加入新房间前必须从旧房间移除"
        on LeaveRoom >>> Outside:
        on Kicked >>> Outside:

module? UserRegistry:
    desc? "由单一 Actor 拥有连接、昵称、用户信息和内存历史，避免并发写入竞态"

    rule? "注册必须同时检查昵称和连接是否已存在"
    func? RegisterUser(nickname, connection):
        requires?: nickname.valid == true
        ensures?: result == accepted or registry.unchanged == true
        ensures?: accepted == true and nickname.unique == true or failure.visible == true
        steps:
            normalize nickname for lookup
            reject nickname collision
            reject duplicate connection
            store connection nickname and user information
            return registration result >>> done

    func? ChangeNickname(old_name, new_name):
        requires?: new_name.valid == true
        ensures?: changed == true or registry.unchanged == true
        steps:
            reject collision
            update nickname indexes and user record together
            return rename result >>> done

    func? UnregisterUser(connection):
        ensures?: all_indexes_agree == true
        ensures?: removed == true or failure.visible == true
        steps:
            locate nickname by connection
            remove connection and nickname indexes
            remove user information
            return removed nickname >>> done

module? RoomDirectory:
    desc? "由独立 Actor 拥有房间成员、主题和欢迎消息"

    rule? "同一用户不能在同一房间中出现两次"
    func? JoinRoom(room, nickname):
        requires?: room.exists == true
        ensures?: member_count.increased_once == true or membership.unchanged == true
        steps:
            decode current member list
            reject duplicate membership
            append nickname
            store updated member list
            return join result >>> done

    func? LeaveRoom(room, nickname):
        ensures?: nickname.absent_from_room == true
        steps:
            remove nickname from member list
            store updated member list
            return leave result >>> done

    func? SetRoomMetadata(room, topic, message_of_the_day):
        requires?: room.exists == true
        ensures?: metadata.visible_to_future_members == true
        steps:
            update topic when supplied
            update message of the day when supplied
            return metadata result >>> done

module? MessageHistory:
    desc? "内存历史保留最新消息，并可保存到配置的 JSON 文件"

    rule? "加载失败或 JSON 无效时不得用损坏数据替换有效内存历史"
    func? AppendMessage(history, message):
        ensures?: result.length <= history.maximum
        ensures?: result.order_preserved == true
        steps:
            append encoded message
            remove oldest entries above maximum
            return bounded history >>> done

    func? PersistHistory(history, path):
        ensures?: success == true or failure.visible == true
        steps:
            encode history as JSON
            write configured file
            return persistence result >>> done

module? ChatServer:
    desc? "TCP 服务接收连接，为每个连接启动处理任务，并通过 UserRegistry 和 RoomDirectory 协调共享状态"

    rule? "并发连接不能绕过 Actor 的状态所有权"
    rule? "max_clients 当前只出现在配置中；是否以及如何强制限制仍需确认"
    func? HandleConnection(connection):
        ensures?: disconnect_cleanup_completed == true
        steps:
            allocate initial nickname
            register user
            join default room
            send welcome topic and message of the day
            receive newline delimited commands
            validate and route each command
            persist history after accepted public messages
            unregister user
            leave current room
            notify remaining room members
            close connection

    func? RouteCommand(command, user):
        ensures?: exactly_one_handler_selected == true or failure.visible == true
        steps:
            handle room message or private message
            handle join leave create nickname and kick
            handle room users history topic and message of the day
            handle ping help and quit
            return routing result >>> done

    func? RunServer(configuration):
        requires?: configuration.port > 0
        ensures?: listener.closed_on_terminal_failure == true
        parasteps "connection tasks":
            accept client connection
            handle independent client connection
        steps:
            initialize user and room actors
            create default room
            load valid history when available
            bind configured port then listen
            keep accepting while service is active

module? ChatClient:
    desc? "交互式客户端并行接收服务端消息，同时读取并发送本地用户输入"

    func? RunClient(host, port, nickname):
        requires?: nickname.valid == true
        ensures?: connection.closed_on_exit == true
        parasteps "interactive session":
            receive and display server messages
            read and encode local commands
        steps:
            connect to server
            send requested nickname
            continue until quit disconnect or receive failure
            close connection

module? ChatAcceptance:
    desc? "源码包含组件测试与集成测试，覆盖编码、校验、Actor 状态、历史上限和基本房间生命周期"

    func? RunAcceptanceSuite():
        ensures?: all_results_reported == true
        steps:
            test JSON encoders and escaping
            test nickname and room validation
            test user registration rename and cleanup
            test room creation join leave and metadata
            test history order and rollover
            test basic user and room integration
            return pass and failure counts >>> done

rule? "管理员身份和 KICK 授权在当前实现中没有独立权限模型，任何已连接用户似乎都可请求踢人"
rule? "消息投递、历史落盘和断开清理之间没有事务边界；部分成功后的恢复语义仍需确认"
rule? "Actor mailbox 上限、慢消费者背压、任务取消和服务优雅停机策略仍未表达"
rule? "协议版本字段存在，但兼容协商、未知字段处理和客户端版本拒绝策略仍未表达"
