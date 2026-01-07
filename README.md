# Behavioral Testing Engine (BTE)

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![CI Status](https://img.shields.io/github/actions/workflow/status/syedazeez337/bte/ci.yml?branch=main&logo=github)](https://github.com/syedazeez337/bte/actions)
[![Docs](https://img.shields.io/docsrs/bte?logo=docs.rs)](https://docs.rs/bte)

**A deterministic, behavioral testing engine for CLI and TUI applications.**

Write once, test everywhere. BTE provides deterministic execution, replay capabilities, and automated invariant verification for terminal applications.

[Features](#features) • [Quick Start](#quick-start) • [Documentation](https://docs.rs/bte) • [Examples](examples/) • [Contributing](#contributing)

</div>

---

## About

BTE (Behavioral Testing Engine) is a framework for **deterministically testing CLI and TUI applications**. Unlike traditional testing approaches that rely on timeouts and fragile selectors, BTE:

- 🖥️ Executes real binaries inside pseudo-terminal (PTY) pairs
- 📊 Captures complete terminal state and output sequences
- 🎯 Enables deterministic replay for failure investigation
- ✅ Verifies behavioral invariants automatically
- 🔒 Includes built-in security scanning for terminal escape sequences

## Why BTE?

| Approach | Determinism | Real Terminal | Invariants | Replay | Security |
|----------|-------------|---------------|------------|--------|----------|
| **BTE** | ✅ Seeded RNG | ✅ PTY | ✅ 11 built-in | ✅ Full | ✅ Built-in |
| Selenium/Playwright | ❌ Wall-clock | ❌ Browser | ❌ Limited | ❌ Partial | ❌ Manual |
| goexpect/pexpect | ⚠️ Limited | ✅ PTY | ❌ Manual | ❌ Manual | ❌ Manual |
| Unit tests | ❌ Variable | ❌ Mocked | ❌ Manual | ❌ Manual | ❌ Manual |

## Use Cases

- **TUI Framework Testing**: Validate `ratatui`, `crossterm`, `tcell` applications
- **CLI Application Testing**: Test interactive CLI tools with proper terminal emulation
- **Terminal Emulator Testing**: Verify escape sequence handling and cursor behavior
- **Regression Testing**: Capture and replay bugs deterministically
- **Property-Based Testing**: Define invariants that must always hold

## Features

### Core Capabilities

- **Real PTY Execution**: Native terminal execution, not simulation or mocking
- **Deterministic Execution**: Seeded RNG, monotonic clock, explicit scheduling boundaries
- **Full ANSI Support**: Complete escape sequence parsing (CSI, OSC, ESC, UTF-8)
- **Screen Modeling**: 2D grid with attributes, scrollback buffer, cursor tracking
- **State Hashing**: FNV-1a hashing for change detection

### Testing Framework

- **Scenario Definition**: Declarative YAML/JSON format for test interactions
- **11 Built-in Invariants**:
  - `cursor_bounds` - Verify cursor stays within screen bounds
  - `no_deadlock` - Detect application hangs with configurable timeouts
  - `signal_handled` - Validate proper signal handling
  - `screen_contains/not_contains` - Content assertions
  - `screen_changed/stability` - Detect flickering or stuck states
  - `viewport_valid` - Ensure cursor and scroll positions are valid
  - `response_time` - Verify applications respond within expected ticks
  - `max_latency` - Ensure latency never exceeds thresholds
  - `process_terminated_cleanly` - Validate clean exit with allowed signals
  - `no_output_after_exit` - Prevent unexpected output post-termination
- **Trace & Replay**: Structured JSON traces for complete failure reproduction
- **Signal Injection**: SIGINT, SIGTERM, SIGKILL, SIGWINCH support

### CLI Interface

| Command | Description |
|---------|-------------|
| `bte run <scenario>` | Execute scenarios and generate traces |
| `bte replay <trace>` | Replay traces for debugging |
| `bte validate <file>` | Validate scenario/trace files |
| `bte info <trace>` | Inspect trace files |

---

## Quick Start

### Installation

**Build from source:**

```bash
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo build --release
```

The binary will be at `target/release/bte`.

**With Docker:**

```bash
docker build -t bte .
docker run --rm bte --help
```

**From source:**
```bash
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo build --release
```

**With Docker:**
```bash
docker run --rm ghcr.io/syedazeez337/bte:latest --help
```

### Basic Example

Create a scenario file (`examples/hello.yaml`):

```yaml
name: "Hello World Test"
description: "Test that echo produces expected output"
command: "bash"

steps:
  - action: wait_for
    pattern: "\\$"
    timeout_ms: 2000
  
  - action: send_keys
    keys: ["echo 'Hello, BTE!'", "Enter"]
  
  - action: wait_for
    pattern: "Hello, BTE!"
    timeout_ms: 2000

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ticks: 100
  - type: screen_contains
    pattern: "Hello, BTE!"

seed: 42
timeout_ms: 10000
```

Run the scenario:

```bash
bte run examples/hello.yaml
```

### Programmatic Usage

```rust
use bte::{runner, scenario, invariants};

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

    match result.exit_code {
        0 => println!("✅ Test passed"),
        -2 => {
            eprintln!("❌ Invariant violations:");
            for violation in result.trace.invariant_results.iter().filter(|r| r.violation()) {
                eprintln!("  - {}", violation.name);
            }
        }
        code => eprintln!("❌ Test failed with code: {}", code),
    }

    Ok(())
}
```

---

## Documentation

### Scenario Format

Scenarios are YAML files defining test interactions:

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
  - type: process_terminated_cleanly
    allowed_signals: [15]

seed: 12345
timeout_ms: 30000
```

### Available Actions

| Action | Description | Parameters |
|--------|-------------|------------|
| `wait_for` | Wait for pattern in output | `pattern`, `timeout_ms` |
| `send_keys` | Send keystrokes | `keys` (array of key names) |
| `resize` | Resize terminal | `cols`, `rows` |
| `send_signal` | Send POSIX signal | `signal` (SIGINT, SIGTERM, etc.) |
| `assert_screen` | Assert screen content | `pattern`, `anywhere` |
| `assert_cursor` | Assert cursor position | `row`, `col` |
| `checkpoint` | Create replay checkpoint | (none) |

### Key Names

Special keys are supported:
- Navigation: `Enter`, `Tab`, `Backspace`, `Escape`
- Arrows: `Up`, `Down`, `Left`, `Right`
- Modifiers: `Ctrl_c`, `Alt_x`, `Shift_a`
- Function: `F1` through `F12`
- Custom: Any string for direct input

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
│  ├── Invariant Engine (11 invariants)                      │
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

# Build
cargo build

# Run tests
cargo test

# Run with specific test filter
cargo test invariant

# Run benchmarks
cargo bench

# Code quality
cargo fmt      # Format
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
│   ├── invariants.rs     # Built-in invariants (v1)
│   ├── invariants_v2.rs  # Built-in invariants (v2)
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

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### How to Contribute

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/amazing-feature`
3. **Commit** your changes: `git commit -m 'Add amazing feature'`
4. **Push** to your branch: `git push origin feature/amazing-feature`
5. **Open** a Pull Request

### Areas for Contribution

- [ ] Additional TUI framework examples
- [ ] Windows PTY support
- [ ] More invariant types
- [ ] Performance optimizations
- [ ] Documentation improvements

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for detailed release history.

### Latest Changes (v0.2.0)

- ✨ **11 Built-in Invariants**: Response time, latency, viewport validity, process termination
- ✨ **Security Scanning**: Escape sequence and command injection detection
- ✨ **ReDoS Protection**: Safe regex with size limits
- ✨ **Deterministic Timing**: Tick-based scheduling without wall-clock dependencies
- ✨ **Enhanced Traces**: Versioned format with checkpoint support

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
