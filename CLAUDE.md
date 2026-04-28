# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cc-pulseline is a high-performance CLI tool that renders a multi-line statusline for Claude Code. It reads JSON from stdin (the Claude Code statusline payload) and outputs formatted text lines to stdout. The binary is designed to be called repeatedly by the Claude Code statusline hook.

## Build & Test Commands

```bash
cargo check          # Type-check without building
cargo build          # Build debug binary
cargo test           # Run all tests
cargo test <name>    # Run a single test by name, e.g. cargo test renders_core_metrics
cargo clippy -- -D warnings  # Lint (CI-enforced)
cargo fmt --check             # Format check (CI-enforced)
cargo bench          # Run benchmarks (benches/render_pipeline.rs)
```

The project uses Rust 2021 edition (MSRV 1.85) with `serde`, `serde_json`, `toml` as dependencies, and `tempfile`, `criterion` as dev-dependencies.

### CLI Flags

```bash
cc-pulseline --init           # Create user config (~/.claude/pulseline/config.toml)
cc-pulseline --init --project # Create project config (.claude/pulseline.toml)
cc-pulseline --check          # Validate config files
cc-pulseline --print          # Show effective merged config
cc-pulseline --preview        # Preview all themes (or --preview theme1 theme2)
cc-pulseline --select-theme   # Interactively select and apply a theme
cc-pulseline --palette-map    # Show palette field → UI element mapping
```

### Configuration

- **User config**: `~/.claude/pulseline/config.toml` — global defaults
- **Project config**: `{project_root}/.claude/pulseline.toml` — per-project overrides (deep merge, project wins)
- `PulselineConfig` (TOML) → `build_render_config()` → `RenderConfig` (runtime struct)
- `ProjectOverrideConfig` uses all-`Option<T>` fields; `merge_configs()` applies `Some` wins over user defaults

## Architecture

### Data Flow Pipeline

```
stdin JSON → StdinPayload (deserialize)
           → PulseLineRunner.run_from_str()
             → providers collect snapshots (env, git, transcript)
             → build RenderFrame (structured metrics)
             → render::layout::render_frame() → Vec<String> lines
           → stdout
```

### Module Responsibilities

- **`types.rs`** — All data structures: `StdinPayload` (input deserialization), `RenderFrame` and its line metrics (`Line1Metrics`, `Line2Metrics`, `Line3Metrics`), plus activity summaries (`ToolSummary`, `AgentSummary`, `TodoSummary`). `RenderFrame::from_payload()` does the initial field extraction.

- **`providers/`** — Trait-based collectors that gather data from external sources. Each has a real implementation and a `Stub*` for testing:
  - `env.rs` — `EnvCollector` scans for CLAUDE.md files, rules, memories, hooks, MCP servers, and skills. MCP parsing uses scoped dedup: user scope (`~/.claude/settings.json` + `~/.claude.json` minus `disabledMcpServers`) and project scope (`.mcp.json` + `.claude/settings.json` + `.claude/settings.local.json` minus `disabledMcpjsonServers`). Memory files are counted from `~/.claude/projects/{encoded-path}/memory/` (flat `.md` scan).
  - `git.rs` — `GitCollector` shells out to `git` for branch, dirty state, ahead/behind, file stats
  - `transcript.rs` — `TranscriptCollector` does incremental JSONL parsing of the Claude Code transcript file with seek-based offsets and poll throttling. This is the most complex provider — it maintains active tool/agent/todo state via `SessionState`

- **`state/mod.rs`** — `SessionState` holds per-session mutable state: transcript file offset, active tools/agents/todo lists, recent tools (persist after completion for display), and cached env/git snapshots. `PulseLineRunner` maintains a `HashMap<String, SessionState>` keyed by session+transcript+project.
  - `state/cache.rs` — Persists `SessionState` to `{temp_dir}/cc-pulseline-{hash}.json` across process invocations (prevents L3 metric flicker). Uses atomic writes (.tmp + rename) with silent failure on errors.

- **`config.rs`** — `RenderConfig` controls rendering behavior: glyph mode, color, `palette: ThemePalette` (resolved via `resolve_palette()`), line caps (`max_tool_lines`, `max_agent_lines`), transcript windowing, poll throttle, terminal width, width degradation strategy order, segment toggles (`show_git_stats`, `show_agent`, `show_worktree`, `show_speed`, `show_quota`, `show_quota_five_hour`, `show_quota_seven_day`), and per-segment visual specs (`context_visual`, `cost_visual`, `quota_visual`, `tools_visual`). Empty visual strings defer to the layout default via `effective_*_visual()` helpers — see `docs/layouts.md`.

- **`render/`** — Pure rendering logic, split into submodules:
  - `layout.rs` — Formats the `RenderFrame` into output lines (L1: identity, L2: config counts, L3: budget, L4+: activity). Applies `WidthDegradeStrategy` when `terminal_width` is set: drop activity lines → compress line 2 → truncate core lines. Dispatches to instrument-cluster layouts (`Cockpit`/`Console`/`Flightstrip`/`Auto`) which own their full pipeline; flat layouts (`None`/`Zones`/`Grid`/`Cards`/`Sections`) fall through to the v1-style line assembly.
  - `pane.rs` — `LayoutStyle` enum (9 variants) + `PaneConfig` chrome wrapper. `apply_pane()` decorates flat-layout output with frame chrome.
  - `frames/` — Per-layout `render()` fns: `cockpit`, `console`, `flightstrip`, `auto`, `cards`, `sections`, `grid`, `zones`. `frames/shared.rs` holds widget call helpers, dispatch hubs (`render_context_visual`, `render_cost_visual`, `render_quota_visual`, `render_tools_visual_inline`), and frame chrome glyph tables. `frames/mod.rs::default_visuals_for(LayoutStyle)` is the per-layout `*_visual` defaults table.
  - `widgets/` — Atomic widget renderers: `gauge` (block / `#`-`-`), `sparkline` (braille, icon-only), `arc` (cost burn, icon-only), `tape` (`▶ Read · ▶ Bash`). All take `(data, …, mode, palette, color)`; ascii-incompatible widgets return `""` so dispatch hubs drop them cleanly.
  - `color.rs` — `ThemePalette` struct (31 color fields), built-in theme loading (JSON via `include_str!`), custom theme discovery (`~/.claude/pulseline/themes/`), `resolve_palette()` for theme+variant+overrides resolution, legacy `pub const` color values for test compatibility, and `colorize()`/`strip_ansi()` utilities
  - `fmt.rs` — Number formatting (`format_number`), duration formatting (`format_duration`), speed formatting (`format_speed`), reset duration formatting (`format_reset_duration`), and agent/todo elapsed formatting (`format_agent_elapsed`)
  - `icons.rs` — Nerd Font icon constants and `glyph()` helper for icon/ascii mode switching

- **`lib.rs`** — Orchestrates the pipeline: `PulseLineRunner` manages sessions, calls providers, assembles the `RenderFrame`, and delegates to the renderer. Also exposes `run_from_str()` as a stateless convenience.

### Layouts & Visual Composition

`pane.rs::LayoutStyle` enumerates 9 layouts (`None`/`Zones`/`Grid`/`Cards`/`Sections` are flat-row decorators; `Cockpit`/`Console`/`Flightstrip`/`Auto` own their full pipeline). Each layout asserts a default `(context, cost, quota, tools)` visual tuple via `frames::default_visuals_for(LayoutStyle)`. The user's TOML `*_visual` strings override per segment when non-empty; otherwise `effective_*_visual()` falls back to the layout default.

Widget composition runs through dispatch hubs in `frames/shared.rs` (`render_context_visual`, `render_cost_visual`, `render_quota_visual`, `render_tools_visual_inline`). Layouts call the hub with their preferred sizing; the hub parses the `+`-joined spec and composes widget outputs. **New widgets must register with a hub** — never call `widgets::*::render` directly from a layout, or the user loses composability for that segment.

Full layout × visual reference: `docs/layouts.md`.

### Output Line Format

- **L1**: `M:{model} | AG:{agent} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [↑n] [↓n] [!n +n ✘n ?n] (WT)`
- **L2**: `1 CLAUDE.md | 2 rules | 3 memories | 1 hooks | 2 MCPs | 2 skills | 1h` (value-first format, all togglable)
- **L3**: `CTX:43% (86.0k/200.0k) | TOK I:10 O:20 ↗1.5K/s C:30/40 | $3.50 ($3.50/h)` (text form; instrument-cluster layouts may render as gauge / sparkline / arc per `*_visual`)
- **Quota**: `Q: 5h: 75% (resets 2h 0m)` (usage quota from CC's native `rate_limits` field, between L3 and activity)
- **L4a**: `✓ Read ×12 | ✓ Bash ×8 | ✓ Edit ×5` (completed tool counts — stable, accumulates over session; capped by `max_completed_lines` rows)
- **L4b**: `T:Read: .../main.rs | T:Bash: cargo test` (recent/running tools with targets — volatile)
- **L5+**: `A:Explore [haiku]: Investigate logic (2m)` (agents — active first, then recent completed)
- **TODO variants**:
  - In-progress: `TODO:Fixing auth bug (1/3) (5s)` or `(1/3, 3 active)` (multi-line, capped by `max_todo_lines`)
  - Pending only: `TODO:3 tasks (0/3)` (task API, no in-progress items)
  - All done: `✓ All todos complete (3/3)` (celebration line)
  - Legacy: `TODO:1/3 done, 2 pending` (old TodoWrite path)

### Testing Patterns

Tests are integration-level in `tests/` and use `tempfile::TempDir` for filesystem isolation:
- **`core_metrics.rs`** — Creates a real git repo + config files in a tempdir, calls `run_from_str()`, asserts output content
- **`activity_pipeline.rs`** — Uses `PulseLineRunner` with incremental transcript appending to test tool/agent/todo lifecycle
- **`adaptive_performance.rs`** — Tests width degradation and rendering performance budgets
- **`smoke_cli.rs`** — Spawns the actual binary with `CARGO_BIN_EXE_cc-pulseline`, pipes fixture JSON via stdin
- **`cli_flags.rs`** — Tests `--init`, `--check`, `--print` CLI flag behavior
- **`config_merge.rs`** — Tests user + project config deep merge logic
- **`segment_toggles.rs`** — Tests individual segment show/hide config toggles
- **`session_cache.rs`** — Tests session state persistence and L3 cache fallback
- **`git_file_stats.rs`** — Tests git file stats (modified/added/deleted/untracked counts)
- **`output_speed.rs`** — Tests output speed tracking (delta-based tok/s computation)
- **`quota_display.rs`** — Tests quota percentage rendering, color thresholds, reset format, width degradation
- **`agent_worktree.rs`** — Tests agent name display on L1 and worktree `(WT)` indicator, including toggle behavior and stdin parsing

Test fixtures live in `tests/fixtures/` as `.json` (stdin payloads) and `.jsonl` (transcript streams).

### Key Design Decisions

- **Trait-based providers with stubs** — Every external data source (env, git, transcript) uses a trait so tests can substitute stubs. The real implementations are `FileSystemEnvCollector`, `LocalGitCollector`, `FileTranscriptCollector`.
- **Incremental transcript parsing** — The transcript collector seeks to the last read offset rather than re-parsing the entire file. It applies event windowing (`transcript_window_events`) and poll throttling (`transcript_poll_throttle_ms`).
- **Session-keyed state** — `PulseLineRunner` tracks multiple sessions by `session_id|transcript_path|project_path` composite key, enabling correct behavior when multiple Claude Code sessions run concurrently.

### Color System

The project uses a `ThemePalette` struct with 31 ANSI 256-color fields, resolved at runtime by `resolve_palette(theme, variant, overrides)`. See `docs/theme-palette.md` for the full specification. Key principles:

- **8 built-in themes** — JSON files in `src/themes/` embedded via `include_str!()`: tokyo-night (default), echo-sub-zero, titanium-precision, cnc-telemetry, cyberdeck-hud, stark-hud, mako-reactor, aburaya-twilight
- **Custom themes** — JSON files in `~/.claude/pulseline/themes/` loaded at runtime with per-process caching
- **Per-color overrides** — `[colors]` TOML section applies on top of any preset (e.g., `alert_red = 160`)
- **Emphasis tiers** (Primary/Secondary/Structural/Separator) vary by dark/light variant within each theme
- **Semantic colors** (stable_blue, alert_red, etc.) are theme-specific but consistent within a theme
- **Icon color = value color** — icons are never independently dimmed
- **Backward compat** — `theme = "dark"` and `"light"` map to tokyo-night with the appropriate variant
- Layout functions receive `&ThemePalette` via `config.palette`; legacy `pub const` values retained for test assertions
- Preview themes: `cc-pulseline --preview`