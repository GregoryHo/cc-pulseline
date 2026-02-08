use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    config::RenderConfig,
    state::SessionState,
    types::{AgentSummary, StdinPayload, TodoSummary, ToolSummary},
};

#[derive(Debug, Clone, Default)]
pub struct TranscriptSnapshot {
    pub tools: Vec<ToolSummary>,
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
            state.active_agents.clear();
            state.todo = None;
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

fn apply_transcript_event(state: &mut SessionState, raw_event: &Value) {
    let event = if let Some(message) = raw_event.get("message").filter(|value| value.is_object()) {
        message
    } else {
        raw_event
    };

    let event_type = find_string(event, &["type", "event", "event_type"])
        .or_else(|| find_string(raw_event, &["type", "event", "event_type"]));

    match event_type.as_deref() {
        Some("tool_use") => handle_tool_use(state, event, raw_event),
        Some("tool_result") => handle_tool_result(state, event, raw_event),
        Some("Task") => handle_task_event(state, event),
        Some("TodoWrite") | Some("TaskUpdate") => {
            state.set_todo(extract_todo_summary(event).or_else(|| extract_todo_summary(raw_event)));
        }
        _ => handle_event_by_name(state, event, raw_event),
    }
}

fn handle_tool_use(state: &mut SessionState, event: &Value, raw_event: &Value) {
    let name = find_string(event, &["name", "tool_name", "tool"])
        .or_else(|| find_string(raw_event, &["name", "tool_name", "tool"]))
        .unwrap_or_else(|| "unknown".to_string());

    if name == "Task" {
        handle_task_from_tool_use(state, event, raw_event);
        return;
    }

    if name == "TodoWrite" || name == "TaskUpdate" {
        state.set_todo(extract_todo_summary(event).or_else(|| extract_todo_summary(raw_event)));
        return;
    }

    let id = find_string(event, &["id", "tool_use_id", "tool_call_id"])
        .or_else(|| find_string(raw_event, &["id", "tool_use_id", "tool_call_id"]))
        .unwrap_or_else(|| format!("{name}-active"));

    state.upsert_tool(id, name);
}

fn handle_tool_result(state: &mut SessionState, event: &Value, raw_event: &Value) {
    if let Some(id) = find_string(event, &["tool_use_id", "id", "tool_call_id"])
        .or_else(|| find_string(raw_event, &["tool_use_id", "id", "tool_call_id"]))
    {
        state.remove_tool(&id);
        state.remove_agent(&id);
    }

    if let Some(todo) = extract_todo_summary(event).or_else(|| extract_todo_summary(raw_event)) {
        state.set_todo(Some(todo));
    }
}

fn handle_task_event(state: &mut SessionState, event: &Value) {
    let id = find_string(event, &["task_id", "id", "name"]).unwrap_or_else(|| "task".to_string());
    let summary =
        find_string(event, &["name", "description", "prompt"]).unwrap_or_else(|| id.clone());
    let status = find_string(event, &["status", "state"]).unwrap_or_else(|| "running".to_string());

    if is_terminal_status(&status) {
        state.remove_agent(&id);
    } else {
        state.upsert_agent(id, summary);
    }
}

fn handle_task_from_tool_use(state: &mut SessionState, event: &Value, raw_event: &Value) {
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
        .unwrap_or_else(|| "Task".to_string());

    state.upsert_agent(id, summary);
}

fn handle_event_by_name(state: &mut SessionState, event: &Value, raw_event: &Value) {
    let Some(name) = find_string(event, &["name", "tool_name", "tool"]) else {
        return;
    };

    match name.as_str() {
        "Task" => handle_task_from_tool_use(state, event, raw_event),
        "TodoWrite" | "TaskUpdate" => {
            state.set_todo(extract_todo_summary(event).or_else(|| extract_todo_summary(raw_event)));
        }
        _ => {}
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "completed" | "done" | "failed" | "cancelled" | "canceled" | "success"
    )
}

fn snapshot_from_state(state: &SessionState, config: &RenderConfig) -> TranscriptSnapshot {
    TranscriptSnapshot {
        tools: state.capped_tools(config.max_tool_lines),
        agents: state.capped_agents(config.max_agent_lines),
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
    })
}

fn find_todos_array(value: &Value) -> Option<&Vec<Value>> {
    value
        .get("todos")
        .and_then(Value::as_array)
        .or_else(|| {
            value
                .get("input")
                .and_then(|input| input.get("todos"))
                .and_then(Value::as_array)
        })
        .or_else(|| {
            value
                .get("arguments")
                .and_then(|arguments| arguments.get("todos"))
                .and_then(Value::as_array)
        })
        .or_else(|| {
            value
                .get("args")
                .and_then(|args| args.get("todos"))
                .and_then(Value::as_array)
        })
        .or_else(|| {
            value
                .get("output")
                .and_then(|output| output.get("todos"))
                .and_then(Value::as_array)
        })
        .or_else(|| {
            value
                .get("result")
                .and_then(|result| result.get("todos"))
                .and_then(Value::as_array)
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
