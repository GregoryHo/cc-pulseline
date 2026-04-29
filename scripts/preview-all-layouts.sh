#!/usr/bin/env bash
# preview-all-layouts.sh
#
# Renders every layout against an enriched fixture so every layout's
# distinguishing features actually show:
#
#   - L1 identity + L2 config + L3 budget    (was already covered)
#   - Running tools + completed tool counts  (added — drives L4a/L4b, ticker)
#   - Running agent with description + model (added — drives agent rows)
#   - Todo in progress with pending items    (added — drives todo line)
#   - rate_limits for 5h + 7d                (added — drives quota row)
#
# Per-iteration project config is a "kitchen-sink-on" preview — quota,
# tools, agents, todo all enabled — so layout differences aren't masked
# by feature toggles. Theme is preserved from your existing config.
# Original config is backed up and restored on exit (incl. Ctrl-C).
#
# Usage:
#   ./scripts/preview-all-layouts.sh                 # 140 cols
#   ./scripts/preview-all-layouts.sh 140 80          # 140 then 80 (degradation)
#   ./scripts/preview-all-layouts.sh 160 110 80      # three widths

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_CONFIG="${REPO_ROOT}/.claude/pulseline.toml"
BINARY="${REPO_ROOT}/target/release/cc-pulseline"

LAYOUTS=(none zones grid sections console ledger)
WIDTHS=("$@")
[[ ${#WIDTHS[@]} -eq 0 ]] && WIDTHS=(140)

if [[ ! -f "$PROJECT_CONFIG" ]]; then
  echo "ERROR: $PROJECT_CONFIG not found." >&2
  exit 1
fi

echo "Building release binary..."
(cd "$REPO_ROOT" && cargo build --release --quiet)

# ── Snapshot project config; restore on any exit ──────────────────────
BACKUP="$(mktemp -t pulseline-config-backup.XXXXXX)"
cp "$PROJECT_CONFIG" "$BACKUP"

# Synthetic transcript: 2 completed tools (drive completed counts), 1
# running tool with target (drives L4b), 1 running agent (drives agent
# row), 1 todo with in-progress + pending (drives todo line).
TRANSCRIPT="$(mktemp -t pulseline-preview-transcript.XXXXXX.jsonl)"
cat > "$TRANSCRIPT" <<EOF
{"timestamp":"2026-04-28T15:00:00.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"${REPO_ROOT}/src/render/layout.rs"}}]}}
{"timestamp":"2026-04-28T15:00:01.000Z","content":[{"type":"tool_result","tool_use_id":"t1"}]}
{"timestamp":"2026-04-28T15:00:02.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"Bash","input":{"command":"cargo test --release"}}]}}
{"timestamp":"2026-04-28T15:00:03.000Z","content":[{"type":"tool_result","tool_use_id":"t2"}]}
{"timestamp":"2026-04-28T15:00:04.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"t3","name":"Read","input":{"file_path":"${REPO_ROOT}/src/render/frames/console.rs"}}]}}
{"timestamp":"2026-04-28T15:00:05.000Z","content":[{"type":"tool_result","tool_use_id":"t3"}]}
{"timestamp":"2026-04-28T15:00:06.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"t4-running","name":"Edit","input":{"file_path":"${REPO_ROOT}/src/render/widgets/gauge.rs"}}]}}
{"timestamp":"2026-04-28T15:00:07.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"a1","name":"Agent","input":{"description":"Investigate parser edge case in transcript dispatcher","subagent_type":"Explore","model":"haiku"}}]}}
{"timestamp":"2026-04-28T15:00:08.000Z","type":"progress","data":{"type":"agent_progress","agentId":"a1","prompt":"Investigate parser edge case in transcript dispatcher","agentType":"Explore"}}
{"timestamp":"2026-04-28T15:00:09.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"todo1","name":"TodoWrite","input":{"todos":[{"content":"Refactor cluster layout","status":"in_progress","activeForm":"Refactoring cluster layout"},{"content":"Add tests for new widget","status":"pending"},{"content":"Update docs/layouts.md","status":"pending"}]}}]}}
EOF

trap 'cp "$BACKUP" "$PROJECT_CONFIG"; rm -f "$BACKUP" "$TRANSCRIPT"; printf "\nRestored %s\n" "$PROJECT_CONFIG"' EXIT INT TERM

# Preserve user's theme; default to "dark" if absent.
THEME="$(awk -F'"' '/^theme *=/ { print $2; exit }' "$BACKUP")"
[[ -z "$THEME" ]] && THEME="dark"

# Resets_at = 2 hours from now (5h window) and 5 days from now (7d window).
NOW="$(date +%s)"
RESET_5H=$((NOW + 7200))
RESET_7D=$((NOW + 432000))

# Enriched stdin payload: real fixture data + transcript_path + rate_limits.
PAYLOAD=$(cat <<EOF
{
  "session_id": "preview-session",
  "cwd": "${REPO_ROOT}",
  "model": { "id": "claude-opus-4-6", "display_name": "Opus 4.6" },
  "workspace": { "current_dir": "${REPO_ROOT}" },
  "output_style": { "name": "explanatory" },
  "version": "2.1.37",
  "transcript_path": "${TRANSCRIPT}",
  "context_window": {
    "context_window_size": 200000,
    "used_percentage": 43,
    "current_usage": {
      "input_tokens": 1,
      "output_tokens": 8,
      "cache_creation_input_tokens": 5655,
      "cache_read_input_tokens": 114510
    }
  },
  "cost": { "total_cost_usd": 4.5575465, "total_duration_ms": 3712191 },
  "rate_limits": {
    "five_hour": { "used_percentage": 62.5, "resets_at": ${RESET_5H} },
    "seven_day": { "used_percentage": 28.0, "resets_at": ${RESET_7D} }
  }
}
EOF
)

# Kitchen-sink-on preview config. Theme inherited; everything else
# enabled so layout differences aren't hidden by toggles.
write_preview_config() {
  local layout="$1"
  cat > "$PROJECT_CONFIG" <<EOF
# AUTO-GENERATED by scripts/preview-all-layouts.sh — restored on exit.
[display]
theme = "${THEME}"
icons = true

[segments.identity]
show_model = true
show_style = true
show_version = true
show_project = true
show_git = true
show_git_stats = true
show_agent = true
show_worktree = true

[segments.config]
show_claude_md = true
show_rules = true
show_memory = true
show_hooks = true
show_mcp = true
show_skills = true
show_plugins = true
show_duration = true

[segments.budget]
show_context = true
show_tokens = true
show_cost = true
show_speed = true

[segments.quota]
enabled = true
show_five_hour = true
show_seven_day = true

[segments.tools]
enabled = true
max_lines = 2
max_completed = 6
max_completed_lines = 2

[segments.agents]
enabled = true
max_lines = 2

[segments.todo]
enabled = true
max_lines = 2

[layout]
name = "${layout}"
EOF
}

# ── Render matrix: layouts × widths ────────────────────────────────────
for cols in "${WIDTHS[@]}"; do
  printf '\n\033[1;33m▓▓▓ COLUMNS=%s ▓▓▓\033[0m\n' "$cols"
  for layout in "${LAYOUTS[@]}"; do
    write_preview_config "$layout"
    printf '\n\033[1;36m═══ %s ═══\033[0m\n' "$layout"
    echo "$PAYLOAD" | COLUMNS="$cols" "$BINARY"
  done
done
