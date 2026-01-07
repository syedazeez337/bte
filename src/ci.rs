//! CI-First UX Module
//!
//! Provides machine-readable summaries, GitHub Actions templates,
//! and CI-friendly output for terminal testing workflows.
//!
//! # Features
//!
//! - Batch test result aggregation
//!- Machine-readable CI summaries
//! - GitHub Actions workflow template generation
//! - Failure artifact management
//! - Exit code mapping for CI integration

#![allow(dead_code)]

use crate::explain::{AIFailureOutput, FailureExplanation};
use crate::scenario::Scenario;
use crate::trace::Trace;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CITestResult {
    pub scenario: String,
    pub scenario_path: String,
    pub passed: bool,
    pub exit_code: i32,
    pub duration_ticks: u64,
    pub invariant_violations: Vec<String>,
    pub trace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CISummary {
    pub version: String,
    pub timestamp: String,
    pub total_scenarios: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub success_rate: f64,
    pub total_duration_ticks: u64,
    pub results: Vec<CITestResult>,
    pub failures: Vec<CIFailureSummary>,
    pub artifacts_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIFailureSummary {
    pub scenario: String,
    pub scenario_path: String,
    pub violation_type: String,
    pub severity: String,
    pub exit_code: i32,
    pub repro_seed: u64,
    pub trace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIBatchConfig {
    pub scenarios_dir: String,
    pub output_dir: String,
    pub parallel: bool,
    pub max_workers: usize,
    pub fail_fast: bool,
    pub generate_ai_reports: bool,
}

impl Default for CIBatchConfig {
    fn default() -> Self {
        Self {
            scenarios_dir: "scenarios".to_string(),
            output_dir: "test-results".to_string(),
            parallel: true,
            max_workers: 4,
            fail_fast: false,
            generate_ai_reports: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitHubActionsTemplate {
    pub name: String,
    pub on: Vec<String>,
    pub jobs: Vec<GitHubActionsJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitHubActionsJob {
    pub name: String,
    pub runs_on: String,
    pub steps: Vec<GitHubActionsStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitHubActionsStep {
    pub name: String,
    pub uses: Option<String>,
    pub run: Option<String>,
    pub with: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIExitCodeMap {
    pub success: i32,
    pub signal_exit: i32,
    pub invariant_violation: i32,
    pub timeout: i32,
    pub error: i32,
    pub replay_divergence: i32,
    pub panic: i32,
}

impl Default for CIExitCodeMap {
    fn default() -> Self {
        Self {
            success: 0,
            signal_exit: -1,
            invariant_violation: -2,
            timeout: -3,
            error: -4,
            replay_divergence: -5,
            panic: -99,
        }
    }
}

pub struct CIReportGenerator {
    pub config: CIBatchConfig,
    pub exit_codes: CIExitCodeMap,
}

impl CIReportGenerator {
    pub fn new(config: CIBatchConfig) -> Self {
        Self {
            config,
            exit_codes: CIExitCodeMap::default(),
        }
    }

    pub fn aggregate_results(&self, results: &[CITestResult]) -> CISummary {
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.iter().filter(|r| !r.passed).count();
        let total = results.len();
        let success_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let failures: Vec<CIFailureSummary> = results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| CIFailureSummary {
                scenario: r.scenario.clone(),
                scenario_path: r.scenario_path.clone(),
                violation_type: r.invariant_violations.first().cloned().unwrap_or_default(),
                severity: "unknown".to_string(),
                exit_code: r.exit_code,
                repro_seed: 0,
                trace_path: r.trace_path.clone().unwrap_or_default(),
            })
            .collect();

        let total_duration_ticks: u64 = results.iter().map(|r| r.duration_ticks).sum();

        CISummary {
            version: "1.0".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_scenarios: total,
            passed,
            failed,
            skipped: 0,
            success_rate,
            total_duration_ticks,
            results: results.to_vec(),
            failures,
            artifacts_dir: self.config.output_dir.clone(),
        }
    }

    pub fn trace_to_ci_result(
        &self,
        scenario: &Scenario,
        scenario_path: &str,
        trace: &Trace,
        trace_path: Option<String>,
    ) -> CITestResult {
        let (passed, exit_code) = self.trace_exit_status(trace);

        let invariant_violations: Vec<String> = trace
            .invariant_results
            .iter()
            .filter(|r| r.violation())
            .map(|r| r.name.clone())
            .collect();

        CITestResult {
            scenario: scenario.name.clone(),
            scenario_path: scenario_path.to_string(),
            passed,
            exit_code,
            duration_ticks: trace.total_ticks,
            invariant_violations,
            trace_path,
        }
    }

    fn trace_exit_status(&self, trace: &Trace) -> (bool, i32) {
        match &trace.outcome {
            crate::trace::TraceOutcome::Success { exit_code, .. } => (*exit_code == 0, *exit_code),
            crate::trace::TraceOutcome::InvariantViolation { .. } => {
                (false, self.exit_codes.invariant_violation)
            }
            crate::trace::TraceOutcome::Timeout { .. } => (false, self.exit_codes.timeout),
            crate::trace::TraceOutcome::Error { .. } => (false, self.exit_codes.error),
            crate::trace::TraceOutcome::Signaled { signal, .. } => (false, -signal),
            crate::trace::TraceOutcome::ReplayDivergence { .. } => {
                (false, self.exit_codes.replay_divergence)
            }
        }
    }

    pub fn generate_github_actions_template(&self) -> GitHubActionsTemplate {
        GitHubActionsTemplate {
            name: "Terminal Tests".to_string(),
            on: vec!["push".to_string(), "pull_request".to_string()],
            jobs: vec![GitHubActionsJob {
                name: "test".to_string(),
                runs_on: "ubuntu-latest".to_string(),
                steps: vec![
                    GitHubActionsStep {
                        name: "Checkout".to_string(),
                        uses: Some("actions/checkout@v4".to_string()),
                        run: None,
                        with: None,
                    },
                    GitHubActionsStep {
                        name: "Install Rust".to_string(),
                        uses: Some("actions-rs/toolchain@v1".to_string()),
                        run: None,
                        with: Some(HashMap::from([("toolchain".to_string(), "stable".to_string())])),
                    },
                    GitHubActionsStep {
                        name: "Build BTE".to_string(),
                        uses: None,
                        run: Some("cargo build --release".to_string()),
                        with: None,
                    },
                    GitHubActionsStep {
                        name: "Run Scenarios".to_string(),
                        uses: None,
                        run: Some(format!(
                            "for scenario in {}/*.yaml; do\n  ./target/release/bte run \"$scenario\" \\\n    --output \"{}/$(basename $scenario).json\"\ndone",
                            self.config.scenarios_dir, self.config.output_dir
                        )),
                        with: None,
                    },
                    GitHubActionsStep {
                        name: "Upload Results".to_string(),
                        uses: Some("actions/upload-artifact@v4".to_string()),
                        run: None,
                        with: Some(HashMap::from([
                            ("name".to_string(), "bte-results".to_string()),
                            ("path".to_string(), format!("{}/", self.config.output_dir)),
                        ])),
                    },
                    GitHubActionsStep {
                        name: "Check Failures".to_string(),
                        uses: None,
                        run: Some(format!(
                            "failed=$(find {} -name \"*.json\" -exec grep -l '\"exit_code\": -[0-9]*' {{}} \\; | wc -l)\nif [ $failed -gt 0 ]; then\n  echo \"::warning title=Failures Detected::$failed scenarios failed\"\n  exit 1\nfi",
                            self.config.output_dir
                        )),
                        with: None,
                    },
                ],
            }],
        }
    }

    pub fn generate_ai_summary(&self, failures: &[FailureExplanation]) -> AIFailureOutput {
        AIFailureOutput {
            version: "1.0".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            scenario: format!("batch_run_{}", self.config.scenarios_dir),
            exit_code: if failures.is_empty() { 0 } else { -2 },
            outcome: if failures.is_empty() {
                "AllPassed".to_string()
            } else {
                format!("{}Failures", failures.len())
            },
            failures: failures.to_vec(),
            summary: crate::explain::FailureSummary {
                total_failures: failures.len(),
                critical_count: failures
                    .iter()
                    .filter(|f| {
                        matches!(
                            f.violation.severity,
                            crate::explain::ViolationSeverity::Critical
                        )
                    })
                    .count(),
                high_count: failures
                    .iter()
                    .filter(|f| {
                        matches!(
                            f.violation.severity,
                            crate::explain::ViolationSeverity::High
                        )
                    })
                    .count(),
                categories: vec![],
                top_violation_types: vec![],
            },
        }
    }

    pub fn exit_code_for_outcome(&self, trace: &Trace) -> i32 {
        self.trace_exit_status(trace).1
    }

    pub fn is_success(&self, exit_code: i32) -> bool {
        exit_code == self.exit_codes.success
    }
}

pub fn find_scenarios(dir: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "yaml" || e == "yml" || e == "json")
                == Some(true)
            {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceOutcome;

    fn sample_ci_test_result() -> CITestResult {
        CITestResult {
            scenario: "test_scenario".to_string(),
            scenario_path: "scenarios/test.yaml".to_string(),
            passed: false,
            exit_code: -2,
            duration_ticks: 150,
            invariant_violations: vec!["cursor_bounds".to_string()],
            trace_path: Some("test-results/test.json".to_string()),
        }
    }

    fn sample_passed_ci_test_result() -> CITestResult {
        CITestResult {
            scenario: "passing_scenario".to_string(),
            scenario_path: "scenarios/pass.yaml".to_string(),
            passed: true,
            exit_code: 0,
            duration_ticks: 100,
            invariant_violations: vec![],
            trace_path: Some("test-results/pass.json".to_string()),
        }
    }

    #[test]
    fn ci_test_result_creation() {
        let result = sample_ci_test_result();
        assert_eq!(result.scenario, "test_scenario");
        assert!(!result.passed);
        assert_eq!(result.exit_code, -2);
    }

    #[test]
    fn ci_summary_aggregation() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let results = vec![sample_ci_test_result(), sample_passed_ci_test_result()];
        let summary = generator.aggregate_results(&results);

        assert_eq!(summary.total_scenarios, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert!((summary.success_rate - 50.0).abs() < 0.01);
    }

    #[test]
    fn ci_summary_empty_results() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let results: Vec<CITestResult> = vec![];
        let summary = generator.aggregate_results(&results);

        assert_eq!(summary.total_scenarios, 0);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.success_rate, 0.0);
    }

    #[test]
    fn ci_failure_summary_creation() {
        let failure = CIFailureSummary {
            scenario: "failing_test".to_string(),
            scenario_path: "scenarios/fail.yaml".to_string(),
            violation_type: "cursor_bounds".to_string(),
            severity: "critical".to_string(),
            exit_code: -2,
            repro_seed: 12345,
            trace_path: "test-results/fail.json".to_string(),
        };

        assert_eq!(failure.violation_type, "cursor_bounds");
        assert_eq!(failure.severity, "critical");
    }

    #[test]
    fn ci_batch_config_defaults() {
        let config = CIBatchConfig::default();
        assert_eq!(config.scenarios_dir, "scenarios");
        assert_eq!(config.output_dir, "test-results");
        assert!(config.parallel);
        assert_eq!(config.max_workers, 4);
    }

    #[test]
    fn ci_exit_code_map_defaults() {
        let codes = CIExitCodeMap::default();
        assert_eq!(codes.success, 0);
        assert_eq!(codes.invariant_violation, -2);
        assert_eq!(codes.timeout, -3);
        assert_eq!(codes.error, -4);
        assert_eq!(codes.replay_divergence, -5);
    }

    #[test]
    fn github_actions_template_creation() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let template = generator.generate_github_actions_template();

        assert_eq!(template.name, "Terminal Tests");
        assert!(template.on.contains(&"push".to_string()));
        assert_eq!(template.jobs.len(), 1);
        assert_eq!(template.jobs[0].name, "test");
        assert_eq!(template.jobs[0].runs_on, "ubuntu-latest");
    }

    #[test]
    fn github_actions_job_steps() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let template = generator.generate_github_actions_template();

        let steps = &template.jobs[0].steps;
        assert!(steps.len() >= 5);
        assert!(steps.iter().any(|s| s.name == "Checkout"));
        assert!(steps.iter().any(|s| s.name == "Build BTE"));
        assert!(steps.iter().any(|s| s.name == "Run Scenarios"));
    }

    #[test]
    fn is_success_check() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);

        assert!(generator.is_success(0));
        assert!(!generator.is_success(-1));
        assert!(!generator.is_success(-2));
    }

    #[test]
    fn exit_code_for_outcome_success() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let trace = Trace {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            seed: 12345,
            scenario: Scenario {
                name: "test".to_string(),
                description: "".to_string(),
                command: crate::scenario::Command::Simple("echo test".to_string()),
                terminal: Default::default(),
                env: Default::default(),
                steps: vec![],
                invariants: vec![],
                seed: Some(12345),
                timeout_ms: Some(30000),
            },
            initial_rng_state: 12345,
            steps: vec![],
            checkpoints: vec![],
            invariant_results: vec![],
            outcome: TraceOutcome::Success {
                exit_code: 0,
                total_ticks: 100,
            },
            final_screen_hash: None,
            total_ticks: 100,
        };

        assert_eq!(generator.exit_code_for_outcome(&trace), 0);
    }

    #[test]
    fn exit_code_for_outcome_violation() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let trace = Trace {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            seed: 12345,
            scenario: Scenario {
                name: "test".to_string(),
                description: "".to_string(),
                command: crate::scenario::Command::Simple("echo test".to_string()),
                terminal: Default::default(),
                env: Default::default(),
                steps: vec![],
                invariants: vec![],
                seed: Some(12345),
                timeout_ms: Some(30000),
            },
            initial_rng_state: 12345,
            steps: vec![],
            checkpoints: vec![],
            invariant_results: vec![],
            outcome: TraceOutcome::InvariantViolation {
                invariant_name: "cursor_bounds".to_string(),
                checkpoint_index: 0,
            },
            final_screen_hash: None,
            total_ticks: 100,
        };

        assert_eq!(generator.exit_code_for_outcome(&trace), -2);
    }

    #[test]
    fn find_scenarios_empty_dir() {
        let paths = find_scenarios("/nonexistent");
        assert!(paths.is_empty());
    }

    #[test]
    fn aggregate_multiple_failures() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let results = vec![
            sample_ci_test_result(),
            sample_ci_test_result(),
            sample_passed_ci_test_result(),
        ];
        let summary = generator.aggregate_results(&results);

        assert_eq!(summary.total_scenarios, 3);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 2);
        assert!((summary.success_rate - 33.33).abs() < 0.1);
        assert_eq!(summary.failures.len(), 2);
    }

    #[test]
    fn ci_summary_timestamp_format() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let results = vec![sample_passed_ci_test_result()];
        let summary = generator.aggregate_results(&results);

        assert!(summary.timestamp.contains("T"));
        assert!(summary.timestamp.ends_with("Z") || summary.timestamp.contains("+"));
    }

    #[test]
    fn trace_to_ci_result_success() {
        let config = CIBatchConfig::default();
        let generator = CIReportGenerator::new(config);
        let scenario = Scenario {
            name: "test".to_string(),
            description: "".to_string(),
            command: crate::scenario::Command::Simple("echo test".to_string()),
            terminal: Default::default(),
            env: Default::default(),
            steps: vec![],
            invariants: vec![],
            seed: Some(12345),
            timeout_ms: Some(30000),
        };
        let trace = Trace {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            seed: 12345,
            scenario: scenario.clone(),
            initial_rng_state: 12345,
            steps: vec![],
            checkpoints: vec![],
            invariant_results: vec![],
            outcome: TraceOutcome::Success {
                exit_code: 0,
                total_ticks: 100,
            },
            final_screen_hash: None,
            total_ticks: 100,
        };

        let result = generator.trace_to_ci_result(&scenario, "test.yaml", &trace, None);

        assert!(result.passed);
        assert_eq!(result.exit_code, 0);
        assert!(result.invariant_violations.is_empty());
    }
}
