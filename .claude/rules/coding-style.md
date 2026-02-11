# Coding Style

## Rust Idioms

- Use `Option<T>` instead of sentinels or magic values
- Use `Result<T, String>` for fallible operations (project convention — not `anyhow` or custom errors)
- Derive `Debug, Clone, Default, Serialize, Deserialize` on data structures
- Prefer `unwrap_or_default()` over `.unwrap()` — never `.unwrap()` on external data
- Chain with `and_then`, `map`, `unwrap_or_else` instead of nested `if let`
- Use `saturating_sub` / `saturating_add` for arithmetic that could underflow/overflow

## Error Handling

- **External data** (file I/O, JSON, TOML): always `map_err` with context string, never `.unwrap()`
- **Internal invariants**: `.expect("reason")` is acceptable for truly impossible states
- **Non-fatal warnings**: `eprintln!` to stderr, then continue with default
- **Cache/state I/O**: silent fallback — all load/save errors are intentionally ignored

## Data Structure Conventions

- **Config overlays**: all-`Option<T>` fields (see `ProjectOverrideConfig`)
- **Runtime structs**: concrete types with `Default` impl (see `RenderConfig`)
- **Serialized types**: `#[serde(default)]` on every field for backward compatibility
- **Display types**: `Serialize + Deserialize` on anything that persists to disk

## File Organization

- 200-400 lines typical, 800 max per file
- Group by responsibility, not by type
- One module = one concern (e.g., `render/color.rs` owns all color logic)
- Re-export public API from `mod.rs`

## Commits

Format: `<type>: <description>`

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

## Pre-Commit Checklist

- [ ] `cargo fmt` — no formatting diffs
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo test` — all tests pass
- [ ] No `.unwrap()` on external data
- [ ] No hardcoded ANSI codes (use `colorize()`)
