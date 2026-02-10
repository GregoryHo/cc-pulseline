use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

use cc_pulseline::config::RenderConfig;
use cc_pulseline::run_from_str;

#[test]
fn smoke_cli_startup_with_fixture() {
    let fixture = fs::read_to_string("tests/fixtures/minimal_statusline_input.json")
        .expect("fixture should exist");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start cc-pulseline binary");

    child
        .stdin
        .as_mut()
        .expect("stdin should be available")
        .write_all(fixture.as_bytes())
        .expect("failed to write fixture to stdin");

    let output = child
        .wait_with_output()
        .expect("failed to wait for process");
    assert!(output.status.success(), "binary should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 lines of output");
    assert!(
        lines[0].contains("Opus 4.6"),
        "line 1 should include model name"
    );
}

#[test]
fn fixture_file_loads_as_json() {
    let fixture = fs::read_to_string("tests/fixtures/minimal_statusline_input.json")
        .expect("fixture should exist");

    let parsed: serde_json::Value =
        serde_json::from_str(&fixture).expect("fixture should be valid json");
    assert!(parsed.is_object(), "fixture root must be an object");
}

// ── Robustness tests ──────────────────────────────────────────────────

#[test]
fn handles_empty_json_object() {
    let config = RenderConfig::default();
    let result = run_from_str("{}", config);
    assert!(result.is_ok(), "empty JSON should not panic: {result:?}");
    let lines = result.unwrap();
    assert!(!lines.is_empty(), "should produce at least one line");
}

#[test]
fn handles_malformed_json_gracefully() {
    let config = RenderConfig::default();
    let result = run_from_str("{invalid json", config);
    assert!(result.is_err(), "malformed JSON should return error");
    let err = result.unwrap_err();
    assert!(
        err.contains("invalid"),
        "error should describe the problem: {err}"
    );
}

#[test]
fn handles_empty_transcript_path() {
    let payload = serde_json::json!({
        "session_id": "test-empty-transcript",
        "transcript_path": "",
        "model": { "display_name": "Test" },
        "version": "1.0"
    });
    let config = RenderConfig::default();
    let result = run_from_str(&payload.to_string(), config);
    assert!(
        result.is_ok(),
        "empty transcript path should not panic: {result:?}"
    );
}

#[test]
fn handles_nonexistent_transcript_path() {
    let payload = serde_json::json!({
        "session_id": "test-missing-transcript",
        "transcript_path": "/tmp/cc-pulseline-nonexistent-transcript-12345.jsonl",
        "model": { "display_name": "Test" },
        "version": "1.0"
    });
    let config = RenderConfig::default();
    let result = run_from_str(&payload.to_string(), config);
    assert!(
        result.is_ok(),
        "missing transcript should not panic: {result:?}"
    );
}

#[test]
fn handles_missing_model_fields() {
    let payload = serde_json::json!({
        "session_id": "test-no-model",
        "version": "1.0"
    });
    let config = RenderConfig::default();
    let result = run_from_str(&payload.to_string(), config);
    assert!(result.is_ok(), "missing model should not panic: {result:?}");
}

#[test]
fn handles_malformed_transcript_lines() {
    let dir = tempfile::TempDir::new().unwrap();
    let transcript_path = dir.path().join("transcript.jsonl");
    // Write a mix of valid and invalid lines
    fs::write(
        &transcript_path,
        "not json at all\n{\"type\":\"assistant\"}\n{broken\n",
    )
    .unwrap();

    let payload = serde_json::json!({
        "session_id": "test-malformed-transcript",
        "transcript_path": transcript_path.to_str().unwrap(),
        "model": { "display_name": "Test" },
        "version": "1.0"
    });
    let config = RenderConfig::default();
    let result = run_from_str(&payload.to_string(), config);
    assert!(
        result.is_ok(),
        "malformed transcript lines should not panic: {result:?}"
    );
}
