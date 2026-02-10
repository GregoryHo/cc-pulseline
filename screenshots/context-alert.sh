#!/usr/bin/env bash
# Generate cc-pulseline output showing Context Alert state (CTX ≥70% → red).
# Usage: ./screenshots/context-alert.sh [COLUMNS]
#
# The payload uses used_percentage=78, which triggers CTX_CRITICAL (ALERT_RED).
# Claude Code auto-compacts at ~80%, so 78% is a realistic "danger zone" value.

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

NOW_S=$(date +%s)
TRANSCRIPT=$(mktemp /tmp/pulseline-ctx-alert-XXXXXX.jsonl)
trap "rm -f '$TRANSCRIPT'" EXIT

# ── Helpers ──────────────────────────────────────────────────────────
to_iso() { date -u -r "$1" +"%Y-%m-%dT%H:%M:%S.000Z"; }

# ── Timeline — a session that's been going for ~1h 20m ───────────────
T1=$((NOW_S - 4800))   # -80m
T2=$((NOW_S - 4200))   # -70m
T3=$((NOW_S - 3600))   # -60m
T4=$((NOW_S - 2700))   # -45m
T5=$((NOW_S - 1800))   # -30m
T6=$((NOW_S - 900))    # -15m
# Running agent: started ~4m ago
RUN_START=$((NOW_S - 240))
# Recent tools still running
T7=$((NOW_S - 60))

cat > "$TRANSCRIPT" << JSONL
{"timestamp":"$(to_iso $T1)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","id":"toolu_02","name":"Grep","input":{"pattern":"SessionState","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T1+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_01"}]}
{"timestamp":"$(to_iso $((T1+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_02"}]}
{"timestamp":"$(to_iso $T2)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_03","name":"Edit","input":{"file_path":"/src/state/mod.rs","old_string":"a","new_string":"b"}},{"type":"tool_use","id":"toolu_04","name":"Read","input":{"file_path":"/src/providers/env.rs"}},{"type":"tool_use","id":"toolu_05","name":"Bash","input":{"command":"cargo test"}}]}}
{"timestamp":"$(to_iso $((T2+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_03"}]}
{"timestamp":"$(to_iso $((T2+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_04"}]}
{"timestamp":"$(to_iso $((T2+25)))","content":[{"type":"tool_result","tool_use_id":"toolu_05"}]}
{"timestamp":"$(to_iso $T3)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_06","name":"Read","input":{"file_path":"/src/render/layout.rs"}},{"type":"tool_use","id":"toolu_07","name":"Edit","input":{"file_path":"/src/render/color.rs","old_string":"c","new_string":"d"}},{"type":"tool_use","id":"toolu_08","name":"Grep","input":{"pattern":"emphasis_for_theme","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T3+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_06"}]}
{"timestamp":"$(to_iso $((T3+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_07"}]}
{"timestamp":"$(to_iso $((T3+4)))","content":[{"type":"tool_result","tool_use_id":"toolu_08"}]}
{"timestamp":"$(to_iso $T4)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_09","name":"Write","input":{"file_path":"/src/render/fmt.rs","content":"updated"}},{"type":"tool_use","id":"toolu_10","name":"Bash","input":{"command":"cargo clippy -- -D warnings"}},{"type":"tool_use","id":"toolu_11","name":"Read","input":{"file_path":"/src/config.rs"}}]}}
{"timestamp":"$(to_iso $((T4+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_09"}]}
{"timestamp":"$(to_iso $((T4+20)))","content":[{"type":"tool_result","tool_use_id":"toolu_10"}]}
{"timestamp":"$(to_iso $((T4+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_11"}]}
{"timestamp":"$(to_iso $T5)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_12","name":"Edit","input":{"file_path":"/src/types.rs","old_string":"e","new_string":"f"}},{"type":"tool_use","id":"toolu_13","name":"Glob","input":{"pattern":"tests/**/*.rs"}},{"type":"tool_use","id":"toolu_14","name":"Read","input":{"file_path":"/src/lib.rs"}}]}}
{"timestamp":"$(to_iso $((T5+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_12"}]}
{"timestamp":"$(to_iso $((T5+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_13"}]}
{"timestamp":"$(to_iso $((T5+4)))","content":[{"type":"tool_result","tool_use_id":"toolu_14"}]}
{"timestamp":"$(to_iso $T6)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_15","name":"Bash","input":{"command":"cargo test -- --nocapture"}},{"type":"tool_use","id":"toolu_16","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"g","new_string":"h"}}]}}
{"timestamp":"$(to_iso $((T6+30)))","content":[{"type":"tool_result","tool_use_id":"toolu_15"}]}
{"timestamp":"$(to_iso $((T6+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_16"}]}
{"timestamp":"$(to_iso $RUN_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task1","name":"Task","input":{"description":"Refactor emphasis tier system","subagent_type":"code-simplifier"}}]}}
{"timestamp":"$(to_iso $((RUN_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-refactor-1","status":"running","prompt":"Simplify and consolidate emphasis tier logic","agentType":"code-simplifier"}}
{"timestamp":"$(to_iso $T7)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_17","name":"Read","input":{"file_path":"/src/render/color.rs"}}]}}
JSONL

# ── Stdin payload — 78% context usage triggers ALERT_RED ─────────────
# 78% of 200k = 156k tokens used — realistic for a long session
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "ctx-alert-$(date +%s)",
  "cwd": "$REPO_ROOT",
  "model": {
    "id": "claude-sonnet-4-5-20250929",
    "display_name": "Sonnet 4.5"
  },
  "workspace": {
    "current_dir": "$REPO_ROOT"
  },
  "output_style": {
    "name": "normal"
  },
  "version": "1.0.26",
  "context_window": {
    "context_window_size": 200000,
    "used_percentage": 78,
    "current_usage": {
      "input_tokens": 42500,
      "output_tokens": 38200,
      "cache_creation_input_tokens": 45800,
      "cache_read_input_tokens": 29500
    }
  },
  "cost": {
    "total_cost_usd": 8.2340,
    "total_duration_ms": 4920000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
