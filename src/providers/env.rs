use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnvSnapshot {
    pub claude_md_count: u32,
    pub rules_count: u32,
    pub hooks_count: u32,
    pub mcp_count: u32,
    pub memory_count: u32,
    pub skills_count: u32,
}

pub trait EnvCollector {
    fn collect_env(&self, cwd: &str) -> EnvSnapshot;
}

#[derive(Debug, Default, Clone)]
pub struct FileSystemEnvCollector {
    pub user_home_override: Option<PathBuf>,
}

impl EnvCollector for FileSystemEnvCollector {
    fn collect_env(&self, cwd: &str) -> EnvSnapshot {
        let root = Path::new(cwd);
        if !root.exists() {
            return EnvSnapshot::default();
        }

        let user_home = self.user_home_override.clone().or_else(|| {
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .ok()
                .map(PathBuf::from)
        });

        let mcp_count = count_mcp_servers_scoped(root, user_home.as_deref());

        let rules_count = count_md_files_recursive(&root.join(".claude/rules"))
            + user_home
                .as_ref()
                .map(|home| count_md_files_recursive(&home.join(".claude/rules")))
                .unwrap_or(0);

        let skills_count = count_skill_dirs(&root.join(".claude/skills"))
            + user_home
                .as_ref()
                .map(|home| count_skill_dirs(&home.join(".claude/skills")))
                .unwrap_or(0)
            + user_home
                .as_ref()
                .map(|home| count_plugin_skills(home))
                .unwrap_or(0);

        let memory_count = count_memory_files(user_home.as_deref(), cwd);

        EnvSnapshot {
            claude_md_count: count_claude_md(root, user_home.as_deref()),
            rules_count,
            memory_count,
            hooks_count: count_hooks_in_json(&root.join(".claude/settings.json"))
                + count_hooks_in_json(&root.join(".claude/settings.local.json"))
                + user_home
                    .as_ref()
                    .map(|h| count_hooks_in_json(&h.join(".claude/settings.json")))
                    .unwrap_or(0)
                + user_home
                    .as_ref()
                    .map(|h| count_plugin_hooks(h))
                    .unwrap_or(0),
            mcp_count,
            skills_count,
        }
    }
}

#[derive(Debug, Default)]
pub struct StubEnvCollector;

impl EnvCollector for StubEnvCollector {
    fn collect_env(&self, _cwd: &str) -> EnvSnapshot {
        EnvSnapshot::default()
    }
}

fn count_claude_md(root: &Path, user_home: Option<&Path>) -> u32 {
    let mut paths = vec![
        root.join("CLAUDE.md"),
        root.join("CLAUDE.local.md"),
        root.join(".claude/CLAUDE.md"),
        root.join(".claude/CLAUDE.local.md"),
    ];

    if let Some(home) = user_home {
        paths.push(home.join(".claude/CLAUDE.md"));
    }

    paths.iter().filter(|path| path.is_file()).count() as u32
}

fn count_md_files_recursive(path: &Path) -> u32 {
    if !path.exists() {
        return 0;
    }

    let mut count = 0;
    let mut stack = vec![PathBuf::from(path)];

    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    if ext == "md" {
                        count += 1;
                    }
                }
            }
        }
    }

    count
}

/// Read and parse a JSON file, returning None on any error.
fn read_json_file(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Extract `mcpServers` object keys from a parsed JSON value.
fn mcp_server_names_from(value: &serde_json::Value) -> HashSet<String> {
    value
        .get("mcpServers")
        .and_then(serde_json::Value::as_object)
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

/// Extract `mcpServers` object keys from a JSON file.
fn get_mcp_server_names(path: &Path) -> HashSet<String> {
    read_json_file(path)
        .map(|v| mcp_server_names_from(&v))
        .unwrap_or_default()
}

/// Extract disabled server names from a JSON array field (string values only).
fn disabled_servers_from(value: &serde_json::Value, key: &str) -> HashSet<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Extract disabled server names from a JSON file.
#[cfg(test)]
fn get_disabled_mcp_servers(path: &Path, key: &str) -> HashSet<String> {
    read_json_file(path)
        .map(|v| disabled_servers_from(&v, key))
        .unwrap_or_default()
}

/// Count MCP servers across user and project scopes with dedup + disabled filtering.
fn count_mcp_servers_scoped(root: &Path, user_home: Option<&Path>) -> u32 {
    let mut user_set = HashSet::new();
    let mut project_set = HashSet::new();

    // === User scope ===
    if let Some(home) = user_home {
        // ~/.claude/settings.json → mcpServers
        for name in get_mcp_server_names(&home.join(".claude/settings.json")) {
            user_set.insert(name);
        }

        // ~/.claude.json → mcpServers + disabledMcpServers (single read)
        if let Some(claude_json) = read_json_file(&home.join(".claude.json")) {
            for name in mcp_server_names_from(&claude_json) {
                user_set.insert(name);
            }
            for name in disabled_servers_from(&claude_json, "disabledMcpServers") {
                user_set.remove(&name);
            }
        }
    }

    // === Project scope ===

    // {root}/.mcp.json → mcpServers (tracked separately for disabled filtering)
    let mut mcp_json_servers = get_mcp_server_names(&root.join(".mcp.json"));

    // {root}/.claude/settings.json → mcpServers
    for name in get_mcp_server_names(&root.join(".claude/settings.json")) {
        project_set.insert(name);
    }

    // {root}/.claude/settings.local.json → mcpServers + disabledMcpjsonServers (single read)
    let local_settings = root.join(".claude/settings.local.json");
    if let Some(local_value) = read_json_file(&local_settings) {
        for name in mcp_server_names_from(&local_value) {
            project_set.insert(name);
        }
        for name in disabled_servers_from(&local_value, "disabledMcpjsonServers") {
            mcp_json_servers.remove(&name);
        }
    }

    // Remaining .mcp.json servers → add to project set
    for name in mcp_json_servers {
        project_set.insert(name);
    }

    // Union across scopes — a server configured in both counts once
    for name in project_set {
        user_set.insert(name);
    }
    user_set.len() as u32
}

/// Count individual hook handlers in a JSON file.
///
/// Structure: `{"hooks": {"EventType": [{"hooks": [handler, ...]}]}}`
/// Counts each entry in the inner `hooks` arrays across all event types and groups.
fn count_hooks_in_json(path: &Path) -> u32 {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let hooks_obj = match value.get("hooks").and_then(serde_json::Value::as_object) {
        Some(obj) => obj,
        None => return 0,
    };

    let mut total = 0u32;
    for (_event_type, groups) in hooks_obj {
        if let Some(groups_arr) = groups.as_array() {
            for group in groups_arr {
                if let Some(handlers) = group.get("hooks").and_then(|h| h.as_array()) {
                    total += handlers.len() as u32;
                }
            }
        }
    }
    total
}

/// Return install paths for all enabled plugins.
///
/// Reads `installed_plugins.json` for plugin entries, cross-references `enabledPlugins`
/// in `settings.json`, and returns the `installPath` of each enabled plugin.
fn get_enabled_plugin_paths(user_home: &Path) -> Vec<PathBuf> {
    let plugins_path = user_home.join(".claude/plugins/installed_plugins.json");
    let settings_path = user_home.join(".claude/settings.json");

    let plugins_text = match fs::read_to_string(&plugins_path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let plugins_value: serde_json::Value = match serde_json::from_str(&plugins_text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let settings_text = fs::read_to_string(&settings_path).unwrap_or_default();
    let settings_value: serde_json::Value =
        serde_json::from_str(&settings_text).unwrap_or(serde_json::Value::Null);
    let enabled_plugins = settings_value.get("enabledPlugins");

    let plugins_obj = match plugins_value.get("plugins").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return Vec::new(),
    };

    let mut paths = Vec::new();
    for (plugin_key, entries) in plugins_obj {
        let is_enabled = enabled_plugins
            .and_then(|ep| ep.get(plugin_key))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !is_enabled {
            continue;
        }

        let install_path = entries
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|entry| entry.get("installPath"))
            .and_then(|v| v.as_str());

        if let Some(path) = install_path {
            paths.push(PathBuf::from(path));
        }
    }

    paths
}

/// Count skills from enabled plugins by reading installed_plugins.json + settings.json.
fn count_plugin_skills(user_home: &Path) -> u32 {
    get_enabled_plugin_paths(user_home)
        .iter()
        .map(|path| count_skill_dirs(&path.join("skills")))
        .sum()
}

/// Count hook handlers from enabled plugins.
///
/// Each plugin may have `hooks/hooks.json` or `hooks/hook.json` (singular fallback).
fn count_plugin_hooks(user_home: &Path) -> u32 {
    get_enabled_plugin_paths(user_home)
        .iter()
        .map(|path| {
            let hooks_file = path.join("hooks/hooks.json");
            let hook_file = path.join("hooks/hook.json");
            if hooks_file.exists() {
                count_hooks_in_json(&hooks_file)
            } else if hook_file.exists() {
                count_hooks_in_json(&hook_file)
            } else {
                0
            }
        })
        .sum()
}

/// Encode a project path to match Claude Code's memory directory naming convention.
/// Replaces `/` and `.` with `-`, matching the format in `~/.claude/projects/`.
pub fn encode_project_path(path: &str) -> String {
    path.trim_end_matches('/').replace(['/', '.'], "-")
}

/// Count `.md` files in the project's memory directory (flat scan, no recursion).
fn count_memory_files(user_home: Option<&Path>, project_path: &str) -> u32 {
    let home = match user_home {
        Some(h) => h,
        None => return 0,
    };

    let encoded = encode_project_path(project_path);
    let memory_dir = home.join(".claude/projects").join(encoded).join("memory");

    let entries = match fs::read_dir(&memory_dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    entries
        .flatten()
        .filter(|entry| {
            entry.path().is_file()
                && entry
                    .path()
                    .extension()
                    .map(|ext| ext == "md")
                    .unwrap_or(false)
        })
        .count() as u32
}

fn count_skill_dirs(skills_root: &Path) -> u32 {
    let entries = match fs::read_dir(skills_root) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn count_claude_md_all_five_paths() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("project");
        let home = tmp.path().join("home");

        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();

        fs::write(root.join("CLAUDE.md"), "").unwrap();
        fs::write(root.join("CLAUDE.local.md"), "").unwrap();
        fs::write(root.join(".claude/CLAUDE.md"), "").unwrap();
        fs::write(root.join(".claude/CLAUDE.local.md"), "").unwrap();
        fs::write(home.join(".claude/CLAUDE.md"), "").unwrap();

        assert_eq!(count_claude_md(&root, Some(&home)), 5);
    }

    #[test]
    fn count_claude_md_no_user_home() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("CLAUDE.md"), "").unwrap();
        assert_eq!(count_claude_md(root, None), 1);
    }

    #[test]
    fn hooks_valid_settings_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        fs::write(
            &path,
            r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"x"}]}],"PostToolUse":[{"hooks":[{"type":"command","command":"y"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(count_hooks_in_json(&path), 2);
    }

    #[test]
    fn hooks_missing_file() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(count_hooks_in_json(&tmp.path().join("missing.json")), 0);
    }

    #[test]
    fn hooks_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.json");
        fs::write(&path, "not json").unwrap();
        assert_eq!(count_hooks_in_json(&path), 0);
    }

    #[test]
    fn hooks_no_hooks_key() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("empty.json");
        fs::write(&path, r#"{"other":"value"}"#).unwrap();
        assert_eq!(count_hooks_in_json(&path), 0);
    }

    #[test]
    fn mcp_server_names_extraction() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("mcp.json");
        fs::write(&path, r#"{"mcpServers":{"a":{},"b":{},"c":{}}}"#).unwrap();
        let names = get_mcp_server_names(&path);
        assert_eq!(names.len(), 3);
        assert!(names.contains("a"));
        assert!(names.contains("b"));
        assert!(names.contains("c"));
    }

    #[test]
    fn mcp_server_names_missing_file() {
        let tmp = TempDir::new().unwrap();
        assert!(get_mcp_server_names(&tmp.path().join("missing.json")).is_empty());
    }

    #[test]
    fn disabled_mcp_string_only_filtering() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        // Array with string + non-string values — only strings extracted
        fs::write(&path, r#"{"disabledMcpServers":["a",42,"b",null]}"#).unwrap();
        let disabled = get_disabled_mcp_servers(&path, "disabledMcpServers");
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains("a"));
        assert!(disabled.contains("b"));
    }

    #[test]
    fn mcp_scoped_disabled_filtering() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("project");
        let home = tmp.path().join("home");

        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();

        // User: settings.json has "user-mcp", .claude.json has "user-extra" but disables it
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"mcpServers":{"user-mcp":{}}}"#,
        )
        .unwrap();
        fs::write(
            home.join(".claude.json"),
            r#"{"mcpServers":{"user-extra":{}},"disabledMcpServers":["user-extra"]}"#,
        )
        .unwrap();

        // Project: .mcp.json has "proj-a" + "proj-disabled", settings.local disables "proj-disabled"
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"proj-a":{},"proj-disabled":{}}}"#,
        )
        .unwrap();
        fs::write(
            root.join(".claude/settings.local.json"),
            r#"{"disabledMcpjsonServers":["proj-disabled"]}"#,
        )
        .unwrap();

        // user_mcp=1 (user-extra disabled), project=1 (proj-disabled removed) → total 2
        assert_eq!(count_mcp_servers_scoped(&root, Some(&home)), 2);
    }

    #[test]
    fn mcp_cross_scope_dedup() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("project");
        let home = tmp.path().join("home");

        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();

        // Same name "shared" in both scopes → deduped to 1
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"mcpServers":{"shared":{}}}"#,
        )
        .unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"mcpServers":{"shared":{}}}"#,
        )
        .unwrap();

        assert_eq!(count_mcp_servers_scoped(&root, Some(&home)), 1);
    }

    #[test]
    fn plugin_skills_counts_enabled_only() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");

        // Create installed_plugins.json with two plugins
        let plugins_dir = home.join(".claude/plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        // Create skill directories for both plugins
        let plugin_a_path = tmp.path().join("cache/plugin-a");
        let plugin_b_path = tmp.path().join("cache/plugin-b");
        fs::create_dir_all(plugin_a_path.join("skills/commit")).unwrap();
        fs::create_dir_all(plugin_a_path.join("skills/review")).unwrap();
        fs::create_dir_all(plugin_b_path.join("skills/deploy")).unwrap();

        let installed = format!(
            r#"{{"version":2,"plugins":{{"plugin-a@org":[{{"scope":"user","installPath":"{}"}}],"plugin-b@org":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
            plugin_a_path.to_str().unwrap().replace('\\', "\\\\"),
            plugin_b_path.to_str().unwrap().replace('\\', "\\\\"),
        );
        fs::write(plugins_dir.join("installed_plugins.json"), &installed).unwrap();

        // Enable only plugin-a, disable plugin-b
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"plugin-a@org":true,"plugin-b@org":false}}"#,
        )
        .unwrap();

        // plugin-a has 2 skill dirs, plugin-b disabled → total 2
        assert_eq!(count_plugin_skills(&home), 2);
    }

    #[test]
    fn plugin_skills_missing_files() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("nonexistent_home");
        assert_eq!(count_plugin_skills(&home), 0);
    }

    #[test]
    fn plugin_skills_integrated_with_filesystem() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("project");
        let home = tmp.path().join("home");

        // Filesystem skills: 1 in project, 1 in user home
        fs::create_dir_all(root.join(".claude/skills/my-skill")).unwrap();
        fs::create_dir_all(home.join(".claude/skills/global-skill")).unwrap();

        // Plugin skills: 1 enabled plugin with 2 skill dirs
        fs::create_dir_all(home.join(".claude/plugins")).unwrap();
        let plugin_path = tmp.path().join("cache/plugin-x");
        fs::create_dir_all(plugin_path.join("skills/feat-a")).unwrap();
        fs::create_dir_all(plugin_path.join("skills/feat-b")).unwrap();

        let installed = format!(
            r#"{{"version":2,"plugins":{{"plugin-x@org":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
            plugin_path.to_str().unwrap().replace('\\', "\\\\"),
        );
        fs::write(
            home.join(".claude/plugins/installed_plugins.json"),
            &installed,
        )
        .unwrap();
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"plugin-x@org":true}}"#,
        )
        .unwrap();

        let collector = FileSystemEnvCollector {
            user_home_override: Some(home),
        };
        let snapshot = collector.collect_env(root.to_str().unwrap());
        // 1 project + 1 user + 2 plugin = 4
        assert_eq!(snapshot.skills_count, 4);
    }

    #[test]
    fn hooks_counts_multiple_handlers_per_group() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        // Notification has 2 handlers in one group + Stop has 1 handler → total 3
        fs::write(
            &path,
            r#"{"hooks":{"Notification":[{"hooks":[{"type":"command","command":"a"},{"type":"command","command":"b"}]}],"Stop":[{"hooks":[{"type":"command","command":"c"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(count_hooks_in_json(&path), 3);
    }

    #[test]
    fn hooks_empty_event_type_array() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        // Event types with empty arrays or empty hooks → 0 handlers
        fs::write(
            &path,
            r#"{"hooks":{"PreToolUse":[],"PostToolUse":[{"hooks":[]}]}}"#,
        )
        .unwrap();
        assert_eq!(count_hooks_in_json(&path), 0);
    }

    #[test]
    fn plugin_hooks_counts_enabled_only() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");

        let plugins_dir = home.join(".claude/plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        // Plugin A: enabled, has hooks/hooks.json with 2 handlers
        let plugin_a = tmp.path().join("cache/plugin-a");
        fs::create_dir_all(plugin_a.join("hooks")).unwrap();
        fs::write(
            plugin_a.join("hooks/hooks.json"),
            r#"{"hooks":{"Notification":[{"hooks":[{"type":"command","command":"a"},{"type":"command","command":"b"}]}]}}"#,
        )
        .unwrap();

        // Plugin B: disabled, has hooks/hooks.json with 1 handler
        let plugin_b = tmp.path().join("cache/plugin-b");
        fs::create_dir_all(plugin_b.join("hooks")).unwrap();
        fs::write(
            plugin_b.join("hooks/hooks.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"c"}]}]}}"#,
        )
        .unwrap();

        let installed = format!(
            r#"{{"version":2,"plugins":{{"plugin-a@org":[{{"scope":"user","installPath":"{}"}}],"plugin-b@org":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
            plugin_a.to_str().unwrap().replace('\\', "\\\\"),
            plugin_b.to_str().unwrap().replace('\\', "\\\\"),
        );
        fs::write(plugins_dir.join("installed_plugins.json"), &installed).unwrap();

        fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"plugin-a@org":true,"plugin-b@org":false}}"#,
        )
        .unwrap();

        // Only plugin-a is enabled → 2 handlers
        assert_eq!(count_plugin_hooks(&home), 2);
    }

    #[test]
    fn plugin_hooks_both_json_filenames() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");

        let plugins_dir = home.join(".claude/plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        // Plugin uses hook.json (singular) instead of hooks.json
        let plugin_path = tmp.path().join("cache/plugin-singular");
        fs::create_dir_all(plugin_path.join("hooks")).unwrap();
        fs::write(
            plugin_path.join("hooks/hook.json"),
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x"}]}]}}"#,
        )
        .unwrap();

        let installed = format!(
            r#"{{"version":2,"plugins":{{"plugin-singular@org":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
            plugin_path.to_str().unwrap().replace('\\', "\\\\"),
        );
        fs::write(plugins_dir.join("installed_plugins.json"), &installed).unwrap();
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"plugin-singular@org":true}}"#,
        )
        .unwrap();

        assert_eq!(count_plugin_hooks(&home), 1);
    }

    #[test]
    fn plugin_hooks_missing_hooks_dir() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");

        let plugins_dir = home.join(".claude/plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        // Plugin exists but has no hooks directory
        let plugin_path = tmp.path().join("cache/plugin-no-hooks");
        fs::create_dir_all(&plugin_path).unwrap();

        let installed = format!(
            r#"{{"version":2,"plugins":{{"plugin-no-hooks@org":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
            plugin_path.to_str().unwrap().replace('\\', "\\\\"),
        );
        fs::write(plugins_dir.join("installed_plugins.json"), &installed).unwrap();
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"plugin-no-hooks@org":true}}"#,
        )
        .unwrap();

        assert_eq!(count_plugin_hooks(&home), 0);
    }

    // ── Memory file counting tests ──────────────────────────────────

    #[test]
    fn encode_project_path_replaces_slashes_and_dots() {
        assert_eq!(
            encode_project_path("/Users/gregho/GitHub/AI/cc-pulseline"),
            "-Users-gregho-GitHub-AI-cc-pulseline"
        );
    }

    #[test]
    fn encode_project_path_handles_dots() {
        assert_eq!(
            encode_project_path("/Users/greg.ho/my.project"),
            "-Users-greg-ho-my-project"
        );
    }

    #[test]
    fn encode_project_path_strips_trailing_slash() {
        assert_eq!(
            encode_project_path("/Users/gregho/project/"),
            "-Users-gregho-project"
        );
    }

    #[test]
    fn memory_count_md_files_only() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let project_path = "/Users/gregho/myproject";
        let encoded = encode_project_path(project_path);
        let memory_dir = home.join(".claude/projects").join(&encoded).join("memory");
        fs::create_dir_all(&memory_dir).unwrap();

        fs::write(memory_dir.join("MEMORY.md"), "# notes").unwrap();
        fs::write(memory_dir.join("patterns.md"), "# patterns").unwrap();
        fs::write(memory_dir.join("debugging.md"), "# debug").unwrap();
        fs::write(memory_dir.join("notes.txt"), "not counted").unwrap();
        fs::write(memory_dir.join("data.json"), "{}").unwrap();

        assert_eq!(count_memory_files(Some(&home), project_path), 3);
    }

    #[test]
    fn memory_count_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let project_path = "/Users/gregho/empty";
        let encoded = encode_project_path(project_path);
        let memory_dir = home.join(".claude/projects").join(&encoded).join("memory");
        fs::create_dir_all(&memory_dir).unwrap();

        assert_eq!(count_memory_files(Some(&home), project_path), 0);
    }

    #[test]
    fn memory_count_nonexistent_dir() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        assert_eq!(
            count_memory_files(Some(&home), "/Users/gregho/nonexistent"),
            0
        );
    }

    #[test]
    fn memory_count_no_home() {
        assert_eq!(count_memory_files(None, "/some/project"), 0);
    }
}
