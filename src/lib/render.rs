use crate::ast::*;
use crate::render_util::{expr_prec, paren_if};

/// Render the AST back into valid MimiSpec source code.
///
/// This performs a pretty-print of the AST, producing normalized `.mms` output.
/// Useful for:
/// - Formatting/pretty-printing existing `.mms` files
/// - Round-trip validation (parse → render → re-parse should yield the same AST)
/// - Code generation backends
///
/// # Example
///
/// ```rust
/// use mimispec::{parse, render::render_file};
///
/// let result = parse("type Status: Active | Inactive");
/// let output = render_file(&result.file);
/// assert!(output.starts_with("type Status:"));
/// ```
pub fn render_file(file: &File) -> String {
    let mut renderer = Renderer::new();
    renderer.render_file(file);
    renderer.finish()
}

struct Renderer {
    buf: String,
    indent: usize,
    indent_cache: String,
    pending_blank: bool,
}

impl Renderer {
    fn new() -> Self {
        Self {
            buf: String::new(),
            indent: 0,
            indent_cache: String::new(),
            pending_blank: false,
        }
    }

    fn finish(self) -> String {
        self.buf.trim_end().to_string() + "\n"
    }

    fn push(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    fn newline(&mut self) {
        self.buf.push('\n');
    }

    fn write_indent(&mut self) {
        self.buf.push_str(&self.indent_cache);
    }

    fn set_indent(&mut self, level: usize) {
        self.indent = level;
        self.indent_cache = "    ".repeat(level);
    }

    fn blank_line(&mut self) {
        self.pending_blank = true;
    }

    fn flush_blank(&mut self) {
        if self.pending_blank {
            self.newline();
            self.pending_blank = false;
        }
    }

    fn render_file(&mut self, file: &File) {
        for imp in &file.imports {
            self.write_indent();
            self.push("@import \"");
            self.push(imp);
            self.push("\"");
            self.newline();
        }
        if !file.imports.is_empty() {
            self.blank_line();
        }

        self.render_items(&file.fragments, true);
    }

    fn render_rule(&mut self, rule: &RuleDef) {
        self.write_indent();
        self.push("rule");
        self.push(&rule.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_fstring(&rule.desc.content));
        self.newline();
    }

    fn render_fragment(&mut self, fragment: &Fragment) {
        self.flush_blank();
        match fragment {
            Fragment::Desc { desc } => self.render_desc(desc),
            Fragment::Rule { rule } => self.render_rule(rule),
            Fragment::Clause { clause } => self.render_clause(clause),
            Fragment::Module { module } => self.render_module(module),
            Fragment::TypeDef { typedef } => self.render_type_def(typedef),
            Fragment::Flow { flow } => self.render_flow(flow),
            Fragment::Func { func } => self.render_func(func),
            Fragment::Ui { ui } => self.render_ui(ui),
            Fragment::Steps {
                keyword_commitment,
                items,
            } => {
                self.write_indent();
                self.push("steps");
                self.push(&keyword_commitment.to_string());
                self.push(":");
                self.newline();
                self.render_steps_block(items);
            }
            Fragment::Step { step } => self.render_step(step),
            Fragment::Expr { expr } => {
                self.write_indent();
                self.push(&render_expr(expr));
                self.newline();
            }
            Fragment::UiNode { node } => {
                self.render_ui_node(node);
            }
            Fragment::Math { math } => self.render_math_block(math),
            Fragment::Field { field } => self.render_field(field),
            Fragment::Variants { variants } => self.render_variant_line(variants, false),
            Fragment::FlowEntry { entry } => self.render_flow_entry(entry),
            Fragment::FlowArm { arm } => self.render_flow_arm(arm),
            Fragment::Placeholder { keyword_commitment } => {
                self.write_indent();
                self.push("...");
                self.push(&keyword_commitment.to_string());
                self.newline();
            }
        }
    }

    fn render_items(&mut self, items: &[Fragment], separate_structures: bool) {
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                let previous = &items[index - 1];
                let attached_transition = matches!(previous, Fragment::Rule { rule }
                    if matches!(rule.attachment, RuleAttachment::Attached { target_index } if target_index == index));
                let consecutive_rules = matches!(previous, Fragment::Rule { .. })
                    && matches!(item, Fragment::Rule { .. });
                let current_continues_environment_chain = matches!(item, Fragment::Rule { rule }
                    if matches!(rule.attachment, RuleAttachment::Environment | RuleAttachment::UnresolvedByRecovery));
                let environment_boundary = matches!(previous, Fragment::Rule { rule }
                    if matches!(rule.attachment, RuleAttachment::Environment | RuleAttachment::UnresolvedByRecovery))
                    && (!consecutive_rules || !current_continues_environment_chain);
                let structure_boundary = separate_structures
                    && !matches!(
                        item,
                        Fragment::Desc { .. } | Fragment::Clause { .. } | Fragment::Step { .. }
                    )
                    && !matches!(
                        previous,
                        Fragment::Desc { .. } | Fragment::Clause { .. } | Fragment::Rule { .. }
                    );
                if !attached_transition && (environment_boundary || structure_boundary) {
                    self.blank_line();
                }
            }
            self.render_fragment(item);
        }
    }

    fn render_module(&mut self, module: &Module) {
        self.write_indent();
        self.push("module");
        self.push(&module.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_ident(&module.name));
        self.push(":");
        self.newline();
        self.set_indent(self.indent + 1);
        if module.items.is_empty() {
            self.write_indent();
            self.push("...");
            self.newline();
        } else {
            self.render_items(&module.items, true);
        }
        self.set_indent(self.indent - 1);
    }

    fn render_type_def(&mut self, typedef: &TypeDef) {
        self.write_indent();
        self.push("type");
        self.push(&typedef.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_ident(&typedef.name));
        self.push(":");

        match &typedef.body {
            TypeBody::Enum {
                inline: true,
                items,
            } if items.len() == 1 => {
                if let Fragment::Variants { variants } = &items[0] {
                    self.push(" ");
                    self.render_variant_values(variants);
                    self.newline();
                } else {
                    self.render_type_block(items);
                }
            }
            TypeBody::Enum { items, .. } | TypeBody::Record { items } => {
                if items.len() == 1 && matches!(items[0], Fragment::Placeholder { .. }) {
                    let Fragment::Placeholder { keyword_commitment } = &items[0] else {
                        unreachable!()
                    };
                    self.push(" ...");
                    self.push(&keyword_commitment.to_string());
                    self.newline();
                } else {
                    self.render_type_block(items);
                }
            }
        }
    }

    fn render_type_block(&mut self, items: &[Fragment]) {
        self.newline();
        self.set_indent(self.indent + 1);
        self.render_items(items, false);
        self.set_indent(self.indent - 1);
    }

    fn render_field(&mut self, field: &Field) {
        self.write_indent();
        self.push(&render_ident(&field.name));
        self.push(":");
        if !field.type_hint.is_empty() {
            self.push(" ");
            self.push(&render_atoms(&field.type_hint));
        }
        self.newline();
    }

    fn render_flow(&mut self, flow: &FlowDef) {
        self.write_indent();
        self.push("flow");
        self.push(&flow.keyword_commitment.to_string());
        if let Some(name) = &flow.name {
            self.push(" ");
            self.push(&render_ident(name));
        }
        self.push(":");
        if flow.items.len() == 1 && matches!(flow.items[0], Fragment::Placeholder { .. }) {
            let Fragment::Placeholder { keyword_commitment } = &flow.items[0] else {
                unreachable!()
            };
            self.push(" ...");
            self.push(&keyword_commitment.to_string());
            self.newline();
        } else {
            self.newline();
            self.set_indent(self.indent + 1);
            self.render_items(&flow.items, false);
            self.set_indent(self.indent - 1);
        }
    }

    fn render_flow_entry(&mut self, entry: &FlowEntry) {
        self.write_indent();
        self.push(&render_ident(&entry.state));
        if entry.items.len() == 1 {
            let Fragment::FlowArm { arm } = &entry.items[0] else {
                self.push(":");
                self.newline();
                self.set_indent(self.indent + 1);
                self.render_items(&entry.items, false);
                self.set_indent(self.indent - 1);
                return;
            };
            self.push(" ");
            self.render_flow_arm_content(arm);
            self.newline();
        } else {
            self.push(":");
            self.newline();
            self.set_indent(self.indent + 1);
            self.render_items(&entry.items, false);
            self.set_indent(self.indent - 1);
        }
    }

    fn render_flow_arm(&mut self, arm: &FlowArm) {
        self.write_indent();
        self.render_flow_arm_content(arm);
        self.newline();
    }

    fn render_flow_arm_content(&mut self, arm: &FlowArm) {
        if let Some(event) = &arm.event {
            self.push("on");
            self.push(&event.keyword_commitment.to_string());
            self.push(" ");
            match &event.name {
                EventName::Ident { value } => self.push(&render_ident(value)),
                EventName::Natural { text } => self.push(&render_fstring(text)),
            }
            self.push(" ");
        }
        self.push(">>>");
        self.push(&arm.to_keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_ident(&arm.to));
        self.push(":");
        for item in &arm.items {
            self.push(" ");
            match item {
                Fragment::Clause { clause } => self.push(&render_clause_inline(clause)),
                Fragment::Desc { desc } => self.push(&render_desc_inline(desc)),
                _ => {}
            }
        }
    }

    fn render_func(&mut self, func: &FuncDef) {
        self.write_indent();
        self.push("func");
        self.push(&func.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_ident(&func.name));
        if !func.params.is_empty() {
            self.push("(");
            self.push(&render_params(&func.params));
            self.push(")");
        }
        if !func.capabilities.is_empty() {
            self.push(" with ");
            self.push(&render_capabilities(&func.capabilities));
        }
        self.push(":");
        if func.items.len() == 1 && matches!(func.items[0], Fragment::Placeholder { .. }) {
            let Fragment::Placeholder { keyword_commitment } = &func.items[0] else {
                unreachable!()
            };
            self.push(" ...");
            self.push(&keyword_commitment.to_string());
            self.newline();
            return;
        }
        self.newline();
        self.set_indent(self.indent + 1);
        self.render_items(&func.items, false);
        self.set_indent(self.indent - 1);
    }

    fn render_clause(&mut self, clause: &Clause) {
        self.write_indent();
        self.push(&render_clause_inline(clause));
        self.newline();
    }

    fn render_variant_line(&mut self, variants: &[Ident], leading_pipe: bool) {
        self.write_indent();
        if leading_pipe {
            self.push("| ");
        }
        self.render_variant_values(variants);
        self.newline();
    }

    fn render_variant_values(&mut self, variants: &[Ident]) {
        for (index, variant) in variants.iter().enumerate() {
            if index > 0 {
                self.push(" | ");
            }
            self.push(&render_ident(variant));
        }
    }

    fn render_desc(&mut self, desc: &Desc) {
        self.write_indent();
        self.push("desc");
        self.push(&desc.need_commitment.to_string());
        self.push(" ");
        self.push(&render_fstring(&desc.content));
        self.newline();
    }

    fn render_math_block(&mut self, math: &MathBlock) {
        self.write_indent();
        self.push("math");
        self.push(&math.keyword_commitment.to_string());
        self.push(":");
        self.newline();
        self.set_indent(self.indent + 1);
        for stmt in &math.statements {
            self.write_indent();
            self.push(&render_math_statement(stmt));
            self.newline();
        }
        self.set_indent(self.indent - 1);
    }

    fn render_steps_block(&mut self, items: &[Fragment]) {
        self.set_indent(self.indent + 1);
        self.render_items(items, false);
        self.set_indent(self.indent - 1);
    }

    fn render_step_items(&mut self, items: &[Fragment]) {
        self.set_indent(self.indent + 1);
        self.render_items(items, false);
        self.set_indent(self.indent - 1);
    }

    fn render_step(&mut self, step: &Step) {
        match step {
            Step::Action { step } => self.render_action_step(step),
            Step::Assign { step } => self.render_assign_step(step),
            Step::If { step } => self.render_if_step(step),
            Step::For { step } => self.render_for_step(step),
            Step::While { step } => self.render_while_step(step),
            Step::Parasteps { step } => self.render_parasteps_step(step),
            Step::Error { step } => self.render_error_step(step),
            Step::Desc { content } => self.render_desc(content),
            Step::Placeholder { keyword_commitment } => {
                self.write_indent();
                self.push("...");
                self.push(&keyword_commitment.to_string());
                self.newline();
            }
        }
    }

    fn render_action_step(&mut self, step: &ActionStep) {
        self.write_indent();
        if !step.label.is_empty() {
            self.push(&render_atoms(&step.label));
        }
        if let Some(desc) = &step.desc {
            self.push(" ");
            self.push(&render_desc_inline(desc));
        }
        if let Some(to) = &step.to {
            self.push(" >>> ");
            self.push(&render_ident(&to.target));
        }
        self.newline();
        for on in &step.on_blocks {
            self.render_on_block(on);
        }
    }

    fn render_assign_step(&mut self, step: &AssignStep) {
        self.write_indent();
        self.push(&render_expr(&step.target));
        self.push(" = ");
        self.push(&render_simple_value(&step.value));
        if let Some(desc) = &step.desc {
            self.push(" ");
            self.push(&render_desc_inline(desc));
        }
        if let Some(to) = &step.to {
            self.push(" >>> ");
            self.push(&render_ident(&to.target));
        }
        self.newline();
        for on in &step.on_blocks {
            self.render_on_block(on);
        }
    }

    fn render_if_step(&mut self, step: &IfStep) {
        self.write_indent();
        self.push("if");
        self.push(&step.if_keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_condition(&step.cond));
        self.push(":");
        self.newline();
        self.render_step_items(&step.then_branch);
        if let Some(else_branch) = &step.else_branch {
            self.write_indent();
            self.push("else");
            self.push(&step.else_keyword_commitment.to_string());
            self.push(":");
            self.newline();
            self.render_step_items(else_branch);
        }
    }

    fn render_for_step(&mut self, step: &ForStep) {
        self.write_indent();
        self.push("for");
        self.push(&step.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_ident(&step.var));
        self.push(" in ");
        self.push(&render_atoms(&step.iterable));
        self.push(":");
        self.newline();
        self.render_step_items(&step.body);
    }

    fn render_while_step(&mut self, step: &WhileStep) {
        self.write_indent();
        self.push("while");
        self.push(&step.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_condition(&step.cond));
        if let Some(desc) = &step.desc {
            self.push(" ");
            self.push(&render_desc_inline(desc));
        }
        self.push(":");
        self.newline();
        self.render_step_items(&step.body);
    }

    fn render_parasteps_step(&mut self, step: &ParastepsStep) {
        self.write_indent();
        self.push("parasteps");
        self.push(&step.keyword_commitment.to_string());
        if let Some(desc) = &step.description {
            self.push(" ");
            self.push(&render_fstring(desc));
        }
        self.push(":");
        self.newline();
        self.render_step_items(&step.steps);
    }

    fn render_error_step(&mut self, step: &ErrorStep) {
        self.write_indent();
        self.push("error");
        self.push(&step.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_fstring(&step.message));
        if let Some(to) = &step.to {
            self.push(" >>> ");
            self.push(&render_ident(&to.target));
        }
        self.newline();
    }

    fn render_on_block(&mut self, on: &OnBlock) {
        self.write_indent();
        self.push("on");
        self.push(&on.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_atoms(&on.condition));
        self.push(":");
        self.newline();
        self.render_step_items(&on.steps);
    }

    fn render_ui(&mut self, ui: &UiDef) {
        self.write_indent();
        self.push("ui");
        self.push(&ui.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_ident(&ui.name));
        if let Some(binds) = &ui.binds {
            self.push(" binds");
            self.push(&ui.binds_keyword_commitment.to_string());
            self.push(" ");
            self.push(&render_ident(binds));
        }
        self.push(":");
        if let [Fragment::Placeholder { keyword_commitment }] = ui.items.as_slice() {
            self.push(" ...");
            self.push(&keyword_commitment.to_string());
            self.newline();
        } else {
            self.newline();
            self.set_indent(self.indent + 1);
            self.render_items(&ui.items, false);
            self.set_indent(self.indent - 1);
        }
    }

    fn render_ui_node(&mut self, node: &UiNode) {
        self.flush_blank();
        match node {
            UiNode::Stack { stack } => self.render_stack_or_parallel("stack", stack),
            UiNode::Parallel { parallel } => self.render_stack_or_parallel("parallel", parallel),
            UiNode::Leaf { leaf } => self.render_ui_leaf(leaf),
            UiNode::Error { error } => self.render_ui_error(error),
        }
    }

    fn render_stack_or_parallel(&mut self, keyword: &str, stack: &StackNode) {
        self.write_indent();
        self.push(keyword);
        self.push(&stack.keyword_commitment.to_string());
        if let Some(desc) = &stack.description {
            self.push(" ");
            self.push(&render_fstring(desc));
        }
        self.push(":");
        self.newline();
        self.set_indent(self.indent + 1);
        if stack.items.is_empty() {
            self.write_indent();
            self.push("...");
            self.newline();
        } else {
            self.render_items(&stack.items, false);
        }
        self.set_indent(self.indent - 1);
    }

    fn render_ui_leaf(&mut self, leaf: &UiLeaf) {
        self.write_indent();
        self.push(&render_fstring(&leaf.content));
        if let Some(desc) = &leaf.desc {
            self.push(" ");
            self.push(&render_desc_inline(desc));
        }
        if let Some(req) = &leaf.requires {
            self.push(" requires");
            self.push(&leaf.requires_keyword_commitment.to_string());
            self.push(" ");
            self.push(&render_condition(req));
        }
        if !leaf.with.is_empty() {
            self.push(" with");
            self.push(&leaf.with_keyword_commitment.to_string());
            self.push(" ");
            self.push(&render_capabilities(&leaf.with));
        }
        if let Some(on) = &leaf.on {
            self.push(" on ");
            self.push(&render_event_name(&on.event_name));
            self.push(": ");
            self.push(&render_action_expr(&on.action));
        }
        self.newline();
    }

    fn render_ui_error(&mut self, error: &UiErrorNode) {
        self.write_indent();
        self.push("error");
        self.push(&error.keyword_commitment.to_string());
        self.push(" ");
        self.push(&render_fstring(&error.message));
        if let Some(desc) = &error.desc {
            self.push(" ");
            self.push(&render_desc_inline(desc));
        }
        self.newline();
    }
}

fn render_math_statement(stmt: &MathStatement) -> String {
    match stmt {
        MathStatement::Define { target, value } => {
            format!("{} = {}", render_expr(target), render_expr(value))
        }
        MathStatement::Expr { expr } => render_expr(expr),
    }
}

fn render_condition(cond: &Condition) -> String {
    match cond {
        Condition::Structured { expr } => render_expr(expr),
        Condition::Natural { text } => render_fstring(text),
    }
}

fn render_event_name(name: &EventName) -> String {
    match name {
        EventName::Ident { value } => render_ident(value),
        EventName::Natural { text } => render_fstring(text),
    }
}

fn render_action_expr(action: &ActionExpr) -> String {
    action
        .actions
        .iter()
        .map(render_action)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_action(action: &Action) -> String {
    match action {
        Action::Call { expr } => render_expr(expr),
        Action::Navigate { target } => format!(">>> {}", render_ident(target)),
        Action::Assign { target, value } => {
            format!("{} = {}", render_expr(target), render_expr(value))
        }
        Action::Natural { text } => render_fstring(text),
    }
}

fn render_params(params: &[Param]) -> String {
    params
        .iter()
        .map(|p| {
            let mut s = render_ident(&p.name);
            if !p.type_hint.is_empty() {
                s.push_str(": ");
                s.push_str(&render_atoms(&p.type_hint));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_capabilities(caps: &[Capability]) -> String {
    caps.iter()
        .map(|capability| render_ident(&capability.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_atoms(atoms: &[Atom]) -> String {
    if atoms.is_empty() {
        return String::new();
    }
    let mut out = render_atom(&atoms[0]);
    for i in 1..atoms.len() {
        let prev = &atoms[i - 1];
        let curr = &atoms[i];
        if needs_space_between(prev, curr) {
            out.push(' ');
        }
        out.push_str(&render_atom(curr));
    }
    out
}

fn needs_space_between(prev: &Atom, curr: &Atom) -> bool {
    // 下一个是右括号/右方框/逗号/冒号/点：不加空格
    // NOTE: `>` 排除在外——MimiSpec 中 `>` 是大于比较运算符，需要前后空格
    if matches!(curr.symbol_value(), Some(")" | "]" | "," | ":" | ".")) {
        return false;
    }
    // 当前是左括号/左方框/左尖括号/点：不加空格
    if matches!(prev.symbol_value(), Some("(" | "[" | "<" | ".")) {
        return false;
    }
    // 逗号/冒号后加空格
    if matches!(prev.symbol_value(), Some("," | ":")) {
        return true;
    }
    // 两个标识符/字符串/数字之间加空格
    if is_atom_word_like(prev) && is_atom_word_like(curr) {
        return true;
    }
    // 默认不加空格
    false
}

fn is_atom_word_like(atom: &Atom) -> bool {
    matches!(
        atom,
        Atom::Ident { .. } | Atom::String { .. } | Atom::Number { .. }
    )
}

impl Atom {
    fn symbol_value(&self) -> Option<&str> {
        match self {
            Atom::Symbol { value } => Some(value.as_str()),
            _ => None,
        }
    }
}

fn render_atom(atom: &Atom) -> String {
    match atom {
        Atom::Ident { value } => render_ident(value),
        Atom::String { value } => render_fstring(value),
        Atom::Number { value } => value.clone(),
        Atom::Symbol { value } => value.clone(),
        Atom::List { items } => {
            let inner = items
                .iter()
                .map(|item| render_atoms(item))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
        Atom::Ellipsis { commitment } => {
            let mut s = String::from("...");
            s.push_str(&commitment.to_string());
            s
        }
    }
}

fn render_simple_value(value: &SimpleValue) -> String {
    match value {
        SimpleValue::Ident { value } => render_ident(value),
        SimpleValue::String { value } => render_fstring(value),
        SimpleValue::Number { value } => value.clone(),
        SimpleValue::Bool {
            value,
            keyword_commitment,
        } => format!("{}{}", value, keyword_commitment),
        SimpleValue::List { items } => {
            let inner = items
                .iter()
                .map(|item| render_atoms(item))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
        SimpleValue::Placeholder { commitment } => {
            let mut s = String::from("...");
            s.push_str(&commitment.to_string());
            s
        }
    }
}

fn render_ident(ident: &Ident) -> String {
    let mut s = ident.name.clone();
    s.push_str(&ident.commitment.to_string());
    s
}

fn render_fstring(s: &FString) -> String {
    let mut out = String::from("\"");
    for c in s.value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out.push_str(&s.commitment.to_string());
    out
}

fn render_desc_inline(desc: &Desc) -> String {
    let mut out = String::from("desc");
    out.push_str(&desc.need_commitment.to_string());
    out.push(' ');
    out.push_str(&render_fstring(&desc.content));
    out
}

fn render_clause_inline(clause: &Clause) -> String {
    let keyword = match clause.clause_kind {
        ClauseKind::Requires => "requires",
        ClauseKind::Ensures => "ensures",
    };
    format!(
        "{}{}: {}",
        keyword,
        clause.keyword_commitment,
        render_condition(&clause.condition)
    )
}

fn render_expr(expr: &Expr) -> String {
    render_expr_prec(expr, 0)
}

fn render_expr_prec(expr: &Expr, parent_prec: u8) -> String {
    let my_prec = expr_prec(expr);
    let s = match expr {
        Expr::Ident { value } => render_ident(value),
        Expr::String { value } => render_fstring(value),
        Expr::Number { value } => value.clone(),
        Expr::Bool {
            value,
            keyword_commitment,
        } => format!("{}{}", value, keyword_commitment),
        Expr::List { items } => {
            let inner = items.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("[{}]", inner)
        }
        Expr::Placeholder { keyword_commitment } => {
            let mut s = String::from("...");
            s.push_str(&keyword_commitment.to_string());
            s
        }
        Expr::Not {
            expr,
            keyword_commitment,
        } => format!(
            "not{} {}",
            keyword_commitment,
            render_expr_prec(expr, my_prec)
        ),
        Expr::Neg {
            expr,
            keyword_commitment,
        } => format!("-{}{}", keyword_commitment, render_expr_prec(expr, my_prec)),
        Expr::BitNot {
            expr,
            keyword_commitment,
        } => format!("~{}{}", keyword_commitment, render_expr_prec(expr, my_prec)),
        Expr::And {
            left,
            right,
            keyword_commitment,
        } => {
            format!(
                "{} and{} {}",
                render_expr_prec(left, my_prec),
                keyword_commitment,
                render_expr_prec(right, my_prec)
            )
        }
        Expr::Or {
            left,
            right,
            keyword_commitment,
        } => {
            format!(
                "{} or{} {}",
                render_expr_prec(left, my_prec),
                keyword_commitment,
                render_expr_prec(right, my_prec)
            )
        }
        Expr::In {
            left,
            right,
            keyword_commitment,
        } => format!(
            "{} in{} {}",
            render_expr_prec(left, my_prec),
            keyword_commitment,
            render_expr_prec(right, my_prec)
        ),
        Expr::Compare {
            left,
            op,
            right,
            keyword_commitment,
        } => {
            let op_s = match op {
                CompareOp::Eq => "==",
                CompareOp::Ne => "!=",
                CompareOp::Lt => "<",
                CompareOp::Gt => ">",
                CompareOp::Le => "<=",
                CompareOp::Ge => ">=",
            };
            format!(
                "{} {}{} {}",
                render_expr_prec(left, my_prec),
                op_s,
                keyword_commitment,
                render_expr_prec(right, my_prec)
            )
        }
        Expr::Add { left, right } => format!(
            "{} + {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Sub { left, right } => format!(
            "{} - {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Mul { left, right } => format!(
            "{} * {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Div { left, right } => format!(
            "{} / {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Pow { left, right } => format!(
            "{} ** {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::MatMul { left, right } => format!(
            "{} @ {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::BitAnd { left, right } => format!(
            "{} & {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::BitOr { left, right } => format!(
            "{} | {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::BitXor { left, right } => format!(
            "{} ^ {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Shl { left, right } => format!(
            "{} << {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Shr { left, right } => format!(
            "{} >> {}",
            render_expr_prec(left, my_prec),
            render_expr_prec(right, my_prec)
        ),
        Expr::Index { object, field } => {
            format!("{}.{}", render_expr_prec(object, 12), render_ident(field))
        }
        Expr::Subscript { object, indices } => {
            let inner = indices
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}[{}]", render_expr_prec(object, 12), inner)
        }
        Expr::Call { callee, args } => {
            let inner = args.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", render_expr_prec(callee, 12), inner)
        }
    };
    paren_if(my_prec < parent_prec, s)
}
