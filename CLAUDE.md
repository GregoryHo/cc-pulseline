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
```

No linter or formatter is currently configured. The project uses Rust 2021 edition with only `serde`, `serde_json`, and `tempfile` (dev) as dependencies.

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
  - `env.rs` — `EnvCollector` scans the project directory for CLAUDE.md files, rules, hooks, MCP servers (parses `.claude/mcp.json`), and skills
  - `git.rs` — `GitCollector` shells out to `git` for branch, dirty state, ahead/behind
  - `transcript.rs` — `TranscriptCollector` does incremental JSONL parsing of the Claude Code transcript file with seek-based offsets and poll throttling. This is the most complex provider — it maintains active tool/agent/todo state via `SessionState`
  - `stdin.rs` — `StdinCollector` (stub-only, currently unused beyond the trait)

- **`state/mod.rs`** — `SessionState` holds per-session mutable state: transcript file offset, active tools/agents/todo lists, and cached env/git snapshots. `PulseLineRunner` maintains a `HashMap<String, SessionState>` keyed by session+transcript+project.

- **`config.rs`** — `RenderConfig` controls rendering behavior: glyph mode, color, line caps (`max_tool_lines`, `max_agent_lines`), transcript windowing, poll throttle, terminal width, and width degradation strategy order.

- **`render/layout.rs`** — Pure rendering logic. Formats the `RenderFrame` into output lines (L1: identity, L2: config counts, L3: budget, L4+: activity). Applies `WidthDegradeStrategy` when `terminal_width` is set: drop activity lines → compress line 2 → truncate core lines.

- **`lib.rs`** — Orchestrates the pipeline: `PulseLineRunner` manages sessions, calls providers, assembles the `RenderFrame`, and delegates to the renderer. Also exposes `run_from_str()` as a stateless convenience.

### Output Line Format

- **L1**: `M:{model} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [↑n] [↓n]`
- **L2**: `C:{claude_md} | R:{rules} | H:{hooks} | M:{mcp} | SK:{skills} | ET:{min}m`
- **L3**: `CTX:{pct}%/{size} | TOK:I:{in} O:{out} C:{cache_create} R:{cache_read} | ${cost} (${rate}/h)`
- **L4+**: `T:{tool}`, `A:{agent}`, `TODO:{summary}` (only when active)

### Testing Patterns

Tests are integration-level in `tests/` and use `tempfile::TempDir` for filesystem isolation:
- **`core_metrics.rs`** — Creates a real git repo + config files in a tempdir, calls `run_from_str()`, asserts output content
- **`activity_pipeline.rs`** — Uses `PulseLineRunner` with incremental transcript appending to test tool/agent/todo lifecycle
- **`adaptive_performance.rs`** — Tests width degradation and rendering performance budgets
- **`smoke_cli.rs`** — Spawns the actual binary with `CARGO_BIN_EXE_cc-pulseline`, pipes fixture JSON via stdin

Test fixtures live in `tests/fixtures/` as `.json` (stdin payloads) and `.jsonl` (transcript streams).

### Key Design Decisions

- **Trait-based providers with stubs** — Every external data source (env, git, transcript) uses a trait so tests can substitute stubs. The real implementations are `FileSystemEnvCollector`, `LocalGitCollector`, `FileTranscriptCollector`.
- **Incremental transcript parsing** — The transcript collector seeks to the last read offset rather than re-parsing the entire file. It applies event windowing (`transcript_window_events`) and poll throttling (`transcript_poll_throttle_ms`).
- **Session-keyed state** — `PulseLineRunner` tracks multiple sessions by `session_id|transcript_path|project_path` composite key, enabling correct behavior when multiple Claude Code sessions run concurrently.

### Color System

The project uses a 256-color ANSI palette with semantic color constants. See `docs/color-spec.md` for the full specification. Key principles:

- **Emphasis tiers** (Emphasis/Muted/Subdued) replace DIM and vary by dark/light theme via `EmphasisTier`
- **Semantic colors** (MODEL_BLUE, GIT_GREEN, COST_GOLD, etc.) are fixed across themes
- **Icon color = value color** — icons are never independently dimmed
- Theme is controlled via `PULSELINE_THEME=light` env var (default: dark)
- Color constants live in `render/color.rs`; theme logic uses `emphasis_for_theme()`
