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
            Fragment::Flow { flow } => flow.name.name == name,
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
        if let Fragment::Module { module } = fragment {
            collect_fragments_recursive(&module.items, out);
        }
    }
}
