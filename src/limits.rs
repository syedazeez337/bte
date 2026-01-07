//! Resource limits and constraints for BTE
//!
//! This module provides resource management to prevent:
//! - Disk exhaustion from trace files
//! - Memory exhaustion from large screens
//! - Process proliferation

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_trace_bytes: usize,
    pub max_screen_bytes: usize,
    pub max_output_buffer_bytes: usize,
    pub max_concurrent_processes: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_trace_bytes: 100 * 1024 * 1024,   // 100 MB
            max_screen_bytes: 10 * 1024 * 1024,   // 10 MB
            max_output_buffer_bytes: 1024 * 1024, // 1 MB
            max_concurrent_processes: 4,
        }
    }
}

impl ResourceLimits {
    /// Create custom limits
    pub fn new(
        max_trace_mb: usize,
        max_screen_mb: usize,
        max_buffer_kb: usize,
        max_processes: usize,
    ) -> Self {
        Self {
            max_trace_bytes: max_trace_mb * 1024 * 1024,
            max_screen_bytes: max_screen_mb * 1024 * 1024,
            max_output_buffer_bytes: max_buffer_kb * 1024,
            max_concurrent_processes: max_processes,
        }
    }

    /// Strict limits for CI
    pub fn strict() -> Self {
        Self::new(10, 1, 256, 2)
    }

    /// Lenient limits for development
    pub fn lenient() -> Self {
        Self::new(500, 50, 4096, 8)
    }
}

#[derive(Debug)]
pub struct ResourceTracker {
    current_trace_bytes: AtomicUsize,
    current_output_bytes: AtomicUsize,
    active_processes: AtomicUsize,
    limits: ResourceLimits,
}

impl ResourceTracker {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            current_trace_bytes: AtomicUsize::new(0),
            current_output_bytes: AtomicUsize::new(0),
            active_processes: AtomicUsize::new(0),
            limits,
        }
    }

    pub fn default() -> Self {
        Self::new(ResourceLimits::default())
    }

    pub fn can_start_process(&self) -> bool {
        self.active_processes.load(Ordering::Relaxed) < self.limits.max_concurrent_processes
    }

    pub fn start_process(&self) {
        self.active_processes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn end_process(&self) {
        self.active_processes.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn can_add_trace_data(&self, bytes: usize) -> bool {
        self.current_trace_bytes.load(Ordering::Relaxed) + bytes <= self.limits.max_trace_bytes
    }

    pub fn add_trace_data(&self, bytes: usize) -> Result<(), ResourceLimitError> {
        let current = self.current_trace_bytes.load(Ordering::Relaxed);
        if current + bytes > self.limits.max_trace_bytes {
            return Err(ResourceLimitError::TraceSizeExceeded {
                current,
                limit: self.limits.max_trace_bytes,
            });
        }
        self.current_trace_bytes
            .store(current + bytes, Ordering::Relaxed);
        Ok(())
    }

    pub fn can_reserve_screen(&self, bytes: usize) -> bool {
        bytes <= self.limits.max_screen_bytes
    }

    pub fn reserve_screen(&self, bytes: usize) -> Result<(), ResourceLimitError> {
        if bytes > self.limits.max_screen_bytes {
            return Err(ResourceLimitError::ScreenSizeExceeded {
                requested: bytes,
                limit: self.limits.max_screen_bytes,
            });
        }
        Ok(())
    }

    pub fn can_buffer_output(&self, bytes: usize) -> bool {
        self.current_output_bytes.load(Ordering::Relaxed) + bytes
            <= self.limits.max_output_buffer_bytes
    }

    pub fn add_output(&self, bytes: usize) -> Result<(), ResourceLimitError> {
        let current = self.current_output_bytes.load(Ordering::Relaxed);
        if current + bytes > self.limits.max_output_buffer_bytes {
            return Err(ResourceLimitError::OutputBufferExceeded {
                current,
                limit: self.limits.max_output_buffer_bytes,
            });
        }
        self.current_output_bytes
            .store(current + bytes, Ordering::Relaxed);
        Ok(())
    }

    pub fn reset_output(&self) {
        self.current_output_bytes.store(0, Ordering::Relaxed);
    }

    pub fn current_usage(&self) -> ResourceUsage {
        ResourceUsage {
            trace_bytes: self.current_trace_bytes.load(Ordering::Relaxed),
            output_bytes: self.current_output_bytes.load(Ordering::Relaxed),
            active_processes: self.active_processes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub trace_bytes: usize,
    pub output_bytes: usize,
    pub active_processes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceLimitError {
    #[error("Trace size exceeded: current={current}, limit={limit}")]
    TraceSizeExceeded { current: usize, limit: usize },
    #[error("Screen size exceeded: requested={requested}, limit={limit}")]
    ScreenSizeExceeded { requested: usize, limit: usize },
    #[error("Output buffer exceeded: current={current}, limit={limit}")]
    OutputBufferExceeded { current: usize, limit: usize },
    #[error("Too many concurrent processes")]
    TooManyProcesses,
}

pub struct ResourceGuard<'a> {
    tracker: &'a ResourceTracker,
    kind: ResourceGuardKind,
}

enum ResourceGuardKind {
    Process,
    Trace(usize),
    Output(usize),
}

impl<'a> ResourceGuard<'a> {
    pub fn start_process(tracker: &'a ResourceTracker) -> Result<Self, ResourceLimitError> {
        if !tracker.can_start_process() {
            return Err(ResourceLimitError::TooManyProcesses);
        }
        tracker.start_process();
        Ok(Self {
            tracker,
            kind: ResourceGuardKind::Process,
        })
    }

    pub fn trace_data(
        tracker: &'a ResourceTracker,
        bytes: usize,
    ) -> Result<Self, ResourceLimitError> {
        tracker.add_trace_data(bytes)?;
        Ok(Self {
            tracker,
            kind: ResourceGuardKind::Trace(bytes),
        })
    }

    pub fn output(tracker: &'a ResourceTracker, bytes: usize) -> Result<Self, ResourceLimitError> {
        tracker.add_output(bytes)?;
        Ok(Self {
            tracker,
            kind: ResourceGuardKind::Output(bytes),
        })
    }
}

impl<'a> Drop for ResourceGuard<'a> {
    fn drop(&mut self) {
        match self.kind {
            ResourceGuardKind::Process => {
                self.tracker.end_process();
            }
            ResourceGuardKind::Trace(bytes) => {
                self.tracker
                    .current_trace_bytes
                    .fetch_sub(bytes, Ordering::Relaxed);
            }
            ResourceGuardKind::Output(bytes) => {
                self.tracker
                    .current_output_bytes
                    .fetch_sub(bytes, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_trace_bytes, 100 * 1024 * 1024);
        assert_eq!(limits.max_screen_bytes, 10 * 1024 * 1024);
        assert_eq!(limits.max_output_buffer_bytes, 1024 * 1024);
        assert_eq!(limits.max_concurrent_processes, 4);
    }

    #[test]
    fn test_resource_tracker_process() {
        let tracker = ResourceTracker::default();
        assert!(tracker.can_start_process());
        assert!(tracker.can_start_process());
        assert!(tracker.can_start_process());
        assert!(tracker.can_start_process());

        let _guard = ResourceGuard::start_process(&tracker).unwrap();
        assert_eq!(tracker.active_processes.load(Ordering::Relaxed), 1);

        drop(_guard);
        assert_eq!(tracker.active_processes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_resource_tracker_trace() {
        let limits = ResourceLimits::new(1, 1, 1, 1); // 1MB limits
        let tracker = ResourceTracker::new(limits);

        assert!(tracker.can_add_trace_data(500 * 1024));
        let _guard = ResourceGuard::trace_data(&tracker, 500 * 1024).unwrap();
        assert!(!tracker.can_add_trace_data(600 * 1024));
    }

    #[test]
    fn test_resource_guard_drop() {
        let tracker = ResourceTracker::default();
        {
            let _guard = ResourceGuard::start_process(&tracker).unwrap();
            assert_eq!(tracker.active_processes.load(Ordering::Relaxed), 1);
        }
        assert_eq!(tracker.active_processes.load(Ordering::Relaxed), 0);
    }
}
