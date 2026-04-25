# Design brief: cc-pulseline v2 — breaking-change statusline redesign

- **Platform:** CLI / statusline (Claude Code statusline hook — stdin JSON, stdout ANSI text)
- **Primary job:** At a glance, communicate (a) where you are (model, project, branch), (b) how much budget is left (context, cost, quota), (c) what's happening right now (tools, agents, todos) — peripherally, while the user codes.
- **Reference / system:** Existing 28-field `ThemePalette` (post tonal-strata redesign). New tier added in this spec.
- **Date:** 2026-04-25
- **Mandate:** breaking changes — new layout, new palette, quality feel + readable.
- **Compatibility:** soft break. v1 frame styles remain functional under the same `pane.style` TOML key; v2 layouts add three new style names alongside.

---

## Why a breaking change

Today's layout is a wide menu bar — every field in `value | value | value` cells. Three failure modes:

1. **No trend, only state.** `CTX:43%` says *now*; you can't tell if the slope is climbing or stable.
2. **Equal-weight typography.** Every metric occupies one line of equal cells. The eye has nowhere to land first.
3. **L2 config counts compete with L1 identity.** Reference data the user knew before opening Claude Code — perpetual chrome, not a live signal.

The new design treats the statusline as a **cockpit instrument cluster**, not a menu bar.

---

## Decisions captured (from gating-question round)

| # | Decision | Notes |
|---|---|---|
| 1 | Width auto-detect | already shipped (`detect_terminal_width()`); v2 adds a width-bracket resolver for `style = "auto"` |
| 2 | **A (Cockpit) is the default**; C (Console) and B (Flightstrip) ship as opt-in styles | inversion of original "C-as-default" — A gets ~70% of user exposure, so design polish lives there |
| 3 | Glyphs: Nerd Font + Unicode blocks (modernized) | block bars for gauges, braille for sparklines, geometric for tier glyphs |
| 4 | Palette: hand-curate **Pulseline Aurora** as the new flagship | bespoke, not borrowed; ships alongside the 9 existing themes |
| 5 | Surface additions: CTX sparkline (5a) ✓, cost burn arc (5c) ✓, recent-tools tape (5e) ✓; tool-burst rate (5b) **deferred** until 5e proves insufficient; quota gradient (5d) **deferred** — text-only `Q5h 75%` until needed |
| 6 | Performance: sparkline + braille gauge approved | total cost ~+290µs, < 1% of 50ms budget |
| 7 | L2 config counts: **kept reachable** via segment toggles | hidden by default in v2 layouts; user with `show_claude_md = true` gets a compact config row inserted between identity and cluster |
| 8 | Phase A (v1 cleanup) ships as one PR | rename + module reshape + docs together |
| 9 | Default flip timing: **"we'll see"** | flip after v2 settles; no fixed deadline |

---

## v1 / v2 namespace plan (the heart of backward compat)

One `pane.style` TOML key, two non-overlapping name spaces.

```
pane.style = ?

  ── v1 frame styles (stable, retained) ──────────────
  "none"        # flat, no decoration                (current default)
  "zones"       # single rule between groups
  "grid"        # label-column layout
  "cards"       # each-group framed
  "sections"    # single outer frame

  ── v2 layout styles (new) ──────────────────────────
  "cockpit"     # 3-row instrument cluster           (default after flip)
  "console"     # 4-5 row framed dashboard
  "flightstrip" # 2-row dense
  "auto"        # width-bracket selector             (recommended for v2 users)

  ── transitional aliases ────────────────────────────
  "v1-classic"  → "none"        (explicit "old default" anchor)
  "v2"          → "auto"         (explicit "new flagship")
```

**Cross-cut concerns that work in both namespaces:**
- All segment toggles (`show_model`, `show_git`, `show_claude_md`, …) — work everywhere. v2 layouts default a few off; users can flip them on.
- `pane.tonal_strata`, `pane.width_mode`, `pane.cc_margin` — work everywhere.
- All themes (existing 9 + Pulseline Aurora) — eat the same `ThemePalette`. The v2-specific aurora_* fields fall back gracefully on v1 themes via the same `unwrap_or` pattern as strata fields (lint enforces presence on built-ins).

**Width-bracket resolver for `style = "auto"`:**
```
width >= 130  → console   (premium, framed dashboard)
110 <= w < 130 → cockpit  (default, instrument cluster)
90  <= w < 110 → flightstrip (dense, 2-row)
width < 90    → degraded cockpit (single row fallback)
```

`style = "auto"` re-evaluates every render tick — window resize triggers layout switch on the next CC statusline poll. Pinned styles (`style = "console"`) honour the user's choice and only run width *content* degradation, never layout *swap*.

---

## Pulseline Aurora — the new flagship palette

### Design intent

A bespoke 3-tier palette designed for *peripheral instrument-cluster reading*. Not borrowed from Catppuccin / Rose Pine / Tokyo Night — those are tuned for full-app surfaces.

| Layer | Role | Direction |
|---|---|---|
| **Substrate** (chrome) | Frame, separators, section breaks — never read, only seen | 2 stops, cool dark grays (or cool light grays in light variant) |
| **Datum** (values) | Numbers, names, token counts | 1 stop, very high contrast (pearl on dark / charcoal on light) |
| **Pulse** (signals) | Anything that *moves* — sparklines, gauges, active rows | 3-stop aurora gradient: cool → mid → warm |

### Pulseline Aurora — full palette spec

**Dark variant:**

| Field | Code | Role |
|---|---|---|
| `primary` | 254 | pearl — values, names |
| `secondary` | 248 | labels |
| `structural` | 244 | dimmed labels, units |
| `separator` | 237 | frame chrome |
| `strata_state` | 237 | state-row separator |
| `strata_activity` | 109 | activity-row separator |
| `alert_red` | 196 | critical |
| `alert_orange` | 178 | warning (calmer than tokyo-night 214) |
| `alert_magenta` | 217 | rose — high cost burn |
| `active_cyan` | 80 | = `aurora_mid` (collapsed) |
| `active_purple` | 80 | = `aurora_mid` |
| `active_teal` | 80 | = `aurora_mid` |
| `active_amber` | 178 | warning |
| `active_coral` | 209 | = `aurora_high` |
| `stable_blue` | 67 | model identity |
| `stable_green` | 71 | git clean |
| `indicator_claude_md` | 109 | desaturated |
| `indicator_rules` | 108 | desaturated |
| `indicator_memory` | 144 | desaturated |
| `indicator_hooks` | 178 | warm anchor |
| `indicator_mcp` | 73 | cool anchor |
| `indicator_skills` | 80 | active mid |
| `indicator_duration` | 152 | quiet |
| `completed_check` | 67 | steel |
| `cost_base` | 222 | warm gold |
| `cost_low_rate` | 80 | aurora mid |
| `cost_med_rate` | 178 | warning |
| `cost_high_rate` | 209 | aurora high |
| **`aurora_low`** | **73** | **pulse stop 1: stable, calm — sparkline floor** |
| **`aurora_mid`** | **80** | **pulse stop 2: active, present — sparkline normal** |
| **`aurora_high`** | **209** | **pulse stop 3: warming, attention — sparkline crest** |

**Light variant** (key differences):
- `primary` → 232, `secondary` → 240, `structural` → 244, `separator` → 252
- `strata_state` → 252, `strata_activity` → 246
- `aurora_*` are **theme-invariant** (same dark + light codes) — they're brand pigment, not mode-aware chrome. Confirmed legible on light backgrounds via 73/80/209 contrast check.

### Aurora seeds for the 9 existing themes

Each existing theme gets `aurora_low`, `aurora_mid`, `aurora_high` so they keep working under v2 layouts. Same playbook as the strata seed pass — proposed values below for visual review before commit. Lint enforces both presence (built-ins must author all 3) and gradient ordering (`|low - mid| ≥ 4 AND |mid - high| ≥ 4` on ansi256).

| Theme | low | mid | high | Rationale |
|---|---|---|---|---|
| **tokyo-night** | 73 | 80 | 209 | reuses `completed_check` (steel) → `active_teal` → `active_coral`; coherent with theme's existing semantic ladder |
| **echo-sub-zero** | 109 | 110 | 191 | desaturated → canonical active → lime warning (matches usage_logic stage 2) |
| **titanium-precision** | 67 | 74 | 167 | steel → `active_cyan` → warm clay (already `cost_high_rate`) |
| **cnc-telemetry** | 109 | 66 | 130 | sage → anodized teal → patina copper (theme's iconic warm) |
| **cyberdeck-hud** | 60 | 51 | 208 | dim violet → laser cyan → cyber orange (strong neon ramp) |
| **stark-hud** | 59 | 74 | 214 | dim cool → mid blue → arc-reactor orange |
| **mako-reactor** | 60 | 43 | 220 | dim Shinra-steel → mako energy → Shinra-gold caution |
| **aburaya-twilight** | 67 | 73 | 210 | twilight blue → Haku dragon teal → bathhouse coral |
| **matte-carbon-neon** | 240 | 51 | 196 | matte carbon → laser cyan → alert red (industrial → neon → alarm) |

Same approval gate as strata: I propose, you `cc-pulseline --palette-map <theme>` each one and veto any value that doesn't feel right under your terminal background.

---

# Three layout variations

## Variation A — "Cockpit" (3 rows, instrument cluster) · DEFAULT

Headline-cluster-ticker pattern. Top row is identity. Middle row is the budget cluster — gauge + sparkline + cost arc + quota text. Bottom row is the activity ticker.

```
                                                                                 
 Opus 4.7   feat/status-pane *  ↑3   ~/cc-pulseline                  43%·86k 
 CTX  ████████░░░░░░░░░░ 43%   ⠀⠀⢀⡠⠊⠁  TOK 1.2K/s   $3.50 ◐  Q5h 75% 02h    
 ▶ Read main.rs   ▶ Bash test   ✓ ×12   A:Explore [haiku] 2m   • 1/3 todos   
                                                                                 
```

**Anatomy:**

- **L1 — Headline.** Bold model, branch with status glyph, ahead/behind, project path. Right-justified pill: current CTX % and absolute tokens. L2 config row hidden by default (re-enable via `show_claude_md = true` etc.).
- **L2 — Cluster.** Six instruments in fixed cells:
  1. **CTX gauge** — 18-cell gradient block bar (`█▉▊▋▌▍▎▏░`), fills `aurora_low → aurora_high`. Position-based gradient + threshold override (turns `alert_amber` ≥55%, `alert_red` ≥70%).
  2. **CTX sparkline** — 6 braille cells, last 30 ticks. The slope is the new information.
  3. **Token rate** — `1.2K/s` with `↗` only when accelerating.
  4. **Cost** — `$3.50` only; *no parenthetical $/h* (the arc replaces it — see Risk-3 in earlier eval).
  5. **Cost burn arc** — 1-2 cells, glyphs `○ ◔ ◑ ◕ ●`, fills proportionally to $/h vs daily budget.
  6. **Quota** — text `Q5h 75% 02h`, suppressed if quota disabled.
- **L3 — Ticker.** `▶` = running, `✓` = completed (with count). Tools, agents, todos all `aurora_mid` — differentiation by *icon shape*, not hue. The eye learns the shapes once.

**Width degradation (within Cockpit):**
1. ≥ 120 cols: full as above
2. 100–119: drop the quota text from L2
3. 80–99: drop sparkline + arc, keep gauge + raw % + cost
4. < 80: collapse to single-row fallback

**State requirements:** ring buffer for `ctx_history` (last 30 samples, ~30 bytes).

**Trade-offs:**
- L2 config row hidden by default (toggleable). Argument: that data is "what's available," not "what's happening."
- Adds ~210µs render time (sparkline + gauge + arc combined). Comfortable in 50ms budget.

### Mock states

**Empty session** — fresh start, no transcript, no cost. Notice everything zero-suppressed: no sparkline yet (history is empty), no cost arc, no quota text, no activity row at all.
```
                                                                                 
 Opus 4.7   main   ~/cc-pulseline                                       3%·6k  
 CTX  ▌░░░░░░░░░░░░░░░░░ 3%                              $0.00                
                                                                                 
```

**Working session** (canonical baseline) — mid-CTX, tools and agent in flight, todos active.
```
                                                                                 
 Opus 4.7   feat/status-pane *  ↑3   ~/cc-pulseline                  43%·86k 
 CTX  ███████▊░░░░░░░░░░ 43%   ⠀⠀⢀⡠⠊⠁  TOK 1.2K/s   $3.50 ◑  Q5h 75% 02h    
 ▶ Read main.rs   ▶ Bash test   ✓ ×12   A:Explore [haiku] 2m   • 1/3 todos   
                                                                                 
```

**Heavy activity** — CTX warning tier, multi-tool burst, two agents running, todos progressing. Gauge shifts to `alert_amber`, cost arc warmer.
```
                                                                                 
 Opus 4.7   feat/status-pane *  ↑3   ~/cc-pulseline                  58%·116k
 CTX  ██████████▍░░░░░░░ 58%   ⢀⠠⠊⠉⠉⠉  TOK 2.4K/s↗  $8.20 ◕  Q5h 82% 01h    
 ▶ Read·Edit·Bash·Grep   ✓ ×27   A:Explore A:Plan 4m   • 2/3 todos (auth)   
                                                                                 
```

**Alert critical** — CTX critical (red), dirty git tracking 7 ahead, high burn rate. Gauge in `alert_red`, cost arc fully filled, quota near limit.
```
                                                                                 
 Opus 4.7   feat/status-pane * !5 +2 ↑7   ~/cc-pulseline             87%·174k
 CTX  ███████████████▋░░ 87%   ⠉⠉⠉⠊⠉⠉  TOK 0.8K/s   $25.40 ●  Q5h 94% 22m   
 ▶ Bash long_running.sh   ✓ ×54   A:Debug [opus] Investigate fail 8m         
                                                                                 
```

**Narrow degraded (100–119 cols)** — quota text dropped, everything else preserved.
```
                                                                                
 Opus 4.7   feat/status-pane *  ~/cc-pulseline           43%·86k             
 CTX  ███████▊░░░░░░░░░░ 43%   ⠀⠀⢀⡠⠊⠁  TOK 1.2K/s  $3.50 ◑                  
 ▶ Read main.rs   ✓ ×12   A:Explore 2m   • 1/3 todos                         
                                                                                
```

**Very narrow (80–99 cols)** — sparkline and arc dropped, gauge compressed to 12 cells, raw `%` only.
```
                                                                            
 Opus 4.7  feat/status-pane*  ~/cc-pulseline            43%·86k            
 CTX ████▊░░░░░░░ 43%   TOK 1.2K/s   $3.50                                 
 ▶ Read main.rs   ✓ ×12   A:Explore 2m                                     
                                                                            
```

---

## Variation B — "Flightstrip" (2 rows, maximum density) · OPT-IN

For narrow IDE statuslines. Inspired by ATC flight strips — everything you need at a glance, nothing else.

```
                                                                                
 ◆ Opus 4.7  feat/status-pane*↑3  ~/cc-pulseline    43% ████████░░░░  $3.50  
 ⠀⡠⠊  ▶ Read·Bash  A:Explore 2m  ✓ ×12  Q5h 75% 02h  • 1/3                    
                                                                                
```

**Anatomy:**
- **L1 — Strip.** Single row, everything `primary` or `secondary`. CTX% + 12-cell gauge + total cost.
- **L2 — Live.** Sparkline anchors left, then activity ticker, then meta cluster. Reads left-to-right as a sentence: "trend → tools running → counters."

**Width degradation:**
1. ≥ 110: full
2. 90–109: drop quota cluster
3. 70–89: drop sparkline; compress gauge to 6 cells; drop cost
4. < 70: single row only

**Trade-offs:**
- Two visual tiers only (datum + chrome). No third tier.
- Recommended for users with ≤ 110-col IDE statusline windows.
- Cost arc dropped here — would crowd L1.

### Mock states

**Working session** (canonical baseline) — 12-cell gauge, leading sparkline anchors L2.
```
                                                                                
 ◆ Opus 4.7  feat/status-pane*↑3  ~/cc-pulseline    43% ███████▊░░░░  $3.50  
 ⠀⡠⠊  ▶ Read·Bash  A:Explore 2m  ✓ ×12  Q5h 75% 02h  • 1/3                    
                                                                                
```

**Alert critical** — CTX red, dirty git, high burn. Sparkline shows the climb that got us here.
```
                                                                                
 ◆ Opus 4.7  feat/status-pane*!5+2↑7  ~/cc-pulseline 87% ███████████▋▌ $25.40
 ⢀⠠⠊⠉⠉⠉  ▶ Bash long_run  A:Debug 8m  ✓ ×54  Q5h 94% 22m                     
                                                                                
```

**Narrow (90–109 cols)** — quota cluster dropped from L2.
```
                                                                            
 ◆ Opus 4.7  feat/status-pane*↑3  ~/cc-pulseline  43% ███████▊░░░░  $3.50  
 ⠀⡠⠊  ▶ Read·Bash  A:Explore 2m  ✓ ×12  • 1/3                              
                                                                            
```

**Very narrow (70–89 cols)** — sparkline dropped, gauge → 6 cells, cost dropped from L1.
```
                                                                    
 ◆ Opus 4.7  feat/status-pane*↑3  ~/cc-pulseline   43% ███▊░░       
 ▶ Read·Bash   A:Explore 2m   ✓ ×12   • 1/3                          
                                                                    
```

**Single-row fallback (< 70 cols)** — L1 only; activity row dropped entirely. Identity + CTX% + cost.
```
                                                                
 ◆ Opus 4.7  feat/status-pane*  ~/cc-pulseline  43%  $3.50    
                                                                
```

---

## Variation C — "Console" (4-5 rows, framed monitor) · OPT-IN

Dashboard feel. Wrapped in a `╭─╮ │ ╰─╯` frame. Highest "quality feel," most chrome. Best when statusline is ≥130 cols.

```
                                                                                 
  ╭─ Opus 4.7 ─ feat/status-pane * ↑3 ─ ~/cc-pulseline ────────────────────╮ 
  │  CTX   ████████████░░░░░░░░░░  43% / 200k         ⠀⠀⢀⡠⠊⠁              │ 
  │  TOK   1.2K/s ↗     COST  $3.50 ◐    Q5h  ████████░░░░  75%  02h      │ 
  │  ─────────────────────────────────────────────────────────────────── │ 
  │  ▶ Read main.rs        ▶ Bash test          ✓ Read ×12  ✓ Bash ×8   │ 
  │  A:Explore [haiku] Investigate logic 2m     • TODO 1/3 (auth fix)   │ 
  ╰──────────────────────────────────────────────────────────────────────╯ 
                                                                                 
```

**Width degradation:**
1. ≥ 130: full framed
2. 110–129: drop inner rule, merge to 4-row
3. 90–109: degrade to Variation A
4. < 90: degrade to Variation B

**Quota becomes a real gauge bar** in Console — the frame gives it room. (5d earned its place here, just not in A/B.)

### Mock states

**Working session** (canonical baseline) — full 5-row framed dashboard. Quota is a real 12-cell gauge.
```
                                                                                 
  ╭─ Opus 4.7 ─ feat/status-pane * ↑3 ─ ~/cc-pulseline ────────────────────╮ 
  │  CTX   ████████████░░░░░░░░░░  43% / 200k         ⠀⠀⢀⡠⠊⠁              │ 
  │  TOK   1.2K/s ↗     COST  $3.50 ◑    Q5h  ████████░░░░  75%  02h      │ 
  │  ─────────────────────────────────────────────────────────────────── │ 
  │  ▶ Read main.rs        ▶ Bash test          ✓ Read ×12  ✓ Bash ×8   │ 
  │  A:Explore [haiku] Investigate logic 2m     • TODO 1/3 (auth fix)   │ 
  ╰──────────────────────────────────────────────────────────────────────╯ 
                                                                                 
```

**Heavy activity** — CTX in warn tier, two agents, multiple tool families completed. Quota gauge climbing.
```
                                                                                 
  ╭─ Opus 4.7 ─ feat/status-pane * ↑3 ─ ~/cc-pulseline ────────────────────╮ 
  │  CTX   ████████████████░░░░░░  58% / 200k         ⢀⠠⠊⠉⠉⠉              │ 
  │  TOK   2.4K/s ↗     COST  $8.20 ◕    Q5h  █████████▏░░  82%  01h      │ 
  │  ─────────────────────────────────────────────────────────────────── │ 
  │  ▶ Read·Edit·Bash·Grep                       ✓ ×27  (Read 11 Bash 9)│ 
  │  A:Explore A:Plan [haiku] 4m                 • TODO 2/3 (auth, tests)│ 
  ╰──────────────────────────────────────────────────────────────────────╯ 
                                                                                 
```

**Alert critical** — CTX red, dirty git with file stats, quota near limit. Gauge bars shift to alert colours; the frame stays calm.
```
                                                                                 
  ╭─ Opus 4.7 ─ feat/status-pane * !5 +2 ↑7 ─ ~/cc-pulseline ───────────────╮
  │  CTX   ████████████████████░░  87% / 200k         ⠉⠉⠉⠊⠉⠉              │
  │  TOK   0.8K/s       COST  $25.40 ●    Q5h  ███████████▎  94%  22m     │
  │  ─────────────────────────────────────────────────────────────────── │
  │  ▶ Bash long_running.sh                      ✓ ×54  (Bash 42 Read 12)│
  │  A:Debug [opus] Investigate auth fail 8m     • TODO 1/3 (urgent)     │
  ╰──────────────────────────────────────────────────────────────────────╯
                                                                                 
```

**Wide (≥ 150 cols)** — extra room shows the L2 config row when toggles are enabled (still frames cleanly without it).
```
                                                                                                       
  ╭─ Opus 4.7 ─ feat/status-pane * ↑3 ─ ~/cc-pulseline ─────────────────────────────────────────╮ 
  │  CFG   2 CLAUDE.md  9 rules  3 memories  1 hooks  2 MCPs  4 skills  2 plugins  · 1h 22m   │ 
  │  CTX   ████████████░░░░░░░░░░  43% / 200k         ⠀⠀⢀⡠⠊⠁                                  │ 
  │  TOK   1.2K/s ↗     COST  $3.50 ◑    Q5h  ████████░░░░  75%  02h    Q7d  ████░░░░  32% 5d │ 
  │  ───────────────────────────────────────────────────────────────────────────────────── │ 
  │  ▶ Read main.rs        ▶ Bash test                ✓ Read ×12  ✓ Bash ×8  ✓ Edit ×3      │ 
  │  A:Explore [haiku] Investigate logic 2m           • TODO 1/3 (auth fix)                  │ 
  ╰─────────────────────────────────────────────────────────────────────────────────────────╯ 
                                                                                                       
```

**110–129 col degraded** — inner rule dropped, frame collapses to 4 rows. Falls back to Cockpit-style mid-tier when ≤ 109.
```
                                                                                                
  ╭─ Opus 4.7 ─ feat/status-pane * ↑3 ─ ~/cc-pulseline ───────────────────────────────╮ 
  │  CTX  ████████████░░░░░░░░░░  43%   ⠀⠀⢀⡠⠊⠁  TOK 1.2K/s   $3.50 ◑   Q5h 75% 02h  │ 
  │  ▶ Read main.rs    ▶ Bash test    ✓ ×12   A:Explore 2m   • 1/3                  │ 
  ╰────────────────────────────────────────────────────────────────────────────────╯ 
                                                                                                
```

---

## Comparison summary

| Dimension | A · Cockpit (default) | B · Flightstrip | C · Console |
|---|---|---|---|
| Rows | 3 | 2 | 4–5 |
| Width floor | 80 | 70 | 90 |
| New widgets | gauge + sparkline + arc + tape | gauge + leading sparkline + tape | gauge + sparkline + arc + framed quota gauge |
| Density per row | High | Maximum | Medium |
| Quality feel | Strong | Sharp / opinionated | Strongest |
| Quota visualization | text | text | gauge bar |
| Cost arc | yes | no (cells too tight) | yes |
| Default? | ✅ | for narrow IDEs | for wide IDEs / dashboard mode |

---

## Implementation phases

### Phase A — v1 organize (one PR, no behavior change)

1. **Rename `PaneStyle` enum variants** for clarity:
   ```rust
   enum PaneStyle {
       V1None, V1Zones, V1Grid, V1Cards, V1Sections,
       // V2 variants added in Phase B
   }
   ```
   TOML strings unchanged — pure rename pass, ~15 callsites.

2. **Move v1 frame impls into `src/render/frames/v1/`** — one file per frame:
   ```
   render/
   ├── pane.rs                # enum + dispatcher + apply_pane()
   └── frames/v1/
       ├── none.rs            # V1None — no-op
       ├── zones.rs           # V1Zones — single horizontal rule
       ├── grid.rs            # V1Grid — label column layout
       ├── cards.rs           # V1Cards — per-group frame
       └── sections.rs        # V1Sections — single outer frame
   ```

3. **Consolidate v1 docs** — one section in `docs/pane-styles.md` (new file) showing all 5 v1 frames with monospace mockups, marked "v1 / stable", explicitly says "v2 layouts ship next."

4. **TOML template — group with comments:**
   ```toml
   [pane]
   # v1 frame styles (stable, backward-compatible):
   #   "none" | "zones" | "grid" | "cards" | "sections"
   # v2 layout styles (new, recommended) — Phase B:
   #   "cockpit" | "console" | "flightstrip" | "auto"
   style = "none"
   ```

5. **CHANGELOG entry** — "Pane styles organized into v1 frames / v2 layouts. No behavior change. v2 styles coming next release."

**Cost:** ~1 day. Zero new tests — existing tests preserve behavior. One PR, easy review, zero risk.

### Phase B — v2 implementation (alongside v1)

1. **New theme: Pulseline Aurora.** `src/themes/pulseline-aurora.json` with the 28+3 fields above.
2. **Aurora-tier additions to `ThemePalette`:** `aurora_low`, `aurora_mid`, `aurora_high` as `String` fields. `Option<u8>` in `PresetColors` with `Some(separator)` / `Some(structural)` / `Some(active_cyan)` fallback chain for custom themes that omit them.
3. **Seed aurora values** in all 9 existing themes per the table above. Lint test `tests/theme_aurora_contrast.rs` enforces presence + monotonic gradient (`|low−mid|≥4`, `|mid−high|≥4`).
4. **Widget module** (~250 LOC, no new deps):
   ```
   render/widgets/
   ├── sparkline.rs    # 6-cell braille sparkline
   ├── gauge.rs        # 1/8-step block-bar gauge
   ├── arc.rs          # cost burn arc using ○ ◔ ◑ ◕ ● glyphs
   ├── tape.rs         # recent-tools horizontal strip
   └── mod.rs
   ```
5. **State extensions:**
   - `SessionState.ctx_history: VecDeque<u8>` (cap 30)
   - Repurpose existing `recent_tools` for tape widget
   - Persist via existing `state/cache.rs` — no new infrastructure
6. **v2 layout impls:**
   ```
   render/frames/v2/
   ├── cockpit.rs      # default (A)
   ├── console.rs      # opt-in (C)
   ├── flightstrip.rs  # opt-in (B)
   └── auto.rs         # width-bracket resolver
   ```
7. **L2 config row** — opt-in via `show_claude_md = true` etc. When at least one config segment is enabled in a v2 layout, render a compact config row between identity and cluster. v1 users see L2 unchanged (always-on).
8. **Tests:**
   - 4 new integration tests (cockpit / console / flightstrip / auto-resolver)
   - Aurora contrast lint
   - Existing tonal_strata + theme_strata_contrast continue passing (Aurora seeds satisfy strata too)

**Cost:** ~3-4 days. ~800 LOC added. Bench impact +290µs. Reversible — gate behind `style != v2*` if needed.

### Phase C — flip the default (no fixed timeline)

Trigger: when v2 has settled and feedback is good. *"We'll see."*

Mechanics:
- `default_pane_style()` returns `"cockpit"` (was `"none"`).
- `default_config_toml()` template suggests `style = "cockpit"`.
- CHANGELOG entry: "v2 cockpit is now default. Existing configs with `style = "none"` (or any v1 style) are unaffected."
- v1 users with `style` *unset* in config (relying on default) get the new default. To preserve old look, one TOML edit: `style = "v1-classic"`.
- Optional: ship `cc-pulseline --migrate` that detects "no style set" and offers to write `style = "v1-classic"`.

~10 LOC change, ~30 lines of CHANGELOG. Zero new tests.

---

## Rust framework choice (recap)

**Path 1 — Hand-rolled widgets** is recommended for v2 ship. ~250 LOC across 4 small files, zero new deps, microsecond render cost, full ownership of visual decisions. Reserve ratatui (Path 2/3) for a future v3 if layout grows toward genuine TUI territory (multi-pane, drill-down, charts beyond sparklines).

The choice of widget library is mostly about authoring ergonomics — and at this scale, hand-rolled is the boring, correct choice.

---

## Open / next

The brief is now complete and decision-converged. Next concrete step:

1. **DAG it.** Scope as `specs/statusline-v2/dag.yaml` with two epics:
   - `epic-phase-a` — v1 organize (single PR)
   - `epic-phase-b` — v2 implementation (Aurora palette + widgets + 3 layouts + L2 toggle behavior + lint)
   - Phase C is a one-line config flip when ready, doesn't need its own epic.
2. **Build mocks.** Optionally — render a clickable monospace mock of all three v2 layouts with realistic data so you can pick visual seeds before Phase B begins.

Recommendation: ship Phase A first as one PR, then before Phase B — render the mocks for each layout, do an aurora preview pass per theme, only then start widget implementation. The cleanup PR is a low-risk warmup that makes Phase B's diff readable.
