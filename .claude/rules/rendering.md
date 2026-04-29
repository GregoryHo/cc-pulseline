# Rendering Rules

## Color System

- ALL rendering functions take `&ThemePalette` (via `config.palette`) + `color_enabled` as parameters
- ALWAYS use `colorize()` — never write raw ANSI escape codes
- Define new colors as `const` in `render/color.rs`
- Define new icons as `const` in `render/icons.rs`
- Use `glyph(icon, ascii)` for dual-mode (Nerd Font / plain ASCII) switching

## Emphasis Tiers

Four tiers that vary by dark/light theme:
- **Primary** — values, important data (brightest)
- **Secondary** — supporting info (model tags, counts)
- **Structural** — labels, static text (dimmest text)
- **Separator** — pipe characters between segments

Thread palette references from `render_frame()` down to every format function. The palette is accessed via `config.palette` in `render_frame()` and passed as `&ThemePalette` to format functions. Never call `resolve_palette()` in leaf functions.

## Semantic Colors

Semantic colors are fields on `ThemePalette` (e.g., `p.stable_blue`, `p.alert_red`). Legacy `pub const` values are retained for test assertions but new code should use palette fields:
- Icon color = value color (icons are NEVER independently dimmed)
- COMPLETED_CHECK (67) for `✓Name` completed items

## Line Layout

- **L1-L3**: always render (identity, config, budget) — core metrics
- **L4**: tool activity line (running + completed counts)
- **L5+**: agent activity lines (active first, then recent completed)

Activity lines (L4+) are dropped first during width degradation.

## Width Degradation Order

When `terminal_width` is set and content exceeds it:
1. Drop activity lines (L4+)
2. Compress Line 2 (shorter labels)
3. Truncate core lines (L1-L3)

## Context Thresholds

- `CTX_WARN_THRESHOLD` = 55% — switches to warning color
- `CTX_CRITICAL_THRESHOLD` = 70% — switches to critical color

## Adding a New Segment

1. Add data field to the appropriate `LineNMetrics` struct in `types.rs`
2. Add `show_*` toggle following the Config Layer Pattern (7 places)
3. Write format function in `render/layout.rs` taking `&ThemePalette` + `color_enabled`
4. Wire into the appropriate line's format function in `layout.rs`
5. Test with `color_enabled: true` AND `color_enabled: false`
6. Verify width degradation still works

## Adding a Widget Variant via Visual Spec

When adding a new widget *variant* for the context or quota segment, wire it through the dispatch hub — never call `widgets::*::render` directly from a layout, or that widget choice can't reach other layouts.

1. Add the renderer in `widgets/foo.rs` with the canonical signature `fn render(data, …, mode: GlyphMode, palette: &ThemePalette, color_enabled: bool) -> String`. Return `""` from incompatible modes (e.g. icon-only widget under `Ascii`) so the dispatch hub drops the cell cleanly.
2. Match its name in the relevant dispatch hub in `render/frames/shared.rs`:
   - context: `render_context_visual` (`gauge`, `sparkline`, `text`, …)
   - quota: `render_quota_visual` (`gauge`, `text`, …)
3. Document the new widget name in `docs/layouts.md` "Recognized widgets per segment" table.
4. Decide layout defaults: if a layout should ship the new widget out of the box, edit `frames::default_visuals_for(LayoutStyle)`.
5. Test the new widget across every layout via `tests/display_axes.rs`. The `ascii_mode_emits_no_unicode_block_chars_across_every_layout` catch-net will fail if the new widget leaks Unicode block glyphs under Ascii.

> **Cost / tools visual hubs are still deleted from the cluster
> consolidation** — the `cost_visual` / `tools_visual` config fields
> exist as forward-compat only and currently render no widgets.
> Adding a variant for these segments first requires resurrecting
> their dispatch hub. See `designs/maintenance-debt.md` Item 1.
> The `quota_visual` hub was restored when the F-style gauge bar
> landed.

## Adding a New Widget-Bearing Segment

If introducing a brand-new segment with widget composition:

1. Add the `*_visual: String` field per the Config Layer Pattern (7 places).
2. Add the per-layout default in `SegmentVisualDefaults` and `default_visuals_for` in `frames/mod.rs`.
3. Add an `effective_*_visual()` helper on `RenderConfig` mirroring `effective_context_visual()`.
4. Add a new dispatch hub `render_<segment>_visual` in `frames/shared.rs`.
5. Refactor every layout that renders the segment to call the hub instead of widgets directly.
