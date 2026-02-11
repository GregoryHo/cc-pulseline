#!/usr/bin/env bash
# Generate cc-pulseline output showcasing the core L1-L3 lines only.
# Usage: ./screenshots/core-lines.sh [COLUMNS]
#
# Shows:
#   L1: Model | Style | Version | Project path | Git branch with status
#   L2: CLAUDE.md | rules | hooks | MCPs | skills | duration
#   L3: Context % | Token breakdown | Cost with hourly rate
#
# Uses an empty transcript so no activity lines (L4+) appear.
# Creates a fake project dir with git repo, config files, hooks, MCPs,
# and skills for realistic L2 counts.

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

TRANSCRIPT=$(mktemp /tmp/pulseline-core-XXXXXX.jsonl)
WORKDIR=$(mktemp -d /tmp/pulseline-core-workdir-XXXXXX)
trap "rm -f '$TRANSCRIPT'; rm -rf '$WORKDIR'" EXIT

# ── Set up fake project dir with git ──────────────────────────────
git -C "$WORKDIR" init -q
git -C "$WORKDIR" checkout -q -b feature/theme-system
# Stage a file so git branch exists, then add a dirty file
echo "init" > "$WORKDIR/README.md"
git -C "$WORKDIR" add README.md
git -C "$WORKDIR" -c user.email="x@x.com" -c user.name="x" commit -q -m "init"
echo "wip" > "$WORKDIR/scratch.txt"

# ── CLAUDE.md files (3 total) ────────────────────────────────────
echo "# Project instructions" > "$WORKDIR/CLAUDE.md"
mkdir -p "$WORKDIR/.claude"
echo "# Internal instructions" > "$WORKDIR/.claude/CLAUDE.md"
echo "# Local overrides" > "$WORKDIR/CLAUDE.local.md"

# ── Rules (3 files) ──────────────────────────────────────────────
mkdir -p "$WORKDIR/.claude/rules"
echo "# Style guide" > "$WORKDIR/.claude/rules/style.md"
echo "# Testing policy" > "$WORKDIR/.claude/rules/testing.md"
echo "# Security checklist" > "$WORKDIR/.claude/rules/security.md"

# ── Hooks (2 hooks in project settings) ──────────────────────────
cat > "$WORKDIR/.claude/settings.json" << 'JSON'
{
  "hooks": {
    "PreToolUse": [{"command": "echo pre"}],
    "PostToolUse": [{"command": "echo post"}]
  }
}
JSON

# ── MCPs (2 servers in .mcp.json) ────────────────────────────────
cat > "$WORKDIR/.mcp.json" << 'JSON'
{
  "mcpServers": {
    "context7": {"command": "npx", "args": ["context7"]},
    "chrome-devtools": {"command": "npx", "args": ["chrome-devtools"]}
  }
}
JSON

# ── Skills (2 skill dirs) ────────────────────────────────────────
mkdir -p "$WORKDIR/.claude/skills/commit"
echo "# Commit skill" > "$WORKDIR/.claude/skills/commit/skill.md"
mkdir -p "$WORKDIR/.claude/skills/review"
echo "# Review skill" > "$WORKDIR/.claude/skills/review/skill.md"

# ── Empty transcript (no tools/agents/todos → clean L1-L3 only) ──
: > "$TRANSCRIPT"

# ── Stdin payload — typical mid-session state ────────────────────
# 45 min session, 43% context, moderate cost ($4.80 over 45m ≈ $6.40/h)
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "core-lines-$(date +%s)",
  "cwd": "$WORKDIR",
  "model": {
    "id": "claude-opus-4-6",
    "display_name": "Opus 4.6"
  },
  "workspace": {
    "current_dir": "$WORKDIR"
  },
  "output_style": {
    "name": "concise"
  },
  "version": "1.0.26",
  "context_window": {
    "context_window_size": 200000,
    "used_percentage": 43,
    "current_usage": {
      "input_tokens": 12840,
      "output_tokens": 24576,
      "cache_creation_input_tokens": 68200,
      "cache_read_input_tokens": 114510
    }
  },
  "cost": {
    "total_cost_usd": 4.8025,
    "total_duration_ms": 2700000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
