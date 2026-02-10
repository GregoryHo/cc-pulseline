#!/usr/bin/env bash
# Generate cc-pulseline output showcasing Todo Tracking.
# Usage: ./screenshots/todo-tracking.sh [COLUMNS]
#
# Creates 5 tasks via TaskCreate, marks 2 completed via TaskUpdate.
# Result: TODO:2/5 done, 3 pending — rendered in TODO_TEAL (color 80).

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

NOW_S=$(date +%s)
TRANSCRIPT=$(mktemp /tmp/pulseline-todo-XXXXXX.jsonl)
trap "rm -f '$TRANSCRIPT'" EXIT

# ── Helpers ──────────────────────────────────────────────────────────
to_iso() { date -u -r "$1" +"%Y-%m-%dT%H:%M:%S.000Z"; }

# ── Timeline — ~20 min session with task-driven workflow ─────────────
T1=$((NOW_S - 1200))   # -20m: initial exploration
T2=$((NOW_S - 1000))   # -~17m: create tasks
T3=$((NOW_S - 800))    # -~13m: start working
T4=$((NOW_S - 500))    # -~8m: complete first task
T5=$((NOW_S - 300))    # -5m: complete second task
T6=$((NOW_S - 60))     # -1m: still working on third

cat > "$TRANSCRIPT" << JSONL
{"timestamp":"$(to_iso $T1)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t01","name":"Read","input":{"file_path":"/src/render/layout.rs"}},{"type":"tool_use","id":"t02","name":"Read","input":{"file_path":"/src/render/color.rs"}}]}}
{"timestamp":"$(to_iso $((T1+2)))","content":[{"type":"tool_result","tool_use_id":"t01"}]}
{"timestamp":"$(to_iso $((T1+3)))","content":[{"type":"tool_result","tool_use_id":"t02"}]}
{"timestamp":"$(to_iso $T2)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t03","name":"TaskCreate","input":{"subject":"Add dark/light theme toggle to config"}},{"type":"tool_use","id":"t04","name":"TaskCreate","input":{"subject":"Implement emphasis tier color mapping"}},{"type":"tool_use","id":"t05","name":"TaskCreate","input":{"subject":"Update render_frame for theme support"}},{"type":"tool_use","id":"t06","name":"TaskCreate","input":{"subject":"Add integration tests for both themes"}},{"type":"tool_use","id":"t07","name":"TaskCreate","input":{"subject":"Update docs with theme configuration"}}]}}
{"timestamp":"$(to_iso $((T2+1)))","content":[{"type":"tool_result","tool_use_id":"t03"}]}
{"timestamp":"$(to_iso $((T2+2)))","content":[{"type":"tool_result","tool_use_id":"t04"}]}
{"timestamp":"$(to_iso $((T2+3)))","content":[{"type":"tool_result","tool_use_id":"t05"}]}
{"timestamp":"$(to_iso $((T2+4)))","content":[{"type":"tool_result","tool_use_id":"t06"}]}
{"timestamp":"$(to_iso $((T2+5)))","content":[{"type":"tool_result","tool_use_id":"t07"}]}
{"timestamp":"$(to_iso $T3)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t08","name":"Edit","input":{"file_path":"/src/config.rs","old_string":"a","new_string":"b"}},{"type":"tool_use","id":"t09","name":"Read","input":{"file_path":"/src/types.rs"}},{"type":"tool_use","id":"t10","name":"Grep","input":{"pattern":"EmphasisTier","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T3+2)))","content":[{"type":"tool_result","tool_use_id":"t08"}]}
{"timestamp":"$(to_iso $((T3+3)))","content":[{"type":"tool_result","tool_use_id":"t09"}]}
{"timestamp":"$(to_iso $((T3+4)))","content":[{"type":"tool_result","tool_use_id":"t10"}]}
{"timestamp":"$(to_iso $T4)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t11","name":"TaskUpdate","input":{"taskId":"1","status":"completed"}},{"type":"tool_use","id":"t12","name":"Edit","input":{"file_path":"/src/render/color.rs","old_string":"c","new_string":"d"}},{"type":"tool_use","id":"t13","name":"Bash","input":{"command":"cargo test -- config"}}]}}
{"timestamp":"$(to_iso $((T4+1)))","content":[{"type":"tool_result","tool_use_id":"t11"}]}
{"timestamp":"$(to_iso $((T4+2)))","content":[{"type":"tool_result","tool_use_id":"t12"}]}
{"timestamp":"$(to_iso $((T4+15)))","content":[{"type":"tool_result","tool_use_id":"t13"}]}
{"timestamp":"$(to_iso $T5)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t14","name":"TaskUpdate","input":{"taskId":"2","status":"completed"}},{"type":"tool_use","id":"t15","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"e","new_string":"f"}}]}}
{"timestamp":"$(to_iso $((T5+1)))","content":[{"type":"tool_result","tool_use_id":"t14"}]}
{"timestamp":"$(to_iso $((T5+2)))","content":[{"type":"tool_result","tool_use_id":"t15"}]}
{"timestamp":"$(to_iso $T6)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t16","name":"Bash","input":{"command":"cargo test"}},{"type":"tool_use","id":"t17","name":"Read","input":{"file_path":"/src/render/layout.rs"}}]}}
JSONL

# ── Stdin payload ────────────────────────────────────────────────────
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "todo-tracking-$(date +%s)",
  "cwd": "$REPO_ROOT",
  "model": {
    "id": "claude-sonnet-4-5-20250929",
    "display_name": "Sonnet 4.5"
  },
  "workspace": {
    "current_dir": "$REPO_ROOT"
  },
  "output_style": {
    "name": "concise"
  },
  "version": "1.0.26",
  "context_window": {
    "context_window_size": 200000,
    "used_percentage": 28,
    "current_usage": {
      "input_tokens": 8200,
      "output_tokens": 12400,
      "cache_creation_input_tokens": 26800,
      "cache_read_input_tokens": 8600
    }
  },
  "cost": {
    "total_cost_usd": 1.2450,
    "total_duration_ms": 1200000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
