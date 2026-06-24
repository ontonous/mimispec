use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ast::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    TypeDef,
    Func,
    Flow,
    Ui,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Module => write!(f, "module"),
            SymbolKind::TypeDef => write!(f, "type"),
            SymbolKind::Func => write!(f, "func"),
            SymbolKind::Flow => write!(f, "flow"),
            SymbolKind::Ui => write!(f, "ui"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SymbolConflict {
    pub name: String,
    pub entries: Vec<SymbolEntry>,
}

pub struct SymbolTable {
    entries: HashMap<String, Vec<SymbolEntry>>,
}

impl SymbolTable {
    pub fn build(files: &HashMap<PathBuf, Arc<File>>) -> Self {
        let mut entries: HashMap<String, Vec<SymbolEntry>> = HashMap::new();

        for (file_path, file) in files {
            Self::collect_fragments(&file.fragments, file_path, &mut entries);
        }

        Self { entries }
    }

    pub fn lookup(&self, name: &str) -> &[SymbolEntry] {
        self.entries
            .get(name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn all_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn conflicts(&self) -> Vec<SymbolConflict> {
        self.entries
            .iter()
            .filter(|(_, entries)| entries.len() > 1)
            .map(|(name, entries)| SymbolConflict {
                name: name.clone(),
                entries: entries.clone(),
            })
            .collect()
    }

    pub fn has_conflicts(&self) -> bool {
        self.entries.values().any(|v| v.len() > 1)
    }

    fn collect_fragments(
        fragments: &[Fragment],
        file_path: &Path,
        entries: &mut HashMap<String, Vec<SymbolEntry>>,
    ) {
        for fragment in fragments {
            match fragment {
                Fragment::Module { module } => {
                    Self::insert_entry(entries, &module.name.name, SymbolKind::Module, file_path);
                    Self::collect_fragments(&module.items, file_path, entries);
                }
                Fragment::TypeDef { typedef } => {
                    Self::insert_entry(entries, &typedef.name.name, SymbolKind::TypeDef, file_path);
                }
                Fragment::Func { func } => {
                    Self::insert_entry(entries, &func.name.name, SymbolKind::Func, file_path);
                }
                Fragment::Flow { flow } => {
                    Self::insert_entry(entries, &flow.name.name, SymbolKind::Flow, file_path);
                }
                Fragment::Ui { ui } => {
                    Self::insert_entry(entries, &ui.name.name, SymbolKind::Ui, file_path);
                }
                _ => {}
            }
        }
    }

    fn insert_entry(
        entries: &mut HashMap<String, Vec<SymbolEntry>>,
        name: &str,
        kind: SymbolKind,
        file: &Path,
    ) {
        entries
            .entry(name.to_string())
            .or_default()
            .push(SymbolEntry {
                name: name.to_string(),
                kind,
                file: file.to_path_buf(),
            });
    }
}
