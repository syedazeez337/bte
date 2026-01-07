//! Opinionated Defaults Module
//!
//! Provides sensible default configurations for terminal testing scenarios.

#![allow(dead_code)]

use crate::scenario::{Scenario, TerminalConfig};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalDefaults {
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalDefaults {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimingDefaults {
    pub tick_nanos: u64,
    pub max_default_ticks: u64,
    pub step_timeout_ticks: u64,
    pub resize_debounce_ticks: u64,
}

impl Default for TimingDefaults {
    fn default() -> Self {
        Self {
            tick_nanos: 10_000_000,
            max_default_ticks: 10_000,
            step_timeout_ticks: 5_000,
            resize_debounce_ticks: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvariantDefaults {
    pub cursor_bounds: bool,
    pub no_deadlock: bool,
    pub deadlock_timeout: u64,
    pub screen_stability: bool,
    pub no_output_after_exit: bool,
}

impl Default for InvariantDefaults {
    fn default() -> Self {
        Self {
            cursor_bounds: true,
            no_deadlock: true,
            deadlock_timeout: 1000,
            screen_stability: false,
            no_output_after_exit: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryDefaults {
    pub enabled: bool,
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f64,
}

impl Default for RetryDefaults {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            exponential_base: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputDefaults {
    pub capture_stdout: bool,
    pub capture_stderr: bool,
    pub max_output_size: usize,
    pub truncate_output: bool,
}

impl Default for OutputDefaults {
    fn default() -> Self {
        Self {
            capture_stdout: true,
            capture_stderr: true,
            max_output_size: 1024 * 1024,
            truncate_output: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BTEDefaults {
    pub terminal: TerminalDefaults,
    pub timing: TimingDefaults,
    pub invariants: InvariantDefaults,
    pub retry: RetryDefaults,
    pub output: OutputDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultScenarioTemplate {
    pub name: String,
    pub description: String,
    pub terminal: TerminalConfig,
    pub default_invariants: Vec<String>,
    pub timeout_ms: u64,
}

impl DefaultScenarioTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: format!("Auto-generated scenario: {}", name),
            terminal: TerminalConfig { cols: 80, rows: 24 },
            default_invariants: vec![
                "cursor_bounds".to_string(),
                "no_deadlock".to_string(),
                "no_output_after_exit".to_string(),
            ],
            timeout_ms: 120_000,
        }
    }

    pub fn interactive(&self) -> Self {
        Self {
            name: format!("{}_interactive", self.name),
            description: "Interactive terminal application test".to_string(),
            terminal: TerminalConfig { cols: 80, rows: 24 },
            default_invariants: vec![
                "cursor_bounds".to_string(),
                "no_deadlock".to_string(),
                "screen_stability".to_string(),
                "no_output_after_exit".to_string(),
            ],
            timeout_ms: 120_000,
        }
    }

    pub fn headless(&self) -> Self {
        Self {
            name: format!("{}_headless", self.name),
            description: "Headless command execution test".to_string(),
            terminal: TerminalConfig { cols: 80, rows: 24 },
            default_invariants: vec![
                "no_deadlock".to_string(),
                "process_terminated_cleanly".to_string(),
                "no_output_after_exit".to_string(),
            ],
            timeout_ms: 60_000,
        }
    }

    pub fn resize_test(&self) -> Self {
        Self {
            name: format!("{}_resize", self.name),
            description: "Terminal resize handling test".to_string(),
            terminal: TerminalConfig { cols: 80, rows: 24 },
            default_invariants: vec![
                "cursor_bounds".to_string(),
                "viewport_valid".to_string(),
                "screen_stability".to_string(),
            ],
            timeout_ms: 60_000,
        }
    }

    pub fn performance(&self) -> Self {
        Self {
            name: format!("{}_performance", self.name),
            description: "Performance and responsiveness test".to_string(),
            terminal: TerminalConfig { cols: 80, rows: 24 },
            default_invariants: vec![
                "response_time".to_string(),
                "max_latency".to_string(),
                "no_deadlock".to_string(),
            ],
            timeout_ms: 180_000,
        }
    }
}

pub struct DefaultConfigurator;

impl DefaultConfigurator {
    pub fn apply_terminal_defaults(config: &mut TerminalConfig) {
        config.cols = config.cols.clamp(40, 200);
        config.rows = config.rows.clamp(10, 100);
    }

    pub fn suggest_timing(tick_nanos: u64, scenario_steps: usize) -> TimingDefaults {
        let max_ticks = (scenario_steps as u64 * 100).max(10000);
        TimingDefaults {
            tick_nanos,
            max_default_ticks: max_ticks,
            step_timeout_ticks: max_ticks / 10,
            resize_debounce_ticks: 5,
        }
    }

    pub fn default_invariants_for_command(command: &str) -> Vec<String> {
        let cmd_lower = command.to_lowercase();
        if cmd_lower.contains("vim") || cmd_lower.contains("nano") || cmd_lower.contains("less") {
            vec![
                "cursor_bounds",
                "no_deadlock",
                "screen_stability",
                "no_output_after_exit",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect()
        } else if cmd_lower.contains("cargo")
            || cmd_lower.contains("npm")
            || cmd_lower.contains("pip")
        {
            vec![
                "no_deadlock",
                "process_terminated_cleanly",
                "no_output_after_exit",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect()
        } else if cmd_lower.contains("ssh") || cmd_lower.contains("telnet") {
            vec!["cursor_bounds", "screen_contains", "no_deadlock"]
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            vec!["cursor_bounds", "no_deadlock", "no_output_after_exit"]
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        }
    }

    pub fn string_to_invariant_ref(name: &str) -> crate::scenario::InvariantRef {
        match name {
            "cursor_bounds" => crate::scenario::InvariantRef::CursorBounds,
            "no_deadlock" => crate::scenario::InvariantRef::NoDeadlock { timeout_ms: None },
            "signal_handled" => crate::scenario::InvariantRef::SignalHandled {
                signal: crate::scenario::SignalName::Sigterm,
            },
            "screen_contains" => crate::scenario::InvariantRef::ScreenContains {
                pattern: ".*".to_string(),
            },
            "screen_not_contains" => crate::scenario::InvariantRef::ScreenNotContains {
                pattern: ".*".to_string(),
            },
            "no_output_after_exit" => crate::scenario::InvariantRef::NoOutputAfterExit,
            "process_terminated_cleanly" => {
                crate::scenario::InvariantRef::ProcessTerminatedCleanly {
                    allowed_signals: vec![],
                }
            }
            "screen_stability" => crate::scenario::InvariantRef::ScreenStability { min_ticks: 10 },
            "viewport_valid" => crate::scenario::InvariantRef::ViewportValid,
            "response_time" => crate::scenario::InvariantRef::ResponseTime { max_ticks: 100 },
            "max_latency" => crate::scenario::InvariantRef::MaxLatency { max_ticks: 50 },
            _ => crate::scenario::InvariantRef::Custom {
                name: name.to_string(),
            },
        }
    }

    pub fn estimate_scenario_duration(command: &str, steps: usize) -> Duration {
        let base_ms = if command.contains("vim") || command.contains("emacs") {
            5000
        } else if command.contains("cargo test") || command.contains("npm test") {
            30000
        } else if command.contains("ls") || command.contains("echo") {
            500
        } else {
            2000
        };
        Duration::from_millis((base_ms * steps as u64).min(300000))
    }

    pub fn build_scenario_with_defaults(
        name: &str,
        command: &str,
        steps: Vec<crate::scenario::Step>,
    ) -> Scenario {
        let defaults = BTEDefaults::default();
        let default_invariants = Self::default_invariants_for_command(command);

        Scenario {
            name: name.to_string(),
            description: format!("Auto-generated scenario for: {}", command),
            command: crate::scenario::Command::Simple(command.to_string()),
            terminal: TerminalConfig {
                cols: defaults.terminal.cols,
                rows: defaults.terminal.rows,
            },
            env: std::collections::HashMap::new(),
            steps,
            invariants: default_invariants
                .into_iter()
                .map(|inv| Self::string_to_invariant_ref(&inv))
                .collect(),
            seed: None,
            timeout_ms: Some(Self::estimate_scenario_duration(command, 1).as_millis() as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_defaults_creation() {
        let defaults = TerminalDefaults::default();
        assert_eq!(defaults.cols, 80);
        assert_eq!(defaults.rows, 24);
    }

    #[test]
    fn timing_defaults_creation() {
        let defaults = TimingDefaults::default();
        assert_eq!(defaults.tick_nanos, 10_000_000);
        assert_eq!(defaults.max_default_ticks, 10_000);
    }

    #[test]
    fn invariant_defaults_creation() {
        let defaults = InvariantDefaults::default();
        assert!(defaults.cursor_bounds);
        assert!(defaults.no_deadlock);
    }

    #[test]
    fn retry_defaults_creation() {
        let defaults = RetryDefaults::default();
        assert!(defaults.enabled);
        assert_eq!(defaults.max_attempts, 3);
    }

    #[test]
    fn bte_defaults_creation() {
        let defaults = BTEDefaults::default();
        assert_eq!(defaults.terminal.cols, 80);
        assert!(defaults.invariants.cursor_bounds);
    }

    #[test]
    fn default_scenario_template_new() {
        let template = DefaultScenarioTemplate::new("test");
        assert_eq!(template.name, "test");
        assert!(template
            .default_invariants
            .contains(&"cursor_bounds".to_string()));
    }

    #[test]
    fn default_scenario_template_interactive() {
        let template = DefaultScenarioTemplate::new("vim").interactive();
        assert_eq!(template.name, "vim_interactive");
        assert!(template
            .default_invariants
            .contains(&"screen_stability".to_string()));
    }

    #[test]
    fn apply_terminal_defaults() {
        let mut config = TerminalConfig {
            cols: 300,
            rows: 200,
        };
        DefaultConfigurator::apply_terminal_defaults(&mut config);
        assert_eq!(config.cols, 200);
        assert_eq!(config.rows, 100);
    }

    #[test]
    fn suggest_timing() {
        let timing = DefaultConfigurator::suggest_timing(10_000_000, 150);
        assert_eq!(timing.max_default_ticks, 15000);
    }

    #[test]
    fn default_invariants_for_editor() {
        let invariants = DefaultConfigurator::default_invariants_for_command("vim");
        assert!(invariants.contains(&"screen_stability".to_string()));
    }

    #[test]
    fn default_invariants_for_builder() {
        let invariants = DefaultConfigurator::default_invariants_for_command("cargo build");
        assert!(invariants.contains(&"process_terminated_cleanly".to_string()));
    }

    #[test]
    fn estimate_scenario_duration_editor() {
        let duration = DefaultConfigurator::estimate_scenario_duration("vim", 1);
        assert!(duration.as_millis() >= 5000);
    }

    #[test]
    fn estimate_scenario_duration_simple() {
        let duration = DefaultConfigurator::estimate_scenario_duration("ls -la", 1);
        assert!(duration.as_millis() < 1000);
    }

    #[test]
    fn build_scenario_with_defaults() {
        let scenario = DefaultConfigurator::build_scenario_with_defaults(
            "test_scenario",
            "echo hello",
            vec![],
        );
        assert_eq!(scenario.name, "test_scenario");
        assert!(scenario.timeout_ms.is_some());
    }

    #[test]
    fn timeout_is_capped() {
        let duration = DefaultConfigurator::estimate_scenario_duration("vim", 100);
        assert_eq!(duration.as_millis(), 300000);
    }
}
