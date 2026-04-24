//! Target extraction for Claude Code tools introduced after 2.1.16.
//!
//! Only tools that flow through `extract_target()` are exercised here:
//! PowerShell, Monitor, PushNotification, Advisor, MCPSearch.
//!
//! Tools like TaskCreate/TaskUpdate route to the TODO dispatcher; Worktree
//! enter/exit, TaskList, and ToolSearch are filtered as NOISE_TOOLS.

use std::fs::OpenOptions;
use std::io::Write;

use cc_pulseline::{config::RenderConfig, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

fn append_event(path: &std::path::Path, event: serde_json::Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open transcript");
    writeln!(file, "{event}").expect("append event");
}

fn render(
    workspace: &std::path::Path,
    transcript: &std::path::Path,
    session_id: &str,
) -> Vec<String> {
    let input = json!({
        "session_id": session_id,
        "cwd": workspace,
        "workspace": {"current_dir": workspace},
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.1.119",
        "transcript_path": transcript,
        "cost": {"total_cost_usd": 0.0, "total_duration_ms": 0},
    })
    .to_string();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };
    runner
        .run_from_str(&input, config)
        .expect("render succeeds")
}

fn tool_use(id: &str, name: &str, input: serde_json::Value) -> serde_json::Value {
    json!({
        "message": {
            "role": "assistant",
            "content": [{"type": "tool_use", "id": id, "name": name, "input": input}]
        }
    })
}

#[test]
fn powershell_extracts_command() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use("tool-1", "PowerShell", json!({"command": "Get-Process"})),
    );

    let joined = render(ws.path(), &tx, "powershell-sess").join("\n");
    assert!(
        joined.contains("T:PowerShell") && joined.contains("Get-Process"),
        "expected PowerShell command target, got:\n{joined}"
    );
}

#[test]
fn monitor_extracts_script_id() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use("tool-1", "Monitor", json!({"script_id": "build-watch"})),
    );

    let joined = render(ws.path(), &tx, "monitor-sess").join("\n");
    assert!(joined.contains("T:Monitor") && joined.contains("build-watch"));
}

#[test]
fn monitor_falls_back_to_pattern() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use("tool-1", "Monitor", json!({"pattern": "error:"})),
    );

    let joined = render(ws.path(), &tx, "monitor-fallback").join("\n");
    assert!(joined.contains("error:"));
}

#[test]
fn push_notification_extracts_title() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use(
            "tool-1",
            "PushNotification",
            json!({"title": "Build done", "body": "..."}),
        ),
    );

    let joined = render(ws.path(), &tx, "push-sess").join("\n");
    assert!(joined.contains("T:PushNotification") && joined.contains("Build done"));
}

#[test]
fn advisor_extracts_query() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use("tool-1", "Advisor", json!({"query": "why is this slow"})),
    );

    let joined = render(ws.path(), &tx, "advisor-sess").join("\n");
    assert!(joined.contains("T:Advisor") && joined.contains("why is this slow"));
}

#[test]
fn mcp_search_extracts_query() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use("tool-1", "MCPSearch", json!({"query": "slack send"})),
    );

    let joined = render(ws.path(), &tx, "mcp-search-sess").join("\n");
    assert!(joined.contains("T:MCPSearch") && joined.contains("slack send"));
}

// ── Verification that CC 2.1.16+ task tools flow correctly ──────────
// TaskCreate/TaskUpdate route to the TODO dispatcher and should NOT
// show up as T:TaskCreate / T:TaskUpdate tool lines. This is Plan item #5.

#[test]
fn task_create_does_not_appear_as_tool_line() {
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");
    append_event(
        &tx,
        tool_use(
            "tool-1",
            "TaskCreate",
            json!({"subject": "Do the thing", "status": "in_progress"}),
        ),
    );

    let joined = render(ws.path(), &tx, "task-not-tool").join("\n");
    assert!(
        !joined.contains("T:TaskCreate"),
        "TaskCreate should route to TODO dispatcher, not tools row. Got:\n{joined}"
    );
}

#[test]
fn noise_tools_do_not_appear_as_tool_lines() {
    // EnterWorktree, ExitWorktree, TaskList, TaskGet, TaskStop, TaskOutput,
    // ToolSearch are filtered from tool tracking.
    let ws = TempDir::new().expect("tmpdir");
    let tx = ws.path().join("t.jsonl");

    for (idx, name) in [
        "EnterWorktree",
        "ExitWorktree",
        "TaskList",
        "TaskGet",
        "TaskStop",
        "TaskOutput",
        "ToolSearch",
    ]
    .iter()
    .enumerate()
    {
        append_event(&tx, tool_use(&format!("tool-{idx}"), name, json!({})));
    }

    let joined = render(ws.path(), &tx, "noise-test").join("\n");
    for name in [
        "T:EnterWorktree",
        "T:ExitWorktree",
        "T:TaskList",
        "T:TaskGet",
        "T:TaskStop",
        "T:TaskOutput",
        "T:ToolSearch",
    ] {
        assert!(
            !joined.contains(name),
            "{name} should be noise-filtered, got:\n{joined}"
        );
    }
}
