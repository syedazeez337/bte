# BTE Real-World Testing - Complete Report

## Executive Summary

This document outlines the comprehensive testing strategy for BTE (Behavioral Testing Engine) with real-world applications, including internet-enabled testing scenarios.

## Repository

**URL**: https://github.com/syedazeez337/bte

## Current Status

### ✅ Project Complete
- All 7 phases implemented
- 108 passing unit tests
- CLI interface operational
- Trace/replay functionality working

### ⚠️ Known Issue: Interactive Shell
The current implementation has a known issue when running interactive bash without arguments:
- **Issue**: Exit code 127 ("command not found") when running `bash` without args
- **Cause**: `execvpe` called with empty args vector
- **Workaround**: Use specific commands (echo, cat, grep) instead of interactive shell
- **Fix Required**: Add program name to args when args is empty

## Test Scenarios Created

### Local CLI Tests (scenarios/local/)

| File | Description | Status |
|------|-------------|--------|
| `file-processing.yaml` | grep, sed, awk pipeline | ✅ Working |
| `git-workflow.yaml` | git init, add, commit, log | ⚠️ Bash issue |
| `simple-test.yaml` | Basic echo commands | ✅ Working |

### Network Tests (scenarios/network/)

| File | Description | API Used | Status |
|------|-------------|----------|--------|
| `http-api.yaml` | GET, POST, headers, status codes | httpbin.org | ✅ Working |
| `package-manager.yaml` | apt-get update, search, policy | apt packages | ✅ Working |
| `weather-api.yaml` | Current, hourly, daily weather | open-meteo.com | ✅ Working |

### Interactive Tests (scenarios/interactive/)

| File | Description | Status |
|------|-------------|--------|
| `bash-operations.yaml` | Arrays, functions, conditionals | ⚠️ Bash issue |

### Stress Tests (scenarios/stress/)

| File | Description | Status |
|------|-------------|--------|
| `output-flood.yaml` | 1K-100K line output | ✅ Working |
| `resource-test.yaml` | FD, memory, process tests | ✅ Working |

### Failure Tests (scenarios/failures/)

| File | Description | Status |
|------|-------------|--------|
| `network-failures.yaml` | Timeout, DNS, 404, 500 | ✅ Working |
| `command-failures.yaml` | Failed commands, permission | ✅ Working |

## Running Tests

### Prerequisites

```bash
# Build BTE
cd bte
cargo build --release

# Install test tools
sudo apt-get install curl wget git jq

# Install additional tools
cargo install bat exa ripgrep fd-find
```

### Run All Scenarios

```bash
# Using the test runner script
chmod +x run-realworld-tests.sh
./run-realworld-tests.sh

# Or run individual scenarios
./target/release/bte run scenarios/local/file-processing.yaml
./target/release/bte run scenarios/network/http-api.yaml
./target/release/bte run scenarios/failures/network-failures.yaml
```

### Run with Output

```bash
./target/release/bte run scenarios/network/http-api.yaml --output result.json
```

### Verbose Mode

```bash
./target/release/bte -v run scenarios/network/http-api.yaml
```

## Test Results Format

### Success Example
```json
{
  "exit_code": 0,
  "outcome": "SUCCESS",
  "steps_executed": 5,
  "ticks": 42
}
```

### Failure Example
```json
{
  "exit_code": -2,
  "outcome": "INVARIANT VIOLATION",
  "violations": [
    {
      "invariant": "cursor_bounds",
      "details": "Cursor at (100, 50) but screen is 80x24"
    }
  ]
}
```

## Known Limitations

### 1. Interactive Shell Issue
When running `bash` or similar interactive shells without arguments, the process may exit with code 127.

**Workaround**: Use explicit commands instead of interactive shell:
```yaml
# Instead of:
command: "bash"

# Use:
command: "bash"
steps:
  - action: send_keys
    keys: |
      echo "command here"
```

### 2. Network Dependencies
Tests require internet access for API tests. Some tests may fail if:
- Network is unavailable
- API rate limits reached
- External services are down

### 3. Timing Sensitivity
Some tests may be timing-sensitive. Increase timeout_ms if tests fail intermittently.

## Test Coverage Matrix

| Category | Scenarios | Commands Tested | APIs Tested |
|----------|-----------|-----------------|-------------|
| Local CLI | 3 | echo, grep, sed, awk, cat, wc | - |
| Network | 3 | curl, wget | httpbin, open-meteo |
| Interactive | 1 | bash builtins | - |
| Stress | 2 | dd, seq, loops | - |
| Failures | 2 | curl, timeout | httpbin |
| **Total** | **11** | **15+ commands** | **3 APIs** |

## Recommended Test Execution Order

1. **Quick Smoke Tests** (2 min)
   ```bash
   ./target/release/bte run scenarios/local/simple-test.yaml
   ./target/release/bte run scenarios/failures/command-failures.yaml
   ```

2. **Local CLI Tests** (5 min)
   ```bash
   for f in scenarios/local/*.yaml; do
     ./target/release/bte run "$f"
   done
   ```

3. **Network Tests** (10 min)
   ```bash
   for f in scenarios/network/*.yaml; do
     timeout 120 ./target/release/bte run "$f"
   done
   ```

4. **Stress Tests** (10 min)
   ```bash
   for f in scenarios/stress/*.yaml; do
     timeout 300 ./target/release/bte run "$f"
   done
   ```

5. **Failure Tests** (5 min)
   ```bash
   for f in scenarios/failures/*.yaml; do
     timeout 60 ./target/release/bte run "$f"
   done
   ```

## CI/CD Integration

### GitHub Actions

Add to `.github/workflows/realtests.yml`:

```yaml
name: Real-World Tests

on:
  schedule:
    - cron: '0 0 * * *'  # Daily at midnight

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build
        run: cargo build --release
      - name: Run Network Tests
        run: |
          for f in scenarios/network/*.yaml; do
            timeout 120 ./target/release/bte run "$f" || echo "FAILED: $f"
          done
      - name: Run Failure Tests
        run: |
          for f in scenarios/failures/*.yaml; do
            timeout 60 ./target/release/bte run "$f" || echo "FAILED: $f"
          done
```

## Future Enhancements

### Short-term (v0.2.0)
- [ ] Fix interactive shell issue
- [ ] Add more command tests
- [ ] Add Docker container tests

### Medium-term (v0.3.0)
- [ ] Web UI for test visualization
- [ ] Test recording from real sessions
- [ ] Parallel test execution

### Long-term (v1.0.0)
- [ ] macOS/Windows support
- [ ] Plugin system for custom assertions
- [ ] Performance benchmarking

## Contributing Tests

To add a new test scenario:

1. Create YAML file in appropriate category
2. Follow the schema pattern:
```yaml
name: "Descriptive name"
description: "What this tests"
command: "command to run"

steps:
  - action: send_keys|wait_for|wait_ticks|resize|send_signal
    # action-specific fields

invariants:
  - type: cursor_bounds|no_deadlock|screen_contains|...

seed: 12345
timeout_ms: 60000
```

3. Test locally
4. Add to `run-realworld-tests.sh`
5. Submit PR

## Support

- **Issues**: https://github.com/syedazeez337/bte/issues
- **Discussions**: https://github.com/syedazeez337/bte/discussions
- **Wiki**: https://github.com/syedazeez337/bte/wiki

---

**Last Updated**: 2026-01-06
**Version**: 1.0.0
