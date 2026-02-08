# Color Specification

Reference guide for the cc-pulseline 256-color palette, organized by information hierarchy.

## Design Principles

Colors are organized into a **three-tier attention system** (for semantic colors) with a **four-tier emphasis hierarchy** (for gray-scale text):

1. **ALERT** — Demands immediate attention (context >=85%, git dirty, high burn rate)
2. **ACTIVE** — Currently happening, dynamically changing (tools, agents, context 70-84%)
3. **STABLE** — Informational, unchanging context (model, version, branch clean, normal context)

This hierarchy guides the eye: red/orange/magenta for urgent issues, mid-saturation for activity, muted grays for stable information.

The palette draws inspiration from **Tokyo Night Storm** (`folke/tokyonight.nvim`), using blue-tinted grays for emphasis tiers and the theme's signature blues and purples for semantic colors. Context, cost, and git state colors are preserved as-is for functional clarity.

## Palette

All colors use `\x1b[38;5;{N}m` format.

### Emphasis Tiers

Four-level hierarchy for text and structural elements. Vary by theme; semantic colors do not.

| Tier | Dark | Light | ■ | Use |
|------|------|-------|---|-----|
| **Primary** | 251 | 236 | ■■ | Reserved (available for high-priority values) |
| **Secondary** | 146 | 243 | ■■ | Values, counts, data (blue-tinted gray) |
| **Structural** | 60 | 246 | ■■ | Icons, labels, supporting text (Tokyo Night comment) |
| **Separator** | 238 | 250 | ■■ | Punctuation only (\|, (), /) |

### Alert Tier — Bright, Saturated, Urgent

| Name | Code | ■ | Purpose |
|------|------|---|---------|
| `ALERT_RED` | 196 | ■ | Context >=85%, critical states |
| `ALERT_ORANGE` | 214 | ■ | Git dirty `*` |
| `ALERT_MAGENTA` | 201 | ■ | Burn rate >$50/h |

### Active Tier — Mid-Saturation, Noticeable

| Name | Code | ■ | Purpose |
|------|------|---|---------|
| `ACTIVE_CYAN` | 117 | ■ | Tool activity (Tokyo Night bright cyan) |
| `ACTIVE_PURPLE` | 183 | ■ | Agent activity (Tokyo Night magenta) |
| `ACTIVE_TEAL` | 80 | ■ | Todo activity |
| `ACTIVE_AMBER` | 178 | ■ | Context 70-84% |
| `ACTIVE_CORAL` | 209 | ■ | Git ahead/behind |

### Stable Tier — Muted, Informational

| Name | Code | ■ | Purpose |
|------|------|---|---------|
| `STABLE_BLUE` | 111 | ■ | Model identity (Tokyo Night main blue) |
| `STABLE_GREEN` | 71 | ■ | Git branch (clean) |

### Cost Tier — Rate-Based Dynamic Coloring

| Name | Code | ■ | Condition |
|------|------|---|-----------|
| `COST_BASE` | 222 | ■ | Total cost display |
| `COST_LOW_RATE` | 186 | ■ | Burn rate <$10/h |
| `COST_MED_RATE` | 221 | ■ | Burn rate $10-50/h |
| `COST_HIGH_RATE` | 201 | ■ | Burn rate >$50/h |

### Legacy Aliases

For backward compatibility, old names map to the new tier system:

| Legacy Name | Points To | Change |
|-------------|-----------|--------|
| `MODEL_BLUE` | `STABLE_BLUE` | → 111 (Tokyo Night main blue) |
| `GIT_GREEN` | `STABLE_GREEN` | 71 (unchanged) |
| `GIT_MODIFIED` | `ALERT_ORANGE` | 214 (unchanged) |
| `GIT_AHEAD` | `ACTIVE_CORAL` | 209 (unchanged) |
| `GIT_BEHIND` | `ACTIVE_CORAL` | 209 (unchanged) |
| `CTX_GOOD` | `STABLE_GREEN` | 71 (unchanged) |
| `CTX_WARN` | `ACTIVE_AMBER` | 178 (unchanged) |
| `CTX_CRITICAL` | `ALERT_RED` | 196 (unchanged) |
| `TOOL_BLUE` | `ACTIVE_CYAN` | → 117 (Tokyo Night bright cyan) |
| `AGENT_PURPLE` | `ACTIVE_PURPLE` | → 183 (Tokyo Night magenta) |
| `TODO_TEAL` | `ACTIVE_TEAL` | 80 (unchanged) |

**Removed**: `PROJECT_CYAN` (51), `COST_GOLD` (220), `RATE_YELLOW` (226) — replaced by emphasis tiers and rate-based cost coloring.

## Element Mapping

### Line 1: Identity (Semantic + Secondary)

```
[STABLE_BLUE(111)]M:model [separator(238/250)]| [secondary(146/243)]S:style [separator]| [secondary]CC:version [separator]| [secondary]P:~/path [separator]| [STABLE_GREEN(71)]G:branch[ALERT_ORANGE(214)]*[ACTIVE_CORAL(209)] ↑n
```

- ■`111` Model: icon+value both STABLE_BLUE (most important identity)
- ■`146/243` Style/Version/Project: icon+value both tier.secondary (promoted from structural — these are important session identifiers)
- ■`71` Git: icon+value both STABLE_GREEN (unless dirty/ahead/behind)
- ■`238/250` Separators: tier.separator

### Line 2: Config Counts (Monochrome Hierarchy)

```
[structural(60/246)]icon [secondary(146/243)]count [structural]label [separator(238/250)]| ...
```

- ■`60/246` Icons+Labels: tier.structural (visual markers and descriptive text)
- ■`146/243` Counts: tier.secondary (the actual data — most prominent on L2)
- ■`238/250` Separators: tier.separator

### Line 3: Resources & Cost (Mixed)

```
[CTX_*(71/178/196)]CTX:pct% [separator(238/250)]([secondary(146/243)]used[separator]/[secondary]total[separator]) [separator]| [structural(60/246)]TOK I:[secondary]val ... [separator]| [COST_BASE(222)]$total [separator]([RATE_*(186/221/201)]$rate/h[separator])
```

- ■`71/178/196` Context: icon+pct both use CTX_GOOD/WARN/CRITICAL (semantic, state-driven)
- ■`60/246` Token labels: tier.structural (I:, O:, C:, R:)
- ■`146/243` Token values: tier.secondary
- ■`222` Total cost: COST_BASE (warm gold)
- ■`186/221/201` Burn rate: COST_LOW/MED/HIGH_RATE (rate-driven)
- ■`238/250` Separators, parentheses: tier.separator

### Line 4+: Activity (Active Tier)

```
[ACTIVE_CYAN(117)]T: tool_text
[ACTIVE_PURPLE(183)]A: agent_text
[ACTIVE_TEAL(80)]TODO: todo_text
```

- ■`117` Tools: icon+text both ACTIVE_CYAN (Tokyo Night bright cyan)
- ■`183` Agents: icon+text both ACTIVE_PURPLE (Tokyo Night magenta)
- ■`80` Todos: icon+text both ACTIVE_TEAL

## Icon Color Rules

1. Icon color ALWAYS matches its value color (never independently dimmed)
2. Line 1 model icon+value: STABLE_BLUE (111)
3. Line 1 style/version/project icon+value: tier.secondary (146/243) — promoted from structural for visual prominence
4. Line 1 git icon+value: STABLE_GREEN (71) or ALERT_ORANGE/ACTIVE_CORAL (state)
5. Line 2 icons+labels match tier.structural (monochrome hierarchy)
6. Context icon matches percentage color (CTX_GOOD/WARN/CRITICAL)
7. Activity icons match their text color (TOOL_BLUE, AGENT_PURPLE, TODO_TEAL)
8. ASCII mode labels (e.g. `M:`, `G:`) receive the same color as the icon they replace

## Rate-Based Cost Coloring

The burn rate (`$/h`) uses dynamic coloring based on spend velocity:

| Rate | Color | Visual |
|------|-------|--------|
| <$10/h | `COST_LOW_RATE` (186) | Subdued peach — normal |
| $10-50/h | `COST_MED_RATE` (221) | Gold — noticeable |
| >$50/h | `COST_HIGH_RATE` (201) | Magenta — urgent, matches ALERT_MAGENTA |

The total cost always uses `COST_BASE` (222, warm gold).

## Theme Support

Set `PULSELINE_THEME=light` for light terminal backgrounds. Only emphasis tiers change between themes; all semantic colors remain the same (they are mid-to-bright saturated colors that work on both dark and light backgrounds).

## Light Theme Readability

### Contrast Strategy

On light backgrounds, the emphasis tiers reverse contrast direction — dark grays on white instead of light grays on black. Semantic colors (blues, greens, teals, etc.) are mid-saturation and inherently readable on both backgrounds.

| Tier | Dark (on ~#24283b) | Light (on ~#d5d6db) | Contrast Direction |
|------|-------------------|--------------------|--------------------|
| **Primary** | 251 (bright white) | 236 (near-black) | Reversed |
| **Secondary** | 146 (blue-gray) | 243 (medium-dark gray) | Reversed |
| **Structural** | 60 (dim blue-gray) | 246 (medium gray) | Reversed |
| **Separator** | 238 (dark gray) | 250 (light gray) | Reversed |

### What Stays Fixed

All semantic colors are theme-invariant — they are chosen to be readable on both dark and light backgrounds:

- Alert tier (196, 214, 201) — bright saturated, always visible
- Active tier (117, 183, 80, 178, 209) — mid-saturation, sufficient contrast on both
- Stable tier (111, 71) — mid-brightness blues/greens, readable on both
- Cost tier (222, 186, 221, 201) — warm/bright tones, always legible
