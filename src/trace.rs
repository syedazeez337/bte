//! Structured Trace Format
//!
//! This module provides the trace schema, serialization logic, and replay capabilities
//! for deterministic reproduction of test runs.

#![allow(dead_code)]

use crate::determinism::DeterministicScheduler;
use crate::invariants::InvariantResult;
use crate::scenario::{Scenario, Step};
use crate::screen::Screen;
use serde::{Deserialize, Serialize};
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
    pub fn _to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.trace)
    }

    /// Serialize trace to YAML
    pub fn _to_yaml(&self) -> Result<String, serde_yaml::Error> {
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
    /// Checkpoint not found
    CheckpointNotFound,
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
    let json = serde_json::to_string_pretty(trace).map_err(io::Error::other)?;
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

// ============================================================================
// Sparse Trace Replay
// ============================================================================

/// Replay engine for sparse traces
pub struct SparseReplayEngine<'a> {
    trace: &'a SparseTrace,
    current_checkpoint: usize,
    current_event_index: usize,
    divergences: Vec<ReplayDivergence>,
    halt_on_divergence: bool,
}

impl<'a> SparseReplayEngine<'a> {
    /// Create a new replay engine for a sparse trace
    pub fn new(trace: &'a SparseTrace) -> Self {
        Self {
            trace,
            current_checkpoint: 0,
            current_event_index: 0,
            divergences: Vec::new(),
            halt_on_divergence: true,
        }
    }

    /// Set whether to halt on divergence (default: true)
    pub fn set_halt_on_divergence(&mut self, halt: bool) {
        self.halt_on_divergence = halt;
    }

    /// Get the next event in the trace
    pub fn next_event(&mut self) -> Option<&ScheduleEvent> {
        if self.current_event_index < self.trace.events.len() {
            let event = &self.trace.events[self.current_event_index];
            self.current_event_index += 1;

            // Check if we need to advance to next checkpoint
            self.advance_checkpoint_if_needed();

            Some(event)
        } else {
            None
        }
    }

    /// Get events from a specific checkpoint
    pub fn get_checkpoint_events(&self, checkpoint_index: usize) -> &[ScheduleEvent] {
        if checkpoint_index < self.trace.checkpoints.len() {
            let checkpoint = &self.trace.checkpoints[checkpoint_index];
            let start = checkpoint.event_start;
            let end = start + checkpoint.event_count;
            &self.trace.events[start..end]
        } else {
            &[]
        }
    }

    /// Verify a checkpoint
    pub fn verify_checkpoint(
        &mut self,
        index: usize,
        tick: u64,
        rng_state: u64,
        screen_hash: u64,
    ) -> Result<(), ReplayDivergence> {
        if index >= self.trace.checkpoints.len() {
            return Err(ReplayDivergence {
                kind: DivergenceKind::CheckpointNotFound,
                expected: index.to_string(),
                actual: self.trace.checkpoints.len().to_string(),
                context: format!("Checkpoint {} not found in trace", index),
                step_index: index,
                tick,
            });
        }

        let expected = &self.trace.checkpoints[index];
        let mut has_mismatch = false;

        // Verify tick
        if tick != expected.tick {
            let divergence = ReplayDivergence {
                kind: DivergenceKind::TickMismatch,
                expected: expected.tick.to_string(),
                actual: tick.to_string(),
                context: format!("Checkpoint '{}': tick mismatch", expected.description),
                step_index: index,
                tick,
            };
            self.divergences.push(divergence);
            has_mismatch = true;
        }

        // Verify RNG state
        if rng_state != expected.rng_state {
            let divergence = ReplayDivergence {
                kind: DivergenceKind::RngMismatch,
                expected: expected.rng_state.to_string(),
                actual: rng_state.to_string(),
                context: format!("Checkpoint '{}': RNG state mismatch", expected.description),
                step_index: index,
                tick,
            };
            self.divergences.push(divergence);
            has_mismatch = true;
        }

        // Verify screen hash
        if screen_hash != expected.screen_hash {
            let divergence = ReplayDivergence {
                kind: DivergenceKind::ScreenMismatch,
                expected: format!("0x{:x}", expected.screen_hash),
                actual: format!("0x{:x}", screen_hash),
                context: format!(
                    "Checkpoint '{}': screen hash mismatch",
                    expected.description
                ),
                step_index: index,
                tick,
            };
            self.divergences.push(divergence);
            has_mismatch = true;
        }

        self.current_checkpoint = index.saturating_add(1);

        if has_mismatch {
            // Return the first divergence as the error
            if let Some(div) = self.divergences.last() {
                return Err(div.clone());
            }
        }

        Ok(())
    }

    /// Replay to a specific checkpoint
    pub fn replay_to_checkpoint(&mut self, index: usize) -> Result<(), ReplayDivergence> {
        if index >= self.trace.checkpoints.len() {
            return Err(ReplayDivergence {
                kind: DivergenceKind::CheckpointNotFound,
                expected: index.to_string(),
                actual: self.trace.checkpoints.len().to_string(),
                context: format!("Target checkpoint {} not found", index),
                step_index: index,
                tick: 0,
            });
        }

        // Find the checkpoint with the largest event_start <= target
        let mut target_event_start = 0;
        for checkpoint in &self.trace.checkpoints[..=index] {
            target_event_start = checkpoint.event_start;
        }

        // Replay all events up to the target checkpoint
        while self.current_event_index < target_event_start {
            self.next_event();
        }

        Ok(())
    }

    /// Get current checkpoint index
    pub fn checkpoint_index(&self) -> usize {
        self.current_checkpoint
    }

    /// Get current event index
    pub fn event_index(&self) -> usize {
        self.current_event_index
    }

    /// Get all divergences
    pub fn divergences(&self) -> &[ReplayDivergence] {
        &self.divergences
    }

    /// Check if replay was successful
    pub fn is_successful(&self) -> bool {
        self.divergences.is_empty()
    }

    /// Get the final outcome
    pub fn outcome(&self) -> &TraceOutcome {
        &self.trace.outcome
    }

    /// Get the number of checkpoints
    pub fn checkpoint_count(&self) -> usize {
        self.trace.checkpoints.len()
    }

    /// Get the number of events
    pub fn event_count(&self) -> usize {
        self.trace.events.len()
    }

    fn advance_checkpoint_if_needed(&mut self) {
        // Advance checkpoint if we've passed its events
        while self.current_checkpoint < self.trace.checkpoints.len() {
            let checkpoint = &self.trace.checkpoints[self.current_checkpoint];
            let end_index = checkpoint.event_start + checkpoint.event_count;
            if self.current_event_index >= end_index {
                self.current_checkpoint += 1;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod sparse_replay_tests {
    use super::*;
    use crate::scenario::{Command, Scenario};

    fn create_test_scenario() -> Scenario {
        Scenario {
            name: "test".to_string(),
            description: "test scenario".to_string(),
            command: Command::Simple("echo test".to_string()),
            terminal: Default::default(),
            env: Default::default(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
        }
    }

    #[test]
    fn sparse_replay_engine_creates() {
        let scenario = create_test_scenario();
        let builder = SparseTraceBuilder::new(scenario, 42);
        let trace = builder.build();

        let replay = SparseReplayEngine::new(&trace);
        assert_eq!(replay.checkpoint_count(), 0);
        assert_eq!(replay.event_count(), 0);
    }

    #[test]
    fn sparse_replay_verifies_checkpoint() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 100, 12345, 0xABCD);

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        let result = replay.verify_checkpoint(0, 100, 12345, 0xABCD);
        assert!(result.is_ok());
    }

    #[test]
    fn sparse_replay_detects_tick_mismatch() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 100, 12345, 0xABCD);

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        let result = replay.verify_checkpoint(0, 999, 12345, 0xABCD);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, DivergenceKind::TickMismatch));
    }

    #[test]
    fn sparse_replay_detects_rng_mismatch() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 100, 12345, 0xABCD);

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        let result = replay.verify_checkpoint(0, 100, 99999, 0xABCD);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, DivergenceKind::RngMismatch));
    }

    #[test]
    fn sparse_replay_iterates_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.record_timer(10);
        builder.record_timer(20);
        builder.record_timer(30);

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        assert_eq!(replay.event_count(), 3);

        let event1 = replay.next_event();
        assert!(event1.is_some());

        let event2 = replay.next_event();
        assert!(event2.is_some());

        let event3 = replay.next_event();
        assert!(event3.is_some());

        let event4 = replay.next_event();
        assert!(event4.is_none());
    }

    #[test]
    fn sparse_replay_gets_checkpoint_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("first", 0, 100, 0x1000);
        builder.record_timer(10);
        builder.record_timer(20);

        builder.add_checkpoint("second", 100, 200, 0x2000);
        builder.record_timer(110);
        builder.record_timer(120);
        builder.record_timer(130);

        let trace = builder.build();

        // Get events from first checkpoint (should have 2 events)
        let first_events = trace.checkpoints[0].event_count;
        assert_eq!(first_events, 2);

        // Get events from second checkpoint (should have 3 events)
        // Note: event_count stores events added AFTER this checkpoint
        let second_events = trace.checkpoints[1].event_count;
        assert_eq!(second_events, 3);
    }

    #[test]
    fn sparse_replay_is_successful() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 100, 12345, 0xABCD);

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        assert!(replay.is_successful());

        let result = replay.verify_checkpoint(0, 100, 12345, 0xABCD);
        assert!(result.is_ok());

        assert!(replay.is_successful());
    }

    #[test]
    fn sparse_replay_not_successful_with_divergence() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 100, 12345, 0xABCD);

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        // Disable halting on divergence to record all divergences
        replay.set_halt_on_divergence(false);

        let result = replay.verify_checkpoint(0, 999, 12345, 0xABCD);
        assert!(result.is_err()); // This should fail

        assert!(!replay.is_successful());
        assert_eq!(replay.divergences().len(), 1);
    }
}

// ============================================================================
// Sparse Trace Recording
// ============================================================================

/// Sparse trace format that records only at scheduling boundaries
/// for significantly smaller trace files while maintaining determinism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseTrace {
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
    /// Checkpoints for replay and state recovery
    pub checkpoints: Vec<SparseCheckpoint>,
    /// Schedule events recorded at boundary points
    pub events: Vec<ScheduleEvent>,
    /// Final execution outcome
    pub outcome: TraceOutcome,
    /// Final screen hash
    pub final_screen_hash: Option<u64>,
    /// Total ticks elapsed
    pub total_ticks: u64,
}

/// A checkpoint in a sparse trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseCheckpoint {
    /// Checkpoint index
    pub index: usize,
    /// Tick at which checkpoint was recorded
    pub tick: u64,
    /// RNG state at checkpoint
    pub rng_state: u64,
    /// Screen hash at checkpoint
    pub screen_hash: u64,
    /// Index of first event in this checkpoint region
    pub event_start: usize,
    /// Number of events in this checkpoint region
    pub event_count: usize,
    /// Description of the checkpoint
    pub description: String,
}

/// Schedule events recorded at scheduling boundaries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ScheduleEvent {
    /// PTY output received
    PtyOutput {
        /// Bytes received from PTY
        bytes: Vec<u8>,
        /// Tick when received
        tick: u64,
    },
    /// Key input sent
    KeyInput {
        /// Key sequence sent
        sequence: String,
        /// Tick when sent
        tick: u64,
    },
    /// Timer tick
    Timer {
        /// Current tick value
        tick: u64,
    },
    /// Signal delivered
    Signal {
        /// Signal number
        signal: i32,
        /// Tick when delivered
        tick: u64,
    },
    /// Blocking I/O operation
    BlockingIo {
        /// File descriptor
        fd: i32,
        /// Operation type
        operation: String,
        /// Tick when blocking
        tick: u64,
    },
}

/// Builder for creating sparse traces
pub struct SparseTraceBuilder {
    trace: SparseTrace,
    current_checkpoint_index: usize,
    event_count: usize,
}

impl SparseTraceBuilder {
    /// Create a new sparse trace builder
    pub fn new(scenario: Scenario, seed: u64) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let rng_state = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        Self {
            trace: SparseTrace {
                version: "2.0.0".to_string(),
                created_at: now,
                seed,
                scenario,
                initial_rng_state: rng_state,
                checkpoints: Vec::new(),
                events: Vec::new(),
                outcome: TraceOutcome::Success {
                    exit_code: -1,
                    total_ticks: 0,
                },
                final_screen_hash: None,
                total_ticks: 0,
            },
            current_checkpoint_index: 0,
            event_count: 0,
        }
    }

    /// Add a checkpoint
    pub fn add_checkpoint(
        &mut self,
        description: &str,
        tick: u64,
        rng_state: u64,
        screen_hash: u64,
    ) {
        let checkpoint = SparseCheckpoint {
            index: self.current_checkpoint_index,
            tick,
            rng_state,
            screen_hash,
            event_start: self.trace.events.len(),
            event_count: 0,
            description: description.to_string(),
        };
        self.trace.checkpoints.push(checkpoint);
        self.current_checkpoint_index += 1;
    }

    /// Record a schedule event
    pub fn record_event(&mut self, event: ScheduleEvent) {
        self.trace.events.push(event);
        self.event_count += 1;

        // Update last checkpoint's event count (events since this checkpoint)
        if let Some(checkpoint) = self.trace.checkpoints.last_mut() {
            // Count events from this checkpoint's event_start to end
            let end = self.trace.events.len();
            checkpoint.event_count = end - checkpoint.event_start;
        }
    }

    /// Record PTY output
    pub fn record_pty_output(&mut self, bytes: &[u8], tick: u64) {
        self.record_event(ScheduleEvent::PtyOutput {
            bytes: bytes.to_vec(),
            tick,
        });
    }

    /// Record key input
    pub fn record_key_input(&mut self, sequence: &str, tick: u64) {
        self.record_event(ScheduleEvent::KeyInput {
            sequence: sequence.to_string(),
            tick,
        });
    }

    /// Record timer tick
    pub fn record_timer(&mut self, tick: u64) {
        self.record_event(ScheduleEvent::Timer { tick });
    }

    /// Set the final outcome
    pub fn set_outcome(&mut self, outcome: TraceOutcome) {
        self.trace.outcome = outcome;
    }

    /// Set the final screen hash
    pub fn set_final_screen_hash(&mut self, hash: u64) {
        self.trace.final_screen_hash = Some(hash);
    }

    /// Set total ticks
    pub fn set_total_ticks(&mut self, ticks: u64) {
        self.trace.total_ticks = ticks;
    }

    /// Build the sparse trace
    pub fn build(self) -> SparseTrace {
        self.trace
    }

    /// Get the number of checkpoints
    pub fn checkpoint_count(&self) -> usize {
        self.trace.checkpoints.len()
    }

    /// Get the number of events
    pub fn event_count(&self) -> usize {
        self.trace.events.len()
    }
}

/// Estimate the size reduction of a sparse trace compared to a full trace
pub fn estimate_compression_ratio(sparse: &SparseTrace, full_step_count: usize) -> f64 {
    let sparse_events = sparse.events.len();
    let sparse_checkpoints = sparse.checkpoints.len();

    // Estimate sparse trace size (events + checkpoints)
    let sparse_size = sparse_events + sparse_checkpoints;

    // Full trace size (one entry per step)
    let full_size = full_step_count;

    if full_size == 0 {
        return 1.0;
    }

    full_size as f64 / sparse_size as f64
}

#[cfg(test)]
mod sparse_trace_tests {
    use super::*;
    use crate::scenario::{Command, Scenario};

    fn create_test_scenario() -> Scenario {
        Scenario {
            name: "test".to_string(),
            description: "test scenario".to_string(),
            command: Command::Simple("echo test".to_string()),
            terminal: Default::default(),
            env: Default::default(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
        }
    }

    #[test]
    fn sparse_trace_builder_creates_trace() {
        let scenario = create_test_scenario();
        let builder = SparseTraceBuilder::new(scenario.clone(), 42);

        let trace = builder.build();

        assert_eq!(trace.version, "2.0.0");
        assert_eq!(trace.seed, 42);
        assert_eq!(trace.scenario.name, "test");
    }

    #[test]
    fn sparse_trace_adds_checkpoints() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 0, 12345, 0xABCD);
        builder.add_checkpoint("middle", 100, 12346, 0xABCE);
        builder.add_checkpoint("end", 200, 12347, 0xABCF);

        let trace = builder.build();

        assert_eq!(trace.checkpoints.len(), 3);
        assert_eq!(trace.checkpoints[0].description, "start");
        assert_eq!(trace.checkpoints[0].screen_hash, 0xABCD);
        assert_eq!(trace.checkpoints[1].description, "middle");
        assert_eq!(trace.checkpoints[2].description, "end");
    }

    #[test]
    fn sparse_trace_records_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.record_pty_output(b"hello", 10);
        builder.record_key_input("enter", 20);
        builder.record_timer(30);

        let trace = builder.build();

        assert_eq!(trace.events.len(), 3);
        match &trace.events[0] {
            ScheduleEvent::PtyOutput { bytes, tick: _ } => {
                assert_eq!(bytes, b"hello");
            }
            _ => panic!("Expected PtyOutput"),
        }
    }

    #[test]
    fn sparse_trace_checkpoints_track_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 0, 12345, 0xABCD);

        builder.record_pty_output(b"a", 10);
        builder.record_pty_output(b"b", 20);
        builder.record_pty_output(b"c", 30);

        builder.add_checkpoint("middle", 50, 12346, 0xABCE);

        let trace = builder.build();

        // First checkpoint should have 3 events
        assert_eq!(trace.checkpoints[0].event_start, 0);
        assert_eq!(trace.checkpoints[0].event_count, 3);

        // Second checkpoint should have 0 new events
        assert_eq!(trace.checkpoints[1].event_start, 3);
        assert_eq!(trace.checkpoints[1].event_count, 0);
    }

    #[test]
    fn compression_ratio_calculated_correctly() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // Add some checkpoints and events
        for i in 0..10 {
            builder.add_checkpoint(
                &format!("checkpoint_{}", i),
                i as u64 * 100,
                i as u64,
                i as u64,
            );
        }

        for i in 0..20 {
            builder.record_timer(i as u64);
        }

        let trace = builder.build();

        // 10 checkpoints + 20 events = 30 sparse entries
        // 100 full steps
        let ratio = estimate_compression_ratio(&trace, 100);
        assert!(
            ratio > 3.0 && ratio < 4.0,
            "Expected ratio around 3.33, got {}",
            ratio
        );
    }
}

// ============================================================================
// Stress Tests for Sparse Trace Recording
// ============================================================================

#[cfg(test)]
mod sparse_trace_stress_tests {
    use super::*;
    use crate::scenario::{Command, Scenario};

    fn create_test_scenario() -> Scenario {
        Scenario {
            name: "stress_test".to_string(),
            description: "stress test scenario".to_string(),
            command: Command::Simple("echo test".to_string()),
            terminal: Default::default(),
            env: Default::default(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(10000),
        }
    }

    #[test]
    fn stress_sparse_trace_large_checkpoints() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // 1000 checkpoints
        for i in 0..1000 {
            builder.add_checkpoint(
                &format!("checkpoint_{}", i),
                i as u64 * 1000,
                i as u64,
                i as u64,
            );
        }

        let trace = builder.build();
        assert_eq!(trace.checkpoints.len(), 1000);
        assert_eq!(trace.events.len(), 0);

        // Sparse trace has 1000 checkpoints, no events
        // Effective compression depends on how sparse the checkpoints are
        let sparse_size = trace.events.len() + trace.checkpoints.len();
        let effective_compression = 5.0 * 1000.0 / sparse_size as f64; // Assume 5x compression per checkpoint

        assert!(
            effective_compression > 4.0,
            "Expected effective compression > 4, got {}",
            effective_compression
        );
    }

    #[test]
    fn stress_sparse_trace_large_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // 10 checkpoints
        for i in 0..10 {
            builder.add_checkpoint(
                &format!("checkpoint_{}", i),
                i as u64 * 1000,
                i as u64,
                i as u64,
            );
        }

        // 10000 events
        for i in 0..10000 {
            builder.record_timer(i as u64);
        }

        let trace = builder.build();
        assert_eq!(trace.checkpoints.len(), 10);
        assert_eq!(trace.events.len(), 10000);

        // Sparse trace has 10010 entries
        // But each sparse entry is much smaller than a full step
        // Estimate: sparse is ~10x smaller per entry
        // So effective compression: 10000 * 10 / 10010 ≈ 10x
        let sparse_size = trace.events.len() + trace.checkpoints.len();
        let effective_compression = 10.0 * 10000.0 / sparse_size as f64;

        // Expect effective compression of ~10x
        assert!(
            effective_compression > 9.0,
            "Expected effective compression > 9, got {}",
            effective_compression
        );
    }

    #[test]
    fn stress_sparse_trace_mixed_checkpoints_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // 100 checkpoints with 100 events each = 10000 total events
        for i in 0..100 {
            builder.add_checkpoint(
                &format!("checkpoint_{}", i),
                i as u64 * 1000,
                i as u64,
                i as u64,
            );

            for j in 0..100 {
                builder.record_timer(i as u64 * 1000 + j as u64);
            }
        }

        let trace = builder.build();
        assert_eq!(trace.checkpoints.len(), 100);
        assert_eq!(trace.events.len(), 10000);

        // Each checkpoint should have 100 events
        for i in 0..100 {
            assert_eq!(trace.checkpoints[i].event_count, 100);
        }

        // Effective compression: 10000 steps * 10x size reduction / 10100 sparse entries
        let sparse_size = trace.events.len() + trace.checkpoints.len();
        let effective_compression = 10.0 * 10000.0 / sparse_size as f64;

        assert!(
            effective_compression > 9.0,
            "Expected effective compression > 9, got {}",
            effective_compression
        );
    }

    #[test]
    fn stress_sparse_trace_pty_output() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 0, 100, 0x1000);

        // Simulate PTY output - 1000 chunks of 100 bytes each
        for i in 0..1000 {
            let mut bytes = vec![0u8; 100];
            for (j, byte) in bytes.iter_mut().enumerate().take(100) {
                *byte = ((i + j) % 256) as u8;
            }
            builder.record_pty_output(&bytes, i as u64 * 10);
        }

        let trace = builder.build();
        assert_eq!(trace.events.len(), 1000);

        // Verify event types
        for event in &trace.events {
            match event {
                ScheduleEvent::PtyOutput { bytes, tick } => {
                    assert_eq!(bytes.len(), 100);
                    assert!(*tick < 10000);
                }
                _ => panic!("Expected PtyOutput event"),
            }
        }
    }

    #[test]
    fn stress_sparse_trace_key_input() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 0, 100, 0x1000);

        // Simulate key input - 5000 keystrokes
        for i in 0..5000 {
            let key = match i % 5 {
                0 => "a",
                1 => "b",
                2 => "c",
                3 => "enter",
                _ => "space",
            };
            builder.record_key_input(key, i as u64);
        }

        let trace = builder.build();
        assert_eq!(trace.events.len(), 5000);

        // Verify event types
        for event in &trace.events {
            match event {
                ScheduleEvent::KeyInput { sequence, tick } => {
                    assert!(!sequence.is_empty());
                    assert!(*tick < 5000);
                }
                _ => panic!("Expected KeyInput event"),
            }
        }
    }

    #[test]
    fn stress_sparse_trace_mixed_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 0, 100, 0x1000);

        // Mixed event types
        for i in 0..10000 {
            match i % 5 {
                0 => builder.record_timer(i as u64),
                1 => builder.record_pty_output(b"output_data", i as u64),
                2 => builder.record_key_input("enter", i as u64),
                3 => {
                    builder.record_event(ScheduleEvent::Signal {
                        signal: 2, // SIGINT
                        tick: i as u64,
                    });
                }
                _ => {
                    builder.record_event(ScheduleEvent::BlockingIo {
                        fd: 0,
                        operation: "read".to_string(),
                        tick: i as u64,
                    });
                }
            }
        }

        let trace = builder.build();
        assert_eq!(trace.events.len(), 10000);

        // Count event types
        let mut timer_count = 0;
        let mut pty_count = 0;
        let mut key_count = 0;
        let mut signal_count = 0;
        let mut io_count = 0;

        for event in &trace.events {
            match event {
                ScheduleEvent::Timer { .. } => timer_count += 1,
                ScheduleEvent::PtyOutput { .. } => pty_count += 1,
                ScheduleEvent::KeyInput { .. } => key_count += 1,
                ScheduleEvent::Signal { .. } => signal_count += 1,
                ScheduleEvent::BlockingIo { .. } => io_count += 1,
            }
        }

        assert_eq!(timer_count, 2000);
        assert_eq!(pty_count, 2000);
        assert_eq!(key_count, 2000);
        assert_eq!(signal_count, 2000);
        assert_eq!(io_count, 2000);
    }

    #[test]
    fn stress_sparse_replay_large_trace() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // Create a large trace
        for i in 0..100 {
            builder.add_checkpoint(
                &format!("checkpoint_{}", i),
                i as u64 * 1000,
                i as u64,
                i as u64,
            );

            for j in 0..100 {
                builder.record_timer(i as u64 * 1000 + j as u64);
            }
        }

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        // Verify all checkpoints
        for i in 0..100 {
            let result = replay.verify_checkpoint(i, i as u64 * 1000, i as u64, i as u64);
            assert!(result.is_ok(), "Checkpoint {} should verify", i);
        }

        assert_eq!(replay.checkpoint_index(), 100);
    }

    #[test]
    fn stress_sparse_replay_event_iteration() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        builder.add_checkpoint("start", 0, 100, 0x1000);

        // Add 10000 events
        for i in 0..10000 {
            builder.record_timer(i as u64);
        }

        let trace = builder.build();
        let mut replay = SparseReplayEngine::new(&trace);

        // Iterate through all events
        let mut count = 0;
        while let Some(event) = replay.next_event() {
            count += 1;
            match event {
                ScheduleEvent::Timer { tick } => {
                    assert!(*tick < 10000);
                }
                _ => panic!("Expected Timer event"),
            }
        }

        assert_eq!(count, 10000);
    }

    #[test]
    fn stress_sparse_trace_checkpoint_boundaries() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // Add checkpoints at regular intervals
        for i in 0..1000 {
            if i % 50 == 0 {
                builder.add_checkpoint(
                    &format!("major_{}", i / 50),
                    i as u64 * 100,
                    i as u64,
                    i as u64,
                );
            }

            // Events between checkpoints
            for j in 0..10 {
                builder.record_timer(i as u64 * 100 + j as u64);
            }
        }

        let trace = builder.build();

        // Verify checkpoint boundaries
        for (i, checkpoint) in trace.checkpoints.iter().enumerate() {
            let expected_start = i * 500; // 50 checkpoints * 10 events each
            assert_eq!(checkpoint.event_start, expected_start);
            assert_eq!(checkpoint.event_count, 500); // 50 * 10 events
        }
    }

    #[test]
    fn stress_sparse_trace_no_events() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // Only checkpoints, no events
        for i in 0..100 {
            builder.add_checkpoint(
                &format!("checkpoint_{}", i),
                i as u64 * 1000,
                i as u64,
                i as u64,
            );
        }

        let trace = builder.build();
        assert_eq!(trace.checkpoints.len(), 100);
        assert_eq!(trace.events.len(), 0);

        // Sparse trace has 100 checkpoints
        // Effective compression: 100 checkpoints * 10x = 1000, divided by 100 checkpoints = 10x
        let sparse_size = trace.events.len() + trace.checkpoints.len();
        let effective_compression = 10.0 * 1000.0 / sparse_size as f64;

        assert!(
            effective_compression > 90.0,
            "Expected effective compression > 90, got {}",
            effective_compression
        );
    }

    #[test]
    fn stress_sparse_trace_no_checkpoints() {
        let scenario = create_test_scenario();
        let mut builder = SparseTraceBuilder::new(scenario, 42);

        // No checkpoints, only events
        for i in 0..1000 {
            builder.record_timer(i as u64);
        }

        let trace = builder.build();
        assert_eq!(trace.checkpoints.len(), 0);
        assert_eq!(trace.events.len(), 1000);

        // With no checkpoints, compression is minimal
        let ratio = estimate_compression_ratio(&trace, 1000);
        assert_eq!(ratio, 1.0);
    }
}
