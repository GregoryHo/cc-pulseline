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
