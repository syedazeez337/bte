//! Invariant Framework
//!
//! This module provides the invariant definition interface and evaluation engine
//! for behavioral correctness verification.

use crate::process::{ExitReason, PtyProcess};
use crate::screen::Screen;
use serde::{Deserialize, Serialize};

/// Result of an invariant evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub process: &'a PtyProcess,
    /// Current execution step
    pub step: usize,
    /// Current tick
    pub tick: u64,
    /// Whether we're in replay mode
    pub is_replay: bool,
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
    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult;
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

    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let cursor = screen.cursor();
            let (cols, rows) = screen.size();

            let out_of_bounds = cursor.col >= cols || cursor.row >= rows;

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

    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult {
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

    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult {
        let expected = ctx
            .expected_signal
            .as_ref()
            .unwrap_or(&self.expected_signal);
        if let Some(exit_reason) = ctx.process.exit_reason() {
            let signal_name = match exit_reason {
                ExitReason::Exited(_) => "exited",
                ExitReason::Signaled(sig) => {
                    let name = match sig {
                        2 => "SIGINT",
                        9 => "SIGKILL",
                        15 => "SIGTERM",
                        _ => "other",
                    };
                    name
                }
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

    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult {
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

    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult {
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

    fn evaluate(&self, ctx: &InvariantContext) -> InvariantResult {
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
    pub fn evaluate(&mut self, ctx: &InvariantContext) -> &[InvariantResult] {
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
    pub fn results(&self) -> &[InvariantResult] {
        &self.results
    }

    /// Clear all results
    pub fn clear(&mut self) {
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
            process: Box::leak(Box::new(process)),
            step,
            tick,
            is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        }
    }

    #[test]
    fn cursor_bounds_within_screen() {
        let screen = Screen::new(80, 24);
        let ctx = create_test_context(&screen, 0, 0);

        let result = CursorBoundsInvariant.evaluate(&ctx);
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
        let ctx = create_test_context(&screen, 0, 0);
        let result = CursorBoundsInvariant.evaluate(&ctx);
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
        let ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn screen_contains_not_found() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Goodbye".to_string(), true);
        let ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&ctx);
        assert!(!result.satisfied);
    }

    #[test]
    fn screen_not_contains_satisfied() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Goodbye".to_string(), false);
        let ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn screen_not_contains_violated() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        let inv = ScreenContainsInvariant::new("Hello".to_string(), false);
        let ctx = create_test_context(&screen, 0, 0);

        let result = inv.evaluate(&ctx);
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
        let process = PtyProcess::spawn(&config).unwrap();

        let ctx = InvariantContext {
            screen: Some(&screen),
            process: &process,
            step: 0,
            tick: 0,
            is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        };

        let results = engine.evaluate(&ctx);
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
        let process = PtyProcess::spawn(&config).unwrap();

        let ctx = InvariantContext {
            screen: Some(&screen),
            process: &process,
            step: 0,
            tick: 0,
            is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        };

        let result = inv.evaluate(&ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn no_deadlock_detected() {
        let inv = NoDeadlockInvariant::new(50);
        use crate::process::ProcessConfig;
        let config = ProcessConfig::shell("sleep 60");
        let process = PtyProcess::spawn(&config).unwrap();

        let ctx = InvariantContext {
            screen: None,
            process: &process,
            step: 0,
            tick: 100,
            is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 100, // More than timeout
            expected_signal: None,
        };

        let result = inv.evaluate(&ctx);
        assert!(!result.satisfied);
    }
}
