# Architecture

## Pipeline Overview

```
                         cc-pulseline Pipeline
    +---------------------------------------------------------+
    |                                                         |
    |  stdin JSON --> StdinPayload (serde deserialize)        |
    |                      |                                  |
    |              +-------+-------+                          |
    |              v       v       v                          |
    |         +--------+ +-----+ +--------------+            |
    |         |  Env   | | Git | |  Transcript   |            |
    |         |Collect.| |Coll.| |  Collector    |            |
    |         +---+----+ +--+--+ +------+-------+            |
    |             |         |           |                     |
    |             v         v           v                     |
    |         +------------------------------+                |
    |         |       SessionState           |                |
    |         |  (keyed: sid|transcript|proj) |                |
    |         |  * env/git cache (10s TTL)   |                |
    |         |  * transcript offset         |                |
    |         |  * active tools/agents/todo  |                |
    |         |  * completed counts          |                |
    |         |  * cached L3 metrics         |                |
    |         |  * Output speed tracking     |                |
    |         +--------------+---------------+                |
    |                        v                                |
    |         +------------------------------+                |
    |         |       RenderFrame            |                |
    |         |  Line1Metrics (identity)     |                |
    |         |  Line2Metrics (config)       |                |
    |         |  Line3Metrics (budget+speed) |                |
    |         |  QuotaMetrics (usage quota)  |                |
    |         |  Vec<ToolSummary> (active)   |                |
    |         |  Vec<AgentSummary> (active)  |                |
    |         |  Vec<TodoSummary> (enriched)  |                |
    |         +--------------+---------------+                |
    |                        v                                |
    |         +------------------------------+                |
    |         |   render::layout             |                |
    |         |   render_frame() -> Vec<Str> |                |
    |         |                              |                |
    |         |   Width Degradation:         |                |
    |         |   1. Drop activity lines     |                |
    |         |   2. Compress L2 separators  |                |
    |         |   3. Truncate core lines     |                |
    |         +--------------+---------------+                |
    |                        v                                |
    |                    stdout                               |
    +---------------------------------------------------------+

    +---------------------------------------------------------+
    |              Session Cache (disk)                       |
    |  {tmp}/cc-pulseline-{hash}.json                        |
    |  * Atomic: write .tmp -> rename                        |
    |  * Silent: all errors ignored                          |
    |  * Purpose: prevent L3 NA flicker across invocations   |
    +---------------------------------------------------------+
```

## Module Responsibilities

### `types.rs` -- Data Structures

All data structures live here:

- **`StdinPayload`** -- Input deserialization from Claude Code's statusline JSON
- **`RenderFrame`** and its line metrics (`Line1Metrics`, `Line2Metrics`, `Line3Metrics`) -- structured output data
- **Activity summaries** (`ToolSummary`, `AgentSummary`, `TodoSummary`, `TodoInProgressItem`) -- live session state
- **`RenderFrame::from_payload()`** -- Initial field extraction from the raw payload

### `providers/` -- Trait-Based Collectors

Each provider has a real implementation and a `Stub*` variant for testing:

| Provider | Trait | Real Implementation | Purpose |
|----------|-------|-------------------|---------|
| `env.rs` | `EnvCollector` | `FileSystemEnvCollector` | Scans for CLAUDE.md files, rules, memories, hooks, MCP servers, skills, plugins |
| `git.rs` | `GitCollector` | `LocalGitCollector` | Shells out to `git` for branch, dirty state, ahead/behind, file stats |
| `transcript.rs` | `TranscriptCollector` | `FileTranscriptCollector` | Incremental JSONL parsing with seek-based offsets |

### `state/mod.rs` -- Session State

`SessionState` holds per-session mutable state:

- Transcript file offset (for incremental parsing)
- Active tools, recent tools (persist after completion for display), agents, and todo lists
- Completed tool counts with last-used timestamps (hybrid scoring: count + recency bonus)
- Completed agents (FIFO buffer, max 10)
- Cached env/git snapshots (with TTL)
- Cached L3 metrics (for flicker prevention)
- Output speed tracking (delta-based tok/s, holds last known)

`PulseLineRunner` maintains a `HashMap<String, SessionState>` keyed by `session_id|transcript_path|project_path`, enabling correct behavior when multiple Claude Code sessions run concurrently.

### `state/cache.rs` -- Disk Persistence

Persists `SessionState` across process invocations:

- **File**: `{temp_dir}/cc-pulseline-{hash}.json` (hash of session key via `DefaultHasher`)
- **Atomic writes**: write to `.tmp` then rename
- **Silent failures**: all load/save errors are ignored (never crashes the statusline)
- **Loaded on fresh**: only when a session key is first encountered

### `config.rs` -- Configuration

`RenderConfig` controls rendering behavior:

- Glyph mode (Nerd Font icons vs ASCII)
- Color enable/disable
- Line caps (`max_tool_lines`, `max_agent_lines`, `max_completed_lines`)
- Transcript windowing and poll throttle
- Terminal width and width degradation strategy order
- Segment toggles for each line
- Per-segment visual specs for context, quota, agents, tools, and todo — see `docs/layouts.md` for the full set

Config files: `~/.claude/pulseline/config.toml` (user) and `{project}/.claude/pulseline.toml` (project override).

### `render/layout.rs` -- Pure Rendering

Formats the `RenderFrame` into output lines:

- **L1**: Identity (model, style, version, project, git + file stats)
- **L2**: Config counts (CLAUDE.md, rules, memories, hooks, MCPs, skills, plugins, duration) — opt-in via `[segments.config] enabled` (default off)
- **L3**: Budget (context, tokens, cost, speed)
- **Quota**: Usage quota (5-hour and 7-day periods, between L3 and activity)
- **L4a**: Completed tool counts (stable, accumulates over session)
- **L4b**: Running/recent tools with targets (volatile)
- **L5+**: Activity (agents, todos -- only when active)

Single rendering pipeline: every layout flows through this assembly and is decorated by `apply_pane()`. The exceptions are the layouts that own their full pipeline (Ledger's TAG-column rhythm and the Rail/Anchor seam rhythm don't compose via `apply_pane`) — see the early-return arms in `render_frame()`.

Applies `WidthDegradeStrategy` when `terminal_width` is set:
1. Drop activity lines
2. Compress L2 separators
3. Truncate core lines

### `render/pane.rs` + `render/frames/` -- Layouts

`pane.rs::LayoutStyle` enumerates the layouts (the enum is the source of truth). `apply_pane()` decorates the flat-pipeline output with frame chrome (console = single outer frame + identity-in-title; `None` / `Compact` / `Budgets` pass through undecorated). `Ledger` owns its full pipeline because its TAG-column rhythm doesn't compose via `apply_pane`. `frames/` holds one file per pipeline-owning layout (`console.rs`, `ledger.rs`, `rail.rs`, `anchor.rs`; `powerline.rs` is rail's seam helper) plus `shared.rs`, which carries the box-drawing glyph tables, identity headline, config row, and the per-segment dispatch hubs `render_context_visual` and `render_quota_visual` (each maps a `+`-joined visual spec like `"text+gauge+sparkline"` onto the relevant atomic widgets).

### `render/widgets/` -- Atomic Widgets

`gauge` (bracketless marks-on-track — `▰▰▰▰▰▰···──·──` in Icon mode with `·` threshold marks on the empty portion, `======:::--:--` in Ascii), `sparkline` (braille, icon-only), `plot` (normalized braille line plot, icon-only), and `effort` (ordinal pip-ramp). Signatures differ per widget — see the doc comments in `widgets/*.rs`. Ascii-incompatible widgets return an empty string under `GlyphMode::Ascii` so dispatch hubs drop the empty cell cleanly without leaking width. The `gauge` widget's `width` is the visible cell count (no frame); the caller supplies threshold marks, fill colour, and pct.

See [`docs/layouts.md`](layouts.md) for the layout × visual reference and the per-layout default-visuals table.

## Transcript Two-Path Dispatcher

Before JSON parsing, a byte-level pre-filter (`is_ignored_metadata_line`) skips metadata lines (`attachment`/bookkeeping payloads) — these are disproportionately the largest lines, and skipping them roughly halves parse cost on busy sessions. Surviving lines flow through a two-path dispatcher (`apply_transcript_event`):

```
    Transcript Line Dispatcher
    +-------------------------+
    |   JSON line parsed      |
    |                         |
    |   Has message.content[]?+--yes--> Path 1: Nested Content
    |         |               |         * tool_use -> upsert_tool(target)
    |         no              |         * tool_result -> remove + count
    |         |               |
    |   type == "progress"?   +--yes--> Path 2: Agent Progress
    |         |               |         * agent_progress -> upsert/remove
    |         no              |
    |         v               |
    |      (skip line)        |
    +-------------------------+
```

### Path 1: Nested Content Blocks

The primary format used by real Claude Code transcripts. Each JSON line contains a `message.content[]` array with typed blocks:

- `{type: "tool_use", id, name, input}` -- Upsert a tool with target extraction
- `{type: "tool_result", tool_use_id}` -- Remove the tool and record completion count

### Path 2: Progress Events

Agent lifecycle events arrive as progress-type messages:

- `{type: "progress", data: {type: "agent_progress", agentId, status, prompt, agentType}}`
- Status transitions: `started` -> upsert agent, `completed` -> remove and record

## Session State Lifecycle

1. **First invocation**: `PulseLineRunner` creates a new `SessionState`, attempts to load cached state from disk
2. **Subsequent invocations**: Runner looks up existing state by composite session key
3. **Provider collection**: Env and git data are refreshed only after TTL expiry (10 seconds)
4. **Transcript parsing**: Seeks to last offset, parses new lines only, applies event windowing
5. **Frame assembly**: Providers + state produce a `RenderFrame`
6. **L3 merge**: Current L3 fields win; if all-NA, falls back to cached L3
7. **Cache save**: State is persisted to disk atomically after each render cycle

## Output Line Format

- **L1**: `M:{model} | AG:{agent} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [↑n] [↓n] [!n +n ✘n ?n] (WT)`
- **L2**: `1 CLAUDE.md | 2 rules | 3 memories | 1 hooks | 2 MCPs | 2 skills | 1 plugins | 1h` (opt-in — `[segments.config] enabled`, default off)
- **L3**: `CTX:43% (86.0k/200.0k) | TOK I:10.0k O:20.0k ↗1.5K/s C:50% | $3.50 ($3.50/h)`
- **Quota**: `Q: 5h: 75% (resets 2h 0m)`
- **L4a**: `✓ Read ×12 | ✓ Bash ×8 | ✓ Edit ×5` (completed counts, capped by `max_completed_lines` rows)
- **L4b**: `T:Read: .../main.rs | T:Bash: cargo test` (recent/running tools)
- **L5+**: `A:Explore [haiku]: Investigate logic (2m)`

All segments are individually togglable via config. Each line has an independent set of toggle flags.
