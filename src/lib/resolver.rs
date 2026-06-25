use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ast::File;
use crate::cache::ImportCache;
use crate::error::ResolveError;
use crate::parse;

const MAX_RESOLVE_DEPTH: u32 = 32;

pub struct Resolver {
    cache: ImportCache,
    resolving: HashSet<PathBuf>,
    resolving_order: Vec<PathBuf>,
    files: HashMap<PathBuf, Arc<File>>,
    errors: Vec<(PathBuf, ResolveError)>,
    root_dir: PathBuf,
    depth: u32,
}

impl Resolver {
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            cache: ImportCache::new(),
            resolving: HashSet::new(),
            resolving_order: Vec::new(),
            files: HashMap::new(),
            errors: Vec::new(),
            root_dir,
            depth: 0,
        }
    }

    pub fn resolve(&mut self, path: &Path) -> Option<Arc<File>> {
        if self.depth > MAX_RESOLVE_DEPTH {
            self.errors.push((
                path.to_path_buf(),
                ResolveError::ImportCycle {
                    chain: self.resolving_order.clone(),
                },
            ));
            return None;
        }

        let canonical = self.canonicalize(path)?;

        if self.files.contains_key(&canonical) {
            return self.files.get(&canonical).cloned();
        }

        if self.resolving.contains(&canonical) {
            let chain = self.resolving_order.clone();
            self.errors
                .push((canonical, ResolveError::ImportCycle { chain }));
            return None;
        }

        let current_mtime = fs::metadata(&canonical).ok().and_then(|m| m.modified().ok());

        if let Some(cached) = self.cache.get(&canonical, current_mtime.as_ref()) {
            self.files.insert(canonical, cached.clone());
            return Some(cached);
        }

        let source = match fs::read_to_string(&canonical) {
            Ok(s) => s,
            Err(e) => {
                self.errors.push((
                    canonical.clone(),
                    ResolveError::IoError {
                        path: canonical,
                        message: e.to_string(),
                    },
                ));
                return None;
            }
        };

        let parse_result = parse(&source);
        let file = parse_result.file;

        if !parse_result.errors.is_empty() {
            self.errors.push((
                canonical.clone(),
                ResolveError::ParseFailed {
                    path: canonical.clone(),
                    errors: parse_result.errors,
                },
            ));
        }

        let file_arc = self.cache.insert(canonical.clone(), file, current_mtime);

        self.depth += 1;
        self.resolving.insert(canonical.clone());
        self.resolving_order.push(canonical.clone());
        let import_paths: Vec<String> = file_arc.imports.clone();
        for import_path in &import_paths {
            let parent = canonical.parent().unwrap_or(Path::new("."));
            let resolved = parent.join(import_path);
            self.resolve(&resolved);
        }
        self.resolving_order.pop();
        self.resolving.remove(&canonical);
        self.depth -= 1;

        self.files.insert(canonical, file_arc.clone());
        Some(file_arc)
    }

    pub fn files(&self) -> &HashMap<PathBuf, Arc<File>> {
        &self.files
    }

    pub fn take_errors(&mut self) -> Vec<(PathBuf, ResolveError)> {
        std::mem::take(&mut self.errors)
    }

    pub fn take_files(&mut self) -> HashMap<PathBuf, Arc<File>> {
        std::mem::take(&mut self.files)
    }

    fn canonicalize(&mut self, path: &Path) -> Option<PathBuf> {
        if path.is_absolute() {
            if path.exists() {
                path.canonicalize().ok()
            } else {
                self.errors
                    .push((path.to_path_buf(), ResolveError::FileNotFound { path: path.to_path_buf() }));
                None
            }
        } else {
            let candidate = self.root_dir.join(path);
            if candidate.exists() {
                candidate.canonicalize().ok()
            } else {
                self.errors.push((
                    path.to_path_buf(),
                    ResolveError::FileNotFound {
                        path: candidate,
                    },
                ));
                None
            }
        }
    }
}
