use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    pub branch: String,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

impl Default for GitSnapshot {
    fn default() -> Self {
        Self {
            branch: "unknown".to_string(),
            dirty: false,
            ahead: 0,
            behind: 0,
        }
    }
}

pub trait GitCollector {
    fn collect_git(&self, cwd: &str) -> GitSnapshot;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalGitCollector;

impl GitCollector for LocalGitCollector {
    fn collect_git(&self, cwd: &str) -> GitSnapshot {
        let mut snapshot = GitSnapshot::default();

        let branch = git_stdout(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .or_else(|| git_stdout(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]));

        if let Some(branch) = branch {
            let trimmed = branch.trim();
            if !trimmed.is_empty() && trimmed != "HEAD" {
                snapshot.branch = trimmed.to_string();
            }
        } else {
            return snapshot;
        }

        if let Some(status_output) = git_stdout(cwd, &["status", "--porcelain=2", "--branch"]) {
            parse_status_output(&status_output, &mut snapshot);
        }

        snapshot
    }
}

#[derive(Debug, Default)]
pub struct StubGitCollector;

impl GitCollector for StubGitCollector {
    fn collect_git(&self, _cwd: &str) -> GitSnapshot {
        GitSnapshot::default()
    }
}

fn git_stdout(cwd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", cwd])
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn parse_status_output(status_output: &str, snapshot: &mut GitSnapshot) {
    for line in status_output.lines() {
        if let Some(value) = line.strip_prefix("# branch.head ") {
            let trimmed = value.trim();
            if !trimmed.is_empty() && trimmed != "(detached)" {
                snapshot.branch = trimmed.to_string();
            }
            continue;
        }

        if let Some(value) = line.strip_prefix("# branch.ab ") {
            for token in value.split_whitespace() {
                if let Some(ahead) = token.strip_prefix('+') {
                    snapshot.ahead = ahead.parse().unwrap_or(0);
                }
                if let Some(behind) = token.strip_prefix('-') {
                    snapshot.behind = behind.parse().unwrap_or(0);
                }
            }
            continue;
        }

        if line.starts_with("1 ")
            || line.starts_with("2 ")
            || line.starts_with("u ")
            || line.starts_with("? ")
            || line.starts_with("! ")
        {
            snapshot.dirty = true;
        }
    }
}
