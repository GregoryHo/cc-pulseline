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
    |         +--------------+---------------+                |
    |                        v                                |
    |         +------------------------------+                |
    |         |       RenderFrame            |                |
    |         |  Line1Metrics (identity)     |                |
    |         |  Line2Metrics (config)       |                |
    |         |  Line3Metrics (budget)       |                |
    |         |  Vec<ToolSummary> (active)   |                |
    |         |  Vec<AgentSummary> (active)  |                |
    |         |  Vec<TodoSummary>            |                |
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
- **Activity summaries** (`ToolSummary`, `AgentSummary`, `TodoSummary`) -- live session state
- **`RenderFrame::from_payload()`** -- Initial field extraction from the raw payload

### `providers/` -- Trait-Based Collectors

Each provider has a real implementation and a `Stub*` variant for testing:

| Provider | Trait | Real Implementation | Purpose |
|----------|-------|-------------------|---------|
| `env.rs` | `EnvCollector` | `FileSystemEnvCollector` | Scans for CLAUDE.md files, rules, hooks, MCP servers, skills |
| `git.rs` | `GitCollector` | `LocalGitCollector` | Shells out to `git` for branch, dirty state, ahead/behind |
| `transcript.rs` | `TranscriptCollector` | `FileTranscriptCollector` | Incremental JSONL parsing with seek-based offsets |
| `stdin.rs` | `StdinCollector` | (stub-only) | Reserved for future use |

### `state/mod.rs` -- Session State

`SessionState` holds per-session mutable state:

- Transcript file offset (for incremental parsing)
- Active tools, agents, and todo lists
- Completed tool/agent counts
- Cached env/git snapshots (with TTL)
- Cached L3 metrics (for flicker prevention)

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
- Line caps (`max_tool_lines`, `max_agent_lines`)
- Transcript windowing and poll throttle
- Terminal width and width degradation strategy order
- Segment toggles for each line

Config files: `~/.claude/pulseline/config.toml` (user) and `{project}/.claude/pulseline.toml` (project override).

### `render/layout.rs` -- Pure Rendering

Formats the `RenderFrame` into output lines:

- **L1**: Identity (model, style, version, project, git)
- **L2**: Config counts (CLAUDE.md, rules, hooks, MCPs, skills, duration)
- **L3**: Budget (context, tokens, cost)
- **L4+**: Activity (tools, agents, todos -- only when active)

Applies `WidthDegradeStrategy` when `terminal_width` is set:
1. Drop activity lines
2. Compress L2 separators
3. Truncate core lines

## Transcript Three-Path Dispatcher

The transcript collector uses a three-path dispatcher to handle different JSONL formats from Claude Code:

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
    |         |               |
    |   type == "tool_use"?   +--yes--> Path 3: Flat Fallback
    |         |               |         * old-style tool lifecycle
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

### Path 3: Flat Format Fallback

Backward compatibility with older transcript formats and test fixtures:

- `{type: "tool_use", name, tool_use_id}` -- Simple tool lifecycle without nested content

## Session State Lifecycle

1. **First invocation**: `PulseLineRunner` creates a new `SessionState`, attempts to load cached state from disk
2. **Subsequent invocations**: Runner looks up existing state by composite session key
3. **Provider collection**: Env and git data are refreshed only after TTL expiry (10 seconds)
4. **Transcript parsing**: Seeks to last offset, parses new lines only, applies event windowing
5. **Frame assembly**: Providers + state produce a `RenderFrame`
6. **L3 merge**: Current L3 fields win; if all-NA, falls back to cached L3
7. **Cache save**: State is persisted to disk atomically after each render cycle

## Output Line Format

- **L1**: `M:{model} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] [up-n] [down-n]`
- **L2**: `1 CLAUDE.md | 2 rules | 1 hooks | 2 MCPs | 2 skills | 1h`
- **L3**: `CTX:43% (86.0k/200.0k) | TOK:I:10 O:20 C:30 R:40 | $3.50 ($3.50/h)`
- **L4**: `T:Read: .../main.rs | T:Bash: cargo test | checkmark-Read x5`
- **L5+**: `A:Explore [haiku]: Investigate logic (2m)`

All segments are individually togglable via config. Each line has an independent set of toggle flags.
