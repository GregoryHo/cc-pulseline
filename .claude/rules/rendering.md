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
