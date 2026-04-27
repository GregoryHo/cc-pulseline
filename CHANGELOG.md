# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **`[pane]` → `[layout]`, `style` → `name` (BREAKING, pre-release)** — Config section + key rename to reflect that the choice is "which layout arranges the lines", not "frame chrome around fixed lines". The TOML enum strings (`none`/`zones`/`grid`/`cards`/`sections`/`cockpit`/`console`/`flightstrip`/`auto`) and internal `PaneStyle` Rust enum are unchanged — only the section/field labels move. v1/v2 distinction is dropped from user-facing comments; layouts are listed alphabetically (flat/framed group + instrument-cluster group) without internal-organization labels. Hard cut: existing `[pane]` blocks are silently ignored. See `designs/style-to-layout-taxonomy.md`
- **v2 layouts now honor `display.icons`** — Hardcoded `A:` agent prefix in cockpit / console / flightstrip is replaced with `glyph(ICON_AGENT, "A:")`; cost arc (`◔◑◕●`) falls back to text rate `($X.X/h)` under `icons = false`. Sparkline emit is gated on the same axis. Every (layout × display) pair now composes cleanly
- Per-frame pane impls split into `src/render/frames/v1/`; `PaneStyle` enum variants gain `V1` prefix. TOML strings unchanged, no behavior change. New `docs/pane-styles.md` documents the v1 frame styles

### Added

- **`show_ctx_sparkline` toggle** — New `[segments.budget]` opt-in (default `false`) for the 6-cell braille trend chart of CTX% history. Layout-agnostic: any layout that renders the CTX segment picks it up (cockpit / console / flightstrip cluster cells, v1 layouts append after CTX% on L3). Auto-hidden when `display.icons = false` since braille has no ASCII fallback
- **Q7d in v2 layouts** — Cockpit cluster row and console quota row now render the seven-day quota window alongside Q5h when `show_quota_seven_day = true`. Previously the toggle was silently dropped on v2 layouts

### Added

- **Tonal strata — palette-native 2-tier chrome** — `ThemePalette` gains two hand-authored fields, `strata_state` and `strata_activity`, that tint the `|` separator differently on state rows (Identity / Config / Budget / Quota) versus activity rows (Tools / Agents / Todos). All 9 built-in themes now ship a chrome pair for both dark and light variants. A new `tests/theme_strata_contrast.rs` lint enforces `|state − activity| ≥ 3` on the ansi256 scale so no shipped theme can collapse the contract. See `designs/tonal-strata-redesign.md` for the design record and per-theme rationale
- **`pane.tonal_strata` ships on by default** — Replaces the prior opt-in 4-way `LineKind→palette` mapping (audited as collapsing to 2–3 visually distinct tiers on every theme; see design doc) with the palette-native 2-tier split. The `pane.tonal_strata` config flag is preserved (`true` = the new behavior, `false` = a flat baseline using `emphasis_separator` on every row)
- **Per-color overrides for strata** — `[colors]` TOML section accepts `strata_state` and `strata_activity` for fine-tuning on top of any theme preset
- **`--palette-map` shows the Strata tier** — Field reference output ends with a `Strata` row listing both new fields and their ansi256 codes

### Changed

- **Theme palette grows to 28 fields** — `palette_mapping` adds `strata_state` and `strata_activity`. Custom themes that omit the fields fall back to `emphasis_separator` and `emphasis_structural` and emit a one-time warning

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

[1.0.6]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.5...v1.0.6
[1.0.5]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/GregoryHo/cc-pulseline/releases/tag/v1.0.0
