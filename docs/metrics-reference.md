# Metrics Reference

Detailed reference for every metric rendered by cc-pulseline, covering data source, parsing method, cache strategy, and color assignment.

## Line 1: Identity

Session identity at a glance (see `IdentitySegmentConfig` in `src/config.rs` for the segment list).

| Metric | Prefix | Data Source | Parsing Method | Cache | Color |
|--------|--------|-------------|----------------|-------|-------|
| Model | `M:` | `payload.model` | JSON field extraction | None | STABLE_BLUE (111) |
| Agent | `AG:` | `payload.agent.name` | JSON field extraction | None | head_agent (183) |
| Style | `S:` | `payload.output_style.name` | JSON field extraction | None | tier.secondary (146/240) |
| Version | `CC:` | `payload.version` | JSON field extraction | None | tier.secondary (146/240) |
| Project | `P:` | `payload.workspace.current_dir` | HOME replaced with `~` via `resolve_project_path_display()` | None | tier.secondary (146/240) |
| Git | `G:` | `git status --porcelain=v2 --branch` | Shell out, parse branch/dirty/ahead/behind | 10s TTL | STABLE_GREEN (71) / ALERT_ORANGE (214) / ACTIVE_CORAL (209) |
| Git File Stats | (inline) | `git status --porcelain=v2` | Classify entries: `!` modified, `+` added, `✘` deleted, `?` untracked | 10s TTL | GIT_MODIFIED (214) / GIT_ADDED (71) / GIT_DELETED (196) / ACTIVE_PURPLE (183) |
| Worktree | `(WT)` | `payload.worktree` / `payload.workspace.git_worktree` | Presence check (`is_in_worktree()`) | None | tier.structural (103/245) |
| Effort | `E:` (effort pill) | `payload.effort.level` | JSON field extraction | None | level-driven (`color_for_effort_level`) |
| Thinking | `[T]` (thinking pill) | `payload.thinking.enabled` | Rendered only when `true` | None | head_thinking (178) |

### Git State Details

| State | Visual | Color |
|-------|--------|-------|
| Clean | `G:main` | STABLE_GREEN (71) |
| Dirty | `G:main*` | ALERT_ORANGE (214) on the `*` |
| Ahead | `G:main up-3` | ACTIVE_CORAL (209) |
| Behind | `G:main down-2` | ACTIVE_CORAL (209) |

### Git File Stats (Starship-style)

| Category | Symbol | Color | Source |
|----------|--------|-------|--------|
| Modified | `!` | GIT_MODIFIED (214) | Porcelain v2 `1 .M` / `1 M.` entries |
| Added | `+` | GIT_ADDED (71) | Porcelain v2 `1 A.` entries |
| Deleted | `✘` | GIT_DELETED (196) | Porcelain v2 `1 .D` / `1 D.` entries |
| Untracked | `?` | ACTIVE_PURPLE (183) | Porcelain v2 `?` entries |

Zero-count categories are omitted. Stats appear after branch/ahead/behind. Toggled via `show_git_stats` (default: false).

All L1 segments are individually togglable via config: `show_model`, `show_style`, `show_version`, `show_project`, `show_git`, `show_git_stats`, `show_agent`, `show_worktree`, `show_effort`, `show_thinking`.

### Example Output

Normal (clean, no remote tracking):

```
M:Opus 4.6 | S:explanatory | CC:2.1.37 | P:~/projects/myapp | G:main
```

Git dirty + ahead:

```
M:Opus 4.6 | S:concise | CC:2.2.0 | P:~/projects/myapp | G:feature/auth* ↑3
```

Git behind:

```
M:Sonnet 4.5 | S:concise | CC:2.2.0 | P:~/work/api | G:main ↓2
```

Git with file stats enabled:

```
M:Opus 4.6 | S:concise | CC:2.2.0 | P:~/projects/myapp | G:feature/auth* ↑3 !3 +1 ✘2 ?4
```

## Line 2: Config Counts

Counts of the project's Claude Code configuration. **The whole row is opt-in** — `[segments.config] enabled` defaults to `false`; the individual `show_*` toggles apply once enabled.

| Metric | Data Source | Parsing Method | Cache | Icon Color |
|--------|-------------|----------------|-------|------------|
| CLAUDE.md | 5 filesystem paths | Check existence of `~/.claude/CLAUDE.md`, `{root}/CLAUDE.md`, `{root}/CLAUDE.local.md`, `{root}/.claude/CLAUDE.md`, `{root}/.claude/CLAUDE.local.md` | 10s TTL | INDICATOR_CLAUDE_MD (109) |
| Rules | `.md` files in rules dirs | Recursive scan of `{root}/.claude/rules/` + `~/.claude/rules/` | 10s TTL | INDICATOR_RULES (108) |
| Memories | `.md` files in memory dir | Flat scan of `~/.claude/projects/{encoded-path}/memory/` | 10s TTL | INDICATOR_MEMORY (182) |
| Hooks | settings.json `hooks` keys | JSON parse of hooks object keys in settings.json (project + local + user) | 10s TTL | INDICATOR_HOOKS (179) |
| MCPs | Multiple config files | Scoped dedup -- user scope (settings.json + .claude.json - disabled) + project scope (.mcp.json + settings.json + settings.local.json - disabledMcpjsonServers) | 10s TTL | INDICATOR_MCP (139) |
| Skills | Skills directories | Count directories in `{root}/.claude/skills/` + `~/.claude/skills/` (excludes .codex/) | 10s TTL | INDICATOR_SKILLS (73) |
| Plugins | `installed_plugins.json` + settings.json | Count enabled Claude Code plugins (CC 2.0.12+) | 10s TTL | INDICATOR_MCP (139, reused) |
| Duration | `payload.cost.total_duration_ms` | Elapsed session duration | None | INDICATOR_DURATION (174) |

### L2 Color Hierarchy

Each L2 segment uses three color layers:
- **Icon**: Per-metric INDICATOR color (unique visual fingerprint)
- **Count**: tier.secondary (146/240) -- the actual data value
- **Label**: tier.structural (103/245) -- descriptive text

The L2 row has a master `enabled` toggle (default `false`); segments are then individually togglable via config: `show_claude_md`, `show_rules`, `show_memory`, `show_hooks`, `show_mcp`, `show_skills`, `show_plugins`, `show_duration`.

### Duration Display Format

| Elapsed | Display |
|---------|---------|
| < 1 minute | `<1m` |
| Minutes | `Xm` |
| Hours + minutes | `Xh Xm` |
| Days + hours | `Xd Xh` |

### Example Output

Normal (ASCII mode, various counts):

```
1 CLAUDE.md | 3 rules | 2 memories | 2 hooks | 4 MCPs | 1 skills | 1h 23m
```

Zero state (no project config):

```
0 CLAUDE.md | 0 rules | 0 memories | 0 hooks | 0 MCPs | 0 skills | <1m
```

Long session:

```
2 CLAUDE.md | 5 rules | 1 memories | 3 hooks | 6 MCPs | 2 skills | 2d 5h
```

## Line 3: Budget

Three segments tracking resource consumption.

| Metric | Prefix | Data Source | Parsing Method | Cache | Color |
|--------|--------|-------------|----------------|-------|-------|
| Context | `CTX:` | `payload.context_window.*` | Percentage = used/total, formatted as `pct% (used/total)` | L3 all-or-nothing fallback | State-driven (see below) |
| Tokens | `TOK:` | `payload.context_window.current_usage.*` | Three sub-fields: I (input), O (output), C (cache hit-rate = `cache_read / (input + cache_creation + cache_read)`, `--` when no cache signal) | L3 all-or-nothing fallback | tier.structural labels, tier.secondary values; C value colored by hit-rate (≥80% green, ≥40% neutral, else warn) |
| Cost | `$` | `payload.cost.total_cost_usd` + `payload.cost.total_duration_ms` | Total cost + computed burn rate ($/h) | L3 all-or-nothing fallback | COST_BASE (222) + rate-based gradient |
| Speed | `↗N/s` (inline in TOK) | Computed from successive output token snapshots | Delta-based tok/s with 2s window; holds last known value when idle | SessionState in-memory | tier.primary when data exists, tier.structural when absent (matches token values) |

### Context Color States

| Condition | Color | Meaning |
|-----------|-------|---------|
| < 55% used | STABLE_GREEN (71) | Normal |
| 55-69% used | ACTIVE_AMBER (178) | Elevated |
| >= 70% used | ALERT_RED (196) | Critical |

### Cost Rate Coloring

| Burn Rate | Color | Visual |
|-----------|-------|--------|
| < $10/h | COST_LOW_RATE (186) | Subdued peach -- normal spending |
| $10-50/h | COST_MED_RATE (221) | Gold -- noticeable |
| > $50/h | COST_HIGH_RATE (201) | Magenta -- urgent, matches ALERT_MAGENTA |

The total cost always uses COST_BASE (222, warm gold) regardless of rate.

### Speed Display (Inline in TOK)

Speed is displayed inline within the TOK segment after output tokens: `O:20.0k ↗1.5K/s`, tracking output tokens only.

| State | Display | Notes |
|-------|---------|-------|
| First invocation | (no speed shown) | Speed is None, omitted entirely |
| Active generation | `↗1.5K/s` | Inline after output tokens |
| Idle (>2s) | `↗1.5K/s` | Holds last known value (no decay) |
| Below 1K | `↗42/s` | Integer format for values <1000 |
| 1K and above | `↗1.5K/s` | One decimal place with uppercase K |

Speed is computed via delta-based tracking: successive output token values are compared with a 2s window. Not included in `has_data()` to avoid interfering with L3 cache logic. When `current_tokens` is `None`, state is preserved (no time anchor corruption).

All L3 segments are individually togglable via config: `show_context`, `show_tokens`, `show_cost`, `show_speed`.

### Example Output

Normal (context <55%, speed enabled):

```
CTX:43% (86.0k/200.0k) | TOK I:10.0k O:20.0k ↗1.5K/s C:50% | $3.50 ($3.50/h)
```

Context warning (55-69%):

```
CTX:62% (124.0k/200.0k) | TOK I:35.0k O:8.0k C:46% | $5.80 ($2.90/h)
```

Context critical (≥70%):

```
CTX:75% (150.0k/200.0k) | TOK I:45.0k O:12.0k C:45% | $8.20 ($4.10/h)
```

High burn rate (>$50/h):

```
CTX:15% (30.0k/200.0k) | TOK I:8.0k O:3.0k C:33% | $12.50 ($75.00/h)
```

Missing data fallback:

```
CTX:--% (--/--) | TOK I:-- O:-- C:-- | $0.00 ($0.00/h)
```

## Quota Line

Usage quota rendered between L3 and activity lines. Shows subscription usage percentage from Claude Code's native `rate_limits` stdin field (CC 2.1.80+).

| Metric | Data Source | Parsing Method | Cache | Color |
|--------|-------------|----------------|-------|-------|
| 5-hour quota | `payload.rate_limits.five_hour` | `used_percentage` (0-100) + `resets_at` (epoch seconds) → relative minutes | None (pure function) | CTX thresholds (green/amber/red) |
| 7-day quota | `payload.rate_limits.seven_day` | Same as above | None | CTX thresholds |

### Quota Architecture

The quota system reads directly from the stdin payload — no network I/O, no background subprocess, no cache files:

- `QuotaMetrics::from_rate_limits(rate_limits, now_secs)` — pure function converting epoch `resets_at` to relative minutes
- `has_data()` — hidden when no percentage data present (API users, old CC versions, pre-first-call)

### Quota Color States

Uses the same CTX threshold colors as context percentage:

| Usage | Color | Meaning |
|-------|-------|---------|
| < 50% | CTX_GOOD / STABLE_GREEN (71) | Normal |
| 50-84% | CTX_WARN / ACTIVE_AMBER (178) | Elevated |
| >= 85% | CTX_CRITICAL / ALERT_RED (196) | Critical |
| 100% | "Limit reached" text | Rate limited |

### Quota Display Format

| State | Display |
|-------|---------|
| Normal (75%) | `Q: 5h: 75% (resets 2h 0m)` |
| Limit reached | `Q: 5h: Limit reached (resets 15m)` |
| Reset unknown | `Q: 5h: 25%` |
| Reset ≥24h | `Q: 7d: 55% (resets 2d 0h 0m)` |
| No data | (no quota line) |

Config toggles: `show_quota` (master), `show_quota_five_hour`, `show_quota_seven_day`. Quota line is treated as activity-level for width degradation (dropped first).

## Line 4+: Activity

Dynamic lines that appear only when tools, agents, or todos are active. Controlled by `show_tools`, `show_agents`, `show_todo` config flags.

### Completed Tool Counts (L4a)

Dedicated line for accumulated completion counts — stable, grows over the session.

| Component | Data Source | Parsing Method | Color |
|-----------|-------------|----------------|-------|
| Completed tools | SessionState completed counts | Accumulate on `tool_result` events | COMPLETED_CHECK (67) |

**Display format**: `✓ Read ×12 | ✓ Bash ×8 | ✓ Edit ×5`

Capped by `max_completed_tools` config value.

**Noise filtering**: 9 internal Claude Code tools are excluded from tracking: EnterPlanMode, ExitPlanMode, EnterWorktree, ExitWorktree, TaskGet, TaskList, TaskOutput, TaskStop, ToolSearch.

**Hybrid scoring**: Tools ranked by `score = count + recency_bonus` where `recency_bonus = 3.0 × max(0, 1 − age_ms / 120,000)`. Recently used tools (within 2 minutes) get up to 3 bonus points, floating up even with low counts. After decay, pure count dominates. Ties broken alphabetically.

**Multi-line wrapping**: Completed tools span up to `max_completed_lines` rows (default: 1). Overflow folds into a ` +N` tail on the last visible row.

### Running/Recent Tools (L4b)

Dedicated line for running and recently used tools with targets — volatile, changes with each tool use.

| Component | Data Source | Parsing Method | Color |
|-----------|-------------|----------------|-------|
| Running/recent tools | Transcript JSONL (nested content blocks) | `tool_use` events -> upsert with target extraction | ACTIVE_CYAN (117) |

**Display format**: `T:Read: .../main.rs | T:Bash: cargo test`

#### Tool Target Extraction

| Tool Name | Target Field | Example |
|-----------|-------------|---------|
| Read, Write, Edit, NotebookEdit | `input.file_path` | `T:Read: .../main.rs` |
| Bash | `input.command` | `T:Bash: cargo test` |
| Glob, Grep | `input.pattern` | `T:Grep: TODO` |
| WebFetch | `input.url` | `T:WebFetch: https://example...` |
| WebSearch | `input.query` | `T:WebSearch: rust async` |
| Skill | `input.skill` | `T:Skill: commit` |
| AskUserQuestion | `input.questions[0].question` | `T:AskUserQuestion: Which dat...` |
| SendMessage | `input.to` | `T:SendMessage: code-reviewer` |
| LSP | `input.command` | `T:LSP: getDefinition` |
| Other (generic) | `file_path` → `command` → `pattern` | Fallback chain |

Both lines are controlled by a single `show_tools` toggle. Display capped by `max_tool_lines`.

### Agents (L5+)

One line per agent, active first then recent completed, capped by `max_agent_lines`.

| Component | Data Source | Parsing Method | Color |
|-----------|-------------|----------------|-------|
| Active agents | Transcript JSONL (Path 2) | `agent_progress` with `status: "started"` | ACTIVE_PURPLE (183) |
| Completed agents | SessionState completed agents | `agent_progress` with `status: "completed"` | COMPLETED_CHECK (67) |

**Display format**:
- Running: `A:Explore [haiku]: Investigate logic (2m)`
- Completed: `checkmark Explore: Task completed (45s)`

Completed agents are stored in a FIFO buffer (max 10), pruned when exceeded.

### Todos

| Component | Data Source | Parsing Method | Color |
|-----------|-------------|----------------|-------|
| Todo items | Transcript JSONL (TaskCreate/TaskUpdate events) | Todo lifecycle tracking | ACTIVE_TEAL (80) |

**Display variants**:
- In-progress (task API): `TODO:Fixing auth bug (1/3) (5s)` — shows active_form text, progress, elapsed
- Multi in-progress: `TODO:Fixing auth bug (1/3, 3 active) (5s)` — overflow count when >1 active
- Pending only (task API): `TODO:3 tasks (0/3)` — no in-progress items
- All done: `✓ All todos complete (3/3)` — celebration line using COMPLETED_CHECK color
- Legacy (TodoWrite): `TODO:1/3 done, 2 pending`

Multi-line todo display is capped by `max_todo_lines` config value (default: 1).

### Example Output

Tools — completed counts on one line, recent/running on another:

```
✓ Read ×12 | ✓ Bash ×5 | ✓ Edit ×3
T:Read: .../src/main.rs | T:Bash: cargo test
```

Agents — running + completed (one per line):

```
A:Explore [haiku]: Investigating auth logic (2m)
A:Bash: Run test suite [done] (45s)
```

Todo progress (task API — in-progress):

```
TODO:Fixing auth bug (1/3) (5s)
TODO:Adding unit tests (1/3) (12s)
```

Todo progress (all done):

```
✓ All todos complete (3/3)
```

## Cache Strategy

cc-pulseline uses a multi-layer caching strategy to balance freshness with performance:

### Layer 1: In-Memory Session State

`PulseLineRunner` maintains a `HashMap<String, SessionState>` keyed by `session_id|transcript_path|project_path`. State persists across invocations within the same process.

### Layer 2: Env/Git TTL Cache

- **TTL**: 10 seconds (`CACHE_TTL_MS`)
- **Scope**: Per-session env and git snapshots stored in `SessionState`
- **Behavior**: On each invocation, check if cached data is younger than TTL. If yes, reuse; if no, re-collect from filesystem/git

### Layer 3: Transcript Incremental Parsing

- **Seek-based offset**: Only new bytes since last read are parsed
- **Poll throttle**: 250ms minimum between transcript reads (`transcript_poll_throttle_ms`)
- **Event window**: Last 400 events retained (`transcript_window_events`)

### Layer 4: L3 All-or-Nothing Fallback

Line 3 metrics (context, tokens, cost) use a special merge strategy:
- If the current payload has L3 data, use it entirely (current wins)
- If the current payload has no L3 data (all fields NA), fall back to the last cached L3
- This prevents the "NA flicker" when Claude Code briefly omits budget data between updates

### Layer 5: Disk Persistence

- **File**: `{temp_dir}/cc-pulseline-{hash}.json`
- **Hash**: Session key hashed via Rust's `DefaultHasher`
- **Write**: Atomic (write `.tmp` file, then rename)
- **Read**: On first encounter of a session key only
- **Errors**: All load/save errors silently ignored (never crashes the statusline)
- **Schema evolution**: Cache uses `#[serde(default)]`; schema changes (e.g., v1.0.4 `completed_tool_counts` format change) cause old files to silently fail deserialization and regenerate fresh

