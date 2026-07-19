// MimiSpec 0.3 Core acceptance corpus — external boundaries.
//
// Exercises rules that bound the system: what is in scope, what is out of
// scope, what must be confirmed by an external system before the internal
// state can advance, and what must not be silently fabricated.
//
// Part of the M5 corpus deliverable (roadmap §10).

desc$ "系统与外部世界的边界必须显式：哪些由我负责，哪些由外部确认"

rule$ "外部确认不可伪造：内部状态不得在未收到外部确认前进入依赖该确认的状态"
rule$ "外部失败必须可见，不能在内部静默吞掉"
rule "外部边界的变化必须留痕"

module$ Boundary:
    desc$ "外部边界的代理层：所有跨边界交互必须经此"

    func$ AwaitExternalConfirmation(external_id):
        desc$ "等待外部系统确认，确认前不得推进内部状态"
        requires$: external_id.target_known == true
        ensures$: internal_state.advanced == true or external_timeout.visible == true
        rule$ "超时不是确认，必须作为外部失败可见"

    func$ ReceiveExternalEvent(event):
        desc$ "外部事件必须先验证来源再处理"
        requires$: event.source_known == true
        requires$: event.signature_valid == true
        ensures$: event.processed == true or event.rejected == true
        rule$ "未通过验证的事件不得进入处理路径"

    func$ PropagateToExternal(decision):
        desc$ "对外部系统的提交必须幂等"
        requires$: decision.authority_known == true
        ensures$: external_system.received == true
        with: Grant
        rule$ "对外提交失败时本地状态必须保留，不能伪装成已提交"

rule$ "外部系统不归我修改：它的行为只能由规则约束，不能由我直接重写"
