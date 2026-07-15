# MimiSpec 语法规范草案（0.3.x 目标）

> **版本状态说明**：当前已发布的参考实现是 `v0.2.1`。本文包含计划在
> `0.3.x` 系列逐步冻结和实现的前瞻语义，不能作为已经发布的
> `v1.0.0-rc.2` 行为声明。0.3.x 的实现顺序见
> [`roadmap-0.3.x.md`](roadmap-0.3.x.md)，后缀的规范语义见
> [`commitment-state-machine.md`](commitment-state-machine.md)。

**MimiSpec**（文件后缀 `.mms`）是一门高信息密度的意图描述语言。

核心设计原则：**任何语法子树都可以作为合法的顶层 Fragment 存在**。从一行 `type` 定义到完整的 `module` 树，从单个 `if` 分支到完整的 UI 视图，`.mms` 文件始终合法、始终精确、始终可解析。

> **设计格言**：From Scratch to Full —— 碎片是起点，聚合是过程，完整是结果。

MimiSpec 以**单文件优先**为设计原则：单个 `.mms` 文件可以独立解析；跨文件引用通过 `@import` 显式声明，链接和项目级分析由 resolver、IDE 或其他工具完成。

---

## 1. 元语法：槽位、实体与约束

MimiSpec 的语法位置本质上都是**槽位**（slot），等待被填充。槽位由缩进层级界定，同一缩进层级内的语句按顺序排列。

### 1.1 三种核心语法元素

| 元素 | 语义 | 语法 | 示例 |
|------|------|------|------|
| **实体（Entity）** | 槽位中的内容 | 独立存在于 block 中 | `type Order:`, `desc "订单数据"`, `check balance` |
| **占位符（Placeholder）** | 待填充的空白槽位 | `...` | `...` |
| **约束（Constraint）** | 施加于下一个实体的规则 | 写在实体前面，前置附着 | `rule "余额必须大于0"` |

### 1.2 `desc` = 实体占位（自然语言实体）

`desc "..."` **不是修饰符，而是一个独立的实体**。它表示：

> **"这里需要一个东西，具体是什么让 AI 决定，但意图是这个描述。"**

```mimispec
func Pay:
    desc "处理支付：检查余额、扣款、改状态"   # ← desc 是 func body 的一个实体元素
    steps:
        desc "检查买家余额是否充足"             # ← desc 是 steps block 的一个实体元素
        desc "调用支付网关扣款"
        desc "把订单状态改为已支付"
```

`desc` 和 `...` 的关系：
- `...` = "我不知道这里要什么"
- `desc "检查余额"` = "我知道这里要一个检查余额的东西，但具体实现交给 AI"

`desc` 的内容是自然语言，解析器不做语义解析，直接透传给 AI。

### 1.3 `rule` = 约束声明（前置附着）

`rule "..."` **不是实体，而是施加于下一个实体的约束**。它表示：

> **"紧随其后的实体，必须满足这条约束。"**

```mimispec
rule "支付必须幂等"
func Pay:
    steps:
        check balance
```

`rule "支付必须幂等"` 附着于 `func Pay`，约束整个函数的实现。

**附着规则**：

1. 连续的 `rule` 语句收集为一个约束列表，全部附着给下一个实体。  
   这里的“连续”允许由普通换行分隔；只要中间没有空行，就视为同一约束链。
2. 空行阻断附着链，未被接收的 `rule` 变为当前层级全局约束：
   - 文件顶层 → `file.rules`
   - `module` body 内 → `module.rules`
   - `func` body 内 → `func.rules`
3. `//` 注释对 parser 来说是透明的，但**独占一行的 `//` 注释会被 lexer 视为空行**，因此也能阻断约束链。阅读器可以把它当作规则分组的标题，但这不是语法要求。
4. `rule` 约束的是**整个实体及其内部所有内容**（递归约束）。

```mimispec
// 文件级约束组
rule "系统必须支持幂等操作"
rule "所有转换必须有日志"

// module 级约束组
rule "模块必须可回滚"
module Shop:
    // 本 module 的约束组
    rule "金额不能为负数"
    rule "订单金额必须大于 0"

    type Order:
        desc "订单数据"
        amount: Money

    // func 级约束组
    rule "支付必须幂等"
    rule "退款必须在有效期内"
    func Pay:
        steps:
            check balance
            charge payment
```

在上例中：

- `系统必须支持幂等操作`、`所有转换必须有日志` 因空行/注释行未被下一个 `module` 接收，进入 `file.rules`；
- `模块必须可回滚` 前有空行，也进入 `file.rules`；
- `金额不能为负数`、`订单金额必须大于 0` 被空行/注释行阻断，进入 `module Shop.rules`；
- `支付必须幂等`、`退款必须在有效期内` 紧跟 `func Pay`（无空行），附着给 `func Pay`。

### 1.4 `desc` vs `rule` 的本质区别

| | `desc` | `rule` |
|--|--------|--------|
| **语法角色** | **实体**（block 中的独立元素） | **修饰符**（前置附着于下一个实体） |
| 是否产生内容 | 是（产生一个自然语言占位实体） | 否（只声明约束） |
| AI 如何理解 | "请在这个位置生成一个东西，意图是..." | "请确保下一个东西满足..." |
| 能否独立存在 | 能（`desc "..."` 单独一行合法） | 不能（`rule` 必须附着于下一个实体） |
| 内容是否被解析 | 否（自然语言，透传 AI） | 否（自然语言，透传 AI） |

---

## 2. 文件结构

### 2.1 顶层 Fragment

一个 `.mms` 文件由零个或多个 `Fragment` 组成。每个 Fragment 独立解析，互不影响。

```
Fragment₁

Fragment₂

...
```

空文件是合法的（零个 Fragment）。

### 2.2 Fragment 类型

| Fragment | 首 token | 说明 |
|----------|----------|------|
| `Module` | `module` | 命名空间容器，可嵌套 |
| `TypeDef` | `type` | 枚举或记录定义 |
| `Flow` | `flow` | 状态机转移图 |
| `Func` | `func` | 函数契约与步骤 |
| `Ui` | `ui` | 视图骨架 |
| `Steps` | `steps` / `if` / `for` / `while` / `parasteps` / `desc` | 裸步骤流（顶层的 `desc`、控制流、动作都会解析为单步 Steps Fragment） |
| `Expr` | 表达式首 token | 裸条件/赋值/动作表达式 |
| `UiNode` | `stack` / `parallel` / `"..."` | 裸 UI 节点 |
| `Placeholder` | `...` | 占位符 |

**注意**：v0.3.1 中 `rule` 不再是独立 Fragment（它是前置约束修饰符）。

---

## 3. 关键字

全部小写，不能用作标识符。

```
module  type    flow    func    steps
requires ensures math    if      else    for
while   >>>     desc    on      with    error
and     or      not     in      done    exit
stack   parallel binds
parasteps
rule
true    false
```

### 3.1 跨文件指令 @import

`@import` 是唯一允许出现在 `.mms` 文件顶部的文件级指令，位于所有 Fragment 之前。

```mimispec
@import "path/to/other.mms"
```

- 可声明多次，每行一个
- 路径为字符串字面量，解析器只校验语法格式，不解析实际文件
- 被引用文件中的定义（`type`、`func` 等）在当前文件中可直接用标识符访问，无需前缀
- 链接与冲突消解由 IDE 或编译工具在后续阶段完成

### 3.2 意图后缀（锁定 + 不确定）

任何关键字、标识符或字符串末尾可紧接 **锁定后缀** 和/或 **不确定后缀**（无空格）。顺序固定为：**先锁定，后不确定**。

**锁定后缀**：

| 标记 | 含义 |
|------|------|
| `$` | 设计锁定：该节点内容已被确认，AI 不得修改 |
| `$$` | 强锁定：需要人类显式解锁才能改动 |

**不确定后缀**：作用对象是其前面的锁定后缀；若无锁定后缀，则作用于节点本身。

| 标记 | 含义 |
|------|------|
| `?` | 不确定 / 请求再审视 |
| `??` | 完全委托 |

**组合后的完整语义**：

| 后缀 | 锁定 | 对锁定/节点的态度 | 说明 |
|------|------|------------------|------|
| （无） | 无 | 正常协作 | 人类写的，AI 可优化 |
| `?` | 无 | 不确定 | 给我几个选项 |
| `??` | 无 | 完全委托 | 我不知道，你全权决定 |
| `$` | 锁定 | 确信 | 人类确认，AI 不得修改 |
| `$$` | 强锁定 | 确信 | 需要人类显式解锁才能改动 |
| `$?` | 锁定 | 不确定 | 我想锁定，但 AI 帮我再看看是否该锁 |
| `$??` | 锁定 | 委托评估 | 内容保持受保护；AI 评估普通锁成熟度并转入人类审阅 |
| `$$?` | 强锁定 | 不确定 | 我想强锁，但 AI 帮我再审视 |
| `$$??` | 强锁定 | 委托评估 | 内容保持受保护；AI 评估强锁成熟度并请求人类确认 |

**顺序规则**：锁定后缀必须在不确定后缀之前。`?$` / `?$$` / `??$` / `??$$` 等顺序非法。

> 示例：
> ```mimispec
> rule$ "支付必须幂等"      # AI 不得修改这条 rule
> rule$? "支付必须幂等"     # 用户想锁定，但请 AI 再审视锁定是否合理
> rule$?? "支付必须幂等"    # 内容受保护；AI 评估普通锁成熟度，不能自行解锁
> ```

Tooling 不是独立授权主体。formatter、CLI、IDE 和迁移器执行协作转移时，必须
明确代理 Human 或 AI，并遵守对应权限。所有包含 `$` 的状态保护当前内容；附着
到实体的前置 `rule` 组属于该实体的结构边界。

---

## 4. 词法规则

### 4.1 标识符

- 以字母或下划线开头，后续可接字母、数字、下划线
- 关键字不可作为标识符
- 支持意图后缀：`ident?`、`ident??`、`ident$`、`ident$$`、`ident$?`、`ident$??`、`ident$$?`、`ident$$??`

### 4.2 字符串字面量

以双引号 `"` 包围的纯文本：

```mimispec
"普通文本"
"包含空格的文本"
"包含 '单引号' 的文本"
```

**重要限制**：
- 字符串**不允许隐式跨行**。未闭合的引号若遇到换行，解析器会立即报 `unterminated string` 错误
- 如需在字符串中表示物理换行，使用转义序列 `\n`

**支持的转义序列**：

| 转义 | 含义 |
|------|------|
| `\\` | 反斜杠 |
| `\"` | 双引号 |
| `\n` | 换行符 |
| `\t` | 制表符 |
| `\r` | 回车符 |

### 4.3 缩进

MimiSpec 使用**基于缩进的块结构**：

- 缩进单位必须是 **4 个空格** 的整数倍（4、8、12...）
- **禁止使用 Tab**
- 同级语句必须对齐在同一缩进层级
- 子块相对父块缩进 4 个空格

### 4.4 注释

单行注释以 `//` 开头，到行尾结束。

```mimispec
// 这是注释
func Pay(): // 行尾注释也可以
    steps:
        charge payment
```

---

## 5. Fragment 规范

### 5.1 module（模块容器）

```mimispec
module ModuleName:
    Entity₁
    Entity₂
    ...
```

- `module` 引导块**必须带冒号**
- 模块内部可以包含任意实体：`Fragment`、`desc`、以及被 `rule` 前置修饰的任何实体
- `rule` 链可被空行或独占一行的 `//` 注释阻断；阻断后未被接收的 `rule` 变为 `module` 级约束
- 模块 body 中首个 `desc` 作为模块描述；后续 `desc` 会解析为包含单个 Desc step 的 `Steps` Fragment

**示例**：
```mimispec
module Shop:
    desc "订单管理模块，处理下单、支付、退款"

    type OrderStatus: New | Pending | Paid

    rule "支付必须幂等"
    func Pay(order, amount):
        desc "处理支付流程"
        steps:
            desc "检查买家余额"
            desc "调用支付网关"
            desc "更新订单状态"

    module Payment:
        func Refund(order):
            desc "处理退款流程"
            steps:
                desc "验证退款条件"
                desc "恢复库存"
                desc "发起退款"
```

### 5.2 type（类型定义）

`type` 统一承担枚举和记录两种定义。解析器根据内容自动推断：出现 `|` 分隔符即为枚举，出现缩进字段列表即为记录。

**枚举**支持两种形式：

单行（变体 ≤4 时推荐）：
```mimispec
type TypeName: VariantA | VariantB | VariantC
```

多行缩进块（变体 >4 时推荐，v0.4+）：
```mimispec
type TypeName:
    | VariantA
    | VariantB
    | VariantC
    | VariantD
    | VariantE
```

也支持无 `|` 前缀的裸标识符形式：
```mimispec
type TypeName:
    VariantA
    VariantB
    VariantC
```

解析器自动判断：缩进块内出现 `|` 或裸标识符（无 `:`）→ 枚举；出现 `field: Type` → 记录。块内也允许一行多个式 `A | B | C`。

**记录**：
```mimispec
type RecordName:
    desc "记录描述"
    field1: TypeHint
    field2: TypeHint
```

字段类型提示可为任意标识符，如 `u64`、`list<...>`、`string` 等，仅用于 AI 上下文，不强制实现细节。

**type body 中的实体**：
- `desc "..."` — 类型级自然语言描述实体；仅首个 `desc` 生效，后续 `desc` 被忽略
- `field: TypeHint` — 字段定义实体
- `...` — 占位符
- `rule "..."` 前置 — 约束该类型（字段级 `rule` 前置已实现，会附着给下一个 field）

### 5.3 rule（约束声明）

```mimispec
rule "自然语言约束描述"
```

- **无标签**，内容就是约束本身
- 字符串支持模糊标记：`"..."?` 或 `"..."??`
- 必须写在实体前面，作为前置附着修饰符
- 连续多条 `rule` 收集为约束列表，全部附着给下一个实体；普通换行分隔仍视为连续
- 空行或独占一行的 `//` 注释阻断附着链，未附着的 `rule` 变为当前层级全局约束
- 阅读器可将紧邻 rule 链上方的 `//` 注释作为分组标题显示，但注释本身不参与语法

**示例**：
```mimispec
// 全局约束
rule "系统必须支持幂等操作"
rule "所有转换必须有日志"

// module 级约束
rule "模块必须可回滚"
module Shop:
    // 本模块约束
    rule "金额不能为负数"
    rule "订单金额必须大于 0"

    type Order:
        desc "订单数据"
        amount: Money

    // func 级约束
    rule "支付必须幂等"
    rule "退款必须在有效期内"
    func Pay:
        // func body 内约束
        rule "扣款前必须校验余额"
        rule "失败必须记录日志"

        steps:
            check balance
            charge payment
```

### 5.4 flow（状态流）

```mimispec
flow FlowName:
    StateA >>> StateB: desc "说明"
    StateB:
        >>> StateC: desc "..."
```

- `FlowName` 可与某个 `type` 枚举名对应，表明该状态机的作用域
- `>>>` 表示单向转移
- 若一个状态有多个出边，必须用缩进列表表达
- flow entry block 内可以包含 `desc` 实体和 `rule` 前置约束

### 5.5 func（函数）

```mimispec
func 函数名[(参数列表)] [with 能力列表]:
    Entity₁
    Entity₂
    ...
```

- 参数列表 `(参数列表)` 为可选项。无参数时可省略括号
- `with` 能力声明为未来编译期能力检查预留接口，当前可忽略
- func body 可以包含的实体：`desc`（首个为函数描述，后续转为 Desc step）、`requires`、`ensures`、`steps`、`...`，以及**直接写在 body 中的裸 step**（如 `if`、`for`、`while`、动作步骤等）
- func body 内的实体可以被 `rule` 前置约束；`rule` 链被空行或独占一行的 `//` 注释阻断后，变为 `func` 级约束

#### 前置/后置条件 (`requires` / `ensures`)

`requires` 和 `ensures` 是 func body 中的**独立实体**。

支持两种模式，通过首字符自动识别：

- **结构化表达式**：不以 `"` 开头，使用字段、比较符和逻辑连接词
  - 支持：`==`, `!=`, `<`, `>`, `<=`, `>=`, `and`, `or`, `not`, `in`
  - 示例：`requires: order.status == Pending and amount > 0`
  - `in` 右侧支持列表字面量：`requires: status in [Pending, Paid]`
- **自然语言字符串**：以 `"` 开头，内部为纯文本，不允许隐式跨行（未闭合引号遇到换行会报错）
  - 示例：`ensures: "payment captured or error"`

#### 步骤块 (`steps`)

步骤块通过缩进包含一系列控制流和动作标签。每个步骤由一行标签表示，可附加 `desc`、`to` 和 `on`。

**基础步骤**：
```mimispec
steps:
    validate input desc "check mandatory fields"
    process data
    return result >>> done
```

**步骤标签中的关键字转义**：

当步骤标签中出现 `on`、`desc`、`error` 等关键字时，可用双引号将冲突词或整句转为字面量：

```mimispec
steps:
    "operate on data"              # 整句引号，避让 on
    check "for" updates            # 单次引号，避让 for
    parse optional "desc"          # 单次引号，避让 desc
```

字符串 `Atom` 是 MimiSpec 标准类型，无需语法扩展。仅在冲突处使用，其余处保持裸标识符。

**`desc` 作为独立步骤**：
```mimispec
steps:
    desc "检查买家余额是否充足"     # ← desc 是独立步骤实体
    desc "调用支付网关扣款"
    desc "把订单状态改为已支付"
```

`desc` 步骤的语义："这里需要一步，具体实现交给 AI，意图是这个描述。"

**流程转移 (`>>>`)**：
在步骤末尾使用 `>>> 目标`，目标可以是状态名、`done`（正常结束）或 `exit`（异常退出）。

```mimispec
order.status = Paid >>> done
mark order Shipped >>> done
```

**赋值 (`=`)**：
动作步骤可使用 `=` 对字段或变量赋值。语法：`target = value [>>> 目标]`。
- 右侧必须是**简单值**：标识符、字符串、数字、`true`/`false`、列表字面量
- 不允许多重赋值或计算表达式

```mimispec
order.status = Paid >>> done
tags = [Urgent, Internal]
```

**控制流**：
- `if 条件:` + 缩进块，可选 `else:` 分支
- `for 条件:` + 缩进块（如 `for each item in order.items`）
- `while 条件:` + 缩进块，建议附带 `desc` 说明终止条件（静态检查会提示，但 parser 层面不强制）

**异常回滚 (`on`)**：
紧接在某步骤后（同级缩进），表示该步骤失败时的补偿逻辑。语法：`on 条件:` + 缩进块，块内语法与 `steps` 相同。支持多个 `on` 分支（如 `on timeout:`、`on lock error:`）。

**错误终止 (`error`)**：
`error "消息" [>>> exit]` 表示显式失败，终止当前路径。`>>> exit` 为可选；若省略，仅终止当前分支而不显式标记为异常退出。

#### 完整示例

```mimispec
func Pay(order, amount) with PaymentCap?:
    desc "处理支付：检查余额、扣款、改状态"
    requires: order.status == Pending and amount == order.total
    ensures: order.status == Paid or (order.status == Pending and no side effects)
    steps:
        check funds desc "verify account balance"
        if insufficient:
            error "insufficient funds" >>> exit
        reserve inventory:
            for item in order.items:
                Inventory.reserve(item)
            on reserve failure:
                release reserved items desc "rollback"
                error "inventory error" >>> exit
        charge payment desc "call PSP"
        on charge failure:
            release inventory desc "compensate"
            error "payment failed" >>> exit
        order.status = Paid >>> done
```

### 5.6 ui（视图）

```mimispec
ui ViewName binds Model:
    stack "根容器":
        "文本" desc "说明"
```

`binds` 表示该视图绑定的数据模型。UI 节点支持：
- `stack`（纵向堆叠）和 `parallel`（横向排列）两种布局容器
- 字符串字面量叶子节点
- `error "消息" [desc "说明"]` 错误节点

布局容器 `stack` / `parallel` 后可跟可选的字符串描述标签。

**事件绑定**：
UI 元素可通过 `on <事件>:` 绑定动作，事件名由标识符或字符串字面量表示（如 `tap`、`click`）：

```mimispec
"支付按钮" desc "主操作" on tap: Pay(order)
"重试" on "long-press": Retry()
```

### 5.7 steps（步骤流）

`steps` 可作为独立的顶层 Fragment。

```mimispec
steps:
    步骤₁
    步骤₂
    ...
```

**语义**：一个无函数签名的纯步骤流。AI 或工具层可以：
- 将其视为匿名过程
- 将其内联到某个 `func` 的 `steps` 中
- 将其作为对话中的"下一步行动"执行

**示例**：
```mimispec
steps:
    validate order desc "检查必填字段"
    if order.total > 1000:
        request approval
    charge payment
    on failure:
        log error
        retry
    order.status = Paid
```

### 5.8 parasteps（并行步骤）

`parasteps` 表示时间上并行执行的步骤块。与 UI 中的 `parallel`（空间横向排列）形成对称。

`parasteps` 内部的动作步骤会并行执行，全部完成后才继续后续步骤。

```mimispec
func LoadDashboard:
    steps:
        parasteps "同时请求多个数据源":
            loadUsers desc "获取用户数据"
            loadOrders desc "获取订单数据"
            loadMetrics desc "获取统计指标"
        combine results >>> done
```

- `parasteps` 后可跟一个可选的字符串标签（如 `"同时请求多个数据源"`），用于说明并行块的意图
- 并行执行内部所有步骤，等待全部完成后执行后续步骤

### 5.9 错误与异常处理

#### error 终止

`error` 终止当前路径，配合 `>>> exit` 表示到达预定义终点（流程失败退出）。

```mimispec
if stock.available < item.qty:
    error "库存不足" >>> exit
```

`>>> exit` 是预定义终点，表示"这道流程做不下去了，直接退出"。

#### on 补偿块

`on` 块紧接在可能失败的步骤之后，用于定义补偿逻辑（回滚/重试/通知）。

```mimispec
charge payment
on gateway error:
    log failure
    error "支付失败" >>> exit
```

```mimispec
steps:
    validate order
    on failure:
        log error
        retry
    order.status = Paid
```

`on` 块与可能失败的步骤同级缩进，表示该步骤失败时的处理预案。

### 5.10 Expr（表达式）

任何条件表达式、赋值表达式或动作表达式都可以作为独立的顶层 Fragment。

```mimispec
order.status == Pending and amount > 0
```

```mimispec
user.role == Admin or user.trustScore >= 80
```

```mimispec
order.status = Paid
```

```mimispec
charge payment desc "调用支付网关"
```

**列表字面量**：
`in` 右侧可以是普通标识符，也可以是列表字面量：

```mimispec
requires: status in [Pending, Paid]
```

列表项限定为简单表达式（标识符、字符串、数字、`true`/`false`）。

**语义**：单行的意图表达。工具层可以：
- 将其作为 `requires` / `ensures` 的候选条件
- 将其作为 `func` 步骤中的某一行
- 将其作为 AI 对话中的"确认点"

### 5.11 UiNode（UI 节点）

任何 UI 节点都可以作为独立的顶层 Fragment。

```mimispec
"支付按钮" desc "主操作" on tap: Pay(order)
```

```mimispec
stack "工具栏":
    "全部" desc "过滤"
    "进行中" desc "过滤"
```

**语义**：独立的 UI 元素描述。工具层可以：
- 将其嵌入到某个 `ui` 视图中
- 将其作为设计系统中的组件复用

### 5.12 Desc（自然语言实体）

`desc "..."` 是**独立的实体**，不是修饰符。它可以出现在任何 block 中，作为自然语言占位。

**在 `module`、`type`、`func` body 中的行为**：
- 首个 `desc` 通常作为容器自身的描述（`module.desc`、`type.desc`、`func.desc`）
- 同一容器内出现多个 `desc` 时，后续 `desc` 在 `func` 中变为 `Desc` step；在 `module` 中变为包含单个 `Desc` step 的 `Steps` Fragment；在 `type` 记录体中则被忽略

**在 `steps` 块中的行为**：
`desc` 是独立步骤实体：

```mimispec
func Pay:
    desc "处理支付：检查余额、扣款、改状态"     # func 描述
    steps:
        desc "检查买家余额是否充足"               # Desc step
        desc "调用支付网关扣款"                   # Desc step
        desc "把订单状态改为已支付"               # Desc step
```

**顶层 `desc`**：
文件顶层的 `desc "..."` 实际解析为 `Fragment::Steps { keyword_commitment, steps: [Step::Desc] }`，不是独立的 `Fragment::Desc`。

**顶层 `...`**：
独立的 `...` 解析为 `Fragment::Placeholder { keyword_commitment }`。

**`desc` 与 `...` 的关系**：
- `...` = "我不知道这里要什么"
- `desc "检查余额"` = "我知道这里要一个检查余额的东西，但具体实现交给 AI"

---

### 5.13 math（数学块）

`math:` 是一个**结构化数学表达式块**，用于精确锁定数值、张量、位运算关系与推导。它出现在 `func`、`module`、`type` body 中，与 `requires:` / `ensures:` / `steps:` 平级。

```mimispec
func CrossAttention(query, key, value):
    math:
        d_k = dim(key, -1)
        scores = query @ key.T / sqrt(d_k)
        weights = softmax(scores, -1)
        context = weights @ value
```

#### 语法

```mimispec
math:
    语句₁
    语句₂
    ...
```

每行是一个数学语句，支持两种形式：

- **定义式**：`target = expr`
- **表达式/约束**：`expr`（如等式、不等式、函数调用）

#### 支持的运算符（按优先级从低到高）

| 优先级 | 运算符 | 说明 |
|--------|--------|------|
| 1 | `or` | 逻辑或 |
| 2 | `and` | 逻辑与 |
| 3 | `== != < > <= >= in` | 比较与成员关系 |
| 4 | `\|` | 按位或 |
| 5 | `^` | 按位异或 |
| 6 | `&` | 按位与 |
| 7 | `<< >>` | 左移 / 右移 |
| 8 | `+ -` | 加 / 减 |
| 9 | `* / @` | 乘 / 除 / 矩阵乘法 |
| 10 | `**` | 幂（右结合） |
| 11 | `-` `~` `not` | 一元负、按位取反、逻辑非 |

#### 数字字面量

math 块中的数字支持整数、小数和科学计数法：

```mimispec
math:
    a = 42
    b = 3.14
    c = 1e-4
    d = 1.5e+3
```

#### 标量算术

```mimispec
math:
    a + b
    a - b
    a * b
    a / b
    a ** b
    -a
```

#### 比较与逻辑

```mimispec
math:
    a == b
    a != b
    a > b
    a >= b
    a < b
    a <= b
    a and b
    a or b
    not a
    a in [1, 2, 3]
```

#### 位运算

```mimispec
math:
    a & b
    a | b
    a ^ b
    ~a
    a << n
    a >> n
```

#### 张量 / 线性代数

```mimispec
math:
    C = A @ B          # 矩阵乘法
    B = A.T            # 转置
    d = dim(x, -1)     # 取维度大小
    s = shape(x)       # 完整形状
    s0 = shape(x, 0)   # 第 0 维大小
    v = x[i]           # 一维索引
    v = x[i, j]        # 多维索引
    v = x[-1]          # 负数索引
```

#### 常用函数

```mimispec
math:
    sqrt(x)
    abs(x)
    exp(x)
    log(x)
    sum(x)
    sum(x, -1)
    mean(x)
    max(x)
    min(x)
    argmax(x, -1)
    argmin(x, -1)
    prod(x)
    variance(x)
    std(x)
    median(x)
    percentile(x, 0.9)
    softmax(x, -1)
    dot(a, b)
    norm(x, 2)
```

#### 微积分

```mimispec
math:
    grad(f, x)
    derivative(f, x)
    partial(f, x)
    jacobian(f, x)
    hessian(f, x)
    integral(f, x, a, b)
```

#### 使用 `math:` 的场景

**形状约束（替代自然语言）**：

```mimispec
# 之前
requires: Q.last_dim == num_heads multiplied by head_dim

# 之后
requires: dim(Q, -1) == num_heads * head_dim
```

**函数内推导**：

```mimispec
func CrossAttention(query, key, value):
    requires: query.dim == 2 and key.dim == 2 and value.dim == 2
    math:
        d_k = dim(key, -1)
        scores = query @ key.T / sqrt(d_k)
        weights = softmax(scores, -1)
        context = weights @ value
        context.shape == [query.shape[0], dim(value, -1)]
```

**模块级不变量**：

```mimispec
module Physics:
    math:
        E = m * c ** 2
```

**类型级约束**：

```mimispec
type Rectangle:
    width: Number
    height: Number
    math:
        area == width * height
```

#### 与 `desc` 的关系

`math:` 负责**可解析、可静态检查**的精确数学意图；`desc "..."` 负责人类可读的自然语言说明（可包含 LaTeX）。两者互补：

```mimispec
func CrossAttention(query, key, value):
    desc "标准的 scaled dot-product attention"
    math:
        scores = query @ key.T / sqrt(dim(key, -1))
```

#### 注意

- `math:` 内不使用 LaTeX；需要排版公式时请用 `desc`
- `=` 在 `math:` 内表示**定义/等式**，`==` 表示**相等比较**
- `|` 在 `type` 枚举体中表示变体分隔，在 `math:` 内表示按位或，由上下文区分

---

## 6. 聚合语法

### 6.1 `...` 占位符

表示"这里的内容待填充"。

```mimispec
func Pay(order, amount):
    requires: ...
    steps:
        check funds
        ...
        order.status = Paid >>> done
```

`...` 可以出现在：
- `requires` / `ensures` 的值位置
- `steps` 块中的任意步骤位置
- `module` 块中的任意位置
- `type` body 中的任意位置
- `func` body 中的任意位置
- 顶层 Fragment 位置

### 6.2 `@import` 跨文件引用

碎片文件可以通过 `@import` 建立跨文件依赖，不破坏碎片的独立性。

```mimispec
@import "common/types.mms"

module UserDomain:
    func GetUser(id):
        requires: id > 0
        steps:
            query database
            return user >>> done
```

`@import` 表示"引用某个在别处定义的文件"。即使被引用的文件不存在于当前目录，该文件仍然是**合法但不可链接**的（parseable but not linkable）。

---

## 7. 模块与引用

MimiSpec v0.3.1 以**单文件优先**为设计原则：同一文件内通过命名空间直接引用，无需前缀。若需引用其他 `.mms` 文件中的定义，使用文件顶部的 `@import` 指令显式声明。

### 7.1 模块内引用

同一文件内，直接使用标识符引用：

```mimispec
module Shop:
    type OrderStatus: New | Paid

    func Pay(order):
        requires: order.status in [New]
        steps:
            order.status = Paid
```

### 7.2 嵌套模块

```mimispec
module Shop:
    module Payment:
        func Pay(): ...

    module Shipping:
        func Ship(): ...
```

跨嵌套模块引用使用点号路径：`Shop.Payment.Pay`

### 7.3 跨文件引用

通过 `@import` 引入其他 `.mms` 文件后，当前文件可直接使用被引入文件中的定义：

```mimispec
// utils.mms
func ValidateEmail(email):
    requires: email != ""
    steps:
        check format >>> done
```

```mimispec
// user.mms
@import "utils.mms"

module UserDomain:
    func Register(email):
        steps:
            ValidateEmail(email)
            persist user >>> done
```

跨文件引用保持**扁平命名空间**：被引入的标识符在当前文件中直接可见。若出现命名冲突，工具层应提示歧义，用户可通过限定路径（如 `ModuleName.ident`）解决。

---

## 8. 静态检查规则

1. **碎片合法性**：任何 Fragment 独立解析时是否合法
2. **`...` 密度**：文件中 `...` 比例过高时提示"请先完成核心结构"
3. **渐进式完整度**：IDE 可显示当前文件处于哪个聚合阶段（碎片/片段/模块/项目）
4. **引用可达性**：`@import` 引用的目标文件是否在链接阶段可解析
5. **状态覆盖**：`flow` 是否覆盖关联 `type` 的所有枚举值（通过命名约定关联）
6. **分支完备性**：`if` 是否缺少 `else` 或明确占位
7. **循环终止**：建议 `while` 附带 `desc` 说明终止条件（parser 层面不强制，静态检查可提示）
8. **契约一致性**：`func` 的 `ensures` 是否与内部 `error` 路径矛盾
9. **锁定与不确定密度**：`?`/`??`/`$`/`$$` 比例过高时提示澄清；`?$` 等非法顺序报错
10. **rule 悬空**：block 末尾的 `rule` 未被任何实体接收时提示

---

## 9. 解析器接口

### 9.1 AST 结构

```rust
/// 源文件根节点
pub struct File {
    pub imports: Vec<String>,
    pub rules: Vec<RuleDef>,        // 全局约束（未被附着的 rule）
    pub fragments: Vec<Fragment>,
}

pub enum Fragment {
    Module { module: Module },
    TypeDef { typedef: TypeDef },
    Flow { flow: FlowDef },
    Func { func: FuncDef },
    Ui { ui: UiDef },
    Steps {
        keyword_commitment: Commitment,
        steps: Vec<Step>,
    },
    Expr { expr: Expr },
    UiNode { node: UiNode },
    Placeholder { keyword_commitment: Commitment },
}

/// 约束声明（前置附着于下一个实体）
pub struct RuleDef {
    pub desc: Desc,
    pub keyword_commitment: Commitment,
    pub line: usize,
}

/// 描述实体
pub struct Desc {
    pub need_commitment: Commitment,
    pub content: FString,
}
```

### 9.2 解析入口

```rust
/// 解析完整文件（提取 @import 并解析 Fragment 组合）
pub fn parse(source: &str) -> ParseResult

/// 解析单个 Fragment（用于 IDE 片段验证）
pub fn parse_fragment(source: &str) -> ParseResult

/// 词法分析
pub fn tokenize(source: &str) -> Result<Vec<Token>, ParseError>
```

---

## 10. 关键字速查表

| 类别 | 关键字 |
|------|--------|
| 容器 | `module` |
| 结构 | `type`, `flow`, `func`, `steps`, `ui` |
| 实体 | `desc` |
| 约束 | `rule` |
| 规则/条件 | `requires`, `ensures` |
| 数学 | `math` |
| 控制流 | `if`, `else`, `for`, `while`, `on` |
| 布局 | `stack`, `parallel`, `binds` |
| 转移/终点 | `>>>`, `done`, `exit` |
| 能力 | `with` |
| 错误 | `error` |
| 逻辑 | `and`, `or`, `not`, `in` |
| 布尔 | `true`, `false` |
| 文件指令 | `@import` |
| 聚合 | `...` |
| 并行 | `parasteps` |
| 意图/锁定 | `?`, `??`, `$`, `$$` （后缀；可组合为 `$?`, `$$?`, `$??`, `$$??`） |

---

## 11. 示例：从零到完整的渐进式蓝图

### 阶段 1 —— 纯意图（desc-only）

```mimispec
// shop.mms —— 阶段 1：完全委托 AI
module?? Shop:
    type?? Order:
        desc?? "订单数据，包含买家、商品、金额和状态"

    rule?? "支付不能重复扣款"
    rule?? "取消订单后库存必须恢复"

    func?? Pay:
        desc?? "处理支付：检查余额、扣款、改状态"
        steps:
            desc?? "检查买家余额是否充足"
            desc?? "调用支付网关扣款"
            desc?? "把订单状态改为已支付"

    func?? Refund:
        desc?? "处理退款：验证条件、恢复库存、退款"
        steps:
            desc?? "验证退款条件"
            desc?? "恢复库存"
            desc?? "发起退款"
```

### 阶段 2 —— 部分结构化

```mimispec
// shop.mms —— 阶段 2：加入类型和约束
module Shop:
    desc "订单管理模块"

    type OrderStatus: New | Pending | Paid | Shipped | Cancelled

    rule "支付必须幂等"
    rule "库存不能为负"

    func Pay(order, amount):
        desc "处理支付流程"
        requires: order.status == Pending
        steps:
            check balance desc "检查余额"
            charge payment desc "调用支付网关"
            order.status = Paid >>> done

    func Refund(order):
        desc "处理退款流程"
        requires: order.status in [Paid, Shipped]
        steps:
            validate conditions
            restore inventory
            initiate refund >>> done
```

### 阶段 3 —— 完整模块

```mimispec
// shop.mms —— 阶段 3：完整蓝图
module OrderDomain:
    desc "订单域：管理订单生命周期"

    type OrderStatus: New | Pending | Paid | Shipped | Cancelled

    rule "支付幂等"
    rule "取消后库存必须恢复"
    rule "退款必须在有效期内"

    flow OrderLifecycle:
        New >>> Pending: desc "客户提交"
        Pending:
            >>> Paid: desc "支付成功"
            >>> Cancelled: desc "客户取消"
        Paid >>> Shipped: desc "已发货"
        Shipped >>> Delivered: desc "已送达"

    func ProcessOrder(order):
        desc "处理订单"
        requires: order.status == New
        ensures: order.status in [Paid, Cancelled]
        steps:
            check inventory
            if stock < order.qty:
                error "out of stock"
            charge payment
            on failure:
                refund
                error "payment failed" >>> exit
            order.status = Paid >>> done

    func CancelOrder(order):
        desc "取消订单"
        requires: order.status in [New, Pending]
        steps:
            restore inventory
            order.status = Cancelled >>> done

    ui OrderPanel binds order:
        stack "订单面板":
            "订单 #order.id" desc "标题"
            parallel "操作栏":
                "支付" desc "按钮" on tap: ProcessOrder(order)
                "取消" desc "按钮" on tap: CancelOrder(order)
```

### 阶段 4 —— 多模块单文件

```mimispec
// shop.mms —— 阶段 4（单文件完整项目）
module OrderDomain:
    type OrderStatus: New | Pending | Paid | Shipped | Cancelled

    func ProcessOrder(order):
        requires: order.status == New
        ensures: order.status in [Paid, Cancelled]
        steps:
            check inventory
            if stock < order.qty:
                error "out of stock"
            charge payment
            on failure:
                refund
                error "payment failed" >>> exit
            order.status = Paid >>> done

module PaymentDomain:
    func Pay(order, amount):
        requires: order.status == Pending
        ensures: order.status == Paid
        steps:
            verify funds
            process transaction
            order.status = Paid >>> done

module ShippingDomain:
    func Ship(order):
        requires: order.status == Paid
        steps:
            allocate warehouse
            dispatch courier
            order.status = Shipped >>> done
```

### 阶段 5 —— 跨文件引用

```mimispec
// types.mms
type OrderStatus: New | Pending | Paid | Shipped | Cancelled

type Order:
    id: u64
    status: OrderStatus
```

```mimispec
// shop.mms
@import "types.mms"

module Shop:
    func ProcessOrder(order):
        requires: order.status == New
        ensures: order.status in [Paid, Cancelled]
        steps:
            check inventory
            if stock < order.qty:
                error "out of stock"
            charge payment
            on failure:
                refund
                error "payment failed" >>> exit
            order.status = Paid >>> done
```

**关键洞察**：阶段 1 到阶段 5 的 `.mms` 文件，每一阶段都是合法的。解析器不因"不完整"而拒绝，只因"语法错误"而拒绝。`@import` 使得碎片可以跨文件聚合，而解析器本身不强制链接，保持了"合法但不可链接"的渐进式特性。

---

## 附录：v0.3 → v0.3.1 变更摘要

| 变更项 | v0.3 | v0.3.1 |
|--------|------|--------|
| `desc` 语义 | 修饰符（后缀） | **实体占位**（独立元素） |
| `rule` 语法 | `rule Label: "描述"` | `rule "描述"`（**无标签**） |
| `rule` 语义 | 独立 Fragment | **前置约束修饰符** |
| `rule` 附着 | 无 | 连续 rule 收集，附着于下一个实体；空行阻断 |
| `type` body | fields / enum only | 支持 `desc` 实体和 `rule` 前置 |
| `func` body | requires/ensures/steps | 支持 `desc` 实体和 `rule` 前置 |
| `steps` | action/assign/control flow | 支持 `desc` 作为独立步骤 |
| `Fragment::Rule` | 存在 | **删除**（rule 不再是 Fragment） |
| `File` | imports + fragments | imports + **rules** + fragments |

---

## 附录：v0.3.1 → v1.0.0-rc.1 变更摘要

| 变更项 | v0.3.1 | v1.0.0-rc.1 |
|--------|--------|-------------|
| 后缀维度 | 仅不确定后缀 `?` / `??` | 增加锁定后缀 `$` / `$$`，形成二维组合后缀 |
| 后缀顺序 | `?` / `??` 直接附加 | 锁定必须在不确定之前：`$?` / `$$?` / `$??` / `$$??` 合法；`?$` / `?$$` 等非法 |
| `$` / `$$` | 不存在 | 设计锁定 / 强锁定；AI 不得删除锁，但可在锁后追加 `?` / `??` |
| 静态检查 | 模糊密度 | 锁定与不确定密度、非法后缀顺序检查 |
