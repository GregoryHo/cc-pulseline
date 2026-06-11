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
6. **`default_config_toml()`** in `config.rs` — add commented example to template
7. **`default_project_config_toml()`** in `config.rs` — add commented example if relevant

All 7 places are in `config.rs`. Miss one and the field silently falls back to default.

## Visual Dispatch Hub Pattern

Widget-bearing segments (context, cost, quota, tools) compose via a `+`-joined visual spec parsed by a per-segment dispatch hub. Layouts call the hub with their preferred sizing; the hub picks which widgets to render and joins their outputs.

```
RenderConfig.context_visual ("gauge+sparkline")
  → frames::shared::render_context_visual(spec, …)
    → for each "+" component:
        match name {
          "gauge" => widgets::gauge::render(...)
          "sparkline" => widgets::sparkline::render(...)
          "text" => ctx_text_cell(...)
          _ => "" // unknown → silently drop, forward-compat
        }
    → join non-empty cells with " "
```

### Why dispatch via spec strings (not function pointers / trait objects)

- Config carries strings end-to-end — TOML, runtime, tests all speak the same vocabulary.
- Unknown widget names drop silently → an older binary parsing a newer config doesn't crash.
- Dispatch is one match arm per widget; the widget code stays free of trait machinery.

### Iron rules

- **Layouts never call `widgets::*::render` directly** — always go through the hub. Direct calls bypass user override capability for that segment.
- **Layout-specific decoration** (e.g. console's `/ <total>` after CTX) is composed *around* the hub output, not inside any widget.
- **Width budgeting** stays in the layout. The layout passes its preferred sizing (e.g. `FULL_GAUGE_WIDTH`) into the hub; the hub passes it through to widgets unchanged.
- **Layout-internal width gates** (e.g. a layout dropping sparkline below 100 cols) act on the spec *before* dispatch, not by branching on widget output.

### Adding a hub for a new segment

1. Add `*_visual: String` field via Config Layer Pattern (7 places).
2. Add `default_visuals_for` entry per layout in `frames/mod.rs`.
3. Add `effective_*_visual()` helper on `RenderConfig`.
4. Implement `render_<segment>_visual` in `frames/shared.rs`.
5. Refactor every existing call site to dispatch through the hub.
6. Add a composability test: pick one layout, override visual, assert output reflects the override. Add an Ascii catch-net assertion if the segment has icon-only widgets.

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
