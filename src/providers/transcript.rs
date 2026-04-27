use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};

// ── ISO 8601 timestamp parsing (no chrono dependency) ────────────────

/// Parse an ISO 8601 timestamp string to Unix epoch milliseconds.
/// Handles formats like "2026-01-18T10:58:40.895Z" and "2026-01-18T10:58:40Z".
fn parse_iso_timestamp(s: &str) -> Option<u64> {
    let b = s.trim().as_bytes();
    if b.len() < 19 {
        return None;
    }

    let year: i64 = std::str::from_utf8(&b[0..4]).ok()?.parse().ok()?;
    if b[4] != b'-' {
        return None;
    }
    let month: u32 = std::str::from_utf8(&b[5..7]).ok()?.parse().ok()?;
    if b[7] != b'-' {
        return None;
    }
    let day: u32 = std::str::from_utf8(&b[8..10]).ok()?.parse().ok()?;
    if b[10] != b'T' && b[10] != b' ' {
        return None;
    }
    let hour: u64 = std::str::from_utf8(&b[11..13]).ok()?.parse().ok()?;
    if b[13] != b':' {
        return None;
    }
    let minute: u64 = std::str::from_utf8(&b[14..16]).ok()?.parse().ok()?;
    if b[16] != b':' {
        return None;
    }
    let second: u64 = std::str::from_utf8(&b[17..19]).ok()?.parse().ok()?;

    // Optional fractional seconds (.mmm)
    let millis: u64 = if b.len() > 19 && b[19] == b'.' {
        let frac_start = 20;
        let frac_end = b[frac_start..]
            .iter()
            .position(|c| !c.is_ascii_digit())
            .map(|i| frac_start + i)
            .unwrap_or(b.len());
        let frac = std::str::from_utf8(&b[frac_start..frac_end]).ok()?;
        match frac.len() {
            0 => 0,
            1 => frac.parse::<u64>().ok()? * 100,
            2 => frac.parse::<u64>().ok()? * 10,
            _ => std::str::from_utf8(&b[frac_start..frac_start + 3])
                .ok()?
                .parse::<u64>()
                .ok()?,
        }
    } else {
        0
    };

    // Days from Unix epoch using Howard Hinnant's algorithm
    let days = days_from_civil(year, month, day)?;
    let secs = days as u64 * 86400 + hour * 3600 + minute * 60 + second;
    Some(secs * 1000 + millis)
}

/// Convert a civil date to days since Unix epoch (1970-01-01).
fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Shift year so March is month 0 (simplifies leap year handling)
    let (y, m) = if month <= 2 {
        (year - 1, (month + 9) as i64)
    } else {
        (year, (month - 3) as i64)
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * m as u64 + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe as i64 - 719468)
}

use serde_json::Value;

use crate::{
    config::RenderConfig,
    state::SessionState,
    types::{AgentSummary, CompletedToolCount, StdinPayload, TodoSummary, ToolSummary},
};

#[derive(Debug, Clone, Default)]
pub struct TranscriptSnapshot {
    pub tools: Vec<ToolSummary>,
    pub completed_counts: Vec<CompletedToolCount>,
    pub agents: Vec<AgentSummary>,
    pub todo: Option<TodoSummary>,
}

pub trait TranscriptCollector {
    fn collect_transcript(
        &self,
        payload: &StdinPayload,
        state: &mut SessionState,
        config: &RenderConfig,
    ) -> TranscriptSnapshot;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileTranscriptCollector;

impl TranscriptCollector for FileTranscriptCollector {
    fn collect_transcript(
        &self,
        payload: &StdinPayload,
        state: &mut SessionState,
        config: &RenderConfig,
    ) -> TranscriptSnapshot {
        let Some(transcript_path) = payload.transcript_path.as_deref() else {
            return snapshot_from_state(state, config);
        };

        state.reset_transcript_if_path_changed(transcript_path);

        let path = Path::new(transcript_path);
        if !path.exists() {
            return snapshot_from_state(state, config);
        }

        if should_throttle(
            state.last_transcript_poll,
            config.transcript_poll_throttle_ms,
        ) {
            return snapshot_from_state(state, config);
        }

        let file_len = path
            .metadata()
            .ok()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if file_len < state.last_transcript_offset {
            state.last_transcript_offset = 0;
            state.active_tools.clear();
            state.recent_tools.clear();
            state.active_agents.clear();
            state.completed_tool_counts.clear();
            state.todo = None;
            state.last_output_tokens = None;
            state.last_output_token_time_ms = None;
            state.output_speed_toks_per_sec = None;
        }

        if let Ok(new_lines) = read_new_lines(path, state.last_transcript_offset) {
            let mut events: Vec<Value> = new_lines
                .iter()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .collect();

            if config.transcript_window_events > 0 && events.len() > config.transcript_window_events
            {
                let keep_from = events.len() - config.transcript_window_events;
                events.drain(0..keep_from);
            }

            for event in events {
                apply_transcript_event(state, &event);
            }
        }

        state.last_transcript_offset = file_len;
        state.last_transcript_poll = Some(Instant::now());

        snapshot_from_state(state, config)
    }
}

#[derive(Debug, Default)]
pub struct StubTranscriptCollector;

impl TranscriptCollector for StubTranscriptCollector {
    fn collect_transcript(
        &self,
        _payload: &StdinPayload,
        _state: &mut SessionState,
        _config: &RenderConfig,
    ) -> TranscriptSnapshot {
        TranscriptSnapshot::default()
    }
}

fn should_throttle(last_poll: Option<Instant>, throttle_ms: u64) -> bool {
    if throttle_ms == 0 {
        return false;
    }

    let Some(last_poll) = last_poll else {
        return false;
    };

    last_poll.elapsed() < Duration::from_millis(throttle_ms)
}

fn read_new_lines(path: &Path, start_offset: u64) -> Result<Vec<String>, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open transcript {}: {error}", path.display()))?;

    file.seek(SeekFrom::Start(start_offset))
        .map_err(|error| format!("failed to seek transcript {}: {error}", path.display()))?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read transcript {}: {error}", path.display()))?;

    let text = String::from_utf8_lossy(&bytes);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

// ── Three-path event dispatcher ──────────────────────────────────────

fn apply_transcript_event(state: &mut SessionState, raw_event: &Value) {
    // Extract event timestamp (epoch millis) from the JSONL line's top-level "timestamp" field
    let event_ts = raw_event
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_iso_timestamp);

    // Anthropic API `message.id` for batch detection — only present on
    // assistant-message envelopes (Path 1). Drives `AgentSummary.message_id`
    // so multiple Agent tool_uses from one assistant turn group as a batch.
    let message_id = raw_event
        .get("message")
        .and_then(|m| m.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    // Path 1: Nested content[] blocks (real Claude Code transcript format)
    // Messages have: { "message": { "role": "assistant", "content": [{...}] } }
    // Or:           { "role": "user", "content": [{...}] }
    if let Some(content_blocks) = extract_content_blocks(raw_event) {
        for block in content_blocks {
            apply_content_block(state, block, event_ts, message_id.clone());
        }
        // Defense-in-depth: also check toolUseResult for agent completion signal
        check_tool_use_result_completion(state, raw_event);
        return;
    }

    // Path 2: Progress events (agent_progress)
    // { "type": "progress", "data": { "type": "agent_progress", ... } }
    if let Some(event_type) = raw_event.get("type").and_then(Value::as_str) {
        if event_type == "progress" {
            if let Some(data) = raw_event.get("data") {
                if data.get("type").and_then(Value::as_str) == Some("agent_progress") {
                    handle_agent_progress(state, data, event_ts);
                    return;
                }
            }
        }
    }

    // Path 3: Flat format fallback (existing test fixtures / simple formats)
    apply_flat_event(state, raw_event, event_ts);
}

/// Extract content[] blocks from nested transcript events.
/// Checks both `raw_event.message.content` and `raw_event.content`.
fn extract_content_blocks(raw_event: &Value) -> Option<Vec<&Value>> {
    // Check message.content[] first (assistant messages)
    let content = raw_event
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(Value::as_array)
        // Then check top-level content[] (user messages with tool_result)
        .or_else(|| raw_event.get("content").and_then(Value::as_array));

    let blocks = content?;

    // Only use this path if content has typed blocks (not plain text strings)
    let has_typed_blocks = blocks
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str).is_some());

    if has_typed_blocks {
        Some(blocks.iter().collect())
    } else {
        None
    }
}

/// Process a single content block from a message's content[] array.
/// Internal Claude Code tools excluded from tool tracking.
/// Referenced by both Path 1 (nested content) and Path 3 (flat fallback).
const NOISE_TOOLS: &[&str] = &[
    "EnterPlanMode",
    "ExitPlanMode",
    "EnterWorktree",
    "ExitWorktree",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "ToolSearch",
];

fn apply_content_block(
    state: &mut SessionState,
    block: &Value,
    event_ts: Option<u64>,
    message_id: Option<String>,
) {
    let block_type = match block.get("type").and_then(Value::as_str) {
        Some(t) => t,
        None => return,
    };

    match block_type {
        "tool_use" => {
            let name = block
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();

            let id = block
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("unknown-id")
                .to_string();

            // Agent tool → push to pending queue for agent linking
            if name == "Agent" {
                let input = block.get("input");
                let description = input
                    .and_then(|i| {
                        i.get("description")
                            .or_else(|| i.get("prompt"))
                            .and_then(Value::as_str)
                    })
                    .unwrap_or("Agent")
                    .to_string();
                let agent_type = input
                    .and_then(|i| i.get("subagent_type").and_then(Value::as_str))
                    .map(ToString::to_string);
                let model = input
                    .and_then(|i| i.get("model").and_then(Value::as_str))
                    .map(ToString::to_string);
                state.push_pending_task(id, description, agent_type, model, event_ts, message_id);
                return;
            }

            // TaskCreate → individual task item tracking
            if name == "TaskCreate" {
                dispatch_task_create(state, block, None);
                return;
            }

            // TaskUpdate → update individual task or old bulk format
            if name == "TaskUpdate" {
                dispatch_task_update(state, block, None);
                return;
            }

            // TodoWrite → old format with todos[] array
            if name == "TodoWrite" {
                dispatch_todo_write(state, block, None);
                return;
            }

            if NOISE_TOOLS.contains(&name.as_str()) {
                return;
            }

            // Extract target from input
            let target = extract_target(&name, block);
            state.upsert_tool(id, name, target);
        }
        "tool_result" => {
            if let Some(id) = block.get("tool_use_id").and_then(Value::as_str) {
                complete_tool_result(state, id, event_ts);
            }

            // Check for todo data in result
            if let Some(todo) = extract_todo_summary(block) {
                state.set_todo(Some(todo));
            }
        }
        _ => {}
    }
}

/// Handle agent_progress events from the progress stream.
fn handle_agent_progress(state: &mut SessionState, data: &Value, event_ts: Option<u64>) {
    let agent_id = data
        .get("agentId")
        .or_else(|| data.get("agent_id"))
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .to_string();

    let status = data
        .get("status")
        .or_else(|| data.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("running");

    if is_terminal_status(status) {
        state.remove_agent(&agent_id);
        return;
    }

    // Check if this is a new agent that should link to a pending Agent tool_use
    let is_new = !state.active_agents.iter().any(|a| a.id == agent_id);
    if is_new {
        if let Some(pending) = state.link_agent_to_pending_task(&agent_id) {
            // Use the Agent tool's description and type instead of agent_progress prompt
            state.upsert_agent(
                agent_id,
                pending.description,
                pending.agent_type,
                pending.event_ts,
                pending.model,
                pending.message_id,
            );
            return;
        }
    }

    // For already-linked agents, skip description overwrite from agent_progress prompt
    if state.is_task_linked_agent(&agent_id) {
        return;
    }

    // Standalone agent_progress (no Agent tool_use): use prompt as description
    let description = data
        .get("description")
        .or_else(|| data.get("prompt"))
        .or_else(|| data.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("Agent")
        .to_string();

    let agent_type = data
        .get("agentType")
        .or_else(|| data.get("subagent_type"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let model = data
        .get("model")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    // Standalone agent_progress carries no Anthropic message envelope;
    // batch detection treats these as `Single`.
    state.upsert_agent(agent_id, description, agent_type, event_ts, model, None);
}

// ── Shared task/todo dispatch helpers ─────────────────────────────────

/// Handle a TaskCreate event by extracting the subject and activeForm from multiple possible locations.
fn dispatch_task_create(state: &mut SessionState, event: &Value, fallback: Option<&Value>) {
    let subject = find_string(event, &["subject"])
        .or_else(|| find_nested_string(event, &[&["input", "subject"]]))
        .or_else(|| fallback.and_then(|v| find_string(v, &["subject"])));
    let active_form = find_string(event, &["activeForm"])
        .or_else(|| find_nested_string(event, &[&["input", "activeForm"]]))
        .or_else(|| fallback.and_then(|v| find_string(v, &["activeForm"])));
    if let Some(subject) = subject {
        state.create_task_item(subject, active_form);
    }
}

/// Handle a TaskUpdate event. Tries new format (taskId + status) first, then falls back
/// to the old todos[] array format.
fn dispatch_task_update(state: &mut SessionState, event: &Value, fallback: Option<&Value>) {
    let task_id = find_string(event, &["taskId"])
        .or_else(|| find_nested_string(event, &[&["input", "taskId"]]))
        .or_else(|| fallback.and_then(|v| find_string(v, &["taskId"])));
    if let Some(task_id) = task_id {
        let status = find_string(event, &["status"])
            .or_else(|| find_nested_string(event, &[&["input", "status"]]))
            .or_else(|| fallback.and_then(|v| find_string(v, &["status"])))
            .unwrap_or_else(|| "pending".to_string());
        state.update_task_item(&task_id, &status);
        return;
    }
    // Fallback: old format with todos[] array
    let todo = extract_todo_summary(event).or_else(|| fallback.and_then(extract_todo_summary));
    state.set_todo(todo);
}

/// Handle a TodoWrite event by extracting the todo summary.
fn dispatch_todo_write(state: &mut SessionState, event: &Value, fallback: Option<&Value>) {
    let todo = extract_todo_summary(event).or_else(|| fallback.and_then(extract_todo_summary));
    state.set_todo(todo);
}

/// Complete a tool_result by resolving agent links: linked agent, pending task, or plain removal.
fn complete_tool_result(state: &mut SessionState, tool_use_id: &str, event_ts: Option<u64>) {
    state.remove_tool(tool_use_id, event_ts);
    if let Some(linked_agent) = state.resolve_task_agent(tool_use_id) {
        state.remove_agent(&linked_agent);
    } else if let Some(pending) = state.drain_pending_task(tool_use_id) {
        state.upsert_agent(
            tool_use_id.to_string(),
            pending.description,
            pending.agent_type,
            pending.event_ts,
            pending.model,
            pending.message_id,
        );
        state.remove_agent(tool_use_id);
    } else {
        state.remove_agent(tool_use_id);
    }
}

// ── Target extraction (Stage 4) ──────────────────────────────────────

/// Extract a human-readable target from a tool_use block's input field.
///
/// Returns the **raw, single-line-sanitized** payload — truncation happens
/// later in `render::activity::builder` per tool kind (each kind picks an
/// appropriate `TruncationStrategy` + ideal width via `target_strategy_for`).
/// This separation lets the renderer make width-aware decisions; previously
/// every target was pre-truncated to a fixed magic constant here.
fn extract_target(name: &str, block: &Value) -> Option<String> {
    let input = block.get("input")?;

    let raw = match name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => {
            input.get("file_path").and_then(Value::as_str)
        }
        // PowerShell (CC 2.1.84 Windows / 2.1.111 Linux & Mac opt-in) is a Bash analog.
        "Bash" | "PowerShell" => input.get("command").and_then(Value::as_str),
        // Background script monitor (CC 2.1.98+).
        "Monitor" => input
            .get("script_id")
            .and_then(Value::as_str)
            .or_else(|| input.get("pattern").and_then(Value::as_str)),
        // Push notifications (CC 2.1.110 / 2.1.113+).
        "PushNotification" => input.get("title").and_then(Value::as_str),
        // Experimental advisor tool (CC 2.1.117+).
        "Advisor" => input.get("query").and_then(Value::as_str),
        // MCP discovery (CC 2.1.79+). ToolSearch is in NOISE_TOOLS and never reaches here.
        "MCPSearch" => input.get("query").and_then(Value::as_str),
        "Glob" | "Grep" => input.get("pattern").and_then(Value::as_str),
        "WebFetch" => input.get("url").and_then(Value::as_str),
        "WebSearch" => input.get("query").and_then(Value::as_str),
        "Skill" => input.get("skill").and_then(Value::as_str),
        "AskUserQuestion" => input
            .get("questions")
            .and_then(Value::as_array)
            .and_then(|qs| qs.first())
            .and_then(|q| q.get("question").and_then(Value::as_str)),
        "SendMessage" => input.get("to").and_then(Value::as_str),
        "LSP" => input.get("command").and_then(Value::as_str),
        "Agent" => None, // Agent → subagent, not tool
        _ => {
            // Generic fallback: file_path → command → pattern (whichever exists first)
            input
                .get("file_path")
                .and_then(Value::as_str)
                .or_else(|| input.get("command").and_then(Value::as_str))
                .or_else(|| input.get("pattern").and_then(Value::as_str))
        }
    }?;

    Some(crate::render::fmt::sanitize_single_line(raw).into_owned())
}

// ── Flat format fallback (Path 3) ────────────────────────────────────

fn apply_flat_event(state: &mut SessionState, raw_event: &Value, event_ts: Option<u64>) {
    let event = if let Some(message) = raw_event.get("message").filter(|value| value.is_object()) {
        message
    } else {
        raw_event
    };

    let event_type = find_string(event, &["type", "event", "event_type"])
        .or_else(|| find_string(raw_event, &["type", "event", "event_type"]));

    match event_type.as_deref() {
        Some("tool_use") => handle_flat_tool_use(state, event, raw_event, event_ts),
        Some("tool_result") => handle_flat_tool_result(state, event, raw_event, event_ts),
        Some("Agent") => handle_task_event(state, event, event_ts),
        Some("TaskCreate") => {
            dispatch_task_create(state, event, Some(raw_event));
        }
        Some("TaskUpdate") => {
            dispatch_task_update(state, event, Some(raw_event));
        }
        Some("TodoWrite") => {
            dispatch_todo_write(state, event, Some(raw_event));
        }
        _ => handle_event_by_name(state, event, raw_event, event_ts),
    }
}

fn handle_flat_tool_use(
    state: &mut SessionState,
    event: &Value,
    raw_event: &Value,
    event_ts: Option<u64>,
) {
    let name = find_string(event, &["name", "tool_name", "tool"])
        .or_else(|| find_string(raw_event, &["name", "tool_name", "tool"]))
        .unwrap_or_else(|| "unknown".to_string());

    match name.as_str() {
        "Agent" => {
            handle_task_from_tool_use(state, event, raw_event, event_ts);
        }
        "TaskCreate" => {
            dispatch_task_create(state, event, Some(raw_event));
        }
        "TaskUpdate" => {
            dispatch_task_update(state, event, Some(raw_event));
        }
        "TodoWrite" => {
            dispatch_todo_write(state, event, Some(raw_event));
        }
        _ => {
            if NOISE_TOOLS.contains(&name.as_str()) {
                return;
            }
            let id = find_string(event, &["id", "tool_use_id", "tool_call_id"])
                .or_else(|| find_string(raw_event, &["id", "tool_use_id", "tool_call_id"]))
                .unwrap_or_else(|| format!("{name}-active"));
            state.upsert_tool(id, name, None);
        }
    }
}

fn handle_flat_tool_result(
    state: &mut SessionState,
    event: &Value,
    raw_event: &Value,
    event_ts: Option<u64>,
) {
    if let Some(id) = find_string(event, &["tool_use_id", "id", "tool_call_id"])
        .or_else(|| find_string(raw_event, &["tool_use_id", "id", "tool_call_id"]))
    {
        complete_tool_result(state, &id, event_ts);
    }

    if let Some(todo) = extract_todo_summary(event).or_else(|| extract_todo_summary(raw_event)) {
        state.set_todo(Some(todo));
    }
}

fn handle_task_event(state: &mut SessionState, event: &Value, event_ts: Option<u64>) {
    let id = find_string(event, &["task_id", "id", "name"]).unwrap_or_else(|| "task".to_string());
    let summary =
        find_string(event, &["name", "description", "prompt"]).unwrap_or_else(|| id.clone());
    let status = find_string(event, &["status", "state"]).unwrap_or_else(|| "running".to_string());

    if is_terminal_status(&status) {
        state.remove_agent(&id);
    } else {
        // Path-3 flat fallback has no message envelope.
        state.upsert_agent(id, summary, None, event_ts, None, None);
    }
}

fn handle_task_from_tool_use(
    state: &mut SessionState,
    event: &Value,
    raw_event: &Value,
    event_ts: Option<u64>,
) {
    let id = find_string(event, &["id", "tool_use_id", "task_id"])
        .or_else(|| find_string(raw_event, &["id", "tool_use_id", "task_id"]))
        .unwrap_or_else(|| "task-active".to_string());

    let summary = find_string(event, &["name", "description", "prompt"])
        .or_else(|| {
            find_nested_string(
                event,
                &[
                    &["input", "description"],
                    &["input", "prompt"],
                    &["arguments", "description"],
                ],
            )
        })
        .unwrap_or_else(|| "Agent".to_string());

    // Path-3 flat fallback: no message envelope present.
    state.upsert_agent(id, summary, None, event_ts, None, None);
}

fn handle_event_by_name(
    state: &mut SessionState,
    event: &Value,
    raw_event: &Value,
    event_ts: Option<u64>,
) {
    let Some(name) = find_string(event, &["name", "tool_name", "tool"]) else {
        return;
    };

    match name.as_str() {
        "Agent" => handle_task_from_tool_use(state, event, raw_event, event_ts),
        "TaskCreate" => {
            dispatch_task_create(state, event, Some(raw_event));
        }
        "TaskUpdate" => {
            dispatch_task_update(state, event, Some(raw_event));
        }
        "TodoWrite" => {
            dispatch_todo_write(state, event, Some(raw_event));
        }
        _ => {}
    }
}

/// Check `toolUseResult` for agent completion (defense-in-depth for tool_result path).
///
/// Claude Code appends a top-level `toolUseResult` object on agent completion events,
/// containing `{ status, agentId, content, ... }`. This is redundant with the `tool_result`
/// content block processed by Path 1, but provides a fallback if the link-based resolution
/// in `complete_tool_result()` fails (e.g., ID mismatch). `remove_agent()` is idempotent.
fn check_tool_use_result_completion(state: &mut SessionState, raw_event: &Value) {
    if let Some(tur) = raw_event.get("toolUseResult") {
        if let Some(agent_id) = tur.get("agentId").and_then(Value::as_str) {
            let status = tur.get("status").and_then(Value::as_str).unwrap_or("");
            if is_terminal_status(status) {
                state.remove_agent(agent_id);
            }
        }
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "completed" | "done" | "failed" | "cancelled" | "canceled" | "success"
    )
}

/// Upper bound on agents passed to the render layer. The activity-row
/// builder runs `classify()` (which groups same-`message_id` agents into
/// one batch row), then caps at `config.max_agent_lines`. Pre-truncating
/// here would drop members of a batch before classification, e.g.
/// rendering "×2 parallel" when 3 agents really exist. 50 is generous
/// and bounded for memory.
const AGENT_SNAPSHOT_CAP: usize = 50;

fn snapshot_from_state(state: &SessionState, config: &RenderConfig) -> TranscriptSnapshot {
    TranscriptSnapshot {
        tools: state.capped_tools(config.max_tool_lines),
        completed_counts: state.scored_completed_tools(config.max_completed_tools),
        agents: state.agents_for_display(AGENT_SNAPSHOT_CAP),
        todo: state.todo.clone(),
    }
}

fn extract_todo_summary(value: &Value) -> Option<TodoSummary> {
    let todos = find_todos_array(value)?;
    if todos.is_empty() {
        return None;
    }

    let completed = todos
        .iter()
        .filter(|todo| {
            todo.get("status")
                .and_then(Value::as_str)
                .map(|status| matches!(status.to_ascii_lowercase().as_str(), "completed" | "done"))
                .unwrap_or(false)
        })
        .count();

    let total = todos.len();
    let pending = total.saturating_sub(completed);

    if pending == 0 {
        return None;
    }

    Some(TodoSummary {
        text: format!("{completed}/{total} done, {pending} pending"),
        pending,
        completed,
        total,
        ..Default::default()
    })
}

fn find_todos_array(value: &Value) -> Option<&Vec<Value>> {
    // Check top-level "todos" first, then nested under common wrapper keys
    const WRAPPER_KEYS: &[&str] = &["input", "arguments", "args", "output", "result"];

    value.get("todos").and_then(Value::as_array).or_else(|| {
        WRAPPER_KEYS.iter().find_map(|key| {
            value
                .get(*key)
                .and_then(|wrapper| wrapper.get("todos"))
                .and_then(Value::as_array)
        })
    })
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn find_nested_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut cursor = value;
        for segment in *path {
            cursor = cursor.get(*segment)?;
        }
        cursor.as_str().map(ToString::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_target_strips_newlines_tabs_carriage_returns() {
        // Regression: a Bash command containing real newlines used to flow
        // into the statusline unchanged, breaking every pane style's
        // 1-logical-line-per-row contract. `extract_target` runs
        // `sanitize_single_line` on every payload before storing.
        let block = json!({
            "input": {
                "command": "python3 -c \"\nimport sys\nfor i in range(10):\n    print(i)\""
            }
        });
        let out = extract_target("Bash", &block).expect("target");
        assert!(!out.contains('\n'), "no raw newline: {out:?}");
        assert!(!out.contains('\r'));
        assert!(!out.contains('\t'));
    }

    #[test]
    fn extract_target_preserves_full_payload_for_render_layer_truncation() {
        // After the activity-width-budget refactor, transcript no longer
        // pre-truncates targets — render layer chooses per-tool strategy.
        let cmd = "sed -i '' 's/^name = \".*\"$/name = \"cards\"/' .claude/pulseline.toml";
        let block = json!({ "input": { "command": cmd } });
        let out = extract_target("Bash", &block).expect("target");
        assert_eq!(out, cmd, "expected raw payload, got {out:?}");
    }
}
