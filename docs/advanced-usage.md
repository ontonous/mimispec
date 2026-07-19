# MimiSpec 高级用法说明

> **状态**：示例对应 main 已实现的 `0.3.0-dev` 快照候选；当前已发布版本和
> Cargo 版本仍为 `v0.2.1`，本文不声明 RC 或稳定版已经发布。
>
> **目标**：说明如何用 MimiSpec 的现有语法精确描述复杂系统结构与行为。重点展示 `rule` 约束链、`math` 数学块、意图锁定后缀等高级特性组合后的表达能力。所有示例均不依赖未实现的语法扩展，可被当前 `mimispec` CLI 直接解析。

---

## 1. 模块化与分层架构

MimiSpec 的 `module` 不仅是一级命名空间，还可以嵌套，从而精确表达分层架构。

### 1.1 领域分层

```mimispec
module ECommerce:
    desc "电商系统"

    module Domain:
        desc "领域层：核心业务规则"

        module User:
            desc "用户子域"

        module Order:
            desc "订单子域"

        module Product:
            desc "商品子域"

    module Application:
        desc "应用层：用例编排"

        module Checkout:
            desc "结账用例"

        module Payment:
            desc "支付用例"

    module Infrastructure:
        desc "基础设施层：存储、消息、外部服务"

        module Database:
            desc "数据库访问"

        module MessageQueue:
            desc "消息队列"

        module Gateway:
            desc "外部网关"
```

### 1.2 微服务映射

每个微服务映射为一个顶层 `module`，内部再分领域。

```mimispec
module OrderService:
    desc "订单服务：负责订单生命周期管理"

    rule "所有写操作必须幂等"
    rule "订单状态变更必须记录审计日志"

    module API:
        desc "对外暴露的接口"

    module Domain:
        desc "领域模型"

    module Integration:
        desc "与其他服务的集成"
```

### 1.3 约束层级

`rule` 的前置附着机制可以表达从全局到局部的约束层级：

```mimispec
rule "系统必须高可用"

module PaymentService:
    rule "支付操作必须幂等"

    func Charge(order):
        rule "扣款前必须校验余额"
        steps:
            check balance
            charge payment
```

- `系统必须高可用` → 文件级全局约束
- `支付操作必须幂等` → module 级约束
- `扣款前必须校验余额` → function 级约束

### 1.4 模块级数学不变量

`math:` 块可以放在 `module` body 中，表达跨类型的全局数学关系。

```mimispec
module Physics:
    desc "物理计算模块"

    math:
        E = m * c ** 2
        F = m * a
        p = m * v

    type Body:
        mass: Number
        velocity: Number
        energy: Number
```

---

## 2. 类型系统高级用法

### 2.1 复合类型与嵌套泛型

MimiSpec 的类型提示支持嵌套泛型，可以表达复杂数据结构。

```mimispec
type EventBus:
    handlers: Map[EventType, List[EventHandler]]
    pendingEvents: Queue[DomainEvent]

func DispatchEvent(event: DomainEvent):
    desc "分发事件到所有订阅者"
    steps:
        find handlers for event.type
        for handler in handlers:
            invoke handler asynchronously
        record dispatch log
```

### 2.2 用枚举表达状态与标签

枚举不仅可以表示状态，还可以表示权限标签、错误码、渠道等。

```mimispec
type Permission: Read | Write | Admin | None
type Channel: Email | SMS | Push | InApp
type ErrorCode: NotFound | Unauthorized | Timeout | InternalError
```

### 2.3 字段级规则表达业务不变量

```mimispec
type Order:
    id: u64
    buyerId: u64
    items: list<OrderItem>
    total: Money
    rule "total 必须等于 sum(items.price * items.quantity)"
    status: OrderStatus
    rule "status 只能是 OrderStatus 枚举值"
    createdAt: Timestamp
    rule "createdAt 不得晚于当前时间"
```

### 2.4 类型级数学不变量

用 `math:` 块锁定类型字段之间的精确数值关系，替代自然语言描述的不变量。

```mimispec
type Rectangle:
    width: Number
    height: Number
    math:
        area == width * height
        perimeter == 2 * (width + height)
        diagonal ** 2 == width ** 2 + height ** 2
```

```mimispec
type Circle:
    radius: Number
    math:
        area == pi * radius ** 2
        circumference == 2 * pi * radius
```

```mimispec
type Triangle:
    a: Number
    b: Number
    c: Number
    math:
        a + b > c
        a + c > b
        b + c > a
```

---

## 3. 函数契约与行为描述

### 3.1 前置/后置条件

`requires` / `ensures` 可以表达 Hoare 式契约，适合精确描述函数语义。

```mimispec
func Withdraw(account, amount):
    desc "从账户扣款"
    requires: account.status == Active
    requires: amount > 0
    requires: account.balance >= amount
    ensures: "account.balance 等于 old.balance 减去 amount"
    steps:
        validate account
        deduct amount from account
        record transaction
        return success >>> done
```

> 注意：自然语言字符串中的 `old.balance` 由 AI 或工具层解释；结构化比较条件则可直接静态检查。

### 3.2 用 math 块精确描述返回值

当返回值或后置条件可以用公式表达时，使用 `math:` 块替代自然语言 `ensures`。

```mimispec
func DiscountedPrice(original, rate):
    desc "计算折扣后价格"
    requires: original > 0
    requires: rate >= 0 and rate <= 1
    math:
        discounted = original * (1 - rate)
    ensures: discounted < original or rate == 0
    steps:
        apply discount rate
        return discounted >>> done
```

```mimispec
func VectorNorm(v, p):
    desc "计算 Lp 范数"
    requires: p >= 1
    math:
        result = norm(v, p)
    steps:
        compute norm
        return result >>> done
```

### 3.3 错误路径与补偿

`error` 和 `on` 块可以表达显式失败和 Saga 式补偿。

```mimispec
func PlaceOrder(order):
    desc "下单并支付"
    requires: order.items.len() > 0
    ensures: order.status in [Paid, Cancelled]
    steps:
        reserve inventory
        on failure:
            error "库存不足" >>> exit

        charge payment
        on failure:
            release inventory desc "补偿：释放库存"
            error "支付失败" >>> exit

        order.status = Paid >>> done
```

### 3.4 副作用顺序

`steps` 块内步骤默认严格从上到下顺序执行。这是 MMS 的核心语义。

```mimispec
rule "以下步骤必须严格顺序执行"
func ProcessOrder(order):
    steps:
        validate order
        reserve inventory
        charge payment
        ship order
        order.status = Completed >>> done
```

---

## 4. 状态机与生命周期

`flow` 可以精确表达复杂状态机。

### 4.1 带守卫条件的状态转移

```mimispec
flow OrderLifecycle:
    New:
        >>> Pending: desc "提交订单"
        >>> Cancelled: desc "直接取消"
    Pending:
        >>> Paid: desc "支付成功" requires: payment.success
        >>> Cancelled: desc "超时取消" requires: "elapsed > 30min"
    Paid:
        >>> Shipped: desc "发货"
        >>> Refunded: desc "退款" requires: refund.approved
    Shipped:
        >>> Delivered: desc "确认收货"
        >>> Returned: desc "退货" requires: return.approved
```

### 4.2 发布流水线状态机

```mimispec
flow ReleasePipeline:
    Development:
        >>> Staging: desc "代码合并到 release 分支"
    Staging:
        >>> Testing: desc "部署到测试环境"
        >>> RolledBack: desc "预发布检查失败"
    Testing:
        >>> Production: desc "测试通过"
        >>> Staging: desc "测试失败，修复后重试"
    Production:
        >>> Monitoring: desc "开始灰度监控"
    Monitoring:
        >>> Stable: desc "监控通过"
        >>> RolledBack: desc "发现异常，回滚"
```

---

## 5. 控制流与并发

### 5.1 分支与循环

```mimispec
func BatchProcess(items):
    desc "批量处理订单项"
    steps:
        if items.len() == 0:
            error "empty batch" >>> exit

        for item in items:
            validate item
            if item.invalid:
                log invalid item
                continue
            process item

        while retryQueue.notEmpty:
            retry failed items desc "重试失败项"
```

### 5.2 并行步骤

`parasteps` 表示多个任务并发执行，全部完成后才继续。

```mimispec
func LoadDashboard(user):
    desc "加载仪表盘数据"
    steps:
        parasteps "并行加载多个数据源":
            load profile for user
            load recent orders
            load notifications
            load recommendations
        render dashboard
```

### 5.3 并发模型与后台任务

当前 MMS 没有专门的 `spawn` / `background` 关键字，但可以用 `rule` 表达并发意图。

```mimispec
rule "该函数必须在独立的 OS 线程中运行"
func Heartbeat:
    desc "后台心跳线程"
    steps:
        while true:
            send heartbeat
            sleep 1 second
```

```mimispec
rule "每个客户端连接必须在独立的线程中处理"
func HandleClient(stream):
    desc "处理单个客户端连接"
    steps:
        read request
        process request
        send response
```

---

## 6. UI 交互结构

`ui` 不仅可以描述页面布局，还可以描述交互流程。

### 6.1 复杂页面结构

```mimispec
ui OrderManagement binds orderState:
    stack "订单管理":
        parallel "筛选栏":
            "全部" desc "筛选按钮" on tap: SetFilter("all")
            "待付款" desc "筛选按钮" on tap: SetFilter("pending")
            "已发货" desc "筛选按钮" on tap: SetFilter("shipped")
        stack "订单列表":
            parallel "订单行":
                "@order.id" desc "订单号"
                "@order.status" desc "状态"
                "@order.total" desc "金额"
                "详情" on tap: >>> OrderDetail
        parallel "底部分页":
            "上一页" on tap: PrevPage()
            "@currentPage / @totalPages" desc "页码"
            "下一页" on tap: NextPage()
```

### 6.2 交互动作组合

```mimispec
"提交订单" desc "主按钮" on tap: ValidateOrder(state), SubmitOrder(state), >>> OrderSuccess
```

---

## 7. 非功能性约束

通过 `rule` 可以精确表达性能、安全、可靠性、合规等非功能性约束。需要量化时，可配合 `math:` 块给出公式。

### 7.1 性能约束

```mimispec
rule "P95 响应延迟必须小于 100ms"
rule "单次查询返回记录数不得超过 1000 条"
rule "批量处理必须在 5 分钟内完成"
```

### 7.2 安全约束

```mimispec
rule "所有密码必须加盐哈希存储"
rule "敏感接口必须二次认证"
rule "用户 Token 必须在 24 小时内过期"
rule "所有对外请求必须使用 TLS 1.3"
```

### 7.3 可靠性与一致性

```mimispec
rule "写操作必须保证最终一致性"
rule "消息投递至少一次，消费幂等"
rule "服务启动失败必须触发告警"
rule "数据库备份必须每日执行并保留 7 天"
```

### 7.4 资源与配额

```mimispec
rule "单用户最多创建 100 个项目"
rule "单文件大小不得超过 100MB"
rule "每个账户并发请求数不得超过 50"
```

### 7.5 带公式的 SLO

```mimispec
module APIGateway:
    desc "API 网关"

    rule "可用性必须达到 99.99%"
    math:
        availability == uptime / (uptime + downtime)
        availability >= 0.9999

    rule "错误预算消耗速率必须可监控"
    math:
        error_budget = 0.0001 * total_requests
        consumed_errors <= error_budget
```

---

## 8. 复杂系统模式

### 8.1 微服务架构

```mimispec
module UserService:
    desc "用户服务"

    func Register(email, password):
        requires: email != ""
        steps:
            check email uniqueness
            hash password
            create user
            publish UserRegistered event >>> done

module OrderService:
    desc "订单服务"

    rule "订单创建必须依赖用户服务验证"
    func CreateOrder(userId, items):
        steps:
            verify user exists
            validate items
            create order
            publish OrderCreated event >>> done
```

### 8.2 事件驱动架构

```mimispec
module EventBus:
    desc "事件总线"

    type Event:
        id: u64
        type: string
        payload: Map[string, any]
        timestamp: Timestamp

    func Publish(event):
        desc "发布事件"
        steps:
            validate event
            route to subscribers
            persist event log

    func Subscribe(eventType, handler):
        desc "订阅事件"
        steps:
            register handler
            start listener

module OrderService:
    desc "订单服务"

    func OnUserRegistered(event):
        desc "处理用户注册事件"
        steps:
            initialize user cart
            send welcome notification
```

### 8.3 Saga 分布式事务

```mimispec
module CheckoutSaga:
    desc "下单 Saga 流程"

    func Execute(order):
        desc "执行下单 Saga"
        requires: order.items.len() > 0
        steps:
            create order
            on failure:
                error "创建订单失败" >>> exit

            reserve inventory
            on failure:
                cancel order desc "补偿：取消订单"
                error "库存预留失败" >>> exit

            charge payment
            on failure:
                release inventory desc "补偿：释放库存"
                cancel order desc "补偿：取消订单"
                error "支付失败" >>> exit

            confirm order >>> done
```

### 8.4 AI / LLM 工作流

```mimispec
module LLMWorkflow:
    desc "LLM 任务处理工作流"

    func ProcessTask(task):
        desc "处理用户任务"
        steps:
            analyze task intent
            if task.needsTools:
                parasteps "并行调用工具":
                    call search tool
                    call calculator tool
                    call code executor tool
                synthesize tool results

            generate response
            on failure:
                retry with exponential backoff
                error "生成失败" >>> exit

            validate safety
            if not safe:
                error "内容不安全" >>> exit

            return response >>> done
```

### 8.5 前端状态管理

```mimispec
module TodoApp:
    desc "待办事项应用"

    type TodoState:
        todos: list<Todo>
        filter: FilterType
        loading: boolean

    type Todo:
        id: u64
        text: string
        completed: boolean

    type FilterType: All | Active | Completed

    func AddTodo(state, text):
        requires: text != ""
        steps:
            create todo
            append to state.todos

    func ToggleTodo(state, todoId):
        steps:
            find todo by id
            toggle todo.completed

    ui TodoView binds TodoState:
        stack "待办应用":
            parallel "输入区":
                "输入新任务..." desc "输入框"
                "添加" desc "按钮" on tap: AddTodo(state, inputValue)
            parallel "筛选栏":
                "全部" on tap: SetFilter(All)
                "进行中" on tap: SetFilter(Active)
                "已完成" on tap: SetFilter(Completed)
            stack "列表":
                "@todo.text" desc "待办项"
                "切换" on tap: ToggleTodo(state, todo.id)
```

---

## 9. 高级 `rule` 约束模式

### 9.1 约束链与分组

连续多条 `rule` 会收集为约束列表并附着给下一个同 scope 非 `rule` 语义项；
只有真实空行会阻断链，使未被接收的 `rule` 成为当前 scope 的 Environment
约束。独占一行的 `//` 注释只是视觉标题，本身不改变 attachment。

```mimispec
// 全局架构约束
rule "所有服务必须无状态"
rule "所有外部调用必须带超时与重试"

// 模块级约束
rule "支付模块必须支持幂等"
rule "支付模块必须可审计"

module Payment:
    desc "支付模块"

    // 核心不变量
    rule "金额必须大于 0"
    rule "同一笔订单不能重复扣款"

    type Money:
        amount: Decimal
        currency: Currency

    // 函数级约束
    rule "扣款前必须校验余额"
    rule "失败必须记录日志"
    func Charge(account, amount):
        steps:
            verify balance
            deduct amount
```

### 9.2 `rule` + `desc` 组合表达模糊意图

当约束尚不够精确时，给 `rule` 附加 `?` 或 `??`，让人类或 AI 后续再审视。

```mimispec
rule? "这里的重试策略可能需要细化"
rule?? "由 AI 决定具体的熔断参数"

func CallExternalAPI(request):
    steps:
        send request
        on failure:
            retry request
```

### 9.3 `rule` + `math` 组合表达可量化约束

自然语言 `rule` 说明业务意图，`math:` 块给出可检查的量化形式。

```mimispec
rule "购物车总价必须等于各商品小计之和"
type ShoppingCart:
    items: list<CartItem>
    total: Money
    math:
        total == sum(items.subtotal)

rule "折扣后价格不得低于成本"
func ApplyDiscount(cart, rate):
    math:
        discounted = cart.total * (1 - rate)
        discounted >= cart.cost
    steps:
        calculate discounted price
```

### 9.4 锁定关键约束

对于已经确认、不允许 AI 修改的约束，使用 `$` 或 `$$` 锁定。

```mimispec
rule$ "用户密码必须加盐哈希存储"
rule$ "所有支付记录必须不可篡改"

func HashPassword(password):
    math:
        salt = generate_random(16)
        hash = bcrypt(password, salt)
    ensures: hash != password
    steps:
        generate salt
        compute hash
```

---

## 10. 高级 `math` 模式

`math:` 块支持标量算术、比较逻辑、位运算、张量/线性代数操作以及常用数学函数。它是把自然语言意图转换为可静态检查结构的核心工具。

### 10.1 机器学习模型规格

用 `math:` 精确描述神经网络前向传播。

```mimispec
func CrossAttention(query, key, value):
    desc "标准的 scaled dot-product attention"
    requires: dim(query, -1) == dim(key, -1)
    math:
        d_k = dim(key, -1)
        scores = query @ key.T / sqrt(d_k)
        weights = softmax(scores, -1)
        context = weights @ value
        context.shape == [query.shape[0], dim(value, -1)]
    steps:
        compute attention scores
        apply softmax
        compute weighted sum
```

```mimispec
func LayerNorm(x):
    desc "层归一化"
    math:
        mean_x = mean(x, -1)
        var_x = variance(x, -1)
        normalized = (x - mean_x) / sqrt(var_x + 1e-5)
        output = gamma * normalized + beta
    steps:
        compute statistics
        normalize
        scale and shift
```

### 10.2 物理与工程公式

```mimispec
module Kinematics:
    desc "运动学计算"

    func FinalVelocity(v0, a, t):
        desc "匀加速直线运动末速度"
        math:
            v = v0 + a * t
        steps:
            compute velocity
            return v >>> done

    func Displacement(v0, a, t):
        desc "匀加速直线运动位移"
        math:
            s = v0 * t + 0.5 * a * t ** 2
        steps:
            compute displacement
            return s >>> done
```

### 10.3 金融计算

```mimispec
module Finance:
    desc "金融计算工具"

    func CompoundInterest(principal, rate, periods):
        desc "复利计算"
        requires: principal >= 0
        requires: rate >= 0
        math:
            amount = principal * (1 + rate) ** periods
            interest = amount - principal
        steps:
            compute amount
            return amount >>> done

    func LoanPayment(principal, rate, n):
        desc "等额本息月供"
        requires: principal > 0
        requires: rate > 0
        requires: n > 0
        math:
            payment = principal * (rate * (1 + rate) ** n) / ((1 + rate) ** n - 1)
        steps:
            compute payment
            return payment >>> done
```

### 10.4 位运算与掩码

```mimispec
func HasFlag(flags, bit):
    desc "检查第 bit 位是否为 1"
    requires: bit >= 0
    math:
        masked = flags & (1 << bit)
        is_set = masked != 0
    steps:
        check bit
        return is_set >>> done
```

### 10.5 形状与维度约束

```mimispec
func MatMulCompatible(A, B):
    desc "检查矩阵乘法兼容性"
    math:
        dim(A, -1) == dim(B, -2)
    steps:
        verify shapes
        return true >>> done

func BatchMatMul(A, B):
    desc "批量矩阵乘法"
    requires: dim(A, -1) == dim(B, -2)
    math:
        C = A @ B
        shape(C, -2) == shape(A, -2)
        shape(C, -1) == shape(B, -1)
        rank(C) == max(rank(A), rank(B))
    steps:
        compute batched product
        return C >>> done
```

---

## 11. 意图锁定与协作工作流

### 11.1 后缀语义速查

| 后缀 | 状态 | 含义 |
|------|------|------|
| （无） | 草案 | 人类已写，AI 可优化 |
| `?` | 不确定 | 需要审阅或给出选项 |
| `??` | 完全委托 | 人类未定义，全权交给 AI |
| `$` | 设计锁定 | 已确认，AI 不得修改 |
| `$$` | 强锁定 | 需人类显式解锁 |
| `$?` / `$$?` | 锁定待审 | 内容受保护，普通锁或强锁成熟度需要审视 |
| `$??` / `$$??` | 锁定委托评估 | 内容受保护，AI 完成成熟度评估后转入人类审阅或确认 |

> 顺序规则：锁定后缀必须在不确定后缀之前。`?$` / `?$$` / `??$` / `??$$` 非法。

### 11.2 渐进式锁定示例

```mimispec
// 阶段 1：完全委托
module?? Shop:
    type?? Order:
        desc?? "订单数据，包含买家、商品、金额和状态"

// 阶段 2：草案
module Shop:
    type Order:
        desc "订单数据"
        buyerId: u64
        total: Money

// 阶段 3：锁定关键结构
module$ Shop:
    desc$ "订单管理模块，处理下单、支付、退款"

    type$ OrderStatus: New | Pending | Paid | Shipped | Cancelled

    rule$ "支付必须幂等"
    func$ Pay(order, amount):
        requires$: order.status == Pending
        steps:
            check$ balance
            charge$ payment
            order.status$ = Paid >>> done
```

### 11.3 锁定关键设计决策

把架构级决策标记为 `$$`，把实现级决策标记为 `$`。

```mimispec
rule$$ "系统必须采用事件溯源架构"
rule$ "订单 ID 使用雪花算法生成"

module OrderService:
    type$$ Order:
        id: u64
        events: list<DomainEvent>

    func$ ReplayOrder(events):
        desc "通过事件重放恢复订单状态"
        steps:
            fold events into state
            return order >>> done
```

---

## 12. 当前限制与 Workaround

| 想表达的能力 | 当前是否支持 | Workaround |
|-------------|------------|-----------|
| 函数返回类型 | 否 | 用 `ensures` 或 `math:` 描述返回值 |
| 算术表达式 | 部分 | 普通 `steps` 中仍不支持计算表达式；使用 `math:` 块表达精确数值关系 |
| 显式 spawn/后台线程 | 否 | `rule` + `while` |
| 显式 async/await | 否 | `rule` 或自然语言描述 |
| 类型别名 | 否 | 直接定义新 type 或字段类型提示 |
| 异常类型体系 | 否 | `error "msg"` + `on condition:` |
| 包级命名空间限定 | 部分 | `@import` 后直接可见 |

---

## 13. 最佳实践

1. **从领域开始**：先定义 `module` 和 `type`，再补充 `func` 和 `flow`。
2. **用 `rule` 表达一切非结构化的约束**：性能、安全、并发、合规都可以写 `rule`。
3. **用 `math:` 精确化数值意图**：当自然语言 `rule` 或 `desc` 涉及公式、形状、维度时，给出 `math:` 块。
4. **组合 `rule` + `math` + `desc`**：自然语言说明“为什么”，数学块说明“是什么”，`rule` 说明“必须满足什么”。
5. **保持 `steps` 顺序语义**：不要把可以并行的步骤写进 `steps`，需要并行时用 `parasteps`。
6. **渐进式细化**：不确定的地方先用 `desc` 或 `...`，后续再结构化为 `requires` / `ensures` / `math:`。
7. **及时锁定**：核心接口、关键不变量、架构决策稳定后，标记 `$` / `$$`，避免 AI 误改。
8. **always validate**：生成后用 `mimispec` CLI 验证每个文件。

```bash
# Option 1: install from crates.io
cargo install mimispec && mimispec ../project-mms/*.mms

# Option 2: build from source
cd mimispec && cargo build --release
./target/release/mimispec ../project-mms/*.mms
```
