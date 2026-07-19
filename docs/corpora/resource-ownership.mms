// MimiSpec 0.3 Core acceptance corpus — resource ownership and permissions.
//
// Exercises typed records, capability attachment via `with`, resource
// lifecycle clauses, and the rule that locked does not mean implemented or
// verified — only that the human has confirmed the intent.
//
// Part of the M5 corpus deliverable (roadmap §10).

rule$ "资源所有权必须显式声明，不能从类型名推断"
rule$ "权限提升必须留痕，不能在静默路径中完成"

type$ Resource:
    desc$ "外部资源的抽象：句柄、生命周期和归属"
    id: Identifier
    owner: Principal
    kind: ResourceKind

type$ Principal:
    desc$ "可承担责任的主体"
    id: Identifier
    capabilities: Capability

type$ Capability:
    desc$ "线性能力：消耗即转移"
    Read | Write | Consume | Grant

module$ Ownership:
    desc$ "所有权转移必须保留原始责任，不能在转移中丢失"

    func$ TransferResource(resource, from, to):
        desc$ "转移是消耗：from 失去能力，to 获得能力"
        requires$: Grant in from.capabilities
        requires$: resource.owner == from
        ensures$: resource.kind in to.capabilities
        ensures$: not (resource.kind in from.capabilities)
        with: Grant, Consume
        rule$ "转移后 from 不得保留对原资源的写能力"

    func$ EscalateCapability(principal, capability):
        desc$ "权限提升必须留痕"
        requires$: principal.identity_verified == true
        ensures$: audit.escalation_recorded == true
        with: Grant
        rule$ "提升记录必须先于能力授予写入，不能事后补登"

rule$ "强锁的资源类型定义不得在未经人类确认下被 AI 改写为更宽的权限"
