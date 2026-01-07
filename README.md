# Behavioral Testing Engine (BTE)

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.82+-blue?logo=rust)](https://www.rust-lang.org/)
[![CI Status](https://img.shields.io/github/actions/workflow/status/syedazeez337/bte/ci.yml?branch=main&logo=github)](https://github.com/syedazeez337/bte/actions)
[![Docs](https://img.shields.io/badge/docs-main-blue?logo=docs)](https://docs.rs/bte)
[![Build](https://img.shields.io/github/actions/workflow/status/syedazeez337/bte/ci.yml?branch=main)](https://github.com/syedazeez337/bte/actions)

**A deterministic, behavioral testing engine for CLI and TUI applications.**

Write once, test everywhere. BTE provides deterministic execution, replay capabilities, and automated invariant verification for terminal applications.

[Features](#features) • [Quick Start](#quick-start) • [Documentation](https://docs.rs/bte) • [Examples](examples/) • [Contributing](#contributing)

</div>

---

## Table of Contents

- [About](#about)
- [Features](#features)
- [Why BTE?](#why-bte)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
  - [Command Line](#command-line)
  - [YAML Scenarios](#yaml-scenarios)
  - [Programmatic API](#programmatic-api)
- [Documentation](#documentation)
  - [Actions](#actions)
  - [Invariants](#invariants)
  - [Exit Codes](#exit-codes)
- [Architecture](#architecture)
- [Security](#security)
- [Development](#development)
- [Contributing](#contributing)
- [Changelog](#changelog)
- [License](#license)

---

## About

BTE (Behavioral Testing Engine) is a framework for **deterministically testing CLI and TUI applications**. Unlike traditional testing approaches that rely on timeouts and fragile selectors, BTE:

- 🖥️ Executes real binaries inside pseudo-terminal (PTY) pairs
- 📊 Captures complete terminal state and output sequences
- 🎯 Enables deterministic replay for failure investigation
- ✅ Verifies behavioral invariants automatically
- 🔒 Includes built-in security scanning for terminal escape sequences

### Use Cases

- **TUI Framework Testing**: Validate `ratatui`, `crossterm`, `tcell` applications
- **CLI Application Testing**: Test interactive CLI tools with proper terminal emulation
- **Terminal Emulator Testing**: Verify escape sequence handling and cursor behavior
- **Regression Testing**: Capture and replay bugs deterministically
- **Property-Based Testing**: Define invariants that must always hold

---

## Features

### Core Capabilities

- **Real PTY Execution**: Native terminal execution, not simulation or mocking
- **Deterministic Execution**: Seeded RNG, monotonic clock, explicit scheduling boundaries
- **Full ANSI Support**: Complete escape sequence parsing (CSI, OSC, ESC, UTF-8)
- **Screen Modeling**: 2D grid with attributes, scrollback buffer, cursor tracking
- **State Hashing**: FNV-1a hashing for change detection

### Testing Framework

- **Scenario Definition**: Declarative YAML format for test interactions
- **Built-in Invariants**:
  - `cursor_bounds` - Verify cursor stays within screen bounds
  - `no_deadlock` - Detect application hangs with configurable timeouts
  - `screen_contains/not_contains` - Content assertions
  - `signal_handled` - Validate proper signal handling
  - `screen_changed/stability` - Detect flickering or stuck states
  - `viewport_valid` - Ensure cursor and scroll positions are valid
  - `response_time` - Verify applications respond within expected ticks
  - `max_latency` - Ensure latency never exceeds thresholds
- **Trace & Replay**: Structured JSON traces for complete failure reproduction
- **Signal Injection**: SIGINT, SIGTERM, SIGKILL, SIGWINCH support

### CLI Commands

| Command | Description |
|---------|-------------|
| `bte run <scenario>` | Execute scenarios and generate traces |
| `bte replay <trace>` | Replay traces for debugging |
| `bte validate <file>` | Validate scenario/trace files |

---

## Why BTE?

| Approach | Determinism | Real Terminal | Invariants | Replay | Security |
|----------|-------------|---------------|------------|--------|----------|
| **BTE** | ✅ Seeded RNG | ✅ PTY | ✅ Built-in | ✅ Full | ✅ Built-in |
| Selenium/Playwright | ❌ Wall-clock | ❌ Browser | ❌ Limited | ❌ Partial | ❌ Manual |
| goexpect/pexpect | ⚠️ Limited | ✅ PTY | ❌ Manual | ❌ Manual | ❌ Manual |
| Unit tests | ❌ Variable | ❌ Mocked | ❌ Manual | ❌ Manual | ❌ Manual |

---

## Quick Start

### Prerequisites

- Rust 1.82 or later (see [rustup.rs](https://rustup.rs/) for installation)
- A Unix-like operating system (Linux, macOS)

```bash
# Verify Rust version
rustc --version  # Must be 1.82+
```

### Installation

#### From Source

```bash
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo build --release
```

The binary will be at `target/release/bte`. Add it to your PATH:

```bash
export PATH="$PATH:$(pwd)/target/release"
bte --help
```

#### From GitHub Releases

Download the latest binary from the [Releases page](https://github.com/syedazeez337/bte/releases):

```bash
# Linux x86_64
curl -L https://github.com/syedazeez337/bte/releases/latest/download/bte-x86_64-unknown-linux-gnu.tar.gz | tar xz
./bte --help
```

### Your First Test

Create a scenario file:

```yaml
# examples/hello.yaml
name: "hello-world test"
description: "Simple test that runs echo hello world"
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
timeout_ms: 10000
```

Run it:

```bash
bte run examples/hello.yaml
```

Expected output:

```
=== Run Result ===
Exit code: 0
Steps executed: 2
Ticks: 0
Status: SUCCESS (exit=0, ticks=0)
```

---

## Usage

### Command Line

```bash
# Run a scenario
bte run scenarios/my-test.yaml

# Run with verbose output
bte -v run scenarios/my-test.yaml

# Replay a trace for debugging
bte replay traces/trace-123.json

# Validate a scenario file
bte validate scenarios/my-test.yaml
```

### YAML Scenarios

Scenarios define test interactions in a declarative YAML format:

```yaml
name: Interactive Editor Test
description: Test a terminal text editor
command: "vim"

steps:
  # Wait for prompt
  - action: wait_for
    pattern: "vim"
    timeout_ms: 5000

  # Enter insert mode
  - action: send_keys
    keys: ["i", "Hello from BTE!", "Escape"]

  # Save and exit
  - action: send_keys
    keys: [":", "wq", "Enter"]

  # Verify output
  - action: assert_screen
    pattern: "Hello from BTE!"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ticks: 500

seed: 12345
timeout_ms: 30000
```

#### Available Actions

| Action | Description | Parameters |
|--------|-------------|------------|
| `wait_for` | Wait for pattern in output | `pattern`, `timeout_ms` |
| `send_keys` | Send keystrokes | `keys` (array or string) |
| `resize` | Resize terminal | `cols`, `rows` |
| `send_signal` | Send POSIX signal | `signal` (SIGINT, SIGTERM, etc.) |
| `assert_screen` | Assert screen content | `pattern`, `anywhere` |
| `assert_cursor` | Assert cursor position | `row`, `col` |
| `wait_ticks` | Wait for N ticks | `ticks` |

#### Key Names

Special keys are supported:
- **Navigation**: `Enter`, `Tab`, `Backspace`, `Escape`
- **Arrows**: `Up`, `Down`, `Left`, `Right`
- **Modifiers**: `Ctrl_c`, `Alt_x`, `Shift_a`
- **Function**: `F1` through `F12`
- **Custom**: Any string for direct input

### Programmatic API

```rust
use bte::{runner, scenario};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load scenario from YAML
    let yaml = std::fs::read_to_string("test.yaml")?;
    let scenario = scenario::Scenario::from_yaml(&yaml)?;

    // Configure deterministic execution
    let config = runner::RunnerConfig {
        seed: Some(42),
        trace_path: Some("trace.json".into()),
        ..Default::default()
    };

    // Execute with deterministic timing
    let result = runner::run_scenario(&scenario, &config)?;

    if result.success {
        println!("Test passed!");
    } else {
        eprintln!("Test failed with exit code: {}", result.exit_code);
        for violation in &result.trace.invariant_results {
            if violation.violation() {
                eprintln!("  - {}: {}", violation.name, violation.message);
            }
        }
    }

    Ok(())
}
```

Add to your `Cargo.toml`:

```toml
[dependencies]
bte = { git = "https://github.com/syedazeez337/bte" }
```

---

## Documentation

### Invariants

Invariants are properties that must always hold during execution:

```yaml
invariants:
  # Cursor must stay within screen bounds
  - type: cursor_bounds

  # No deadlock within 100 ticks
  - type: no_deadlock
    timeout_ticks: 100

  # Screen must contain expected text
  - type: screen_contains
    pattern: "Expected output"

  # No privilege escalation attempts
  - type: no_privilege_escalation
```

### Exit Codes

| Code | Meaning | Description |
|------|---------|-------------|
| `0` | Success | All steps completed, invariants passed |
| `-1` | Signaled | Process terminated by signal |
| `-2` | Violation | Invariant check failed |
| `-3` | Timeout | Step timed out |
| `-4` | Error | Other error occurred |
| `-5` | Divergence | Replay diverged from trace |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        BTE Core                              │
├─────────────────────────────────────────────────────────────┤
│  Determinism Layer                                          │
│  ├── Monotonic Clock (no wall-clock dependencies)          │
│  ├── Seeded RNG (xorshift64)                               │
│  └── Scheduler (explicit execution boundaries)             │
├─────────────────────────────────────────────────────────────┤
│  PTY Layer                                                  │
│  ├── PTY Allocation (nix::pty)                             │
│  ├── Process Spawn (fork+exec)                             │
│  ├── Signal Handling (SIGINT/TERM/KILL/WINCH)              │
│  └── Non-blocking IO (epoll/kqueue)                        │
├─────────────────────────────────────────────────────────────┤
│  Terminal Model                                             │
│  ├── ANSI Parser (CSI, OSC, ESC, UTF-8)                    │
│  ├── Screen Grid (2D cells with attributes)                │
│  ├── Scrollback Buffer                                     │
│  └── State Hashing (FNV-1a)                                │
├─────────────────────────────────────────────────────────────┤
│  Testing Framework                                          │
│  ├── Scenario Executor                                     │
│  ├── Invariant Engine                                      │
│  ├── Trace Recorder (v2 format with checkpoints)           │
│  └── Replay Engine (deterministic reproduction)            │
├─────────────────────────────────────────────────────────────┤
│  Security Layer                                             │
│  ├── Escape Sequence Filter (OSC, DCS, ANSI)               │
│  ├── Command Injection Detection                           │
│  ├── Privilege Escalation Checks                           │
│  └── Bounds Verification                                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Security

BTE includes built-in security features to safely test untrusted applications:

### Security Invariants

- **EscapeSequenceFilter**: Detects dangerous terminal escape sequences (OSC 0, OSC 52, etc.)
- **NoCommandInjection**: Blocks shell metacharacters (`; | & $ ( ) { } < >`)
- **NoPrivilegeEscalation**: Monitors for privilege escalation patterns
- **BoundsCheckInvariant**: Validates cursor stays within screen bounds

### Safe Regex

Built-in ReDoS protection prevents catastrophic backtracking:

```rust
use bte::safe_regex::SafeRegex;

// Creates regex with size limits to prevent ReDoS
let regex = SafeRegex::with_default_limits(pattern)?;
let result = regex.is_match(input);
```

---

## Development

### Getting Started

```bash
# Clone the repository
git clone https://github.com/syedazeez337/bte.git
cd bte

# Build in development mode
cargo build

# Run tests
cargo test

# Run with specific test filter
cargo test invariant

# Code quality checks
cargo fmt      # Format code
cargo clippy   # Lint
cargo check    # Type check
```

### Project Structure

```
bte/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library root
│   ├── runner.rs         # Scenario execution engine
│   ├── scenario.rs       # Scenario parsing/validation
│   ├── invariants.rs     # Built-in invariants
│   ├── security.rs       # Security scanning invariants
│   ├── safe_regex.rs     # ReDoS-protected regex
│   ├── process.rs        # PTY process management
│   ├── screen.rs         # Terminal screen model
│   ├── ansi.rs           # ANSI escape sequence parser
│   ├── vtparse.rs        # VT parsing state machine
│   └── ...
├── examples/             # Example scenarios
├── tests/                # Integration tests
├── CHANGELOG.md
└── Cargo.toml
```

### Testing Philosophy

BTE follows deterministic testing principles:

1. **Seed-based reproducibility**: Every run can be reproduced with the same seed
2. **State inspection**: Full terminal state capture at each step
3. **Invariant verification**: Properties that must always hold
4. **Checkpoint-based replay**: Debug failures by replaying specific checkpoints

---

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

### How to Contribute

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/amazing-feature`
3. **Commit** your changes: `git commit -m 'Add amazing feature'`
4. **Push** to your branch: `git push origin feature/amazing-feature`
5. **Open** a Pull Request

### Requirements

- All tests must pass: `cargo test`
- Code must be formatted: `cargo fmt`
- No clippy warnings: `cargo clippy`

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for detailed release history.

### Latest Changes (v0.2.0)

- Built-in invariants for response time, latency, and process termination
- Security scanning for escape sequences and command injection
- ReDoS protection with safe regex
- Deterministic timing with tick-based scheduling
- Enhanced traces with checkpoint support

---

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

---

## Acknowledgments

Built with ❤️ using these excellent projects:

- [nix](https://docs.rs/nix/) - POSIX bindings for PTY and signals
- [serde](https://serde.rs/) - Serialization framework
- [clap](https://docs.rs/clap/) - Command-line argument parsing
- [regex](https://docs.rs/regex/) - Regular expression library

---

<div align="center">

**Built with ❤️ for deterministic terminal testing**

[GitHub](https://github.com/syedazeez337/bte) • [Docs](https://docs.rs/bte)

</div>
