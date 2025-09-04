//! Caching module for cargo-status
//!
//! Provides caching of command results and tool availability checks
//! to improve performance on repeated runs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Cache entry with timestamp for expiration
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    timestamp: Instant,
}

/// Thread-safe cache with TTL (Time To Live)
#[derive(Debug, Clone)]
pub struct Cache<T: Clone> {
    entries: Arc<Mutex<HashMap<String, CacheEntry<T>>>>,
    ttl: Duration,
}

impl<T: Clone> Cache<T> {
    /// Creates a new cache with the specified TTL in seconds
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    /// Gets a value from the cache if it exists and hasn't expired
    pub fn get(&self, key: &str) -> Option<T> {
        let mut entries = self.entries.lock().unwrap();

        if let Some(entry) = entries.get(key) {
            if entry.timestamp.elapsed() < self.ttl {
                return Some(entry.value.clone());
            } else {
                // Remove expired entry
                entries.remove(key);
            }
        }

        None
    }

    /// Inserts a value into the cache
    pub fn insert(&self, key: String, value: T) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(
            key,
            CacheEntry {
                value,
                timestamp: Instant::now(),
            },
        );
    }

    /// Clears all entries from the cache
    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
    }

    /// Removes expired entries from the cache
    pub fn prune(&self) {
        let mut entries = self.entries.lock().unwrap();
        let now = Instant::now();

        entries.retain(|_, entry| now.duration_since(entry.timestamp) < self.ttl);
    }
}

// Global cache for tool availability checks
lazy_static::lazy_static! {
    pub static ref TOOL_CACHE: Cache<bool> = Cache::new(300); // 5 minutes TTL
}

/// Cached check for tool availability
pub fn has_tool_cached(tool_name: &str, check_fn: impl FnOnce() -> bool) -> bool {
    let cache_key = format!("tool_{}", tool_name);

    if let Some(cached) = TOOL_CACHE.get(&cache_key) {
        return cached;
    }

    let available = check_fn();
    TOOL_CACHE.insert(cache_key, available);
    available
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cache_basic() {
        let cache: Cache<String> = Cache::new(1);

        cache.insert("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        assert_eq!(cache.get("key2"), None);
    }

    #[test]
    fn test_cache_expiration() {
        let cache: Cache<i32> = Cache::new(1);

        cache.insert("key".to_string(), 42);
        assert_eq!(cache.get("key"), Some(42));

        thread::sleep(Duration::from_secs(2));
        assert_eq!(cache.get("key"), None);
    }

    #[test]
    fn test_cache_clear() {
        let cache: Cache<String> = Cache::new(60);

        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());

        cache.clear();

        assert_eq!(cache.get("key1"), None);
        assert_eq!(cache.get("key2"), None);
    }

    #[test]
    fn test_cache_prune() {
        let cache: Cache<i32> = Cache::new(1);

        cache.insert("key1".to_string(), 1);
        thread::sleep(Duration::from_millis(500));
        cache.insert("key2".to_string(), 2);

        thread::sleep(Duration::from_millis(600));

        cache.prune();

        assert_eq!(cache.get("key1"), None); // Should be pruned
        assert_eq!(cache.get("key2"), Some(2)); // Should still exist
    }
}
