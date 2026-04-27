# Design review: pane.style → layout taxonomy

- **Platform:** CLI / statusline
- **Primary job:** clean up the `[pane].style` config axis so user-facing
  choices read as composable orthogonal dimensions (layout × display × theme ×
  segments) rather than the current hybrid where v2 layouts silently bypass
  the `display.icons` toggle.
- **Reference:** existing `cc-pulseline` palette + glyph discipline
  (`docs/theme-palette.md`, `src/render/icons.rs`).
- **Source reviewed:** `src/config.rs` TOML template, `src/render/pane.rs`,
  `src/render/frames/v2/{cockpit,console,flightstrip,shared}.rs`.
- **Date:** 2026-04-27

## Critique

### Hierarchy
The TOML user sees today implies four orthogonal axes:
`[display]` (visual rendering) · `[colors]` (overrides) · `[segments.*]`
(content selection) · `[pane]` (layout). That mental model is correct **for
v1 frames**. The v2 layouts (cockpit / console / flightstrip / auto) break it:
they own their own pipeline (`render_frame` short-circuits on
`PaneStyle::is_v2()` at `layout.rs:42`) and emit hardcoded glyphs that don't
consult `display.icons`. So the hierarchy in the config file is a *partial
truth* — the layout dimension is leaking into the display dimension.

### Affordance
The TOML comments still read:
```
# v1 frame styles (stable, backward-compatible):
# v2 layout styles (new, recommended) — coming next release:
```
That phrasing telegraphs an internal code-org distinction (v1 vs v2) into the
user surface. From the user's POV they're picking *one layout*; the v1/v2
split has no meaningful affordance — there's no breaking-change to negotiate
because v1 strings still parse identically.

### Density & rhythm
The four axes themselves are well-separated. The `[pane]` block is the only
one that has internal hierarchy collapse: `style` mixes two semantically
different kinds of choice (frame chrome vs layout) under one key.

### Consistency
Hard inconsistencies between v1 and v2:

| Element | v1 honors `icons`? | v2 honors `icons`? |
|---|---|---|
| Tool prefix (`▶` / `>`) | ✅ | ✅ |
| Completed check (`✓` / `[done]`) | ✅ | ✅ |
| Agent prefix (`A:` / glyph) | ✅ (text-only by design) | ❌ **hardcoded `A:` in `shared.rs:304` and `console.rs:237`** |
| Box drawing | ✅ | ✅ |
| CTX sparkline (braille) | n/a | ❌ no ASCII fallback |
| CTX/Q5h gauge bar | n/a | ❌ no ASCII fallback |
| Cost arc (`◔◑◕●`) | n/a | ❌ no ASCII fallback |

v2 widgets weren't audited against the existing `icons = false` contract.
That's the source of the "A: text mixed with Nerd Font icons" feeling.

### Accessibility
Browsers and SSH sessions without Nerd Font are a real audience. With `icons =
false`, v2 layouts render half-broken (gauges OK because U+2588 is in
practically every font, but braille sparklines and arc glyphs fall back to
boxes on plain terminals).

### Platform fit
The `cc-pulseline` reference (per `references/cli.md` line 92) is explicit:
> Icon = value color rule. Icons never get independently dimmed.
> Nerd Font / Powerline glyphs — great when available, **must have ASCII fallback**.

v2 violates the second clause for sparkline / gauge / arc / agent prefix.

## Adjustments

### Must
- **Rename `[pane]` → `[layout]` with `name` field; drop v1/v2 from comments**
  (Hierarchy + Affordance). User-facing TOML becomes:
  ```toml
  [layout]
  name = "cockpit"     # none | zones | grid | cards | sections |
                       # cockpit | console | flightstrip | auto
  width_mode = "auto"
  min_width = 60
  max_width = 160
  tonal_strata = true
  ```
  Internal Rust enum keeps `V1*` / `V2*` for module organization. Backward
  compat: `parse_pane_style` already accepts the bare strings, just add
  `[pane]` as an accepted-but-deprecated alias for `[layout]` for one release.

- **v2 agent prefix must honor `display.icons`** (Consistency). Replace the
  literal `colorize("A:", ...)` in `shared.rs:304` and `console.rs:237` with
  `glyph(NF_AGENT, "A:")` using a new icon constant in `render/icons.rs`. Pick
  a single Nerd Font codepoint and own it (suggestion: `\u{f0091}` md-robot
  or `\u{f0e6f}` mdi-account-outline — visually distinct from existing tool
  / completed glyphs). Verify ascii fallback still produces `A:`.

- **Cost arc must honor `display.icons`** (Consistency). When `icons = false`,
  fall back to text rate suffix (`$4.56 (1.5/h)`) instead of `◔◑◕●`.

### Should
- **Sparkline behind a toggle** (Density). Half the audience finds it
  insightful, half finds it noise. Add `show_ctx_sparkline = false` to
  `[segments.budget]` (default off). When on, gated on `display.icons`
  because ASCII has no equivalent — degrade to omitting the sparkline rather
  than hacking a non-braille version.

- **Q7d in v2 layouts** (Affordance). User config has `show_seven_day = true`
  but cockpit / console / flightstrip ignore it (silent drop). Either render
  Q7d alongside Q5h in console (there's room — see "CTX 右邊空白" question)
  or drop `show_seven_day` from the schema for v2 layouts and document that
  v2 only surfaces Q5h. Ignoring an opt-in flag is the worst option.

- **Document the layout × display matrix.** A small table in
  `docs/pane-styles.md` (already exists) showing which glyphs each layout
  emits and how each degrades under `icons = false`. Future widgets must
  populate this table or be rejected.

### Could
- **Rename `style` → `layout` in CLI flags + `--print` output too** for
  consistency with the section rename.
- **Group themes by family** in the `--preview` output once `pulseline-aurora`
  is the v2 flagship (e.g. "v2-tuned: aurora; v1-tuned: tokyo-night,
  echo-sub-zero, ..."). Pure ergonomics, no functional change.

## Taxonomy proposal (the user's "composable axes" vision)

Four orthogonal axes after the rename. Each row is one user choice:

| Axis | TOML | What it controls | v1 layouts | v2 layouts |
|---|---|---|---|---|
| **Layout** | `[layout].name` | which renderer fires | none/zones/grid/cards/sections | cockpit/console/flightstrip/auto |
| **Display** | `[display]` | theme + variant + icons | honors `icons` | **must honor `icons` (post-fix)** |
| **Colors** | `[colors]` | per-color overrides | applied | applied |
| **Segments** | `[segments.*]` | what content shows | applied | applied + opt-in L2 |

After the must-fixes, every (layout × display) pair becomes valid:
`layout = "console"` + `display.icons = false` → console frame with ASCII
prefixes, no sparkline, gauge in pure block characters (already ASCII-safe),
no arc glyph.

## V2 widget classification (Q3 from the user)

Each widget answers: is this a **standalone toggle**, or **layout-owned standard**?

| Widget | Type | Reasoning |
|---|---|---|
| Sparkline (CTX history braille) | Toggle (`show_ctx_sparkline`, default off) | Polarizing; pure-NF; Phase B added it without consultation |
| Gauge bar (CTX/Q5h) | Layout-owned standard | Natural visual encoding of `show_context` / `show_quota`; ASCII-safe (block chars in every font) |
| Cost arc | Layout-owned standard, gated on `icons` | Same channel as `$X.XX`, just enriched; off when ASCII |
| Agent glyph prefix | Layout-owned standard, gated on `icons` | Visual consistency with tool prefix `▶`; v1 keeps text-only by tradition |
| Tools tape | Layout-owned standard | Already aligned across v1/v2 |
| Frame box drawing | Layout-owned (per-layout chrome) | Each layout chooses its own frame; not user-toggleable |

Only **one** new toggle (sparkline). The rest are standards that need a glyph
audit, not new config knobs.

## Open questions

1. Field name preference: `[layout].name = "cockpit"` or `[layout].layout =
   "cockpit"`? Section + field repetition is awkward but `name` is also
   generic. (Recommendation: `name`.)
2. Deprecate `[pane]` immediately or accept as alias for one release?
   (Recommendation: alias for one release; emit one-time stderr warning.)
3. Pick the Nerd Font glyph for agent prefix (`󰚩` md-robot / `󱙺` md-robot-outline / `` octicon-rocket / `` mdi-cog-pause)?
4. Sparkline default — `false` (conservative) or `true` (showcase v2)?
5. Q7d in v2 — render alongside Q5h (where? cost row? new row?), or document
   v2 as Q5h-only and drop the toggle?

## Next step

Pick (1), (2), (3), (4), (5) above; I'll execute the rename + glyph audit +
sparkline toggle as a single PR. Estimated diff: ~300 LOC across `config.rs`,
`render/icons.rs`, `render/frames/v2/shared.rs`, `cockpit.rs`, `console.rs`,
`flightstrip.rs`, plus 3-4 new tests for the icons-off matrix.
