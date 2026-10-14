//! The function-hooks mod path: activity arrives on the payload, already
//! assembled from engine events, instead of being parsed out of a transcript.

use std::fs::{self, OpenOptions};
use std::io::Write;

use cc_pulseline::{config::RenderConfig, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

fn config() -> RenderConfig {
    RenderConfig {
        transcript_poll_throttle_ms: 0,
        color_enabled: false,
        max_tool_lines: 4,
        max_agent_lines: 4,
        ..RenderConfig::default()
    }
}

fn base_payload(workspace: &TempDir) -> serde_json::Value {
    json!({
        "session_id": "mod-activity",
        "cwd": workspace.path(),
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus"},
        "version": "2.1.275"
    })
}

#[test]
fn renders_activity_supplied_on_the_payload() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "tools": [{"id": "t1", "name": "Bash", "target": "cargo test"}],
        "completed_counts": [{"name": "Read", "count": 12, "failed": 2}],
        "completed_total": 12,
        "failed_total": 2,
        "agents": [{
            "id": "a1",
            "description": "Investigate logic",
            "agent_type": "Explore"
        }],
        "todo": {
            "text": "Fixing auth bug",
            "pending": 2,
            "completed": 1,
            "total": 3,
            "is_task_api": true,
            "in_progress_items": [{"text": "Fixing auth bug"}]
        }
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        joined.contains("Bash"),
        "running tool should render: {joined}"
    );
    assert!(
        joined.contains("Read"),
        "completed count should render: {joined}"
    );
    assert!(
        joined.contains("Investigate logic"),
        "agent row should render: {joined}"
    );
    assert!(
        joined.contains("Fixing auth bug"),
        "todo row should render: {joined}"
    );
}

#[test]
fn payload_activity_wins_over_the_transcript_file() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("session.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_tool_flow.jsonl")
        .expect("tool fixture should exist");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&transcript)
        .expect("transcript should open");
    for line in fixture.lines() {
        writeln!(file, "{line}").expect("line should append");
    }

    let mut payload = base_payload(&workspace);
    payload["transcript_path"] = json!(transcript);
    payload["activity"] = json!({
        "tools": [{"id": "t1", "name": "Grep", "target": "needle"}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        joined.contains("Grep"),
        "payload activity should render: {joined}"
    );
    assert!(
        !joined.contains("ReadFile"),
        "the transcript must not be parsed on the mod path: {joined}"
    );
}

#[test]
fn a_payload_without_activity_still_reads_the_transcript() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("session.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_tool_flow.jsonl")
        .expect("tool fixture should exist");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&transcript)
        .expect("transcript should open");
    for line in fixture.lines() {
        writeln!(file, "{line}").expect("line should append");
    }

    let mut payload = base_payload(&workspace);
    payload["transcript_path"] = json!(transcript);

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        joined.contains("ReadFile"),
        "the classic statusline path must keep parsing the transcript: {joined}"
    );
}

#[test]
fn env_counts_from_the_payload_override_the_scan() {
    let workspace = TempDir::new().expect("temp workspace");
    // A CLAUDE.md on disk the scan would find and count.
    fs::write(workspace.path().join("CLAUDE.md"), "# project\n").expect("file writes");

    let mut payload = base_payload(&workspace);
    payload["env"] = json!({"claude_md_count": 7, "mcp_count": 2});

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        joined.contains("7 CLAUDE.md"),
        "the payload's count wins over the scan: {joined}"
    );
    assert!(
        joined.contains("2 MCPs"),
        "the payload's MCP count is rendered: {joined}"
    );
}

#[test]
fn a_fractional_context_percentage_rounds_instead_of_failing() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    // TypeScript hands over whatever `usage.context.percent` holds; a whole
    // number on the statusline path can be `43.7` on the mod path.
    payload["context_window"] = json!({
        "context_window_size": 200_000,
        "used_percentage": 43.7
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a fractional percentage must not reject the payload");

    assert!(
        lines.join("\n").contains("44%"),
        "43.7 rounds to 44%: {}",
        lines.join("\n")
    );
}

#[test]
fn a_fractional_token_count_rounds_instead_of_failing() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    // Every producer today increments an integer, so a float here is
    // defensive — but one unreadable figure must cost its own cell, not the
    // whole payload, exactly as for `used_percentage`.
    payload["context_window"] = json!({
        "context_window_size": 200_000,
        "used_percentage": 43,
        "current_usage": {"input_tokens": 10.6, "output_tokens": 20}
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a fractional token count must not reject the payload");

    assert!(
        lines.join("\n").contains("I:11"),
        "10.6 input tokens round to 11: {}",
        lines.join("\n")
    );
}

#[test]
fn a_context_window_without_a_percentage_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["context_window"] = json!({ "context_window_size": 200_000 });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing percentage must stay optional");

    assert!(!lines.is_empty(), "the identity line still renders");
}

// --- A missing key must cost its own cell, never the whole payload. -------
//
// serde rejects the *entire* stdin JSON on one unreadable field, which blanks
// every statusline row. A mod assembles the payload in TypeScript, so a scalar
// it leaves out has to fall back instead. One test per hardened field, so a
// regression names the field that broke.

#[test]
fn a_completed_count_without_count_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "completed_counts": [{"name": "Read"}, {"name": "Bash", "count": 3}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `count` must not reject the payload");

    assert!(
        lines.join("\n").contains("Bash"),
        "the well-formed sibling proves the activity was parsed: {}",
        lines.join("\n")
    );
}

#[test]
fn a_completed_count_without_failed_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "completed_counts": [{"name": "Read", "count": 12}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `failed` must not reject the payload");

    assert!(
        lines.join("\n").contains("Read"),
        "the completed count still renders: {}",
        lines.join("\n")
    );
}

#[test]
fn a_todo_without_pending_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "todo": {"text": "Fixing auth bug", "completed": 1, "total": 3}
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `pending` must not reject the payload");

    assert!(
        lines.join("\n").contains("Fixing auth bug"),
        "the todo row still renders: {}",
        lines.join("\n")
    );
}

#[test]
fn a_todo_without_completed_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "todo": {"text": "Fixing auth bug", "pending": 2, "total": 3}
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `completed` must not reject the payload");

    assert!(
        lines.join("\n").contains("Fixing auth bug"),
        "the todo row still renders: {}",
        lines.join("\n")
    );
}

#[test]
fn a_todo_without_total_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "todo": {"text": "Fixing auth bug", "pending": 2, "completed": 1}
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `total` must not reject the payload");

    assert!(
        lines.join("\n").contains("Fixing auth bug"),
        "the todo row still renders: {}",
        lines.join("\n")
    );
}

#[test]
fn a_running_tool_without_id_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "tools": [{"name": "Bash", "target": "cargo test"}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `id` must not reject the payload");

    assert!(
        lines.join("\n").contains("Bash"),
        "the running tool still renders: {}",
        lines.join("\n")
    );
}

#[test]
fn a_running_tool_without_name_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "tools": [{"id": "t1", "target": "cargo test"}],
        "completed_counts": [{"name": "Read", "count": 4}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `name` must not reject the payload");

    assert!(
        lines.join("\n").contains("Read"),
        "the rest of the activity still renders: {}",
        lines.join("\n")
    );
}

// --- The same contract for the remaining `String` fields. ----------------
//
// These had neither `Option` nor a default, so a missing key rejected the
// whole payload exactly as the scalars above did. They are now covered by
// struct-level `#[serde(default)]`, which states the contract once per struct
// rather than once per field.

#[test]
fn an_agent_without_id_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "agents": [
            {"description": "Investigate logic"},
            {"id": "a2", "description": "Check config"}
        ]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing agent `id` must not reject the payload");

    assert!(
        lines.join("\n").contains("Check config"),
        "the well-formed sibling proves the agents were parsed: {}",
        lines.join("\n")
    );
}

#[test]
fn an_agent_without_description_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "agents": [
            {"id": "a1"},
            {"id": "a2", "description": "Check config"}
        ]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing agent `description` must not reject the payload");

    assert!(
        lines.join("\n").contains("Check config"),
        "the well-formed sibling proves the agents were parsed: {}",
        lines.join("\n")
    );
}

#[test]
fn a_completed_count_without_name_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "completed_counts": [{"count": 3}, {"name": "Bash", "count": 5}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing `name` must not reject the payload");

    assert!(
        lines.join("\n").contains("Bash"),
        "the well-formed sibling proves the counts were parsed: {}",
        lines.join("\n")
    );
}

#[test]
fn a_todo_without_text_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "todo": {"pending": 1, "completed": 0, "total": 1},
        "completed_counts": [{"name": "Read", "count": 4}]
    });

    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config())
        .expect("a missing todo `text` must not reject the payload");

    assert!(
        lines.join("\n").contains("Read"),
        "the rest of the activity proves the payload was parsed: {}",
        lines.join("\n")
    );
}

#[test]
fn a_todo_item_without_text_still_renders() {
    let workspace = TempDir::new().expect("temp workspace");
    let mut payload = base_payload(&workspace);
    payload["activity"] = json!({
        "todo": {
            "text": "Fixing auth bug",
            "pending": 1,
            "completed": 0,
            "total": 2,
            // Per-item rows need the task API plus room for more than one.
            "is_task_api": true,
            "in_progress_items": [{"started_at": 1}, {"text": "Second item"}]
        }
    });

    let config = RenderConfig {
        max_todo_lines: 4,
        ..config()
    };
    let lines = PulseLineRunner::default()
        .run_from_str(&payload.to_string(), config)
        .expect("a missing todo item `text` must not reject the payload");

    assert!(
        lines.join("\n").contains("Second item"),
        "the well-formed sibling item proves the todo was parsed: {}",
        lines.join("\n")
    );
}
