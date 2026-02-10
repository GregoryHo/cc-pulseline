#!/usr/bin/env bash
# Generate realistic cc-pulseline output for README hero screenshot.
# Usage: ./screenshots/hero.sh [COLUMNS]
#
# This creates a transcript with timestamps relative to NOW so that
# agent durations look natural (e.g., "3m", "5m").
# Pipe the output through a terminal screenshot tool of your choice.

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

NOW_S=$(date +%s)
TRANSCRIPT=$(mktemp /tmp/pulseline-hero-XXXXXX.jsonl)
trap "rm -f '$TRANSCRIPT'" EXIT

# ── Helpers ──────────────────────────────────────────────────────────
to_iso() { date -u -r "$1" +"%Y-%m-%dT%H:%M:%S.000Z"; }

# ── Timeline (minutes ago) ───────────────────────────────────────────
# Completed tools: scattered across the session
T1=$((NOW_S - 2700))   # -45m: Read + Grep
T2=$((NOW_S - 2400))   # -40m: Edit + Read + Glob + Bash
T3=$((NOW_S - 2100))   # -35m: Read + Write + Grep
T4=$((NOW_S - 1800))   # -30m: Edit ×2 + Bash + WebSearch
# Completed agent: started ~8m ago, ran for ~3.5m
COMP_START=$((NOW_S - 480))
COMP_END=$((NOW_S - 270))
# Running agent: started ~3m ago
RUN_START=$((NOW_S - 180))
# Recent tools (still running)
T7=$((NOW_S - 120))

cat > "$TRANSCRIPT" << JSONL
{"timestamp":"$(to_iso $T1)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01","name":"Read","input":{"file_path":"/src/render/layout.rs"}},{"type":"tool_use","id":"toolu_02","name":"Grep","input":{"pattern":"render_frame","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T1+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_01"}]}
{"timestamp":"$(to_iso $((T1+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_02"}]}
{"timestamp":"$(to_iso $T2)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_03","name":"Edit","input":{"file_path":"/src/render/color.rs","old_string":"old","new_string":"new"}},{"type":"tool_use","id":"toolu_04","name":"Read","input":{"file_path":"/src/types.rs"}},{"type":"tool_use","id":"toolu_05","name":"Glob","input":{"pattern":"**/*.rs"}},{"type":"tool_use","id":"toolu_06","name":"Bash","input":{"command":"cargo test"}}]}}
{"timestamp":"$(to_iso $((T2+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_03"}]}
{"timestamp":"$(to_iso $((T2+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_04"}]}
{"timestamp":"$(to_iso $((T2+4)))","content":[{"type":"tool_result","tool_use_id":"toolu_05"}]}
{"timestamp":"$(to_iso $((T2+30)))","content":[{"type":"tool_result","tool_use_id":"toolu_06"}]}
{"timestamp":"$(to_iso $T3)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_07","name":"Read","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","id":"toolu_08","name":"Write","input":{"file_path":"/src/render/icons.rs","content":"updated"}},{"type":"tool_use","id":"toolu_09","name":"Grep","input":{"pattern":"EmphasisTier","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T3+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_07"}]}
{"timestamp":"$(to_iso $((T3+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_08"}]}
{"timestamp":"$(to_iso $((T3+4)))","content":[{"type":"tool_result","tool_use_id":"toolu_09"}]}
{"timestamp":"$(to_iso $T4)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_10","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"a","new_string":"b"}},{"type":"tool_use","id":"toolu_11","name":"Edit","input":{"file_path":"/src/config.rs","old_string":"c","new_string":"d"}},{"type":"tool_use","id":"toolu_12","name":"Bash","input":{"command":"cargo clippy -- -D warnings"}},{"type":"tool_use","id":"toolu_13","name":"WebSearch","input":{"query":"rust ansi 256 color codes"}}]}}
{"timestamp":"$(to_iso $((T4+2)))","content":[{"type":"tool_result","tool_use_id":"toolu_10"}]}
{"timestamp":"$(to_iso $((T4+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_11"}]}
{"timestamp":"$(to_iso $((T4+25)))","content":[{"type":"tool_result","tool_use_id":"toolu_12"}]}
{"timestamp":"$(to_iso $((T4+8)))","content":[{"type":"tool_result","tool_use_id":"toolu_13"}]}
{"timestamp":"$(to_iso $COMP_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task1","name":"Task","input":{"description":"Explore color system patterns","subagent_type":"Explore","model":"haiku"}}]}}
{"timestamp":"$(to_iso $((COMP_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-explore-1","status":"running","prompt":"Analyze color patterns across render modules","agentType":"Explore","model":"claude-haiku-4-5"}}
{"timestamp":"$(to_iso $COMP_END)","content":[{"type":"tool_result","tool_use_id":"toolu_task1"}]}
{"timestamp":"$(to_iso $COMP_END)","type":"progress","data":{"type":"agent_progress","agentId":"agent-explore-1","status":"completed"}}
{"timestamp":"$(to_iso $RUN_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_14","name":"Read","input":{"file_path":"/src/render/fmt.rs"}},{"type":"tool_use","id":"toolu_task2","name":"Task","input":{"description":"Review test coverage","subagent_type":"Bash"}}]}}
{"timestamp":"$(to_iso $((RUN_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-bash-1","status":"running","prompt":"Run full test suite and analyze coverage","agentType":"Bash"}}
{"timestamp":"$(to_iso $((RUN_START+3)))","content":[{"type":"tool_result","tool_use_id":"toolu_14"}]}
{"timestamp":"$(to_iso $T7)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_15","name":"Bash","input":{"command":"cargo test --lib"}},{"type":"tool_use","id":"toolu_16","name":"Read","input":{"file_path":"/src/providers/transcript.rs"}}]}}
JSONL

# ── Stdin payload ────────────────────────────────────────────────────
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "hero-$(date +%s)",
  "cwd": "$REPO_ROOT",
  "model": {
    "id": "claude-opus-4-6",
    "display_name": "Opus 4.6"
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
    "used_percentage": 43,
    "current_usage": {
      "input_tokens": 12840,
      "output_tokens": 24576,
      "cache_creation_input_tokens": 68200,
      "cache_read_input_tokens": 114510
    }
  },
  "cost": {
    "total_cost_usd": 3.4725,
    "total_duration_ms": 2712000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
