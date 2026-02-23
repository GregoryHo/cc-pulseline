# Theme & Color Palette

## Quick Reference

| Category | Colors | Codes |
|----------|--------|-------|
| Alert | Red, Orange, Magenta | 196, 214, 201 |
| Active | Cyan, Purple, Teal, Amber, Coral | 117, 183, 80, 178, 209 |
| Stable | Blue, Green | 111, 71 |
| Indicator | Steel, Sage, Lilac, Amber, Lavender, Teal, Rose | 109, 108, 182, 179, 139, 73, 174 |
| Cost | Base, Low, Med, High | 222, 186, 221, 201 |
| Emphasis (Dark) | Primary, Secondary, Structural, Separator | 251, 146, 103, 238 |
| Emphasis (Light) | Primary, Secondary, Structural, Separator | 234, 240, 245, 252 |

---

# Color Specification

Reference guide for the cc-pulseline 256-color palette, organized by information hierarchy.

## Design Principles

Colors are organized into a **three-tier attention system** (for semantic colors) with a **four-tier emphasis hierarchy** (for gray-scale text) and an **indicator tier** (for L2 metric anchoring):

1. **ALERT** — Demands immediate attention (context >=70%, git dirty, high burn rate)
2. **ACTIVE** — Currently happening, dynamically changing (tools, agents, context 55-69%)
3. **STABLE** — Informational, unchanging context (model, version, branch clean, normal context)

Plus:
4. **INDICATOR** — Muted per-metric accents for L2 icons, providing "visual fingerprints" for quick scanning

This hierarchy guides the eye: red/orange/magenta for urgent issues, mid-saturation for activity, muted grays for stable information, and unique muted accents for L2 metric scanning.

The palette draws inspiration from **Tokyo Night Storm** (`folke/tokyonight.nvim`), using blue-tinted grays for emphasis tiers and the theme's signature blues and purples for semantic colors. Context, cost, and git state colors are preserved as-is for functional clarity.

## Palette

All colors use `\x1b[38;5;{N}m` format.

### Emphasis Tiers

Four-level hierarchy for text and structural elements. Vary by theme; semantic colors do not.

| Tier | Dark | Light | Use |
|------|------|-------|-----|
| **Primary** | 251 | 234 | Reserved (available for high-priority values) |
| **Secondary** | 146 | 240 | Values, counts, data (blue-tinted gray) |
| **Structural** | 103 | 245 | Icons, labels, supporting text (blue-purple, brighter than old 60) |
| **Separator** | 238 | 252 | Punctuation only (\|, (), /) |

**Light theme gap distribution**: 234->240(6), 240->245(5), 245->252(7) -- even distribution for readability. Previously: 236->243(7), 243->246(3), 246->250(4) -- the 3-point gap between secondary and structural was nearly indistinguishable.

### Alert Tier -- Bright, Saturated, Urgent

| Name | Code | Purpose |
|------|------|---------|
| `ALERT_RED` | 196 | Context >=70%, critical states |
| `ALERT_ORANGE` | 214 | Git dirty `*` |
| `ALERT_MAGENTA` | 201 | Burn rate >$50/h |

### Active Tier -- Mid-Saturation, Noticeable

| Name | Code | Purpose |
|------|------|---------|
| `ACTIVE_CYAN` | 117 | Tool activity (Tokyo Night bright cyan) |
| `ACTIVE_PURPLE` | 183 | Agent activity (Tokyo Night magenta) |
| `ACTIVE_TEAL` | 80 | Todo activity |
| `ACTIVE_AMBER` | 178 | Context 55-69% |
| `ACTIVE_CORAL` | 209 | Git ahead/behind |

### Stable Tier -- Muted, Informational

| Name | Code | Purpose |
|------|------|---------|
| `STABLE_BLUE` | 111 | Model identity (Tokyo Night main blue) |
| `STABLE_GREEN` | 71 | Git branch (clean) |

### Indicator Tier -- Muted Per-Metric Accents

Provides unique icon colors for each L2 metric, enabling fast visual scanning. Counts stay `tier.secondary` for data consistency; labels stay `tier.structural`.

| Name | Code | L2 Metric | Visual Rationale |
|------|------|-----------|------------------|
| `INDICATOR_CLAUDE_MD` | 109 | CLAUDE.md | Muted steel -- documentation/config |
| `INDICATOR_RULES` | 108 | Rules | Muted sage -- governance |
| `INDICATOR_MEMORY` | 182 | Memories | Muted lilac -- knowledge/memories |
| `INDICATOR_HOOKS` | 179 | Hooks | Muted amber -- active/intercepting |
| `INDICATOR_MCP` | 139 | MCPs | Muted lavender -- extensions |
| `INDICATOR_SKILLS` | 73 | Skills | Muted teal -- capabilities |
| `INDICATOR_DURATION` | 174 | Duration | Muted rose -- time passage |

### Completed Tool Accent

| Name | Code | Purpose |
|------|------|---------|
| `COMPLETED_CHECK` | 67 | Completed tool checkmark+name -- steel blue, links to active tool cyan |

### Cost Tier -- Rate-Based Dynamic Coloring

| Name | Code | Condition |
|------|------|-----------|
| `COST_BASE` | 222 | Total cost display |
| `COST_LOW_RATE` | 186 | Burn rate <$10/h |
| `COST_MED_RATE` | 221 | Burn rate $10-50/h |
| `COST_HIGH_RATE` | 201 | Burn rate >$50/h |

### Legacy Aliases

For backward compatibility, old names map to the new tier system:

| Legacy Name | Points To | Change |
|-------------|-----------|--------|
| `MODEL_BLUE` | `STABLE_BLUE` | -> 111 (Tokyo Night main blue) |
| `GIT_GREEN` | `STABLE_GREEN` | 71 (unchanged) |
| `GIT_MODIFIED` | `ALERT_ORANGE` | 214 (unchanged) |
| `GIT_AHEAD` | `ACTIVE_CORAL` | 209 (unchanged) |
| `GIT_BEHIND` | `ACTIVE_CORAL` | 209 (unchanged) |
| `CTX_GOOD` | `STABLE_GREEN` | 71 (unchanged) |
| `CTX_WARN` | `ACTIVE_AMBER` | 178 (unchanged) |
| `CTX_CRITICAL` | `ALERT_RED` | 196 (unchanged) |
| `TOOL_BLUE` | `ACTIVE_CYAN` | -> 117 (Tokyo Night bright cyan) |
| `AGENT_PURPLE` | `ACTIVE_PURPLE` | -> 183 (Tokyo Night magenta) |
| `TODO_TEAL` | `ACTIVE_TEAL` | 80 (unchanged) |

**Removed**: `PROJECT_CYAN` (51), `COST_GOLD` (220), `RATE_YELLOW` (226) -- replaced by emphasis tiers and rate-based cost coloring.

## Tier Summary (6 types, ~25 unique colors)

| Tier | Colors | Purpose | Status |
|------|--------|---------|--------|
| ALERT | 3 (196/214/201) | Critical states | Unchanged |
| ACTIVE | 5 (117/183/80/178/209) | Live activity | Unchanged |
| STABLE | 2 (111/71) | Static identity | Unchanged |
| INDICATOR | 7 (109/108/182/179/139/73/174) | L2 metric-specific anchoring | Added |
| Emphasis | 4x2 themes | Gray hierarchy | Light values revised |
| Cost | 4 (222/186/221/201) | Rate-based | Unchanged |

## Element Mapping

### Line 1: Identity (Semantic + Secondary)

```
[STABLE_BLUE(111)]M:model [separator(238/252)]| [secondary(146/240)]S:style [separator]| [secondary]CC:version [separator]| [secondary]P:~/path [separator]| [STABLE_GREEN(71)]G:branch[ALERT_ORANGE(214)]*[ACTIVE_CORAL(209)] up-n
```

- `111` Model: icon+value both STABLE_BLUE (most important identity)
- `146/240` Style/Version/Project: icon+value both tier.secondary (promoted from structural -- these are important session identifiers)
- `71` Git: icon+value both STABLE_GREEN (unless dirty/ahead/behind)
- `238/252` Separators: tier.separator

### Line 2: Config Counts (Indicator + Monochrome Hierarchy)

```
[INDICATOR_CLAUDE_MD(109)]icon [secondary(146/240)]count [structural(103/245)]label [separator(238/252)]| [INDICATOR_RULES(108)]icon [secondary]count [structural]label | ...
```

- `109/108/182/179/139/73/174` Icons: per-metric INDICATOR color (visual fingerprints)
- `146/240` Counts: tier.secondary (the actual data -- most prominent on L2)
- `60/247` Labels: tier.structural (descriptive text)
- `238/252` Separators: tier.separator
- **ASCII mode**: icons are absent, counts and labels use the same hierarchy

### Line 3: Resources & Cost (Mixed)

```
[CTX_*(71/178/196)]CTX:pct% [separator(238/252)]([secondary(146/240)]used[separator]/[secondary]total[separator]) [separator]| [structural(103/245)]TOK I:[primary(251/234)]val O:[primary]val [primary]↗speed C:[primary]val [separator]| [COST_BASE(222)]$total [separator]([RATE_*(186/221/201)]$rate/h[separator])
```

- `71/178/196` Context: icon+pct both use CTX_GOOD/WARN/CRITICAL (semantic, state-driven)
- `60/247` Token labels: tier.structural (I:, O:, C:, R:)
- `251/234` Token values + speed: tier.primary (val_color) when data exists, tier.structural when absent
- `222` Total cost: COST_BASE (warm gold)
- `186/221/201` Burn rate: COST_LOW/MED/HIGH_RATE (rate-driven)
- `238/252` Separators, parentheses: tier.separator

### Line 4+: Activity (Active Tier)

```
[ACTIVE_CYAN(117)]T: tool_text
[COMPLETED_CHECK(67)]checkmark-Name [secondary]xN
[ACTIVE_PURPLE(183)]A: agent_text
[ACTIVE_TEAL(80)]TODO: todo_text (in-progress with active_form)
[COMPLETED_CHECK(67)]checkmark All todos complete (N/N) (all-done celebration)
```

- `117` Running tools: icon+text both ACTIVE_CYAN (Tokyo Night bright cyan)
- `67` Completed tools: checkmark+name both COMPLETED_CHECK (steel blue, links to active cyan)
- `183` Agents: icon+text both ACTIVE_PURPLE (Tokyo Night magenta)
- `80` Todos (in-progress): icon+text both ACTIVE_TEAL
- `67` Todos (all done): checkmark+text COMPLETED_CHECK (same as completed tools/agents)

## Rendered Output Examples

Complete output lines with every color code annotated, using the existing `[COLOR_NAME(code)]` pattern.

### Normal State (Dark Theme)

ASCII mode — L1 through L5 with every color annotated:

```
[STABLE_BLUE(111)]M:Opus 4.6 [separator(238)]| [secondary(146)]S:explanatory [separator]| [secondary]CC:2.1.37 [separator]| [secondary]P:~/projects/myapp [separator]| [STABLE_GREEN(71)]G:main [ACTIVE_CORAL(209)]↑2
[primary(251)]1 [structural(103)]CLAUDE.md [separator(238)]| [primary]3 [structural]rules [separator]| [primary]2 [structural]memories [separator]| [primary]2 [structural]hooks [separator]| [primary]4 [structural]MCPs [separator]| [primary]1 [structural]skills [separator]| [primary]1h
[STABLE_GREEN(71)]CTX:43% [separator(238)]([secondary(146)]86.0k[separator]/[secondary]200.0k[separator]) [separator]| [structural(103)]TOK [structural]I: [primary(251)]10.0k [structural]O: [primary]20.0k [primary]↗1.5K/s [structural]C:[primary]30.0k[separator]/[primary]40.0k [separator]| [COST_BASE(222)]$3.50 [separator]([COST_LOW_RATE(186)]$3.50/h[separator])
[structural(103)]Q:[secondary(146)]Pro [secondary]5h: [CTX_GOOD(71)]25% [separator(238)]([structural(103)]resets 2h 0m[separator])
[COMPLETED_CHECK(67)]✓ Read [secondary(146)]×12 [separator(238)]| [COMPLETED_CHECK]✓ Bash [secondary]×5 [separator]| [COMPLETED_CHECK]✓ Edit [secondary]×3
[ACTIVE_CYAN(117)]T:Read: [secondary(146)].../src/main.rs [separator(238)]| [ACTIVE_CYAN]T:Bash: [secondary]cargo test
[ACTIVE_PURPLE(183)]A:Explore [structural(103)][haiku][ACTIVE_PURPLE]: [secondary(146)]Investigating auth logic [separator(238)]([structural]2m[separator])
```

In icon mode, L2 gains per-metric indicator colors on icons (109/108/182/179/139/73/174) before each count.

### Alert State (Dark Theme)

Context critical (≥70%) + high burn rate (>$50/h):

```
[ALERT_RED(196)]CTX:75% [separator(238)]([secondary(146)]150.0k[separator]/[secondary]200.0k[separator]) [separator]| [structural(103)]TOK [structural]I: [primary(251)]45.0k [structural]O: [primary]12.0k [structural]C:[primary]50.0k[separator]/[primary]77.0k [separator]| [COST_BASE(222)]$12.50 [separator]([COST_HIGH_RATE(201)]$75.00/h[separator])
```

Note: `ALERT_RED` (196) replaces `STABLE_GREEN` (71) on the CTX prefix and percentage. `COST_HIGH_RATE` (201, magenta) replaces `COST_LOW_RATE` (186, peach) on the burn rate. All other colors remain identical.

### Light Theme

Same output, different emphasis tier codes — semantic colors are unchanged:

```
[STABLE_BLUE(111)]M:Opus 4.6 [separator(252)]| [secondary(240)]S:explanatory [separator]| [secondary]CC:2.1.37 [separator]| [secondary]P:~/projects/myapp [separator]| [STABLE_GREEN(71)]G:main
[primary(234)]1 [structural(245)]CLAUDE.md [separator(252)]| [primary]3 [structural]rules [separator]| [primary]2 [structural]memories [separator]| [primary]2 [structural]hooks [separator]| [primary]4 [structural]MCPs [separator]| [primary]1 [structural]skills [separator]| [primary]1h
[STABLE_GREEN(71)]CTX:43% [separator(252)]([secondary(240)]86.0k[separator]/[secondary]200.0k[separator]) [separator]| [structural(245)]TOK [structural]I: [primary(234)]10.0k [structural]O: [primary]20.0k [primary]↗1.5K/s [structural]C:[primary]30.0k[separator]/[primary]40.0k [separator]| [COST_BASE(222)]$3.50 [separator]([COST_LOW_RATE(186)]$3.50/h[separator])
```

Emphasis tier shifts: Primary 251→234, Secondary 146→240, Structural 103→245, Separator 238→252. All semantic colors (STABLE_BLUE 111, STABLE_GREEN 71, COST_BASE 222, etc.) remain identical.

## Icon Color Rules

1. Icon color ALWAYS matches its value color (never independently dimmed)
2. Line 1 model icon+value: STABLE_BLUE (111)
3. Line 1 style/version/project icon+value: tier.secondary (146/240) -- promoted from structural for visual prominence
4. Line 1 git icon+value: STABLE_GREEN (71) or ALERT_ORANGE/ACTIVE_CORAL (state)
5. Line 2 icons: per-metric INDICATOR color (109/108/182/179/139/73/174) -- unique visual fingerprints
6. Line 2 counts: tier.secondary; labels: tier.structural
7. Context icon matches percentage color (CTX_GOOD/WARN/CRITICAL)
8. Activity icons match their text color (TOOL_BLUE, AGENT_PURPLE, TODO_TEAL)
9. Completed tool checkmark+name: COMPLETED_CHECK (67) -- steel blue linking to active tools
10. ASCII mode labels (e.g. `M:`, `G:`) receive the same color as the icon they replace

## Rate-Based Cost Coloring

The burn rate (`$/h`) uses dynamic coloring based on spend velocity:

| Rate | Color | Visual |
|------|-------|--------|
| <$10/h | `COST_LOW_RATE` (186) | Subdued peach -- normal |
| $10-50/h | `COST_MED_RATE` (221) | Gold -- noticeable |
| >$50/h | `COST_HIGH_RATE` (201) | Magenta -- urgent, matches ALERT_MAGENTA |

The total cost always uses `COST_BASE` (222, warm gold).

## Theme Support

Set `theme = "light"` in config for light terminal backgrounds. Only emphasis tiers change between themes; all semantic colors (including INDICATOR) remain the same -- they are mid-to-bright saturated colors that work on both dark and light backgrounds.

## Light Theme Readability

### Contrast Strategy

On light backgrounds, the emphasis tiers reverse contrast direction -- dark grays on white instead of light grays on black. Semantic colors (blues, greens, teals, etc.) are mid-saturation and inherently readable on both backgrounds.

| Tier | Dark (on ~#24283b) | Light (on ~#d5d6db) | Contrast Direction |
|------|-------------------|--------------------|--------------------|
| **Primary** | 251 (bright white) | 234 (near-black) | Reversed |
| **Secondary** | 146 (blue-gray) | 240 (medium-dark gray) | Reversed |
| **Structural** | 103 (blue-purple) | 245 (medium gray) | Reversed |
| **Separator** | 238 (dark gray) | 252 (light gray) | Reversed |

### What Stays Fixed

All semantic colors are theme-invariant -- they are chosen to be readable on both dark and light backgrounds:

- Alert tier (196, 214, 201) -- bright saturated, always visible
- Active tier (117, 183, 80, 178, 209) -- mid-saturation, sufficient contrast on both
- Stable tier (111, 71) -- mid-brightness blues/greens, readable on both
- Indicator tier (109, 108, 182, 179, 139, 73, 174) -- muted pastels, readable on both
- Cost tier (222, 186, 221, 201) -- warm/bright tones, always legible

## How to Customize

Set the theme in your config file:

```toml
# ~/.claude/pulseline/config.toml
[display]
theme = "dark"  # or "light"
icons = true    # Nerd Font icons (false for ASCII)
```

### NO_COLOR Support

When the `NO_COLOR` environment variable is set (any value), all color output is disabled. This follows the [no-color.org](https://no-color.org) convention.
