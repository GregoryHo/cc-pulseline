use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvSnapshot {
    pub claude_md_count: u32,
    pub rules_count: u32,
    pub hooks_count: u32,
    pub mcp_count: u32,
    pub skills_count: u32,
}

pub trait EnvCollector {
    fn collect_env(&self, cwd: &str) -> EnvSnapshot;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemEnvCollector;

impl EnvCollector for FileSystemEnvCollector {
    fn collect_env(&self, cwd: &str) -> EnvSnapshot {
        let root = Path::new(cwd);
        if !root.exists() {
            return EnvSnapshot::default();
        }

        let user_home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(PathBuf::from);

        let mcp_count = count_mcp_servers(root)
            .unwrap_or_else(|| count_files_recursive(&root.join(".claude/mcp")));

        let rules_count = count_md_files_recursive(&root.join(".claude/rules"))
            + user_home
                .as_ref()
                .map(|home| count_md_files_recursive(&home.join(".claude/rules")))
                .unwrap_or(0);

        let skills_count = count_skill_dirs(&root.join(".codex/skills"))
            + count_skill_dirs(&root.join(".claude/skills"))
            + user_home
                .as_ref()
                .map(|home| count_skill_dirs(&home.join(".claude/skills")))
                .unwrap_or(0);

        EnvSnapshot {
            claude_md_count: count_claude_md(root),
            rules_count,
            hooks_count: count_files_recursive(&root.join(".claude/hooks")),
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

fn count_claude_md(root: &Path) -> u32 {
    [root.join("CLAUDE.md"), root.join(".claude/CLAUDE.md")]
        .iter()
        .filter(|path| path.is_file())
        .count() as u32
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

fn count_files_recursive(path: &Path) -> u32 {
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
                count += 1;
            }
        }
    }

    count
}

fn count_mcp_servers(root: &Path) -> Option<u32> {
    let candidates = [root.join(".claude/mcp.json"), root.join(".mcp.json")];

    for path in candidates {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(servers) = value.get("servers").and_then(serde_json::Value::as_object) {
            if !servers.is_empty() {
                return Some(servers.len() as u32);
            }
        }
    }

    None
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
