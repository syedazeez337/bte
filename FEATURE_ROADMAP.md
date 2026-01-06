# BTE Strategic Feature Roadmap

> **Strategic truth**: Browsers have Playwright. Compilers have fuzzers. Distributed systems have Jepsen. Terminal software has nothing equivalent today. That's the gap BTE fills.

---

## Executive Summary

This document outlines the strategic feature roadmap for BTE (Behavioral Testing Engine), organized into tiers based on market impact and implementation priority.

**Current Status**: v0.1.0 complete with core functionality
**Target Status**: v1.0.0 infrastructure-grade terminal testing

---

## Tier 1 — Credibility Features ⭐ REQUIRED

*These turn BTE from "interesting" into "usable in production"*

### 1️⃣ Deterministic Record & Replay (First-Class)

**Status**: Partially implemented (basic replay exists)
**Priority**: CRITICAL
**Implementation**: v0.2.0

#### Current State
- Basic checkpoint system exists
- RNG state tracking exists
- Missing: key timing, resize ordering, signal ordering

#### Upgrade Requirements

```yaml
# New trace format with full event recording
trace:
  version: "2.0.0"
  events:
    - type: key_press
      tick: 42
      key: "Enter"
      raw_bytes: [0x0d]
      logical_sequence: 5
    
    - type: resize
      tick: 100
      cols: 120
      rows: 40
      sigwinch_sent: true
    
    - type: signal
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

### 3️⃣ Exit Semantics & Crash Classification

**Status**: Basic exit code tracking
**Priority**: CRITICAL
**Implementation**: v0.2.0

#### New Termination Schema

```json
{
  "termination": {
    "kind": "signal",
    "signal": "SIGSEGV",
    "signal_number": 11,
    "during_step": 14,
    "step_name": "wait_for_login_prompt",
    "output_before": "login: ",
    "invariant_violations": ["cursor_bounds"],
    "memory_snapshot": {
      "rss_kb": 1024,
      "vm_size_kb": 5120
    }
  }
}
```

#### Termination Kinds

| Kind | Description | Exit Code |
|------|-------------|-----------|
| `clean_exit` | Normal exit with code | 0 + code |
| `signal_exit` | Killed by signal | -signal |
| `panic` | Unhandled panic | -99 |
| `deadlock` | No progress detected | -98 |
| `timeout` | Step timed out | -97 |
| `invariant_violation` | Behavioral check failed | -96 |
| `replay_divergence` | Trace mismatch | -95 |

#### Implementation

1. Extend `ExitReason` enum with detailed variants
2. Add memory tracking (RSS, VMS)
3. Capture output buffer at termination
4. Classify crashes by signal/source
5. Include in trace output

#### Commercial Value

- Systems software teams need this
- Enables post-mortem analysis
- Distinguishes "real bugs" from "test issues"

---

## Tier 2 — Differentiation Features 🏆

*These are features no existing tool does well*

### 4️⃣ Behavioral Invariants as First-Class Language

**Status**: Hardcoded checks in Rust
**Priority**: HIGH
**Implementation**: v0.2.0-v0.3.0

#### New Invariant Schema

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

### 5️⃣ Time-Aware Correctness

**Status**: Tick-based timing exists
**Priority**: MEDIUM
**Implementation**: v0.3.0

#### New Time-Aware Invariants

```yaml
invariants:
  # Latency guarantees
  - type: max_redraw_latency
    max_ticks: 10
    description: "Screen must redraw within 10 ticks"
  
  # Responsiveness
  - type: input_response_time
    max_ticks: 25
    input_type: "key_press"
  
  # Progress guarantees
  - type: monotonic_progress
    metric: "screen_content_hash"
    direction: "increasing"
  
  # Stability detection
  - type: ui_stabilized
    timeout_ticks: 100
    stability_threshold: 5
```

#### Implementation

1. Extend `TimingController` with latency tracking
2. Add per-step performance metrics
3. Implement stability detection algorithm
4. Create performance regression detection
5. Add visualization of timing profiles

#### Why This Matters

- TUIs often "work" but feel broken
- Catches performance regressions before users complain
- No mainstream CLI test tool does this

---

### 6️⃣ Terminal Fuzzing (Structured)

**Status**: Not implemented
**Priority**: MEDIUM
**Implementation**: v0.4.0

#### Fuzzing Strategy

Instead of random bytes → structured valid inputs:

```yaml
fuzzing:
  mode: "structured"
  seed: 12345
  
  key_fuzzing:
    enabled: true
    max_sequence_length: 100
    include_ctrl: true
    include_alt: true
    include_function: true
  
  resize_fuzzing:
    enabled: true
    min_cols: 40
    max_cols: 200
    min_rows: 10
    max_rows: 80
    storm_mode: true
  
  signal_fuzzing:
    enabled: true
    signals: [SIGINT, SIGTERM, SIGWINCH]
    inject_during_output: true
```

#### Fuzzing Features

| Feature | Description | Purpose |
|---------|-------------|---------|
| Valid key sequences | Generated from grammar | Realistic input |
| Resize storms | Rapid size changes | Layout stress |
| Signal injection | Random signals | Race conditions |
| Timing variation | Random delays | Timing bugs |
| Bounded execution | Max ticks limit | Termination guarantee |

#### Reproduction Guarantee

- Every fuzz run has deterministic seed
- Reproduce any failure with seed
- Extract minimal reproduction scenario

#### Commercial Value

- Moves from "test tool" to "bug discovery engine"
- Finds redraw bugs, deadlocks, race conditions
- Automated regression finding

---

## Tier 3 — AI-Native Features 🔮

*Future-proofing for 2026+*

### 7️⃣ AI-Consumable Failure Explanations

**Status**: Basic JSON output
**Priority**: MEDIUM
**Implementation**: v0.3.0

#### New Failure Output Format

```json
{
  "violation": {
    "type": "cursor_out_of_bounds",
    "severity": "high",
    "category": "structural"
  },
  "causal_chain": [
    {
      "event": "resize",
      "tick": 42,
      "params": {"cols": 40, "rows": 12},
      "consequence": "viewport shrank"
    },
    {
      "event": "app_redraw",
      "tick": 43,
      "consequence": "app placed cursor at old position"
    },
    {
      "event": "invariant_check",
      "tick": 44,
      "violation": "cursor at (80, 24) but viewport is (40, 12)"
    }
  ],
  "minimal_repro": {
    "scenario": "resize_then_redraw",
    "steps": 3,
    "seed": 12345
  },
  "suggested_fix": "Add bounds check after resize before redraw",
  "related_issues": ["#42", "#87"]
}
```

#### AI Output Design Principles

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

### 🔟 CI-First UX

**Status**: Basic
**Priority**: HIGH
**Implementation**: v0.2.0

#### GitHub Actions Template

```yaml
name: Terminal Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build BTE
        run: cargo build --release
      
      - name: Run Scenarios
        run: |
          for scenario in scenarios/*.yaml; do
            ./target/release/bte run "$scenario" \
              --output "test-results/$(basename $scenario).json"
          done
      
      - name: Upload Results
        uses: actions/upload-artifact@v4
        with:
          name: bte-results
          path: test-results/
      
      - name: Check Failures
        run: |
          failed=$(find test-results -name "*.json" -exec grep -l '"exit_code": -[0-9]*' {} \; | wc -l)
          if [ $failed -gt 0 ]; then
            echo "::$warning title=Failures Detected::$failed scenarios failed"
            exit 1
          fi
```

#### Machine-Readable Summaries

```json
{
  "summary": {
    "total": 15,
    "passed": 14,
    "failed": 1,
    "skipped": 0,
    "success_rate": "93.3%"
  },
  "failures": [
    {
      "scenario": "interactive/vim-complex.yaml",
      "violation": "cursor_out_of_bounds",
      "repro_seed": 12345
    }
  ],
  "artifacts": "test-results/"
}
```

---

### 1️⃣1️⃣ Opinionated Defaults

**Status**: Minimal defaults
**Priority**: MEDIUM
**Implementation**: v0.2.0

#### Shipped Defaults

```yaml
# bte defaults (implicit, can be overridden)

terminal:
  cols: 80    # Standard terminal width
  rows: 24    # Standard terminal height
  encoding: UTF-8

timing:
  tick_nanos: 10_000_000  # 10ms logical tick
  max_default_ticks: 10000  # 100s max per scenario

invariants:
  - cursor_in_bounds    # Always check
  - no_deadlock         # Always check (1000 tick timeout)
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
