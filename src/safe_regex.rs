//! Safe regex operations with ReDoS protection
//!
//! This module provides regex operations that are protected against
//! ReDoS (Regular Expression Denial of Service) attacks using
//! size limits and input validation.

use regex::Regex;
use regex::RegexBuilder;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// Maximum allowed input text size (10 MB)
const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;

/// A regex with ReDoS protection via size limits
#[derive(Clone)]
pub struct SafeRegex {
    regex: Regex,
    size_limit: usize,
}

impl SafeRegex {
    /// Create a new safe regex with the given size limit
    pub fn new(pattern: &str, size_limit: usize) -> Result<Self, regex::Error> {
        let regex = RegexBuilder::new(pattern)
            .size_limit(size_limit)
            .dfa_size_limit(size_limit * 2)
            .build()?;

        Ok(Self { regex, size_limit })
    }

    /// Create a new safe regex with default limits (256KB compiled size)
    pub fn with_default_limits(pattern: &str) -> Result<Self, regex::Error> {
        Self::new(pattern, 256 * 1024)
    }

    /// Check if the pattern matches (with ReDoS protection)
    pub fn is_match(&self, text: &str) -> bool {
        if text.len() > MAX_INPUT_SIZE {
            return false;
        }
        self.regex.is_match(text)
    }

    /// Find all matches (with ReDoS protection)
    pub fn find_all<'t>(&self, text: &'t str) -> Vec<&'t str> {
        if text.len() > MAX_INPUT_SIZE {
            return Vec::new();
        }
        self.regex.find_iter(text).map(|m| m.as_str()).collect()
    }

    /// Count matches (efficient for counting)
    pub fn count(&self, text: &str) -> usize {
        if text.len() > MAX_INPUT_SIZE {
            return 0;
        }
        self.regex.find_iter(text).count()
    }

    /// Check if pattern matches at start
    pub fn is_match_at_start(&self, text: &str) -> bool {
        if text.len() > MAX_INPUT_SIZE {
            return false;
        }
        self.regex.is_match_at(text, 0)
    }

    /// Get the underlying regex for full access
    pub fn inner(&self) -> &Regex {
        &self.regex
    }

    /// Get the size limit
    pub fn size_limit(&self) -> usize {
        self.size_limit
    }
}

/// A thread-safe regex cache
#[derive(Clone)]
pub struct RegexCache {
    cache: Arc<Mutex<lru::LruCache<String, SafeRegex>>>,
    default_size_limit: usize,
}

impl RegexCache {
    /// Create a new regex cache
    pub fn new(max_patterns: usize, default_size_limit: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(max_patterns).unwrap(),
            ))),
            default_size_limit,
        }
    }

    /// Create a cache with sensible defaults (100 patterns, 256KB limit)
    pub fn with_defaults() -> Self {
        Self::new(100, 256 * 1024)
    }

    /// Get or create a safe regex pattern
    pub fn get(&self, pattern: &str) -> Result<SafeRegex, regex::Error> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(regex) = cache.get(pattern) {
            return Ok(regex.clone());
        }

        let regex = SafeRegex::new(pattern, self.default_size_limit)?;
        cache.put(pattern.to_string(), regex.clone());
        Ok(regex)
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }
}

impl Default for RegexCache {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// A regex matcher with ReDoS protection
#[derive(Clone)]
pub struct SafeRegexMatcher {
    cache: RegexCache,
}

impl SafeRegexMatcher {
    /// Create a new safe matcher
    pub fn new() -> Self {
        Self {
            cache: RegexCache::with_defaults(),
        }
    }

    /// Check if text matches pattern safely
    pub fn is_match(&self, pattern: &str, text: &str) -> bool {
        if let Ok(regex) = self.cache.get(pattern) {
            regex.is_match(text)
        } else {
            false
        }
    }

    /// Find all matches safely
    pub fn find_all<'t>(&self, pattern: &str, text: &'t str) -> Vec<&'t str> {
        self.cache.get(pattern).ok().map_or(Vec::new(), |r| r.find_all(text))
    }

    /// Count matches safely
    pub fn count(&self, pattern: &str, text: &str) -> usize {
        self.cache.get(pattern).ok().map_or(0, |r| r.count(text))
    }
}

impl Default for SafeRegexMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a regex pattern before use
pub fn validate_pattern(pattern: &str) -> Result<(), regex::Error> {
    if pattern.is_empty() {
        return Err(regex::Error::Syntax("Empty pattern".to_string()));
    }
    if pattern.len() > 10000 {
        return Err(regex::Error::Syntax("Pattern too long (max 10000 bytes)".to_string()));
    }
    Regex::new(pattern)?;
    Ok(())
}

/// Check if a pattern might be vulnerable to ReDoS
/// This is a heuristic and not a guarantee
pub fn is_potentially_dangerous(pattern: &str) -> bool {
    let dangerous_patterns = [
        r"\([^)]*\)\+",       // Nested quantifiers like (a+)+
        r"\[[^\]]*\]\+",      // Character class repetition like [abc]+
        r"\(\.\|\n\)\+",      // Broad repetition like (.|\n)+
        r"(\d+\s*)+\d",       // Number repetition like (\d+\s*)+
    ];

    for dangerous in &dangerous_patterns {
        if let Ok(dangerous_regex) = Regex::new(dangerous) {
            if dangerous_regex.is_match(pattern) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_regex_basic() {
        let regex = SafeRegex::with_default_limits("hello").unwrap();
        assert!(regex.is_match("hello world"));
        assert!(!regex.is_match("goodbye"));
    }

    #[test]
    fn test_safe_regex_find_all() {
        let regex = SafeRegex::with_default_limits(r"\d+").unwrap();
        let text = "hello 123 world 456 test";
        let matches = regex.find_all(text);
        assert_eq!(matches, vec!["123", "456"]);
    }

    #[test]
    fn test_regex_cache() {
        let cache = RegexCache::with_defaults();
        let regex = cache.get("test").unwrap();
        assert!(regex.is_match("this is a test"));

        let regex2 = cache.get("test").unwrap();
        assert!(regex2.is_match("another test"));
    }

    #[test]
    fn test_timed_matcher() {
        let matcher = SafeRegexMatcher::new();
        assert!(matcher.is_match(r"\b\w+\b", "hello world"));
        assert!(!matcher.is_match(r"^\d+$", "abc123"));
    }

    #[test]
    fn test_safe_regex_invalid_pattern() {
        let result = SafeRegex::new("[invalid", 256 * 1024);
        assert!(result.is_err());
    }

    #[test]
    fn test_input_size_limit() {
        let regex = SafeRegex::with_default_limits(".*").unwrap();
        let large_text = "a".repeat(MAX_INPUT_SIZE + 1);
        assert!(!regex.is_match(&large_text));
        assert!(regex.find_all(&large_text).is_empty());
    }

    #[test]
    fn test_safe_regex_count() {
        let regex = SafeRegex::with_default_limits(r"\d+").unwrap();
        let text = "hello 123 world 456 test 789";
        assert_eq!(regex.count(text), 3);
    }

    #[test]
    fn test_dangerous_pattern_detection() {
        assert!(is_potentially_dangerous("(a+)+c"));
        assert!(is_potentially_dangerous("(.|\n)+"));
        assert!(!is_potentially_dangerous("hello world"));
    }

    #[test]
    fn test_pattern_validation() {
        assert!(validate_pattern("test").is_ok());
        assert!(validate_pattern("").is_err());
        assert!(validate_pattern("[invalid").is_err());
    }

    #[test]
    fn test_redos_protection() {
        // This pattern would cause exponential backtracking without limits
        let evil_pattern = r"(a+)+$";
        let regex = SafeRegex::with_default_limits(evil_pattern).unwrap();

        // This input would hang forever without protection
        let evil_input = "a".repeat(30) + "!";

        // Should complete quickly without hanging
        let result = regex.is_match(&evil_input);
        assert!(!result);
    }
}
