# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.5] - 2026-05-19

Feature release on two axes: sub-agent TODO visibility from dispatched
agent transcripts, and a ledger spacing fix that eliminates trailing
blank rows above the bottom frame plus a new `ledger_dense` toggle.

### Added

- **Sub-agent TODO tailing** — When the active session dispatches one or
  more sub-agents (via the `Agent` tool), the TODO row now surfaces
  in-progress state from the sub-agent's own transcript rather than
  silently reporting the lead agent's empty list. Counts in TOOL /
  AGENT / TODO rows aggregate across the lead session and active
  sub-agents so the statusline reflects the whole tree of work.
- **`[layout] ledger_dense`** (default `false`) — compact rhythm for
  the `ledger` layout that drops every inter-group blank row. Useful
  when statusline vertical room is tight. Group separators in legacy
  mode (`false`) are now inserted *lazily*, so no trailing blank can
  reach the bottom frame regardless of which groups are populated.

### Fixed

- **Stray trailing blank above ledger `bottom_frame` when TOOL was the
  last non-empty group** — the inter-group blank after TOOL was pushed
  unconditionally whenever any tool rows rendered, even when AGENT /
  TODO were empty. The render pipeline now builds groups first and
  inserts separators lazily, so the bottom frame closes flush against
  the last content row regardless of which groups are present.

### Internal

- Ledger `render()` refactored to a `Vec<Vec<String>>` group pipeline
  with a single lazy-separator flatten loop, replacing three hardcoded
  `blank_row()` push points in `src/render/frames/ledger.rs`.
- 3 new ledger spacing tests:
  `ledger_no_trailing_blank_when_tools_last`,
  `ledger_dense_drops_all_inter_group_blanks`,
  `ledger_dense_false_preserves_legacy_spacing`
  (`tests/ledger_layout.rs`).
- New transcript dispatcher for sub-agent events with state-merge
  helpers in `src/state/mod.rs` and `src/providers/transcript.rs`;
  covered by `tests/sub_agent_transcripts.rs` and an updated
  `tests/adaptive_performance.rs`.

## [1.1.4] - 2026-05-13

Patch release fixing a regression introduced by v1.1.3's ledger headline
overflow cascade: the `CC:` version pill (and other optional pills) was
being dropped before `project_path` / `git_branch` were compressed, so a
long project path could hide `CC:` even when the user had `show_version =
true`. The cascade is now inverted — data is compressed first, pills drop
only as the last resort before tail-ellipsis truncation.

### Fixed

- **Ledger headline dropping `CC:` version pill (and other optional
  pills) before compressing long path/branch** — `top_frame`'s overflow
  cascade was ordered `bounded(drops pills) → compress path → compress
  branch → tail`. With a long `project_path` (e.g. 57+ chars) the
  `identity_headline_bounded` call at stage 1 already dropped `version`
  via `DROP_ORDER`, so subsequent compression stages saw a pill-less
  headline and the user lost `CC:` even at wide terminals. The cascade
  is now reordered to `full → compress path → compress branch → bounded
  (drops pills) → tail`: pill drop only fires after both data
  compressions failed to fit.

### Internal

- Regression test `ledger_preserves_show_version_when_path_is_long`
  (`tests/ledger_layout.rs`) — Pins the new behavior using the
  platform-web case (57-char path, 33-char branch) across widths 120
  through 240; asserts `top.contains("2.1.138")` (the CC: pill version
  string) at every width.

## [1.1.3] - 2026-05-12

Patch release fixing two ledger-rendering bugs exposed by the v1.1.2 `CC:`
pill addition: a top-frame border misalignment under long path/branch, and
a silent fallback from ledger to Console layout when terminal-width
detection fails (the common case in the Claude Code statusline hook
context).

### Fixed

- **Ledger top-frame border misalignment** — When the identity headline
  exceeded the inner frame width, the trailing-dashes math used
  `saturating_sub`, which kept the right corner `╮` visually but did not
  truncate the headline itself; the result was a `╮` pushed past the body
  rows. `top_frame` now caps the headline via `identity_headline_bounded`
  (drops optional pills in `version → git_stats → worktree → effort →
  agent → thinking` order), then compresses `project_path` and
  `git_branch` with segment-aware ellipsis (`{first}/…/{last}` →
  `…/{last}` → `keep_tail`), with `truncate_to_width` as a final safety
  net.
- **Ledger silently downgrading to Console layout** — When
  `terminal_width` detection failed (statusline hook context with no
  accessible `/dev/tty` and `COLUMNS=0` or unset), ledger fell back to
  Console (sections-style with `├─┼─┤` row separators and `│ TAG │ body │`
  cell borders) — but the hook context is the common-case invocation, so
  users were losing their chosen `ledger` layout silently. The renderer
  now assumes `pane_max_width` (default 140) when detection fails; only
  an *observed* width below 90 still triggers Console fallback.
- **`resolve_terminal_width` accepts `COLUMNS="0"` as a valid width** —
  Both the env-var branch and the ioctl branch now reject 0, falling
  through cleanly so the function returns `None` when no usable width
  exists.

### Internal

- **`compress_path_segments`** (`render/activity/truncate.rs`) — New
  helper that retains the leaf segment and progressively drops the
  middle (`{first}/…/{last}` → `…/{last}` → `keep_tail`). Char-safe; used
  by ledger's `top_frame` overflow cascade for both project paths and
  branch names containing `/`.
- **`identity_headline_bounded`** (`render/frames/shared.rs`) — Width-aware
  wrapper around `identity_headline` that progressively turns off
  optional pills until the rendered width fits. Mirrors `config_row`'s
  clone-mutate-remeasure pattern; never mutates `Line1Metrics`.
- **Doc-rot fix**: `docs/theme-palette.md` Tier Summary header updated
  from "31 unique fields" to "34 unique fields" to match the current
  `ThemePalette` struct (drift from 1.1.1's `tag_label` / `head_agent` /
  `head_thinking` additions).

## [1.1.2] - 2026-05-07

Render the Claude Code version pill (`CC:`) in the ledger layout's identity
headline. Other layouts already honored the `show_version` toggle via
`render::layout::format_line1`; ledger had a parallel render path
(`identity_headline` in `render/frames/shared.rs`) that was missing the
branch, so flipping `show_version` was a no-op for ledger users.

### Added

- **`CC:` pill in ledger identity headline** — Wires `show_version`
  through `identity_headline` so the ledger top-frame title now shows
  `CC:{version}` before the model name, using the same icon (`ICON_VERSION`)
  and `secondary` palette tier as the non-ledger layouts.

## [1.1.1] - 2026-05-01

Decouple the L1 HEAD pills (`AG:` agent, `[T]` thinking) and the ledger
TAG column from the inherited palette tiers they were borrowing colors
from. Themers can now tune each role independently, and the M:/AG:
collision on L1 is fixed in every built-in theme.

### Added

- **`tag_label` / `head_agent` / `head_thinking` palette fields** — Three
  new optional `ThemePalette` slots for layout-specific roles. Each
  falls back to the prior tier when absent from theme JSON
  (`secondary` / `active_purple` / `active_amber`), so existing custom
  themes keep working unchanged. Override via `[colors]` TOML section
  or per-theme JSON. Same pattern as `strata_*` and `aurora_*`.
- **"Heads & Tag" section in `--palette-map`** — Runtime palette anatomy
  printer lists the new fields alongside the existing tier sections.
  L1 anatomy preview also gains `AG:greg-bot` and `[T]` rows.

### Changed

- **L1 `AG:` pill rewired to `head_agent`** — Was `stable_blue`,
  visually colliding with the `M:` model pill. Default fallback puts
  AG: on `active_purple` so it matches L5+ `A:Explore` agent rows.
- **L1 `[T]` thinking pill rewired to `head_thinking`** — Was
  `active_purple` (collided with the agent purple). Default fallback
  is `active_amber`.
- **Ledger TAG column rewired to `tag_label`** — Was `secondary`,
  dragging L1 secondary text along with any TAG tuning.
- **Macro 4-group label `Config` → `ENV`** in zones / grid / sections /
  console layouts. Aligns with the ledger TAG vocabulary
  (`ENV / CTX / TOK / COST / ...`) and the underlying `EnvCollector`
  provider. No config schema change; only the rendered group label.
- **Per-theme intentional values for all 10 built-in themes.** Each
  theme authors `tag_label / head_agent / head_thinking` designed
  for that theme's palette story. Mono-accent themes preserve their
  contract on the activity tier; `head_agent` on L1 diverges where
  needed for visual disambiguation (e.g. echo-sub-zero picks
  `active_coral` for AG: rather than colliding with `stable_blue`).

### Fixed

- **`identity_headline` (Console / Ledger frame title) AG / [T] colors.**
  The title-hoist formatter had its own duplicate copy of the AG / [T]
  color path that wasn't picked up by the L1 rewire on first pass. It
  now reads the same `head_agent` / `head_thinking` fields as the flat
  L1 formatter.

## [1.1.0] - 2026-04-30

> **⚠ Breaking changes** are clearly marked below. The `[pane]` → `[layout]`
> config section rename and the removal of four layouts (`cards`, `cockpit`,
> `flightstrip`, `auto`) require user-config updates. Old `[pane]` blocks
> are silently ignored; configs naming a removed layout fall back to
> `console` with a stderr warning.

### Added

- **`ledger` layout** — New label-value layout with a fixed-width `TAG`
  column (`ENV / CTX / TOK / COST / 5h / 7d / TOOL / AGENT / TODO`); blank
  rows separate groups, parallel agent batches get their own group, and
  the CTX row ships sparkline + delta-time annotation by default. Tallest
  layout, designed for ≥110 cols. See `docs/layouts.md`
- **`gauge` widget — marks-on-track form** — Replaces the prior bracket
  battery `[████▎    ]` with a bracketless bar (`▰▰▰▰▰▰···──·──` icon,
  `======...:--:--` ascii) where threshold positions are marked with `·`
  (or `:` in ascii) on the empty track. CTX uses window-aware marks
  (200k window → 55%/70%); quota uses fixed marks at 50%/85%. Fill
  colour comes from the existing `color_for_*_pct` helpers — chroma
  escalates through good/warn/critical at the same points the marks
  call out. New widget signature: `(pct, width, marks, fill_color,
  palette, mode, color)` — caller owns thresholds and fill colour.
- **`quota_visual` config** — Per-segment spec (`"text"` | `"gauge"`)
  routes through a new `render_quota_visual` dispatch hub. Per-layout
  defaults: `none`/`zones`/`grid` → `text`; `sections`/`console`/
  `ledger` → `gauge`. User overrides via `[segments.quota] visual = "..."`.
- **`context_visual` gains `gauge` keyword** — `context_visual` now
  composes `text` + `gauge` + `sparkline` via `+`-joined spec. Defaults:
  flat layouts → `text`; `ledger` → `text+sparkline`. CTX gauge is
  opt-in everywhere; users add `"gauge"` to the spec to enable.
- **`agents_visual` config** — Per-segment composable spec for agent
  rendering with `name` + `description` + `model` atoms.
- **`show_ctx_sparkline` toggle** — `[segments.budget]` opt-in (default
  `false`) for a 6-cell braille trend chart of CTX% history. Auto-hidden
  under `display.icons = false`.
- **Pulseline Aurora theme + matte-carbon-neon theme** — Two new
  built-in themes. `pulseline-aurora` is the flagship: it uses the new
  3-stop aurora gradient (`aurora_low` / `aurora_mid` / `aurora_high`)
  to colour the ledger CTX sparkline by consumption velocity (calm /
  active / hot). `matte-carbon-neon` adds an industrial chrome +
  piercing neon variant. Theme count grows from 8 → 10 built-in presets.
- **Tonal strata — palette-native 2-tier chrome** — `ThemePalette`
  gains `strata_state` and `strata_activity`, tinting the `|` separator
  differently on state rows (Identity / Config / Budget / Quota) vs.
  activity rows (Tools / Agents / Todos). All built-in themes ship a
  chrome pair for both dark and light variants. A `tests/
  theme_strata_contrast.rs` lint enforces `|state − activity| ≥ 3` on
  the ansi256 scale. `pane.tonal_strata = true` is the default.
- **Per-color overrides for strata** — `[colors]` TOML accepts
  `strata_state` and `strata_activity`.
- **`--palette-map` shows the Strata tier** — Field reference output
  ends with a `Strata` row listing both fields and their ansi256 codes.
- **Q7d in framed layouts** — `console`, `sections`, and `ledger`
  honour `show_quota_seven_day = true`, rendering the seven-day window
  alongside Q5h.
- **Frontmatter hooks detection** — Env collector counts hooks declared
  via skill frontmatter alongside `settings.json` `hooks` entries.
- **Plugins count metric** — New L2 segment counts active Claude Code
  plugins discovered from `~/.claude/settings.json`.
- **Effort + thinking display** — When CC payloads carry `effort` or
  `thinking` fields, they surface alongside the model identity.
- **Tool target extraction expanded** — Coverage for additional CC
  tool names + new payload fields (see `tests/payload_new_fields.rs`,
  `tests/new_tool_targets.rs`).
- **`preview-all-layouts.sh` dev script** — Renders every layout
  against a synthetic fixture at multiple widths (`./scripts/
  preview-all-layouts.sh 160 110 80`) without touching the live CC
  session. Companion skill at `.claude/skills/preview-layouts/`.
- **MSRV verification CI job** — `chore(msrv): bump to 1.85`. CI now
  pins and verifies the Minimum Supported Rust Version.

### Changed

- **BREAKING: `[pane]` → `[layout]`, `style` → `name`** — Config section
  and key rename. Old `[pane]` blocks are silently ignored — users must
  rename to `[layout]` to keep their settings.
- **BREAKING: 4 layouts removed** — `cards`, `cockpit`, `flightstrip`,
  `auto` are gone (along with the `arc` and `tape` cluster sub-widgets).
  Configs naming any of them fall back to `console` with a stderr
  warning. The shipping six are: `none`, `zones`, `grid`, `sections`,
  `console`, `ledger`.
- **`console` rebuilt** — Now `sections` + identity-in-frame-title,
  recommended ≥110 cols. Replaces the prior cluster-era `console` impl.
- **Activity-row rendering rebuilt around a width-budget allocator** —
  The four ad-hoc per-line formatters in `render/layout.rs` and the
  inline `truncate_str`/`truncate_path` calls in `providers/transcript.
  rs::extract_target` are replaced by a single `render::activity`
  module: 5 content-typed truncation strategies (`KeepHead`, `KeepTail`,
  `KeepMiddle`, `Sentence`, `CommandSmart`), `Cell`/`CellBody`/
  `TailFragment` data shapes, a row allocator, and a tool-kind→strategy
  table. **Bash command targets** now surface the meaningful payload
  (regex / file arg) instead of the verb+flag chain — e.g. `sed -i ''
  's/^name = …` becomes `s/^name = …`. **Agent batches** spawned in one
  assistant turn (sharing the Anthropic `message.id`) collapse to one
  `parallel` row showing `×N` count and the first description.
  Heterogeneous parallel groups (mixed `agent_type`s in one turn) get
  a dedicated `‖ ×N parallel:` row joining each `type: description`
  with ` + `. Sequential overflow emits `… + K more agents` at the
  top of the agent rows. `tools_per_line` / `max_tool_lines` /
  `max_agent_lines` / `max_completed_tools` become caps rather than
  fixed counts — the budget allocator decides how much fits per row.
  See `designs/activity-width-budget.md`.
- **`AgentSummary` and `PendingTask` schemas gain `message_id:
  Option<String>`** — Captured by the transcript Path-1 dispatcher
  from `event.message.id` (Anthropic API). Drives batch detection.
  `#[serde(default)]` keeps existing cache files compatible (legacy
  entries get `None`, classify treats them as `Single` agents).
- **Theme palette grows from 26 → 31 fields** — Adds `strata_state`,
  `strata_activity`, and aurora-tier fields. Custom themes missing
  the new fields fall back to `emphasis_separator`/`emphasis_structural`
  with a one-time warning. The `palette_mapping` reference table in
  `docs/theme-palette.md` is updated.
- **Sparkline takes caller-supplied fill colour** — Each call site
  picks the colour (e.g. `ledger` picks aurora-tier colour from CTX
  consumption velocity). The widget no longer encodes a default colour.
- **CTX thresholds simplified to fixed 55/70** — Drops the prior
  window-aware threshold logic. Window-aware mark positions live in
  the new `palette.ctx_marks_for_window(window_size)` helper instead.
- **CTX text format** — `used / total` now uses a slash separator
  (was a space): `86.0k/200.0k`. Icon prefix restored when the gauge
  replaces the percentage.
- **Identity row order** — Project segment now emits before git+stats,
  matching the console-v2 spec.
- **Composable rendering refactor (Phases 1-3)** — Per-segment visual
  specs flow through a unified dispatch-hub pattern in
  `render/frames/shared.rs`. Widgets share a unified `glyph_mode`
  signature with explicit ASCII fallbacks. Layouts call hubs; never
  call `widgets::*::render` directly. See `.claude/rules/patterns.md`
  Visual Dispatch Hub Pattern.
- **`recent_tool` cell builder hosted in the widgets layer** — Moved
  from `widgets/` to `render/activity/cells/`; transitional inline
  formatter dropped.
- **Passive worktree detection + token-based 1M-context thresholds** —
  Worktree state is now detected without git invocations; CTX colour
  thresholds adapt to the active context-window size.
- **Adaptive pane width with `cc_margin` safety margin** — Pane sizing
  subtracts a configurable margin to leave room for CC's own statusline
  trim.
- **Aligned with Claude Code 2.1.119+** — Stdin payload changes (new
  fields, renamed fields) absorbed.

### Removed

- **BREAKING: `cards`, `cockpit`, `flightstrip`, `auto` layouts** — See
  Changed above.
- **Cluster-only helpers** — ~14 helpers purged from
  `render/frames/shared.rs`; `arc` / `tape` sub-widgets removed.
- **v1/v2/cluster terminology** — Scrubbed from source. Layouts are
  listed alphabetically without internal-organization labels.
- **Old bracket-bar gauge implementation** — Replaced at the same
  module path (`widgets::gauge::render`); existing
  `context_visual = "gauge"` configs continue to work but render the
  new marks-on-track form. No deprecation shim — visual change is
  accepted (see `git log` for the F-style gauge migration rationale).

### Fixed

- **`ledger`: agent rendering** — Routed through `activity::builder` so
  the bracketed/bucketed format matches the console layout (`397ebca`).
- **`ledger`: tool target truncation** — Targets now truncate to fit
  `content_width` (subtracting `TAG_COL_WIDTH`, not just indent), and
  the layout falls back to `console` when terminal width is unknown.
- **`ledger`: parallel agent groups + right margin** restored.
- **`transcript`: register sub-agents as ACTIVE on Agent tool_use** —
  Sub-agents spawned via the `Agent` tool now appear in the active
  list immediately rather than only after their first progress event.
- **P2 codex findings on PR #11** — Env / layout fixes from review.
- **Tests**: use `serde_json::json!` macro to escape Windows paths
  correctly.
- **`cc_margin` overflow, stale groups, over-aggressive activity drop**
  in adaptive width degradation.

## [1.0.6] - 2026-03-24

### Added

- **Theme system with 8 built-in presets** — `ThemePalette` struct replaces hardcoded color constants. Presets: tokyo-night (default), echo-sub-zero, titanium-precision, cnc-telemetry, cyberdeck-hud, stark-hud, mako-reactor, aburaya-twilight. Set via `theme = "preset-name"` in config
- **Custom themes** — Drop a JSON file in `~/.claude/pulseline/themes/` and reference by filename (without `.json`). Full 26-field `palette_mapping` controls all rendered colors
- **Per-color TOML overrides** — `[colors]` section in config applies on top of any theme preset (e.g., `alert_red = 160`)
- **`--preview` CLI flag** — `cc-pulseline --preview` renders all themes; `--preview theme1 theme2` previews specific themes side-by-side
- **`variant` config field** — Explicit dark/light selection independent of theme name (`variant = "light"`)
- **`--select-theme` CLI flag** — Interactive theme selector with color swatches and descriptions. Writes to user config; `--select-theme --project` writes to project config
- **`--palette-map` CLI flag** — Colored ASCII diagram showing all 26 `palette_mapping` fields → rendered UI elements, grouped by category

### Changed

- **Render functions use `&ThemePalette`** — Layout functions receive the resolved palette from `RenderConfig` instead of threading `EmphasisTier` enums. All color selection goes through palette fields
- **Theme presets use embedded JSON** — 8 theme files in `src/themes/` loaded via `include_str!()` and parsed with serde. Replaces hardcoded Rust `const` color values
- **`resolve_palette()` replaces `emphasis_for_theme()`** — Theme resolution: name lookup → variant selection → TOML color overrides applied last
- **Backward-compatible theme names** — `theme = "dark"` and `theme = "light"` map to tokyo-night with the appropriate variant, preserving existing configs

## [1.0.5] - 2026-03-23

### Added

- **Native rate limit display** — Adopts CC 2.1.80's `rate_limits` stdin field for quota display. 5-hour and 7-day usage percentages with reset countdowns are now read directly from the statusline payload instead of fetched via background subprocess
- **Agent identity on L1** — New `AG:{name}` segment shows the active session agent when launched with `--agent`. Uses `STABLE_BLUE` color, toggled via `show_agent` (default: true). Only appears when agent data is present
- **Worktree indicator on L1** — `(WT)` suffix appended to git status when running in a CC-managed worktree (`claude --worktree`). Toggled via `show_worktree` (default: true)

### Removed

- **Background quota fetch** — Deleted `providers/quota.rs` and `providers/quota_fetch.rs` (~610 lines): OAuth credential reading (macOS Keychain + file fallback), Anthropic usage API calls, ISO 8601 timestamp parsing, cache file management, and `--fetch-quota` CLI subprocess
- **Plan type prefix** — Quota line no longer shows subscription type (`Q:Pro 5h: 75%` → `Q: 5h: 75%`) since CC's `rate_limits` field doesn't include plan type

### Changed

- **QuotaMetrics simplified** — Removed `plan_type` and `available` fields; replaced `from_snapshot(now_ms)` with pure `from_rate_limits(rate_limits, now_secs)`. Visibility now uses `has_data()` instead of dual-field gate
- **L1 output format** — Now supports optional `AG:{agent}` segment and `(WT)` worktree indicator: `M:{model} | AG:{agent} | S:{style} | CC:{version} | P:{path} | G:{branch}[*] (WT)`

## [1.0.4] - 2026-03-13

### Added

- **Noise tool filtering** — 9 internal Claude Code tools (EnterPlanMode, ExitPlanMode, EnterWorktree, ExitWorktree, TaskGet, TaskList, TaskOutput, TaskStop, ToolSearch) excluded from tool tracking, keeping completed counts and recent tools focused on user-visible tools
- **Hybrid tool scoring** — Completed tools ranked by `count + recency_bonus` where the recency bonus decays linearly over 2 minutes — recently used tools float up even with low total counts
- **Expanded target extraction** — Skill (skill name), AskUserQuestion (question text), SendMessage (recipient), LSP (command), WebFetch (URL), WebSearch (query), NotebookEdit (file path), plus generic fallback chain (`file_path` → `command` → `pattern`) for unknown tools
- **Multi-line completed tools** — `tools_per_line` config (default 6) wraps completed tool counts across multiple lines

### Changed

- **Completed tool sorting** — Replaced count-only sorting (`top_completed_tools()`) with hybrid scoring (`scored_completed_tools()`)
- **Cache schema** — `completed_tool_counts` internal format changed to track last completion timestamp; old cache files silently discarded and regenerated

## [1.0.3] - 2026-03-11

### Fixed

- **Agent tracking** — Claude Code 2.1.72 renamed the subagent tool from `Task` to `Agent`, causing phantom agents that never complete (stuck showing growing elapsed time). Updated all tool name checks and added `toolUseResult` fallback completion signal as defense-in-depth.

## [1.0.2] - 2026-02-23

### Added

- **Git file stats** — Starship-style `!3 +1 ✘2 ?4` (modified/added/deleted/untracked) on L1, toggled via `show_git_stats`
- **Output speed tracking** — Delta-based tok/s inline in TOK segment (`↗1.5K/s`), toggled via `show_speed`
- **Usage quota system** — Two-process design: background subprocess (`--fetch-quota`) reads OAuth + calls Anthropic API; main process reads cache only. Displays 5h/7d percentages with reset countdown. Color thresholds: <50% green, 50-84% amber, ≥85% red
- **Rich TODO display** — In-progress tasks show `active_form` text + elapsed time; pending-only, all-done, and legacy variants
- **Two-line tool split** — L4a (stable completed counts) + L4b (volatile recent/running tools with targets)
- **Recent tools persistence** — Tools stay visible after completion (FIFO cap of 10)

### Changed

- **Quota display** simplified from bar+percentage to percentage-only (matches CTX style)
- **Token/speed color** promoted to `tier.primary` for better hierarchy
- **Structural emphasis** updated: dark 60→103 (brighter blue-purple), light 247→245 (refined gap distribution)

### Fixed

- **Theme config** now case-insensitive ("Light"/"LIGHT" work)
- **Git dirty detection** removed dead `! ` prefix check (unreachable in porcelain v2)
- **Output speed** — `None` output tokens no longer corrupt state anchors
- **MCP deduplication** — user/project scopes properly dedup shared servers
- **Project config template** — added missing activity segment examples

## [1.0.1] - 2026-02-11

### Added

- **Memories metric** on L2 — counts `.md` files in `~/.claude/projects/{path}/memory/`, with `INDICATOR_MEMORY` color and `show_memory` config toggle
- **Claude Code plugin packaging** — plugin manifest, marketplace config, four slash commands (`/pulseline:setup`, `config`, `status`, `uninstall`), and auto-invoked troubleshooting skill
- **Project-level Claude Code rules** — `.claude/rules/` with 5 behavioral files (coding style, testing, patterns, rendering, performance)
- **Integration contract docs** — rules documenting the external Claude Code contract (stdin schema, transcript format, output contract)
- **Core-metrics screenshot** and generator script for README

### Changed

- **Codebase simplification** — removed dead code (`providers/stdin.rs`, `RenderCacheEntry`, `RunnerState`, unused `StdinPayload` methods, `tokyo_bg` config field), eliminated double JSON deserialization via `run_from_payload()` API, reduced file I/O in env.rs, unified format_tokens_segment branches, extracted `write_init_file()` helper
- **Documentation cleanup** — removed stale `stdin.rs` references

## [1.0.0] - 2026-02-10

### Added

- **Multi-line statusline** with four always-visible metric lines: identity (L1), config counts (L2), budget (L3), and live activity (L4+)
- **Context and cost monitoring** — context window percentage with color alerts, token breakdown, total cost, and hourly burn rate
- **Live tool tracking** — see running tools with file/command targets and completed tool counts, updated as Claude Code works
- **Agent and todo tracking** — running and recently completed agents with duration, plus task progress from TaskCreate/TaskUpdate
- **TOML configuration** with user-level (`~/.claude/pulseline/config.toml`) and project-level (`.claude/pulseline.toml`) configs that deep-merge
- **Segment toggles** — individually show or hide every metric segment via config
- **Adaptive rendering** — width degradation that progressively drops activity lines, compresses L2, then truncates core lines for narrow terminals
- **Dark and light themes** — Tokyo Night Storm 256-color palette with `theme = "light"` config support
- **Nerd Font icons** with automatic ASCII fallback
- **CLI commands** — `--init`, `--init --project`, `--check`, `--print` for config management
- **Cross-platform distribution** — npm binary packages (macOS, Linux with glibc/musl, Windows), cargo install, and shell install script
- **`NO_COLOR` support** — respects the standard `NO_COLOR` environment variable
- **Context alert thresholds** at 70%/55% — warnings appear before Claude Code's ~80% auto-compact triggers
- **Steel blue completed checkmarks** — distinct from plan-mode green to avoid visual collision

[1.1.5]: https://github.com/GregoryHo/cc-pulseline/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/GregoryHo/cc-pulseline/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/GregoryHo/cc-pulseline/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/GregoryHo/cc-pulseline/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/GregoryHo/cc-pulseline/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.6...v1.1.0
[1.0.6]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.5...v1.0.6
[1.0.5]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/GregoryHo/cc-pulseline/releases/tag/v1.0.0
