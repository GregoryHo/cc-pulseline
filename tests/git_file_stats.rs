use std::{fs, path::Path, process::Command};

use cc_pulseline::{
    config::RenderConfig,
    render::color::{GIT_ADDED, GIT_DELETED, GIT_MODIFIED},
    run_from_str,
};
use serde_json::json;
use tempfile::TempDir;

fn run_cmd(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-C", dir.to_str().expect("utf-8 path")])
        .args(args)
        .status()
        .expect("git command should run");
    assert!(
        status.success(),
        "git command failed: git {}",
        args.join(" ")
    );
}

/// Build a git repo with various file states for testing git stats.
fn build_git_stats_workspace() -> TempDir {
    let tmp = TempDir::new().expect("tempdir should be created");
    let root = tmp.path();

    // Init git repo and create initial commit
    run_cmd(root, &["init"]);
    run_cmd(root, &["config", "user.email", "test@test.com"]);
    run_cmd(root, &["config", "user.name", "Test User"]);
    run_cmd(root, &["checkout", "-b", "stats-test"]);

    fs::write(root.join("existing.rs"), "fn main() {}\n").expect("write existing");
    fs::write(root.join("to_modify.rs"), "fn old() {}\n").expect("write to_modify");
    fs::write(root.join("to_delete.rs"), "fn doomed() {}\n").expect("write to_delete");
    run_cmd(root, &["add", "."]);
    run_cmd(root, &["commit", "-m", "initial"]);

    // Now create various file states:
    // Modified: edit an existing file
    fs::write(root.join("to_modify.rs"), "fn new() {}\n").expect("modify file");
    // Deleted: remove a tracked file
    fs::remove_file(root.join("to_delete.rs")).expect("delete file");
    // Added: create a new file and stage it
    fs::write(root.join("new_file.rs"), "fn added() {}\n").expect("add new file");
    run_cmd(root, &["add", "new_file.rs"]);
    // Untracked: create a file without staging
    fs::write(root.join("untracked.txt"), "untracked\n").expect("untracked file");
    fs::write(root.join("untracked2.txt"), "untracked2\n").expect("untracked2 file");

    tmp
}

fn make_input(cwd: &str) -> String {
    json!({
        "session_id": "git-stats-test",
        "cwd": cwd,
        "workspace": {"current_dir": cwd},
        "model": {"display_name": "Opus"},
        "version": "1.0",
    })
    .to_string()
}

#[test]
fn git_stats_hidden_by_default() {
    let workspace = build_git_stats_workspace();
    let cwd = workspace.path().to_str().unwrap();
    let input = make_input(cwd);

    let config = RenderConfig {
        show_git_stats: false,
        ..Default::default()
    };

    let lines = run_from_str(&input, config).expect("should render");
    let line1 = &lines[0];

    // Stats symbols should not appear
    assert!(
        !line1.contains("!"),
        "should not show ! when stats disabled"
    );
    assert!(
        !line1.contains("✘"),
        "should not show ✘ when stats disabled"
    );
    // The ? from untracked should not appear in stats format
    // (Note: branch name could contain these chars, so check more specifically)
    assert!(
        !line1.contains("?2"),
        "should not show ?N when stats disabled"
    );
}

#[test]
fn git_stats_show_counts() {
    let workspace = build_git_stats_workspace();
    let cwd = workspace.path().to_str().unwrap();
    let input = make_input(cwd);

    let config = RenderConfig {
        show_git_stats: true,
        ..Default::default()
    };

    let lines = run_from_str(&input, config).expect("should render");
    let line1 = &lines[0];

    // Should contain dirty marker and stats
    assert!(line1.contains("stats-test"), "should show branch name");
    assert!(line1.contains("*"), "should show dirty marker");
    assert!(line1.contains("!1"), "should show 1 modified file");
    assert!(line1.contains("+1"), "should show 1 added file");
    assert!(line1.contains("✘1"), "should show 1 deleted file");
    assert!(line1.contains("?2"), "should show 2 untracked files");
}

#[test]
fn git_stats_omit_zero_categories() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();

    // Create repo with only untracked files (no modified/added/deleted)
    run_cmd(root, &["init"]);
    run_cmd(root, &["config", "user.email", "test@test.com"]);
    run_cmd(root, &["config", "user.name", "Test User"]);
    run_cmd(root, &["checkout", "-b", "clean-test"]);
    fs::write(root.join("tracked.rs"), "fn main() {}\n").expect("tracked");
    run_cmd(root, &["add", "."]);
    run_cmd(root, &["commit", "-m", "initial"]);

    // Only add untracked files
    fs::write(root.join("new_untracked.txt"), "hello\n").expect("untracked");

    let cwd = root.to_str().unwrap();
    let input = make_input(cwd);
    let config = RenderConfig {
        show_git_stats: true,
        ..Default::default()
    };

    let lines = run_from_str(&input, config).expect("should render");
    let line1 = &lines[0];

    // Should show untracked but NOT modified/added/deleted
    assert!(line1.contains("?1"), "should show 1 untracked");
    assert!(!line1.contains("!"), "should not show modified when zero");
    assert!(
        !line1.contains("+"),
        "should not show added when zero (after branch)"
    );
    assert!(!line1.contains("✘"), "should not show deleted when zero");
}

#[test]
fn git_stats_color_output() {
    let workspace = build_git_stats_workspace();
    let cwd = workspace.path().to_str().unwrap();
    let input = make_input(cwd);

    let config = RenderConfig {
        show_git_stats: true,
        color_enabled: true,
        ..Default::default()
    };

    let lines = run_from_str(&input, config).expect("should render");
    let line1 = &lines[0];

    // Verify color codes are applied
    assert!(
        line1.contains(GIT_MODIFIED),
        "modified should use GIT_MODIFIED color"
    );
    assert!(
        line1.contains(GIT_ADDED),
        "added should use GIT_ADDED color"
    );
    assert!(
        line1.contains(GIT_DELETED),
        "deleted should use GIT_DELETED color"
    );
}
