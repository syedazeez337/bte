//! Invariant Framework
//!
//! This module provides the invariant definition interface and evaluation engine
//! for behavioral correctness verification.

use crate::process::{ExitReason, PtyProcess};
use crate::screen::Screen;
use serde::{Deserialize, Serialize};

/// Result of an invariant evaluation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct InvariantResult {
    /// Name of the invariant
    pub name: String,
    /// Whether the invariant was satisfied
    pub satisfied: bool,
    /// Description of the check
    pub description: String,
    /// Any details about the check
    pub details: Option<String>,
    /// Step at which this was checked
    pub step: usize,
    /// Tick at which this was checked
    pub tick: u64,
}

impl InvariantResult {
    pub fn new(
        name: &str,
        satisfied: bool,
        description: &str,
        details: Option<String>,
        step: usize,
        tick: u64,
    ) -> Self {
        Self {
            name: name.to_string(),
            satisfied,
            description: description.to_string(),
            details,
            step,
            tick,
        }
    }

    pub fn violation(&self) -> bool {
        !self.satisfied
    }
}

/// Context available when evaluating invariants
pub struct InvariantContext<'a> {
    /// The screen state
    pub screen: Option<&'a Screen>,
    /// The process (may have exited)
    pub process: &'a mut PtyProcess,
    /// Current execution step
    pub step: usize,
    /// Current tick
    pub tick: u64,
    /// Whether we're in replay mode
    pub _is_replay: bool,
    /// Last screen hash (for change detection)
    pub last_screen_hash: Option<u64>,
    /// Number of consecutive ticks with no output
    pub no_output_ticks: u64,
    /// Expected signal for SignalHandled invariant (if applicable)
    pub expected_signal: Option<String>,
}

/// Trait for invariants that can be evaluated
pub trait Invariant: Send + Sync {
    /// Get the name of this invariant
    fn name(&self) -> &str;
    /// Get a description of what this invariant checks
    fn description(&self) -> &str;
    /// Evaluate the invariant with the given context
    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult;
}

/// Built-in invariant types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BuiltInInvariant {
    /// Cursor must stay within screen bounds
    #[serde(rename = "cursor_bounds")]
    CursorBounds,
    /// No deadlock (output within timeout)
    #[serde(rename = "no_deadlock")]
    NoDeadlock {
        /// Timeout in ticks for detecting deadlock
        #[serde(default = "default_deadlock_timeout")]
        timeout_ticks: u64,
    },
    /// Process responds to signals appropriately
    #[serde(rename = "signal_handled")]
    SignalHandled {
        /// Expected signal that should cause termination
        signal: String,
    },
    /// Screen content matches pattern
    #[serde(rename = "screen_contains")]
    ScreenContains {
        /// Pattern to look for
        pattern: String,
    },
    /// Screen content does not match pattern
    #[serde(rename = "screen_not_contains")]
    ScreenNotContains {
        /// Pattern that should not be present
        pattern: String,
    },
    /// Screen has changed since last check
    #[serde(rename = "screen_changed")]
    ScreenChanged,
    /// Screen has not changed (stable)
    #[serde(rename = "screen_stable")]
    ScreenStable {
        /// Minimum ticks to wait before considering stable
        #[serde(default = "default_stable_ticks")]
        min_ticks: u64,
    },
    /// No output after process exit
    #[serde(rename = "no_output_after_exit")]
    NoOutputAfterExit,
    /// Process terminated cleanly (exit code or expected signal)
    #[serde(rename = "process_terminated_cleanly")]
    ProcessTerminatedCleanly {
        /// Signals that are considered clean termination
        #[serde(default)]
        allowed_signals: Vec<i32>,
    },
    /// Viewport is valid (cursor in bounds, no scroll issues)
    #[serde(rename = "viewport_valid")]
    ViewportValid,
    /// Response time constraint
    #[serde(rename = "response_time")]
    ResponseTime {
        /// Maximum ticks for response
        max_ticks: u64,
    },
    /// Maximum latency constraint
    #[serde(rename = "max_latency")]
    MaxLatency {
        /// Maximum ticks for latency
        max_ticks: u64,
    },

    /// Custom invariant with pattern-based checking
    #[serde(rename = "custom")]
    Custom {
        /// Name of the custom invariant
        name: String,
        /// Pattern to check (optional)
        #[serde(default)]
        pattern: Option<String>,
        /// Expected to contain pattern (true) or not contain (false)
        #[serde(default = "default_contains")]
        should_contain: bool,
        /// Expected cursor row (0-indexed, None means don't check)
        #[serde(default)]
        expected_row: Option<usize>,
        /// Expected cursor column (0-indexed, None means don't check)
        #[serde(default)]
        expected_col: Option<usize>,
        /// Custom description for this invariant
        #[serde(default)]
        description: Option<String>,
    },
}

fn default_contains() -> bool {
    true
}

fn default_deadlock_timeout() -> u64 {
    100
}

fn default_stable_ticks() -> u64 {
    10
}

impl BuiltInInvariant {
    /// Create an invariant evaluator from this specification
    pub fn to_evaluator(&self) -> Box<dyn Invariant> {
        match self {
            BuiltInInvariant::CursorBounds => Box::new(CursorBoundsInvariant),
            BuiltInInvariant::NoDeadlock { timeout_ticks } => {
                Box::new(NoDeadlockInvariant::new(*timeout_ticks))
            }
            BuiltInInvariant::SignalHandled { signal } => {
                Box::new(SignalHandledInvariant::new(signal.clone()))
            }
            BuiltInInvariant::ScreenContains { pattern } => {
                Box::new(ScreenContainsInvariant::new(pattern.clone(), true))
            }
            BuiltInInvariant::ScreenNotContains { pattern } => {
                Box::new(ScreenContainsInvariant::new(pattern.clone(), false))
            }
            BuiltInInvariant::ScreenChanged => Box::new(ScreenChangedInvariant),
            BuiltInInvariant::ScreenStable { min_ticks } => {
                Box::new(ScreenStableInvariant::new(*min_ticks))
            }
            BuiltInInvariant::NoOutputAfterExit => Box::new(NoOutputAfterExitInvariant),
            BuiltInInvariant::ProcessTerminatedCleanly { allowed_signals } => Box::new(
                ProcessTerminatedCleanlyInvariant::new(allowed_signals.clone()),
            ),
            BuiltInInvariant::ViewportValid => Box::new(ViewportValidInvariant),
            BuiltInInvariant::ResponseTime { max_ticks } => {
                Box::new(ResponseTimeInvariant::new(*max_ticks))
            }
            BuiltInInvariant::MaxLatency { max_ticks } => {
                Box::new(MaxLatencyInvariant::new(*max_ticks))
            }
            BuiltInInvariant::Custom {
                name,
                pattern,
                should_contain,
                expected_row,
                expected_col,
                description,
            } => Box::new(CustomInvariant::new(
                name.clone(),
                pattern.clone(),
                *should_contain,
                *expected_row,
                *expected_col,
                description
                    .clone()
                    .or(Some(format!("Custom invariant: {}", name))),
            )),
        }
    }
}

/// Cursor bounds invariant - ensures cursor never leaves screen
pub struct CursorBoundsInvariant;

impl Invariant for CursorBoundsInvariant {
    fn name(&self) -> &str {
        "cursor_bounds"
    }

    fn description(&self) -> &str {
        "Cursor position must always be within screen bounds"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let cursor = screen.cursor();
            let (cols, rows) = screen.size();

            // Allow cursor at exactly cols/rows (one past visible is valid for "next position")
            // Only flag if strictly beyond visible area
            let out_of_bounds = cursor.col > cols || cursor.row > rows;

            InvariantResult::new(
                self.name(),
                !out_of_bounds,
                self.description(),
                if out_of_bounds {
                    Some(format!(
                        "Cursor at ({}, {}) but screen is {}x{}",
                        cursor.col, cursor.row, cols, rows
                    ))
                } else {
                    None
                },
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(
                self.name(),
                true,
                "No screen available - skipping cursor bounds check",
                None,
                ctx.step,
                ctx.tick,
            )
        }
    }
}

/// No deadlock invariant - detects if process is stuck
pub struct NoDeadlockInvariant {
    timeout_ticks: u64,
}

impl NoDeadlockInvariant {
    pub fn new(timeout_ticks: u64) -> Self {
        Self { timeout_ticks }
    }
}

impl Invariant for NoDeadlockInvariant {
    fn name(&self) -> &str {
        "no_deadlock"
    }

    fn description(&self) -> &str {
        "Process must produce output within timeout"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let is_stuck = ctx.no_output_ticks >= self.timeout_ticks;

        // Also check if process has exited normally
        let process_exited = ctx.process.has_exited();

        InvariantResult::new(
            self.name(),
            !is_stuck || process_exited,
            self.description(),
            if is_stuck && !process_exited {
                Some(format!(
                    "No output for {} ticks (timeout: {})",
                    ctx.no_output_ticks, self.timeout_ticks
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

/// Signal handling invariant - verifies signal was handled
pub struct SignalHandledInvariant {
    expected_signal: String,
}

impl SignalHandledInvariant {
    pub fn new(signal: String) -> Self {
        Self {
            expected_signal: signal,
        }
    }
}

impl Invariant for SignalHandledInvariant {
    fn name(&self) -> &str {
        "signal_handled"
    }

    fn description(&self) -> &str {
        "Process must respond appropriately to signals"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let expected = ctx
            .expected_signal
            .as_ref()
            .unwrap_or(&self.expected_signal);
        if let Some(exit_reason) = ctx.process.exit_reason() {
            let signal_name = match exit_reason {
                ExitReason::Exited(_) => "exited",
                ExitReason::Signaled(sig) => match sig {
                    2 => "SIGINT",
                    9 => "SIGKILL",
                    15 => "SIGTERM",
                    _ => "other",
                },
                ExitReason::Running => "running",
            };

            let handled = match exit_reason {
                ExitReason::Exited(_) => expected == "exit",
                ExitReason::Signaled(sig) => {
                    let expected_upper = expected.to_uppercase();
                    match expected_upper.as_str() {
                        "SIGINT" => sig == 2,
                        "SIGTERM" => sig == 15,
                        "SIGKILL" => sig == 9,
                        _ => false,
                    }
                }
                ExitReason::Running => false,
            };

            InvariantResult::new(
                self.name(),
                handled,
                self.description(),
                Some(format!("Expected: {}, Got: {}", expected, signal_name)),
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(
                self.name(),
                false,
                "Process still running - cannot verify signal handling",
                Some("Signal invariant requires process to have exited".to_string()),
                ctx.step,
                ctx.tick,
            )
        }
    }
}

/// Screen content invariant - checks for pattern presence/absence
pub struct ScreenContainsInvariant {
    pattern: String,
    should_contain: bool,
}

impl ScreenContainsInvariant {
    pub fn new(pattern: String, should_contain: bool) -> Self {
        Self {
            pattern,
            should_contain,
        }
    }
}

impl Invariant for ScreenContainsInvariant {
    fn name(&self) -> &str {
        if self.should_contain {
            "screen_contains"
        } else {
            "screen_not_contains"
        }
    }

    fn description(&self) -> &str {
        if self.should_contain {
            "Screen must contain expected pattern"
        } else {
            "Screen must not contain forbidden pattern"
        }
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let screen_text = ctx.screen.map(|s| s.text()).unwrap_or_default();
        let contains = screen_text.contains(&self.pattern);

        let satisfied = if self.should_contain {
            contains
        } else {
            !contains
        };

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            Some(format!(
                "Pattern '{}': {} (screen content length: {})",
                self.pattern,
                if contains { "found" } else { "not found" },
                screen_text.len()
            )),
            ctx.step,
            ctx.tick,
        )
    }
}

/// Screen changed invariant - detects if screen has updated
pub struct ScreenChangedInvariant;

impl Invariant for ScreenChangedInvariant {
    fn name(&self) -> &str {
        "screen_changed"
    }

    fn description(&self) -> &str {
        "Screen must have changed since last check"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_hash = ctx.screen.map(|s| s.state_hash());
        let changed = current_hash != ctx.last_screen_hash;

        InvariantResult::new(
            self.name(),
            changed,
            self.description(),
            Some(format!(
                "Changed: {}, Last hash: {:?}, Current hash: {:?}",
                changed, ctx.last_screen_hash, current_hash
            )),
            ctx.step,
            ctx.tick,
        )
    }
}

/// Screen stable invariant - detects when screen stops changing
pub struct ScreenStableInvariant {
    min_ticks: u64,
}

impl ScreenStableInvariant {
    pub fn new(min_ticks: u64) -> Self {
        Self { min_ticks }
    }
}

impl Invariant for ScreenStableInvariant {
    fn name(&self) -> &str {
        "screen_stable"
    }

    fn description(&self) -> &str {
        "Screen should remain stable for minimum ticks"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_hash = ctx.screen.map(|s| s.state_hash());
        let stable = current_hash == ctx.last_screen_hash && ctx.no_output_ticks >= self.min_ticks;

        InvariantResult::new(
            self.name(),
            stable,
            self.description(),
            Some(format!(
                "Stable: {}, Consecutive no-change ticks: {} (min: {})",
                stable, ctx.no_output_ticks, self.min_ticks
            )),
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct NoOutputAfterExitInvariant;

impl Invariant for NoOutputAfterExitInvariant {
    fn name(&self) -> &str {
        "no_output_after_exit"
    }

    fn description(&self) -> &str {
        "No output should be produced after process exit"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let exit_status = ctx.process.try_wait();
        let has_exited = match exit_status {
            Ok(Some(ExitReason::Running)) => false,
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(_) => false,
        };
        let satisfied = if has_exited {
            ctx.no_output_ticks >= 1
        } else {
            true
        };

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some("Process exited but output was detected".to_string())
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct ProcessTerminatedCleanlyInvariant {
    allowed_signals: Vec<i32>,
}

impl ProcessTerminatedCleanlyInvariant {
    pub fn new(allowed_signals: Vec<i32>) -> Self {
        Self { allowed_signals }
    }
}

impl Invariant for ProcessTerminatedCleanlyInvariant {
    fn name(&self) -> &str {
        "process_terminated_cleanly"
    }

    fn description(&self) -> &str {
        "Process should terminate with clean exit code or allowed signal"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let exit_status = ctx.process.try_wait();
        let satisfied = match exit_status {
            Ok(Some(exit_reason)) => match exit_reason {
                ExitReason::Exited(code) => code >= 0,
                ExitReason::Signaled(sig) => self.allowed_signals.contains(&sig),
                ExitReason::Running => true,
            },
            Ok(None) => true,
            Err(_) => true,
        };

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            match exit_status {
                Ok(Some(exit_reason)) => match exit_reason {
                    ExitReason::Exited(code) if code < 0 => {
                        Some(format!("Process exited with error code {}", code))
                    }
                    ExitReason::Signaled(sig) if !self.allowed_signals.contains(&sig) => {
                        Some(format!("Process killed by signal {}", sig))
                    }
                    _ => None,
                },
                _ => None,
            },
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct ViewportValidInvariant;

impl Invariant for ViewportValidInvariant {
    fn name(&self) -> &str {
        "viewport_valid"
    }

    fn description(&self) -> &str {
        "Viewport should be valid (cursor in bounds, no scroll issues)"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let valid = if let Some(screen) = ctx.screen {
            let cursor = screen.cursor();
            let (cols, rows) = screen.size();
            cursor.col < cols && cursor.row < rows
        } else {
            true
        };

        InvariantResult::new(
            self.name(),
            valid,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct ResponseTimeInvariant {
    max_ticks: u64,
}

impl ResponseTimeInvariant {
    pub fn new(max_ticks: u64) -> Self {
        Self { max_ticks }
    }
}

impl Invariant for ResponseTimeInvariant {
    fn name(&self) -> &str {
        "response_time"
    }

    fn description(&self) -> &str {
        "Process should respond within expected time"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let satisfied = ctx.tick <= self.max_ticks;

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some(format!(
                    "Tick {} exceeds max allowed {}",
                    ctx.tick, self.max_ticks
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct MaxLatencyInvariant {
    max_ticks: u64,
}

impl MaxLatencyInvariant {
    pub fn new(max_ticks: u64) -> Self {
        Self { max_ticks }
    }
}

impl Invariant for MaxLatencyInvariant {
    fn name(&self) -> &str {
        "max_latency"
    }

    fn description(&self) -> &str {
        "Maximum latency should not exceed threshold"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let satisfied = ctx.tick <= self.max_ticks;

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if !satisfied {
                Some(format!(
                    "Latency {} ticks exceeds max {}",
                    ctx.tick, self.max_ticks
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

/// Custom invariant with flexible pattern and cursor position checking
pub struct CustomInvariant {
    name: String,
    pattern: Option<String>,
    should_contain: bool,
    expected_row: Option<usize>,
    expected_col: Option<usize>,
    description: Option<String>,
}

impl CustomInvariant {
    pub fn new(
        name: String,
        pattern: Option<String>,
        should_contain: bool,
        expected_row: Option<usize>,
        expected_col: Option<usize>,
        description: Option<String>,
    ) -> Self {
        Self {
            name,
            pattern,
            should_contain,
            expected_row,
            expected_col,
            description,
        }
    }
}

impl Invariant for CustomInvariant {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        self.description.as_deref().unwrap_or_else(|| {
            static DEFAULT_DESC: &str = "Custom invariant check";
            DEFAULT_DESC
        })
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let mut satisfied = true;
        let mut details = Vec::new();

        // Check pattern if specified
        if let Some(pattern) = &self.pattern {
            if let Some(screen) = ctx.screen {
                let screen_text = screen.text();
                let contains = screen_text.contains(pattern);

                if self.should_contain {
                    if !contains {
                        satisfied = false;
                        details.push(format!(
                            "Pattern '{}' not found in screen content (length: {})",
                            pattern,
                            screen_text.len()
                        ));
                    }
                } else {
                    if contains {
                        satisfied = false;
                        details.push(format!(
                            "Pattern '{}' should not be present but was found",
                            pattern
                        ));
                    }
                }
            } else {
                details.push("No screen available for pattern check".to_string());
            }
        }

        // Check cursor position if specified
        if let (Some(row), Some(col)) = (self.expected_row, self.expected_col) {
            if let Some(screen) = ctx.screen {
                let cursor = screen.cursor();
                let cursor_row = cursor.row as usize;
                let cursor_col = cursor.col as usize;

                if cursor_row != row || cursor_col != col {
                    satisfied = false;
                    details.push(format!(
                        "Expected cursor at ({}, {}) but was at ({}, {})",
                        row, col, cursor_row, cursor_col
                    ));
                }
            } else {
                details.push("No screen available for cursor check".to_string());
            }
        }

        // Check only row if specified
        if let (Some(row), None) = (self.expected_row, self.expected_col) {
            if let Some(screen) = ctx.screen {
                let cursor_row = screen.cursor().row as usize;
                if cursor_row != row {
                    satisfied = false;
                    details.push(format!(
                        "Expected cursor row {} but was at row {}",
                        row, cursor_row
                    ));
                }
            }
        }

        // Check only col if specified
        if let (None, Some(col)) = (self.expected_row, self.expected_col) {
            if let Some(screen) = ctx.screen {
                let cursor_col = screen.cursor().col as usize;
                if cursor_col != col {
                    satisfied = false;
                    details.push(format!(
                        "Expected cursor column {} but was at column {}",
                        col, cursor_col
                    ));
                }
            }
        }

        InvariantResult::new(
            self.name(),
            satisfied,
            self.description(),
            if details.is_empty() {
                None
            } else {
                Some(details.join("; "))
            },
            ctx.step,
            ctx.tick,
        )
    }
}

/// Engine for evaluating invariants
pub struct InvariantEngine {
    invariants: Vec<Box<dyn Invariant>>,
    results: Vec<InvariantResult>,
}

impl InvariantEngine {
    /// Create a new engine with no invariants
    pub fn new() -> Self {
        Self {
            invariants: Vec::new(),
            results: Vec::new(),
        }
    }

    /// Add an invariant to the engine
    pub fn add_invariant(&mut self, invariant: Box<dyn Invariant>) {
        self.invariants.push(invariant);
    }

    /// Add built-in invariants from specification
    pub fn add_builtin_invariants(&mut self, invariants: &[BuiltInInvariant]) {
        for inv in invariants {
            self.add_invariant(inv.to_evaluator());
        }
    }

    /// Evaluate all invariants
    pub fn evaluate(&mut self, ctx: &mut InvariantContext) -> &[InvariantResult] {
        self.results.clear();
        for invariant in &self.invariants {
            let result = invariant.evaluate(ctx);
            self.results.push(result);
        }
        &self.results
    }

    /// Check if all invariants are satisfied
    pub fn all_satisfied(&self) -> bool {
        self.results.iter().all(|r| r.satisfied)
    }

    /// Get all violations
    pub fn violations(&self) -> Vec<&InvariantResult> {
        self.results.iter().filter(|r| r.violation()).collect()
    }

    /// Get all results
    pub fn _results(&self) -> &[InvariantResult] {
        &self.results
    }

    /// Clear all results
    pub fn _clear(&mut self) {
        self.results.clear();
    }
}

impl Default for InvariantEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;

    fn create_test_context<'a>(screen: &'a Screen, step: usize, tick: u64) -> InvariantContext<'a> {
        use crate::process::ProcessConfig;

        let config = ProcessConfig::shell("sleep 1");
        let process = PtyProcess::spawn(&config).unwrap();

        InvariantContext {
            screen: Some(screen),
            process: &mut *Box::leak(Box::new(process)),
            step,
            tick,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        }
    }

    #[test]
    fn cursor_bounds_within_screen() {
        let screen = Screen::new(80, 24);
        let mut ctx = create_test_context(&screen, 0, 0);

        let result = CursorBoundsInvariant.evaluate(&mut ctx);
        assert!(result.satisfied);
        assert_eq!(result.name, "cursor_bounds");
    }

    #[test]
    fn cursor_bounds_out_of_bounds() {
        let mut screen = Screen::new(10, 5);
        screen.process(b"\x1b[100;100H"); // Try to move way out of bounds
        let cursor = screen.cursor();

        // The screen should clamp cursor to valid bounds
        assert!(
            cursor.col < 10,
            "Cursor column {} should be < 10",
            cursor.col
        );
        assert!(cursor.row < 5, "Cursor row {} should be < 5", cursor.row);

        // The invariant should be satisfied because the screen prevents out-of-bounds
        let mut ctx = create_test_context(&screen, 0, 0);
        let result = CursorBoundsInvariant.evaluate(&mut ctx);
        assert!(
            result.satisfied,
            "Invariant should be satisfied as screen clamps cursor"
        );
    }

    #[test]
    fn screen_contains_found() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Hello".to_string(), true);
        let mut ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&mut ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn screen_contains_not_found() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Goodbye".to_string(), true);
        let mut ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&mut ctx);
        assert!(!result.satisfied);
    }

    #[test]
    fn screen_not_contains_satisfied() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Goodbye".to_string(), false);
        let mut ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&mut ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn screen_not_contains_violated() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Hello".to_string(), false);
        let mut ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&mut ctx);
        assert!(!result.satisfied);
    }

    #[test]
    fn engine_evaluates_all_invariants() {
        let mut engine = InvariantEngine::new();
        engine.add_invariant(Box::new(CursorBoundsInvariant));
        engine.add_invariant(Box::new(ScreenChangedInvariant));

        let screen = Screen::new(80, 24);
        use crate::process::ProcessConfig;
        let config = ProcessConfig::shell("sleep 1");
        let mut process = PtyProcess::spawn(&config).unwrap();

        let mut ctx = InvariantContext {
            screen: Some(&screen),
            process: &mut process,
            step: 0,
            tick: 0,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        };

        let results = engine.evaluate(&mut ctx);
        assert_eq!(results.len(), 2);
        assert!(engine.all_satisfied());
        assert!(engine.violations().is_empty());
    }

    #[test]
    fn no_deadlock_with_output() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello");

        let inv = NoDeadlockInvariant::new(100);
        use crate::process::ProcessConfig;
        let config = ProcessConfig::shell("sleep 1");
        let mut process = PtyProcess::spawn(&config).unwrap();

        let mut ctx = InvariantContext {
            screen: Some(&screen),
            process: &mut process,
            step: 0,
            tick: 0,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        };

        let result = inv.evaluate(&mut ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn no_deadlock_detected() {
        let inv = NoDeadlockInvariant::new(50);
        use crate::process::ProcessConfig;
        let config = ProcessConfig::shell("sleep 60");
        let mut process = PtyProcess::spawn(&config).unwrap();

        let mut ctx = InvariantContext {
            screen: None,
            process: &mut process,
            step: 0,
            tick: 100,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 100, // More than timeout
            expected_signal: None,
        };

        let result = inv.evaluate(&mut ctx);
        assert!(!result.satisfied);
    }
}
