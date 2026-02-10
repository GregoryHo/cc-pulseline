#!/usr/bin/env bash
# Generate cc-pulseline output showcasing Tool Tracking on L4.
# Usage: ./screenshots/tool-tracking.sh [COLUMNS]
#
# Shows 2 running tools with targets + 4 completed tool counts (top by count).
# No agents — keeps the focus squarely on the tool activity line.

set -euo pipefail

COLS="${1:-130}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${BINARY:-$REPO_ROOT/target/release/cc-pulseline}"

if [[ ! -x "$BINARY" ]]; then
  echo "Building release binary..." >&2
  (cd "$REPO_ROOT" && cargo build --release 2>&1 >&2)
fi

NOW_S=$(date +%s)
TRANSCRIPT=$(mktemp /tmp/pulseline-tools-XXXXXX.jsonl)
trap "rm -f '$TRANSCRIPT'" EXIT

# ── Helpers ──────────────────────────────────────────────────────────
to_iso() { date -u -r "$1" +"%Y-%m-%dT%H:%M:%S.000Z"; }

# ── Timeline — ~25 min session with heavy tool churn ─────────────────
T1=$((NOW_S - 1500))   # -25m
T2=$((NOW_S - 1200))   # -20m
T3=$((NOW_S - 900))    # -15m
T4=$((NOW_S - 600))    # -10m
T5=$((NOW_S - 300))    # -5m
T6=$((NOW_S - 120))    # -2m  (batch with running tools)
T7=$((NOW_S - 30))     # -30s (final running tools)

# Goal: Read ×8, Edit ×5, Bash ×4, Grep ×3 completed + 2 running (Bash + Edit)
cat > "$TRANSCRIPT" << JSONL
{"timestamp":"$(to_iso $T1)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t01","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","id":"t02","name":"Read","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","id":"t03","name":"Grep","input":{"pattern":"RenderFrame","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T1+1)))","content":[{"type":"tool_result","tool_use_id":"t01"}]}
{"timestamp":"$(to_iso $((T1+2)))","content":[{"type":"tool_result","tool_use_id":"t02"}]}
{"timestamp":"$(to_iso $((T1+3)))","content":[{"type":"tool_result","tool_use_id":"t03"}]}
{"timestamp":"$(to_iso $T2)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t04","name":"Read","input":{"file_path":"/src/types.rs"}},{"type":"tool_use","id":"t05","name":"Edit","input":{"file_path":"/src/types.rs","old_string":"a","new_string":"b"}},{"type":"tool_use","id":"t06","name":"Bash","input":{"command":"cargo test"}},{"type":"tool_use","id":"t07","name":"Grep","input":{"pattern":"SessionState","path":"/src/"}}]}}
{"timestamp":"$(to_iso $((T2+1)))","content":[{"type":"tool_result","tool_use_id":"t04"}]}
{"timestamp":"$(to_iso $((T2+2)))","content":[{"type":"tool_result","tool_use_id":"t05"}]}
{"timestamp":"$(to_iso $((T2+20)))","content":[{"type":"tool_result","tool_use_id":"t06"}]}
{"timestamp":"$(to_iso $((T2+3)))","content":[{"type":"tool_result","tool_use_id":"t07"}]}
{"timestamp":"$(to_iso $T3)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t08","name":"Read","input":{"file_path":"/src/render/layout.rs"}},{"type":"tool_use","id":"t09","name":"Read","input":{"file_path":"/src/render/color.rs"}},{"type":"tool_use","id":"t10","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"c","new_string":"d"}},{"type":"tool_use","id":"t11","name":"Bash","input":{"command":"cargo clippy -- -D warnings"}}]}}
{"timestamp":"$(to_iso $((T3+1)))","content":[{"type":"tool_result","tool_use_id":"t08"}]}
{"timestamp":"$(to_iso $((T3+2)))","content":[{"type":"tool_result","tool_use_id":"t09"}]}
{"timestamp":"$(to_iso $((T3+3)))","content":[{"type":"tool_result","tool_use_id":"t10"}]}
{"timestamp":"$(to_iso $((T3+25)))","content":[{"type":"tool_result","tool_use_id":"t11"}]}
{"timestamp":"$(to_iso $T4)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t12","name":"Read","input":{"file_path":"/src/config.rs"}},{"type":"tool_use","id":"t13","name":"Edit","input":{"file_path":"/src/render/color.rs","old_string":"e","new_string":"f"}},{"type":"tool_use","id":"t14","name":"Grep","input":{"pattern":"colorize","path":"/src/render/"}},{"type":"tool_use","id":"t15","name":"Bash","input":{"command":"cargo test -- render"}}]}}
{"timestamp":"$(to_iso $((T4+1)))","content":[{"type":"tool_result","tool_use_id":"t12"}]}
{"timestamp":"$(to_iso $((T4+2)))","content":[{"type":"tool_result","tool_use_id":"t13"}]}
{"timestamp":"$(to_iso $((T4+3)))","content":[{"type":"tool_result","tool_use_id":"t14"}]}
{"timestamp":"$(to_iso $((T4+18)))","content":[{"type":"tool_result","tool_use_id":"t15"}]}
{"timestamp":"$(to_iso $T5)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t16","name":"Read","input":{"file_path":"/src/render/fmt.rs"}},{"type":"tool_use","id":"t17","name":"Edit","input":{"file_path":"/src/render/fmt.rs","old_string":"g","new_string":"h"}},{"type":"tool_use","id":"t18","name":"Edit","input":{"file_path":"/src/render/icons.rs","old_string":"i","new_string":"j"}},{"type":"tool_use","id":"t19","name":"Bash","input":{"command":"cargo test -- fmt"}}]}}
{"timestamp":"$(to_iso $((T5+1)))","content":[{"type":"tool_result","tool_use_id":"t16"}]}
{"timestamp":"$(to_iso $((T5+2)))","content":[{"type":"tool_result","tool_use_id":"t17"}]}
{"timestamp":"$(to_iso $((T5+3)))","content":[{"type":"tool_result","tool_use_id":"t18"}]}
{"timestamp":"$(to_iso $((T5+15)))","content":[{"type":"tool_result","tool_use_id":"t19"}]}
{"timestamp":"$(to_iso $T7)","message":{"role":"assistant","content":[{"type":"tool_use","id":"t20","name":"Bash","input":{"command":"cargo test --lib"}},{"type":"tool_use","id":"t21","name":"Edit","input":{"file_path":"/src/render/layout.rs","old_string":"k","new_string":"l"}}]}}
JSONL
# t20 (Bash) and t21 (Edit) are left without tool_result → running

# Completed tally: Read ×7, Edit ×5, Bash ×4, Grep ×3 → top 4 shown
# Running: Bash: cargo test --lib | Edit: /src/render/layout.rs

# ── Stdin payload — moderate session, focus on tool richness ─────────
cat << JSON | COLUMNS="$COLS" PULSELINE_THEME=dark "$BINARY"
{
  "session_id": "tool-tracking-$(date +%s)",
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
    "used_percentage": 31,
    "current_usage": {
      "input_tokens": 9800,
      "output_tokens": 16400,
      "cache_creation_input_tokens": 28600,
      "cache_read_input_tokens": 7200
    }
  },
  "cost": {
    "total_cost_usd": 1.8420,
    "total_duration_ms": 1500000
  },
  "transcript_path": "$TRANSCRIPT"
}
JSON
