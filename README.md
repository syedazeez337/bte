# Behavioral Testing Engine for CLI/TUI Applications

<div align="center">

![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
![Tests](https://img.shields.io/badge/tests-108-green.svg)

A deterministic behavioral testing engine for terminal applications with PTY control, replay capabilities, and invariant verification.

[Features](#features) • [Quick Start](#quick-start) • [Documentation](#documentation) • [Contributing](#contributing)

</div>

## Overview

BTE (Behavioral Testing Engine) provides a framework for deterministically testing CLI and TUI applications. It:

- Executes real binaries inside PTY (pseudo-terminal) pairs
- Captures all terminal output and state changes
- Supports deterministic replay for debugging
- Verifies behavioral invariants automatically

## Features

### Core Capabilities
- **PTY Execution**: Real terminal execution, not simulation
- **Deterministic Execution**: Seeded RNG, monotonic clock, explicit scheduling
- **ANSI Parsing**: Full escape sequence support including UTF-8
- **Screen Modeling**: 2D grid with attributes, scrollback, cursor tracking

### Testing Framework
- **Scenario Definition**: YAML/JSON declarative interaction format
- **Invariant Verification**: Cursor bounds, deadlock detection, signal handling
- **Trace & Replay**: Structured JSON traces for failure reproduction
- **Signal Injection**: SIGINT, SIGTERM, SIGKILL, SIGWINCH support

### CLI Interface
- `bte run` - Execute scenarios and generate traces
- `bte replay` - Replay traces for debugging
- `bte validate` - Validate scenario files
- `bte info` - Inspect trace files

## Quick Start

### Installation

```bash
cargo install bte
```

Or build from source:

```bash
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo build --release
```

### Basic Usage

Create a scenario file (`example.yaml`):

```yaml
name: "hello-world test"
description: "Test that echo produces expected output"
command: "echo 'Hello, World!'"

steps:
  - action: wait_for
    pattern: "Hello, World"
  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 5000

seed: 42
```

Run the scenario:

```bash
bte run example.yaml
```

## Documentation

### Scenario Format

Scenarios are YAML files defining test interactions:

```yaml
name: Test name
description: What this tests
command: Command to run

steps:
  - action: wait_for       # Wait for pattern in output
    pattern: "prompt>"
    timeout_ms: 5000
      
  - action: send_keys      # Send keystrokes
    keys: "command\n"
      
  - action: resize         # Resize terminal
    cols: 120
    rows: 40
      
  - action: send_signal    # Send signal
    signal: SIGINT
        
  - action: assert_screen  # Assert screen content
    pattern: "expected text"
    anywhere: true
        
  - action: assert_cursor  # Assert cursor position
    row: 5
    col: 10

invariants:
  - type: cursor_bounds
  - type: no_deadlock
  - type: screen_contains
    pattern: "expected"

seed: 42
timeout_ms: 30000
```

### Key Injection

Send special keys:

```yaml
steps:
  - action: send_keys
    keys:
      - Enter
      - Tab
      - Backspace
      - Escape
      - Up
      - Down
      - Left
      - Right
      - Ctrl_c
      - Alt_x
      - F1
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| -1 | Process signaled |
| -2 | Invariant violation |
| -3 | Timeout |
| -4 | Error |
| -5 | Replay divergence |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   BTE Core                          │
├─────────────────────────────────────────────────────┤
│  Determinism Layer                                  │
│  ├── Monotonic Clock (no wall-clock)               │
│  ├── Seeded RNG (xorshift64)                       │
│  └── Scheduler (explicit boundaries)               │
├─────────────────────────────────────────────────────┤
│  PTY Layer                                          │
│  ├── PTY Allocation (nix::pty)                     │
│  ├── Process Spawn (fork+exec)                     │
│  ├── Signal Handling (SIGINT/TERM/KILL/WINCH)      │
│  └── Non-blocking IO (epoll/poll)                  │
├─────────────────────────────────────────────────────┤
│  Terminal Model                                     │
│  ├── ANSI Parser (CSI, OSC, ESC sequences)         │
│  ├── Screen Grid (2D cells with attributes)        │
│  ├── Scrollback Buffer                             │
│  └── State Hashing (FNV-1a)                        │
├─────────────────────────────────────────────────────┤
│  Testing Framework                                  │
│  ├── Scenario Executor                             │
│  ├── Invariant Engine (cursor, deadlock, etc.)     │
│  ├── Trace Recorder (JSON format)                  │
│  └── Replay Engine (deterministic reproduction)    │
└─────────────────────────────────────────────────────┘
```

## API Usage

```rust
use bte::{runner, scenario, invariants};

// Load scenario
let scenario = scenario::Scenario::from_yaml(yaml)?;

// Run with deterministic seed
let config = runner::RunnerConfig {
    seed: Some(42),
    trace_path: Some("trace.json"),
    ..Default::default()
};

let result = runner::run_scenario(&scenario, &config);

// Access trace for debugging
println!("Exit code: {}", result.exit_code);
println!("Steps: {}", result.trace.steps.len());

// Check invariant violations
for violation in result.trace.invariant_results.iter().filter(|r| r.violation()) {
    eprintln!("Invariant violated: {}", violation.name);
}
```

## Development

### Running Tests

```bash
cargo test
cargo test --release  # Performance tests
```

### Building

```bash
cargo build              # Debug build
cargo build --release    # Optimized build
cargo build --all-features
```

### Code Quality

```bash
cargo fmt               # Format code
cargo clippy            # Lint
cargo check             # Type check
cargo bench             # Run benchmarks
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [nix](https://docs.rs/nix/) for PTY and signal support
- [serde](https://serde.rs/) for serialization
- [clap](https://docs.rs/clap/) for CLI
- [chrono](https://docs.rs/chrono/) for timestamps

---

<div align="center">
Built with ❤️ for deterministic terminal testing
</div>
