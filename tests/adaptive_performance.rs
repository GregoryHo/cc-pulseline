use std::{
    fs::{self, OpenOptions},
    io::Write,
    time::{Duration, Instant},
};

use cc_pulseline::{config::RenderConfig, render::color::visible_width, PulseLineRunner};
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
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 72,
            "current_usage": {
                "input_tokens": 100,
                "output_tokens": 200,
                "cache_creation_input_tokens": 300,
                "cache_read_input_tokens": 400
            }
        },
        "cost": {
            "total_cost_usd": 9.25,
            "total_duration_ms": 5400000
        },
        "transcript_path": transcript_path
    })
    .to_string()
}

#[test]
fn degrades_layout_for_narrow_terminal_widths() {
    let workspace = TempDir::new().expect("temp workspace");
    fs::create_dir_all(workspace.path().join(".claude/rules")).expect("rules dir");
    fs::write(workspace.path().join("CLAUDE.md"), "# Claude\n").expect("claude file");

    let transcript = workspace.path().join("narrow-flow.jsonl");
    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"tool-1","name":"ReadFile"}"#,
    );
    append_line(
        &transcript,
        r#"{"type":"Task","task_id":"agent-1","name":"Planner","status":"running"}"#,
    );
    append_line(
        &transcript,
        r#"{"type":"TodoWrite","todos":[{"content":"a","status":"pending"}]}"#,
    );

    let payload = payload_json(&workspace, &transcript, "narrow-layout");
    let mut runner = PulseLineRunner::default();

    let wide = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };
    let wide_lines = runner
        .run_from_str(&payload, wide)
        .expect("wide render should succeed");
    assert!(
        wide_lines.len() > 3,
        "wide render should include activity lines"
    );

    let narrow = RenderConfig {
        transcript_poll_throttle_ms: 0,
        terminal_width: Some(36),
        ..RenderConfig::default()
    };
    let narrow_lines = runner
        .run_from_str(&payload, narrow)
        .expect("narrow render should succeed");

    assert_eq!(
        narrow_lines.len(),
        3,
        "narrow render should prioritize core lines"
    );
    assert!(
        narrow_lines.iter().all(|line| visible_width(line) <= 36),
        "all lines should fit target width"
    );
    assert!(
        narrow_lines.iter().all(|line| !line.starts_with("T:")),
        "tool lines should be dropped during degradation"
    );
    assert!(
        narrow_lines.iter().all(|line| !line.starts_with("A:")),
        "agent lines should be dropped during degradation"
    );
    assert!(
        narrow_lines.iter().all(|line| !line.starts_with("TODO:")),
        "todo lines should be dropped during degradation"
    );
}

fn write_large_transcript(path: &std::path::Path, iterations: usize) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("transcript file should open");

    for i in 0..iterations {
        writeln!(
            file,
            "{{\"type\":\"tool_use\",\"tool_use_id\":\"tool-{i}\",\"name\":\"ReadFile\"}}"
        )
        .expect("write tool_use");

        if i % 2 == 0 {
            writeln!(
                file,
                "{{\"type\":\"tool_result\",\"tool_use_id\":\"tool-{i}\"}}"
            )
            .expect("write tool_result");
        }

        writeln!(
            file,
            "{{\"type\":\"Task\",\"task_id\":\"agent-{i}\",\"name\":\"Agent{i}\",\"status\":\"running\"}}"
        )
        .expect("write task running");

        if i % 3 == 0 {
            writeln!(
                file,
                "{{\"type\":\"Task\",\"task_id\":\"agent-{i}\",\"name\":\"Agent{i}\",\"status\":\"completed\"}}"
            )
            .expect("write task completed");
        }
    }

    writeln!(
        file,
        "{{\"type\":\"TodoWrite\",\"todos\":[{{\"content\":\"a\",\"status\":\"completed\"}},{{\"content\":\"b\",\"status\":\"pending\"}},{{\"content\":\"c\",\"status\":\"pending\"}}]}}"
    )
    .expect("write todo summary");
}

#[test]
fn handles_large_transcript_and_stays_within_render_budget() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("large.jsonl");
    write_large_transcript(&transcript, 2500);

    let payload = payload_json(&workspace, &transcript, "large-perf");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        max_tool_lines: 2,
        max_agent_lines: 1,
        transcript_window_events: 1000,
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    let first = runner
        .run_from_str(&payload, config.clone())
        .expect("initial render should succeed");
    assert!(
        first.len() <= 7,
        "core + capped activity lines should remain bounded"
    );

    let mut durations = Vec::with_capacity(120);
    for _ in 0..120 {
        let start = Instant::now();
        let _ = runner
            .run_from_str(&payload, config.clone())
            .expect("render should succeed");
        durations.push(start.elapsed());
    }

    durations.sort();
    let idx = ((durations.len() as f64) * 0.95).floor() as usize;
    let p95 = durations[idx.min(durations.len() - 1)];

    assert!(
        p95 < Duration::from_millis(50),
        "p95 render latency {:?} exceeded 50ms budget",
        p95
    );
}
