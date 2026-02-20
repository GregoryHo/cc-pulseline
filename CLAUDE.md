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

The project uses Rust 2021 edition (MSRV 1.74) with `serde`, `serde_json`, `toml` as dependencies, and `tempfile`, `criterion` as dev-dependencies.

### CLI Flags

```bash
cc-pulseline --init           # Create user config (~/.claude/pulseline/config.toml)
cc-pulseline --init --project # Create project config (.claude/pulseline.toml)
cc-pulseline --check          # Validate config files
cc-pulseline --print          # Show effective merged config
cc-pulseline --fetch-quota    # Internal: background subprocess that fetches usage quota
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
  - `quota.rs` — `QuotaCollector` reads a quota cache file written by the background fetch subprocess. `CachedFileQuotaCollector` is the real implementation; `StubQuotaCollector` for tests.
  - `quota_fetch.rs` — Entry point for the `--fetch-quota` background subprocess. Reads OAuth credentials (macOS Keychain or file fallback), calls the Anthropic usage API, and writes the quota cache file.

- **`state/mod.rs`** — `SessionState` holds per-session mutable state: transcript file offset, active tools/agents/todo lists, recent tools (persist after completion for display), and cached env/git snapshots. `PulseLineRunner` maintains a `HashMap<String, SessionState>` keyed by session+transcript+project.
  - `state/cache.rs` — Persists `SessionState` to `{temp_dir}/cc-pulseline-{hash}.json` across process invocations (prevents L3 metric flicker). Uses atomic writes (.tmp + rename) with silent failure on errors.

- **`config.rs`** — `RenderConfig` controls rendering behavior: glyph mode, color, line caps (`max_tool_lines`, `max_agent_lines`), transcript windowing, poll throttle, terminal width, width degradation strategy order, and segment toggles (`show_git_stats`, `show_speed`, `show_quota`, `show_quota_five_hour`, `show_quota_seven_day`).

- **`render/`** — Pure rendering logic, split into submodules:
  - `layout.rs` — Formats the `RenderFrame` into output lines (L1: identity, L2: config counts, L3: budget, L4+: activity). Applies `WidthDegradeStrategy` when `terminal_width` is set: drop activity lines → compress line 2 → truncate core lines.
  - `color.rs` — 256-color ANSI palette with semantic color constants and `EmphasisTier` theme logic
  - `fmt.rs` — Number formatting (`format_number`), duration formatting (`format_duration`), speed formatting (`format_speed`), reset duration formatting (`format_reset_duration`), and agent/todo elapsed formatting (`format_agent_elapsed`)
  - `icons.rs` — Nerd Font icon constants and `glyph()` helper for icon/ascii mode switching

- **`lib.rs`** — Orchestrates the pipeline: `PulseLineRunner` manages sessions, calls providers, assembles the `RenderFrame`, and delegates to the renderer. Also exposes `run_from_str()` as a stateless convenience.

### Output Line Format

- **L1**: `M:{model} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [↑n] [↓n] [!n +n ✘n ?n]`
- **L2**: `1 CLAUDE.md | 2 rules | 3 memories | 1 hooks | 2 MCPs | 2 skills | 1h` (value-first format, all togglable)
- **L3**: `CTX:43% (86.0k/200.0k) | TOK I:10 O:20 ↗1.5K/s C:30/40 | $3.50 ($3.50/h)`
- **Quota**: `Q:Pro 5h: 75% (resets 2h 0m)` (usage quota, between L3 and activity)
- **L4a**: `✓ Read ×12 | ✓ Bash ×8 | ✓ Edit ×5` (completed tool counts — stable, accumulates over session)
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

Test fixtures live in `tests/fixtures/` as `.json` (stdin payloads) and `.jsonl` (transcript streams).

### Key Design Decisions

- **Trait-based providers with stubs** — Every external data source (env, git, transcript) uses a trait so tests can substitute stubs. The real implementations are `FileSystemEnvCollector`, `LocalGitCollector`, `FileTranscriptCollector`.
- **Incremental transcript parsing** — The transcript collector seeks to the last read offset rather than re-parsing the entire file. It applies event windowing (`transcript_window_events`) and poll throttling (`transcript_poll_throttle_ms`).
- **Session-keyed state** — `PulseLineRunner` tracks multiple sessions by `session_id|transcript_path|project_path` composite key, enabling correct behavior when multiple Claude Code sessions run concurrently.

### Color System

The project uses a 256-color ANSI palette with semantic color constants. See `docs/theme-palette.md` for the full specification. Key principles:

- **Emphasis tiers** (Primary/Secondary/Structural/Separator) vary by dark/light theme via `EmphasisTier`
- **Semantic colors** (STABLE_BLUE, GIT_GREEN, etc.) are fixed across themes
- **Icon color = value color** — icons are never independently dimmed
- **L1 hierarchy**: model/git use semantic colors; style/version/project use tier.secondary (promoted from structural)
- Theme is controlled via `PULSELINE_THEME=light` env var (default: dark)
- Color constants live in `render/color.rs`; theme logic uses `emphasis_for_theme()`