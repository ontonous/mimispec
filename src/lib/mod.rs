pub mod ast;
pub mod cache;
pub mod collaboration;
pub mod diagnostics;
pub mod error;
pub mod format;
pub mod ide;
pub mod latex;
pub mod lexer;
pub mod lossless;
pub mod materialize;
pub mod parser;
pub mod profile;
pub mod query;
pub mod render;
mod render_util;
pub mod resolver;
pub mod session;
pub mod symbol;
pub mod workflow;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use error::ParseResult;
use lexer::Lexer;
use parser::Parser;

/// Return type for [`parse_file()`]: (main ParseResult, all resolved files, non-parse resolve errors).
pub type ParseFileResult = (
    ParseResult,
    HashMap<PathBuf, Arc<ast::File>>,
    Vec<(PathBuf, error::ResolveError)>,
);

/// Parse a MimiSpec source string into an AST, recovering as many fragments as possible.
///
/// This is the main entry point for parsing `.mms` files. Unlike a traditional parser
/// that fails on the first error, this parser employs multi-level error recovery to
/// extract as much structure as possible from the input even when it contains errors.
///
/// The returned [`ParseResult`] contains both the partially-parsed AST (`File`) and
/// any errors encountered. Callers should always check `result.errors.is_empty()`
/// before relying on the AST being complete.
///
/// # Errors
///
/// - Lexer errors (e.g. unterminated strings, invalid escapes) are returned directly
///   without producing tokens, resulting in an empty AST.
/// - Parser errors are accumulated in `result.errors`. Each error includes the error
///   code, source location, message, and optional fix suggestions.
///
/// # Example
///
/// ```rust
/// use mimispec::parse;
///
/// let result = parse("type Status: Active | Inactive");
/// assert!(result.errors.is_empty());
/// assert_eq!(result.file.fragments.len(), 1);
/// ```
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

/// Parse a document while preserving its exact source text and physical layout.
///
/// This opt-in 0.3.1 API keeps the existing semantic AST alongside comments,
/// whitespace, exact newline kinds, paragraph breaks, explicit suffix spans,
/// nested Field/FlowEntry/FlowArm/Step node IDs, line-comment attachment, and
/// structured Fragment moves that carry attached rule preludes.
/// The current semantic parser remains authoritative for AST and diagnostics.
pub fn parse_lossless(source: &str) -> lossless::LosslessParseResult {
    let source: Arc<str> = Arc::from(source);
    match Lexer::new(&source).tokenize() {
        Ok(tokens) => {
            let mut parser = Parser::new_recording(tokens.clone());
            let result = parser.parse_file();
            let recorded = parser.take_recorded_commitments();
            let recorded_nodes = parser.take_recorded_nodes();
            let recorded_rules = parser.take_recorded_rules();
            let document = lossless::build_document(
                source,
                result.file,
                &tokens,
                &recorded,
                &recorded_nodes,
                &recorded_rules,
            );
            lossless::LosslessParseResult {
                document,
                errors: result.errors,
            }
        }
        Err(error) => {
            let file = ast::File {
                imports: vec![],
                rules: vec![],
                fragments: vec![],
            };
            let document = lossless::build_document(source, file, &[], &[], &[], &[]);
            lossless::LosslessParseResult {
                document,
                errors: vec![error],
            }
        }
    }
}

/// Parse a single MimiSpec fragment in isolation (useful for IDE snippet validation).
///
/// Unlike [`parse()`], this function expects only one Fragment (e.g. a single `type`,
/// `func`, or expression). It returns a [`ParseResult`] with at most one fragment.
///
/// # Example
///
/// ```rust
/// use mimispec::parse_fragment;
///
/// let result = parse_fragment("func Hello: ...");
/// assert!(result.errors.is_empty());
/// ```
pub fn parse_fragment(source: &str) -> ParseResult {
    match Lexer::new(source).tokenize() {
        Ok(tokens) => {
            let mut parser = Parser::new(tokens);
            let mut errors = Vec::new();
            let mut fragments = Vec::new();
            parser.skip_newlines();
            match parser.parse_fragment() {
                Ok(f) => fragments.push(f),
                Err(e) => errors.push(e),
            }
            let mut all_errors = parser.take_errors();
            all_errors.extend(errors);
            ParseResult {
                file: ast::File {
                    imports: vec![],
                    rules: Vec::new(),
                    fragments,
                },
                errors: all_errors,
            }
        }
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

/// Tokenize a MimiSpec source string into a sequence of tokens.
///
/// This is a lower-level API that exposes the lexer output directly. Most users
/// should use [`parse()`] instead, which combines tokenization and parsing.
///
/// # Errors
///
/// Returns `Err` if the source contains lexically invalid input (e.g. unterminated
/// strings, invalid escape sequences, or indentation errors).
pub fn tokenize(source: &str) -> Result<Vec<lexer::Token>, error::ParseError> {
    Lexer::new(source).tokenize()
}

// Content hash for IncrementalCache keys.
fn hash_source(source: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// Parse a `.mms` file from disk and resolve its `@import` directives recursively.
///
/// Uses a [`resolver::Resolver`] to handle cross-file resolution with cycle detection.
/// Returns the main file and a map of all resolved files.
///
/// # Errors
///
/// Returns `Err` if the file cannot be read or a cycle is detected. Parse errors
/// within individual files are accumulated and accessible via the returned
/// `Vec<(PathBuf, ResolveError)>`.
pub fn parse_file(path: &Path) -> ParseFileResult {
    let root_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut resolver = resolver::Resolver::new(root_dir);
    let file = resolver.resolve(path);

    let mut r_errors = Vec::new();
    let mut resolve_errors = Vec::new();
    for (p, err) in resolver.take_errors() {
        match err {
            error::ResolveError::ParseFailed {
                errors: parse_errs, ..
            } => {
                r_errors.extend(parse_errs);
            }
            other => {
                resolve_errors.push((p, other));
            }
        }
    }

    let result = if file.is_none() && r_errors.is_empty() {
        // resolve() returned None but no parse errors — the failure is
        // in resolve_errors (IoError, ImportCycle). Surface it in
        // ParseResult.errors so callers checking result.errors see it.
        let msg = resolve_errors
            .first()
            .map(|(p, e)| format!("{}: {}", p.display(), e))
            .unwrap_or_else(|| "unknown resolve error".into());
        ParseResult {
            file: ast::File {
                imports: vec![],
                rules: vec![],
                fragments: vec![],
            },
            errors: vec![error::ParseError::internal(msg, 0, 0)],
        }
    } else {
        ParseResult {
            file: file
                .as_ref()
                .map(|f| ast::File::clone(f))
                .unwrap_or_else(|| ast::File {
                    imports: vec![],
                    rules: vec![],
                    fragments: vec![],
                }),
            errors: r_errors,
        }
    };
    let files = resolver.take_files();
    (result, files, resolve_errors)
}

/// Cache that avoids re-parsing unchanged sources.
///
/// Useful for IDE scenarios where the same file is parsed repeatedly.
/// Uses a content hash of the source as the cache key for space efficiency.
pub struct IncrementalCache {
    cache: HashMap<u64, (String, ParseResult)>,
}

impl Default for IncrementalCache {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Parse `source`, or return a cached result if the source text matches.
    /// The returned `ParseResult` is a clone of the cached entry.
    pub fn parse(&mut self, source: &str) -> ParseResult {
        let key = hash_source(source);
        if let Some((cached_source, cached_result)) = self.cache.get(&key) {
            if cached_source == source {
                return cached_result.clone();
            }
        }
        let result = parse(source);
        self.cache.insert(key, (source.to_string(), result.clone()));
        result
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, ParseError};
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
            "返回" desc "按钮" on tap: >>> TaskPanel
            "保存" desc "按钮" on "提交": Save(state), >>> HomeScreen
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
            combine results >>> done
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
        let Fragment::Steps { steps, .. } = &file.fragments[0] else {
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
        order.status = Paid >>> done
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
        assert!(matches!(&file.fragments[0], Fragment::Placeholder { .. }));
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
        charge payment >>> done
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
        assert!(matches!(step.value, SimpleValue::Placeholder { .. }));
    }

    #[test]
    fn parse_import_directive() {
        let src = r#"@import "common/types.mms"

module UserDomain:
    func GetUser(id):
        requires: id > 0
        steps:
            query database
            return user >>> done
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
    Idle >>> Active: desc "启动"
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
            ParseError {
                code: ErrorCode::E0005,
                line,
                col,
                ..
            } => {
                assert_eq!(*line, 1, "error should be on line 1 (opening quote)");
                assert_eq!(*col, 6, "error should be at column 6 (opening quote)");
            }
            _ => panic!(
                "expected UnterminatedString (E0005), got {:?}",
                result.errors[0]
            ),
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
        assert!(
            !result.errors.is_empty(),
            "expected errors for invalid token"
        );
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
mod edge_case_tests {
    use super::*;
    use crate::ast::*;
    use crate::error::{ErrorCode, ParseError};
    use crate::render::render_file;

    #[test]
    fn parse_empty_input() {
        let result = parse("");
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.file.imports.is_empty());
        assert!(result.file.rules.is_empty());
        assert!(result.file.fragments.is_empty());
    }

    #[test]
    fn parse_only_comments_and_blank_lines() {
        let src = "\n// just a comment\n\n   \n// another\n";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.file.fragments.is_empty());
    }

    #[test]
    fn parse_string_escape_roundtrip() {
        let src = r#"desc "line1\nline2\ttab\"quote""#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "rendered failed: {:?}\n{}",
            reparsed.errors,
            rendered
        );
        let Fragment::Steps { steps, .. } = &reparsed.file.fragments[0] else {
            panic!("expected steps")
        };
        let Step::Desc { content } = &steps[0] else {
            panic!("expected desc")
        };
        assert_eq!(content.content.value, "line1\nline2\ttab\"quote");
    }

    #[test]
    fn parse_string_with_backslash() {
        let src = r#"desc "path\\to\\file""#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(
            rendered.contains("\\\\"),
            "backslash must be escaped in output: {}",
            rendered
        );
    }

    #[test]
    fn parse_deeply_nested_modules() {
        let src = r#"module A:
    module B:
        module C:
            func Deep():
                steps:
                    done
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Module { module: a } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        let Fragment::Module { module: b } = &a.items[0] else {
            panic!("expected nested module")
        };
        let Fragment::Module { module: c } = &b.items[0] else {
            panic!("expected nested module")
        };
        assert_eq!(c.name.name, "C".to_string());
        let Fragment::Func { func } = &c.items[0] else {
            panic!("expected func")
        };
        assert_eq!(func.name.name, "Deep".to_string());
    }

    #[test]
    fn assignment_error_reports_position() {
        let src = r#"func F():
    steps:
        a.b = = 1
"#;
        let result = parse(src);
        assert!(!result.errors.is_empty(), "expected errors");
        assert!(
            result.errors.iter().any(|e| matches!(e, ParseError { code: ErrorCode::E0010, message, .. } if message.contains("single assignment"))),
            "expected single assignment error, got {:?}",
            result.errors
        );
    }

    #[test]
    fn parse_math_right_associative_power() {
        let src = r#"func F():
    math:
        a = 2 ** 3 ** 2
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn render_preserves_string_commitment() {
        let src = r#"desc "hello"$"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("\"hello\"$"), "rendered: {}", rendered);
    }

    #[test]
    fn render_preserves_expression_commitment_slots() {
        let cases = [
            "not$ ready",
            "-$value",
            "~$mask",
            "left and$ right",
            "left or$$? right",
            "item in$?? items",
            "left ==$ right",
            "true$$?",
        ];

        for src in cases {
            let result = parse(src);
            assert!(result.errors.is_empty(), "{src}: {:?}", result.errors);
            let rendered = render_file(&result.file);
            let reparsed = parse(&rendered);
            assert!(
                reparsed.errors.is_empty(),
                "{src} rendered as {rendered:?}: {:?}",
                reparsed.errors
            );
            assert_eq!(result.file, reparsed.file, "source: {src}");
        }
    }

    #[test]
    fn render_preserves_flow_commitment_slots() {
        let src = r#"flow$ Checkout:
    Pending >>>$? Paid$: requires$??: ready and$ verified
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains(">>>$? Paid$"), "rendered: {rendered}");
        assert!(
            rendered.contains("requires$??: ready and$ verified"),
            "rendered: {rendered}"
        );
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn flow_rules_attach_to_entries_and_arms() {
        let src = r#"flow Checkout:
    rule "entry constraint"
    Pending:
        rule "arm constraint"
        >>> Paid:
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Flow { flow } = &result.file.fragments[0] else {
            panic!("expected flow")
        };
        assert_eq!(flow.entries[0].rules.len(), 1);
        assert_eq!(flow.entries[0].arms[0].rules.len(), 1);

        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn rules_before_unsupported_fragments_become_environment_rules() {
        let top_level = parse(
            r#"rule "applies to the file"
steps:
    do work
"#,
        );
        assert!(top_level.errors.is_empty(), "{:?}", top_level.errors);
        assert_eq!(top_level.file.rules.len(), 1);

        let nested = parse(
            r#"module App:
    rule "applies to the module"
    steps:
        do work
"#,
        );
        assert!(nested.errors.is_empty(), "{:?}", nested.errors);
        let Fragment::Module { module } = &nested.file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(module.rules.len(), 1);
    }

    #[test]
    fn render_preserves_simple_bool_commitment() {
        let src = r#"func Configure:
    steps:
        enabled = true$
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("enabled = true$"), "rendered: {rendered}");
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn capability_uses_one_commitment_slot() {
        let src = "func Run with Network$?: ...\n";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(
            func.capabilities[0].name.commitment,
            Commitment::LockedQuestion
        );
        assert_eq!(func.capabilities[0].commitment, Commitment::LockedQuestion);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn parse_func_with_param_type_hints() {
        let src = r#"func Compute(x: Number, y: List[Number]): ..."#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.params.len(), 2);
        assert!(!func.params[0].type_hint.is_empty());
        assert!(!func.params[1].type_hint.is_empty());
    }

    #[test]
    fn parse_ui_leaf_with_requires_and_with() {
        let src = r#"module App:
    ui View:
        stack:
            "item" desc "a leaf" requires state.active with AdminCap on tap: Select()
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn parse_error_recovery_keeps_subsequent_fragments() {
        let src = r#"type A: ...
func Broken(:
flow Good: ...
"#;
        let result = parse(src);
        assert!(!result.errors.is_empty(), "expected errors");
        assert!(result
            .file
            .fragments
            .iter()
            .any(|f| matches!(f, Fragment::TypeDef { typedef } if typedef.name.name == "A")));
        assert!(result
            .file
            .fragments
            .iter()
            .any(|f| matches!(f, Fragment::Flow { flow } if flow.name.name == "Good")));
    }

    #[test]
    fn render_atom_sequence_roundtrip() {
        // Atom 序列空格启发式（needs_space_between）的往返测试。
        // 涵盖标识符、符号、列表字面量的各种组合。
        let cases = vec![
            "Map[K, V]",
            "List[Map[K, V]]",
            "a, b, c",
            "func(a, b) -> Result",
            "x = y",
            "type X: A | B | C",
        ];
        for src in cases {
            let result = parse(src);
            // 部分 case 可能不是合法顶层 fragment，但至少能产生 AST 且往返不 panic
            let rendered = render_file(&result.file);
            // 解析渲染结果：不应有新的错误
            let reparsed = parse(&rendered);
            if !result.errors.is_empty() {
                // 输入本身有错误是预期的（如逗号在顶层无意义）
                continue;
            }
            assert!(
                reparsed.errors.is_empty(),
                "round-trip failed for {src:?}:\nrendered:\n{rendered}\nerrors: {:?}",
                reparsed.errors
            );
        }
    }
}

#[cfg(test)]
mod v0_2_tests {
    use super::*;
    use crate::ast::*;
    use crate::error::ResolveError;
    use crate::render::render_file;
    use crate::resolver::Resolver;
    use crate::symbol::SymbolTable;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let id = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("mimispec_test_{}_{}", prefix, id))
    }

    fn create_temp_mms(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn incremental_cache_reuses_result() {
        let mut cache = IncrementalCache::new();
        let r1 = cache.parse("type X: A | B");
        let r2 = cache.parse("type X: A | B");
        assert!(r1.errors.is_empty());
        assert!(r2.errors.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn incremental_cache_detects_change() {
        let mut cache = IncrementalCache::new();
        let r1 = cache.parse("type X: A | B");
        let r2 = cache.parse("type Y: C | D");
        assert_eq!(cache.len(), 2);
        assert_eq!(r1.file.fragments.len(), 1);
        assert_eq!(r2.file.fragments.len(), 1);
    }

    #[test]
    fn atom_ellipsis_in_action_label() {
        // `...` on its own line becomes Step::Placeholder, but we verify
        // the render round-trip preserves it
        let src = "func F:\n    steps:\n        ...\n";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        assert!(rendered.contains("..."), "rendered: {}", rendered);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }

    #[test]
    fn atom_ellipsis_in_type_hint() {
        // `...` as a type hint on a func param should parse as Atom::Ellipsis
        let src = "func F(x: ...): ...";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.params.len(), 1);
        assert_eq!(func.params[0].type_hint.len(), 1);
        assert!(
            matches!(&func.params[0].type_hint[0], Atom::Ellipsis { .. }),
            "expected Atom::Ellipsis, got {:?}",
            func.params[0].type_hint[0]
        );
    }

    #[test]
    fn placeholder_in_steps_with_desc() {
        // `... desc "text"` on one line: `...` is a Step::Placeholder,
        // `desc "text"` is a separate Step::Desc
        let src = "func F:\n    steps:\n        ... desc \"placeholder step\"\n";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        assert_eq!(func.steps.len(), 2);
        assert!(matches!(&func.steps[0], Step::Placeholder { .. }));
        assert!(matches!(&func.steps[1], Step::Desc { .. }));
    }

    #[test]
    fn simple_value_placeholder_in_assign() {
        let src = r#"
func F:
    steps:
        x = ...
"#;
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Func { func } = &result.file.fragments[0] else {
            panic!("expected func")
        };
        let Step::Assign { step } = &func.steps[0] else {
            panic!("expected assign")
        };
        assert!(
            matches!(step.value, SimpleValue::Placeholder { .. }),
            "expected Placeholder, got {:?}",
            step.value
        );
    }

    #[test]
    fn cross_file_roundtrip() {
        let dir = unique_temp_dir("cross_file_roundtrip");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        create_temp_mms(
            &dir,
            "types.mms",
            "type OrderStatus: New | Paid | Cancelled\n",
        );
        create_temp_mms(
            &dir,
            "main.mms",
            r#"@import "types.mms"

module Shop:
    func Process(order):
        steps:
            check status >>> done
"#,
        );

        let main_path = dir.join("main.mms");
        let mut resolver = Resolver::new(dir.clone());
        resolver.resolve(&main_path);
        let files = resolver.take_files();

        assert!(files.contains_key(&main_path.canonicalize().unwrap()));
        assert!(files.contains_key(&dir.join("types.mms").canonicalize().unwrap()));

        let main_file = files.get(&main_path.canonicalize().unwrap()).unwrap();
        assert_eq!(main_file.imports.len(), 1);
        assert_eq!(main_file.imports[0], "types.mms");

        // Round-trip render
        let rendered = render_file(main_file);
        assert!(rendered.contains("@import"));
        assert!(rendered.contains("module Shop"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cross_file_cycle_detection() {
        let dir = unique_temp_dir("cycle");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        create_temp_mms(&dir, "a.mms", "@import \"b.mms\"\ntype A: X\n");
        create_temp_mms(&dir, "b.mms", "@import \"a.mms\"\ntype B: Y\n");

        let mut resolver = Resolver::new(dir.clone());
        resolver.resolve(&dir.join("a.mms"));
        let errors = resolver.take_errors();

        let has_cycle = errors
            .iter()
            .any(|(_p, e)| matches!(e, ResolveError::ImportCycle { .. }));
        assert!(has_cycle, "expected ImportCycle error, got: {:?}", errors);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn symbol_table_conflict_detection() {
        let dir = unique_temp_dir("symbols");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        create_temp_mms(&dir, "a.mms", "type Order: Pending | Paid\n");
        create_temp_mms(&dir, "b.mms", "type Order: New | Old\n");

        let mut resolver = Resolver::new(dir.clone());
        resolver.resolve(&dir.join("a.mms"));
        resolver.resolve(&dir.join("b.mms"));
        let files = resolver.take_files();

        let symbols = SymbolTable::build(&files);
        let conflicts = symbols.conflicts();

        let order_conflict = conflicts.iter().find(|c| c.name == "Order");
        assert!(
            order_conflict.is_some(),
            "expected conflict for 'Order', got: {:?}",
            conflicts
        );

        if let Some(conflict) = order_conflict {
            assert_eq!(conflict.entries.len(), 2);
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolver_handles_file_not_found() {
        let mut resolver = Resolver::new(PathBuf::from("/nonexistent"));
        resolver.resolve(Path::new("/nonexistent/file.mms"));
        let errors = resolver.take_errors();
        assert!(!errors.is_empty(), "expected errors for nonexistent file");
    }

    #[test]
    fn flow_def_no_desc_field() {
        // FlowDef.desc was removed — verify flow parsing still works
        let src = "flow Lifecycle:\n    Idle >>> Active: desc \"startup\"\n";
        let result = parse(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Flow { flow } = &result.file.fragments[0] else {
            panic!("expected flow")
        };
        assert_eq!(flow.entries.len(), 1);
        // desc is now on the FlowArm, not FlowDef
        assert!(flow.entries[0].arms[0].desc.is_some());
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;
    use crate::ast::Fragment;
    use crate::render::render_file;

    fn build_large_module(items: usize) -> String {
        let mut src = String::from("module Stress:\n");
        for i in 0..items {
            src.push_str(&format!(
                "    func Func{}(x, y):\n        requires: x > 0 and y > 0\n        steps:\n            compute sum\n            return sum >>> done\n",
                i
            ));
        }
        src
    }

    #[test]
    fn stress_parse_large_file() {
        let src = build_large_module(1000);
        let result = parse(&src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.file.fragments.len(), 1);
        let Fragment::Module { module } = &result.file.fragments[0] else {
            panic!("expected module")
        };
        assert_eq!(module.items.len(), 1000);
    }

    #[test]
    fn stress_render_and_reparse_large_file() {
        let src = build_large_module(500);
        let result = parse(&src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
    }
}

// ── Fuzzy matching helpers ────────────────────────────────────────────────────

/// Compute the Levenshtein edit distance between two strings.
///
/// Uses O(m·n) time and O(n) space (optimized with a single-row DP array).
///
/// # Example
///
/// ```rust
/// use mimispec::edit_distance;
///
/// assert_eq!(edit_distance("kitten", "sitting"), 3);
/// assert_eq!(edit_distance("foo", "foo"), 0);
/// ```
pub fn edit_distance(a: &str, b: &str) -> usize {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let m = ac.len();
    let n = bc.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if ac[i - 1] == bc[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Collect all identifier, string, and number names from a token slice for fuzzy lookup.
///
/// This is used internally by the error reporting system to suggest corrections
/// for undefined variables. It extracts all `Ident`, `String`, and `Number` token
/// values into a flat list.
pub fn known_names_from_tokens(tokens: &[crate::lexer::Token]) -> Vec<String> {
    use crate::lexer::TokenKind;
    tokens
        .iter()
        .filter_map(|tok| match &tok.kind {
            TokenKind::Ident(s) | TokenKind::String(s) | TokenKind::Number(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// Find the closest known name to `target` within a maximum edit distance.
///
/// Uses Levenshtein distance to find the best match. Returns `None` if no name
/// is within `max_dist` edits.
///
/// # Example
///
/// ```rust
/// use mimispec::find_suggestion;
///
/// let known = vec!["PAYMENT".into(), "REFUND".into()];
/// assert_eq!(find_suggestion("PAYMEN", &known, 2), Some("PAYMENT".into()));
/// assert_eq!(find_suggestion("FOO", &known, 1), None);
/// ```
pub fn find_suggestion(target: &str, known: &[String], max_dist: usize) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for name in known {
        let d = edit_distance(target, name);
        if d <= max_dist {
            match best {
                None => best = Some((d, name.clone())),
                Some((bd, _)) if d < bd => best = Some((d, name.clone())),
                _ => {}
            }
        }
    }
    best.map(|(_, s)| s)
}

#[cfg(test)]
mod fuzzy_tests {
    use super::*;

    #[test]
    fn edit_distance_identical() {
        assert_eq!(edit_distance("foo", "foo"), 0);
    }
    #[test]
    fn edit_distance_substitution() {
        assert_eq!(edit_distance("foo", "fob"), 1);
    }
    #[test]
    fn edit_distance_insert() {
        assert_eq!(edit_distance("ab", "abc"), 1);
    }
    #[test]
    fn edit_distance_delete() {
        assert_eq!(edit_distance("abc", "ab"), 1);
    }
    #[test]
    fn edit_distance_empty() {
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", ""), 3);
    }
    #[test]
    fn find_suggestion_exact() {
        let known = vec!["FOO".into(), "BAR".into()];
        assert_eq!(find_suggestion("FOO", &known, 3), Some("FOO".into()));
    }
    #[test]
    fn find_suggestion_near_miss() {
        let known = vec!["FOO".into(), "BAR".into()];
        assert_eq!(find_suggestion("FO", &known, 3), Some("FOO".into()));
    }
    #[test]
    fn find_suggestion_too_far() {
        let known = vec!["FOO".into()];
        assert_eq!(find_suggestion("XYZ", &known, 1), None);
    }
    #[test]
    fn find_suggestion_picks_closest() {
        let known = vec!["FOO".into(), "FOOBAR".into()];
        let r = find_suggestion("FOOB", &known, 3);
        assert!(r.is_some());
    }
}
