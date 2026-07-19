use std::sync::Arc;

use crate::ast::*;

pub struct FileQuery {
    file: Arc<File>,
}

impl FileQuery {
    pub fn new(file: Arc<File>) -> Self {
        Self { file }
    }

    pub fn fragments(&self) -> FragmentIter<'_> {
        FragmentIter {
            fragments: &self.file.fragments,
            index: 0,
        }
    }

    /// Preferred 0.3 name for the Document Context's ordered item sequence.
    pub fn items(&self) -> FragmentIter<'_> {
        self.fragments()
    }

    pub fn descriptions(&self) -> impl Iterator<Item = &Desc> {
        self.file.fragments.iter().filter_map(Fragment::desc)
    }

    pub fn clauses(&self) -> impl Iterator<Item = &Clause> {
        self.file.fragments.iter().filter_map(Fragment::clause)
    }

    pub fn rules(&self) -> impl Iterator<Item = &RuleDef> {
        self.file.fragments.iter().filter_map(Fragment::rule)
    }

    pub fn environment_rules(&self) -> impl Iterator<Item = &RuleDef> {
        self.rules()
            .filter(|rule| rule.attachment == RuleAttachment::Environment)
    }

    pub fn attached_rules(&self, target_index: usize) -> impl Iterator<Item = &RuleDef> {
        self.rules()
            .filter(move |rule| rule.attachment == RuleAttachment::Attached { target_index })
    }
}

pub struct FragmentIter<'a> {
    fragments: &'a [Fragment],
    index: usize,
}

impl<'a> Iterator for FragmentIter<'a> {
    type Item = &'a Fragment;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.fragments.len() {
            let item = &self.fragments[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<'a> FragmentIter<'a> {
    pub fn descriptions(self) -> impl Iterator<Item = &'a Desc> + 'a {
        self.filter_map(Fragment::desc)
    }

    pub fn clauses(self) -> impl Iterator<Item = &'a Clause> + 'a {
        self.filter_map(Fragment::clause)
    }

    pub fn rules(self) -> impl Iterator<Item = &'a RuleDef> + 'a {
        self.filter_map(Fragment::rule)
    }

    pub fn environment_rules(self) -> impl Iterator<Item = &'a RuleDef> + 'a {
        self.rules()
            .filter(|rule| rule.attachment == RuleAttachment::Environment)
    }

    pub fn modules(self) -> impl Iterator<Item = &'a Module> + 'a {
        self.filter_map(|f| match f {
            Fragment::Module { module } => Some(module),
            _ => None,
        })
    }

    pub fn type_defs(self) -> impl Iterator<Item = &'a TypeDef> + 'a {
        self.filter_map(|f| match f {
            Fragment::TypeDef { typedef } => Some(typedef),
            _ => None,
        })
    }

    pub fn funcs(self) -> impl Iterator<Item = &'a FuncDef> + 'a {
        self.filter_map(|f| match f {
            Fragment::Func { func } => Some(func),
            _ => None,
        })
    }

    pub fn flows(self) -> impl Iterator<Item = &'a FlowDef> + 'a {
        self.filter_map(|f| match f {
            Fragment::Flow { flow } => Some(flow),
            _ => None,
        })
    }

    pub fn uis(self) -> impl Iterator<Item = &'a UiDef> + 'a {
        self.filter_map(|f| match f {
            Fragment::Ui { ui } => Some(ui),
            _ => None,
        })
    }

    pub fn with_name(self, name: &'a str) -> impl Iterator<Item = &'a Fragment> + 'a {
        self.filter(move |f| match f {
            Fragment::Module { module } => module.name.name == name,
            Fragment::TypeDef { typedef } => typedef.name.name == name,
            Fragment::Func { func } => func.name.name == name,
            Fragment::Flow { flow } => flow.name.as_ref().is_some_and(|value| value.name == name),
            Fragment::Ui { ui } => ui.name.name == name,
            _ => false,
        })
    }
}

/// Walk module items recursively to find all fragments of a given kind.
pub fn collect_fragments(fragments: &[Fragment]) -> Vec<&Fragment> {
    let mut result = Vec::new();
    collect_fragments_recursive(fragments, &mut result);
    result
}

fn collect_fragments_recursive<'a>(fragments: &'a [Fragment], out: &mut Vec<&'a Fragment>) {
    for fragment in fragments {
        out.push(fragment);
        match fragment {
            Fragment::Module { module } => collect_fragments_recursive(&module.items, out),
            Fragment::TypeDef { typedef } => {
                collect_fragments_recursive(typedef.items(), out);
            }
            Fragment::Flow { flow } => collect_fragments_recursive(&flow.items, out),
            Fragment::FlowEntry { entry } => collect_fragments_recursive(&entry.items, out),
            Fragment::FlowArm { arm } => collect_fragments_recursive(&arm.items, out),
            Fragment::Func { func } => collect_fragments_recursive(&func.items, out),
            Fragment::Steps { items, .. } => collect_fragments_recursive(items, out),
            Fragment::Step { step } => collect_step_items(step, out),
            Fragment::Ui { ui } => collect_fragments_recursive(&ui.items, out),
            Fragment::UiNode { node } => collect_ui_items(node, out),
            Fragment::Desc { .. }
            | Fragment::Rule { .. }
            | Fragment::Clause { .. }
            | Fragment::Expr { .. }
            | Fragment::Math { .. }
            | Fragment::Field { .. }
            | Fragment::Variants { .. }
            | Fragment::Placeholder { .. } => {}
        }
    }
}

fn collect_step_items<'a>(step: &'a Step, out: &mut Vec<&'a Fragment>) {
    match step {
        Step::If { step } => {
            collect_fragments_recursive(&step.then_branch, out);
            if let Some(else_branch) = &step.else_branch {
                collect_fragments_recursive(else_branch, out);
            }
        }
        Step::For { step } => collect_fragments_recursive(&step.body, out),
        Step::While { step } => collect_fragments_recursive(&step.body, out),
        Step::Parasteps { step } => collect_fragments_recursive(&step.steps, out),
        Step::Action { step } => {
            for on in &step.on_blocks {
                collect_fragments_recursive(&on.steps, out);
            }
        }
        Step::Assign { step } => {
            for on in &step.on_blocks {
                collect_fragments_recursive(&on.steps, out);
            }
        }
        Step::Error { .. } | Step::Desc { .. } | Step::Placeholder { .. } => {}
    }
}

fn collect_ui_items<'a>(node: &'a UiNode, out: &mut Vec<&'a Fragment>) {
    match node {
        UiNode::Stack { stack } | UiNode::Parallel { parallel: stack } => {
            collect_fragments_recursive(&stack.items, out);
        }
        UiNode::Leaf { .. } | UiNode::Error { .. } => {}
    }
}
