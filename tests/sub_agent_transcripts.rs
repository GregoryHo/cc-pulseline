//! Sub-agent transcript tailing.
//!
//! Verifies that when the parent transcript dispatches an Agent (Task tool)
//! and CC's `toolUseResult` carries `status: "async_launched"` + an
//! `agentId`, cc-pulseline tails
//! `<parent>/subagents/agent-<id>.jsonl` and aggregates the sub-agent's
//! TODO state into the rendered statusline.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use cc_pulseline::{config::RenderConfig, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

fn append_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should be creatable");
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("transcript file should open");
    writeln!(file, "{line}").expect("line should append");
}

fn payload_json(workspace: &TempDir, transcript_path: &Path, session_id: &str) -> String {
    json!({
        "session_id": session_id,
        "cwd": workspace.path(),
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.2.0",
        "transcript_path": transcript_path,
        "cost": {"total_cost_usd": 1.0, "total_duration_ms": 60000}
    })
    .to_string()
}

fn agent_dispatch_event(tool_use_id: &str, description: &str) -> String {
    json!({
        "type": "assistant",
        "timestamp": "2026-05-16T10:00:00.000Z",
        "message": {
            "id": "msg_test1",
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": tool_use_id,
                "name": "Agent",
                "input": {
                    "description": description,
                    "subagent_type": "general-purpose",
                    "prompt": "Do some work."
                }
            }]
        }
    })
    .to_string()
}

fn async_launched_event(tool_use_id: &str, agent_id: &str) -> String {
    json!({
        "type": "user",
        "timestamp": "2026-05-16T10:00:01.000Z",
        "message": {
            "role": "user",
            "content": [{
                "tool_use_id": tool_use_id,
                "type": "tool_result",
                "content": [{"type": "text", "text": "Async agent launched successfully."}]
            }]
        },
        "toolUseResult": {
            "isAsync": true,
            "status": "async_launched",
            "agentId": agent_id
        }
    })
    .to_string()
}

fn agent_completed_event(tool_use_id: &str, agent_id: &str) -> String {
    json!({
        "type": "user",
        "timestamp": "2026-05-16T10:05:00.000Z",
        "message": {
            "role": "user",
            "content": [{
                "tool_use_id": tool_use_id,
                "type": "tool_result",
                "content": [{"type": "text", "text": "Agent completed."}]
            }]
        },
        "toolUseResult": {
            "isAsync": true,
            "status": "completed",
            "agentId": agent_id
        }
    })
    .to_string()
}

fn task_create_event(agent_id: &str, subject: &str, active_form: &str) -> String {
    json!({
        "type": "assistant",
        "isSidechain": true,
        "agentId": agent_id,
        "timestamp": "2026-05-16T10:00:02.000Z",
        "message": {
            "id": "msg_sub1",
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": format!("toolu_sub_{agent_id}_{subject}"),
                "name": "TaskCreate",
                "input": {"subject": subject, "activeForm": active_form}
            }]
        }
    })
    .to_string()
}

fn sub_agent_path(parent_transcript: &Path, agent_id: &str) -> std::path::PathBuf {
    let stem = parent_transcript
        .file_stem()
        .expect("parent has stem")
        .to_str()
        .expect("stem utf8");
    parent_transcript
        .parent()
        .expect("parent dir")
        .join(stem)
        .join("subagents")
        .join(format!("agent-{agent_id}.jsonl"))
}

#[test]
fn surfaces_todo_from_a_single_sub_agent() {
    let workspace = TempDir::new().expect("temp workspace");
    let parent = workspace.path().join("parent.jsonl");
    let agent_id = "a1234567890abcdef";
    let tool_use_id = "toolu_async_one";

    append_line(&parent, &agent_dispatch_event(tool_use_id, "Investigate"));
    append_line(&parent, &async_launched_event(tool_use_id, agent_id));

    let sub_path = sub_agent_path(&parent, agent_id);
    append_line(
        &sub_path,
        &task_create_event(agent_id, "Step1", "Doing step 1"),
    );
    append_line(
        &sub_path,
        &task_create_event(agent_id, "Step2", "Doing step 2"),
    );
    append_line(
        &sub_path,
        &task_create_event(agent_id, "Step3", "Doing step 3"),
    );

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };
    let lines = runner
        .run_from_str(&payload_json(&workspace, &parent, "sub-one"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        joined.contains("TODO"),
        "expected a TODO line surfacing sub-agent task counts: {joined}"
    );
    assert!(
        joined.contains("3 tasks") && joined.contains("(0/3)"),
        "expected aggregated 0/3 from sub-agent's TaskCreate calls: {joined}"
    );
    assert!(
        joined.contains("(1 agent)"),
        "TODO line should annotate sub-agent count: {joined}"
    );
}

#[test]
fn drops_sub_agent_state_when_agent_completes() {
    let workspace = TempDir::new().expect("temp workspace");
    let parent = workspace.path().join("parent.jsonl");
    let agent_id = "a2222222222222222";
    let tool_use_id = "toolu_async_two";

    append_line(&parent, &agent_dispatch_event(tool_use_id, "Sweep"));
    append_line(&parent, &async_launched_event(tool_use_id, agent_id));
    let sub_path = sub_agent_path(&parent, agent_id);
    append_line(
        &sub_path,
        &task_create_event(agent_id, "OnlyStep", "Doing OnlyStep"),
    );

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // First tick: sub-agent TODO surfaces.
    let lines = runner
        .run_from_str(&payload_json(&workspace, &parent, "sub-gc"), config.clone())
        .expect("render should succeed");
    let joined_first = lines.join("\n");
    assert!(
        joined_first.contains("(0/1)") && joined_first.contains("(1 agent)"),
        "expected sub-agent's 0/1 initially: {lines:?}"
    );

    // Parent gets the terminal toolUseResult → sub-agent state should be GC'd.
    append_line(&parent, &agent_completed_event(tool_use_id, agent_id));
    let lines = runner
        .run_from_str(&payload_json(&workspace, &parent, "sub-gc"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");
    assert!(
        !joined.contains("TODO"),
        "TODO line should disappear once the only sub-agent completes: {joined}"
    );
}

#[test]
fn aggregates_todo_across_two_concurrent_sub_agents() {
    let workspace = TempDir::new().expect("temp workspace");
    let parent = workspace.path().join("parent.jsonl");

    let agent_a = "aaaaaaaaaaaaaaaaa";
    let agent_b = "bbbbbbbbbbbbbbbbb";
    let tuid_a = "toolu_async_a";
    let tuid_b = "toolu_async_b";

    append_line(&parent, &agent_dispatch_event(tuid_a, "Branch A"));
    append_line(&parent, &async_launched_event(tuid_a, agent_a));
    append_line(&parent, &agent_dispatch_event(tuid_b, "Branch B"));
    append_line(&parent, &async_launched_event(tuid_b, agent_b));

    let sub_a = sub_agent_path(&parent, agent_a);
    append_line(&sub_a, &task_create_event(agent_a, "A1", "Doing A1"));
    append_line(&sub_a, &task_create_event(agent_a, "A2", "Doing A2"));

    let sub_b = sub_agent_path(&parent, agent_b);
    append_line(&sub_b, &task_create_event(agent_b, "B1", "Doing B1"));
    append_line(&sub_b, &task_create_event(agent_b, "B2", "Doing B2"));
    append_line(&sub_b, &task_create_event(agent_b, "B3", "Doing B3"));

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };
    let lines = runner
        .run_from_str(&payload_json(&workspace, &parent, "sub-agg"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");

    assert!(
        joined.contains("5 tasks") && joined.contains("(0/5)"),
        "expected aggregated 0/5 across two sub-agents (2 + 3 tasks): {joined}"
    );
    assert!(
        joined.contains("(2 agents)"),
        "TODO line should annotate two sub-agents: {joined}"
    );
}

#[test]
fn parent_owned_todo_wins_over_sub_agent_todo() {
    // When the parent session has its own TODO in flight, aggregation
    // should NOT replace it with sub-agent aggregate — the user's own
    // TodoWrite/TaskCreate stays authoritative.
    let workspace = TempDir::new().expect("temp workspace");
    let parent = workspace.path().join("parent.jsonl");
    let agent_id = "a3333333333333333";
    let tuid = "toolu_async_three";

    // Parent does its own TaskCreate first.
    let parent_task = json!({
        "type": "assistant",
        "timestamp": "2026-05-16T09:59:00.000Z",
        "message": {
            "id": "msg_parent",
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "toolu_parent_task",
                "name": "TaskCreate",
                "input": {"subject": "ParentTask", "activeForm": "Doing ParentTask"}
            }]
        }
    });
    append_line(&parent, &parent_task.to_string());

    // Then dispatches a sub-agent with its own tasks.
    append_line(&parent, &agent_dispatch_event(tuid, "Sub work"));
    append_line(&parent, &async_launched_event(tuid, agent_id));
    let sub_path = sub_agent_path(&parent, agent_id);
    append_line(
        &sub_path,
        &task_create_event(agent_id, "SubStep1", "Doing SubStep1"),
    );
    append_line(
        &sub_path,
        &task_create_event(agent_id, "SubStep2", "Doing SubStep2"),
    );

    let mut runner = PulseLineRunner::default();
    let config = RenderConfig {
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };
    let lines = runner
        .run_from_str(&payload_json(&workspace, &parent, "sub-precedence"), config)
        .expect("render should succeed");
    let joined = lines.join("\n");

    // Parent's "1 tasks (0/1)" should appear — NOT the aggregated 0/3.
    assert!(
        joined.contains("1 tasks") && joined.contains("(0/1)"),
        "parent's own TODO should win when present: {joined}"
    );
    assert!(
        !joined.contains("(1 agent)") && !joined.contains("(2 agents)"),
        "parent-owned TODO should not be annotated with sub-agent count: {joined}"
    );
}
