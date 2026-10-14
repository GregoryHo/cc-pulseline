# The activity band

The half of the `pulseline` plugin that draws from engine events instead of
the statusline hook — a **mod**, in the engine's sense: behaviour that lives
in a hooks module. It paints a band above the prompt while a turn is working,
and a pane on `/pulseline`.

It ships inside the same plugin as the statusline's commands, so there is one
thing to install and one version to track. The statusline path is untouched
and stays the default: the two are different canvases with different jobs,
and installing the plugin does not ask you to drop your `statusLine` setting.

It passes `claude plugin validate`, has 111 tests across seven files that run
against the engine itself, and type-checks once the contract is in place
(see **Type-checking** below). There is no gate flag and no hard-coded path.
What has not changed is the API underneath: the engine's own contract says
**EARLY ACCESS: this surface may change between releases without notice**,
in those words, so expect to follow it.

## Run it

```bash
claude --plugin-dir .
```

An interactive session: the render hooks yield on any surface but the
terminal (`e.surface !== 'terminal'`), and a `-p` run draws nothing to put
the band in. `/pulseline` opens a pane, `/pulseline` again closes it.

It finds the binary itself — see **Finding the binary** below. Nothing else
is needed on a machine that has cc-pulseline 1.5.0 or newer installed.

`--plugin-dir .` is the development loop. For everyone else the plugin
installs in one step:

```
/plugin install pulseline --marketplace GregoryHo/cc-pulseline
```

That line lands with the v1.5.0 release and not before, because the band's
version floor is 1.5.0 — see **Finding the binary**. Installing the plugin
against an older binary is the one case the ladder has to explain rather
than fix.

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

`tsc -p .` type-checks against `.claude-plugin/types/`, which **the
engine writes** — there is no command that generates it — and which is
gitignored, because it is ~21,000 lines that go stale with every release.
So in a fresh clone it is absent and type-checking does not work at all:

```
$ tsc -p .
hooks/payload.ts(1,46): error TS2307: Cannot find module 'claude-code' …
… 220 errors
```

`claude plugin validate .` and `claude plugin test .` are **unaffected**
— both pass with no contract present, because only `tsc` reads the `.d.ts`.

Two ways to get it:

1. **Load the plugin once** — the better option, if it works. `claude
   --plugin-dir .` in an interactive session; the contract's own header
   says the engine lays
   `.claude-plugin/types/claude-code/index.d.ts` beside the plugin each
   time it loads it from a folder you own, with the built-in tools
   (`claude-code-tools/`) and the connected MCP tools (`claude-code-mcp/`)
   next to it. That is quoted from the header, not something this repo has
   watched happen — every contract here so far has been laid by hand, so
   if it does not appear, use option 2 and say so.

   Two ways that look like they should work and **do not**, both tested on
   2.1.299 with the contract moved aside: `claude plugin test .` loads the
   mod through the engine's own host and still writes nothing, and
   `claude -p --plugin-dir .` loads it (the hooks answer) and also writes
   nothing. So an interactive session is the only candidate left, and a
   headless run will not get you the types however many times you try it.
2. **Copy it by hand**, which is enough for `tsc` and no more. The bundled
   `plugin-authoring` skill writes the same API contract for the running
   build at `<skill>/types/claude-code.d.ts`; copy that one file to
   `.claude-plugin/types/claude-code/index.d.ts` — that exact path,
   which is where a real load overwrites it. It gets you the API but not
   the tool contracts, so `e.tool === "Bash"` will not narrow. Its first
   line names the version that wrote it; the band was last checked against
   `// Written by Claude Code 2.1.296.`

This is a deliberate trade: the alternative is committing a generated file
that is wrong the moment Claude Code updates.

## Verify it

```bash
# the Rust side: the activity input and the --rows/--columns/--layout flags
cargo test

# the loader's own static scan: hooks, `$` calls, `$.state` keys, env reads,
# and whether every gating hook has a `.catch`
claude plugin validate .

# the plugin's own tests, run against the engine itself
command claude plugin test .

# types — needs the contract above
tsc -p .
```

`command claude` is not a typo. Where the shell's `claude` is a function
that injects an option — `claude --allow-dangerously-skip-permissions "$@"`
is a common one — `claude plugin test` refuses it ("not run from this
spelling … with no options before `plugin`"), while `validate` tolerates it.
`command` bypasses the function.

The 111 tests cover: the stdin payload's shape (`payload`), the ANSI and
argument readers (`parse`), the refresh gate (`throttle`), what each event
accumulates (`activity`), the binary ladder and version gate (`binary`),
`$.state` surviving a reload (`state`), and what the band and pane actually
draw (`render`) — colours, row clipping against `maxRows`, the width
reaching the binary, and the pane's switcher.

`hooks/tests/harness.ts` is the shared bottom. Its header documents the
non-obvious things about the test kit — among them that `$.state` cannot be
read from a test at all, and the differing result shapes a bottom hook must
return. Read it before adding a test.

## What it draws

| Site | Rows | Layout |
| --- | --- | --- |
| `AbovePrompt` band | up to `maxRows`, which is small outside fullscreen | whatever the effective config selects |
| `Pane` | opened asking for 12; drawn at the box it gets (`scroll.bodyRows`), less the switcher's row | switchable: ledger / budgets / compact / rail |

Both sites run the binary themselves, because one row count cannot serve a
small band and a twelve-row pane. The pane run only happens while the pane
is open.

The band's ANSI is parsed into `Text` props by `spansOf` rather than passed
through, because the band refuses a whole tree if any text child holds a
control character. What it does **not** restrict is which characters you
may draw: `TextProps` carries no code-point rule, so braille gauges, box
drawing and plane-15 Nerd Font glyphs all land (there is a test). The
width-1 BMP restriction in the contract belongs to the cell-grid's `cells`
field, which the hooks module does not use.

## How it gets its data

| Row | Source |
| --- | --- |
| Running tools, per-tool counts, failures | `tool.call` for every tool but the ones the transcript path dispatches elsewhere or ignores (`Agent`, the Task API's and TodoWrite's own calls, and that file's `NOISE_TOOLS`): marked before `next(e)` and refreshed there, so the running tool is in the band while it runs; counted after, `result.isError` is the failure flag. Both ends refresh unconditionally: the 400ms gate, which still guards the events that fire without a tool call of their own, would drop a start whose window the previous call's completion had just closed, and that start has no later refresh to fall back on |
| Tool argument (`cargo test`, a file path) | the tool's own arguments on `e`, at the key that tool uses — the same per-tool table as the transcript path's `extract_target`, so a `Grep` shows its pattern rather than its directory. Passed whole; the renderer truncates per tool kind and width |
| Todo | the `TodoWrite` call's `todos` array, or the Task API's `TaskCreate` / `TaskUpdate` calls accumulated in the `tasks` atom. All three are read from the call's *result*, so a refused list or task is never drawn as current, and a created task is keyed by the id the engine answered with rather than by a count of creations |
| Agents | `$.agent.list()` |
| Context, cost, rate limits | `$.session.usage()` |
| Token breakdown | `turn.complete`'s `e.usage` (per turn, not per tick) |
| Claude Code's version | `claude --version`, probed once. `CLAUDE_CODE_EXECPATH` is tried first and is never there — see below |
| Effort level | `classic.PostToolUse`'s `e.effort.level` |
| Output style | `prompt.compose`'s `e.outputStyle` |
| Agent start and stop times | `classic.SubagentStart` / `classic.SubagentStop` |
| Compaction count | `classic.PreCompact` |
| API error | `turn.complete` where `e.reason` is `'error'` |
| Cumulative cache reads | summed from `turn.complete`'s usage, once per turn, and sent as a total — the binary cannot total it, since every refresh re-sends the same last-turn usage. Kept across compaction, as the transcript path keeps it |
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
| tok/s, and the cache trend's *sparkline* | the token breakdown arrives once per turn, not per API call, so there is nothing to plot a per-call hit rate from — the dedup by `message.id` that the transcript path does needs the transcript. The CACHE row's cumulative total is **not** missing: the mod sums it per turn and sends `activity.cache_read_total` |
| Sub-agent todo aggregation | each `TodoWrite` is seen, but the fold is not written |
| The band's first draw of a session, at the measured width | `session.start` refreshes before anything has drawn, so that spawn uses the closure's initial `3x120`. The render hook measures `maxRows` and `bodyColumns` but cannot act on it: `$.state.set` is refused while a drawing is in flight, and no `$` call answers the band's geometry from outside a render — `viewport` is on the render event's envelope only. So unlike the pane, which `ui.open` resolves "once drawn" and can therefore refresh against, the band converges on the next refresh instead. That is now the next tool call's spawn on the way in rather than a completion, so the window is one tool call of a turn the band only appears during |
| `--rows` in the pane, where the layout is `ledger` | the pane tells the binary the box it got, but the ledger returns before the height ladder — `--layout ledger --rows 4` still emits eleven rows (`src/render/layout.rs`'s comment says so: the ledger owns its pipeline and `ledger_dense` is its height lever). The pane's body is a window over the tree, so nothing is lost — it scrolls — but the switcher sits under the rows and goes below the fold in a short dock. Closing it properly means either the ledger honouring the cap or wiring `ledger_dense`, both inside `src/render/`, which the band does not touch |
| The API-error badge, in practice | the mod sends `last_api_error_ms` and the Rust side carries it into the frame, but the badge is emitted from one place — `src/render/layout.rs` — so the layouts that own their own pipeline never draw it, the band's and the pane's `ledger` among them. Closing it means editing `src/render/`, which the band does not touch; it is tracked as separate work |

`classic.*` events close what the engine's own events do not: effort rides
on every classic hook input (except `classic.PreToolUse`, whose argument is
the tool call envelope), `SubagentStart` / `SubagentStop` give real agent
timings, and `PreCompact` is the compaction the transcript parser counts by
watching for `compact_boundary`.

None of those hooks run in a plugin a person installs where an organization
manages Claude Code — there they pass over the plugins you install, and only
a plugin the organization lists in managed settings runs them. `validate`
names the list, and `prompt.compose` is on it too, so output style goes with
them. Everything in the table above that comes from `$` directly is
unaffected.

## Files

The plugin root is the repository root, so these sit beside the Rust crate:

```
.claude-plugin/plugin.json   manifest, `types`, and the binaryPath field
hooks/hooks.json             names the module
hooks/register.tsx           the hooks, the atoms, and the resolver
hooks/payload.ts             the binary's stdin contract, pure
hooks/binary.ts              the ladder and the version gate, pure
hooks/throttle.ts            the refresh gate, pure
hooks/tests/harness.ts       shared bottom, and the kit's sharp edges
hooks/tests/*.test.ts(x)     111 tests
types/index.d.ts             the `$.state` contract
tsconfig.json
```

`Cargo.toml`'s `exclude` keeps all of it out of the crates.io tarball, and
`cargo` ignores `hooks/tests/*.test.ts` the way it ignores any non-`.rs`
file, so the TypeScript tests and the Rust ones never see each other.

`types/index.d.ts` is the plugin's `$.state` contract, named in
`plugin.json`; `validate` holds every key the module touches to it. It is
deliberately self-contained — a contract that imports is refused, which is
why the span styles are spelled out there rather than taken from
`TextProps`.

Not in the tree, and not in git: `.claude-plugin/types/`, which the engine
writes. See **Type-checking** above.
