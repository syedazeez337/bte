//! Structured Trace Format
//!
//! This module provides the trace schema, serialization logic, and replay capabilities
//! for deterministic reproduction of test runs.

use crate::determinism::DeterministicScheduler;
use crate::invariants::{BuiltInInvariant, InvariantResult};
use crate::process::{ExitReason, PtyProcess};
use crate::scenario::{Scenario, Step};
use crate::screen::Screen;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

/// Trace version for forward compatibility
pub const TRACE_VERSION: &str = "1.0.0";

/// A complete execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    /// Trace format version
    pub version: String,
    /// Timestamp when trace was created (ISO 8601)
    pub created_at: String,
    /// Seed used for deterministic replay
    pub seed: u64,
    /// The scenario that was executed
    pub scenario: Scenario,
    /// RNG state at start of execution
    pub initial_rng_state: u64,
    /// Recorded execution steps
    pub steps: Vec<TraceStep>,
    /// Checkpoints for replay verification
    pub checkpoints: Vec<TraceCheckpoint>,
    /// Invariant evaluation results
    pub invariant_results: Vec<InvariantResult>,
    /// Final execution outcome
    pub outcome: TraceOutcome,
    /// Screen state at the end
    pub final_screen_hash: Option<u64>,
    /// Total ticks elapsed
    pub total_ticks: u64,
}

/// A single step in the trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    /// Step index
    pub index: usize,
    /// The step that was executed
    pub step: Step,
    /// Tick when this step started
    pub start_tick: u64,
    /// Tick when this step completed
    pub end_tick: u64,
    /// Screen hash before this step
    pub before_screen_hash: Option<u64>,
    /// Screen hash after this step
    pub after_screen_hash: Option<u64>,
    /// Whether any invariants were violated during this step
    pub invariant_violations: Vec<String>,
    /// Raw bytes read from PTY during this step
    #[serde(default)]
    pub pty_output: Vec<u8>,
    /// Any error that occurred (if step failed)
    pub error: Option<String>,
}

/// A checkpoint for replay verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCheckpoint {
    /// Checkpoint index
    pub index: usize,
    /// Tick at which checkpoint was recorded
    pub tick: u64,
    /// RNG state at checkpoint
    pub rng_state: u64,
    /// Screen hash at checkpoint
    pub screen_hash: Option<u64>,
    /// Description of the checkpoint
    pub description: String,
}

/// Final outcome of trace execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum TraceOutcome {
    /// Execution completed successfully
    Success {
        /// Exit code of the process
        exit_code: i32,
        /// Total ticks elapsed
        total_ticks: u64,
    },
    /// Execution failed due to invariant violation
    InvariantViolation {
        /// The invariant that was violated
        invariant_name: String,
        /// Checkpoint where violation occurred
        checkpoint_index: usize,
    },
    /// Execution timed out
    Timeout {
        /// Maximum allowed ticks
        max_ticks: u64,
        /// Ticks elapsed before timeout
        elapsed_ticks: u64,
    },
    /// Execution error
    Error {
        /// Error message
        message: String,
        /// Step where error occurred
        step_index: usize,
    },
    /// Process was killed by signal
    Signaled {
        /// Signal number
        signal: i32,
        /// Signal name
        signal_name: String,
    },
    /// Replay divergence detected
    ReplayDivergence {
        /// Expected value
        expected: String,
        /// Actual value
        actual: String,
        /// Context of divergence
        context: String,
    },
}

impl TraceOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, TraceOutcome::Success { .. })
    }
}

/// Builder for creating traces
pub struct TraceBuilder {
    trace: Trace,
    current_step_index: usize,
}

impl TraceBuilder {
    /// Create a new trace builder
    pub fn new(scenario: Scenario, seed: u64) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            trace: Trace {
                version: TRACE_VERSION.to_string(),
                created_at: now,
                seed,
                scenario,
                initial_rng_state: 0,
                steps: Vec::new(),
                checkpoints: Vec::new(),
                invariant_results: Vec::new(),
                outcome: TraceOutcome::Success {
                    exit_code: -1,
                    total_ticks: 0,
                },
                final_screen_hash: None,
                total_ticks: 0,
            },
            current_step_index: 0,
        }
    }

    /// Set the initial RNG state
    pub fn set_initial_rng_state(&mut self, state: u64) {
        self.trace.initial_rng_state = state;
    }

    /// Start recording a step
    pub fn start_step(
        &mut self,
        step: Step,
        screen: Option<&Screen>,
        scheduler: &DeterministicScheduler,
    ) {
        let screen_hash = screen.map(|s| s.state_hash());

        self.trace.steps.push(TraceStep {
            index: self.current_step_index,
            step,
            start_tick: scheduler.now(),
            end_tick: 0,
            before_screen_hash: screen_hash,
            after_screen_hash: None,
            invariant_violations: Vec::new(),
            pty_output: Vec::new(),
            error: None,
        });
        self.current_step_index += 1;
    }

    /// Record output from PTY for current step
    pub fn record_pty_output(&mut self, data: &[u8]) {
        if let Some(step) = self.trace.steps.last_mut() {
            step.pty_output.extend(data);
        }
    }

    /// End the current step
    pub fn end_step(&mut self, screen: Option<&Screen>, scheduler: &DeterministicScheduler) {
        if let Some(step) = self.trace.steps.last_mut() {
            step.end_tick = scheduler.now();
            step.after_screen_hash = screen.map(|s| s.state_hash());
        }
    }

    /// Record an error in the current step
    pub fn record_error(&mut self, error: &str) {
        if let Some(step) = self.trace.steps.last_mut() {
            step.error = Some(error.to_string());
        }
    }

    /// Record an invariant violation
    pub fn record_invariant_violation(&mut self, name: &str) {
        if let Some(step) = self.trace.steps.last_mut() {
            step.invariant_violations.push(name.to_string());
        }
    }

    /// Add a checkpoint
    pub fn add_checkpoint(
        &mut self,
        description: &str,
        scheduler: &DeterministicScheduler,
        screen: Option<&Screen>,
    ) {
        self.trace.checkpoints.push(TraceCheckpoint {
            index: self.trace.checkpoints.len(),
            tick: scheduler.now(),
            rng_state: scheduler.rng_state(),
            screen_hash: screen.map(|s| s.state_hash()),
            description: description.to_string(),
        });
    }

    /// Record invariant result
    pub fn record_invariant_result(&mut self, result: &InvariantResult) {
        self.trace.invariant_results.push(result.clone());
    }

    /// Set the final outcome
    pub fn set_outcome(&mut self, outcome: TraceOutcome) {
        self.trace.outcome = outcome;
    }

    /// Set final screen hash
    pub fn set_final_screen_hash(&mut self, hash: Option<u64>) {
        self.trace.final_screen_hash = hash;
    }

    /// Set total ticks
    pub fn set_total_ticks(&mut self, ticks: u64) {
        self.trace.total_ticks = ticks;
    }

    /// Get all checkpoints
    pub fn checkpoints(&self) -> &[TraceCheckpoint] {
        &self.trace.checkpoints
    }

    /// Build the final trace
    pub fn build(self) -> Trace {
        self.trace
    }

    /// Serialize trace to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.trace)
    }

    /// Serialize trace to YAML
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.trace)
    }
}

/// Replay engine for deterministic reproduction
pub struct ReplayEngine<'a> {
    trace: &'a Trace,
    /// RNG state at each checkpoint for verification
    expected_checkpoints: Vec<TraceCheckpoint>,
    /// Current step index
    step_index: usize,
    /// Current tick
    tick: u64,
    /// Divergences detected
    divergences: Vec<ReplayDivergence>,
    /// Whether to halt on first divergence
    halt_on_divergence: bool,
}

/// A detected divergence during replay
#[derive(Debug, Clone)]
pub struct ReplayDivergence {
    /// Type of divergence
    pub kind: DivergenceKind,
    /// Expected value
    pub expected: String,
    /// Actual value
    pub actual: String,
    /// Context where divergence occurred
    pub context: String,
    /// Step index where divergence occurred
    pub step_index: usize,
    /// Tick at which divergence was detected
    pub tick: u64,
}

#[derive(Debug, Clone)]
pub enum DivergenceKind {
    /// Screen content doesn't match
    ScreenMismatch,
    /// Cursor position doesn't match
    CursorMismatch,
    /// Tick count doesn't match
    TickMismatch,
    /// RNG state doesn't match
    RngMismatch,
    /// Step output doesn't match
    OutputMismatch,
    /// Unexpected invariant violation
    UnexpectedInvariantViolation,
}

impl<'a> ReplayEngine<'a> {
    /// Create a new replay engine
    pub fn new(trace: &'a Trace) -> Self {
        let expected_checkpoints = trace.checkpoints.clone();
        Self {
            trace,
            expected_checkpoints,
            step_index: 0,
            tick: 0,
            divergences: Vec::new(),
            halt_on_divergence: true,
        }
    }

    /// Set whether to halt on first divergence
    pub fn set_halt_on_divergence(&mut self, halt: bool) {
        self.halt_on_divergence = halt;
    }

    /// Get the seed for replay
    pub fn seed(&self) -> u64 {
        self.trace.seed
    }

    /// Get the scenario being replayed
    pub fn scenario(&self) -> &Scenario {
        &self.trace.scenario
    }

    /// Get expected checkpoints
    pub fn expected_checkpoints(&self) -> &[TraceCheckpoint] {
        &self.expected_checkpoints
    }

    /// Verify a checkpoint matches expected
    pub fn verify_checkpoint(
        &mut self,
        checkpoint_index: usize,
        current_tick: u64,
        current_rng_state: u64,
        current_screen_hash: Option<u64>,
    ) -> Result<(), ReplayDivergence> {
        if checkpoint_index >= self.expected_checkpoints.len() {
            return Err(ReplayDivergence {
                kind: DivergenceKind::TickMismatch,
                expected: format!("checkpoint {}", checkpoint_index),
                actual: format!("only {} checkpoints exist", self.expected_checkpoints.len()),
                context: "Checkpoint index out of bounds".to_string(),
                step_index: self.step_index,
                tick: current_tick,
            });
        }

        let expected = &self.expected_checkpoints[checkpoint_index];

        // Verify tick
        if current_tick != expected.tick {
            let divergence = ReplayDivergence {
                kind: DivergenceKind::TickMismatch,
                expected: expected.tick.to_string(),
                actual: current_tick.to_string(),
                context: format!("Checkpoint '{}': tick mismatch", expected.description),
                step_index: self.step_index,
                tick: current_tick,
            };
            if self.halt_on_divergence {
                return Err(divergence);
            } else {
                self.divergences.push(divergence);
            }
        }

        // Verify RNG state
        if current_rng_state != expected.rng_state {
            let divergence = ReplayDivergence {
                kind: DivergenceKind::RngMismatch,
                expected: expected.rng_state.to_string(),
                actual: current_rng_state.to_string(),
                context: format!("Checkpoint '{}': RNG state mismatch", expected.description),
                step_index: self.step_index,
                tick: current_tick,
            };
            if self.halt_on_divergence {
                return Err(divergence);
            } else {
                self.divergences.push(divergence);
            }
        }

        // Verify screen hash (if both are present)
        if let (Some(expected_hash), Some(actual_hash)) =
            (expected.screen_hash, current_screen_hash)
        {
            if expected_hash != actual_hash {
                let divergence = ReplayDivergence {
                    kind: DivergenceKind::ScreenMismatch,
                    expected: format!("0x{:x}", expected_hash),
                    actual: format!("0x{:x}", actual_hash),
                    context: format!(
                        "Checkpoint '{}': screen hash mismatch",
                        expected.description
                    ),
                    step_index: self.step_index,
                    tick: current_tick,
                };
                if self.halt_on_divergence {
                    return Err(divergence);
                } else {
                    self.divergences.push(divergence);
                }
            }
        }

        Ok(())
    }

    /// Verify screen content matches expected
    pub fn verify_screen(
        &self,
        step_index: usize,
        expected_screen: &Screen,
        actual_screen: &Screen,
    ) -> Result<(), ReplayDivergence> {
        if !expected_screen.visual_equals(actual_screen) {
            return Err(ReplayDivergence {
                kind: DivergenceKind::ScreenMismatch,
                expected: expected_screen.text(),
                actual: actual_screen.text(),
                context: format!("Step {}: screen content mismatch", step_index),
                step_index,
                tick: self.tick,
            });
        }
        Ok(())
    }

    /// Advance the step counter
    pub fn advance_step(&mut self) {
        self.step_index += 1;
    }

    /// Get current step index
    pub fn step_index(&self) -> usize {
        self.step_index
    }

    /// Set current tick
    pub fn set_tick(&mut self, tick: u64) {
        self.tick = tick;
    }

    /// Get all divergences
    pub fn divergences(&self) -> &[ReplayDivergence] {
        &self.divergences
    }

    /// Check if replay was successful (no divergences)
    pub fn is_successful(&self) -> bool {
        self.divergences.is_empty()
    }

    /// Get the final outcome
    pub fn outcome(&self) -> &TraceOutcome {
        &self.trace.outcome
    }
}

/// Load a trace from a file
pub fn load_trace(path: &Path) -> Result<Trace, io::Error> {
    let file = File::open(path)?;
    let trace: Trace = serde_json::from_reader(file)?;
    Ok(trace)
}

/// Save a trace to a file
pub fn save_trace(trace: &Trace, path: &Path) -> Result<(), io::Error> {
    let mut file = File::create(path)?;
    let json =
        serde_json::to_string_pretty(trace).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

/// Print trace summary to stdout
pub fn print_trace_summary(trace: &Trace) {
    println!("=== Trace Summary ===");
    println!("Version: {}", trace.version);
    println!("Created: {}", trace.created_at);
    println!("Scenario: {}", trace.scenario.name);
    println!("Seed: {}", trace.seed);
    println!("Steps: {}", trace.steps.len());
    println!("Checkpoints: {}", trace.checkpoints.len());
    println!("Invariant Results: {}", trace.invariant_results.len());

    match &trace.outcome {
        TraceOutcome::Success {
            exit_code,
            total_ticks,
        } => {
            println!("Status: SUCCESS");
            println!("Exit Code: {}", exit_code);
            println!("Total Ticks: {}", total_ticks);
        }
        TraceOutcome::InvariantViolation {
            invariant_name,
            checkpoint_index,
        } => {
            println!("Status: INVARIANT VIOLATION");
            println!("Invariant: {}", invariant_name);
            println!("Checkpoint: {}", checkpoint_index);
        }
        TraceOutcome::Timeout {
            max_ticks,
            elapsed_ticks,
        } => {
            println!("Status: TIMEOUT");
            println!("Max Ticks: {}", max_ticks);
            println!("Elapsed: {}", elapsed_ticks);
        }
        TraceOutcome::Error {
            message,
            step_index,
        } => {
            println!("Status: ERROR");
            println!("Message: {}", message);
            println!("Step: {}", step_index);
        }
        TraceOutcome::Signaled {
            signal,
            signal_name,
        } => {
            println!("Status: SIGNALED");
            println!("Signal: {} ({})", signal_name, signal);
        }
        TraceOutcome::ReplayDivergence {
            expected,
            actual,
            context,
        } => {
            println!("Status: REPLAY DIVERGENCE");
            println!("Expected: {}", expected);
            println!("Actual: {}", actual);
            println!("Context: {}", context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::determinism::DeterministicScheduler;
    use crate::process::ProcessConfig;
    use crate::scenario::{Command, TerminalConfig};
    use std::collections::HashMap;

    fn create_test_scenario() -> Scenario {
        Scenario {
            name: "test scenario".to_string(),
            description: "A test scenario".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(5000),
        }
    }

    #[test]
    fn trace_builder_creates_valid_trace() {
        let scenario = create_test_scenario();
        let mut builder = TraceBuilder::new(scenario, 42);

        let scheduler = DeterministicScheduler::new(42);
        builder.set_initial_rng_state(scheduler.rng_state());

        let screen = Screen::new(80, 24);
        builder.add_checkpoint("initial", &scheduler, Some(&screen));

        builder.start_step(Step::WaitTicks { ticks: 10 }, Some(&screen), &scheduler);
        builder.end_step(Some(&screen), &scheduler);

        let trace = builder.build();

        assert_eq!(trace.version, "1.0.0");
        assert_eq!(trace.seed, 42);
        assert_eq!(trace.steps.len(), 1);
        assert_eq!(trace.checkpoints.len(), 1);
    }

    #[test]
    fn trace_serializes_to_json() {
        let scenario = create_test_scenario();
        let builder = TraceBuilder::new(scenario, 42);
        let trace = builder.build();

        let json = serde_json::to_string(&trace).unwrap();
        let parsed: Trace = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.seed, 42);
        assert_eq!(parsed.version, "1.0.0");
    }

    #[test]
    fn trace_serializes_to_yaml() {
        let scenario = create_test_scenario();
        let builder = TraceBuilder::new(scenario, 42);
        let trace = builder.build();

        let yaml = serde_yaml::to_string(&trace).unwrap();
        let parsed: Trace = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.seed, 42);
    }

    #[test]
    fn replay_engine_verifies_checkpoints() {
        let scenario = create_test_scenario();
        let scheduler = DeterministicScheduler::new(42);

        let mut builder = TraceBuilder::new(scenario, 42);
        builder.set_initial_rng_state(scheduler.rng_state());

        let screen = Screen::new(80, 24);
        let hash = screen.state_hash();

        // Add a checkpoint at tick 0
        scheduler.boundary(crate::determinism::BoundaryKind::BeforeInput);
        builder.add_checkpoint("initial", &scheduler, Some(&screen));

        let trace = builder.build();
        let mut replay = ReplayEngine::new(&trace);

        // Verify the checkpoint matches
        let result = replay.verify_checkpoint(0, 1, scheduler.rng_state(), Some(hash));
        assert!(result.is_ok());
    }

    #[test]
    fn replay_engine_detects_tick_mismatch() {
        let scenario = create_test_scenario();
        let scheduler = DeterministicScheduler::new(42);

        let mut builder = TraceBuilder::new(scenario, 42);
        builder.set_initial_rng_state(scheduler.rng_state());

        let screen = Screen::new(80, 24);
        let hash = screen.state_hash();

        // Add a checkpoint at tick 0
        builder.add_checkpoint("initial", &scheduler, Some(&screen));

        let trace = builder.build();
        let mut replay = ReplayEngine::new(&trace);

        // Try to verify with wrong tick
        let result = replay.verify_checkpoint(0, 999, scheduler.rng_state(), Some(hash));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, DivergenceKind::TickMismatch));
    }

    #[test]
    fn replay_engine_detects_rng_mismatch() {
        let scenario = create_test_scenario();

        let mut builder1 = TraceBuilder::new(scenario.clone(), 42);
        let scheduler1 = DeterministicScheduler::new(42);
        let screen = Screen::new(80, 24);
        builder1.add_checkpoint("initial", &scheduler1, Some(&screen));
        let trace1 = builder1.build();

        let scheduler2 = DeterministicScheduler::new(999); // Different seed
        let mut replay = ReplayEngine::new(&trace1);

        let result = replay.verify_checkpoint(
            0,
            scheduler2.now(),
            scheduler2.rng_state(),
            Some(screen.state_hash()),
        );
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, DivergenceKind::RngMismatch));
    }

    #[test]
    fn trace_outcome_variants() {
        let success = TraceOutcome::Success {
            exit_code: 0,
            total_ticks: 100,
        };
        assert!(success.is_success());

        let violation = TraceOutcome::InvariantViolation {
            invariant_name: "cursor_bounds".to_string(),
            checkpoint_index: 5,
        };
        assert!(!violation.is_success());
    }

    #[test]
    fn load_and_save_trace() {
        let scenario = create_test_scenario();
        let builder = TraceBuilder::new(scenario, 42);
        let trace = builder.build();

        // Save to temp file
        let path = Path::new("/tmp/test_trace.json");
        save_trace(&trace, path).unwrap();

        // Load it back
        let loaded = load_trace(path).unwrap();

        assert_eq!(trace.seed, loaded.seed);
        assert_eq!(trace.version, loaded.version);
        assert_eq!(trace.steps.len(), loaded.steps.len());

        // Clean up
        std::fs::remove_file(path).ok();
    }
}
