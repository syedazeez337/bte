# BTE

[![CI](https://github.com/syedazeez337/bte/actions/workflows/ci.yml/badge.svg)](https://github.com/syedazeez337/bte/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)

**Behavioral Testing Engine** — Deterministic testing for terminal applications.

BTE is like [Playwright](https://playwright.dev/) for the terminal. It spawns real processes in a PTY, sends input, captures output, and verifies behavior automatically.

## Features

- **Language-agnostic** — Test any CLI/TUI app: Rust, Python, Go, Node.js, Bash, C/C++
- **Deterministic** — Seed-based execution for reproducible tests
- **Declarative** — Define tests in YAML, no code required
- **Comprehensive** — Signal injection, mouse events, screen assertions, custom invariants
- **Debuggable** — Trace recording and replay for investigating failures

## Installation

```bash
git clone https://github.com/syedazeez337/bte.git
cd bte
cargo build --release
```

The binary will be at `./target/release/bte`.

**Requirements:** Rust 1.82+, Linux (macOS experimental)

## Quick Start

Create `test.yaml`:

```yaml
name: hello-world
command: "echo 'Hello, World!'"

steps:
  - action: wait_for
    pattern: "Hello, World"

invariants:
  - type: cursor_bounds
```

Run it:

```bash
$ bte run test.yaml
=== Run Result ===
Exit code: 0
Steps executed: 1
Ticks: 0
Status: SUCCESS (exit=0, ticks=0)
```

## Usage

```bash
bte run <scenario.yaml>           # Run a test
bte run <scenario> -o trace.json  # Run and save trace
bte validate <scenario.yaml>      # Validate syntax
bte info <trace.json>             # Inspect trace
bte replay <trace.json>           # Replay for verification
```

**Options:**

```
-s, --seed <N>       Override random seed
--max-ticks <N>      Max execution ticks (default: 10000)
-v, --verbose        Debug output
```

## Scenario Format

```yaml
name: my-test
command: bash
terminal: { cols: 80, rows: 24 }

steps:
  - action: send_keys
    keys: "echo hello\n"
  - action: wait_for
    pattern: "hello"
    timeout_ms: 5000
  - action: send_signal
    signal: SIGTERM

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 5000

seed: 42
timeout_ms: 30000
```

### Actions

| Action | Description |
|--------|-------------|
| `send_keys` | Send keystrokes (`\n`, `${Enter}`, `${Ctrl_c}`) |
| `wait_for` | Wait for regex pattern in output |
| `wait_screen` | Wait for pattern in screen buffer |
| `wait_ticks` | Wait N scheduling ticks |
| `send_signal` | Send signal (SIGTERM, SIGINT, SIGKILL, etc.) |
| `resize` | Resize terminal (cols, rows) |
| `mouse_click` | Click at position |
| `mouse_scroll` | Scroll at position |
| `assert_screen` | Assert screen contains pattern |
| `assert_cursor` | Assert cursor position |
| `snapshot` | Capture screen state |

### Invariants

| Type | Description |
|------|-------------|
| `cursor_bounds` | Cursor stays within screen |
| `no_deadlock` | Process produces output within timeout |
| `screen_contains` | Screen contains pattern |
| `screen_stable` | Screen unchanged for N ticks |
| `custom` | User-defined pattern/cursor checks |

### Special Keys

```
${Enter}  ${Escape}  ${Tab}  ${Backspace}
${Up}  ${Down}  ${Left}  ${Right}
${Home}  ${End}  ${PageUp}  ${PageDown}
${F1}..${F12}
${Ctrl_a}..${Ctrl_z}
${Alt_x}
```

## Examples

**Interactive shell test:**

```yaml
name: bash-test
command: bash

steps:
  - action: send_keys
    keys: "ls -la\n"
  - action: wait_for
    pattern: "total"
    timeout_ms: 5000
  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
```

**Signal handling:**

```yaml
name: signal-test
command: "sleep 30"

steps:
  - action: wait_ticks
    ticks: 10
  - action: send_signal
    signal: SIGTERM
```

**TUI application:**

```yaml
name: fzf-test
command: fzf

steps:
  - action: send_keys
    keys: "query"
  - action: wait_screen
    pattern: "query"
  - action: send_keys
    keys: "${Enter}"

invariants:
  - type: no_deadlock
    timeout_ms: 10000
```

See [`examples/`](examples/) and [`scenarios/`](scenarios/) for more.

## Debugging

Save execution traces for debugging:

```bash
# Save trace
bte run test.yaml -o trace.json

# Inspect
bte info trace.json

# Replay to verify determinism
bte replay trace.json
```

Use verbose mode for detailed output:

```bash
bte -v run test.yaml
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
| 124+ | Signaled (124 = SIGTERM, 137 = SIGKILL) |

## Documentation

- [Tutorial](docs/TUTORIAL.md) — Step-by-step guide
- [API Reference](docs/API.md) — Complete documentation
- [Changelog](CHANGELOG.md) — Version history
- [Roadmap](FUTURE.md) — Planned features

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting PRs.

```bash
cargo test          # Run tests
cargo clippy        # Lint
cargo fmt           # Format
```

## License

MIT — see [LICENSE](LICENSE)
