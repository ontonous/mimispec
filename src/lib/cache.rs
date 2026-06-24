use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::ast::File;

struct CacheEntry {
    mtime: Option<SystemTime>,
    file: Arc<File>,
}

pub(crate) struct ImportCache {
    entries: HashMap<PathBuf, CacheEntry>,
}

impl ImportCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Get cached file, validating mtime if available.
    /// Returns `None` if not cached, or if mtime is provided and differs from cached.
    pub fn get(&self, path: &Path, mtime: Option<&SystemTime>) -> Option<Arc<File>> {
        self.entries.get(path).and_then(|e| {
            match (mtime, &e.mtime) {
                (Some(current), Some(cached)) if current == cached => Some(e.file.clone()),
                (None, _) => Some(e.file.clone()),
                _ => None,
            }
        })
    }

    pub fn insert(&mut self, path: PathBuf, file: File, mtime: Option<SystemTime>) -> Arc<File> {
        let file = Arc::new(file);
        self.entries.insert(
            path,
            CacheEntry {
                mtime,
                file: file.clone(),
            },
        );
        file
    }
}
