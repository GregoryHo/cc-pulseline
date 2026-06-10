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
        r#"{"timestamp":"2026-04-27T10:00:00.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"ReadFile","input":{}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"timestamp":"2026-04-27T10:00:01.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"agent-1","name":"Agent","input":{"description":"Planner","subagent_type":"general-purpose"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"timestamp":"2026-04-27T10:00:02.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"todo-1","name":"TodoWrite","input":{"todos":[{"content":"a","status":"pending"}]}}]}}"#,
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

    // Width-fit invariant — the only hard requirement for narrow renders.
    // The previous version of this test also asserted that activity lines
    // were dropped at width=36, but that codified the old over-aggressive
    // `DropActivityLinesFirst` strategy. The new degrade order compresses
    // first and only drops activity as a last resort, so short activity
    // rows ("T:ReadFile", etc.) are correctly preserved when they fit.
    assert!(
        narrow_lines.iter().all(|line| visible_width(line) <= 36),
        "all lines should fit target width"
    );
    assert!(
        !narrow_lines.is_empty(),
        "narrow render should still produce core lines"
    );
}

#[test]
fn drop_activity_strategy_still_fires_when_compression_alone_cannot_help() {
    // Direct test of the strategy as a last-resort mechanism. Use a
    // pathologically narrow width and a custom degrade_order containing
    // only DropActivityLinesFirst — verify activity rows are still dropped.
    let workspace = TempDir::new().expect("temp workspace");
    fs::write(workspace.path().join("CLAUDE.md"), "# Claude\n").expect("claude file");
    let transcript = workspace.path().join("drop-flow.jsonl");
    append_line(
        &transcript,
        r#"{"timestamp":"2026-04-27T10:00:00.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"ReadFile","input":{}}]}}"#,
    );

    let payload = payload_json(&workspace, &transcript, "drop-test");
    let mut runner = PulseLineRunner::default();

    let cfg = RenderConfig {
        transcript_poll_throttle_ms: 0,
        terminal_width: Some(36),
        degrade_order: vec![cc_pulseline::config::WidthDegradeStrategy::DropActivityLinesFirst],
        ..RenderConfig::default()
    };
    let lines = runner.run_from_str(&payload, cfg).expect("render");
    assert!(
        lines.iter().all(|line| !line.starts_with("T:")),
        "DropActivityLinesFirst should remove tool lines when configured alone"
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
            "{{\"timestamp\":\"2026-04-27T10:00:00.000Z\",\"message\":{{\"role\":\"assistant\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"tool-{i}\",\"name\":\"ReadFile\",\"input\":{{}}}}]}}}}"
        )
        .expect("write tool_use");

        if i % 2 == 0 {
            writeln!(
                file,
                "{{\"timestamp\":\"2026-04-27T10:00:01.000Z\",\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"tool-{i}\"}}]}}"
            )
            .expect("write tool_result");
        }

        writeln!(
            file,
            "{{\"timestamp\":\"2026-04-27T10:00:02.000Z\",\"message\":{{\"role\":\"assistant\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"agent-{i}\",\"name\":\"Agent\",\"input\":{{\"description\":\"Agent{i}\",\"subagent_type\":\"general-purpose\"}}}}]}}}}"
        )
        .expect("write agent running");

        if i % 3 == 0 {
            writeln!(
                file,
                "{{\"timestamp\":\"2026-04-27T10:00:03.000Z\",\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"agent-{i}\"}}],\"toolUseResult\":{{\"agentId\":\"agent-{i}\",\"status\":\"completed\"}}}}"
            )
            .expect("write agent completed");
        }
    }

    writeln!(
        file,
        "{{\"timestamp\":\"2026-04-27T10:00:04.000Z\",\"message\":{{\"role\":\"assistant\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"todo-final\",\"name\":\"TodoWrite\",\"input\":{{\"todos\":[{{\"content\":\"a\",\"status\":\"completed\"}},{{\"content\":\"b\",\"status\":\"pending\"}},{{\"content\":\"c\",\"status\":\"pending\"}}]}}}}]}}}}"
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

#[test]
fn sub_agent_tailing_stays_within_render_budget() {
    // Parent + 3 active sub-agents (each with their own ~800-event
    // transcript). Verifies the worst-case `1 + N` file-IO per tick
    // (where N = active sub-agents) still fits inside the 50ms p95
    // budget at MAX_SUBAGENT_TAILS-class fanout.
    let workspace = TempDir::new().expect("temp workspace");
    let parent = workspace.path().join("parent.jsonl");

    // Three Agent dispatches + their async_launched tool_results.
    let agents = [
        ("aaaaaaaaaaaaaaaaa", "toolu_perf_a"),
        ("bbbbbbbbbbbbbbbbb", "toolu_perf_b"),
        ("ccccccccccccccccc", "toolu_perf_c"),
    ];
    for (agent_id, tuid) in agents {
        let dispatch = json!({
            "type": "assistant",
            "timestamp": "2026-05-16T10:00:00.000Z",
            "message": {
                "id": format!("msg_{tuid}"),
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": tuid,
                    "name": "Agent",
                    "input": {"description": "perf", "subagent_type": "general-purpose", "prompt": "x"}
                }]
            }
        });
        append_line(&parent, &dispatch.to_string());
        let launched = json!({
            "type": "user",
            "timestamp": "2026-05-16T10:00:01.000Z",
            "message": {
                "role": "user",
                "content": [{
                    "tool_use_id": tuid,
                    "type": "tool_result",
                    "content": [{"type": "text", "text": "Async agent launched successfully."}]
                }]
            },
            "toolUseResult": {"isAsync": true, "status": "async_launched", "agentId": agent_id}
        });
        append_line(&parent, &launched.to_string());
    }

    // Each sub-agent's transcript: ~800 lines of TaskCreate + a few updates.
    let parent_stem = parent.file_stem().unwrap().to_str().unwrap();
    let subagents_dir = parent.parent().unwrap().join(parent_stem).join("subagents");
    fs::create_dir_all(&subagents_dir).expect("subagents dir");
    for (agent_id, _) in agents {
        let sub_path = subagents_dir.join(format!("agent-{agent_id}.jsonl"));
        for i in 0..800 {
            let event = json!({
                "type": "assistant",
                "isSidechain": true,
                "agentId": agent_id,
                "timestamp": "2026-05-16T10:00:02.000Z",
                "message": {
                    "id": format!("msg_{agent_id}_{i}"),
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": format!("toolu_{agent_id}_{i}"),
                        "name": "TaskCreate",
                        "input": {"subject": format!("Step{i}"), "activeForm": format!("Doing Step{i}")}
                    }]
                }
            });
            append_line(&sub_path, &event.to_string());
        }
    }

    let payload = payload_json(&workspace, &parent, "sub-perf");
    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        max_tool_lines: 2,
        max_agent_lines: 3,
        transcript_window_events: 1000,
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // First tick warms the offsets; subsequent ticks read 0 new bytes per
    // sub-agent — exactly the steady-state case we're protecting.
    let _ = runner
        .run_from_str(&payload, config.clone())
        .expect("first render");

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
        "p95 with sub-agent tailing {:?} exceeded 50ms budget",
        p95
    );
}
