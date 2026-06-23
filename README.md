# MimiSpec

> **高信息密度的意图描述语言** —— 从碎片到完整的渐进式规范。

MimiSpec（`.mms`）是一门意图描述语言，专为**人-AI 协作设计**。它把"不确定 → 部分结构化 → 完整锁定"的渐进式工作流嵌入语法本身，让每一阶段的碎片都成为合法的可解析文件。

```
// 阶段 1：纯意图，完全委托 AI
module?? Shop:
    type?? Order:
        desc?? "订单数据，包含买家、商品、金额和状态"
    func?? Pay:
        steps:
            desc?? "检查余额"
            desc?? "扣款"

// 阶段 5：完整锁定架构
module$ Shop:
    type$ OrderStatus: New | Pending | Paid | Shipped | Cancelled
    rule$ "支付必须幂等"
    func$ Pay(order, amount):
        requires$: order.status == Pending
        ensures$: order.status == Paid
        steps:
            check$ balance
            charge$ payment
            order.status$ = Paid >>> done
```

从阶段 1 到阶段 5，每一阶段都**语法合法、可被解析器接受**。

---

## 核心特性

- **Fragment 架构** —— `module`、`type`、`flow`、`func`、`ui`、`steps`、表达式、UI 节点……任何语法子树都是合法的顶层 Fragment
- **渐进式精确** —— `desc`（自然语言占位）→ `requires/ensures` → `math:` 块，逐步精确化
- **内置锁定系统** —— `$`/`$$` 标记已确认设计，`?`/`??` 标记不确定性，AI 协作不越界
- **约束链** —— `rule` 前置附着机制，从文件级到函数级精确表达约束层级
- **结构化数学** —— `math:` 块支持张量运算、位运算、微积分，替代自然语言描述
- **状态机** —— `flow` 定义，`>>>` 转移操作符，`requires` 守卫条件
- **UI 视图** —— `stack`/`parallel` 布局，`on` 事件绑定，Saga 补偿
- **错误恢复** —— 多级同步策略，从不因局部错误丢失整体 AST
- **纯 Rust** —— 零依赖外部运行时，CLI 单二进制发布

---

## 快速开始

### 安装

```bash
git clone https://github.com/ontonous/mimispec.git
cd mimispec
cargo build --release
```

### 命令行使用

```bash
# 解析文件并输出 AST
mimispec path/to/file.mms --ast

# 输出 JSON（IDE 集成用）
mimispec path/to/file.mms --json

# 渲染回源码（格式规范化）
mimispec path/to/file.mms --render

# 渲染数学块为 LaTeX
mimispec path/to/file.mms --latex

# 从标准输入读取
echo "func Hello: steps:\n    say hi" | mimispec - --ast

# 同时处理多个文件
mimispec *.mms --json
```

### 作为库使用

```toml
[dependencies]
mimispec = { git = "https://github.com/ontonous/mimispec" }
```

```rust
use mimispec::parse;

let source = r#"
type Status: Active | Inactive
func Toggle(user):
    requires: user.status in [Active, Inactive]
    steps:
        user.status = Inactive >>> done
"#;

let result = parse(source);
if result.errors.is_empty() {
    println!("{} fragments", result.file.fragments.len());
} else {
    for err in &result.errors {
        eprintln!("{}", mimispec::format::format_diagnostic(err, source));
    }
}
```

---

## 语法预览

| 结构 | 示例 |
|------|------|
| 枚举 | `type Status: New \| Pending \| Paid` |
| 记录 | `type Order:\n    id: u64\n    status: Status` |
| 函数 | `func Pay(order):\n    requires: order.status == Pending\n    steps:\n        charge payment >>> done` |
| 状态机 | `flow Lifecycle:\n    New >>> Pending:\n    Pending >>> Paid:\n    Paid >>> Done:` |
| 视图 | `ui Panel binds Model:\n    stack:\n        "标题" on tap: DoSomething()` |
| 并行 | `parasteps "加载数据":\n    load users\n    load orders` |
| 数学 | `math:\n    scores = Q @ K.T / sqrt(d_k)` |

完整语法见 [docs/specification.md](docs/specification.md)。

---

## 项目结构

```
mimispec/
├── src/
│   ├── main.rs                  # CLI 入口
│   └── lib/
│       ├── mod.rs               # 公共 API (parse, tokenize, ...)
│       ├── ast.rs               # AST 类型定义
│       ├── error.rs             # 结构化错误系统
│       ├── lexer.rs             # 词法分析器 (indent/dedent)
│       ├── parser/
│       │   ├── mod.rs           # 解析器核心 (token 导航、规则管理、错误恢复)
│       │   ├── expr.rs          # Pratt 表达式解析器
│       │   ├── fragment.rs      # Fragment 分发
│       │   ├── func.rs          # FuncDef 解析
│       │   ├── module.rs        # Module 解析
│       │   ├── flow.rs          # FlowDef 解析
│       │   ├── step.rs          # Step 解析 (if/for/while/action/assign/...)
│       │   ├── type.rs          # TypeDef 解析 (enum/record)
│       │   ├── ui.rs            # UiDef 解析
│       │   └── rule.rs          # RuleDef 解析
│       ├── render.rs            # AST → MimiSpec 渲染器
│       ├── render_util.rs       # 表达式优先级工具
│       ├── format.rs            # 诊断格式化
│       └── latex.rs             # LaTeX 数学渲染器
├── docs/
│   ├── specification.md         # 语法规范
│   ├── advanced-usage.md        # 高级用法模式
│   ├── version-management.md    # 版本管理与 CI/CD 规范
│   └── stdlib-api.md            # Mimi 运行时标准库参考
├── mimispec-parser-mms/         # 用 MimiSpec 写的解析器参考实现
├── CHANGELOG.md
├── CONTRIBUTING.md
└── AGENTS.md                    # AI 代理协作说明
```

---

## 文档

| 文档 | 说明 |
|------|------|
| [语法规范](docs/specification.md) | 完整语言参考，含所有 Fragment 类型、关键字、后缀系统 |
| [高级用法](docs/advanced-usage.md) | 模块化架构、契约设计、Saga、ML 模型规格、SLO 约束 |
| [版本管理](docs/version-management.md) | SemVer 规则、分支模型、发布流程、CI/CD 配置 |
| [标准库 API](docs/stdlib-api.md) | Mimi 运行时 16 模块 295 个函数/常量参考 |
| [贡献指南](CONTRIBUTING.md) | 开发环境、代码规范、PR 流程 |

---

## 许可证

Apache 2.0 © 2026 ontonous

---

## 设计哲学

> **From Scratch to Full** —— 碎片是起点，聚合是过程，完整是结果。

MimiSpec 不要求你一次性写出完整的规范。

- 不确定时写 `desc "..."`，AI 负责填充细节
- 模糊时加 `?`，锁定后加 `$`
- 碎片阶段就是一个合法的 `.mms` 文件
- 解析器不因"不完整"而拒绝，只因"语法错误"而拒绝
