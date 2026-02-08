use std::{
    fs::{self, OpenOptions},
    io::Write,
    thread,
    time::Duration,
};

use cc_pulseline::{config::RenderConfig, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

fn append_line(path: &std::path::Path, line: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("transcript file should open");
    writeln!(file, "{line}").expect("line should append");
}

fn payload_json(
    workspace: &TempDir,
    transcript_path: &std::path::Path,
    session_id: &str,
) -> String {
    json!({
        "session_id": session_id,
        "cwd": workspace.path(),
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.2.0",
        "transcript_path": transcript_path,
        "cost": {
            "total_cost_usd": 1.0,
            "total_duration_ms": 60000
        }
    })
    .to_string()
}

#[test]
fn tracks_tool_lifecycle_incrementally_from_transcript() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("tool-flow.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_tool_flow.jsonl")
        .expect("tool fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "tool-flow"),
            config.clone(),
        )
        .expect("render should succeed");
    assert!(
        lines.iter().any(|line| line == "T:ReadFile"),
        "tool_use should produce an active tool line"
    );

    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "tool-flow"), config)
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.starts_with("T:")),
        "tool_result should clear active tool lines"
    );
}

#[test]
fn caps_agent_lines_and_applies_task_completion_updates() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("agent-flow.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_agent_flow.jsonl")
        .expect("agent fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_agent_lines: 2,
        ..RenderConfig::default()
    };

    append_line(&transcript, events[0]);
    append_line(&transcript, events[1]);
    append_line(&transcript, events[2]);

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "agent-flow"),
            config.clone(),
        )
        .expect("render should succeed");
    let agent_lines: Vec<&String> = lines.iter().filter(|line| line.starts_with("A:")).collect();

    assert_eq!(agent_lines.len(), 2, "agent line cap should be enforced");
    assert!(agent_lines.iter().any(|line| line.contains("Planner")));
    assert!(agent_lines.iter().any(|line| line.contains("Reviewer")));

    append_line(&transcript, events[3]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "agent-flow"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        !joined.contains("A:Planner"),
        "completed task should be removed"
    );
    assert!(
        joined.contains("A:Indexer"),
        "still-running task should remain"
    );
    assert!(
        joined.contains("A:Reviewer"),
        "still-running task should remain"
    );
}

#[test]
fn updates_todo_line_from_todowrite_and_taskupdate_events() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("todo-flow.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_todo_flow.jsonl")
        .expect("todo fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "todo-flow"),
            config.clone(),
        )
        .expect("render should succeed");
    assert!(
        lines.iter().any(|line| line == "TODO:1/3 done, 2 pending"),
        "TodoWrite should create a TODO summary line"
    );

    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "todo-flow"), config)
        .expect("render should succeed");

    assert!(
        lines.iter().all(|line| !line.starts_with("TODO:")),
        "TaskUpdate that completes all todos should clear TODO line"
    );
}

#[test]
fn throttles_transcript_polling_between_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("throttle-flow.jsonl");

    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"tool-1","name":"Bash"}"#,
    );

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        max_tool_lines: 2,
        transcript_poll_throttle_ms: 120,
        ..RenderConfig::default()
    };

    let payload = payload_json(&workspace, &transcript, "throttle-flow");

    let lines = runner
        .run_from_str(&payload, config.clone())
        .expect("render should succeed");
    assert!(lines.iter().any(|line| line == "T:Bash"));

    append_line(
        &transcript,
        r#"{"type":"tool_result","tool_use_id":"tool-1"}"#,
    );

    let lines = runner
        .run_from_str(&payload, config.clone())
        .expect("render should succeed");
    assert!(
        lines.iter().any(|line| line == "T:Bash"),
        "poll throttling should delay transcript refresh"
    );

    thread::sleep(Duration::from_millis(140));
    let lines = runner
        .run_from_str(&payload, config)
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.starts_with("T:")),
        "line should disappear once throttle period elapses"
    );
}

#[test]
fn applies_transcript_windowing_to_new_event_batches() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("window-flow.jsonl");

    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"tool-1","name":"ToolA"}"#,
    );
    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"tool-2","name":"ToolB"}"#,
    );
    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"tool-3","name":"ToolC"}"#,
    );

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        max_tool_lines: 3,
        transcript_poll_throttle_ms: 0,
        transcript_window_events: 2,
        ..RenderConfig::default()
    };

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "window-flow"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        !joined.contains("T:ToolA"),
        "oldest event should fall out of window"
    );
    assert!(
        joined.contains("T:ToolB"),
        "window should include newer events"
    );
    assert!(
        joined.contains("T:ToolC"),
        "window should include newest events"
    );
}
