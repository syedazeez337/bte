//! Scenario Runner
//!
//! This module provides the execution engine for running scenarios
//! and generating traces.

use crate::determinism::DeterministicScheduler;
use crate::invariants::{BuiltInInvariant, InvariantContext, InvariantEngine};
use crate::io_loop::IoLoop;
use crate::keys::KeyInjector;
use crate::process::{ProcessConfig, PtyProcess};
use crate::scenario::{InvariantRef, Scenario, Step};
use crate::screen::Screen;
use crate::timing::TimingController;
use crate::trace::{Trace, TraceBuilder, TraceOutcome};
use regex::Regex;
use std::path::Path;

// ============================================================================
// RunResult
// ============================================================================

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

// ============================================================================
// RunnerConfig
// ============================================================================

#[derive(Debug, Clone)]
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

// ============================================================================
// Main Entry Point
// ============================================================================

pub fn run_scenario(scenario: &Scenario, config: &RunnerConfig) -> RunResult {
    // Phase 1: Initialize all components
    let seed = determine_seed(config.seed, scenario.seed);
    let mut scheduler = DeterministicScheduler::new(seed);
    let mut timing = TimingController::new(seed);

    let (proc_config, mut trace_builder) = initialize_components(scenario, &scheduler, seed);

    // Handle process spawn failure gracefully instead of panicking
    let mut process = match spawn_process_safe(&proc_config, &mut trace_builder) {
        Ok(p) => p,
        Err(run_result) => return run_result, // Early return with error result
    };

    let mut io = IoLoop::new();
    let mut screen = Screen::new(
        scenario.terminal.cols as usize,
        scenario.terminal.rows as usize,
    );

    // Phase 2: Setup invariants (with fail-fast for custom invariants)
    check_custom_invariants(&scenario.invariants, &mut trace_builder);
    let mut invariant_engine = build_invariant_engine(&scenario.invariants);

    // Phase 3: Execute scenario steps
    let (step_index, timed_out, step_error, last_screen_hash, no_output_ticks) = execute_step_loop(
        scenario,
        config,
        &mut process,
        &mut io,
        &mut screen,
        &mut scheduler,
        &mut timing,
        &mut trace_builder,
        &mut invariant_engine,
    );

    // Phase 4: Final invariant evaluation
    evaluate_final_invariants(
        &mut invariant_engine,
        &mut process,
        &screen,
        step_index,
        scheduler.now(),
        last_screen_hash,
        no_output_ticks,
        &mut trace_builder,
    );

    // Phase 5: Determine outcome and build trace
    let outcome = determine_outcome(
        &step_error,
        timed_out,
        &invariant_engine,
        &mut process,
        config.max_ticks,
        scheduler.now(),
        step_index,
    );
    let exit_code = exit_code_from_outcome(&outcome);

    trace_builder.set_outcome(outcome);
    trace_builder.set_final_screen_hash(Some(screen.state_hash()));
    trace_builder.set_total_ticks(scheduler.now());

    let trace = trace_builder.build();
    save_trace(&trace, config.trace_path.as_deref());

    RunResult {
        trace,
        exit_code,
        success: exit_code == 0,
    }
}

// ============================================================================
// Phase 1: Initialization
// ============================================================================

fn determine_seed(config_seed: Option<u64>, scenario_seed: Option<u64>) -> u64 {
    config_seed.or(scenario_seed).unwrap_or_else(|| {
        let mut rng = fastrand::Rng::new();
        rng.u64(..)
    })
}

fn initialize_components(
    scenario: &Scenario,
    scheduler: &DeterministicScheduler,
    seed: u64,
) -> (ProcessConfig, TraceBuilder) {
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

    let mut trace_builder = TraceBuilder::new(scenario.clone(), seed);
    trace_builder.set_initial_rng_state(scheduler.rng_state().unwrap_or(0));

    (proc_config, trace_builder)
}

/// Spawn a process, handling errors gracefully
fn spawn_process_safe(
    proc_config: &ProcessConfig,
    trace_builder: &mut TraceBuilder,
) -> Result<PtyProcess, RunResult> {
    PtyProcess::spawn(proc_config).map_err(|e| {
        let trace = trace_builder.build_error(format!("Failed to spawn process: {}", e));
        RunResult {
            trace,
            exit_code: -1,
            success: false,
        }
    })
}

// ============================================================================
// Phase 2: Invariant Setup
// ============================================================================

fn check_custom_invariants(_invariants: &[InvariantRef], _trace_builder: &mut TraceBuilder) {
    // Custom invariants are now supported via BuiltInInvariant::Custom
    // This function is kept for any pre-flight validation if needed
    // Currently, custom invariants are handled directly in build_invariant_engine
}

fn build_invariant_engine(invariants: &[InvariantRef]) -> InvariantEngine {
    let mut engine = InvariantEngine::new();

    if invariants.is_empty() {
        return engine;
    }

    let builtins: Vec<BuiltInInvariant> = invariants
        .iter()
        .filter_map(|inv| match inv {
            InvariantRef::CursorBounds => Some(BuiltInInvariant::CursorBounds),
            InvariantRef::NoDeadlock { timeout_ms } => {
                let ticks = timeout_ms.unwrap_or(1000) / 10;
                Some(BuiltInInvariant::NoDeadlock {
                    timeout_ticks: ticks.max(10),
                })
            }
            InvariantRef::SignalHandled { signal } => Some(BuiltInInvariant::SignalHandled {
                signal: format!("{:?}", signal).to_uppercase(),
            }),
            InvariantRef::ScreenContains { pattern } => Some(BuiltInInvariant::ScreenContains {
                pattern: pattern.clone(),
            }),
            InvariantRef::ScreenNotContains { pattern } => {
                Some(BuiltInInvariant::ScreenNotContains {
                    pattern: pattern.clone(),
                })
            }
            InvariantRef::NoOutputAfterExit => Some(BuiltInInvariant::NoOutputAfterExit),
            InvariantRef::ProcessTerminatedCleanly { allowed_signals } => {
                Some(BuiltInInvariant::ProcessTerminatedCleanly {
                    allowed_signals: allowed_signals.clone(),
                })
            }
            InvariantRef::ScreenStable { min_ticks } => Some(BuiltInInvariant::ScreenStable {
                min_ticks: *min_ticks,
            }),
            InvariantRef::ViewportValid => Some(BuiltInInvariant::ViewportValid),
            InvariantRef::ResponseTime { max_ticks } => Some(BuiltInInvariant::ResponseTime {
                max_ticks: *max_ticks,
            }),
            InvariantRef::MaxLatency { max_ticks } => Some(BuiltInInvariant::MaxLatency {
                max_ticks: *max_ticks,
            }),
            InvariantRef::Custom {
                name,
                pattern,
                should_contain,
                expected_row,
                expected_col,
                description,
            } => Some(BuiltInInvariant::Custom {
                name: name.clone(),
                pattern: pattern.clone(),
                should_contain: *should_contain,
                expected_row: *expected_row,
                expected_col: *expected_col,
                description: description
                    .clone()
                    .or(Some(format!("Custom invariant: {}", name))),
            }),
        })
        .collect();

    engine.add_builtin_invariants(&builtins);
    engine
}

// ============================================================================
// Phase 3: Step Execution Loop
// ============================================================================

struct LoopState {
    step_index: usize,
    last_screen_hash: Option<u64>,
    no_output_ticks: u64,
}

fn execute_step_loop(
    scenario: &Scenario,
    config: &RunnerConfig,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
    scheduler: &mut DeterministicScheduler,
    timing: &mut TimingController,
    trace_builder: &mut TraceBuilder,
    invariant_engine: &mut InvariantEngine,
) -> (usize, bool, Option<String>, Option<u64>, u64) {
    let mut state = LoopState {
        step_index: 0,
        last_screen_hash: None,
        no_output_ticks: 0,
    };
    let mut timed_out = false;
    let mut step_error = None;

    trace_builder.add_checkpoint("initial", scheduler, Some(screen));

    for step in &scenario.steps {
        // Check timeout - use >= to trigger at exactly max_ticks
        if scheduler.now() >= config.max_ticks {
            timed_out = true;
            break;
        }

        // Evaluate invariants before step
        let mut ctx = InvariantContext {
            screen: Some(screen),
            process,
            step: state.step_index,
            tick: scheduler.now(),
            _is_replay: false,
            last_screen_hash: state.last_screen_hash,
            no_output_ticks: state.no_output_ticks,
            expected_signal: None,
        };
        record_invariant_results(invariant_engine.evaluate(&mut ctx), trace_builder);

        // Execute step and record output
        trace_builder.start_step(step.clone(), Some(screen), scheduler);
        step_error = execute_and_record_step(
            step,
            process,
            io,
            screen,
            scheduler,
            timing,
            config,
            trace_builder,
        );

        // Update screen state tracking
        let current_hash = screen.state_hash();
        if Some(current_hash) == state.last_screen_hash {
            state.no_output_ticks += 1;
        } else {
            state.no_output_ticks = 0;
        }
        state.last_screen_hash = Some(current_hash);

        // Record checkpoint
        trace_builder.end_step(Some(screen), scheduler);
        trace_builder.add_checkpoint(
            &format!("after_step_{}", state.step_index),
            scheduler,
            Some(screen),
        );

        state.step_index += 1;

        // Check invariant violations
        if !invariant_engine.all_satisfied() {
            break;
        }
    }

    (
        state.step_index,
        timed_out,
        step_error,
        state.last_screen_hash,
        state.no_output_ticks,
    )
}

fn record_invariant_results(
    results: &[crate::invariants::InvariantResult],
    trace_builder: &mut TraceBuilder,
) {
    for result in results {
        trace_builder.record_invariant_result(result);
        if result.violation() {
            trace_builder.record_invariant_violation(&result.name);
        }
    }
}

fn execute_and_record_step(
    step: &Step,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
    scheduler: &mut DeterministicScheduler,
    timing: &mut TimingController,
    config: &RunnerConfig,
    trace_builder: &mut TraceBuilder,
) -> Option<String> {
    let result = execute_step(step, process, io, screen, scheduler, timing, config);

    match result {
        StepResult::Ok => {
            let _ = io.read_available(process);
            let output = io.take_output();
            trace_builder.record_pty_output(&output);
            None
        }
        StepResult::Output(output) => {
            trace_builder.record_pty_output(&output);
            None
        }
        StepResult::Error(e) => {
            trace_builder.record_error(&e);
            Some(e)
        }
    }
}

// ============================================================================
// Phase 4: Final Evaluation
// ============================================================================

fn evaluate_final_invariants(
    invariant_engine: &mut InvariantEngine,
    process: &mut PtyProcess,
    screen: &Screen,
    step_index: usize,
    tick: u64,
    last_screen_hash: Option<u64>,
    no_output_ticks: u64,
    trace_builder: &mut TraceBuilder,
) {
    let mut ctx = InvariantContext {
        screen: Some(screen),
        process,
        step: step_index,
        tick,
        _is_replay: false,
        last_screen_hash,
        no_output_ticks,
        expected_signal: None,
    };
    for result in invariant_engine.evaluate(&mut ctx) {
        trace_builder.record_invariant_result(result);
    }
}

// ============================================================================
// Phase 5: Outcome Determination
// ============================================================================

fn determine_outcome(
    step_error: &Option<String>,
    timed_out: bool,
    invariant_engine: &InvariantEngine,
    process: &mut PtyProcess,
    max_ticks: u64,
    elapsed_ticks: u64,
    step_index: usize,
) -> TraceOutcome {
    if let Some(ref e) = step_error {
        return TraceOutcome::Error {
            message: e.clone(),
            step_index,
        };
    }

    if timed_out {
        return TraceOutcome::Timeout {
            max_ticks,
            elapsed_ticks,
        };
    }

    if let Some(ref violation) = invariant_engine.violations().first() {
        // We can't access checkpoints from here directly, so use a placeholder
        return TraceOutcome::InvariantViolation {
            invariant_name: violation.name.clone(),
            checkpoint_index: 0,
        };
    }

    // Determine process exit status
    let exit_reason = match process.wait() {
        Ok(reason) => Some(reason),
        Err(_) => process.exit_reason(),
    };

    match exit_reason {
        Some(crate::process::ExitReason::Exited(code)) => TraceOutcome::Success {
            exit_code: code,
            total_ticks: elapsed_ticks,
        },
        Some(crate::process::ExitReason::Signaled(sig)) => {
            use nix::sys::signal::Signal;
            let signal_name = match Signal::try_from(sig) {
                Ok(Signal::SIGINT) => "SIGINT",
                Ok(Signal::SIGKILL) => "SIGKILL",
                Ok(Signal::SIGTERM) => "SIGTERM",
                Ok(Signal::SIGHUP) => "SIGHUP",
                Ok(Signal::SIGQUIT) => "SIGQUIT",
                Ok(Signal::SIGABRT) => "SIGABRT",
                Ok(Signal::SIGFPE) => "SIGFPE",
                Ok(Signal::SIGSEGV) => "SIGSEGV",
                Ok(Signal::SIGPIPE) => "SIGPIPE",
                Ok(Signal::SIGALRM) => "SIGALRM",
                Ok(Signal::SIGUSR1) => "SIGUSR1",
                Ok(Signal::SIGUSR2) => "SIGUSR2",
                Ok(Signal::SIGCHLD) => "SIGCHLD",
                Ok(Signal::SIGCONT) => "SIGCONT",
                Ok(Signal::SIGSTOP) => "SIGSTOP",
                Ok(Signal::SIGTSTP) => "SIGTSTP",
                Ok(Signal::SIGTTIN) => "SIGTTIN",
                Ok(Signal::SIGTTOU) => "SIGTTOU",
                Ok(Signal::SIGBUS) => "SIGBUS",
                Ok(Signal::SIGSYS) => "SIGSYS",
                Ok(Signal::SIGTRAP) => "SIGTRAP",
                Ok(Signal::SIGURG) => "SIGURG",
                Ok(Signal::SIGVTALRM) => "SIGVTALRM",
                Ok(Signal::SIGXCPU) => "SIGXCPU",
                Ok(Signal::SIGXFSZ) => "SIGXFSZ",
                Ok(Signal::SIGWINCH) => "SIGWINCH",
                _ => "UNKNOWN",
            }
            .to_string();
            TraceOutcome::Signaled {
                signal: sig,
                signal_name,
            }
        }
        Some(crate::process::ExitReason::Running) | None => TraceOutcome::Error {
            message: "Process did not exit or exit status unavailable".to_string(),
            step_index,
        },
    }
}

fn exit_code_from_outcome(outcome: &TraceOutcome) -> i32 {
    // Use positive exit codes in the reserved range (124-125)
    // Unix exit codes: 0=success, 1=general error, 2= misuse,
    // 126=not executable, 127=not found, 128+N=signal N
    match outcome {
        TraceOutcome::Success { exit_code, .. } => *exit_code,
        // 124: commonly used for timeout or signal death
        TraceOutcome::Signaled { .. } => 124,
        // 125: general error / invariant violation
        TraceOutcome::InvariantViolation { .. } => 125,
        // 124: timeout (didn't complete normally)
        TraceOutcome::Timeout { .. } => 124,
        // 125: general error
        TraceOutcome::Error { .. } => 125,
        // 125: replay divergence is a test failure
        TraceOutcome::ReplayDivergence { .. } => 125,
    }
}

// ============================================================================
// Trace Finalization
// ============================================================================

fn save_trace(trace: &Trace, path: Option<&str>) {
    if let Some(p) = path {
        let path = Path::new(p);
        if let Err(e) = crate::trace::save_trace(trace, path) {
            eprintln!("Warning: Failed to save trace to {}: {}", path.display(), e);
        }
    }
}

// ============================================================================
// Step Execution
// ============================================================================

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
        } => execute_wait_for(pattern, *timeout_ms, process, io, screen, timing, config),

        Step::WaitForFuzzy {
            pattern,
            max_distance,
            min_similarity,
            timeout_ms,
        } => execute_wait_for_fuzzy(
            pattern,
            *max_distance,
            *min_similarity,
            *timeout_ms,
            process,
            io,
            screen,
            timing,
            config,
        ),

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

        Step::AssertScreen { pattern, .. } => execute_assert_screen(pattern, process, io, screen),

        Step::AssertCursor { row, col } => execute_assert_cursor(screen, *row, *col),

        Step::Snapshot { .. } => StepResult::Ok,

        Step::CheckInvariant { .. } => StepResult::Ok,

        Step::MouseClick {
            row,
            col,
            button,
            enable_tracking,
        } => execute_mouse_click(*row, *col, *button, *enable_tracking, &keys),

        Step::MouseScroll {
            row,
            col,
            direction,
            count,
            enable_tracking,
        } => execute_mouse_scroll(*row, *col, direction, *count, *enable_tracking, &keys),

        Step::WaitScreen {
            pattern,
            timeout_ms,
        } => execute_wait_screen(pattern, *timeout_ms, process, io, screen, timing, config),

        Step::AssertNotScreen { pattern } => execute_assert_not_screen(pattern, screen),

        Step::TakeScreenshot { path, description } => {
            execute_take_screenshot(path, description.clone(), screen, timing)
        }

        Step::AssertScreenshot {
            path,
            max_differences,
            ignore_regions,
            compare_colors,
            compare_text,
        } => execute_assert_screenshot(
            path,
            *max_differences,
            ignore_regions.clone(),
            *compare_colors,
            *compare_text,
            screen,
            timing,
        ),
    }
}

/// Check if a regex pattern might cause catastrophic backtracking
/// Returns None if safe, Some(message) if potentially unsafe
fn check_regex_complexity(pattern: &str) -> Option<String> {
    // Count nesting depth of quantifiers that can cause exponential backtracking
    let mut nest_level = 0;
    let mut max_nest = 0;
    let mut in_bracket = false;
    let mut prev_char = '\0';

    for c in pattern.chars() {
        match c {
            '[' => in_bracket = true,
            ']' => in_bracket = false,
            '(' if !in_bracket => {
                nest_level += 1;
                max_nest = max_nest.max(nest_level);
            }
            ')' if !in_bracket => {
                if nest_level > 0 {
                    nest_level -= 1;
                }
            }
            '+' | '*' | '?' if prev_char != '\\' && !in_bracket => {
                // Quantifier after something - check if we're nested
                if nest_level > 2 {
                    return Some(format!(
                        "Regex has deeply nested quantifiers (nest level {}), which may cause catastrophic backtracking",
                        nest_level
                    ));
                }
            }
            '{' if prev_char != '\\' && !in_bracket => {
                // Could be {n,m} quantifier, check for nesting
                if nest_level > 2 {
                    return Some(format!(
                        "Regex has nested quantifiers with {{}} syntax, which may cause catastrophic backtracking",
                    ));
                }
            }
            _ => {}
        }
        prev_char = c;
    }

    // Check for obviously problematic patterns
    let problematic = [
        r"\(\S+\)\+",      // (something)+
        r"\[\S+\]\+",      // [something]+
        r"\(\S+\s*\)\+\?", // Nested optional groups
        r"^\(\S+\|\)",     // Left-recursive alternation at start
    ];

    for pat in &problematic {
        if pattern.contains(pat) {
            return Some(format!(
                "Regex contains potentially catastrophic pattern: {}",
                pat
            ));
        }
    }

    None
}

fn execute_wait_for(
    pattern: &str,
    timeout_ms: Option<u64>,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
    timing: &mut TimingController,
    config: &RunnerConfig,
) -> StepResult {
    let timeout_ticks = timeout_ms.unwrap_or(5000) / 10;

    // Check regex complexity before compiling
    if let Some(msg) = check_regex_complexity(pattern) {
        return StepResult::Error(format!("Unsafe regex pattern: {}", msg));
    }

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
            let screen_text = screen.text();
            let preview = truncate_screen_preview(&screen_text);
            if config.verbose {
                eprintln!(
                    "[DEBUG] wait_for TIMEOUT: ticks_waited={}, timeout_ticks={}, screen_text_len={}",
                    ticks_waited, timeout_ticks, screen_text.len()
                );
                eprintln!("[DEBUG] Screen preview: {}", preview);
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

        if config.verbose && ticks_waited % 50000 == 0 && ticks_waited <= timeout_ticks {
            eprintln!(
                "[DEBUG] wait_for loop: ticks_waited={}, timeout_ticks={}, pattern_found={}",
                ticks_waited, timeout_ticks, has_pattern
            );
        }
    }
}

/// Execute fuzzy wait - waits for output approximately matching a pattern
fn execute_wait_for_fuzzy(
    pattern: &str,
    max_distance: usize,
    min_similarity: Option<f64>,
    timeout_ms: Option<u64>,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
    timing: &mut TimingController,
    config: &RunnerConfig,
) -> StepResult {
    use crate::fuzzy::contains_fuzzy;

    let timeout_ticks = timeout_ms.unwrap_or(5000) / 10;
    let effective_max_distance = if let Some(similarity) = min_similarity {
        // Calculate max distance from similarity threshold
        // If similarity >= threshold, distance is acceptable
        // We use a dynamic calculation: for 90% similarity, max distance is 10% of pattern length
        let pattern_len = pattern.chars().count();
        let threshold_distance = (pattern_len as f64 * (1.0 - similarity)) as usize;
        threshold_distance.max(max_distance)
    } else {
        max_distance
    };

    let mut ticks_waited = 0u64;
    if config.verbose {
        eprintln!(
            "[DEBUG] wait_for_fuzzy started: pattern='{}', max_distance={}, timeout_ticks={}",
            pattern, effective_max_distance, timeout_ticks
        );
    }

    loop {
        if ticks_waited > timeout_ticks {
            let screen_text = screen.text();
            let preview = truncate_screen_preview(&screen_text);
            if config.verbose {
                eprintln!(
                    "[DEBUG] wait_for_fuzzy TIMEOUT: ticks_waited={}, pattern='{}'",
                    ticks_waited, pattern
                );
                eprintln!("[DEBUG] Screen preview: {}", preview);
            }
            return StepResult::Error(format!(
                "Timeout waiting for fuzzy pattern: {} (max_distance={})",
                pattern, effective_max_distance
            ));
        }

        let _ = io.read_available(process);
        let output = io.take_output();
        screen.process(&output);

        let screen_text = screen.text();

        // Check for fuzzy match
        if let Some(fuzzy_match) = contains_fuzzy(&screen_text, pattern, effective_max_distance) {
            let actual_similarity = fuzzy_match.similarity;
            let distance = fuzzy_match.distance;

            if config.verbose {
                eprintln!(
                    "[DEBUG] wait_for_fuzzy found match after {} ticks: distance={}, similarity={:.2}",
                    ticks_waited, distance, actual_similarity
                );
            }

            if let Some(threshold) = min_similarity {
                if actual_similarity < threshold {
                    if config.verbose {
                        eprintln!(
                            "[DEBUG] wait_for_fuzzy similarity {:.2} below threshold {:.2}, continuing",
                            actual_similarity, threshold
                        );
                    }
                } else {
                    return StepResult::Ok;
                }
            } else {
                return StepResult::Ok;
            }
        }

        let _ = timing.wait_ticks(1);
        ticks_waited += 1;
    }
}

fn truncate_screen_preview(text: &str) -> String {
    if text.len() <= 200 {
        return text.to_string();
    }
    let mut end = 200;
    while !text.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    if end > 0 {
        format!("{}...", &text[..end])
    } else {
        "[binary data]".to_string()
    }
}

fn execute_assert_screen(
    pattern: &str,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
) -> StepResult {
    // Check regex complexity before compiling
    if let Some(msg) = check_regex_complexity(pattern) {
        return StepResult::Error(format!("Unsafe regex pattern: {}", msg));
    }

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

fn execute_assert_cursor(screen: &Screen, expected_row: usize, expected_col: usize) -> StepResult {
    let cursor = screen.cursor();
    if cursor.row != expected_row || cursor.col != expected_col {
        return StepResult::Error(format!(
            "Cursor at ({}, {}), expected ({}, {})",
            cursor.col, cursor.row, expected_col, expected_row
        ));
    }
    StepResult::Ok
}

/// Enable xterm mouse tracking mode
fn enable_mouse_tracking(keys: &KeyInjector) -> StepResult {
    // SGR 1006 mode - extended mouse reporting
    let seq = b"\x1b[?1006h";
    match keys.inject_raw(seq) {
        Ok(_) => StepResult::Ok,
        Err(e) => StepResult::Error(format!("Failed to enable mouse tracking: {}", e)),
    }
}

/// Execute mouse click at specified position
fn execute_mouse_click(
    row: u16,
    col: u16,
    button: u8,
    enable_tracking: bool,
    keys: &KeyInjector,
) -> StepResult {
    if enable_tracking {
        match enable_mouse_tracking(keys) {
            StepResult::Ok => {}
            StepResult::Error(e) => return StepResult::Error(e),
            StepResult::Output(_) => unreachable!(),
        }
    }

    // SGR mouse event format: CSI M Cb Cx Cy
    // Cb = button code (0=press, 3=release) + 32
    // Cx, Cy = column, row + 32 (1-indexed)
    //
    // For coordinates >= 256, SGR uses UTF-8 encoding of 0x100 + value:
    // - Value 0-255: single byte (value)
    // - Value 256+: two bytes 0xC2 (value >> 8), 0xXX (value & 0xFF)
    let cxb = (button + 32) as u8;

    // Clamp to reasonable bounds (2000 is way beyond any terminal size)
    let col_clamped = (col + 1).min(2000);
    let row_clamped = (row + 1).min(2000);

    // Encode column and row coordinates
    let cxx = encode_sgr_coordinate(col_clamped);
    let cxy = encode_sgr_coordinate(row_clamped);

    // CSI M Cb Cx Cy - cxb is a single byte, cxx/cxy are UTF-8 encoded strings
    let seq = format!("\x1b[M{}{}{}", cxb as char, cxx, cxy);
    let bytes = seq.as_bytes();

    match keys.inject_raw(bytes) {
        Ok(_) => StepResult::Ok,
        Err(e) => StepResult::Error(format!("Failed to send mouse click: {}", e)),
    }
}

/// Encode a coordinate for SGR mouse protocol.
///
/// Returns a String containing the UTF-8 encoded coordinate:
/// - 0-255: single byte (value + 32)
/// - 256+: two bytes (0xC2, 0x100 + value + 32)
fn encode_sgr_coordinate(value: u16) -> String {
    let encoded = (value + 32) as u32;
    if encoded <= 255 {
        ((encoded as u8) as char).to_string()
    } else {
        // Two-byte UTF-8 encoding for values >= 256
        // 0xC2 is the leading byte for 0x100-0x7FF range
        let high = (0xC2u8) as char;
        let low = ((encoded - 256) as u8) as char;
        format!("{}{}", high, low)
    }
}

/// Execute mouse scroll at specified position
fn execute_mouse_scroll(
    row: u16,
    col: u16,
    direction: &crate::scenario::ScrollDirection,
    count: u8,
    enable_tracking: bool,
    keys: &KeyInjector,
) -> StepResult {
    if enable_tracking {
        match enable_mouse_tracking(keys) {
            StepResult::Ok => {}
            StepResult::Error(e) => return StepResult::Error(e),
            StepResult::Output(_) => unreachable!(),
        }
    }

    // Scroll up button = 64, scroll down button = 65
    let button = match direction {
        crate::scenario::ScrollDirection::Up => 64u8,
        crate::scenario::ScrollDirection::Down => 65u8,
    };

    // Clamp to reasonable bounds and encode properly
    let col_clamped = (col + 1).min(2000);
    let row_clamped = (row + 1).min(2000);
    let cxx = encode_sgr_coordinate(col_clamped);
    let cxy = encode_sgr_coordinate(row_clamped);

    // Send multiple scroll events if count > 1
    for _ in 0..count {
        // CSI M Cb Cx Cy (button press)
        let press_seq = format!("\x1b[M{}{}{}", button as char, cxx, cxy);
        // CSI M Cb Cx Cy (button release - button + 3)
        let release_seq = format!("\x1b[M{}{}{}", (button + 3) as char, cxx, cxy);

        if let Err(e) = keys.inject_raw(press_seq.as_bytes()) {
            return StepResult::Error(format!("Failed to send scroll press: {}", e));
        }
        if let Err(e) = keys.inject_raw(release_seq.as_bytes()) {
            return StepResult::Error(format!("Failed to send scroll release: {}", e));
        }
    }

    StepResult::Ok
}

/// Wait for pattern in screen content (checks screen state, not stream)
fn execute_wait_screen(
    pattern: &str,
    timeout_ms: Option<u64>,
    process: &mut PtyProcess,
    io: &mut IoLoop,
    screen: &mut Screen,
    timing: &mut TimingController,
    _config: &RunnerConfig,
) -> StepResult {
    let timeout_ticks = timeout_ms.unwrap_or(5000) / 10;

    // Check regex complexity before compiling
    if let Some(msg) = check_regex_complexity(pattern) {
        return StepResult::Error(format!("Unsafe regex pattern: {}", msg));
    }

    let regex = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => return StepResult::Error(format!("Invalid regex: {}", e)),
    };

    let mut ticks_waited = 0u64;

    loop {
        // Drain any available output and update screen
        let _ = io.read_available(process);
        let output = io.take_output();
        screen.process(&output);

        // Check if pattern is in screen content
        if regex.is_match(&screen.text()) {
            return StepResult::Ok;
        }

        // Check timeout
        if ticks_waited >= timeout_ticks {
            let screen_text = screen.text();
            let preview = truncate_screen_preview(&screen_text);
            return StepResult::Error(format!(
                "wait_screen timeout after {} ticks. Screen preview:\n{}",
                ticks_waited, preview
            ));
        }

        // Wait for next tick
        let _ = timing.wait_ticks(1);
        ticks_waited += 1;
    }
}

/// Assert screen does NOT contain pattern
fn execute_assert_not_screen(pattern: &str, screen: &Screen) -> StepResult {
    if screen.text().contains(pattern) {
        return StepResult::Error(format!(
            "Screen contains pattern '{}' but should not",
            pattern
        ));
    }
    StepResult::Ok
}

/// Take a screenshot of the current screen state
fn execute_take_screenshot(
    path: &str,
    description: Option<String>,
    screen: &Screen,
    timing: &TimingController,
) -> StepResult {
    use crate::screenshot::Screenshot;
    use std::fs;

    // Validate path to prevent path traversal attacks
    let path_obj = std::path::Path::new(path);
    if path_obj.is_absolute() {
        // Allow absolute paths but warn about security implications
        // In production, you might want to restrict this to a sandbox directory
    } else if path_obj.starts_with("..") {
        return StepResult::Error(
            "Path traversal (../) is not allowed for screenshots".to_string(),
        );
    }

    // Normalize and check for embedded path traversal
    let normalized = match path_obj.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If canonicalize fails, do basic validation
            let clean = std::path::Path::new(path).components().fold(
                std::path::PathBuf::new(),
                |mut acc, comp| {
                    if comp == std::path::Component::ParentDir {
                        acc.pop();
                    } else {
                        acc.push(comp);
                    }
                    acc
                },
            );
            if clean.to_string_lossy().contains("..") {
                return StepResult::Error(
                    "Path traversal (../) is not allowed for screenshots".to_string(),
                );
            }
            clean
        }
    };

    // Optional: restrict to current working directory and subdirectories
    // let cwd = std::env::current_dir().map_err(|_| StepResult::Error("Cannot determine current directory".to_string()))?;
    // if !normalized.starts_with(&cwd) {
    //     return StepResult::Error("Screenshot path must be within current directory".to_string());
    // }

    let timestamp = timing.now();
    let screenshot = Screenshot::from_screen(screen, timestamp);

    let data = match serde_yaml::to_string(&screenshot) {
        Ok(d) => d,
        Err(e) => {
            return StepResult::Error(format!("Failed to serialize screenshot: {}", e));
        }
    };

    let mut output = String::new();
    if let Some(desc) = description {
        output.push_str(&format!("# Description: {}\n", desc));
    }
    output.push_str(&format!("# Taken at tick: {}\n", timestamp));
    output.push_str(&format!(
        "# Dimensions: {}x{}\n",
        screenshot.cols, screenshot.rows
    ));
    output.push_str(&format!(
        "# Cursor: ({},{})\n",
        screenshot.cursor.0, screenshot.cursor.1
    ));
    output.push_str("---\n");
    output.push_str(&data);

    if let Some(parent) = normalized.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return StepResult::Error(format!(
                    "Failed to create directory for screenshot: {}",
                    e
                ));
            }
        }
    }

    if let Err(e) = fs::write(&normalized, &output) {
        return StepResult::Error(format!("Failed to write screenshot: {}", e));
    }

    StepResult::Ok
}

/// Assert screen matches a baseline screenshot
fn execute_assert_screenshot(
    path: &str,
    max_differences: usize,
    ignore_regions: Vec<crate::scenario::IgnoreRegionConfig>,
    compare_colors: bool,
    compare_text: bool,
    screen: &Screen,
    timing: &TimingController,
) -> StepResult {
    use crate::screenshot::{compare_screenshots, DiffConfig, IgnoreRegion, Screenshot};

    // Load baseline screenshot
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return StepResult::Error(format!("Failed to read baseline screenshot: {}", e));
        }
    };

    let baseline: Screenshot = match serde_yaml::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            return StepResult::Error(format!("Failed to parse baseline screenshot: {}", e));
        }
    };

    // Capture current screen
    let timestamp = timing.now();
    let actual = Screenshot::from_screen(screen, timestamp);

    // Build diff config
    let mut ignore_regions_list = Vec::new();
    for r in ignore_regions {
        ignore_regions_list.push(IgnoreRegion::new(r.top, r.left, r.bottom, r.right));
    }

    let config = DiffConfig {
        ignore_regions: ignore_regions_list,
        max_differences,
        compare_colors,
        compare_text,
        compare_cursor: true,
        diff_char: '?',
    };

    let result = compare_screenshots(&baseline, &actual, &config);

    if result.matches {
        StepResult::Ok
    } else {
        let mut error_msg = format!(
            "Screenshot mismatch: {} different cells, similarity={:.2}%",
            result.different_cells,
            result.similarity * 100.0
        );

        if result.size_mismatch {
            error_msg.push_str(&format!(
                ", SIZE MISMATCH (baseline={}x{}, actual={}x{})",
                baseline.cols, baseline.rows, actual.cols, actual.rows
            ));
        }

        if result.cursor_mismatch {
            error_msg.push_str(&format!(
                ", CURSOR MISMATCH (expected={:?}, actual={:?})",
                baseline.cursor, actual.cursor
            ));
        }

        StepResult::Error(error_msg)
    }
}

// ============================================================================
// Replay
// ============================================================================

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Command, TerminalConfig};
    use std::collections::HashMap;
    use tempfile::TempDir;

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
            tags: vec![],
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
                Step::WaitFor {
                    pattern: "test".to_string(),
                    timeout_ms: Some(1000),
                },
                Step::AssertScreen {
                    pattern: "test".to_string(),
                    anywhere: true,
                    row: None,
                },
            ],
            invariants: vec![crate::scenario::InvariantRef::CursorBounds],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
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
            tags: vec![],
        };

        // Use a unique temp file instead of hardcoded path to avoid race conditions
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("bte_test_trace.json");

        let config = RunnerConfig {
            trace_path: Some(temp_path.to_string_lossy().to_string()),
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);

        assert!(temp_path.exists());

        let loaded = crate::trace::load_trace(&temp_path).unwrap();
        assert_eq!(loaded.seed, 42);

        // Cleanup is automatic when TempDir is dropped
        assert!(result.exit_code >= 0);
    }

    #[test]
    fn test_echo_command() {
        let scenario = Scenario {
            name: "echo-test".to_string(),
            description: "Test echo command".to_string(),
            command: Command::Simple("echo hello_world".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitFor {
                pattern: "hello_world".to_string(),
                timeout_ms: Some(5000),
            }],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert_eq!(result.exit_code, 0, "Echo should succeed with exit code 0");
        assert!(result.trace.steps.len() >= 1);
    }

    #[test]
    fn test_send_keys_with_read() {
        use crate::scenario::KeySequence;
        // Use `read` command which reads one line and exits
        let scenario = Scenario {
            name: "send-keys-test".to_string(),
            description: "Test send_keys with read".to_string(),
            command: Command::Simple("read variable; echo $variable".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![
                Step::SendKeys {
                    keys: KeySequence::Text("test_value".to_string()),
                },
                Step::SendKeys {
                    keys: KeySequence::Text("\n".to_string()),
                },
                Step::WaitFor {
                    pattern: "test_value".to_string(),
                    timeout_ms: Some(5000),
                },
            ],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 2000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert_eq!(result.exit_code, 0, "Read should complete and echo input");
    }

    #[test]
    fn test_resize_step() {
        let scenario = Scenario {
            name: "resize-test".to_string(),
            description: "Test resize step".to_string(),
            command: Command::Simple("stty size".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![
                Step::Resize {
                    cols: 120,
                    rows: 40,
                },
                Step::WaitTicks { ticks: 5 },
            ],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert_eq!(result.exit_code, 0, "Resize should not cause errors");
    }

    #[test]
    fn test_signal_termination() {
        use crate::scenario::SignalName;
        let scenario = Scenario {
            name: "signal-test".to_string(),
            description: "Test SIGTERM signal".to_string(),
            command: Command::Simple("sleep 30".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![
                Step::WaitTicks { ticks: 10 },
                Step::SendSignal {
                    signal: SignalName::Sigterm,
                },
                Step::WaitTicks { ticks: 5 },
            ],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        // SIGTERM should terminate the process (exit code 143 on most systems)
        assert_ne!(
            result.exit_code, 0,
            "Process should be terminated by SIGTERM"
        );
    }

    #[test]
    fn test_multiline_output() {
        let scenario = Scenario {
            name: "multiline-test".to_string(),
            description: "Test multiline output".to_string(),
            command: Command::Simple("printf 'line1\\nline2\\nline3\\n'".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitFor {
                pattern: "line2".to_string(),
                timeout_ms: Some(5000),
            }],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert_eq!(result.exit_code, 0, "Multi-line output should be captured");
    }

    #[test]
    fn test_seed_determinism() {
        let scenario = Scenario {
            name: "seed-test".to_string(),
            description: "Test seed determinism".to_string(),
            command: Command::Simple("echo test".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitFor {
                pattern: "test".to_string(),
                timeout_ms: Some(5000),
            }],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config1 = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };
        let config2 = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result1 = run_scenario(&scenario, &config1);
        let result2 = run_scenario(&scenario, &config2);

        assert_eq!(
            result1.exit_code, result2.exit_code,
            "Same seed should produce same exit code"
        );
        assert_eq!(
            result1.trace.seed, result2.trace.seed,
            "Seed should be recorded in trace"
        );
    }

    #[test]
    fn test_trace_records_steps() {
        let scenario = Scenario {
            name: "trace-test".to_string(),
            description: "Test trace recording".to_string(),
            command: Command::Simple("echo test".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitFor {
                pattern: "test".to_string(),
                timeout_ms: Some(5000),
            }],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);

        assert!(result.exit_code == 0);
        assert!(!result.trace.version.is_empty());
        assert!(result.trace.steps.len() >= 1);
    }

    #[test]
    fn test_assert_screen_step() {
        let scenario = Scenario {
            name: "assert-test".to_string(),
            description: "Test assert_screen step".to_string(),
            command: Command::Simple("printf 'expected_content\\n'".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![
                Step::WaitFor {
                    pattern: "expected_content".to_string(),
                    timeout_ms: Some(5000),
                },
                Step::AssertScreen {
                    pattern: "expected_content".to_string(),
                    anywhere: true,
                    row: None,
                },
            ],
            invariants: vec![],
            seed: Some(42),
            timeout_ms: Some(1000),
            tags: vec![],
        };

        let config = RunnerConfig {
            trace_path: None,
            verbose: false,
            max_ticks: 1000,
            tick_delay_ms: 0,
            seed: Some(42),
        };

        let result = run_scenario(&scenario, &config);
        assert_eq!(
            result.exit_code, 0,
            "Assert screen should pass when pattern exists"
        );
    }
}
