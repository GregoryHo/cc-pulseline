# The activity band

The half of the `pulseline` plugin that draws from engine events instead of
the statusline hook — a **mod**, in the engine's sense: behaviour that lives
in a hooks module. It paints a band above the prompt while a turn is working,
and a pane on `/pulseline`.

It ships inside the same plugin as the statusline's commands, so there is one
thing to install and one version to track. The statusline path is untouched
and stays the default: the two are different canvases with different jobs,
and installing the plugin does not ask you to drop your `statusLine` setting.

It passes `claude plugin validate`, has tests across the eight files in hooks/tests/ that run
against the engine itself, and type-checks once the contract is in place
(see **Type-checking** below). There is no gate flag and no hard-coded path.
What has not changed is the API underneath: the engine's own contract says
**EARLY ACCESS: this surface may change between releases without notice**,
in those words, so expect to follow it.

## Run it

```bash
claude --plugin-dir .
```

An interactive session: the render hooks draw on the terminal and in the
desktop app (see **The desktop app** below) and yield on any other surface,
and a `-p` run draws nothing to put the band in. `/pulseline` opens a pane,
`/pulseline` again closes it.

It finds the binary itself — see **Finding the binary** below. Nothing else
is needed on a machine that has cc-pulseline 1.7.0 or newer installed.

`--plugin-dir .` is the development loop. For everyone else the plugin
installs in one step:

```
/plugin install pulseline --marketplace GregoryHo/cc-pulseline
```

The plugin and the binary ship from the same tag, so the floor is the
release you are installing — see **Finding the binary**. Installing the
plugin against an older binary is the one case the ladder has to explain
rather than fix.

## Finding the binary

Five rungs, in order, the first one that answers and is new enough winning:

| | Where | |
| --- | --- | --- |
| 1 | the `binaryPath` setting | what you named, always wins |
| 2 | `<plugin>/bin/cc-pulseline` | reserved for a bundling decision not yet taken; expected to miss |
| 3 | `~/.claude/pulseline/cc-pulseline` | where `scripts/install.sh` puts it on its download path |
| 4 | `cc-pulseline` on `PATH` | the npm package, or `cargo install` |
| 5 | `<root>/target/release/cc-pulseline` | **the dev rung** — this repository's own build. `<root>` is `$.session.root()`, read before every walk, so `/cd` or a worktree move finds the new checkout's build |

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

**The band needs 1.7.0 or newer. An older binary leaves your statusline
completely unaffected.** The floor tracks the release, because each one so
far has added the CLI the band of that release drives: `--rows`, `--columns`
and the `activity` input in 1.5.0, `--layout activity` in 1.6.0, `--no-icons`
in 1.7.0. None of those is an error to an older binary. A 1.6.x binary
ignores `--no-icons`, so on the desktop it would draw Nerd Font glyphs the
app's face does not have — a row of boxes, recorded as `'ok'`. An unknown layout name in particular
warns on stderr, falls back to `none` and exits 0 — so a 1.5.x binary would
answer a band spawn with the identity and budget rows, which is the
statusline drawn a second time above the prompt and, since the band is now
resident, drawn there permanently. `bandRun` would read `'ok'` and nothing
would be said. That is much harder to diagnose than an outright no, which is
what the floor turns it into.

**A too-old rung is passed over, not accepted and not fatal.** The walk goes
on, and a later rung may be new enough. This is not hypothetical: on the
machine this was written on, `cargo install` had left an older copy on
`PATH` (rung 4) while the dev build was current (rung 5). A ladder that took
the first thing to answer would have drawn the old one. The too-old message is
reached only when no rung at all is new enough, and it names the newest
copy found, so you are not sent updating to a version you already have.

### The `binaryPath` setting

A `userConfig` string field, so it is a row in `/config` and is stored in
settings under `pluginConfigs`. Put an absolute path to a cc-pulseline
binary of 1.7.0 or newer in it; leave it empty to use the ladder. Changing
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
   `// Written by Claude Code 2.1.300.`

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

The tests cover: the stdin payload's shape (`payload`), the ANSI and
argument readers (`parse`), the refresh gate (`throttle`), what each event
accumulates (`activity`), the binary ladder and version gate (`binary`),
`$.state` surviving a reload (`state`), the quiet desktop band's strip,
alerts, button and notifications (`desktop`), and what the band and pane actually
draw (`render`) — colours, row clipping against `maxRows`, the width and the
layout reaching the binary, when the band yields and when it speaks, the
pane's switcher, and the desktop app's own runs (`the desktop app` in
`render`).

`hooks/tests/harness.ts` is the shared bottom. Its header documents the
non-obvious things about the test kit — among them that `$.state` cannot be
read from a test at all, and the differing result shapes a bottom hook must
return. Read it before adding a test.

## What it draws

| Site | When | Rows | Layout |
| --- | --- | --- | --- |
| `AbovePrompt` band | always, once something has run | up to `maxRows`, which is small outside fullscreen | `activity` — activity rows only, asked for by name |
| `Pane` | while it is open | opened asking for 12; drawn at the box it gets (`scroll.bodyRows`), less the switcher's row | switchable: ledger / budgets / compact / rail |

Both sites run the binary themselves, because one row count cannot serve a
small band and a twelve-row pane. The pane run only happens while the pane
is open.

**The band stacks on the chain, never replaces it.** The hook awaits
`next(e)` first and draws its rows above whatever the mods beneath it drew
(another plugin's `AbovePrompt` buttons, say), so installing pulseline hides
nobody else's band. `maxRows` clips only pulseline's own rows; a taller tree
scrolls in the engine's window, and with the terminal's two rows outside
fullscreen, what lies beneath can start scrolled out of view.

**The band's argv names the `activity` layout** (`--layout activity`), the one
layout that draws no identity and no budget row. That is what makes the two
sites a division of labour rather than a copy:

| | statusline | band |
| --- | --- | --- |
| identity, context, tokens, TOK/s, cache trend, cost, quota | ✅ | — |
| running tools and their targets | — | ✅ |
| per-tool completed counts and failure marks | — | ✅ |
| agents and their real elapsed time | — | ✅ |
| todo | — | ✅ |

The split is not a preference. TOK/s and the cache hit-rate trend need two
samples less than two seconds apart (`update_output_speed`), and only the
transcript path sees usage per API call — the mod sees it once per turn, so
those two can only live on the statusline. In the other direction the engine
events carry the tool's own arguments, so the running target and the
`isError` failure mark are exact on the band where the transcript path has
to infer them from a 50-line field table.

`activity` leads with the running tools, so the `--rows 2` the band gets
outside fullscreen cuts the cumulative counts rather than the live row. See
[`docs/layouts.md`](layouts.md#activity--activity-rows-only-the-bands-layout)
for the row-by-row ladder.

**Between turns the band holds the state it last drew** — the tools that
finished with their counts and failure marks, the agents that ran, the todo.
Nothing reads as in flight that is not: the binary marks a finished item with
`✓` and a running one with the tool's own glyph and no tick, so a resident
band is a record of the turn that just happened rather than a claim about now.

That costs the rows it occupies: outside fullscreen `maxRows` is 2, and after
the first tool call those two rows are the band's for the session. The trade
was the other way for one release — `e.props.isWorking` gated the draw,
because the band then drew the layout the person had configured, which is the
identity and budget rows the statusline draws on its own line. Stacked, the
two were the same thing twice. Asking for `activity` ended that, and with the
duplication gone the gate had nothing left to prevent.

**Zero rows still yields.** `activity`'s output is empty when nothing has run,
which is the ordinary state at the start of a session rather than a fault, and
an empty box above the prompt says less than no box at all.

**`/clear` takes the band down, and that is right.** A `/clear` ends the
conversation and the process goes on under a new session id (`SessionEndInput`
on `reason: 'clear'`), and `$.state` is held "for the session" — so every atom
reads its initial value again, the output atom included, and zero rows yields.
The counts, the todo and the session's cache-read total all start over with
the conversation they belonged to. The band comes back on the next tool call:
`refresh` walks the binary ladder whenever `binaryChoice` is unset, so nothing
has to re-run `session.start` — which is just as well, because no
`session.start` fires after a `/clear`.

Keeping it there needs no extra invalidation: `turn.complete` refreshes, and
the render hook's `read` of the output atom subscribes that drawing, so the
rows settle to the finished state by themselves and then stand until the next
turn moves them. A terminal resized while nothing is running redraws from the
same rows — they were packed for the old box, so the band clips them until
the next refresh spawns at the width this hook just measured.

**A failed run still draws.** A missing or too-old binary raises no event of
its own, and an install line nobody sees fixes nothing, so that note is drawn
whether or not a turn is running. It is keyed on how the run ended
(`bandRun: 'never' | 'ok' | 'failed'` on the output atom), never on the row
count — with `activity` zero rows is normal, and a note keyed on it would sit
over an idle prompt for the whole session.

## The desktop app

The same plugin draws in the Code tab of Claude Code Desktop, which raises
both sites — the band and the pane — on the `desktop` surface. Two things
there are not the terminal's, and each changes what the band asks for:

| | terminal | desktop |
| --- | --- | --- |
| statusline under the prompt | yes | **no** — the Code tab does not run the `statusLine` command |
| band layout | `activity` | the `desktopLayout` setting: `quiet` (default — no binary run; a strip and alerts, see below), `compact` (identity and budget in one head row, activity under it), or the powerline `rail` / `anchor` as a head with `activity` run under it in the rows the head leaves (every row but one goes to the head) |
| glyphs | Nerd Font icons | Unicode (`--no-icons`): no Private Use Area glyphs, which the app's face lacks — icons become ASCII labels and powerline seams half blocks `▐▌`; braille, gauges and `✓✘` stay |
| idle band | yields until something has run | always draws: `quiet` its strip of quota, cost and branch (only what the mods beneath draw when it has no reading at all); the others their head row, the only place context and cost show |

The split in **What it draws** above is a terminal fact. On the desktop it
would leave context, cost and quota nowhere at all, so the desktop band draws
them itself. TOK/s and the cache sparkline stay missing there, for the reason
they are missing from the band anywhere (**Known gaps**).

### The quiet band

The Code tab already shows the model and effort, the permission mode, a
context ring, and every tool in the transcript, so `compact` drew most of
its row twice. `quiet` draws only what the Code tab lacks. `refresh` works it
out from `$.session.usage()`, the last turn's tokens and one `git status`
(`hooks/desktop.ts`) and publishes it with the rows; the render hook only
reads it.

One thin strip stays on screen: a small `Svg` gauge per quota window with
its percent and reset (red from 85%), the session cost and `$/h`, and
`G:branch* ↑n ↓n`. A reset a day or more away reads in days (`6d10h`).

Each window also carries its pace: the percent used divided by the share of
the window gone, read off `resets_at` and the window's length (5h, 7d), so
no history is kept. Under the limit it reads `→ ~19% at reset`; on pace to
run out first, `full in ~3d2h` in amber, with the gauge and label amber too.
A tick on the gauge, standing above and below the bar, marks the time gone:
a fill past it is a window used faster than it runs. Nothing is projected
in the first 5% of a window (15 minutes of five hours, 8.4 hours of seven
days), for a window of unknown length, or without a reset. The pace is an
average, so a seven-day window reads it with the nights folded in.

With no quota reading it draws the cost alone; outside a
repository it leaves `G:` out. The branch is read with `git status
--porcelain=v2 --branch` at most once every 10 seconds per root, as the
binary's own git cache does. `S:` and the activity rows are not drawn: the
Code tab shows the turn, and the pane keeps the rest.

Under the strip, a row opens for each value past its limit:

| alert | opens at | row | action |
| --- | --- | --- | --- |
| context | 70% (the binary's critical mark) | `Svg` gauge, `CTX 73%`, `146.0k / 200.0k` | a `/compact` button that fills an empty prompt, never sends; a draft is kept and a toast says why |
| cache | under 40% read with half or more written (the `C:%` "rebuilding" rung) | `cache 22%`, the share written | none |

Quota has no row of its own: the strip draws it. A window at 85% raises one
`$.ui.notify` per window and reset, kept in the atom so a reload does not
repeat it. When the band has fewer rows than it needs, the strip goes before
an alert. Every path, the empty one too, stacks on what the mods beneath draw.

A turn on the main loop that runs three minutes or more, not interrupted,
raises a notification when it ends, wherever the desktop is attached. Where
no channel sends a notification (`isSent: false`, which a session the desktop
hosts alone can answer), a toast says it instead.

Not yet seen on a real desktop: whether a one-row band clips an `Svg`. The
alert gauge is fixed at 120x10 CSS pixels, the strip's at 64x8.

Every span is drawn with `mono`. The desktop draws a `Text` in a proportional
face by default and the binary packs its columns for a monospaced one; the
terminal is always monospaced and ignores the prop.

Each surface gets a run of its own, chosen by `$.session.surfaces()`: a
session the desktop hosts alone spawns no terminal band, a terminal session
spawns no desktop one. A desktop that attaches mid-session gets its run from
`session.attach`, rather than waiting for the next tool call. The two share
one `bandRun` and `runNote`, so a failure on either is said on both. The pane
is one run for whichever surfaces show it, with `--no-icons` whenever the
desktop is among them.

`desktopLayout` is a `userConfig` field, so it is a row in `/config` (or
`/plugin configure pulseline@cc-pulseline`), and changing it reloads the
module. Its `options` are `quiet` and the layouts that carry identity and
budget in a few rows; a stored value outside them reads as unset, so `quiet`.

`mobile` and `vscode` still yield: the mobile app draws no `Input` or
`Select`, and VS Code attaches no surface yet.

### Running it in the desktop app

`--plugin-dir` cannot be given to a session the desktop app starts. Two ways
instead:

- **Installed**: + next to the prompt box → Plugins → Add marketplace
  `GregoryHo/cc-pulseline`, then add `pulseline`. A plugin installed at user
  scope from a terminal (`/plugin install`) loads in the desktop's sessions
  too.
- **This checkout**: name it in `CLAUDE_CODE_PLUGIN_DIRS`, in the `env` block
  of `~/.claude/settings.json` (a project's settings are not read for it), and
  start a new session:

  ```json
  { "env": { "CLAUDE_CODE_PLUGIN_DIRS": "/path/to/cc-pulseline" } }
  ```

  A long-lived desktop session reloads on save only with
  `CLAUDE_CODE_PLUGIN_DIR_WATCH=1` set the same way.

The binary ladder is the same; rung 5 resolves against the session's root.

## What it does not say

Nothing. A plugin a person installs pins no status entry beside their prompt
and writes no line into their transcript, so `$.ui.status` and `$.ui.log` are
behind the module's `DEBUG` constant and off. A failure has one channel — the
band's own note, drawn in place of the rows it could not get.

`session.start` also clears the entry rather than merely declining to set
one. A status entry belongs to the session, not to the load that pinned it:
a version that used to pin one would otherwise leave it beside the prompt for
the rest of that session, through every reload, because the module that would
have replaced it no longer says anything.

## When the note says "binary: not run yet"

That string is the output atom's initial value, and the band **does not draw
it**: the initial `bandRun` is `'never'`, which yields. Nothing has gone
wrong before the first walk finishes, and a note keyed on the not-run-yet
state would flash over the prompt every time `session.start` probed. It
reaches a surface only through `claude --debug` or the module's `DEBUG`
constant.

What the band does draw is a run that *failed*: `cc-pulseline not found`, the
version it found and rejected, a non-zero exit with the binary's stderr, or a
refused spawn. A refresh runs on
`session.start`, on both ends of every tool call, on a subagent starting or
stopping, at the end of a turn, on a settings change, and whenever the person
opens the pane or presses a layout key. The first of those is immediate: a
version probe and a render both measure about 10ms here, so the band is drawn
from real rows within a moment of the session opening.

So a band drawing nothing at all, with a turn running, has either not had
`session.start` complete or has nothing in flight to show. `claude --debug`
carries the per-hook lines that say which.

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
| Claude Code's version | `$.session.version()`, the engine the session runs on — which is not always the `claude` on `PATH`: the desktop app runs a Claude Code of its own. Asked on every refresh until it answers; `session.start` alone falls back to `CLAUDE_CODE_EXECPATH` (never there) and then `claude --version` |
| Effort level | `classic.PostToolUse`'s and `classic.Stop`'s `e.effort.level`. Two hooks because `BaseHookInput.effort` rides only on the hooks that fire "within a tool-use context … absent for session-lifecycle hooks": `PostToolUse` is the frequent one, `Stop` is all a turn with no tool call in it has |
| Output style | `prompt.compose`'s `e.outputStyle` |
| Agent start and stop times | `classic.SubagentStart` / `classic.SubagentStop` |
| Compaction count | `classic.PostCompact` — the end that means one happened. `PreCompact` can be blocked (`SessionCompactSkipped`: "Core answers it too, when a classic PreCompact hook blocks"), so counting there raised the badge over a compaction that never ran |
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
| Pane geometry with the pane open on two surfaces | one pane run serves every surface that shows it, sized from whichever surface drew it last — so with the pane open in a terminal and on the desktop at once (Remote Control), one of them gets rows packed for the other's box |
| A failure on one surface hides the other's rows | `bandRun` and `runNote` are shared: a failing desktop run puts its note on the terminal band in place of that band's healthy rows, and the reverse |

`classic.*` events close what the engine's own events do not: effort rides
on the classic hook inputs that fire within a tool-use context — not
`classic.PreToolUse`, whose argument is the tool call envelope, and not the
session-lifecycle ones — `SubagentStart` / `SubagentStop` give real agent
timings, and `PostCompact` is the compaction the transcript parser counts by
watching for `compact_boundary` — both fire once one has finished.

Effort has a window nothing closes: a turn's level is unknown until something
in it finishes, so the first tool of a session's first turn draws without the
pip ramp, and a level changed mid-turn reaches the band when that turn's
first tool returns. `turn.start` carries only `text` and `turnId`, no `$`
call answers effort, and the one event holding the request's own effort is
`turn.step` — which streams and sits on every model request, so reading it
is part of the deferred serve-mode work rather than a fix for this.

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
hooks/desktop.ts             the quiet desktop band's strip, alerts and gauges, pure
hooks/tests/harness.ts       shared bottom, and the kit's sharp edges
hooks/tests/*.test.ts(x)     the hook tests
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
