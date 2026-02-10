# Contributing to cc-pulseline

Thank you for your interest in contributing to cc-pulseline!

## Development Setup

### Prerequisites

- Rust 1.74+ (for `let-else` syntax and `toml` 0.8)
- Git

### Getting Started

```bash
git clone https://github.com/GregoryHo/cc-pulseline.git
cd cc-pulseline
cargo test
```

### Running Checks

```bash
cargo test                       # Run all tests
cargo clippy -- -D warnings      # Lint with zero warnings
cargo fmt --check                # Verify formatting
cargo bench                      # Run benchmarks (optional)
```

## Making Changes

1. Fork the repository and create a branch from `main`
2. Write tests for new functionality
3. Ensure all tests pass: `cargo test`
4. Ensure clippy passes: `cargo clippy -- -D warnings`
5. Format your code: `cargo fmt`
6. Submit a pull request

## Project Structure

- `src/` - Main source code
  - `types.rs` - Data structures
  - `config.rs` - TOML config and runtime config
  - `providers/` - External data collectors (env, git, transcript)
  - `render/` - Output formatting (layout, color, formatting)
  - `state/` - Session state and disk cache
- `tests/` - Integration tests
- `benches/` - Criterion benchmarks
- `docs/` - Documentation

## Testing

Tests are integration-level in `tests/` and use `tempfile::TempDir` for filesystem isolation. Test patterns:

- Create fixture JSON payloads
- Call `run_from_str()` or use `PulseLineRunner`
- Assert output content with `contains()` (avoids brittle exact matches)
- Use `color_enabled: false` in config for predictable plain-text assertions

## Code Style

- Follow existing patterns in the codebase
- Keep dependencies minimal (currently 3 runtime crates)
- Prefer composition over inheritance
- Test behavior, not implementation details
- Use `RenderConfig::default()` for test configs, override only what's needed

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
