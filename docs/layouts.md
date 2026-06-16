# Layouts

`layout.name` chooses how cc-pulseline arranges and decorates its rendered
rows. The shipping layouts:

| Layout | What it does |
|---|---|
| `none` | Flat output, no chrome. Default. |
| `compact` | 2–3 row micro layout: packed identity row, packed budget+quota row, activity ticker on row 3 (only when active). |
| `budgets` | Flat dashboard: identity row, then `CONTEXT / 5H QUOTA / 7D QUOTA` as three column-aligned equal-weight gauges, then a TOKENS + cost row. Compares burn across the three windows on a shared axis. Quota rows need `[segments.quota]` enabled. |
| `console` | Single `╭─...─╮` outer frame with `├─┼─┤` between groups and the Identity row hoisted into the top frame title. Recommended ≥110 cols. |
| `ledger` | Label-value pairs in a fixed-width TAG column (`ENV / CTX / TOK / COST / 5h / 7d / TOOL / AGENT / TODO`); blank rows separate groups. Tallest layout. Ships sparkline + delta-time on the CTX row by default. |
| `rail` | **Height 1.** One connected Powerline bar (identity → pressure) riding a gray ink ramp; exactly one segment tints when its state crosses a threshold (the live signal). Nerd-font tier, stdin-only. See `seams`. |
| `anchor` | **Height 1.** A reverse-video hero capsule (model) anchors the line by silhouette; the rest trail as dim text where colour marks the one live signal. Nerd-font tier, stdin-only. See `seams`. |

All share the same theme palette, segment toggles, and per-segment
visual composition (see [Visual Composition](#visual-composition)). The
TOML strings below are stable — internally they map 1-to-1 onto
`pane::LayoutStyle` variants.

> **Removed in v1.1.0:** `cards`, `cockpit`, `flightstrip`, `auto` —
> user configs naming any of these fall back to `console` with a stderr
> warning.
>
> **Removed in the 7→4 consolidation:** `zones` and `grid` fall back to
> `none` (they shared its visual defaults); `sections` falls back to
> `console` (its identity-in-title sibling). Each emits a stderr
> warning.

```toml
[layout]
name        = "none"     # see catalog below
min_width   = 60         # skip framing when terminal can't fit this many cols
max_width   = 160        # clamp auto-sized frames to this many cols
cc_margin   = 4          # cols subtracted from the detected terminal width
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

### `compact` — 2–3 row micro layout

Everything packs onto at most three rows, no chrome. Row 1 packs the
identity (L1) cells; row 2 packs the budget (L3) cells plus a compact
quota (`5h:62%` — threshold-colored percentage, reset time dropped) with
the standard ` | ` separator; which cells appear is still governed by
the `show_*` toggles. Each row packs width-aware independently: the
identity row drops its reference cells (path, version, style) first,
and the budget row drops the TOK cell (including the `C:%` cache hit
rate) before CTX/cost/quota — so the hit-rate survives much narrower
terminals than the old single fused row. Row 3 is a single packed
activity ticker (completed grand total → first running tool → agent
groups → todo counts) and only appears while there is activity — idle
footprint is exactly **2 rows**. L2 config counts have no home here by
design (reference data — flip to another layout when you need them).

With `show_cache_trend = true` (`[segments.budget]`, default off) the
budget row appends an opt-in C-cell trend sparkline — a 6-cell braille
strip of the cache hit-rate history — right after the TOK cell. It packs
as an Optional cell, so under width pressure it drops FIRST (before TOK,
never instead of the Required CTX/cost/quota cells). Icon-only: empty
under `display.icons = false`.

```
M:Opus 4.7 | S:concise | CC:2.2.0 | P:~/proj | G:feat/x *
CTX:43% (86.0k/200.0k) | TOK I:10 O:20 C:50% | $3.50 | 5h:62%
✓ 25 tools | T:Bash: cargo test | A:Explore (2m) | TODO:1/3
```

**Cost:** 2–3 rows total. (Absolute minimum footprint: `none` +
`max_total_lines = 1–2`, which lands on the FuseCore 1-row fused head.)
**Pick when** the statusline keeps squeezing Claude Code's footer or the
in-terminal agents panel; pairs well with trimming L1 via
`[segments.identity]` toggles (e.g. `show_style = false`,
`show_version = false`, `show_project = false`).

The height-degradation ladder (`max_total_lines`) reuses this fused head
as its final rung: running tools → completed tools → agents → todo →
merged activity row → quota into L3 → drop config row → **fuse-core**
(identity + budget + compact quota fused into the row above). But note
that compact ≠ `none` + `max_total_lines = 2` (idle differs): idle, the
ladder stops early at the quota merge — L1 and L3+quota keep separate
rows and fuse-core is never reached — while compact always fuses.

### `budgets` — three parallel gauges

A flat (frameless) dashboard that stacks the three budget windows as
column-aligned, equal-weight gauges under the identity row, so burn
across them reads on one shared spatial axis (the inline
`CTX:43% … 5h:62% … 7d:28%` form scatters them with no common baseline):

```
M:Opus 4.7 | E:high ▮▮▮▮▮ | P:~/cc-pulseline | G:feat/x *
CONTEXT   43%  ▰▰▰▰▰▰▰▰▰▰───·───·──────   86.0k/200.0k   ⟳2
5H QUOTA  62%  ▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰─────·───   resets 1h 59m
7D QUOTA  28%  ▰▰▰▰▰▰▰─────·───────·───   resets 4d 6h
TOK I:10 O:20 C:50%   $4.56 ($4.56/h)
```

Each gauge reuses the shipped bracketless `▰─·` marks-gauge (no second
`█▌░` dialect) at a wider width; the three share one width that scales
down together on narrow terminals (clamped so they never vanish). The
percentage is the **D2 anchor** — the row's hero metric in its threshold
color, with the label (tag tier) and trailing context (structural)
receding. There is no SGR-bold primitive, so the color hierarchy carries
the emphasis (same as the ledger budget rows). The CONTEXT row appends
the `⟳N` compaction marker when the session has auto-compacted. Leads
with the effort ramp (`effort_visual` defaults to `word+ramp` here).

**Cost:** identity + CONTEXT + (5H/7D when quota enabled) + TOKENS +
activity. **Pick when** you want to watch context and subscription burn
side-by-side. Quota rows only appear with `[segments.quota] enabled`.

### Trend-forward CONTEXT plot — a recipe, not a layout

There is no `velocity` layout. "Trend-forward" was a config preset (`none`
with a `plot` CTX default) and **no** bespoke builder, so it read as `none`
with two metrics swapped — and the design's actual hero (an oversized %
and a per-data-column gradient) is impossible in a fixed-cell terminal. It
was removed; the **`plot` widget lives on**, and any layout opts into the
trend-forward view through the dispatch hub:

```toml
[layout]
name = "none"                    # or console / ledger / …

[segments.budget]
context_visual = "plot+text"     # braille line-plot + delta-time tail, then the number

[segments.quota]
enabled = true
quota_visual = "gauge"
```

```
M:Opus 4.7 | E:high ▮▮▮▮▮ | P:~/cc-pulseline | G:feat/x *
⡠⠔⠊⠉ 18→43% in 5m  43% 86.0k/200.0k | TOK I:10 O:20 C:50% | $4.56 ($4.56/h)
Q: 5h: ▰▰▰▰▰▰▰▰▰───·─ 62% (resets 1h 59m)
```

The `plot` widget is a normalized braille **line**, distinct from the
ledger's bottom-up `sparkline`: it rescales the window to its own min→max
so a shallow 30→43% climb fills the cell height instead of collapsing into
the global 0–100 buckets. It's icon-only, so under `display.icons = false`
the plot glyph drops and the `30→43% in 5m` delta-time tail carries the
trend (the same axis-and-tail honesty the ledger sparkline keeps). Setting
`name = "velocity"` now warns and falls back to `none` — use the recipe
above.

### `console` — framed dashboard with identity-in-title

One outer `╭─...─╮ … ╰─┴─╯` wrapper around every group, with `├─┼─┤`
between every pair of non-empty groups, and the Identity row hoisted
into the top frame border:

```
╭─ Opus 4.7 · medium · ~/cc-pulseline · feat/x* ──────────────────────────╮
│ ENV      │ 󰈙 2 CLAUDE.md | 󰱇 10 rules | 󰧜 4 memories | 󱭧 36 hooks       │
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

**Cost:** +2 rows + 1 per gap between non-empty groups (Identity is in
the title, not a labelled row).
**Pick when** terminal is ≥ 110 cols and you want a polished framed
presentation. Below ~90 cols the frame chrome gets cramped — `none` is
friendlier there.

### `ledger` — TAG-column typographic rhythm

Label-value pairs aligned in a fixed left column, like an accounting
ledger. Each metric occupies its own row, prefixed by a 6-char TAG; blank
rows separate logical groups. Tallest layout.

```
╭─ CC:2.1.119 · Opus 4.6 · ~/cc-pulseline · feat/status-pane* ↑21 ────────╮
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
╰─────────────────────────────────────────────────────────────────────────╯
```

The CTX row is the only row with a sparkline (ledger is the only layout
that ships the sparkline by default — any layout can opt in via
`context_visual`). Its color carries CTX consumption
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

With `show_cache_trend = true` a CACHE TAG row renders between TOK and
COST:

```
│  CACHE   ⣀⣄⣦   4.4M read total   5% create                              │
```

Sparkline of the cache hit-rate history (filled with the creation-aware
cache color of the latest sample — not the velocity aurora), the
cumulative cache read total (raw, deduped per API call), and the
creation share. The row drops entirely when there is no cache signal;
under Ascii the sparkline vanishes but the text cells remain. Note this
is gated by the `show_cache_trend` segment toggle, **not** a `*_visual`
spec widget — there is no `cache` entry in the visual-spec vocabulary.

**Cost:** 12-15 rows when fully populated.
**Pick when** the statusline pane has plenty of vertical room and you
want to scan metrics by group rather than density. Below 90 cols falls
back to `console`.

#### Spacing: `ledger_dense`

Group separators in ledger are inserted *lazily* — a blank row only
appears between two non-empty groups. Trailing blanks above the bottom
frame are not possible regardless of which groups are populated.

`[layout] ledger_dense = true` drops every inter-group blank for a
compact rhythm, useful when statusline vertical room is tight:

```
╭─ … ──────────────────────────────────╮
│  ENV     ...                          │
│  CTX     ...                          │
│  TOK     ...                          │
│  COST    ...                          │
│  5h      ...                          │
│  TOOL    ...                          │
│  AGENT   ...                          │
│  TODO    ...                          │
╰───────────────────────────────────────╯
```

Default is `false` (legacy rhythm: one blank between ENV, Budget+Quota,
TOOL, and AGENT+TODO groups).

---

### `rail` — one connected Powerline bar

**Height 1, stdin-only, nerd-font tier.** A single row of segments joined by
real Powerline seams. The left cluster runs identity → pressure; the right
cluster (`cost`, `version`) is pushed toward the far edge with seams pointing
inward at the middle gap.

```
 Opus 4.6  high  cc-pulseline  feat/ledger-quota +2 ~1  43%        $3.47  v2.1.153
└─ model ─┘└eff┘└── cwd ──┘└──── git ────┘└ctx┘         └cost┘└version┘
   high  ▲ the ONE tinted segment (effort=high → warn/amber); rest = gray ramp
```

The bar is **monochrome by default** — every segment rides a 3-step gray ink
ramp. **Colour is reserved for the live signal**: a segment leaves the ramp and
takes a render-role tint only when its state crosses a threshold:

| Segment | Tints when | Colour |
|---|---|---|
| effort | level ≥ `high` | `color_for_effort_level` (warn → crit up the scale) |
| ctx | pct ≥ 55 (first `ctx_marks()` mark) | `color_for_ctx_pct` (warn ≥55, crit ≥70) |
| git | working tree dirty | `alert_orange` on the `~N` count only — branch stays ramp |

In the default session that is exactly **one** segment — no rainbow. Cost is
**not** a signal (informational); it stays gray even at high burn. Under width
pressure, cells drop in priority order **version → cost → cwd → git → effort**;
**model** and **ctx** (the hero + the signal) survive longest. Below
`min_width` the bar bypasses to flat `none`.

### `anchor` — hero capsule + dim trail

**Height 1, stdin-only, nerd-font tier.** One reverse-video **capsule** (angled
Powerline caps) anchors the line; the remaining fields trail as dim text. Two
orthogonal channels: **shape = identity, colour = state** — so the capsule is a
stable identity colour *and* the trail can still flash one signal.

```
 Opus 4.6 ▒ high · cc-pulseline · feat/ledger-quota +2 ~1 · 43% · $3.47 · v2.1.153
└─ capsule ─┘  ▲ amber          └──────────── dim trail (structural) ───────────┘
   (model bg)    (effort=high, the lone tint)
```

The hero is `model.display_name` (capsule body = the model role colour, text =
reverse-video). The trail (`effort · cwd · git · ctx · cost · version`) is dim
(`structural`) except the one item whose state crosses threshold — same tinting
rule as `rail`.

#### Capability tiers: `seams`

Both `rail` and `anchor` are nerd-font tier. One config knob picks the seam
vocabulary, and every higher tier names a lower fallback so nothing renders as
tofu:

```toml
[layout]
seams = "powerline"   # powerline | blocks   (default: powerline)
```

| Element | `seams = "powerline"` (default) | `seams = "blocks"` | `display.icons = false` / `NO_COLOR` floor |
|---|---|---|---|
| rail seam | PUA `\u{e0b0}`/`\u{e0b2}` | unicode half-block `▐`/`▌` | ` \| ` separator, no fill |
| anchor cap | PUA `\u{e0b2}`/`\u{e0b0}` | reverse-cap `▐body▌` | `[model]` bracket + dim trail |
| cell icon | MD glyph + space | MD glyph + space | ASCII prefix (`M:`, `G:`, …) |

`blocks` is the unicode rung: a connected coloured bar in any UTF-8 terminal
without a patched font. `seams` is independent of `display.icons` (which gates
only the per-cell MD glyphs); colour is the hard gate — without it the bar
can't read, so it collapses to the ` | ` floor (NO_COLOR / dumb terminal).

> **Colour fidelity note.** The design specifies a truecolor RGB ink ramp
> blended from `term_bg`. The renderer is 256-colour and has no `term_bg`
> source (not in the stdin contract, not detectable), so the bed is a fixed
> 256 grayscale ramp — the standard vim-airline / tmux Powerline approach.
> A truecolor backend is a later swap, gated on a `term_bg` source.

---

## Visual Composition

Every layout owns *row arrangement and chrome*. Widget choices for the
context, quota, tools, agents, and todo segments live on a separate
axis: the `*_visual` config fields (`agents_visual` is covered
separately [below](#why-agents_visual-has-no-symmetric-dispatch-hub)). The user's TOML overrides the layout's
default; otherwise the layout's default applies.

This is **Variation B** from the design process: each layout asserts a
tasteful default so the out-of-the-box experience preserves its
identity (ledger looks like ledger, none looks like none), but every
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
| `context_visual` | `gauge`, `sparkline`, `plot`, `text` | `gauge` is bracketless (`▰▰▰▰▰▰···──·──` icon, `======:::--:--` ascii) with threshold marks at the percentages where colour transitions. CTX marks are fixed at `[55, 70]` (`ThemePalette::ctx_marks()`); the bar's fill colour matches `color_for_ctx_pct` so the chroma escalates through good/warn/critical at the same points the marks call out. `sparkline` is icon-only — empty under `display.icons = false`. `plot` is a normalized braille **line** plot (`widgets::plot`): one dot per column at the sample's height, rescaled to the window's own min→max so the trend *shape* shows even for a small range — distinct from `sparkline`'s bottom-up bars. It carries a `30→43% in 5m` delta-time tail (text, so the trend survives ascii where the braille drops out) and is filled by CTX consumption velocity. Also icon-only. `text` is the standard `<glyph>43% (86.0k/200.0k)` form. |
| `quota_visual` | `gauge`, `text` | Same widget as CTX, with quota's fixed marks `[50, 85]`. `gauge` adds the bar before the percentage. `text` produces no bar — caller renders the existing `5h: 62% (resets ...)` text only. |
| `tools_visual` | `counts`, `targets`, `ticker` | Row-selection atoms parsed by `ToolsVisualSpec` (`activity/builder.rs`). `counts` = completed-tool count rows (`✓ Bash ×12`, capped by `max_completed_lines`, ` +N` fold). `targets` = the running/recent tools row (`T:Bash: cargo test`). `ticker` subsumes both: grand total + running tools fused into ONE row (`✓ 25 tools \| T:Bash: cargo test`). All text — ascii-safe. |
| `todo_visual` | `text`, `bar` | Parsed by `TodoVisualSpec` (`activity/builder.rs`). `text` = item text / task summary (current form). `bar` = 5-cell progress gauge of completed/total slotted after the TODO prefix (`▰▰───` icon, `==---` ascii; no threshold marks). `bar` alone keeps the `(c/t)` counts. The celebration and legacy-text rows ignore `bar` (nothing to gauge). |
| `effort_visual` | `word`, `ramp` | Identity-row effort cell, dispatched by `render_effort_visual`. `word` = the level name (`high`) — today's behaviour. `ramp` = an ordinal pip ramp (`widgets::effort`) pinned to the 5-step scale (`low/medium/high/xhigh/max`); lit count = the level's 1-based ordinal, fill colour escalates via `color_for_effort_level`. `▮▮▮▮▮` icon — a single `▮` (U+25AE) glyph for every cell, lit/dim split purely by colour (lit = effort colour, dim = `separator`), per the design system; `===--` ascii / NO_COLOR fallback (one glyph can't read without colour), so it is **not** icon-gated (renders under `display.icons = false`, unlike `sparkline`). Off-scale values (e.g. `auto`, a future level) degrade to a single pip — no false N-of-5. `word+ramp` is the gauge-alongside-text pairing the rendering principle permits; `ramp` alone is parseable but discouraged (pips need the word to decode), and a spec resolving to nothing falls back to `word` so the cell never collapses to a bare `E:` label. |

### Per-layout defaults

Set in `frames::default_visuals_for(LayoutStyle)`. Resolved at
`build_render_config` time when the user TOML field is empty;
`RenderConfig::effective_*_visual()` provides the same fallback for code
paths (mostly tests) that construct `RenderConfig` directly.

| Layout | `context_visual` | `quota_visual` | `tools_visual` | `agents_visual` | `todo_visual` | `effort_visual` |
|--------|------------------|----------------|----------------|-----------------|---------------|-----------------|
| `none` | `text` | `text` | `counts+targets` | `name+description+model` | `text` | `word` |
| `compact` | `text` | `text` | `ticker`¹ | `name` | `text` | `word` |
| `budgets` | `gauge`² | `gauge`² | `counts+targets` | `name+description+model` | `text` | `word+ramp` |
| `console` | `text` | `gauge` | `counts+targets` | `name+description+model` | `text` | `word` |
| `ledger` | `text+sparkline` | `gauge` | `counts+targets` | `name+description+model` | `text` | `word` |
| `rail` | `text`³ | `text`³ | `ticker`³ | `name`³ | `text`³ | `word` |
| `anchor` | `text`³ | `text`³ | `ticker`³ | `name`³ | `text`³ | `word` |

¹ Informational: compact always renders the fused inline activity row,
which is the ticker form by construction.

² Informational: budgets composes its CTX + quota gauges inline (aligned
label column + pct + bar, see `layout::assemble_budgets`), so the bars
render by construction; the `gauge` defaults document the intent rather
than flowing through the dispatch hubs. Consequently `context_visual` /
`quota_visual` overrides are **inert** on budgets — the three-gauge
alignment is the layout's identity (the same stance ledger takes toward
its TAG rhythm).

³ Informational: `rail`/`anchor` are single-row, stdin-only — CTX renders
as an inline tinted bar segment (not via `render_context_visual`), and
there are no activity rows. The `*_visual` hubs are therefore **inert**;
the defaults document intent. `effort_visual` stays `word` because the
ordinal pip ramp would double the bar's own colour signal.

CTX bar (`context_visual = "gauge"`) is opt-in for every layout —
the framed-layout defaults stay `text` for CTX and add `gauge` only
to quota. The asymmetry is intentional: CTX has more competing data
on its row (token counts, cache); quota has just `pct + reset` and
benefits more from the bar's spatial signal.

### Composability examples

```toml
# Console layout, but with a CTX gauge added (default is text-only).
[layout]
name = "console"
[segments.budget]
context_visual = "gauge"

# Console without the quota gauge — text-only quota.
[layout]
name = "console"
[segments.quota]
visual = "text"

# None layout (flat, minimalist) but with quota bar opt-in.
[layout]
name = "none"
[segments.quota]
visual = "gauge"
```

### Implementation pointers

For contributors:
- Live dispatch hubs in `frames/shared.rs`:
  `render_context_visual`, `render_quota_visual`. Layouts call them;
  never call `widgets::*::render` directly from a layout (or that
  segment loses user composability). Enforced by
  `tests/dispatch_hub_iron_rule.rs`.
- Atomic widgets live in `widgets/`. The `gauge` widget takes
  `(pct, width, marks, fill_color, palette, mode, color)` — caller
  picks fill colour and threshold marks. Ascii-incompatible widgets
  (sparkline) return `""` under `GlyphMode::Ascii` so dispatch hubs
  drop the empty cell cleanly.
- To add a new widget variant: add the renderer in `widgets/foo.rs`,
  match its name in the relevant dispatch hub, document it in this
  file's [Recognized widgets](#recognized-widgets-per-segment) table.

### Why `agents_visual` has no symmetric dispatch hub

The `*_visual` segments aren't fully symmetric. `context_visual`
and `quota_visual` flow through `render_context_visual` /
`render_quota_visual` hubs; `agents_visual` doesn't. Its spec parser
(`AgentVisualSpec::parse`) is called inline from
`activity/builder.rs`.

This is intentional. Agent rendering is inherently multi-cell —
description, model tag, elapsed — assembled into a row, not a single
`+`-joined string of swappable widget cells. The hub abstraction
(parse spec → run each widget → join with spaces) doesn't model the
agent-row structure cleanly. Forcing it through a hub would either
flatten the row (losing the per-cell budgeting the activity builder
does today) or invent a richer hub contract that only `agents` would
ever use.

Treat the asymmetry as a feature: hubs cover segments composed of
single-cell widgets; agents stays inline because its row isn't that
shape. If a third agent rendering flavour appears (e.g.
`agents_visual = "graph"`), revisit — but the visual-spec abstraction
has hit its useful ceiling for this segment.

### Why ledger ignores `pane_tonal_strata`

`tonal_strata = false` silences the tier-2 separator tint in flat
layouts (see `tinted_palette` in `layout.rs`). Ledger doesn't read
this flag — its TAG column is the rhythm anchor, and stacking a
strata tint on top of the per-row TAG would compete with that
structure rather than reinforce it.

If you toggle `tonal_strata = false` and the ledger output looks
unchanged, that's by design. For all other layouts the tint applies
normally.

---

## Width handling

When the auto-detected `terminal_width` (via `COLUMNS` env or `ioctl`)
is narrower than `layout.min_width`, the active frame is bypassed
entirely and rows render flat — the binary will not output a half-
collapsed frame.

`cc_margin` cols are subtracted from the detected terminal width before
layout. The default `cc_margin = 4` is the empirically verified safe
value for Claude Code 2.1.119; CC allocates the statusline a sub-region
~1–4 cols narrower than the raw terminal, and lines at exactly the raw
width trigger wrap and collapse the multi-line render to a single
visible line.

Layouts may apply additional per-segment width gating inside their
dispatch (e.g. dropping `sparkline` from the spec below a threshold).
These limits are layout-internal; the user-supplied spec is the
*target*, not the guarantee.

`rail`/`anchor` are single-row, so there is no height ladder; instead
they drop cells in priority order (**version → cost → cwd → git →
effort**) until the row fits — `rail` keeps **model + ctx**, `anchor`
keeps the **capsule (model) + ctx** — then bypass to flat `none` below
`min_width` or if even those survivors overflow.

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

### Gauge visual: marks-on-track (1.1.0)

The `gauge` widget is bracketless and uses position-encoded threshold
marks rather than per-cell colour zones: `▰` for filled cells, `─` for
empty, `·` at threshold positions on the empty portion only (a mark
falling inside the filled region is hidden by fill — by design). Ascii
mode swaps to `=` / `-` / `:`. Caller supplies `(pct, width, marks,
fill_color, palette, mode, color)`; `width` is the visible cell count.
Both CTX and quota route through this single widget — quota passes
fixed marks `[50, 85]`, CTX passes `ThemePalette::ctx_marks()` (`[55,
70]`). The pct text and the `(resets …)` countdown always render
alongside the gauge — opting into `gauge` adds visualisation, never
strips information.
