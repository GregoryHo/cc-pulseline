# Architecture: Activity Width Budget

- **Platform:** CLI / statusline (cc-pulseline)
- **Primary job:** every activity row knows its width budget and chooses the
  best representation that fits, instead of using ad-hoc magic numbers
  (`truncate_str(30)`, `tools_per_line=6`, `ACTIVITY_TEXT_MAX_CHARS=40`)
  that don't compose.
- **Date:** 2026-04-27
- **Predecessor:** `designs/none-layout-redesign.md` (decided that the
  underlying problem is structural, not surface)

---

## 1. Problems being solved

Today there are **9 disconnected truncation knobs** (4 hard-coded constants
across two files + 5 config keys):

| Knob | Where | Today's value |
|---|---|---|
| `truncate_path(_, 30)` | `transcript.rs:599` | path → `.../leaf` |
| `truncate_str(_, 30)` | `transcript.rs:617` | command, URL, query |
| `truncate_str(_, 20)` | `transcript.rs:617` | pattern, skill, LSP |
| `ACTIVITY_TEXT_MAX_CHARS` | `layout.rs:247` | 40 (agent desc + todo text) |
| `tools_per_line` | config | 6 |
| `max_tool_lines` | config | 2 |
| `max_agent_lines` | config | 2 |
| `max_todo_lines` | config | 2 |
| `max_completed_tools` | config | 4 |

Concrete consequences in real sessions:

1. **Bash target keeps the wrong half.** `truncate_str` is prefix-keep; for
   `Read /path/main.rs` that's right; for
   `sed -i '' 's/^name = …` it strips the regex (the meaningful payload).

2. **`tools_per_line` ignores width.** `tools_per_line=4` on a 220-col
   terminal orphans `✓ Skill ×1` onto its own row even though all 5 fit on
   a single row.

3. **Description sharing one constant.** Agent.description and Todo.text
   both clamp at 40 chars regardless of how much room the row has.

4. **`max_*_lines` are absolute counts.** Whether the terminal is 80 or 240
   wide, the same number of agents/tools display — so wide terminals waste
   space and narrow terminals overflow.

5. **No notion of priority.** Inside a row, `model = [haiku]` and
   `description = "..."` get equal share of the bytes; one is decoration,
   the other is the load-bearing content.

The user's expectation (correctly): activity rendering should adapt to
available width AND know what's important within each cell.

---

## 2. Architecture: Activity Width Budget

Three layers, decoupled.

### 2.1 Cell

A `Cell` is the smallest displayable unit. Every cell declares:

```rust
struct Cell {
    /// Unbreakable prefix — icon, label, structural punctuation. Always rendered.
    head: String,
    /// Variable-width content — fitted to budget by `truncator`.
    body: Option<CellBody>,
    /// Suffix — elapsed, count, model tag. Dropped under priority pressure.
    tail: Vec<TailFragment>,
    /// How to behave when budget < ideal_width.
    priority: CellPriority,
}

enum CellPriority {
    /// Drop entire cell when overflowing (e.g., recent-tool target on a
    /// completely full row).
    Optional,
    /// Show head only when budget < head + min(body) (e.g., agent type).
    Required,
    /// Always rendered, never truncated below `head` (model + branch on L1).
    Anchored,
}

struct CellBody {
    raw: String,
    truncator: TruncationStrategy,
    min_width: usize,    // shortest still-meaningful form (e.g., 8 chars)
    ideal_width: usize,  // full content; cap at this even when budget allows more
}

enum TailFragment {
    /// Always shown if cell is shown (e.g., elapsed for running agents).
    Pinned(String),
    /// Shown if budget permits (e.g., model tag `[haiku]`).
    Slack(String),
}
```

### 2.2 Truncation strategies

Pluggable, content-typed. **Replaces the three current `truncate_*` helpers.**

| Strategy | When | Algorithm |
|---|---|---|
| `KeepHead` | verb-led short labels (skill name, glob pattern) | take first N chars, append `…` |
| `KeepTail` | file paths | `truncate_path` shape: `.../leaf` if leaf fits, else tail-N + leading `…` |
| `KeepMiddle` | both ends meaningful (URL with host + leaf path) | `prefix…suffix` |
| `CommandSmart` | shell commands (Bash/PowerShell) | see §2.3 below |
| `Sentence` | descriptions, prompts, questions | truncate at last word boundary ≤ N, no mid-word cut |

Each strategy is a free function `fn(raw: &str, max_chars: usize) -> String`,
unit-tested in isolation. Strategy selection lives in **one** table per
tool kind in `providers/transcript.rs::extract_target` — replaces the
current per-tool `truncate_str(_, N)` literals with
`(strategy, target_width_kind)` pairs.

### 2.3 `CommandSmart` for shell commands

```
Input:  sed -i '' 's/^name = ".*"$/name = "cards"/' .claude/pulseline.toml
Output: s/^name = ".*"$/name = "cards"/  .claude/pulseline.toml
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        first quoted payload preserved + path arg appended
```

Algorithm:

1. **Strip the verb chain.** Pop the first bare token; while the next token
   starts with `-` or `--`, pop it. Skip anything inside quotes.
2. **Find the first payload.** The first surviving token from one of:
   - Quoted string content (drop the quotes)
   - Path-looking arg (`./x`, `/x`, `~/x`, contains `/`, has `.ext`)
   - Plain bare token
3. **Append further file-path args** if budget remains.
4. **Truncate the result** with `Sentence` (word-boundary).

Edge cases:

| Command | Result (~40 char budget) |
|---|---|
| `cargo test --all-features` | `cargo test --all-features` (fits, no strip) |
| `git commit -m "feat: add foo"` | `feat: add foo` (quoted payload) |
| `find . -name "*.tmp" -delete` | `*.tmp` |
| `sed 's/x/y/' file.txt` | `s/x/y/  file.txt` |
| `node scripts/build.js --watch` | `scripts/build.js` |
| `claude code --resume <hash>` | `claude code <hash>` (verb kept when no quoted payload) |

When everything is too long, fall back to `KeepHead`.

### 2.4 Row Allocator

A row receives:
- `available_width: usize` (already cc_margin-adjusted)
- `cells: Vec<Cell>`
- `inter_cell_separator_width: usize` (e.g. 3 for `" | "`)

Allocation pass:

```
1. Sum `head` widths + separator widths. If > available, drop Optional
   cells from the right until it fits, or return empty.
2. Distribute remaining slack across cells with bodies, weighted by
   priority. Each body gets at most its `ideal_width`; never less than
   `min_width` (if not satisfiable, escalate to step 3).
3. If still over: drop Slack tail fragments from the lowest-priority
   cells first. If still over: drop Optional cells. If still over:
   collapse Required-cell bodies to `min_width` then accept the row may
   visibly clip.
```

Implementation: ~80 LOC in a new `src/render/activity/budget.rs`.

### 2.5 Multi-row planning

Today the rule "tools_per_line = N" lives in config. After this redesign:

- `frame.completed_tools` arrives as `Vec<CompletedToolCount>`.
- The planner asks: given `available_width`, how many `✓ Name ×N` cells
  fit on one row? Use `Cell::ideal_width` summed with separators.
- If all fit: one row. If not: chunk by what fits, not by a fixed N.
- `tools_per_line` config becomes a **cap** (don't put more than N on one
  row even if they fit) — defaults to `usize::MAX` (unbounded).

Same idea for `max_tool_lines` (recent tools): split into rows when one
row's budget can't hold them all, never when it can.

---

## 3. Agent batch detection (revised after transcript audit)

**The user's correct objection**: agents are tracked sequentially. Folding
3 sequentially-spawned agents under one heading misrepresents the timeline
— it would look like a batch fan-out when it's actually history.

**Original proposal (DISCARDED)**: a `BATCH_WINDOW_MS` time-window
heuristic. Empirically wrong: in this session's transcript, the 3 agents
spawned by `/simplify` have JSONL `timestamp` values **20 seconds apart**
even though they belong to the same Anthropic API message (single assistant
turn). The JSONL `timestamp` is "wall-clock when CC wrote that block to
disk", not "when the assistant decided to spawn". Any window-based
heuristic would miss real batches.

**Real signal: `message.id` equality.** All tool_use blocks emitted by a
single assistant turn share the same Anthropic API `message.id`. The
transcript audit confirmed this for two batches in the current session
(`msg_01QbqAgkFJDBLoGgjEvFBPYW` for `/simplify`,
`msg_013BWKhgtB2rPvsKtVQMUTBC` for the strata reviews). Cross-checked
against `claude-code-guide`: the JSONL schema is internal and undocumented,
but `message.id` carries the Anthropic API contract that one ID = one
assistant turn — a stable signal as long as CC keeps surfacing it.

**Schema additions**:

```rust
struct AgentSummary {
    id: String,
    description: String,
    agent_type: Option<String>,
    started_at: Option<u64>,
    model: Option<String>,
    completed_at: Option<u64>,
    /// Parent assistant message ID (Anthropic API). Agents that share this
    /// ID were spawned in the same assistant turn — i.e. a parallel batch.
    /// Optional because: (a) old cache files may not have it, (b) CC's
    /// JSONL field is internal/undocumented and may move.
    #[serde(default)]
    message_id: Option<String>,
}
```

**Detection**:

```rust
fn is_batch(group: &[&AgentSummary]) -> bool {
    // Same parent message AND same agent_type → batch fan-out.
    if group.len() < 2 { return false; }
    let mid = match &group[0].message_id {
        Some(m) => m,
        None => return false,   // no message_id → fall through to sequential
    };
    group.iter().all(|a| a.message_id.as_deref() == Some(mid))
        && group.iter().all(|a| a.agent_type == group[0].agent_type)
}
```

**Ingestion plumbing** (`providers/transcript.rs::dispatch_event_path1`):
when an Agent tool_use block is observed inside a message envelope,
capture the enclosing `event.message.id` and stash it on the
`PendingTask` / `AgentSummary` record.

**Fallback behavior**: if `message_id` is missing on every agent (older
CC, cache rehydrated from pre-upgrade binary), `is_batch` returns false
and every agent renders on its own row. Same as today's behavior — safe
degradation.

```rust
// (Old window heuristic removed — kept here as a stub so diff readers
//  see the deletion intent.)
fn _legacy_window_heuristic_DELETED(group: &[&AgentSummary]) -> bool {
    if group.len() < 2 { return false; }
    let same_type = group.iter().all(|a| a.agent_type == group[0].agent_type);
    if !same_type { return false; }
    let starts: Vec<u64> = group.iter().filter_map(|a| a.started_at).collect();
    if starts.len() != group.len() { return false; }
    let min = starts.iter().min().unwrap();
    let max = starts.iter().max().unwrap();
    (max - min) <= BATCH_WINDOW_MS
}
```

**Display split** (4 cases, two fold flavors):

| Group shape | Detection | Display |
|---|---|---|
| Single agent | n=1 OR not in any same-`message_id` cluster | `🤖 type [model]: description (elapsed)` (today's shape) |
| **Homogeneous batch** | n≥2, same `message_id`, **same `agent_type`** | `🤖 type ×N parallel [model] (avg 1m): first description + N-1 more` |
| **Heterogeneous parallel group** | n≥2, same `message_id`, **mixed `agent_type`** | `‖ parallel ×N (avg 1m): Explore·invest auth │ general-purpose·code reuse │ general-purpose·code quality │ code-reviewer·final` |
| **Sequential** | different `message_id` (or `message_id` is `None`) | each agent gets its own row — exactly today |

**Two fold flavors, two glyphs**:
- 🤖 (`ICON_AGENT`) — homogeneous batch keeps the same single-agent prefix; the `×N parallel` makes the count + relationship explicit while preserving "this is a typed agent group" reading.
- ‖ (U+2016 `DOUBLE VERTICAL LINE`, math notation for "parallel") — reserved for heterogeneous groups. Distinct from 🤖 so the eye reads "different shape, different rules" at a glance. ASCII fallback: `||`.

**Heterogeneous group cell internals**:
- Header: `‖ ×N parallel (avg <elapsed>):`
- Body: per-agent `{type}: {first-line of description}` joined by ` + ` (U+002B plus, single space each side). Plus reads as semantic "and" (`A + B + C` = "these run together"), is shape-distinct from the row-level ` | ` separator, and the space-padding makes it visually unambiguous against in-text `+` (descriptions like `C++` or `1+2` have no surrounding spaces). Considered and rejected: `•` (heavier visual weight), `/` (collides with paths), `▸` (implies sequence).
- Within each sub-item the `:` matches the single-agent format (`type: description`), so users learn one shape.
- Tail: `(avg elapsed)` already in the header (no trailing tail needed for groups).
- Truncator priority: when budget tight, shorten per-agent descriptions first via `Sentence`. **No type abbreviation** — types stay as-is and descriptions get clipped further; in the worst case the group still reads as "we have 4 parallel agents of these types, descriptions cut". Abbrevation tables (`gp`, `rv`, `ex`) were considered and rejected: they save 6-10 chars at the cost of a learning curve and ambiguity for unknown subagent types.

**Why the heterogeneous group always takes its own row** (and other cells don't share it): a parallel-group cell with 4 sub-items needs ~120-200 chars to be useful; mixing it on a row with completed-tool counts or todos forces aggressive truncation that loses information. The row planner reserves a dedicated row for any group cell.

The `parallel` keyword in the batch form is the **explicit signal** that
these were spawned together. Prevents the "did you fold my history?"
confusion. Also: each row in a batch can still expand to a separate row
when `max_agent_lines` budget permits — folding is a width-pressure
response, not a default behavior.

**Sequential overflow** (>`max_agent_lines` agents, all distinct
`message_id`s): show the first `max_agent_lines - 1` agents in full, then
emit one summary row `… +K more agents` where `K` = remainder. This
preserves the "sequential = full per-agent context" rule while making the
truncation visible. We **don't** fold them into a single sequential
summary — that would lie about the relationship.

```
🤖 Explore: investigate auth (1m 30s)
🤖 general-purpose: code reuse review (45s)
… + 3 more agents
```

**Description in batch form**: `first description…` shows just the
**first agent's** description (truncated), `+N-1 more` makes count
obvious. If the user wants the others, expand the budget by raising
`max_agent_lines` or going to a layout with more vertical room.

**Description handling generally**: same `Sentence` truncator with
budget-derived `max_chars` instead of the fixed 40. So a wider terminal
shows more of the description — the way the user expected.

---

## 4. Concrete row definitions after the rewrite

Each row is a `Vec<Cell>` with budgets. Showing the cell descriptors:

### 4.1 Recent tool row (was L4b)

```rust
fn build_recent_tool_row(tool: &ToolSummary) -> Vec<Cell> {
    vec![Cell {
        head: glyph(ICON_TOOL, "T:") + tool.name + ":",
        body: tool.target.map(|t| CellBody {
            raw: t,
            truncator: target_strategy_for(&tool.name),  // table-driven
            min_width: 8,
            ideal_width: 60,
        }),
        tail: vec![],
        priority: CellPriority::Required,
    }]
}
```

The `target_strategy_for` table:

| Tool name | Strategy |
|---|---|
| Read / Write / Edit / NotebookEdit | `KeepTail` |
| **Bash / PowerShell** | **`CommandSmart`** |
| Glob / Grep | `KeepHead` |
| WebFetch | `KeepMiddle` (host + leaf) |
| WebSearch / Skill / Advisor / MCPSearch | `Sentence` |
| AskUserQuestion | `Sentence` |
| SendMessage | `KeepHead` |
| LSP / Monitor | `KeepHead` |
| _unknown_ | `KeepHead` |

### 4.2 Completed tool row (was L4a)

```rust
fn build_completed_tool_cell(c: &CompletedToolCount) -> Cell {
    Cell {
        head: format!("✓ {}", c.name),
        body: None,
        tail: vec![TailFragment::Pinned(format!(" ×{}", c.count))],
        priority: CellPriority::Optional,  // can drop low-rank ones
    }
}
```

Allocator decides how many fit per row. No more `tools_per_line` orphans.

### 4.3 Agent row — single

```rust
Cell {
    head: glyph(ICON_AGENT, "A:") + agent_type + ":",
    body: Some(CellBody {
        raw: agent.description.lines().next().unwrap_or(""),
        truncator: Sentence,
        min_width: 12,
        ideal_width: 80,
    }),
    tail: vec![
        TailFragment::Slack(format!("[{model}]")),  // drop first under pressure
        TailFragment::Pinned(format!("({elapsed})")),
    ],
    priority: CellPriority::Required,
}
```

### 4.4 Agent row — homogeneous batch (same `message_id` + same type)

```rust
Cell {
    head: glyph(ICON_AGENT, "A:") + agent_type + format!(" ×{N}"),
    body: Some(CellBody {
        raw: format!("{first_desc} + {N-1} more"),
        truncator: Sentence,
        min_width: 16,
        ideal_width: 80,
    }),
    tail: vec![
        TailFragment::Pinned("parallel".into()),
        TailFragment::Slack(format!("[{model}]")),
        TailFragment::Pinned(format!("(avg {elapsed})")),
    ],
    priority: CellPriority::Required,
}
```

### 4.4b Agent row — heterogeneous parallel group (same `message_id`, mixed types)

```rust
Cell {
    head: format!("{} ×{N} parallel (avg {elapsed}):", glyph(GROUP_PARALLEL, "||")),
    body: Some(CellBody {
        raw: agents.iter().map(|a| {
            let t = a.agent_type.as_deref().unwrap_or("agent");
            let d = first_line_of(&a.description);
            format!("{t}: {d}")
        }).collect::<Vec<_>>().join(" + "),
        truncator: GroupSentence,  // truncates each sub-item independently
        min_width: 24,
        ideal_width: 240,  // greedy — group rows usually own the whole row
    }),
    tail: vec![],  // header carries the elapsed average; no per-agent model tag here
    priority: CellPriority::Required,
}

const GROUP_PARALLEL: &str = "\u{2016}";  // ‖
const GROUP_SEPARATOR: &str = " + ";       // space-padded plus
```

`GroupSentence` is a new variant of the existing truncators that knows
about ` + ` as a sub-item boundary: it shortens the longest sub-item's
description first, then the next-longest, etc., so under width pressure
all sub-items shrink proportionally instead of trailing ones disappearing.

**Type names are never abbreviated** (decision per user). When the budget
is too tight to show every type:description pair, descriptions get
shortened toward `min_width` first; in the absolute worst case the
sub-item collapses to `<type>: …` showing only the type. We never drop
agents from the group — the count `×N` in the header is always honest.

### 4.5 Todo row — in progress

```rust
Cell {
    head: glyph(ICON_TODO, "TODO:"),
    body: Some(CellBody {
        raw: item.text,
        truncator: Sentence,
        min_width: 12,
        ideal_width: 80,
    }),
    tail: vec![
        TailFragment::Pinned(format!("({completed}/{total})")),
        TailFragment::Slack(format!("({elapsed})")),
    ],
    priority: CellPriority::Required,
}
```

---

## 5. Migration plan

Single PR, ~600 LOC.

1. **New module `src/render/activity/`**:
   - `mod.rs` — public API: `build_activity_rows(frame, config) -> Vec<String>`
   - `cell.rs` — `Cell`, `CellBody`, `TailFragment`, `CellPriority`
   - `truncate.rs` — 5 strategies + tests
   - `budget.rs` — row allocator
   - `agent_groups.rs` — batch detection

2. **Replace** `format_completed_tool_lines` / `format_recent_tool_line` /
   `format_agent_line` / `format_todo_lines` in `layout.rs` with calls
   to `activity::build_activity_rows`. v1 layouts (`none/zones/grid/cards/
   sections`) all consume the same output.

3. **Update** `providers/transcript.rs::extract_target` — replace inline
   `truncate_str(_, N)` calls with `(strategy, content_kind)` table
   lookups. The transcript layer now stores the **untruncated** target;
   render layer truncates against budget. **Important**: this means
   `ToolSummary.target` is no longer pre-truncated — the target storage
   gets the full string (with sanitize_single_line still applied), and
   render does the cell-time truncation.

4. **Deprecate config knobs**:
   - `tools_per_line` — keep as upper cap; default `usize::MAX`
   - `max_tool_lines` — keep as upper cap; default `usize::MAX`
   - `max_agent_lines` / `max_todo_lines` — keep as upper cap
   - `max_completed_tools` — keep as upper cap
   
   None removed; all become "ceiling" rather than "exact count".

5. **Tests** — new file `tests/activity_budget.rs`:
   - `CommandSmart` table-driven (the 6 examples in §2.3)
   - Row allocator: 4-5 cases (everything fits / drop Optional / drop
     Slack tails / drop body to min_width / overflow)
   - Batch detection by `message_id`: same-id-same-type → batch,
     different-id-same-type → sequential, same-id-different-type →
     sequential (each type gets its own row), all-`None` → sequential
   - End-to-end activity rendering at widths 80 / 120 / 160 / 200

6. **Performance**: budget allocation is O(cells × passes), with cells ≤ 30
   in the worst case (10 completed + 5 running + 5 agents + 2 todo +
   chrome). Sub-microsecond. Safe for the 50ms budget.

7. **CHANGELOG entry** under "Changed" — flag that several config keys
   become caps instead of exact counts.

---

## 6. Out of scope (deliberately)

- Touching v2 layouts (cockpit/console/flightstrip). Their widget rows
  use `widgets::tape::render` etc. — different allocation model. If we
  want shared budget logic across v1 + v2 it's a follow-up.
- WebFetch URL truncation tuning (`KeepMiddle` is sketched but not deeply
  tuned for host-vs-path balance).
- Splitting completed-tool sort/score from display (current `scored_*`
  logic stays as input; budget logic is downstream).

---

## 7. Open questions (just 1)

~~`BATCH_WINDOW_MS`~~ — **resolved** (see §3 revision). Replaced by
`message_id` equality after transcript audit confirmed JSONL `timestamp`
spans 20+ seconds within a single assistant turn. No tunable constant
needed.

1. **Batch label wording** — `parallel` in the row is the most explicit.
   Alternatives: `(batch)`, `(fan-out)`, `(group)`. Or use a glyph (`⫶`,
   `⋮`, `⫻`) instead of a word. Glyph saves chars but adds learning cost.

## Next step

Approve this architecture; I implement as one PR. Estimated 2-3 hours
including tests. Touches `providers/transcript.rs`, all v1 layouts via
`render/layout.rs`, and adds the new `render/activity/` subtree.
