# Behavioral Testing Engine (BTE)

**Deterministic testing for CLI and TUI applications.**

BTE executes real terminal applications in a pseudo-terminal (PTY), captures all output, and verifies behavioral invariants automatically.

[![Rust 1.82+](https://img.shields.io/badge/rust-1.82+-blue?logo=rust)](https://rustup.rs/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

---

## Quick Start

```bash
# Build from source
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo build --release

# Run a test scenario
./target/release/bte run /home/aze/gitcontribs/personal/exp/bte-test-projects/tests/unit/process/exit-codes.yaml
```

---

## What BTE Does

```
┌─────────────┐     ┌─────────┐     ┌──────────────┐     ┌───────────┐
│ YAML Scenario├────►│ BTE     ├────►│ PTY Process  ├────►│ Invariants│
└─────────────┘     └─────────┘     └──────────────┘     └───────────┘
                                              │                  │
                                              ▼                  ▼
                                       ┌──────────────┐     ┌───────────┐
                                       │ ANSI Parser  │     │ Pass/Fail │
                                       │ Screen Model │     └───────────┘
                                       └──────────────┘
```

1. **Reads** a YAML scenario describing test steps
2. **Spawns** the application in a real PTY
3. **Sends** keystrokes, waits for patterns, injects signals
4. **Verifies** invariants (cursor bounds, deadlock, screen content)
5. **Reports** pass/fail with detailed output

---

## Writing Tests

Create a `.yaml` file with your test scenario:

```yaml
name: "test-name"
description: "What this test validates"
command: "sh"

steps:
  - action: send_keys
    keys: "echo 'Hello!'\n"
  - action: wait_for
    pattern: "Hello!"
    timeout_ms: 5000

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 10000

seed: 42
timeout_ms: 30000
```

### Available Actions

| Action | Description |
|--------|-------------|
| `send_keys` | Send keystrokes to the terminal |
| `wait_for` | Wait for a pattern to appear |
| `wait_ticks` | Wait for N scheduling ticks |
| `resize` | Change terminal size (cols, rows) |
| `send_signal` | Send POSIX signal (SIGINT, SIGTERM, SIGKILL, SIGWINCH, SIGCONT) |

### Key Names

Special keys: `Enter`, `Escape`, `Tab`, `Backspace`, `Up`, `Down`, `Left`, `Right`

Ctrl modifiers: `Ctrl_c` (Ctrl+C), `Ctrl_d`, `Ctrl_z`

### Invariants

```yaml
invariants:
  - type: cursor_bounds           # Cursor stays on screen
  - type: no_deadlock             # App doesn't hang
    timeout_ms: 30000
  - type: screen_contains         # Expected text appears
    pattern: "success"
```

---

## Running Tests

```bash
# Run a single scenario
bte run my-test.yaml

# Exit codes:
#   0 - All steps completed, invariants passed
#  -1 - Process terminated by signal
#  -2 - Invariant violation
#  -3 - Timeout
```

### Test Results

```
=== Run Result ===
Exit code: 0
Steps executed: 5
Ticks: 0
Status: SUCCESS (exit=0, ticks=0)
```

---

## Project Structure

```
bte/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library API
│   ├── runner.rs         # Scenario execution
│   ├── scenario.rs       # YAML parsing
│   ├── invariants.rs     # Built-in invariants
│   ├── process.rs        # PTY management
│   ├── screen.rs         # Terminal state
│   ├── ansi.rs           # ANSI escape parsing
│   └── vtparse.rs        # VT sequence parser
├── tests/                # Integration tests
└── Cargo.toml
```

---

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
bte = { path = "/path/to/bte" }
```

Use programmatically:

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

---

## Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run all tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

---

## Tested Applications

BTE has been validated with:

- **Shells**: bash, sh
- **TUI apps**: gitui, fzf, bottom
- **CLI tools**: Any command-line application

---

## License

MIT License. See [LICENSE](LICENSE) for details.

## Roadmap

See [FUTURE.md](FUTURE.md) for the comprehensive feature roadmap, including:
- Missing actions (mouse, clipboard, conditional logic)
- Missing invariants (memory, flicker, color validation)
- Unsupported ANSI escape sequences
- Platform support plans (macOS, Windows)
- Implementation phases and success metrics
