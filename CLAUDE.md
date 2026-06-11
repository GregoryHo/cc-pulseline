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

- **`config.rs`** — `RenderConfig` controls rendering behavior: glyph mode, color, `palette: ThemePalette` (resolved via `resolve_palette()`), line caps (`max_tool_lines`, `max_agent_lines`), transcript windowing, poll throttle, terminal width, width degradation strategy order, segment toggles (`show_git_stats`, `show_agent`, `show_worktree`, `show_speed`, `show_quota`, `show_quota_five_hour`, `show_quota_seven_day`), the total-row cap (`max_total_lines` + `height_degrade_order`), and per-segment visual specs (`context_visual`, `quota_visual`, `agents_visual`, `tools_visual`, `todo_visual`). Empty visual strings defer to the layout default via `effective_*_visual()` helpers — see `docs/layouts.md`. Transcript perf knobs (`transcript_window_events`, `transcript_poll_throttle_ms`) are tunable via the `[performance]` TOML section.

- **`render/`** — Pure rendering logic, split into submodules:
  - `layout.rs` — Assembles the `RenderFrame` into output lines (L1: identity, L2: config counts — opt-in, L3: budget, L4+: activity). Applies `WidthDegradeStrategy` when `terminal_width` is set: drop activity lines → compress line 2 → truncate core lines. When `max_total_lines` is set (integer or `"auto"` = ~25% of terminal height from `LINES`/ioctl), a `HeightDegradeStrategy` ladder re-assembles with cumulative collapses (drop running tools → completed/agents/todo to 1 row each → fuse activity into one row → quota into L3 → drop L2 → fuse-core: identity+budget+quota into the compact head row → hard truncate); chrome rows count against the cap; Ledger is exempt (use `ledger_dense`). Note compact ≠ none + `max_total_lines = 2`: idle, the ladder stops early (L1 / L3+quota on separate rows) and FuseCore is never reached. Single pipeline: every layout except `Ledger` flows through here and is decorated by `apply_pane()`; `Ledger` owns its full pipeline because the TAG-column rhythm doesn't compose via `apply_pane`; `Compact` short-circuits to its own 1–2 row assembly inside the flat pipeline.
  - `pane.rs` — `LayoutStyle` enum (4 variants: `None`/`Compact`/`Console`/`Ledger`) + `PaneConfig` chrome wrapper. `apply_pane()` decorates the assembled lines with frame chrome.
  - `frames/` — Per-layout `render()` fns: `console`, `ledger`. `console` renders the single outer `╭─...─╮` frame with `├─┼─┤` group separators and the Identity row hoisted into the top frame title. `frames/shared.rs` holds the box-drawing glyphs, label/content padding, identity headline, config row, and the per-segment dispatch hubs (`render_context_visual` and `render_quota_visual` — each maps a `+`-joined visual spec onto `widgets::gauge::render` / `widgets::sparkline::render` / inline text cells). `frames/mod.rs::default_visuals_for(LayoutStyle)` is the per-layout `*_visual` defaults table.
  - `widgets/` — Atomic widget renderers: `gauge` (bracketless marks-on-track — `▰` filled / `─` empty / `·` threshold marks in Icon mode, `=` / `-` / `:` in Ascii) and `sparkline` (braille, icon-only — caller picks the fill color). Both take `(data, …, marks, mode, palette, color)` shape; ascii-incompatible widgets return `""` so dispatch hubs drop them cleanly. `gauge`'s `width` is the visible cell count (no frame); caller supplies threshold marks (CTX → `ThemePalette::ctx_marks()` = `[55, 70]`; quota → `[50, 85]`).
  - `color.rs` — `ThemePalette` struct (34 color fields), built-in theme loading (JSON via `include_str!`), custom theme discovery (`~/.claude/pulseline/themes/`), `resolve_palette()` for theme+variant+overrides resolution, legacy `pub const` color values for test compatibility, and `colorize()`/`strip_ansi()` utilities
  - `fmt.rs` — Number formatting (`format_number`), duration formatting (`format_duration`), speed formatting (`format_speed`), reset duration formatting (`format_reset_duration`), and agent/todo elapsed formatting (`format_agent_elapsed`)
  - `icons.rs` — Nerd Font icon constants and `glyph()` helper for icon/ascii mode switching

- **`lib.rs`** — Orchestrates the pipeline: `PulseLineRunner` manages sessions, calls providers, assembles the `RenderFrame`, and delegates to the renderer. Also exposes `run_from_str()` as a stateless convenience.

### Layouts & Visual Composition

`pane.rs::LayoutStyle` enumerates the layouts (`None` / `Compact` (1–2 rows, idle = 1) / `Console` (framed, identity-in-title) / `Ledger`; treat the enum as the source of truth). Each layout asserts a default `(context, cost, quota, tools)` visual tuple via `frames::default_visuals_for(LayoutStyle)`. The user's TOML `*_visual` strings override per segment when non-empty; otherwise `effective_*_visual()` falls back to the layout default.

CTX widget composition runs through the dispatch hub `render_context_visual` in `frames/shared.rs`. Layouts call the hub with their preferred gauge sizing; the hub parses the `+`-joined spec (e.g. `"text+sparkline"`) and composes widget outputs. **New widgets must register with the hub** — never call `widgets::*::render` directly from a layout, or the user loses composability for that segment. (Ledger renders the sparkline directly because it picks the aurora fill color from CTX consumption velocity.)

Full layout × visual reference: `docs/layouts.md`.

### Output Line Format

- **L1**: `M:{model} | AG:{agent} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [↑n] [↓n] [!n +n ✘n ?n] (WT)`
- **L2**: `1 CLAUDE.md | 2 rules | 3 memories | 1 hooks | 2 MCPs | 2 skills | 1h` (value-first format; **opt-in** — `[segments.config] enabled` defaults to false, individual `show_*` toggles apply once enabled)
- **L3**: `CTX:43% (86.0k/200.0k) | TOK I:10 O:20 ↗1.5K/s C:50% | $3.50 ($3.50/h)` (default `text` form; opt in to gauge or sparkline via `context_visual`).
- **Quota**: `Q: 5h: 75% (resets 2h 0m)` (single `Q:` group prefix). Driven by CC's native `rate_limits` field.
- **L4a**: `✓ Read ×12 | ✓ Bash ×8 ✘2 | ✓ Edit ×5 +3` (completed tool counts with failure marks — stable, accumulates over session; capped by `max_completed_lines` rows, overflow folds into a ` +N` tail)
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
- **`height_degrade.rs`** — Tests `max_total_lines` + the `HeightDegradeStrategy` ladder and the `compact` layout's 1–2 row contract
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

The project uses a `ThemePalette` struct with 34 ANSI 256-color fields, resolved at runtime by `resolve_palette(theme, variant, overrides)`. See `docs/theme-palette.md` for the full specification. Key principles:

- **Built-in themes** — JSON files in `src/themes/` embedded via `include_str!()`. tokyo-night is the default; the directory is the source of truth for the full set.
- **Custom themes** — JSON files in `~/.claude/pulseline/themes/` loaded at runtime with per-process caching
- **Per-color overrides** — `[colors]` TOML section applies on top of any preset (e.g., `alert_red = 160`)
- **Emphasis tiers** (Primary/Secondary/Structural/Separator) vary by dark/light variant within each theme
- **Semantic colors** (stable_blue, alert_red, etc.) are theme-specific but consistent within a theme
- **Icon color = value color** — icons are never independently dimmed
- **Backward compat** — `theme = "dark"` and `"light"` map to tokyo-night with the appropriate variant
- Layout functions receive `&ThemePalette` via `config.palette`; legacy `pub const` values retained for test assertions
- Preview themes: `cc-pulseline --preview`