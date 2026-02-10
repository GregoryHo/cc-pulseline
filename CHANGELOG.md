# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-02-10

### Added

- **L1 Identity line**: Model, output style, Claude Code version, project path, git status (branch, dirty, ahead/behind)
- **L2 Config line**: CLAUDE.md count, rules, hooks, MCP servers, skills, elapsed time
- **L3 Budget line**: Context window usage (percentage + absolute), token breakdown (input/output/cache), total cost with hourly burn rate
- **L4+ Activity lines**: Active tools with targets, completed tool counts, agent status with duration, todo items
- **Incremental transcript parsing**: Seek-based JSONL parsing with per-session offsets, event windowing, and poll throttling
- **Three-path transcript dispatcher**: Nested content blocks, agent progress events, flat format fallback
- **Session state management**: Multi-session support keyed by session/transcript/project composite key
- **Session cache**: Disk-persisted state across process invocations with atomic writes and silent failure handling
- **TOML configuration**: User config (`~/.claude/pulseline/config.toml`) and project config (`.claude/pulseline.toml`) with deep merge
- **Segment toggles**: Individually toggle every metric segment on/off via config
- **Tokyo Night Storm color palette**: 6-tier color system with dark/light theme support
- **Nerd Font icons**: Optional icon mode with ASCII fallback
- **Width degradation**: Adaptive rendering that drops activity, compresses L2, then truncates core lines
- **CLI commands**: `--init`, `--init --project`, `--check`, `--print`, `--help`, `--version`
- **NO_COLOR support**: Respects the `NO_COLOR` environment variable convention
- **npm distribution**: Cross-platform binary packages for macOS, Linux, and Windows
- **Install script**: Build from source or download prebuilt binaries

[1.0.0]: https://github.com/GregoryHo/cc-pulseline/releases/tag/v1.0.0
