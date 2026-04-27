# cc-pulseline design docs

Living design documents. Each file is either the architectural reference for
a shipped subsystem or an iteration brief with open items. When a design is
fully absorbed into code with no open follow-ups, it is deleted — the code
is then the source of truth.

## Current docs

| Doc | Scope | Status |
|---|---|---|
| [`statusline-v2-redesign.md`](statusline-v2-redesign.md) | Comprehensive v2 layout brief — cockpit / console / flightstrip / auto, Pulseline Aurora flagship theme, sparkline + braille gauge widgets, v1↔v2 namespace plan | Shipped (commits `7a768cf`, `3566440`, `45a2e00`). Deferred items still on the shelf: tool-burst rate (5b), quota gradient (5d) |
| [`activity-width-budget.md`](activity-width-budget.md) | Architecture spec for the activity-row width-budget allocator (`render/activity/{budget,builder,cell,truncate}.rs`) — every row knows its char budget; truncators (KeepHead / Sentence / CommandSmart) compose | Shipped (commits `cce2b83`, `6e760db`). Active reference for `render/activity/`. |
| [`none-layout-redesign.md`](none-layout-redesign.md) | Density + consistency rework of the default `none` layout. Two iterations. | Iteration 2 Must items shipped (`6e760db`): bracketed parallel cells, width-adaptive completed-tool rows, verb-first Bash. **Open**: A1 (promote all-done todo), A6 (overflow language), open questions 1–4 |

## Deleted (superseded by code)

These docs were retired on 2026-04-27 once their content was either fully in
code (the code is now the spec) or rolled into the docs above:

- `pane-variations.md` — early pane-style explorations; legacy styles retired (`c356ba8`)
- `theme-and-pane-review.md` — reviewed Rail / Box pane styles, both retired
- `zones-style.md` — implemented as `frames/v1/zones.rs`
- `grid-layout.md` — implemented as `frames/v1/grid.rs`
- `frame-sections-cards-review.md` — sections + cards implemented; frame style retired
- `adaptive-quality-review.md` — broad early review; recommendations absorbed into the v2 redesign
- `tonal-strata-redesign.md` — palette redesign shipped (`ded559f`)
- `style-to-layout-taxonomy.md` — `[pane]` → `[layout]` rename shipped (`ca7163b`)

## When to add / delete a doc

- **Add** a doc when the work spans more than one PR or has variations to
  weigh against each other. The doc lives until its open items close.
- **Delete** a doc when every Must / Should item ships and the residual
  Could / Open items are no longer interesting. Don't keep "historical
  record" docs — git history fills that role.
