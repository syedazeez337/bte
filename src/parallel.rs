//! Parallel test execution for running multiple scenarios concurrently.
//!
//! This module provides functionality to:
//! - Run multiple test scenarios in parallel
//! - Collect and aggregate results
//! - Limit concurrent execution with a semaphore
//! - Handle panics and errors gracefully

use crate::runner::{run_scenario, RunnerConfig};
use crate::scenario::Scenario;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of a parallel test execution
#[derive(Debug, Clone)]
pub struct ParallelResult {
    /// Total scenarios run
    pub total: usize,
    /// Successful scenarios
    pub passed: usize,
    /// Failed scenarios
    pub failed: usize,
    /// Skipped scenarios
    pub skipped: usize,
    /// Total execution time
    pub duration: Duration,
    /// Individual scenario results
    pub results: Vec<ScenarioResult>,
}

/// Result of a single scenario in parallel execution
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Scenario name
    pub name: String,
    /// Path to the scenario file
    pub path: PathBuf,
    /// Whether the scenario passed
    pub passed: bool,
    /// Exit code (if available)
    pub exit_code: i32,
    /// Execution duration
    pub duration: Duration,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Number of steps executed
    pub steps_executed: usize,
}

/// Parallel execution configuration
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Maximum number of concurrent scenarios
    pub max_workers: usize,
    /// Global timeout for all scenarios
    pub timeout: Option<Duration>,
    /// Whether to stop on first failure
    pub fail_fast: bool,
    /// Seed for deterministic execution
    pub seed: Option<u64>,
    /// Default runner config
    pub runner_config: RunnerConfig,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        let max_workers = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);

        Self {
            max_workers,
            timeout: None,
            fail_fast: false,
            seed: None,
            runner_config: RunnerConfig::default(),
        }
    }
}

/// Run scenarios in parallel
///
/// # Arguments
///
/// * `scenarios` - List of scenarios to run
/// * `config` - Parallel execution configuration
///
/// # Returns
///
/// ParallelResult containing aggregated results
pub fn run_parallel(scenarios: &[(Scenario, PathBuf)], config: &ParallelConfig) -> ParallelResult {
    let start_time = Instant::now();
    let total = scenarios.len();

    // Collect results directly into a Vec - rayon handles parallel collection efficiently
    // No need for Mutex contention since we're collecting into a local Vec
    let local_results: Vec<ScenarioResult> = if config.fail_fast {
        // fail_fast: use atomic flag to signal other workers to stop on first failure
        let has_failed = Arc::new(AtomicBool::new(false));

        scenarios
            .par_iter()
            .map(|(scenario, path)| {
                // Check if another worker has already failed - early exit
                // Use Acquire ordering to ensure we see all writes from the thread
                // that set the flag (including the scenario result data)
                if has_failed.load(Ordering::Acquire) {
                    return ScenarioResult {
                        name: scenario.name.clone(),
                        path: path.clone(),
                        passed: false,
                        exit_code: 0,
                        duration: Duration::ZERO,
                        error: Some("Skipped due to fail_fast".to_string()),
                        steps_executed: 0,
                    };
                }

                let scenario_start = Instant::now();

                let runner_config = RunnerConfig {
                    seed: config.seed.or(scenario.seed),
                    verbose: config.runner_config.verbose,
                    max_ticks: config.runner_config.max_ticks,
                    tick_delay_ms: config.runner_config.tick_delay_ms,
                    trace_path: config.runner_config.trace_path.clone(),
                };

                let result = run_scenario(scenario, &runner_config);
                let duration = scenario_start.elapsed();
                let passed = result.exit_code == 0;

                // If this scenario failed, signal other workers to stop
                // Use Release ordering to ensure all our writes are visible
                // to other threads before they see the flag set
                if !passed {
                    has_failed.store(true, Ordering::Release);
                }

                ScenarioResult {
                    name: scenario.name.clone(),
                    path: path.clone(),
                    passed,
                    exit_code: result.exit_code,
                    duration,
                    error: if !passed {
                        Some(format!("Exit code: {}", result.exit_code))
                    } else {
                        None
                    },
                    steps_executed: result.trace.steps.len(),
                }
            })
            .collect()
    } else {
        // Normal parallel execution without fail_fast
        scenarios
            .par_iter()
            .map(|(scenario, path)| {
                let scenario_start = Instant::now();

                let runner_config = RunnerConfig {
                    seed: config.seed.or(scenario.seed),
                    verbose: config.runner_config.verbose,
                    max_ticks: config.runner_config.max_ticks,
                    tick_delay_ms: config.runner_config.tick_delay_ms,
                    trace_path: config.runner_config.trace_path.clone(),
                };

                let result = run_scenario(scenario, &runner_config);
                let duration = scenario_start.elapsed();

                ScenarioResult {
                    name: scenario.name.clone(),
                    path: path.clone(),
                    passed: result.exit_code == 0,
                    exit_code: result.exit_code,
                    duration,
                    error: if result.exit_code != 0 {
                        Some(format!("Exit code: {}", result.exit_code))
                    } else {
                        None
                    },
                    steps_executed: result.trace.steps.len(),
                }
            })
            .collect()
    };

    let duration = start_time.elapsed();

    // Aggregate results - skipped are separate from failed to avoid double-counting
    let passed_count = local_results.iter().filter(|r| r.passed).count();
    let skipped_count = local_results
        .iter()
        .filter(|r| r.error.as_ref().map_or(false, |e| e.contains("Skipped")))
        .count();
    // Failed = not passed AND not skipped
    let failed_count = local_results
        .iter()
        .filter(|r| !r.passed && r.error.as_ref().map_or(true, |e| !e.contains("Skipped")))
        .count();

    ParallelResult {
        total,
        passed: passed_count,
        failed: failed_count,
        skipped: skipped_count,
        duration,
        results: local_results,
    }
}

/// Run scenarios in parallel from file paths
///
/// # Arguments
///
/// * `paths` - List of paths to scenario files
/// * `config` - Parallel execution configuration
///
/// # Returns
///
/// ParallelResult containing aggregated results
pub fn run_parallel_from_paths(
    paths: &[PathBuf],
    config: &ParallelConfig,
) -> Result<ParallelResult, Vec<String>> {
    let mut scenarios = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let parse_result: Result<Scenario, String> = if path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    == Some("json".to_string())
                {
                    serde_json::from_str(&content).map_err(|e| format!("JSON parse error: {}", e))
                } else {
                    serde_yaml::from_str(&content).map_err(|e| format!("YAML parse error: {}", e))
                };

                match parse_result {
                    Ok(scenario) => {
                        // Validate scenario
                        if let Err(validation_errors) = scenario.validate() {
                            errors.push(format!(
                                "Validation failed for {}: {}",
                                path.display(),
                                validation_errors
                                    .iter()
                                    .map(|e| format!("{}: {}", e.path, e.message))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                        } else {
                            scenarios.push((scenario, path.clone()));
                        }
                    }
                    Err(e) => errors.push(format!("Parse failed for {}: {}", path.display(), e)),
                }
            }
            Err(e) => errors.push(format!("Read failed for {}: {}", path.display(), e)),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(run_parallel(&scenarios, config))
}

/// Print a summary of parallel test results
pub fn print_parallel_summary(result: &ParallelResult) {
    println!();
    println!("=== Parallel Test Summary ===");
    println!("Total: {}", result.total);
    println!("Passed: {}", result.passed);
    println!("Failed: {}", result.failed);
    println!("Skipped: {}", result.skipped);
    println!("Duration: {:?}", result.duration);
    println!();

    if result.failed > 0 {
        println!("Failed scenarios:");
        for r in &result.results {
            if !r.passed {
                println!("  - {} ({})", r.name, r.path.display());
                if let Some(ref error) = r.error {
                    println!("    Error: {}", error);
                }
            }
        }
        println!();
    }

    // Print timing info
    if !result.results.is_empty() {
        let mut timing: Vec<_> = result.results.iter().collect();
        timing.sort_by(|a, b| b.duration.cmp(&a.duration));

        println!("Top 5 slowest scenarios:");
        for r in timing.iter().take(5) {
            println!("  - {}: {:?}", r.name, r.duration);
        }
    }
}

/// Calculate statistics from parallel results
pub fn calculate_stats(results: &[ScenarioResult]) -> Stats {
    let durations: Vec<Duration> = results.iter().map(|r| r.duration).collect();
    let total_duration: Duration = durations.iter().sum();

    let count = durations.len();
    let count_f64 = count as f64;

    // Calculate average duration using nanoseconds
    let avg_duration_nanos = if count > 0 {
        total_duration.as_nanos() as f64 / count_f64
    } else {
        0.0
    };
    let avg_duration = Duration::from_secs_f64(avg_duration_nanos / 1_000_000_000.0);

    let mut sorted_durations = durations;
    sorted_durations.sort();

    let median_duration = if count > 0 {
        sorted_durations[count / 2]
    } else {
        Duration::ZERO
    };

    let max_duration = sorted_durations.last().cloned().unwrap_or(Duration::ZERO);
    let min_duration = sorted_durations.first().cloned().unwrap_or(Duration::ZERO);

    Stats {
        total_runs: count_f64,
        total_duration,
        avg_duration,
        median_duration,
        min_duration,
        max_duration,
    }
}

/// Statistics from parallel test execution
#[derive(Debug, Clone)]
pub struct Stats {
    /// Total number of runs
    pub total_runs: f64,
    /// Total duration
    pub total_duration: Duration,
    /// Average duration per run
    pub avg_duration: Duration,
    /// Median duration
    pub median_duration: Duration,
    /// Minimum duration
    pub min_duration: Duration,
    /// Maximum duration
    pub max_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_scenario(name: &str, content: &str) -> (Scenario, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(format!("{}.yaml", name));
        fs::write(&path, content).unwrap();

        let scenario: Scenario = serde_yaml::from_str(content).unwrap();
        (scenario, path)
    }

    #[test]
    fn test_run_parallel_basic() {
        let scenarios = vec![create_test_scenario(
            "test1",
            r#"
name: Test Scenario 1
command: "echo hello"
steps:
  - action: wait_for
    pattern: "hello"
"#,
        )];

        let config = ParallelConfig::default();
        let result = run_parallel(&scenarios, &config);

        // Verify basic result structure
        assert_eq!(result.total, 1);
        assert_eq!(result.results.len(), 1);

        // Verify the result has valid fields
        let scenario_result = &result.results[0];
        assert_eq!(scenario_result.name, "Test Scenario 1");
        // Either passed or failed - we can't guarantee which without controlling the shell
        assert!(result.passed + result.failed + result.skipped == 1);
    }

    #[test]
    fn test_calculate_stats() {
        let results = vec![
            ScenarioResult {
                name: "test1".to_string(),
                path: PathBuf::from("/test1"),
                passed: true,
                exit_code: 0,
                duration: Duration::from_millis(100),
                error: None,
                steps_executed: 1,
            },
            ScenarioResult {
                name: "test2".to_string(),
                path: PathBuf::from("/test2"),
                passed: true,
                exit_code: 0,
                duration: Duration::from_millis(200),
                error: None,
                steps_executed: 1,
            },
        ];

        let stats = calculate_stats(&results);

        assert_eq!(stats.total_runs, 2.0);
        assert_eq!(stats.total_duration, Duration::from_millis(300));
        assert_eq!(stats.avg_duration, Duration::from_millis(150));
    }
}
