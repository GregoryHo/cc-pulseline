---
name: preview-layouts
description: Render every cc-pulseline layout against a synthetic fixture so visual differences (tools/agents/todo/quota/sparkline) all show without touching the live Claude Code session. Use when the user says "preview layouts", "show all layouts", "render layouts at width N", "compare layouts", or wants to see them side-by-side. Also use when adding/removing/renaming a layout, adding a new theme, or vetting a redesign before implementation.
user-invocable: true
argument-hint: optional widths, e.g. "140" or "160 110 80"
allowed-tools:
  - Bash
  - Read
  - Edit
---

# Preview Layouts

Run every cc-pulseline layout against a fixed, kitchen-sink-on fixture so layout differences aren't hidden by feature toggles or empty data. Backs up the project config and restores on exit.

## When This Triggers

- User says "preview layouts", "show all layouts", "render every layout", "compare layouts"
- User invokes `/preview-layouts` directly
- User pastes/edits a new layout and wants to see it next to the others
- User updates a theme and wants to verify across layouts
- User wants to compare degradation behaviour across widths

The script writes a synthetic transcript and a temporary preview config — it does not touch the live Claude Code session or the user-level config.

## How To Run

```bash
# Default — single width, 140 cols
./scripts/preview-all-layouts.sh

# One width
./scripts/preview-all-layouts.sh 140

# Compare multiple widths (degradation)
./scripts/preview-all-layouts.sh 160 110 80
```

### Troubleshooting

- `ERROR: .../.claude/pulseline.toml not found` — you're outside the repo root, or the project config was deleted. Re-run `cc-pulseline --init --project` first.
- Build fails with `cargo build --release --quiet` swallowing the error — re-run `cargo check` from the repo root to see the actual compiler diagnostic.
- Trap didn't restore the config (e.g. SIGKILL) — the backup is at `$TMPDIR/pulseline-config-backup.*`; copy it back manually.

The script:
1. Builds release binary (`cargo build --release --quiet`)
2. Backs up `.claude/pulseline.toml` to a temp file
3. Writes a synthetic transcript with running tools, completed tools, an active agent, and an in-progress todo
4. For each `(width, layout)` pair: rewrites the project config with `[layout].name = "<layout>"` and the kitchen-sink toggles, pipes a fixture stdin payload (CTX 43%, cost $4.55, 5h/7d quota) into the binary at `COLUMNS=<width>`
5. Restores the original config on EXIT/INT/TERM (trap)

The fixture preserves the user's currently-selected `theme = "..."` so theme experiments survive the preview.

## When To Edit `scripts/preview-all-layouts.sh`

### Adding a new layout

```bash
LAYOUTS=(none zones grid sections console ledger)
```

Add the new name to the array. Keep ordering meaningful (group related layouts so visual diff is easy — current order is minimal → maximal density).

### Removing / renaming a layout

Update the `LAYOUTS` array, then keep these three sites in sync — drift here is silent:

1. `LayoutStyle` enum in `src/render/pane.rs` — the runtime parser
2. `default_visuals_for` in `src/render/frames/mod.rs` — per-layout default visual specs
3. `docs/layouts.md` — the human-facing layout × visuals reference table

If any of these is out of date, the binary will accept the name but render with the wrong defaults, and reviewers reading the docs will get a wrong picture.

### Toggling more segments on/off in the fixture

Edit the `write_preview_config()` heredoc. Defaults today: every `show_*` is `true`, all `enabled = true`, `max_lines` fixed. If you want to test "what does this layout look like with `show_speed = false`", flip it there — the change applies uniformly across all layouts and the original config is still restored on exit.

### Changing the synthetic transcript

The `TRANSCRIPT` heredoc drives L4a/L4b (completed counts, running tool with target), agent rows, and the TODO line. To exercise more states (e.g. multiple running agents, all-done todo, more completed tools), append more JSONL lines following the same schema:

- `tool_use` with `id` + `name` + `input` for tool start
- `tool_result` with `tool_use_id` for tool finish
- `Agent` tool_use + `progress / agent_progress` event for agent activity
- `TodoWrite` tool_use with `todos` array for todo state

Refer to `providers/transcript.rs` and `tests/fixtures/*.jsonl` for the canonical event shapes.

### Changing the budget / quota numbers

Edit the `PAYLOAD` heredoc. Common adjustments:
- `used_percentage` (CTX) — flip 43 → 65 to see warn color, → 82 to see crit
- `total_cost_usd` / `total_duration_ms` — drives the `$X.XX/h` rate color buckets
- `rate_limits.{five_hour,seven_day}.used_percentage` — flip to 90+ to see the "Limit reached" path

### Adding a new width to default

If you find yourself always passing the same widths, change the fallback at the top:
```bash
[[ ${#WIDTHS[@]} -eq 0 ]] && WIDTHS=(140)
```

## What To Do With The Output

- **Sharing / review**: pipe to `tee preview.txt` and paste the result into a PR or design issue
- **Theme review**: change `theme = "..."` in `.claude/pulseline.toml` first, then run — every layout reflects the new palette
- **Implementation gating**: after a redesign, run with `160 110 80` and visually confirm all three widths still hold up before opening the PR

## Notes

- The script writes a temp fixture transcript, not the live one — running it does not interfere with your active Claude Code session
- Color escapes go through the binary's normal `colorize()` path; `NO_COLOR=1 ./scripts/preview-all-layouts.sh` works to compare ascii fallbacks
- The script does not touch user config (`~/.claude/pulseline/config.toml`) — only the project config in this repo
