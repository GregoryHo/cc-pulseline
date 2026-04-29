# Layouts

`layout.name` chooses how cc-pulseline arranges and decorates its rendered
rows. The shipping layouts:

| Layout | What it does |
|---|---|
| `none` | Flat output, no chrome. Default. |
| `zones` | Inserts a single `─── activity ───` rule between state and activity rows. |
| `grid` | Fixed label column + `│` divider + right-padded content. |
| `sections` | Single `╭─┬─╮` outer frame with `├─┼─┤` between groups. |
| `console` | `sections` with the Identity row hoisted into the top frame title. Recommended ≥110 cols. |
| `ledger` | Label-value pairs in a fixed-width TAG column (`ENV / CTX / TOK / COST / 5h / 7d / TOOL / AGENT / TODO`); blank rows separate groups. Tallest layout. Ships sparkline + delta-time on the CTX row by default. |

All six share the same theme palette, segment toggles, and per-segment
visual composition (see [Visual Composition](#visual-composition)). The
TOML strings below are stable — internally they map 1-to-1 onto
`pane::LayoutStyle` variants.

> **Removed in v1.0.6:** `cards`, `cockpit`, `flightstrip`, `auto` —
> consolidated into the surviving six above. User configs naming any of
> the removed layouts fall back to `console` with a stderr warning.

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

## Design Principles

> 新增的視覺元素必須追加資訊或節奏感，絕不能取代已有的可讀資訊。
>
> **Added visual elements must add information or rhythm — never
> substitute meaning with decoration.**

This is the gate every widget, glyph, and per-layout flourish has to
pass. It governs which `*_visual` variants are accepted, how
`apply_pane` decorates flat output, and whether a redesign earns its
extra row.

What the principle disqualifies, by example:

- A glyph that requires a legend to decode (an arc widget showing
  `◔ ◑ ●` next to a number the reader already has).
- A trend chart with no axis or baseline (an unscaled sparkline
  beside a percentage that already changes color at the threshold).
- A compressed display that drops information the flat layout
  rendered fully (a `▶ Read · ▶ Edit` tape that erases the file
  paths the user came to read).

What the principle permits, conditionally:

- Gauges and running indicators **alongside** the original text — the
  text stays readable; the gauge adds spatial intuition the text
  alone can't provide.
- Sparkline **with** a delta-time label or aurora-velocity color —
  the trend acquires a scale, so it answers a question the number
  alone doesn't.
- Layout-specific rhythm (the ledger TAG column, the console
  identity-in-frame title) — these add structure, not decoration.

When in doubt, ask: *if I delete this widget, what information does
the reader lose?* If the answer is "none — the text already said
it", the widget fails the principle.

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

**Cost:** +2 rows + 1 per gap between non-empty groups.
**Pick when** you want a framed dashboard feel.

### `console` — sections + identity-in-title

`console` is structurally just `sections` with the Identity row hoisted
into the top frame border:

```
╭─ Opus 4.7 · medium · ~/cc-pulseline · feat/x* ──────────────────────────╮
│ Config   │ 󰈙 2 CLAUDE.md | 󰱇 10 rules | 󰧜 4 memories | 󱭧 36 hooks       │
├──────────┼─────────────────────────────────────────────────────────────┤
│ Budget   │ CTX:43% (86.0k/200.0k) | TOK I:1 O:8 C:5.7k/114.5k | $4.56  │
├──────────┼─────────────────────────────────────────────────────────────┤
│ Activity │ ✓ Read ×2 | ✓ Bash ×1                                       │
│          │ T:Read .../console.rs | T:Edit .../gauge.rs                 │
╰──────────┴─────────────────────────────────────────────────────────────╯
```

The title separator is ` · ` (middle dot) in `structural` color. Identity
fields keep their existing semantic colors (model → `stable_blue`, branch
→ `stable_green` / `alert_orange` for dirty, etc.).

**Cost:** 1 row saved over `sections` (Identity is in the title, not a
labelled row).
**Pick when** terminal is ≥ 110 cols and you want a polished framed
presentation. Below ~90 cols the frame chrome gets cramped — `sections`
or `none` is friendlier there.

### `ledger` — TAG-column typographic rhythm

Label-value pairs aligned in a fixed left column, like an accounting
ledger. Each metric occupies its own row, prefixed by a 6-char TAG; blank
rows separate logical groups. Tallest layout.

```
╭─ Opus 4.6 · ~/cc-pulseline · feat/status-pane* ↑21 ─────────────────────╮
│                                                                         │
│  ENV     󰈙 2 CLAUDE.md   󰱇 10 rules   󰧜 4 memories   󱭧 36 hooks        │
│                                                                         │
│  CTX     43%   86.0k / 200.0k   ⠀⠀⠀⠀⠀⢠   30→43% in 5m                  │
│  TOK     1 in   8 out   5.7k / 114.5k cache                             │
│  COST    $4.56   $4.42/h                                                │
│                                                                         │
│  5h      62%   resets 1h 59m                                            │
│  7d      28%   resets 4d 23h 59m                                        │
│                                                                         │
│  TOOL    ✓ Read ×2   ✓ Bash ×1                                          │
│          ▶ Read   .../console.rs                                        │
│                                                                         │
│  AGENT   󱦻 Explore   Investigate parser edge case   [haiku]   <1s       │
│  TODO    0/3 done · 3 pending                                           │
│                                                                         │
╰─────────────────────────────────────────────────────────────────────────╯
```

The CTX row is the only row with a sparkline (and the only place in the
codebase the sparkline is rendered). Its color carries CTX consumption
**velocity**, picked from the aurora palette stops:

| Velocity (% / min) | Color |
|---|---|
| < 1 | `aurora_low` (calm / idle) |
| 1 – 5 | `aurora_mid` (active) |
| ≥ 5 | `aurora_high` (hot — burning context fast) |

The sparkline shape carries direction (rising / falling / flat); the
delta-time tail (`30→43% in 5m`) carries the trend in plain text so the
information survives `display.icons = false` (sparkline disappears,
delta label remains).

**Cost:** 12-15 rows when fully populated.
**Pick when** the statusline pane has plenty of vertical room and you
want to scan metrics by group rather than density. Below 90 cols falls
back to `sections`.

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
context_visual = "text"             # default for every layout except ledger
context_visual = "gauge"            # gauge bar replaces percentage text
context_visual = "text+sparkline"   # numbers + braille trend (ledger default)
context_visual = ""                 # = layout default
```

### Recognized widgets per segment

| Segment | Widgets | Visual contract |
|---------|---------|-----------------|
| `context_visual` | `gauge`, `sparkline`, `text` | `gauge` is bracket-framed (`[████▎      ]` icon, `[####------]` ascii); empty cells are literal whitespace inside the frame so they read as "unused capacity" regardless of fill colour. `sparkline` is icon-only — empty under `display.icons = false`. `text` is the standard `<glyph>43% (86.0k/200.0k)` form. |

> Earlier versions exposed widget composition for `cost_visual`,
> `quota_visual`, and `tools_visual` — those segments now render in a
> single canonical form (text for cost / quota; one-line list for tools).
> The hub-dispatch infrastructure remains for future widget additions.

### Per-layout defaults

Set in `frames::default_visuals_for(LayoutStyle)`. Resolved at
`build_render_config` time when the user TOML field is empty;
`RenderConfig::effective_*_visual()` provides the same fallback for code
paths (mostly tests) that construct `RenderConfig` directly.

| Layout | `context_visual` | `cost_visual` | `quota_visual` | `tools_visual` |
|--------|------------------|---------------|----------------|----------------|
| `none`, `zones`, `grid`, `sections`, `console` | `text` | `text` | `text` | `list` |
| `ledger` | `text+sparkline` | `text` | `text` | `list` |

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
