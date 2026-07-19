//! MimiSpec canonical parser, lossless document, collaboration, and tooling APIs.

pub mod ast;
pub mod cache;
pub mod collaboration;
pub mod conformance;
pub mod diagnostics;
pub mod error;
pub mod format;
pub mod ide;
pub mod latex;
pub mod lexer;
pub mod lossless;
pub mod lsp;
#[cfg(feature = "experimental-targets")]
pub mod materialize;
pub mod parser;
#[cfg(feature = "experimental-targets")]
pub mod profile;
pub mod protocol;
#[cfg(feature = "experimental-provenance")]
pub mod provenance;
pub mod query;
pub mod render;
mod render_util;
pub mod resolver;
pub mod session;
pub mod symbol;
pub mod usability;
#[cfg(feature = "experimental-targets")]
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
        Ok(tokens) => {
            let mut result = Parser::new(tokens).parse_file();
            error::enrich_action_syntax_errors(source, &mut result.errors);
            result
        }
        Err(e) => ParseResult::new(
            ast::File {
                imports: vec![],
                fragments: vec![],
            },
            vec![e],
        ),
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
            let mut result = parser.parse_file();
            error::enrich_action_syntax_errors(&source, &mut result.errors);
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
                status: result.status,
            }
        }
        Err(error) => {
            let file = ast::File {
                imports: vec![],
                fragments: vec![],
            };
            let document = lossless::build_document(source, file, &[], &[], &[], &[]);
            lossless::LosslessParseResult {
                document,
                errors: vec![error],
                status: error::ParseStatus::Partial,
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
            let mut parser = Parser::new_recording(tokens.clone());
            let mut result = parser.parse_file();
            error::enrich_action_syntax_errors(source, &mut result.errors);
            let root_items = parser.take_root_item_tokens();
            let recorded_rules = parser.take_recorded_rules();
            let mut unit_starts = root_items;
            let mut environment_rules: Vec<_> = recorded_rules
                .iter()
                .filter_map(|rule| match rule.decision {
                    parser::RecordedRuleDecision::Environment { scope_token: None } => {
                        Some(rule.tokens.clone())
                    }
                    _ => None,
                })
                .collect();
            environment_rules.sort_by_key(|range| range.start);
            let mut previous_end = None;
            for range in environment_rules {
                let begins_new_chain = previous_end.is_none_or(|end| {
                    tokens[end..range.start]
                        .iter()
                        .filter(|token| matches!(token.kind, lexer::TokenKind::Newline))
                        .count()
                        >= 3
                });
                if begins_new_chain {
                    unit_starts.push(range.start);
                }
                previous_end = Some(range.end);
            }
            unit_starts.sort_unstable();
            unit_starts.dedup();
            if let Some(offending) = unit_starts.get(1).and_then(|index| tokens.get(*index)) {
                result.push_error(error::ParseError::unexpected_token(
                    "additional Context Unit".into(),
                    "exactly one Context Unit (one item with its rule prelude, or one terminal rule chain)"
                        .into(),
                    offending.line,
                    offending.col,
                ));
            }
            result
        }
        Err(e) => ParseResult::new(
            ast::File {
                imports: vec![],
                fragments: vec![],
            },
            vec![e],
        ),
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
        ParseResult::new(
            ast::File {
                imports: vec![],
                fragments: vec![],
            },
            vec![error::ParseError::internal(msg, 0, 0)],
        )
    } else {
        ParseResult::new(
            file.as_ref()
                .map(|f| ast::File::clone(f))
                .unwrap_or_else(|| ast::File {
                    imports: vec![],
                    fragments: vec![],
                }),
            r_errors,
        )
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
        let Some(UiNode::Stack { stack }) = ui.root() else {
            panic!("expected stack")
        };
        let children = stack.children();
        assert_eq!(children.len(), 4);

        // Check navigation action
        let UiNode::Leaf { leaf } = children[0] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert!(
            matches!(&on.action.actions[0], Action::Navigate { target } if target.name == "TaskPanel")
        );

        // Check composite action
        let UiNode::Leaf { leaf } = children[1] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert_eq!(on.action.actions.len(), 2);

        // Check assign action
        let UiNode::Leaf { leaf } = children[2] else {
            panic!("expected leaf")
        };
        let on = leaf.on.as_ref().unwrap();
        assert!(matches!(&on.action.actions[0], Action::Assign { .. }));

        // Check natural language action
        let UiNode::Leaf { leaf } = children[3] else {
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
        let Some(UiNode::Stack { stack }) = ui.root() else {
            panic!("expected stack")
        };
        let children = stack.children();
        let UiNode::Leaf { leaf } = children[0] else {
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
        assert_eq!(func.steps().len(), 2);
        match func.steps()[0] {
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
        let Fragment::Steps { items, .. } = &file.fragments[0] else {
            panic!("expected steps")
        };
        assert_eq!(items.len(), 3);
        assert!(matches!(
            &items[0],
            Fragment::Step {
                step: Step::Action { .. }
            }
        ));
        assert!(matches!(
            &items[1],
            Fragment::Step {
                step: Step::If { .. }
            }
        ));
        assert!(matches!(
            &items[2],
            Fragment::Step {
                step: Step::Action { .. }
            }
        ));
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
        println!("file.rules: {}", result1.file.rules().len());
        let Fragment::Module { module } = &result1.file.fragments[1] else {
            panic!()
        };
        for r in result1.file.rules() {
            println!("  - {}", r.desc.content.value);
        }
        assert_eq!(result1.file.rules().len(), 1);
        assert_eq!(
            result1.file.rules()[0].attachment,
            RuleAttachment::Attached { target_index: 1 }
        );
        assert_eq!(result1.file.rules()[0].desc.content.value, "约束module");
        assert!(module.rules().is_empty());

        // 场景2: 空行阻断，rule 变为全局
        let src2 = r#"rule "全局约束"

module Shop:
    func Pay:
        steps:
            check balance"#;
        let result2 = parse(src2);
        println!("\n=== 场景2: 空行阻断 ===");
        println!("file.rules: {}", result2.file.rules().len());
        for r in result2.file.rules() {
            println!("  - {}", r.desc.content.value);
        }
        assert_eq!(result2.file.rules().len(), 1, "空行阻断后 rule 应变为全局");
        assert_eq!(
            result2.file.rules()[0].attachment,
            RuleAttachment::Environment
        );
        assert_eq!(result2.file.rules()[0].desc.content.value, "全局约束");

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
        println!("module.rules(): {}", module.rules().len());
        for r in module.rules() {
            println!("  - {}", r.desc.content.value);
        }
        assert_eq!(module.rules().len(), 1);
        assert_eq!(module.rules()[0].desc.content.value, "模块级约束");
        assert_eq!(module.rules()[0].attachment, RuleAttachment::Environment);
        assert_eq!(
            module.items.len(),
            3,
            "module 应保留 rule、type 和 func 顺序"
        );
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
        assert_eq!(result.file.rules().len(), 3);
        assert_eq!(result.file.fragments.len(), 4);
        let Fragment::Module { module } = &result.file.fragments[3] else {
            panic!("expected module fragment")
        };
        assert!(module.rules().is_empty());
        let rules = result.file.rules();
        assert!(rules
            .iter()
            .all(|rule| { rule.attachment == RuleAttachment::Attached { target_index: 3 } }));
        assert_eq!(rules[0].desc.content.value, "rule A");
        assert_eq!(rules[1].desc.content.value, "rule B");
        assert_eq!(rules[2].desc.content.value, "rule C");
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
        // Rule 作为有序 item 保留；真实空行使其成为 Environment。
        assert_eq!(file.fragments.len(), 3);
        assert!(matches!(&file.fragments[0], Fragment::TypeDef { .. }));
        assert!(matches!(&file.fragments[1], Fragment::Rule { .. }));
        assert!(matches!(&file.fragments[2], Fragment::Steps { .. }));
        assert_eq!(file.rules().len(), 1);
        assert_eq!(file.rules()[0].attachment, RuleAttachment::Environment);
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
        assert_eq!(func.steps().len(), 3);
        assert!(matches!(func.steps()[1], Step::Placeholder { .. }));
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
        assert!(func.requires().is_empty());
        assert!(func.ensures().is_empty());
        assert!(func.steps().is_empty());
        assert!(matches!(
            func.items.as_slice(),
            [Fragment::Placeholder { .. }]
        ));
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
        assert!(func.steps().is_empty());
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
        assert_eq!(func.requires().len(), 1);
        let cond = &func.requires()[0].condition;
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
        assert!(matches!(
            &typedef.body,
            TypeBody::Record { items }
                if matches!(items.as_slice(), [Fragment::Placeholder { .. }])
        ));
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
        assert_eq!(flow.name.as_ref().unwrap().name, "Lifecycle");
        assert!(flow.entries().is_empty());
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
        assert!(ui.is_placeholder());
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
        assert_eq!(func.steps().len(), 1);
        let Step::Assign { step } = &func.steps()[0] else {
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
            .any(|f| matches!(f, Fragment::Flow { flow } if flow.name.as_ref().is_some_and(|name| name.name == "Lifecycle"))));
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
        let fields = typedef.fields();
        assert_eq!(fields.len(), 3);

        assert_eq!(fields[0].name.name, "id");
        let id_rules = rules_attached_to(typedef.items(), 1);
        assert_eq!(id_rules.len(), 1);
        assert_eq!(id_rules[0].desc.content.value, "id 必须大于 0");

        assert_eq!(fields[1].name.name, "status");
        let status_rules = rules_attached_to(typedef.items(), 4);
        assert_eq!(status_rules.len(), 2);
        assert_eq!(status_rules[0].desc.content.value, "status 必须有效");
        assert_eq!(
            status_rules[1].desc.content.value,
            "status 不能是 Cancelled"
        );

        assert_eq!(fields[2].name.name, "amount");
        assert!(rules_attached_to(typedef.items(), 5).is_empty());
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
        let Fragment::TypeDef { typedef } = &file.fragments[1] else {
            panic!("expected type")
        };

        assert_eq!(file.rules().len(), 1);
        assert_eq!(file.rules()[0].desc.content.value, "type-level constraint");
        assert_eq!(
            file.rules()[0].attachment,
            RuleAttachment::Attached { target_index: 1 }
        );

        let fields = typedef.fields();
        assert_eq!(fields.len(), 1);
        let field_rules = rules_attached_to(typedef.items(), 1);
        assert_eq!(field_rules.len(), 1);
        assert_eq!(field_rules[0].desc.content.value, "field-level constraint");
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
            func.steps().len(),
            1,
            "expected 1 step after skipping invalid 'if :'"
        );
        assert!(
            matches!(&func.steps()[0], Step::Assign { .. }),
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
        let fields = typedef.fields();
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
        let fields = typedef.fields();
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
        let math = func.math_blocks()[0];
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
        let math = func.math_blocks()[0];
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
        let requires = func.requires();
        let Condition::Structured { expr } = &requires[0].condition else {
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
        let math = func.math_blocks()[0];
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
        let math = func.math_blocks()[0];
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
        let math = func.math_blocks()[0];
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
        assert_eq!(module.math_blocks().len(), 1);
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
        assert!(typedef
            .items()
            .iter()
            .any(|item| matches!(item, Fragment::Math { .. })));
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
        assert!(result.file.rules().is_empty());
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
        let Fragment::Desc { desc } = &reparsed.file.fragments[0] else {
            panic!("expected root desc")
        };
        assert_eq!(desc.content.value, "line1\nline2\ttab\"quote");
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
        assert_eq!(flow.rules().len(), 1);
        let entry = flow.entries()[0];
        assert_eq!(entry.rules().len(), 1);
        assert_eq!(entry.arms().len(), 1);

        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(reparsed.errors.is_empty(), "{:?}", reparsed.errors);
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn rules_before_all_context_items_attach_uniformly() {
        let top_level = parse(
            r#"rule "applies to the file"
steps:
    do work
"#,
        );
        assert!(top_level.errors.is_empty(), "{:?}", top_level.errors);
        assert_eq!(
            top_level.file.rules()[0].attachment,
            RuleAttachment::Attached { target_index: 1 }
        );

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
        assert_eq!(module.rules().len(), 1);
        assert_eq!(
            module.rules()[0].attachment,
            RuleAttachment::Attached { target_index: 1 }
        );
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
            .any(|f| matches!(f, Fragment::Flow { flow } if flow.name.as_ref().is_some_and(|name| name.name == "Good"))));
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
        assert_eq!(func.steps().len(), 2);
        assert!(matches!(&func.steps()[0], Step::Placeholder { .. }));
        assert!(matches!(&func.steps()[1], Step::Desc { .. }));
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
        let Step::Assign { step } = &func.steps()[0] else {
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
        assert_eq!(flow.entries().len(), 1);
        // desc is now on the FlowArm, not FlowDef
        assert_eq!(flow.entries()[0].arms()[0].descs().len(), 1);
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

    #[test]
    fn stress_collect_semantic_slots_large_file() {
        let src = build_large_module(1000);
        let result = parse_lossless(&src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let slots = crate::collaboration::collect_semantic_slot_snapshots(&result.document);
        assert!(
            slots.len() >= 5_000,
            "unexpected slot count: {}",
            slots.len()
        );
        assert!(slots
            .iter()
            .all(|slot| result.document.node(slot.node).is_some()));
    }

    /// Consolidated 0.3 performance guard: slot count must scale linearly with
    /// module size, and every slot must resolve to a live node. This is a
    /// deterministic regression guard (no wall-clock dependency). Measured
    /// baseline numbers live in `docs/roadmap-0.3.x.md` §10 and in
    /// `examples/perf_baseline.rs`.
    #[test]
    fn stress_slot_count_scales_linearly_with_module_size() {
        let counts: Vec<usize> = [250, 500, 1000, 2000]
            .iter()
            .map(|&n| {
                let src = build_large_module(n);
                let result = parse_lossless(&src);
                assert!(result.errors.is_empty(), "n={n}: {:?}", result.errors);
                crate::collaboration::collect_semantic_slot_snapshots(&result.document).len()
            })
            .collect();

        // Each func contributes a fixed number of slots (~20). The per-func
        // yield must not regress: divide through by module size.
        let per_func: Vec<f64> = counts
            .iter()
            .zip([250, 500, 1000, 2000])
            .map(|(&slots, n)| slots as f64 / n as f64)
            .collect();

        // Sanity: every size yields a positive, finite per-func slot count.
        assert!(
            per_func.iter().all(|x| *x > 0.0 && x.is_finite()),
            "per-func slot yield must be positive and finite: {per_func:?}"
        );

        // Linearity guard: doubling module size must double slot count within
        // ±5%. If this fails, either slot collection gained a quadratic term
        // or a per-func slot was silently added/removed — both are 0.3
        // release blockers.
        for window in per_func.windows(2) {
            let ratio = window[1] / window[0];
            assert!(
                (0.95..=1.05).contains(&ratio),
                "slot-per-func ratio out of linear band: {ratio} (per_func={per_func:?}, counts={counts:?})"
            );
        }

        // Absolute floor: 1000 funcs must yield at least 10K slots. If this
        // drops, a slot category was lost silently.
        assert!(
            counts[2] >= 10_000,
            "1000-func slot count dropped below 10K: {}",
            counts[2]
        );
    }
}

#[cfg(test)]
mod v0_3_core_target_tests {
    use super::*;
    use crate::ast::{Commitment, Fragment, Step, UiNode};
    use crate::lossless::RuleAttachment;
    use crate::render::render_file;

    fn json(file: &crate::ast::File) -> serde_json::Value {
        serde_json::to_value(file).expect("AST must serialize")
    }

    fn count_kind(value: &serde_json::Value, expected: &str) -> usize {
        match value {
            serde_json::Value::Array(values) => {
                values.iter().map(|value| count_kind(value, expected)).sum()
            }
            serde_json::Value::Object(map) => {
                usize::from(map.get("kind").and_then(|kind| kind.as_str()) == Some(expected))
                    + map
                        .values()
                        .map(|value| count_kind(value, expected))
                        .sum::<usize>()
            }
            _ => 0,
        }
    }

    #[test]
    fn root_desc_and_clauses_are_first_class_context_items() {
        let result = parse(
            r#"desc "提交前确认外部结果"
requires: amount > 0
ensures: committed == true
"#,
        );
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let value = json(&result.file);
        assert_eq!(count_kind(&value, "Desc"), 1, "{value:#}");
        assert_eq!(count_kind(&value, "Clause"), 2, "{value:#}");
        assert_eq!(count_kind(&value, "Action"), 0, "{value:#}");
    }

    #[test]
    fn repeated_descriptions_and_clauses_are_never_overwritten() {
        let result = parse(
            r#"func Pay(order, amount):
    desc "处理支付"
    desc "订单系统保持提交权威"
    requires: order.status == Pending
    requires$: amount > 0
    ensures: order.status == Paid
    ensures$: audit.recorded == true
"#,
        );
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let value = json(&result.file);
        assert_eq!(count_kind(&value, "Desc"), 2, "{value:#}");
        assert_eq!(count_kind(&value, "Clause"), 4, "{value:#}");

        let rendered = render_file(&result.file);
        assert_eq!(rendered.matches("    desc").count(), 2, "{rendered}");
        assert_eq!(rendered.matches("    requires").count(), 2, "{rendered}");
        assert_eq!(rendered.matches("    ensures").count(), 2, "{rendered}");
    }

    #[test]
    fn descriptions_before_enum_variants_remain_descriptions() {
        let result = parse(
            r#"type FailureScope:
    desc "传播责任"
    desc "恢复责任"
    Local | Peer | External
"#,
        );
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let value = json(&result.file);
        assert_eq!(count_kind(&value, "Desc"), 2, "{value:#}");
        let rendered = render_file(&result.file);
        assert_eq!(rendered.matches("    desc").count(), 2, "{rendered}");
        assert!(rendered.contains("Local | Peer | External"), "{rendered}");
    }

    #[test]
    fn flow_accepts_anonymous_name_and_event_labels() {
        let result = parse(
            r#"flow$:
    Pending:
        on CaptureConfirmed$ >>> Paid$: desc$ "确认后提交"
        on CancelAccepted >>> Cancelled: requires: allowed == true
"#,
        );
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let value = json(&result.file);
        assert_eq!(count_kind(&value, "Flow"), 1, "{value:#}");
        let rendered = render_file(&result.file);
        assert!(rendered.starts_with("flow$:\n"), "{rendered}");
        assert!(
            rendered.contains("on CaptureConfirmed$ >>> Paid$"),
            "{rendered}"
        );
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "{:?}\n{rendered}",
            reparsed.errors
        );
    }

    #[test]
    fn every_context_item_can_receive_a_rule_prelude() {
        let source = r#"rule "约束步骤"
// comment-only line does not break the paragraph
steps:
    do work
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.document.rules().len(), 1);
        assert_eq!(
            result.document.rules()[0].attachment,
            RuleAttachment::Attached
        );
    }

    #[test]
    fn physical_blank_line_creates_environment_attachment() {
        let source = r#"rule "当前 Context 约束"

steps:
    do work
"#;
        let result = parse_lossless(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.document.rules().len(), 1);
        assert_eq!(
            result.document.rules()[0].attachment,
            RuleAttachment::Environment
        );
    }

    #[test]
    fn parse_fragment_rejects_an_unrelated_trailing_context_unit() {
        let result = parse_fragment("desc \"one\"\ndesc \"two\"\n");
        assert!(
            !result.errors.is_empty(),
            "trailing input must not be ignored"
        );
        assert_eq!((result.errors[0].line, result.errors[0].col), (2, 1));
        assert!(result.is_partial());
    }

    #[test]
    fn parse_fragment_distinguishes_terminal_rule_chains() {
        let one_chain = parse_fragment("rule \"one\"\nrule \"two\"\n");
        assert!(one_chain.errors.is_empty(), "{:?}", one_chain.errors);

        let two_chains = parse_fragment("rule \"one\"\n\nrule \"two\"\n");
        assert!(!two_chains.errors.is_empty());
        assert_eq!(
            (two_chains.errors[0].line, two_chains.errors[0].col),
            (3, 1)
        );
    }

    #[test]
    fn renderer_preserves_environment_then_attached_rule_boundary() {
        let source = r#"desc "first"
rule "document environment"

rule "clause prelude"
requires$: ready
desc "last"
"#;
        let result = parse(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let rules = result.file.rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].attachment, crate::ast::RuleAttachment::Environment);
        assert!(matches!(
            rules[1].attachment,
            crate::ast::RuleAttachment::Attached { .. }
        ));

        let rendered = render_file(&result.file);
        assert!(
            rendered.contains(
                "rule \"document environment\"\n\nrule \"clause prelude\"\nrequires$: ready"
            ),
            "{rendered}"
        );
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "{:?}\n{rendered}",
            reparsed.errors
        );
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn flow_preserves_multiple_inline_tails_and_placeholder_commitment() {
        let source = r#"flow$?:
    Pending:
        on$ Capture?? >>>$? Paid$: requires$: ready ensures?: committed desc$ "first" desc "second"
"#;
        let result = parse(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Flow { flow } = &result.file.fragments[0] else {
            panic!("expected flow")
        };
        let arm = flow.entries()[0].arms()[0];
        assert_eq!(arm.items.len(), 4);
        assert_eq!(arm.clauses().len(), 2);
        assert_eq!(arm.descs().len(), 2);
        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "{:?}\n{rendered}",
            reparsed.errors
        );
        assert_eq!(result.file, reparsed.file);

        let placeholder = parse("flow Draft: ...$??\n");
        assert!(placeholder.errors.is_empty(), "{:?}", placeholder.errors);
        let rendered = render_file(&placeholder.file);
        assert_eq!(rendered, "flow Draft: ...$??\n");
        assert_eq!(placeholder.file, parse(&rendered).file);
    }

    #[test]
    fn nested_step_scopes_use_ordered_items_and_rule_attachments() {
        let source = r#"steps:
    if ready:
        rule "attached to work"
        do work
        rule "branch environment"

        do later
"#;
        let result = parse(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Steps { items, .. } = &result.file.fragments[0] else {
            panic!("expected steps")
        };
        let Fragment::Step {
            step: Step::If { step },
        } = &items[0]
        else {
            panic!("expected if")
        };
        assert_eq!(step.then_branch.len(), 4);
        let Fragment::Rule { rule: attached } = &step.then_branch[0] else {
            panic!("expected attached rule")
        };
        assert_eq!(
            attached.attachment,
            crate::ast::RuleAttachment::Attached { target_index: 1 }
        );
        let Fragment::Rule { rule: environment } = &step.then_branch[2] else {
            panic!("expected environment rule")
        };
        assert_eq!(
            environment.attachment,
            crate::ast::RuleAttachment::Environment
        );

        let lossless = parse_lossless(source);
        assert!(lossless.errors.is_empty(), "{:?}", lossless.errors);
        assert_eq!(lossless.document.rules().len(), 2);
        assert_eq!(
            lossless.document.rules()[0].attachment,
            RuleAttachment::Attached
        );
        assert_eq!(
            lossless.document.rules()[1].attachment,
            RuleAttachment::Environment
        );

        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "{:?}\n{rendered}",
            reparsed.errors
        );
        assert_eq!(result.file, reparsed.file);
    }

    #[test]
    fn ui_scopes_preserve_rule_items_and_commitment_slots() {
        let source = r#"ui$ View binds? Model$:
    rule "root prelude"
    stack$ "root":
        rule "child prelude"
        "button" desc "primary"
"#;
        let result = parse(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let Fragment::Ui { ui } = &result.file.fragments[0] else {
            panic!("expected ui")
        };
        assert_eq!(ui.binds_keyword_commitment, Commitment::Question);
        assert_eq!(ui.rules().len(), 1);
        assert_eq!(
            ui.rules()[0].attachment,
            crate::ast::RuleAttachment::Attached { target_index: 1 }
        );
        let Some(UiNode::Stack { stack }) = ui.root() else {
            panic!("expected stack")
        };
        assert_eq!(stack.rules().len(), 1);
        assert_eq!(stack.children().len(), 1);

        let lossless = parse_lossless(source);
        assert!(lossless.errors.is_empty(), "{:?}", lossless.errors);
        assert_eq!(lossless.document.rules().len(), 2);
        assert!(lossless
            .document
            .rules()
            .iter()
            .all(|rule| rule.attachment == RuleAttachment::Attached));
        assert!(
            lossless
                .document
                .nodes()
                .iter()
                .filter(|node| node.kind == crate::lossless::SourceNodeKind::UiNode)
                .count()
                >= 2
        );

        let rendered = render_file(&result.file);
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "{:?}\n{rendered}",
            reparsed.errors
        );
        assert_eq!(result.file, reparsed.file);

        let placeholder = parse("ui View: ...$$?\n");
        assert!(placeholder.errors.is_empty(), "{:?}", placeholder.errors);
        assert_eq!(render_file(&placeholder.file), "ui View: ...$$?\n");
    }

    #[test]
    fn comment_trivia_and_real_blank_lines_have_distinct_attachment() {
        let attached = parse("rule \"attached\"\n// comment-only trivia\ndesc \"target\"\n");
        assert!(attached.errors.is_empty(), "{:?}", attached.errors);
        assert!(matches!(
            attached.file.rules()[0].attachment,
            crate::ast::RuleAttachment::Attached { .. }
        ));

        let environment =
            parse("rule \"environment\"\n// comment-only trivia\n\ndesc \"target\"\n");
        assert!(environment.errors.is_empty(), "{:?}", environment.errors);
        assert_eq!(
            environment.file.rules()[0].attachment,
            crate::ast::RuleAttachment::Environment
        );
    }

    #[test]
    fn recovery_is_explicitly_partial_and_json_is_versioned() {
        let source = "rule \"protected\"\nfunc Broken(:\ntype Good: ...\n";
        let result = parse(source);
        assert!(result.is_partial());
        assert_eq!(result.status, crate::error::ParseStatus::Partial);
        assert!(matches!(
            result.file.rules()[0].attachment,
            crate::ast::RuleAttachment::UnresolvedByRecovery
        ));
        let value = json(&result.file);
        assert_eq!(value["schema_version"], crate::ast::AST_SCHEMA_VERSION);

        let lossless = parse_lossless(source);
        assert!(lossless.is_partial());
        assert_eq!(lossless.status, crate::error::ParseStatus::Partial);
        assert_eq!(
            lossless.document.rules()[0].attachment,
            RuleAttachment::DroppedByRecovery
        );
    }

    #[test]
    fn recovery_keeps_following_reserved_context_items() {
        let source = r#"func Broken(:
desc "preserved"
requires: ready
ensures: completed
math:
    total = 1
"#;
        let result = parse(source);
        assert!(result.is_partial());
        let value = json(&result.file);
        assert_eq!(count_kind(&value, "Desc"), 1, "{value:#}");
        assert_eq!(count_kind(&value, "Clause"), 2, "{value:#}");
        assert_eq!(count_kind(&value, "Math"), 1, "{value:#}");
    }

    #[test]
    fn context_queries_expose_descriptions_clauses_and_environment_rules() {
        use std::sync::Arc;

        let result = parse("desc \"context\"\nrequires: ready\nrule \"environment\"\n");
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let query = crate::query::FileQuery::new(Arc::new(result.file));
        assert_eq!(query.items().count(), 3);
        assert_eq!(query.descriptions().count(), 1);
        assert_eq!(query.clauses().count(), 1);
        assert_eq!(query.environment_rules().count(), 1);
    }
}

// ── Consolidated 0.3 cross-domain acceptance corpus
//
// Exercises the consolidated 0.3 roadmap §10 corpus covering plain-language product
// intent; state transitions and forbidden behavior; failure and recovery;
// resource ownership and permissions; ordered communication; external
// boundaries; multilingual descriptions; and one cohesive real-world product
// document.
//
// The corpus files live under `docs/corpora/` and are the canonical
// cross-domain intent fixtures. This test embeds them with `include_str!`
// so a syntax change that breaks any corpus fails CI immediately, with the
// file name in the panic message. Each corpus must:
//   1. parse cleanly (no errors, Complete status);
//   2. round-trip through the semantic renderer with an equivalent AST;
//   3. serialize as valid 0.3 JSON carrying `schema_version`.
//
// A regression here is a 0.3 release blocker: it means the parser
// silently lost or rejected a real-world intent pattern.
#[cfg(test)]
mod corpus_acceptance_tests {
    use super::*;
    use crate::ast::{Fragment, AST_SCHEMA_VERSION};
    use crate::render::render_file;

    const PLAIN_PRODUCT_INTENT: &str = include_str!("../../docs/corpora/plain-product-intent.mms");
    const STATE_TRANSITIONS: &str = include_str!("../../docs/corpora/state-transitions.mms");
    const FAILURE_AND_RECOVERY: &str = include_str!("../../docs/corpora/failure-and-recovery.mms");
    const RESOURCE_OWNERSHIP: &str = include_str!("../../docs/corpora/resource-ownership.mms");
    const ORDERED_COMMUNICATION: &str =
        include_str!("../../docs/corpora/ordered-communication.mms");
    const EXTERNAL_BOUNDARIES: &str = include_str!("../../docs/corpora/external-boundaries.mms");
    const MULTILINGUAL: &str = include_str!("../../docs/corpora/multilingual.mms");
    const REAL_WORLD_FAMILY_LEDGER: &str =
        include_str!("../../docs/corpora/real-world-family-ledger.mms");
    const MIMI_KV_REAL_PROJECT: &str = include_str!("../../docs/corpora/mimi-kv-real-project.mms");
    const MIMICHAT_REAL_PROJECT: &str =
        include_str!("../../docs/corpora/mimichat-real-project.mms");
    const MIMI_MARKDOWN_REAL_PROJECT: &str =
        include_str!("../../docs/corpora/mimi-markdown-real-project.mms");
    const MIMI_LOG_REAL_PROJECT: &str =
        include_str!("../../docs/corpora/mimi-log-real-project.mms");

    fn assert_corpus_round_trips(name: &'static str, src: &'static str) {
        let first = parse(src);
        assert!(
            first.errors.is_empty(),
            "corpus {name} failed to parse: {:?}",
            first.errors
        );
        assert_eq!(
            first.status,
            error::ParseStatus::Complete,
            "corpus {name} returned partial status"
        );

        let rendered = render_file(&first.file);
        let reparsed = parse(&rendered);
        assert!(
            reparsed.errors.is_empty(),
            "corpus {name} reparsed with errors: {:?}\nrendered:\n{rendered}",
            reparsed.errors
        );
        assert_eq!(
            first.file, reparsed.file,
            "corpus {name} AST diverged after round-trip"
        );

        let value = serde_json::to_value(&first.file)
            .unwrap_or_else(|e| panic!("corpus {name} failed to serialize: {e}"));
        assert_eq!(
            value
                .as_object()
                .and_then(|o| o.get("schema_version"))
                .and_then(|v| v.as_str()),
            Some(AST_SCHEMA_VERSION),
            "corpus {name} missing schema_version"
        );
    }

    #[test]
    fn corpus_plain_product_intent_round_trips() {
        assert_corpus_round_trips("plain-product-intent", PLAIN_PRODUCT_INTENT);
    }

    #[test]
    fn corpus_state_transitions_round_trips() {
        assert_corpus_round_trips("state-transitions", STATE_TRANSITIONS);
    }

    #[test]
    fn corpus_failure_and_recovery_round_trips() {
        assert_corpus_round_trips("failure-and-recovery", FAILURE_AND_RECOVERY);
    }

    #[test]
    fn corpus_resource_ownership_round_trips() {
        assert_corpus_round_trips("resource-ownership", RESOURCE_OWNERSHIP);
    }

    #[test]
    fn corpus_ordered_communication_round_trips() {
        assert_corpus_round_trips("ordered-communication", ORDERED_COMMUNICATION);
    }

    #[test]
    fn corpus_external_boundaries_round_trips() {
        assert_corpus_round_trips("external-boundaries", EXTERNAL_BOUNDARIES);
    }

    #[test]
    fn corpus_multilingual_round_trips() {
        assert_corpus_round_trips("multilingual", MULTILINGUAL);
    }

    #[test]
    fn corpus_real_world_family_ledger_is_usable_end_to_end() {
        use crate::diagnostics::{analyze_document, DiagnosticClass};

        assert_corpus_round_trips("real-world-family-ledger", REAL_WORLD_FAMILY_LEDGER);

        let lossless = parse_lossless(REAL_WORLD_FAMILY_LEDGER);
        assert_eq!(
            lossless.document.render_lossless(),
            REAL_WORLD_FAMILY_LEDGER
        );
        assert_eq!(lossless.status, error::ParseStatus::Complete);

        let all_items = crate::query::collect_fragments(&lossless.document.semantic().fragments);
        assert_eq!(
            all_items
                .iter()
                .filter(|item| matches!(item, Fragment::TypeDef { .. }))
                .count(),
            3
        );
        assert_eq!(
            all_items
                .iter()
                .filter(|item| matches!(item, Fragment::Flow { .. }))
                .count(),
            1
        );
        assert_eq!(
            all_items
                .iter()
                .filter(|item| matches!(item, Fragment::Func { .. }))
                .count(),
            3
        );
        assert_eq!(
            all_items
                .iter()
                .filter(|item| matches!(item, Fragment::Ui { .. }))
                .count(),
            2
        );

        let report = analyze_document(&lossless.document, &lossless.errors);
        assert_eq!(report.delegation_queue.len(), 2);
        assert!(report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.class != DiagnosticClass::IntentGap));
        assert!(report.diagnostics.iter().all(|diagnostic| {
            !matches!(
                diagnostic.class,
                DiagnosticClass::Syntax | DiagnosticClass::Attachment
            )
        }));
    }

    struct RealProjectExpectations {
        types: usize,
        flows: usize,
        funcs: usize,
        modules: usize,
        slots: usize,
        decisions: usize,
        delegations: usize,
    }

    fn assert_real_project_transcription(
        name: &'static str,
        src: &'static str,
        expected: RealProjectExpectations,
    ) {
        use crate::diagnostics::{analyze_document, DiagnosticClass};

        assert_corpus_round_trips(name, src);
        let lossless = parse_lossless(src);
        assert_eq!(lossless.document.render_lossless(), src);
        assert_eq!(lossless.status, error::ParseStatus::Complete);

        let all_items = crate::query::collect_fragments(&lossless.document.semantic().fragments);
        for (label, actual, expected) in [
            (
                "types",
                all_items
                    .iter()
                    .filter(|item| matches!(item, Fragment::TypeDef { .. }))
                    .count(),
                expected.types,
            ),
            (
                "flows",
                all_items
                    .iter()
                    .filter(|item| matches!(item, Fragment::Flow { .. }))
                    .count(),
                expected.flows,
            ),
            (
                "functions",
                all_items
                    .iter()
                    .filter(|item| matches!(item, Fragment::Func { .. }))
                    .count(),
                expected.funcs,
            ),
            (
                "modules",
                all_items
                    .iter()
                    .filter(|item| matches!(item, Fragment::Module { .. }))
                    .count(),
                expected.modules,
            ),
        ] {
            assert_eq!(actual, expected, "{name} lost {label}");
        }

        let report = analyze_document(&lossless.document, &lossless.errors);
        assert_eq!(report.summary.commit_ready, 0);
        assert_eq!(
            report.summary.total_slots, expected.slots,
            "{name} slot drift"
        );
        assert_eq!(
            report.decision_queue.len(),
            expected.decisions,
            "{name} decision queue drift"
        );
        assert_eq!(
            report.delegation_queue.len(),
            expected.delegations,
            "{name} delegation queue drift"
        );
        assert!(
            report.diagnostics.iter().all(|diagnostic| {
                matches!(
                    diagnostic.class,
                    DiagnosticClass::Decision | DiagnosticClass::Delegation
                )
            }),
            "{name} has a non-queue diagnostic: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn corpus_mimi_kv_real_project_transcription_round_trips() {
        assert_real_project_transcription(
            "mimi-kv-real-project",
            MIMI_KV_REAL_PROJECT,
            RealProjectExpectations {
                types: 4,
                flows: 2,
                funcs: 7,
                modules: 2,
                slots: 401,
                decisions: 48,
                delegations: 0,
            },
        );
    }

    #[test]
    fn corpus_mimichat_real_project_transcription_round_trips() {
        assert_real_project_transcription(
            "mimichat-real-project",
            MIMICHAT_REAL_PROJECT,
            RealProjectExpectations {
                types: 4,
                flows: 2,
                funcs: 13,
                modules: 6,
                slots: 680,
                decisions: 73,
                delegations: 0,
            },
        );
    }

    #[test]
    fn corpus_mimi_markdown_real_project_transcription_round_trips() {
        assert_real_project_transcription(
            "mimi-markdown-real-project",
            MIMI_MARKDOWN_REAL_PROJECT,
            RealProjectExpectations {
                types: 4,
                flows: 2,
                funcs: 13,
                modules: 5,
                slots: 649,
                decisions: 70,
                delegations: 0,
            },
        );
    }

    #[test]
    fn corpus_mimi_log_real_project_transcription_round_trips() {
        assert_real_project_transcription(
            "mimi-log-real-project",
            MIMI_LOG_REAL_PROJECT,
            RealProjectExpectations {
                types: 5,
                flows: 2,
                funcs: 14,
                modules: 5,
                slots: 752,
                decisions: 79,
                delegations: 0,
            },
        );
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

// ── Consolidated 0.3 multilingual / Unicode acceptance tests
//
// Covers the consolidated 0.3 multilingual roadmap deliverable:
//   "Multilingual: Unicode descriptions and rules preserve exact content."
//
// MimiSpec treats natural language as a first-class citizen. The parser must
// not silently normalize, transcode, or truncate CJK content, emoji, or
// combining-mark sequences in `desc` / `rule` / string-literal payloads.
// Identifier syntax remains ASCII-only by design (see lexer `is_ident_start`),
// so these tests only exercise string-content invariants.
#[cfg(test)]
mod multilingual_tests {
    use super::*;
    use crate::ast::Fragment;
    use crate::lossless::ColumnEncoding;
    use crate::render::render_file;

    fn desc_text(file: &crate::ast::File) -> &str {
        let Fragment::Desc { desc } = &file.fragments[0] else {
            panic!("expected a Desc fragment");
        };
        &desc.content.value
    }

    #[test]
    fn cjk_description_round_trips_byte_for_byte() {
        let src = "desc \"老人也可以轻松使用\"";
        let first = parse(src);
        assert!(first.errors.is_empty(), "{:?}", first.errors);
        assert_eq!(desc_text(&first.file), "老人也可以轻松使用");

        let rendered = render_file(&first.file);
        assert_eq!(rendered, "desc \"老人也可以轻松使用\"\n", "{rendered}");

        let second = parse(&rendered);
        assert!(second.errors.is_empty(), "{:?}", second.errors);
        assert_eq!(first.file, second.file, "AST diverged after round-trip");
    }

    #[test]
    fn cjk_rule_text_preserves_punctuation_and_emoji() {
        let src = "rule \"支付必须幂等 💰 — 不允许重复扣款\"";
        let first = parse(src);
        assert!(first.errors.is_empty(), "{:?}", first.errors);
        let Fragment::Rule { rule } = &first.file.fragments[0] else {
            panic!("expected a Rule fragment");
        };
        assert_eq!(rule.desc.content.value, "支付必须幂等 💰 — 不允许重复扣款");

        let rendered = render_file(&first.file);
        let reparsed = parse(&rendered);
        assert_eq!(first.file, reparsed.file, "rule text diverged after render");
    }

    #[test]
    fn cjk_field_and_value_strings_round_trip() {
        let src = "type 账户:\n    名称: \"家庭账户\"\n    余额: \"¥1,234\"\n";
        // Note: `账户`, `名称`, `余额` are type/field NAMES, which lexer
        // restricts to ASCII. This must parse-fail cleanly rather than panic
        // or silently mis-tokenize.
        let result = parse(src);
        assert!(
            !result.errors.is_empty(),
            "CJK identifiers must be rejected, not silently accepted: {:?}",
            result.errors
        );
        // But the parser must not panic and must produce a stable partial AST.
        assert_eq!(result.status, error::ParseStatus::Partial);
    }

    #[test]
    fn combining_marks_are_not_normalized() {
        // é as a single codepoint vs 'e' + U+0301 combining acute. The parser
        // must preserve the exact byte sequence the author wrote, not the NFC
        // form.
        let composed = "desc \"café\"";
        let decomposed = "desc \"cafe\u{0301}\"";
        let parsed_composed = parse(composed);
        let parsed_decomposed = parse(decomposed);
        assert!(parsed_composed.errors.is_empty());
        assert!(parsed_decomposed.errors.is_empty());
        assert_eq!(desc_text(&parsed_composed.file), "café");
        assert_eq!(desc_text(&parsed_decomposed.file), "cafe\u{0301}");
        assert_ne!(
            desc_text(&parsed_composed.file),
            desc_text(&parsed_decomposed.file),
            "parser must not silently normalize Unicode"
        );
        // Round-trip preserves the exact form.
        assert_eq!(render_file(&parsed_composed.file), "desc \"café\"\n");
        assert_eq!(
            render_file(&parsed_decomposed.file),
            "desc \"cafe\u{0301}\"\n"
        );
    }

    #[test]
    fn lossless_column_is_unicode_scalar_for_cjk() {
        // A CJK char occupies 3 UTF-8 bytes but 1 column. The lexer column
        // must use Unicode scalar count, not byte count, or downstream span
        // math would point past the string. This is a 0.3.1 invariant.
        let src = "desc \"老人也可以轻松使用\"";
        let result = parse_lossless(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        // Find the string literal token in the token stream and verify its
        // start column is at scalar offset 6 (after `desc `).
        let string_token = tokenize(src)
            .expect("tokenize must succeed")
            .into_iter()
            .find(|t| matches!(t.kind, lexer::TokenKind::String(_)))
            .expect("a String token must exist");
        assert_eq!(
            string_token.col, 6,
            "CJK description string must start at Unicode-scalar column 6, got {}",
            string_token.col
        );

        // The lossless document must render back the exact source, including
        // multi-byte CJK content, without truncation or replacement.
        assert_eq!(
            result.document.render_lossless(),
            src,
            "lossless render must byte-match CJK source"
        );

        // Smoke-check that span queries don't panic on CJK input.
        assert!(!result.document.nodes().is_empty());
        let _ = ColumnEncoding::UnicodeScalar;
    }

    #[test]
    fn multilingual_corpus_round_trips() {
        let cases = &[
            // Simplified Chinese
            "desc \"我想做一个帮助家庭记录日常开销的应用\"",
            // Traditional Chinese
            "desc \"我想做一個幫助家庭記錄日常開銷的應用\"",
            // Japanese (hiragana, katakana, kanji)
            "desc \"家族の日常の出費を記録するアプリを作りたい\"",
            // Korean (Hangul)
            "desc \"가족의 일상 지출을 기록하는 앱을 만들고 싶어요\"",
            // Arabic (RTL)
            "desc \"أريد إنشاء تطبيق يساعد العائلة في تتبع المصاريف اليومية\"",
            // Cyrillic
            "desc \"хочу создать приложение для учёта семейных расходов\"",
            // Mixed emoji + Latin
            "rule \"all payments must be idempotent 🔒💵\"",
        ];
        for &src in cases {
            let first = parse(src);
            assert!(
                first.errors.is_empty(),
                "multilingual case failed to parse: {:?}\nsrc: {src}",
                first.errors
            );
            let rendered = render_file(&first.file);
            let reparsed = parse(&rendered);
            assert_eq!(
                first.file, reparsed.file,
                "multilingual round-trip diverged\nsrc: {src}\nrendered: {rendered}"
            );
        }
    }
}

// ── Consolidated 0.3 property and fuzz tests
//
// These tests cover the consolidated 0.3 parser/formatter property gate
// deliverable without pulling in external proptest/quickcheck dependencies
// (the crate intentionally stays zero-dev-dependency). They use a deterministic
// linear congruential generator so failures are reproducible from the seed
// printed in the panic message.
//
// Invariants covered:
//   1. Idempotent render — for any cleanly-parsed source S,
//      `parse(render(parse(S))).file == parse(S).file`.
//   2. Render determinism — the same AST renders to byte-identical output.
//   3. AST JSON serializes and carries the 0.3 schema version.
//   4. Lossless no-panic — arbitrary byte input never panics `parse_lossless`.
//   5. Error containment — `errors.is_empty()` agrees with `status == Complete`
//      and disagrees with `status == Partial`.
//   6. Tokenizer-then-parser equivalence — `parse(S)` and the explicit
//      `Lexer::new(S).tokenize()` + `Parser::new(t).parse_file()` path produce
//      the same `File` on the OK branch.
//
// These are CI gates: a regression in any of these invariants is a silent
// intent-loss bug, which the 0.3 release gate explicitly forbids.
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::ast::AST_SCHEMA_VERSION;
    use crate::render::render_file;

    /// Deterministic, seedable pseudo-random generator.
    ///
    /// PCG-style LCG with odd increment: cheap, good distribution for test
    /// generation, and fully reproducible from `seed`. The seed is printed in
    /// every assertion message so any failure can be replayed locally.
    struct Lcg {
        state: u64,
    }

    impl Lcg {
        fn new(seed: u64) -> Self {
            // Avoid the all-zero state, which would otherwise stay at 0.
            Self {
                state: seed.wrapping_add(0x9e37_79b9_7f4a_7c15),
            }
        }

        fn next_u64(&mut self) -> u64 {
            // Numerical Recipes LCG constants, with a permutation mix.
            self.state = self
                .state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // xorshift mix for better low-bit entropy.
            let mut x = self.state;
            x ^= x >> 33;
            x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
            x ^= x >> 33;
            x
        }

        fn range(&mut self, lo: usize, hi_inclusive: usize) -> usize {
            assert!(hi_inclusive >= lo, "invalid range");
            if hi_inclusive == lo {
                return lo;
            }
            lo + (self.next_u64() as usize) % (hi_inclusive - lo + 1)
        }

        /// Pick a `&str` from a slice of string literals. Returns the borrowed
        /// string directly to avoid the `&&str` double-reference that a fully
        /// generic `pick<T>` would produce on `&[&str]`.
        fn pick_str<'a>(&mut self, slice: &'a [&'a str]) -> &'a str {
            assert!(!slice.is_empty(), "pick_str from empty slice");
            let idx = self.range(0, slice.len() - 1);
            slice[idx]
        }

        fn bool(&mut self) -> bool {
            self.next_u64() & 1 == 1
        }
    }

    // A pool of independently-valid Context Items. Each entry parses cleanly
    // on its own. The generator concatenates a random subset of these (with
    // blank-line separators) to exercise the cross-item boundary handling.
    const VALID_ITEMS: &[&str] = &[
        "desc \"记录家庭日常开销\"",
        "desc?? \"辅助 AI 完成方案设计\"",
        "rule \"老人也可以轻松使用\"",
        "rule \"财务数据默认只保存在本地\"",
        "rule$ \"支付必须幂等\"",
        "type Status: Pending | Paid | Cancelled",
        "type Account:\n    id: Int\n    balance: Int\n",
        "func Pay(order, amount):\n    requires: amount > 0\n    ensures: order.paid\n    steps:\n        charge card\n        return receipt >>> done\n",
        "func Hello: ...",
        "flow:\n    Pending:\n        on Confirm >>> Paid:\n        on Cancel >>> Cancelled:\n",
        "ui Summary:\n    stack:\n        \"余额\"",
        "steps:\n    prepare data\n    commit",
        "requires: amount > 0\nensures: committed",
        "...",
    ];

    // A pool of fragment prefixes that take a rule prelude, exercising the
    // attached-vs-environment rule attachment distinction.
    const RULE_PRELUDE_TARGETS: &[&str] = &[
        "steps:\n    do work",
        "type Color: Red | Green | Blue",
        "func Hello: ...",
        "ui Card:\n    stack:\n        \"hi\"",
    ];

    fn gen_valid_source(rng: &mut Lcg) -> String {
        let n_items = rng.range(1, 5);
        let mut src = String::new();
        for i in 0..n_items {
            if i > 0 {
                // Physical blank line separates top-level Context Items.
                src.push('\n');
            }
            // Occasionally prepend a rule prelude to the next item.
            if rng.bool() {
                let prelude_count = rng.range(1, 3);
                for _ in 0..prelude_count {
                    src.push_str(rng.pick_str(&[
                        "rule \"前置约束\"",
                        "rule$ \"锁定约束\"",
                        "rule?? \"待确认约束\"",
                    ]));
                    src.push('\n');
                }
                // 50% of the time, follow with a blank line so the prelude
                // becomes an environment rule rather than attached.
                if rng.bool() {
                    src.push('\n');
                }
            }
            let item = if rng.bool() {
                rng.pick_str(VALID_ITEMS)
            } else {
                rng.pick_str(RULE_PRELUDE_TARGETS)
            };
            src.push_str(item);
            src.push('\n');
        }
        src
    }

    /// Any byte slice the caller cares to throw at the parser. We build these
    /// from a small alphabet that stresses indentation, string boundaries,
    /// suffix characters, and CJK content without producing invalid UTF-8.
    fn gen_arbitrary_bytes(rng: &mut Lcg) -> String {
        let alphabet: &[&str] = &[
            "desc ",
            "rule ",
            "func ",
            "type ",
            "flow ",
            "ui ",
            "steps ",
            "  ",
            "    ",
            "\n",
            "\"",
            ":",
            "|",
            "$",
            "?",
            "??",
            "(",
            ")",
            ".",
            ">",
            "<",
            "=",
            "+",
            "-",
            "*",
            "/",
            "and",
            "or",
            "not",
            "on",
            ">>>",
            "requires",
            "ensures",
            "x",
            "y",
            "1",
            "0",
            "中文",
            "支付",
            "记录",
            "余额",
            "老人",
            "财务",
            "🔐",
            "\n\n",
            "Pending",
            "Paid",
            "{",
            "}",
            "[",
            "]",
            "@",
            "#",
            "// comment",
            "/* block */",
        ];
        let n = rng.range(0, 40);
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(rng.pick_str(alphabet));
        }
        s
    }

    fn assert_parse_entry_does_not_panic<F>(seed: u64, src: &str, name: &str, f: F)
    where
        F: FnOnce() + std::panic::UnwindSafe,
    {
        match std::panic::catch_unwind(f) {
            Ok(()) => {}
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
                    .unwrap_or("<non-string panic payload>");
                panic!(
                    "seed {seed}: parse entry `{name}` panicked on arbitrary input \
                     (this is a 0.3 release blocker)\nsrc: {src:?}\npayload: {msg}"
                );
            }
        }
    }

    #[test]
    fn property_idempotent_render_across_seeds() {
        for seed in 0u64..64 {
            let mut rng = Lcg::new(seed);
            for _ in 0..8 {
                let src = gen_valid_source(&mut rng);
                let first = parse(&src);
                if !first.errors.is_empty() {
                    // Generator produced something the parser rejected —
                    // surface it loudly so we can either fix the generator
                    // or the parser, but never silently accept it.
                    panic!(
                        "seed {seed} produced parse errors: {:?}\nsrc:\n{src}",
                        first.errors
                    );
                }
                let rendered = render_file(&first.file);
                let second = parse(&rendered);
                assert!(
                    second.errors.is_empty(),
                    "seed {seed}: reparsed rendered output had errors: {:?}\nrendered:\n{rendered}",
                    second.errors
                );
                assert_eq!(
                    first.file, second.file,
                    "seed {seed}: round-trip altered AST\nrendered:\n{rendered}"
                );
            }
        }
    }

    #[test]
    fn property_render_is_deterministic() {
        for seed in 0u64..32 {
            let mut rng = Lcg::new(seed.wrapping_mul(101));
            let src = gen_valid_source(&mut rng);
            let result = parse(&src);
            // The generator is supposed to produce clean input; a parse error
            // here means either the generator or the parser rotted. Surface
            // it loudly — never `continue` past it.
            assert!(
                result.errors.is_empty(),
                "seed {seed}: generator produced unparseable source: {:?}\nsrc:\n{src}",
                result.errors
            );
            let r1 = render_file(&result.file);
            let r2 = render_file(&result.file);
            assert_eq!(r1, r2, "seed {seed}: render is not deterministic");
        }
    }

    #[test]
    fn property_ast_json_is_serializable_and_versioned() {
        for seed in 0u64..32 {
            let mut rng = Lcg::new(seed.wrapping_mul(202));
            let src = gen_valid_source(&mut rng);
            let result = parse(&src);
            // Even on partial parse the AST must serialize — partial AST is
            // still a valid 0.3 document.
            let value = serde_json::to_value(&result.file).expect("AST must serialize");
            let obj = value
                .as_object()
                .expect("serialized File must be a JSON object");
            assert_eq!(
                obj.get("schema_version").and_then(|v| v.as_str()),
                Some(AST_SCHEMA_VERSION),
                "seed {seed}: missing or wrong schema_version"
            );
        }
    }

    #[test]
    fn fuzz_lossless_never_panics_on_arbitrary_input() {
        for seed in 0u64..128 {
            let mut rng = Lcg::new(seed.wrapping_mul(303));
            for _ in 0..4 {
                let src = gen_arbitrary_bytes(&mut rng);
                // The invariant: this call must never panic, regardless of
                // input. Errors are fine; aborts are not. We MUST assert
                // `is_ok()` — silently dropping the `Err` with `let _` turns
                // this into a test that can only pass, never fail.
                let s = src.clone();
                assert_parse_entry_does_not_panic(seed, &src, "parse_lossless", || {
                    let _ = parse_lossless(&s);
                });
                let s = src.clone();
                assert_parse_entry_does_not_panic(seed, &src, "parse", || {
                    let _ = parse(&s);
                });
                let s = src.clone();
                assert_parse_entry_does_not_panic(seed, &src, "parse_fragment", || {
                    let _ = parse_fragment(&s);
                });
                let s = src.clone();
                assert_parse_entry_does_not_panic(seed, &src, "tokenize", || {
                    let _ = tokenize(&s);
                });
            }
        }
    }

    #[test]
    fn property_error_status_is_consistent() {
        for seed in 0u64..64 {
            let mut rng = Lcg::new(seed.wrapping_mul(404));
            // Mix valid and arbitrary input.
            let src = if rng.bool() {
                gen_valid_source(&mut rng)
            } else {
                gen_arbitrary_bytes(&mut rng)
            };
            let result = parse(&src);
            let errors_empty = result.errors.is_empty();
            let complete = result.status == error::ParseStatus::Complete;
            assert_eq!(
                errors_empty,
                complete,
                "seed {seed}: status {:?} inconsistent with errors.len() = {}",
                result.status,
                result.errors.len()
            );
            // Same invariant for the lossless path.
            let lossless = parse_lossless(&src);
            let lerrors_empty = lossless.errors.is_empty();
            let lcomplete = lossless.status == error::ParseStatus::Complete;
            assert_eq!(
                lerrors_empty,
                lcomplete,
                "seed {seed}: lossless status {:?} inconsistent with errors.len() = {}",
                lossless.status,
                lossless.errors.len()
            );
        }
    }

    #[test]
    fn property_lossless_semantics_match_parse() {
        for seed in 0u64..64 {
            let mut rng = Lcg::new(seed.wrapping_mul(454));
            let src = if rng.bool() {
                gen_valid_source(&mut rng)
            } else {
                gen_arbitrary_bytes(&mut rng)
            };
            let semantic = parse(&src);
            let lossless = parse_lossless(&src);
            assert_eq!(
                lossless.document.semantic(),
                &semantic.file,
                "seed {seed}: parse_lossless semantic AST diverged from parse()\nsrc: {src:?}"
            );
            assert_eq!(
                lossless.errors, semantic.errors,
                "seed {seed}: parse_lossless diagnostics diverged from parse()\nsrc: {src:?}"
            );
            assert_eq!(
                lossless.status, semantic.status,
                "seed {seed}: parse_lossless status diverged from parse()\nsrc: {src:?}"
            );
        }
    }

    #[test]
    fn property_tokenize_then_parse_matches_parse() {
        for seed in 0u64..32 {
            let mut rng = Lcg::new(seed.wrapping_mul(505));
            let src = gen_valid_source(&mut rng);
            let direct = parse(&src);
            // The generator is supposed to produce clean input. A parse error
            // here means generator or parser rot; never silently `continue`.
            assert!(
                direct.errors.is_empty(),
                "seed {seed}: generator produced unparseable source: {:?}\nsrc:\n{src}",
                direct.errors
            );
            let tokens = tokenize(&src).unwrap_or_else(|err| {
                panic!("seed {seed}: tokenize failed on parseable source: {err}")
            });
            let via_tokens = {
                let mut parser = parser::Parser::new(tokens);
                parser.parse_file()
            };
            assert_eq!(
                direct.file, via_tokens.file,
                "seed {seed}: tokenize-then-parse diverged from direct parse (File)"
            );
            assert_eq!(
                direct.errors, via_tokens.errors,
                "seed {seed}: tokenize-then-parse diverged from direct parse (errors)"
            );
            assert_eq!(
                direct.status, via_tokens.status,
                "seed {seed}: tokenize-then-parse diverged from direct parse (status)"
            );
        }
    }
}
