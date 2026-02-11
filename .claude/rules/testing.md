# Testing

## Structure

- **Unit tests**: `#[cfg(test)] mod tests` inside source files
- **Integration tests**: `tests/*.rs` — each file covers one concern
- **Binary tests**: `tests/smoke_cli.rs` — spawns actual binary via `CARGO_BIN_EXE_cc-pulseline`
- **Benchmarks**: `benches/` — uses `criterion`

## Filesystem Isolation (CRITICAL)

ALWAYS use `tempfile::TempDir` for tests that touch the filesystem:
- Create fake HOME directories to isolate env collection
- Create real git repos in tempdirs for git provider tests
- Never rely on the host machine's real filesystem state
- Use `FileSystemEnvCollector { user_home_override: Some(fake_home) }` for env tests

## Transcript Testing Pattern

```
1. Create TempDir
2. Write transcript lines with append_line() helper
3. Call run_from_str() or PulseLineRunner methods
4. Assert output with contains() checks
```

## Test Config Overrides

Always set these in test configs to avoid flaky timing:
- `transcript_poll_throttle_ms: 0` — disable poll throttling
- Explicit `max_tool_lines` / `max_agent_lines` — don't rely on defaults
- `color_enabled: false` — unless specifically testing color output

## Assertion Patterns

- **Line 2** (env counts): use `contains()` — counts vary by user scope / real HOME
- **Line 3** (budget metrics): use `assert_eq!` — fully deterministic from stdin payload
- **Activity lines**: check prefixes like `"T:"`, `"A:"`, `"TODO:"` — plain text when color disabled
- **Tool targets**: check that target extraction works (e.g., `"T:Read: .../main.rs"`)

## Config Testing

- Parse TOML strings directly: `toml::from_str::<PulselineConfig>(&toml_str)`
- Test merge: create user + project configs, call `merge_configs()`, verify `Some` wins
- Test build: call `build_render_config()`, verify `RenderConfig` fields

## Adding a New Test

1. Find the most similar existing test in `tests/`
2. Follow its setup pattern (TempDir, fixtures, config)
3. Test behavior, not implementation details
4. One logical assertion per test when possible
5. Use descriptive `#[test] fn` names: `verb_noun_condition`
