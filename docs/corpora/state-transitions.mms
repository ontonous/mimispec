// MimiSpec 0.3 Core acceptance corpus — state transitions and forbidden
// behavior.
//
// Exercises anonymous Flow, named Flow, event-labelled edges, repeatable
// requires/ensures on Flow arms, and the open-world rule: absence of an
// edge is not a prohibition unless an explicit rule closes the world.
//
// Part of the M5 corpus deliverable (roadmap §10).

rule$ "支付必须幂等：同一笔订单在任意状态被同一事件触发两次，结果不得不同"
rule$ "未列出的状态事件组合是否禁止，必须由明确规则表达，不能从缺省边推断"

flow$ OrderLifecycle:
    Pending:
        on CaptureConfirmed$ >>> Paid: requires$: capture.authority_known == true ensures$: order.paid == true
        on Cancelled >>> Cancelled: requires$: cancellation.requested == true ensures$: order.open == false
    Paid:
        on Refunded >>> Refunded: requires$: refund.amount <= order.captured ensures$: order.refunded == true
        on Settled$ >>> Settled: ensures$: order.final == true

flow$:
    Idle:
        on Start >>> Running: requires$: resource.available == true
        on Stop >>> Stopped:
    Running:
        on Stop >>> Stopped:
        on Fail >>> Failed: desc$ "运行中失败必须保留失败责任，不能静默回 Idle"

rule "Failed 必须有人或系统显式确认后才能回到 Idle"
