//! Deterministic runtime guardrails
//!
//! This module provides abstractions that ensure deterministic execution:
//! - Monotonic clock that produces identical timestamps across runs
//! - Seeded RNG wrapper for reproducible randomness
//! - Explicit scheduling boundaries

use std::sync::atomic::{AtomicU64, Ordering};

/// A monotonic clock that advances deterministically.
///
/// Instead of using wall-clock time, this clock advances only when
/// explicitly ticked, ensuring identical timestamps across runs.
///
/// This struct is thread-safe using atomic operations.
#[derive(Debug)]
pub struct DeterministicClock {
    /// Current tick count (monotonically increasing) - using atomics for thread safety
    ticks: AtomicU64,
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
            ticks: AtomicU64::new(0),
            nanos_per_tick,
        }
    }

    /// Create a clock with default granularity (1ms per tick).
    pub fn default_granularity() -> Self {
        Self::new(1_000_000)
    }

    /// Get the current tick count.
    ///
    /// Uses acquire-load ordering for safe cross-thread reads.
    #[must_use]
    pub fn now(&self) -> u64 {
        self.ticks.load(Ordering::Acquire)
    }

    /// Get the current time as nanoseconds.
    #[must_use]
    pub fn now_nanos(&self) -> u64 {
        self.now().saturating_mul(self.nanos_per_tick)
    }

    /// Advance the clock by one tick.
    /// Returns true if the clock advanced, false if it was already at MAX (saturated).
    ///
    /// # Warning
    /// If this returns false, the clock is stuck at u64::MAX and will not advance.
    /// This indicates a serious error condition in a long-running test.
    pub fn tick(&self) -> bool {
        loop {
            let current = self.ticks.load(Ordering::Acquire);
            if current == u64::MAX {
                return false; // Already saturated
            }
            // Try to CAS from current to current + 1
            match self.ticks.compare_exchange_weak(
                current,
                current.saturating_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => {
                    // Another thread modified the value, retry
                    std::hint::spin_loop();
                }
            }
        }
    }

    /// Advance the clock by a specific number of ticks.
    /// Returns true if the clock advanced fully, false if it saturated at MAX.
    ///
    /// # Warning
    /// If this returns false, the clock hit u64::MAX and may not have advanced
    /// by the full requested amount. This indicates a serious error condition.
    pub fn advance(&self, ticks: u64) -> bool {
        loop {
            let current = self.ticks.load(Ordering::Acquire);
            match current.checked_add(ticks) {
                Some(new_val) => {
                    match self.ticks.compare_exchange_weak(
                        current,
                        new_val,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => return true,
                        Err(_) => {
                            std::hint::spin_loop();
                        }
                    }
                }
                None => {
                    // Would overflow - saturate at MAX using atomic max
                    // This is a best-effort; in practice we expect no overflow
                    self.ticks.store(u64::MAX, Ordering::Release);
                    return false;
                }
            }
        }
    }

    /// Check if the clock has saturated at u64::MAX.
    /// A saturated clock cannot advance further.
    #[must_use]
    pub fn is_saturated(&self) -> bool {
        self.ticks.load(Ordering::Acquire) == u64::MAX
    }

    /// Reset the clock to tick 0.
    pub fn reset(&self) {
        self.ticks.store(0, Ordering::Release);
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
    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        // xorshift64 algorithm
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Maximum iterations for rejection sampling before falling back to modulo.
    /// This prevents infinite loops for adversarial inputs while maintaining
    /// good distribution for normal cases. 64 iterations gives < 2^-64 probability
    /// of fallback for reasonable max values.
    const MAX_REJECTION_ITERATIONS: u32 = 64;

    /// Generate a u64 in the range [0, max).
    /// Uses rejection sampling to avoid modulo bias, with a fallback to modulo
    /// after MAX_REJECTION_ITERATIONS to guarantee bounded runtime.
    /// Returns 0 if max is 0.
    #[must_use]
    pub fn next_u64_max(&mut self, max: u64) -> u64 {
        if max == 0 {
            return 0;
        }
        if max == 1 {
            return 0;
        }
        // Use rejection sampling to avoid modulo bias
        // Calculate the largest multiple of max that fits in u64
        let limit = u64::MAX - (u64::MAX % max);

        // Bounded loop to prevent infinite iteration for adversarial inputs
        for _ in 0..Self::MAX_REJECTION_ITERATIONS {
            let val = self.next_u64();
            if val < limit {
                return val % max;
            }
            // Reject and retry
        }

        // Fallback: accept modulo bias after too many rejections
        // This is extremely rare (probability < 2^-64 for reasonable max values)
        // but guarantees bounded runtime
        self.next_u64() % max
    }

    /// Generate a bool with the given probability of being true.
    #[must_use]
    pub fn next_bool(&mut self, probability: f64) -> bool {
        let probability = probability.clamp(0.0, 1.0);
        let threshold = (probability * u64::MAX as f64) as u64;
        self.next_u64() < threshold
    }

    /// Generate a usize in the range [0, max).
    /// Returns 0 if max is 0.
    #[must_use]
    pub fn usize(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        self.next_u64_max(max as u64) as usize
    }

    /// Generate a usize in the range [min, max).
    /// Returns min if max <= min.
    #[must_use]
    pub fn usize_range(&mut self, min: usize, max: usize) -> usize {
        if max <= min {
            return min;
        }
        min + self.usize(max - min)
    }

    /// Get the current seed/state for replay purposes.
    #[must_use]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SchedulingBoundary {
    /// Unique identifier for this boundary
    pub id: u64,
    /// Description of what this boundary represents
    pub kind: BoundaryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
///
/// This struct uses atomic operations for thread-safe state management.
/// The scheduler ensures that the same sequence of operations produces
/// identical results across multiple runs with the same seed.
#[derive(Debug)]
pub struct DeterministicScheduler {
    /// Current boundary counter - using atomic for thread safety
    boundary_id: AtomicU64,
    /// Clock for timing
    clock: DeterministicClock,
    /// RNG for any randomized decisions - using Mutex for thread-safe access
    rng: std::sync::Mutex<SeededRng>,
}

impl DeterministicScheduler {
    /// Create a new scheduler with the given RNG seed.
    ///
    /// # Arguments
    /// * `seed` - The seed for deterministic RNG. Must be non-zero for xorshift.
    pub fn new(seed: u64) -> Self {
        Self {
            boundary_id: AtomicU64::new(0),
            clock: DeterministicClock::default(),
            rng: std::sync::Mutex::new(SeededRng::new(seed)),
        }
    }

    /// Mark a scheduling boundary and advance the clock.
    ///
    /// Returns a boundary marker that can be used for replay verification.
    pub fn boundary(&self, kind: BoundaryKind) -> SchedulingBoundary {
        let id = self.boundary_id.fetch_add(1, Ordering::AcqRel);
        self.clock.tick();
        SchedulingBoundary { id, kind }
    }

    /// Get the current clock time.
    #[must_use]
    pub fn now(&self) -> u64 {
        self.clock.now()
    }

    /// Get the current clock time in nanoseconds.
    #[must_use]
    pub fn now_nanos(&self) -> u64 {
        self.clock.now_nanos()
    }

    /// Get a random value using the deterministic RNG.
    ///
    /// This operation is thread-safe but may block if another thread
    /// holds the RNG mutex. Returns an error if the mutex is poisoned.
    pub fn random_u64(&self) -> Result<u64, String> {
        self.rng
            .lock()
            .map_err(|_| "RNG mutex poisoned".to_string())
            .map(|mut rng| rng.next_u64())
    }

    /// Get the current RNG state for replay purposes.
    ///
    /// The state can be used to reconstruct the RNG at a later point.
    /// Returns an error if the mutex is poisoned.
    pub fn rng_state(&self) -> Result<u64, String> {
        self.rng
            .lock()
            .map_err(|_| "RNG mutex poisoned".to_string())
            .map(|rng| rng.state())
    }

    /// Get the current boundary ID.
    #[must_use]
    pub fn current_boundary_id(&self) -> u64 {
        self.boundary_id.load(Ordering::Acquire)
    }

    /// Reset the scheduler to initial state with a new seed.
    ///
    /// This clears all boundaries and resets the clock.
    /// Returns an error if the mutex is poisoned.
    pub fn reset(&self, seed: u64) -> Result<(), String> {
        self.boundary_id.store(0, Ordering::Release);
        self.clock.reset();
        self.rng
            .lock()
            .map_err(|_| "RNG mutex poisoned".to_string())
            .map(|mut rng| rng.set_state(seed))
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

        assert!(clock1.tick());
        assert!(clock2.tick());
        assert_eq!(clock1.now(), clock2.now());

        assert!(clock1.advance(10));
        assert!(clock2.advance(10));
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
        assert_eq!(sched1.random_u64().unwrap(), sched2.random_u64().unwrap());
    }

    #[test]
    fn zero_seed_handled() {
        // Zero seed should not cause issues
        let rng = SeededRng::new(0);
        assert_ne!(rng.state(), 0); // Should be normalized to non-zero
    }

    #[test]
    fn clock_overflow_saturates_and_reports() {
        let clock = DeterministicClock::new(1);

        // Normal advance should succeed
        assert!(clock.advance(u64::MAX - 10));
        assert_eq!(clock.now(), u64::MAX - 10);
        assert!(!clock.is_saturated());

        // Advance that would overflow should return false and saturate
        assert!(!clock.advance(20));
        assert_eq!(clock.now(), u64::MAX);
        assert!(clock.is_saturated());

        // Further ticks should return false (clock is stuck)
        assert!(!clock.tick());
        assert_eq!(clock.now(), u64::MAX);
    }

    #[test]
    fn clock_nanos_overflow_saturates() {
        let clock = DeterministicClock::new(1_000_000_000); // 1 second per tick
        clock.advance(u64::MAX / 1_000_000_000 + 1);

        // Should saturate, not overflow
        let nanos = clock.now_nanos();
        // Value should be at or very close to u64::MAX after saturation
        assert!(nanos >= u64::MAX - 1_000_000_000);
    }

    #[test]
    fn clock_tick_returns_true_normally() {
        let clock = DeterministicClock::new(1);
        assert!(clock.tick());
        assert_eq!(clock.now(), 1);
        assert!(clock.tick());
        assert_eq!(clock.now(), 2);
    }

    #[test]
    fn clock_at_max_minus_one_saturates_correctly() {
        let clock = DeterministicClock::new(1);
        assert!(clock.advance(u64::MAX - 1));
        assert_eq!(clock.now(), u64::MAX - 1);
        assert!(!clock.is_saturated());

        // One more tick should work
        assert!(clock.tick());
        assert_eq!(clock.now(), u64::MAX);
        assert!(clock.is_saturated());

        // Now it should fail
        assert!(!clock.tick());
    }

    #[test]
    fn rng_usize_zero_returns_zero() {
        let mut rng = SeededRng::new(42);
        // Should return 0, not panic
        assert_eq!(rng.usize(0), 0);
        assert_eq!(rng.next_u64_max(0), 0);
    }

    #[test]
    fn rng_usize_one_returns_zero() {
        let mut rng = SeededRng::new(42);
        // Range [0, 1) should always be 0
        for _ in 0..100 {
            assert_eq!(rng.usize(1), 0);
            assert_eq!(rng.next_u64_max(1), 0);
        }
    }

    #[test]
    fn rng_usize_range_edge_cases() {
        let mut rng = SeededRng::new(42);

        // min == max should return min
        assert_eq!(rng.usize_range(5, 5), 5);

        // min > max should return min
        assert_eq!(rng.usize_range(10, 5), 10);
    }

    #[test]
    fn rng_bool_probability_clamped() {
        let mut rng = SeededRng::new(42);

        // Probability > 1.0 should be clamped to 1.0 (always true)
        let mut all_true = true;
        for _ in 0..100 {
            if !rng.next_bool(2.0) {
                all_true = false;
                break;
            }
        }
        assert!(all_true, "Probability 2.0 should clamp to always true");

        // Probability < 0.0 should be clamped to 0.0 (always false)
        let mut all_false = true;
        for _ in 0..100 {
            if rng.next_bool(-1.0) {
                all_false = false;
                break;
            }
        }
        assert!(all_false, "Probability -1.0 should clamp to always false");
    }

    #[test]
    fn rng_distribution_is_uniform() {
        // Simple chi-squared test for uniformity
        let mut rng = SeededRng::new(12345);
        let buckets = 10;
        let samples = 10000;
        let mut counts = vec![0usize; buckets];

        for _ in 0..samples {
            let val = rng.usize(buckets);
            counts[val] += 1;
        }

        // Expected count per bucket
        let expected = samples / buckets;

        // Check that no bucket is too far from expected (within 20%)
        for (i, &count) in counts.iter().enumerate() {
            let deviation = (count as f64 - expected as f64).abs() / expected as f64;
            assert!(
                deviation < 0.2,
                "Bucket {} has count {} (expected ~{}), deviation {:.1}%",
                i,
                count,
                expected,
                deviation * 100.0
            );
        }
    }

    #[test]
    fn rng_max_two_is_uniform() {
        // max=2 is an interesting edge case because rejection rate is near 0
        // (only values >= u64::MAX - u64::MAX%2 are rejected, which is 1 value)
        let mut rng = SeededRng::new(42);
        let mut zeros = 0;
        let mut ones = 0;
        let samples = 10000;

        for _ in 0..samples {
            match rng.next_u64_max(2) {
                0 => zeros += 1,
                1 => ones += 1,
                _ => panic!("Value out of range"),
            }
        }

        // Should be roughly 50/50
        let ratio = zeros as f64 / ones as f64;
        assert!(
            (0.8..1.25).contains(&ratio),
            "Distribution not uniform: {} zeros, {} ones, ratio {}",
            zeros,
            ones,
            ratio
        );
    }

    #[test]
    fn rng_high_rejection_max() {
        // max close to u64::MAX/2 has highest rejection rate (~50%)
        // This tests that the bounded loop prevents infinite iteration
        let mut rng = SeededRng::new(42);
        let max = u64::MAX / 2 + 1;

        // This should complete quickly (bounded by MAX_REJECTION_ITERATIONS)
        for _ in 0..100 {
            let val = rng.next_u64_max(max);
            assert!(val < max);
        }
    }

    #[test]
    fn rng_power_of_two_has_no_bias() {
        // Powers of two should have zero rejection (no modulo bias)
        // Test smaller powers where we can have enough samples for good statistics
        let mut rng = SeededRng::new(42);

        // Only test smaller powers where we can get reliable statistics
        // With 100 samples per bucket, variance is too high for large bucket counts
        for power in [2, 4, 8, 16, 256] {
            let mut counts = vec![0usize; power];
            let samples = power * 1000; // More samples for better statistics

            for _ in 0..samples {
                let val = rng.usize(power);
                counts[val] += 1;
            }

            // Check distribution is reasonable
            let expected = samples / power;
            for (i, &count) in counts.iter().enumerate() {
                let deviation = (count as f64 - expected as f64).abs() / expected as f64;
                assert!(
                    deviation < 0.15, // Stricter threshold with more samples
                    "Power {} bucket {} has count {} (expected ~{}), deviation {:.1}%",
                    power,
                    i,
                    count,
                    expected,
                    deviation * 100.0
                );
            }
        }
    }
}
