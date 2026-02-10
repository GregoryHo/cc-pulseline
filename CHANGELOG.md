# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-02-10

### Added

- **Multi-line statusline** with four always-visible metric lines: identity (L1), config counts (L2), budget (L3), and live activity (L4+)
- **Context and cost monitoring** — context window percentage with color alerts, token breakdown, total cost, and hourly burn rate
- **Live tool tracking** — see running tools with file/command targets and completed tool counts, updated as Claude Code works
- **Agent and todo tracking** — running and recently completed agents with duration, plus task progress from TaskCreate/TaskUpdate
- **TOML configuration** with user-level (`~/.claude/pulseline/config.toml`) and project-level (`.claude/pulseline.toml`) configs that deep-merge
- **Segment toggles** — individually show or hide every metric segment via config
- **Adaptive rendering** — width degradation that progressively drops activity lines, compresses L2, then truncates core lines for narrow terminals
- **Dark and light themes** — Tokyo Night Storm 256-color palette with `PULSELINE_THEME=light` support
- **Nerd Font icons** with automatic ASCII fallback
- **CLI commands** — `--init`, `--init --project`, `--check`, `--print` for config management
- **Cross-platform distribution** — npm binary packages (macOS, Linux with glibc/musl, Windows), cargo install, and shell install script
- **`NO_COLOR` support** — respects the standard `NO_COLOR` environment variable
- **Context alert thresholds** at 70%/55% — warnings appear before Claude Code's ~80% auto-compact triggers
- **Steel blue completed checkmarks** — distinct from plan-mode green to avoid visual collision

[1.0.0]: https://github.com/GregoryHo/cc-pulseline/releases/tag/v1.0.0
