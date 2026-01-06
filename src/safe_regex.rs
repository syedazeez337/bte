//! Safe regex operations with timeout protection
//!
//! This module provides regex operations that are protected against
//! ReDoS (Regular Expression Denial of Service) attacks.

use regex::Regex;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A regex with timeout protection
#[derive(Clone)]
pub struct SafeRegex {
    regex: Regex,
    timeout: Duration,
}

impl SafeRegex {
    /// Create a new safe regex with the given timeout
    pub fn new(pattern: &str, timeout: Duration) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
            timeout,
        })
    }

    /// Create a new safe regex with default timeout (100ms)
    pub fn with_default_timeout(pattern: &str) -> Result<Self, regex::Error> {
        Self::new(pattern, Duration::from_millis(100))
    }

    /// Check if the pattern matches (with timeout)
    pub fn is_match(&self, text: &str) -> bool {
        let start = Instant::now();
        let result = self.regex.is_match(text);
        let elapsed = start.elapsed();
        if elapsed > self.timeout {
            false
        } else {
            result
        }
    }

    /// Find all matches (with timeout)
    pub fn find_all<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let start = Instant::now();
        let mut results = Vec::new();
        for m in self.regex.find_iter(text) {
            if start.elapsed() > self.timeout {
                break;
            }
            results.push(m.as_str());
        }
        results
    }

    /// Get the underlying regex for full access
    pub fn inner(&self) -> &Regex {
        &self.regex
    }
}

/// A thread-safe regex cache with timeout
#[derive(Clone)]
pub struct RegexCache {
    cache: Arc<Mutex<lru::LruCache<String, SafeRegex>>>,
    default_timeout: Duration,
}

impl RegexCache {
    /// Create a new regex cache
    pub fn new(max_patterns: usize, default_timeout: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(max_patterns).unwrap(),
            ))),
            default_timeout,
        }
    }

    /// Create a cache with sensible defaults
    pub fn with_defaults() -> Self {
        Self::new(100, Duration::from_millis(100))
    }

    /// Get or create a safe regex pattern
    pub fn get(&self, pattern: &str) -> Result<SafeRegex, regex::Error> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(regex) = cache.get(pattern) {
            return Ok(regex.clone());
        }

        let regex = SafeRegex::new(pattern, self.default_timeout)?;
        cache.put(pattern.to_string(), regex.clone());
        Ok(regex)
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }
}

impl Default for RegexCache {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// A regex matcher that tracks execution time
#[derive(Clone)]
pub struct TimedRegexMatcher {
    cache: RegexCache,
}

impl TimedRegexMatcher {
    /// Create a new timed matcher
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
    pub fn find_all<'a>(&self, pattern: &str, text: &'a str) -> Vec<&'a str> {
        if let Ok(regex) = self.cache.get(pattern) {
            regex.find_all(text)
        } else {
            Vec::new()
        }
    }
}

impl Default for TimedRegexMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_regex_basic() {
        let regex = SafeRegex::with_default_timeout("hello").unwrap();
        assert!(regex.is_match("hello world"));
        assert!(!regex.is_match("goodbye"));
    }

    #[test]
    fn test_safe_regex_find_all() {
        let regex = SafeRegex::with_default_timeout(r"\d+").unwrap();
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
        let matcher = TimedRegexMatcher::new();
        assert!(matcher.is_match(r"\b\w+\b", "hello world"));
        assert!(!matcher.is_match(r"^\d+$", "abc123"));
    }

    #[test]
    fn test_safe_regex_invalid_pattern() {
        let result = SafeRegex::new("[invalid", Duration::from_millis(100));
        assert!(result.is_err());
    }
}
