# Layouts

`layout.name` chooses how cc-pulseline arranges and decorates its rendered
rows. Nine layouts ship in two flavours:

| Flavour | Names | What they do |
|---|---|---|
| **Flat-row decorators** | `none`, `zones`, `grid`, `cards`, `sections` | Render the same 3 core rows (Identity / Config / Budget) plus optional Activity rows, then optionally wrap them in chrome (rules, label columns, frames) |
| **Instrument-cluster** | `cockpit`, `console`, `flightstrip`, `auto` | Own their full pipeline. Pack widget cells (gauge, sparkline, cost arc, tape) into a tighter row layout designed for live monitoring |

All nine share the same theme palette, segment toggles, and per-segment
visual composition (see [Visual Composition](#visual-composition)). The
TOML strings below are stable — internally they map 1-to-1 onto
`pane::LayoutStyle` variants.

```toml
[layout]
name        = "none"     # see catalog below
width_mode  = "auto"     # "auto" | "terminal" | "fixed"
fixed_width = 100        # only used when width_mode = "fixed"
min_width   = 60         # skip framing when terminal can't fit this many cols
max_width   = 160        # clamp auto-sized frames to this many cols
cc_margin   = 4          # cols subtracted from detected width in "terminal" mode
tonal_strata = true      # 2-tier separator tint (see docs/theme-palette.md)
```

`min_width`, `max_width`, `cc_margin`, `tonal_strata`, and the
`[segments.*]` toggles work identically across every layout.

---

## Layout catalog

### `none` — flat output (default)

No decoration. Rendered rows pass through unchanged. Lowest-overhead
choice.

```
M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline | G:feat/x *
1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs | 4 skills | 1h 22m
CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50
```

**Pick when** you want the minimal status line and care about screen real
estate above all else.

### `zones` — single labelled rule between state and activity

Inserts one horizontal rule (`─── activity ───`) between the **state**
rows (Identity / Config / Budget) and the **activity** rows (Tools /
Agents / Todos). Echoes Claude Code's own input-box rules so the
statusline reads as a continuation of CC chrome.

```
M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline | G:feat/x *
1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs | 4 skills | 1h 22m
CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50
─── activity ─────────────────────────────────────────────
T:Read main.rs | T:Bash cargo test
A:Explore [haiku]: Investigate logic (2m)
TODO:Fixing auth bug (1/3)
```

**Cost:** +1 row when activity present; degrades to flat otherwise.
**Pick when** you want a single visual cue marking "this is what's
happening" without introducing borders.

### `grid` — fixed label column + divider

Table layout with a fixed-width label column, a `│` divider, and
right-padded content. Every line begins and ends at the same visual
position. Activity continuation rows blank the label so the divider
lines up.

```
Identity  │ M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline
Config    │ 1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs
Budget    │ CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50
Activity  │ T:Read main.rs | T:Bash cargo test
          │ A:Explore [haiku]: Investigate logic (2m)
          │ TODO:Fixing auth bug (1/3)
```

**Cost:** 0 rows; ~12 cols on the left.
**Pick when** you want explicit group labels and aligned right edges
without row overhead.

### `cards` — one independent frame per group

Each non-empty group becomes its own `╭─┬─╮ … ╰─┴─╯` card, stacked
vertically. All cards share global `max_label_width` and
`max_content_width` so internal divider and outer walls align column-for-
column.

```
╭──────────┬───────────────────────────────────────────────────────────╮
│ Identity │ M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline │
╰──────────┴───────────────────────────────────────────────────────────╯
╭──────────┬───────────────────────────────────────────────────────────╮
│ Budget   │ CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50            │
╰──────────┴───────────────────────────────────────────────────────────╯
…
```

**Cost:** +2 rows per non-empty group.
**Pick when** you want strong visual separation between groups; have
plenty of vertical room.

### `sections` — single outer frame with internal separators

One outer `╭─┬─╮ … ╰─┴─╯` wrapper around every group, with `├─┼─┤`
between every pair of non-empty groups. Reads as one container.

```
╭──────────┬───────────────────────────────────────────────────────────╮
│ Identity │ M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline │
├──────────┼───────────────────────────────────────────────────────────┤
│ Budget   │ CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50            │
├──────────┼───────────────────────────────────────────────────────────┤
│ Activity │ T:Read main.rs | T:Bash cargo test                        │
╰──────────┴───────────────────────────────────────────────────────────╯
```

**Cost:** +2 rows + 1 per gap between non-empty groups (cheaper than
`cards`, same per-group separation).
**Pick when** you want the framed dashboard feel of `cards` without the
row cost between groups.

### `cockpit` — 3-row instrument cluster (recommended live default)

Identity headline + cluster row (gauge, sparkline, cost arc, quota) +
activity ticker.

```
Opus 4.7  feat/x ↑3  ~/cc-pulseline                              43%·86k
CFG  󰈙1  󰱇5  󰧜2  󱭧1  󰆧2  󰐱4  1h
CTX  [██████▋          ] 43% ⠀⠀⠀⠀⠀⢠   TOK 1.2K/s   $3.50 ◑   5h [██▏        ] 17% (resets 2h 0m)
▶ Read · ▶ Bash   ✓ ×8   A:Explore (2m)   • 6/6 todos
```

**Cost:** 3-4 rows.
**Pick when** you want the "instrument-cluster" feel with widget
visualisation. Width-adaptive: collapses gracefully below 100 cols; falls
back to a single line below 80.

### `console` — framed dashboard, ≥130 cols recommended

Higher "quality feel" than cockpit. Wraps content in `╭─╮ │ ╰─╯`. Quota
defaults to a gauge bar instead of text.

**Cost:** 5-7 rows.
**Pick when** terminal is wide enough (laptop, external monitor) and you
want the most polished presentation.

### `flightstrip` — dense 2-row strip for narrow IDE statuslines

L1: identity + pct + gauge + cost. L2: sparkline + activity ticker.
Below 90 cols collapses to a single line.

**Cost:** 1-2 rows.
**Pick when** terminal is narrow (split panes, IDE bottom strip).

### `auto` — width-bracket resolver

Re-runs every render tick. Picks the cluster layout that fits:

| Width    | Picks         |
|----------|---------------|
| ≥ 130    | `console`     |
| 110..130 | `cockpit`     |
| 90..110  | `flightstrip` |
| < 90     | `cockpit` (which itself collapses to a single row below 80) |

Window resize triggers a layout switch on the next CC poll without any
state. Inherits `cockpit`'s visual defaults.

---

## Visual Composition

Every layout owns *row arrangement and chrome*. Widget choices for the
context, cost, quota, and tools segments live on a separate axis: the
`*_visual` config fields. The user's TOML overrides the layout's
default; otherwise the layout's default applies.

This is **Variation B** from the design process: each layout asserts a
tasteful default so the out-of-the-box experience preserves its
identity (cockpit looks like cockpit, none looks like none), but every
visual decision is overridable per segment.

### Spec syntax

A `*_visual` value is a `+`-joined list of widget names. Empty string
means "use layout default". Unknown widget names are silently dropped.

```toml
[segments.budget]
context_visual = "gauge+sparkline"  # cockpit / console default
context_visual = "gauge"            # text-free dashboard
context_visual = "text"             # plain text inside cockpit
context_visual = ""                 # = layout default

cost_visual = "text+arc"            # cockpit / console default
cost_visual = "arc"                 # icon only
cost_visual = "text"                # text only

[segments.quota]
visual = "text"                     # cockpit / flightstrip default
visual = "bar"                      # console default — gauge + pct overlay

[segments.tools]
visual = "tape"                     # instrument-cluster default (▶ Read · ▶ Bash)
visual = "list"                     # flat-layout default (per-row T:Read: ...)
```

### Recognized widgets per segment

| Segment | Widgets | Visual contract |
|---------|---------|-----------------|
| `context_visual` | `gauge`, `sparkline`, `text` | `gauge` is bracket-framed (`[████▎      ]` icon, `[####------]` ascii); empty cells are literal whitespace inside the frame so they read as "unused capacity" regardless of fill colour. `sparkline` is icon-only — empty under `display.icons = false`. `text` is the legacy `<glyph>43% (86.0k/200.0k)` form. |
| `cost_visual` | `text`, `arc` | `arc` is icon-only — empty under `display.icons = false`. Lone `text` always carries `($X.X/h)` rate annotation; compound `text+arc` text picks up the rate when arc disappears (so the user always sees burn rate, in icon or text form). |
| `quota_visual` | `text`, `bar` | `bar` adds a gauge before the pct text — same bracket-framed visual as `context_visual`'s `gauge`. **pct number and `(resets …)` annotation render unconditionally** when their data is available; `bar` is purely additional visualisation. |
| `tools_visual` | `tape`, `list` | `tape` arrow `▶` → `>` under ASCII; `list` is multi-row (used by flat-layout activity rows) — silently dropped from inline cluster contexts. |

### Per-layout defaults

Set in `frames::default_visuals_for(LayoutStyle)`. Resolved at
`build_render_config` time when the user TOML field is empty;
`RenderConfig::effective_*_visual()` provides the same fallback for code
paths (mostly tests) that construct `RenderConfig` directly.

| Layout | `context_visual` | `cost_visual` | `quota_visual` | `tools_visual` |
|--------|------------------|---------------|----------------|----------------|
| `cockpit` | `gauge+sparkline` | `text+arc` | `text` | `tape` |
| `console` | `gauge+sparkline` | `text+arc` | `bar` | `tape` |
| `flightstrip` | `gauge` | `text` | `text` | `tape` |
| `auto` | `gauge+sparkline` | `text+arc` | `text` | `tape` |
| `none`, `zones`, `grid`, `cards`, `sections` | `text` | `text` | `text` | `list` |

### Composability examples

```toml
# Cockpit, but I prefer plain text — keeps the 3-row structure and
# activity ticker, drops gauge/sparkline/arc.
[layout]
name = "cockpit"
[segments.budget]
context_visual = "text"
cost_visual = "text"

# Cards frame, but with a gauge inside the Budget card. Was impossible
# before Phase 3 (flat layouts hardcoded text rendering for L3).
[layout]
name = "cards"
[segments.budget]
context_visual = "gauge"

# Console without quota gauge — text quota only.
[layout]
name = "console"
[segments.quota]
visual = "text"
```

### Implementation pointers

For contributors:
- Dispatch hubs live in `frames/shared.rs`:
  `render_context_visual`, `render_cost_visual`, `render_quota_visual`,
  `render_tools_visual_inline`. Layouts call them; never call
  `widgets::*::render` directly from a layout (or that segment loses
  user composability).
- Atomic widgets live in `widgets/`. Each takes
  `(data, …, mode, palette, color)`. Ascii-incompatible widgets (sparkline,
  arc) return `""` under `GlyphMode::Ascii` so dispatch hubs drop the
  empty cell cleanly.
- To add a new widget variant: add the renderer in `widgets/foo.rs`,
  match its name in the relevant dispatch hub, document it in this
  file's [Recognized widgets](#recognized-widgets-per-segment) table.

---

## Width handling

When the auto-detected `terminal_width` (via `COLUMNS` env or `ioctl`)
is narrower than `layout.min_width`, the active frame is bypassed
entirely and rows render flat — the binary will not output a half-
collapsed frame.

`width_mode = "terminal"` makes framed styles span the detected
terminal width minus `cc_margin` cols. The default `cc_margin = 4` is
the empirically verified safe value for Claude Code 2.1.119; CC
allocates the statusline a sub-region ~1–4 cols narrower than the raw
terminal, and lines at exactly the raw width trigger wrap and collapse
the multi-line render to a single visible line.

`width_mode = "fixed"` pins a frame to `fixed_width` cols regardless of
terminal size — useful for screenshot fixtures and reproducible mockups.

Instrument-cluster layouts apply additional per-segment width gating
inside their dispatch (e.g. `cockpit` drops `sparkline` from the spec
below 100 cols, forces `cost_visual = "text"` below 100). These limits
are layout-internal; the user-supplied spec is the *target*, not the
guarantee.

---

## Migration notes

### From `[pane]` to `[layout]` (released)

The TOML section was renamed `[pane]` → `[layout]` and the Rust enum
`PaneStyle::V1*/V2*` → `LayoutStyle::*`. Existing TOML strings
(`"none"`, `"cockpit"`, etc.) are unchanged. Old `[pane]` sections in
TOML are silently ignored — fall back to defaults.

### `show_ctx_sparkline` removed

Replaced by `context_visual` containing `sparkline`. Migration:

```toml
# Old
[segments.budget]
show_ctx_sparkline = true

# New
[segments.budget]
context_visual = "gauge+sparkline"
```

The branch was unreleased so this is a clean break — no deprecation
shim. `lib.rs` still gates the `ctx_history` allocation copy on whether
the resolved spec includes `sparkline`, so nothing leaks for users who
don't opt in.

### Quota labels: `Q5h`/`Q7d` → `5h`/`7d`

Cluster-row quota cells used to repeat the `Q` prefix on every window
(`Q5h 17%   Q7d 60%`). Now each cell carries a bare label (`5h …`,
`7d …`); the v1 flat layout still uses a single `Q:` group prefix
followed by `5h: 75%` / `7d: 60%` (unchanged). No config migration —
the prefix is a render-time string, not a setting.

### Gauge visual: bracket-framed

The `gauge` widget changed from a colour-only "battery" texture
(filled `█` in fill colour, empty `█` in `structural`) to a
bracket-framed indicator (`[████▎      ]`). The interior empty cells
are now literal whitespace inside `[ ]` brackets. The `width`
parameter passed to `widgets::gauge::render(pct, width, …)` is the
*interior* cell count — visible width is `width + 2`. Sub-cell
precision (`▏▎▍▌▋▊▉`) at the fill boundary is preserved.

### Quota cell: pct + `(resets …)` always shown

`render_quota_visual` previously bound `pct` text to the `text`
widget and `(reset)` to text mode only. Now both render unconditionally
whenever the data is available, regardless of whether the spec asks
for `bar`. The `bar` widget is purely additional visualisation in
front of the pct — opting into `bar` no longer drops the precise
percentage or the reset countdown.
