# Design review + brief: `none` layout redesign

- **Platform:** CLI / statusline (cc-pulseline, called every ~300ms by Claude Code)
- **Primary job:** zero-chrome multi-line statusline that gives at-a-glance
  awareness of identity, budget, and live activity, with **maximum information
  density per row**
- **Reference:** existing `cc-pulseline` palette + glyph discipline + the
  L1/L2/L3 stability-layering principle from `references/cli.md`
- **Source reviewed:** screenshot at width ≈ 220 cols, `name = "none"`, all
  segments enabled, mid-session with ≥5 completed tools / ≥3 completed
  agents / completed todo
- **Date:** 2026-04-27

---

## Critique (Mode A)

### Hierarchy
The L1→L6 stack puts stable info on top and volatile on the bottom — correct
per the cli.md L1/L2/L3 layering principle. **No change needed**.

### Affordance
Pipe `|` separators read as field boundaries. Icons distinguish segment
classes. **No change needed**.

### Density & rhythm
**The single biggest problem.** The screenshot is two visually different
designs glued together:
- **Top half (L1–L3.5)** — packed horizontally. 8 segments per line, dense.
- **Bottom half (L4–L6)** — one piece of info per line:
  - L4a: 5 completed-tool counts split across **two rows** (`tools_per_line=4`
    setting forces the orphan `✓ Skill ×1` onto its own row)
  - L4b: 2 running tools, OK on one row
  - L5: **3 separate rows** for 3 agents — all with the same prefix
    `✓ general-purpose:`
  - L6: `✓ All todos complete (6/6)` alone on a row

That's ~6 rows of bottom-half content carrying maybe 2 rows worth of
information. The redundancy is structural, not data — the same data
arranged differently fits in 2 rows.

### Consistency
**`✓` is used for three different things**: completed tools, completed
agents, completed todo. The eye can't distinguish them. v2 layouts already
have a glyph vocabulary per role; v1's `none` should borrow it instead of
reusing `✓` everywhere.

Tools use `×N` count format; agents use `(1m)` elapsed; todo uses `6/6`
ratio — three different "I'm done" notations sitting adjacent.

### Accessibility
Color usage is fine (NO_COLOR-respectful, threshold-tied, not the only signal
since labels always present). Text clear at standard terminal sizes.

### Platform fit
`none` is meant to be the "flat, no chrome" baseline. The current design
honors that constraint (no box drawing, no group labels). Improvements must
stay within it.

---

## Adjustments (Must / Should / Could)

### Must

- **Group repeated agents into a single line** (Density). Three rows of
  `✓ general-purpose: <task>` becomes one: `A: general-purpose × 3 (avg 1m)`.
  Concrete: when ≥2 agents share `agent_type`, collapse to the typed-summary
  form and drop the per-agent description. Saves 2 rows per cluster — the
  most common slop in real sessions.

- **Collapse single-occupant rows when wide** (Density). At width ≥ 120,
  merge `✓ All todos complete (6/6)` onto the same row as the running-tools
  ticker (or the agent summary if no running tools). At width < 120, keep
  the current behavior (each on its own row). Concrete: a new
  `format_activity_summary` that joins what fits on one row with `  |  `
  separators, falling back to multi-row only when content overflows.

### Should

- **Differentiate `✓` by role glyph** (Consistency). v2 already chose:
  `▶` for running tools, `✓` for completed tools, `󱙺` (or `A:`) for agents,
  `•` for in-progress todos. `none` should borrow this — keep `✓` only for
  completed tools, use the v2 agent prefix for agents, and either `✓ all
  done` (different color) or simply drop the celebratory todo line when
  todo is `6/6` and there are no in-progress items.

- **`tools_per_line` should be width-derived, not a fixed config** (Density).
  Today the user has to set `tools_per_line = 4`; result is the orphan
  `✓ Skill ×1` row. Compute how many fit in `terminal_width - prefix` and
  use that. Config can stay as a cap.

- **Quota merges into L3 when wide** (Density). At width ≥ 160, append
  `5h:17% 7d:49%` after the cost segment instead of using a dedicated row.
  Below that, keep the current dedicated row. v2's `cockpit` already does
  this width-bracket trick; `none` borrows it.

### Could

- **Truncate long bash command targets at the LEFT, not the right**.
  `sed -i '' 's/^name = ".*"$/...` is the worst possible truncation — the
  meaningful payload (the regex/replacement) is the part that's cut. Either
  show last N chars (so the target verb is visible) or keep the current
  behavior but set a max char cap that's tighter (~40 chars), forcing a
  visual ellipsis earlier.

- **Memory + plugins counts visually outranked by their icons**. `󰧜 4
  memories` and `󰐱 21 plugins` have the same prefix weight as `󰈙 2
  CLAUDE.md` despite being roughly an order of magnitude less actionable.
  Could collapse the always-present zero-info ones (when count = 0) and
  visually de-emphasize the rest. Skipped for now — affects all v1 layouts,
  not just `none`.

---

## Variations (Mode B)

Three remixes of the same data, each preserving the "no chrome" constraint.
Mockups assume width = 200 cols; same data as the screenshot.

### Variation A — Conservative refinement

Keep every visual feature of the current `none`; only collapse the wasted
rows from the critique.

```
 Opus 4.7 (1M, high) | 󰌵 default | 󰚥 2.1.119 | 󰉋 ~/cc-pulseline | 󰊢 feature/status-pane* ↑13 ?13
󰈙 2 CLAUDE.md | 󰱇 10 rules | 󰧜 4 memories | 󱭧 36 hooks | 󰆧 1 MCPs |  61 skills | 󰐱 21 plugins | 󰔚 1d 17h
󰈚 34% (340.0k/1.0M) | TOK 󰍓 1 󰓏 13 ↗307/s 󰆐 263/339.5k | $42.38 ($1.01/h) | 5h 17% (3h 59m) · 7d 49% (2d 20h)
✓ Bash×163 | ✓ Edit×95 | ✓ Read×86 | ✓ Write×7 | ✓ Skill×1
▶ Bash·sed | ▶ Bash·sed | A: general-purpose × 3 (avg 1m) | ✓ TODO 6/6
```

**Changes vs current**: 5 rows instead of 8–10. Quota merges into L3.
Completed tools all on one row (width-derived `tools_per_line`). Agents
collapse to typed summary. Todo merges with running tools.

**Trade-off**: closest to current = lowest user friction. Doesn't reshape
the experience, just removes the slop.

### Variation B — Density-first compact

Identity row strips ornamental segments (style, version, effort) into a
suffix; everything else collapses harder.

```
 Opus 4.7 (1M)  󰊢 feature/status-pane* ↑13 ?13   󰉋 ~/cc-pulseline   1d 17h   default · high · 2.1.119
 cfg ·  2 CLAUDE  󰱇 10 rules  󰧜 4 mem  󱭧 36 hooks  󰆧 1 MCP   61 skills  󰐱 21 plugins
󰈚 CTX 34% 340k/1M    TOK 1↑ 13↓ ↗307/s 263k/340k    COST $42.38 ($1.01/h)    Q 5h 17% · 7d 49%
 ▶ Bash·sed · Bash·sed     ✓ Bash 163 · Edit 95 · Read 86 · Write 7 · Skill 1     A: general-purpose × 3   ✓ TODO 6/6
```

**Changes vs A**: 4 rows. Identity demotes ornamental info to right side.
Activity all on one row (joined by 2 + spaces between groups, dot between
items inside a group). Reads as 4 horizontal "bands" of equal density.

**Trade-off**: highest density, riskier — long sessions with 6+ running
tools or many distinct completed tools will overflow this single activity
row and need width-aware trimming (drop completed tool list first, then
running, then agent summary).

### Variation C — Sectioned dense (novel)

Use **inline section markers** instead of pipe separators between groups.
A short uppercased lowercase-tinted prefix at the start of each band
(`STATE`, `BUDGET`, `WORK`) groups segments without box drawing.

```
ID    Opus 4.7 (1M, high)   󰊢 feature/status-pane* ↑13 ?13   󰉋 ~/cc-pulseline   default · 2.1.119 · 1d 17h
CFG    󰈙 2 CLAUDE  󰱇 10 rules  󰧜 4 mem  󱭧 36 hooks  󰆧 1 MCP   61 skills  󰐱 21 plugins
USE    34% 340k/1M   TOK 1↑ 13↓ ↗307/s 263k/340k   $42.38 (1.01/h)   5h 17% · 7d 49%
RUN    ▶ Bash·sed · Bash·sed     A general-purpose×3 (1m)     ✓ TODO 6/6
DONE   ✓ Bash 163 · Edit 95 · Read 86 · Write 7 · Skill 1
```

**Changes vs B**: 5 rows but with a tiny structural cue (the prefix label),
so the eye can lock onto rows by purpose without box chrome. `RUN` (live
work) and `DONE` (completed counts) split off from each other so the live
row is small and stable, the done row can stretch. The prefix labels are
3-character all-caps in `structural` color — reads as a margin marker, not
content.

**Trade-off**: introduces a "border-light" label column (not a literal box
edge but visually plays a similar role). More hierarchical than A/B but
arguably violates the "no chrome" identity of `none`. Best as a different
named layout (`tagged` / `labeled`?) rather than the new `none`.

---

## Recommendation

**Land Variation A as the new `none`.** It's a strict superset of the
current behavior — same look at narrow widths, but at width ≥ 160 the same
data renders in 5 rows instead of 8–10. Zero new visual primitives, zero
config knobs to learn.

**B is a candidate for a new layout name** (`compact` / `dense`?) shipping
alongside. Different shape entirely — too aggressive to silently inherit
under the `none` name.

**C should not become `none`** — the section labels add chrome that
conflicts with the layout's identity. Worth shipping as its own layout if
the labelled aesthetic is desired (similar to v2's `cockpit` but without
widgets).

## Implementation sketch (if A approved)

Touch points:
- `src/render/layout.rs::render_frame` — add an `activity_row_consolidated`
  branch when `terminal_width >= 120` that joins running tools + agent
  summary + todo into one row
- `src/render/layout.rs::format_completed_tool_lines` — replace the
  fixed `tools_per_line` chunking with width-derived: estimate per-segment
  width, fit as many as possible per line, fall back to config cap as
  upper bound
- New helper `format_agent_summary` — when `frame.agents.iter().filter(…
  same agent_type).count() >= 2`, emit `A: <type> × N (avg <elapsed>)`
  instead of one line per agent
- `format_quota_line` becomes optional in v1 path: when width ≥ 160 and L3
  is the only consumer, append `5h:17% 7d:49%` to L3 instead of emitting a
  dedicated row
- New tests: 3-4 in `tests/none_layout_density.rs` covering wide + narrow +
  many-agents-same-type + many-completed-tools

Estimated diff: ~150 LOC. No new config keys, no breaking changes.

## Open questions

1. The width threshold for "merge quota into L3" — 160 is a guess. Should it
   be tied to `pane_max_width` or computed from the L3 string's projected
   width?
2. Agent grouping — collapse only same-type agents (current proposal), or
   group all completed agents into a single counter (`A: ×3 done`) and only
   running agents get individual rows?
3. Should the activity-row consolidation honor `max_agent_lines = N`? Today
   N=2 means 2 agent rows; with consolidation it's 1 row with N inside.
   Drop the config or repurpose as "max distinct agent types displayed"?
4. Variation B (compact) and C (labeled) — defer entirely, or also draft
   them as proper layouts with names? If yes, what names?

## Next step

Pick A vs A+B vs A+B+C; answer the open questions; then I implement.
