# Design brief: tonal strata — palette-native 2-tier chrome (redesign of proposal 1)

- **Platform:** CC statusline (CLI/TUI sub-region)
- **Primary job:** Give the eye a reliable "state vs activity" split that survives **every** shipped theme — no collapse, no categorical misuse, no design-language fragmentation
- **Reference:** Existing `ThemePalette` (26 semantic fields, 9 shipped themes, dark + light variants); `references/cli.md` color-tier principles
- **Date:** 2026-04-25
- **Supersedes:** ad-hoc mapping in proposal 1 (Identity=stable_blue / Config=separator / Budget=structural / Activity=active_cyan)

## Why this brief exists

Proposal 1 shipped a 4-way mapping from `LineKind` to existing palette fields. An audit across all 9 shipped themes (this doc, §"Audit") proves it fails: **no theme produces 4 visually distinct tiers, and 4 of 9 collapse to only 2 tiers**. Root cause: the mapping borrowed *categorical* semantic colors (`stable_blue`, `active_cyan`) and used them as *ordinal* chrome. Categorical colors don't stack into a reliable gradient across themes.

The fix is not a re-tune of ad-hoc values. It's to commit that **"2-tier chrome strata" is a first-class design contract** the palette must satisfy — then extend the palette accordingly.

## Audit — what proposal 1 actually delivered

| Theme | Ident | Conf | Budg | Acti | Distinct tiers |
|---|---|---|---|---|---|
| aburaya-twilight | 67 | 239 | 109 | 80 | 3 (Ident≈Acti) |
| cnc-telemetry | 109 | 235 | 238 | 66 | 3 (Conf≈Budg) |
| cyberdeck-hud | 246 | 239 | 242 | 51 | 3 (Conf≈Budg) |
| echo-sub-zero | 109 | 239 | 244 | 110 | **2** |
| mako-reactor | 39 | 239 | 244 | 43 | **2** |
| matte-carbon-neon | 39 | 234 | 240 | 51 | **2** |
| stark-hud | 39 | 236 | 240 | 51 | **2** |
| titanium-precision | 109 | 236 | 240 | 74 | 3 (Conf≈Budg) |
| tokyo-night | 111 | 238 | 103 | 117 | 3 (Ident≈Acti) |

Zero themes reach 4 tiers. Half collapse to 2. The design promise was never honored.

---

## System — what exists in ThemePalette today

| Class | Fields | Shape |
|---|---|---|
| **Emphasis tier** | primary / secondary / structural / separator | **Ordinal 4-step ladder** — the only ordered gradient in the palette |
| Alert tier | alert_red / alert_orange / alert_magenta | Categorical (severity families) |
| Active tier | active_cyan / active_purple / active_teal / active_amber / active_coral | Categorical (5 "running" roles) |
| Stable tier | stable_blue / stable_green | Categorical (2 "settled" roles) |
| Indicator tier | 7 L2 icon colors | Categorical |
| Accent | completed_check / cost_base/low/med/high | Categorical + 1 cost ladder |

**Observation:** only one ladder exists (`emphasis`), and it is designed for **value emphasis**, not chrome grouping. Reusing emphasis for chrome collides with the "icon/value color" rule — a row's `|` separator shouldn't compete in brightness with its own values.

Therefore: **a chrome strata ladder does not exist in the palette today. To deliver the promise, we add it.**

---

## Design principles for the new strata tier

1. **2 tiers, not 4.** The only genuine stability break in statusline data is `state vs activity` (Identity/Config/Budget vs Tools/Agents/Todos). Any 4-way split imposes structure the data doesn't have. CC's own input-box ruler idiom encodes the same binary split.
2. **Chrome, not value.** Strata colors live *under* emphasis tiers — dimmer than `secondary`, similar to or a touch above `separator`/`structural`. They tint the `|` between segments; they never render values.
3. **Theme-authored, not formula-derived.** Each theme picks its own pair of chrome colors by hand. The 26-field palette is already hand-authored; strata joins that tradition.
4. **Lint-enforced contrast floor.** Any new theme must declare a Δ ≥ 3 on the ansi256 scale between `strata_state` and `strata_activity` (dark and light). Violations fail `cargo test` — no collapsed themes ever ship.
5. **Graceful fallback for custom themes.** User-authored themes in `~/.claude/pulseline/themes/` that omit strata fields fall back to `separator` + `structural` with a `warn!` log. Existing third-party customizations keep working; nothing silently breaks.

---

## The spec

### New palette fields

Add to `ThemePalette` (in `src/render/color.rs`):

```rust
pub struct ThemePalette {
    // …existing 26 fields…
    pub strata_state: String,     // L1 Identity, L2 Config, L3 Budget, Quota — baseline chrome
    pub strata_activity: String,  // L4+ Tools/Agents/Todos — lifted chrome
}
```

Add to each theme JSON's `palette_mapping` (dark-variant defaults) **and** to `light_emphasis` (light-variant overrides):

```json
{
  "palette_mapping": {
    "strata_state": 239,
    "strata_activity": 66,
    // …existing fields…
  },
  "light_emphasis": {
    "strata_state": 253,
    "strata_activity": 247,
    // …existing light-variant fields…
  }
}
```

### Per-theme value proposals (seed values)

Values below are *initial* theme-coherent proposals. Each one respects the theme's existing color story. Mark any that don't feel right in a live preview — final values go through a visual review.

| Theme | dark.state | dark.activity | light.state | light.activity | Rationale |
|---|---|---|---|---|---|
| **aburaya-twilight** | `239` | `66` | `253` | `247` | Dark: separator → muted dragon-teal mid; steam-between-worlds feel. Light: baseline separator → soft neutral drop. |
| **cnc-telemetry** | `235` | `66` | `250` | `245` | Dark: deepest carbon → anodized teal (lines up with theme's patina language). Light: neutral 1-step. |
| **cyberdeck-hud** | `239` | `60` | `250` | `245` | Dark: separator → dim neon-violet chrome (HUD dimmed data-trace), not the bright `51` which is used for `active_cyan`. Light: neutral. |
| **echo-sub-zero** | `239` | `244` | `253` | `248` | Minimalist signaling theme — deliberately tight Δ. Uses existing structural as activity (since structural isn't otherwise used as chrome). |
| **mako-reactor** | `239` | `60` | `252` | `245` | Dark: separator → dim Shinra-steel mid (not the bright `43` Mako-cyan which is semantic). Light: neutral. |
| **matte-carbon-neon** | `234` | `240` | `253` | `247` | Matte/industrial: deliberate 1-tier lift using existing structural. Theme's restraint comes first. |
| **stark-hud** | `236` | `59` | `252` | `245` | Dark: separator → dim arc-reactor mid (reserved `51` for semantic active). Light: neutral. |
| **titanium-precision** | `236` | `240` | `252` | `247` | Utilitarian: same move as matte-carbon — use existing structural as activity chrome. |
| **tokyo-night** | `238` | `103` | `253` | `246` | Dark: separator → theme's existing `structural` (103 is the theme's iconic blue-gray); light: standard neutral step. |

**Note on values `66`, `60`, `59`:** these are in the ansi256 "dim cool greys/teals" range. They're muted enough to read as chrome but distinct from `separator` (~235-239). None of them currently exist in any shipped theme's mapping — which is the point. Chrome should not re-use colors that already carry semantic meaning.

### Runtime wiring

1. `tinted_palette(p, kind, tonal)` in `src/render/layout.rs` is the existing helper (shipped in proposal 1). **Replace the 4-arm match** with a 2-arm match:

   ```rust
   out.separator = match kind {
       LineKind::Activity => base.strata_activity.clone(),
       // Identity / Config / Budget / (implicit) Quota all treat as state
       _ => base.strata_state.clone(),
   };
   ```

2. Proposal 1's config field `pane.tonal_strata: bool` is **reused as-is**. It still means "do the chrome tint on the `|`". Nothing changes for users who already toggled it.

3. Per-color `[colors]` override in `pulseline.toml` gains two new optional keys:
   ```toml
   [colors]
   strata_state = 239
   strata_activity = 66
   ```

### Fallback for custom themes

In `src/render/color.rs:resolve_palette`, if a custom theme's `palette_mapping` omits `strata_state` / `strata_activity`:

- `strata_state` → fall back to `separator`
- `strata_activity` → fall back to `structural`
- Emit a `warn!` on first resolution (not on every render) pointing at the theme file

This keeps custom themes loadable but tells authors what to add.

### Lint test — contrast floor

Add `tests/theme_strata_contrast.rs`:

```rust
#[test]
fn every_shipped_theme_has_min_strata_delta() {
    for theme_name in BUILT_IN_THEMES {
        for variant in ["dark", "light"] {
            let p = resolve_palette(theme_name, Some(variant), &ColorsConfig::default());
            let state = parse_ansi_code(&p.strata_state);
            let activity = parse_ansi_code(&p.strata_activity);
            let delta = (state as i32 - activity as i32).abs();
            assert!(
                delta >= 3,
                "{theme_name} {variant}: strata_state={state} strata_activity={activity} Δ={delta}; need Δ≥3"
            );
        }
    }
}
```

Future theme additions that collapse strata fail CI before merge.

---

## Implementation plan

Work split so each step is independently verifiable:

### Step 1 — Schema (≈30 min)

- Add `strata_state`, `strata_activity` to `ThemePalette` struct
- Add fields to `ThemeJsonPreset` + light-variant structs
- Add to `ColorsConfig` override + `merge_configs` field list (2 `merge_color!` lines)
- Add fallback logic in `resolve_palette` for missing fields
- Update `--palette-map` output to include the new fields

### Step 2 — Seed values for 9 built-in themes (≈1-2 hours + visual review)

- Update each of `src/themes/*.json` with the proposed values
- Run `cargo test` — new contrast lint catches any typo or too-small Δ
- Visual review: `cc-pulseline --preview` across all themes, with a fixture payload that has both state and activity lines — confirm the chrome lift feels right, not loud
- Adjust any value the user rejects after seeing it live

### Step 3 — Runtime wiring (≈15 min)

- Swap `tinted_palette` to the 2-arm match above
- Delete the old 4-way color mapping from docs / comments
- Update `tonal_strata` test cases in `tests/tonal_strata.rs` to match the new mapping:
  - `L1 (Identity)` → `strata_state`
  - `L2 (Config)` → `strata_state`
  - `L3 (Budget)` → `strata_state`
  - Activity lines → `strata_activity`

### Step 4 — Docs + migration note (≈20 min)

- Update `docs/theme-palette.md` with the new 28-field count + strata design rationale (this doc becomes the permanent design record; the spec section above gets pasted in)
- Add a CHANGELOG entry
- `--init` TOML template gets the updated `[pane]` comment + optional `[colors]` strata override example
- Update `preview-theme` skill if present so new theme authors see strata in their preview output

### Step 5 — Verification

- `cargo fmt` / `cargo clippy -- -D warnings` clean
- `cargo test` — 199+ existing tests pass, +1 contrast lint, +3 updated tonal_strata tests
- Release build + live session visual check

---

## What ships, what doesn't

### In scope for this redesign

- Two new palette fields: `strata_state`, `strata_activity` (both variants, all 9 themes)
- 2-arm simplification of `tinted_palette`
- Contrast floor lint
- Per-color override support
- Custom-theme fallback path

### Out of scope (explicit)

- **No 4-way mapping.** `LineKind` stays as-is in code, but the binary grouping (state / activity) is enforced at the `tinted_palette` layer. Anyone later arguing for a 4th tier must bring theme-author consent for all 9 themes.
- **No content/layout changes.** Row positions, segments, separator characters, truncation logic — unchanged.
- **No deprecation of emphasis tier fields.** `separator` / `structural` / `secondary` / `primary` remain exactly as used today; strata is a new layer above them.
- **No changes to Activity decoration glyphs** (`T:` / `A:` / `TODO:` / `✓`). Those are orthogonal to chrome.

---

## Open questions for review

1. **Should Quota lines inherit `state` or `activity`?** Spec above puts them in `state` (they're derived budget data, change only when rate_limits refresh). Happy to flip to `activity` if you prefer — one-line change.
2. **Seed values** — I'll propose live; you review before we commit to all 9 themes. Any theme's dark.activity value you veto we re-pick together.
3. **Custom-theme warning policy** — fall back silently vs. log once. Proposal: log once (authors need the signal; users don't need noise).
4. **Light-variant strata** — I kept them tight (Δ 4-6) because light-bg terminals are low-contrast by nature. If you want bolder light-variant lift, we can push dark.activity-analogous hues (e.g., aburaya light.activity = 109 instead of 247). Visual call.

---

## Why this is the right answer (and not an over-commitment)

- **Theme authors already hand-author 26 colors per variant.** Two more is ~8% more authoring. Not a burden.
- **The contrast floor lint makes the design contract self-enforcing.** Once in, it can't silently rot.
- **Fallback path protects existing customizations.** Users who already wrote custom themes get a warning and a sensible default, not a broken statusline.
- **2 tiers is the *real* split**, not an arbitrary reduction from 4. Every further refinement (proposal 2's segment budget, proposal 3's style collapse) can lean on this binary state/activity distinction cleanly.

This is what "做出有品質感" looks like when applied to a CLI design system: a small but uncompromising palette contract that every component inherits, rather than a clever per-line mapping that fragments under variation.

---

## Next step

Say `do it` and I'll execute Step 1 + Step 2 in one pass (schema change + all 9 theme value seeds), then pause before Step 3 so you can visually review the seed values on each theme with `cc-pulseline --preview` before we swap the runtime.
