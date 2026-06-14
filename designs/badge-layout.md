# Badge Layout — Design Spec

> Status: **proposed / experimental** · Author: design session 2026-06-14
> Decision locked with user: build the **unified design, 3 degrade tiers**,
> single cool accent = **deep teal (ANSI 30)**.

## 1. What & why

A new, additive `LayoutStyle::Badge` — the existing four layouts
(`None` / `Compact` / `Console` / `Ledger`) are untouched. It renders the
user's data domains as **discrete two-tone badges** ("pills" / shields.io
chips) across **3 rows**, each domain its own visible group, with generous
whitespace between groups.

### The structural leap (why this is "煥然一新", not a restyle)

Every existing layout is **foreground-only**: `colorize()` emits
`\x1b[38;5;Nm{text}\x1b[0m` — colored glyphs on the terminal's default
background, separated by `|` pipes or box-drawing chrome. The eye parses
that as *text to read sequentially*.

Badge introduces the renderer's **first background-fill primitive**
(`\x1b[48;5;Nm`). A filled, capped shape reads **pre-attentively as a
discrete object**. That figure/ground inversion is the whole point — the
prior two rejected directions failed precisely because they were still
fg-only text rows.

## 2. Scope (locked)

Fields shown — **only these**, no tools/agents/todo/cost rows:

| Row | Group | Cells |
|-----|-------|-------|
| 1 | **IDENTITY** | model · effort · version · cwd · git (branch + `*` dirty + `↑n↓n`) |
| 2 | **BUDGET** | tokens in · out · cache `%` · CTX `%` (+ `used/total`) |
| 3 | **QUOTA** | 5h (`%` + reset) · 7d (`%` + reset) |

3 rows fixed (inside the user's 3–4 row budget). Each row = one domain =
one visible group. Within a row, badges float with a small gutter; group
sub-clusters separated by a wider moat (e.g. identity → cwd/git).

## 3. Three render tiers (automatic, from existing config axes)

The "unified design, 3 tiers" maps cleanly onto axes the renderer already
has — `(color_enabled, glyph_mode)`. No new config knob needed to pick a
tier.

| Condition | Tier | Edge treatment |
|-----------|------|----------------|
| `color_enabled && glyph_mode == Icon` | **DRIFT** | rounded powerline caps `` `` (fg = pill bg) + bg fill |
| `color_enabled && glyph_mode == Ascii` | **STENCIL** | capless reverse-video slabs; label/value fused by the `236→238` bg step (no cap glyph) |
| `!color_enabled` (`NO_COLOR`) | **ASCII** | `[label value]` plain brackets, no bg, no caps |

DRIFT degrades to STENCIL when no Nerd Font (caps would be tofu, but the
bg fill still reads as a badge); STENCIL degrades to ASCII when color is
off. One coherent design, three faces.

## 4. Color scheme (deep teal 30, restrained)

Grayscale everywhere; **one** cool accent on **exactly two domains**
(budget + quota, incl. cache); warm **only** as a near-limit alarm.

| Role | ANSI 256 | Used for |
|------|----------|----------|
| label bg | `236` | every label half |
| value bg | `238` | every neutral value half (identity, tokens, ctx<70) |
| label fg | `245` | label text (dim) |
| value fg | `252` | value text (bright) |
| **accent bg** | **`30`** (deep teal) | value half of **cache %, CTX %, quota %** when healthy |
| accent fg | `231` | text on teal |
| warn bg | `173` (terracotta) | value half when a window crosses its **warn** line |
| crit bg | `130` | value half at **critical** |
| warn/crit fg | `236` | dark text on warm (reverse-video reads crisp) |
| git fleck fg | `173` | `*` dirty + `↑n↓n`, inline in the otherwise-neutral git pill |

色階 (graduated shades) is expressed **only** as the grayscale ramp
`236 → 238 → 245 → 252` (bg step + fg lift) — never as hue. The teal is a
single fixed stop, not a gradient.

### Thresholds (reuse existing ladders)

- CTX value half: teal until `used_percentage ≥ 70` (`CTX_CRITICAL`),
  then warm. (`CTX_WARN_THRESHOLD = 55`, `CTX_CRITICAL_THRESHOLD = 70`.)
- Quota value half: teal until `≥ 50` (warn) → `173`; `≥ 85` (crit) → `130`.
  (Matches `color_for_quota_pct` marks `[50, 85]`.)
- Cache value half: teal when present; uses the existing creation-aware
  hit-rate semantics for its *number*, but the badge bg stays teal (cache
  is "live", not "alarming").

### Alert behavior (folded in from the HUDLINE concept)

When a value half trips its threshold, the **whole value half swaps to a
filled warm block** (bg `173`/`130`, fg `236`) — the alarm is a colored
*object*, not a recolored digit. Identity badges never change color
(except the inline git fleck).

## 5. Palette fields (backward-compatible)

Add badge-specific fields to `ThemePalette` with `#[serde(default = ...)]`
so **existing theme JSONs parse unchanged** and inherit the deep-teal
defaults (no need to edit every `src/themes/*.json`). Per coding-style:
"Serialized types: `#[serde(default)]` on every field for backward
compatibility."

```
badge_label_bg   default "236"   badge_value_bg  default "238"
badge_label_fg   default "245"   badge_value_fg  default "252"
badge_accent_bg  default "30"    badge_accent_fg default "231"
badge_warn_bg    default "173"   badge_crit_bg   default "130"
badge_warn_fg    default "236"
```

These flow through the existing `[colors]` per-color override mechanism, so
a user can retune the accent (`badge_accent_bg = 23`) without a new config
section. Light-variant defaults: bump bg shades lighter
(`254 / 252` label/value, accent stays teal) — picked in `light_emphasis`
resolution; deferred to a follow-up if light themes aren't in v1 scope.

## 6. Rendering primitive

`render/color.rs` — add one helper next to `colorize()`:

```rust
/// Foreground + background 256-color run, reset after. `fg`/`bg` are the
/// numeric codes (e.g. "30"); empty/"" bg means no background fill.
pub fn colorize_badge(text: &str, fg: &str, bg: &str, enabled: bool) -> String {
    if !enabled { return text.to_string(); }
    if bg.is_empty() {
        format!("\x1b[38;5;{fg}m{text}{RESET}")
    } else {
        format!("\x1b[38;5;{fg}m\x1b[48;5;{bg}m{text}{RESET}")
    }
}
```

`RESET` (`\x1b[0m`) clears both planes, so **one reset per badge** prevents
bg bleed into the gutter. `strip_ansi()` already consumes `\x1b[48;5;Nm`
(it eats any `\x1b[…<alpha>`), so `visible_width()` / truncation math is
unaffected — **verified**.

## 7. Pipeline: Badge owns its pipeline (like Ledger)

Badge does **not** flow through `apply_pane`; it renders directly from
`render_frame`, because the badge rhythm (bg runs, caps, per-tier edge) does
not compose via the flat-row chrome decorator — same rationale Ledger uses.

New module `src/render/frames/badge.rs` with:
`pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String>`

It builds 3 rows, picks the tier from `(color_enabled, glyph_mode)`, and
emits badges via `colorize_badge`.

## 8. Touch-points (verified against current code)

Compiler-enforced (exhaustive `match` on `LayoutStyle` — omitting an arm
fails the build):

1. `src/render/pane.rs:15` — add `Badge` variant to `LayoutStyle` (+ doc).
2. `src/render/pane.rs:78` — `apply_pane`: add `Badge` to the pass-through
   arm (`None | Compact | Ledger | Badge => return lines`).
3. `src/render/layout.rs:49` — `render_frame` dispatch:
   `LayoutStyle::Badge => return frames::badge::render(frame, config, palette)`.
4. `src/render/layout.rs:264` — `style_overhead` match: `Badge => 0`
   (unreached for Badge — returns early — but must compile).
5. `src/render/frames/mod.rs:50` — `default_visuals_for`: add `Badge` arm
   (CTX `text`, quota `text`; badge.rs reads token/ctx/quota directly, so
   visual specs are mostly informational here).

Not compiler-enforced (silent fallback to default if missed):

6. `src/config.rs:1461` — `parse_layout_name`: `"badge" => LayoutStyle::Badge`.
7. `src/render/frames/mod.rs:19` — `pub mod badge;`.

Config-template docs (Config Layer Pattern, non-fatal if skipped):

8. `default_config_toml()` / `docs/layouts.md` — document `name = "badge"`
   and the badge palette fields.

## 9. Width fallback (owned, like Ledger)

Badge is fixed 3 rows and **exempt** from the flat height-degradation
ladder (like Ledger). Its own width policy, in order:

1. drop CTX `used/total` (keep `%`)
2. drop quota reset countdowns (keep `%`)
3. middle-truncate cwd (`~/AI/…/cc-pulseline`)
4. below a minimum width → **fall back to `Compact`** (mirrors
   Ledger→Console), restoring `terminal_width` first.

Badge always sizes against `terminal_width - pane_cc_margin` (the adjusted
width `render_frame` already computes).

## 10. Tests (`tests/badge_layout.rs` + axes)

- DRIFT tier (color + Icon): asserts a badge bg code (`48;5;30`) is present
  on the CTX value and absent on identity values; one `RESET` per badge.
- STENCIL tier (color + Ascii): asserts bg fills present **and no powerline
  cap glyphs / no Unicode block chars** — satisfies the existing
  `tests/display_axes.rs` ascii catch-net
  (`ascii_mode_emits_no_unicode_block_chars_across_every_layout`).
- ASCII tier (`color_enabled = false`): asserts plain `[...]`, **no**
  `48;5` / `38;5` codes at all.
- Alert: feed quota `used_percentage = 88` → assert warm bg (`48;5;173`) on
  that value half, teal elsewhere.
- Absent data (`current_usage = null`, no `rate_limits`): budget/quota
  values render `--` inside neutral badges (no teal, no warm); geometry
  stable.
- Width fallback: narrow `terminal_width` → assert used/total then reset
  text drop; very narrow → assert Compact fallback.

## 11. Out of scope (v1)

- Tools / agents / todo / cost rows.
- Light-theme bg ramp (defaults provided; fine-tune later).
- A 4th optional trend row (could revisit; user chose 3-row budget).
- New gradient engine (色階 stays the grayscale ramp + one teal stop).
