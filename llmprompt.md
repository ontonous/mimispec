# LLM Prompt：让 AI 正确书写 MimiSpec v1.0.0-rc.1

> **目标**：你正在阅读的是 MimiSpec（`.mms`）的 LLM 写作指南。读完本指南后，你应该能够独立生成**语法合法、结构清晰、意图明确**的 MimiSpec 文件。
>
> **版本**：基于 `mimispec_v0.3.1_v1.0.0-rc.1.md` 与 Rust 解析器实现。
> **输出要求**：只输出 `.mms` 源码文本，不要用 Markdown 代码块包裹。

---

## 1. 核心哲学：From Scratch to Full

MimiSpec 是一门**高信息密度的意图描述语言**，不是编程语言。

- **人定方向，AI 填实现**：只写"要什么"，不写"怎么做"。
- **任何 Fragment 都可以独立合法**：从一行 `type` 到完整 `module`，从单个 `if` 到完整 `func`，都是合法文件。
- **缩进即结构**：4 个空格一级，禁止 Tab。
- **模糊是合法的**：用 `?` / `??` 标记不确定，用 `$` / `$$` 标记锁定。

---

## 2. 绝对规则（Parser 硬约束）

违反以下任何一条，文件都会解析失败。

### 2.1 缩进

- 必须是 **4 个空格的整数倍**（4、8、12...）。
- **禁止使用 Tab**。
- 子块必须比父块多 4 个空格。

```mimispec
module Shop:                    // 0 缩进
    type OrderStatus: New | Paid // 4 缩进

    func Pay(order):            // 4 缩进
        requires: order.status == New  // 8 缩进
        steps:                  // 8 缩进
            charge payment      // 12 缩进
```

### 2.2 字符串

- 字符串用双引号 `"..."`。
- **禁止隐式跨行**：未闭合的引号遇到换行会报 `unterminated string`。
- 需要物理换行时用转义 `\n`。
- 支持转义：`\\`、`\"`、`\n`、`\t`、`\r`。

```mimispec
// ❌ 错误：字符串跨行
func Pay:
    desc "处理支付：
          检查余额、扣款、改状态"

// ✅ 正确
desc "处理支付：\n检查余额、扣款、改状态"
```

### 2.3 关键字不可用作标识符

```
module  type    flow    func    steps
requires ensures         if      else    for
while   to      desc    on      with    error
and     or      not     in      done    exit
stack   parallel binds
parasteps
rule
true    false
```

### 2.4 冒号规则

以下结构**必须**在名字/签名后加 `:`：

- `module Name:`
- `type Name:` 或 `type Name: VariantA | VariantB`
- `flow Name:`
- `func Name(...):` 或 `func Name:`
- `ui Name binds Model:`
- `steps:`、`if cond:`、`else:`、`for var in iterable:`、`while cond:`、`parasteps "label":`
- `requires:`、`ensures:`

```mimispec
// ❌ 错误：module 忘冒号
module Shop
    func Pay:

// ✅ 正确
module Shop:
    func Pay:
```

### 2.5 意图后缀顺序

后缀可以加在关键字、标识符、字符串末尾，**无空格**。

- 合法：`?`、`??`、`$`、`$$`、`$?`、`$??`、`$$?`、`$$??`
- **顺序固定：先锁定，后不确定**。
- 非法：`?$`、`?$$`、`??$`、`??$$`

```mimispec
func$? Pay(order?)     // ✅ 想锁定函数，但请 AI 审视
rule$ "必须幂等"        // ✅ 锁定这条规则
module?? Shop:          // ✅ 完全委托模块设计
```

---

## 3. 关键区分：`desc` 是实体，`rule` 是修饰符

这是最容易混淆的点。

| | `desc "..."` | `rule "..."` |
|---|---|---|
| 角色 | **独立实体** | **前置约束修饰符** |
| 能否单独存在 | ✅ 能 | ❌ 不能，必须附着于下一个实体 |
| 语义 | "这里需要一个东西，意图是..." | "紧随其后的实体必须满足..." |
| 位置 | block 中任意位置 | 目标实体前面 |

```mimispec
// ✅ desc 作为 func 描述（func 内首个 desc）
func Pay:
    desc "处理支付流程"
    steps:
        desc "检查余额"       // ✅ desc 作为独立步骤

// ✅ rule 前置约束，附着于 func Pay
rule "支付必须幂等"
func Pay:
    steps:
        check balance
```

**`rule` 附着规则**：

1. 连续多条 `rule` 收集为列表，全部附着给**下一个实体**。
2. **空行阻断**附着链；未被接收的 `rule` 变为当前层级全局约束。
3. `rule` 约束整个实体及其内部所有内容（递归约束）。

```mimispec
rule "系统级约束"

rule "模块级A"
rule "模块级B"
module Shop:                    // ← 接收 A+B
    rule "函数级"
    func Pay:                   // ← 接收"函数级"
        steps:
            check balance
```

---

## 4. Fragment 写作模板

### 4.1 `module` 模块

```mimispec
module Shop:
    desc "订单管理模块"

    type OrderStatus: New | Pending | Paid

    rule "支付必须幂等"
    func Pay(order):
        desc "处理支付"
        steps:
            check balance
            charge payment
            order.status = Paid to done
```

- 模块内首个 `desc` 是模块描述。
- 模块内后续 `desc` 会解析为包含单个 Desc step 的 `Steps` Fragment。
- 模块可嵌套，跨嵌套模块引用用点号路径：`Shop.Payment.Pay`。

### 4.2 `type` 类型

**枚举**（单行，`|` 分隔）：

```mimispec
type OrderStatus: New | Pending | Paid | Cancelled
```

**记录**（缩进块，字段 + 类型提示）：

```mimispec
type Order:
    desc "订单数据"
    id: u64
    buyer: string
    items: list<Item>
    status: OrderStatus
```

- 记录体内首个 `desc` 是类型描述，后续 `desc` 被忽略。
- 字段类型提示只是给 AI 的提示，不强制检查。

### 4.3 `func` 函数

```mimispec
func Pay(order, amount) with PaymentCap?:
    desc "处理支付：检查余额、扣款、改状态"
    requires: order.status == Pending and amount > 0
    ensures: order.status == Paid
    steps:
        check funds desc "检查余额"
        if insufficient:
            error "insufficient funds" to exit
        charge payment
        order.status = Paid to done
```

**函数体可包含的实体**：

- `desc`：首个为函数描述，后续转为 Desc step。
- `rule`：前置约束，附着于下一个实体。
- `requires:` / `ensures:`：契约条件。
- `steps:`：执行步骤块。
- 裸 step：`if` / `for` / `while` / 动作步骤等直接写在 func body 中（等价于在 steps 块里）。

**前置/后置条件两种写法**：

```mimispec
// 结构化表达式
requires: order.status == Pending and amount > 0
ensures: order.status in [Paid, Cancelled]

// 自然语言字符串
requires: "订单必须存在且可支付"
ensures: "支付完成或无副作用回滚"
```

### 4.4 `steps` 步骤块

```mimispec
steps:
    validate input desc "检查必填字段"
    if order.total > 1000:
        request approval
    charge payment
    on failure:
        log error
        retry
    order.status = Paid
```

**基础步骤**：`动作标签 [desc "..."] [to 目标]`

```mimispec
check funds desc "验证余额"
charge payment to done
```

**流程转移**：`to` 目标可以是 `done`（正常结束）、`exit`（异常退出）或自定义状态名。

```mimispec
order.status = Paid to done
error "支付失败" to exit
```

**赋值**：`target = simple_value [to 目标]`

- 右侧必须是**简单值**：标识符、字符串、数字、`true`/`false`、列表字面量。
- 不允许多重赋值或计算表达式。

```mimispec
order.status = Paid
tags = [Urgent, Internal]
```

**控制流**：

```mimispec
if stock.available < item.qty:
    error "库存不足" to exit
else:
    reserve inventory

for item in order.items:
    process item

while queue.notEmpty:
    process next desc "持续处理直到队列排空"
```

**异常回滚 `on`**：紧接在动作步骤后，同级缩进。

```mimispec
reserve inventory:
    for item in order.items:
        Inventory.reserve(item)
    on reserve failure:
        release reserved items desc "rollback"
        error "inventory error" to exit
```

**注意**：`on` 只能跟在**动作步骤**后，不能跟在 `if` / `for` / `while` / `error` / `=` 后。

### 4.5 `parasteps` 并行步骤

```mimispec
func LoadDashboard:
    steps:
        parasteps "同时请求多个数据源":
            loadUsers desc "获取用户数据"
            loadOrders desc "获取订单数据"
            loadMetrics desc "获取统计指标"
        combine results to done
```

### 4.6 `flow` 状态流

```mimispec
flow OrderLifecycle:
    New to Pending: desc "客户提交"
    Pending:
        to Paid: desc "支付成功"
        to Cancelled: desc "客户取消"
    Paid to Shipped: desc "已发货"
```

- 单个出边：`StateA to StateB: desc "..."`
- 多个出边：缩进块，每行 `to StateC: desc "..."`
- 转移可附加 `requires:` 守卫条件。

### 4.7 `ui` 视图

```mimispec
ui OrderPanel binds order:
    stack "订单面板":
        "订单 #order.id" desc "标题"
        parallel "操作栏":
            "支付" desc "按钮" on tap: ProcessOrder(order)
            "取消" desc "按钮" on tap: CancelOrder(order)
```

- `ui Name binds Model:`，`binds` 可选。
- 根节点**只能有一个**，必须是 `stack` 或 `parallel`。
- 子节点可以是：字符串叶子、`stack`、`parallel`、`error "消息"`。
- 事件绑定：`on 事件名: 动作`，事件名可以是标识符（`tap`）或字符串（`"long-press"`）。
- 动作可以是函数调用、赋值、导航（`to Ident`）或自然语言字符串。

### 4.8 顶层独立 Fragment

MimiSpec 支持不包在 module 里的顶层 Fragment：

```mimispec
type Status: Active | Inactive

func Toggle(status):
    steps:
        if status == Active:
            status = Inactive
        else:
            status = Active

order.status == Pending and amount > 0
```

---

## 5. 占位符与渐进式完整

### 5.1 `...` 占位符

`...` 表示"这里待填充"，可以出现在：

- `requires:` / `ensures:` 的值位置
- `steps` 块中的任意步骤位置
- `module` / `type` / `func` body 中
- 顶层 Fragment 位置

```mimispec
func Pay(order, amount):
    requires: ...
    steps:
        check funds
        ...
        order.status = Paid to done
```

### 5.2 `desc` vs `...`

- `...` = "我不知道这里要什么"
- `desc "检查余额"` = "我知道这里要检查余额，但具体实现交给 AI"

---

## 6. 跨文件引用

`@import` 必须写在文件顶部、所有 Fragment 之前。

```mimispec
@import "common/types.mms"
@import "utils/validators.mms"

module Shop:
    func ProcessOrder(order):
        requires: order.status == New
        steps:
            ValidateOrder(order)
            order.status = Paid to done
```

- 被引入文件中的定义在当前文件中**直接可见**，无需前缀。
- 命名冲突时用 `ModuleName.ident` 限定。
- 解析器只校验语法格式，不解析实际文件。

---

## 7. 常见错误清单

| 错误 | 正确 |
|---|---|
| `module Shop`（无冒号） | `module Shop:` |
| `rule R1: "..."`（带标签） | `rule "..."` |
| `desc 检查余额`（无引号） | `desc "检查余额"` |
| `action desc "说明"`（desc 当后缀） | `desc "说明"` 作为独立步骤 |
| `type Status:\n    Healthy\n    Degraded`（枚举多行） | `type Status: Healthy \| Degraded` |
| `requires: status ==\n    Pending`（条件换行） | `requires: status == Pending` 或 `requires: "status 为 Pending"` |
| `amount = price * 0.8`（计算表达式） | `apply discount desc "AI 计算折扣"` |
| `order.status = CalculateTax()`（函数调用赋值） | `calculate tax` 作为动作步骤 |
| `on` 跟在 `if` / `for` / `=` 后 | `on` 只能跟在动作步骤后 |
| Tab 缩进 | 4 个空格缩进 |
| 字符串未闭合跨行 | 使用 `\n` 或闭合引号 |
| `?$` / `?$$` 后缀顺序 | `$?` / `$$?` |

---

## 8. 完整示例

```mimispec
// shop.mms —— 订单域完整蓝图
module OrderDomain:
    desc "订单域：管理订单生命周期"

    type OrderStatus: New | Pending | Paid | Shipped | Cancelled

    type Order:
        id: u64
        buyer: string
        total: Money
        status: OrderStatus

    rule "支付必须幂等"
    rule "取消后库存必须恢复"
    rule "退款必须在有效期内"

    flow OrderLifecycle:
        New to Pending: desc "客户提交订单"
        Pending:
            to Paid: desc "支付成功"
            to Cancelled: desc "客户取消"
        Paid to Shipped: desc "已发货"

    func ProcessOrder(order):
        desc "处理新订单"
        requires: order.status == New
        ensures: order.status in [Paid, Cancelled]
        steps:
            check inventory
            if stock.available < order.total:
                error "库存不足" to exit
            reserve inventory:
                for item in order.items:
                    Inventory.reserve(item)
                on reserve failure:
                    release reserved items desc "rollback"
                    error "库存预留失败" to exit
            charge payment
            on failure:
                release inventory desc "compensate"
                error "支付失败" to exit
            order.status = Paid to done

    func CancelOrder(order):
        desc "取消订单"
        requires: order.status in [New, Pending]
        steps:
            restore inventory
            order.status = Cancelled to done

    ui OrderPanel binds order:
        stack "订单面板":
            "订单 #order.id" desc "标题"
            parallel "操作栏":
                "支付" desc "按钮" on tap: ProcessOrder(order)
                "取消" desc "按钮" on tap: CancelOrder(order)
```

---

## 9. 写作检查清单

生成 `.mms` 文件后，逐项检查：

- [ ] 所有 `module` / `type` / `flow` / `func` / `ui` 都带了 `:`。
- [ ] 缩进是 4 空格，没有 Tab。
- [ ] 所有字符串都已闭合，没有跨行。
- [ ] `rule` 没有标签，格式为 `rule "..."`。
- [ ] `rule` 写在它要约束的实体前面。
- [ ] `desc` 是独立实体，不是后缀。
- [ ] `requires:` / `ensures:` 后接合法条件。
- [ ] 赋值右侧是简单值，没有函数调用或计算。
- [ ] `on` 块只跟在动作步骤后，同级缩进。
- [ ] 意图后缀顺序正确：锁定在前，不确定在后。
- [ ] `@import` 在文件最顶部。

---

## 10. 速查表

| 你要表达 | 写法 |
|---|---|
| 模块 | `module Name:` |
| 枚举 | `type T: A \| B \| C` |
| 记录 | `type T:\n    field: Type` |
| 函数 | `func Name(p):` |
| 函数描述 | `desc "..."`（func 内首个） |
| 步骤块 | `steps:` |
| 自然语言步骤 | `desc "..."` |
| 动作步骤 | `action_label [desc "..."] [to done]` |
| 赋值 | `target = simple_value` |
| 条件 | `if cond:` / `else:` |
| 循环 | `for var in iterable:` / `while cond:` |
| 并行 | `parasteps "label":` |
| 错误 | `error "msg" [to exit]` |
| 回滚 | `on condition:` |
| 规则约束 | `rule "..."`（前置） |
| 占位 | `...` |
| 不确定 | `?` / `??` |
| 锁定 | `$` / `$$` |
| 跨文件 | `@import "path.mms"` |
