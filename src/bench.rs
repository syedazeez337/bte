//! Performance benchmarking for BTE scenarios.
//!
//! This module provides functionality to:
//! - Run benchmarks on scenarios
//! - Store and compare against baselines
//! - Detect performance regressions
//! - Generate performance reports

// Benchmarking requires real-time measurement
#![allow(clippy::disallowed_types)]

use crate::runner::{run_scenario, RunnerConfig};
use crate::scenario::Scenario;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn get_rust_version() -> String {
    // Use the version from cargo package
    option_env!("CARGO_PKG_VERSION")
        .map(|s| format!("rust {}", s))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations to run for averaging
    pub iterations: usize,
    /// Warmup iterations (not counted)
    pub warmup_iterations: usize,
    /// Maximum acceptable variance (coefficient of variation)
    pub max_cv: f64,
    /// Path to baseline metrics file
    pub baseline_path: Option<PathBuf>,
    /// Regression tolerance (percentage)
    pub regression_tolerance: f64,
    /// Output directory for reports
    pub report_dir: Option<PathBuf>,
}

/// Result of a single benchmark iteration
#[derive(Debug, Clone)]
pub struct BenchmarkIteration {
    pub duration: Duration,
    pub exit_code: i32,
    pub steps_executed: usize,
    pub ticks: u64,
}

/// Result of a benchmark run
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Scenario name
    pub name: String,
    /// Scenario path
    pub path: PathBuf,
    /// Number of iterations run
    pub iterations: usize,
    /// Mean duration
    pub mean_duration: Duration,
    /// Standard deviation
    pub std_dev: Duration,
    /// Coefficient of variation
    pub cv: f64,
    /// Min duration
    pub min_duration: Duration,
    /// Max duration
    pub max_duration: Duration,
    /// Percentiles
    pub p50: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    /// All iterations
    pub iterations_data: Vec<BenchmarkIteration>,
    /// Whether this is a regression vs baseline
    pub is_regression: bool,
    /// Regression details if applicable
    pub regression_details: Option<RegressionDetails>,
}

/// Details about a performance regression
#[derive(Debug, Clone)]
pub struct RegressionDetails {
    /// Baseline mean duration
    pub baseline_mean: Duration,
    /// Current mean duration
    pub current_mean: Duration,
    /// Percentage change
    pub percentage_change: f64,
    /// Absolute change
    pub absolute_change: Duration,
    /// Threshold that was exceeded
    pub threshold: f64,
}

/// Stored baseline metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct BaselineMetrics {
    /// Scenario name
    pub name: String,
    /// Scenario path hash
    pub path_hash: String,
    /// Mean duration
    pub mean_duration_ns: u64,
    /// Std dev in ns
    pub std_dev_ns: u64,
    /// Coefficient of variation
    pub cv: f64,
    /// Min duration in ns
    pub min_duration_ns: u64,
    /// Max duration in ns
    pub max_duration_ns: u64,
    /// Timestamp of when baseline was recorded (ISO 8601 string)
    pub timestamp: String,
    /// Rust version used
    pub rust_version: String,
    /// Platform info
    pub platform: String,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 10,
            warmup_iterations: 3,
            max_cv: 0.1, // 10% max coefficient of variation
            baseline_path: None,
            regression_tolerance: 5.0, // 5% tolerance by default
            report_dir: None,
        }
    }
}

/// Run a benchmark on a single scenario
pub fn benchmark_scenario(
    scenario: &Scenario,
    path: &Path,
    config: &BenchmarkConfig,
) -> BenchmarkResult {
    let mut iterations_data = Vec::with_capacity(config.iterations);
    let mut durations = Vec::with_capacity(config.iterations);

    // Warmup iterations - use random seed for warmup to avoid caching effects
    for _ in 0..config.warmup_iterations {
        let runner_config = RunnerConfig {
            seed: Some(fastrand::u64(..)),
            ..RunnerConfig::default()
        };
        let _ = run_scenario(scenario, &runner_config);
    }

    // Benchmark iterations - use scenario seed if available for determinism, otherwise random
    for i in 0..config.iterations {
        let seed = scenario.seed.unwrap_or_else(|| fastrand::u64(..));
        let runner_config = RunnerConfig {
            seed: Some(seed),
            ..RunnerConfig::default()
        };

        let start = Instant::now();
        let result = run_scenario(scenario, &runner_config);
        let duration = start.elapsed();

        let iteration = BenchmarkIteration {
            duration,
            exit_code: result.exit_code,
            steps_executed: result.trace.steps.len(),
            ticks: result.trace.total_ticks,
        };

        iterations_data.push(iteration);
        durations.push(duration);

        if config.iterations > 10 && i % 10 == 0 {
            eprintln!("  Iteration {}/{}", i + 1, config.iterations);
        }
    }

    // Calculate statistics
    durations.sort();
    let count = durations.len();
    let count_f64 = count as f64;

    let min_duration = durations.first().cloned().unwrap_or(Duration::ZERO);
    let max_duration = durations.last().cloned().unwrap_or(Duration::ZERO);

    let total_duration: Duration = durations.iter().sum();
    // Calculate mean duration using nanoseconds
    let mean_duration_nanos = if count > 0 {
        total_duration.as_nanos() as f64 / count_f64
    } else {
        0.0
    };
    let mean_duration = Duration::from_secs_f64(mean_duration_nanos / 1_000_000_000.0);

    // Calculate std dev
    let variance: f64 = durations
        .iter()
        .map(|d| {
            let diff = d.as_secs_f64() - mean_duration.as_secs_f64();
            diff * diff
        })
        .sum();
    let std_dev = Duration::from_secs_f64((variance / count_f64).sqrt());

    // Coefficient of variation
    let cv = if mean_duration.as_secs_f64() > 0.0 {
        std_dev.as_secs_f64() / mean_duration.as_secs_f64()
    } else {
        0.0
    };

    // Calculate percentiles with safe bounds checking
    // Uses linear interpolation for more accurate percentiles with small samples
    let percentile = |p: f64| -> Duration {
        if count == 0 {
            return Duration::ZERO;
        }
        if count == 1 {
            return durations[0];
        }

        // Use linear interpolation between ranks
        // rank = p * (count - 1) for 0-indexed ranks
        let rank = p * (count as f64 - 1.0);
        let lower = rank.floor() as usize;
        let upper = (lower + 1).min(count - 1);
        let frac = rank - lower as f64;

        if lower == upper {
            durations[lower]
        } else {
            // Linear interpolation between lower and upper
            let lower_secs = durations[lower].as_secs_f64();
            let upper_secs = durations[upper].as_secs_f64();
            Duration::from_secs_f64(lower_secs + frac * (upper_secs - lower_secs))
        }
    };

    let p50 = percentile(0.50);
    let p90 = percentile(0.90);
    let p95 = percentile(0.95);
    let p99 = percentile(0.99);

    // Check for regression against baseline
    let (is_regression, regression_details) =
        check_regression(&scenario.name, path, mean_duration, config);

    BenchmarkResult {
        name: scenario.name.clone(),
        path: path.to_path_buf(),
        iterations: config.iterations,
        mean_duration,
        std_dev,
        cv,
        min_duration,
        max_duration,
        p50,
        p90,
        p95,
        p99,
        iterations_data,
        is_regression,
        regression_details,
    }
}

/// Check if current results indicate a regression vs baseline
fn check_regression(
    scenario_name: &str,
    path: &Path,
    current_mean: Duration,
    config: &BenchmarkConfig,
) -> (bool, Option<RegressionDetails>) {
    if let Some(baseline_path) = &config.baseline_path {
        if baseline_path.exists() {
            if let Ok(baselines) = load_baselines(baseline_path) {
                // Calculate path hash for matching
                let path_hash = format!("{:x}", seahash::hash(path.to_string_lossy().as_bytes()));

                // Find matching baseline by name or path hash
                for baseline in baselines {
                    // Match by scenario name or path hash
                    if baseline.name == scenario_name || baseline.path_hash == path_hash {
                        let baseline_mean = Duration::from_nanos(baseline.mean_duration_ns);
                        let percentage_change = if baseline_mean.as_secs_f64() > 0.0 {
                            ((current_mean.as_secs_f64() - baseline_mean.as_secs_f64())
                                / baseline_mean.as_secs_f64())
                                * 100.0
                        } else {
                            0.0
                        };

                        let threshold = config.regression_tolerance;
                        if percentage_change.abs() > threshold {
                            // Use saturating_sub to handle the case where performance improved
                            let absolute_change = current_mean.saturating_sub(baseline_mean);
                            return (
                                true,
                                Some(RegressionDetails {
                                    baseline_mean,
                                    current_mean,
                                    percentage_change,
                                    absolute_change,
                                    threshold,
                                }),
                            );
                        }
                        // Found matching baseline, no regression
                        return (false, None);
                    }
                }
            }
        }
    }

    (false, None)
}

/// Load baseline metrics from file
pub fn load_baselines(path: &Path) -> Result<Vec<BaselineMetrics>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Save baseline metrics to file
pub fn save_baselines(baselines: &[BaselineMetrics], path: &Path) -> Result<(), std::io::Error> {
    let content = serde_json::to_string_pretty(baselines)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Create a baseline from benchmark results
pub fn create_baseline(result: &BenchmarkResult) -> BaselineMetrics {
    BaselineMetrics {
        name: result.name.clone(),
        path_hash: format!(
            "{:x}",
            seahash::hash(result.path.to_string_lossy().as_bytes())
        ),
        mean_duration_ns: result.mean_duration.as_nanos() as u64,
        std_dev_ns: result.std_dev.as_nanos() as u64,
        cv: result.cv,
        min_duration_ns: result.min_duration.as_nanos() as u64,
        max_duration_ns: result.max_duration.as_nanos() as u64,
        timestamp: chrono::Utc::now().to_rfc3339(),
        rust_version: get_rust_version(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    }
}

/// Print benchmark summary
pub fn print_benchmark_summary(results: &[BenchmarkResult]) {
    println!();
    println!("=== Benchmark Summary ===");
    println!();

    for result in results {
        println!("{}:", result.name);
        println!("  Mean: {:?}", result.mean_duration);
        println!("  StdDev: {:?}", result.std_dev);
        println!("  CV: {:.2}%", result.cv * 100.0);
        println!("  Min: {:?}", result.min_duration);
        println!("  Max: {:?}", result.max_duration);
        println!("  P50: {:?}", result.p50);
        println!("  P99: {:?}", result.p99);

        if let Some(ref regression) = result.regression_details {
            println!(
                "  REGRESSION: {:.2}% slower than baseline",
                regression.percentage_change
            );
            println!("    Baseline: {:?}", regression.baseline_mean);
            println!("    Current: {:?}", regression.current_mean);
        } else {
            println!("  Status: OK");
        }

        if result.cv > 0.1 {
            println!("  WARNING: High variance (CV > 10%)");
        }

        println!();
    }

    // Summary
    let total_regressions = results.iter().filter(|r| r.is_regression).count();
    let high_variance = results.iter().filter(|r| r.cv > 0.1).count();

    println!("Total scenarios: {}", results.len());
    println!("Regressions: {}", total_regressions);
    println!("High variance: {}", high_variance);

    if total_regressions > 0 {
        println!("\nFAILED: Performance regressions detected");
    } else {
        println!("\nPASSED: No performance regressions");
    }
}

/// Run benchmarks on multiple scenarios
pub fn run_benchmarks(
    scenarios: &[(Scenario, PathBuf)],
    config: &BenchmarkConfig,
) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    for (scenario, path) in scenarios {
        eprintln!("Benchmarking: {}", scenario.name);
        let result = benchmark_scenario(scenario, path, config);
        results.push(result);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_scenario(name: &str) -> (Scenario, PathBuf) {
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
    fn test_benchmark_single_scenario() {
        let (scenario, path) = create_test_scenario("bench_test");
        let config = BenchmarkConfig {
            iterations: 3,
            warmup_iterations: 0,
            ..Default::default()
        };

        let result = benchmark_scenario(&scenario, &path, &config);

        assert_eq!(result.name, "bench_test");
        assert_eq!(result.iterations, 3);
        assert!(result.mean_duration > Duration::ZERO);
        assert!(!result.is_regression);
    }

    #[test]
    fn test_benchmark_statistics() {
        let (scenario, path) = create_test_scenario("bench_stats_test");
        let config = BenchmarkConfig {
            iterations: 5,
            warmup_iterations: 0,
            ..Default::default()
        };

        let result = benchmark_scenario(&scenario, &path, &config);

        assert!(result.std_dev <= result.mean_duration); // CV <= 1.0
        assert!(result.p50 >= result.min_duration);
        assert!(result.p99 <= result.max_duration);
    }

    #[test]
    fn test_baseline_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let baseline_path = temp_dir.path().join("baselines.json");

        let baseline = BaselineMetrics {
            name: "test".to_string(),
            path_hash: "abc123".to_string(),
            mean_duration_ns: 1_000_000,
            std_dev_ns: 100_000,
            cv: 0.1,
            min_duration_ns: 900_000,
            max_duration_ns: 1_100_000,
            timestamp: chrono::Utc::now().to_rfc3339(),
            rust_version: "1.0".to_string(),
            platform: "linux x86_64".to_string(),
        };

        save_baselines(&[baseline], &baseline_path).unwrap();
        let loaded = load_baselines(&baseline_path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "test");
        assert_eq!(loaded[0].mean_duration_ns, 1_000_000);
    }
}
