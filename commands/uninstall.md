---
description: Remove cc-pulseline binary, config, and statusline registration
allowed-tools: Bash, Read, Write, Edit
---

You are removing cc-pulseline from this system. This is a destructive operation — confirm with the user before proceeding.

## Step 1: Confirm

Ask the user: "This will remove the cc-pulseline binary, config files, and statusline registration. Do you want to proceed?"

**Do not proceed until the user explicitly confirms.**

## Step 2: Deregister Statusline

Read `~/.claude/settings.json`, remove only the `statusLine` key, and write back the remaining settings.

```bash
cat ~/.claude/settings.json
```

Parse the JSON, remove the `statusLine` key, and write the updated JSON back. If `statusLine` is the only key, write `{}`. Preserve all other settings.

## Step 3: Remove Binary and Config

```bash
rm -f ~/.claude/pulseline/cc-pulseline
rm -f ~/.claude/pulseline/config.toml
rmdir ~/.claude/pulseline 2>/dev/null || true
```

## Step 4: Clear Session Cache

```bash
rm -f ${TMPDIR:-/tmp}/cc-pulseline-*.json
```

## Step 5: Check Project Config

Check if a project-level config exists:

```bash
ls -la .claude/pulseline.toml 2>/dev/null
```

If found, ask the user: "A project-level config was found at `.claude/pulseline.toml`. Remove it too?"

If confirmed:

```bash
rm -f .claude/pulseline.toml
```

## Step 6: Report

Show what was removed:

- Binary: `~/.claude/pulseline/cc-pulseline`
- User config: `~/.claude/pulseline/config.toml`
- Settings entry: `statusLine` removed from `~/.claude/settings.json`
- Session cache: `${TMPDIR}/cc-pulseline-*.json`
- Project config: `.claude/pulseline.toml` (if removed)

Remind the user:
- If installed via npm, also run: `npm uninstall -g cc-pulseline`
- If installed via cargo, also run: `cargo uninstall cc-pulseline`
- The statusline will stop appearing on the next Claude Code session restart
