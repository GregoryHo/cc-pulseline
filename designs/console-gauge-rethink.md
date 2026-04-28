# Design review: Console layout — gauge placeholder & width system

- **Platform:** CLI / TUI (cc-pulseline statusline)
- **Primary job:** Let the user glance at the framed dashboard and instantly read CTX usage + quota burn, without parsing a wall of placeholders.
- **Reference:** in-codebase — `src/render/frames/console.rs`, `widgets/gauge.rs`, `frames/shared.rs`. Compared against Cockpit / Flightstrip siblings.
- **Source reviewed:** screenshot from active session (CTX percentage absent → empty bracket; 5h@14% / 7d@74% quota gauges side-by-side).
- **Date:** 2026-04-28

## Observed output

```
╭─ Opus 4.7 (1M context)  high  feature/status-pane*  ↑20  ?5  ~/GitHub/AI/cc-pulseline ──╮
│  📄 2 CLAUDE.md  📜 10 rules  🧠 4 memories  🪝 36 hooks  🧊 1 MCPs  ⚡ 61 skills  🧩 21 plugins  ⏱ 2d 22h │
│  🗄 [                    ]                                                                 │
│  TOK --   COST  $447.03 ◐   5h  [█▏          ] 14% (resets 3h 17m)   7d  [████████▊   ] 74% (resets 1d 15h 17m) │
╰──────────────────────────────────────────────────────────────────────────────────────────╯
```

Two design problems coexist in that CTX row:

1. **The placeholder is the heaviest element on the row** — when CTX has no data the cell renders an icon + 22-cell empty bar (24 visible chars) and nothing else. It outweighs every populated cell on row 4.
2. **Gauge widths feel ad-hoc** — CTX is 22, the quota bars are 12, and similar gauges in Cockpit/Flightstrip pick yet other numbers. There is no documented principle.

## Critique

### Hierarchy

- The empty CTX placeholder commands more visual weight than the live quota bars. That inverts the intended priority: CTX is the "primary instrument", but in the no-data state it's just air inside brackets — air should be the lightest pixel on the row, not the loudest.
- The two quota bars are visually almost the same size as the (live) data row's cost/burn cells, yet they're reference info — secondary. Width signal isn't carrying hierarchy.

### Affordance

- An empty bracket gauge is ambiguous: is it "0% used", "loading", "no data yet", or "broken"? Compare to `ctx_text_cell`, which already has a clean `🗄 --% --/--` placeholder. The signal "I have no number for you" is well-handled in the text variant; the gauge variant inherited none of that work.
- `token_rate_cell` next to it (line 4) **does** show `TOK --` for the no-data state — so the eye expects parallel treatment in CTX. The gauge breaks that pattern.

### Density & rhythm

- Console's intended bracket is ≥130 cols, but the inner content lives in roughly 130-2 = ~128 cells. CTX gauge alone consumes 24 of those (~19%). When that 24-cell block is empty, it's the largest patch of dead space in the entire frame. Even when populated, 24 is generous.
- The quota row uses `parts.join("    ")` (4-space gap) and the two quota cells eat ~40 cells together. If CTX gets 24 and quotas get 14+14=28, the row's visual rhythm is "huge huge quiet quiet" — not what the framed dashboard wants.

### Consistency

CTX gauge interior widths across layouts:

| Layout / mode | CTX gauge | Quota gauge | Ratio CTX:Quota |
|---|---|---|---|
| Console | 22 | 12 | 1.83 |
| Cockpit (full ≥110) | 18 | 12 | 1.50 |
| Cockpit (compact <110) | 12 | 12 | 1.00 |
| Flightstrip (full ≥70) | 12 | 12 | 1.00 |
| Flightstrip (narrow <70) | 6 | 12 | **0.50** ← inverted |

`QUOTA_BAR_WIDTH` is a single global constant (12) used by every layout. CTX is per-layout, hand-picked. That's why Flightstrip-narrow ends up with CTX < Quota — nobody owns the cross-layout invariant "CTX ≥ Quota".

### Accessibility

- Empty placeholder + no number means a user on a screen reader, or a user who is glancing past a no-color terminal, gets effectively zero information from row 3. The text placeholder `🗄 --% --/--` is at least readable and unambiguous.
- The wider the empty bracket, the longer it takes to scan past it on a small screen.

### Platform fit

- Statuslines should degrade their no-data states to the lightest possible shape, not the heaviest. The Cockpit `ctx_text_cell` already follows this. Console is the outlier.
- Brackets-only without content reads to a Unix-trained eye as "spinner without the spinner" — i.e., broken.

## Width-definition status quo (how it's actually defined)

For someone reading the codebase to understand "what's the width rule":

```
Console.GAUGE_WIDTH                = 22         (frames/console.rs:26)
Cockpit.FULL_GAUGE_WIDTH           = 18         (frames/cockpit.rs:22)
Cockpit.COMPACT_GAUGE_WIDTH        = 12         (frames/cockpit.rs:23)
Flightstrip.FULL_GAUGE_WIDTH       = 12         (frames/flightstrip.rs:18)
Flightstrip.NARROW_GAUGE_WIDTH     =  6         (frames/flightstrip.rs:19)
shared.QUOTA_BAR_WIDTH             = 12         (frames/shared.rs:591)
```

Each layout picks its own CTX width by a hand-tuned constant. Quota is one global. There is no shared sizing scheme, and the dispatch hubs (`render_context_visual`, `render_quota_visual`) treat the width arg as an opaque pass-through. Hence the inconsistency.

## Adjustments

### Must

- **Fix the empty CTX placeholder in Console** (Hierarchy + Affordance).
  In `shared::ctx_gauge_cell`, when `context_used_percentage` is `None`, return the `ctx_text_cell` placeholder shape (`🗄 --% --/--`) instead of `🗄 [22 spaces]`. The gauge widget is for showing fill, not for showing absence. Concrete change:

  ```rust
  // ctx_gauge_cell, current:
  match (line3.context_used_percentage, line3.context_window_size) {
      (Some(_), Some(size)) => format!("{icon} {bar}{used_str}{slash}{total_str}"),
      _ => format!("{icon} {bar}"),                       // ← problem
  }

  // proposed:
  match (line3.context_used_percentage, line3.context_window_size) {
      (Some(_), Some(size)) => format!("{icon} {bar}{used_str}{slash}{total_str}"),
      _ => ctx_text_cell(line3, mode, p, color_enabled),  // delegate
  }
  ```
  This single-line redirect makes the no-data state of `gauge` and `text` identical, restores parallel treatment with `TOK --`, and removes the 24-cell empty-bracket eyesore from every layout — not just Console.

### Should

- **Establish a single width scheme for both gauges, and enforce CTX ≥ Quota** (Consistency + Hierarchy).
  Replace the six scattered constants with two named "tier" sizes per layout, expressed as a table that's easy to read in one place. Suggested values:

  | Width breakpoint | CTX gauge (`HERO`) | Quota gauge (`PILL`) |
  |---|---|---|
  | ≥ 130 (Console)            | 18 | 12 |
  | 110–129 (Cockpit full)     | 16 | 10 |
  |  90–109 (Cockpit compact)  | 12 |  8 |
  |  70– 89 (Flightstrip full) | 10 |  8 |
  |  < 70 (Flightstrip narrow) |  8 |  6 |

  Rationale: CTX always ≥ Quota by ~1.4–1.7×; both shrink together as terminal narrows; Console's CTX moves down from 22 → 18 (the row visually tightens, the populated cells get more visual share, and 18 is still a 22% improvement in resolution over Cockpit-full). If 22 is precious for Console, keep it but make Quota 14 in Console only — the rule "CTX ≥ Quota by ≥4 cells" is the invariant, not the absolute numbers.

  Implementation surface: one `gauge_widths_for(width: usize) -> (usize, usize)` helper in `frames/shared.rs`; each layout calls it instead of owning its own constants. `QUOTA_BAR_WIDTH` (the global) becomes a *default* used only when a caller doesn't have a layout context.

- **Document the rule next to the helper** (Consistency).
  One sentence in the helper docstring: *"CTX gauge is the hero instrument and is always wider than Quota; both shrink together with available width."* That's the invariant a future contributor needs to see, not a column of magic numbers.

### Could

- **Tighten the row 4 separator from 4 spaces to 3** (Density).
  `parts.join("    ")` in `tok_cost_quota_row` is generous because it has to compete with the very-wide CTX gauge above. Once CTX is sane, the 4-space gap on row 4 starts to feel sparse. Drop to 3 spaces and let the row breathe more naturally — this is the same gap used by `activity_ticker`.

- **Add an explicit "no data yet" affordance for very early sessions** (Affordance).
  When *both* CTX and TOK are absent (= pre-first-call), Console could collapse rows 3+4 into one row that says `🗄 ready · awaiting first response · COST $0.00`. Specific to the cold-start moment; out of scope for the current fix but worth noting because the empty-gauge bug is most visible exactly then.

## Visual sketch — proposed Console after Must + Should

Cold-start (no CTX yet):

```
╭─ Opus 4.7  feature/status-pane* ↑20 ?5  ~/cc-pulseline ──────────────────╮
│  📄 2 CLAUDE.md · 📜 10 rules · 🧠 4 memories · …                        │
│  🗄 --% --/--                                                            │
│  TOK --   COST  $0.00 ◐   5h [████        ] 14% (resets 3h 17m)   …     │
╰──────────────────────────────────────────────────────────────────────────╯
```

Live state (after first call):

```
╭─ Opus 4.7  feature/status-pane* ↑20 ?5  ~/cc-pulseline ──────────────────╮
│  📄 2 CLAUDE.md · 📜 10 rules · 🧠 4 memories · …                        │
│  🗄 [████▊             ] 22% 220.0k/1.0M                                 │
│  TOK 1.2K/s   COST $447.03 ◐   5h [█▏          ] 14% (resets 3h 17m)  … │
╰──────────────────────────────────────────────────────────────────────────╯
```

Note the placeholder row is now 12 visible cells instead of 26 — it disappears into the layout when there's no data, exactly as it should.

## Open questions

- Is the 22-cell CTX width a deliberate choice (eg. it matches a specific terminal width target you tested), or just a number that landed there during iteration? If deliberate, keep it and only move Quota up to 14 — the *ratio* and the *no-data fix* are what carry the design.
- Do you want the `(None, Some(size))` half-data state (window known, percentage unknown) to render `🗄 [empty bar] --/200.0k` (gauge with size context) or fall to `--% --/200.0k` (text)? The current draft routes both cases to text; the gauge variant could be retained if the size-known case feels too information-poor as text.
- For the width table — pick numbers, or first add the `gauge_widths_for(width)` helper signature and let me wire layouts to it?

## Next step

Pick one of:
1. **Land just the Must** — one-line redirect in `ctx_gauge_cell`, ship it, observe.
2. **Land Must + the helper** — redirect + introduce `gauge_widths_for(width)`, plumb each layout, but keep current numbers (no behaviour change beyond placeholder).
3. **Full rethink** — Must + helper + new width table per the Should section.

I'd recommend (2): biggest leverage with smallest blast radius, leaves the contentious "what numbers" decision for a follow-up where you can A/B them in the live statusline.

---

## Iteration 2 — Option 2 landed; Option 3 preview

**Status (2026-04-28):**
- ✅ Must (placeholder fix) — landed: `ctx_gauge_cell` now delegates to `ctx_text_cell` when `(None, _)`.
- ✅ Helper — landed as `shared::gauge_widths_for(LayoutStyle, width) -> (ctx, quota)`. Layout-aware (not just width-keyed) because Cockpit-compact and Flightstrip-narrow disagree on what to do at the same physical width.
- ✅ Five constants removed (Console.GAUGE_WIDTH=22, Cockpit.FULL=18 / COMPACT=12, Flightstrip.FULL=12 / NARROW=6).
- ✅ All 208 unit tests + integration tests pass under the refactor.
- ⚠ Numbers preserved verbatim — Option 3's "is this the right width" question is below.

### Live render — placeholder fix (Must, Option 2 numbers)

Cold-start session, no `context_window`, no `cost`, no `rate_limits` (the exact state of the user's reported screenshot, right after a fresh session boots):

**Before:**

```
│    [                      ] │
```

**After:**

```
│    --% --/--                │
```

The 22-cell empty bracket is gone. CTX now reads as "no data yet" in the same shape as `TOK --` next to it on row 4.

### Live render — populated state at 140 cols (Console)

Both Option 2 (current) and Option 3 (proposed) rendered at width 140 with the same fixture (`Opus 4.7`, `feature/status-pane*`, `220k/1M ctx`, `5h@14% / 7d@74% quota`):

**Option 2 — preserved current numbers (CTX 22, Quota 12)**

```
╭─ Opus 4.7 (1M context)  feature/status-pane* ↑20 !5 ?6  ~/GitHub/AI/cc-pulseline ──────────────────────────────────────────────────╮
│  󰈙 2 CLAUDE.md  󰱇 10 rules  󰧜 4 memories  󱭧 36 hooks  󰆧 1 MCPs   61 skills  󰐱 21 plugins   2d 15h                                │
│    [████▊                 ] 220.0k/1.0M ⠀⠀⠀⠀⠀⣀                                                                                    │
│  TOK --    COST  $447.03 ◑    5h  [█▋          ]  14% (resets ...)    7d  [████████▉   ]  74% (resets ...)                        │
│  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────│
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

**Option 3 — proposed (CTX 18, Quota 12 — table from "Should" section)**

```
╭─ Opus 4.7 (1M context)  feature/status-pane* ↑20 !5 ?6  ~/GitHub/AI/cc-pulseline ──────────────────────────────────────────────────╮
│  󰈙 2 CLAUDE.md  󰱇 10 rules  󰧜 4 memories  󱭧 36 hooks  󰆧 1 MCPs   61 skills  󰐱 21 plugins   2d 15h                                │
│    [███▉              ] 220.0k/1.0M ⠀⠀⠀⠀⠀⣀                                                                                        │
│  TOK --    COST  $447.03 ◑    5h  [█▋          ]  14% (resets ...)    7d  [████████▉   ]  74% (resets ...)                        │
│  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────│
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

The CTX bar drops from 22 → 18 cells (≈18% narrower). The bar still dominates row 3 but no longer feels disproportionate to the populated cells on row 4. Trailing whitespace on row 3 grows by 4 cells — that whitespace reads as "row 3 is intentionally light" rather than "row 3 is broken / loading".

Quota gauges are unchanged at 12 cells in this preview. If the *ratio* matters more than absolute size, you can also try:

**Option 3b — keep CTX 22, bump Quota to 14**

This would widen the quota bars to 14 cells, restoring CTX/Quota visual hierarchy without shrinking the hero. Not rendered yet — say the word.

### Live render — Flightstrip narrow at 80 cols (the inversion that Option 3 fixes)

Original CTX(6) < Quota(12) on this layout. Option 3 inverts it to CTX(8) > Quota(6). The flightstrip-narrow row at width 80 collapses to a single line, so the visual diff is small in this fixture but the gauge itself reads more like a "primary instrument" once it's wider than the secondary one.

### What Option 3 buys you

1. **Console row 3 is no longer the heaviest line** — CTX bar shrinks 22→18 (4 cells back to the row's whitespace budget).
2. **Flightstrip narrow stops inverting hierarchy** — CTX(8) > Quota(6) at the smallest layout.
3. **Cockpit gets a deliberate intermediate** — full(16,10) / compact(12,8) instead of full(18,12) / compact(12,12) which had the curious property of compact and full sharing a quota size while CTX shrunk.

### What Option 3 *doesn't* buy you in the demo render

The current demo fixture has very low 5h quota usage (14%), so the quota bar shape is dominated by the empty portion regardless of size. The Option 3 sizing differences will show more clearly on a session with higher quota burn (50–80%).

### Decision needed

- **3a (proposed table)** — change to (Console 18/12, Cockpit 16/10 or 12/8, Flightstrip 10/8 or 8/6). Modest CTX shrink, fixes the Flightstrip inversion.
- **3b (alt)** — keep Console CTX=22 but bump Quota to 14 in Console only; leave others as Option 2.
- **Stay on 2** — ship current numbers, revisit later.

Tell me which (or supply your own numbers) and I'll wire them through `gauge_widths_for` — that's now the only place that needs editing.

---

## Iteration 3 — Option 3a landed

**Status (2026-04-28):** ✅ Numbers wired into `gauge_widths_for`. All 208 unit + integration tests pass.

The shipped breakpoint table lives in the doc-comment of `shared::gauge_widths_for` (`src/render/frames/shared.rs`) — that is the single source of truth, so re-tuning shows up in IDE hover and in `cargo doc`. Summary: CTX ≥ Quota at every breakpoint, ratios between 1.25× and 1.60× — the previous Flightstrip-narrow inversion (CTX=6 < Quota=12) is gone.

### Final live render (Console @ 140 cols)

```
╭─ Opus 4.7 (1M context)  feature/status-pane* ↑20 !5 ?6  ~/cc-pulseline ────────────────────────────────────────────────────────────╮
│  󰈙 2 CLAUDE.md  󰱇 10 rules  󰧜 4 memories  󱭧 36 hooks  󰆧 1 MCPs   61 skills  󰐱 21 plugins   2d 15h                                │
│    [███▉              ] 220.0k/1.0M ⠀⠀⠀⠀⠀⣀                                                                                        │
│  TOK --    COST  $447.03 ◑    5h  [█▋          ]  14% (resets ...)    7d  [████████▉   ]  74% (resets ...)                        │
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

Cold-start (no CTX data) — same row 3 placeholder lands as `--% --/--`, no empty bracket.

### Future tweaks (single-line edits)

Any further re-tuning lands in one match arm in `gauge_widths_for`. To bump Console quota to 14 (Option 3b spirit), change `Console => (18, 12)` → `Console => (18, 14)` and the entire pipeline picks it up — `render_quota_visual` reads `bar_width` from the helper, no other call sites need touching.
