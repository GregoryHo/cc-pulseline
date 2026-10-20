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

- Glyph mode (`GlyphMode`: Icon / Unicode / Ascii; see the glyph tiers in `.claude/rules/plugin-mod.md`)
- Color enable/disable
- Line caps (`max_tool_lines`, `max_agent_lines`, `max_completed_lines`)
- Transcript windowing and poll throttle
- Terminal width and width degradation strategy order
- Segment toggles for each line
- Per-segment visual specs — see `SegmentVisualDefaults` in `src/render/frames/mod.rs` and `docs/layouts.md` for the full set

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

Single rendering pipeline for the flat layouts (`None` / `Compact` / `Budgets` / `Console`); `apply_pane()` decorates only `Console`, and `Compact` short-circuits to its own assembly. The pipeline-owning layouts early-return in `render_frame()`.

Applies `WidthDegradeStrategy` when `terminal_width` is set:
1. Drop activity lines
2. Compress L2 separators
3. Truncate core lines

### `render/pane.rs` + `render/frames/` -- Layouts

`pane.rs::LayoutStyle` enumerates the layouts (the enum is the source of truth). `apply_pane()` decorates the flat-pipeline output with frame chrome — only `Console` is actually decorated; `None` / `Compact` / `Budgets` pass through undecorated. The pipeline-owning frames early-return in `render_frame()`'s match and never reach `apply_pane`. `frames/` holds one file per such layout (see `src/render/frames/`, where `powerline.rs` is rail's seam helper) plus `shared.rs`, which carries the box-drawing glyph tables, identity headline, config row, and the per-segment dispatch hubs (grep `fn render_.*_visual` for the current set; each maps a `+`-joined visual spec like `"text+gauge+sparkline"` onto the relevant atomic widgets).

### `render/widgets/` -- Atomic Widgets

`gauge` (bracketless marks-on-track — `▰▰▰▰▰▰···──·──` in the Unicode/Icon tiers with `·` threshold marks on the empty portion, `======:::--:--` in Ascii), `sparkline` (braille, Unicode tier: empty under `GlyphMode::Ascii`), `plot` (normalized braille line plot, Unicode tier: empty under `GlyphMode::Ascii`), and `effort` (ordinal pip-ramp). Signatures differ per widget — see the doc comments in `widgets/*.rs`. Ascii-incompatible widgets return an empty string under `GlyphMode::Ascii` so dispatch hubs drop the empty cell cleanly without leaking width. The `gauge` widget's `width` is the visible cell count (no frame); the caller supplies threshold marks, fill colour, and pct.

See [`docs/layouts.md`](layouts.md) for the layout × visual reference and the per-layout default-visuals table.

## Activity Width Budget

`render/activity/` renders the activity rows (L4+) so that every row knows its character budget. Fixed per-tool truncation lengths and a fixed tools-per-row count can't do that: a wide terminal would still cut descriptions at 40 characters, and a narrow one would overflow. The code is the source of truth; this section records why it is shaped the way it is.

### Cell (`cell.rs`)

A row is a `Vec<Cell>`. Each cell has:

- `head`: the unbreakable prefix (icon, label, punctuation), always drawn as is.
- `body`: optional variable-width content, with a `TruncationStrategy`, a `min_width` (the shortest form that still means something) and an `ideal_width` (the full content). The allocator never gives a body more than `ideal_width`.
- `tail`: `Pinned` fragments survive whenever the cell is drawn (an agent's elapsed time); `Slack` fragments are the first thing dropped under pressure (a model tag like `[haiku]`).
- `priority`: `Optional` cells are dropped whole when the row overflows; `Required` cells keep at least their `head`.

Truncation works on plain text and colour is applied afterwards, so a cut never slices an ANSI escape.

### Truncation strategies (`truncate.rs`)

Each strategy is a free function `fn(&str, max_chars) -> String`, picked by content type rather than by a per-tool length:

| Strategy | For | Shape |
|---|---|---|
| `KeepHead` | short verb-led labels (skill names, glob patterns) | first N chars + `…` |
| `KeepTail` | file paths | `.../leaf` when the leaf fits |
| `KeepMiddle` | both ends matter (a URL's host and leaf) | `prefix…suffix` |
| `Sentence` | descriptions, prompts, questions | cut at the last word boundary, never mid-word |
| `CommandSmart` | shell commands | drop the verb and its flags, keep the first quoted payload or path argument, then `Sentence`; falls back to `KeepHead` |

`CommandSmart` exists because the useful part of a shell command is rarely its start: `git commit -m "feat: add foo"` shows as `feat: add foo`, and `sed 's/x/y/' file.txt` as `s/x/y/  file.txt`.

### Row allocator (`budget.rs`)

`pack` fits cells into one row of `available` visible characters:

1. **Survival.** Sum every cell's minimum (head, body `min_width`, pinned tails) plus separators. While that overflows, drop the rightmost `Optional` cell. With none left to drop, accept the overflow.
2. **Slack.** Hand the remaining width to bodies, left to right, `Required` cells before `Optional` ones, each up to its `ideal_width`.
3. **Slack tails.** A cell draws its `Slack` tails only if its body got its full `ideal_width`, that is, only when the row had room to spare.

`pack_multi_row` chunks cells by what fits rather than by a fixed count, so the completed-tool counts take one row when they fit and wrap only when they don't. It returns how many cells it placed, so the caller can add a `+N` overflow tail.

### Agent groups (`agent_groups.rs`)

Agents spawned by one assistant turn share the Anthropic API `message.id`, which the transcript collector stores on each `AgentSummary`. `classify` groups by it:

| Group | Rule | Drawn as |
|---|---|---|
| `Single` | alone in its `message.id`, or no `message.id` | one row per agent |
| `Homogeneous` | ≥2 agents, same `message.id`, same `agent_type` | `🤖 type ×N parallel (avg …): first description + N-1 more` |
| `Heterogeneous` | ≥2 agents, same `message.id`, mixed types | `‖ ×N parallel (avg …): type1: desc1 + type2: desc2 + …` (ASCII `\|\|`) |

`message.id`, not a time window: the JSONL `timestamp` is when Claude Code wrote each block, so agents from one turn can be 20 seconds apart, and a window would miss real batches. Agents from different turns are never folded together, because that would make history look like a parallel fan-out. A missing `message.id` (an older Claude Code, or a cache from an older binary) falls back to one row per agent.

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

- **L1**: `M:{model} | E:{effort} | [T] | AG:{agent} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [↑n] [↓n] [!n +n ✘n ?n] (WT)`
- **L2**: `1 CLAUDE.md | 2 rules | 3 memories | 1 hooks | 2 MCPs | 2 skills | 1 plugins | 1h` (opt-in — `[segments.config] enabled`, default off)
- **L3**: `CTX:43% (86.0k/200.0k) | TOK I:10.0k O:20.0k ↗1.5K/s C:50% | $3.50 ($3.50/h)`
- **Quota**: `Q: 5h: 75% (resets 2h 0m)`
- **L4a**: `✓ Read ×12 | ✓ Bash ×8 | ✓ Edit ×5` (completed counts, capped by `max_completed_lines` rows)
- **L4b**: `T:Read: .../main.rs | T:Bash: cargo test` (recent/running tools)
- **L5+**: `A:Explore [haiku]: Investigate logic (2m)`

All segments are individually togglable via config. Each line has an independent set of toggle flags.
