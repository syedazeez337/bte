//! Deterministic runtime guardrails
//!
//! This module provides abstractions that ensure deterministic execution:
//! - Monotonic clock that produces identical timestamps across runs
//! - Seeded RNG wrapper for reproducible randomness
//! - Explicit scheduling boundaries

use std::cell::Cell;

/// A monotonic clock that advances deterministically.
///
/// Instead of using wall-clock time, this clock advances only when
/// explicitly ticked, ensuring identical timestamps across runs.
#[derive(Debug)]
pub struct DeterministicClock {
    /// Current tick count (monotonically increasing)
    ticks: Cell<u64>,
    /// Nanoseconds per tick (for conversion to duration-like values)
    nanos_per_tick: u64,
}

impl DeterministicClock {
    /// Create a new deterministic clock starting at tick 0.
    ///
    /// `nanos_per_tick` controls the granularity of time representation.
    /// Default is 1_000_000 (1ms per tick).
    pub fn new(nanos_per_tick: u64) -> Self {
        Self {
            ticks: Cell::new(0),
            nanos_per_tick,
        }
    }

    /// Create a clock with default granularity (1ms per tick).
    pub fn default_granularity() -> Self {
        Self::new(1_000_000)
    }

    /// Get the current tick count.
    pub fn now(&self) -> u64 {
        self.ticks.get()
    }

    /// Get the current time as nanoseconds.
    pub fn now_nanos(&self) -> u64 {
        self.ticks.get() * self.nanos_per_tick
    }

    /// Advance the clock by one tick.
    pub fn tick(&self) {
        self.ticks.set(self.ticks.get() + 1);
    }

    /// Advance the clock by a specific number of ticks.
    pub fn advance(&self, ticks: u64) {
        self.ticks.set(self.ticks.get() + ticks);
    }

    /// Reset the clock to tick 0.
    pub fn reset(&self) {
        self.ticks.set(0);
    }
}

impl Default for DeterministicClock {
    fn default() -> Self {
        Self::default_granularity()
    }
}

/// A seeded random number generator for deterministic randomness.
///
/// Uses a simple xorshift64 algorithm for reproducibility across platforms.
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    /// Create a new RNG with the given seed.
    ///
    /// The same seed always produces the same sequence.
    pub fn new(seed: u64) -> Self {
        // Ensure non-zero state (xorshift requires this)
        let state = if seed == 0 { 1 } else { seed };
        Self { state }
    }

    /// Generate the next u64 value.
    pub fn next_u64(&mut self) -> u64 {
        // xorshift64 algorithm
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Generate a u64 in the range [0, max).
    pub fn next_u64_max(&mut self, max: u64) -> u64 {
        if max == 0 {
            return 0;
        }
        self.next_u64() % max
    }

    /// Generate a bool with the given probability of being true.
    pub fn next_bool(&mut self, probability: f64) -> bool {
        let threshold = (probability * u64::MAX as f64) as u64;
        self.next_u64() < threshold
    }

    /// Get the current seed/state for replay purposes.
    pub fn state(&self) -> u64 {
        self.state
    }

    /// Reset to a specific state (for replay).
    pub fn set_state(&mut self, state: u64) {
        self.state = if state == 0 { 1 } else { state };
    }
}

/// Scheduling boundary marker.
///
/// This is used to explicitly mark points in execution where
/// scheduling decisions are made, ensuring deterministic interleaving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulingBoundary {
    /// Unique identifier for this boundary
    pub id: u64,
    /// Description of what this boundary represents
    pub kind: BoundaryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    /// Before reading from PTY
    BeforePtyRead,
    /// After reading from PTY
    AfterPtyRead,
    /// Before writing to PTY
    BeforePtyWrite,
    /// After writing to PTY
    AfterPtyWrite,
    /// Before processing input
    BeforeInput,
    /// After processing input
    AfterInput,
    /// Before checking invariants
    BeforeInvariantCheck,
    /// After checking invariants
    AfterInvariantCheck,
}

/// A scheduler that tracks execution boundaries for deterministic replay.
#[derive(Debug)]
pub struct DeterministicScheduler {
    /// Current boundary counter
    boundary_id: Cell<u64>,
    /// Clock for timing
    clock: DeterministicClock,
    /// RNG for any randomized decisions
    rng: std::cell::RefCell<SeededRng>,
}

impl DeterministicScheduler {
    /// Create a new scheduler with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        Self {
            boundary_id: Cell::new(0),
            clock: DeterministicClock::default(),
            rng: std::cell::RefCell::new(SeededRng::new(seed)),
        }
    }

    /// Mark a scheduling boundary and advance the clock.
    pub fn boundary(&self, kind: BoundaryKind) -> SchedulingBoundary {
        let id = self.boundary_id.get();
        self.boundary_id.set(id + 1);
        self.clock.tick();
        SchedulingBoundary { id, kind }
    }

    /// Get the current clock time.
    pub fn now(&self) -> u64 {
        self.clock.now()
    }

    /// Get the current clock time in nanoseconds.
    pub fn now_nanos(&self) -> u64 {
        self.clock.now_nanos()
    }

    /// Get a random value using the deterministic RNG.
    pub fn random_u64(&self) -> u64 {
        self.rng.borrow_mut().next_u64()
    }

    /// Get the current RNG state for replay.
    pub fn rng_state(&self) -> u64 {
        self.rng.borrow().state()
    }

    /// Get the current boundary ID.
    pub fn current_boundary_id(&self) -> u64 {
        self.boundary_id.get()
    }

    /// Reset the scheduler to initial state with a new seed.
    pub fn reset(&self, seed: u64) {
        self.boundary_id.set(0);
        self.clock.reset();
        self.rng.borrow_mut().set_state(seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_produces_identical_timestamps() {
        let clock1 = DeterministicClock::default();
        let clock2 = DeterministicClock::default();

        // Same operations produce same timestamps
        assert_eq!(clock1.now(), clock2.now());

        clock1.tick();
        clock2.tick();
        assert_eq!(clock1.now(), clock2.now());

        clock1.advance(10);
        clock2.advance(10);
        assert_eq!(clock1.now(), clock2.now());
    }

    #[test]
    fn rng_produces_identical_sequences() {
        let mut rng1 = SeededRng::new(42);
        let mut rng2 = SeededRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn rng_different_seeds_produce_different_sequences() {
        let mut rng1 = SeededRng::new(42);
        let mut rng2 = SeededRng::new(43);

        // Very unlikely to match
        assert_ne!(rng1.next_u64(), rng2.next_u64());
    }

    #[test]
    fn scheduler_is_deterministic() {
        let sched1 = DeterministicScheduler::new(42);
        let sched2 = DeterministicScheduler::new(42);

        // Same operations produce same state
        sched1.boundary(BoundaryKind::BeforePtyRead);
        sched2.boundary(BoundaryKind::BeforePtyRead);
        assert_eq!(sched1.now(), sched2.now());
        assert_eq!(sched1.current_boundary_id(), sched2.current_boundary_id());
        assert_eq!(sched1.random_u64(), sched2.random_u64());
    }

    #[test]
    fn zero_seed_handled() {
        // Zero seed should not cause issues
        let rng = SeededRng::new(0);
        assert_ne!(rng.state(), 0); // Should be normalized to non-zero
    }
}
