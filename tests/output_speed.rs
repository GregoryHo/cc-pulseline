use std::thread;
use std::time::Duration;

use cc_pulseline::{config::RenderConfig, types::StdinPayload, PulseLineRunner};
use serde_json::json;

fn make_input_with_tokens(session_id: &str, input_tokens: u64, output_tokens: u64) -> String {
    json!({
        "session_id": session_id,
        "model": {"display_name": "Opus"},
        "version": "1.0",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 30,
            "current_usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 0
            }
        },
        "cost": {"total_cost_usd": 1.0, "total_duration_ms": 60000}
    })
    .to_string()
}

#[test]
fn speed_hidden_by_default() {
    let input = make_input_with_tokens("speed-hidden", 5000, 100);
    let config = RenderConfig {
        show_speed: false,
        ..Default::default()
    };
    let mut runner = PulseLineRunner::default();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let lines = runner
        .run_from_payload(&payload, config)
        .expect("should render");
    let line3 = &lines[2];
    assert!(
        !line3.contains("↗"),
        "should not show ↗ speed arrow when speed disabled"
    );
}

#[test]
fn speed_absent_on_first_invocation() {
    let input = make_input_with_tokens("speed-first", 5000, 100);
    let config = RenderConfig {
        show_speed: true,
        ..Default::default()
    };
    let mut runner = PulseLineRunner::default();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let lines = runner
        .run_from_payload(&payload, config)
        .expect("should render");
    let line3 = &lines[2];
    // First invocation: no speed segment (speed is None)
    assert!(
        !line3.contains("↗"),
        "first invocation should not show ↗ speed, got: {line3}"
    );
    // Should still show token counts
    assert!(line3.contains("I:"), "should show input token label");
    assert!(line3.contains("O:"), "should show output token label");
}

#[test]
fn speed_shows_own_segment_after_two_invocations() {
    let config = RenderConfig {
        show_speed: true,
        ..Default::default()
    };

    let mut runner = PulseLineRunner::default();

    // First invocation: establishes baseline
    let input1 = make_input_with_tokens("speed-rate", 5000, 100);
    let payload1: StdinPayload = serde_json::from_str(&input1).unwrap();
    let _ = runner.run_from_payload(&payload1, config.clone());

    // Small sleep to ensure delta_ms > 0 (ms-resolution timer)
    thread::sleep(Duration::from_millis(5));

    // Second invocation shortly after: should compute speed
    let input2 = make_input_with_tokens("speed-rate", 6000, 200);
    let payload2: StdinPayload = serde_json::from_str(&input2).unwrap();
    let lines = runner
        .run_from_payload(&payload2, config)
        .expect("should render");
    let line3 = &lines[2];

    // Speed should appear inline within the TOK segment (after output tokens)
    assert!(
        line3.contains("↗"),
        "should contain ↗ speed arrow, got: {line3}"
    );
    assert!(
        line3.contains("/s"),
        "should contain /s speed unit, got: {line3}"
    );
    // Speed is inline — should NOT be a separate pipe-separated segment
    assert!(
        !line3.contains("| ↗"),
        "speed should be inline, not pipe-separated, got: {line3}"
    );
}

#[test]
fn speed_color_output() {
    let input = make_input_with_tokens("speed-color", 5000, 100);
    let config = RenderConfig {
        show_speed: true,
        color_enabled: true,
        ..Default::default()
    };
    let mut runner = PulseLineRunner::default();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let lines = runner
        .run_from_payload(&payload, config)
        .expect("should render");
    let line3 = &lines[2];
    assert!(line3.contains("\x1b["), "should contain ANSI color codes");
    // Token labels should be present
    assert!(line3.contains("I:"), "should contain input token label");
    assert!(line3.contains("O:"), "should contain output token label");
}
