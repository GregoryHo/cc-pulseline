use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn smoke_cli_startup_with_fixture() {
    let fixture = fs::read_to_string("tests/fixtures/minimal_statusline_input.json")
        .expect("fixture should exist");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .env("PULSELINE_ICONS", "ascii")
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
        lines[0].contains("M:"),
        "line 1 should include model segment"
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
