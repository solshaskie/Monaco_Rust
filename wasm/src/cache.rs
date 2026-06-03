use std::cell::RefCell;
use std::collections::HashMap;

use crate::tokenize::{tokenize_source, WasmToken};

/// A cached tokenization result for a single resource.
#[derive(Clone)]
struct CacheEntry {
    source: String,
    language: String,
    tokens: Vec<WasmToken>,
}

thread_local! {
    static TOKEN_CACHE: RefCell<HashMap<String, CacheEntry>> = RefCell::new(HashMap::new());
}

/// Store a full snapshot and its tokenization result.
pub fn prime(resource: &str, source: &str, language: &str) -> usize {
    let tokens = tokenize_source(source, language);
    let count = tokens.len();
    TOKEN_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            resource.to_string(),
            CacheEntry {
                source: source.to_string(),
                language: language.to_string(),
                tokens,
            },
        );
    });
    count
}

/// Return cached tokens if the source snapshot is unchanged; otherwise re-tokenize
/// and update the cache.
pub fn tokenize(resource: &str, source: &str, language: &str) -> Vec<WasmToken> {
    TOKEN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get(resource) {
            if entry.source == source && entry.language == language {
                return entry.tokens.clone();
            }
        }
        let tokens = tokenize_source(source, language);
        cache.insert(
            resource.to_string(),
            CacheEntry {
                source: source.to_string(),
                language: language.to_string(),
                tokens: tokens.clone(),
            },
        );
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
    TOKEN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get(resource) {
            if entry.source == source && entry.language == language {
                return entry
                    .tokens
                    .iter()
                    .filter(|t| t.line >= start_line && t.line < end_line)
                    .cloned()
                    .collect();
            }
        }
        let tokens = tokenize_source(source, language);
        let range_tokens: Vec<WasmToken> = tokens
            .iter()
            .filter(|t| t.line >= start_line && t.line < end_line)
            .cloned()
            .collect();
        cache.insert(
            resource.to_string(),
            CacheEntry {
                source: source.to_string(),
                language: language.to_string(),
                tokens,
            },
        );
        range_tokens
    })
}

/// Remove a single resource from the cache.
pub fn invalidate(resource: &str) {
    TOKEN_CACHE.with(|cache| {
        cache.borrow_mut().remove(resource);
    });
}

/// Clear the entire cache.
pub fn clear() {
    TOKEN_CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
}

/// Return the number of cached resources.
pub fn len() -> usize {
    TOKEN_CACHE.with(|cache| cache.borrow().len())
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
}
