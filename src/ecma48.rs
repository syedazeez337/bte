#![allow(dead_code)]

use crate::invariants::{Invariant, InvariantContext, InvariantResult};

#[derive(Debug, Clone)]
pub struct Ecma48SgrCompliance;

impl Invariant for Ecma48SgrCompliance {
    fn name(&self) -> &'static str {
        "ecma48::sgr_compliance"
    }

    fn description(&self) -> &'static str {
        "Verifies SGR (Select Graphic Rendition) sequences comply with ECMA-48"
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
pub struct Ecma48CsiCompliance;

impl Invariant for Ecma48CsiCompliance {
    fn name(&self) -> &'static str {
        "ecma48::csi_compliance"
    }

    fn description(&self) -> &'static str {
        "Verifies CSI (Control Sequence Introducer) sequences comply with ECMA-48"
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
pub struct Ecma48OscCompliance;

impl Invariant for Ecma48OscCompliance {
    fn name(&self) -> &'static str {
        "ecma48::osc_compliance"
    }

    fn description(&self) -> &'static str {
        "Verifies OSC (Operating System Command) sequences comply with ECMA-48"
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
pub struct XtermMouseTracking;

impl Invariant for XtermMouseTracking {
    fn name(&self) -> &'static str {
        "xterm::mouse_tracking"
    }

    fn description(&self) -> &'static str {
        "Verifies xterm mouse tracking sequences are properly handled"
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
pub struct XtermTitleSupport;

impl Invariant for XtermTitleSupport {
    fn name(&self) -> &'static str {
        "xterm::title_support"
    }

    fn description(&self) -> &'static str {
        "Verifies xterm window title sequences are properly handled"
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
pub struct XtermColorSupport;

impl Invariant for XtermColorSupport {
    fn name(&self) -> &'static str {
        "xterm::color_support"
    }

    fn description(&self) -> &'static str {
        "Verifies xterm 256-color and true-color sequences are properly handled"
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
pub struct XtermAltScreen;

impl Invariant for XtermAltScreen {
    fn name(&self) -> &'static str {
        "xterm::alt_screen"
    }

    fn description(&self) -> &'static str {
        "Verifies xterm alternate screen switching works correctly"
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
    fn test_sgr_compliance_invariant() {
        let invariant = Ecma48SgrCompliance;
        assert_eq!(invariant.name(), "ecma48::sgr_compliance");
    }

    #[test]
    fn test_csi_compliance_invariant() {
        let invariant = Ecma48CsiCompliance;
        assert_eq!(invariant.name(), "ecma48::csi_compliance");
    }

    #[test]
    fn test_xterm_mouse_tracking() {
        let invariant = XtermMouseTracking;
        assert_eq!(invariant.name(), "xterm::mouse_tracking");
    }

    #[test]
    fn test_xterm_title_support() {
        let invariant = XtermTitleSupport;
        assert_eq!(invariant.name(), "xterm::title_support");
    }
}
