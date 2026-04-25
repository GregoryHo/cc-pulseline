# Pane styles

`pane.style` controls how cc-pulseline frames the rendered statusline rows.
There are two non-overlapping namespaces under one config key:

| Namespace | Status | Default? | TOML values |
|---|---|---|---|
| **v1 frames** | stable | yes (`"none"`) | `none`, `zones`, `grid`, `cards`, `sections` |
| **v2 layouts** | coming next release | no | `cockpit`, `console`, `flightstrip`, `auto` |

This page documents the **v1 frames** — the original frame primitives. They
wrap rendered rows in chrome (rules, label columns, borders) but do not
introduce new widgets or change the per-row layout grammar. v2 layouts ship
next; see `designs/statusline-v2-redesign.md` for the design record.

> **Naming note.** TOML strings (`"none"`, `"zones"`, …) are stable and
> unchanged. Internally the Rust `PaneStyle` enum carries a `V1` prefix
> (`V1None`, `V1Zones`, …) to leave room for the v2 variants — this is a
> source-code rename only, no config migration needed.

All v1 frames respect the same shared options:

```toml
[pane]
style       = "..."     # see below
width_mode  = "auto"    # "auto" | "terminal" | "fixed"
fixed_width = 100       # only used when width_mode = "fixed"
min_width   = 60        # skip framing when terminal can't fit this many cols
max_width   = 160       # clamp auto-sized frames to this many cols
cc_margin   = 4         # cols subtracted from detected width in "terminal" mode
tonal_strata = true     # 2-tier separator tint (see docs/theme-palette.md)
```

The `min_width`, `max_width`, `cc_margin`, `tonal_strata`, and segment
toggles all work identically across every v1 frame and will continue to
work under the v2 layouts.

---

## `none` — flat output (default)

No decoration. Rendered rows pass through unchanged. This is the v1 default
and the lowest-overhead style.

```
M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline | G:feat/status-pane *
1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs | 4 skills | 1h 22m
CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50
```

**When to pick it.** Want the minimal status line; care about screen real
estate above all else.

---

## `zones` — single labelled rule between state and activity

Inserts one horizontal rule (`─── activity ───`) between the **state** rows
(Identity / Config / Budget) and the **activity** rows (Tools / Agents /
Todos). Echoes Claude Code's own input-box rules so the statusline reads
as a continuation of CC's chrome.

```
M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline | G:feat/status-pane *
1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs | 4 skills | 1h 22m
CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50
─── activity ─────────────────────────────────────────────
T:Read main.rs | T:Bash cargo test
A:Explore [haiku]: Investigate logic (2m)
TODO:Fixing auth bug (1/3)
```

Adds **+1 row** when activity is present; degrades to plain output otherwise.

**When to pick it.** Want a single visual cue marking "this is what's
happening" without introducing borders.

---

## `grid` — fixed label column + divider

Table layout: a fixed-width label column, a `│` divider, and right-padded
content. Every line begins and ends at the same visual position, which
solves the jagged-right-edge problem of `none` without adding rows.
Activity continuation rows show a blank label so the divider lines up.

```
Identity  │ M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline | G:feat/status-pane *
Config    │ 1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs | 4 skills | 1h 22m
Budget    │ CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50
Activity  │ T:Read main.rs | T:Bash cargo test
          │ A:Explore [haiku]: Investigate logic (2m)
          │ TODO:Fixing auth bug (1/3)
```

Adds **0 rows**. Costs ~12 cols on the left for the label column.

**When to pick it.** Want explicit group labels and aligned right edges, but
don't want the row overhead of framed styles.

---

## `cards` — one independent frame per group

Each non-empty group becomes its own `╭─┬─╮ … ╰─┴─╯` card, stacked
vertically. All cards share the same global `max_label_width` and
`max_content_width` so their internal divider and outer walls align
column-for-column.

```
╭──────────┬───────────────────────────────────────────────────────────────╮
│ Identity │ M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline    │
╰──────────┴───────────────────────────────────────────────────────────────╯
╭──────────┬───────────────────────────────────────────────────────────────╮
│ Config   │ 1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs         │
╰──────────┴───────────────────────────────────────────────────────────────╯
╭──────────┬───────────────────────────────────────────────────────────────╮
│ Budget   │ CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50                │
╰──────────┴───────────────────────────────────────────────────────────────╯
╭──────────┬───────────────────────────────────────────────────────────────╮
│ Activity │ T:Read main.rs | T:Bash cargo test                            │
│          │ A:Explore [haiku]: Investigate logic (2m)                     │
│          │ TODO:Fixing auth bug (1/3)                                    │
╰──────────┴───────────────────────────────────────────────────────────────╯
```

Adds **+2 rows per non-empty group** (top + bottom of each card).

**When to pick it.** Want strong visual separation between groups; have
plenty of vertical room.

---

## `sections` — single outer frame with internal separators

One outer `╭─┬─╮ … ╰─┴─╯` wrapper around every group, with a `├─┼─┤`
separator between every pair of non-empty groups. Reads as one container
with explicit internal dividers.

```
╭──────────┬───────────────────────────────────────────────────────────────╮
│ Identity │ M:Opus 4.7 | S:explanatory | CC:2.1.119 | P:~/cc-pulseline    │
├──────────┼───────────────────────────────────────────────────────────────┤
│ Config   │ 1 CLAUDE.md | 5 rules | 2 memories | 1 hooks | 2 MCPs         │
├──────────┼───────────────────────────────────────────────────────────────┤
│ Budget   │ CTX:43% (86.0k/200.0k) | TOK I:10 O:20 | $3.50                │
├──────────┼───────────────────────────────────────────────────────────────┤
│ Activity │ T:Read main.rs | T:Bash cargo test                            │
│          │ A:Explore [haiku]: Investigate logic (2m)                     │
│          │ TODO:Fixing auth bug (1/3)                                    │
╰──────────┴───────────────────────────────────────────────────────────────╯
```

Adds **+2 rows + 1 per gap** between non-empty groups. Cheaper row-wise
than `cards` (no double-border gap between adjacent groups) and gives the
same per-group separation.

**When to pick it.** Want the framed dashboard feel of `cards` but
without the row cost between groups.

---

## Width handling

When `pane.terminal_width` (auto-detected via `COLUMNS` or `ioctl`) is
narrower than `pane.min_width`, the active frame is bypassed entirely and
rows render flat — the binary will not output a half-collapsed frame.

`width_mode = "terminal"` makes framed styles span the detected terminal
width minus `cc_margin` cols. The default `cc_margin = 4` is the
empirically verified safe value for Claude Code 2.1.119; CC allocates the
statusline a sub-region ~1–4 cols narrower than the raw terminal, and lines
at exactly the raw width trigger wrap.

`width_mode = "fixed"` pins a frame to `fixed_width` cols regardless of
terminal size; useful for screenshot fixtures and reproducible mockups.

---

## What's next

v2 layouts (`cockpit`, `console`, `flightstrip`, `auto`) bring widgets —
sparklines, gauges, cost-burn arcs — and a width-bracket auto-resolver.
They share the same `pane.style` TOML key, the same segment toggles, and
the same theme palette. Existing v1 configs continue to work unchanged.

See `designs/statusline-v2-redesign.md` for the full design record.
