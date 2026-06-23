# MimiSpec Agent Vision

## The Third Way

Human-agent interaction today is trapped between two broken paradigms:

| Chat | Workflow YAML |
|------|---------------|
| Ephemeral — every session starts from scratch | Over-specified — human must think like a computer |
| No structure — intent is buried in prose | No autonomy — agent follows tracks, can't deviate |
| No persistence — the artifact is the chat log, not the intent | No ambiguity — everything must be declared upfront |

**MimiSpec offers a third way: the `.mms` file as the persistent human-agent interface.**

```
Human writes intent ──→ .mms ──→ Agent reads boundaries
       ↑                          ↓
       └──── spec evolves ────── agent acts
```

The `.mms` file is not instructions. It is **territory with fences** — a set of intent declarations (`desc`), constraints (`rule`), confidence markers (`?`/`$`), and transitional structures (`flow`, `steps:`). The agent navigates the territory autonomously within the fences.

---

## Core Thesis

> **The `.mms` file is the deliverable. The conversation is ephemeral.**

In chat-based interaction, the value dies with the session. In MimiSpec, the value accumulates in a single structured file that:
- Starts as vague intent (`desc?? "..."`, `module??`)
- Evolves through partial structure (`type?`, `func?`)
- Ends as locked specification (`$`, `$$`)
- Is **always parsable** — every intermediate state is valid `.mms`
- Is **agent-agnostic** — one `.mms` can drive Claude today, GPT tomorrow, a local model next year

---

## How It Works

### 1. Human works at intent level

```mms
// Tell the agent WHAT, not HOW
@import "project.mms"
func RefactorPayment:
    desc "把支付模块从 Stripe 切换到 Adyen，保持接口不变"
    rule "切换过程不能导致订单状态丢失"
    ensures: payment.status in [Pending, Paid, Failed]
    steps:
        desc "迁移数据库 schema"
        desc "替换 API 调用"
        desc "验证全量回放"
```

### 2. Agent owns the HOW

The agent reads the `.mms`, understands:
- **Intent** (`desc`): what the human wants
- **Constraints** (`rule`): what must not break
- **Boundaries** (`ensures`): what must be true when done
- **Confidence** (`?`/`$`): which parts are settled vs exploratory

Then it plans, executes, and reports back — not in prose, but by evolving the `.mms`:

```mms
// Before: human wrote this
func RefactorPayment:
    desc "把支付模块从 Stripe 切换到 Adyen"
    steps:
        desc "迁移数据库 schema"

// After: agent evolved it
func RefactorPayment:
    desc "把支付模块从 Stripe 切换到 Adyen，保持接口不变"
    rule "切换过程不能导致订单状态丢失"
    ensures: payment.status in [Pending, Paid, Failed]
    steps:
        // 数据库迁移 / Database migration ✅
        migrate$ payments from stripe to adyen
        // API 替换 / API replacement ✅
        replace$ StripeClient with AdyenClient
        // 全量回放验证 / Replay verification ✅
        replay$ payments log verify all passed
```

### 3. `.mms` as trust boundary

```
?  = "I'm exploring, feedback welcome"
?? = "I'm very uncertain, please guide"
$  = "This is correct, move forward"
$$ = "Audited and locked, do not touch"
```

This is not syntax decoration. It is a **communication protocol** between human and agent — a way to say "this part is tentative" vs "this part is settled" without needing a separate conversation about it.

---

## Universal Agent Scenarios

### Code Development

| Phase | `.mms` State | Agent Role |
|-------|-------------|------------|
| Requirements | `desc?? "..."` | Propose architecture, identify ambiguity |
| Architecture | `type?`, `func?` | Draft structures, flag trade-offs |
| Implementation | `steps:` with actions | Write code, run tests |
| Review | `$` lock | Human reviews `.mms` + diff, locks |
| Maintenance | `$$` | Agent respects locked intent, works around it |

### Daily Work Automation

```mms
// 整理开发环境 / Tidy dev environment
func?? Organize:
    desc?? "清理 ~/Downloads 里的文件，按类型归档"
    rule "不要移动最近 7 天内的文件"
    rule "图片放到 ~/Pictures, 文档放到 ~/Documents"
    steps:
        desc?? "扫描目录"
        desc?? "分类文件"
    on "数据被误删":
        error "还原备份" >>> exit
```

The agent understands `rule` as hard constraint, `desc??` as negotiable intent, and `on` as recovery plan. It acts autonomously, reports back by locking `?` → `$`.

### Multi-step Research

```mms
flow Research:
    ??? "选题背景" >>> "文献综述":
        desc "搜索近三年的相关论文"
        rule "优先引用顶会论文"
    ??? "文献综述" >>> "方案设计":
        desc "提炼三种可行方案"
        on "现有方案都不合适":
            desc "尝试 hybrid 方案"
    ??? "方案设计" >>> "报告输出":
        ensures: report contains at least 3 references
```

`flow` encodes the research process as a state machine. The agent traverses states, human can `?`/`$` each state independently. If the agent finds literature gaps, it stays in "文献综述" autonomously.

---

## Why This Works

| Property | Why it matters |
|----------|---------------|
| **Progressive** | Human doesn't need to specify everything upfront. Start with `?`, lock with `$`. |
| **Autonomous-friendly** | Agent works inside the fences (`rule`, `ensures`), not on rails (`step1 → step2 → step3`). |
| **Ephemeral-resistant** | The `.mms` survives. Sessions end, but the spec persists. Open it next week, continue. |
| **Multi-model** | The same `.mms` can drive Claude for planning, a local model for execution, GPT for review. |
| **Verifiable** | `requires/ensures` are machine-checkable. `rule` is human-readable guardrail. |
| **Parsable** | Every tool — editors, CI, linters — can read `.mms`. It's not locked in a chat backend. |

---

## What MimiSpec Is Not

- Not a workflow engine — it doesn't orchestrate steps, it fences intent
- Not a programming language — it doesn't execute, it describes
- Not a prompt template — it doesn't dictate LLM behavior, it constrains it
- Not a chat replacement — it's the artifact chat produces

---

## The Loop

```
┌─────────────────────────────────────────────────────┐
│                                                      │
│   ┌──────┐    .mms intent    ┌───────┐    action    │
│   │Human │ ────────────────→ │ Agent │ ──────────→  │  World
│   │      │ ←──────────────── │       │ ←──────────  │
│   └──────┘   .mms evolved    └───────┘   outcome    │
│        ↑                                        │   │
│        └────────────────────────────────────────┘   │
│                 review + lock (? → $)               │
│                                                      │
└─────────────────────────────────────────────────────┘
```

Human writes intent. Agent acts. `.mms` evolves. Human reviews and locks. The file gets tighter with each iteration. The chat window is closed. The `.mms` remains.
