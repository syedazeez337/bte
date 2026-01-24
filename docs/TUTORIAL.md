# BTE Tutorial

A step-by-step guide to testing CLI/TUI applications with the Behavioral Testing Engine.

## Introduction

BTE (Behavioral Testing Engine) is a deterministic testing framework for terminal applications. It spawns real processes in a pseudo-terminal (PTY), sends input, and verifies output against expected patterns.

## Prerequisites

- Rust 1.82+ (for building from source)
- Linux or macOS (Windows support planned)

## Installation

```bash
# From source
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo install --path .

# From crates.io
cargo install bte
```

## Your First Test

### Step 1: Create a Test Scenario

Create a file called `hello-test.yaml`:

```yaml
name: "hello-world"
description: "Test that echo works correctly"
command: "echo 'Hello, World!'"

steps:
  - action: wait_for
    pattern: "Hello, World!"
    timeout_ms: 1000

invariants:
  - type: cursor_bounds

seed: 42
timeout_ms: 5000
```

### Step 2: Run the Test

```bash
bte run hello-test.yaml
```

Expected output:
```
Running scenario: hello-world
✓ Scenario passed (exit code: 0)
```

## Understanding Scenarios

A BTE scenario consists of:

1. **Metadata**: `name`, `description`, `seed`
2. **Command**: The program to test
3. **Steps**: Actions to perform
4. **Invariants**: Conditions that must always hold
5. **Timeout**: Maximum execution time

### Terminal Configuration

```yaml
terminal:
  cols: 80      # Terminal width
  rows: 24      # Terminal height
```

### Environment Variables

```yaml
env:
  TERM: xterm-256color
  MY_VAR: my_value
```

## Common Actions

### Send Keys

```yaml
- action: send_keys
  keys: "ls -la\n"
```

Special keys use `${}` syntax:
```yaml
- action: send_keys
  keys: "${Ctrl_c}"  # Send Ctrl+C
```

### Wait for Output

```yaml
- action: wait_for
  pattern: "expected text"  # Regex pattern
  timeout_ms: 5000
```

### Wait for Screen Content

```yaml
- action: wait_screen
  pattern: "text on screen"
  timeout_ms: 5000
```

### Mouse Events

```yaml
- action: mouse_click
  row: 5
  col: 10
  button: 0  # Left button
  enable_tracking: true

- action: mouse_scroll
  row: 5
  col: 10
  direction: up
  count: 3
```

### Send Signals

```yaml
- action: send_signal
  signal: SIGTERM
```

### Resize Terminal

```yaml
- action: resize
  cols: 120
  rows: 40
```

## Invariants

Invariants are conditions checked throughout test execution.

### Built-in Invariants

```yaml
invariants:
  # Cursor stays within bounds
  - type: cursor_bounds

  # Process produces output (no deadlock)
  - type: no_deadlock
    timeout_ms: 5000

  # Screen contains pattern
  - type: screen_contains
    pattern: "expected"

  # Screen does NOT contain pattern
  - type: screen_not_contains
    pattern: "error"

  # Screen stable for N ticks
  - type: screen_stable
    min_ticks: 10
```

### Custom Invariants

```yaml
invariants:
  - type: custom
    name: "prompt-visible"
    pattern: "\\$\\s*$"
    should_contain: true
    description: "Shell prompt should be visible"
```

## Visual Testing with Screenshots

### Capture a Screenshot

```yaml
- action: take_screenshot
  path: "screenshots/output.yaml"
  description: "After login"
```

### Assert Against Baseline

```yaml
- action: assert_screenshot
  path: "golden/expected.yaml"
  max_differences: 0
  compare_colors: true
  compare_text: true
```

## Example: Testing a TUI Application

```yaml
name: "fzf-test"
description: "Test fzf fuzzy finder"
command: "echo -e 'apple\nbanana\ncherry' | fzf"

terminal:
  cols: 80
  rows: 24

steps:
  - action: wait_for
    pattern: "apple"
    timeout_ms: 2000

  - action: send_keys
    keys: "ban"

  - action: wait_screen
    pattern: "banana"
    timeout_ms: 1000

  - action: send_keys
    keys: "${Enter}"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 5000

seed: 12345
timeout_ms: 30000
```

## Fuzzy Pattern Matching

For approximate matching:

```yaml
- action: wait_for_fuzzy
  pattern: "hello world"
  max_distance: 2      # Allow 2 edits
  min_similarity: 0.85 # Or 85% similarity
  timeout_ms: 5000
```

## Debugging Tips

### Verbose Output

```bash
bte run --verbose scenario.yaml
```

### Save Execution Trace

```bash
bte run --trace trace.json scenario.yaml
```

### Validate Scenario Syntax

```bash
bte validate scenario.yaml
```

## Next Steps

- See [API.md](API.md) for complete API reference
- Check [FUTURE.md](../FUTURE.md) for planned features
- Browse [scenarios/](../scenarios/) for more examples
