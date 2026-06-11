use cc_pulseline::config::{
    build_render_config, merge_configs, GlyphMode, ProjectOverrideConfig, PulselineConfig,
};
use cc_pulseline::render::color::visible_width;
use cc_pulseline::render::pane::{apply_pane, LayoutStyle, LineKind, PaneConfig, PaneGroup};

fn base_config(style: LayoutStyle) -> PaneConfig {
    PaneConfig {
        style,
        min_width: 10,
        max_width: 200,
        groups: vec![
            PaneGroup {
                label: "Identity".into(),
                kinds: vec![LineKind::Identity],
            },
            PaneGroup {
                label: "ENV".into(),
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
fn console_ascii_fallback_uses_plus_and_dash() {
    let lines = vec!["alpha".to_string(), "act".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Activity, 1..2)];
    let mut cfg = base_config(LayoutStyle::Console);
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

// Cards layout was removed in the layout consolidation; the
// `cards_emits_one_frame_per_group` and `cards_skips_empty_groups`
// tests were specific to that layout's per-group framing.

#[test]
fn console_wraps_once_with_separator_between_every_group() {
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
    let cfg = base_config(LayoutStyle::Console);
    let out = apply_pane(lines, &groups, &cfg);

    // Identity hoists into the title, so the body holds 4 content rows.
    // 1 top (title border) + Config + sep + Budget + sep + Activity×2
    // + 1 bottom = 8 rows.
    assert_eq!(out.len(), 8, "console row count; got {:#?}", out);

    // Exactly one ╭ top and one ╰ bottom — single outer frame, with the
    // identity line baked into the top border as the title.
    let tops = out.iter().filter(|l| l.starts_with('╭')).count();
    let bottoms = out.iter().filter(|l| l.starts_with('╰')).count();
    assert_eq!(tops, 1, "single outer top");
    assert_eq!(bottoms, 1, "single outer bottom");
    assert!(
        out[0].contains("alpha"),
        "identity row hoisted into the title: {:?}",
        out[0]
    );

    // 2 mid-separators (between the 3 non-empty body groups).
    let mids: Vec<_> = out.iter().filter(|l| l.starts_with('├')).collect();
    assert_eq!(mids.len(), 2, "separator between every body group pair");

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

    // Layout: title / Config / sep / Budget / sep / Activity×2 / bottom
    assert!(out[0].starts_with('╭'));
    assert!(out[1].starts_with("│ ENV"));
    assert!(out[2].starts_with('├'));
    assert!(out[3].starts_with("│ Budget"));
    assert!(out[4].starts_with('├'));
    assert!(out[5].starts_with("│ Activity"));
    assert!(out[6].starts_with("│          "));
    assert!(out[7].starts_with('╰'));
}

#[test]
fn console_skips_empty_groups_for_separator_count() {
    let lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let groups = vec![
        (LineKind::Identity, 0..1),
        (LineKind::Config, 1..2),
        (LineKind::Budget, 2..2), // empty
        (LineKind::Activity, 2..3),
    ];
    let cfg = base_config(LayoutStyle::Console);
    let out = apply_pane(lines, &groups, &cfg);

    // Identity is in the title; 2 body content rows + top + 1 sep
    // (between the 2 non-empty body groups) + bottom = 5 rows.
    assert_eq!(out.len(), 5, "console row count; got {:#?}", out);
    let mids = out.iter().filter(|l| l.starts_with('├')).count();
    assert_eq!(mids, 1, "empty groups must not trigger separators");
}

#[test]
fn sections_keyword_falls_back_to_console() {
    // The `sections` layout was folded into `console` (its
    // identity-in-title sibling) in the 7→4 layout consolidation. The
    // parser maps the removed name → `console` (with a stderr warning)
    // so existing user configs keep a framed layout.
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"[layout]
name = "sections"
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, LayoutStyle::Console);
}

#[test]
fn cards_keyword_falls_back_to_console_after_consolidation() {
    // The `cards` layout was removed in the layout consolidation. The
    // parser maps removed names → `console` (with a stderr warning) so
    // existing user configs degrade gracefully rather than fall to plain
    // `none`.
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"[layout]
name = "cards"
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, LayoutStyle::Console);
}

#[test]
fn zones_keyword_falls_back_to_none() {
    // `zones` was removed in the 7→4 layout consolidation. It shared
    // none's visual defaults, so the parser maps it → `none` (with a
    // stderr warning) rather than surprise-flipping users to a framed
    // layout with a gauge quota.
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"[layout]
name = "zones"
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, LayoutStyle::None);
}

#[test]
fn grid_keyword_falls_back_to_none() {
    // Same consolidation rationale as `zones` — grid shared none's
    // visual defaults, so `none` is the least surprising fallback.
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"[layout]
name = "grid"
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, LayoutStyle::None);
}

#[test]
fn none_style_returns_lines_unchanged() {
    let lines = vec!["a".to_string(), "b".to_string()];
    let groups = vec![(LineKind::Identity, 0..1), (LineKind::Config, 1..2)];
    let cfg = base_config(LayoutStyle::None);
    let out = apply_pane(lines.clone(), &groups, &cfg);
    assert_eq!(out, lines, "None style must be a strict passthrough");
}

#[test]
fn config_defaults_pane_style_to_none() {
    let cfg = PulselineConfig::default();
    let render_cfg = build_render_config(&cfg);
    assert_eq!(
        render_cfg.pane_style,
        LayoutStyle::None,
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
name = "ledger"
min_width = 80
"#,
    )
    .expect("toml parse");
    let merged = merge_configs(user, &project);
    let render_cfg = build_render_config(&merged);
    assert_eq!(render_cfg.pane_style, LayoutStyle::Ledger);
    assert_eq!(render_cfg.pane_min_width, 80);
    assert_eq!(
        render_cfg.pane_max_width, 140,
        "max_width inherits user default"
    );
}
