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
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:ReadFile"),
        "tool should persist in recent display after completion"
    );
    assert!(
        joined.contains("✓"),
        "completed tool count should appear: got {joined}"
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
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Bash"),
        "tool should persist in recent display even after tool_result: got {joined}"
    );
    assert!(
        joined.contains("✓"),
        "completed tool count should appear: got {joined}"
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

    // Event 1: user message with tool_result → tool persists in recent, records completion
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "nested-tool"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read"),
        "tool should persist in recent display after completion: got {joined}"
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

    // Event 1: tool_results for both → both persist in recent, both completed
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "nested-multi"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read") && joined.contains("T:Bash"),
        "tools should persist in recent display after completion: got {joined}"
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

    // Event 1: progress → agent_progress (completed) → shows as done
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "agent-progress"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Explore: Investigate L4+ logic [done]"),
        "completed agent_progress should show as done: got {joined}"
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

    // Event 0: Task tool_use → pushes to pending queue, no agent visible yet
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("T:Task"),
        "Task should not appear as a tool line: got {joined}"
    );

    // Event 1: agent_progress → links to pending Task, creates agent with Task's description
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Architect: Refactor parser"),
        "agent_progress should link to Task and use its description: got {joined}"
    );

    // Event 2: tool_result → agent becomes completed with [done] tag
    append_line(&transcript, events[2]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "task-agent"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Architect: Refactor parser [done]"),
        "tool_result should show completed Task agent with [done]: got {joined}"
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

    // Event 1: tool_result with todos[] → Read persists in recent, completed + TODO appear
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "todo-result"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read"),
        "tool should persist in recent display after completion: got {joined}"
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

    // Event 1: state=completed (snake_case) → agent shows as done
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "snake-case"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Detective: Analyze logs [done]"),
        "completed via state field should show [done]: got {joined}"
    );
}

#[test]
fn handles_terminal_status_variety() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("terminal-status.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_progress_terminal_statuses.jsonl")
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

    // Event 3: a1 failed → becomes completed [done]
    append_line(&transcript, events[3]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Task A") && joined.contains("[done]"),
        "failed agent a1 should show as done: got {joined}"
    );
    assert!(
        joined.contains("Task B") && joined.contains("Task C"),
        "agents a2 and a3 should remain: got {joined}"
    );

    // Event 4: a2 cancelled → becomes completed [done]
    append_line(&transcript, events[4]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Task B") && joined.contains("[done]"),
        "cancelled agent a2 should show as done: got {joined}"
    );
    assert!(
        joined.contains("Task C"),
        "agent a3 should still remain: got {joined}"
    );

    // Event 5: a3 done → all three completed
    append_line(&transcript, events[5]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "terminal-status"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Task C") && joined.contains("[done]"),
        "done agent a3 should show as done: got {joined}"
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
    // Agent completed (progress event 4) → shows as done
    assert!(
        joined.contains("[done]") && joined.contains("Explore"),
        "completed agent should show as done: got {joined}"
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

    // Event 0: Task with empty input → pushes to pending queue, no agent visible yet
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-bare"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("T:Task"),
        "Task should never appear as tool line: got {joined}"
    );

    // Event 1: tool_result → drains pending, creates+completes agent inline with [done]
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "task-bare"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Task") && joined.contains("[done]"),
        "tool_result should drain pending and show default Task agent as done: got {joined}"
    );
}

// ── Fix 1: Agent linking tests ──────────────────────────────────────

#[test]
fn links_agent_progress_to_task_tool_use() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("real-agent.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_real_agent_lifecycle.jsonl")
        .expect("real agent lifecycle fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: Task tool_use → pending queue, no agent visible
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "real-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:"),
        "Task tool_use should not create visible agent yet: got {joined}"
    );

    // Event 1: first agent_progress → links to pending, creates single agent
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "real-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Explore agent tracking code"),
        "linked agent should use Task's description, not prompt: got {joined}"
    );
    assert!(
        joined.contains("A:Explore"),
        "linked agent should show agent type: got {joined}"
    );
    // Must be exactly one agent line — no duplicate
    let agent_lines: Vec<&String> = lines.iter().filter(|l| l.starts_with("A:")).collect();
    assert_eq!(
        agent_lines.len(),
        1,
        "should be exactly one agent (no duplicate): got {agent_lines:?}"
    );

    // Event 2: second agent_progress → should NOT overwrite description
    append_line(&transcript, events[2]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "real-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Explore agent tracking code"),
        "description should be preserved from Task, not overwritten by prompt: got {joined}"
    );

    // Event 3: tool_result → agent completes via linked ID
    append_line(&transcript, events[3]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "real-agent"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Explore agent tracking code") && joined.contains("[done]"),
        "tool_result should complete the linked agent: got {joined}"
    );
}

#[test]
fn links_concurrent_agents_fifo() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("concurrent-agents.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_real_concurrent_agents.jsonl")
        .expect("concurrent agents fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_agent_lines: 4,
        ..RenderConfig::default()
    };

    // Event 0: Two Task tool_uses in one message → two pending tasks
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "concurrent-agents"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:"),
        "no agents should be visible yet: got {joined}"
    );

    // Events 1-2: agent_progress for each → FIFO linking
    append_line(&transcript, events[1]);
    append_line(&transcript, events[2]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "concurrent-agents"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Explore: Search for config files"),
        "first agent should get first Task's description: got {joined}"
    );
    assert!(
        joined.contains("A:Bash: Run test suite"),
        "second agent should get second Task's description: got {joined}"
    );
    let agent_lines: Vec<&String> = lines.iter().filter(|l| l.starts_with("A:")).collect();
    assert_eq!(
        agent_lines.len(),
        2,
        "should be exactly two agents: got {agent_lines:?}"
    );

    // Event 3: first tool_result → first agent completes
    append_line(&transcript, events[3]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "concurrent-agents"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Search for config files") && joined.contains("[done]"),
        "first agent should be completed: got {joined}"
    );
    assert!(
        joined.contains("Run test suite") && !joined.contains("Run test suite [done]"),
        "second agent should still be running: got {joined}"
    );

    // Event 4: second tool_result → second agent completes
    append_line(&transcript, events[4]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "concurrent-agents"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Run test suite") && joined.contains("[done]"),
        "second agent should now be completed: got {joined}"
    );
}

#[test]
fn standalone_agent_progress_without_task() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("standalone-agent.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // agent_progress with no preceding Task tool_use → standalone agent
    append_line(
        &transcript,
        r#"{"type":"progress","data":{"type":"agent_progress","agentId":"standalone-1","prompt":"Investigate the bug","agentType":"Explore"}}"#,
    );

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "standalone-agent"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Explore: Investigate the bug"),
        "standalone agent_progress should use prompt as description: got {joined}"
    );

    // Completed via status
    append_line(
        &transcript,
        r#"{"type":"progress","data":{"type":"agent_progress","agentId":"standalone-1","status":"completed"}}"#,
    );

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "standalone-agent"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Investigate the bug") && joined.contains("[done]"),
        "standalone agent should complete via status: got {joined}"
    );
}

#[test]
fn task_completes_without_agent_progress() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("task-no-progress.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Task tool_use → pending queue
    append_line(
        &transcript,
        r#"{"timestamp":"2026-01-18T10:50:00.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_fast","name":"Task","input":{"description":"Quick lookup","subagent_type":"Explore"}}]}}"#,
    );

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-no-progress"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("A:"),
        "no agent visible during pending: got {joined}"
    );

    // tool_result with NO agent_progress in between → drain pending, create+complete
    append_line(
        &transcript,
        r#"{"timestamp":"2026-01-18T10:50:05.000Z","content":[{"type":"tool_result","tool_use_id":"toolu_fast"}]}"#,
    );

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-no-progress"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("A:Explore: Quick lookup") && joined.contains("[done]"),
        "task should appear as completed agent via drain_pending_task: got {joined}"
    );
}

// ── Fix 2: Todo tracking tests ──────────────────────────────────────

#[test]
fn tracks_task_create_and_update_as_todo() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("task-todo.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_real_task_todo.jsonl")
        .expect("real task todo fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Events 0-2: Three TaskCreates → pending only, shows "3 tasks (0/3)"
    append_line(&transcript, events[0]);
    append_line(&transcript, events[1]);
    append_line(&transcript, events[2]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-todo"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("TODO:3 tasks") && joined.contains("(0/3)"),
        "3 TaskCreates should show TODO:3 tasks (0/3): got {joined}"
    );

    // Event 3: TaskUpdate task 1 → in_progress → shows active_form text
    append_line(&transcript, events[3]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-todo"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Fixing authentication bug") && joined.contains("(0/3)"),
        "in_progress should show active_form text with progress: got {joined}"
    );

    // Event 4: TaskUpdate task 1 → completed
    append_line(&transcript, events[4]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-todo"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("TODO:") && joined.contains("(1/3)"),
        "completing task 1 should show (1/3): got {joined}"
    );

    // Event 5: TaskUpdate task 2 → completed
    append_line(&transcript, events[5]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-todo"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("TODO:") && joined.contains("(2/3)"),
        "completing task 2 should show (2/3): got {joined}"
    );

    // Event 6: TaskUpdate task 3 → completed (all done → celebration line)
    append_line(&transcript, events[6]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "task-todo"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("All todos complete") && joined.contains("(3/3)"),
        "all tasks completed should show celebration line: got {joined}"
    );
}

#[test]
fn task_update_delete_removes_from_count() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("task-delete.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_real_task_todo.jsonl")
        .expect("real task todo fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Events 0-6: Create 3 tasks, complete all → all-done celebration
    for event in &events[..7] {
        append_line(&transcript, event);
    }
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-delete"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("All todos complete") && joined.contains("(3/3)"),
        "all completed should show celebration line: got {joined}"
    );

    // Event 7: TaskUpdate task 2 → deleted → now 2/2 done → still all-done
    append_line(&transcript, events[7]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "task-delete"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("All todos complete") && joined.contains("(2/2)"),
        "deleting a completed task should still show all-done: got {joined}"
    );
}

#[test]
fn old_todowrite_format_still_works() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("old-todo.jsonl");
    let fixture = fs::read_to_string("tests/fixtures/transcript_todo_flow.jsonl")
        .expect("old todo fixture should exist");
    let events: Vec<&str> = fixture.lines().collect();

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Event 0: Old-format TodoWrite with todos[] array
    append_line(&transcript, events[0]);
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "old-todo"),
            config.clone(),
        )
        .expect("render should succeed");
    assert!(
        lines.iter().any(|line| line == "TODO:1/3 done, 2 pending"),
        "old TodoWrite format should still work"
    );

    // Event 1: Old-format TaskUpdate with todos[] array (all completed)
    append_line(&transcript, events[1]);
    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "old-todo"), config)
        .expect("render should succeed");
    assert!(
        lines.iter().all(|line| !line.starts_with("TODO:")),
        "old TaskUpdate format that completes all should clear TODO line"
    );
}

// ── New tests: Recent tools display ─────────────────────────────────

#[test]
fn recent_tools_persist_after_completion() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("recent-persist.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_tool_lines: 2,
        ..RenderConfig::default()
    };

    // tool_use Read
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"/src/main.rs"}}]}}"#,
    );
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "recent-persist"),
            config.clone(),
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read") && joined.contains("/src/main.rs"),
        "running tool should appear with target: got {joined}"
    );

    // tool_result Read → tool persists in recent display
    append_line(
        &transcript,
        r#"{"content":[{"type":"tool_result","tool_use_id":"t1"}]}"#,
    );
    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "recent-persist"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read") && joined.contains("/src/main.rs"),
        "tool should persist in recent display after completion: got {joined}"
    );
}

#[test]
fn recent_tools_displaced_by_newer() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("recent-displace.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_tool_lines: 2,
        ..RenderConfig::default()
    };

    // Three tool_uses: Read, Write, Bash (with max_tool_lines=2, Read displaced)
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"/src/main.rs"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"Write","input":{"file_path":"/src/lib.rs"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"t3","name":"Bash","input":{"command":"cargo test"}}]}}"#,
    );

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "recent-displace"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        !joined.contains("T:Read"),
        "oldest tool (Read) should be displaced by newer: got {joined}"
    );
    assert!(
        joined.contains("T:Write"),
        "second tool should still be visible: got {joined}"
    );
    assert!(
        joined.contains("T:Bash"),
        "newest tool should be visible: got {joined}"
    );
}

#[test]
fn recent_tools_cleared_on_transcript_change() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript1 = workspace.path().join("transcript-1.jsonl");
    let transcript2 = workspace.path().join("transcript-2.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Add tool to first transcript
    append_line(
        &transcript1,
        r#"{"type":"tool_use","tool_use_id":"t1","name":"Read"}"#,
    );
    let payload1 = json!({
        "session_id": "session-change",
        "cwd": workspace.path(),
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus"},
        "version": "2.2.0",
        "transcript_path": transcript1,
    })
    .to_string();

    let lines = runner
        .run_from_str(&payload1, config.clone())
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("T:Read"),
        "tool should appear: got {joined}"
    );

    // Switch transcript path → tools should clear
    fs::write(&transcript2, "").expect("create empty transcript");
    let payload2 = json!({
        "session_id": "session-change",
        "cwd": workspace.path(),
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus"},
        "version": "2.2.0",
        "transcript_path": transcript2,
    })
    .to_string();

    let lines = runner
        .run_from_str(&payload2, config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("T:Read"),
        "tools should clear on transcript path change: got {joined}"
    );
}

// ── New tests: Rich TODO display ────────────────────────────────────

#[test]
fn todo_rich_display_in_progress() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("todo-rich.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // TaskCreate ×3
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc1","name":"TaskCreate","input":{"subject":"Fix auth bug","activeForm":"Fixing auth bug"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc2","name":"TaskCreate","input":{"subject":"Add tests","activeForm":"Adding tests"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc3","name":"TaskCreate","input":{"subject":"Update docs","activeForm":"Updating docs"}}]}}"#,
    );

    // TaskUpdate task 1 → in_progress
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu1","name":"TaskUpdate","input":{"taskId":"1","status":"in_progress"}}]}}"#,
    );

    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "todo-rich"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("Fixing auth bug") && joined.contains("(0/3)"),
        "in_progress task should show active_form text with progress: got {joined}"
    );
}

#[test]
fn todo_all_done_shows_checkmark() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("todo-done.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Create 2 tasks, complete both
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc1","name":"TaskCreate","input":{"subject":"Task A"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc2","name":"TaskCreate","input":{"subject":"Task B"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu1","name":"TaskUpdate","input":{"taskId":"1","status":"completed"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu2","name":"TaskUpdate","input":{"taskId":"2","status":"completed"}}]}}"#,
    );

    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "todo-done"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("All todos complete") && joined.contains("(2/2)"),
        "all-done should show celebration line: got {joined}"
    );
}

#[test]
fn todo_pending_only_shows_task_count() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("todo-pending.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Create 3 tasks, no updates
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc1","name":"TaskCreate","input":{"subject":"Task 1"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc2","name":"TaskCreate","input":{"subject":"Task 2"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc3","name":"TaskCreate","input":{"subject":"Task 3"}}]}}"#,
    );

    let lines = runner
        .run_from_str(
            &payload_json(&workspace, &transcript, "todo-pending"),
            config,
        )
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        joined.contains("TODO:3 tasks") && joined.contains("(0/3)"),
        "pending-only should show task count format: got {joined}"
    );
}

#[test]
fn todo_max_lines_caps_display() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("todo-cap.jsonl");

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        max_todo_lines: 2,
        ..RenderConfig::default()
    };

    // Create 3 tasks, set all to in_progress
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc1","name":"TaskCreate","input":{"subject":"Task A","activeForm":"Working on A"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc2","name":"TaskCreate","input":{"subject":"Task B","activeForm":"Working on B"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tc3","name":"TaskCreate","input":{"subject":"Task C","activeForm":"Working on C"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu1","name":"TaskUpdate","input":{"taskId":"1","status":"in_progress"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu2","name":"TaskUpdate","input":{"taskId":"2","status":"in_progress"}}]}}"#,
    );
    append_line(
        &transcript,
        r#"{"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu3","name":"TaskUpdate","input":{"taskId":"3","status":"in_progress"}}]}}"#,
    );

    let lines = runner
        .run_from_str(&payload_json(&workspace, &transcript, "todo-cap"), config)
        .expect("render should succeed");

    let todo_lines: Vec<&String> = lines.iter().filter(|l| l.starts_with("TODO:")).collect();
    assert_eq!(
        todo_lines.len(),
        2,
        "should be capped at max_todo_lines=2: got {todo_lines:?}"
    );

    // First line should include progress indicator
    let first = &todo_lines[0];
    assert!(
        first.contains("(0/3"),
        "first line should include progress indicator: got {first}"
    );
    assert!(
        first.contains("3 active"),
        "first line should show overflow count when more active than shown: got {first}"
    );
}
