//! Resize & Timing Control
//!
//! This module provides deterministic timing and resize injection for scenario execution.
//! - Tick-based step timing using deterministic clock
//! - Resize injection that notifies app via SIGWINCH
//! - Non-deterministic scheduling error detection

use crate::determinism::{BoundaryKind, DeterministicScheduler};
use crate::process::{ProcessError, PtyProcess};

/// Error type for timing and scheduling operations
#[derive(Debug)]
pub enum TimingError {
    /// Process error during resize
    Process(ProcessError),
    /// Non-deterministic scheduling detected
    NonDeterministic(NonDeterministicError),
    /// Step execution failed
    StepFailed(String),
    /// Timeout reached
    Timeout {
        expected_ticks: u64,
        elapsed_ticks: u64,
    },
}

impl std::fmt::Display for TimingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimingError::Process(e) => write!(f, "Process error: {}", e),
            TimingError::NonDeterministic(e) => write!(f, "Non-deterministic scheduling: {}", e),
            TimingError::StepFailed(s) => write!(f, "Step failed: {}", s),
            TimingError::Timeout {
                expected_ticks,
                elapsed_ticks,
            } => {
                write!(
                    f,
                    "Timeout: expected {} ticks, elapsed {}",
                    expected_ticks, elapsed_ticks
                )
            }
        }
    }
}

impl std::error::Error for TimingError {}

impl From<ProcessError> for TimingError {
    fn from(e: ProcessError) -> Self {
        TimingError::Process(e)
    }
}

/// Non-deterministic scheduling error details
#[derive(Debug, Clone)]
pub struct NonDeterministicError {
    /// Expected state hash/value
    pub expected: u64,
    /// Actual state hash/value
    pub actual: u64,
    /// Description of where the divergence occurred
    pub context: String,
    /// Boundary ID where divergence was detected
    pub boundary_id: u64,
}

impl std::fmt::Display for NonDeterministicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "at boundary {}: {} (expected {}, got {})",
            self.boundary_id, self.context, self.expected, self.actual
        )
    }
}

/// Execution checkpoint for replay verification
#[derive(Debug, Clone)]
pub struct ExecutionCheckpoint {
    /// Tick at which this checkpoint was recorded
    pub tick: u64,
    /// Boundary ID at this checkpoint
    pub boundary_id: u64,
    /// RNG state at this checkpoint
    pub rng_state: u64,
    /// Screen state hash (if captured)
    pub screen_hash: Option<u64>,
    /// Description of the checkpoint
    pub description: String,
}

/// Timing controller for deterministic scenario execution
pub struct TimingController {
    /// Deterministic scheduler
    scheduler: DeterministicScheduler,
    /// Execution checkpoints for replay verification
    checkpoints: Vec<ExecutionCheckpoint>,
    /// Expected checkpoints for replay mode
    expected_checkpoints: Option<Vec<ExecutionCheckpoint>>,
    /// Current checkpoint index for replay verification
    checkpoint_index: usize,
    /// Whether to halt on non-deterministic behavior
    halt_on_divergence: bool,
}

impl TimingController {
    /// Create a new timing controller with the given seed
    pub fn new(seed: u64) -> Self {
        Self {
            scheduler: DeterministicScheduler::new(seed),
            checkpoints: Vec::new(),
            expected_checkpoints: None,
            checkpoint_index: 0,
            halt_on_divergence: true,
        }
    }

    /// Create a timing controller in replay mode with expected checkpoints
    pub fn with_expected_checkpoints(seed: u64, expected: Vec<ExecutionCheckpoint>) -> Self {
        Self {
            scheduler: DeterministicScheduler::new(seed),
            checkpoints: Vec::new(),
            expected_checkpoints: Some(expected),
            checkpoint_index: 0,
            halt_on_divergence: true,
        }
    }

    /// Set whether to halt on non-deterministic behavior
    pub fn set_halt_on_divergence(&mut self, halt: bool) {
        self.halt_on_divergence = halt;
    }

    /// Get current tick count
    pub fn now(&self) -> u64 {
        self.scheduler.now()
    }

    /// Get the deterministic scheduler
    pub fn scheduler(&self) -> &DeterministicScheduler {
        &self.scheduler
    }

    /// Advance time by a specific number of ticks
    pub fn wait_ticks(&mut self, ticks: u64) -> Result<(), TimingError> {
        for _ in 0..ticks {
            self.scheduler.boundary(BoundaryKind::AfterInput);
        }
        Ok(())
    }

    /// Record a checkpoint at the current execution point
    pub fn checkpoint(
        &mut self,
        description: &str,
        screen_hash: Option<u64>,
    ) -> Result<(), TimingError> {
        let checkpoint = ExecutionCheckpoint {
            tick: self.scheduler.now(),
            boundary_id: self.scheduler.current_boundary_id(),
            rng_state: self.scheduler.rng_state().unwrap_or(0),
            screen_hash,
            description: description.to_string(),
        };

        // Verify against expected if in replay mode
        if let Some(ref expected) = self.expected_checkpoints {
            if self.checkpoint_index < expected.len() {
                let expected_cp = &expected[self.checkpoint_index];

                // Check tick
                if checkpoint.tick != expected_cp.tick {
                    let err = NonDeterministicError {
                        expected: expected_cp.tick,
                        actual: checkpoint.tick,
                        context: format!("tick mismatch at checkpoint '{}'", description),
                        boundary_id: checkpoint.boundary_id,
                    };
                    if self.halt_on_divergence {
                        return Err(TimingError::NonDeterministic(err));
                    }
                }

                // Check boundary ID
                if checkpoint.boundary_id != expected_cp.boundary_id {
                    let err = NonDeterministicError {
                        expected: expected_cp.boundary_id,
                        actual: checkpoint.boundary_id,
                        context: format!("boundary ID mismatch at checkpoint '{}'", description),
                        boundary_id: checkpoint.boundary_id,
                    };
                    if self.halt_on_divergence {
                        return Err(TimingError::NonDeterministic(err));
                    }
                }

                // Check RNG state
                if checkpoint.rng_state != expected_cp.rng_state {
                    let err = NonDeterministicError {
                        expected: expected_cp.rng_state,
                        actual: checkpoint.rng_state,
                        context: format!("RNG state mismatch at checkpoint '{}'", description),
                        boundary_id: checkpoint.boundary_id,
                    };
                    if self.halt_on_divergence {
                        return Err(TimingError::NonDeterministic(err));
                    }
                }

                // Check screen hash if both are present
                if let (Some(expected_hash), Some(actual_hash)) =
                    (expected_cp.screen_hash, checkpoint.screen_hash)
                {
                    if expected_hash != actual_hash {
                        let err = NonDeterministicError {
                            expected: expected_hash,
                            actual: actual_hash,
                            context: format!(
                                "screen hash mismatch at checkpoint '{}'",
                                description
                            ),
                            boundary_id: checkpoint.boundary_id,
                        };
                        if self.halt_on_divergence {
                            return Err(TimingError::NonDeterministic(err));
                        }
                    }
                }

                self.checkpoint_index += 1;
            }
        }

        self.checkpoints.push(checkpoint);
        Ok(())
    }

    /// Get all recorded checkpoints
    pub fn checkpoints(&self) -> &[ExecutionCheckpoint] {
        &self.checkpoints
    }

    /// Reset the controller to initial state
    pub fn reset(&mut self, seed: u64) {
        self.scheduler.reset(seed);
        self.checkpoints.clear();
        self.checkpoint_index = 0;
    }
}

/// Resize controller for PTY processes
pub struct ResizeController;

impl ResizeController {
    /// Resize the PTY and send SIGWINCH to the process
    ///
    /// This is the canonical way to resize a terminal:
    /// 1. Update the PTY window size via ioctl
    /// 2. Send SIGWINCH to notify the application
    pub fn resize(process: &mut PtyProcess, cols: u16, rows: u16) -> Result<(), TimingError> {
        // Validate dimensions
        if cols == 0 || rows == 0 {
            return Err(TimingError::StepFailed(format!(
                "Invalid resize dimensions: {}x{}",
                cols, rows
            )));
        }

        // Resize the PTY - this updates the window size and sends SIGWINCH
        process.resize(cols, rows)?;

        Ok(())
    }

    /// Get the current PTY size
    pub fn current_size(process: &PtyProcess) -> (u16, u16) {
        process.pty().size()
    }
}

/// Step executor that combines timing with step execution
pub struct StepExecutor {
    /// Timing controller
    timing: TimingController,
}

impl StepExecutor {
    /// Create a new step executor
    pub fn new(seed: u64) -> Self {
        Self {
            timing: TimingController::new(seed),
        }
    }

    /// Create in replay mode with expected checkpoints
    pub fn with_checkpoints(seed: u64, checkpoints: Vec<ExecutionCheckpoint>) -> Self {
        Self {
            timing: TimingController::with_expected_checkpoints(seed, checkpoints),
        }
    }

    /// Get the timing controller
    pub fn timing(&self) -> &TimingController {
        &self.timing
    }

    /// Get mutable access to timing controller
    pub fn timing_mut(&mut self) -> &mut TimingController {
        &mut self.timing
    }

    /// Execute a wait_ticks step
    pub fn wait_ticks(&mut self, ticks: u64) -> Result<(), TimingError> {
        self.timing.wait_ticks(ticks)
    }

    /// Execute a resize step
    pub fn resize(
        &mut self,
        process: &mut PtyProcess,
        cols: u16,
        rows: u16,
    ) -> Result<(), TimingError> {
        // Mark boundary before resize
        self.timing.scheduler.boundary(BoundaryKind::BeforeInput);

        // Perform resize
        ResizeController::resize(process, cols, rows)?;

        // Mark boundary after resize
        self.timing.scheduler.boundary(BoundaryKind::AfterInput);

        Ok(())
    }

    /// Record a checkpoint
    pub fn checkpoint(
        &mut self,
        description: &str,
        screen_hash: Option<u64>,
    ) -> Result<(), TimingError> {
        self.timing.checkpoint(description, screen_hash)
    }

    /// Get current tick
    pub fn now(&self) -> u64 {
        self.timing.now()
    }

    /// Get recorded checkpoints
    pub fn checkpoints(&self) -> &[ExecutionCheckpoint] {
        self.timing.checkpoints()
    }

    /// Reset to initial state
    pub fn reset(&mut self, seed: u64) {
        self.timing.reset(seed);
    }
}

/// Verify that two execution runs are deterministic
pub fn verify_deterministic(
    checkpoints1: &[ExecutionCheckpoint],
    checkpoints2: &[ExecutionCheckpoint],
) -> Result<(), NonDeterministicError> {
    if checkpoints1.len() != checkpoints2.len() {
        return Err(NonDeterministicError {
            expected: checkpoints1.len() as u64,
            actual: checkpoints2.len() as u64,
            context: "checkpoint count mismatch".to_string(),
            boundary_id: 0,
        });
    }

    for (i, (cp1, cp2)) in checkpoints1.iter().zip(checkpoints2.iter()).enumerate() {
        if cp1.tick != cp2.tick {
            return Err(NonDeterministicError {
                expected: cp1.tick,
                actual: cp2.tick,
                context: format!("tick mismatch at checkpoint {}", i),
                boundary_id: cp1.boundary_id,
            });
        }

        if cp1.boundary_id != cp2.boundary_id {
            return Err(NonDeterministicError {
                expected: cp1.boundary_id,
                actual: cp2.boundary_id,
                context: format!("boundary ID mismatch at checkpoint {}", i),
                boundary_id: cp1.boundary_id,
            });
        }

        if cp1.rng_state != cp2.rng_state {
            return Err(NonDeterministicError {
                expected: cp1.rng_state,
                actual: cp2.rng_state,
                context: format!("RNG state mismatch at checkpoint {}", i),
                boundary_id: cp1.boundary_id,
            });
        }

        if let (Some(h1), Some(h2)) = (cp1.screen_hash, cp2.screen_hash) {
            if h1 != h2 {
                return Err(NonDeterministicError {
                    expected: h1,
                    actual: h2,
                    context: format!("screen hash mismatch at checkpoint {}", i),
                    boundary_id: cp1.boundary_id,
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessConfig;

    #[test]
    fn resize_notifies_via_sigwinch() {
        // Start a process
        let config = ProcessConfig::shell("sleep 5").with_size(80, 24);
        let mut process = PtyProcess::spawn(&config).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Verify initial size
        assert_eq!(ResizeController::current_size(&process), (80, 24));

        // Resize the terminal
        ResizeController::resize(&mut process, 120, 40).unwrap();

        // Verify new size
        assert_eq!(ResizeController::current_size(&process), (120, 40));

        // The SIGWINCH should have been sent (process.resize() does this)
        // We can't easily verify the signal was received by sleep, but we verified
        // the resize mechanism works correctly

        // Clean up
        let _ = process.signal_kill();
    }

    #[test]
    fn timing_is_deterministic() {
        // Run the same sequence twice with the same seed
        let seed = 12345u64;

        // First run
        let mut executor1 = StepExecutor::new(seed);
        executor1.wait_ticks(10).unwrap();
        executor1.checkpoint("after wait", None).unwrap();
        executor1.wait_ticks(5).unwrap();
        executor1.checkpoint("final", None).unwrap();
        let checkpoints1 = executor1.checkpoints().to_vec();

        // Second run
        let mut executor2 = StepExecutor::new(seed);
        executor2.wait_ticks(10).unwrap();
        executor2.checkpoint("after wait", None).unwrap();
        executor2.wait_ticks(5).unwrap();
        executor2.checkpoint("final", None).unwrap();
        let checkpoints2 = executor2.checkpoints().to_vec();

        // Verify they're identical
        assert_eq!(checkpoints1.len(), checkpoints2.len());
        for (cp1, cp2) in checkpoints1.iter().zip(checkpoints2.iter()) {
            assert_eq!(cp1.tick, cp2.tick);
            assert_eq!(cp1.boundary_id, cp2.boundary_id);
            assert_eq!(cp1.rng_state, cp2.rng_state);
        }

        // Use the verify function
        assert!(verify_deterministic(&checkpoints1, &checkpoints2).is_ok());
    }

    #[test]
    fn checkpoint_captures_state() {
        let mut executor = StepExecutor::new(42);

        executor.wait_ticks(5).unwrap();
        executor.checkpoint("first", Some(0xDEADBEEF)).unwrap();

        executor.wait_ticks(3).unwrap();
        executor.checkpoint("second", Some(0xCAFEBABE)).unwrap();

        let checkpoints = executor.checkpoints();
        assert_eq!(checkpoints.len(), 2);

        assert_eq!(checkpoints[0].description, "first");
        assert_eq!(checkpoints[0].screen_hash, Some(0xDEADBEEF));

        assert_eq!(checkpoints[1].description, "second");
        assert_eq!(checkpoints[1].screen_hash, Some(0xCAFEBABE));

        // Second checkpoint should have more ticks
        assert!(checkpoints[1].tick > checkpoints[0].tick);
    }

    #[test]
    fn resize_with_invalid_dimensions_fails() {
        let config = ProcessConfig::shell("sleep 1");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Zero columns should fail
        let result = ResizeController::resize(&mut process, 0, 24);
        assert!(matches!(result, Err(TimingError::StepFailed(_))));

        // Zero rows should fail
        let result = ResizeController::resize(&mut process, 80, 0);
        assert!(matches!(result, Err(TimingError::StepFailed(_))));

        let _ = process.signal_kill();
    }

    #[test]
    fn step_executor_resize_marks_boundaries() {
        let config = ProcessConfig::shell("sleep 1").with_size(80, 24);
        let mut process = PtyProcess::spawn(&config).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));

        let mut executor = StepExecutor::new(42);

        let before_tick = executor.now();
        executor.resize(&mut process, 100, 50).unwrap();
        let after_tick = executor.now();

        // Resize should advance the clock (two boundaries: before and after)
        assert!(after_tick > before_tick);
        assert_eq!(after_tick - before_tick, 2); // BeforeInput + AfterInput

        let _ = process.signal_kill();
    }

    #[test]
    fn verify_deterministic_catches_differences() {
        let cp1 = vec![ExecutionCheckpoint {
            tick: 10,
            boundary_id: 5,
            rng_state: 100,
            screen_hash: Some(0x1234),
            description: "test".to_string(),
        }];

        // Same checkpoints should pass
        assert!(verify_deterministic(&cp1, &cp1).is_ok());

        // Different tick should fail
        let cp_diff_tick = vec![ExecutionCheckpoint {
            tick: 11,
            boundary_id: 5,
            rng_state: 100,
            screen_hash: Some(0x1234),
            description: "test".to_string(),
        }];
        let result = verify_deterministic(&cp1, &cp_diff_tick);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.context.contains("tick"));

        // Different RNG state should fail
        let cp_diff_rng = vec![ExecutionCheckpoint {
            tick: 10,
            boundary_id: 5,
            rng_state: 200,
            screen_hash: Some(0x1234),
            description: "test".to_string(),
        }];
        let result = verify_deterministic(&cp1, &cp_diff_rng);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.context.contains("RNG"));

        // Different screen hash should fail
        let cp_diff_hash = vec![ExecutionCheckpoint {
            tick: 10,
            boundary_id: 5,
            rng_state: 100,
            screen_hash: Some(0x5678),
            description: "test".to_string(),
        }];
        let result = verify_deterministic(&cp1, &cp_diff_hash);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.context.contains("screen hash"));
    }

    #[test]
    fn controller_reset_works() {
        let mut executor = StepExecutor::new(42);

        executor.wait_ticks(100).unwrap();
        executor.checkpoint("mid", None).unwrap();

        assert!(executor.now() > 0);
        assert!(!executor.checkpoints().is_empty());

        // Reset
        executor.reset(999);

        assert_eq!(executor.now(), 0);
        assert!(executor.checkpoints().is_empty());
    }

    #[test]
    fn different_seeds_produce_different_execution() {
        let mut executor1 = StepExecutor::new(42);
        let mut executor2 = StepExecutor::new(43);

        executor1.wait_ticks(10).unwrap();
        executor1.checkpoint("test", None).unwrap();

        executor2.wait_ticks(10).unwrap();
        executor2.checkpoint("test", None).unwrap();

        let cp1 = &executor1.checkpoints()[0];
        let cp2 = &executor2.checkpoints()[0];

        // Ticks should be same (deterministic timing)
        assert_eq!(cp1.tick, cp2.tick);

        // RNG state should be different (different seeds)
        assert_ne!(cp1.rng_state, cp2.rng_state);
    }
}
