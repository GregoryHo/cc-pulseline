use cc_pulseline::config::{merge_configs, ProjectOverrideConfig, PulselineConfig};

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
