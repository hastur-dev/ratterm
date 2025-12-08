# Testing Guide

This document covers how to run tests for Ratterm locally and in CI.

## Quick Start

```bash
# Run all Rust tests
cargo test --all-features

# Run format check
cargo fmt --all -- --check

# Run clippy lints
cargo clippy --all-targets --all-features -- -D warnings

# Build documentation
cargo doc --no-deps --all-features
```

## Test Categories

### Unit Tests

Unit tests are located alongside the source code in `src/` and in the `tests/` directory.

```bash
# Run all unit tests
cargo test

# Run specific test module
cargo test editor::buffer::tests

# Run tests with output
cargo test -- --nocapture

# Run ignored tests (slow PTY tests)
cargo test -- --ignored
```

### Integration Tests

Integration tests are in the `tests/` directory:

| File | Description |
|------|-------------|
| `tests/editor_buffer_tests.rs` | Text buffer operations |
| `tests/terminal_grid_tests.rs` | Terminal grid rendering |
| `tests/terminal_parser_tests.rs` | ANSI escape sequence parsing |
| `tests/terminal_pty_tests.rs` | PTY operations (slow on Windows) |

### PTY Tests

PTY tests involve actual terminal interactions and can be slow, especially on Windows:

```bash
# Run PTY tests (may take 60+ seconds per test on Windows)
cargo test --test terminal_pty_tests

# Skip slow PTY tests during development
cargo test -- --skip test_pty
```

## Docker-Based CI Testing

For consistent CI testing that mirrors GitHub Actions, use Docker:

### Prerequisites

- Docker Desktop installed and running
- Docker Compose available

### Running Docker Tests

**Windows (PowerShell):**
```powershell
# Run all CI checks
.\scripts\test-local.ps1

# Run specific check
.\scripts\test-local.ps1 fmt      # Format check
.\scripts\test-local.ps1 clippy   # Clippy lints
.\scripts\test-local.ps1 test     # Unit tests
.\scripts\test-local.ps1 docs     # Documentation
.\scripts\test-local.ps1 audit    # Security audit
.\scripts\test-local.ps1 msrv     # MSRV check (Rust 1.85)

# Clean up Docker resources
.\scripts\test-local.ps1 clean
```

**Linux/macOS (Bash):**
```bash
# Run all CI checks
./scripts/test-local.sh

# Run specific check
./scripts/test-local.sh fmt
./scripts/test-local.sh clippy
./scripts/test-local.sh test
./scripts/test-local.sh docs
./scripts/test-local.sh audit
./scripts/test-local.sh msrv

# Clean up
./scripts/test-local.sh clean
```

### Docker Services

The Docker Compose configuration provides these services:

| Service | Description | Base Image |
|---------|-------------|------------|
| `fmt` | Format check | rust:1.85-bookworm |
| `clippy` | Clippy lints | rust:1.85-bookworm |
| `test` | Unit tests | rust:1.85-bookworm |
| `docs` | Documentation build | rust:1.85-bookworm |
| `audit` | Security audit | rust:1.85-bookworm |
| `msrv` | MSRV check | rust:1.85-bookworm |
| `ci-all` | All checks combined | rust:1.85-bookworm |

### Docker Volumes

Docker uses cached volumes for faster rebuilds:

- `cargo-cache`: Cargo registry cache
- `target-cache`: Build artifacts
- `target-cache-msrv`: MSRV-specific build artifacts

## Install Script Testing

Test the installation scripts locally before release:

### Windows PowerShell Script

```powershell
# Test install script syntax
.\scripts\test-install.ps1 syntax

# Test install script in dry-run mode (no actual install)
.\scripts\test-install.ps1 dry-run

# Test full install/uninstall cycle (uses temp directory)
.\scripts\test-install.ps1 full
```

### Linux/macOS Bash Script

```bash
# Test install script in Docker containers
./scripts/test-install.sh

# Test specific platform
./scripts/test-install.sh linux-x64
```

## CI Pipeline

GitHub Actions runs these checks on every PR:

1. **Format Check** - `cargo fmt --all -- --check`
2. **Clippy Lints** - `cargo clippy --all-targets --all-features -- -D warnings`
3. **Unit Tests** - `cargo test --all-features`
4. **Documentation** - `cargo doc --no-deps --all-features`
5. **Security Audit** - `cargo audit`
6. **MSRV Check** - Build with Rust 1.85

### Platform Matrix

CI tests run on:

| Platform | Architecture | Runner |
|----------|--------------|--------|
| Linux | x86_64 | ubuntu-latest |
| Windows | x86_64 | windows-latest |
| macOS | x86_64 | macos-13 |

## Code Coverage

Generate code coverage reports:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --all-features --out Html
```

## Mutation Testing

Verify test quality with mutation testing:

```bash
# Install cargo-mutants
cargo install cargo-mutants

# Run mutation testing
cargo mutants --all-features
```

## Benchmarks

Run performance benchmarks:

```bash
# Run benchmarks (requires nightly)
cargo +nightly bench
```

## Troubleshooting

### PTY Tests Hang on Windows

PTY tests can hang on Windows due to shell initialization. Solutions:

1. Use `--skip test_pty` to skip slow tests
2. Increase test timeout in CI
3. Run in Docker for consistent behavior

### Docker Build Fails

If Docker builds fail:

```powershell
# Clean Docker cache
.\scripts\test-local.ps1 clean

# Rebuild with no cache
docker compose -f docker/docker-compose.yml build --no-cache
```

### Cargo Audit Fails

If cargo-audit fails to install:

```bash
# Use --locked flag
cargo install cargo-audit --locked
```

## Writing Tests

### Test Guidelines

1. **Naming**: Use descriptive names like `test_buffer_insert_at_end`
2. **Isolation**: Tests should not depend on external state
3. **Assertions**: Include at least 2 assertions per test
4. **Bounded Loops**: All loops must have fixed upper bounds
5. **No Recursion**: Use iterative approaches

### Example Test

```rust
#[test]
fn test_buffer_insert_char() {
    // Arrange
    let mut buffer = Buffer::new();
    assert!(buffer.is_empty(), "Buffer should start empty");

    // Act
    buffer.insert_char('a', Position::new(0, 0));

    // Assert
    assert_eq!(buffer.len_chars(), 1, "Buffer should have 1 char");
    assert_eq!(buffer.get_char(0), Some('a'), "First char should be 'a'");
}
```

### Property-Based Tests

Use `proptest` for property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_buffer_insert_preserves_content(s in "\\PC*") {
        let mut buffer = Buffer::from(&s);
        prop_assert_eq!(buffer.to_string(), s);
    }
}
```
