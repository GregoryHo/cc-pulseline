# Event coverage: what the mod path replaces

`src/providers/transcript.rs` (1,336 lines) exists because the statusline
hook hands the binary a payload and a file path, and everything else —
which tool is running, which agents are alive, where the todo list stands —
has to be reconstructed by tailing Claude Code's transcript JSONL.

A function-hooks mod is handed those facts as events. This file checks that
claim function by function, so the question "could the transcript parser go
away?" has an answer built on the real event contract rather than on hope.

Verified against the `claude-code.d.ts` the engine itself writes for plugin
authors (written by Claude Code 2.1.296, checked on 2.1.299). There is no
command to run: the engine writes it beside the mod, at
`mod/.claude-plugin/types/claude-code/index.d.ts`, each time it loads the mod
from a folder the author owns. Also against a running spike,
`mod/hooks/register.ts`.

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
| Token counts per API call, deduped by `message.id` | 486–516 | `turn.complete` gives the four counts per **turn**, so the cache trend samples once per turn — and tok/s drops out altogether: `update_output_speed` in `src/state/mod.rs` only computes a rate when two samples land `delta_ms <= 2000` apart, while per-turn counts arrive minutes apart and non-monotonic, so it always returns `None`. Restoring it needs per-step token events, which is deferred |
| API error badge | 446–469 | `turn.complete` carries `reason: 'error'` (`TurnCompleteReason` is `'answer' \| 'aborted' \| 'refusal' \| 'error'`), so a turn-level error signal does exist — but at turn granularity. The per-message `isApiErrorMessage` lines the transcript carries have no event equivalent; the contract does not mention the flag |
| Agent completion time | 926–970 | `classic.SubagentStart` / `classic.SubagentStop` give real timings; `$.agent.list()` still supplies the list |
| `aggregate_todo` — fold several sub-agents' todo lists into one row | 1007–1054 | each `TodoWrite` is seen, but the fold has to be rewritten in the mod; not done yet |

## Not covered by any event

No *fact* any more. Both entries this section used to hold moved up: output
style is carried outright, and the API error badge arrives at turn
granularity.

What is still missing is *resolution*, not facts, and both cases are in the
table above: per-API-call token counts (the mod sees one total per turn) and
per-message API errors (the mod sees one flag per turn). Neither has an
event equivalent today.

## What first looked uncovered

Three facts that first looked uncovered are not: **effort** rides on every
classic hook input (`BaseHookInput.effort`), the **compaction count** is
`classic.PreCompact`, and the **output style** rides on `prompt.compose`'s
input, which the engine resolves as
`outputStyle: { name, isKeepingCodingInstructions } | null` at the moment it
renders a system prompt. The exception is `classic.PreToolUse`, whose
argument `ClassicEventOf` special-cases to the tool call envelope, so the
effort is read from `classic.PostToolUse` instead.

## What this adds up to

Everything the parser reconstructs about **tools, agents and todos** is
carried by an event, usually more directly: a tool's arguments instead of a
per-tool key table, one closure instead of id matching, a boolean instead of
six spellings of "done". The machinery for reading a file that was never an
event stream — offsets, truncation, windowing, the metadata pre-filter, the
replay guard — has nothing to do on this path.

What does **not** survive the move is per-tick token resolution — one item,
against roughly 1,300 lines of reconstruction.

That was never the deciding question, though. A mod cannot draw where the
statusline draws: `ui.render`'s `RenderComponent` enumerates fifteen places
a mod may draw — `AbovePrompt` and `Pane` among them — and the statusline's
row is not one of them. So the two paths divide the work instead of
replacing each other. The **statusline** keeps drawing the steady-state
summary, which is why it keeps parsing the transcript — it is the only way
to put anything at that position. The **band** (`AbovePrompt`, assembled
from engine events) draws the activity detail, where per-event resolution is
the point. Their output is not required to match, and neither is a degraded
copy of the other.

So `src/providers/transcript.rs` stays, by division of labour rather than as
a decision deferred. Nor is anything waiting on a flag any more:
`claude plugin validate mod` passes on 2.1.299 with no
`CLAUDE_CODE_ENABLE_FUNCTION_HOOKS` set.
