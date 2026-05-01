//! L1 effort + thinking pill rendering (CC 2.1.119+).
//!
//! Validates the new stdin fields `effort.level` and `thinking.enabled`
//! flow through `RenderFrame` into the Line 1 output with correct toggles.

use cc_pulseline::{config::RenderConfig, run_from_str};
use serde_json::json;

fn payload(effort: Option<&str>, thinking: Option<bool>) -> String {
    let mut root = serde_json::Map::new();
    root.insert("session_id".into(), json!("test"));
    if let Some(level) = effort {
        root.insert("effort".into(), json!({"level": level}));
    }
    if let Some(enabled) = thinking {
        root.insert("thinking".into(), json!({"enabled": enabled}));
    }
    serde_json::to_string(&serde_json::Value::Object(root)).expect("payload serializes")
}

fn render_default(input: &str) -> Vec<String> {
    // RenderConfig::default() has color_enabled=false, glyph_mode=Ascii →
    // effort pill renders as "E:<level>", thinking pill renders as "[T]".
    run_from_str(input, RenderConfig::default()).expect("render succeeds")
}

fn line1(lines: &[String]) -> &str {
    lines.first().expect("at least one line").as_str()
}

#[test]
fn effort_high_renders_on_line1() {
    let lines = render_default(&payload(Some("high"), None));
    assert!(
        line1(&lines).contains("E:high"),
        "expected E:high pill, got: {}",
        line1(&lines)
    );
}

#[test]
fn effort_xhigh_renders_with_passthrough_label() {
    let lines = render_default(&payload(Some("xhigh"), None));
    assert!(line1(&lines).contains("E:xhigh"));
}

#[test]
fn unknown_effort_level_still_renders() {
    // Future-proofing: Claude Code may add new levels (e.g. `ultraplus`).
    // The pill should render the raw string rather than dropping it silently.
    let lines = render_default(&payload(Some("ultraplus"), None));
    assert!(line1(&lines).contains("E:ultraplus"));
}

#[test]
fn missing_effort_omits_pill() {
    let lines = render_default(&payload(None, None));
    assert!(!line1(&lines).contains("E:"));
}

#[test]
fn thinking_enabled_true_renders_pill() {
    let lines = render_default(&payload(None, Some(true)));
    assert!(
        line1(&lines).contains("[T]"),
        "expected [T] pill, got: {}",
        line1(&lines)
    );
}

#[test]
fn thinking_enabled_false_hides_pill() {
    let lines = render_default(&payload(None, Some(false)));
    assert!(!line1(&lines).contains("[T]"));
}

#[test]
fn missing_thinking_hides_pill() {
    let lines = render_default(&payload(None, None));
    assert!(!line1(&lines).contains("[T]"));
}

#[test]
fn both_pills_render_in_order() {
    // Effort appears before thinking on L1.
    let lines = render_default(&payload(Some("medium"), Some(true)));
    let l1 = line1(&lines);
    let effort_pos = l1.find("E:medium").expect("effort pill present");
    let thinking_pos = l1.find("[T]").expect("thinking pill present");
    assert!(
        effort_pos < thinking_pos,
        "effort should precede thinking on L1: {}",
        l1
    );
}

#[test]
fn show_effort_toggle_hides_pill() {
    let config = RenderConfig {
        show_effort: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&payload(Some("high"), None), config).expect("render succeeds");
    assert!(!line1(&lines).contains("E:high"));
}

#[test]
fn show_thinking_toggle_hides_pill() {
    let config = RenderConfig {
        show_thinking: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&payload(None, Some(true)), config).expect("render succeeds");
    assert!(!line1(&lines).contains("[T]"));
}

#[test]
fn effort_pill_appears_after_model_before_agent() {
    // Ordering contract: M: then E: then [T] then AG: then S: ...
    let input = json!({
        "session_id": "test",
        "model": {"display_name": "Opus 4.7"},
        "effort": {"level": "high"},
        "thinking": {"enabled": true},
        "agent": {"name": "explore"}
    });
    let lines = render_default(&input.to_string());
    let l1 = line1(&lines);

    let model_pos = l1.find("M:Opus 4.7").expect("model pill");
    let effort_pos = l1.find("E:high").expect("effort pill");
    let thinking_pos = l1.find("[T]").expect("thinking pill");
    let agent_pos = l1.find("AG:explore").expect("agent pill");

    assert!(model_pos < effort_pos);
    assert!(effort_pos < thinking_pos);
    assert!(thinking_pos < agent_pos);
}
