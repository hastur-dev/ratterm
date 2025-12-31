//! LRU cache for completion results.
//!
//! Caches completion results keyed by file path, line number, and prefix
//! to avoid redundant completion requests.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::provider::CompletionItem;

/// Maximum number of cache entries.
const MAX_CACHE_ENTRIES: usize = 50;

/// Default cache TTL in seconds.
const DEFAULT_TTL_SECS: u64 = 30;

/// Cache key for completion results.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    /// File path (or empty for unsaved buffers).
    file_path: Option<PathBuf>,

    /// Line number.
    line: usize,

    /// Prefix text before cursor.
    prefix: String,

    /// Language ID.
    language_id: String,
}

impl CacheKey {
    /// Creates a new cache key.
    #[must_use]
    pub fn new(
        file_path: Option<PathBuf>,
        line: usize,
        prefix: impl Into<String>,
        language_id: impl Into<String>,
    ) -> Self {
        Self {
            file_path,
            line,
            prefix: prefix.into(),
            language_id: language_id.into(),
        }
    }
}

/// Cached completion entry with timestamp.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Cached completion items.
    items: Vec<CompletionItem>,

    /// When this entry was created.
    created_at: Instant,

    /// Time-to-live duration.
    ttl: Duration,

    /// Access count for LRU ordering.
    access_count: u64,
}

impl CacheEntry {
    /// Creates a new cache entry.
    fn new(items: Vec<CompletionItem>, ttl: Duration) -> Self {
        Self {
            items,
            created_at: Instant::now(),
            ttl,
            access_count: 0,
        }
    }

    /// Returns whether this entry has expired.
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    /// Marks this entry as accessed.
    fn touch(&mut self) {
        self.access_count = self.access_count.saturating_add(1);
    }
}

/// LRU cache for completion results.
#[derive(Debug)]
pub struct CompletionCache {
    /// Cache entries by key.
    entries: HashMap<CacheKey, CacheEntry>,

    /// Maximum number of entries.
    max_entries: usize,

    /// Default TTL for entries.
    default_ttl: Duration,

    /// Total hit count (for metrics).
    hit_count: u64,

    /// Total miss count (for metrics).
    miss_count: u64,
}

impl CompletionCache {
    /// Creates a new completion cache with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(MAX_CACHE_ENTRIES)
    }

    /// Creates a new completion cache with the given capacity.
    #[must_use]
    pub fn with_capacity(max_entries: usize) -> Self {
        assert!(max_entries > 0, "cache capacity must be positive");

        Self {
            entries: HashMap::with_capacity(max_entries),
            max_entries,
            default_ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// Gets cached completions for the given key.
    ///
    /// Returns `None` if not cached or expired.
    pub fn get(&mut self, key: &CacheKey) -> Option<&[CompletionItem]> {
        // Check if entry exists and is not expired
        let is_valid = self
            .entries
            .get(key)
            .map(|e| !e.is_expired())
            .unwrap_or(false);

        if is_valid {
            self.hit_count = self.hit_count.saturating_add(1);
            if let Some(entry) = self.entries.get_mut(key) {
                entry.touch();
                return Some(&entry.items);
            }
        }

        self.miss_count = self.miss_count.saturating_add(1);
        None
    }

    /// Inserts completions into the cache.
    pub fn insert(&mut self, key: CacheKey, items: Vec<CompletionItem>) {
        // Evict expired and LRU entries if at capacity
        if self.entries.len() >= self.max_entries {
            self.evict();
        }

        let entry = CacheEntry::new(items, self.default_ttl);
        self.entries.insert(key, entry);
    }

    /// Removes expired entries and evicts LRU if still over capacity.
    fn evict(&mut self) {
        // Remove expired entries first
        self.entries.retain(|_, entry| !entry.is_expired());

        // If still over capacity, remove least accessed entries
        if self.entries.len() >= self.max_entries {
            let target_size = self.max_entries / 2;
            let mut entries: Vec<_> = self.entries.iter().collect();
            entries.sort_by_key(|(_, e)| e.access_count);

            let to_remove: Vec<CacheKey> = entries
                .iter()
                .take(self.entries.len().saturating_sub(target_size))
                .map(|(k, _)| (*k).clone())
                .collect();

            for key in to_remove {
                self.entries.remove(&key);
            }
        }
    }

    /// Clears all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Invalidates cache entries for a specific file.
    pub fn invalidate_file(&mut self, file_path: &PathBuf) {
        self.entries
            .retain(|k, _| k.file_path.as_ref() != Some(file_path));
    }

    /// Returns the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the cache hit rate (0.0 to 1.0).
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f64 / total as f64
        }
    }

    /// Returns cache statistics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            max_entries: self.max_entries,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            hit_rate: self.hit_rate(),
        }
    }
}

impl Default for CompletionCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current number of entries.
    pub entries: usize,

    /// Maximum number of entries.
    pub max_entries: usize,

    /// Total cache hits.
    pub hit_count: u64,

    /// Total cache misses.
    pub miss_count: u64,

    /// Cache hit rate (0.0 to 1.0).
    pub hit_rate: f64,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::completion::provider::CompletionKind;

    fn make_item(label: &str) -> CompletionItem {
        CompletionItem::new(
            label.to_string(),
            label.to_string(),
            CompletionKind::Variable,
            "test".to_string(),
        )
    }

    #[test]
    fn test_cache_creation() {
        let cache = CompletionCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = CompletionCache::new();
        let key = CacheKey::new(Some(PathBuf::from("test.rs")), 10, "foo", "rust");
        let items = vec![make_item("foobar"), make_item("foobaz")];

        cache.insert(key.clone(), items);
        assert_eq!(cache.len(), 1);

        let result = cache.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = CompletionCache::new();
        let key = CacheKey::new(Some(PathBuf::from("test.rs")), 10, "foo", "rust");

        let result = cache.get(&key);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_invalidate_file() {
        let mut cache = CompletionCache::new();
        let path = PathBuf::from("test.rs");
        let key1 = CacheKey::new(Some(path.clone()), 10, "foo", "rust");
        let key2 = CacheKey::new(Some(path.clone()), 20, "bar", "rust");
        let key3 = CacheKey::new(Some(PathBuf::from("other.rs")), 10, "foo", "rust");

        cache.insert(key1, vec![make_item("foo")]);
        cache.insert(key2, vec![make_item("bar")]);
        cache.insert(key3.clone(), vec![make_item("baz")]);
        assert_eq!(cache.len(), 3);

        cache.invalidate_file(&path);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&key3).is_some());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = CompletionCache::new();
        let key = CacheKey::new(Some(PathBuf::from("test.rs")), 10, "foo", "rust");

        cache.insert(key, vec![make_item("foo")]);
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = CompletionCache::new();
        let key = CacheKey::new(Some(PathBuf::from("test.rs")), 10, "foo", "rust");

        cache.insert(key.clone(), vec![make_item("foo")]);

        // Hit
        cache.get(&key);

        // Miss
        let missing_key = CacheKey::new(None, 0, "x", "rust");
        cache.get(&missing_key);

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
        assert!((stats.hit_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = CompletionCache::with_capacity(3);

        for i in 0..5 {
            let key = CacheKey::new(None, i, format!("prefix{i}"), "rust");
            cache.insert(key, vec![make_item(&format!("item{i}"))]);
        }

        // Should have evicted some entries
        assert!(cache.len() <= 3);
    }

    #[test]
    #[should_panic(expected = "cache capacity must be positive")]
    fn test_cache_zero_capacity_panics() {
        let _ = CompletionCache::with_capacity(0);
    }
}
