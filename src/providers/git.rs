use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GitSnapshot {
    pub branch: String,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    #[serde(default)]
    pub modified_count: u32,
    #[serde(default)]
    pub added_count: u32,
    #[serde(default)]
    pub deleted_count: u32,
    #[serde(default)]
    pub untracked_count: u32,
}

impl Default for GitSnapshot {
    fn default() -> Self {
        Self {
            branch: "unknown".to_string(),
            dirty: false,
            ahead: 0,
            behind: 0,
            modified_count: 0,
            added_count: 0,
            deleted_count: 0,
            untracked_count: 0,
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
pub struct StubGitCollector {
    pub snapshot: GitSnapshot,
}

impl GitCollector for StubGitCollector {
    fn collect_git(&self, _cwd: &str) -> GitSnapshot {
        self.snapshot.clone()
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

        if line.starts_with("? ") {
            snapshot.dirty = true;
            snapshot.untracked_count += 1;
        } else if line.starts_with("1 ") {
            snapshot.dirty = true;
            classify_ordinary_change(line, snapshot);
        } else if line.starts_with("2 ") {
            // Renamed/copied entry
            snapshot.dirty = true;
            snapshot.modified_count += 1;
        } else if line.starts_with("u ") {
            // Unmerged entry
            snapshot.dirty = true;
            snapshot.modified_count += 1;
        }
    }
}

/// Classify an ordinary change entry (`1 XY ...`) by examining the index (X) and worktree (Y) codes.
/// Priority: D > A > M (each file counted once).
fn classify_ordinary_change(line: &str, snapshot: &mut GitSnapshot) {
    // Format: `1 XY <sub> <mH> <mI> <mW> <hH> <hI> <path>`
    // XY starts at byte offset 2
    let bytes = line.as_bytes();
    if bytes.len() < 4 {
        snapshot.modified_count += 1;
        return;
    }
    let x = bytes[2] as char;
    let y = bytes[3] as char;

    if x == 'D' || y == 'D' {
        snapshot.deleted_count += 1;
    } else if x == 'A' || y == 'A' {
        snapshot.added_count += 1;
    } else {
        // M, R, C, or any other non-'.' status
        snapshot.modified_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status_output_counts_files() {
        let porcelain = "\
# branch.oid abc123
# branch.head main
# branch.ab +2 -1
1 M. N... 100644 100644 100644 abc def src/main.rs
1 .M N... 100644 100644 100644 abc def src/lib.rs
1 A. N... 100644 100644 100644 abc def src/new.rs
1 .D N... 100644 100644 100644 abc def src/old.rs
1 D. N... 100644 100644 100644 abc def src/removed.rs
2 R. N... 100644 100644 100644 abc def renamed.rs\told.rs
u UU N... 100644 100644 100644 abc def conflict.rs
? untracked1.rs
? untracked2.rs";

        let mut snapshot = GitSnapshot::default();
        parse_status_output(porcelain, &mut snapshot);

        assert_eq!(snapshot.branch, "main");
        assert_eq!(snapshot.ahead, 2);
        assert_eq!(snapshot.behind, 1);
        assert!(snapshot.dirty);
        assert_eq!(snapshot.modified_count, 4); // 2 modified + 1 renamed + 1 unmerged
        assert_eq!(snapshot.added_count, 1);
        assert_eq!(snapshot.deleted_count, 2);
        assert_eq!(snapshot.untracked_count, 2);
    }

    #[test]
    fn parse_status_output_clean_repo() {
        let porcelain = "\
# branch.oid abc123
# branch.head feature/test
# branch.ab +0 -0";

        let mut snapshot = GitSnapshot::default();
        parse_status_output(porcelain, &mut snapshot);

        assert_eq!(snapshot.branch, "feature/test");
        assert!(!snapshot.dirty);
        assert_eq!(snapshot.modified_count, 0);
        assert_eq!(snapshot.added_count, 0);
        assert_eq!(snapshot.deleted_count, 0);
        assert_eq!(snapshot.untracked_count, 0);
    }

    #[test]
    fn stub_git_collector_returns_preset() {
        let stub = StubGitCollector {
            snapshot: GitSnapshot {
                branch: "test".to_string(),
                dirty: true,
                modified_count: 3,
                ..Default::default()
            },
        };
        let result = stub.collect_git("/any");
        assert_eq!(result.branch, "test");
        assert!(result.dirty);
        assert_eq!(result.modified_count, 3);
    }
}
