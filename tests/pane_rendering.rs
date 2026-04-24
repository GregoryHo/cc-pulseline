use cc_pulseline::config::{
    build_render_config, merge_configs, GlyphMode, ProjectOverrideConfig, PulselineConfig,
    RenderConfig,
};
use cc_pulseline::render::color::visible_width;
use cc_pulseline::render::pane::{
    apply_pane, LineKind, PaneConfig, PaneGroup, PaneStyle, PaneWidth,
};
use cc_pulseline::PulseLineRunner;
use serde_json::json;
use tempfile::TempDir;

fn base_config(style: PaneStyle) -> PaneConfig {
    PaneConfig {
        style,
        width_mode: PaneWidth::Auto,
        min_width: 10,
        max_width: 200,
        groups: vec![
            PaneGroup {
                label: "Identity".into(),
                kinds: vec![LineKind::Identity],
            },
            PaneGroup {
                label: "Config".into(),
                kinds: vec![LineKind::Config],
            },
            PaneGroup {
                label: "Budget".into(),
                kinds: vec![LineKind::Budget],
            },
        ],
        glyph_mode: GlyphMode::Icon,
        terminal_width: None,
    }
}

#[test]
fn box_mode_wraps_with_borders_and_aligned_right_edge() {
    let lines = vec![
        "hello world (long)".to_string(),
        "foo".to_string(),
        "baz bar baz".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
    ];
    let cfg = base_config(PaneStyle::Box);
    let out = apply_pane(lines, &groups, &cfg);

    // 3 content lines + top border + 2 inner dividers + bottom border = 7 rendered lines.
    assert_eq!(
        out.len(),
        7,
        "box mode should add top/bottom + N-1 dividers for N=3 groups; got {} lines: {:#?}",
        out.len(),
        out
    );
    assert!(
        out[0].starts_with('╭'),
        "top border must start with rounded corner; got: {:?}",
        out[0]
    );
    assert!(
        out[0].contains("Identity"),
        "top border must display first group's label inline; got: {:?}",
        out[0]
    );
    assert!(
        out[2].starts_with('├'),
        "inter-group divider must start with tee-left; got: {:?}",
        out[2]
    );
    assert!(
        out[2].contains("Config"),
        "inter-group divider must include the group label; got: {:?}",
        out[2]
    );
    assert!(
        out[6].starts_with('╰'),
        "bottom border must start with rounded corner; got: {:?}",
        out[6]
    );

    let widths: Vec<usize> = out.iter().map(|s| visible_width(s)).collect();
    let first = widths[0];
    assert!(
        widths.iter().all(|&w| w == first),
        "all framed lines must align to identical visible width; got widths {:?}",
        widths
    );
}

#[test]
fn rail_mode_shows_left_guide_only_without_right_border() {
    let lines = vec![
        "line one".to_string(),
        "another line".to_string(),
        "x".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
    ];
    let cfg = base_config(PaneStyle::Rail);
    let out = apply_pane(lines.clone(), &groups, &cfg);

    // 3 content lines + top + 2 inner dividers + bottom closer = 7 lines,
    // same as box mode (same group structure).
    assert_eq!(
        out.len(),
        7,
        "rail should preserve top/divider/bottom count"
    );

    // Rail lines must not end with the box right border `│`.
    for line in &out {
        assert!(
            !line.trim_end().ends_with('│')
                || line.starts_with('│') && line.len() > 1 && !is_box_right_border(line),
            "rail line unexpectedly has a right-side vertical bar: {:?}",
            line
        );
    }

    // Rail top begins with ╭ then label (no right-side corner).
    assert!(out[0].starts_with('╭'), "rail top: {:?}", out[0]);
    assert!(!out[0].contains('╮'), "rail must not emit ╮: {:?}", out[0]);
    assert!(out[0].contains("Identity"));

    // Dividers begin with ├ and never close with ┤ on the right.
    assert!(out[2].starts_with('├'), "rail mid: {:?}", out[2]);
    assert!(!out[2].contains('┤'), "rail must not emit ┤: {:?}", out[2]);
    assert!(out[2].contains("Config"));

    // Content lines start with `│ ` and carry original text (no right padding/border).
    assert!(out[1].starts_with('│'), "content rail: {:?}", out[1]);
    assert!(
        out[1].contains("line one"),
        "content preserved: {:?}",
        out[1]
    );

    // Bottom closer is lone ╰ glyph (no horizontal fill, no ╯).
    assert!(out[6].starts_with('╰'), "rail bottom: {:?}", out[6]);
    assert!(!out[6].contains('╯'), "rail must not emit ╯: {:?}", out[6]);
}

fn is_box_right_border(line: &str) -> bool {
    // A box right border is ` │` at end-of-line following content.
    line.trim_end().ends_with(" │")
}

#[test]
fn ascii_glyph_mode_avoids_unicode_box_chars() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Box);
    cfg.glyph_mode = GlyphMode::Ascii;
    let out = apply_pane(lines, &groups, &cfg);

    let joined = out.join("\n");
    for glyph in ["╭", "╮", "╰", "╯", "─", "│", "├", "┤"] {
        assert!(
            !joined.contains(glyph),
            "ASCII mode must not emit Unicode frame glyph {:?}; output was:\n{}",
            glyph,
            joined
        );
    }
    // Plausible ASCII substitutes are expected: +, -, |
    assert!(
        joined.contains('+'),
        "expected '+' corner/tee chars in ASCII mode"
    );
    assert!(
        joined.contains('-'),
        "expected '-' horizontal chars in ASCII mode"
    );
    assert!(
        joined.contains('|'),
        "expected '|' vertical chars in ASCII mode"
    );
}

#[test]
fn none_style_returns_lines_unchanged() {
    let lines = vec!["a".to_string(), "b".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(PaneStyle::None);
    let out = apply_pane(lines.clone(), &groups, &cfg);
    assert_eq!(out, lines, "None style must be a strict passthrough");
}

#[test]
fn config_defaults_pane_style_to_none() {
    let cfg = PulselineConfig::default();
    let render_cfg = build_render_config(&cfg);
    assert_eq!(
        render_cfg.pane_style,
        PaneStyle::None,
        "default pane style must be None so existing users see no change"
    );
    assert_eq!(render_cfg.pane_min_width, 60);
    assert_eq!(render_cfg.pane_max_width, 140);
}

#[test]
fn project_config_overrides_pane_style() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[pane]
style = "box"
min_width = 80
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, PaneStyle::Box);
    assert_eq!(render_cfg.pane_min_width, 80);
    assert_eq!(
        render_cfg.pane_max_width, 140,
        "max_width inherits user default"
    );
}

#[test]
fn render_frame_integration_applies_box_pane_when_enabled() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("empty.jsonl");
    std::fs::write(&transcript, "").expect("empty transcript");

    let payload = json!({
        "session_id": "pane-integration",
        "cwd": workspace.path(),
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus"},
        "version": "2.2.0",
        "transcript_path": transcript,
    })
    .to_string();

    let cfg = RenderConfig {
        transcript_poll_throttle_ms: 0,
        glyph_mode: GlyphMode::Icon,
        pane_style: PaneStyle::Box,
        pane_width_mode: PaneWidth::Auto,
        pane_min_width: 40,
        pane_max_width: 200,
        ..RenderConfig::default()
    };

    let mut runner = PulseLineRunner::default();
    let lines = runner
        .run_from_str(&payload, cfg)
        .expect("render should succeed with pane enabled");

    assert!(
        lines.iter().any(|l| l.starts_with('╭')),
        "output must contain box top-left corner when pane_style=Box; got:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|l| l.starts_with('╰')),
        "output must contain box bottom-left corner when pane_style=Box"
    );
    assert!(
        lines.iter().any(|l| l.contains("Identity")),
        "output must label the Identity group"
    );

    // All framed interior lines share the same visible width.
    let framed_widths: Vec<usize> = lines
        .iter()
        .filter(|l| {
            l.starts_with('╭') || l.starts_with('│') || l.starts_with('├') || l.starts_with('╰')
        })
        .map(|l| visible_width(l))
        .collect();
    let first = framed_widths[0];
    assert!(
        framed_widths.iter().all(|&w| w == first),
        "all framed lines must be equal width; got {:?}",
        framed_widths
    );
}

#[test]
fn box_mode_disables_when_terminal_too_narrow_for_min_width() {
    let lines = vec!["x".to_string(), "y".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Box);
    cfg.min_width = 60;
    cfg.terminal_width = Some(40); // 40 < 60 + 4 ⇒ can't fit the user's minimum pane width
    let out = apply_pane(lines.clone(), &groups, &cfg);
    assert_eq!(
        out, lines,
        "frame must silently disable when terminal can't fit min_width + border cost"
    );
}
