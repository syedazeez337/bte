# Contributing to BTE

Thank you for your interest in contributing to the Behavioral Testing Engine! This document outlines the process for contributing.

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Git
- GitHub account

### Development Setup

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR-USERNAME/bte.git
   cd bte
   ```

3. Set up the development environment:
   ```bash
   cargo build
   cargo test
   ```

## Making Changes

### Code Style

We follow Rust standard formatting:
```bash
cargo fmt
```

### Linting

We use clippy for linting:
```bash
cargo clippy
```

### Running Tests

```bash
cargo test              # Run all tests
cargo test --release    # Performance tests
cargo test -- --nocapture  # Show output
```

### Building Documentation

```bash
cargo doc --no-deps
```

## Submitting Changes

### Pull Request Process

1. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes and commit:
   ```bash
   git add .
   git commit -m "Add your feature description"
   ```

3. Push to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

4. Open a Pull Request against the `main` branch

### Pull Request Requirements

- All tests must pass
- Code must be formatted (`cargo fmt`)
- No clippy warnings
- Tests for new functionality
- Documentation updates
- Changelog entry

## Code Guidelines

### Determinism

All code must be deterministic:
- No `SystemTime` or wall-clock usage
- Use `DeterministicClock` for timing
- Use `SeededRng` for randomness
- Mark scheduling boundaries

### Testing

- Write tests for all new functionality
- Tests should be deterministic
- Avoid external dependencies in tests

### Documentation

- Document public APIs
- Update README for user-facing changes
- Add examples where helpful

## Reporting Issues

When reporting issues, include:
- Rust version (`rustc --version`)
- Operating system
- Steps to reproduce
- Expected behavior
- Actual behavior
- Any error messages

## Style Guide

- No comments (code should be self-documenting)
- Follow Rust naming conventions
- Use Result/Option appropriately
- Prefer explicit over implicit

## Communication

- Open an issue for bugs
- Open a discussion for ideas
- Be respectful and constructive

## Recognition

Contributors will be recognized in the README and release notes.

Thank you for contributing to BTE!
