#!/usr/bin/env bash
# Generate cc-pulseline output showcasing Agent Tracking on L5+.
# Usage: ./screenshots/agent-tracking.sh [COLUMNS]
#
# Shows 2 running agents (different types/models) + 2 completed agents.
# Uses a temporary project config with agents.max_lines=4 to display all four.
# Creates a fake git repo so L1 shows a real branch name.

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

NOW_S=$(date +%s)
TRANSCRIPT=$(mktemp /tmp/pulseline-agents-XXXXXX.jsonl)
WORKDIR=$(mktemp -d /tmp/pulseline-agent-workdir-XXXXXX)
trap "rm -f '$TRANSCRIPT'; rm -rf '$WORKDIR'" EXIT

# ── Set up fake project dir with git + config ────────────────────────
git -C "$WORKDIR" init -q
git -C "$WORKDIR" checkout -q -b feature/multi-agent-refactor
# Create a dirty file so we get the * indicator
echo "wip" > "$WORKDIR/scratch.txt"
git -C "$WORKDIR" add scratch.txt

mkdir -p "$WORKDIR/.claude"
cat > "$WORKDIR/.claude/pulseline.toml" << 'TOML'
[segments.agents]
max_lines = 4
TOML

# Also create some CLAUDE.md files for realistic L2 counts
echo "# Project" > "$WORKDIR/CLAUDE.md"
echo "# Local" > "$WORKDIR/.claude/CLAUDE.md"
mkdir -p "$WORKDIR/.claude/rules"
echo "# Rule 1" > "$WORKDIR/.claude/rules/style.md"

# ── Helpers ──────────────────────────────────────────────────────────
to_iso() { date -u -r "$1" +"%Y-%m-%dT%H:%M:%S.000Z"; }

# ── Timeline — ~35 min session with multi-agent orchestration ────────
T1=$((NOW_S - 2100))   # -35m
T2=$((NOW_S - 1800))   # -30m
T3=$((NOW_S - 1200))   # -20m

# Completed agent 1: Explore, started ~18m ago, ran ~6m
COMP1_START=$((NOW_S - 1080))
COMP1_END=$((NOW_S - 720))

# Completed agent 2: Plan [sonnet], started ~10m ago, ran ~3m
COMP2_START=$((NOW_S - 600))
COMP2_END=$((NOW_S - 420))

# Running agent 1: code-reviewer, started ~5m ago
RUN1_START=$((NOW_S - 300))

# Running agent 2: Explore [haiku], started ~2m ago
RUN2_START=$((NOW_S - 120))

# Recent running tool
T_RECENT=$((NOW_S - 20))

cat > "$TRANSCRIPT" << JSONL
{"timestamp":"$(to_iso $T1)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t01","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","id":"t02","name":"Grep","input":{"pattern":"render_frame","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T1+2)))","content":[{"type":"tool_result","tool_use_id":"t01"}]}
{"timestamp":"$(to_iso $((T1+3)))","content":[{"type":"tool_result","tool_use_id":"t02"}]}
{"timestamp":"$(to_iso $T2)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t03","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"a","new_string":"b"}},{"type":"tool_use","id":"t04","name":"Bash","input":{"command":"cargo test"}},{"type":"tool_use","id":"t05","name":"Read","input":{"file_path":"/src/types.rs"}}]}}
{"timestamp":"$(to_iso $((T2+2)))","content":[{"type":"tool_result","tool_use_id":"t03"}]}
{"timestamp":"$(to_iso $((T2+20)))","content":[{"type":"tool_result","tool_use_id":"t04"}]}
{"timestamp":"$(to_iso $((T2+3)))","content":[{"type":"tool_result","tool_use_id":"t05"}]}
{"timestamp":"$(to_iso $T3)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t06","name":"Read","input":{"file_path":"/src/render/color.rs"}},{"type":"tool_use","id":"t07","name":"Edit","input":{"file_path":"/src/config.rs","old_string":"c","new_string":"d"}}]}}
{"timestamp":"$(to_iso $((T3+2)))","content":[{"type":"tool_result","tool_use_id":"t06"}]}
{"timestamp":"$(to_iso $((T3+3)))","content":[{"type":"tool_result","tool_use_id":"t07"}]}
{"timestamp":"$(to_iso $COMP1_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task1","name":"Task","input":{"description":"Explore codebase architecture","subagent_type":"Explore"}}]}}
{"timestamp":"$(to_iso $((COMP1_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-explore-arch","status":"running","prompt":"Map module dependencies and data flow patterns","agentType":"Explore"}}
{"timestamp":"$(to_iso $COMP1_END)","content":[{"type":"tool_result","tool_use_id":"toolu_task1"}]}
{"timestamp":"$(to_iso $COMP1_END)","type":"progress","data":{"type":"agent_progress","agentId":"agent-explore-arch","status":"completed"}}
{"timestamp":"$(to_iso $COMP2_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task2","name":"Task","input":{"description":"Design config migration plan","subagent_type":"Plan","model":"sonnet"}}]}}
{"timestamp":"$(to_iso $((COMP2_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-plan-cfg","status":"running","prompt":"Plan config schema migration strategy","agentType":"Plan","model":"claude-sonnet-4-5-20250929"}}
{"timestamp":"$(to_iso $COMP2_END)","content":[{"type":"tool_result","tool_use_id":"toolu_task2"}]}
{"timestamp":"$(to_iso $COMP2_END)","type":"progress","data":{"type":"agent_progress","agentId":"agent-plan-cfg","status":"completed"}}
{"timestamp":"$(to_iso $RUN1_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task3","name":"Task","input":{"description":"Review render pipeline changes","subagent_type":"code-reviewer"}}]}}
{"timestamp":"$(to_iso $((RUN1_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-reviewer-1","status":"running","prompt":"Review render pipeline changes for correctness","agentType":"code-reviewer"}}
{"timestamp":"$(to_iso $RUN2_START)","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_task4","name":"Task","input":{"description":"Search for similar color patterns","subagent_type":"Explore","model":"haiku"}}]}}
{"timestamp":"$(to_iso $((RUN2_START+1)))","type":"progress","data":{"type":"agent_progress","agentId":"agent-explore-clr","status":"running","prompt":"Find similar ANSI color implementations","agentType":"Explore","model":"claude-haiku-4-5"}}
{"timestamp":"$(to_iso $T_RECENT)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t08","name":"Read","input":{"file_path":"/src/render/layout.rs"}}]}}
JSONL

# ── Stdin payload ────────────────────────────────────────────────────
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "agent-tracking-$(date +%s)",
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
    "used_percentage": 52,
    "current_usage": {
      "input_tokens": 22400,
      "output_tokens": 35800,
      "cache_creation_input_tokens": 38600,
      "cache_read_input_tokens": 7200
    }
  },
  "cost": {
    "total_cost_usd": 5.6230,
    "total_duration_ms": 2100000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
