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

### Strata Tier -- Two-Tier Chrome Split (state vs activity)

The strata tier tints the `|` separator on a per-row basis when
`layout.tonal_strata = true` (default). Two values per theme variant:

| Field | Tokyo Night Dark | Tokyo Night Light | Use |
|-------|------------------|-------------------|-----|
| `strata_state` | 238 | 253 | Identity / Config / Budget / Quota rows |
| `strata_activity` | 103 | 246 | Tools / Agents / Todos rows |

Strata is **chrome, not value** — these colors live below the emphasis
tiers and never render data. Theme authors hand-pick both values per
variant; a CI lint enforces `|state − activity| ≥ 3` on the ansi256
scale so no shipped theme can collapse the contract. See
`designs/tonal-strata-redesign.md` for the full spec and per-theme
rationale.

**Custom themes that omit the fields** fall back to `separator` /
`structural` and emit a one-time warning pointing at the missing field
names — existing customizations keep rendering.

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

## Tier Summary (8 types, 31 unique fields)

| Tier | Colors | Purpose | Status |
|------|--------|---------|--------|
| ALERT | 3 (196/214/201) | Critical states | Unchanged |
| ACTIVE | 5 (117/183/80/178/209) | Live activity | Unchanged |
| STABLE | 2 (111/71) | Static identity | Unchanged |
| INDICATOR | 7 (109/108/182/179/139/73/174) | L2 metric-specific anchoring | Unchanged |
| Emphasis | 4x2 themes | Gray hierarchy | Unchanged |
| Cost | 4 (222/186/221/201) | Rate-based | Unchanged |
| Strata | 2x2 themes (state/activity) | Per-row separator chrome | Added in 1.1.0 |
| Aurora | 3 (low/mid/high) | Sparkline velocity gradient on ledger CTX | **Added in 1.1.0** |

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
- `103/245` Labels: tier.structural (descriptive text)
- `238/252` Separators: tier.separator
- **ASCII mode**: icons are absent, counts and labels use the same hierarchy

### Line 3: Resources & Cost (Mixed)

```
[CTX_*(71/178/196)]CTX:pct% [separator(238/252)]([secondary(146/240)]used[separator]/[secondary]total[separator]) [separator]| [structural(103/245)]TOK I:[primary(251/234)]val O:[primary]val [primary]↗speed C:[primary]val [separator]| [COST_BASE(222)]$total [separator]([RATE_*(186/221/201)]$rate/h[separator])
```

- `71/178/196` Context: icon+pct both use CTX_GOOD/WARN/CRITICAL (semantic, state-driven)
- `103/245` Token labels: tier.structural (I:, O:, C:, R:)
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

## Built-in Themes

| Theme | Description |
|-------|-------------|
| `tokyo-night` | Blue-tinted grays, 25+ semantic colors (default) |
| `pulseline-aurora` | Aurora-pulse flagship: 3-stop velocity gradient on the ledger CTX sparkline |
| `echo-sub-zero` | Mono-accent minimalist, 3-stage CTX/cost signaling |
| `titanium-precision` | Industrial steel blues, amber warnings, brick reds |
| `cnc-telemetry` | Hardware telemetry: anodized teal, matte copper, rust red |
| `cyberdeck-hud` | Sci-Fi HUD: neon cyan, cyber orange, laser crimson |
| `stark-hud` | Iron Man: Arc Reactor cyan, Armor red, Faceplate gold |
| `mako-reactor` | FFVII: Shinra steel, Mako cyan-green, Materia accents |
| `aburaya-twilight` | Spirited Away: bathhouse red, dragon teal, spirit blues |
| `matte-carbon-neon` | Industrial tech: grayscale chrome, piercing neon accents |

> The full set of theme JSON files lives in `src/themes/`. New themes
> dropped into that directory are picked up at build time — this table
> is a curated snapshot, not the source of truth.

Set theme in config:

```toml
[display]
theme = "echo-sub-zero"   # or tokyo-night, titanium-precision
variant = "dark"           # dark | light
```

Preview all themes: `cc-pulseline --preview`

### Per-Color Overrides

Override individual colors on top of any preset:

```toml
[colors]
alert_red = 160        # ANSI 256-color code (0-255)
stable_blue = 75
```

## Custom Themes

Drop a JSON file in `~/.claude/pulseline/themes/` and set `theme` to its filename (without `.json`).

### Creating a Custom Theme

1. Copy an existing theme as a starting point:
   ```bash
   mkdir -p ~/.claude/pulseline/themes
   cp src/themes/echo-sub-zero.json ~/.claude/pulseline/themes/my-theme.json
   ```

2. Edit `palette_mapping` — these are the 34 ANSI 256-color codes that control rendering:

   | Field | Purpose |
   |-------|---------|
   | `emphasis_primary` | Core data values (brightest text) |
   | `emphasis_secondary` | Supporting data, counts |
   | `emphasis_structural` | Labels, icons, metadata |
   | `emphasis_separator` | Punctuation: `\|` `(` `)` `/` |
   | `alert_red` | Context >=70%, quota >=85% |
   | `alert_orange` | Git dirty `*` |
   | `alert_magenta` | Cost burn >$50/h |
   | `active_cyan` | Tool activity |
   | `active_purple` | Agent activity |
   | `active_teal` | Todo activity |
   | `active_amber` | Context warning 55-69% |
   | `active_coral` | Git ahead/behind |
   | `stable_blue` | Model identity on L1 |
   | `stable_green` | Git branch (clean) |
   | `indicator_claude_md` | L2 icon: CLAUDE.md |
   | `indicator_rules` | L2 icon: rules |
   | `indicator_memory` | L2 icon: memories |
   | `indicator_hooks` | L2 icon: hooks |
   | `indicator_mcp` | L2 icon: MCPs |
   | `indicator_skills` | L2 icon: skills |
   | `indicator_duration` | L2 icon: duration |
   | `completed_check` | Checkmark + completed name |
   | `cost_base` | Total cost display |
   | `cost_low_rate` | Burn rate <$10/h |
   | `cost_med_rate` | Burn rate $10-50/h |
   | `cost_high_rate` | Burn rate >$50/h |
   | `strata_state` | Chrome on the `\|` for state rows (L1/L2/L3/Quota) |
   | `strata_activity` | Chrome on the `\|` for activity rows (Tools/Agents/Todos) |
   | `aurora_low` | Sparkline fill at low CTX-consumption velocity (calm / idle, < 1%/min) |
   | `aurora_mid` | Sparkline fill at mid velocity (active, 1–5%/min) |
   | `aurora_high` | Sparkline fill at high velocity (hot, ≥ 5%/min) |
   | `tag_label` | Ledger TAG column (ENV / CTX / TOK / COST / TOOL / AGENT / TODO). Falls back to `secondary`. |
   | `head_agent` | L1 `AG:agent-name` pill (CC `--agent`). Falls back to `active_purple` so it matches L5+ `A:Explore` rows. |
   | `head_thinking` | L1 `[T]` thinking pill (CC `thinking.enabled`). Falls back to `active_amber` to stay distinct from `head_agent`. |

   The strata pair must satisfy `\|state − activity\| ≥ 3` on the ansi256
   scale; the `theme_strata_contrast` test fails CI otherwise. The aurora
   triple is enforced separately by `tests/theme_aurora_contrast.rs`
   (minimum spread between adjacent stops). Authors should pick strata
   chrome that reads quieter than the theme's `emphasis_primary` /
   `emphasis_secondary` so the separator never competes with values; the
   aurora triple should read as a smooth velocity gradient.

### Palette → UI Mapping

How each `palette_mapping` field connects to the rendered statusline:

```
 JSON palette_mapping                          Rendered UI
 ─────────────────────                         ──────────────────────────────────────────
                                               L1: Identity
 stable_blue ─────────────────────────────────→ M:Opus 4.6
 head_agent ──────────────────────────────────→ AG:greg-bot (when --agent active; default = active_purple)
 head_thinking ───────────────────────────────→ [T] (when thinking.enabled; default = active_amber)
 emphasis_secondary ──────────────────────────→ S:explanatory | CC:2.1.80 | P:~/myapp
 stable_green ────────────────────────────────→ G:main (clean branch)
 alert_orange ────────────────────────────────→ G:main* (dirty asterisk)
 active_coral ────────────────────────────────→ ↑2 ↓1 (ahead/behind)
 emphasis_separator ──────────────────────────→ | (default pipe color when tonal_strata = false)
 strata_state ────────────────────────────────→ | (pipes on L1/L2/L3/Quota when tonal_strata = true)

                                               L2: Config Counts
 indicator_claude_md ─┐                        ┌→ 󰈙 (CLAUDE.md icon)
 indicator_rules ─────┤ (7 indicator fields    ├→ 󰱇 (rules icon)
 indicator_memory ────┤  control icon colors   ├→ 󰧜 (memories icon)
 indicator_hooks ─────┤  independently —       ├→ 󱭧 (hooks icon)
 indicator_mcp ───────┤  or set all to same    ├→ 󰆧 (MCPs icon)
 indicator_skills ────┤  value for ghosting)   ├→ ⚡ (skills icon)
 indicator_duration ──┘                        └→ ⏱ (duration icon)
 emphasis_primary ────────────────────────────→ 2, 9, 1, 32 (count values)
 emphasis_structural ─────────────────────────→ CLAUDE.md, rules, ... (labels)

                                               L3: Budget — Triple-Stage Context
 stable_green ────────── (<55%) ─────────────→ CTX:43% (good — calm)
 active_amber ────────── (55-69%) ───────────→ CTX:60% (warn — attention)
 alert_red ───────────── (>=70%) ────────────→ CTX:82% (crit — urgent)
 emphasis_primary ────────────────────────────→ 86.0k/200.0k (token values)
 emphasis_structural ─────────────────────────→ TOK I: O: C: (labels)

                                               L3: Budget — Triple-Stage Cost
 cost_base ───────────────────────────────────→ $3.50 (total cost)
 cost_low_rate ───────── (<$10/h) ───────────→ $3.50/h (low burn)
 cost_med_rate ───────── ($10-50/h) ─────────→ $25/h (med burn)
 cost_high_rate ──────── (>$50/h) ───────────→ $85/h (high burn)

                                               Quota
 stable_green ────────── (<50%) ─────────────→ 25% (good)
 active_amber ────────── (50-84%) ───────────→ 65% (warn)
 alert_red ───────────── (>=85%) ────────────→ 92% (crit)

                                               Activity Lines
 completed_check ─────────────────────────────→ ✓ Read ×12 | ✓ Bash ×5
 active_cyan ─────────────────────────────────→ T:Read: .../main.rs
 active_purple ───────────────────────────────→ A:Explore [haiku]: ...
 active_teal ─────────────────────────────────→ TODO:Fixing auth bug
 completed_check ─────────────────────────────→ ✓ All todos complete
 strata_activity ─────────────────────────────→ | (pipes on activity rows when tonal_strata = true)
```

### JSON File Structure

```
theme.json
├── "$schema"              (string, optional — points to docs/theme-schema.json)
├── "theme"                (string, required — identifier used in config.toml)
├── "author"               (string, optional)
├── "description"          (string, optional)
│
├── "palette_mapping"      ★ REQUIRED — the 34 ANSI color codes that control rendering
│   ├── emphasis_primary        (u8) ─── brightest text: token values, counts
│   ├── emphasis_secondary      (u8) ─── supporting: style, version, project, targets
│   ├── emphasis_structural     (u8) ─── labels, icons, metadata text
│   ├── emphasis_separator      (u8) ─── punctuation: | ( ) /
│   ├── alert_red               (u8) ─── CTX >=70%, quota >=85%, cost >$50/h
│   ├── alert_orange            (u8) ─── git dirty asterisk (*)
│   ├── alert_magenta           (u8) ─── (alias for high cost burn)
│   ├── active_cyan             (u8) ─── tool activity (T:Read, T:Bash)
│   ├── active_purple           (u8) ─── agent activity (A:Explore)
│   ├── active_teal             (u8) ─── todo activity (TODO:...)
│   ├── active_amber            (u8) ─── CTX 55-69%, quota 50-84%
│   ├── active_coral            (u8) ─── git ahead/behind (↑n ↓n)
│   ├── stable_blue             (u8) ─── model name on L1
│   ├── stable_green            (u8) ─── git branch (clean), CTX <55%
│   ├── indicator_claude_md     (u8) ┐
│   ├── indicator_rules         (u8) │
│   ├── indicator_memory        (u8) │  L2 icon colors
│   ├── indicator_hooks         (u8) │  (set all same for ghosting,
│   ├── indicator_mcp           (u8) │   or unique per-metric)
│   ├── indicator_skills        (u8) │
│   ├── indicator_duration      (u8) ┘
│   ├── completed_check         (u8) ─── ✓ checkmark + completed name
│   ├── cost_base               (u8) ─── total cost display
│   ├── cost_low_rate           (u8) ─── burn <$10/h
│   ├── cost_med_rate           (u8) ─── burn $10-50/h
│   ├── cost_high_rate          (u8) ─── burn >$50/h
│   ├── strata_state            (u8) ─── chrome on `|` for state rows
│   ├── strata_activity         (u8) ─── chrome on `|` for activity rows
│   ├── aurora_low              (u8) ─── sparkline fill at low CTX velocity (<1%/min)
│   ├── aurora_mid              (u8) ─── sparkline fill at mid CTX velocity (1–5%/min)
│   ├── aurora_high             (u8) ─── sparkline fill at high CTX velocity (≥5%/min)
│   ├── tag_label               (u8, optional) ─── ledger TAG column (defaults to secondary)
│   ├── head_agent              (u8, optional) ─── L1 AG: pill (defaults to active_purple)
│   └── head_thinking           (u8, optional) ─── L1 [T] pill (defaults to active_amber)
│
├── "light_emphasis"       (optional — overrides emphasis tiers for light backgrounds)
│   ├── primary            (u8)
│   ├── secondary          (u8)
│   ├── structural         (u8)
│   ├── separator          (u8)
│   ├── strata_state       (u8, optional — falls back to dark variant if absent)
│   ├── strata_activity    (u8, optional — falls back to dark variant if absent)
│   ├── aurora_low         (u8, optional — falls back to dark variant if absent)
│   ├── aurora_mid         (u8, optional — falls back to dark variant if absent)
│   ├── aurora_high        (u8, optional — falls back to dark variant if absent)
│   ├── tag_label          (u8, optional — falls back to dark variant if absent)
│   ├── head_agent         (u8, optional — falls back to dark variant if absent)
│   └── head_thinking      (u8, optional — falls back to dark variant if absent)
│
├── "colors"               (optional — design documentation, not consumed by code)
├── "element_mapping"      (optional — documents which UI element uses which color)
├── "usage_logic"          (optional — documents context threshold stages)
├── "cost_logic"           (optional — documents cost threshold stages)
└── "design_notes"         (optional — philosophy, color vocabulary)
```

### Design Patterns

**Mono-accent** (echo-sub-zero, titanium-precision, cnc-telemetry):
Set `active_cyan = active_purple = active_teal` to the same value.
Color signals STATE (active/done/danger), not TYPE (tool/agent/todo).

**Multi-accent** (tokyo-night):
Each activity type gets a unique color. Color signals WHAT something is.

**Ghosted icons** (all minimalist themes):
Set all 7 `indicator_*` fields to `emphasis_structural`. Icons blend with labels.

**Rainbow icons** (tokyo-night):
Each `indicator_*` gets a unique muted accent color for fast visual scanning.

**Invisible "good" state** (cyberdeck-hud, cc-vanguard-telemetry):
Set `stable_green` very dark. "Good" context percentage vanishes — only warnings and critical states draw attention.

3. Optionally add `light_emphasis` for light terminal backgrounds:
   ```json
   "light_emphasis": {
     "primary": 234,
     "secondary": 240,
     "structural": 245,
     "separator": 252
   }
   ```

4. Set it in config:
   ```toml
   [display]
   theme = "my-theme"
   ```

5. Preview: `cc-pulseline --preview my-theme`

### Theme JSON Schema

Theme files are validated against `docs/theme-schema.json`. Only `palette_mapping` is required — all other sections (`colors`, `element_mapping`, `usage_logic`, `design_notes`) are optional design documentation.

### Tips

- Multiple fields CAN share the same color code (mono-accent themes use one color for tools/agents/todos)
- Use `--preview theme1 theme2` to compare two themes side by side
- The `[colors]` TOML section in config applies on top of ANY theme (built-in or custom)

### NO_COLOR Support

When the `NO_COLOR` environment variable is set (any value), all color output is disabled. This follows the [no-color.org](https://no-color.org) convention.
