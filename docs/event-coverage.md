# Event coverage: what the mod path replaces

`src/providers/transcript.rs` (1,336 lines) exists because the statusline
hook hands the binary a payload and a file path, and everything else —
which tool is running, which agents are alive, where the todo list stands —
has to be reconstructed by tailing Claude Code's transcript JSONL.

A function-hooks mod is handed those facts as events. This file checks that
claim function by function, so the question "could the transcript parser go
away?" has an answer built on the real event contract rather than on hope.

Verified against `mods/types/claude-code.d.ts` (Claude Code 2.1.277) and a
running spike, `mod/hooks/register.ts`.

## Covered: the event carries the fact outright

| What the parser does | Lines | The event that carries it |
| --- | --- | --- |
| `apply_content_block` — read `tool_use` blocks to learn a tool started | 554–679 | `tool.call`, before `next(e)` |
| `complete_tool_result` — match a `tool_result` back to its `tool_use` | 801–874 | the same hook's return: one closure holds both ends, no id matching |
| `extract_target` — a per-tool table of which input key to show | 875–925 | the tool's own arguments ride on `e` (`e.command`, `e.file_path`) |
| `is_terminal_status` — accept six spellings of "finished" | 971–985 | `result.isError` is a boolean |
| `dispatch_todo_write` / `extract_todo_summary` / `find_todos_array` — three todo schemas, probed by shape | 795–800, 1196–1241 | `tool.call` where `e.tool === 'TodoWrite'`: the array as the tool received it |
| `handle_agent_progress` — infer agent state from progress events | 680–766 | `$.agent.list()` returns id, description, type, status, parent |
| `sub_agent_transcript_path` + `tail_sub_agent_transcripts` + `apply_sub_agent_event` — open and tail each sub-agent's own JSONL | 1055–1195 | `tool.call` carries `agentId`; a sub-agent's calls arrive in the same stream |
| `accumulate_usage` + the replay/dedupe guards | 486–516 | `turn.complete` raises `e.usage` once per turn |
| `is_ignored_metadata_line` — a byte-level pre-filter for lines that are not events | 274–321 | there is no file to filter |
| `read_new_lines`, offset seeking, truncation detection | 113–210, 322–356 | there is no file to seek |
| `should_throttle` — poll throttling | 248–273 | events arrive when they happen |

## Partly covered: the fact arrives, at lower resolution

| What the parser does | Lines | On the event path |
| --- | --- | --- |
| Token counts per API call, deduped by `message.id` | 486–516 | `turn.complete` gives the four counts per **turn**, so tok/s becomes a turn average and the cache trend samples once per turn |
| Agent completion time | 926–970 | `classic.SubagentStart` / `classic.SubagentStop` give real timings; `$.agent.list()` still supplies the list |
| `aggregate_todo` — fold several sub-agents' todo lists into one row | 1007–1054 | each `TodoWrite` is seen, but the fold has to be rewritten in the mod; not done yet |

## Not covered by any event

| Fact | Where it comes from today |
| --- | --- |
| Output style | the statusline payload only; no call on `$` exposes it |
| API error badge | `isApiErrorMessage` lines in the transcript; `turn.complete`'s `reason: 'error'` is close but not the same signal |

Two facts that first looked uncovered are not: **effort** rides on every
classic hook input (`BaseHookInput.effort`), and the **compaction count**
is `classic.PreCompact`. The exception is `classic.PreToolUse`, whose
argument `ClassicEventOf` special-cases to the tool call envelope, so the
effort is read from `classic.PostToolUse` instead.

## What this adds up to

Everything the parser reconstructs about **tools, agents and todos** is
carried by an event, usually more directly: a tool's arguments instead of a
per-tool key table, one closure instead of id matching, a boolean instead of
six spellings of "done". The machinery for reading a file that was never an
event stream — offsets, truncation, windowing, the metadata pre-filter, the
replay guard — has nothing to do on this path.

What does **not** survive the move is a handful of session-level facts
(output style, effort, compaction count) and per-tick token resolution.

So the honest reading is: the transcript parser is not obsolete, it is
*unnecessary on one path*. Deleting it stays a decision for the day the
classic statusline is retired — and that day depends on
`CLAUDE_CODE_ENABLE_FUNCTION_HOOKS` shipping on by default, which is not
this author's call.
