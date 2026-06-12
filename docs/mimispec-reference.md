# MimiSpec 文件结构规范参考

> 本文档定义 `.mms` 文件的语法结构、模块组织及最佳实践。

---

## 1. 文件结构概览

```
project/
├── project.mms                    # 主规格文件
├── modules/
│   ├── domain.mms                 # 领域模块
│   ├── ui.mms                     # UI 模块
│   └── service.mms                # 服务模块
├── types/
│   ├── base.mms                   # 基础类型定义
│   └── enums.mms                  # 枚举类型
└── flows/
    └── lifecycle.mms              # 流程定义
```

### 顶层结构

一个 `.mms` 文件由以下部分组成：

```
@import "path/to/module.mms"        # 导入（可选，多个）

rule "全局约束"                     # 全局规则（可选）

[fragment]                          # 顶层片段（至少一个）
```

---

## 2. 顶层片段类型

### 2.1 模块 (module)

```mms
module ModuleName:
    desc "模块描述"

    rule "模块级约束"

    type Order: ...
    func Process(): ...

module Counter:
    desc "计数器模块"

    type State:
        count: Number
        lastUpdated: DateTime

    ui CounterView binds CounterModel:
        stack:
            "@count" desc "当前计数"
            "重置" on tap: Reset()
```

### 2.2 类型定义 (type)

#### 枚举类型

```mms
type OrderStatus: New | Pending | Paid | Shipped | Cancelled

type HttpMethod:
    GET | POST | PUT | DELETE | PATCH

type Priority: Low | Medium | High | Critical
```

#### 记录类型

```mms
type User:
    id: String
    name: String
    email: String
    rule "email 必须有效"
    createdAt: DateTime

type Order:
    id: String
    items: Item[]
    total: Money
    rule "total = sum(items.price * items.qty)"
```

### 2.3 函数 (func)

```mms
func GetUser(id):
    requires: id > 0
    steps:
        query database by id
        return user to done

func ProcessOrder(order):
    requires: order.status == Pending
    ensures: result.status == Processed
    steps:
        validate order
        if not valid:
            error "Invalid order"
        charge payment
        ship order to done
```

### 2.4 流程 (flow)

```mms
flow OrderLifecycle:
    Pending:
        to Processing: desc "开始处理"
        to Cancelled: desc "用户取消"
    Processing:
        to Shipped: desc "发货"
        to Refunded: desc "退款"
    Shipped:
        to Delivered: desc "确认收货"
    Cancelled:
        to Refunded: desc "退款处理"
```

### 2.5 独立步骤块 (steps)

```mms
steps:
    check inventory
    if stock < qty:
        error "out of stock"
    charge payment
    send notification
```

### 2.6 UI 节点

```mms
stack "垂直布局":
    "标题" desc "大号加粗"
    parallel "水平排列":
        "按钮A" on tap: ActionA()
        "按钮B" on tap: ActionB()
    scroll:
        "@items" desc "列表内容"
```

### 2.7 表达式片段

```mms
order.status == Pending and amount > 0
```

---

## 3. 导入 (@import)

```mms
@import "types/base.mms"
@import "domain/user.mms"
@import "ui/components.mms"

module App:
    ...
```

---

## 4. 约束规则 (rule)

### 全局规则

```mms
rule "所有金额必须为正数"

type Order:
    amount: Money
```

### 模块级规则

```mms
module Payment:
    rule "支付前必须验证用户"

    func Process():
        steps:
            verify user
            charge payment
```

### 字段级规则

```mms
type User:
    email: String
    rule "email 必须符合格式"
    age: Number
    rule "age 必须 >= 0"
```

---

## 5. 意图后缀 (Commitment)

### 锁定后缀 (`$`)

```mms
module$ Shop:                    # 锁定：Shop 模块不可修改
    type$$ Order:                # 强锁定：Order 类型及其 body 都不可修改
        id: String
```

### 不确定后缀 (`?`)

```mms
module? Shop:                    # 不确定：Shop 可能不存在
    func Pay??():                 # 更不确定：Pay 函数及其签名都可能变化
        ...
```

### 组合后缀

```mms
module$? Shop:                   # 锁定+不确定
    rule$ "支付幂等"$             # 规则描述需锁定，规则关键字需不确定

func Pay$$?():                   # 强锁定 + 不确定
    ...
```

---

## 6. UI 节点类型

### 6.1 stack (垂直堆叠)

```mms
stack "垂直布局":
    "标题"
    "内容"
    "按钮"
```

### 6.2 parallel (水平排列)

```mms
parallel "工具栏":
    "新建"
    "保存"
    "删除"
```

### 6.3 scroll (滚动区域)

```mms
scroll "可滚动区域":
    "@items" desc "列表"
```

### 6.4 leaf (叶子节点)

```mms
"提交按钮" desc "主操作" on tap: Submit()
"输入框" desc "文本输入" on change: UpdateValue(inputValue)
```

---

## 7. 操作 (Action)

### 导航

```mms
"返回" on tap: to HomeScreen
"详情" on tap: to DetailView
```

### 函数调用

```mms
"保存" on tap: Save(state)
"提交" on "双击": SubmitForm(data)
```

### 赋值

```mms
"输入框" on change: state.query = inputValue
```

### 复合操作

```mms
"提交" on tap: Save(state), to ResultScreen
```

---

## 8. 类型系统

### 8.1 基础类型

```
String      字符串
Number      数字
Boolean     布尔值
DateTime    日期时间
Money       货币（带单位）
```

### 8.2 集合类型

```mms
items: Item[]          # 数组
users: Map<String, User>  # Map
```

### 8.3 自定义类型

```mms
type OrderId: String
type UserId: String
```

---

## 9. 项目组织模式

### 9.1 单文件模式（小型项目）

```mms
# project.mms
@import "types.mms"

rule "项目级约束"

module App:
    type State: ...
    ui MainView: ...
    func Init(): ...
```

### 9.2 多模块模式（中型项目）

```
project/
├── project.mms           # 入口，导入子模块
├── types.mms            # 共享类型
├── domain/
│   ├── user.mms
│   └── order.mms
└── ui/
    ├── components.mms
    └── screens.mms
```

**project.mms**
```mms
@import "types.mms"
@import "domain/user.mms"
@import "domain/order.mms"
@import "ui/screens.mms"

module App:
    ...
```

### 9.3 插件模式（大型项目）

```
project/
├── project.mms
├── plugins/
│   ├── auth/
│   │   ├── auth.mms
│   │   └── types.mms
│   └── payment/
│       ├── payment.mms
│       └── flows.mms
└── extensions/
    └── custom/
```

---

## 10. 工作流程示例

### 10.1 订单处理流程

```mms
module OrderDomain:
    desc "订单领域"

    type OrderStatus: New | Processing | Shipped | Delivered | Cancelled

    type Order:
        id: String
        items: OrderItem[]
        status: OrderStatus
        total: Money

    flow OrderLifecycle:
        New:
            to Processing: desc "开始处理"
            to Cancelled: desc "取消订单"
        Processing:
            to Shipped: desc "已发货"
            to Cancelled: desc "退款取消"
        Shipped:
            to Delivered: desc "确认收货"

    func CreateOrder(items):
        requires: items.len() > 0
        steps:
            validate items
            calculate total
            save to database
            return order to New

    func ShipOrder(order):
        requires: order.status == Processing
        steps:
            pack items
            update status to Shipped
            send notification
            return order to Shipped
```

### 10.2 UI 驱动模式

```mms
module TaskManager:
    desc "任务管理"

    type State:
        tasks: Task[]
        filter: String

    ui TaskPanel binds TaskManager:
        stack "整体垂直":
            parallel "工具栏":
                "全部" desc "过滤" on tap: SetFilter("all")
                "进行中" desc "过滤" on tap: SetFilter("active")
                "已完成" desc "过滤" on tap: SetFilter("done")
            scroll "任务列表":
                parallel "单行任务":
                    checkbox on tap: ToggleTask(task.id)
                    "@task.title" desc "@task.desc"
                    delete on tap: DeleteTask(task.id)
            "添加任务" on tap: ShowAddDialog()
```

---

## 11. 错误处理

### 11.1 条件错误

```mms
if amount <= 0:
    error "金额必须大于 0"
```

### 11.2 前置条件

```mms
func Withdraw(amount):
    requires: amount > 0
    requires: balance >= amount
    steps:
        balance = balance - amount
        return success to done
```

### 11.3 后置条件

```mms
func Deposit(amount):
    ensures: balance == old.balance + amount
    steps:
        balance = balance + amount
```

---

## 12. 占位符

### 12.1 步骤占位

```mms
func TODO():
    steps:
        check funds
        ...
        transfer complete
```

### 12.2 类型占位

```mms
type FutureType: ...
```

### 12.3 函数占位

```mms
func Placeholder(): ...
func PlaceholderWithParams(a, b): ...
```

### 12.4 条件占位

```mms
func Process():
    requires: ...
    steps:
        handle
```

---

## 13. 元数据注释

### desc（描述）

```mms
module App:
    desc "应用主模块"

type Order:
    desc "订单类型"

func GetUser:
    desc "获取用户信息"
```

### on（事件绑定）

```mms
"按钮" on tap: Action()
"输入框" on change: Update(value)
"提交" on "双击": Submit()
```

---

## 14. 最佳实践

1. **模块化**: 按领域拆分类型和函数，每个模块专注一个职责
2. **命名**: 使用驼峰命名，类型首字母大写
3. **约束**: 使用 rule 明确业务约束，rule 跟随被约束的元素
4. **意图后缀**: 根据确定性选择合适的后缀
   - `$` 用于稳定接口
   - `?` 用于探索性代码
   - `$$?` 用于高度不确定的原型
5. **导入**: 使用相对路径，避免循环导入
6. **类型**: 优先使用具体类型定义而非裸类型

---

## 15. 完整示例

```mms
@import "types/base.mms"
@import "domain/user.mms"

rule "所有金额必须为正数"

module Shop:
    desc "商店领域"

    type OrderStatus: New | Paid | Shipped | Delivered | Cancelled

    type Order:
        id: String
        items: Item[]
        total: Money
        status: OrderStatus
        rule "total = sum(items.price * items.qty)"

    flow OrderLifecycle:
        New:
            to Paid: desc "支付完成"
            to Cancelled: desc "取消"
        Paid:
            to Shipped: desc "发货"
            to Cancelled: desc "退款"
        Shipped:
            to Delivered: desc "确认收货"

    func CreateOrder(items):
        requires: items.len() > 0
        steps:
            validate items
            calculate total
            save to database
            return order to New

    func ProcessPayment(order):
        requires: order.status == New
        ensures: result.status == Paid
        steps:
            check funds
            charge payment
            update status to Paid
            return order to Paid

    ui CheckoutView binds Shop:
        stack:
            "商品列表" desc "@order.items"
            "总计: @order.total"
            "支付" on tap: ProcessPayment(order)
            "取消" on tap: CancelOrder(order)
```