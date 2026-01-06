//! Exit Semantics & Crash Classification
//!
//! This module provides detailed termination classification for BTE test runs.
//! Instead of just exit codes, we classify the full termination semantics.

#![allow(dead_code)]

use crate::determinism::DeterministicClock;
use crate::process::{ExitReason, PtyProcess};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Comprehensive termination classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationReport {
    /// Unique report ID
    pub id: String,
    /// Classification of termination
    pub classification: TerminationClassification,
    /// Process exit information
    pub exit_info: ExitInfo,
    /// Performance metrics
    pub metrics: TerminationMetrics,
    /// Invariant violations at termination
    pub violations: Vec<InvariantViolation>,
    /// Memory and resource info
    pub resources: ResourceInfo,
    /// Whether this was a replay
    pub is_replay: bool,
    /// Original trace seed (if replay)
    pub trace_seed: Option<u64>,
}

/// High-level termination classification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TerminationClassification {
    /// Clean normal exit
    CleanExit { exit_code: i32, description: String },
    /// Process terminated by signal
    SignalExit {
        signal: String,
        signal_num: i32,
        core_dumped: bool,
        description: String,
    },
    /// Panic or unhandled exception
    Panic {
        message: String,
        panic_type: String,
        during_step: Option<String>,
        during_event: Option<u64>,
        description: String,
    },
    /// Deadlock detected (no progress)
    Deadlock {
        last_activity_tick: u64,
        stuck_duration_ticks: u64,
        no_output_ticks: u64,
        invariant_checked: Option<String>,
        description: String,
    },
    /// Step timeout
    Timeout {
        step_index: usize,
        step_type: String,
        max_ticks: u64,
        elapsed_ticks: u64,
        description: String,
    },
    /// Invariant violation
    InvariantViolation {
        invariant: String,
        checkpoint_index: usize,
        event_sequence: u64,
        details: String,
        description: String,
    },
    /// Replay divergence
    ReplayDivergence {
        trace_seed: u64,
        divergence_type: String,
        expected_value: String,
        actual_value: String,
        at_event: u64,
        description: String,
    },
    /// User interrupt
    UserInterrupt { signal: String, description: String },
    /// Unknown termination
    Unknown {
        exit_reason: String,
        description: String,
    },
}

/// Process exit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitInfo {
    /// Process ID
    pub pid: i32,
    /// Exit reason from OS
    pub os_reason: String,
    /// Exit code (if applicable)
    pub exit_code: Option<i32>,
    /// Signal number (if applicable)
    pub signal: Option<i32>,
    /// Whether core was dumped
    pub core_dumped: bool,
    /// Total runtime
    pub runtime: Duration,
    /// Steps executed
    pub steps_executed: usize,
    /// Events processed
    pub events_processed: u64,
}

/// Performance metrics at termination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationMetrics {
    /// Total ticks elapsed
    pub total_ticks: u64,
    /// Ticks per second (logical)
    pub ticks_per_second: f64,
    /// Total output bytes
    pub output_bytes: u64,
    /// Total input bytes
    pub input_bytes: u64,
    /// Screen redraw count
    pub redraw_count: u64,
    /// Peak memory (RSS in KB)
    pub peak_memory_kb: Option<u64>,
    /// Peak virtual memory (KB)
    pub peak_vm_kb: Option<u64>,
}

/// Invariant violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantViolation {
    /// Invariant name
    pub name: String,
    /// Invariant type
    pub invariant_type: String,
    /// When it was detected
    pub step: usize,
    pub tick: u64,
    /// Violation details
    pub details: String,
    /// Expected vs actual
    pub expected: Option<String>,
    pub actual: Option<String>,
}

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    /// RSS at end (KB)
    pub rss_kb: u64,
    /// Virtual memory (KB)
    pub vm_kb: u64,
    /// Open file descriptors
    pub open_fds: u64,
    /// Child processes
    pub child_count: u64,
}

/// Terminator - generates termination reports
pub struct Terminator {
    start_ticks: u64,
    clock: DeterministicClock,
    steps_executed: usize,
    events_processed: u64,
    output_bytes: u64,
    input_bytes: u64,
    redraw_count: u64,
    no_output_ticks: u64,
    peak_rss_kb: u64,
    peak_vm_kb: u64,
}

impl Terminator {
    /// Create a new terminator
    pub fn new() -> Self {
        let clock = DeterministicClock::default();
        let start_ticks = clock.now();
        Self {
            start_ticks,
            clock,
            steps_executed: 0,
            events_processed: 0,
            output_bytes: 0,
            input_bytes: 0,
            redraw_count: 0,
            no_output_ticks: 0,
            peak_rss_kb: 0,
            peak_vm_kb: 0,
        }
    }

    /// Record a step execution
    pub fn record_step(&mut self) {
        self.steps_executed += 1;
    }

    /// Record an event
    pub fn record_event(&mut self) {
        self.events_processed += 1;
    }

    /// Record output bytes
    pub fn record_output(&mut self, bytes: usize) {
        self.output_bytes += bytes as u64;
    }

    /// Record input bytes
    pub fn record_input(&mut self, bytes: usize) {
        self.input_bytes += bytes as u64;
    }

    /// Record a screen redraw
    pub fn record_redraw(&mut self) {
        self.redraw_count += 1;
    }

    /// Record no output ticks
    pub fn record_no_output(&mut self) {
        self.no_output_ticks += 1;
    }

    /// Reset no-output counter
    pub fn reset_no_output(&mut self) {
        self.no_output_ticks = 0;
    }

    /// Update memory stats
    pub fn update_memory(&mut self, rss_kb: u64, vm_kb: u64) {
        self.peak_rss_kb = self.peak_rss_kb.max(rss_kb);
        self.peak_vm_kb = self.peak_vm_kb.max(vm_kb);
    }

    /// Generate a termination report
    pub fn generate_report(
        &mut self,
        classification: TerminationClassification,
        process: &PtyProcess,
        exit_reason: Option<ExitReason>,
        is_replay: bool,
        trace_seed: Option<u64>,
        violations: &[crate::invariants::InvariantResult],
    ) -> TerminationReport {
        let elapsed_ticks = self.clock.now() - self.start_ticks;
        let runtime_ms = elapsed_ticks * 1; // 1ms per tick
        let runtime = Duration::from_millis(runtime_ms);

        let ticks_per_second = if self.no_output_ticks > 0 {
            100.0
        } else {
            100.0
        };

        let exit_info = self.create_exit_info(process, exit_reason, runtime);

        let metrics = TerminationMetrics {
            total_ticks: self.no_output_ticks * 10,
            ticks_per_second,
            output_bytes: self.output_bytes,
            input_bytes: self.input_bytes,
            redraw_count: self.redraw_count,
            peak_memory_kb: Some(self.peak_rss_kb),
            peak_vm_kb: Some(self.peak_vm_kb),
        };

        let resource_info = ResourceInfo {
            rss_kb: self.peak_rss_kb,
            vm_kb: self.peak_vm_kb,
            open_fds: 0, // Would need procfs
            child_count: 0,
        };

        let invariant_violations = violations
            .iter()
            .filter(|v| v.violation())
            .map(|v| InvariantViolation {
                name: v.name.clone(),
                invariant_type: v.description.clone(),
                step: v.step,
                tick: v.tick,
                details: v.details.clone().unwrap_or_default(),
                expected: None,
                actual: None,
            })
            .collect();

        TerminationReport {
            id: format!("term_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            classification,
            exit_info,
            metrics,
            violations: invariant_violations,
            resources: resource_info,
            is_replay,
            trace_seed,
        }
    }

    fn create_exit_info(
        &self,
        process: &PtyProcess,
        exit_reason: Option<ExitReason>,
        runtime: Duration,
    ) -> ExitInfo {
        let (os_reason, exit_code, signal, core_dumped) = match exit_reason {
            Some(ExitReason::Exited(code)) => ("exited".to_string(), Some(code), None, false),
            Some(ExitReason::Signaled(sig)) => {
                let sig_name = match sig {
                    1 => "SIGHUP",
                    2 => "SIGINT",
                    3 => "SIGQUIT",
                    6 => "SIGABRT",
                    9 => "SIGKILL",
                    11 => "SIGSEGV",
                    13 => "SIGPIPE",
                    15 => "SIGTERM",
                    _ => "SIGUNKNOWN",
                };
                (
                    format!("signaled ({})", sig_name),
                    None,
                    Some(sig),
                    sig == 11,
                )
            }
            Some(ExitReason::Running) | None => ("unknown".to_string(), None, None, false),
        };

        ExitInfo {
            pid: process.pid_raw(),
            os_reason,
            exit_code,
            signal,
            core_dumped,
            runtime,
            steps_executed: self.steps_executed,
            events_processed: self.events_processed,
        }
    }
}

impl Default for Terminator {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a classification from exit reason
pub fn classify_termination(
    exit_reason: Option<ExitReason>,
    _exit_code: i32,
    last_activity_tick: u64,
    no_output_ticks: u64,
    max_ticks: u64,
    step_index: usize,
    step_name: &str,
    violations: &[crate::invariants::InvariantResult],
    is_timeout: bool,
) -> TerminationClassification {
    // Check for timeout first
    if is_timeout {
        return TerminationClassification::Timeout {
            step_index,
            step_type: step_name.to_string(),
            max_ticks,
            elapsed_ticks: last_activity_tick + no_output_ticks * 10,
            description: format!(
                "Step {} ({}) timed out after {} ticks",
                step_index, step_name, max_ticks
            ),
        };
    }

    // Check for deadlock
    if no_output_ticks > 1000 {
        return TerminationClassification::Deadlock {
            last_activity_tick,
            stuck_duration_ticks: no_output_ticks * 10,
            no_output_ticks,
            invariant_checked: None,
            description: format!(
                "No output for {} ticks (deadlock suspected)",
                no_output_ticks * 10
            ),
        };
    }

    // Check for invariant violations
    let first_violation = violations.iter().find(|v| v.violation());
    if let Some(v) = first_violation {
        return TerminationClassification::InvariantViolation {
            invariant: v.name.clone(),
            checkpoint_index: v.step,
            event_sequence: v.tick,
            details: v.details.clone().unwrap_or_default(),
            description: format!("Invariant '{}' violated: {}", v.name, v.description),
        };
    }

    // Classify by exit reason
    match exit_reason {
        Some(ExitReason::Exited(code)) => {
            if code == 0 {
                TerminationClassification::CleanExit {
                    exit_code: 0,
                    description: "Process exited successfully".to_string(),
                }
            } else {
                TerminationClassification::CleanExit {
                    exit_code: code,
                    description: format!("Process exited with code {}", code),
                }
            }
        }
        Some(ExitReason::Signaled(sig)) => {
            let sig_name = match sig {
                1 => "SIGHUP (hangup)",
                2 => "SIGINT (interrupt)",
                3 => "SIGQUIT (quit)",
                6 => "SIGABRT (abort)",
                9 => "SIGKILL (kill)",
                11 => "SIGSEGV (segmentation fault)",
                13 => "SIGPIPE (broken pipe)",
                15 => "SIGTERM (termination)",
                _ => &format!("Signal {}", sig),
            };
            TerminationClassification::SignalExit {
                signal: sig_name.to_string(),
                signal_num: sig,
                core_dumped: sig == 11,
                description: format!("Process killed by {}", sig_name),
            }
        }
        Some(ExitReason::Running) | None => TerminationClassification::Unknown {
            exit_reason: "unknown".to_string(),
            description: "Process termination reason unknown".to_string(),
        },
    }
}

/// Exit code mapping for CLI
pub fn exit_code_from_classification(classification: &TerminationClassification) -> i32 {
    match classification {
        TerminationClassification::CleanExit { exit_code, .. } => *exit_code,
        TerminationClassification::SignalExit { .. } => -1,
        TerminationClassification::Panic { .. } => -99,
        TerminationClassification::Deadlock { .. } => -98,
        TerminationClassification::Timeout { .. } => -97,
        TerminationClassification::InvariantViolation { .. } => -96,
        TerminationClassification::ReplayDivergence { .. } => -95,
        TerminationClassification::UserInterrupt { .. } => -130, // Same as SIGINT
        TerminationClassification::Unknown { .. } => -4,
    }
}

/// Machine-readable summary for CI
#[derive(Debug, Serialize)]
pub struct CISummary {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub success_rate: f64,
    pub duration_ms: u64,
    pub has_failures: bool,
}

impl CISummary {
    pub fn new(
        total: usize,
        passed: usize,
        failed: usize,
        skipped: usize,
        duration_ms: u64,
    ) -> Self {
        let success_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Self {
            total_tests: total,
            passed,
            failed,
            skipped,
            success_rate,
            duration_ms,
            has_failures: failed > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminator_records_metrics() {
        let mut terminator = Terminator::new();
        terminator.record_step();
        terminator.record_event();
        terminator.record_output(100);
        terminator.record_input(10);
        terminator.record_redraw();
        terminator.record_no_output();
        terminator.update_memory(1024, 5120);

        assert_eq!(terminator.steps_executed, 1);
        assert_eq!(terminator.events_processed, 1);
        assert_eq!(terminator.output_bytes, 100);
        assert_eq!(terminator.input_bytes, 10);
        assert_eq!(terminator.redraw_count, 1);
        assert_eq!(terminator.peak_rss_kb, 1024);
        assert_eq!(terminator.peak_vm_kb, 5120);
    }

    #[test]
    fn exit_code_mapping() {
        let clean = TerminationClassification::CleanExit {
            exit_code: 0,
            description: "ok".to_string(),
        };
        assert_eq!(exit_code_from_classification(&clean), 0);

        let signal = TerminationClassification::SignalExit {
            signal: "SIGINT".to_string(),
            signal_num: 2,
            core_dumped: false,
            description: "killed".to_string(),
        };
        assert_eq!(exit_code_from_classification(&signal), -1);

        let panic = TerminationClassification::Panic {
            message: "test".to_string(),
            panic_type: "panic".to_string(),
            during_step: None,
            during_event: None,
            description: "crashed".to_string(),
        };
        assert_eq!(exit_code_from_classification(&panic), -99);
    }

    #[test]
    fn ci_summary_format() {
        let summary = CISummary::new(100, 90, 5, 5, 5000);
        assert_eq!(summary.total_tests, 100);
        assert_eq!(summary.passed, 90);
        assert_eq!(summary.failed, 5);
        assert_eq!(summary.success_rate, 90.0);
        assert!(summary.has_failures);
    }
}
