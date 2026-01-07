#![allow(dead_code)]

use crate::invariants::{Invariant, InvariantContext, InvariantResult};

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[derive(Debug, Clone)]
pub struct EscapeSequenceFilter {
    dangerous_patterns: Vec<(&'static [u8], &'static str)>,
}

impl Default for EscapeSequenceFilter {
    fn default() -> Self {
        Self {
            dangerous_patterns: vec![
                (b"\x1b]0;", "Window title manipulation (OSC 0)"),
                (b"\x1b]52;", "Clipboard access (OSC 52)"),
                (b"\x1b[?1049h", "Alternate screen buffer"),
                (b"\x1b#8", "DEC alignment test"),
                (b"\x1b(0", "DEC Special Graphics"),
                (b"\x05", "ENQ - terminal identification"),
                (b"\x1b[c", "Device attributes request"),
                (b"\x1b[6n", "Cursor position report"),
            ],
        }
    }
}

impl Invariant for EscapeSequenceFilter {
    fn name(&self) -> &'static str {
        "escape_sequence_filter"
    }

    fn description(&self) -> &'static str {
        "Verifies dangerous escape sequences are filtered"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let screen = match ctx.screen {
            Some(s) => s,
            None => {
                return InvariantResult::new(
                    self.name(),
                    true,
                    self.description(),
                    Some("No screen available".to_string()),
                    ctx.step,
                    ctx.tick,
                );
            }
        };

        let screen_text = screen.text();
        let screen_bytes = screen_text.as_bytes();
        let mut found_patterns: Vec<String> = Vec::new();

        for (pattern, description) in &self.dangerous_patterns {
            if contains_subsequence(screen_bytes, pattern) {
                found_patterns.push(description.to_string());
            }
        }

        let satisfied = found_patterns.is_empty();

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some(format!(
                    "Dangerous escape sequences detected: {:?}",
                    found_patterns
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

#[derive(Debug, Clone)]
pub struct NoCommandInjection {
    shell_metacharacters: Vec<u8>,
}

impl Default for NoCommandInjection {
    fn default() -> Self {
        Self {
            // Comprehensive list of shell metacharacters
            shell_metacharacters: vec![
                b';',  // Command separator
                b'|',  // Pipe
                b'&',  // Background/AND
                b'`',  // Command substitution (backtick)
                b'$',  // Variable expansion
                b'(',  // Subshell start
                b')',  // Subshell end
                b'{',  // Brace expansion start
                b'}',  // Brace expansion end
                b'<',  // Input redirection
                b'>',  // Output redirection
                b'!',  // History expansion
                b'\n', // Newline (command separator)
                b'#',  // Comment (can hide code after it)
            ],
        }
    }
}

impl Invariant for NoCommandInjection {
    fn name(&self) -> &'static str {
        "no_command_injection"
    }

    fn description(&self) -> &'static str {
        "Verifies no shell command injection via input"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let screen = match ctx.screen {
            Some(s) => s,
            None => {
                return InvariantResult::new(
                    self.name(),
                    true,
                    self.description(),
                    Some("No screen available".to_string()),
                    ctx.step,
                    ctx.tick,
                );
            }
        };

        let screen_text = screen.text();
        let screen_bytes = screen_text.as_bytes();
        let mut found_injections: Vec<String> = Vec::new();

        for &ch in &self.shell_metacharacters {
            if screen_bytes.contains(&ch) {
                found_injections.push(format!("Shell metacharacter '{}' detected", ch as char));
                break;
            }
        }

        let satisfied = found_injections.is_empty();

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some(format!(
                    "Potential command injection: {:?}",
                    found_injections
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

#[derive(Debug, Clone)]
pub struct NoPrivilegeEscalation {
    /// Patterns with word boundary requirements: (pattern, description, require_word_boundary)
    escalation_patterns: Vec<(&'static str, &'static str, bool)>,
}

impl Default for NoPrivilegeEscalation {
    fn default() -> Self {
        Self {
            escalation_patterns: vec![
                // Commands that require word boundaries to avoid false positives
                ("sudo", "sudo command", true), // Avoid matching "pseudocode"
                ("doas", "doas command", true), // Avoid matching "ecdoas"
                // "su " has trailing space so doesn't need word boundary
                ("su ", "su command", false),
                // Paths are specific enough
                ("/etc/passwd", "Password file access", false),
                ("/etc/shadow", "Shadow file access", false),
                ("chmod +s", "Setuid bit manipulation", false),
                ("chown root", "Root ownership change", false),
            ],
        }
    }
}

impl Invariant for NoPrivilegeEscalation {
    fn name(&self) -> &'static str {
        "no_privilege_escalation"
    }

    fn description(&self) -> &'static str {
        "Verifies no privilege escalation attempts via escape sequences"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let screen = match ctx.screen {
            Some(s) => s,
            None => {
                return InvariantResult::new(
                    self.name(),
                    true,
                    self.description(),
                    Some("No screen available".to_string()),
                    ctx.step,
                    ctx.tick,
                );
            }
        };

        let screen_text = screen.text().to_lowercase();
        let mut found_escalations: Vec<String> = Vec::new();

        for (pattern, description, require_word_boundary) in &self.escalation_patterns {
            let lower_pattern = pattern.to_lowercase();
            if *require_word_boundary {
                // Check for word boundaries to avoid false positives
                // Must check ALL occurrences, not just the first one
                // Pattern must be preceded by start of string or non-word char
                // and followed by end of string or non-word char
                let mut search_start = 0;
                while let Some(rel_pos) = screen_text[search_start..].find(&lower_pattern) {
                    let pos = search_start + rel_pos;

                    let before_ok = pos == 0
                        || !screen_text[..pos]
                            .chars()
                            .last()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);
                    let after_pos = pos + lower_pattern.len();
                    let after_ok = after_pos >= screen_text.len()
                        || !screen_text[after_pos..]
                            .chars()
                            .next()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);

                    if before_ok && after_ok {
                        found_escalations.push(description.to_string());
                        break; // Found one match, no need to continue for this pattern
                    }

                    // Move past this occurrence to search for the next one
                    search_start = pos + 1;
                    if search_start >= screen_text.len() {
                        break;
                    }
                }
            } else if screen_text.contains(&lower_pattern) {
                found_escalations.push(description.to_string());
            }
        }

        let satisfied = found_escalations.is_empty();

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some(format!(
                    "Potential privilege escalation: {:?}",
                    found_escalations
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

#[derive(Debug, Clone)]
pub struct BoundsCheckInvariant;

impl Invariant for BoundsCheckInvariant {
    fn name(&self) -> &'static str {
        "bounds_check"
    }

    fn description(&self) -> &'static str {
        "Verifies all cursor and scroll position bounds are validated"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let screen = match ctx.screen {
            Some(s) => s,
            None => {
                return InvariantResult::new(
                    self.name(),
                    true,
                    self.description(),
                    Some("No screen available".to_string()),
                    ctx.step,
                    ctx.tick,
                );
            }
        };

        let (cols, rows) = screen.size();
        let cursor = screen.cursor();

        let mut bounds_violations: Vec<String> = Vec::new();

        if cursor.row >= rows {
            bounds_violations.push(format!(
                "Cursor row {} out of bounds (max: {})",
                cursor.row,
                rows.saturating_sub(1)
            ));
        }

        if cursor.col >= cols {
            bounds_violations.push(format!(
                "Cursor column {} out of bounds (max: {})",
                cursor.col,
                cols.saturating_sub(1)
            ));
        }

        let satisfied = bounds_violations.is_empty();

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some(format!("Bounds violations: {:?}", bounds_violations))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct SecurityScanner {
    dangerous_patterns: Vec<(&'static [u8], &'static str, SecuritySeverity)>,
}

impl SecurityScanner {
    pub fn new() -> Self {
        Self {
            dangerous_patterns: vec![
                (
                    b"\x1b]0;",
                    "Window title manipulation (OSC 0)",
                    SecuritySeverity::Medium,
                ),
                (
                    b"\x1b]52;",
                    "Clipboard access (OSC 52)",
                    SecuritySeverity::High,
                ),
                (
                    b"\x1b[?1049h",
                    "Alternate screen buffer",
                    SecuritySeverity::Low,
                ),
                (b"\x1b#8", "DEC alignment test", SecuritySeverity::Low),
                (b"\x1b(0", "DEC Special Graphics", SecuritySeverity::Low),
                (
                    b"\x05",
                    "ENQ - terminal identification",
                    SecuritySeverity::Medium,
                ),
                (
                    b"\x1b[c",
                    "Device attributes request",
                    SecuritySeverity::Medium,
                ),
                (b"\x1b[6n", "Cursor position report", SecuritySeverity::Low),
                (
                    b"\x1b[?25l",
                    "Hide cursor (potential UI spoofing)",
                    SecuritySeverity::Low,
                ),
                (
                    b"\x1bP",
                    "DCS - Device Control String start",
                    SecuritySeverity::Medium,
                ),
            ],
        }
    }

    pub fn scan(&self, input: &[u8]) -> Vec<SecurityIssue> {
        let mut issues = Vec::new();

        if input.contains(&0x00) {
            issues.push(SecurityIssue::new(
                "Null byte detected - potential injection",
                SecuritySeverity::Medium,
            ));
        }

        if input.len() > 10000 {
            issues.push(SecurityIssue::new(
                "Oversized input - potential DoS",
                SecuritySeverity::Low,
            ));
        }

        for (pattern, description, severity) in &self.dangerous_patterns {
            if contains_subsequence(input, pattern) {
                issues.push(SecurityIssue::new(description, *severity));
            }
        }

        let shell_chars = [
            b';', b'|', b'&', b'`', b'$', b'(', b')', b'{', b'}', b'<', b'>',
        ];
        for ch in &shell_chars {
            if input.contains(ch) {
                issues.push(SecurityIssue::new(
                    &format!("Shell metacharacter '{}' detected", *ch as char),
                    SecuritySeverity::Low,
                ));
                break;
            }
        }

        issues
    }
}

impl Default for SecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SecurityIssue {
    description: String,
    severity: SecuritySeverity,
}

impl SecurityIssue {
    pub fn new(description: &str, severity: SecuritySeverity) -> Self {
        Self {
            description: description.to_string(),
            severity,
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn severity(&self) -> SecuritySeverity {
        self.severity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_sequence_filter() {
        let invariant = EscapeSequenceFilter::default();
        assert_eq!(invariant.name(), "escape_sequence_filter");
    }

    #[test]
    fn test_escape_sequence_filter_detection() {
        use crate::process::{ProcessConfig, PtyProcess};

        let invariant = EscapeSequenceFilter::default();
        let config = ProcessConfig::shell("sleep 0.1");
        let mut process = PtyProcess::spawn(&config).unwrap();
        let mut ctx = InvariantContext {
            screen: None,
            process: &mut process,
            step: 0,
            tick: 0,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        };
        let result = invariant.evaluate(&mut ctx);
        assert!(result.satisfied);

        // Clean up: kill the process if still running
        let _ = process.signal_kill();
        let _ = process.wait();
    }

    #[test]
    fn test_no_command_injection() {
        let invariant = NoCommandInjection::default();
        assert_eq!(invariant.name(), "no_command_injection");
    }

    #[test]
    fn test_no_privilege_escalation() {
        let invariant = NoPrivilegeEscalation::default();
        assert_eq!(invariant.name(), "no_privilege_escalation");
    }

    #[test]
    fn test_bounds_check_invariant() {
        let invariant = BoundsCheckInvariant;
        assert_eq!(invariant.name(), "bounds_check");
    }

    #[test]
    fn test_security_scanner_escape_sequences() {
        let scanner = SecurityScanner::new();

        let issues = scanner.scan(b"\x1b]0;Evil Title\x07");
        assert!(!issues.is_empty());
        assert!(issues
            .iter()
            .any(|i| i.description.contains("Window title")));

        let issues = scanner.scan(b"\x1b]52;c;base64data\x07");
        assert!(!issues.is_empty());

        let issues = scanner.scan(b"Hello, World!");
        assert!(issues.is_empty());
    }

    #[test]
    fn test_shell_metacharacter_detection() {
        let scanner = SecurityScanner::new();

        let issues = scanner.scan(b"echo hello; rm -rf /");
        assert!(issues
            .iter()
            .any(|i| i.description.contains("metacharacter")));
    }

    #[test]
    fn test_security_scanner_null_byte() {
        let scanner = SecurityScanner::new();

        let issues = scanner.scan(b"\x00hello");
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.description.contains("Null byte")));
    }

    #[test]
    fn test_security_scanner_oversized() {
        let scanner = SecurityScanner::new();

        let large_input = b"a".repeat(10001);
        let issues = scanner.scan(&large_input);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.description.contains("Oversized")));
    }

    #[test]
    fn test_security_issue_severity() {
        let issue = SecurityIssue::new("Test", SecuritySeverity::High);
        assert_eq!(issue.severity, SecuritySeverity::High);
    }

    #[test]
    fn test_word_boundary_detection_avoids_false_positives() {
        // Test that "pseudocode" doesn't match "sudo"
        let invariant = NoPrivilegeEscalation::default();

        // Create a mock test by directly testing the word boundary logic
        let text = "pseudocode is helpful";
        let lower_text = text.to_lowercase();
        let pattern = "sudo";

        // The pattern should not match because it's in the middle of "pseudocode"
        if let Some(pos) = lower_text.find(pattern) {
            let before_ok = pos == 0
                || !lower_text[..pos]
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);
            let after_pos = pos + pattern.len();
            let after_ok = after_pos >= lower_text.len()
                || !lower_text[after_pos..]
                    .chars()
                    .next()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);

            // Should NOT match because "p" comes before and "c" comes after
            assert!(
                !before_ok || !after_ok,
                "pseudocode should not trigger sudo detection"
            );
        }

        // Test that standalone "sudo" does match
        let text2 = "run sudo command";
        let lower_text2 = text2.to_lowercase();

        if let Some(pos) = lower_text2.find(pattern) {
            let before_ok = pos == 0
                || !lower_text2[..pos]
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);
            let after_pos = pos + pattern.len();
            let after_ok = after_pos >= lower_text2.len()
                || !lower_text2[after_pos..]
                    .chars()
                    .next()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);

            // Should match because there's a space before and after
            assert!(
                before_ok && after_ok,
                "standalone sudo should trigger detection"
            );
        }

        assert_eq!(invariant.name(), "no_privilege_escalation");
    }

    #[test]
    fn test_shell_metacharacters_comprehensive() {
        let invariant = NoCommandInjection::default();

        // Check all expected metacharacters are in the list
        let expected_chars = [
            b';', b'|', b'&', b'`', b'$', b'(', b')', b'{', b'}', b'<', b'>', b'!', b'\n', b'#',
        ];

        for &ch in &expected_chars {
            assert!(
                invariant.shell_metacharacters.contains(&ch),
                "Expected shell metacharacter '{}' (0x{:02x}) to be in the list",
                ch as char,
                ch
            );
        }
    }

    #[test]
    fn test_word_boundary_multiple_occurrences() {
        // Test that we find "sudo" even when preceded by "pseudocode"
        // The search should check ALL occurrences, not just the first one
        let text = "pseudocode is great, but sudo rm -rf is dangerous";
        let lower_text = text.to_lowercase();
        let pattern = "sudo";

        // Manually search for all occurrences and check word boundaries
        let mut found_standalone = false;
        let mut search_start = 0;

        while let Some(rel_pos) = lower_text[search_start..].find(pattern) {
            let pos = search_start + rel_pos;

            let before_ok = pos == 0
                || !lower_text[..pos]
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);
            let after_pos = pos + pattern.len();
            let after_ok = after_pos >= lower_text.len()
                || !lower_text[after_pos..]
                    .chars()
                    .next()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);

            if before_ok && after_ok {
                found_standalone = true;
                break;
            }

            search_start = pos + 1;
            if search_start >= lower_text.len() {
                break;
            }
        }

        assert!(
            found_standalone,
            "Should find standalone 'sudo' even after 'pseudocode'"
        );
    }

    #[test]
    fn test_word_boundary_no_match_when_all_embedded() {
        // Test that we don't match when ALL occurrences are embedded
        let text = "pseudocode and ecdoas are not commands";
        let lower_text = text.to_lowercase();
        let pattern = "sudo";

        let mut found_standalone = false;
        let mut search_start = 0;

        while let Some(rel_pos) = lower_text[search_start..].find(pattern) {
            let pos = search_start + rel_pos;

            let before_ok = pos == 0
                || !lower_text[..pos]
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);
            let after_pos = pos + pattern.len();
            let after_ok = after_pos >= lower_text.len()
                || !lower_text[after_pos..]
                    .chars()
                    .next()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);

            if before_ok && after_ok {
                found_standalone = true;
                break;
            }

            search_start = pos + 1;
            if search_start >= lower_text.len() {
                break;
            }
        }

        assert!(
            !found_standalone,
            "Should NOT find standalone 'sudo' when only embedded in 'pseudocode'"
        );
    }
}
