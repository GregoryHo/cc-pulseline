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
        cc_margin: 4,
        group_colors: [String::new(), String::new(), String::new(), String::new()],
        color_enabled: false,
        identity_spacing: false,
    }
}

#[test]
fn box_mode_emits_section_dividers_without_outer_borders() {
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

    // Divider + content per group, including the first. 3 groups × 2 rows = 6 lines.
    assert_eq!(
        out.len(),
        6,
        "box should emit (divider + content) for every group; got {:#?}",
        out
    );
    assert!(
        out[0].starts_with("─") && out[0].contains("Identity"),
        "first line is the Identity divider; got: {:?}",
        out[0]
    );
    assert_eq!(
        out[1], "hello world (long)",
        "Identity content follows its divider; got: {:?}",
        out[1]
    );
    assert!(
        out[2].starts_with("─") && out[2].contains("Config"),
        "divider before second group; got: {:?}",
        out[2]
    );
    assert!(
        out[4].starts_with("─") && out[4].contains("Budget"),
        "divider before third group; got: {:?}",
        out[4]
    );

    for line in &out {
        for border in ['╭', '╮', '╰', '╯', '├', '┤', '│'] {
            assert!(
                !line.contains(border),
                "box no longer emits outer border glyph {:?}; got line: {:?}",
                border,
                line
            );
        }
    }
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
    // Dividers use plain `-` in ASCII mode.
    assert!(
        joined.contains('-'),
        "expected '-' horizontal chars in ASCII mode; got:\n{joined}"
    );
}

#[test]
fn gutter_style_prepends_icon_per_group() {
    let lines = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
    ];
    let cfg = base_config(PaneStyle::Gutter);
    let out = apply_pane(lines, &groups, &cfg);

    assert_eq!(out.len(), 3, "gutter adds zero rows; got {:#?}", out);
    assert!(
        out[0].starts_with('\u{f007}') && out[0].ends_with("alpha"),
        "identity line starts with user icon; got: {:?}",
        out[0]
    );
    assert!(
        out[1].starts_with('\u{f013}') && out[1].ends_with("beta"),
        "config line starts with cog icon; got: {:?}",
        out[1]
    );
    assert!(
        out[2].starts_with('\u{f155}') && out[2].ends_with("gamma"),
        "budget line starts with dollar icon; got: {:?}",
        out[2]
    );
}

#[test]
fn labeled_style_uses_three_letter_text_prefix() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(PaneStyle::Labeled);
    let out = apply_pane(lines, &groups, &cfg);

    assert_eq!(out.len(), 2, "labeled adds zero rows");
    assert!(
        out[0].starts_with("id  · ") && out[0].ends_with("alpha"),
        "identity line: {:?}",
        out[0]
    );
    assert!(
        out[1].starts_with("cfg · ") && out[1].ends_with("beta"),
        "config line: {:?}",
        out[1]
    );
}

#[test]
fn bullet_style_uses_single_unicode_glyph_per_group() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(PaneStyle::Bullet);
    let out = apply_pane(lines, &groups, &cfg);

    assert!(out[0].starts_with('●'), "bullet identity: {:?}", out[0]);
    assert!(out[1].starts_with('○'), "bullet config: {:?}", out[1]);
}

#[test]
fn bullet_style_ascii_fallback_uses_plain_chars() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Bullet);
    cfg.glyph_mode = GlyphMode::Ascii;
    let out = apply_pane(lines, &groups, &cfg);
    assert!(
        out[0].starts_with('*'),
        "ASCII bullet identity: {:?}",
        out[0]
    );
    assert!(out[1].starts_with('o'), "ASCII bullet config: {:?}", out[1]);
    for line in &out {
        for glyph in ['●', '○', '◆', '▸'] {
            assert!(
                !line.contains(glyph),
                "ASCII mode must not emit Unicode bullet {:?}; got: {:?}",
                glyph,
                line
            );
        }
    }
}

#[test]
fn pill_style_uses_bracketed_tag_per_group() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(PaneStyle::Pill);
    let out = apply_pane(lines, &groups, &cfg);
    assert!(
        out[0].starts_with(" ID  ") && out[0].ends_with("alpha"),
        "pill identity: {:?}",
        out[0]
    );
    assert!(
        out[1].starts_with(" CF  ") && out[1].ends_with("beta"),
        "pill config: {:?}",
        out[1]
    );
}

#[test]
fn identity_spacing_adds_blank_row_after_identity() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Labeled);
    cfg.identity_spacing = true;
    let out = apply_pane(lines, &groups, &cfg);
    assert_eq!(
        out.len(),
        3,
        "expected identity line + blank row + config line; got {:#?}",
        out
    );
    assert!(out[0].contains("alpha"));
    assert_eq!(out[1], "", "row after identity must be blank");
    assert!(out[2].contains("beta"));
}

#[test]
fn color_enabled_gutter_wraps_prefix_in_ansi_escapes() {
    let lines = vec!["alpha".to_string()];
    let groups = vec![(LineKind::Identity, 0..1)];
    let mut cfg = base_config(PaneStyle::Gutter);
    cfg.color_enabled = true;
    cfg.group_colors[0] = "\x1b[38;5;111m".to_string();
    let out = apply_pane(lines, &groups, &cfg);
    assert!(
        out[0].starts_with("\x1b[38;5;111m") && out[0].contains("\x1b[0m"),
        "colored prefix must open with the group fg escape and reset after the glyph; got: {:?}",
        out[0]
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
        lines
            .iter()
            .any(|l| l.contains("Config") && l.contains('─')),
        "expected a ─── Config ─── divider when pane_style=Box; got:\n{}",
        lines.join("\n")
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Budget") && l.contains('─')),
        "expected a ─── Budget ─── divider"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Identity") && l.contains('─')),
        "expected a ─── Identity ─── divider above the first group"
    );
    for line in &lines {
        for border in ['╭', '╮', '╰', '╯', '├', '┤', '│'] {
            assert!(
                !line.contains(border),
                "unexpected outer frame char {:?} in: {:?}",
                border,
                line
            );
        }
    }

    // Dividers should all be the same width (resolve_inner_width determines it).
    let divider_widths: Vec<usize> = lines
        .iter()
        .filter(|l| l.starts_with("───"))
        .map(|l| visible_width(l))
        .collect();
    if let Some(&first) = divider_widths.first() {
        assert!(
            divider_widths.iter().all(|&w| w == first),
            "all dividers should share the same width; got {:?}",
            divider_widths
        );
    }
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

#[test]
fn terminal_mode_subtracts_cc_margin_from_detected_width() {
    // Regression: Claude Code allocates the statusline a sub-region that is
    // narrower than the raw terminal (confirmed empirically on CC 2.1.119: a
    // 149-col divider in a 149-col raw terminal triggered wrap and collapsed
    // the multi-line render to 1 visible line). `cc_margin` subtracts a few
    // cols from the detected width so every line stays strictly inside CC's
    // slot. This test locks in that behavior — divider width MUST equal
    // `terminal_width - cc_margin` (not `terminal_width`).
    let lines = vec!["line-a".to_string(), "line-b".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Box);
    cfg.width_mode = PaneWidth::Terminal;
    cfg.terminal_width = Some(149);
    cfg.cc_margin = 4;
    cfg.min_width = 20;
    cfg.max_width = 300;

    let out = apply_pane(lines, &groups, &cfg);
    let divider_widths: Vec<usize> = out
        .iter()
        .filter(|l| l.starts_with("───"))
        .map(|l| visible_width(l))
        .collect();

    assert!(!divider_widths.is_empty());
    assert_eq!(
        divider_widths[0],
        145,
        "divider must be terminal_width (149) minus cc_margin (4) = 145, not the raw {}",
        cfg.terminal_width.unwrap()
    );
}

#[test]
fn terminal_mode_cc_margin_zero_uses_raw_width() {
    // Escape hatch: `cc_margin = 0` → divider == detected terminal width.
    // The margin is tunable, not a hard law — future CC versions or other
    // hosts may not need it.
    let lines = vec!["x".to_string(), "y".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Box);
    cfg.width_mode = PaneWidth::Terminal;
    cfg.terminal_width = Some(149);
    cfg.cc_margin = 0;
    cfg.min_width = 20;
    cfg.max_width = 300;

    let out = apply_pane(lines, &groups, &cfg);
    let divider = out.iter().find(|l| l.starts_with("───")).unwrap();
    assert_eq!(visible_width(divider), 149);
}

#[test]
fn terminal_mode_with_unknown_width_fits_to_content_not_max_width() {
    // Scenario: width_mode = "terminal" but detection failed (terminal_width = None).
    // This happens in Claude Code hook contexts where the spawned statusline
    // process inherits no TTY and CC doesn't pass COLUMNS — terminal_size()
    // returns None and /dev/tty is unreachable. The frame must fall back to
    // content-fit (Auto behavior), NOT blow out to max_width and wrap in the
    // real terminal.
    let lines = vec!["short-L1".to_string(), "short-L2".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let mut cfg = base_config(PaneStyle::Box);
    cfg.width_mode = PaneWidth::Terminal;
    cfg.terminal_width = None;
    cfg.min_width = 20;
    cfg.max_width = 300;

    let out = apply_pane(lines, &groups, &cfg);
    let divider_widths: Vec<usize> = out
        .iter()
        .filter(|l| l.starts_with("───"))
        .map(|l| visible_width(l))
        .collect();

    assert!(!divider_widths.is_empty(), "expected at least one divider");
    let max_divider = *divider_widths.iter().max().unwrap();
    assert!(
        max_divider < cfg.max_width,
        "divider width {} should not inflate to max_width {} when terminal_width is None — \
         frame would overflow the real terminal",
        max_divider,
        cfg.max_width
    );
    assert!(
        max_divider <= 50,
        "divider should fit short content (got {}), not blow up",
        max_divider
    );
}
