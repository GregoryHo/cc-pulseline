---
description: Install cc-pulseline binary, register statusline, and create default config
argument-hint: [npm|cargo|binary]
allowed-tools: Bash, Read, Write, Edit, Glob
---

You are setting up cc-pulseline, a high-performance multi-line statusline for Claude Code. Follow these steps in order.

## Step 1: Check Existing Installation

Check if the binary is already installed:

```bash
~/.claude/pulseline/cc-pulseline --help 2>/dev/null && echo "INSTALLED" || echo "NOT_INSTALLED"
```

If already installed, show the current version and ask the user if they want to reinstall or skip to registration.

## Step 2: Determine Install Method

The user can specify an install method via `$ARGUMENTS`:
- **`npm`** (default if no argument) — `npm install -g cc-pulseline`, then copy binary
- **`cargo`** — `cargo install cc-pulseline`, then copy binary
- **`binary`** — download prebuilt binary from GitHub releases

If `$ARGUMENTS` is empty, default to `npm`.

### npm Install

```bash
npm install -g cc-pulseline
# Find where npm installed it and copy to Claude's expected location
mkdir -p ~/.claude/pulseline
NPM_BIN=$(npm root -g)/cc-pulseline/npm/cc-pulseline
if [ -f "$NPM_BIN" ]; then
  cp "$NPM_BIN" ~/.claude/pulseline/cc-pulseline
else
  # npm global bin fallback
  cp "$(which cc-pulseline)" ~/.claude/pulseline/cc-pulseline
fi
chmod +x ~/.claude/pulseline/cc-pulseline
```

### cargo Install

```bash
cargo install cc-pulseline
mkdir -p ~/.claude/pulseline
cp "$(which cc-pulseline)" ~/.claude/pulseline/cc-pulseline
chmod +x ~/.claude/pulseline/cc-pulseline
```

### binary Install

Detect the platform and download from GitHub releases:

```bash
mkdir -p ~/.claude/pulseline
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$OS-$ARCH" in
  darwin-arm64|darwin-aarch64) PLATFORM="darwin-arm64" ;;
  darwin-x86_64)               PLATFORM="darwin-x64" ;;
  linux-x86_64)                PLATFORM="linux-x64" ;;
  linux-aarch64|linux-arm64)   PLATFORM="linux-arm64" ;;
  *) echo "Unsupported platform: $OS-$ARCH"; exit 1 ;;
esac
curl -fsSL "https://github.com/GregoryHo/cc-pulseline/releases/latest/download/cc-pulseline-${PLATFORM}.tar.gz" | tar xz -C ~/.claude/pulseline
chmod +x ~/.claude/pulseline/cc-pulseline
```

## Step 3: Verify Binary

```bash
~/.claude/pulseline/cc-pulseline --help
```

If this fails, report the error and stop.

## Step 4: Register Statusline

Read `~/.claude/settings.json`, merge in the `statusLine` entry, and write back. Preserve all existing settings.

The required entry is:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/pulseline/cc-pulseline"
  }
}
```

**Important**: Read the existing file first. If it exists, parse the JSON, add/update only the `statusLine` key, and write back the full object. If the file doesn't exist, create it with just the statusLine entry.

## Step 5: Create Default Config

```bash
~/.claude/pulseline/cc-pulseline --init
```

This creates `~/.claude/pulseline/config.toml` with sensible defaults.

## Step 6: Verify Runtime

Pipe a test payload to verify the binary works end-to-end:

```bash
echo '{"session_id":"test","version":"1.0.2","model":{"id":"claude-sonnet-4-5-20250929","display_name":"Claude Sonnet 4.5"}}' | ~/.claude/pulseline/cc-pulseline
```

This should output formatted statusline text. If it produces output, the installation is working.

## Step 7: Report Summary

Show the user:

1. **Binary location**: `~/.claude/pulseline/cc-pulseline`
2. **Config location**: `~/.claude/pulseline/config.toml`
3. **Settings updated**: `~/.claude/settings.json`
4. **Status**: Working / Error details

Tell them:
- The statusline will appear on their next Claude Code session restart
- Use `/pulseline:config` to customize the display
- Use `/pulseline:status` to run diagnostics
- Use `/pulseline:config project` to create per-project overrides
