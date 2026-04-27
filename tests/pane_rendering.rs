use cc_pulseline::config::{
    build_render_config, merge_configs, GlyphMode, ProjectOverrideConfig, PulselineConfig,
};
use cc_pulseline::render::color::visible_width;
use cc_pulseline::render::pane::{
    apply_pane, LineKind, PaneConfig, PaneGroup, PaneStyle, PaneWidth,
};

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
            PaneGroup {
                label: "Activity".into(),
                kinds: vec![LineKind::Activity],
            },
        ],
        glyph_mode: GlyphMode::Icon,
        terminal_width: None,
        cc_margin: 4,
    }
}

#[test]
fn grid_adds_label_column_with_divider_and_aligned_content() {
    let lines = vec![
        "alpha content".to_string(),
        "longer beta content here".to_string(),
        "gamma".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
    ];
    let cfg = base_config(PaneStyle::V1Grid);
    let out = apply_pane(lines, &groups, &cfg);

    assert_eq!(out.len(), 3, "grid adds zero rows");

    // Left label column contains the group name; `│` follows.
    assert!(
        out[0].starts_with("Identity  │"),
        "first row shows Identity label + divider; got: {:?}",
        out[0]
    );
    assert!(
        out[1].starts_with("Config    │"),
        "second row aligns label to same width; got: {:?}",
        out[1]
    );
    assert!(out[2].starts_with("Budget    │"), "third row: {:?}", out[2]);

    // Every row right-padded to the same visible width.
    let widths: Vec<usize> = out.iter().map(|s| visible_width(s)).collect();
    assert!(
        widths.iter().all(|&w| w == widths[0]),
        "all rows must be padded to equal visible width; got {:?}",
        widths
    );
}

#[test]
fn grid_continuation_rows_blank_the_label() {
    let lines = vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
    ];
    // Single group spanning all three lines — continuations should blank the label.
    let groups = vec![(LineKind::Activity, 0..3)];
    let mut cfg = base_config(PaneStyle::V1Grid);
    cfg.groups = vec![PaneGroup {
        label: "Activity".into(),
        kinds: vec![LineKind::Activity],
    }];
    let out = apply_pane(lines, &groups, &cfg);

    assert_eq!(out.len(), 3);
    assert!(out[0].starts_with("Activity"), "first row labeled");
    assert!(
        out[1].starts_with("          │"),
        "second row blank label, divider aligns; got: {:?}",
        out[1]
    );
    assert!(
        out[2].starts_with("          │"),
        "third row blank label; got: {:?}",
        out[2]
    );
}

#[test]
fn sections_ascii_fallback_uses_plus_and_dash() {
    let lines = vec!["alpha".to_string(), "act".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Activity, 1..2)];
    let mut cfg = base_config(PaneStyle::V1Sections);
    cfg.glyph_mode = GlyphMode::Ascii;
    let out = apply_pane(lines, &groups, &cfg);

    let joined = out.join("\n");
    for glyph in ["╭", "╮", "╰", "╯", "─", "│", "├", "┤", "┬", "┴", "┼"] {
        assert!(
            !joined.contains(glyph),
            "ASCII mode must not emit Unicode glyph {:?}; got:\n{}",
            glyph,
            joined
        );
    }
    assert!(joined.contains('+') && joined.contains('-') && joined.contains('|'));
}

#[test]
fn cards_emits_one_frame_per_group() {
    let lines = vec![
        "alpha".to_string(),
        "longer beta".to_string(),
        "gamma".to_string(),
        "act-1".to_string(),
        "act-2".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
        (LineKind::Activity, 3..5),
    ];
    let cfg = base_config(PaneStyle::V1Cards);
    let out = apply_pane(lines, &groups, &cfg);

    // 4 non-empty groups × (top + bottom) = 8 decoration rows + 5 content rows = 13.
    assert_eq!(out.len(), 13, "cards count; got {:#?}", out);

    // Every card opens with ╭ and closes with ╰ — never the frame's ├ middle.
    let tops: Vec<_> = out.iter().filter(|l| l.starts_with('╭')).collect();
    let bottoms: Vec<_> = out.iter().filter(|l| l.starts_with('╰')).collect();
    let mids: Vec<_> = out.iter().filter(|l| l.starts_with('├')).collect();
    assert_eq!(tops.len(), 4, "one ╭ top per group");
    assert_eq!(bottoms.len(), 4, "one ╰ bottom per group");
    assert!(mids.is_empty(), "cards must not emit ├─┼─┤ mid-separators");

    // All tops are the same width — shared global label_width + content_width.
    let top_widths: Vec<usize> = tops.iter().map(|s| visible_width(s)).collect();
    assert!(
        top_widths.iter().all(|&w| w == top_widths[0]),
        "all card tops must share visible width (aligned columns); got {:?}",
        top_widths
    );

    // The first card contains Identity with its content, flanked by ╭ / ╰.
    assert!(out[0].starts_with('╭'), "first row = Identity top");
    assert!(
        out[1].starts_with("│ Identity"),
        "Identity content row: {:?}",
        out[1]
    );
    assert!(out[2].starts_with('╰'), "Identity bottom");
    assert!(out[3].starts_with('╭'), "Config top immediately after");
}

#[test]
fn cards_skips_empty_groups() {
    let lines = vec!["alpha".to_string(), "beta".to_string()];
    // Budget group range is empty (2..2); should NOT emit a card for it.
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..2),
    ];
    let cfg = base_config(PaneStyle::V1Cards);
    let out = apply_pane(lines, &groups, &cfg);

    // 2 non-empty groups × 2 decoration + 2 content = 6 rows.
    assert_eq!(out.len(), 6);
    let tops = out.iter().filter(|l| l.starts_with('╭')).count();
    assert_eq!(tops, 2, "empty group must not get its own card");
}

#[test]
fn sections_wraps_once_with_separator_between_every_group() {
    let lines = vec![
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
        "act-1".to_string(),
        "act-2".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
        (LineKind::Activity, 3..5),
    ];
    let cfg = base_config(PaneStyle::V1Sections);
    let out = apply_pane(lines, &groups, &cfg);

    // 5 content rows + 1 top + 3 internal separators (between 4 groups) + 1 bottom = 10 rows.
    assert_eq!(out.len(), 10, "sections row count; got {:#?}", out);

    // Exactly one ╭ top and one ╰ bottom — single outer frame.
    let tops = out.iter().filter(|l| l.starts_with('╭')).count();
    let bottoms = out.iter().filter(|l| l.starts_with('╰')).count();
    assert_eq!(tops, 1, "single outer top");
    assert_eq!(bottoms, 1, "single outer bottom");

    // 3 mid-separators (between 4 groups).
    let mids: Vec<_> = out.iter().filter(|l| l.starts_with('├')).collect();
    assert_eq!(mids.len(), 3, "separator between every group pair");

    // All outer/separator rows share visible width.
    let frame_widths: Vec<usize> = out
        .iter()
        .filter(|l| l.starts_with('╭') || l.starts_with('├') || l.starts_with('╰'))
        .map(|s| visible_width(s))
        .collect();
    assert!(
        frame_widths.iter().all(|&w| w == frame_widths[0]),
        "all frame lines share width; got {:?}",
        frame_widths
    );

    // Layout: top / Identity / sep / Config / sep / Budget / sep / Activity×2 / bottom
    assert!(out[0].starts_with('╭'));
    assert!(out[1].starts_with("│ Identity"));
    assert!(out[2].starts_with('├'));
    assert!(out[3].starts_with("│ Config"));
    assert!(out[4].starts_with('├'));
    assert!(out[5].starts_with("│ Budget"));
    assert!(out[6].starts_with('├'));
    assert!(out[7].starts_with("│ Activity"));
    assert!(out[8].starts_with("│          "));
    assert!(out[9].starts_with('╰'));
}

#[test]
fn sections_skips_empty_groups_for_separator_count() {
    let lines = vec!["a".to_string(), "b".to_string()];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..2),   // empty
        (LineKind::Activity, 2..2), // empty
    ];
    let cfg = base_config(PaneStyle::V1Sections);
    let out = apply_pane(lines, &groups, &cfg);

    // 2 content + top + 1 sep (between 2 non-empty groups) + bottom = 5 rows.
    assert_eq!(out.len(), 5);
    let mids = out.iter().filter(|l| l.starts_with('├')).count();
    assert_eq!(mids, 1, "empty groups must not trigger separators");
}

#[test]
fn sections_parser_accepts_sections_keyword() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"[layout]
name = "sections"
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, PaneStyle::V1Sections);
}

#[test]
fn cards_parser_accepts_cards_keyword() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"[layout]
name = "cards"
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, PaneStyle::V1Cards);
}

#[test]
fn zones_inserts_single_rule_before_activity() {
    let lines = vec![
        "identity-line".to_string(),
        "config-line".to_string(),
        "budget-line".to_string(),
        "activity-line-1".to_string(),
        "activity-line-2".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..3),
        (LineKind::Activity, 3..5),
    ];
    let cfg = base_config(PaneStyle::V1Zones);
    let out = apply_pane(lines, &groups, &cfg);

    // 5 content lines + 1 rule = 6 rows.
    assert_eq!(out.len(), 6, "zones adds exactly one rule; got {:#?}", out);

    assert_eq!(out[0], "identity-line", "state content emitted first");
    assert_eq!(out[1], "config-line");
    assert_eq!(out[2], "budget-line");

    // The rule must appear between Budget (out[2]) and the first Activity line.
    assert!(
        out[3].starts_with('─') && out[3].contains("activity"),
        "rule must precede Activity and be labelled 'activity'; got: {:?}",
        out[3]
    );
    assert_eq!(out[4], "activity-line-1");
    assert_eq!(out[5], "activity-line-2");
}

#[test]
fn zones_omits_rule_when_no_activity() {
    let lines = vec!["identity-line".to_string(), "config-line".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(PaneStyle::V1Zones);
    let out = apply_pane(lines.clone(), &groups, &cfg);
    assert_eq!(
        out, lines,
        "zones should pass lines through unchanged when there is no activity"
    );
}

#[test]
fn none_style_returns_lines_unchanged() {
    let lines = vec!["a".to_string(), "b".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(PaneStyle::V1None);
    let out = apply_pane(lines.clone(), &groups, &cfg);
    assert_eq!(out, lines, "None style must be a strict passthrough");
}

#[test]
fn config_defaults_pane_style_to_none() {
    let cfg = PulselineConfig::default();
    let render_cfg = build_render_config(&cfg);
    assert_eq!(
        render_cfg.pane_style,
        PaneStyle::V1None,
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
[layout]
name = "grid"
min_width = 80
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, PaneStyle::V1Grid);
    assert_eq!(render_cfg.pane_min_width, 80);
    assert_eq!(
        render_cfg.pane_max_width, 140,
        "max_width inherits user default"
    );
}

#[test]
fn terminal_mode_subtracts_cc_margin_from_detected_width() {
    // Regression: Claude Code allocates the statusline a sub-region that is
    // narrower than the raw terminal (confirmed empirically on CC 2.1.119: a
    // 149-col divider in a 149-col raw terminal triggered wrap and collapsed
    // the multi-line render to 1 visible line). `cc_margin` subtracts a few
    // cols from the detected width so every line stays strictly inside CC's
    // slot. This test locks in that behavior — rule width MUST equal
    // `terminal_width - cc_margin` (not `terminal_width`).
    let lines = vec![
        "line-a".to_string(),
        "line-b".to_string(),
        "activity-line".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Activity, 2..3),
    ];
    let mut cfg = base_config(PaneStyle::V1Zones);
    cfg.width_mode = PaneWidth::Terminal;
    cfg.terminal_width = Some(149);
    cfg.cc_margin = 4;
    cfg.min_width = 20;
    cfg.max_width = 300;

    let out = apply_pane(lines, &groups, &cfg);
    let rule_widths: Vec<usize> = out
        .iter()
        .filter(|l| l.starts_with("───"))
        .map(|l| visible_width(l))
        .collect();

    assert!(!rule_widths.is_empty());
    assert_eq!(
        rule_widths[0],
        145,
        "rule must be terminal_width (149) minus cc_margin (4) = 145, not the raw {}",
        cfg.terminal_width.unwrap()
    );
}

#[test]
fn terminal_mode_cc_margin_zero_uses_raw_width() {
    // Escape hatch: `cc_margin = 0` → rule == detected terminal width.
    // The margin is tunable, not a hard law — future CC versions or other
    // hosts may not need it.
    let lines = vec!["x".to_string(), "y".to_string(), "activity".to_string()];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Activity, 2..3),
    ];
    let mut cfg = base_config(PaneStyle::V1Zones);
    cfg.width_mode = PaneWidth::Terminal;
    cfg.terminal_width = Some(149);
    cfg.cc_margin = 0;
    cfg.min_width = 20;
    cfg.max_width = 300;

    let out = apply_pane(lines, &groups, &cfg);
    let rule = out.iter().find(|l| l.starts_with("───")).unwrap();
    assert_eq!(visible_width(rule), 149);
}

#[test]
fn terminal_mode_with_unknown_width_fits_to_content_not_max_width() {
    // Scenario: width_mode = "terminal" but detection failed (terminal_width = None).
    // This happens in Claude Code hook contexts where the spawned statusline
    // process inherits no TTY and CC doesn't pass COLUMNS — terminal_size()
    // returns None and /dev/tty is unreachable. The frame must fall back to
    // content-fit (Auto behavior), NOT blow out to max_width and wrap in the
    // real terminal.
    let lines = vec![
        "short-L1".to_string(),
        "short-L2".to_string(),
        "activity".to_string(),
    ];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Activity, 2..3),
    ];
    let mut cfg = base_config(PaneStyle::V1Zones);
    cfg.width_mode = PaneWidth::Terminal;
    cfg.terminal_width = None;
    cfg.min_width = 20;
    cfg.max_width = 300;

    let out = apply_pane(lines, &groups, &cfg);
    let rule_widths: Vec<usize> = out
        .iter()
        .filter(|l| l.starts_with("───"))
        .map(|l| visible_width(l))
        .collect();

    assert!(!rule_widths.is_empty(), "expected at least one rule");
    let max_rule = *rule_widths.iter().max().unwrap();
    assert!(
        max_rule < cfg.max_width,
        "rule width {} should not inflate to max_width {} when terminal_width is None — \
         frame would overflow the real terminal",
        max_rule,
        cfg.max_width
    );
    assert!(
        max_rule <= 50,
        "rule should fit short content (got {}), not blow up",
        max_rule
    );
}
