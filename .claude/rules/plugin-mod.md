# Plugin & Mod

The repository is two things that share one binary: the **statusline**
(`statusLine.command` in settings, the Rust path described in
`integration.md`) and the **`pulseline` plugin**, whose function-hooks module
in `hooks/` draws above the prompt and in a pane. `docs/band.md` is the
plugin's full reference; this file is the contract to hold changes to.

## Supported surfaces

| Surface | Statusline | Band (`AbovePrompt`) | Pane (`/pulseline`) |
| --- | --- | --- | --- |
| terminal (Claude Code CLI) | yes, the identity and budget rows | `--layout activity`: the turn in flight only, empty until something runs | switchable: ledger / budgets / compact / rail |
| desktop (Claude Code Desktop, Code tab) | **no**: the Code tab does not run `statusLine` | the `desktopLayout` setting. `quiet` (default) runs no binary: one thin strip kept on screen (`Svg` quota gauges with their pace and reset, cost and `$/h`, and under them this session's share of what the active desktop sessions spend; cost alone with no quota reading; no `G:`, which the Code tab shows itself), plus a row when context or the cache passes its limit, with a `/compact` button for context (`hooks/desktop.ts`). Every path stacks on what the mods beneath draw. `compact` alone, or `rail` / `anchor` as a powerline head with `activity` stacked under it, are the binary's rows, always with `--no-icons` and drawn idle as well | same switcher, `--no-icons` while the desktop shows it |
| mobile | — | yields: the app draws no `Input`/`Select` | yields |
| vscode | — | yields: Claude Code for VS Code attaches no surface yet | yields |
| `-p` / SDK | runs if configured | nothing to draw into | nothing to draw into |

The terminal split is a division of labour: the statusline owns identity
and budget, the band owns activity, and neither draws the other's rows. The
desktop has no statusline, and its own UI already shows the model, the
effort, a context ring and every tool, so its default band draws only what
passes a limit; the binary-drawn desktop layouts carry both. A change that moves
a row between them changes this table first.

## Glyph tiers

`GlyphMode` has three tiers, and the band picks one per surface:

- `Icon`: Nerd Font icons and powerline seams. The terminal default.
- `Unicode`: everything but the Private Use Area. Braille, gauge marks, `✓✘` and the `▐▌` half-block seams stay. This is what `--no-icons` asks for, and every desktop spawn passes it.
- `Ascii`: `display.icons = false`, the person's own choice. `--no-icons` never raises a render to `Unicode` from here.

`GlyphMode::nerd_font()` gates PUA icons and `GlyphMode::unicode()` gates
everything else. A new glyph decides which one it needs; it never matches on
the variants directly.

## Writing the hooks module

Each rule below cost a release to learn. The reasons are in `docs/band.md`.

- **`$` only reaches functions declared at the top of the module.** A closure inside `register` that takes `$` is refused at load, and the whole module fails with it.
- **Write every `$` call literally.** The loader's static scan lists them, and an aliased call is refused at run time.
- **No `$.state` write while a `ui.render` hook draws.** Geometry measured in a render goes in the closure (`State`). A refresh started from a render is floated (`void drawSoon(...)`) and gated on a change.
- **Every gating hook ends `.catch(($, e, next) => next(e))`.** The mod only observes, so a failure answers with whatever the chain beneath says.
- **Refresh only through `drawSoon`.** It is the one place a rejected refresh is recorded under its own sequence number. A refresh is never awaited unless a person is waiting on the drawing.
- **The binary's ANSI goes through `spansOf`.** A text child holding a control character makes the engine refuse the whole tree.
- **Never pass JSX children through a spread.** `<Text {...span} />` type-checks and draws no text, so split `children` out.
- **Every span is `mono`.** The desktop draws `Text` in a proportional face, and the binary packs columns for a monospaced one.
- **Version every new `$.state` field in `types/index.d.ts`, and read it with `?? default`.** State survives the reload that upgrades the plugin, so an atom written by an older version lacks the new field.

## Binary contract

- The plugin needs a **minimum binary version** (`MIN_VERSION` in `hooks/binary.ts`). It moves with any release that adds a CLI the band's argv uses, because an older binary ignores an unknown flag, exits 0 and draws the wrong thing. Raise it in the same PR, along with `plugin.json`'s `binaryPath` description and `docs/band.md`.
- The band spawns the binary with `--rows`, `--columns`, `--layout` and, on the desktop, `--no-icons`. These are runtime overrides applied in `src/main.rs`, never config fields.
- Claude Code's version comes from `$.session.version()`, the engine the session runs on. The desktop app runs its own Claude Code, so the `claude` on `PATH` can be a different version.

## Testing

- `command claude plugin test .`: `command` bypasses a shell function that injects options, which `plugin test` refuses.
- A UI case mounts through `harness.ts`'s `BAND` / `DESKTOP_BAND` / `PANE` / `DESKTOP_PANE` and passes `surfaces` to `bottom()`. Behaviour that can differ by surface is tested on both `terminal` and `desktop`.
- Settings are given per test: `test(name, { options: { desktopLayout: 'rail' } }, body)`.
- A test cannot raise `session.start`. Logic that must be testable runs on the refresh path too (see `syncVersion`).
- `tsc -p .` needs `.claude-plugin/types/`, which the engine writes. See `docs/band.md` → Type-checking.

## Running it locally

- Terminal: `claude --plugin-dir .`
- Installed from this checkout: `claude plugin marketplace add <repo path>`, then `claude plugin install pulseline@cc-pulseline --scope user`. The plugin is read from the folder, so `/reload-plugins` picks up edits. A rebuilt binary reaches other projects through `cargo install --path . --force`.
- Desktop: the same install, or `CLAUDE_CODE_PLUGIN_DIRS` in `~/.claude/settings.json`'s `env`. Start a new session to load it.
