# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-01-24

### Added
- **Platform Abstraction Layer**: New `src/platform/` module with `TerminalBackend` trait
  - Linux PTY backend (extracted from process.rs)
  - macOS backend (experimental)
  - Platform detection with graceful error messages
  - Prepared structure for Windows ConPTY support
- **Visual Testing**: Screenshot capture and comparison
  - `take_screenshot` action to capture terminal state
  - `assert_screenshot` action to compare against golden files
  - `--update-snapshots` CLI flag for regenerating baselines
  - Configurable ignore regions for dynamic content
- **256-Color and Truecolor Support**
  - SGR 38;5;N / 48;5;N (256-color palette)
  - SGR 38;2;R;G;B / 48;2;R;G;B (24-bit truecolor)
  - Full color attribute tracking in screen model
- **Fuzzy Pattern Matching**
  - `wait_for_fuzzy` action with Levenshtein distance
  - Configurable `max_distance` and `min_similarity` parameters
  - Edit distance calculation for approximate matching
- **Custom Invariants**: User-defined invariant checks
  - Pattern-based screen content validation
  - Optional cursor position assertions
  - Named invariants with descriptions
- **Mouse Support**: SGR 1006 protocol
  - `mouse_click` action with button selection
  - `mouse_scroll` action for scroll events
  - Terminal-native mouse event injection
- **New Actions**
  - `wait_screen`: Wait for pattern in screen content
  - `assert_not_screen`: Negative screen assertions
  - `assert_cursor`: Cursor position verification

### Changed
- Improved error messages throughout the codebase
- Better platform detection and warnings
- Enhanced test stability for PTY-based tests
- Code refactored for clippy compliance

### Fixed
- Flaky test in `test_send_keys_with_read` (added initialization delay)
- Unreachable pattern warning in process.rs
- Multiple clippy warnings and suggestions

### Documentation
- Added `docs/TUTORIAL.md`: Step-by-step guide for beginners
- Added `docs/API.md`: Complete API reference
- Updated README with new features and examples
- Updated FUTURE.md to reflect v0.4.0 completion

### Tests
- 293 total tests (255 unit + 38 integration)
- All tests pass with zero failures
- Zero clippy warnings with `-D warnings`
- Code formatted with `cargo fmt`

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
