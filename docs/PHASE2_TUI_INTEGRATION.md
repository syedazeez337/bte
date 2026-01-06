# Phase 2 Implementation: TUI Framework Integration

## Overview

Phase 2 focuses on integrating BTE with major TUI frameworks to become the **default testing solution** for terminal applications.

## Target Frameworks

| Framework | Stars | Language | Integration Priority |
|-----------|-------|----------|---------------------|
| **ratatui** | 17.2k | Rust | HIGH (current) |
| **textual** | 12.6k | Python | MEDIUM |
| **blessed** | - | Python | LOW |

## Ratatui Integration Plan

### 1. Create BTE Plugin for Ratatui

**File**: `src/ratatui_integration.rs`

```rust
use crate::{Scenario, Invariant, Tester};

pub struct RatatuiTester<'a> {
    scenario: &'a Scenario,
    invariants: Vec<Box<dyn Invariant>>,
}

impl<'a> RatatuiTester<'a> {
    pub fn new(scenario: &'a Scenario) -> Self {
        Self {
            scenario,
            invariants: Vec::new(),
        }
    }

    /// Verify widgets stay within layout constraints
    pub fn widget_bounds(self) -> Self {
        self.add_invariant(RatatuiWidgetBounds)
    }

    /// Verify text doesn't overflow widgets
    pub fn text_overflow(self) -> Self {
        self.add_invariant(RatatuiTextOverflow)
    }

    /// Verify focus cycling works correctly
    pub fn focus_cycling(self) -> Self {
        self.add_invariant(RatatuiFocusCycling)
    }

    pub fn run(&self) -> TestResult {
        // Run the scenario with ratatui-specific invariants
    }
}

pub struct RatatuiWidgetBounds;

impl Invariant for RatatuiWidgetBounds {
    fn name(&self) -> &str {
        "ratatui_widget_bounds"
    }

    fn check(&self, screen: &Screen) -> Result<(), Violation> {
        // Check all widget rects are within screen bounds
        // Check widgets don't overlap incorrectly
    }
}
```

### 2. Create Ratatui-Specific Scenario Steps

**New Step Types**:
```rust
pub enum RatatuiStep {
    /// Trigger a widget action (click, focus, etc.)
    WidgetAction {
        widget_id: String,
        action: WidgetAction,
    },

    /// Verify widget state
    AssertWidgetState {
        widget_id: String,
        expected: WidgetState,
    },

    /// Navigate focus
    FocusNext,
    FocusPrevious,
    FocusById(String),
}
```

### 3. Create Ratatui Example Applications

**Example 1: Counter App** (`examples/ratatui/counter.rs`)
- Simple counter with increment/decrement
- Tests basic key handling
- Validates screen updates

**Example 2: Form App** (`examples/ratatui/form.rs`)
- Multiple input fields
- Focus navigation
- Validation display

**Example 3: Layout App** (`examples/ratatui/layout.rs`)
- Complex layout with panels
- Resize handling
- Multiple widgets

### 4. Create Test Scenarios

```yaml
# examples/ratatui/counter.yaml
name: ratatui-counter-test
steps:
  - type: spawn
    command: cargo run --example counter --manifest-path examples/ratatui/Cargo.toml
  - type: wait: 500ms
  - type: expect_screen: "Counter: 0"
  - type: send_keys: ["i", "i", "i"]
  - type: expect_screen: "Counter: 3"
  - type: send_keys: ["d"]
  - type: expect_screen: "Counter: 2"
  - type: send_keys: ["q"]
```

## Implementation Tasks

### Task 1: Core Integration Module (Week 1)
- [x] Create `src/ratatui_integration.rs`
- [ ] Implement `RatatuiTester` struct
- [ ] Add ratatui-specific invariants
- [ ] Create helper methods

### Task 2: Example Applications (Week 1)
- [x] Counter app (`examples/ratatui/counter.rs`)
- [ ] Form app (`examples/ratatui/form.rs`)
- [ ] Layout app (`examples/ratatui/layout.rs`)

### Task 3: Scenarios (Week 2)
- [x] Counter scenario (`examples/ratatui/counter.yaml`)
- [ ] Form scenario
- [ ] Layout scenario

### Task 4: Documentation (Week 2)
- [ ] Write integration guide
- [ ] Create API documentation
- [ ] Add examples to README

## Testing Strategy

### Unit Tests
- Invariant implementations
- Step parsing
- Widget state assertions

### Integration Tests
- Run ratatui examples with BTE
- Verify screen content
- Check key handling

### Performance Tests
- Measure test execution time
- Verify no regressions
- Track memory usage

## API Design

### High-Level API
```rust
use bte_ratatui::{RatatuiTester, RatatuiScenario};

// Create a test for a ratatui application
let tester = RatatuiTester::new("my-app")
    .widget_bounds()        // Check widget positions
    .text_overflow()        // Check text fits
    .focus_cycling()        // Check focus navigation
    .run();

// Check results
assert!(tester.passed());
assert_eq!(tester.violations().len(), 0);
```

### Low-Level API
```rust
use bte_ratatui::{RatatuiWidget, RatatuiLayout};

// Define custom widget to test
let widget = RatatuiWidget::new("counter")
    .at(0, 0)
    .size(20, 10)
    .content("Counter: 0");

// Assert state
assert_eq!(widget.content(), "Counter: 0");
assert!(widget.is_focused());
```

## Comparison with Existing Solutions

### vs pyte
| Feature | BTE + Ratatui | pyte |
|---------|--------------|------|
| Deterministic replay | ✓ | ✗ |
| PTY-based | ✓ | ✗ |
| Key injection | ✓ | ✗ |
| Invariants | ✓ | ✗ |
| Sparse traces | ✓ | ✗ |

### vs libvte
| Feature | BTE + Ratatui | libvte |
|---------|--------------|-------|
| Easy integration | ✓ | ✗ |
| Rust support | ✓ | ✗ |
| Scenarios | ✓ | ✗ |
| CI integration | ✓ | ✗ |

## Deliverables for v0.4.0

1. **`bte-ratatui`** crate with:
   - `RatatuiTester` struct
   - `RatatuiWidgetBounds` invariant
   - `RatatuiTextOverflow` invariant
   - `RatatuiFocusCycling` invariant

2. **3 example applications**:
   - Counter (basic interaction)
   - Form (focus navigation)
   - Layout (complex widgets)

3. **5 test scenarios**:
   - Counter increment/decrement
   - Form navigation
   - Layout resize
   - Focus cycling
   - Error handling

4. **Documentation**:
   - Integration guide
   - API reference
   - Example tutorials

## Success Metrics

| Metric | Target |
|--------|--------|
| Ratatui integration tests | 5+ passing |
| Example applications | 3 |
| Scenario coverage | 100% of examples |
| Documentation pages | 10+ |
| Community interest | 50+ GitHub stars on integration |

## Next Steps

1. Complete `src/ratatui_integration.rs` implementation
2. Add form and layout examples
3. Create comprehensive scenarios
4. Write integration documentation
5. Announce on ratatui Discord/GitHub
