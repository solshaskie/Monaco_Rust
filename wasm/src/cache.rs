use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;

use crate::tokenize::{tokenize_source, WasmToken};

/// 32 MiB maximum cache size.
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;

/// A cached tokenization result for a single resource.
#[derive(Clone)]
struct CacheEntry {
    /// SHA-256 of the source text at the time of tokenization.
    source_hash: [u8; 32],
    language: String,
    tokens: Vec<WasmToken>,
    /// Approximate memory footprint of this entry (source len + token text lengths).
    size_bytes: usize,
    /// Monotonically-increasing access counter for LRU eviction.
    last_access: u64,
}

thread_local! {
    static TOKEN_CACHE: RefCell<HashMap<String, CacheEntry>> = RefCell::new(HashMap::new());
    static TOTAL_BYTES: RefCell<usize> = RefCell::new(0);
    static ACCESS_COUNTER: RefCell<u64> = RefCell::new(0);
}

fn next_access_counter() -> u64 {
    ACCESS_COUNTER.with(|c| {
        let mut c = c.borrow_mut();
        *c += 1;
        *c
    })
}

fn compute_source_hash(source: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.finalize().into()
}

fn entry_size(source: &str, tokens: &[WasmToken]) -> usize {
    let text_size: usize = tokens.iter().map(|t| t.text.len()).sum();
    source.len() + text_size + tokens.len() * std::mem::size_of::<WasmToken>()
}

fn hash_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a == b
}

/// Evict entries until there is at least `needed` bytes of room.
fn evict_if_needed(needed: usize) {
    if needed > MAX_CACHE_BYTES {
        // A single entry exceeds the entire cache; don't cache it.
        return;
    }
    TOKEN_CACHE.with(|cache| {
        TOTAL_BYTES.with(|total| {
            let mut cache = cache.borrow_mut();
            let mut total = total.borrow_mut();
            while *total + needed > MAX_CACHE_BYTES && !cache.is_empty() {
                // Find LRU entry (smallest last_access)
                let lru_key = cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_access)
                    .map(|(k, _)| k.clone());
                if let Some(key) = lru_key {
                    if let Some(entry) = cache.remove(&key) {
                        *total = total.saturating_sub(entry.size_bytes);
                    }
                } else {
                    break;
                }
            }
        });
    });
}

/// Store a full snapshot and its tokenization result.
pub fn prime(resource: &str, source: &str, language: &str) -> usize {
    let tokens = tokenize_source(source, language);
    let count = tokens.len();
    let size = entry_size(source, &tokens);
    evict_if_needed(size);
    let hash = compute_source_hash(source);

    TOKEN_CACHE.with(|cache| {
        TOTAL_BYTES.with(|total| {
            let mut cache = cache.borrow_mut();
            let mut total = total.borrow_mut();
            if let Some(old) = cache.remove(resource) {
                *total = total.saturating_sub(old.size_bytes);
            }
            cache.insert(
                resource.to_string(),
                CacheEntry {
                    source_hash: hash,
                    language: language.to_string(),
                    tokens,
                    size_bytes: size,
                    last_access: next_access_counter(),
                },
            );
            *total += size;
        });
    });
    count
}

/// Return cached tokens if the source snapshot is unchanged; otherwise re-tokenize
/// and update the cache.
pub fn tokenize(resource: &str, source: &str, language: &str) -> Vec<WasmToken> {
    let hash = compute_source_hash(source);
    TOKEN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get_mut(resource) {
            if hash_eq(&entry.source_hash, &hash) && entry.language == language {
                entry.last_access = next_access_counter();
                return entry.tokens.clone();
            }
        }
        drop(cache);
        // Cache miss — tokenize and insert
        let tokens = tokenize_source(source, language);
        let size = entry_size(source, &tokens);
        evict_if_needed(size);
        TOKEN_CACHE.with(|cache| {
            TOTAL_BYTES.with(|total| {
                let mut cache = cache.borrow_mut();
                let mut total = total.borrow_mut();
                if let Some(old) = cache.remove(resource) {
                    *total = total.saturating_sub(old.size_bytes);
                }
                cache.insert(
                    resource.to_string(),
                    CacheEntry {
                        source_hash: hash,
                        language: language.to_string(),
                        tokens: tokens.clone(),
                        size_bytes: size,
                        last_access: next_access_counter(),
                    },
                );
                *total += size;
            });
        });
        tokens
    })
}

/// Tokenize a line range, using the full-file cache when possible.
/// If the resource is not in cache or the source changed, the full file is
/// re-tokenized and cached before slicing the range.
pub fn tokenize_range(
    resource: &str,
    source: &str,
    language: &str,
    start_line: usize,
    end_line: usize,
) -> Vec<WasmToken> {
    let hash = compute_source_hash(source);
    TOKEN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get_mut(resource) {
            if hash_eq(&entry.source_hash, &hash) && entry.language == language {
                entry.last_access = next_access_counter();
                return entry
                    .tokens
                    .iter()
                    .filter(|t| t.line >= start_line && t.line < end_line)
                    .cloned()
                    .collect();
            }
        }
        drop(cache);
        // Cache miss
        let tokens = tokenize_source(source, language);
        let range_tokens: Vec<WasmToken> = tokens
            .iter()
            .filter(|t| t.line >= start_line && t.line < end_line)
            .cloned()
            .collect();
        let size = entry_size(source, &tokens);
        evict_if_needed(size);
        TOKEN_CACHE.with(|cache| {
            TOTAL_BYTES.with(|total| {
                let mut cache = cache.borrow_mut();
                let mut total = total.borrow_mut();
                if let Some(old) = cache.remove(resource) {
                    *total = total.saturating_sub(old.size_bytes);
                }
                cache.insert(
                    resource.to_string(),
                    CacheEntry {
                        source_hash: hash,
                        language: language.to_string(),
                        tokens,
                        size_bytes: size,
                        last_access: next_access_counter(),
                    },
                );
                *total += size;
            });
        });
        range_tokens
    })
}

/// Remove a single resource from the cache.
pub fn invalidate(resource: &str) {
    TOKEN_CACHE.with(|cache| {
        TOTAL_BYTES.with(|total| {
            let mut cache = cache.borrow_mut();
            let mut total = total.borrow_mut();
            if let Some(entry) = cache.remove(resource) {
                *total = total.saturating_sub(entry.size_bytes);
            }
        });
    });
}

/// Clear the entire cache.
pub fn clear() {
    TOKEN_CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
    TOTAL_BYTES.with(|total| {
        *total.borrow_mut() = 0;
    });
}

/// Return the number of cached resources.
pub fn len() -> usize {
    TOKEN_CACHE.with(|cache| cache.borrow().len())
}

/// Return approximate total cached bytes.
pub fn total_bytes() -> usize {
    TOTAL_BYTES.with(|total| *total.borrow())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit() {
        clear();
        let source = "fn main() {}";
        let toks1 = tokenize("test.rs", source, "rust");
        let toks2 = tokenize("test.rs", source, "rust");
        assert_eq!(toks1, toks2);
        assert_eq!(len(), 1);
    }

    #[test]
    fn cache_invalidation_on_source_change() {
        clear();
        let toks1 = tokenize("test.rs", "fn main() {}", "rust");
        let toks2 = tokenize("test.rs", "fn foo() {}", "rust");
        assert_ne!(toks1, toks2);
        assert_eq!(len(), 1);
    }

    #[test]
    fn tokenize_range_uses_cache() {
        clear();
        let source = "line1\nline2\nline3\nline4";
        // prime via range call
        let range = tokenize_range("test.rs", source, "rust", 1, 3);
        assert!(!range.is_empty());
        // second range call should hit cache
        let range2 = tokenize_range("test.rs", source, "rust", 1, 3);
        assert_eq!(range, range2);
    }

    #[test]
    fn cache_eviction_on_size_cap() {
        clear();
        // Create a large source that, when tokenized, will exceed the 32MB cap
        // when multiple copies are inserted.
        let big_source = "a".repeat(8 * 1024 * 1024); // 8 MiB
        tokenize("big1.rs", &big_source, "rust");
        let after_first = len();
        assert_eq!(after_first, 1);

        tokenize("big2.rs", &big_source, "rust");
        tokenize("big3.rs", &big_source, "rust");
        tokenize("big4.rs", &big_source, "rust");
        tokenize("big5.rs", &big_source, "rust");

        // At least one entry should have been evicted to stay under 32MB
        assert!(len() < 5, "expected eviction under 32MB cap, got {} entries", len());
    }
}
