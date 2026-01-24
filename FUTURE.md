# BTE Future Roadmap

> Comprehensive plan for making BTE a complete end-to-end solution for CLI/TUI testing.

---

## Executive Summary

**Vision**: Become the Playwright for terminal applications - the standard for behavioral testing of CLI/TUI software.

**Current Status**: v0.2.0 with core functionality (deterministic PTY execution, invariants, replay)

**Target Status**: v1.0.0 infrastructure-grade terminal testing platform

---

## Missing Actions (Required for Real-World Testing)

### Critical - Phase 1

| Action | Description | Priority |
|--------|-------------|----------|
| `mouse_click(row, col)` | Click at position | Critical |
| `mouse_scroll(direction, count)` | Scroll up/down | Critical |
| `mouse_drag(from, to)` | Drag selection | High |
| `paste(text)` | Clipboard paste | High |
| `wait_screen(pattern)` | Wait for screen content | High |
| `assert_not_screen(pattern)` | Negative assertion | Medium |

### High - Phase 2

| Action | Description | Priority |
|--------|-------------|----------|
| `timeout_ms` per step | Per-step timeout override | High |
| `loop/while` | Conditional logic | Medium |
| `background(command)` | Spawn background process | Medium |
| `read_file(path)` | Read file content | Medium |
| `write_file(path, content)` | Write file content | Medium |
| `wait_not_screen(pattern)` | Wait for pattern to disappear | Medium |

### Nice to Have

| Action | Description |
|--------|-------------|
| `focus_event(event)` | Window focus/blur |
| `set_env(key, value)` | Set environment variable |
| `take_screenshot()` | Capture visual output |
| `wait_till_stable()` | Wait for screen stability |

### Key Name Support

Currently supported: `Enter`, `Escape`, `Tab`, `Backspace`, `Up`, `Down`, `Left`, `Right`, `Ctrl_c`, `Ctrl_d`, `Ctrl_z`

**Missing**: Function keys F1-F12, `Ctrl_a` through `Ctrl_z` (complete set), `Alt_*` combinations

---

## Missing Invariants

### Core Invariants (Current)

```
✅ cursor_bounds, no_deadlock, screen_contains, screen_not_contains,
   screen_changed, screen_stable, viewport_valid, response_time,
   max_latency, no_output_after_exit, process_terminated_cleanly
```

### Missing for Comprehensive Testing

| Invariant | Purpose | Priority |
|-----------|---------|----------|
| `no_memory_leak` | RSS stays reasonable | High |
| `no_screen_flicker` | Detect rapid unnecessary redraws | High |
| `color_palette_valid` | Colors in valid range | Medium |
| `unicode_width_correct` | CJK renders as double-width | Medium |
| `tab_expansion_correct` | Tabs expand to 8 spaces | Medium |
| `hyperlink_valid` | OSC 8 links well-formed | Medium |
| `no_ansi_injection` | No dangerous escape sequences | High |
| `cursor_blink_consistent` | Blink state toggles properly | Low |
| `scrollback_contains` | Historical content preserved | Medium |
| `response_time_percentile` | P95/P99 latency bounds | Medium |

### Custom Invariants (BLOCKED)

**Status**: Currently panics if user tries to add custom invariants

Required implementation:
```rust
pub trait Invariant {
    fn name(&self) -> &str;
    fn check(&self, screen: &Screen, context: &Context) -> Result<(), Violation>;
}
```

---

## Unsupported ANSI Escape Sequences

### Currently Supported

- C0 control codes (0x00-0x1F)
- Basic CSI sequences (cursor, erase, SGR 0-7)
- ESC sequences (save/restore cursor)
- OSC (Operating System Command) - title setting

### Missing (Modern Terminal Features)

| Sequence | Purpose | Priority |
|----------|---------|----------|
| SGR 38:5 / 48:5 | 256-color mode | High |
| SGR 38:2 / 48:2 | Truecolor (24-bit) | High |
| OSC 8 | Hyperlinks | High |
| DECSET | Private mode (mouse tracking, bracketed paste) | Critical |
| ICH/DCH | Insert/delete character | Medium |
| IL/DL | Insert/delete line | Medium |
| SU/SD | Scroll up/down | Medium |
| CPR | Cursor position report | Medium |
| DECSCUSR | Cursor style (block, underline, bar) | Low |
| RIS | Reset to initial state | Low |

---

## Platform Limitations

### Current Status

| Platform | Status |
|----------|--------|
| Linux | ✅ Supported (nix crate) |
| macOS | ❌ Not implemented |
| Windows | ❌ Not implemented (ConPTY) |
| FreeBSD | ❌ Not implemented |

### Required Abstraction Layer

```rust
trait TerminalBackend {
    fn spawn(&self, config: &SpawnConfig) -> Result<Box<dyn TerminalProcess>>;
    fn resize(&self, process: &mut dyn TerminalProcess, cols: u16, rows: u16) -> Result<()>;
    fn send_signal(&self, process: &mut dyn TerminalProcess, signal: Signal) -> Result<()>;
    fn read_output(&self, process: &dyn TerminalProcess, timeout_ms: u32) -> Result<Vec<u8>>;
}
```

### Roadmap

- v0.3.0: Linux-only (current)
- v0.5.0: macOS support
- v0.7.0: Windows ConPTY support

---

## Missing Signal Handling

### Currently Supported

✅ SIGINT, SIGTERM, SIGKILL, SIGWINCH, SIGSTOP, SIGCONT

### Missing

| Signal | Purpose | Priority |
|--------|---------|----------|
| SIGHUP | Hangup (daemon testing) | Medium |
| SIGUSR1/SIGUSR2 | User-defined signals | Low |
| Signal masking | Block/unblock signals | Medium |
| Signal during I/O | Handle signals during read/write | Medium |

---

## Edge Cases Not Handled

| Edge Case | Risk | Priority |
|-----------|------|----------|
| Terminal size > 2000x2000 | Crash/overflow | High |
| Resize during output | Race condition | High |
| Malformed UTF-8 bytes | Parsing errors | Medium |
| Signal during critical section | State corruption | Medium |
| Infinite output loop | Memory exhaustion | High |
| Setuid/setgid processes | Security context | Low |
| Circular fork/exec | Infinite recursion | Low |
| Zero-width Unicode chars | Rendering issues | Medium |
| Bidirectional text (RTL) | Wrong cursor position | Medium |

---

## Test Framework Gaps

### Current Limitations

- Tests run sequentially (no parallelization)
- No test filtering by tags
- No flaky test retry mechanism
- No performance baseline tracking
- No snapshot auto-update

### Required Features

| Feature | Description | Priority |
|---------|-------------|----------|
| `--parallel` | Run tests concurrently | High |
| `--tag` | Filter tests by category | Medium |
| `--retry N` | Retry flaky tests | Medium |
| `--update-snapshots` | Auto-update golden files | Medium |
| `--benchmark` | Track performance baselines | Low |
| `--html-report` | Rich HTML test reports | Low |

---

## PTY/Terminal Model Gaps

### Current Implementation

- Basic PTY allocation
- Raw mode configuration
- Non-blocking I/O
- Terminal resize with SIGWINCH

### Missing Features

| Feature | Purpose | Priority |
|---------|---------|----------|
| Mouse protocol support | X10/UTF-8/SGR mouse reporting | Critical |
| Bracketed paste mode | Test paste behavior | High |
| Focus reporting | Focus in/out events | Medium |
| Synchronized output | OSC 4 protocol | Medium |
| True color validation | 24-bit color correctness | Medium |
| Wide character support | CJK double-width | Medium |
| Combining characters | Unicode combining marks | Medium |
| Tab expansion | Tab-to-spaces conversion | Medium |

---

## Visual/Screenshot Testing

### Current State

No visual regression testing capability.

### Required Implementation

| Feature | Description | Priority |
|---------|-------------|----------|
| Screenshot capture | Capture terminal visual state | High |
| Image diff | Compare screenshots pixel-by-pixel | High |
| Ignore regions | Exclude dynamic content (clock, cursor) | Medium |
| Threshold control | Adjust sensitivity | Medium |

---

## Implementation Roadmap

### Phase 1: Foundation (v0.3.0) - COMPLETED ✓

```
1. ✓ Fix custom invariant support (custom invariants with pattern and cursor checks)
2. ✓ Add mouse event support (mouse_click, mouse_scroll with SGR protocol)
3. ✓ Add per-step timeout override (wait_for and wait_screen now support timeout_ms)
4. ✓ Add wait_screen / assert_not_screen actions
5. ✓ Fix platform detection (Linux-only warning on non-Linux platforms)
```

### Phase 2: Completeness (v0.4.0) - COMPLETED ✓

```
1. ✓ Platform abstraction layer (src/platform/ module with TerminalBackend trait)
2. ✓ Add fuzzy/approximate pattern matching (Levenshtein distance in src/fuzzy.rs)
3. ✓ Implement screenshot comparison with diff (src/screenshot.rs)
4. ✓ Add 256-color and truecolor support (SGR 38;5/48;5 and 38;2/48;2)
5. OSC 8 hyperlink validation (deferred to v0.5.0)
```

### Phase 3: Scale (v0.5.0)

```
1. macOS PTY support
2. Parallel test execution
3. Performance benchmarking with baselines
4. Test tags and filtering
5. Flaky test detection and retry
```

### Phase 4: Polish (v0.6.0)

```
1. Windows ConPTY support
2. Rich HTML test reports
3. Advanced ANSI support (all missing sequences)
4. Test data generators
5. CI/CD pipeline templates
```

---

## Success Metrics

### v0.3.0 - COMPLETED ✓

- [x] Custom invariants work (with pattern and cursor position checks)
- [x] Mouse events supported (click, scroll with SGR 1006 protocol)
- [x] All 8 unit tests pass (100% pass rate)

### v0.4.0 - COMPLETED ✓

- [x] 21 test scenarios (more to be added)
- [x] Screenshot comparison works (take_screenshot, assert_screenshot)
- [x] 256-color and truecolor support
- [x] Platform abstraction layer
- [x] Fuzzy pattern matching
- [x] 293 passing tests

### v0.5.0

- [ ] macOS support
- [ ] Parallel test execution
- [ ] 15+ invariants

### v1.0.0

- [ ] Cross-platform (Linux, macOS, Windows)
- [ ] 30+ invariants
- [ ] Visual regression testing
- [ ] CI/CD integration templates

---

## Files Removed

This roadmap consolidates and supersedes:
- `FEATURE_ROADMAP.md` - Integrated into this document
- `TESTING_PLAN.md` - Integrated into this document
- `REALWORLD_TESTING.md` - Integrated into this document
- `docs/PHASE2_RESEARCH.md` - Integrated into this document
- `docs/PHASE2_TUI_INTEGRATION.md` - Integrated into this document
- `AGENTS.md` - Removed (AI assistant guidelines not needed in repo)
- `CONTRIBUTING.md` - Removed (overkill for early stage project)

---

**Document Version**: 2.0.0
**Last Updated**: 2026-01-10
