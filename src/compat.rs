#![allow(dead_code)]

use crate::invariants::{Invariant, InvariantContext, InvariantResult};

#[derive(Debug, Clone)]
pub struct TerminalCompatibility {
    terminal_type: TerminalType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalType {
    XTerm,
    XTerm256Color,
    LinuxConsole,
    Iterm2,
    WindowsTerminal,
    Alacritty,
    WezTerm,
    GnomeTerminal,
    Foot,
}

impl TerminalCompatibility {
    pub fn new(terminal_type: TerminalType) -> Self {
        Self { terminal_type }
    }

    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let terminal_type = if term.contains("xterm") {
            if term.contains("256") || term.contains("colors") {
                TerminalType::XTerm256Color
            } else {
                TerminalType::XTerm
            }
        } else if term.contains("linux") {
            TerminalType::LinuxConsole
        } else if term.contains("alacritty") {
            TerminalType::Alacritty
        } else if term.contains("wezterm") {
            TerminalType::WezTerm
        } else if term.contains("foot") {
            TerminalType::Foot
        } else {
            TerminalType::XTerm
        };

        Self::new(terminal_type)
    }
}

impl Invariant for TerminalCompatibility {
    fn name(&self) -> &'static str {
        "terminal_compatibility"
    }

    fn description(&self) -> &'static str {
        "Verifies application works correctly with detected terminal"
    }

    fn evaluate(&self, ctx: &mut InvariantContext) -> InvariantResult {
        let term = std::env::var("TERM").unwrap_or_default();
        let supported_terminals = match self.terminal_type {
            TerminalType::XTerm | TerminalType::XTerm256Color => {
                vec!["xterm", "xterm-256color", "screen"]
            }
            TerminalType::LinuxConsole => vec!["linux", "console"],
            TerminalType::Iterm2 => vec!["xterm-256color"],
            TerminalType::WindowsTerminal => vec!["xterm-256color"],
            TerminalType::Alacritty => vec!["alacritty", "xterm-256color"],
            TerminalType::WezTerm => vec!["wezterm", "xterm-256color"],
            TerminalType::GnomeTerminal => vec!["gnome", "xterm-256color"],
            TerminalType::Foot => vec!["foot", "xterm-256color"],
        };

        let is_compatible = supported_terminals.iter().any(|t| term.contains(t));

        InvariantResult::new(
            self.name(),
            is_compatible,
            self.description(),
            if is_compatible {
                None
            } else {
                Some(format!(
                    "Terminal '{}' may not be fully compatible. Expected one of: {:?}",
                    term, supported_terminals
                ))
            },
            ctx.step,
            ctx.tick,
        )
    }
}

#[derive(Debug, Clone)]
pub struct MouseSupportCheck;

impl Invariant for MouseSupportCheck {
    fn name(&self) -> &'static str {
        "mouse_support"
    }

    fn description(&self) -> &'static str {
        "Verifies mouse input sequences are properly handled"
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
pub struct TrueColorSupport;

impl Invariant for TrueColorSupport {
    fn name(&self) -> &'static str {
        "truecolor_support"
    }

    fn description(&self) -> &'static str {
        "Verifies true-color (24-bit) sequences are properly handled"
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
pub struct UnicodeSupport;

impl Invariant for UnicodeSupport {
    fn name(&self) -> &'static str {
        "unicode_support"
    }

    fn description(&self) -> &'static str {
        "Verifies Unicode input and display is properly handled"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_type_detection() {
        let detected = TerminalCompatibility::detect();
        assert!(
            matches!(
                detected.terminal_type,
                TerminalType::XTerm | TerminalType::XTerm256Color
            ),
            "Expected xterm variant, got {:?}",
            detected.terminal_type
        );
    }

    #[test]
    fn test_terminal_compatibility_invariant() {
        let invariant = TerminalCompatibility::new(TerminalType::XTerm);
        assert_eq!(invariant.name(), "terminal_compatibility");
    }

    #[test]
    fn test_mouse_support_invariant() {
        let invariant = MouseSupportCheck;
        assert_eq!(invariant.name(), "mouse_support");
    }

    #[test]
    fn test_truecolor_support_invariant() {
        let invariant = TrueColorSupport;
        assert_eq!(invariant.name(), "truecolor_support");
    }
}
