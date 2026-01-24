# Behavioral Testing Engine (BTE)

<p align="center">

[![Crates.io](https://img.shields.io/crates/v/bte.svg)](https://crates.io/crates/bte)
[![CI](https://github.com/syedazeez337/bte/workflows/CI/badge.svg)](https://github.com/syedazeez337/bte/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Deterministic behavioral testing for terminal applications.**

</p>

BTE executes real terminal applications in a pseudo-terminal (PTY), captures all output, and verifies behavioral invariants automatically. Think of it as [Playwright](https://playwright.dev/) for CLI/TUI applications.

## Scope

BTE is **language-agnostic** and can test **any** terminal/CLI application, regardless of the language it's written in. Since BTE spawns real processes in a PTY and communicates via stdin/stdout/stderr, it can test applications written in:

- **Rust**, Python, Go, Node.js, C/C++, Bash, and any language that runs in a terminal
- Shell scripts, TUI frameworks (crossterm, ratatui, tview, blessed, etc.)
- Any command-line tool with textual input/output

Example test scenarios can target:
```yaml
# Test a Python TUI app
command: "python3 myapp.py"

# Test a Node.js CLI tool
command: "node mycli.js"

# Test a Rust binary
command: "./my-rust-app"

# Test a shell script
command: "bash script.sh"
```

## About

BTE provides a declarative, YAML-based approach to testing terminal applications:

- **Real PTY execution** - Tests run against actual terminal processes
- **Deterministic replay** - Seed-based execution for reproducible results
- **Behavioral invariants** - Automatic verification of cursor bounds, deadlocks, screen content
- **Signal injection** - Test SIGINT, SIGTERM, SIGKILL, and other signals
- **Mouse support** - Click and scroll events via SGR 1006 protocol

## Installation

### From Source

```bash
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo install --path .
```

### From Crates.io

```bash
cargo install bte
```

### Pre-built Binaries

Download pre-built binaries from the [Releases](https://github.com/syedazeez337/bte/releases) page.

## Quick Start

Create a test scenario file:

```yaml
name: "example-test"
command: "sh"

steps:
  - action: send_keys
    keys: "echo 'Hello, World!'\n"
  - action: wait_for
    pattern: "Hello, World!"
    timeout_ms: 5000

invariants:
  - type: cursor_bounds
  - type: no_deadlock

seed: 42
timeout_ms: 30000
```

Run the test:

```bash
bte run example.yaml
```

## Features

### Actions

| Action | Description |
|--------|-------------|
| `send_keys` | Send keystrokes to the terminal |
| `wait_for` | Wait for a regex pattern in output stream |
| `wait_screen` | Wait for a pattern in screen content |
| `wait_ticks` | Wait for N scheduling ticks |
| `resize` | Change terminal dimensions |
| `send_signal` | Send POSIX signals |
| `mouse_click` | Mouse click at position (SGR protocol) |
| `mouse_scroll` | Mouse scroll at position |
| `assert_screen` | Assert screen contains pattern |
| `assert_not_screen` | Assert screen does not contain pattern |
| `assert_cursor` | Assert cursor position |
| `snapshot` | Capture screen state |
| `check_invariant` | Manually trigger invariant check |

### Invariants

| Invariant | Description |
|-----------|-------------|
| `cursor_bounds` | Cursor stays within screen bounds |
| `no_deadlock` | Process produces output within timeout |
| `screen_contains` | Screen contains expected pattern |
| `screen_not_contains` | Screen does not contain pattern |
| `screen_stable` | Screen remains stable for N ticks |
| `viewport_valid` | Viewport dimensions are valid |
| `response_time` | Response within time limit |
| `max_latency` | Maximum screen redraw latency |
| `custom` | Custom invariant with pattern/cursor checks |

### Special Keys

```
Enter, Escape, Tab, Backspace
Up, Down, Left, Right
Home, End, PageUp, PageDown
Insert, Delete
F1-F12
Ctrl_a through Ctrl_z
Alt_<key>
```

## Examples

### Testing a TUI Application

```yaml
name: "fzf-search-test"
command: "fzf"

steps:
  - action: send_keys
    keys: "test query"
  - action: wait_for
    pattern: "test query"
  - action: send_keys
    keys: "${Enter}"
  - action: wait_ticks
    ticks: 5

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 10000

seed: 12345
```

### Signal Handling Test

```yaml
name: "signal-test"
command: "sh"

steps:
  - action: send_keys
    keys: "sleep 30 &\n"
  - action: wait_ticks
    ticks: 5
  - action: send_signal
    signal: SIGTERM

invariants:
  - type: signal_handled
    signal: SIGTERM
  - type: cursor_bounds

timeout_ms: 30000
```

### Mouse Interaction

```yaml
name: "mouse-test"
command: "sh"

steps:
  - action: mouse_click
    row: 0
    col: 0
    button: 0
    enable_tracking: true
  - action: mouse_scroll
    row: 0
    col: 0
    direction: up
    count: 1

invariants:
  - type: cursor_bounds
```

## Programmatic Usage

Add BTE as a dependency:

```toml
[dependencies]
bte = "0.2"
```

Use the library API:

```rust
use bte::{runner, scenario};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = std::fs::read_to_string("test.yaml")?;
    let scenario = scenario::Scenario::from_yaml(&yaml)?;

    let config = runner::RunnerConfig {
        seed: Some(42),
        trace_path: Some("trace.json".into()),
        ..Default::default()
    };

    let result = runner::run_scenario(&scenario, &config)?;

    if result.success {
        println!("Test passed!");
    } else {
        eprintln!("Test failed: exit_code={}", result.exit_code);
    }

    Ok(())
}
```

## Exit Codes

| Code | Description |
|------|-------------|
| `0` | Success - all steps completed, invariants passed |
| `-1` | Process terminated by signal |
| `-2` | Invariant violation |
| `-3` | Timeout |
| `1` | Other error |

## Documentation

- [User Guide](https://github.com/syedazeez337/bte/wiki)
- [API Documentation](https://docs.rs/bte)
- [Examples](scenarios/)
- [Future Roadmap](FUTURE.md)

## Project Structure

```
bte/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── runner.rs         # Scenario execution engine
│   ├── scenario.rs       # YAML parsing and types
│   ├── invariants.rs     # Invariant framework and checks
│   ├── process.rs        # PTY process management
│   ├── screen.rs         # Terminal screen model
│   ├── ansi.rs           # ANSI escape sequence handling
│   ├── vtparse.rs        # VT sequence parser
│   ├── keys.rs           # Key injection
│   ├── io_loop.rs        # I/O event loop
│   ├── timing.rs         # Timing controller
│   ├── determinism.rs    # Deterministic scheduling
│   ├── replay.rs         # Trace replay engine
│   ├── trace.rs          # Trace recording
│   └── termination.rs    # Termination detection
├── scenarios/            # Example test scenarios
├── tests/                # Integration tests
└── Cargo.toml
```

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench

# Format code
cargo fmt

# Lint
cargo clippy
```

## Tested Applications

BTE has been validated with:

- **Shells**: bash, sh
- **TUI Applications**: fzf, gitui, bottom
- **CLI Tools**: Any command-line application

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a list of changes.

## Roadmap

See [FUTURE.md](FUTURE.md) for the feature roadmap and implementation plan.

## License

MIT License. See [LICENSE](LICENSE) for details.
