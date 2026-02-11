---
description: View and manage cc-pulseline configuration (show, edit, project, reset)
argument-hint: [show|edit|project|reset]
allowed-tools: Bash, Read, Write, Edit
---

You are managing cc-pulseline configuration. The subcommand is determined by `$ARGUMENTS`:

- **`show`** (default if no argument) — display effective merged config
- **`edit`** — interactive config editing
- **`project`** — create or edit project-level config
- **`reset`** — reset config to defaults

## Subcommand: show

Display the effective merged configuration and file locations:

```bash
~/.claude/pulseline/cc-pulseline --print
```

Also show config file locations and whether they exist:

```bash
echo "--- Config Files ---"
ls -la ~/.claude/pulseline/config.toml 2>/dev/null && echo "User config: EXISTS" || echo "User config: NOT FOUND"
ls -la .claude/pulseline.toml 2>/dev/null && echo "Project config: EXISTS" || echo "Project config: NOT FOUND"
```

## Subcommand: edit

1. Read the current user config:

```bash
cat ~/.claude/pulseline/config.toml
```

2. Show the user what's currently configured and present the available options:

### Configuration Reference

**Display Settings**
| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `color_enabled` | bool | `false` | Enable 256-color ANSI output |
| `use_icons` | bool | `false` | Use Nerd Font icons (requires patched font) |
| `terminal_width` | int | 0 (auto) | Override terminal width for layout |
| `theme` | string | `"dark"` | Color theme: `"dark"` or `"light"` |

**Line 1 Segments** (identity)
| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `show_model` | bool | `true` | Model name (e.g., `M:Sonnet 4.5`) |
| `show_style` | bool | `true` | Output style (e.g., `S:explanatory`) |
| `show_version` | bool | `true` | Claude Code version (e.g., `CC:2.1.37`) |
| `show_project` | bool | `true` | Project path (e.g., `P:~/project`) |
| `show_git` | bool | `true` | Git branch + status |

**Line 2 Segments** (environment)
| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `show_claude_md` | bool | `true` | CLAUDE.md file count |
| `show_rules` | bool | `true` | Rules file count |
| `show_memory` | bool | `true` | Memory file count |
| `show_hooks` | bool | `true` | Hooks count |
| `show_mcp` | bool | `true` | MCP server count |
| `show_skills` | bool | `true` | Skills count |
| `show_duration` | bool | `true` | Session duration |

**Line 3 Segments** (budget)
| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `show_context` | bool | `true` | Context window usage % |
| `show_tokens` | bool | `true` | Token breakdown (I/O/C/R) |
| `show_cost` | bool | `true` | Session cost and rate |

**Activity Settings**
| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `show_tools` | bool | `true` | Running tools + completed counts |
| `show_agents` | bool | `true` | Active and completed agents |
| `show_todo` | bool | `true` | Todo list status |
| `max_tool_lines` | int | `1` | Max tool display lines |
| `max_agent_lines` | int | `3` | Max agent display lines |
| `max_completed_tools` | int | `5` | Max completed tool counts shown |

3. Ask the user what they want to change.

4. Apply the edits to `~/.claude/pulseline/config.toml` using the Edit tool.

5. Validate the config:

```bash
~/.claude/pulseline/cc-pulseline --check
```

6. Show the updated effective config:

```bash
~/.claude/pulseline/cc-pulseline --print
```

## Subcommand: project

Create or edit a project-level config override:

1. Check if `.claude/pulseline.toml` exists in the current project:

```bash
ls -la .claude/pulseline.toml 2>/dev/null
```

2. If it doesn't exist, create it:

```bash
~/.claude/pulseline/cc-pulseline --init --project
```

3. If it exists, read it and offer to edit (same flow as the `edit` subcommand but targeting `.claude/pulseline.toml`).

4. Explain that project config overrides user config — only fields explicitly set in the project config take effect, everything else falls back to user config.

## Subcommand: reset

1. Ask the user to confirm: "This will delete your current config and create fresh defaults. Continue?"

2. If confirmed:

```bash
rm -f ~/.claude/pulseline/config.toml
~/.claude/pulseline/cc-pulseline --init
```

3. Show the new config:

```bash
~/.claude/pulseline/cc-pulseline --print
```
