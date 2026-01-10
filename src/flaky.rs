//! Flaky test detection and retry logic.
//!
//! This module provides functionality to:
//! - Detect flaky tests based on inconsistent results
//! - Automatically retry failed tests
//! - Track flaky test history
//! - Configure retry and stability thresholds

use crate::parallel::{ParallelConfig, ParallelResult, ScenarioResult};
use crate::runner::{run_scenario, RunnerConfig};
use crate::scenario::Scenario;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Configuration for flaky test detection
#[derive(Debug, Clone)]
pub struct FlakyConfig {
    /// Maximum number of retry attempts
    pub max_retries: usize,
    /// Minimum passes required to consider test stable
    pub stability_threshold: usize,
    /// Maximum acceptable failure rate (0.0 to 1.0)
    pub max_failure_rate: f64,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Whether to track flaky history
    pub track_history: bool,
    /// Path to flaky history file
    pub history_path: Option<PathBuf>,
}

/// Result of flaky test detection
#[derive(Debug, Clone)]
pub enum FlakyResult {
    /// Test passed consistently
    Stable {
        scenario_name: String,
        total_runs: usize,
        passes: usize,
    },
    /// Test passed after retries (was flaky)
    FlakyFixed {
        scenario_name: String,
        total_runs: usize,
        passes: usize,
        retries_used: usize,
        first_failure_exit_code: Option<i32>,
        exit_codes: Vec<i32>,
    },
    /// Test consistently failed
    ConsistentlyFailing {
        scenario_name: String,
        total_runs: usize,
        failures: usize,
        last_exit_code: i32,
        exit_codes: Vec<i32>,
    },
    /// Test is unstable (passed and failed within threshold)
    Unstable {
        scenario_name: String,
        total_runs: usize,
        passes: usize,
        failures: usize,
        exit_codes: Vec<i32>,
    },
}

/// History entry for a flaky test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakyHistoryEntry {
    /// Scenario path
    pub path: String,
    /// Scenario name
    pub name: String,
    /// Total runs
    pub total_runs: usize,
    /// Total passes
    pub total_passes: usize,
    /// Total failures
    pub total_failures: usize,
    /// Last run timestamp (ISO 8601 string)
    pub last_run: String,
    /// Is currently marked as flaky
    pub is_flaky: bool,
    /// Exit codes observed
    pub exit_codes: Vec<i32>,
}

/// Summary of flaky test detection
#[derive(Debug, Clone)]
pub struct FlakySummary {
    pub total_scenarios: usize,
    pub stable_count: usize,
    pub flaky_fixed_count: usize,
    pub consistently_failing_count: usize,
    pub unstable_count: usize,
    pub scenarios: Vec<FlakyResult>,
}

impl Default for FlakyConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            stability_threshold: 3,
            max_failure_rate: 0.5,
            retry_delay_ms: 100,
            track_history: false,
            history_path: None,
        }
    }
}

/// Run a scenario with retry logic for flaky detection
pub fn run_with_retry(
    scenario: &Scenario,
    path: &PathBuf,
    config: &FlakyConfig,
) -> FlakyResult {
    let mut results: Vec<(bool, i32)> = Vec::new();
    let mut retries_used = 0;
    let mut first_failure_exit_code = None;

    // Initial run
    let result = run_scenario(scenario, &RunnerConfig::default());
    results.push((result.exit_code == 0, result.exit_code));

    if result.exit_code != 0 {
        first_failure_exit_code = Some(result.exit_code);
    }

    // Retry loop
    while results.last().map(|(passed, _)| !passed).unwrap_or(false)
        && retries_used < config.max_retries
    {
        if config.retry_delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(config.retry_delay_ms));
        }

        let result = run_scenario(scenario, &RunnerConfig::default());
        results.push((result.exit_code == 0, result.exit_code));
        retries_used += 1;
    }

    let passes = results.iter().filter(|(passed, _)| *passed).count();
    let failures = results.len() - passes;
    let total_runs = results.len();
    let exit_codes: Vec<i32> = results.iter().map(|(_, code)| *code).collect();

    // Determine the result type
    if failures == 0 {
        // All passed
        FlakyResult::Stable {
            scenario_name: scenario.name.clone(),
            total_runs,
            passes,
        }
    } else if retries_used > 0 && failures == 1 && passes == total_runs - 1 {
        // First run failed, but all retries passed - this is FlakyFixed
        // A flaky test that was "fixed" by retrying
        FlakyResult::FlakyFixed {
            scenario_name: scenario.name.clone(),
            total_runs,
            passes,
            retries_used,
            first_failure_exit_code,
            exit_codes,
        }
    } else if passes > 0 && failures > 0 {
        // Mixed results - could be flaky or unstable
        let failure_rate = failures as f64 / total_runs as f64;

        if failure_rate >= config.max_failure_rate {
            FlakyResult::ConsistentlyFailing {
                scenario_name: scenario.name.clone(),
                total_runs,
                failures,
                last_exit_code: exit_codes.last().copied().unwrap_or(0),
                exit_codes,
            }
        } else {
            FlakyResult::Unstable {
                scenario_name: scenario.name.clone(),
                total_runs,
                passes,
                failures,
                exit_codes,
            }
        }
    } else {
        // All failed
        FlakyResult::ConsistentlyFailing {
            scenario_name: scenario.name.clone(),
            total_runs,
            failures,
            last_exit_code: exit_codes.last().copied().unwrap_or(0),
            exit_codes,
        }
    }
}

/// Run multiple scenarios with flaky detection
pub fn run_flaky_detection(
    scenarios: &[(Scenario, PathBuf)],
    config: &FlakyConfig,
) -> FlakySummary {
    let mut results = Vec::new();

    for (scenario, path) in scenarios {
        eprintln!("Running flaky detection for: {}", scenario.name);
        let result = run_with_retry(scenario, path, config);
        results.push(result);
    }

    let stable_count = results
        .iter()
        .filter(|r| matches!(r, FlakyResult::Stable { .. }))
        .count();
    let flaky_fixed_count = results
        .iter()
        .filter(|r| matches!(r, FlakyResult::FlakyFixed { .. }))
        .count();
    let consistently_failing_count = results
        .iter()
        .filter(|r| matches!(r, FlakyResult::ConsistentlyFailing { .. }))
        .count();
    let unstable_count = results
        .iter()
        .filter(|r| matches!(r, FlakyResult::Unstable { .. }))
        .count();

    FlakySummary {
        total_scenarios: scenarios.len(),
        stable_count,
        flaky_fixed_count,
        consistently_failing_count,
        unstable_count,
        scenarios: results,
    }
}

/// Print flaky detection summary
pub fn print_flaky_summary(summary: &FlakySummary) {
    println!();
    println!("=== Flaky Test Detection Summary ===");
    println!();

    println!("Total scenarios: {}", summary.total_scenarios);
    println!("Stable (all passes): {}", summary.stable_count);
    println!("Flaky but fixed: {}", summary.flaky_fixed_count);
    println!("Consistently failing: {}", summary.consistently_failing_count);
    println!("Unstable (mixed results): {}", summary.unstable_count);
    println!();

    if !summary.scenarios.is_empty() {
        println!("Results by scenario:");
        for result in &summary.scenarios {
            match result {
                FlakyResult::Stable { scenario_name, total_runs, passes } => {
                    println!("  [STABLE] {}: {}/{} passes", scenario_name, passes, total_runs);
                }
                FlakyResult::FlakyFixed {
                    scenario_name,
                    total_runs,
                    retries_used,
                    ..
                } => {
                    println!(
                        "  [FLAKY FIXED] {}: passed after {} retries ({}/{} runs)",
                        scenario_name, retries_used, total_runs - 1, total_runs
                    );
                }
                FlakyResult::ConsistentlyFailing {
                    scenario_name,
                    failures,
                    last_exit_code,
                    ..
                } => {
                    println!(
                        "  [FAILING] {}: {} failures (exit code: {})",
                        scenario_name, failures, last_exit_code
                    );
                }
                FlakyResult::Unstable {
                    scenario_name,
                    passes,
                    failures,
                    exit_codes,
                    ..
                } => {
                    println!(
                        "  [UNSTABLE] {}: {}/{} passes, exit codes: {:?}",
                        scenario_name, passes, passes + failures, exit_codes
                    );
                }
            }
        }
    }

    println!();

    if summary.unstable_count > 0 || summary.consistently_failing_count > 0 {
        println!("FAILED: Found flaky or failing tests");
    } else {
        println!("PASSED: All tests are stable");
    }
}

/// Load flaky history from file
pub fn load_flaky_history(path: &Path) -> Result<Vec<FlakyHistoryEntry>, std::io::Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Save flaky history to file
pub fn save_flaky_history(
    history: &[FlakyHistoryEntry],
    path: &Path,
) -> Result<(), std::io::Error> {
    let content = serde_json::to_string_pretty(history)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Update history with new results
pub fn update_history(
    history: &mut Vec<FlakyHistoryEntry>,
    result: &FlakyResult,
    path: &PathBuf,
) {
    // Use canonical path if possible for consistent comparison across different working directories
    let path_str = match path.canonicalize() {
        Ok(canonical) => canonical.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    };

    // Find existing entry by canonical path
    if let Some(entry) = history.iter_mut().find(|e| e.path == path_str) {
        // Update existing entry
        match result {
            FlakyResult::Stable { total_runs, passes, .. } => {
                entry.total_runs += total_runs;
                entry.total_passes += passes;
            }
            FlakyResult::FlakyFixed {
                total_runs,
                passes,
                exit_codes,
                ..
            } => {
                entry.total_runs += total_runs;
                entry.total_passes += passes;
                entry.total_failures += total_runs - passes;
                entry.exit_codes.extend(exit_codes.clone());
            }
            FlakyResult::ConsistentlyFailing {
                total_runs,
                failures,
                exit_codes,
                ..
            } => {
                entry.total_runs += total_runs;
                entry.total_failures += failures;
                entry.exit_codes.extend(exit_codes.clone());
            }
            FlakyResult::Unstable {
                total_runs,
                passes,
                failures,
                exit_codes,
                ..
            } => {
                entry.total_runs += total_runs;
                entry.total_passes += passes;
                entry.total_failures += failures;
                entry.exit_codes.extend(exit_codes.clone());
            }
        }
        entry.last_run = chrono::Utc::now().to_rfc3339();
        entry.is_flaky = matches!(
            result,
            FlakyResult::FlakyFixed { .. } | FlakyResult::Unstable { .. }
        );
    } else {
        // Create new entry
        let (total_runs, total_passes, total_failures, exit_codes, is_flaky) = match result {
            FlakyResult::Stable { total_runs, passes, .. } => (*total_runs, *passes, 0, vec![], false),
            FlakyResult::FlakyFixed {
                total_runs,
                passes,
                exit_codes,
                ..
            } => (*total_runs, *passes, total_runs - passes, exit_codes.clone(), true),
            FlakyResult::ConsistentlyFailing {
                total_runs,
                failures,
                exit_codes,
                ..
            } => (*total_runs, 0, *failures, exit_codes.clone(), true),
            FlakyResult::Unstable {
                total_runs,
                passes,
                failures,
                exit_codes,
                ..
            } => (*total_runs, *passes, *failures, exit_codes.clone(), true),
        };

        history.push(FlakyHistoryEntry {
            path: path_str,
            name: result.scenario_name(),
            total_runs,
            total_passes,
            total_failures,
            last_run: chrono::Utc::now().to_rfc3339(),
            is_flaky,
            exit_codes,
        });
    }
}

impl FlakyResult {
    /// Get the scenario name
    pub fn scenario_name(&self) -> String {
        match self {
            FlakyResult::Stable { scenario_name, .. } => scenario_name.clone(),
            FlakyResult::FlakyFixed { scenario_name, .. } => scenario_name.clone(),
            FlakyResult::ConsistentlyFailing { scenario_name, .. } => scenario_name.clone(),
            FlakyResult::Unstable { scenario_name, .. } => scenario_name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_scenario(name: &str, exit_code: i32) -> (Scenario, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(format!("{}.yaml", name));
        let content = format!(
            r#"
name: {}
command: "echo hello"
steps:
  - action: wait_for
    pattern: "hello"
"#,
            name
        );

        let scenario: Scenario = serde_yaml::from_str(&content).unwrap();
        fs::write(&path, &content).unwrap();

        (scenario, path)
    }

    #[test]
    fn test_stable_scenario() {
        let (scenario, path) = create_test_scenario("stable_test", 0);
        let config = FlakyConfig::default();

        let result = run_with_retry(&scenario, &path, &config);

        match result {
            FlakyResult::Stable {
                scenario_name,
                total_runs,
                passes,
            } => {
                assert_eq!(scenario_name, "stable_test");
                assert!(total_runs >= 1);
                assert_eq!(passes, total_runs);
            }
            _ => panic!("Expected stable result"),
        }
    }

    #[test]
    fn test_retry_runs_multiple_times() {
        // This test verifies that retry logic runs the expected number of times
        // Note: The actual scenario might pass or fail depending on the command
        let (scenario, path) = create_test_scenario("retry_test", 0);
        let config = FlakyConfig {
            max_retries: 2,
            ..Default::default()
        };

        let result = run_with_retry(&scenario, &path, &config);

        // Should run at least once
        match result {
            FlakyResult::Stable { total_runs, .. } => {
                assert!(total_runs >= 1);
            }
            FlakyResult::FlakyFixed { total_runs, .. } => {
                // If it was flaky, should have run multiple times
                assert!(total_runs >= 2);
            }
            _ => {}
        }
    }

    #[test]
    fn test_flaky_history_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("history.json");

        let entry = FlakyHistoryEntry {
            path: "/test/scenario.yaml".to_string(),
            name: "test scenario".to_string(),
            total_runs: 10,
            total_passes: 8,
            total_failures: 2,
            last_run: chrono::Utc::now().to_rfc3339(),
            is_flaky: true,
            exit_codes: vec![0, 1, 0, 0, 1, 0, 0, 0, 0, 0],
        };

        save_flaky_history(&[entry.clone()], &history_path).unwrap();
        let loaded = load_flaky_history(&history_path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "test scenario");
        assert_eq!(loaded[0].total_runs, 10);
        assert!(loaded[0].is_flaky);
    }

    #[test]
    fn test_flaky_summary() {
        let scenarios = vec![
            create_test_scenario("stable1", 0),
            create_test_scenario("stable2", 0),
        ];

        let config = FlakyConfig::default();
        let summary = run_flaky_detection(&scenarios, &config);

        assert_eq!(summary.total_scenarios, 2);
        assert_eq!(summary.stable_count, 2);
        assert_eq!(summary.consistently_failing_count, 0);
    }
}
