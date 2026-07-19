// MimiSpec 0.3 Core acceptance corpus — failure and recovery.
//
// Exercises repeatable descriptions on failure modes, persistent resource
// policy, and recovery contracts that must not silently fabricate default
// values for resources that cannot be defaulted.
//
// Part of the M5 corpus deliverable (roadmap §10).

desc$ "失败传播和恢复是业务决定，不是默认值构造"

rule$ "恢复不得用零值、空列表或伪造句柄代替无法默认构造的外部资源"
rule "失败责任必须显式保留，不能在恢复路径中丢失"

type$ FailureScope:
    desc$ "区分失败的传播责任"
    desc$ "同名失败在不同 scope 中仍可能具有不同恢复责任"
    LocalOperation | BusinessState | ConcurrentPeer | ExternalBoundary

module$ Recovery:
    desc$ "恢复路径必须保留原始失败责任和已尝试的恢复步骤"

    func$ RecoverBusinessState(fault):
        desc$ "恢复是业务决定"
        desc$ "无法默认构造的外部资源必须由人类或外部系统提供"
        requires$: fault.kind_is_known == true
        requires$: persistent_resource_policy_is_explicit == true
        ensures$: recovered_state_is_valid == true or recovery_failure_is_visible == true
        rule$ "恢复失败必须可见，不能静默回滚到伪装的成功状态"

    func$ EscalateToHuman(fault):
        desc$ "当系统无法自动恢复时，必须升级给人类"
        requires$: fault.kind_is_known == true
        requires$: automatic_recovery_attempted == true
        ensures$: human_queue.length > 0
        ensures$: fault.preserved == true

rule$ "重试不是恢复：重试必须受限于原始失败 scope，不能跨 scope 副作用"
