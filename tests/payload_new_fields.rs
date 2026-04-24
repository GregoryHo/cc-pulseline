use cc_pulseline::types::{RenderFrame, StdinPayload};

#[test]
fn deserializes_effort_and_thinking() {
    let json = r#"{
        "session_id": "test",
        "effort": {"level": "xhigh"},
        "thinking": {"enabled": true}
    }"#;

    let payload: StdinPayload = serde_json::from_str(json).expect("parses");
    let effort = payload.effort.as_ref().expect("effort present");
    assert_eq!(effort.level.as_deref(), Some("xhigh"));
    let thinking = payload.thinking.as_ref().expect("thinking present");
    assert_eq!(thinking.enabled, Some(true));
}

#[test]
fn missing_effort_and_thinking_default_to_none() {
    let json = r#"{"session_id": "test"}"#;

    let payload: StdinPayload = serde_json::from_str(json).expect("parses");
    assert!(payload.effort.is_none(), "effort should default to None");
    assert!(
        payload.thinking.is_none(),
        "thinking should default to None"
    );
}

#[test]
fn render_frame_populates_line1_from_payload() {
    let json = r#"{
        "session_id": "test",
        "effort": {"level": "high"},
        "thinking": {"enabled": true}
    }"#;

    let payload: StdinPayload = serde_json::from_str(json).expect("parses");
    let frame = RenderFrame::from_payload(&payload);

    assert_eq!(frame.line1.effort_level.as_deref(), Some("high"));
    assert_eq!(frame.line1.thinking_enabled, Some(true));
}

#[test]
fn render_frame_handles_missing_new_fields() {
    let payload: StdinPayload = serde_json::from_str(r#"{"session_id": "test"}"#).expect("parses");
    let frame = RenderFrame::from_payload(&payload);

    assert!(frame.line1.effort_level.is_none());
    assert!(frame.line1.thinking_enabled.is_none());
    assert_eq!(frame.line2.plugins_count, 0);
}

#[test]
fn accepts_unknown_effort_levels_as_pass_through() {
    // Claude Code may add new effort levels (e.g. xhigh was brand-new in 2.1.111).
    // We keep the level as Option<String> so future values parse without code changes.
    let json = r#"{"effort": {"level": "ultraplus"}}"#;
    let payload: StdinPayload = serde_json::from_str(json).expect("parses");
    assert_eq!(
        payload.effort.and_then(|e| e.level).as_deref(),
        Some("ultraplus")
    );
}

#[test]
fn thinking_enabled_false_is_preserved() {
    let json = r#"{"thinking": {"enabled": false}}"#;
    let payload: StdinPayload = serde_json::from_str(json).expect("parses");
    assert_eq!(payload.thinking.and_then(|t| t.enabled), Some(false));
}

#[test]
fn stdin_payload_serializes_round_trip() {
    // Guard against accidental removal of Serialize derives — they matter for
    // session cache persistence.
    let json = r#"{
        "effort": {"level": "medium"},
        "thinking": {"enabled": true}
    }"#;
    let payload: StdinPayload = serde_json::from_str(json).expect("parses");
    let reserialized = serde_json::to_string(&payload).expect("serializes");
    let re_parsed: StdinPayload = serde_json::from_str(&reserialized).expect("re-parses");

    assert_eq!(
        re_parsed.effort.and_then(|e| e.level).as_deref(),
        Some("medium")
    );
    assert_eq!(re_parsed.thinking.and_then(|t| t.enabled), Some(true));
}
