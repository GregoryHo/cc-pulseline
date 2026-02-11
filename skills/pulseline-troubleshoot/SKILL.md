---
name: pulseline-troubleshoot
description: Diagnose and fix cc-pulseline statusline issues. Use when the user mentions their statusline is blank, broken, not updating, showing wrong data, or displaying rendering problems. Also use when users report missing segments, broken icons, or color issues.
tools: Bash, Read
user-invocable: false
---

# Pulseline Troubleshooter

You are automatically diagnosing a cc-pulseline statusline issue. Run quick diagnostics, identify the problem, and suggest a fix.

## Quick Diagnostics

Run these checks to identify the issue:

### 1. Binary Health

```bash
# Binary exists and is executable
ls -la ~/.claude/pulseline/cc-pulseline 2>/dev/null

# Binary runs without error
~/.claude/pulseline/cc-pulseline --check 2>&1

# Test with sample payload
echo '{"session_id":"diag","version":"1.0.1"}' | ~/.claude/pulseline/cc-pulseline 2>&1
```

### 2. Settings Registration

Read `~/.claude/settings.json` and check that `statusLine.command` points to `~/.claude/pulseline/cc-pulseline`.

### 3. Config Validation

```bash
~/.claude/pulseline/cc-pulseline --check 2>&1
~/.claude/pulseline/cc-pulseline --print 2>&1
```

### 4. Cache State

```bash
ls -la ${TMPDIR:-/tmp}/cc-pulseline-*.json 2>/dev/null
```

## Common Issues and Resolutions

### Blank Statusline
- **Binary not found**: Binary missing or wrong path in settings.json. Fix: `/pulseline:setup`
- **Binary crashes**: Check stderr output from test payload. May need rebuild: `/pulseline:setup cargo`
- **Settings not registered**: `statusLine` key missing from settings.json. Fix: `/pulseline:setup`
- **Performance timeout**: Binary taking >50ms. Check for slow git operations or large transcript files

### Wrong or Stale Data
- **Stale cache**: Remove cache files: `rm -f ${TMPDIR:-/tmp}/cc-pulseline-*.json`
- **Config mismatch**: Project config overriding user config unexpectedly. Run `/pulseline:config show` to see merged result
- **Segment disabled**: A segment toggle is set to `false`. Run `/pulseline:config edit` to re-enable

### Broken Icons or Formatting
- **Missing Nerd Font**: Icons require a Nerd Font patched terminal font. Set `use_icons = false` in config to use ASCII fallback
- **Color issues**: Check `color_enabled` in config. Set to `false` if terminal doesn't support 256-color, or set `theme = "light"` for light terminal backgrounds
- **Truncated output**: Terminal width too narrow. Widen terminal or reduce segments via `/pulseline:config edit`

### Missing Segments
- **Segment toggled off**: Check `show_*` fields in config via `/pulseline:config show`
- **No data available**: Some segments only appear when data exists (e.g., agents only show during agent activity, cost only shows after API calls)
- **Width degradation**: When terminal is narrow, activity lines are dropped first, then L2 is compressed. Widen terminal or reduce content

## Resolution

After diagnosing, recommend the appropriate `/pulseline:*` command:
- `/pulseline:setup` — reinstall or re-register
- `/pulseline:config edit` — adjust display settings
- `/pulseline:config reset` — reset to defaults
- `/pulseline:status` — full diagnostic report
