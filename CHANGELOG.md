# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[1.0.2]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/GregoryHo/cc-pulseline/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/GregoryHo/cc-pulseline/releases/tag/v1.0.0
