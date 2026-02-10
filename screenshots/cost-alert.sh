#!/usr/bin/env bash
# Generate cc-pulseline output showing Cost Alert state (burn rate >$50/h → magenta).
# Usage: ./screenshots/cost-alert.sh [COLUMNS]
#
# The payload simulates an intense Opus session: $18.75 in 12 minutes = $93.75/h.
# This triggers COST_HIGH_RATE (color 201, magenta) on the hourly rate display.

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

NOW_S=$(date +%s)
TRANSCRIPT=$(mktemp /tmp/pulseline-cost-alert-XXXXXX.jsonl)
trap "rm -f '$TRANSCRIPT'" EXIT

# ── Helpers ──────────────────────────────────────────────────────────
to_iso() { date -u -r "$1" +"%Y-%m-%dT%H:%M:%S.000Z"; }

# ── Timeline — a short, intense burst session (~12 minutes) ──────────
T1=$((NOW_S - 720))    # -12m: session start
T2=$((NOW_S - 600))    # -10m
T3=$((NOW_S - 420))    # -7m
T4=$((NOW_S - 240))    # -4m
# Running agent: started ~2m ago
RUN_START=$((NOW_S - 120))
# Recent tool still running
T5=$((NOW_S - 30))

cat > "$TRANSCRIPT" << JSONL
{"timestamp":"$(to_iso $T1)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","id":"toolu_02","name":"Read","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","id":"toolu_03","name":"Grep","input":{"pattern":"pub fn","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T1+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_01"}]}
{"timestamp":"$(to_iso $((T1+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_02"}]}
{"timestamp":"$(to_iso $((T1+4)))","content":[{"type":"tool_result","tool_use_id":"toolu_03"}]}
{"timestamp":"$(to_iso $T2)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_04","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"a","new_string":"b"}},{"type":"tool_use","id":"toolu_05","name":"Edit","input":{"file_path":"/src/render/color.rs","old_string":"c","new_string":"d"}},{"type":"tool_use","id":"toolu_06","name":"Bash","input":{"command":"cargo test"}}]}}
{"timestamp":"$(to_iso $((T2+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_04"}]}
{"timestamp":"$(to_iso $((T2+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_05"}]}
{"timestamp":"$(to_iso $((T2+20)))","content":[{"type":"tool_result","tool_use_id":"toolu_06"}]}
{"timestamp":"$(to_iso $T3)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_07","name":"Read","input":{"file_path":"/src/types.rs"}},{"type":"tool_use","id":"toolu_08","name":"Write","input":{"file_path":"/src/render/fmt.rs","content":"updated"}},{"type":"tool_use","id":"toolu_09","name":"Bash","input":{"command":"cargo clippy -- -D warnings"}}]}}
{"timestamp":"$(to_iso $((T3+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_07"}]}
{"timestamp":"$(to_iso $((T3+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_08"}]}
{"timestamp":"$(to_iso $((T3+25)))","content":[{"type":"tool_result","tool_use_id":"toolu_09"}]}
{"timestamp":"$(to_iso $T4)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_10","name":"Edit","input":{"file_path":"/src/state/mod.rs","old_string":"e","new_string":"f"}},{"type":"tool_use","id":"toolu_11","name":"Read","input":{"file_path":"/src/config.rs"}}]}}
{"timestamp":"$(to_iso $((T4+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_10"}]}
{"timestamp":"$(to_iso $((T4+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_11"}]}
{"timestamp":"$(to_iso $RUN_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task1","name":"Task","input":{"description":"Validate render pipeline changes","subagent_type":"Bash"}}]}}
{"timestamp":"$(to_iso $((RUN_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-validate-1","status":"running","prompt":"Run full test suite and verify render output","agentType":"Bash"}}
{"timestamp":"$(to_iso $T5)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_12","name":"Bash","input":{"command":"cargo test --lib -- render"}},{"type":"tool_use","id":"toolu_13","name":"Read","input":{"file_path":"/src/render/layout.rs"}}]}}
JSONL

# ── Stdin payload ────────────────────────────────────────────────────
# $18.75 in 720000ms (12 min) = $93.75/h → triggers COST_HIGH_RATE (201, magenta)
# Context at a moderate 38% — keep attention on the cost, not context
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "cost-alert-$(date +%s)",
  "cwd": "$REPO_ROOT",
  "model": {
    "id": "claude-opus-4-6",
    "display_name": "Opus 4.6"
  },
  "workspace": {
    "current_dir": "$REPO_ROOT"
  },
  "output_style": {
    "name": "verbose"
  },
  "version": "1.0.26",
  "context_window": {
    "context_window_size": 200000,
    "used_percentage": 38,
    "current_usage": {
      "input_tokens": 18400,
      "output_tokens": 31200,
      "cache_creation_input_tokens": 22800,
      "cache_read_input_tokens": 3600
    }
  },
  "cost": {
    "total_cost_usd": 18.7500,
    "total_duration_ms": 720000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
