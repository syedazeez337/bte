//! Fuzzy string matching algorithms for approximate pattern matching.
//!
//! This module provides various algorithms for fuzzy/fuzzy string matching:
//! - Levenshtein distance (edit distance)
//! - Jaro-Winkler similarity
//! - Token-based similarity (for multi-word patterns)
//!
//! # Example
//!
//! ```
//! use bte::fuzzy::fuzzy_match;
//!
//! // Direct matching
//! assert!(fuzzy_match("hello world", "hello world", 0).is_some());
//!
//! // Allow some differences
//! assert!(fuzzy_match("hello world", "helo world", 2).is_some());
//!
//! // Too different
//! assert!(fuzzy_match("hello world", "goodbye world", 3).is_none());
//! ```

/// Result of a fuzzy match operation
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyMatch {
    /// The matched text
    pub text: String,
    /// Edit distance from pattern
    pub distance: usize,
    /// Similarity ratio (0.0 to 1.0)
    pub similarity: f64,
    /// Position where match was found
    pub position: usize,
}

/// Calculate Levenshtein distance between two strings.
///
/// The Levenshtein distance is the minimum number of single-character edits
/// (insertions, deletions, or substitutions) required to change one string into the.
///
/// # Arguments
///
/// * `a` - First string
/// * `b` - Second string
///
/// # Returns
///
/// The edit distance between the two strings.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use character indices for Unicode-aware matching
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    // Optimize for when b is a prefix of a or vice versa
    if a_len < b_len {
        if b_chars[..a_len] == a_chars {
            return b_len - a_len;
        }
    } else if a_len > b_len {
        if a_chars[..b_len] == b_chars {
            return a_len - b_len;
        }
    }

    // Standard DP algorithm with space optimization
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut current_row: Vec<usize> = Vec::with_capacity(b_len + 1);

    for i in 1..=a_len {
        current_row.clear();
        current_row.push(i);

        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };

            let deletion = prev_row[j] + 1;
            let insertion = current_row[j - 1] + 1;
            let substitution = prev_row[j - 1] + cost;

            let min = deletion.min(insertion).min(substitution);
            current_row.push(min);
        }

        std::mem::swap(&mut prev_row, &mut current_row);
    }

    prev_row[b_len]
}

/// Calculate Jaro-Winkler similarity between two strings.
///
/// Jaro-Winkler similarity gives more weight to prefixes that match.
/// Returns a value between 0.0 (no similarity) and 1.0 (exact match).
///
/// # Arguments
///
/// * `a` - First string
/// * `b` - Second string
///
/// # Returns
///
/// Similarity ratio between 0.0 and 1.0
pub fn jaro_winkler_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }

    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 || b_len == 0 {
        return 0.0;
    }

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    // Jaro similarity
    let match_distance = (a_len.max(b_len) / 2).saturating_sub(1);

    let mut a_matches = vec![false; a_len];
    let mut b_matches = vec![false; b_len];

    let mut matches = 0usize;
    let mut transpositions = 0usize;
    let mut k = 0usize;

    for i in 0..a_len {
        let start = if i >= match_distance {
            i - match_distance
        } else {
            0
        };
        let end = (i + match_distance + 1).min(b_len);

        for j in start..end {
            if b_matches[j] || a_chars[i] != b_chars[j] {
                continue;
            }
            a_matches[i] = true;
            b_matches[j] = true;
            matches += 1;
            break;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    for i in 0..a_len {
        if !a_matches[i] {
            continue;
        }
        while k < b_len && !b_matches[k] {
            k += 1;
        }
        if k < b_len && a_chars[i] != b_chars[k] {
            transpositions += 1;
        }
        k += 1;
    }

    let jaro = (matches as f64 / a_len as f64
        + matches as f64 / b_len as f64
        + (matches as f64 - transpositions as f64 / 2.0) / matches as f64)
        / 3.0;

    // Winkler modification - bonus for common prefix
    let prefix_len = common_prefix_length(&a_chars, &b_chars);
    let boost = (prefix_len as f64).min(4.0) * 0.1 * (1.0 - jaro);

    (jaro + boost).clamp(0.0, 1.0)
}

/// Calculate common prefix length between two character slices.
fn common_prefix_length(a: &[char], b: &[char]) -> usize {
    let mut len = 0;
    for (ca, cb) in a.iter().zip(b.iter()) {
        if ca == cb {
            len += 1;
        } else {
            break;
        }
    }
    len
}

/// Calculate similarity ratio from Levenshtein distance.
///
/// Converts edit distance to a similarity percentage.
///
/// # Arguments
///
/// * `distance` - Edit distance
/// * `max_len` - Maximum length of the two strings
///
/// # Returns
///
/// Similarity ratio between 0.0 and 1.0
pub fn similarity_from_distance(distance: usize, max_len: usize) -> f64 {
    if max_len == 0 {
        return 1.0;
    }
    (max_len.saturating_sub(distance) as f64) / max_len as f64
}

/// Check if a string approximately matches a pattern.
///
/// # Arguments
///
/// * `text` - The text to search in
/// * `pattern` - The pattern to match against
/// * `max_distance` - Maximum allowed edit distance (0 for exact match)
///
/// # Returns
///
/// `Some(FuzzyMatch)` if a match is found within the threshold, `None` otherwise.
pub fn fuzzy_match(text: &str, pattern: &str, max_distance: usize) -> Option<FuzzyMatch> {
    if pattern.is_empty() {
        return Some(FuzzyMatch {
            text: String::new(),
            distance: 0,
            similarity: 1.0,
            position: 0,
        });
    }

    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    if pattern_chars.len() > text_chars.len() {
        // Pattern longer than text - calculate full distance
        let distance = levenshtein_distance(text, pattern);
        if distance <= max_distance {
            return Some(FuzzyMatch {
                text: text.to_string(),
                distance,
                similarity: similarity_from_distance(
                    distance,
                    pattern_chars.len().max(text_chars.len()),
                ),
                position: 0,
            });
        }
        return None;
    }

    // Search for best match within text
    let mut best_match: Option<FuzzyMatch> = None;

    // Consider all possible starting positions, not just where full pattern fits
    for start in 0..text_chars.len() {
        // For each start, try windows of different lengths around pattern size
        let max_window_len = pattern_chars.len() + max_distance * 2;
        let window_len = pattern_chars.len().min(max_window_len);
        let window_end = (start + window_len).min(text_chars.len());
        let window: String = text_chars[start..window_end].iter().collect();

        let distance = levenshtein_distance(&window, pattern);

        if distance <= max_distance {
            let similarity =
                similarity_from_distance(distance, pattern_chars.len().max(window.len()));

            let candidate = FuzzyMatch {
                text: window,
                distance,
                similarity,
                position: start,
            };

            if best_match.as_ref().map_or(true, |b| distance < b.distance) {
                best_match = Some(candidate);
            }
        }
    }

    best_match
}

/// Check if text contains a approximately matching substring.
///
/// # Arguments
///
/// * `text` - The text to search in
/// * `pattern` - The pattern to find
/// * `max_distance` - Maximum allowed edit distance for the best match
///
/// # Returns
///
/// `Some(FuzzyMatch)` if a close match is found, `None` otherwise.
pub fn contains_fuzzy(text: &str, pattern: &str, max_distance: usize) -> Option<FuzzyMatch> {
    if pattern.is_empty() {
        return Some(FuzzyMatch {
            text: String::new(),
            distance: 0,
            similarity: 1.0,
            position: 0,
        });
    }

    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    let mut best_match: Option<FuzzyMatch> = None;

    // Slide window over text
    for start in 0..text_chars.len() {
        for end in (start + 1)..=text_chars.len() {
            let window: String = text_chars[start..end].iter().collect();
            let distance = levenshtein_distance(&window, pattern);

            if distance <= max_distance {
                let similarity =
                    similarity_from_distance(distance, pattern_chars.len().max(window.len()));

                let candidate = FuzzyMatch {
                    text: window,
                    distance,
                    similarity,
                    position: start,
                };

                if best_match.as_ref().map_or(true, |b| distance < b.distance) {
                    best_match = Some(candidate);
                }
            }
        }
    }

    best_match
}

/// Token-based similarity for multi-word patterns.
///
/// Calculates similarity based on matching words rather than characters.
/// Useful for matching phrases with minor word order changes.
pub fn token_similarity(text: &str, pattern: &str) -> f64 {
    let text_tokens: std::collections::HashSet<String> =
        text.split_whitespace().map(|s| s.to_lowercase()).collect();

    let pattern_tokens: std::collections::HashSet<String> = pattern
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect();

    if pattern_tokens.is_empty() {
        return 1.0;
    }

    let intersection = text_tokens.intersection(&pattern_tokens).count();
    let union = text_tokens.union(&pattern_tokens).count();

    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_exact() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_insertion() {
        assert_eq!(levenshtein_distance("hello", "hello "), 1);
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
    }

    #[test]
    fn test_levenshtein_deletion() {
        assert_eq!(levenshtein_distance("hello ", "hello"), 1);
        assert_eq!(levenshtein_distance("hallo", "hello"), 1);
    }

    #[test]
    fn test_levenshtein_substitution() {
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("hello", "help"), 2); // l→p + delete o
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein_distance("", "hello"), 5);
        assert_eq!(levenshtein_distance("hello", ""), 5);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn test_levenshtein_unicode() {
        // Test with multi-byte characters
        assert_eq!(levenshtein_distance("hello", "hëllo"), 1);
        assert_eq!(levenshtein_distance("日本語", "日本語"), 0);
    }

    #[test]
    fn test_jaro_winkler_exact() {
        assert_eq!(jaro_winkler_similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn test_jaro_winkler_similar() {
        let similarity = jaro_winkler_similarity("hello", "hallo");
        assert!(similarity > 0.8 && similarity < 1.0);
    }

    #[test]
    fn test_jaro_winkler_different() {
        let similarity = jaro_winkler_similarity("hello", "world");
        assert!(similarity < 0.5);
    }

    #[test]
    fn test_fuzzy_match_exact() {
        let result = fuzzy_match("hello world", "hello world", 0);
        assert!(result.is_some());
        assert_eq!(result.unwrap().distance, 0);
    }

    #[test]
    fn test_fuzzy_match_with_edits() {
        let result = fuzzy_match("hello world", "helo world", 2);
        assert!(result.is_some());
        assert!(result.unwrap().distance <= 2);
    }

    #[test]
    fn test_fuzzy_match_too_different() {
        let result = fuzzy_match("hello world", "goodbye world", 3);
        assert!(result.is_none());
    }

    #[test]
    fn test_fuzzy_match_substring() {
        let result = fuzzy_match("the quick brown fox", "quick", 2);
        assert!(result.is_some());
    }

    #[test]
    fn test_contains_fuzzy() {
        let result = contains_fuzzy("the quick brown fox", "quick", 2);
        assert!(result.is_some());
        assert_eq!(result.unwrap().position, 4);
    }

    #[test]
    fn test_token_similarity() {
        assert_eq!(token_similarity("hello world", "hello world"), 1.0);
        let sim = token_similarity("hello world", "world hello");
        assert_eq!(sim, 1.0); // Same tokens
        let sim = token_similarity("hello world", "hello universe");
        assert!(sim > 0.3 && sim < 1.0); // Jaccard: 1/3 ≈ 0.333
    }

    #[test]
    fn test_similarity_from_distance() {
        assert_eq!(similarity_from_distance(0, 10), 1.0);
        assert_eq!(similarity_from_distance(5, 10), 0.5);
        assert_eq!(similarity_from_distance(10, 10), 0.0);
    }
}
