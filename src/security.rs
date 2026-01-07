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
            shell_metacharacters: vec![
                b';', b'|', b'&', b'`', b'$', b'(', b')', b'{', b'}', b'<', b'>',
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
    escalation_patterns: Vec<(&'static [u8], &'static str)>,
}

impl Default for NoPrivilegeEscalation {
    fn default() -> Self {
        Self {
            escalation_patterns: vec![
                (b"sudo", "sudo command"),
                (b"su ", "su command"),
                (b"doas", "doas command"),
                (b"/etc/passwd", "Password file access"),
                (b"/etc/shadow", "Shadow file access"),
                (b"chmod +s", "Setuid bit manipulation"),
                (b"chown root", "Root ownership change"),
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

        for (pattern, description) in &self.escalation_patterns {
            let lower_pattern = String::from_utf8_lossy(pattern).to_lowercase();
            if screen_text.contains(&lower_pattern) {
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
        let config = ProcessConfig::shell("sleep 1");
        let mut process = PtyProcess::spawn(&config).unwrap();
        let ctx = InvariantContext {
            screen: None,
            process: &mut *Box::leak(Box::new(process)),
            step: 0,
            tick: 0,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        };
        let mut ctx = ctx;
        let result = invariant.evaluate(&mut ctx);
        assert!(result.satisfied);
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
}
