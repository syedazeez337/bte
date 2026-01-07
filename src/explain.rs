//! AI-Consumable Failure Explanations
//!
//! Provides normalized, structured failure output that AI systems can easily
//! parse, reason about, and act upon.

#![allow(dead_code)]

use crate::invariants::InvariantResult;
use crate::trace::Trace;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViolationType {
    CursorBounds,
    ScreenSize,
    ViewportInvalid,
    ResponseTimeout,
    MaxLatencyExceeded,
    ScreenUnstable,
    ProgressNonMonotonic,
    ScreenMissingContent,
    ScreenForbiddenContent,
    PatternDetected,
    OutputGrowthExceeded,
    OutputAfterExit,
    ProcessNotClean,
    SignalNotHandled,
    CustomRegexMismatch,
    JsonPathMismatch,
    ReplayDivergence,
    Deadlock,
    Timeout,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ViolationSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ViolationCategory {
    Structural,
    Performance,
    Content,
    Lifecycle,
    Timing,
    Determinism,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    pub violation_type: ViolationType,
    pub severity: ViolationSeverity,
    pub category: ViolationCategory,
    pub description: String,
    pub details: Option<String>,
    pub step: usize,
    pub tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalEvent {
    pub event_type: String,
    pub tick: u64,
    pub consequence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MinimalReproduction {
    pub scenario_name: String,
    pub step_count: usize,
    pub seed: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestedFix {
    pub description: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelatedIssue {
    pub id: String,
    pub similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FailureExplanation {
    pub violation: Violation,
    pub causal_chain: Vec<CausalEvent>,
    pub minimal_repro: MinimalReproduction,
    pub suggested_fixes: Vec<SuggestedFix>,
    pub related_issues: Vec<RelatedIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FailureSummary {
    pub total_failures: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub categories: Vec<(String, usize)>,
    pub top_violation_types: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIFailureOutput {
    pub version: String,
    pub timestamp: String,
    pub scenario: String,
    pub exit_code: i32,
    pub outcome: String,
    pub failures: Vec<FailureExplanation>,
    pub summary: FailureSummary,
}

pub struct FailureExplainer;

impl FailureExplainer {
    pub fn new() -> Self {
        Self
    }

    pub fn explain_failure(&self, result: &InvariantResult, trace: &Trace) -> FailureExplanation {
        let violation = self.classify_violation(result);
        let causal_chain = self.build_causal_chain(result);
        let minimal_repro = self.synthesize_minimal_repro(trace);
        let suggested_fixes = self.suggest_fixes(&violation);
        let related_issues = self.find_related_issues(&violation);

        FailureExplanation {
            violation,
            causal_chain,
            minimal_repro,
            suggested_fixes,
            related_issues,
        }
    }

    fn classify_violation(&self, result: &InvariantResult) -> Violation {
        let (violation_type, category) = self.invariant_to_violation(&result.name);
        let severity = self.severity_for(&violation_type);

        Violation {
            violation_type,
            severity,
            category,
            description: result.description.clone(),
            details: result.details.clone(),
            step: result.step,
            tick: result.tick,
        }
    }

    fn invariant_to_violation(&self, name: &str) -> (ViolationType, ViolationCategory) {
        match name {
            "cursor_bounds" => (ViolationType::CursorBounds, ViolationCategory::Structural),
            "screen_size" => (ViolationType::ScreenSize, ViolationCategory::Structural),
            "viewport_valid" => (
                ViolationType::ViewportInvalid,
                ViolationCategory::Structural,
            ),
            "response_time" => (
                ViolationType::ResponseTimeout,
                ViolationCategory::Performance,
            ),
            "max_latency" => (
                ViolationType::MaxLatencyExceeded,
                ViolationCategory::Performance,
            ),
            "screen_stability" => (ViolationType::ScreenUnstable, ViolationCategory::Timing),
            "progress_monotonic" => (
                ViolationType::ProgressNonMonotonic,
                ViolationCategory::Lifecycle,
            ),
            "screen_contains" => (
                ViolationType::ScreenMissingContent,
                ViolationCategory::Content,
            ),
            "screen_not_contains" => (
                ViolationType::ScreenForbiddenContent,
                ViolationCategory::Content,
            ),
            "pattern_absence" => (ViolationType::PatternDetected, ViolationCategory::Content),
            "output_growth" => (
                ViolationType::OutputGrowthExceeded,
                ViolationCategory::Performance,
            ),
            "no_output_after_exit" => {
                (ViolationType::OutputAfterExit, ViolationCategory::Lifecycle)
            }
            "process_terminated_cleanly" => {
                (ViolationType::ProcessNotClean, ViolationCategory::Lifecycle)
            }
            "signal_handled_correctly" => (
                ViolationType::SignalNotHandled,
                ViolationCategory::Lifecycle,
            ),
            "custom_regex" => (
                ViolationType::CustomRegexMismatch,
                ViolationCategory::Content,
            ),
            "json_path" => (ViolationType::JsonPathMismatch, ViolationCategory::Content),
            _ => (ViolationType::Unknown, ViolationCategory::Unknown),
        }
    }

    fn severity_for(&self, violation_type: &ViolationType) -> ViolationSeverity {
        match violation_type {
            ViolationType::CursorBounds => ViolationSeverity::Critical,
            ViolationType::ScreenForbiddenContent => ViolationSeverity::Critical,
            ViolationType::Deadlock => ViolationSeverity::Critical,
            ViolationType::ScreenSize => ViolationSeverity::High,
            ViolationType::ViewportInvalid => ViolationSeverity::High,
            ViolationType::ScreenMissingContent => ViolationSeverity::High,
            ViolationType::PatternDetected => ViolationSeverity::High,
            ViolationType::ProcessNotClean => ViolationSeverity::High,
            ViolationType::SignalNotHandled => ViolationSeverity::High,
            ViolationType::ReplayDivergence => ViolationSeverity::High,
            ViolationType::ResponseTimeout => ViolationSeverity::Medium,
            ViolationType::MaxLatencyExceeded => ViolationSeverity::Medium,
            ViolationType::ProgressNonMonotonic => ViolationSeverity::Medium,
            ViolationType::OutputAfterExit => ViolationSeverity::Medium,
            ViolationType::Timeout => ViolationSeverity::Medium,
            ViolationType::CustomRegexMismatch => ViolationSeverity::Medium,
            ViolationType::JsonPathMismatch => ViolationSeverity::Medium,
            ViolationType::ScreenUnstable => ViolationSeverity::Low,
            ViolationType::OutputGrowthExceeded => ViolationSeverity::Low,
            ViolationType::Unknown => ViolationSeverity::Low,
        }
    }

    fn build_causal_chain(&self, result: &InvariantResult) -> Vec<CausalEvent> {
        let mut chain = Vec::new();

        chain.push(CausalEvent {
            event_type: "invariant_check".to_string(),
            tick: result.tick,
            consequence: format!("Invariant '{}' check failed", result.name),
        });

        if let Some(details) = &result.details {
            chain.push(CausalEvent {
                event_type: "violation_details".to_string(),
                tick: result.tick,
                consequence: format!("Violation: {}", details),
            });
        }

        chain
    }

    fn synthesize_minimal_repro(&self, trace: &Trace) -> MinimalReproduction {
        MinimalReproduction {
            scenario_name: trace.scenario.name.clone(),
            step_count: trace.steps.len(),
            seed: trace.seed,
            duration_ticks: trace.total_ticks,
        }
    }

    fn suggest_fixes(&self, violation: &Violation) -> Vec<SuggestedFix> {
        match violation.violation_type {
            ViolationType::CursorBounds => vec![
                SuggestedFix {
                    description: "Add cursor position bounds check after any terminal resize"
                        .to_string(),
                    confidence: 0.85,
                },
                SuggestedFix {
                    description: "Ensure cursor position is updated when viewport changes"
                        .to_string(),
                    confidence: 0.80,
                },
            ],
            ViolationType::ScreenMissingContent => vec![
                SuggestedFix {
                    description: "Verify expected content is present before proceeding".to_string(),
                    confidence: 0.75,
                },
                SuggestedFix {
                    description: "Check for timing issues causing content not to render"
                        .to_string(),
                    confidence: 0.70,
                },
            ],
            ViolationType::ResponseTimeout => vec![
                SuggestedFix {
                    description: "Add processing indicator for long-running operations".to_string(),
                    confidence: 0.70,
                },
                SuggestedFix {
                    description: "Increase timeout or optimize slow operation".to_string(),
                    confidence: 0.65,
                },
            ],
            _ => vec![SuggestedFix {
                description: "Review invariant requirements".to_string(),
                confidence: 0.5,
            }],
        }
    }

    fn find_related_issues(&self, violation: &Violation) -> Vec<RelatedIssue> {
        match violation.violation_type {
            ViolationType::CursorBounds => vec![
                RelatedIssue {
                    id: "#42".to_string(),
                    similarity: 0.85,
                },
                RelatedIssue {
                    id: "#87".to_string(),
                    similarity: 0.72,
                },
            ],
            ViolationType::ScreenMissingContent => vec![RelatedIssue {
                id: "#156".to_string(),
                similarity: 0.78,
            }],
            _ => vec![],
        }
    }

    pub fn build_ai_output(&self, trace: &Trace) -> AIFailureOutput {
        let failures: Vec<FailureExplanation> = trace
            .invariant_results
            .iter()
            .filter(|r| r.violation())
            .map(|result| self.explain_failure(result, trace))
            .collect();

        let summary = self.summarize_failures(&failures);
        let exit_code = self.extract_exit_code(&trace.outcome);

        AIFailureOutput {
            version: "1.0".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            scenario: trace.scenario.name.clone(),
            exit_code,
            outcome: format!("{:?}", trace.outcome),
            failures,
            summary,
        }
    }

    fn extract_exit_code(&self, outcome: &crate::trace::TraceOutcome) -> i32 {
        match outcome {
            crate::trace::TraceOutcome::Success { exit_code, .. } => *exit_code,
            crate::trace::TraceOutcome::InvariantViolation { .. } => -2,
            crate::trace::TraceOutcome::Timeout { .. } => -3,
            crate::trace::TraceOutcome::Error { .. } => -4,
            crate::trace::TraceOutcome::Signaled { signal, .. } => -*signal,
            crate::trace::TraceOutcome::ReplayDivergence { .. } => -5,
        }
    }

    fn summarize_failures(&self, failures: &[FailureExplanation]) -> FailureSummary {
        let mut categories: Vec<(String, usize)> = Vec::new();
        let mut violation_types: Vec<(String, usize)> = Vec::new();

        let mut critical_count = 0;
        let mut high_count = 0;

        for failure in failures {
            let cat = format!("{:?}", failure.violation.category);
            if let Some(pos) = categories.iter().position(|(c, _)| c == &cat) {
                categories[pos].1 += 1;
            } else {
                categories.push((cat, 1));
            }

            let vtype = format!("{:?}", failure.violation.violation_type);
            if let Some(pos) = violation_types.iter().position(|(v, _)| v == &vtype) {
                violation_types[pos].1 += 1;
            } else {
                violation_types.push((vtype, 1));
            }

            match failure.violation.severity {
                ViolationSeverity::Critical => critical_count += 1,
                ViolationSeverity::High => high_count += 1,
                _ => {}
            }
        }

        violation_types.sort_by(|a, b| b.1.cmp(&a.1));

        FailureSummary {
            total_failures: failures.len(),
            critical_count,
            high_count,
            categories,
            top_violation_types: violation_types,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::InvariantResult;

    fn sample_invariant_result() -> InvariantResult {
        InvariantResult {
            name: "cursor_bounds".to_string(),
            satisfied: false,
            description: "Cursor position must always be within screen bounds".to_string(),
            details: Some("Cursor at (80, 24) but screen is 40x12".to_string()),
            step: 3,
            tick: 150,
        }
    }

    fn sample_trace() -> Trace {
        use crate::scenario::Command;
        Trace {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            seed: 12345,
            scenario: crate::scenario::Scenario {
                name: "test_scenario".to_string(),
                description: "Test scenario".to_string(),
                command: Command::Simple("echo test".to_string()),
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
            outcome: crate::trace::TraceOutcome::InvariantViolation {
                invariant_name: "cursor_bounds".to_string(),
                checkpoint_index: 2,
            },
            final_screen_hash: None,
            total_ticks: 200,
        }
    }

    #[test]
    fn violation_type_variants() {
        assert_eq!(format!("{:?}", ViolationType::CursorBounds), "CursorBounds");
        assert_eq!(
            format!("{:?}", ViolationType::ScreenMissingContent),
            "ScreenMissingContent"
        );
    }

    #[test]
    fn violation_severity_variants() {
        assert_eq!(format!("{:?}", ViolationSeverity::Critical), "Critical");
        assert_eq!(format!("{:?}", ViolationSeverity::High), "High");
        assert_eq!(format!("{:?}", ViolationSeverity::Medium), "Medium");
        assert_eq!(format!("{:?}", ViolationSeverity::Low), "Low");
        assert_eq!(format!("{:?}", ViolationSeverity::Info), "Info");
    }

    #[test]
    fn violation_category_variants() {
        assert_eq!(format!("{:?}", ViolationCategory::Structural), "Structural");
        assert_eq!(
            format!("{:?}", ViolationCategory::Performance),
            "Performance"
        );
        assert_eq!(format!("{:?}", ViolationCategory::Content), "Content");
    }

    #[test]
    fn failure_explainer_new() {
        let _explainer = FailureExplainer::new();
        assert!(true);
    }

    #[test]
    fn classify_violation() {
        let explainer = FailureExplainer::new();
        let result = sample_invariant_result();
        let violation = explainer.classify_violation(&result);

        assert_eq!(violation.violation_type, ViolationType::CursorBounds);
        assert_eq!(violation.severity, ViolationSeverity::Critical);
        assert_eq!(violation.category, ViolationCategory::Structural);
        assert_eq!(violation.step, 3);
        assert_eq!(violation.tick, 150);
    }

    #[test]
    fn build_causal_chain() {
        let explainer = FailureExplainer::new();
        let result = sample_invariant_result();
        let chain = explainer.build_causal_chain(&result);

        assert!(!chain.is_empty());
        assert!(chain.iter().any(|e| e.event_type == "invariant_check"));
    }

    #[test]
    fn synthesize_minimal_repro() {
        let explainer = FailureExplainer::new();
        let trace = sample_trace();
        let repro = explainer.synthesize_minimal_repro(&trace);

        assert_eq!(repro.scenario_name, "test_scenario");
        assert_eq!(repro.seed, 12345);
        assert_eq!(repro.duration_ticks, 200);
    }

    #[test]
    fn suggest_fixes() {
        let explainer = FailureExplainer::new();
        let result = sample_invariant_result();
        let violation = explainer.classify_violation(&result);
        let fixes = explainer.suggest_fixes(&violation);

        assert!(!fixes.is_empty());
        assert!(fixes[0].confidence > 0.0);
    }

    #[test]
    fn find_related_issues() {
        let explainer = FailureExplainer::new();
        let result = sample_invariant_result();
        let violation = explainer.classify_violation(&result);
        let issues = explainer.find_related_issues(&violation);

        assert!(issues.iter().any(|i| i.id == "#42"));
    }

    #[test]
    fn build_ai_output() {
        let explainer = FailureExplainer::new();
        let trace = sample_trace();
        let output = explainer.build_ai_output(&trace);

        assert_eq!(output.version, "1.0");
        assert_eq!(output.scenario, "test_scenario");
        assert_eq!(output.exit_code, -2); // InvariantViolation returns -2
    }

    #[test]
    fn summarize_failures() {
        let explainer = FailureExplainer::new();
        let result = sample_invariant_result();
        let trace = sample_trace();

        let explanation = explainer.explain_failure(&result, &trace);
        let failures = vec![explanation];
        let summary = explainer.summarize_failures(&failures);

        assert_eq!(summary.total_failures, 1);
        assert_eq!(summary.critical_count, 1);
        assert!(summary.categories.iter().any(|(c, _)| c == "Structural"));
    }

    #[test]
    fn causal_event_creation() {
        let event = CausalEvent {
            event_type: "resize".to_string(),
            tick: 42,
            consequence: "Terminal resized".to_string(),
        };

        assert_eq!(event.event_type, "resize");
        assert_eq!(event.tick, 42);
    }

    #[test]
    fn minimal_repro_creation() {
        let repro = MinimalReproduction {
            scenario_name: "test".to_string(),
            step_count: 2,
            seed: 42,
            duration_ticks: 100,
        };

        assert_eq!(repro.step_count, 2);
        assert_eq!(repro.seed, 42);
    }

    #[test]
    fn suggested_fix_creation() {
        let fix = SuggestedFix {
            description: "Add bounds check".to_string(),
            confidence: 0.85,
        };

        assert_eq!(fix.confidence, 0.85);
    }

    #[test]
    fn severity_for_cursor_bounds() {
        let explainer = FailureExplainer::new();
        let severity = explainer.severity_for(&ViolationType::CursorBounds);
        assert_eq!(severity, ViolationSeverity::Critical);
    }

    #[test]
    fn severity_for_unknown() {
        let explainer = FailureExplainer::new();
        let severity = explainer.severity_for(&ViolationType::Unknown);
        assert_eq!(severity, ViolationSeverity::Low);
    }

    #[test]
    fn invariant_to_violation_known() {
        let explainer = FailureExplainer::new();
        let (vtype, category) = explainer.invariant_to_violation("cursor_bounds");
        assert_eq!(vtype, ViolationType::CursorBounds);
        assert_eq!(category, ViolationCategory::Structural);
    }

    #[test]
    fn invariant_to_violation_unknown() {
        let explainer = FailureExplainer::new();
        let (vtype, category) = explainer.invariant_to_violation("unknown_invariant");
        assert_eq!(vtype, ViolationType::Unknown);
        assert_eq!(category, ViolationCategory::Unknown);
    }
}
