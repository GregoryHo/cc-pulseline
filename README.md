# cc-pulseline

High-performance multi-line statusline for Claude Code with deep observability.

```
M:Opus 4.6 | S:explanatory | CC:2.1.37 | P:~/projects/myapp | G:main ↑2
1 CLAUDE.md | 3 rules | 2 hooks | 4 MCPs | 1 skills | 1h 23m
CTX:43% (86.0k/200.0k) | TOK I:10.0k O:20.0k C:30.0k/40.0k | $3.50 ($3.41/h)
T:Read: .../src/main.rs | T:Bash: cargo test | ✓Read ×12 | ✓Bash ×5
A:Explore [haiku]: Investigating auth logic (2m)
```

## Features

- **4-line metrics dashboard** — Identity, config counts, budget, and live activity
- **Incremental transcript parsing** — Seek-based JSONL parsing with per-session offsets
- **Deep observability** — Active tools with targets, agent status, todo tracking
- **Session-aware** — Concurrent Claude Code sessions tracked independently
- **Adaptive rendering** — Width degradation for narrow terminals
- **Tokyo Night color palette** — 256-color ANSI with dark/light theme support
- **Minimal dependencies** — 3 runtime crates (serde, serde_json, toml)
- **Configurable** — TOML config with per-project overrides and segment toggles

## Quickstart

### 1. Install

```bash
# npm (recommended — works on macOS, Linux, Windows)
npm install -g @cc-pulseline/cc-pulseline

# From source
cargo install cc-pulseline

# Or clone and build
git clone https://github.com/GregoryHo/cc-pulseline.git
cd cc-pulseline && ./scripts/install.sh
```

### 2. Configure Claude Code

Add to `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/pulseline/cc-pulseline"
  }
}
```

### 3. Done

Start a Claude Code session — the statusline appears automatically.

## Installation Methods

| Method | Command | Best For |
|--------|---------|----------|
| **npm** | `npm i -g @cc-pulseline/cc-pulseline` | Claude Code users |
| **cargo-binstall** | `cargo binstall cc-pulseline` | Rust devs (prebuilt) |
| **cargo install** | `cargo install cc-pulseline` | Rust devs (from source) |
| **install.sh** | `./scripts/install.sh` | Local clone |

## Configuration

cc-pulseline uses TOML configuration with two scopes:

- **User**: `~/.claude/pulseline/config.toml`
- **Project**: `{project}/.claude/pulseline.toml` (overrides user)

```bash
cc-pulseline --init              # Create user config
cc-pulseline --init --project    # Create project config
cc-pulseline --check             # Validate configs
cc-pulseline --print             # Show effective merged config
```

### Example Config

```toml
[display]
theme = "dark"    # "dark" or "light"
icons = true      # Nerd Font icons (false for ASCII)

[segments.identity]
show_model = true
show_style = true
show_version = true
show_project = true
show_git = true

[segments.config]
show_claude_md = true
show_rules = true
show_hooks = true
show_mcp = true
show_skills = true
show_duration = true

[segments.budget]
show_context = true
show_tokens = true
show_cost = true

[segments.tools]
enabled = true
max_completed = 4
```

## Output Format

| Line | Content | Example |
|------|---------|---------|
| **L1** | Identity | `M:Opus 4.6 \| S:explanatory \| CC:2.1.37 \| P:~/app \| G:main*` |
| **L2** | Config | `1 CLAUDE.md \| 3 rules \| 2 hooks \| 4 MCPs \| 1 skills \| 1h` |
| **L3** | Budget | `CTX:43% (86.0k/200.0k) \| TOK I:10k O:20k C:30k/40k \| $3.50` |
| **L4** | Tools | `T:Read: .../main.rs \| ✓Bash ×5` (only when active) |
| **L5+** | Agents | `A:Explore: Investigating logic (2m)` (only when active) |

## Environment Variables

| Variable | Effect |
|----------|--------|
| `NO_COLOR` | Disable all color output |
| `COLUMNS` | Terminal width for layout degradation |

## Performance

Designed for repeated invocation by Claude Code's statusline hook:

| Scenario | Target (p95) |
|----------|-------------|
| Baseline (no activity) | < 10ms |
| Active session (10 tools + 5 agents) | < 20ms |
| Large transcript (2500 events) | < 50ms |

Run benchmarks: `cargo bench`

## Documentation

- [Architecture](docs/architecture.md) — Pipeline design and module responsibilities
- [Metrics Reference](docs/metrics-reference.md) — Data sources, parsing, and caching for each metric
- [Theme & Palette](docs/theme-palette.md) — Color system specification

## License

[MIT](LICENSE)
