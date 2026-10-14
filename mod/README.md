# pulseline-mod

A Claude Code **mod** — a plugin whose behaviour lives in a hooks module —
that draws cc-pulseline from engine events instead of the statusline hook.

It passes `claude plugin validate`, has 88 tests across seven files that run
against the engine itself, and type-checks once the contract is in place
(see **Type-checking** below). There is no gate flag and no hard-coded path.
What has not changed is the API underneath: the engine's own contract says
**EARLY ACCESS: this surface may change between releases without notice**,
in those words, so expect to follow it. The classic statusline path is
untouched and stays the default.

## Run it

```bash
claude --plugin-dir mod
```

An interactive session: the render hooks yield on any surface but the
terminal (`e.surface !== 'terminal'`), and a `-p` run draws nothing to put
the band in. `/pulseline` opens a pane, `/pulseline` again closes it.

It finds the binary itself — see **Finding the binary** below. Nothing else
is needed on a machine that has cc-pulseline 1.5.0 or newer installed.

`--plugin-dir` is the only way to run it today, and deliberately so. There
is no `/plugin install … --marketplace` line yet — `claude plugin validate`
will advise adding one — because the marketplace listing waits on the
v1.5.0 release that the version floor above depends on.

## Finding the binary

Five rungs, in order, the first one that answers and is new enough winning:

| | Where | |
| --- | --- | --- |
| 1 | the `binaryPath` setting | what you named, always wins |
| 2 | `<plugin>/bin/cc-pulseline` | reserved for a bundling decision not yet taken; expected to miss |
| 3 | `~/.claude/pulseline/cc-pulseline` | where `scripts/install.sh` puts it on its download path |
| 4 | `cc-pulseline` on `PATH` | the npm package, or `cargo install` |
| 5 | `<cwd>/target/release/cc-pulseline` | **the dev rung** — this repository's own build |

Rung 5 is what makes this repository's own session work, and it is not
redundant with rung 4. Developing means building and wanting the band to
use *that* build; and an installed copy is easily older than the checkout.
On the machine this was written on it was exactly that — `cargo install`
had left 1.4.0 on `PATH`, so without rung 5 there would have been no usable
binary at all. It follows the session's working directory rather than
anyone's absolute path, so it works in any checkout.

Each rung is probed by **running** it with `--version`, never by
`$.fs.stat`. That is not caution, it is rung 3. `install.sh` ends with
`mkdir -p ~/.claude/pulseline` and `--init` on *both* its paths, so a
machine installed from source has that directory holding `config.toml` and
no binary at all — the binary went to `~/.cargo/bin`. A stat of the
directory succeeds and tells you nothing. Running the candidate is the only
thing that tells a binary from a folder, and it checks the version in the
same call.

### The version floor, and why

**The band needs 1.5.0 or newer. An older binary leaves your statusline
completely unaffected.** 1.4.0 draws the statusline perfectly well; what it
lacks is `--rows`, `--columns` and the `activity` input. So it does not
refuse — it draws the classic layout into the band at the wrong width and
row count, which is much harder to diagnose than an outright no.

**A too-old rung is passed over, not accepted and not fatal.** The walk goes
on, and a later rung may be new enough. This is not hypothetical: on the
machine this was written on, `cargo install` had left 1.4.0 on `PATH`
(rung 4) while the dev build was 1.5.0 (rung 5). A ladder that took the
first thing to answer would have drawn 1.4.0. The too-old message is
reached only when no rung at all is new enough, and it names the newest
copy found, so you are not sent updating to a version you already have.

### The `binaryPath` setting

A `userConfig` string field, so it is a row in `/config` and is stored in
settings under `pluginConfigs`. Put an absolute path to a cc-pulseline
binary of 1.5.0 or newer in it; leave it empty to use the ladder. Changing
it reloads the module, and a `classic.ConfigChange` also clears the
remembered resolution, so installing the binary mid-session is picked up
without a restart.

## Type-checking needs a step that a clone does not have

`tsc -p mod` type-checks against `mod/.claude-plugin/types/`, which **the
engine writes** — there is no command that generates it — and which is
gitignored, because it is ~21,000 lines that go stale with every release.
So in a fresh clone it is absent and type-checking does not work at all:

```
$ tsc -p mod
mod/hooks/payload.ts(1,46): error TS2307: Cannot find module 'claude-code' …
… 220 errors
```

`claude plugin validate mod` and `claude plugin test mod` are **unaffected**
— both pass with no contract present, because only `tsc` reads the `.d.ts`.

Two ways to get it:

1. **Run the mod once** — the better option. `claude --plugin-dir mod` in
   an interactive session; the engine lays
   `mod/.claude-plugin/types/claude-code/index.d.ts` beside the mod as it
   loads it, and per its own header the built-in tools
   (`claude-code-tools/`) and the connected MCP tools (`claude-code-mcp/`)
   with it.
2. **Copy it by hand**, which is enough for `tsc` and no more. The bundled
   `plugin-authoring` skill writes the same API contract for the running
   build at `<skill>/types/claude-code.d.ts`; copy that one file to
   `mod/.claude-plugin/types/claude-code/index.d.ts` — that exact path,
   which is where a real load overwrites it. It gets you the API but not
   the tool contracts, so `e.tool === "Bash"` will not narrow. Its first
   line names the version that wrote it; this mod was last checked against
   `// Written by Claude Code 2.1.296.`

This is a deliberate trade: the alternative is committing a generated file
that is wrong the moment Claude Code updates.

## Verify it

```bash
# the Rust side: the activity input and the --rows/--columns/--layout flags
cargo test

# the loader's own static scan: hooks, `$` calls, `$.state` keys, env reads,
# and whether every gating hook has a `.catch`
claude plugin validate mod

# the mod's own tests, run against the engine itself
command claude plugin test mod

# types — needs the contract above
tsc -p mod
```

`command claude` is not a typo. Where the shell's `claude` is a function
that injects an option — `claude --allow-dangerously-skip-permissions "$@"`
is a common one — `claude plugin test` refuses it ("not run from this
spelling … with no options before `plugin`"), while `validate` tolerates it.
`command` bypasses the function.

The 88 tests cover: the stdin payload's shape (`payload`), the ANSI and
argument readers (`parse`), the refresh gate (`throttle`), what each event
accumulates (`activity`), the binary ladder and version gate (`binary`),
`$.state` surviving a reload (`state`), and what the band and pane actually
draw (`render`) — colours, row clipping against `maxRows`, the width
reaching the binary, and the pane's switcher.

`mod/tests/harness.ts` is the shared bottom. Its header documents the
non-obvious things about the test kit — among them that `$.state` cannot be
read from a test at all, and the differing result shapes a bottom hook must
return. Read it before adding a test.

## What it draws

| Site | Rows | Layout |
| --- | --- | --- |
| `AbovePrompt` band | up to `maxRows`, which is small outside fullscreen | whatever the effective config selects |
| `Pane` | opened asking for 12; drawn at the box it gets (`scroll.bodyRows`) | switchable: ledger / budgets / compact / rail |

Both sites run the binary themselves, because one row count cannot serve a
small band and a twelve-row pane. The pane run only happens while the pane
is open.

The band's ANSI is parsed into `Text` props by `spansOf` rather than passed
through, because the band refuses a whole tree if any text child holds a
control character. What it does **not** restrict is which characters you
may draw: `TextProps` carries no code-point rule, so braille gauges, box
drawing and plane-15 Nerd Font glyphs all land (there is a test). The
width-1 BMP restriction in the contract belongs to the cell-grid's `cells`
field, which this mod does not use.

## How it gets its data

| Row | Source |
| --- | --- |
| Running tools, per-tool counts, failures | `tool.call`: marked before `next(e)`, counted after, `result.isError` is the failure flag |
| Tool argument (`cargo test`, a file path) | the tool's own arguments on `e` — a lookup, not a guess |
| Todo | the `TodoWrite` call's `todos` array |
| Agents | `$.agent.list()` |
| Context, cost, rate limits | `$.session.usage()` |
| Token breakdown | `turn.complete`'s `e.usage` (per turn, not per tick) |
| Claude Code's version | `claude --version`, probed once. `CLAUDE_CODE_EXECPATH` is tried first and is never there — see below |
| Effort level | `classic.PostToolUse`'s `e.effort.level` |
| Output style | `prompt.compose`'s `e.outputStyle` |
| Agent start and stop times | `classic.SubagentStart` / `classic.SubagentStop` |
| Compaction count | `classic.PreCompact` |
| API error | `turn.complete` where `e.reason` is `'error'` |
| MCP count, instruction counts | `$.tool.list()` and `classic.InstructionsLoaded` — what actually loaded, not what exists on disk |

None of it is read from the transcript: on this path the binary uses
`PayloadActivityCollector` and its `FileTranscriptCollector` never runs.

Everything the person would notice losing is held in `$.state`, so a hot
reload does not zero it — the counts, the todo, agent ages, the pane you
opened and the layout you picked, and the binary's last output, so a reload
redraws rather than flashing a not-run-yet note. Geometry stays in the
module's closure because the host refuses a state write while a `ui.render`
hook draws.

## What the first runs established

- A user-tier hooks module loads, and `$.process.run` with `stdin` is
  allowed there — so the Rust binary stays the renderer and nothing is
  reimplemented in TypeScript.
- The band **refuses a whole tree** if any text child holds a control
  character, so the binary's ANSI is parsed into `Text` props (`spansOf`).
- `$` may only be passed to a function declared at the top of the module.
  A closure inside `register` is refused, by name, at load. The same scan
  decides which `$` calls are allowed at run time: a call it did not list
  is refused.
- `$.env` is Claude Code's own environment, not the one it hands to the
  children it spawns: `CLAUDE_CODE_EXECPATH` and `CLAUDE_EFFORT` are not
  on it.
- JSX children must be element children, never a prop. `<Text {...span} />`
  with `children` inside the spread type-checks, compiles, and draws the
  style with no text in it; a hand-built tree carrying `props.children` is
  refused outright ("Text prop \"children\" is not allowed"). Both call
  sites split the text out, and a test guards it.

## Known gaps against the statusline path

| Missing | Why |
| --- | --- |
| tok/s, cache trend | the token breakdown arrives per turn, not per API call, and the per-call dedup the Rust side does needs the transcript |
| Sub-agent todo aggregation | each `TodoWrite` is seen, but the fold is not written |
| The API-error badge, in practice | the mod sends `last_api_error_ms` and the Rust side carries it into the frame, but the badge is emitted from one place — `src/render/layout.rs` — so the layouts that own their own pipeline never draw it, the band's and the pane's `ledger` among them. Closing it means editing `src/render/`, which this mod does not touch; it is tracked as separate work |

`classic.*` events close what the engine's own events do not: effort rides
on every classic hook input (except `classic.PreToolUse`, whose argument is
the tool call envelope), `SubagentStart` / `SubagentStop` give real agent
timings, and `PreCompact` is the compaction the transcript parser counts by
watching for `compact_boundary`. Those hooks do not run in a plugin a person
installs where an organization manages Claude Code; `validate` lists which.

## Files

```
mod/
├── .claude-plugin/plugin.json   manifest, `types`, and the binaryPath field
├── hooks/hooks.json             names the module
├── hooks/register.tsx           the hooks, the atoms, and the resolver
├── hooks/payload.ts             the binary's stdin contract, pure
├── hooks/binary.ts              the ladder and the version gate, pure
├── hooks/throttle.ts            the refresh gate, pure
├── types/index.d.ts             the `$.state` contract
├── tests/harness.ts             shared bottom, and the kit's sharp edges
├── tests/*.test.ts(x)           88 tests
└── tsconfig.json
```

`types/index.d.ts` is the mod's own `$.state` contract, named in
`plugin.json`; `validate` holds every key the module touches to it. It is
deliberately self-contained — a contract that imports is refused, which is
why the span styles are spelled out there rather than taken from
`TextProps`.

Not in the tree, and not in git: `.claude-plugin/types/`, which the engine
writes. See **Type-checking** above.
