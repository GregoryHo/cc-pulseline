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

// ── Existing flat-format tests (Path 3 backward compat) ──────────────

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
        lines.iter().any(|line| line.contains("T:ReadFile")),
        "tool_use should produce an active tool line"
    );

    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "tool-flow"), config)
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.starts_with("T:")),
        "tool_result should clear active tool lines (completed shows as ✓)"
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
    assert!(
        lines.iter().any(|line| line.contains("T:Bash")),
        "running tool should appear"
    );

    append_line(
        &transcript,
        r#"{"type":"tool_result","tool_use_id":"tool-1"}"#,
    );

    let lines = runner
        .run_from_str(&payload, config.clone())
        .expect("render should succeed");
    assert!(
        lines.iter().any(|line| line.contains("T:Bash")),
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

// ── New tests: nested content[] format (Path 1) ─────────────────────

#[test]
fn tracks_nested_tool_with_target() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("nested-tool.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_nested_tool_flow.jsonl")
        .expect("nested tool fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: assistant message with tool_use Read + input.file_path
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "nested-tool"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read"),
        "nested tool_use should produce running tool: got {joined}"
    );
    assert!(
        joined.contains("/src/main.rs"),
        "target should include file path: got {joined}"
    );

    // Event 1: user message with tool_result → clears tool, records completion
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "nested-tool"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("T:Read"),
        "tool_result should clear running tool line"
    );
    assert!(
        joined.contains("✓ Read"),
        "completed tool count should appear: got {joined}"
    );
    assert!(
        joined.contains("×1"),
        "completed count should be 1: got {joined}"
    );
}

#[test]
fn tracks_nested_multi_block_tools() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("nested-multi.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_nested_multi_block.jsonl")
        .expect("nested multi block fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_tool_lines: 3,
        ..RenderConfig::default()
    };

    // Event 0: assistant message with 2 parallel tool_use blocks
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "nested-multi"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read"),
        "first parallel tool should appear: got {joined}"
    );
    assert!(
        joined.contains("T:Bash"),
        "second parallel tool should appear: got {joined}"
    );
    assert!(
        joined.contains("cargo test"),
        "Bash target should show command: got {joined}"
    );

    // Event 1: tool_results for both → both cleared, both completed
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "nested-multi"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("T:Read") && !joined.contains("T:Bash"),
        "all running tools should clear: got {joined}"
    );
    assert!(
        joined.contains("✓ Read") && joined.contains("✓ Bash"),
        "both completed counts should appear: got {joined}"
    );
}

// ── New tests: agent_progress events (Path 2) ───────────────────────

#[test]
fn tracks_agent_progress_events() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("agent-progress.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_nested_agent_progress.jsonl")
        .expect("agent progress fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: progress → agent_progress (running)
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "agent-progress"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Explore: Investigate L4+ logic"),
        "agent_progress should create agent line with type and description: got {joined}"
    );

    // Event 1: progress → agent_progress (completed)
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "agent-progress"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:Explore"),
        "completed agent_progress should remove agent line: got {joined}"
    );
}

// ── New tests: config toggles ────────────────────────────────────────

#[test]
fn config_disables_agents() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("config-agents.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_agent_flow.jsonl")
        .expect("agent fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        show_agents: false,
        ..RenderConfig::default()
    };

    append_line(&transcript, events[0]); // Add some agents

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "config-agents"),
            config,
        )
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.starts_with("A:")),
        "agents should be hidden when show_agents=false"
    );
}

#[test]
fn config_disables_todo() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("config-todo.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_todo_flow.jsonl")
        .expect("todo fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        show_todo: false,
        ..RenderConfig::default()
    };

    append_line(&transcript, events[0]); // Add todo

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "config-todo"),
            config,
        )
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.starts_with("TODO:")),
        "todo should be hidden when show_todo=false"
    );
}

#[test]
fn config_disables_tools() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("config-tools.jsonl");

    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"tool-1","name":"Bash"}"#,
    );

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        show_tools: false,
        ..RenderConfig::default()
    };

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "config-tools"),
            config,
        )
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.contains("T:Bash")),
        "tools should be hidden when show_tools=false"
    );
}

#[test]
fn loads_default_config_when_missing() {
    // Verifying that PulselineConfig::default() produces sensible values
    use cc_pulseline::config::{build_render_config, PulselineConfig};

    let config = PulselineConfig::default();
    let render = build_render_config(&config);

    assert!(render.show_tools, "tools should be enabled by default");
    assert!(render.show_agents, "agents should be enabled by default");
    assert!(render.show_todo, "todo should be enabled by default");
    assert_eq!(render.max_tool_lines, 2);
    assert_eq!(render.max_completed_tools, 4);
    assert_eq!(render.max_agent_lines, 2);
}

// ── Fixture-based coverage tests ─────────────────────────────────────

#[test]
fn tracks_task_tool_as_agent() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("task-agent.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_nested_task_agent.jsonl")
        .expect("task agent fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: Task tool_use → should appear as agent, not tool
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Architect: Refactor parser"),
        "Task tool_use should create agent line with type and description: got {joined}"
    );
    assert!(
        !joined.contains("T:Task"),
        "Task should not appear as a tool line: got {joined}"
    );

    // Event 1: tool_result → removes agent
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-agent"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:Architect"),
        "tool_result should remove Task-based agent: got {joined}"
    );
}

#[test]
fn extracts_todo_from_tool_result() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("todo-result.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_nested_todo_in_result.jsonl")
        .expect("todo in result fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: Read tool_use → running tool
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "todo-result"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read"),
        "Read tool should be running: got {joined}"
    );

    // Event 1: tool_result with todos[] → Read completed, TODO appears
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "todo-result"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("T:Read"),
        "tool_result should clear running Read tool: got {joined}"
    );
    assert!(
        joined.contains("✓ Read"),
        "completed Read should appear: got {joined}"
    );
    assert!(
        joined.contains("TODO:1/3 done, 2 pending"),
        "todos from tool_result should create TODO line: got {joined}"
    );
}

#[test]
fn handles_snake_case_progress_fields() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("snake-case.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_progress_snake_case.jsonl")
        .expect("snake case fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: agent_id + state + subagent_type (snake_case fields)
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "snake-case"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Detective: Analyze logs"),
        "snake_case fields should work: got {joined}"
    );

    // Event 1: state=completed (snake_case) → removes agent
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "snake-case"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:Detective"),
        "completed via state field should remove agent: got {joined}"
    );
}

#[test]
fn handles_terminal_status_variety() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("terminal-status.jsonl");
    let fixture =
        fs::read_to_string("tests/fixtures/transcript_progress_terminal_statuses.jsonl")
            .expect("terminal status fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_agent_lines: 3,
        ..RenderConfig::default()
    };

    // Events 0-2: three agents running
    append_line(&transcript, events[0]);
    append_line(&transcript, events[1]);
    append_line(&transcript, events[2]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config.clone(),
        )
        .expect("render should succeed");
    let agent_lines: Vec<&String> = lines.iter().filter(|l| l.starts_with("A:")).collect();
    assert_eq!(
        agent_lines.len(),
        3,
        "three agents should be running: got {agent_lines:?}"
    );

    // Event 3: a1 failed
    append_line(&transcript, events[3]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Task A"),
        "failed agent a1 should be removed: got {joined}"
    );
    assert!(
        joined.contains("Task B") && joined.contains("Task C"),
        "agents a2 and a3 should remain: got {joined}"
    );

    // Event 4: a2 cancelled
    append_line(&transcript, events[4]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Task B"),
        "cancelled agent a2 should be removed: got {joined}"
    );
    assert!(
        joined.contains("Task C"),
        "agent a3 should still remain: got {joined}"
    );

    // Event 5: a3 done
    append_line(&transcript, events[5]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Task C"),
        "done agent a3 should be removed: got {joined}"
    );
}

#[test]
fn handles_mixed_three_path_transcript() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("mixed-paths.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_mixed_three_paths.jsonl")
        .expect("mixed paths fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_tool_lines: 3,
        ..RenderConfig::default()
    };

    // Append all 5 events at once
    for event in &events {
        append_line(&transcript, event);
    }

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "mixed-paths"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");

    // Nested Read completed (tool_result in event 3)
    assert!(
        joined.contains("✓ Read"),
        "nested Read should be completed: got {joined}"
    );
    // Agent completed (progress event 4)
    assert!(
        !joined.contains("A:Explore"),
        "agent should be removed after completed: got {joined}"
    );
    // Flat Bash still running (no tool_result for it)
    assert!(
        joined.contains("T:Bash"),
        "flat-format Bash should still be running: got {joined}"
    );
}

#[test]
fn task_tool_defaults_missing_fields() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("task-bare.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_nested_task_no_fields.jsonl")
        .expect("task no fields fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: Task with empty input → defaults to "Task" description, no agent_type
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-bare"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Task"),
        "Task with missing fields should default to 'Task' description: got {joined}"
    );
    assert!(
        !joined.contains("T:Task"),
        "Task should never appear as tool line: got {joined}"
    );

    // Event 1: tool_result removes the agent
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-bare"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:Task"),
        "tool_result should remove default Task agent: got {joined}"
    );
}
