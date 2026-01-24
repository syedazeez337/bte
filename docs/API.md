# BTE API Reference

Complete API documentation for the Behavioral Testing Engine.

## Scenario Format

### Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Unique scenario identifier |
| `description` | string | No | Human-readable description |
| `command` | string/object | Yes | Command to execute |
| `terminal` | object | No | Terminal configuration |
| `env` | object | No | Environment variables |
| `steps` | array | Yes | Test steps to execute |
| `invariants` | array | No | Invariants to check |
| `seed` | number | No | RNG seed for determinism |
| `timeout_ms` | number | No | Global timeout (default: 30000) |
| `tags` | array | No | Tags for filtering |

### Command Variants

Simple command:
```yaml
command: "echo hello"
```

Command with arguments:
```yaml
command:
  program: "python3"
  args: ["script.py", "--verbose"]
```

Shell command:
```yaml
command:
  shell: "for i in 1 2 3; do echo $i; done"
```

### Terminal Configuration

```yaml
terminal:
  cols: 80    # Width in columns (default: 80)
  rows: 24    # Height in rows (default: 24)
```

## Actions

### send_keys

Send keystrokes to the terminal.

```yaml
- action: send_keys
  keys: "text to send"
```

**Special Keys:**
- `${Enter}`, `${Tab}`, `${Escape}`, `${Backspace}`
- `${Up}`, `${Down}`, `${Left}`, `${Right}`
- `${Home}`, `${End}`, `${PageUp}`, `${PageDown}`
- `${Insert}`, `${Delete}`
- `${F1}` through `${F12}`
- `${Ctrl_a}` through `${Ctrl_z}`
- `${Alt_a}` through `${Alt_z}`

### wait_for

Wait for regex pattern in output stream.

```yaml
- action: wait_for
  pattern: "regex pattern"
  timeout_ms: 5000  # Optional, default from scenario
```

### wait_for_fuzzy

Wait for approximate pattern match.

```yaml
- action: wait_for_fuzzy
  pattern: "expected text"
  max_distance: 2       # Max edit distance
  min_similarity: 0.85  # Minimum similarity (0.0-1.0)
  timeout_ms: 5000
```

### wait_screen

Wait for pattern in current screen content.

```yaml
- action: wait_screen
  pattern: "screen text"
  timeout_ms: 5000
```

### wait_ticks

Wait for N scheduling ticks.

```yaml
- action: wait_ticks
  ticks: 10
```

### send_signal

Send POSIX signal to process.

```yaml
- action: send_signal
  signal: SIGTERM  # SIGINT, SIGTERM, SIGKILL, SIGSTOP, SIGCONT, SIGHUP
```

### resize

Resize terminal dimensions.

```yaml
- action: resize
  cols: 120
  rows: 40
```

### mouse_click

Send mouse click event (SGR 1006 protocol).

```yaml
- action: mouse_click
  row: 5
  col: 10
  button: 0           # 0=left, 1=middle, 2=right
  enable_tracking: true
```

### mouse_scroll

Send mouse scroll event.

```yaml
- action: mouse_scroll
  row: 5
  col: 10
  direction: up  # up or down
  count: 3
  enable_tracking: true
```

### assert_screen

Assert screen contains pattern (fails immediately if not).

```yaml
- action: assert_screen
  pattern: "expected text"
```

### assert_not_screen

Assert screen does NOT contain pattern.

```yaml
- action: assert_not_screen
  pattern: "error"
```

### assert_cursor

Assert cursor position.

```yaml
- action: assert_cursor
  row: 0
  col: 0
```

### snapshot

Capture named screen state.

```yaml
- action: snapshot
  name: "after-login"
```

### take_screenshot

Save screen to file.

```yaml
- action: take_screenshot
  path: "screenshots/output.yaml"
  description: "After login screen"
```

### assert_screenshot

Compare screen against baseline file.

```yaml
- action: assert_screenshot
  path: "golden/expected.yaml"
  max_differences: 0
  compare_colors: true
  compare_text: true
  ignore_regions:
    - row: 0
      col: 0
      width: 10
      height: 1
```

### check_invariant

Manually trigger invariant check.

```yaml
- action: check_invariant
  name: "cursor_bounds"
```

## Invariants

### cursor_bounds

Cursor stays within terminal bounds.

```yaml
- type: cursor_bounds
```

### no_deadlock

Process produces output within timeout.

```yaml
- type: no_deadlock
  timeout_ms: 5000
```

### screen_contains

Screen contains pattern at all times.

```yaml
- type: screen_contains
  pattern: "regex"
```

### screen_not_contains

Screen never contains pattern.

```yaml
- type: screen_not_contains
  pattern: "error"
```

### screen_stable

Screen unchanged for N ticks.

```yaml
- type: screen_stable
  min_ticks: 10
```

### viewport_valid

Viewport dimensions valid.

```yaml
- type: viewport_valid
```

### response_time

Response within tick limit.

```yaml
- type: response_time
  max_ticks: 100
```

### max_latency

Maximum screen update latency.

```yaml
- type: max_latency
  max_ticks: 50
```

### signal_handled

Process handles specified signal.

```yaml
- type: signal_handled
  signal: SIGTERM
```

### no_output_after_exit

No output after process exits.

```yaml
- type: no_output_after_exit
```

### process_terminated_cleanly

Process exits with code 0 or allowed signals.

```yaml
- type: process_terminated_cleanly
  allowed_signals: ["SIGTERM", "SIGINT"]
```

### custom

Custom invariant with pattern and cursor checks.

```yaml
- type: custom
  name: "prompt-visible"
  pattern: "\\$\\s*$"
  should_contain: true
  expected_row: null  # Optional cursor row check
  expected_col: null  # Optional cursor col check
  description: "Shell prompt should always be visible"
```

## CLI Reference

### Run Command

```bash
bte run [OPTIONS] <SCENARIO>

OPTIONS:
  -v, --verbose           Enable verbose output
  -t, --trace <PATH>      Save execution trace to file
  -s, --seed <SEED>       Override scenario seed
  --update-snapshots      Update golden snapshot files
```

### Validate Command

```bash
bte validate <SCENARIO>
```

### List Command

```bash
bte list <DIRECTORY>
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| -1 | Process terminated by signal |
| -2 | Invariant violation |
| -3 | Timeout |

## Rust Library API

```rust
use bte::{runner, scenario};

// Load scenario from YAML
let yaml = std::fs::read_to_string("test.yaml")?;
let scenario = scenario::Scenario::from_yaml(&yaml)?;

// Configure runner
let config = runner::RunnerConfig {
    seed: Some(42),
    trace_path: Some("trace.json".into()),
    verbose: false,
    max_ticks: 10000,
    tick_delay_ms: 0,
};

// Run scenario
let result = runner::run_scenario(&scenario, &config);

// Check result
if result.success {
    println!("Test passed!");
} else {
    println!("Test failed: exit_code={}", result.exit_code);
}

// Access trace
let trace = result.trace;
println!("Steps executed: {}", trace.steps.len());
```

## Color Support

BTE supports:
- Standard 8/16 ANSI colors
- 256-color mode (SGR 38;5;N, 48;5;N)
- 24-bit truecolor (SGR 38;2;R;G;B, 48;2;R;G;B)

## Platform Support

| Platform | Status |
|----------|--------|
| Linux | Full support |
| macOS | Experimental |
| Windows | Planned (ConPTY) |
