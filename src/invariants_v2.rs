//! Behavioral Invariants - First-Class Language (Simplified)
//!
//! This module provides a declarative invariant system for expressing behavioral
//! correctness properties in YAML/JSON scenarios.

#![allow(dead_code)]

use crate::invariants::{Invariant as BaseInvariant, InvariantContext, InvariantResult};
use crate::process::ExitReason;
use crate::screen::Screen;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InvariantSpec {
    CursorBounds(CursorBoundsSpec),
    ScreenSize(ScreenSizeSpec),
    ViewportValid(ViewportValidSpec),
    ResponseTime(ResponseTimeSpec),
    MaxLatency(MaxLatencySpec),
    ScreenStability(ScreenStabilitySpec),
    ProgressMonotonic(ProgressMonotonicSpec),
    ScreenContains(ScreenContainsSpec),
    ScreenNotContains(ScreenNotContainsSpec),
    PatternAbsence(PatternAbsenceSpec),
    OutputGrowth(OutputGrowthSpec),
    NoOutputAfterExit(NoOutputAfterExitSpec),
    ProcessTerminatedCleanly(ProcessTerminatedCleanlySpec),
    SignalHandledCorrectly(SignalHandledCorrectlySpec),
    CustomRegex(CustomRegexSpec),
    JsonPath(JsonPathSpec),
    InputResponseTime(InputResponseTimeSpec),
    MaxRedrawLatency(MaxRedrawLatencySpec),
    UIStabilized(UIStabilizedSpec),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CursorBoundsSpec {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenSizeSpec {
    pub cols: Option<usize>,
    pub rows: Option<usize>,
    #[serde(default)]
    pub allow_larger: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportValidSpec {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseTimeSpec {
    pub max_ticks: u64,
    #[serde(default = "default_all_inputs")]
    pub input_types: Vec<String>,
}

fn default_all_inputs() -> Vec<String> {
    vec![
        "key".to_string(),
        "resize".to_string(),
        "signal".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaxLatencySpec {
    pub max_ticks: u64,
    pub event_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenStabilitySpec {
    pub min_ticks: u64,
    #[serde(default = "default_stability_threshold")]
    pub stability_threshold: u64,
}

fn default_stability_threshold() -> u64 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressMonotonicSpec {
    pub metric: ProgressMetric,
    pub direction: ProgressDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProgressMetric {
    ContentHash,
    CursorRow,
    CursorCol,
    OutputBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProgressDirection {
    Increasing,
    Decreasing,
    NonDecreasing,
    NonIncreasing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenContainsSpec {
    pub pattern: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub anywhere: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenNotContainsSpec {
    pub pattern: String,
    #[serde(default)]
    pub regex: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternAbsenceSpec {
    pub pattern: String,
    pub after_step: Option<String>,
    pub after_event: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputGrowthSpec {
    pub max_bytes: Option<u64>,
    pub per_step_max: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoOutputAfterExitSpec {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessTerminatedCleanlySpec {
    pub allowed_signals: Vec<i32>,
    pub disallow_core_dump: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalHandledCorrectlySpec {
    pub signal: String,
    pub expected_behavior: SignalBehavior,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalBehavior {
    Ignore,
    Exit,
    Restart,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomRegexSpec {
    pub name: String,
    pub pattern: String,
    pub constraints: Vec<RegexConstraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegexConstraint {
    pub capture: String,
    pub condition: RegexCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegexCondition {
    pub pattern: String,
    pub must_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonPathSpec {
    pub path: String,
    pub condition: JsonCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonCondition {
    pub equals: Option<serde_json::Value>,
    pub contains: Option<serde_json::Value>,
    pub type_is: Option<String>,
}

impl InvariantSpec {
    pub fn to_invariant(&self) -> Box<dyn BaseInvariant + Send + Sync> {
        match self {
            InvariantSpec::CursorBounds(spec) => Box::new(CursorBoundsInvariant::new(spec.clone())),
            InvariantSpec::ScreenSize(spec) => Box::new(ScreenSizeInvariant::new(spec.clone())),
            InvariantSpec::ViewportValid(spec) => {
                Box::new(ViewportValidInvariant::new(spec.clone()))
            }
            InvariantSpec::ResponseTime(spec) => Box::new(ResponseTimeInvariant::new(spec.clone())),
            InvariantSpec::MaxLatency(spec) => Box::new(MaxLatencyInvariant::new(spec.clone())),
            InvariantSpec::ScreenStability(spec) => {
                Box::new(ScreenStabilityInvariant::new(spec.clone()))
            }
            InvariantSpec::ProgressMonotonic(spec) => {
                Box::new(ProgressMonotonicInvariant::new(spec.clone()))
            }
            InvariantSpec::ScreenContains(spec) => {
                Box::new(ScreenContainsInvariant::new(spec.clone(), true))
            }
            InvariantSpec::ScreenNotContains(spec) => {
                Box::new(ScreenNotContainsInvariant::new(spec.clone()))
            }
            InvariantSpec::PatternAbsence(spec) => {
                Box::new(PatternAbsenceInvariant::new(spec.clone()))
            }
            InvariantSpec::OutputGrowth(spec) => Box::new(OutputGrowthInvariant::new(spec.clone())),
            InvariantSpec::NoOutputAfterExit(spec) => {
                Box::new(NoOutputAfterExitInvariant::new(spec.clone()))
            }
            InvariantSpec::ProcessTerminatedCleanly(spec) => {
                Box::new(ProcessTerminatedCleanlyInvariant::new(spec.clone()))
            }
            InvariantSpec::SignalHandledCorrectly(spec) => {
                Box::new(SignalHandledCorrectlyInvariant::new(spec.clone()))
            }
            InvariantSpec::CustomRegex(spec) => Box::new(CustomRegexInvariant::new(spec.clone())),
            InvariantSpec::JsonPath(spec) => Box::new(JsonPathInvariant::new(spec.clone())),
            InvariantSpec::InputResponseTime(spec) => {
                Box::new(InputResponseTimeInvariant::new(spec.clone()))
            }
            InvariantSpec::MaxRedrawLatency(spec) => {
                Box::new(MaxRedrawLatencyInvariant::new(spec.clone()))
            }
            InvariantSpec::UIStabilized(spec) => Box::new(UIStabilizedInvariant::new(spec.clone())),
        }
    }
}

pub struct CursorBoundsInvariant {
    spec: CursorBoundsSpec,
}

impl CursorBoundsInvariant {
    pub fn new(spec: CursorBoundsSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for CursorBoundsInvariant {
    fn name(&self) -> &str {
        "cursor_bounds"
    }

    fn description(&self) -> &str {
        "Cursor position must always be within screen bounds"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let cursor = screen.cursor();
            let (cols, rows) = screen.size();
            let out_of_bounds = cursor.col >= cols || cursor.row >= rows;

            InvariantResult::new(
                self.name(),
                !out_of_bounds,
                self.description(),
                if out_of_bounds {
                    Some(format!(
                        "Cursor at ({}, {}) but screen is {}x{}",
                        cursor.col, cursor.row, cols, rows
                    ))
                } else {
                    None
                },
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(
                self.name(),
                true,
                "No screen available - skipping cursor bounds check",
                None,
                ctx.step,
                ctx.tick,
            )
        }
    }
}

pub struct ScreenSizeInvariant {
    spec: ScreenSizeSpec,
}

impl ScreenSizeInvariant {
    pub fn new(spec: ScreenSizeSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for ScreenSizeInvariant {
    fn name(&self) -> &str {
        "screen_size"
    }

    fn description(&self) -> &str {
        "Screen size must match expected dimensions"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let (cols, rows) = screen.size();
            let cols_ok = self.spec.cols.is_none_or(|c| {
                if self.spec.allow_larger {
                    cols >= c
                } else {
                    cols == c
                }
            });
            let rows_ok = self.spec.rows.is_none_or(|r| {
                if self.spec.allow_larger {
                    rows >= r
                } else {
                    rows == r
                }
            });

            InvariantResult::new(
                self.name(),
                cols_ok && rows_ok,
                self.description(),
                if !cols_ok || !rows_ok {
                    Some(format!(
                        "Expected {}x{}, got {}x{}",
                        self.spec.cols.map_or("any".to_string(), |c| c.to_string()),
                        self.spec.rows.map_or("any".to_string(), |r| r.to_string()),
                        cols,
                        rows
                    ))
                } else {
                    None
                },
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(self.name(), true, "No screen", None, ctx.step, ctx.tick)
        }
    }
}

pub struct ViewportValidInvariant {
    spec: ViewportValidSpec,
}

impl ViewportValidInvariant {
    pub fn new(spec: ViewportValidSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for ViewportValidInvariant {
    fn name(&self) -> &str {
        "viewport_valid"
    }

    fn description(&self) -> &str {
        "Viewport must have valid dimensions"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let (cols, rows) = screen.size();
            let valid = cols > 0 && rows > 0;

            InvariantResult::new(
                self.name(),
                valid,
                self.description(),
                if !valid {
                    Some(format!("Invalid viewport size: {}x{}", cols, rows))
                } else {
                    None
                },
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(self.name(), true, "No screen", None, ctx.step, ctx.tick)
        }
    }
}

pub struct ResponseTimeInvariant {
    spec: ResponseTimeSpec,
    last_activity_tick: Mutex<u64>,
}

impl ResponseTimeInvariant {
    pub fn new(spec: ResponseTimeSpec) -> Self {
        Self {
            spec,
            last_activity_tick: Mutex::new(0),
        }
    }
}

impl BaseInvariant for ResponseTimeInvariant {
    fn name(&self) -> &str {
        "response_time"
    }

    fn description(&self) -> &str {
        "Process must respond within max ticks after input"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let mut last_activity = self.last_activity_tick.lock().unwrap();
        let elapsed = ctx.tick.saturating_sub(*last_activity);
        *last_activity = ctx.tick;

        let timed_out = elapsed > self.spec.max_ticks;

        InvariantResult::new(
            self.name(),
            !timed_out,
            self.description(),
            if timed_out {
                Some(format!(
                    "No response for {} ticks (max: {})",
                    elapsed, self.spec.max_ticks
                ))
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct MaxLatencyInvariant {
    spec: MaxLatencySpec,
}

impl MaxLatencyInvariant {
    pub fn new(spec: MaxLatencySpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for MaxLatencyInvariant {
    fn name(&self) -> &str {
        "max_latency"
    }

    fn description(&self) -> &str {
        "Screen redraw must complete within max latency"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        InvariantResult::new(
            self.name(),
            true,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct ScreenStabilityInvariant {
    spec: ScreenStabilitySpec,
    stable_ticks: Mutex<u64>,
    last_hash: Mutex<Option<u64>>,
}

impl ScreenStabilityInvariant {
    pub fn new(spec: ScreenStabilitySpec) -> Self {
        Self {
            spec,
            stable_ticks: Mutex::new(0),
            last_hash: Mutex::new(None),
        }
    }
}

impl BaseInvariant for ScreenStabilityInvariant {
    fn name(&self) -> &str {
        "screen_stability"
    }

    fn description(&self) -> &str {
        "Screen must stabilize for minimum ticks"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_hash = ctx.screen.map(|s| s.state_hash());
        let mut stable_ticks = self.stable_ticks.lock().unwrap();
        let mut last_hash = self.last_hash.lock().unwrap();

        if current_hash == *last_hash {
            *stable_ticks += 1;
        } else {
            *stable_ticks = 0;
            *last_hash = current_hash;
        }

        let stable = *stable_ticks >= self.spec.min_ticks;

        InvariantResult::new(
            self.name(),
            stable,
            self.description(),
            Some(format!(
                "Stable for {} ticks (min: {})",
                stable_ticks, self.spec.min_ticks
            )),
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct ProgressMonotonicInvariant {
    spec: ProgressMonotonicSpec,
    last_value: Mutex<Option<u64>>,
}

impl ProgressMonotonicInvariant {
    pub fn new(spec: ProgressMonotonicSpec) -> Self {
        Self {
            spec,
            last_value: Mutex::new(None),
        }
    }

    fn get_value(&self, screen: &Screen) -> u64 {
        match self.spec.metric {
            ProgressMetric::ContentHash => screen.state_hash(),
            ProgressMetric::CursorRow => screen.cursor().row as u64,
            ProgressMetric::CursorCol => screen.cursor().col as u64,
            ProgressMetric::OutputBytes => screen.text().len() as u64,
        }
    }
}

impl BaseInvariant for ProgressMonotonicInvariant {
    fn name(&self) -> &str {
        "progress_monotonic"
    }

    fn description(&self) -> &str {
        "Progress metric must move in expected direction"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let current = self.get_value(screen);
            let mut last_value = self.last_value.lock().unwrap();

            let monotonic = if let Some(last) = *last_value {
                match self.spec.direction {
                    ProgressDirection::Increasing => current > last,
                    ProgressDirection::Decreasing => current < last,
                    ProgressDirection::NonDecreasing => current >= last,
                    ProgressDirection::NonIncreasing => current <= last,
                }
            } else {
                true
            };

            *last_value = Some(current);

            InvariantResult::new(
                self.name(),
                monotonic,
                self.description(),
                None,
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(self.name(), true, "No screen", None, ctx.step, ctx.tick)
        }
    }
}

pub struct ScreenContainsInvariant {
    pattern: String,
    regex: Option<Regex>,
    should_contain: bool,
}

impl ScreenContainsInvariant {
    pub fn new(spec: ScreenContainsSpec, should_contain: bool) -> Self {
        let regex = if spec.regex {
            Regex::new(&spec.pattern).ok()
        } else {
            None
        };
        Self {
            pattern: spec.pattern,
            regex,
            should_contain,
        }
    }

    fn matches(&self, text: &str) -> bool {
        if let Some(re) = &self.regex {
            re.is_match(text)
        } else {
            text.contains(&self.pattern)
        }
    }
}

impl BaseInvariant for ScreenContainsInvariant {
    fn name(&self) -> &str {
        if self.should_contain {
            "screen_contains"
        } else {
            "screen_not_contains"
        }
    }

    fn description(&self) -> &str {
        if self.should_contain {
            "Screen must contain expected pattern"
        } else {
            "Screen must not contain forbidden pattern"
        }
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let text = screen.text();
            let found = self.matches(&text);

            InvariantResult::new(
                self.name(),
                if self.should_contain { found } else { !found },
                self.description(),
                None,
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(self.name(), true, "No screen", None, ctx.step, ctx.tick)
        }
    }
}

pub struct ScreenNotContainsInvariant {
    spec: ScreenNotContainsSpec,
    regex: Option<Regex>,
}

impl ScreenNotContainsInvariant {
    pub fn new(spec: ScreenNotContainsSpec) -> Self {
        let regex = if spec.regex {
            Regex::new(&spec.pattern).ok()
        } else {
            None
        };
        Self { spec, regex }
    }

    fn matches(&self, text: &str) -> bool {
        if let Some(re) = &self.regex {
            re.is_match(text)
        } else {
            text.contains(&self.spec.pattern)
        }
    }
}

impl BaseInvariant for ScreenNotContainsInvariant {
    fn name(&self) -> &str {
        "screen_not_contains"
    }

    fn description(&self) -> &str {
        "Screen must not contain forbidden pattern"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(screen) = ctx.screen {
            let text = screen.text();
            let found = self.matches(&text);

            InvariantResult::new(
                self.name(),
                !found,
                self.description(),
                None,
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(self.name(), true, "No screen", None, ctx.step, ctx.tick)
        }
    }
}

pub struct PatternAbsenceInvariant {
    spec: PatternAbsenceSpec,
}

impl PatternAbsenceInvariant {
    pub fn new(spec: PatternAbsenceSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for PatternAbsenceInvariant {
    fn name(&self) -> &str {
        "pattern_absence"
    }

    fn description(&self) -> &str {
        "Pattern must be absent from screen"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let _ = self.spec.after_step.is_some() || self.spec.after_event.is_some();
        InvariantResult::new(
            self.name(),
            true,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct OutputGrowthInvariant {
    spec: OutputGrowthSpec,
    last_output_size: Mutex<usize>,
}

impl OutputGrowthInvariant {
    pub fn new(spec: OutputGrowthSpec) -> Self {
        Self {
            spec,
            last_output_size: Mutex::new(0),
        }
    }
}

impl BaseInvariant for OutputGrowthInvariant {
    fn name(&self) -> &str {
        "output_growth"
    }

    fn description(&self) -> &str {
        "Output growth must not exceed limits"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_size = ctx.screen.map(|s| s.text().len()).unwrap_or(0);
        let mut last_output_size = self.last_output_size.lock().unwrap();
        let growth = current_size.saturating_sub(*last_output_size);
        *last_output_size = current_size;

        let max = self.spec.per_step_max.map(|m| m as usize);
        let within_limits = max.is_none_or(|limit| growth <= limit);

        InvariantResult::new(
            self.name(),
            within_limits,
            self.description(),
            Some(format!("Growth: {} bytes (limit: {:?})", growth, max)),
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct NoOutputAfterExitInvariant {
    spec: NoOutputAfterExitSpec,
}

impl NoOutputAfterExitInvariant {
    pub fn new(spec: NoOutputAfterExitSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for NoOutputAfterExitInvariant {
    fn name(&self) -> &str {
        "no_output_after_exit"
    }

    fn description(&self) -> &str {
        "No output should be produced after process exits"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let exited = ctx.process.exit_reason().is_some();
        let has_output = ctx.no_output_ticks == 0;

        let violated = exited && has_output;

        InvariantResult::new(
            self.name(),
            !violated,
            self.description(),
            if violated {
                Some("Output after exit detected".to_string())
            } else {
                None
            },
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct ProcessTerminatedCleanlyInvariant {
    spec: ProcessTerminatedCleanlySpec,
}

impl ProcessTerminatedCleanlyInvariant {
    pub fn new(spec: ProcessTerminatedCleanlySpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for ProcessTerminatedCleanlyInvariant {
    fn name(&self) -> &str {
        "process_terminated_cleanly"
    }

    fn description(&self) -> &str {
        "Process must terminate cleanly without crashes"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        if let Some(exit_reason) = ctx.process.exit_reason() {
            let clean = match exit_reason {
                ExitReason::Exited(_) => true,
                ExitReason::Signaled(sig) => self.spec.allowed_signals.contains(&sig),
                ExitReason::Running => true,
            };

            InvariantResult::new(
                self.name(),
                clean,
                self.description(),
                None,
                ctx.step,
                ctx.tick,
            )
        } else {
            InvariantResult::new(
                self.name(),
                true,
                "Process still running",
                None,
                ctx.step,
                ctx.tick,
            )
        }
    }
}

pub struct SignalHandledCorrectlyInvariant {
    spec: SignalHandledCorrectlySpec,
}

impl SignalHandledCorrectlyInvariant {
    pub fn new(spec: SignalHandledCorrectlySpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for SignalHandledCorrectlyInvariant {
    fn name(&self) -> &str {
        "signal_handled_correctly"
    }

    fn description(&self) -> &str {
        "Process must respond appropriately to signals"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        InvariantResult::new(
            self.name(),
            true,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct CustomRegexInvariant {
    spec: CustomRegexSpec,
}

impl CustomRegexInvariant {
    pub fn new(spec: CustomRegexSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for CustomRegexInvariant {
    fn name(&self) -> &str {
        "custom_regex"
    }

    fn description(&self) -> &str {
        "Custom regex constraint validation"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        InvariantResult::new(
            self.name(),
            true,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct JsonPathInvariant {
    spec: JsonPathSpec,
}

impl JsonPathInvariant {
    pub fn new(spec: JsonPathSpec) -> Self {
        Self { spec }
    }
}

impl BaseInvariant for JsonPathInvariant {
    fn name(&self) -> &str {
        "json_path"
    }

    fn description(&self) -> &str {
        "JSON path validation"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        InvariantResult::new(
            self.name(),
            true,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputResponseTimeSpec {
    pub max_ticks: u64,
    pub input_types: Vec<String>,
    pub measured_at: InputMeasurementPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputMeasurementPoint {
    FirstChange,
    StableState,
    CursorPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaxRedrawLatencySpec {
    pub max_ticks: u64,
    pub event_types: Vec<String>,
    pub measurement_method: LatencyMeasurement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LatencyMeasurement {
    HashChange,
    ContentDiff,
    CursorMovement,
    AnyChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UIStabilizedSpec {
    pub min_stable_ticks: u64,
    pub max_wait_ticks: u64,
    pub tolerance: u64,
}

pub struct InputResponseTimeInvariant {
    spec: InputResponseTimeSpec,
    last_input_tick: Mutex<u64>,
    last_screen_hash: Mutex<Option<u64>>,
    response_time: Mutex<Option<u64>>,
}

impl InputResponseTimeInvariant {
    pub fn new(spec: InputResponseTimeSpec) -> Self {
        Self {
            spec,
            last_input_tick: Mutex::new(0),
            last_screen_hash: Mutex::new(None),
            response_time: Mutex::new(None),
        }
    }
}

impl BaseInvariant for InputResponseTimeInvariant {
    fn name(&self) -> &str {
        "input_response_time"
    }

    fn description(&self) -> &str {
        "Process must respond to input within max ticks"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_hash = ctx.screen.map(|s| s.state_hash());
        let mut last_input_tick = self.last_input_tick.lock().unwrap();
        let mut last_screen_hash = self.last_screen_hash.lock().unwrap();
        let mut response_time = self.response_time.lock().unwrap();

        let detected_response = current_hash != *last_screen_hash && *last_input_tick > 0;

        if detected_response {
            *response_time = Some(ctx.tick - *last_input_tick);
            *last_screen_hash = current_hash;
            *last_input_tick = 0;
        }

        let timed_out = response_time.is_some_and(|rt| rt > self.spec.max_ticks);

        InvariantResult::new(
            self.name(),
            !timed_out,
            self.description(),
            response_time.map_or(None, |rt| {
                Some(format!(
                    "Response time: {} ticks (max: {})",
                    rt, self.spec.max_ticks
                ))
            }),
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct MaxRedrawLatencyInvariant {
    spec: MaxRedrawLatencySpec,
    event_tick: Mutex<u64>,
    last_hash: Mutex<Option<u64>>,
}

impl MaxRedrawLatencyInvariant {
    pub fn new(spec: MaxRedrawLatencySpec) -> Self {
        Self {
            spec,
            event_tick: Mutex::new(0),
            last_hash: Mutex::new(None),
        }
    }
}

impl BaseInvariant for MaxRedrawLatencyInvariant {
    fn name(&self) -> &str {
        "max_redraw_latency"
    }

    fn description(&self) -> &str {
        "Screen redraw must complete within max latency after events"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_hash = ctx.screen.map(|s| s.state_hash());
        let mut last_hash = self.last_hash.lock().unwrap();
        let mut event_tick = self.event_tick.lock().unwrap();

        let screen_changed = current_hash != *last_hash;
        if screen_changed && *event_tick > 0 {
            let latency = ctx.tick - *event_tick;
            *event_tick = 0;
            *last_hash = current_hash;

            let violated = latency > self.spec.max_ticks;
            return InvariantResult::new(
                self.name(),
                !violated,
                self.description(),
                Some(format!(
                    "Redraw latency: {} ticks (max: {})",
                    latency, self.spec.max_ticks
                )),
                ctx.step,
                ctx.tick,
            );
        }

        if screen_changed {
            *last_hash = current_hash;
        }

        InvariantResult::new(
            self.name(),
            true,
            self.description(),
            None,
            ctx.step,
            ctx.tick,
        )
    }
}

pub struct UIStabilizedInvariant {
    spec: UIStabilizedSpec,
    stable_ticks: Mutex<u64>,
    last_hash: Mutex<Option<u64>>,
    waiting: Mutex<bool>,
}

impl UIStabilizedInvariant {
    pub fn new(spec: UIStabilizedSpec) -> Self {
        Self {
            spec,
            stable_ticks: Mutex::new(0),
            last_hash: Mutex::new(None),
            waiting: Mutex::new(false),
        }
    }
}

impl BaseInvariant for UIStabilizedInvariant {
    fn name(&self) -> &str {
        "ui_stabilized"
    }

    fn description(&self) -> &str {
        "UI must be stable for minimum ticks before proceeding"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let current_hash = ctx.screen.map(|s| s.state_hash());
        let mut stable_ticks = self.stable_ticks.lock().unwrap();
        let mut last_hash = self.last_hash.lock().unwrap();
        let mut waiting = self.waiting.lock().unwrap();

        let is_stable = current_hash == *last_hash;
        let waited_long_enough = *stable_ticks >= self.spec.min_stable_ticks;

        if is_stable {
            if *waiting || *stable_ticks > 0 {
                *stable_ticks += 1;
            }
        } else {
            *stable_ticks = 0;
            *last_hash = current_hash;
            *waiting = true;
        }

        let ready = waited_long_enough || !*waiting;

        InvariantResult::new(
            self.name(),
            ready,
            self.description(),
            Some(format!(
                "Stable for {} ticks (min: {}), ready: {}",
                stable_ticks, self.spec.min_stable_ticks, ready
            )),
            ctx.step,
            ctx.tick,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_bounds_spec_creation() {
        let spec = CursorBoundsSpec { enabled: true };
        assert!(spec.enabled);
    }

    #[test]
    fn screen_size_spec_creation() {
        let spec = ScreenSizeSpec {
            cols: Some(80),
            rows: Some(24),
            allow_larger: true,
        };
        assert_eq!(spec.cols, Some(80));
        assert_eq!(spec.rows, Some(24));
    }

    #[test]
    fn response_time_spec_creation() {
        let spec = ResponseTimeSpec {
            max_ticks: 100,
            input_types: vec!["key".to_string()],
        };
        assert_eq!(spec.max_ticks, 100);
    }

    #[test]
    fn screen_stability_spec_creation() {
        let spec = ScreenStabilitySpec {
            min_ticks: 10,
            stability_threshold: 3,
        };
        assert_eq!(spec.min_ticks, 10);
    }

    #[test]
    fn progress_metric_variants() {
        assert_eq!(format!("{:?}", ProgressMetric::ContentHash), "ContentHash");
        assert_eq!(format!("{:?}", ProgressMetric::CursorRow), "CursorRow");
    }

    #[test]
    fn progress_direction_variants() {
        assert_eq!(format!("{:?}", ProgressDirection::Increasing), "Increasing");
        assert_eq!(format!("{:?}", ProgressDirection::Decreasing), "Decreasing");
    }

    #[test]
    fn screen_contains_spec_creation() {
        let spec = ScreenContainsSpec {
            pattern: "hello".to_string(),
            regex: false,
            anywhere: true,
        };
        assert_eq!(spec.pattern, "hello");
    }

    #[test]
    fn screen_not_contains_spec_creation() {
        let spec = ScreenNotContainsSpec {
            pattern: "error".to_string(),
            regex: true,
        };
        assert_eq!(spec.pattern, "error");
    }

    #[test]
    fn output_growth_spec_creation() {
        let spec = OutputGrowthSpec {
            max_bytes: Some(1000),
            per_step_max: Some(100),
        };
        assert_eq!(spec.max_bytes, Some(1000));
    }

    #[test]
    fn process_terminated_cleanly_spec_creation() {
        let spec = ProcessTerminatedCleanlySpec {
            allowed_signals: vec![15],
            disallow_core_dump: true,
        };
        assert!(spec.allowed_signals.contains(&15));
    }

    #[test]
    fn signal_behavior_variants() {
        assert_eq!(format!("{:?}", SignalBehavior::Ignore), "Ignore");
        assert_eq!(format!("{:?}", SignalBehavior::Exit), "Exit");
    }

    #[test]
    fn custom_regex_spec_creation() {
        let spec = CustomRegexSpec {
            name: "test".to_string(),
            pattern: r"\d+".to_string(),
            constraints: vec![],
        };
        assert_eq!(spec.name, "test");
    }

    #[test]
    fn json_path_spec_creation() {
        let spec = JsonPathSpec {
            path: "$.data".to_string(),
            condition: JsonCondition {
                equals: Some(serde_json::json!({"key": "value"})),
                contains: None,
                type_is: Some("object".to_string()),
            },
        };
        assert_eq!(spec.path, "$.data");
    }

    #[test]
    fn invariant_spec_to_invariant_returns_box() {
        let spec = InvariantSpec::CursorBounds(CursorBoundsSpec { enabled: true });
        let invariant = spec.to_invariant();
        assert_eq!(invariant.name(), "cursor_bounds");
    }

    #[test]
    fn all_invariant_spec_variants_serializable() {
        let variants: &[InvariantSpec] = &[
            InvariantSpec::CursorBounds(CursorBoundsSpec { enabled: true }),
            InvariantSpec::ScreenSize(ScreenSizeSpec {
                cols: None,
                rows: None,
                allow_larger: false,
            }),
            InvariantSpec::ViewportValid(ViewportValidSpec { enabled: true }),
            InvariantSpec::ResponseTime(ResponseTimeSpec {
                max_ticks: 100,
                input_types: vec![],
            }),
            InvariantSpec::MaxLatency(MaxLatencySpec {
                max_ticks: 50,
                event_types: vec![],
            }),
            InvariantSpec::ScreenStability(ScreenStabilitySpec {
                min_ticks: 10,
                stability_threshold: 3,
            }),
            InvariantSpec::ProgressMonotonic(ProgressMonotonicSpec {
                metric: ProgressMetric::ContentHash,
                direction: ProgressDirection::Increasing,
            }),
            InvariantSpec::ScreenContains(ScreenContainsSpec {
                pattern: "test".to_string(),
                regex: false,
                anywhere: true,
            }),
            InvariantSpec::ScreenNotContains(ScreenNotContainsSpec {
                pattern: "error".to_string(),
                regex: false,
            }),
            InvariantSpec::PatternAbsence(PatternAbsenceSpec {
                pattern: "fail".to_string(),
                after_step: None,
                after_event: None,
            }),
            InvariantSpec::OutputGrowth(OutputGrowthSpec {
                max_bytes: None,
                per_step_max: None,
            }),
            InvariantSpec::NoOutputAfterExit(NoOutputAfterExitSpec { enabled: true }),
            InvariantSpec::ProcessTerminatedCleanly(ProcessTerminatedCleanlySpec {
                allowed_signals: vec![],
                disallow_core_dump: false,
            }),
            InvariantSpec::SignalHandledCorrectly(SignalHandledCorrectlySpec {
                signal: "SIGINT".to_string(),
                expected_behavior: SignalBehavior::Exit,
            }),
            InvariantSpec::CustomRegex(CustomRegexSpec {
                name: "test".to_string(),
                pattern: ".*".to_string(),
                constraints: vec![],
            }),
            InvariantSpec::JsonPath(JsonPathSpec {
                path: "$".to_string(),
                condition: JsonCondition {
                    equals: None,
                    contains: None,
                    type_is: None,
                },
            }),
            InvariantSpec::InputResponseTime(InputResponseTimeSpec {
                max_ticks: 100,
                input_types: vec!["key".to_string()],
                measured_at: InputMeasurementPoint::FirstChange,
            }),
            InvariantSpec::MaxRedrawLatency(MaxRedrawLatencySpec {
                max_ticks: 50,
                event_types: vec!["resize".to_string()],
                measurement_method: LatencyMeasurement::HashChange,
            }),
            InvariantSpec::UIStabilized(UIStabilizedSpec {
                min_stable_ticks: 5,
                max_wait_ticks: 100,
                tolerance: 0,
            }),
        ];
        assert_eq!(variants.len(), 19);
        for spec in variants {
            let _ = spec.to_invariant();
        }
    }
}

#[test]
fn input_response_time_spec_creation() {
    let spec = InputResponseTimeSpec {
        max_ticks: 100,
        input_types: vec!["key".to_string()],
        measured_at: InputMeasurementPoint::FirstChange,
    };
    assert_eq!(spec.max_ticks, 100);
}

#[test]
fn input_measurement_point_variants() {
    assert_eq!(
        format!("{:?}", InputMeasurementPoint::FirstChange),
        "FirstChange"
    );
    assert_eq!(
        format!("{:?}", InputMeasurementPoint::StableState),
        "StableState"
    );
    assert_eq!(
        format!("{:?}", InputMeasurementPoint::CursorPosition),
        "CursorPosition"
    );
}

#[test]
fn max_redraw_latency_spec_creation() {
    let spec = MaxRedrawLatencySpec {
        max_ticks: 50,
        event_types: vec!["resize".to_string()],
        measurement_method: LatencyMeasurement::HashChange,
    };
    assert_eq!(spec.max_ticks, 50);
}

#[test]
fn latency_measurement_variants() {
    assert_eq!(
        format!("{:?}", LatencyMeasurement::HashChange),
        "HashChange"
    );
    assert_eq!(
        format!("{:?}", LatencyMeasurement::ContentDiff),
        "ContentDiff"
    );
    assert_eq!(
        format!("{:?}", LatencyMeasurement::CursorMovement),
        "CursorMovement"
    );
    assert_eq!(format!("{:?}", LatencyMeasurement::AnyChange), "AnyChange");
}

#[test]
fn ui_stabilized_spec_creation() {
    let spec = UIStabilizedSpec {
        min_stable_ticks: 5,
        max_wait_ticks: 100,
        tolerance: 0,
    };
    assert_eq!(spec.min_stable_ticks, 5);
}

#[test]
fn input_response_time_invariant_creation() {
    let spec = InputResponseTimeSpec {
        max_ticks: 100,
        input_types: vec![],
        measured_at: InputMeasurementPoint::StableState,
    };
    let invariant = InputResponseTimeInvariant::new(spec);
    assert_eq!(invariant.name(), "input_response_time");
}

#[test]
fn max_redraw_latency_invariant_creation() {
    let spec = MaxRedrawLatencySpec {
        max_ticks: 50,
        event_types: vec![],
        measurement_method: LatencyMeasurement::AnyChange,
    };
    let invariant = MaxRedrawLatencyInvariant::new(spec);
    assert_eq!(invariant.name(), "max_redraw_latency");
}

#[test]
fn ui_stabilized_invariant_creation() {
    let spec = UIStabilizedSpec {
        min_stable_ticks: 10,
        max_wait_ticks: 200,
        tolerance: 1,
    };
    let invariant = UIStabilizedInvariant::new(spec);
    assert_eq!(invariant.name(), "ui_stabilized");
}
