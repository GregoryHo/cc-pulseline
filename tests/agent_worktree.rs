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

#[test]
fn agent_pill_uses_head_agent_color_distinct_from_model() {
    // Regression test for the head_agent palette field (commit b of the
    // HEAD-pills cleanup). Before the rewire, AG: borrowed `stable_blue`
    // — same as `M:` — making the two pills indistinguishable on L1.
    // The rewire routes AG: through `head_agent` (default fallback:
    // active_purple). If a future change collapses head_agent back onto
    // stable_blue, this test fails.
    let payload = make_payload_with_agent("code-reviewer");
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_agent: true,
        color_enabled: true,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    let l1 = &lines[0];

    // Default theme (tokyo-night dark): stable_blue=111, head_agent=183.
    // Both pills carry an ANSI prefix `\x1b[38;5;<code>m` directly before
    // their label glyph.
    let m_idx = l1.find("M:").expect("M: pill present");
    let ag_idx = l1.find("AG:").expect("AG: pill present");
    let m_prefix = &l1[..m_idx];
    let ag_prefix = &l1[..ag_idx];
    assert!(
        m_prefix.ends_with("\x1b[38;5;111m"),
        "M: should be stable_blue (111); got prefix: {m_prefix:?}"
    );
    assert!(
        ag_prefix.ends_with("\x1b[38;5;183m"),
        "AG: should be head_agent (default 183, active_purple); got prefix: {ag_prefix:?}"
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

// ── Passive worktree detection via workspace.git_worktree (CC 2.1.97+) ──
// These tests cover the case where the user enters a git worktree manually
// (not via `claude --worktree`), so `payload.worktree` is absent but
// `payload.workspace.git_worktree` is set.

fn make_payload_with_passive_worktree(value: serde_json::Value) -> StdinPayload {
    let input = json!({
        "session_id": "test",
        "version": "2.1.97",
        "model": {"display_name": "Opus"},
        "workspace": {
            "current_dir": "/tmp/linked-worktree",
            "git_worktree": value
        }
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

#[test]
fn passive_git_worktree_bool_true_triggers_indicator() {
    let payload = make_payload_with_passive_worktree(json!(true));
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    assert!(
        lines[0].contains("(WT)"),
        "bool true should trigger WT indicator, got: {}",
        lines[0]
    );
}

#[test]
fn passive_git_worktree_object_triggers_indicator() {
    // CC schema may evolve to emit an object; any non-null value counts.
    let payload = make_payload_with_passive_worktree(json!({"branch": "feature"}));
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    assert!(lines[0].contains("(WT)"));
}

#[test]
fn passive_git_worktree_null_does_not_trigger() {
    let payload = make_payload_with_passive_worktree(json!(null));
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    assert!(!lines[0].contains("(WT)"));
}

#[test]
fn explicit_worktree_overrides_absent_passive_field() {
    // --worktree session still works when workspace.git_worktree is absent.
    let payload = make_payload_with_worktree();
    let frame = RenderFrame::from_payload(&payload);
    let config = RenderConfig {
        show_worktree: true,
        show_git: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = cc_pulseline::render::layout::render_frame(&frame, &config);
    assert!(lines[0].contains("(WT)"));
}

#[test]
fn is_in_worktree_helper_reflects_both_signals() {
    // Neither → false
    let neither: StdinPayload = serde_json::from_str(r#"{}"#).unwrap();
    assert!(!neither.is_in_worktree());

    // Only --worktree → true
    let explicit = make_payload_with_worktree();
    assert!(explicit.is_in_worktree());

    // Only passive → true
    let passive = make_payload_with_passive_worktree(json!(true));
    assert!(passive.is_in_worktree());

    // Both → true
    let both: StdinPayload =
        serde_json::from_str(r#"{"worktree":{"name":"x"},"workspace":{"git_worktree":true}}"#)
            .unwrap();
    assert!(both.is_in_worktree());
}
