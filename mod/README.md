# pulseline-mod

An experimental Claude Code **mod** — a plugin whose behaviour lives in a
hooks module — that draws cc-pulseline from engine events instead of the
statusline hook.

Gated, undocumented, and written against an API its own README warns "may
change between releases without notice". Treat it as a spike, not a
product: the classic statusline path is unchanged and stays the default.

## Run it

```bash
CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 claude --plugin-dir mod
```

It needs a release build of the binary at the path `BINARY` names in
`hooks/register.ts`, an interactive session (function hooks do not load
under `-p`), and the flag: it defaults to off.

`/pulseline` opens a pane; `/pulseline` again closes it.

## What it draws

| Site | Rows | Layout |
| --- | --- | --- |
| `AbovePrompt` band | 1–3, whatever `maxRows` gives | the config's own (`rail` here) |
| `Pane` | the pane's height | switchable: ledger / budgets / compact / rail |

Both sites run the binary themselves, because one row count cannot serve a
1-row band and a 14-row pane. The pane run only happens while the pane is
open.

## How it gets its data

| Row | Source |
| --- | --- |
| Running tools, per-tool counts, failures | `tool.call`: marked before `next(e)`, counted after, `result.isError` is the failure flag |
| Tool argument (`cargo test`, a file path) | the tool's own arguments on `e` — a lookup, not a guess |
| Todo | the `TodoWrite` call's `todos` array |
| Agents | `$.agent.list()` |
| Context, cost, rate limits | `$.session.usage()` |
| Token breakdown | `turn.complete`'s `e.usage` (per turn, not per tick) |
| Claude Code's version | `claude --version`, probed once |

None of it is read from the transcript: on this path the binary's
`FileTranscriptCollector` never runs.

## What the first runs established

- A user-tier hooks module loads, and `$.process.run` with `stdin` is
  allowed there — so the Rust binary stays the renderer and nothing is
  reimplemented in TypeScript.
- The band **refuses a whole tree** if any text child holds a control
  character, so the binary's ANSI is parsed into `Text` props (`spansOf`)
  rather than passed through.
- `$` may only be passed to a function declared at the top of the module.
  A closure inside `register` is refused, by name, at load.
- `$.env` is Claude Code's own environment, not the one it hands to the
  children it spawns: `CLAUDE_CODE_EXECPATH` and `CLAUDE_EFFORT` are not
  on it.
- `Raster` (the cell grid that would reproduce the palette exactly) takes
  only width-1 BMP code points, and 11 of the 31 icons in
  `src/render/icons.rs` are plane-15 Nerd Font glyphs.

## Known gaps against the statusline path

| Missing | Why |
| --- | --- |
| Output style | no call on `$` exposes it |
| Effort level | only on `turn.step`, a streaming event this mod does not hook |
| tok/s, cache trend | the token breakdown arrives per turn, not per tick |
| Agent completion time | inferred from `$.agent.list()` status, not a real timestamp |

## Files

```
mod/
├── .claude-plugin/plugin.json
├── hooks/hooks.json          names the module
├── hooks/register.ts         the whole mod
├── tsconfig.json
└── types/claude-code.d.ts    vendored from anthropics/claude-code
```

`types/claude-code.d.ts` is Anthropic's published declaration file, copied
so `tsc -p mod` type-checks without a session; `/plugin-types` writes the
same contract into `.claude/types/`.
