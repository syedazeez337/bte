# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-01-06

### Added
- Initial release of Behavioral Testing Engine (BTE)
- PTY creation and management
- Process spawning inside PTY
- Signal injection (SIGINT, SIGTERM, SIGKILL, SIGWINCH)
- Non-blocking IO loop with epoll
- ANSI escape sequence parser
- Screen grid model with attributes
- Scrollback buffer
- Cursor tracking and state hashing
- Deterministic clock and seeded RNG
- Scenario schema (YAML/JSON)
- Key injection engine
- Resize and timing control
- Invariant framework
- Core invariants (cursor bounds, deadlock, signal handling)
- Trace format and serialization
- Replay engine
- CLI interface (run, replay, validate, info commands)

### Features
- Deterministic execution with seed control
- Full terminal state capture
- Replayable traces for debugging
- Invariant-based testing
- Multiple output formats (JSON, YAML)

### Tests
- 108 passing unit tests
- Integration tests for core functionality

### Documentation
- Complete README with examples
- API documentation in code
- Scenario format reference

## [0.0.1] - 2026-01-05

### Initial Development
- Prototype implementation
- Core PTY functionality
- Basic screen model
