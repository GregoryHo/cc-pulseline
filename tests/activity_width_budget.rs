//! End-to-end coverage for the activity-width-budget rendering. Pins the
//! contract from `designs/activity-width-budget.md` against real
//! `PulseLineRunner` calls (transcript ingestion + render).

use std::fs;
use std::path::Path;

use cc_pulseline::config::{GlyphMode, PulselineConfig, RenderConfig};
use cc_pulseline::PulseLineRunner;
use tempfile::TempDir;

/// Append one JSONL line to the transcript.
fn append_line(path: &Path, line: &str) {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open transcript");
    writeln!(file, "{line}").expect("write line");
}

fn payload(workspace: &Path, transcript: &Path, session: &str) -> String {
    format!(
        r#"{{"session_id":"{session}","model":{{"id":"x","display_name":"Opus 4.7"}},"version":"2.1.119","cwd":"{cwd}","workspace":{{"current_dir":"{cwd}"}},"context_window":{{"context_window_size":200000,"used_percentage":34,"current_usage":{{"input_tokens":1,"output_tokens":13,"cache_creation_input_tokens":5700,"cache_read_input_tokens":114500}}}},"cost":{{"total_cost_usd":4.56,"total_duration_ms":3700000}},"transcript_path":"{tp}"}}"#,
        cwd = workspace.display(),
        tp = transcript.display(),
    )
}

fn cfg() -> RenderConfig {
    RenderConfig {
        glyph_mode: GlyphMode::Ascii,
        color_enabled: false,
        terminal_width: Some(200),
        transcript_poll_throttle_ms: 0,
        max_agent_lines: 2,
        ..PulselineConfig::default().into_render_config()
    }
}

// PulselineConfig doesn't have into_render_config; build via build_render_config.
trait IntoRender {
    fn into_render_config(self) -> RenderConfig;
}
impl IntoRender for PulselineConfig {
    fn into_render_config(self) -> RenderConfig {
        cc_pulseline::config::build_render_config(&self)
    }
}

#[test]
fn parallel_batch_collapses_to_one_row_with_full_count() {
    // 3 Agent tool_uses sharing the same Anthropic message.id should render
    // as a single homogeneous batch row showing ×3 — even though
    // `max_agent_lines = 2` (the cap is on rows, not on batch members).
    let workspace = TempDir::new().expect("temp ws");
    let transcript = workspace.path().join("batch.jsonl");

    for (i, desc) in [
        ("a1", "Code reuse review"),
        ("a2", "Code quality review"),
        ("a3", "Efficiency review"),
    ]
    .iter()
    .enumerate()
    {
        let id = desc.0;
        let d = desc.1;
        append_line(
            &transcript,
            &format!(
                r#"{{"timestamp":"2026-04-27T10:00:0{i}.000Z","message":{{"id":"msg_BATCH","role":"assistant","content":[{{"type":"tool_use","id":"{id}","name":"Agent","input":{{"description":"{d}","subagent_type":"general-purpose"}}}}]}}}}"#
            ),
        );
        append_line(
            &transcript,
            &format!(
                r#"{{"timestamp":"2026-04-27T10:00:0{i}.500Z","type":"progress","data":{{"type":"agent_progress","agentId":"{id}","status":"running"}}}}"#
            ),
        );
    }

    let mut runner = PulseLineRunner::default();
    let lines = runner
        .run_from_str(
            &payload(workspace.path(), &transcript, "batch-session"),
            cfg(),
        )
        .expect("render");
    let blob = lines.join("\n");

    // Homogeneous batch: agent type + ×N + bracketed body, no `parallel` label.
    let agent_row = lines
        .iter()
        .find(|l| l.contains("general-purpose") && l.contains("×3"))
        .unwrap_or_else(|| panic!("no batch row in:\n{blob}"));
    assert!(
        agent_row.contains(" [") && agent_row.contains("]"),
        "body must be bracketed: {agent_row:?}"
    );
    // All three descriptions are joined with ` + ` — no summary phrase.
    assert!(
        !agent_row.contains("+ 2 more"),
        "summary phrase must be removed, got: {agent_row:?}"
    );
    for desc in [
        "Code reuse review",
        "Code quality review",
        "Efficiency review",
    ] {
        assert!(
            agent_row.contains(desc),
            "expected description {desc:?}, got: {agent_row:?}"
        );
    }
}

#[test]
fn bash_target_keeps_verb_at_row_start() {
    // The verb (`sed`, `cargo`, `grep`, …) anchors the rendered cell so the
    // operator can recognise the command at a glance — end-to-end check
    // that transcript stores raw + render layer keeps the head intact.
    let workspace = TempDir::new().expect("temp ws");
    let transcript = workspace.path().join("bash.jsonl");

    let cmd = "sed -i '' 's/^name = \".*\"$/name = \"cards\"/' .claude/pulseline.toml";
    append_line(
        &transcript,
        &format!(
            r#"{{"timestamp":"2026-04-27T10:00:00.000Z","message":{{"id":"msg_BASH","role":"assistant","content":[{{"type":"tool_use","id":"b1","name":"Bash","input":{{"command":"{cmd}"}}}}]}}}}"#,
            cmd = cmd.replace('"', "\\\"")
        ),
    );

    let mut runner = PulseLineRunner::default();
    let lines = runner
        .run_from_str(
            &payload(workspace.path(), &transcript, "bash-session"),
            cfg(),
        )
        .expect("render");
    let blob = lines.join("\n");

    let bash_row = lines
        .iter()
        .find(|l| l.contains("T:Bash"))
        .unwrap_or_else(|| panic!("no T:Bash row in:\n{blob}"));
    assert!(
        bash_row.contains("T:Bash: sed"),
        "verb must lead the cell body, got: {bash_row:?}"
    );
}

#[test]
fn sequential_agents_render_individually_no_fold() {
    // Different `message.id`s = sequential. Even though they share a type,
    // they get one row each (with overflow summary if max_agent_lines is
    // exceeded). This pins the "no false fold" guarantee from §3.
    let workspace = TempDir::new().expect("temp ws");
    let transcript = workspace.path().join("seq.jsonl");

    for (i, desc) in [("first agent"), ("second agent"), ("third agent")]
        .iter()
        .enumerate()
    {
        let id = format!("a{i}");
        append_line(
            &transcript,
            &format!(
                r#"{{"timestamp":"2026-04-27T10:00:0{i}.000Z","message":{{"id":"msg_{i}","role":"assistant","content":[{{"type":"tool_use","id":"{id}","name":"Agent","input":{{"description":"{desc}","subagent_type":"general-purpose"}}}}]}}}}"#
            ),
        );
        append_line(
            &transcript,
            &format!(
                r#"{{"timestamp":"2026-04-27T10:00:0{i}.500Z","type":"progress","data":{{"type":"agent_progress","agentId":"{id}","status":"running"}}}}"#
            ),
        );
    }

    let mut runner = PulseLineRunner::default();
    let mut config = cfg();
    config.max_agent_lines = 2;
    let lines = runner
        .run_from_str(
            &payload(workspace.path(), &transcript, "seq-session"),
            config,
        )
        .expect("render");
    let blob = lines.join("\n");

    // No `parallel` label — these are sequential.
    assert!(
        !blob.contains("parallel"),
        "sequential agents must not collapse: {blob}"
    );
    // 2 individual rows visible + 1 overflow summary. Pluralization is
    // count-correct: 1 → `more agent`, 2+ → `more agents`.
    assert!(
        blob.contains("more agent"),
        "expected overflow summary, got: {blob}"
    );
}
