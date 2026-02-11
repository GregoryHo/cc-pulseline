use cc_pulseline::{config::RenderConfig, run_from_str};
use serde_json::json;

fn basic_input() -> String {
    json!({
        "model": {"display_name": "Opus 4.6"},
        "output_style": {"name": "explanatory"},
        "version": "2.1.37",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 43,
            "current_usage": {
                "input_tokens": 10,
                "output_tokens": 20,
                "cache_creation_input_tokens": 30,
                "cache_read_input_tokens": 40
            }
        },
        "cost": {
            "total_cost_usd": 3.5,
            "total_duration_ms": 3600000
        }
    })
    .to_string()
}

// ── Line 1 toggles ──────────────────────────────────────────────────

#[test]
fn line1_hides_style_when_disabled() {
    let config = RenderConfig {
        show_style: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(
        !lines[0].contains("S:"),
        "L1 should not contain style when show_style=false"
    );
    assert!(
        lines[0].contains("M:Opus 4.6"),
        "L1 should still show model"
    );
}

#[test]
fn line1_hides_version_and_project() {
    let config = RenderConfig {
        show_version: false,
        show_project: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(!lines[0].contains("CC:"), "L1 should not contain version");
    assert!(!lines[0].contains("P:"), "L1 should not contain project");
    assert!(
        lines[0].contains("M:Opus 4.6"),
        "L1 should still show model"
    );
    assert!(lines[0].contains("G:"), "L1 should still show git");
}

#[test]
fn line1_shows_only_model_and_git() {
    let config = RenderConfig {
        show_style: false,
        show_version: false,
        show_project: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(lines[0].contains("M:Opus 4.6"), "L1 should show model");
    assert!(lines[0].contains("G:"), "L1 should show git");
    assert!(!lines[0].contains("S:"), "L1 should not show style");
    assert!(!lines[0].contains("CC:"), "L1 should not show version");
    assert!(!lines[0].contains("P:"), "L1 should not show project");
}

// ── Line 2 toggles ──────────────────────────────────────────────────

#[test]
fn line2_hides_memory_when_disabled() {
    let config = RenderConfig {
        show_memory: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(
        !lines[1].contains("memories"),
        "L2 should not contain memories when show_memory=false"
    );
    assert!(
        lines[1].contains("CLAUDE.md"),
        "L2 should still show CLAUDE.md"
    );
    assert!(lines[1].contains("rules"), "L2 should still show rules");
}

#[test]
fn line2_hides_tokens_and_rules() {
    let config = RenderConfig {
        show_rules: false,
        show_duration: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(
        !lines[1].contains("rules"),
        "L2 should not contain rules when show_rules=false"
    );
    assert!(
        lines[1].contains("CLAUDE.md"),
        "L2 should still show CLAUDE.md"
    );
    assert!(lines[1].contains("hooks"), "L2 should still show hooks");
    assert!(lines[1].contains("MCPs"), "L2 should still show MCPs");
    assert!(lines[1].contains("skills"), "L2 should still show skills");
    // Duration should be absent — last item won't end with time format
    assert!(
        !lines[1].ends_with("1h"),
        "L2 should not show duration when show_duration=false"
    );
}

#[test]
fn line2_shows_only_claude_md_and_mcp() {
    let config = RenderConfig {
        show_rules: false,
        show_memory: false,
        show_hooks: false,
        show_skills: false,
        show_duration: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(lines[1].contains("CLAUDE.md"), "L2 should show CLAUDE.md");
    assert!(lines[1].contains("MCPs"), "L2 should show MCPs");
    assert!(!lines[1].contains("rules"), "L2 should not show rules");
    assert!(
        !lines[1].contains("memories"),
        "L2 should not show memories"
    );
    assert!(!lines[1].contains("hooks"), "L2 should not show hooks");
    assert!(!lines[1].contains("skills"), "L2 should not show skills");
}

// ── Line 3 toggles ──────────────────────────────────────────────────

#[test]
fn line3_hides_tokens_segment() {
    let config = RenderConfig {
        show_tokens: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(
        !lines[2].contains("TOK"),
        "L3 should not contain tokens when show_tokens=false"
    );
    assert!(lines[2].contains("CTX:"), "L3 should still show context");
    assert!(lines[2].contains("$3.50"), "L3 should still show cost");
}

#[test]
fn line3_hides_context_segment() {
    let config = RenderConfig {
        show_context: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(
        !lines[2].contains("CTX:"),
        "L3 should not contain context when show_context=false"
    );
    assert!(lines[2].contains("TOK"), "L3 should still show tokens");
    assert!(lines[2].contains("$3.50"), "L3 should still show cost");
}

#[test]
fn line3_shows_only_cost() {
    let config = RenderConfig {
        show_context: false,
        show_tokens: false,
        ..RenderConfig::default()
    };
    let lines = run_from_str(&basic_input(), config).unwrap();
    assert!(!lines[2].contains("CTX:"), "L3 should not contain context");
    assert!(!lines[2].contains("TOK"), "L3 should not contain tokens");
    assert!(lines[2].contains("$3.50"), "L3 should only show cost");
    // Should not have leading separators
    assert!(
        !lines[2].starts_with(" |"),
        "L3 should not start with separator"
    );
}

// ── Config deserialization ──────────────────────────────────────────

#[test]
fn partial_toml_config_deserializes_with_defaults() {
    let toml_str = r#"
[segments.identity]
show_style = false

[segments.budget]
show_tokens = false
"#;
    let config: cc_pulseline::config::PulselineConfig =
        toml::from_str(toml_str).expect("should deserialize");

    assert!(!config.segments.identity.show_style);
    assert!(
        config.segments.identity.show_model,
        "unset fields default to true"
    );
    assert!(
        config.segments.identity.show_git,
        "unset fields default to true"
    );
    assert!(!config.segments.budget.show_tokens);
    assert!(
        config.segments.budget.show_context,
        "unset fields default to true"
    );
    assert!(
        config.segments.config.show_claude_md,
        "entire section defaults to true"
    );
}
