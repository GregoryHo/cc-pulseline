# Architectural Patterns

## Provider Trait Pattern

Every external data source follows this structure:

```
pub trait FooCollector {
    fn collect(&self, ...) -> FooSnapshot;
}

struct RealFooCollector { ... }     // production impl
struct StubFooCollector { ... }     // test stub with preset data
```

When adding a new provider:
1. Define the trait in `providers/foo.rs`
2. Implement the real collector (e.g., `FileSystemFooCollector`)
3. Implement a stub collector (`StubFooCollector`) with builder-style setters
4. Re-export both from `providers/mod.rs`
5. Wire into `PulseLineRunner` in `lib.rs`
6. Add a field to `RenderFrame` in `types.rs`

## Config Layer Pattern

Three-layer config: TOML file → merge → runtime struct.

```
PulselineConfig          (concrete, with Default)
  + ProjectOverrideConfig  (all Option<T>, project wins)
    → merge_configs()
      → build_render_config()
        → RenderConfig       (flat runtime struct)
```

### Adding a New Config Field

Touch these places in order:

1. **`PulselineConfig`** — add field with `#[serde(default)]` + default in `Default` impl
2. **`ProjectOverrideConfig`** — add as `Option<T>` with `#[serde(default)]`
3. **`merge_configs()`** — add `if let Some(v) = project.field { user.field = v; }`
4. **`build_render_config()`** — wire the field to `RenderConfig`
5. **`RenderConfig`** — add the runtime field
6. **`default_config_toml()`** in `main.rs` — add commented example to template
7. **`default_project_config_toml()`** in `main.rs` — add commented example if relevant

All 7 places are in `config.rs` + `main.rs`. Miss one and the field silently falls back to default.

## Session State Pattern

- `PulseLineRunner` holds `HashMap<String, SessionState>` keyed by `session_id|transcript_path|project_path`
- `SessionState` tracks mutable per-session data: file offsets, active tools/agents, caches
- State persists to disk via `state/cache.rs` for cross-invocation continuity
- On first encounter of a session key, load from cache file if available

## Data Pipeline

```
stdin JSON → StdinPayload (serde)
  → PulseLineRunner.run()
    → providers collect snapshots
    → assemble RenderFrame
    → render::layout::render_frame() → Vec<String>
  → stdout (one line per element)
```

### Adding a New Data Source

1. Create provider trait + impls in `providers/`
2. Add snapshot fields to `RenderFrame` in `types.rs`
3. Wire provider call in `PulseLineRunner::run()` in `lib.rs`
4. Add formatting in `render/layout.rs`
5. Add show/hide toggle following the Config Layer Pattern above
6. Write integration test in `tests/`
