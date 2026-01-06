# BTE Strategic Feature Roadmap

> **Strategic truth**: Browsers have Playwright. Compilers have fuzzers. Distributed systems have Jepsen. Terminal software has nothing equivalent today. That's the gap BTE fills.

---

## Executive Summary

This document outlines the strategic feature roadmap for BTE (Behavioral Testing Engine), organized into tiers based on market impact and implementation priority.

**Current Status**: v0.2.0-rc1 complete with core functionality
**Target Status**: v1.0.0 infrastructure-grade terminal testing

---

## Tier 1 — Credibility Features ✅ COMPLETE

*These turned BTE from "interesting" into "usable in production"*

### 1️⃣ Deterministic Record & Replay (First-Class) ✅ DONE

**Status**: **COMPLETE** (v0.2.0)
**Location**: `src/replay.rs`

- Full trace format v2.0.0 with event recording
- Key timing, resize ordering, signal ordering
- Screen checkpoints with hash verification
- Partial replay from any checkpoint
- Trace checksums with seahash

### 2️⃣ Exit Semantics & Crash Classification ✅ DONE

**Status**: **COMPLETE** (v0.2.0)
**Location**: `src/termination.rs`

- 9 termination classifications (CleanExit, SignalExit, Panic, Deadlock, Timeout, InvariantViolation, ReplayDivergence, UserInterrupt, Unknown)
- Performance metrics (ticks, memory, I/O)
- CI-friendly `CISummary` format
- Exit code mapping (-1 to -99)
      tick: 200
      signal: SIGINT
      source: "user"
```

#### New Capabilities

| Feature | Description | Use Case |
|---------|-------------|----------|
| Key timing | Record inter-key delays | Reproduce race conditions |
| Resize ordering | Track resize sequence | Debug layout bugs |
| Signal ordering | Record signal sequence | Test signal handlers |
| Partial replay | Replay from step N | Debug CI failures |
| Logical timestamps | No wall-clock dependency | Determinism guarantee |

#### Implementation Plan

1. Extend `TraceStep` to include event timing
2. Add `EventRecorder` for capturing all inputs
3. Implement `PartialReplay` from checkpoint
4. Add `ReplayController` with seek functionality
5. Verify determinism with multiple replay runs

#### Commercial Value

- CI failures become debuggable → Enterprise adoption
- Maintainers trust it → Community growth
- AI agents can bisect regressions → AI-native workflows

---

### 2️⃣ Cross-Platform Terminal Abstraction

**Status**: Linux-only (nix crate)
**Priority**: HIGH
**Roadmap**: v0.3.0 (Linux), v0.5.0 (macOS), v0.7.0 (Windows)

#### Platform Support Matrix

| Platform | PTY Mechanism | Status | ETA |
|----------|--------------|--------|-----|
| Linux | nix::pty | ✅ Done | v0.1.0 |
| macOS | Ikitty (termios) | 🔜 | v0.5.0 |
| Windows | ConPTY (winapi) | 🔜 | v0.7.0 |

#### Abstraction Layer

```rust
trait TerminalBackend {
    fn spawn(&self, config: &SpawnConfig) -> Result<Box<dyn TerminalProcess>>;
    fn resize(&self, process: &mut dyn TerminalProcess, cols: u16, rows: u16) -> Result<()>;
    fn send_signal(&self, process: &mut dyn TerminalProcess, signal: Signal) -> Result<()>;
    fn read_output(&self, process: &dyn TerminalProcess, timeout_ms: u32) -> Result<Vec<u8>>;
}
```

#### Why This Matters

- CLI/TUI tooling is inherently cross-platform
- GitHub Actions runs Linux/macOS/Windows
- Large OSS projects expect multi-platform support

**Market signal**: Linux-only forever = niche. Linux-first with clear roadmap = infrastructure.

---

### 3️⃣ Exit Semantics & Crash Classification ✅ DONE

**Status**: **COMPLETE** (v0.2.0)
**Location**: `src/termination.rs`

- 9 termination classifications (CleanExit, SignalExit, Panic, Deadlock, Timeout, InvariantViolation, ReplayDivergence, UserInterrupt, Unknown)
- Performance metrics (ticks, memory, I/O)
- CI-friendly `CISummary` format
- Exit code mapping (-1 to -99)

---

## Tier 2 — Differentiation Features 🏆

*These are features no existing tool does well*

### 4️⃣ Behavioral Invariants as First-Class Language 🚧 IN PROGRESS

**Status**: **IN PROGRESS** (v0.2.0-rc1)
**Location**: `src/invariants_v2.rs`

- 16+ invariant types (cursor_bounds, screen_size, response_time, screen_stability, etc.)
- Parameterized invariants with configurable thresholds
- Regex-based content invariants
- State tracking via Mutex for deterministic evaluation
- Declarative YAML/JSON syntax

#### Invariant Types Implemented

```yaml
invariants:
  # Built-in invariants
  - cursor_in_bounds
  - no_output_after_exit
  - process_terminated_cleanly
  
  # Parameterized invariants
  - type: response_time
    max_ticks: 50
    description: "UI must respond within 50 ticks"
  
  - type: screen_stability
    min_ticks: 20
    description: "Screen must stabilize before next input"
  
  - type: output_growth
    max_growth_bytes: 10000
    per_step: true
  
  # Custom regex invariants
  - type: pattern_absence
    pattern: "error"
    after_step: login_complete
    description: "No error messages after login"
  
  - type: cursor_progression
    expected_direction: "right"
    tolerance: 2
```

#### Invariant Categories

| Category | Examples | Purpose |
|----------|----------|---------|
| **Bounds** | cursor_in_bounds, viewport_valid | Structural correctness |
| **Temporal** | response_time, screen_stability | Performance correctness |
| **Output** | output_growth, pattern_absence | Content correctness |
| **State** | process_terminated_cleanly | Lifecycle correctness |
| **Custom** | user_defined_regex | Domain-specific |

#### Implementation Plan

1. Create `Invariant` trait with evaluation context
2. Add `InvariantSpec` enum for YAML/JSON schema
3. Implement parameterized invariant constructor
4. Add regex-based invariants
5. Create invariant documentation generator

#### Commercial Value

- Teams express intent, not implementation
- AI can generate invariants from requirements
- **Intellectual moat** - this is BTE's equivalent to Playwright's DOM assertions

---

### 5️⃣ Time-Aware Correctness ✅ DONE

**Status**: **COMPLETE** (v0.2.0-rc1)
**Location**: `src/invariants_v2.rs`

#### Implemented Time-Aware Invariants

| Invariant | Purpose |
|-----------|---------|
| `response_time` | Process must respond within max ticks |
| `max_latency` | Screen redraw must complete within max latency |
| `screen_stability` | Screen must stabilize for minimum ticks |
| `input_response_time` | Measure time from input to screen change |
| `max_redraw_latency` | Measure screen redraw latency after events |
| `ui_stabilized` | UI must be stable before proceeding |

#### Key Implementation Details

- Mutex-based state tracking for deterministic evaluation
- Multiple measurement points (FirstChange, StableState, CursorPosition)
- Latency measurement methods (HashChange, ContentDiff, CursorMovement, AnyChange)
- Configurable stability thresholds and timeouts

---

### 6️⃣ Terminal Fuzzing Engine ✅ DONE

**Status**: **COMPLETE** (v0.2.0-rc1)
**Location**: `src/fuzzing.rs`

#### Implemented Fuzzing Strategies

| Strategy | Description | Purpose |
|----------|-------------|---------|
| `key_sequence` | Valid key combinations with modifiers | Realistic input |
| `resize_storm` | Rapid terminal resize events | Layout stress |
| `signal_injection` | Randomized signal delivery | Race conditions |
| `input_flood` | Burst input sequences | Timing bugs |

#### Key Features

- **Deterministic**: Every fuzz run has a seed for reproduction
- **Structured**: Generates valid inputs, not random bytes
- **Configurable**: Intensity levels (Low/Medium/High/Extreme)
- **Composable**: Multiple strategies can be combined

#### Example Usage

```yaml
fuzz:
  enabled: true
  seed: 12345
  strategies:
    - type: key_sequence
      min_length: 5
      max_length: 20
      include_modifiers: true
    - type: resize_storm
      min_size: [40, 10]
      max_size: [200, 60]
      burst_count: 10
```

---

## Tier 3 — AI-Native Features 🔮

*Future-proofing for 2026+*

### 7️⃣ AI-Consumable Failure Explanations ✅ DONE

**Status**: **COMPLETE** (v0.2.0-rc1)
**Location**: `src/explain.rs`

#### Implemented Features

| Feature | Description |
|---------|-------------|
| Violation Classification | 20+ violation types with severity/category |
| Causal Chain | Event sequence leading to failure |
| Minimal Reproduction | Scenario name, seed, step count, duration |
| Suggested Fixes | AI-generated fix suggestions with confidence |
| Related Issues | Similar historical issues with similarity scores |

#### Example Output

```json
{
  "version": "1.0",
  "timestamp": "2024-01-06T10:30:00Z",
  "scenario": "resize_test",
  "exit_code": -2,
  "outcome": "InvariantViolation",
  "failures": [{
    "violation": {
      "type": "cursor_bounds",
      "severity": "critical",
      "category": "structural",
      "description": "Cursor position must always be within screen bounds",
      "step": 3,
      "tick": 150
    },
    "causal_chain": [
      {"event_type": "invariant_check", "tick": 150, "consequence": "Invariant 'cursor_bounds' check failed"},
      {"event_type": "violation_details", "tick": 150, "consequence": "Violation: Cursor at (80, 24) but screen is 40x12"}
    ],
    "minimal_repro": {
      "scenario_name": "resize_test",
      "step_count": 5,
      "seed": 12345,
      "duration_ticks": 200
    },
    "suggested_fixes": [
      {"description": "Add cursor position bounds check after any terminal resize", "confidence": 0.85}
    ],
    "related_issues": [
      {"id": "#42", "similarity": 0.85},
      {"id": "#87", "similarity": 0.72}
    ]
  }],
  "summary": {
    "total_failures": 1,
    "critical_count": 1,
    "high_count": 0,
    "categories": [["Structural", 1]],
    "top_violation_types": [["CursorBounds", 1]]
  }
}
```

#### Design Principles

1. **Normalized types** → AI can categorize
2. **Causal chains** → AI can reason about cause
3. **Minimal repro** → AI can iterate quickly
4. **Suggested actions** → AI can automate fixes

---

### 8️⃣ Minimal Reproduction Synthesizer

**Status**: Not implemented
**Priority**: LOW
**Implementation**: v0.5.0

#### Algorithm

1. Start with full failing scenario
2. Binary search to find minimal failing subset
3. Try removing steps, simplifying inputs
4. Verify failure still occurs
5. Output shortest equivalent scenario

#### Output

```yaml
name: "minimal_crash_repro"
steps:
  - resize: {cols: 40, rows: 12}
  - send_keys: "x"
invariant_violations: ["cursor_out_of_bounds"]
seed: 12345
original_scenario: "full_ui_test.yaml"
reduction_ratio: "94%"
```

---

### 9️⃣ Language-Agnostic Positioning

**Status**: Rust implementation, Rust-agnostic usage
**Priority**: Already satisfied
**Implementation**: Ongoing

#### Position Statement

> BTE is a **behavioral correctness engine for terminal software**, not a "Rust testing tool."

#### Supported Languages (All with terminal UI)

| Language | Example Projects | Status |
|----------|-----------------|--------|
| Rust | bat, eza, zellij | ✅ Tested |
| C/C++ | vim, neovim, htop | ✅ Works |
| Go | docker, kubectl, glow | ✅ Works |
| Python | httpie, rich, tui | ✅ Works |
| Node | chalk, inquirer | ✅ Works |

---

## Tier 4 — Ecosystem & Adoption 🌐

*Non-technical but critical*

### 🔟 CI-First UX ✅ DONE

**Status**: **COMPLETE** (v0.2.0-rc1)
**Location**: `src/ci.rs`

#### Implemented Features

| Feature | Description |
|---------|-------------|
| Batch Result Aggregation | Combine multiple scenario results |
| Machine-Readable Summaries | JSON format with pass/fail counts |
| GitHub Actions Template | Pre-built CI workflow |
| Exit Code Mapping | CI-friendly exit codes (-1 to -99) |
| Failure Artifacts | Automatic trace file management |

#### Example Summary Output

```json
{
  "version": "1.0",
  "timestamp": "2024-01-06T10:30:00Z",
  "total_scenarios": 15,
  "passed": 14,
  "failed": 1,
  "success_rate": "93.3%",
  "failures": [{
    "scenario": "interactive/vim-complex.yaml",
    "violation_type": "cursor_bounds",
    "severity": "critical",
    "exit_code": -2,
    "repro_seed": 12345
  }],
  "artifacts_dir": "test-results/"
}
```

#### Exit Code Mapping

| Exit Code | Meaning |
|-----------|---------|
| 0 | Success |
| -1 | Signal exit |
| -2 | Invariant violation |
| -3 | Timeout |
| -4 | Error |
| -5 | Replay divergence |
| -99 | Panic |

#### GitHub Actions Integration

The module generates ready-to-use GitHub Actions workflows with:
- Artifact upload
- Failure detection
- Warning annotations

---

### 1️⃣1️⃣ Opinionated Defaults ✅ DONE

**Status**: **COMPLETE** (v0.2.0-rc1)
**Location**: `src/defaults.rs`

#### Implemented Defaults

| Category | Default | Value |
|----------|---------|-------|
| Terminal | cols/rows | 80x24 |
| Timing | tick_nanos | 10_000_000 (10ms) |
| Timing | max_default_ticks | 10_000 |
| Invariants | cursor_bounds | true |
| Invariants | no_deadlock | true (1000 tick timeout) |
| Invariants | no_output_after_exit | true |
| Retry | max_attempts | 3 |
| Output | max_size | 1MB |

#### Smart Configuration

- **Auto-sizes terminal** (40-200 cols, 10-100 rows)
- **Command-aware invariants** (editors get screen_stability, builders get process_terminated_cleanly)
- **Estimated timeouts** (vim: 5s, cargo test: 30s, simple: 500ms)
- **Scenario templates** (interactive, headless, resize_test, performance)

#### Example

```rust
let scenario = DefaultConfigurator::build_scenario_with_defaults(
    "my_test",
    "cargo build",
    vec![],
);
// Automatically sets:
// - Terminal: 80x24
// - Invariants: no_deadlock, process_terminated_cleanly, no_output_after_exit
// - Timeout: 30000ms (for cargo build)
```

---

### 1️⃣2️⃣ Public Philosophy & Guarantees

**Status**: Partial documentation
**Priority**: MEDIUM
**Implementation**: v0.2.0

#### Guarantee Document

**Determinism Guarantee**:
> Given identical seed and scenario, BTE produces identical execution traces.

**Replay Guarantee**:
> Any trace recorded by BTE can be replayed to produce identical checkpoints.

**What We Model**:
- PTY input/output
- Signal delivery
- Terminal resize
- Key timing (with tolerance)

**What We Don't Model**:
- Network timing variability
- External process interference
- Hardware acceleration timing

---

## Implementation Roadmap

### v0.2.0 (Next Release) - "Credibility Release"

| Feature | Status | Lines Added |
|---------|--------|-------------|
| Deterministic Replay (enhanced) | 🔄 In Progress | ~500 |
| Exit Semantics v2 | 🔄 In Progress | ~300 |
| Invariant Language v2 | 📋 Planned | ~800 |
| CI Templates | 📋 Planned | ~200 |
| **Total** | | **~1800** |

### v0.3.0 - "Correctness Release"

| Feature | Status | Lines Added |
|---------|--------|-------------|
| Time-Aware Invariants | 📋 Planned | ~600 |
| AI Failure Explanations | 📋 Planned | ~400 |
| Opinionated Defaults | 📋 Planned | ~200 |
| **Total** | | **~1200** |

### v0.4.0 - "Fuzzing Release"

| Feature | Status | Lines Added |
|---------|--------|-------------|
| Terminal Fuzzing | 📋 Planned | ~1000 |
| Minimal Repro Synthesizer | 📋 Planned | ~500 |
| **Total** | | **~1500** |

### v0.5.0 - "Cross-Platform Release"

| Feature | Status | Lines Added |
|---------|--------|-------------|
| macOS Support | 📋 Planned | ~1500 |
| Partial Replay UI | 📋 Planned | ~300 |
| **Total** | | **~1800** |

---

## Success Metrics

### Quantitative

| Metric | v0.1.0 | v0.2.0 | v1.0.0 |
|--------|--------|--------|--------|
| Test scenarios | 11 | 50 | 200 |
| CI templates | 0 | 3 | 10 |
| Invariant types | 6 | 15 | 30 |
| Platform support | 1 | 1 | 3 |
| Determinism guarantee | 95% | 99% | 100% |

### Qualitative

- [ ] 100+ GitHub stars
- [ ] 10+ OSS projects using BTE
- [ ] 3+ companies in production
- [ ] 1 conference talk
- [ ] Published blog post with case study

---

## What NOT to Build

| Feature | Why Not |
|---------|---------|
| GUI dashboards | Dilutes focus, CI-first is better |
| Cloud SaaS early | No value without users |
| Plugin ecosystem | Premature abstraction |
| Screenshot diffing | Wrong abstraction level |
| YAML explosion | Keep schema minimal |

---

## Conclusion

This roadmap transforms BTE from a "terminal testing tool" into **infrastructure for terminal software correctness**.

The strategic pillars:
1. **Credibility** → Determinism, cross-platform, crash classification
2. **Differentiation** → Invariant language, time-awareness, fuzzing
3. **AI-Native** → Machine-readable failures, minimal repro
4. **Ecosystem** → CI-first, defaults, guarantees

At v1.0.0, BTE will be the Playwright for terminals.

---

**Document Version**: 1.0.0
**Last Updated**: 2026-01-06
**Next Review**: v0.2.0 release
