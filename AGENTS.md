# Agent Guidelines for BTE Development

This document provides guidelines for AI assistants contributing to the Behavioral Testing Engine (BTE) project.

## Project Overview

BTE is a deterministic behavioral testing engine for CLI/TUI applications written in Rust. It provides:
- PTY-based terminal execution
- Deterministic scheduling and timing
- Trace and replay capabilities
- Invariant-based verification

## Core Principles

### Determinism Requirements

All code must be deterministic:
1. **No wall-clock time**: Use `DeterministicClock` instead of `SystemTime`
2. **Seeded RNG**: Use `SeededRng` with project seed for any randomness
3. **Explicit boundaries**: Mark scheduling boundaries with `scheduler.boundary()`

```rust
// BAD: Uses wall clock
let now = std::time::SystemTime::now();

// GOOD: Uses deterministic clock
let now = deterministic_clock.now();
```

### Code Style

1. **No comments**: The codebase follows a no-comment policy
2. **Minimal code**: Only what's necessary for functionality
3. **Testable**: All code should be unit testable
4. **Deterministic**: Identical runs produce identical results

### Module Organization

```
src/
├── ansi.rs          # ANSI escape parser
├── determinism.rs   # Clock, RNG, scheduler
├── invariants.rs    # Invariant framework and implementations
├── io_loop.rs       # Non-blocking IO
├── keys.rs          # Key injection
├── main.rs          # CLI entry point
├── process.rs       # PTY process management
├── pty.rs           # PTY allocation
├── runner.rs        # Scenario execution
├── scenario.rs      # Scenario schema
├── screen.rs        # Terminal screen model
├── timing.rs        # Timing and checkpoints
└── trace.rs         # Trace format and replay
```

## Testing Guidelines

### Test Requirements

1. All modules must have `#[cfg(test)]` tests
2. Tests should be deterministic
3. Tests should not depend on external resources
4. Prefer unit tests over integration tests

### Test Naming

```rust
#[test]
fn feature_does_expected_thing() {
    // Test implementation
}
```

## Scenario Format

When modifying the scenario schema (`scenario.rs`):
- Maintain backward compatibility
- Add validation for all new fields
- Document all step types
- Keep declarative (no imperative scripting)

## Invariant Development

When adding new invariants:
1. Implement `Invariant` trait
2. Add to `BuiltInInvariant` enum
3. Add to `to_evaluator()` method
4. Write unit tests
5. Document the invariant behavior

## Trace Format

When modifying trace format:
- Version the trace format
- Never remove fields (add new ones instead)
- Include replay seed in trace
- Document all checkpoint fields

## CLI Development

When adding new commands:
1. Add to `Command` enum
2. Implement handler function
3. Use proper error handling with `anyhow`
4. Return appropriate exit codes
5. Support `--help` documentation

### Exit Codes
- 0: Success
- -1: Process signaled
- -2: Invariant violation
- -3: Timeout
- -4: Error
- -5: Replay divergence

## Performance Considerations

1. Avoid allocations in hot paths
2. Use bounded buffers for IO
3. Consider lazy evaluation where appropriate
4. Profile before optimizing

## Security Considerations

1. Never log secrets or keys
2. Validate all inputs from scenarios
3. Use proper error handling
4. Sanitize trace output

## Common Patterns

### Adding a New Step Type

1. Add to `Step` enum in `scenario.rs`
2. Add validation in `validate_step()`
3. Implement in `execute_step()` in `runner.rs`
4. Add test

### Adding a New Invariant

1. Add struct implementing `Invariant` in `invariants.rs`
2. Add variant to `BuiltInInvariant` enum
3. Implement `to_evaluator()`
4. Add tests

### Adding a New Signal

1. Add to `SignalName` enum in `scenario.rs`
2. Add conversion in `to_nix_signal()`
3. Add test

## Dependencies

New dependencies must be:
1. Well-maintained
2. No unused features
3. Compatible with MIT license
4. Reviewed for security

Run `cargo audit` before adding dependencies.

## Pull Request Process

1. Ensure all tests pass
2. Run `cargo fmt` and `cargo clippy`
3. Update CHANGELOG.md
4. Update README.md if needed
5. Add tests for new functionality
6. Document breaking changes clearly

## CI/CD

The project uses GitHub Actions for:
- Building on multiple platforms
- Running all tests
- Code quality checks (clippy, fmt)
- Security auditing (cargo audit)

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Nomicon](https://doc.rust-lang.org/nomicon/)
