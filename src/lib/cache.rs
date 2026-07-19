use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::ast::File;

struct CacheEntry {
    mtime: Option<SystemTime>,
    file: Arc<File>,
    last_access: Cell<u64>,
}

/// Import parse cache with optional finite-capacity LRU eviction.
///
/// [`ImportCache::new`] preserves the historical unlimited behavior. The LRU
/// clock uses interior mutability so the existing `get(&self, ...)` API remains
/// source-compatible.
pub struct ImportCache {
    entries: HashMap<PathBuf, CacheEntry>,
    capacity: Option<usize>,
    access_clock: Cell<u64>,
}

impl Default for ImportCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            capacity: None,
            access_clock: Cell::new(0),
        }
    }

    /// Create a cache that retains at most `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity: Some(capacity),
            access_clock: Cell::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_clock.set(0);
    }

    /// Get cached file, validating mtime if available.
    /// Returns `None` if not cached, or if mtime is provided and differs from cached.
    pub fn get(&self, path: &Path, mtime: Option<&SystemTime>) -> Option<Arc<File>> {
        let entry = self.entries.get(path)?;
        let valid = match (mtime, &entry.mtime) {
            (Some(current), Some(cached)) => current == cached,
            (None, _) => true,
            _ => false,
        };
        if !valid {
            return None;
        }
        entry.last_access.set(self.next_access());
        Some(entry.file.clone())
    }

    pub fn insert(&mut self, path: PathBuf, file: File, mtime: Option<SystemTime>) -> Arc<File> {
        let file = Arc::new(file);
        if self.capacity == Some(0) {
            return file;
        }
        let access = self.next_access();
        self.entries.insert(
            path,
            CacheEntry {
                mtime,
                file: file.clone(),
                last_access: Cell::new(access),
            },
        );
        self.evict_to_capacity();
        file
    }

    fn next_access(&self) -> u64 {
        let next = self.access_clock.get().saturating_add(1);
        self.access_clock.set(next);
        next
    }

    fn evict_to_capacity(&mut self) {
        let Some(capacity) = self.capacity else {
            return;
        };
        while self.entries.len() > capacity {
            let Some(lru) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access.get())
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            self.entries.remove(&lru);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn file(label: &str) -> File {
        crate::parse(&format!("desc \"{label}\"\n")).file
    }

    #[test]
    fn default_cache_remains_unbounded_and_clearable() {
        let mut cache = ImportCache::new();
        for index in 0..8 {
            cache.insert(PathBuf::from(format!("{index}.mms")), file("x"), None);
        }
        assert_eq!(cache.len(), 8);
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn finite_cache_evicts_the_least_recent_valid_hit() {
        let mut cache = ImportCache::with_capacity(2);
        let a = PathBuf::from("a.mms");
        let b = PathBuf::from("b.mms");
        let c = PathBuf::from("c.mms");
        cache.insert(a.clone(), file("a"), None);
        cache.insert(b.clone(), file("b"), None);
        assert!(cache.get(&a, None).is_some());
        cache.insert(c.clone(), file("c"), None);
        assert!(cache.get(&a, None).is_some());
        assert!(cache.get(&b, None).is_none());
        assert!(cache.get(&c, None).is_some());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn mtime_miss_does_not_refresh_lru_and_zero_capacity_stores_nothing() {
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let new = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let mut cache = ImportCache::with_capacity(2);
        let a = PathBuf::from("a.mms");
        let b = PathBuf::from("b.mms");
        cache.insert(a.clone(), file("a"), Some(old));
        cache.insert(b.clone(), file("b"), None);
        assert!(cache.get(&a, Some(&new)).is_none());
        cache.insert(PathBuf::from("c.mms"), file("c"), None);
        assert!(cache.get(&a, Some(&old)).is_none());

        let mut disabled = ImportCache::with_capacity(0);
        let returned = disabled.insert(a.clone(), file("a"), None);
        assert!(!returned.fragments.is_empty());
        assert_eq!(disabled.len(), 0);
        assert!(disabled.get(&a, None).is_none());
    }
}
