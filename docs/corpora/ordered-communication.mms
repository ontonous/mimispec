// MimiSpec 0.3 Core acceptance corpus — ordered communication.
//
// Exercises ordered steps with explicit control flow, parallel steps, and
// the rule that communication order must be preserved — the parser must not
// reorder steps to "tidy" the document.
//
// Part of the M5 corpus deliverable (roadmap §10).

rule$ "通信顺序是协议语义的一部分，parser 不得重排"
rule "请求-响应必须成对，不能有未配对的发起"

module$ Protocol:
    desc$ "有状态的通信协议：每一步都依赖前一步的完成"

    func$ Handshake(client, server):
        desc$ "三次握手"
        requires$: client.identity_known == true
        requires$: server.identity_known == true
        ensures$: session.established == true
        steps:
            desc$ "1. 客户端发送 SYN"
            desc$ "2. 服务端回复 SYN-ACK"
            desc$ "3. 客户端发送 ACK"
            compute syn
            return session >>> established

    func$ RequestResponse(request):
        desc$ "请求必须先于响应"
        requires$: request.target_known == true
        ensures$: response.correlated == true
        steps:
            prepare request
            send request
            receive response
            return response >>> done

    func$ ParallelNegotiation(peers):
        desc$ "多个并行协商，但汇总必须串行"
        parasteps "并发协商阶段":
            negotiate peer_a
            negotiate peer_b
            negotiate peer_c
        steps:
            merge results
            return merged >>> done

rule$ "未到达的响应不得提前消费：响应必须在对应请求发出后才可见"
