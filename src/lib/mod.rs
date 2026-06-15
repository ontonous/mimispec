pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod render;

use error::ParseResult;
use lexer::Lexer;
use parser::Parser;

/// 解析 MimiSpec 源字符串，返回 AST 和错误列表（编辑器场景：尽可能解析）。
pub fn parse(source: &str) -> ParseResult {
    match Lexer::new(source).tokenize() {
        Ok(tokens) => Parser::new(tokens).parse_file(),
        Err(e) => ParseResult {
            file: ast::File {
                imports: vec![],
                rules: vec![],
                fragments: vec![],
            },
            errors: vec![e],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ParseError;
    use ast::*;

    #[test]
    fn parse_ui_minimal() {
        let src = r#"
module App:
    ui CounterView binds CounterModel:
        stack:
            "当前计数" desc "大号数字"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Module { module } = &file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Ui { ui } = &module.items[0] else {
            panic!("expected ui")
        };
        assert_eq!(ui.name.name, "CounterView");
        assert_eq!(ui.binds.as_ref().unwrap().name, "CounterModel");
    }

    #[test]
    fn parse_ui_full_task_manager() {
        let src = r#"
module TaskManager:
    ui TaskPanel binds state:
        stack "整个面板垂直排列":
            parallel "水平工具栏":
                "任务列表" desc "标题，加粗"
                parallel "右侧按钮组":
                    "全部" desc "过滤按钮" on tap: SetFilter("all")
                    "已完成" desc "过滤按钮" on tap: SetFilter("done") requires state.selectedFilter != "done"
            parallel "输入区":
                "输入新任务..." desc "文本输入框"
                "添加" desc "主按钮" on tap: AddTask(state, inputValue) requires inputValue != ""
            stack "弹性高度，可滚动":
                parallel "单行任务":
                    "复选框" desc "勾选表示完成" on tap: ToggleTask(state, task.id)
                    "@task.title" desc "任务标题"
                    "删除" desc "按钮" on tap: DeleteTask(state, task.id)
            "暂无任务" desc "当过滤后列表为空时显示" requires state.tasks.len() == 0
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Module { module } = &file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Ui { ui } = &module.items[0] else {
            panic!("expected ui")
        };
        assert_eq!(ui.name.name, "TaskPanel");
    }

    #[test]
    fn parse_ui_action_variants() {
        let src = r#"
module App:
    ui Detail:
        stack:
            "返回" desc "按钮" on tap: to TaskPanel
            "保存" desc "按钮" on "提交": Save(state), to HomeScreen
            "搜索" desc "按钮" on tap: state.query = inputValue
            "执行" desc "按钮" on tap: "执行搜索并刷新列表"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        let Fragment::Module { module } = &file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Ui { ui } = &module.items[0] else {
            panic!("expected ui")
        };
        let UiNode::Stack { stack } = &ui.root else {
            panic!("expected stack")
        };
        assert_eq!(stack.children.len(), 4);

        // Check navigation action
        let UiNode::Leaf { leaf } = &stack.children[0] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert!(
            matches!(&on.action.actions[0], Action::Navigate { target } if target.name == "TaskPanel")
        );

        // Check composite action
        let UiNode::Leaf { leaf } = &stack.children[1] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert_eq!(on.action.actions.len(), 2);

        // Check assign action
        let UiNode::Leaf { leaf } = &stack.children[2] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert!(matches!(&on.action.actions[0], Action::Assign { .. }));

        // Check natural language action
        let UiNode::Leaf { leaf } = &stack.children[3] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert!(matches!(&on.action.actions[0], Action::Natural { .. }));
    }

    #[test]
    fn parse_ui_event_name_string() {
        let src = r#"
module App:
    ui Test:
        stack:
            "提交" on "双击": DoSomething()
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        let Fragment::Module { module } = &file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Ui { ui } = &module.items[0] else {
            panic!("expected ui")
        };
        let UiNode::Stack { stack } = &ui.root else {
            panic!("expected stack")
        };
        let UiNode::Leaf { leaf } = &stack.children[0] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert!(matches!(&on.event_name, EventName::Natural { text } if text.value == "双击"));
    }

    #[test]
    fn parse_parasteps() {
        let src = r#"
module App:
    func LoadDashboard:
        steps:
            parasteps "同时请求多个数据源":
                loadUsers desc "GET /users"
                loadOrders desc "GET /orders"
                loadMetrics desc "GET /metrics"
            combine results to done
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        let Fragment::Module { module } = &file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Func { func } = &module.items[0] else {
            panic!("expected func")
        };
        assert_eq!(func.steps.len(), 2);
        match &func.steps[0] {
            Step::Parasteps { step: p } => {
                assert_eq!(p.description.as_ref().unwrap().value, "同时请求多个数据源");
                assert_eq!(p.steps.len(), 3);
            }
            _ => panic!("expected parasteps"),
        }
    }

    // ── v0.3 新增测试 ─────────────────────────────────────────────────────────

    #[test]
    fn parse_standalone_steps_fragment() {
        // v0.3: 裸 steps 可以作为顶层 Fragment
        let src = r#"
steps:
    check inventory
    if stock < qty:
        error "out of stock"
    charge payment
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Steps { steps } = &file.fragments[0] else {
            panic!("expected steps")
        };
        assert_eq!(steps.len(), 3);
        assert!(matches!(&steps[0], Step::Action { .. }));
        assert!(matches!(&steps[1], Step::If { .. }));
        assert!(matches!(&steps[2], Step::Action { .. }));
    }

    #[test]
    fn rule_scope_test() {
        // 场景1: 顶层 rule 附着给下一个 fragment（无空行）
        let src1 = r#"rule "约束module"
module Shop:
    func Pay:
        steps:
            check balance"#;
        let result1 = parse(src1);
        println!("=== 场景1: 顶层 rule 附着给 module ===");
        println!("file.rules: {}", result1.file.rules.len());
        let Fragment::Module { module } = &result1.file.fragments[0] else {
            panic!()
        };
        println!("module.rules: {}", module.rules.len());
        for r in &module.rules {
            println!("  - {}", r.desc.content.value);
        }
        assert_eq!(result1.file.rules.len(), 0, "顶层 rule 不应留在 file.rules");
        assert_eq!(module.rules.len(), 1, "rule 应附着给 module");
        assert_eq!(module.rules[0].desc.content.value, "约束module");

        // 场景2: 空行阻断，rule 变为全局
        let src2 = r#"rule "全局约束"

module Shop:
    func Pay:
        steps:
            check balance"#;
        let result2 = parse(src2);
        println!("\n=== 场景2: 空行阻断 ===");
        println!("file.rules: {}", result2.file.rules.len());
        for r in &result2.file.rules {
            println!("  - {}", r.desc.content.value);
        }
        assert_eq!(result2.file.rules.len(), 1, "空行阻断后 rule 应变为全局");
        assert_eq!(result2.file.rules[0].desc.content.value, "全局约束");

        // 场景3: module 内 rule 约束 module 本身（不因内部空行中断）
        let src3 = r#"module Shop:
    rule "模块级约束"
    
    type Order:
        id: u64
    
    func Pay:
        steps:
            check balance"#;
        let result3 = parse(src3);
        println!("\n=== 场景3: module 内 rule 不因空行中断 ===");
        let Fragment::Module { module } = &result3.file.fragments[0] else {
            panic!()
        };
        println!("module.rules: {}", module.rules.len());
        for r in &module.rules {
            println!("  - {}", r.desc.content.value);
        }
        assert_eq!(module.rules.len(), 1);
        assert_eq!(module.rules[0].desc.content.value, "模块级约束");
        assert_eq!(module.items.len(), 2, "module 应包含 type 和 func");
    }

    #[test]
    fn parse_multiple_attached_rules_before_fragment() {
        // 多条 rule 连续写在 fragment 之前（无空行），应全部附着给下一个 fragment
        let src = r#"rule "rule A"
rule "rule B"
rule "rule C"
module Agent:
    desc "agent desc"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.file.rules.len(), 0, "未空行的 rule 不应变为全局约束");
        assert_eq!(result.file.fragments.len(), 1);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module fragment")
        };
        assert_eq!(module.rules.len(), 3, "三条 rule 应全部附着给 module");
        assert_eq!(module.rules[0].desc.content.value, "rule A");
        assert_eq!(module.rules[1].desc.content.value, "rule B");
        assert_eq!(module.rules[2].desc.content.value, "rule C");
    }

    #[test]
    fn parse_multiple_top_level_fragments() {
        // v0.3.1: rule 作为前置约束修饰符，不再是独立 Fragment
        let src = r#"
type OrderStatus: New | Pending | Paid | Shipped | Cancelled

rule "支付幂等"

steps:
    check inventory
    charge payment
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        // rule 作为前置约束修饰符附着到 steps，所以只有 2 个 fragment
        assert_eq!(file.fragments.len(), 2);
        assert!(matches!(&file.fragments[0], Fragment::TypeDef { .. }));
        // steps 带有前置 rule
        assert!(matches!(&file.fragments[1], Fragment::Steps { .. }));
        // 全局 rule 被收集到 file.rules
        assert_eq!(file.rules.len(), 1);
    }

    #[test]
    fn parse_expr_fragment() {
        // v0.3: 裸表达式可以作为顶层 Fragment
        let src = "order.status == Pending and amount > 0";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Expr { expr } = &file.fragments[0] else {
            panic!("expected expr")
        };
        assert!(matches!(expr, Expr::And { .. }));
    }

    #[test]
    fn parse_placeholder_step() {
        // v0.3: ... 占位符
        let src = r#"
func Pay(order, amount):
    steps:
        check funds
        ...
        order.status = Paid to done
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        let Fragment::Func { func } = &file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.steps.len(), 3);
        assert!(matches!(&func.steps[1], Step::Placeholder { .. }));
    }

    #[test]
    fn parse_standalone_uinode() {
        // v0.3: 裸 UI 节点可以作为顶层 Fragment
        let src = r#"stack "工具栏":
    "全部" desc "过滤"
    "进行中" desc "过滤"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        assert!(matches!(&file.fragments[0], Fragment::UiNode { .. }));
        let Fragment::UiNode { node } = &file.fragments[0] else {
            panic!("expected uinode")
        };
        assert!(matches!(node, UiNode::Stack { .. }));
    }

    #[test]
    fn parse_placeholder_fragment() {
        // v0.3: 单独的 ... 作为顶层 Fragment
        let src = "...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        assert!(matches!(&file.fragments[0], Fragment::Placeholder));
    }

    #[test]
    fn parse_func_single_line_placeholder() {
        // v0.3: func Pay(order): ... 作为单行函数体占位
        let src = "func Pay(order): ...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Func { func } = &file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.name.name, "Pay");
        assert_eq!(func.params.len(), 1);
        assert!(func.requires.is_none());
        assert!(func.ensures.is_none());
        assert!(func.steps.is_empty());
    }

    #[test]
    fn parse_func_block_placeholder() {
        // v0.3: func Pay(order):\n    ... 作为 block 内占位
        let src = "func Pay(order):\n    ...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Func { func } = &file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.name.name, "Pay");
        assert!(func.steps.is_empty());
    }

    #[test]
    fn parse_requires_placeholder() {
        // v0.3: requires: ... 作为条件占位
        let src = r#"
func Pay(order):
    requires: ...
    steps:
        charge payment to done
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        let Fragment::Func { func } = &file.fragments[0] else {
            panic!("expected func")
        };
        assert!(func.requires.is_some());
        let cond = func.requires.as_ref().unwrap();
        assert!(matches!(
            cond,
            Condition::Structured {
                expr: Expr::Placeholder { .. }
            }
        ));
    }

    #[test]
    fn parse_type_placeholder() {
        // v0.3: type Order: ... 作为单行类型定义占位
        let src = "type Order: ...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::TypeDef { typedef } = &file.fragments[0] else {
            panic!("expected type")
        };
        assert_eq!(typedef.name.name, "Order");
        assert!(matches!(&typedef.body, TypeBody::Record { fields } if fields.is_empty()));
    }

    #[test]
    fn parse_flow_placeholder() {
        // v0.3: flow Lifecycle: ... 作为单行 flow 占位
        let src = "flow Lifecycle: ...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Flow { flow } = &file.fragments[0] else {
            panic!("expected flow")
        };
        assert_eq!(flow.name.name, "Lifecycle");
        assert!(flow.entries.is_empty());
    }

    #[test]
    fn parse_ui_placeholder() {
        // v0.3: ui View: ... 作为单行 UI 占位
        let src = "ui View: ...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Ui { ui } = &file.fragments[0] else {
            panic!("expected ui")
        };
        assert_eq!(ui.name.name, "View");
        assert!(matches!(ui.root, UiNode::Stack { .. }));
    }

    #[test]
    fn parse_assignment_placeholder() {
        // v0.3: order.status = ... 作为赋值右侧占位
        let src = r#"
func Pay(order):
    steps:
        order.status = ...
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        let Fragment::Func { func } = &file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.steps.len(), 1);
        let Step::Assign { step } = &func.steps[0] else {
            panic!("expected assign")
        };
        assert!(matches!(step.value, SimpleValue::Ident { ref value } if value.name == "..."));
    }

    #[test]
    fn parse_import_directive() {
        let src = r#"@import "common/types.mms"

module UserDomain:
    func GetUser(id):
        requires: id > 0
        steps:
            query database
            return user to done
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0], "common/types.mms");
        assert_eq!(file.fragments.len(), 1);
        let Fragment::Module { module } = &file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(module.name.name, "UserDomain");
    }

    #[test]
    fn parse_multiple_imports() {
        let src = r#"@import "a.mms"
@import "b.mms"

type X: A | B
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let file = result.file;
        assert_eq!(file.imports.len(), 2);
        assert_eq!(file.imports[0], "a.mms");
        assert_eq!(file.imports[1], "b.mms");
    }

    // ── 多错误报告测试 ────────────────────────────────────────────────────────

    #[test]
    fn parse_multiple_errors_recover_at_fragment_boundary() {
        // func 缺少 `)`，内部 steps 被当作独立 fragment 解析，后续 type 仍能正常解析
        let src = r#"func Broken(order:
    steps:
        do something

type Order: A | B | C
"#;
        let result = parse(src);
        assert_eq!(
            result.errors.len(),
            1,
            "expected 1 error, got {:?}",
            result.errors
        );
        // steps 块被当作独立顶层 fragment 恢复解析
        assert!(result
            .file
            .fragments
            .iter()
            .any(|f| matches!(f, Fragment::Steps { .. })));
        assert!(result
            .file
            .fragments
            .iter()
            .any(|f| matches!(f, Fragment::TypeDef { typedef } if typedef.name.name == "Order")));
    }

    #[test]
    fn parse_error_recovery_between_fragments() {
        // 语法错误 fragment 后紧跟合法 fragment
        let src = r#"func Broken(:
    steps:
        do something

func Good(): ...

flow Lifecycle:
    Idle to Active: desc "启动"
"#;
        let result = parse(src);
        assert!(
            !result.errors.is_empty(),
            "expected at least one error, got {:?}",
            result.errors
        );
        // 应该能解析出 func Good 和 flow Lifecycle（中间的 steps 被当作独立 fragment）
        assert!(result
            .file
            .fragments
            .iter()
            .any(|f| matches!(f, Fragment::Func { func } if func.name.name == "Good")));
        assert!(result
            .file
            .fragments
            .iter()
            .any(|f| matches!(f, Fragment::Flow { flow } if flow.name.name == "Lifecycle")));
    }

    #[test]
    fn parse_import_error_recovery() {
        // @import 后面缺少路径，后续的 fragment 仍能解析
        let src = r#"@import

func Main(): ...
"#;
        let result = parse(src);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.file.fragments.len(), 1);
        assert!(
            matches!(&result.file.fragments[0], Fragment::Func { func } if func.name.name == "Main")
        );
    }

    #[test]
    fn parse_field_level_rules() {
        // type body 内的 rule 应作为字段级约束附着给下一个 field
        let src = r#"
type Order:
    rule "id 必须大于 0"
    id: u64
    rule "status 必须有效"
    rule "status 不能是 Cancelled"
    status: OrderStatus
    amount: Money
"#;
        let result = parse(src);
        assert!(
            result.errors.is_empty(),
            "parse errors: {:?}",
            result.errors
        );
        let file = result.file;
        assert_eq!(file.fragments.len(), 1);
        let Fragment::TypeDef { typedef } = &file.fragments[0] else {
            panic!("expected type")
        };
        let TypeBody::Record { fields } = &typedef.body else {
            panic!("expected record")
        };
        assert_eq!(fields.len(), 3);

        assert_eq!(fields[0].name.name, "id");
        assert_eq!(fields[0].rules.len(), 1);
        assert_eq!(fields[0].rules[0].desc.content.value, "id 必须大于 0");

        assert_eq!(fields[1].name.name, "status");
        assert_eq!(fields[1].rules.len(), 2);
        assert_eq!(fields[1].rules[0].desc.content.value, "status 必须有效");
        assert_eq!(
            fields[1].rules[1].desc.content.value,
            "status 不能是 Cancelled"
        );

        assert_eq!(fields[2].name.name, "amount");
        assert_eq!(fields[2].rules.len(), 0);
    }

    #[test]
    fn parse_type_level_and_field_level_rules() {
        // 外部 rule 给 type，内部 rule 给 field
        let src = r#"
rule "type-level constraint"
type Order:
    rule "field-level constraint"
    id: u64
"#;
        let result = parse(src);
        assert!(
            result.errors.is_empty(),
            "parse errors: {:?}",
            result.errors
        );
        let file = result.file;
        let Fragment::TypeDef { typedef } = &file.fragments[0] else {
            panic!("expected type")
        };

        assert_eq!(typedef.rules.len(), 1);
        assert_eq!(typedef.rules[0].desc.content.value, "type-level constraint");

        let TypeBody::Record { fields } = &typedef.body else {
            panic!("expected record")
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].rules.len(), 1);
        assert_eq!(
            fields[0].rules[0].desc.content.value,
            "field-level constraint"
        );
    }

    #[test]
    fn parse_module_inner_error_recovery() {
        // module 内部有错误时不应丢失整个 module，
        // 而应跳过错误 fragment，继续解析 module 内后续实体及顶层 fragment。
        let src = r#"module App:
    func Broken(:
        steps:
            do something

    func Good(): ...

type Status: A | B
"#;
        let result = parse(src);
        assert!(!result.errors.is_empty(), "expected errors");

        // module App 应该保留，且内部包含 func Good
        let module = result.file.fragments.iter().find_map(|f| match f {
            Fragment::Module { module } if module.name.name == "App" => Some(module),
            _ => None,
        });
        assert!(module.is_some(), "module App should be preserved");
        let module = module.unwrap();
        assert!(
            module
                .items
                .iter()
                .any(|f| matches!(f, Fragment::Func { func } if func.name.name == "Good")),
            "func Good should be parsed after error recovery"
        );

        // 后续顶层 type Status 仍能解析
        assert!(
            result.file.fragments.iter().any(
                |f| matches!(f, Fragment::TypeDef { typedef } if typedef.name.name == "Status")
            ),
            "expected type Status to be parsed"
        );
    }
    #[test]
    fn parse_func_inner_step_error_recovery() {
        // func body 内的错误 step 不应导致整个函数丢失，
        // 后续合法 step 仍应被解析。
        let src = r#"func Compute(x):
    steps:
        if :
        result = x
"#;
        let result = parse(src);
        assert!(
            !result.errors.is_empty(),
            "expected errors for invalid step"
        );

        let func = result.file.fragments.iter().find_map(|f| match f {
            Fragment::Func { func } if func.name.name == "Compute" => Some(func),
            _ => None,
        });
        assert!(func.is_some(), "func Compute should be preserved");
        let func = func.unwrap();
        assert_eq!(
            func.steps.len(),
            1,
            "expected 1 step after skipping invalid 'if :'"
        );
        assert!(
            matches!(&func.steps[0], Step::Assign { .. }),
            "step should be assignment 'result = x'"
        );
    }

    #[test]
    fn parse_type_def_field_error_recovery() {
        // type record 内某个 field 解析失败不应丢失整个 type，
        // 后续 field 仍应被解析。
        let src = r#"type Cat:
    name: String
    : String
    color: String
"#;
        let result = parse(src);
        assert!(
            !result.errors.is_empty(),
            "expected errors for malformed field"
        );

        let typedef = result.file.fragments.iter().find_map(|f| match f {
            Fragment::TypeDef { typedef } if typedef.name.name == "Cat" => Some(typedef),
            _ => None,
        });
        assert!(typedef.is_some(), "type Cat should be preserved");
        let typedef = typedef.unwrap();
        let fields = match &typedef.body {
            TypeBody::Record { fields } => fields,
            _ => panic!("expected record body"),
        };
        assert_eq!(fields.len(), 2, "expected 2 fields: name and color");
        assert_eq!(fields[0].name.name, "name");
        assert_eq!(fields[1].name.name, "color");
    }

    #[test]
    fn parse_unterminated_string_line() {
        // 未闭合字符串应当在开引号所在行报错，而不是漂移到很后面。
        let src = r#"desc "missing close
module App:
    func Good(): ...
"#;
        let result = parse(src);
        assert_eq!(result.errors.len(), 1, "expected exactly one error");
        match &result.errors[0] {
            ParseError::UnterminatedString { line, col } => {
                assert_eq!(*line, 1, "error should be on line 1 (opening quote)");
                assert_eq!(*col, 6, "error should be at column 6 (opening quote)");
            }
            other => panic!("expected UnterminatedString, got {:?}", other),
        }
    }

    #[test]
    fn parse_nested_generic_type_hint() {
        // 嵌套泛型列表字面量应作为类型提示正常解析。
        let src = r#"type X:
    handlers: Map[EventType, List[EventHandler]]
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::TypeDef { typedef } = &result.file.fragments[0] else {
            panic!("expected type")
        };
        let TypeBody::Record { fields } = &typedef.body else {
            panic!("expected record")
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.name, "handlers");
        // type_hint: Map [EventType, List[EventHandler]]
        assert_eq!(fields[0].type_hint.len(), 2);
        let Atom::List { items } = &fields[0].type_hint[1] else {
            panic!("expected list atom")
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].len(), 1);
        assert_eq!(items[1].len(), 2);
    }

    #[test]
    fn error_recovery_does_not_hang_on_invalid_token() {
        // 非法字符应被快速报告，而不是进入无限循环或 panic。
        let src = "func Compute(x):\n    steps:\n        result = ~x\n";
        let result = parse(src);
        assert!(!result.errors.is_empty(), "expected errors for invalid token");
    }

    #[test]
    fn module_error_recovery_at_fragment_boundary() {
        // module 内错误后紧跟合法 fragment，同步点停在 fragment 开头不应死循环。
        let src = r#"module App:
    func Broken(:
    func Good(): ...
"#;
        let result = parse(src);
        assert!(!result.errors.is_empty(), "expected errors");

        let module = result.file.fragments.iter().find_map(|f| match f {
            Fragment::Module { module } if module.name.name == "App" => Some(module),
            _ => None,
        });
        assert!(module.is_some(), "module App should be preserved");
        let module = module.unwrap();
        assert!(
            module
                .items
                .iter()
                .any(|f| matches!(f, Fragment::Func { func } if func.name.name == "Good")),
            "func Good should be parsed after error recovery"
        );
    }

    // ── math 块测试 ───────────────────────────────────────────────────────────

    #[test]
    fn parse_math_block_basic() {
        let src = r#"
func CrossAttention(query, key, value):
    math:
        d_k = dim(key, -1)
        scores = query @ key.T / sqrt(d_k)
        weights = softmax(scores, -1)
        context = weights @ value
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let math = func.math.as_ref().expect("expected math block");
        assert_eq!(math.statements.len(), 4);
        assert!(matches!(math.statements[0], MathStatement::Define { .. }));
        assert!(matches!(math.statements[1], MathStatement::Define { .. }));
    }

    #[test]
    fn parse_math_arithmetic_and_bitwise() {
        let src = r#"
func Compute(x, y):
    math:
        a = x + y * 2
        b = (x - y) / 4
        c = a ** 3
        mask = x & 255 | y << 8
        inv = ~x ^ y
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let math = func.math.as_ref().unwrap();
        assert_eq!(math.statements.len(), 5);
    }

    #[test]
    fn parse_math_in_requires() {
        let src = r#"
func MultiHead(Q, num_heads, head_dim):
    requires: dim(Q, -1) == num_heads * head_dim
    steps:
        return output
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let requires = func.requires.as_ref().unwrap();
        let Condition::Structured { expr } = requires else {
            panic!("expected structured condition")
        };
        assert!(matches!(expr, Expr::Compare { .. }));
    }

    #[test]
    fn parse_math_subscript_and_unary() {
        let src = r#"
func Last(x):
    math:
        last = x[-1]
        neg = -last
        bits = ~neg
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let math = func.math.as_ref().unwrap();
        assert_eq!(math.statements.len(), 3);
    }

    #[test]
    fn parse_math_precedence_and_associativity() {
        let src = r#"
func Test():
    math:
        a = x + y * z
        b = x - -y
        c = a ** b ** 2
        d = p | q & r
        e = x << 2 + 1
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn parse_math_multi_subscript() {
        let src = r#"
func Test():
    math:
        v = x[i, j]
        last = tensor[-1, -2]
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let math = func.math.as_ref().unwrap();
        assert_eq!(math.statements.len(), 2);
        let MathStatement::Define { target, .. } = &math.statements[0] else {
            panic!("expected define")
        };
        assert!(matches!(target, Expr::Ident { .. }));
    }

    #[test]
    fn parse_math_type_enum_still_works() {
        let src = r#"
type Status: New | Pending | Paid

func Check(s):
    math:
        flags = s | 1
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn parse_math_decimal_numbers() {
        let src = r#"
func Configure():
    math:
        lr = 1e-4
        dropout = 0.15
        scale = 1.5e-3
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let math = func.math.as_ref().unwrap();
        assert_eq!(math.statements.len(), 3);
    }

    #[test]
    fn parse_math_in_module() {
        let src = r#"
module Physics:
    math:
        E = m * c ** 2

    func Energy(m, c):
        steps:
            return E
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        assert!(module.math.is_some());
    }

    #[test]
    fn parse_math_in_type() {
        let src = r#"
type Rectangle:
    width: Number
    height: Number
    math:
        area == width * height
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::TypeDef { typedef } = &result.file.fragments[0] else {
            panic!("expected type")
        };
        assert!(typedef.math.is_some());
    }
}

#[cfg(test)]
mod default_code_test {
    use super::*;
    use crate::ast::*;
    #[test]
    fn parse_default_code() {
        // 这个用例覆盖 rule 分组语义：
        // 1. 文件顶层 rule 组（被 // 注释/空行分隔）进入 file.rules；
        // 2. module 前无空行的 rule 组附着给 module；
        // 3. module 内被 // 注释/空行分隔的 rule 组进入 module.rules；
        // 4. func 前无空行的 rule 组附着给 func；
        // 5. func body 内被 // 注释/空行分隔的 rule 组进入 func.rules。
        let src = r#"desc "Meowthos猫咖营业手册喵！"
//全局约束1
rule$ "1"
rule$ "2"
rule$ "3"
//全局约束2
rule$ "4"
rule$ "5"
rule$ "6"
//module约束1
rule$ "7"
rule$ "8"
module MeowCafe:
    desc "猫猫的梦幻咖啡屋喵"
    //module约束2
    rule$ "9"
    rule$ "10"
    //module约束3
    rule$ "11"
    rule$ "12"

    type OrderStatus: New | Brewing | Served | Spilled | Refunded | Done

    type Drink:
        name: String
        price: Number
        desc "这杯饮料的描述，比如'超烫的焦糖玛奇朵'"

    //func约束1
    rule$ "13"
    rule$ "14"
    func$ Brew(drink, temp):
        //func约束2
        rule$ "15"
        rule$ "16"

        requires: temp >= 60
        ensures: drink.status == Served
        //func约束3
        rule$ "17"
        rule$ "18"
        steps:
            desc "先把咖啡豆磨成粉... 啊，磨太细了喵！"
            grind beans
            if temp > 90:
                desc "太烫了会烫到舌头喵！"
                cool down
            else:
                ...
            pour milk
            desc "拉花环节！画个猫爪... 哎呀画成狗了喵"
            drink.status = Served
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
        assert_eq!(result.file.fragments.len(), 2, "expected 2 fragments: Steps(Desc) + Module");
        assert!(matches!(&result.file.fragments[0], Fragment::Steps { .. }));

        let Fragment::Module { module } = &result.file.fragments[1] else {
            panic!("expected Module fragment at index 1");
        };
        assert_eq!(module.name.name, "MeowCafe");

        let file_rule_values: Vec<&str> = result
            .file
            .rules
            .iter()
            .map(|r| r.desc.content.value.as_str())
            .collect();
        assert_eq!(
            file_rule_values,
            vec!["1", "2", "3", "4", "5", "6"],
            "顶层 rule 组应全部进入 file.rules"
        );

        let module_rule_values: Vec<&str> = module
            .rules
            .iter()
            .map(|r| r.desc.content.value.as_str())
            .collect();
        assert_eq!(
            module_rule_values,
            vec!["7", "8", "9", "10", "11", "12"],
            "module 级 rule 组应全部进入 module.rules"
        );

        assert_eq!(module.items.len(), 3, "module 应有 3 个 item：type enum、type record、func Brew");

        let Fragment::Func { func } = &module.items[2] else {
            panic!("expected func Brew at index 2")
        };
        assert_eq!(func.name.name, "Brew");

        let mut func_rule_values: Vec<&str> = func
            .rules
            .iter()
            .map(|r| r.desc.content.value.as_str())
            .collect();
        func_rule_values.sort();
        assert_eq!(
            func_rule_values,
            vec!["13", "14", "15", "16", "17", "18"],
            "func 级 rule 组应全部进入 func.rules"
        );

        assert!(func.requires.is_some(), "func Brew 应有 requires");
        assert!(func.ensures.is_some(), "func Brew 应有 ensures");
    }
}

#[cfg(test)]
mod lock_suffix_tests {
    use super::*;
    use crate::error::ParseError;
    use ast::*;

    #[test]
    fn parse_lock_suffix_on_keyword() {
        let src = r#"module$ Shop:
    type$$ Order:
        id: String
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(module.keyword_commitment, Commitment::Locked);

        let Fragment::TypeDef { typedef } = &module.items[0] else {
            panic!("expected type")
        };
        assert_eq!(typedef.keyword_commitment, Commitment::StrongLocked);
    }

    #[test]
    fn parse_lock_suffix_on_identifier() {
        let src = r#"module Shop$:
    func Pay$?():
        steps:
            check balance
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(module.name.commitment, Commitment::Locked);

        let Fragment::Func { func } = &module.items[0] else {
            panic!("expected func")
        };
        assert_eq!(func.name.commitment, Commitment::LockedQuestion);
    }

    #[test]
    fn parse_lock_suffix_on_string_content() {
        // rule 与 module 之间无空行，会作为前置约束附着给 module
        let src = r#"rule$? "支付必须幂等"$
module Shop:
    func Pay:
        steps:
            desc$?? "检查余额"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(
            module.rules[0].keyword_commitment,
            Commitment::LockedQuestion
        );
        assert_eq!(module.rules[0].desc.content.commitment, Commitment::Locked);
        let Fragment::Func { func } = &module.items[0] else {
            panic!("expected func")
        };
        let Step::Desc { content } = &func.steps[0] else {
            panic!("expected desc step")
        };
        assert_eq!(content.need_commitment, Commitment::LockedQuestionQuestion);
        assert_eq!(content.content.commitment, Commitment::None);
    }

    #[test]
    fn parse_strong_lock_with_question() {
        let src = r#"module Shop:
    func Pay() with PaymentCap$$?:
        steps:
            charge payment
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Func { func } = &module.items[0] else {
            panic!("expected func")
        };
        assert_eq!(
            func.capabilities[0].name.commitment,
            Commitment::StrongLockedQuestion
        );
    }

    #[test]
    fn parse_invalid_lock_order_question_before_lock() {
        let src = r#"rule$? "合法"
rule?$ "非法：不确定在锁之前"
"#;
        let result = parse(src);
        // 第一个合法，第二个非法
        assert!(
            result.errors.iter().any(|e| matches!(e, ParseError::UnexpectedToken { expected, .. } if expected.contains("锁后缀必须在不确定后缀之前"))),
            "expected invalid suffix order error, got {:?}", result.errors
        );
    }

    #[test]
    fn parse_lock_density_does_not_break_existing_fuzzy() {
        let src = r#"module? Shop:
    func Pay??():
        steps:
            check balance
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(module.keyword_commitment, Commitment::Question);
        let Fragment::Func { func } = &module.items[0] else {
            panic!("expected func")
        };
        assert_eq!(func.name.commitment, Commitment::QuestionQuestion);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::render::render_file;

    #[test]
    fn render_and_reparse_simple_module() {
        let src = r#"module Shop:
    type OrderStatus: New | Pending | Paid

    rule "支付必须幂等"
    func Pay(order, amount):
        desc "处理支付"
        requires: order.status == Pending and amount > 0
        ensures: order.status == Paid
        steps:
            check balance
            charge payment
            order.status = Paid to done
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "rendered source failed to parse: {:?}\nrendered:\n{}", reparsed.errors, rendered);
    }

    #[test]
    fn render_preserves_math_precedence() {
        let src = r#"func Compute(x, y):
    math:
        a = (x - y) / 4
        b = x + y * 2
        c = a ** b ** 2
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("(x - y) / 4"), "rendered should preserve parentheses:\n{}", rendered);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_multi_subscript() {
        let src = r#"func Test():
    math:
        v = x[i, j]
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("x[i, j]"), "rendered should contain multi-subscript:\n{}", rendered);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_ui_with_event_binding() {
        let src = r#"module App:
    ui CounterView binds CounterModel:
        stack:
            "当前计数" desc "大号数字"
            "加" desc "按钮" on tap: Increment()
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_lock_suffixes() {
        let src = r#"module$ Shop:
    func Pay$?():
        steps:
            desc$?? "检查余额"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("module$ Shop:"), "rendered should preserve lock suffixes:\n{}", rendered);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_placeholders() {
        let src = r#"type Order: ...
flow Lifecycle: ...
func Pay(order): ...
ui View: ...
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("type Order: ..."), "rendered:\n{}", rendered);
        assert!(rendered.contains("flow Lifecycle: ..."), "rendered:\n{}", rendered);
        assert!(rendered.contains("func Pay(order): ..."), "rendered:\n{}", rendered);
        assert!(rendered.contains("ui View: ..."), "rendered:\n{}", rendered);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_flow_compact_arm() {
        let src = r#"flow Lifecycle:
    New to Active: desc "启动"
    Active:
        to Done: desc "完成"
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("New to Active: desc \"启动\""), "rendered:\n{}", rendered);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_natural_condition() {
        let src = r#"func Pay():
    requires: "payment captured or error"
    ensures: "status is Paid"
    steps:
        charge payment
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_type_hint_with_generics() {
        let src = r#"type X:
    handlers: Map[EventType, List[EventHandler]]
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(
            rendered.contains("handlers: Map[EventType, List[EventHandler]]"),
            "rendered:\n{}",
            rendered
        );
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_top_level_fragments() {
        let src = r#"order.status == Pending and amount > 0

steps:
    check inventory
    charge payment

...
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }
}
