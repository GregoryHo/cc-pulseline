use cc_pulseline::config::{
    build_render_config, merge_configs, ProjectOverrideConfig, PulselineConfig,
};

#[test]
fn merge_with_empty_project_is_noop() {
    let user = PulselineConfig::default();
    let project = ProjectOverrideConfig::default();
    let merged = merge_configs(user.clone(), &project);

    assert_eq!(merged.display.theme, "dark");
    assert!(merged.display.icons);
    assert!(merged.segments.identity.show_model);
    assert!(merged.segments.config.show_claude_md);
    assert!(merged.segments.budget.show_context);
    assert!(merged.segments.tools.enabled);
    assert_eq!(merged.segments.tools.max_lines, 2);
}

#[test]
fn merge_project_overrides_theme() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[display]
theme = "light"
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert_eq!(merged.display.theme, "light");
    assert!(
        merged.display.icons,
        "icons should inherit from user default"
    );
}

#[test]
fn merge_project_overrides_partial_identity() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.identity]
show_style = false
show_version = false
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.identity.show_model,
        "model should inherit true"
    );
    assert!(
        !merged.segments.identity.show_style,
        "style should be overridden to false"
    );
    assert!(
        !merged.segments.identity.show_version,
        "version should be overridden to false"
    );
    assert!(
        merged.segments.identity.show_project,
        "project should inherit true"
    );
    assert!(merged.segments.identity.show_git, "git should inherit true");
}

#[test]
fn merge_project_overrides_tools_config() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.tools]
max_completed = 8
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(merged.segments.tools.enabled, "enabled should inherit true");
    assert_eq!(
        merged.segments.tools.max_lines, 2,
        "max_lines should inherit default"
    );
    assert_eq!(
        merged.segments.tools.max_completed, 8,
        "max_completed should be overridden"
    );
}

#[test]
fn merge_project_overrides_show_memory() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.config]
show_memory = false
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.config.show_claude_md,
        "claude_md should inherit"
    );
    assert!(merged.segments.config.show_rules, "rules should inherit");
    assert!(
        !merged.segments.config.show_memory,
        "memory should be overridden to false"
    );
    assert!(merged.segments.config.show_hooks, "hooks should inherit");
}

#[test]
fn merge_project_overrides_budget_and_config() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.budget]
show_tokens = false

[segments.config]
show_skills = false
show_duration = false
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.budget.show_context,
        "context should inherit"
    );
    assert!(
        !merged.segments.budget.show_tokens,
        "tokens should be overridden"
    );
    assert!(merged.segments.budget.show_cost, "cost should inherit");
    assert!(
        merged.segments.config.show_claude_md,
        "claude_md should inherit"
    );
    assert!(
        !merged.segments.config.show_skills,
        "skills should be overridden"
    );
    assert!(
        !merged.segments.config.show_duration,
        "duration should be overridden"
    );
}

#[test]
fn merge_full_project_override() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[display]
theme = "light"
icons = false

[segments.identity]
show_model = false
show_style = false
show_version = false
show_project = false
show_git = false

[segments.tools]
enabled = false
max_lines = 5
max_completed = 10

[segments.agents]
enabled = false
max_lines = 3

[segments.todo]
enabled = false
max_lines = 1
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert_eq!(merged.display.theme, "light");
    assert!(!merged.display.icons);
    assert!(!merged.segments.identity.show_model);
    assert!(!merged.segments.tools.enabled);
    assert_eq!(merged.segments.tools.max_lines, 5);
    assert_eq!(merged.segments.tools.max_completed, 10);
    assert!(!merged.segments.agents.enabled);
    assert_eq!(merged.segments.agents.max_lines, 3);
    assert!(!merged.segments.todo.enabled);
    assert_eq!(merged.segments.todo.max_lines, 1);
}

#[test]
fn merge_project_overrides_git_stats() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.identity]
show_git_stats = true
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.identity.show_git_stats,
        "git_stats should be overridden to true"
    );
    assert!(
        merged.segments.identity.show_git,
        "show_git should inherit default (true)"
    );
}

#[test]
fn merge_project_overrides_show_speed() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.budget]
show_speed = true
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.budget.show_speed,
        "show_speed should be overridden to true"
    );
    assert!(
        merged.segments.budget.show_context,
        "show_context should inherit default (true)"
    );
    assert!(
        merged.segments.budget.show_cost,
        "show_cost should inherit default (true)"
    );
}

#[test]
fn merge_project_overrides_quota() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.quota]
enabled = true
show_seven_day = true
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.quota.enabled,
        "quota enabled should be overridden to true"
    );
    assert!(
        merged.segments.quota.show_seven_day,
        "show_seven_day should be overridden to true"
    );
    assert!(
        merged.segments.quota.show_five_hour,
        "show_five_hour should inherit default (true)"
    );
}

#[test]
fn project_override_config_deserializes_empty() {
    let project: ProjectOverrideConfig = toml::from_str("").unwrap();
    assert!(project.display.is_none());
    assert!(project.segments.is_none());
}

#[test]
fn show_cache_trend_parses_and_reaches_render_config() {
    let user: PulselineConfig = toml::from_str(
        r#"
[segments.budget]
show_cache_trend = true
"#,
    )
    .unwrap();

    let render_cfg = build_render_config(&user);
    assert!(
        render_cfg.show_cache_trend,
        "show_cache_trend = true should reach RenderConfig"
    );
}

#[test]
fn show_cache_trend_defaults_to_false() {
    let user = PulselineConfig::default();
    assert!(!user.segments.budget.show_cache_trend);

    let render_cfg = build_render_config(&user);
    assert!(
        !render_cfg.show_cache_trend,
        "show_cache_trend should default to false"
    );
}

#[test]
fn merge_project_overrides_show_cache_trend() {
    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[segments.budget]
show_cache_trend = true
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    assert!(
        merged.segments.budget.show_cache_trend,
        "show_cache_trend should be overridden to true"
    );
    assert!(
        !merged.segments.budget.show_speed,
        "show_speed should inherit default (false)"
    );
}

#[test]
fn layout_dials_parse_valid_and_fall_back_on_unknown() {
    use cc_pulseline::config::{ColorBudget, Headline};

    // valid strings parse to their enums.
    let cfg: PulselineConfig = toml::from_str(
        r#"
[layout]
color_budget = "vivid"
headline = "inline"
"#,
    )
    .unwrap();
    let rc = build_render_config(&cfg);
    assert_eq!(rc.pane_color_budget, ColorBudget::Vivid);
    assert_eq!(rc.pane_headline, Headline::Inline);

    // unknown strings warn (stderr) and fall back to the defaults — the Must.
    let cfg: PulselineConfig = toml::from_str(
        r#"
[layout]
color_budget = "bogus"
headline = "nonsense"
"#,
    )
    .unwrap();
    let rc = build_render_config(&cfg);
    assert_eq!(rc.pane_color_budget, ColorBudget::Signal);
    assert_eq!(rc.pane_headline, Headline::Column);

    // absent fields default to signal / column.
    let rc = build_render_config(&PulselineConfig::default());
    assert_eq!(rc.pane_color_budget, ColorBudget::Signal);
    assert_eq!(rc.pane_headline, Headline::Column);
}

#[test]
fn merge_project_overrides_layout_dials() {
    use cc_pulseline::config::{ColorBudget, Headline};

    let user = PulselineConfig::default();
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[layout]
color_budget = "mono"
headline = "inline"
"#,
    )
    .unwrap();

    let merged = merge_configs(user, &project);
    let rc = build_render_config(&merged);
    assert_eq!(
        rc.pane_color_budget,
        ColorBudget::Mono,
        "project color_budget should win"
    );
    assert_eq!(
        rc.pane_headline,
        Headline::Inline,
        "project headline should win"
    );
}

#[test]
fn default_templates_parse_cleanly() {
    use cc_pulseline::config::{default_config_toml, default_project_config_toml};

    // The active --init template must round-trip (it had no parse test before).
    let user: PulselineConfig =
        toml::from_str(default_config_toml()).expect("default user template must parse");
    // The commented rail_* arrangement examples are inert → fields stay empty.
    assert!(user.layout.rail_identity_order.is_empty());
    assert!(user.layout.rail_usage_hero.is_empty());
    // ...while the active dials parse to their documented defaults.
    assert_eq!(user.layout.color_budget, "signal");
    assert_eq!(user.layout.headline, "column");

    let _project: ProjectOverrideConfig =
        toml::from_str(default_project_config_toml()).expect("default project template must parse");
}

#[test]
fn merge_project_overrides_rail_arrangement() {
    let user = PulselineConfig::default();
    // Distinct per-row values so a cross-field mis-wire in merge_configs or
    // build_render_config (6 near-identical lines each) can't pass.
    let project: ProjectOverrideConfig = toml::from_str(
        r#"
[layout]
rail_identity_order = ["git", "model"]
rail_identity_hero = "git"
rail_usage_order = ["cost", "ctx"]
rail_usage_hero = "ctx"
rail_quota_order = ["7d", "5h"]
rail_quota_hero = "5h"
"#,
    )
    .unwrap();

    let rc = build_render_config(&merge_configs(user, &project));
    assert_eq!(rc.rail_identity_order, vec!["git", "model"]);
    assert_eq!(rc.rail_identity_hero, "git");
    assert_eq!(rc.rail_usage_order, vec!["cost", "ctx"]);
    assert_eq!(rc.rail_usage_hero, "ctx");
    assert_eq!(rc.rail_quota_order, vec!["7d", "5h"]);
    assert_eq!(rc.rail_quota_hero, "5h");
}
