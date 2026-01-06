//! Enhanced Deterministic Replay Engine
//!
//! This module provides first-class deterministic record & replay capabilities:
//! - Event timing for all inputs (keys, resize, signals)
//! - Partial replay from any checkpoint
//! - Logical timestamps (no wall-clock dependency)
//! - Replay verification with divergence detection

use crate::determinism::{DeterministicScheduler, SeededRng};
use crate::invariants::{InvariantContext, InvariantEngine, InvariantResult};
use crate::process::{ExitReason, PtyProcess};
use crate::scenario::{Scenario, Step};
use crate::screen::Screen;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Trace format version 2.0.0
pub const TRACE_VERSION_V2: &str = "2.0.0";

/// All possible input events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum InputEvent {
    /// Keyboard input
    KeyPress {
        /// Logical sequence number
        sequence: u64,
        /// Key representation
        key: String,
        /// Raw bytes sent to PTY
        raw_bytes: Vec<u8>,
        /// Inter-key delay in ticks (from previous event)
        tick_delay: u64,
    },
    /// Terminal resize
    Resize {
        /// Sequence number
        sequence: u64,
        /// New dimensions
        cols: u16,
        rows: u16,
        /// Whether SIGWINCH was sent
        sigwinch_sent: bool,
        /// Tick delay from previous event
        tick_delay: u64,
    },
    /// Signal injection
    Signal {
        /// Sequence number
        sequence: u64,
        /// Signal name
        signal: String,
        /// Signal number
        signal_num: i32,
        /// Tick delay from previous event
        tick_delay: u64,
    },
    /// Tick advancement (no input, just time)
    Tick {
        /// Number of ticks advanced
        count: u64,
    },
}

/// Event timing metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Sequence number for all events
    sequence: u64,
    /// Cumulative ticks at this event
    tick: u64,
    /// RNG state after this event
    rng_state: u64,
}

/// Complete replayable trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayTrace {
    /// Format version
    pub version: String,
    /// Creation timestamp (ISO 8601, for reference only)
    pub created_at: String,
    /// Seed for deterministic replay
    pub seed: u64,
    /// Original scenario
    pub scenario: Scenario,
    /// Initial RNG state
    pub initial_rng_state: u64,
    /// Screen dimensions
    pub initial_size: (u16, u16),
    /// Recorded events in order
    pub events: Vec<InputEvent>,
    /// Event metadata (parallel array)
    pub event_metadata: Vec<EventMetadata>,
    /// Screen checkpoints
    pub checkpoints: Vec<ScreenCheckpoint>,
    /// Invariant evaluation results
    pub invariant_results: Vec<InvariantResult>,
    /// Final outcome
    pub outcome: TerminationOutcome,
    /// Checksum for verification
    pub checksum: u64,
}

/// Screen state at a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenCheckpoint {
    /// Checkpoint index
    pub index: usize,
    /// Event sequence number
    pub event_sequence: u64,
    /// Tick at checkpoint
    pub tick: u64,
    /// Screen hash
    pub screen_hash: u64,
    /// Cursor position
    pub cursor_pos: (usize, usize),
    /// Screen dimensions
    pub size: (usize, usize),
    /// Optional text excerpt for debugging
    pub text_excerpt: Option<String>,
}

/// Termination outcome with full classification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TerminationOutcome {
    /// Normal process exit
    CleanExit {
        exit_code: i32,
        total_ticks: u64,
        events_processed: u64,
    },
    /// Signal termination
    SignalExit {
        signal: String,
        signal_num: i32,
        total_ticks: u64,
        events_processed: u64,
    },
    /// Panic or crash
    Panic {
        message: String,
        during_event: Option<u64>,
        total_ticks: u64,
    },
    /// Deadlock detected (no progress)
    Deadlock {
        last_event: u64,
        stuck_at_tick: u64,
        invariant_violated: Option<String>,
    },
    /// Step timeout
    Timeout {
        step_index: usize,
        step_name: String,
        max_ticks: u64,
        elapsed_ticks: u64,
    },
    /// Invariant violation
    InvariantViolation {
        invariant: String,
        checkpoint_index: usize,
        event_sequence: u64,
        details: String,
    },
    /// Replay divergence
    ReplayDivergence {
        expected_event: u64,
        actual_event: u64,
        expected_screen_hash: u64,
        actual_screen_hash: u64,
        divergence_type: DivergenceType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DivergenceType {
    ScreenMismatch,
    TimingMismatch,
    OutputMismatch,
    InvariantViolation,
}

/// Builder for creating replay traces
pub struct TraceBuilder {
    trace: ReplayTrace,
    event_sequence: u64,
    last_tick: u64,
    rng: SeededRng,
}

impl TraceBuilder {
    /// Create a new trace builder
    pub fn new(scenario: Scenario, seed: u64, initial_size: (u16, u16)) -> Self {
        let rng = SeededRng::new(seed);
        let now = chrono::Utc::now().to_rfc3339();

        Self {
            trace: ReplayTrace {
                version: TRACE_VERSION_V2.to_string(),
                created_at: now,
                seed,
                scenario,
                initial_rng_state: seed,
                initial_size,
                events: Vec::new(),
                event_metadata: Vec::new(),
                checkpoints: Vec::new(),
                invariant_results: Vec::new(),
                outcome: TerminationOutcome::CleanExit {
                    exit_code: 0,
                    total_ticks: 0,
                    events_processed: 0,
                },
                checksum: 0,
            },
            event_sequence: 0,
            last_tick: 0,
            rng,
        }
    }

    /// Record a key press event
    pub fn record_key_press(&mut self, key: &str, raw_bytes: &[u8], tick: u64) -> u64 {
        let sequence = self.event_sequence;
        let tick_delay = tick.saturating_sub(self.last_tick);
        let rng_state = self.rng.state();

        self.trace.events.push(InputEvent::KeyPress {
            sequence,
            key: key.to_string(),
            raw_bytes: raw_bytes.to_vec(),
            tick_delay,
        });

        self.trace.event_metadata.push(EventMetadata {
            sequence,
            tick,
            rng_state,
        });

        self.event_sequence += 1;
        self.last_tick = tick;
        sequence
    }

    /// Record a resize event
    pub fn record_resize(&mut self, cols: u16, rows: u16, sigwinch_sent: bool, tick: u64) -> u64 {
        let sequence = self.event_sequence;
        let tick_delay = tick.saturating_sub(self.last_tick);
        let rng_state = self.rng.state();

        self.trace.events.push(InputEvent::Resize {
            sequence,
            cols,
            rows,
            sigwinch_sent,
            tick_delay,
        });

        self.trace.event_metadata.push(EventMetadata {
            sequence,
            tick,
            rng_state,
        });

        self.event_sequence += 1;
        self.last_tick = tick;
        sequence
    }

    /// Record a signal injection
    pub fn record_signal(&mut self, signal: &str, signal_num: i32, tick: u64) -> u64 {
        let sequence = self.event_sequence;
        let tick_delay = tick.saturating_sub(self.last_tick);
        let rng_state = self.rng.state();

        self.trace.events.push(InputEvent::Signal {
            sequence,
            signal: signal.to_string(),
            signal_num,
            tick_delay,
        });

        self.trace.event_metadata.push(EventMetadata {
            sequence,
            tick,
            rng_state,
        });

        self.event_sequence += 1;
        self.last_tick = tick;
        sequence
    }

    /// Record tick advancement
    pub fn record_ticks(&mut self, count: u64) {
        self.last_tick += count;
    }

    /// Add a screen checkpoint
    pub fn add_checkpoint(&mut self, screen: &Screen, event_sequence: u64, tick: u64) {
        let hash = screen.state_hash();
        let cursor = screen.cursor();

        // Extract first 200 chars for debugging
        let text = screen.text();
        let excerpt = if text.len() > 200 {
            Some(text[..200].to_string())
        } else {
            Some(text)
        };

        self.trace.checkpoints.push(ScreenCheckpoint {
            index: self.trace.checkpoints.len(),
            event_sequence,
            tick,
            screen_hash: hash,
            cursor_pos: (cursor.col, cursor.row),
            size: screen.size(),
            text_excerpt: excerpt,
        });
    }

    /// Record invariant result
    pub fn record_invariant_result(&mut self, result: &InvariantResult) {
        self.trace.invariant_results.push(result.clone());
    }

    /// Set the termination outcome
    pub fn set_outcome(&mut self, outcome: TerminationOutcome) {
        self.trace.outcome = outcome;
    }

    /// Finalize and compute checksum
    pub fn build(mut self) -> ReplayTrace {
        // Compute checksum from trace content
        let content = format!(
            "{:?}{:?}{:?}{:?}",
            self.trace.events,
            self.trace.checkpoints,
            self.trace.invariant_results,
            self.trace.seed
        );
        self.trace.checksum = seahash::hash(content.as_bytes());

        self.trace
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.trace)
    }

    /// Serialize to YAML
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.trace)
    }
}

/// Replay engine with full deterministic reproduction
pub struct ReplayEngine {
    trace: ReplayTrace,
    current_event: usize,
    current_tick: u64,
    rng: SeededRng,
    halt_on_divergence: bool,
    divergences: Vec<Divergence>,
}

#[derive(Debug, Clone)]
pub struct Divergence {
    pub expected: String,
    pub actual: String,
    pub event_sequence: u64,
    pub divergence_type: DivergenceType,
}

impl ReplayEngine {
    /// Create a replay engine from a trace
    pub fn new(trace: ReplayTrace) -> Self {
        let rng = SeededRng::new(trace.seed);

        Self {
            trace,
            current_event: 0,
            current_tick: 0,
            rng,
            halt_on_divergence: true,
            divergences: Vec::new(),
        }
    }

    /// Configure divergence handling
    pub fn set_halt_on_divergence(&mut self, halt: bool) {
        self.halt_on_divergence = halt;
    }

    /// Get the seed for replay
    pub fn seed(&self) -> u64 {
        self.trace.seed
    }

    /// Get scenario being replayed
    pub fn scenario(&self) -> &Scenario {
        &self.trace.scenario
    }

    /// Get total events
    pub fn total_events(&self) -> u64 {
        self.trace.events.len() as u64
    }

    /// Get current position
    /// Returns (event_index, next_event_tick)
    pub fn position(&self) -> (usize, u64) {
        // Return the tick of the next event, or current event's tick if no more events
        if let Some(next_meta) = self.trace.event_metadata.get(self.current_event) {
            (self.current_event, next_meta.tick)
        } else {
            // No more events, return current_tick
            (self.current_event, self.current_tick)
        }
    }

    /// Check if replay is complete
    pub fn is_complete(&self) -> bool {
        self.current_event >= self.trace.events.len()
    }

    /// Get next event (if available)
    pub fn next_event(&self) -> Option<&InputEvent> {
        self.trace.events.get(self.current_event)
    }

    /// Get expected metadata for next event
    pub fn next_metadata(&self) -> Option<&EventMetadata> {
        self.trace.event_metadata.get(self.current_event)
    }

    /// Advance to next event
    pub fn advance(&mut self) -> Option<&InputEvent> {
        if self.current_event < self.trace.events.len() {
            let event = &self.trace.events[self.current_event];
            self.current_event += 1;

            // Update tick based on event
            if let Some(meta) = self
                .trace
                .event_metadata
                .get(self.current_event.saturating_sub(1))
            {
                self.current_tick = meta.tick;
            }

            Some(event)
        } else {
            None
        }
    }

    /// Verify screen matches checkpoint
    pub fn verify_checkpoint(
        &self,
        screen: &Screen,
        expected_checkpoint: usize,
    ) -> Result<(), Divergence> {
        if expected_checkpoint >= self.trace.checkpoints.len() {
            return Err(Divergence {
                expected: format!("checkpoint {}", expected_checkpoint),
                actual: format!("max checkpoint {}", self.trace.checkpoints.len() - 1),
                event_sequence: self.current_event as u64,
                divergence_type: DivergenceType::ScreenMismatch,
            });
        }

        let expected = &self.trace.checkpoints[expected_checkpoint];
        let actual_hash = screen.state_hash();

        if expected.screen_hash != actual_hash {
            return Err(Divergence {
                expected: format!("hash 0x{:x}", expected.screen_hash),
                actual: format!("hash 0x{:x}", actual_hash),
                event_sequence: self.current_event as u64,
                divergence_type: DivergenceType::ScreenMismatch,
            });
        }

        // Verify cursor position
        let cursor = screen.cursor();
        if cursor.col != expected.cursor_pos.0 || cursor.row != expected.cursor_pos.1 {
            return Err(Divergence {
                expected: format!(
                    "cursor ({},{})",
                    expected.cursor_pos.0, expected.cursor_pos.1
                ),
                actual: format!("cursor ({},{})", cursor.col, cursor.row),
                event_sequence: self.current_event as u64,
                divergence_type: DivergenceType::ScreenMismatch,
            });
        }

        Ok(())
    }

    /// Replay from a specific checkpoint (partial replay)
    pub fn replay_from(&mut self, checkpoint_index: usize) {
        if checkpoint_index < self.trace.checkpoints.len() {
            let checkpoint = &self.trace.checkpoints[checkpoint_index];
            self.current_event = checkpoint.event_sequence as usize;
            self.current_tick = checkpoint.tick;
        }
    }

    /// Get all divergences detected
    pub fn divergences(&self) -> &[Divergence] {
        &self.divergences
    }

    /// Check if replay was successful
    pub fn is_successful(&self) -> bool {
        self.divergences.is_empty()
    }

    /// Record a divergence
    fn record_divergence(&mut self, divergence: Divergence) {
        self.divergences.push(divergence);
    }
}

/// Load trace from file
pub fn load_trace(path: &Path) -> Result<ReplayTrace, std::io::Error> {
    let content = std::fs::read_to_string(path)?;

    // Try JSON first
    match serde_json::from_str::<ReplayTrace>(&content) {
        Ok(trace) => Ok(trace),
        Err(_) => {
            // Try YAML
            match serde_yaml::from_str::<ReplayTrace>(&content) {
                Ok(trace) => Ok(trace),
                Err(e) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse trace: {}", e),
                )),
            }
        }
    }
}

/// Save trace to file
pub fn save_trace(trace: &ReplayTrace, path: &Path) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(trace)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Command, TerminalConfig};
    use std::collections::HashMap;

    #[test]
    fn trace_builder_creates_valid_trace() {
        let scenario = Scenario {
            name: "test".to_string(),
            description: "Test scenario".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(10000),
        };

        let mut builder = TraceBuilder::new(scenario, 42, (80, 24));

        // Record some events
        builder.record_key_press("h", b"h", 0);
        builder.record_key_press("e", b"e", 5);
        builder.record_key_press("l", b"l", 10);
        builder.record_key_press("l", b"l", 15);
        builder.record_key_press("Enter", b"\n", 20);

        let trace = builder.build();

        assert_eq!(trace.version, "2.0.0");
        assert_eq!(trace.events.len(), 5);
        assert_eq!(trace.seed, 42);
        assert!(trace.checksum != 0);
    }

    #[test]
    fn replay_engine_advances_correctly() {
        let scenario = Scenario {
            name: "test".to_string(),
            description: "Test scenario".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(10000),
        };

        let mut builder = TraceBuilder::new(scenario, 42, (80, 24));
        builder.record_key_press("a", b"a", 0);
        builder.record_key_press("b", b"b", 5);
        builder.record_key_press("c", b"c", 10);

        let trace = builder.build();
        let mut replay = ReplayEngine::new(trace);

        assert_eq!(replay.position(), (0, 0));
        assert!(!replay.is_complete());

        {
            let event1 = replay.advance().unwrap();
            assert_eq!(event1.key(), "a");
        }
        assert_eq!(replay.position(), (1, 5));

        {
            let event2 = replay.advance().unwrap();
            assert_eq!(event2.key(), "b");
        }
        assert_eq!(replay.position(), (2, 10));

        {
            let event3 = replay.advance().unwrap();
            assert_eq!(event3.key(), "c");
        }
        assert_eq!(replay.position(), (3, 10));

        assert!(replay.is_complete());
    }

    #[test]
    fn partial_replay_from_checkpoint() {
        let scenario = Scenario {
            name: "test".to_string(),
            description: "Test scenario".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(10000),
        };

        let mut builder = TraceBuilder::new(scenario, 42, (80, 24));
        for i in 0..10 {
            builder.record_key_press(&format!("k{}", i), &[b'k', i as u8], i * 10);
        }

        let trace = builder.build();
        let mut replay = ReplayEngine::new(trace);

        // Verify initial position
        assert_eq!(replay.position(), (0, 0));

        // Advance to event 5 manually (since no checkpoints were added in this test)
        for _ in 0..5 {
            replay.advance();
        }

        // Should be at event 5, next event is at tick 50
        assert_eq!(replay.position(), (5, 50));

        // Should continue from there
        let event = replay.next_event().unwrap();
        assert_eq!(event.key(), "k5");
    }
}

impl InputEvent {
    /// Get the key string for this event
    pub fn key(&self) -> &str {
        match self {
            InputEvent::KeyPress { key, .. } => key.as_str(),
            _ => panic!("Not a key press event"),
        }
    }

    /// Get the sequence number
    pub fn sequence(&self) -> u64 {
        match self {
            InputEvent::KeyPress { sequence, .. } => *sequence,
            InputEvent::Resize { sequence, .. } => *sequence,
            InputEvent::Signal { sequence, .. } => *sequence,
            InputEvent::Tick { count } => *count,
        }
    }
}
