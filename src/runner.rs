//! Scenario Runner
//!
//! This module provides the execution engine for running scenarios
//! and generating traces.

#![allow(dead_code)]

use crate::determinism::DeterministicScheduler;
use crate::invariants::{BuiltInInvariant, InvariantEngine};
use crate::io_loop::IoLoop;
use crate::keys::KeyInjector;
use crate::process::{ProcessConfig, PtyProcess};
use crate::scenario::{Scenario, Step};
use crate::screen::Screen;
use crate::timing::TimingController;
use crate::trace::{Trace, TraceBuilder, TraceOutcome};
use regex::Regex;
use std::path::Path;

pub struct RunResult {
    pub trace: Trace,
    pub exit_code: i32,
    pub success: bool,
}

impl RunResult {
    pub fn new(trace: Trace, exit_code: i32, success: bool) -> Self {
        Self {
            trace,
            exit_code,
            success,
        }
    }
}

#[derive(Debug)]
pub struct RunnerConfig {
    pub trace_path: Option<String>,
    pub verbose: bool,
    pub max_ticks: u64,
    pub tick_delay_ms: u64,
    pub seed: Option<u64>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            trace_path: None,
            verbose: false,
            max_ticks: 10000,
            tick_delay_ms: 0,
            seed: None,
        }
    }
}

pub fn run_scenario(scenario: &Scenario, config: &RunnerConfig) -> RunResult {
    let seed = config.seed.or(scenario.seed).unwrap_or_else(|| {
        let mut rng = fastrand::Rng::new();
        rng.u64(..)
    });

    let mut scheduler = DeterministicScheduler::new(seed);
    let mut timing = TimingController::new(seed);

    let pty_config = crate::pty::PtyConfig {
        size: (scenario.terminal.cols, scenario.terminal.rows),
        raw_mode: true,
        non_blocking: true,
    };

    let env = if scenario.env.is_empty() {
        None
    } else {
        Some(scenario.env.clone())
    };

    let proc_config = ProcessConfig {
        program: scenario.command.program().to_string(),
        args: scenario.command.args(),
        env,
        cwd: None,
        pty_config,
    };

    let mut process = PtyProcess::spawn(&proc_config).expect("Failed to spawn process");
    let mut io = IoLoop::new();
    let mut screen = Screen::new(
        scenario.terminal.cols as usize,
        scenario.terminal.rows as usize,
    );

    let mut invariant_engine = InvariantEngine::new();
    if !scenario.invariants.is_empty() {
        let builtins: Vec<BuiltInInvariant> = scenario
            .invariants
            .iter()
            .map(|inv| match inv {
                crate::scenario::InvariantRef::CursorBounds => BuiltInInvariant::CursorBounds,
                crate::scenario::InvariantRef::NoDeadlock { timeout_ms } => {
                    let ticks = timeout_ms.unwrap_or(1000) / 10;
                    BuiltInInvariant::NoDeadlock {
                        timeout_ticks: ticks.max(10),
                    }
                }
                crate::scenario::InvariantRef::SignalHandled { signal } => {
                    BuiltInInvariant::SignalHandled {
                        signal: format!("{:?}", signal).to_uppercase(),
                    }
                }
                crate::scenario::InvariantRef::ScreenContains { pattern } => {
                    BuiltInInvariant::ScreenContains {
                        pattern: pattern.clone(),
                    }
                }
                crate::scenario::InvariantRef::ScreenNotContains { pattern } => {
                    BuiltInInvariant::ScreenNotContains {
                        pattern: pattern.clone(),
                    }
                }
                crate::scenario::InvariantRef::NoOutputAfterExit => {
                    BuiltInInvariant::NoOutputAfterExit
                }
                crate::scenario::InvariantRef::ProcessTerminatedCleanly { allowed_signals } => {
                    BuiltInInvariant::ProcessTerminatedCleanly {
                        allowed_signals: allowed_signals.clone(),
                    }
                }
                crate::scenario::InvariantRef::ScreenStability { min_ticks } => {
                    BuiltInInvariant::ScreenStability {
                        min_ticks: *min_ticks,
                    }
                }
                crate::scenario::InvariantRef::ViewportValid => BuiltInInvariant::ViewportValid,
                crate::scenario::InvariantRef::ResponseTime { max_ticks } => {
                    BuiltInInvariant::ResponseTime {
                        max_ticks: *max_ticks,
                    }
                }
                crate::scenario::InvariantRef::MaxLatency { max_ticks } => {
                    BuiltInInvariant::MaxLatency {
                        max_ticks: *max_ticks,
                    }
                }
                crate::scenario::InvariantRef::Custom { name: _ } => {
                    BuiltInInvariant::NoDeadlock { timeout_ticks: 100 }
                }
            })
            .collect();
        invariant_engine.add_builtin_invariants(&builtins);
    }

    let mut trace_builder = TraceBuilder::new(scenario.clone(), seed);
    trace_builder.set_initial_rng_state(scheduler.rng_state());

    let mut last_screen_hash = None;
    let mut no_output_ticks = 0u64;
    let mut step_index = 0usize;
    let mut timed_out = false;
    let mut error_message = None;

    trace_builder.add_checkpoint("initial", &scheduler, Some(&screen));

    for step in &scenario.steps {
        if scheduler.now() > config.max_ticks {
            timed_out = true;
            break;
        }

        let mut ctx = crate::invariants::InvariantContext {
            screen: Some(&screen),
            process: &mut process,
            step: step_index,
            tick: scheduler.now(),
            _is_replay: false,
            last_screen_hash,
            no_output_ticks,
            expected_signal: None,
        };

        let results = invariant_engine.evaluate(&mut ctx);
        for result in results {
            trace_builder.record_invariant_result(result);
            if result.violation() {
                trace_builder.record_invariant_violation(&result.name);
            }
        }

        trace_builder.start_step(step.clone(), Some(&screen), &scheduler);

        let step_result = execute_step(
            step,
            &mut process,
            &mut io,
            &mut screen,
            &mut scheduler,
            &mut timing,
            config,
        );

        match step_result {
            StepResult::Ok => {
                let _ = io.read_available(&process);
                let output = io.take_output();
                trace_builder.record_pty_output(&output);
            }
            StepResult::Output(output) => {
                trace_builder.record_pty_output(&output);
            }
            StepResult::Error(e) => {
                error_message = Some(e.clone());
                trace_builder.record_error(&e);
            }
        }

        let current_hash = screen.state_hash();
        if Some(current_hash) == last_screen_hash {
            no_output_ticks += 1;
        } else {
            no_output_ticks = 0;
        }
        last_screen_hash = Some(current_hash);

        trace_builder.end_step(Some(&screen), &scheduler);
        trace_builder.add_checkpoint(
            &format!("after_step_{}", step_index),
            &scheduler,
            Some(&screen),
        );

        step_index += 1;

        if !invariant_engine.all_satisfied() {
            break;
        }
    }

    let mut ctx = crate::invariants::InvariantContext {
        screen: Some(&screen),
        process: &mut process,
        step: step_index,
        tick: scheduler.now(),
        _is_replay: false,
        last_screen_hash,
        no_output_ticks,
        expected_signal: None,
    };
    let results = invariant_engine.evaluate(&mut ctx);
    for result in results {
        trace_builder.record_invariant_result(result);
    }

    let outcome = if let Some(e) = error_message {
        TraceOutcome::Error {
            message: e,
            step_index,
        }
    } else if timed_out {
        TraceOutcome::Timeout {
            max_ticks: config.max_ticks,
            elapsed_ticks: scheduler.now(),
        }
    } else if let Some(violation) = invariant_engine.violations().first() {
        let checkpoints = trace_builder.checkpoints();
        TraceOutcome::InvariantViolation {
            invariant_name: violation.name.clone(),
            checkpoint_index: checkpoints.len().saturating_sub(1),
        }
    } else {
        let exit_reason = process.wait().ok();
        match exit_reason {
            Some(crate::process::ExitReason::Exited(code)) => TraceOutcome::Success {
                exit_code: code,
                total_ticks: scheduler.now(),
            },
            Some(crate::process::ExitReason::Signaled(sig)) => {
                let name = match sig {
                    2 => "SIGINT",
                    9 => "SIGKILL",
                    15 => "SIGTERM",
                    _ => "UNKNOWN",
                }
                .to_string();
                TraceOutcome::Signaled {
                    signal: sig,
                    signal_name: name,
                }
            }
            Some(crate::process::ExitReason::Running) | None => TraceOutcome::Error {
                message: "Process did not exit".to_string(),
                step_index,
            },
        }
    };

    let exit_code = match &outcome {
        TraceOutcome::Success { exit_code, .. } => *exit_code,
        TraceOutcome::Signaled { .. } => -1,
        TraceOutcome::InvariantViolation { .. } => -2,
        TraceOutcome::Timeout { .. } => -3,
        TraceOutcome::Error { .. } => -4,
        TraceOutcome::ReplayDivergence { .. } => -5,
    };

    trace_builder.set_outcome(outcome);
    trace_builder.set_final_screen_hash(Some(screen.state_hash()));
    trace_builder.set_total_ticks(scheduler.now());

    let trace = trace_builder.build();

    if let Some(path) = &config.trace_path {
        let path = Path::new(path);
        if let Err(e) = crate::trace::save_trace(&trace, path) {
            eprintln!("Warning: Failed to save trace to {}: {}", path.display(), e);
        }
    }

    RunResult {
        trace,
        exit_code,
        success: exit_code == 0,
    }
}

enum StepResult {
    Ok,
    Output(Vec<u8>),
    Error(String),
}

fn execute_step(
    step: &Step,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
    _scheduler: &mut DeterministicScheduler,
    timing: &mut TimingController,
    config: &RunnerConfig,
) -> StepResult {
    let keys = KeyInjector::new(process);

    match step {
        Step::WaitFor {
            pattern,
            timeout_ms,
        } => {
            let timeout_ticks = timeout_ms.unwrap_or(5000) / 10;
            let regex = match Regex::new(pattern) {
                Ok(r) => r,
                Err(e) => return StepResult::Error(format!("Invalid regex: {}", e)),
            };

            let mut ticks_waited = 0u64;
            if config.verbose {
                eprintln!("[DEBUG] wait_for started: timeout_ticks={}", timeout_ticks);
            }
            loop {
                if ticks_waited > timeout_ticks {
                    if config.verbose {
                        let screen_text = screen.text();
                        let preview = if screen_text.len() > 200 {
                            // Safely truncate to char boundary
                            let mut end = 200;
                            while !screen_text.is_char_boundary(end) && end > 0 {
                                end -= 1;
                            }
                            if end > 0 {
                                format!("{}...", &screen_text[..end])
                            } else {
                                "[binary data]".to_string()
                            }
                        } else {
                            screen_text.clone()
                        };
                        eprintln!(
                            "[DEBUG] wait_for TIMEOUT: ticks_waited={}, timeout_ticks={}",
                            ticks_waited, timeout_ticks
                        );
                        eprintln!("[DEBUG] Screen text length: {}", screen_text.len());
                        eprintln!("[DEBUG] Screen text preview: {}", preview);
                    }
                    return StepResult::Error(format!("Timeout waiting for pattern: {}", pattern));
                }

                let _ = io.read_available(process);
                let output = io.take_output();
                screen.process(&output);

                let screen_text = screen.text();
                let has_pattern = regex.is_match(&screen_text);

                if has_pattern {
                    if config.verbose {
                        eprintln!(
                            "[DEBUG] wait_for found pattern after {} ticks",
                            ticks_waited
                        );
                    }
                    return StepResult::Ok;
                }

                let _ = timing.wait_ticks(1);
                ticks_waited += 1;

                // Debug every 50000 iterations
                if config.verbose && ticks_waited % 50000 == 0 && ticks_waited <= timeout_ticks {
                    eprintln!("[DEBUG] wait_for loop: ticks_waited={}, timeout_ticks={}, pattern_found={}", ticks_waited, timeout_ticks, has_pattern);
                }
            }
        }

        Step::WaitTicks { ticks } => {
            let _ = timing.wait_ticks(*ticks);
            StepResult::Ok
        }

        Step::SendKeys { keys: key_seq } => {
            let bytes = key_seq.to_bytes();
            match keys.inject_raw(&bytes) {
                Ok(_) => StepResult::Ok,
                Err(e) => StepResult::Error(e.to_string()),
            }
        }

        Step::SendSignal { signal } => {
            let sig = signal.to_nix_signal();
            match process.send_signal(sig) {
                Ok(_) => StepResult::Ok,
                Err(e) => StepResult::Error(e.to_string()),
            }
        }

        Step::Resize { cols, rows } => match process.resize(*cols, *rows) {
            Ok(_) => StepResult::Ok,
            Err(e) => StepResult::Error(e.to_string()),
        },

        Step::AssertScreen { pattern, .. } => {
            let regex = match Regex::new(pattern) {
                Ok(r) => r,
                Err(e) => return StepResult::Error(format!("Invalid regex: {}", e)),
            };

            let _ = io.read_available(process);
            let output = io.take_output();
            screen.process(&output);

            if !regex.is_match(&screen.text()) {
                return StepResult::Error(format!("Screen does not match pattern: {}", pattern));
            }
            StepResult::Ok
        }

        Step::AssertCursor { row, col } => {
            let cursor = screen.cursor();
            if cursor.row != *row || cursor.col != *col {
                return StepResult::Error(format!(
                    "Cursor at ({}, {}), expected ({}, {})",
                    cursor.col, cursor.row, col, row
                ));
            }
            StepResult::Ok
        }

        Step::Snapshot { name: _ } => StepResult::Ok,

        Step::CheckInvariant { invariant: _ } => StepResult::Ok,
    }
}

pub fn replay_trace(trace: &crate::trace::Trace, _config: &RunnerConfig) -> Result<i32, String> {
    let replay = crate::trace::ReplayEngine::new(trace);

    if replay.is_successful() {
        Ok(0)
    } else {
        for div in replay.divergences() {
            eprintln!("Divergence at step {}: {:?}", div.step_index, div.kind);
            eprintln!("  Expected: {}", div.expected);
            eprintln!("  Actual: {}", div.actual);
            eprintln!("  Context: {}", div.context);
        }
        Err("Replay failed - divergences detected".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Command, TerminalConfig};
    use std::collections::HashMap;

    #[test]
    fn run_simple_scenario() {
        let scenario = Scenario {
            name: "test".to_string(),
            description: "Test scenario".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitTicks { ticks: 10 }],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert!(result.success, "Scenario should complete successfully");
        assert_eq!(result.trace.steps.len(), 1);
    }

    #[test]
    fn run_scenario_with_invariants() {
        let scenario = Scenario {
            name: "test with invariants".to_string(),
            description: "Test scenario with invariants".to_string(),
            command: Command::Simple("echo test".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![
                Step::WaitTicks { ticks: 5 },
                Step::AssertScreen {
                    pattern: "test".to_string(),
                    anywhere: true,
                    row: None,
                },
            ],
            invariants: vec![crate::scenario::InvariantRef::CursorBounds],
            seed: Some(42),
            timeout_ms: Some(1000),
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert!(result.success, "Scenario should complete successfully");
        assert!(!result.trace.invariant_results.is_empty());
    }

    #[test]
    fn trace_saved_when_path_provided() {
        let scenario = Scenario {
            name: "test trace".to_string(),
            description: "Test".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
        };

        let temp_path = "/tmp/bte_test_trace.json";

        let config = RunnerConfig {
            trace_path: Some(temp_path.to_string()),
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);

        let path = std::path::Path::new(temp_path);
        assert!(path.exists());

        let loaded = crate::trace::load_trace(path).unwrap();
        assert_eq!(loaded.seed, 42);

        std::fs::remove_file(path).ok();

        assert!(result.exit_code >= 0);
    }
}
