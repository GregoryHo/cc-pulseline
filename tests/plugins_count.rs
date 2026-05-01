//! L2 plugins segment (CC 2.0.12+).
//!
//! Plugins are counted from `~/.claude/plugins/installed_plugins.json`
//! cross-referenced with `enabledPlugins` in `~/.claude/settings.json`.

use std::fs;
use std::path::Path;

use cc_pulseline::{config::RenderConfig, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

/// Create a fake HOME with the given set of installed / enabled plugins.
///
/// `entries` is a slice of `(plugin_key, enabled)` tuples. Plugin keys follow
/// the CC convention `"<name>@<marketplace>"`.
fn setup_fake_home(entries: &[(&str, bool)]) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let home = tmp.path();
    let claude_dir = home.join(".claude");
    let plugins_dir = claude_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).expect("plugins dir");

    let mut plugins_map = serde_json::Map::new();
    let mut enabled_map = serde_json::Map::new();
    for (idx, (key, enabled)) in entries.iter().enumerate() {
        let install_path = plugins_dir.join(format!("cache-{idx}"));
        fs::create_dir_all(&install_path).expect("install path");
        plugins_map.insert(
            key.to_string(),
            json!([{"installPath": install_path.to_str().unwrap(), "version": "1.0.0"}]),
        );
        enabled_map.insert(key.to_string(), json!(*enabled));
    }

    fs::write(
        plugins_dir.join("installed_plugins.json"),
        serde_json::to_string_pretty(&json!({"plugins": plugins_map})).unwrap(),
    )
    .expect("installed_plugins.json");

    fs::write(
        claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&json!({"enabledPlugins": enabled_map})).unwrap(),
    )
    .expect("settings.json");

    tmp
}

fn run_with_home(home: &Path) -> Vec<String> {
    let mut runner = PulseLineRunner::default().with_user_home(home.to_path_buf());
    let input = json!({
        "session_id": "plugins-test",
        "cwd": home.to_str().unwrap(),
    })
    .to_string();
    runner
        .run_from_str(&input, RenderConfig::default())
        .expect("render succeeds")
}

fn line2(lines: &[String]) -> &str {
    lines.get(1).expect("line2 present").as_str()
}

#[test]
fn counts_only_enabled_plugins() {
    // 3 installed, 2 enabled, 1 disabled → should count 2.
    let tmp = setup_fake_home(&[
        ("alpha@market", true),
        ("beta@market", true),
        ("gamma@market", false),
    ]);
    let lines = run_with_home(tmp.path());
    assert!(
        line2(&lines).contains("2 plugins"),
        "expected '2 plugins' on L2, got: {}",
        line2(&lines)
    );
}

#[test]
fn hides_segment_when_zero_plugins() {
    let tmp = setup_fake_home(&[("alpha@market", false), ("beta@market", false)]);
    let lines = run_with_home(tmp.path());
    assert!(
        !line2(&lines).contains("plugins"),
        "segment should be hidden when count is 0, got: {}",
        line2(&lines)
    );
}

#[test]
fn hides_segment_when_no_plugins_directory() {
    // No installed_plugins.json at all.
    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join(".claude")).expect("claude dir");
    let lines = run_with_home(tmp.path());
    assert!(!line2(&lines).contains("plugins"));
}

#[test]
fn show_plugins_toggle_hides_segment_when_false() {
    let tmp = setup_fake_home(&[("alpha@market", true), ("beta@market", true)]);
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().to_path_buf());
    let input = json!({"session_id": "t", "cwd": tmp.path().to_str().unwrap()}).to_string();
    let config = RenderConfig {
        show_plugins: false,
        ..RenderConfig::default()
    };
    let lines = runner.run_from_str(&input, config).expect("render");
    assert!(!line2(&lines).contains("plugins"));
}

#[test]
fn handles_missing_enabled_plugins_key() {
    // installed_plugins.json exists but settings.json has no enabledPlugins key.
    // No plugin should be counted (enabledPlugins defaults to all-disabled).
    let tmp = TempDir::new().expect("tempdir");
    let claude = tmp.path().join(".claude");
    let plugins_dir = claude.join("plugins");
    fs::create_dir_all(&plugins_dir).expect("dirs");
    fs::write(
        plugins_dir.join("installed_plugins.json"),
        r#"{"plugins":{"a@m":[{"installPath":"/tmp/a"}]}}"#,
    )
    .expect("installed_plugins");
    fs::write(claude.join("settings.json"), r#"{}"#).expect("settings");

    let lines = run_with_home(tmp.path());
    assert!(!line2(&lines).contains("plugins"));
}

#[test]
fn plugin_provided_mcps_count_towards_mcp_total() {
    // Enabled plugin with a .mcp.json exposing 2 servers should add 2 to the
    // L2 "MCPs" count. Dedup guarantees the same-named server doesn't
    // double-count if the user also configured it.
    let tmp = setup_fake_home(&[("alpha@market", true)]);

    // Find the plugin's install dir and drop a .mcp.json in it.
    let plugins_root = tmp.path().join(".claude/plugins");
    let install_dir = plugins_root.join("cache-0");
    fs::write(
        install_dir.join(".mcp.json"),
        r#"{"mcpServers":{"plugin-mcp-1":{},"plugin-mcp-2":{}}}"#,
    )
    .expect(".mcp.json write");

    let lines = run_with_home(tmp.path());
    let l2 = line2(&lines);
    assert!(
        l2.contains("2 MCPs"),
        "expected 2 MCPs from plugin, got: {l2}"
    );
}

#[test]
fn plugin_mcps_dedup_against_user_scope() {
    // Same server name in both user scope and plugin scope counts once.
    let tmp = setup_fake_home(&[("alpha@market", true)]);

    let plugins_root = tmp.path().join(".claude/plugins");
    let install_dir = plugins_root.join("cache-0");
    fs::write(
        install_dir.join(".mcp.json"),
        r#"{"mcpServers":{"shared":{}}}"#,
    )
    .expect("plugin mcp");

    // Re-write settings.json to add `mcpServers` alongside enabledPlugins.
    let settings_path = tmp.path().join(".claude/settings.json");
    let existing = fs::read_to_string(&settings_path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&existing).unwrap();
    value["mcpServers"] = json!({"shared": {}});
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();

    let lines = run_with_home(tmp.path());
    let l2 = line2(&lines);
    assert!(
        l2.contains("1 MCPs"),
        "same server in user + plugin should dedup to 1, got: {l2}"
    );
}

#[test]
fn plugins_segment_appears_after_skills() {
    // Layout ordering: ... | N skills | N plugins | <duration>
    let tmp = setup_fake_home(&[("alpha@market", true)]);
    let lines = run_with_home(tmp.path());
    let l2 = line2(&lines);

    let skills_pos = l2.find("skills").expect("skills segment present");
    let plugins_pos = l2.find("plugins").expect("plugins segment present");

    assert!(
        skills_pos < plugins_pos,
        "plugins should appear after skills on L2: {}",
        l2
    );
}
