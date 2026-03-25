---
name: preview-theme
description: Preview and add cc-pulseline color themes. Use when user pastes a theme JSON with palette_mapping, asks to preview a theme, says "try this theme", or wants to add a new theme to the codebase. Also use when user shares a color palette design or asks to render theme colors in the terminal.
user-invocable: true
argument-hint: Paste a theme JSON or say "preview <theme-name>"
allowed-tools:
  - Bash
  - Read
  - Edit
  - Write
  - Glob
  - Grep
  - AskUserQuestion
---

# Preview Theme

Render a cc-pulseline color theme in the terminal, then optionally add it to the codebase.

## When This Triggers

- User pastes JSON with a `palette_mapping` section containing ANSI 256-color codes
- User says "preview this theme", "try this theme", "render this palette"
- User invokes `/preview-theme` directly

## Step 1: Extract Colors

Parse the user's JSON. The critical section is `palette_mapping` with exactly 26 fields:

| Group | Fields |
|-------|--------|
| Emphasis (4) | `emphasis_primary`, `emphasis_secondary`, `emphasis_structural`, `emphasis_separator` |
| Alert (3) | `alert_red`, `alert_orange`, `alert_magenta` |
| Active (5) | `active_cyan`, `active_purple`, `active_teal`, `active_amber`, `active_coral` |
| Stable (2) | `stable_blue`, `stable_green` |
| Indicator (7) | `indicator_claude_md`, `indicator_rules`, `indicator_memory`, `indicator_hooks`, `indicator_mcp`, `indicator_skills`, `indicator_duration` |
| Completed (1) | `completed_check` |
| Cost (4) | `cost_base`, `cost_low_rate`, `cost_med_rate`, `cost_high_rate` |

Also extract:
- `theme` — name (required; ask if missing)
- `description` — one-liner (required; ask if missing)
- `light_emphasis` — optional object with `primary`, `secondary`, `structural`, `separator`

Validate all 26 values are integers 0-255. Report errors and stop if invalid.

## Step 2: Render Terminal Preview

Use a single Bash command with printf + ANSI 256-color escapes. Substitute the extracted values into this template.

The escape format: `\033[38;5;{N}m` for color, `\033[0m` for reset.

```bash
# Assign variables from palette_mapping
PRI={emphasis_primary}; SEC={emphasis_secondary}; STR={emphasis_structural}; SEP={emphasis_separator}
ARED={alert_red}; AORG={alert_orange}; AMAG={alert_magenta}
ACYN={active_cyan}; APUR={active_purple}; ATEAL={active_teal}; AAMB={active_amber}; ACOR={active_coral}
SBLU={stable_blue}; SGRN={stable_green}
ICMD={indicator_claude_md}; IRUL={indicator_rules}; IMEM={indicator_memory}; IHOK={indicator_hooks}
IMCP={indicator_mcp}; ISKL={indicator_skills}; IDUR={indicator_duration}
CCHK={completed_check}; CBAS={cost_base}; CLOW={cost_low_rate}; CMED={cost_med_rate}; CHI={cost_high_rate}
C="\033[38;5;"; R="\033[0m"

printf "\n\033[1m═══ {theme_name} ═══\033[0m\n\n"

# L1: Identity (stable_blue, secondary, stable_green, alert_orange, active_coral)
printf "  ${C}${SBLU}mM:Opus 4.6${R} ${C}${SEP}m|${R} ${C}${SEC}mS:explanatory${R} ${C}${SEP}m|${R} ${C}${SEC}mCC:2.1.80${R} ${C}${SEP}m|${R} ${C}${SEC}mP:~/myapp${R} ${C}${SEP}m|${R} ${C}${SGRN}mG:main${R}${C}${AORG}m*${R} ${C}${ACOR}m↑2${R}\n"

# L2: Config counts (all 7 indicators, primary, structural)
printf "  ${C}${ICMD}m󰈙${R} ${C}${PRI}m2${R} ${C}${STR}mCLAUDE.md${R} ${C}${SEP}m|${R} ${C}${IRUL}m󰱇${R} ${C}${PRI}m9${R} ${C}${STR}mrules${R} ${C}${SEP}m|${R} ${C}${IMEM}m󰧜${R} ${C}${PRI}m1${R} ${C}${STR}mmemories${R} ${C}${SEP}m|${R} ${C}${IHOK}m󱭧${R} ${C}${PRI}m2${R} ${C}${STR}mhooks${R} ${C}${SEP}m|${R} ${C}${IMCP}m󰆧${R} ${C}${PRI}m4${R} ${C}${STR}mMCPs${R} ${C}${SEP}m|${R} ${C}${ISKL}m⚡${R} ${C}${PRI}m1${R} ${C}${STR}mskills${R} ${C}${SEP}m|${R} ${C}${IDUR}m⏱${R} ${C}${PRI}m1h${R}\n"

# L3: Triple-stage context (stable_green=good, active_amber=warn, alert_red=crit)
printf "  good  ${C}${SGRN}mCTX:43%%${R} ${C}${SEP}m(${R}${C}${PRI}m86.0k${R}${C}${SEP}m/${R}${C}${PRI}m200.0k${R}${C}${SEP}m)${R} ${C}${SEP}m|${R} ${C}${STR}mTOK I:${R}${C}${PRI}m86.0k${R} ${C}${STR}mO:${R}${C}${PRI}m20k${R} ${C}${SEP}m|${R} ${C}${CBAS}m\$3.50${R} ${C}${SEP}m(${R}${C}${CLOW}m\$3.50/h${R}${C}${SEP}m)${R}\n"
printf "  warn  ${C}${AAMB}mCTX:60%%${R} ${C}${SEP}m(${R}${C}${PRI}m120.0k${R}${C}${SEP}m/${R}${C}${PRI}m200.0k${R}${C}${SEP}m)${R} ${C}${SEP}m|${R} ${C}${STR}mTOK I:${R}${C}${PRI}m120k${R} ${C}${STR}mO:${R}${C}${PRI}m15k${R} ${C}${SEP}m|${R} ${C}${CBAS}m\$25.00${R} ${C}${SEP}m(${R}${C}${CMED}m\$25/h${R}${C}${SEP}m)${R}\n"
printf "  crit  ${C}${ARED}mCTX:82%%${R} ${C}${SEP}m(${R}${C}${PRI}m164.0k${R}${C}${SEP}m/${R}${C}${PRI}m200.0k${R}${C}${SEP}m)${R} ${C}${SEP}m|${R} ${C}${STR}mTOK I:${R}${C}${PRI}m164k${R} ${C}${STR}mO:${R}${C}${PRI}m12k${R} ${C}${SEP}m|${R} ${C}${CBAS}m\$85.00${R} ${C}${SEP}m(${R}${C}${CHI}m\$85/h${R}${C}${SEP}m)${R}\n"

# Quota (stable_green=good, active_amber=warn, alert_red=crit — different thresholds: <50%, 50-84%, >=85%)
printf "  ${C}${STR}mQ:${R}${C}${SEC}m5h:${R} ${C}${SGRN}m25%%${R}${C}${SEP}m (${R}${C}${STR}mresets 2h 0m${R}${C}${SEP}m)${R}\n"

# Completed tools (completed_check, secondary)
printf "  ${C}${CCHK}m✓ Read${R} ${C}${SEC}m×12${R} ${C}${SEP}m|${R} ${C}${CCHK}m✓ Bash${R} ${C}${SEC}m×5${R} ${C}${SEP}m|${R} ${C}${CCHK}m✓ Edit${R} ${C}${SEC}m×3${R}\n"

# Running tools (active_cyan, secondary)
printf "  ${C}${ACYN}mT:Read:${R} ${C}${SEC}m.../src/main.rs${R} ${C}${SEP}m|${R} ${C}${ACYN}mT:Bash:${R} ${C}${SEC}mcargo test${R}\n"

# Running agent (active_purple, structural, secondary)
printf "  ${C}${APUR}mA:Explore${R}${C}${STR}m [haiku]${R}${C}${APUR}m:${R} ${C}${SEC}mInvestigating auth logic${R} ${C}${SEP}m(${R}${C}${STR}m2m${R}${C}${SEP}m)${R}\n"

# Todo in-progress (active_teal, secondary, structural)
printf "  ${C}${ATEAL}mTODO:${R}${C}${ATEAL}mFixing auth bug${R} ${C}${SEP}m(${R}${C}${SEC}m1/3${R}${C}${SEP}m)${R} ${C}${SEP}m(${R}${C}${STR}m5s${R}${C}${SEP}m)${R}\n"

# Todo all done (completed_check)
printf "  ${C}${CCHK}m✓ All todos complete (3/3)${R}\n"

# Color swatch
printf "\n  \033[2mPalette:\033[0m\n"
printf "  ${C}${PRI}mpri${R}  ${C}${SEC}msec${R}  ${C}${STR}mstr${R}  ${C}${SEP}msep${R}\n"
printf "  ${C}${ACYN}maccent${R}  ${C}${AAMB}mwarn${R}  ${C}${ARED}malert${R}  ${C}${AORG}mdirty${R}  ${C}${CCHK}mdone${R}  ${C}${SBLU}mmodel${R}\n"
```

This exercises all 26 palette colors across realistic UI states.

After the color preview, also print the field→UI mapping for the specific theme values:

```bash
printf "\n  \033[2mField → UI Mapping:\033[0m\n"
printf "  L1: stable_blue(${SBLU})→model  secondary(${SEC})→style/ver/proj  alert_orange(${AORG})→dirty*\n"
printf "  L2: indicator_*(${ICMD})→icons  primary(${PRI})→counts  structural(${STR})→labels\n"
printf "  L3: stable_green(${SGRN})→good  active_amber(${AAMB})→warn  alert_red(${ARED})→crit\n"
printf "  $$: cost_low(${CLOW})→<10/h  cost_med(${CMED})→10-50/h  cost_high(${CHI})→>50/h\n"
printf "  Act: active_cyan(${ACYN})→tools  active_purple(${APUR})→agents  completed(${CCHK})→✓done\n"
```

## Step 3: Ask User Intent

After rendering, ask:

> Would you like to:
> 1. **Add as built-in theme** — saves to `src/themes/` and registers in binary
> 2. **Save as custom user theme** — saves to `~/.claude/pulseline/themes/`
> 3. **Preview only** — done

## Step 4a: Add as Built-in Theme

If user chooses "add as built-in":

### Save theme JSON

Write to `src/themes/{name}.json`. Use existing themes as reference (read `src/themes/tokyo-night.json`). The file must include:
- `"$schema": "../../docs/theme-schema.json"`
- `theme`, `author`, `description`
- `palette_mapping` (the 26 fields)
- `light_emphasis` (if provided)
- Any optional sections the user included (`colors`, `element_mapping`, `design_notes`, etc.)

### Register in BUILTIN_THEMES

Edit `src/render/color.rs`. Find `static BUILTIN_THEMES` and add before the closing `];`:

```rust
    (
        "{name}",
        include_str!("../themes/{name}.json"),
    ),
```

### Update help text

Edit `src/main.rs`. Find the `THEMES:` section in `print_help()` and add a new line:

```
    {name:<20}{description}
```

Align with existing entries (4-space indent before name).

### Update docs

Edit `docs/theme-palette.md`. Find the "Built-in Themes" table and add:

```
| `{name}` | {description} |
```

### Verify

Run in sequence:
1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo build --release`
4. `cargo test`

Fix any failures before proceeding.

### Confirm with binary preview

```bash
target/release/cc-pulseline --preview {name}
```

This confirms the theme works through the actual render pipeline.

## Step 4b: Save as Custom User Theme

If user chooses "custom user theme":

```bash
mkdir -p ~/.claude/pulseline/themes
```

Write the JSON file to `~/.claude/pulseline/themes/{name}.json`. Done — the binary will discover it automatically on next render.

## Notes

- Context thresholds: <55% = `stable_green`, 55-69% = `active_amber`, >=70% = `alert_red`
- Cost thresholds: <$10/h = `cost_low_rate`, $10-50/h = `cost_med_rate`, >$50/h = `cost_high_rate`
- Quota thresholds: <50% = `stable_green`, 50-84% = `active_amber`, >=85% = `alert_red`
- `completed_check` is used for ALL completion states: tool checkmarks, agent done, todo all-done
- Icon color always matches its adjacent value color (icon=value rule)
- Multiple palette fields CAN share the same ANSI code (mono-accent themes do this)
