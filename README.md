# cc-pulseline

High-performance Claude Code statusline focused on observability.

## Implemented Scope

- L1 core identity: `model | style | Claude Code version | project path | git status`
- L2 local config metrics: `CLAUDE.md | rules | hooks | MCP servers | skills | elapsed time`
- L3 budget metrics: `context window | token usage (in/out/cache) | total + hourly cost`
- L4+ activity: tool, agent, and todo lines rendered only when currently active

## Activity Pipeline

- Incremental JSONL transcript parsing with per-session offsets
- Windowed event processing (`transcript_window_events`)
- Poll throttling (`transcript_poll_throttle_ms`)
- Line caps (`max_tool_lines`, `max_agent_lines`)

## Adaptive Rendering

- Width degradation strategies:
  - drop activity lines first
  - compress line 2
  - truncate/compress core lines
- Optional width target via `terminal_width`

## Runtime Contract

- Input: Claude Code statusline JSON on `stdin`
- Output: multiline statusline text on `stdout`

## Validation

- `cargo check`
- `cargo test`

Both pass with fixture-driven core-metrics tests, transcript activity flow tests, narrow/wide rendering tests, and performance budget regression tests.
