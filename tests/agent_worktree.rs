use cc_pulseline::{
    config::RenderConfig,
    types::{RenderFrame, StdinPayload},
};
use serde_json::json;

fn make_payload_with_agent(agent_name: &str) -> StdinPayload {
    let input = json!({
        "session_id": "test",
        "version": "2.1.80",
        "model": {"display_name": "Opus"},
        "agent": {"name": agent_name, "type": "custom"}
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

fn make_payload_with_worktree() -> StdinPayload {
    let input = json!({
        "session_id": "test",
        "version": "2.1.80",
        "model": {"display_name": "Opus"},
        "worktree": {
            "name": "fix-bug",
            "branch": "fix-auth",
            "original_branch": "main",
            "path": "/tmp/worktree"
        }
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

fn make_plain_payload() -> StdinPayload {
    let input = json!({
        "session_id": "test",
        "version": "2.1.80",
        "model": {"display_name": "Opus"}
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

// ── Agent tests ──────────────────────────────────────────────────────

#[test]
fn agent_name_displayed_on_l1() {
    let payload = make_payload_with_agent("code-reviewer");
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_agent: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        l1.contains("AG:code-reviewer"),
        "L1 should contain agent name, got: {l1}"
    );
}

#[test]
fn agent_hidden_when_absent() {
    let payload = make_plain_payload();
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_agent: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        !l1.contains("AG:"),
        "L1 should not contain AG: when no agent, got: {l1}"
    );
}

#[test]
fn agent_hidden_when_toggle_off() {
    let payload = make_payload_with_agent("code-reviewer");
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_agent: false,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        !l1.contains("AG:"),
        "L1 should not contain AG: when toggle off, got: {l1}"
    );
}

// ── Worktree tests ───────────────────────────────────────────────────

#[test]
fn worktree_indicator_appended_to_git() {
    let payload = make_payload_with_worktree();
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        l1.contains("(WT)"),
        "L1 should contain worktree indicator, got: {l1}"
    );
}

#[test]
fn worktree_hidden_when_not_in_worktree() {
    let payload = make_plain_payload();
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        !l1.contains("(WT)"),
        "L1 should not contain WT when not in worktree, got: {l1}"
    );
}

#[test]
fn worktree_hidden_when_toggle_off() {
    let payload = make_payload_with_worktree();
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: false,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        !l1.contains("(WT)"),
        "L1 should not contain WT when toggle off, got: {l1}"
    );
}

// ── Worktree project path resolution ─────────────────────────────────

#[test]
fn worktree_original_cwd_used_for_project_path() {
    let input = json!({
        "session_id": "test",
        "workspace": {"current_dir": "/tmp/worktree-abc"},
        "worktree": {
            "name": "fix",
            "original_cwd": "/home/user/myproject",
            "original_branch": "main"
        }
    })
    .to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();

    assert_eq!(
        payload.resolve_project_path().as_deref(),
        Some("/home/user/myproject"),
        "worktree.original_cwd should take priority over workspace.current_dir"
    );
}

#[test]
fn non_worktree_uses_workspace_current_dir() {
    let input = json!({
        "session_id": "test",
        "workspace": {"current_dir": "/home/user/myproject"}
    })
    .to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();

    assert_eq!(
        payload.resolve_project_path().as_deref(),
        Some("/home/user/myproject"),
        "without worktree, workspace.current_dir should be used"
    );
}

// ── Combined + stdin parsing tests ───────────────────────────────────

#[test]
fn agent_and_worktree_from_stdin() {
    let input = json!({
        "session_id": "test",
        "version": "2.1.80",
        "model": {"display_name": "Opus"},
        "agent": {"name": "feature-dev", "type": "custom"},
        "worktree": {
            "name": "epic",
            "branch": "epic-branch",
            "original_branch": "main"
        }
    })
    .to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let frame = RenderFrame::from_payload(&payload);

    assert_eq!(frame.line1.agent_name.as_deref(), Some("feature-dev"));
    assert!(frame.line1.in_worktree);

    let config = RenderConfig {
        show_agent: true,
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];
    assert!(
        l1.contains("AG:feature-dev"),
        "should show agent, got: {l1}"
    );
    assert!(l1.contains("(WT)"), "should show worktree, got: {l1}");
}
