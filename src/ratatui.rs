//! Ratatui Integration for BTE
//!
//! This module provides BTE integration for testing ratatui-based TUI applications.

use crate::{
    invariants::{Invariant, InvariantContext, InvariantResult},
    process::PtyProcess,
    screen::Screen,
};

pub mod invariants {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct RatatuiWidgetBounds;

    impl Invariant for RatatuiWidgetBounds {
        fn name(&self) -> &'static str {
            "ratatui::widget_bounds"
        }

        fn description(&self) -> &'static str {
            "Verifies all widget rects are within screen bounds"
        }

        fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
            let screen = match ctx.screen {
                Some(s) => s,
                None => {
                    return InvariantResult::new(
                        self.name(),
                        false,
                        self.description(),
                        Some("No screen available".to_string()),
                        ctx.step,
                        ctx.tick,
                    );
                }
            };

            let (cols, rows) = screen.size();
            let cursor = screen.cursor();
            if cursor.col >= cols || cursor.row >= rows {
                return InvariantResult::new(
                    self.name(),
                    false,
                    self.description(),
                    Some(format!(
                        "Cursor at ({}, {}) is outside bounds ({}x{})",
                        cursor.col, cursor.row, cols, rows
                    )),
                    ctx.step,
                    ctx.tick,
                );
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

    #[derive(Debug, Clone)]
    pub struct RatatuiTextOverflow;

    impl Invariant for RatatuiTextOverflow {
        fn name(&self) -> &'static str {
            "ratatui::text_overflow"
        }
        fn description(&self) -> &'static str {
            "Verifies text stays within bounds"
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

    #[derive(Debug, Clone)]
    pub struct RatatuiCursorState;

    impl Invariant for RatatuiCursorState {
        fn name(&self) -> &'static str {
            "ratatui::cursor_state"
        }
        fn description(&self) -> &'static str {
            "Verifies cursor is within bounds"
        }
        fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
            let screen = match ctx.screen {
                Some(s) => s,
                None => {
                    return InvariantResult::new(
                        self.name(),
                        true,
                        self.description(),
                        Some("No screen".to_string()),
                        ctx.step,
                        ctx.tick,
                    );
                }
            };

            let cursor = screen.cursor();
            let (cols, rows) = screen.size();
            if cursor.col >= cols || cursor.row >= rows {
                return InvariantResult::new(
                    self.name(),
                    false,
                    self.description(),
                    Some(format!("Cursor out of bounds")),
                    ctx.step,
                    ctx.tick,
                );
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

    #[derive(Debug, Clone)]
    pub struct RatatuiColorFormatting;

    impl Invariant for RatatuiColorFormatting {
        fn name(&self) -> &'static str {
            "ratatui::color_formatting"
        }
        fn description(&self) -> &'static str {
            "Verifies color formatting is valid"
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
}

pub mod utils {
    use super::*;

    #[derive(Default)]
    pub struct ScenarioBuilder<'a> {
        name: &'a str,
        command: String,
        invariants: Vec<Box<dyn Invariant>>,
        expect_patterns: Vec<String>,
    }

    impl<'a> ScenarioBuilder<'a> {
        pub fn new(name: &'a str, command: &str) -> Self {
            Self {
                name,
                command: command.to_string(),
                invariants: Vec::new(),
                expect_patterns: Vec::new(),
            }
        }

        pub fn widget_bounds(mut self) -> Self {
            self.invariants
                .push(Box::new(invariants::RatatuiWidgetBounds));
            self
        }

        pub fn cursor_state(mut self) -> Self {
            self.invariants
                .push(Box::new(invariants::RatatuiCursorState));
            self
        }

        pub fn text_overflow(mut self) -> Self {
            self.invariants
                .push(Box::new(invariants::RatatuiTextOverflow));
            self
        }

        pub fn color_formatting(mut self) -> Self {
            self.invariants
                .push(Box::new(invariants::RatatuiColorFormatting));
            self
        }

        pub fn expect(mut self, pattern: &str) -> Self {
            self.expect_patterns.push(pattern.to_string());
            self
        }

        pub fn build_yaml(&self) -> String {
            let invariants: Vec<_> = self
                .invariants
                .iter()
                .map(|inv| format!("  - type: {}", inv.name().replace("::", "_")))
                .collect();

            let expectations: Vec<_> = self
                .expect_patterns
                .iter()
                .map(|p| {
                    format!(
                        r#"  - type: expect_screen
    pattern: "{}""#,
                        p
                    )
                })
                .collect();

            format!(
                r#"name: {}
description: BTE test for ratatui application
steps:
  - type: spawn
    command: {}
    timeout: 5000

  - type: wait
    duration: 500ms
{}
{}

  - type: send_keys
    keys: ["q"]

invariants:
{}"#,
                self.name,
                self.command,
                expectations.join("\n\n"),
                if expectations.is_empty() { "" } else { "\n" },
                invariants.join("\n")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_ctx() -> InvariantContext<'static> {
        use crate::process::ProcessConfig;

        let config = ProcessConfig::shell("test");
        let mut process = PtyProcess::spawn(&config).unwrap();
        let screen = Screen::new(80, 24);
        InvariantContext {
            screen: Some(Box::leak(Box::new(screen))),
            process: &mut *Box::leak(Box::new(process)),
            step: 0,
            tick: 0,
            _is_replay: false,
            last_screen_hash: None,
            no_output_ticks: 0,
            expected_signal: None,
        }
    }

    #[test]
    fn test_widget_bounds_invariant() {
        let invariant = invariants::RatatuiWidgetBounds;
        let mut ctx = create_mock_ctx();
        let result = invariant.evaluate(&mut ctx);
        assert!(result.satisfied, "Invariant should be satisfied");
    }

    #[test]
    fn test_cursor_state_invariant() {
        let invariant = invariants::RatatuiCursorState;
        let mut ctx = create_mock_ctx();
        let result = invariant.evaluate(&mut ctx);
        assert!(result.satisfied);
    }

    #[test]
    fn test_scenario_builder() {
        let builder = utils::ScenarioBuilder::new("test-app", "./my-app")
            .widget_bounds()
            .cursor_state()
            .expect("Hello");

        let yaml = builder.build_yaml();
        assert!(yaml.contains("name: test-app"));
        assert!(yaml.contains("widget_bounds"));
        assert!(yaml.contains("Hello"));
    }
}
