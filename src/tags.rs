//! Test tagging and filtering for scenarios.
//!
//! This module provides functionality to:
//! - Tag scenarios with categories and metadata
//! - Filter scenarios by tags
//! - Parse tag filter expressions
//! - Combine filters with AND/OR logic

use crate::scenario::{Scenario, Tag};
use std::collections::HashSet;

/// Filter expression for tag matching
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagFilter {
    /// Match any of the conditions (OR)
    Or(Vec<TagFilter>),
    /// Match all conditions (AND)
    And(Vec<TagFilter>),
    /// Match scenarios with all specified tags
    HasAll(Vec<String>),
    /// Match scenarios with any of the specified tags
    HasAny(Vec<String>),
    /// Match scenarios with none of the specified tags
    HasNone(Vec<String>),
    /// Match scenarios that have a tag in the given category
    HasCategory(String),
    /// Negate a filter
    Not(Box<TagFilter>),
    /// Match all scenarios (passthrough)
    All,
    /// Match no scenarios (empty set)
    None,
}

/// Tag filter parse error
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected token: {0}")]
    UnexpectedToken(String),

    #[error("unclosed parenthesis")]
    UnclosedParen,

    #[error("empty filter expression")]
    Empty,

    #[error("invalid regex: {0}")]
    InvalidRegex(String),
}

/// Parse a filter expression string
///
/// Supports:
/// - `tag1` - match scenarios with tag "tag1"
/// - `tag1 tag2` - match scenarios with BOTH tags (AND)
/// - `tag1 | tag2` - match scenarios with ANY tag (OR)
/// - `tag1 & tag2` - match scenarios with BOTH tags (AND)
/// - `!tag1` - match scenarios WITHOUT tag "tag1"
/// - `category:type=slow` - match tag with category and value
/// - `(tag1 | tag2) & !tag3` - grouped expressions
///
/// Examples:
/// ```
/// let filter = TagFilter::parse("slow | network").unwrap();
/// let filter = TagFilter::parse("unit & !flaky").unwrap();
/// let filter = TagFilter::parse("(integration | e2e) & !skip").unwrap();
/// ```
impl TagFilter {
    pub fn parse(expr: &str) -> Result<Self, ParseError> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err(ParseError::Empty);
        }

        // Use a simple recursive descent parser
        let mut parser = Parser::new(expr);
        parser.parse()
    }

    /// Check if a scenario matches this filter
    pub fn matches(&self, scenario: &Scenario) -> bool {
        self.matches_tags(&scenario.tags)
    }

    /// Check if a set of tags matches this filter
    pub fn matches_tags(&self, tags: &[Tag]) -> bool {
        match self {
            TagFilter::Or(filters) => filters.iter().any(|f| f.matches_tags(tags)),
            TagFilter::And(filters) => filters.iter().all(|f| f.matches_tags(tags)),
            TagFilter::HasAll(tag_names) => {
                tag_names.iter().all(|n| {
                    // Check if it's a category=value pattern
                    if let Some((cat, val)) = n.split_once('=') {
                        // Match tags with this category AND name
                        tags.iter()
                            .any(|t| t.category.as_ref().is_some_and(|c| c == cat) && t.name == val)
                    } else if let Some((cat, name)) = n.split_once(':') {
                        // Match tags with this category:name
                        tags.iter().any(|t| {
                            t.category.as_ref().is_some_and(|c| c == cat) && t.name == name
                        })
                    } else {
                        // Match by tag name
                        tags.iter().any(|t| t.name == *n)
                    }
                })
            }
            TagFilter::HasAny(tag_names) => {
                tag_names.iter().any(|n| {
                    // Check if it's a category=value pattern
                    if let Some((cat, val)) = n.split_once('=') {
                        tags.iter()
                            .any(|t| t.category.as_ref().is_some_and(|c| c == cat) && t.name == val)
                    } else if let Some((cat, name)) = n.split_once(':') {
                        tags.iter().any(|t| {
                            t.category.as_ref().is_some_and(|c| c == cat) && t.name == name
                        })
                    } else {
                        tags.iter().any(|t| t.name == *n)
                    }
                })
            }
            TagFilter::HasNone(tag_names) => !tag_names.iter().any(|n| {
                if let Some((cat, val)) = n.split_once('=') {
                    tags.iter()
                        .any(|t| t.category.as_ref().is_some_and(|c| c == cat) && t.name == val)
                } else if let Some((cat, name)) = n.split_once(':') {
                    tags.iter()
                        .any(|t| t.category.as_ref().is_some_and(|c| c == cat) && t.name == name)
                } else {
                    tags.iter().any(|t| t.name == *n)
                }
            }),
            TagFilter::HasCategory(category) => {
                tags.iter().any(|t| t.category.as_ref() == Some(category))
            }
            TagFilter::Not(inner) => !inner.matches_tags(tags),
            TagFilter::All => true,
            TagFilter::None => false,
        }
    }
}

/// Simple recursive descent parser for filter expressions
struct Parser<'a> {
    expr: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(expr: &'a str) -> Self {
        Self { expr, pos: 0 }
    }

    fn parse(&mut self) -> Result<TagFilter, ParseError> {
        self.skip_whitespace();
        let result = self.parse_or()?;
        self.skip_whitespace();
        if !self.is_at_end() {
            Err(ParseError::UnexpectedToken(self.remaining().to_string()))
        } else {
            Ok(result)
        }
    }

    fn parse_or(&mut self) -> Result<TagFilter, ParseError> {
        let mut filters = Vec::new();
        filters.push(self.parse_and()?);

        while self.consume_or() {
            filters.push(self.parse_and()?);
        }

        if filters.len() == 1 {
            Ok(filters.into_iter().next().unwrap())
        } else {
            Ok(TagFilter::Or(filters))
        }
    }

    fn parse_and(&mut self) -> Result<TagFilter, ParseError> {
        let mut filters = Vec::new();
        filters.push(self.parse_unary()?);

        while self.consume_and() {
            filters.push(self.parse_unary()?);
        }

        if filters.len() == 1 {
            Ok(filters.into_iter().next().unwrap())
        } else {
            Ok(TagFilter::And(filters))
        }
    }

    fn parse_unary(&mut self) -> Result<TagFilter, ParseError> {
        self.skip_whitespace();

        if self.consume('(') {
            self.skip_whitespace();
            let inner = self.parse_or()?;
            self.skip_whitespace();
            if !self.consume(')') {
                return Err(ParseError::UnclosedParen);
            }
            return Ok(inner);
        }

        if self.consume('!') {
            let inner = self.parse_unary()?;
            return Ok(TagFilter::Not(Box::new(inner)));
        }

        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<TagFilter, ParseError> {
        self.skip_whitespace();

        if self.is_at_end() {
            return Err(ParseError::Empty);
        }

        // Parse an identifier (tag name or category:name pattern)
        if let Some(first) = self.parse_identifier() {
            self.skip_whitespace();

            // Check for category=value pattern (category:value or category=key)
            if self.consume('=') {
                self.skip_whitespace();
                if let Some(value) = self.parse_identifier() {
                    // category=value pattern - match tag with this category and value
                    return Ok(TagFilter::HasAll(vec![format!("{}={}", first, value)]));
                }
            }

            // Check for category:name pattern
            if self.consume(':') {
                self.skip_whitespace();
                if let Some(name) = self.parse_identifier() {
                    // category:name pattern
                    return Ok(TagFilter::HasAll(vec![format!("{}:{}", first, name)]));
                }
                // If no name after ':', treat first as category
                return Ok(TagFilter::HasCategory(first));
            }

            // Just a tag name - match by name
            return Ok(TagFilter::HasAll(vec![first]));
        }

        Err(ParseError::UnexpectedToken(self.remaining().to_string()))
    }

    fn consume(&mut self, c: char) -> bool {
        self.skip_whitespace();
        if !self.is_at_end() && self.expr.as_bytes()[self.pos] as char == c {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn consume_or(&mut self) -> bool {
        self.skip_whitespace();
        if self.expr[self.pos..].starts_with("|") {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn consume_and(&mut self) -> bool {
        self.skip_whitespace();
        if self.expr[self.pos..].starts_with("&") {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            let c = self.expr.as_bytes()[self.pos] as char;
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.expr.len()
    }

    fn remaining(&self) -> &str {
        &self.expr[self.pos..]
    }

    fn parse_identifier(&mut self) -> Option<String> {
        let start = self.pos;
        let mut has_content = false;

        while !self.is_at_end() {
            let c = self.expr.as_bytes()[self.pos] as char;
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                self.pos += 1;
                has_content = true;
            } else {
                break;
            }
        }

        if has_content {
            Some(self.expr[start..self.pos].to_string())
        } else {
            None
        }
    }
}

/// Filter scenarios by tags
pub fn filter_scenarios(
    scenarios: &[(Scenario, std::path::PathBuf)],
    filter: &TagFilter,
) -> Vec<(Scenario, std::path::PathBuf)> {
    scenarios
        .iter()
        .filter(|(s, _)| filter.matches(s))
        .cloned()
        .collect()
}

/// Get all unique tags from scenarios
pub fn get_all_tags(scenarios: &[(Scenario, std::path::PathBuf)]) -> Vec<Tag> {
    let mut all_tags: Vec<Tag> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (scenario, _) in scenarios {
        for tag in &scenario.tags {
            let key = format!(
                "{}:{}",
                tag.category.as_ref().unwrap_or(&"".to_string()),
                tag.name
            );
            if !seen.contains(&key) {
                seen.insert(key.clone());
                all_tags.push(tag.clone());
            }
        }
    }

    all_tags
}

/// Tag statistics
#[derive(Debug, Clone)]
pub struct TagStats {
    pub total_scenarios: usize,
    pub total_tags: usize,
    pub tags_by_category: std::collections::HashMap<String, usize>,
    pub most_common_tags: Vec<(String, usize)>,
}

/// Calculate tag statistics for a set of scenarios
pub fn calculate_tag_stats(scenarios: &[(Scenario, std::path::PathBuf)]) -> TagStats {
    let mut tag_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut category_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (scenario, _) in scenarios {
        for tag in &scenario.tags {
            let key = format!(
                "{}:{}",
                tag.category.as_ref().unwrap_or(&"".to_string()),
                tag.name
            );
            *tag_counts.entry(key.clone()).or_insert(0) += 1;

            if let Some(ref cat) = tag.category {
                *category_counts.entry(cat.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut most_common: Vec<(String, usize)> = tag_counts.into_iter().collect();
    most_common.sort_by(|a, b| b.1.cmp(&a.1));

    TagStats {
        total_scenarios: scenarios.len(),
        total_tags: most_common.iter().map(|(_, c)| c).sum(),
        tags_by_category: category_counts,
        most_common_tags: most_common,
    }
}

/// Print tag statistics
pub fn print_tag_stats(stats: &TagStats) {
    println!();
    println!("=== Tag Statistics ===");
    println!("Total scenarios: {}", stats.total_scenarios);
    println!("Total tags: {}", stats.total_tags);
    println!();

    if !stats.tags_by_category.is_empty() {
        println!("Tags by category:");
        for (category, count) in &stats.tags_by_category {
            println!("  {}: {}", category, count);
        }
        println!();
    }

    if !stats.most_common_tags.is_empty() {
        println!("Most common tags:");
        for (tag, count) in stats.most_common_tags.iter().take(10) {
            println!("  {}: {}", tag, count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_scenario_with_tags(name: &str, tags: Vec<Tag>) -> Scenario {
        Scenario {
            name: name.to_string(),
            description: "".to_string(),
            command: crate::scenario::Command::Simple("echo test".to_string()),
            terminal: Default::default(),
            env: Default::default(),
            steps: vec![],
            invariants: vec![],
            seed: None,
            timeout_ms: None,
            tags,
        }
    }

    #[test]
    fn test_parse_simple_tag() {
        let filter = TagFilter::parse("slow").unwrap();
        let scenario = create_scenario_with_tags(
            "test",
            vec![Tag {
                name: "slow".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        assert!(filter.matches(&scenario));
    }

    #[test]
    fn test_parse_and_filter() {
        let filter = TagFilter::parse("slow & network").unwrap();
        let scenario = create_scenario_with_tags(
            "test",
            vec![
                Tag {
                    name: "slow".to_string(),
                    category: None,
                    metadata: Default::default(),
                },
                Tag {
                    name: "network".to_string(),
                    category: None,
                    metadata: Default::default(),
                },
            ],
        );
        assert!(filter.matches(&scenario));

        // Missing one tag
        let scenario2 = create_scenario_with_tags(
            "test2",
            vec![Tag {
                name: "slow".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        assert!(!filter.matches(&scenario2));
    }

    #[test]
    fn test_parse_or_filter() {
        let filter = TagFilter::parse("slow | fast").unwrap();
        let slow = create_scenario_with_tags(
            "slow_test",
            vec![Tag {
                name: "slow".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        let fast = create_scenario_with_tags(
            "fast_test",
            vec![Tag {
                name: "fast".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        let neither = create_scenario_with_tags(
            "neither",
            vec![Tag {
                name: "other".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );

        assert!(filter.matches(&slow));
        assert!(filter.matches(&fast));
        assert!(!filter.matches(&neither));
    }

    #[test]
    fn test_parse_negation() {
        let filter = TagFilter::parse("!skip").unwrap();
        let with_skip = create_scenario_with_tags(
            "skipped",
            vec![Tag {
                name: "skip".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        let without_skip = create_scenario_with_tags("normal", vec![]);

        assert!(!filter.matches(&with_skip));
        assert!(filter.matches(&without_skip));
    }

    #[test]
    fn test_parse_complex_expression() {
        let filter = TagFilter::parse("(integration | e2e) & !skip").unwrap();

        let integration = create_scenario_with_tags(
            "int_test",
            vec![Tag {
                name: "integration".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        let e2e = create_scenario_with_tags(
            "e2e_test",
            vec![Tag {
                name: "e2e".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );
        let skipped_int = create_scenario_with_tags(
            "skipped_int",
            vec![
                Tag {
                    name: "integration".to_string(),
                    category: None,
                    metadata: Default::default(),
                },
                Tag {
                    name: "skip".to_string(),
                    category: None,
                    metadata: Default::default(),
                },
            ],
        );
        let unit = create_scenario_with_tags(
            "unit_test",
            vec![Tag {
                name: "unit".to_string(),
                category: None,
                metadata: Default::default(),
            }],
        );

        assert!(filter.matches(&integration));
        assert!(filter.matches(&e2e));
        assert!(!filter.matches(&skipped_int));
        assert!(!filter.matches(&unit));
    }

    #[test]
    fn test_category_matching() {
        let filter = TagFilter::parse("type=integration").unwrap();
        let scenario = create_scenario_with_tags(
            "test",
            vec![Tag {
                name: "integration".to_string(),
                category: Some("type".to_string()),
                metadata: Default::default(),
            }],
        );
        assert!(filter.matches(&scenario));
    }

    #[test]
    fn test_filter_scenarios() {
        let scenarios = vec![
            (
                create_scenario_with_tags(
                    "slow_test",
                    vec![Tag {
                        name: "slow".to_string(),
                        category: None,
                        metadata: Default::default(),
                    }],
                ),
                std::path::PathBuf::from("/test1.yaml"),
            ),
            (
                create_scenario_with_tags(
                    "fast_test",
                    vec![Tag {
                        name: "fast".to_string(),
                        category: None,
                        metadata: Default::default(),
                    }],
                ),
                std::path::PathBuf::from("/test2.yaml"),
            ),
            (
                create_scenario_with_tags(
                    "both",
                    vec![
                        Tag {
                            name: "slow".to_string(),
                            category: None,
                            metadata: Default::default(),
                        },
                        Tag {
                            name: "fast".to_string(),
                            category: None,
                            metadata: Default::default(),
                        },
                    ],
                ),
                std::path::PathBuf::from("/test3.yaml"),
            ),
        ];

        let filter = TagFilter::parse("slow").unwrap();
        let filtered = filter_scenarios(&scenarios, &filter);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].0.name, "slow_test");
        assert_eq!(filtered[1].0.name, "both");
    }

    #[test]
    fn test_get_all_tags() {
        let scenarios = vec![
            (
                create_scenario_with_tags(
                    "test1",
                    vec![
                        Tag {
                            name: "slow".to_string(),
                            category: Some("priority".to_string()),
                            metadata: Default::default(),
                        },
                        Tag {
                            name: "network".to_string(),
                            category: Some("type".to_string()),
                            metadata: Default::default(),
                        },
                    ],
                ),
                std::path::PathBuf::from("/test1.yaml"),
            ),
            (
                create_scenario_with_tags(
                    "test2",
                    vec![Tag {
                        name: "slow".to_string(),
                        category: Some("priority".to_string()),
                        metadata: Default::default(),
                    }],
                ),
                std::path::PathBuf::from("/test2.yaml"),
            ),
        ];

        let all_tags = get_all_tags(&scenarios);
        assert_eq!(all_tags.len(), 2); // slow (priority), network (type)
    }

    #[test]
    fn test_calculate_tag_stats() {
        let scenarios = vec![
            (
                create_scenario_with_tags(
                    "test1",
                    vec![
                        Tag {
                            name: "slow".to_string(),
                            category: Some("priority".to_string()),
                            metadata: Default::default(),
                        },
                        Tag {
                            name: "network".to_string(),
                            category: Some("type".to_string()),
                            metadata: Default::default(),
                        },
                    ],
                ),
                std::path::PathBuf::from("/test1.yaml"),
            ),
            (
                create_scenario_with_tags(
                    "test2",
                    vec![Tag {
                        name: "slow".to_string(),
                        category: Some("priority".to_string()),
                        metadata: Default::default(),
                    }],
                ),
                std::path::PathBuf::from("/test2.yaml"),
            ),
        ];

        let stats = calculate_tag_stats(&scenarios);
        assert_eq!(stats.total_scenarios, 2);
        assert_eq!(stats.total_tags, 3); // 2 in test1 + 1 in test2
        assert_eq!(stats.tags_by_category.get("priority").unwrap(), &2);
        assert_eq!(stats.tags_by_category.get("type").unwrap(), &1);
    }
}
