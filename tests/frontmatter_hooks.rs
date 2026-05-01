//! Frontmatter hook counting for skills and agents.
//!
//! Claude Code 2.1.0+ supports declaring hooks in skill SKILL.md frontmatter,
//! and 2.1.43+ supports the same in agent `.md` frontmatter. cc-pulseline's
//! L2 `hooks` count needs to include these alongside settings.json hooks.

use std::fs;

use cc_pulseline::{config::RenderConfig, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

fn setup_workspace() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    fs::create_dir_all(root.join(".claude/skills")).expect("skills dir");
    fs::create_dir_all(root.join(".claude/agents")).expect("agents dir");
    tmp
}

fn render(ws: &std::path::Path, home: &std::path::Path) -> Vec<String> {
    let input = json!({
        "session_id": "fm-hooks",
        "cwd": ws,
        "workspace": {"current_dir": ws},
    })
    .to_string();

    let mut runner = PulseLineRunner::default().with_user_home(home.to_path_buf());
    runner
        .run_from_str(&input, RenderConfig::default())
        .expect("render")
}

fn line2(lines: &[String]) -> &str {
    lines.get(1).expect("line2").as_str()
}

fn hooks_count(line2: &str) -> u32 {
    // Line2 format: "... | 1 hooks | ..." — extract the number before " hooks".
    line2
        .split('|')
        .find_map(|seg| {
            let trimmed = seg.trim();
            trimmed
                .strip_suffix(" hooks")
                .and_then(|n| n.parse::<u32>().ok())
        })
        .unwrap_or(0)
}

#[test]
fn counts_skill_frontmatter_hooks() {
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");
    fs::create_dir_all(ws.path().join(".claude/skills/my-skill")).unwrap();
    fs::write(
        ws.path().join(".claude/skills/my-skill/SKILL.md"),
        r#"---
name: my-skill
description: A skill
hooks:
  PreToolUse:
    - type: command
      command: echo pre
  PostToolUse:
    - type: command
      command: echo post
---

Skill body.
"#,
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(
        hooks_count(line2(&lines)),
        2,
        "expected 2 frontmatter hooks from SKILL.md, got: {}",
        line2(&lines)
    );
}

#[test]
fn counts_agent_frontmatter_hooks() {
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");
    fs::write(
        ws.path().join(".claude/agents/reviewer.md"),
        r#"---
name: reviewer
hooks:
  Stop:
    - type: command
      command: notify
---

Agent body.
"#,
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(hooks_count(line2(&lines)), 1);
}

#[test]
fn skills_and_agents_frontmatter_combine_with_settings_json() {
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");

    // settings.json hook
    fs::write(
        ws.path().join(".claude/settings.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"s"}]}]}}"#,
    )
    .unwrap();

    // skill frontmatter hook
    fs::create_dir_all(ws.path().join(".claude/skills/foo")).unwrap();
    fs::write(
        ws.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\nhooks:\n  Stop:\n    - type: command\n      command: x\n---\n",
    )
    .unwrap();

    // agent frontmatter hook
    fs::write(
        ws.path().join(".claude/agents/bar.md"),
        "---\nname: bar\nhooks:\n  Stop:\n    - type: command\n      command: y\n---\n",
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(
        hooks_count(line2(&lines)),
        3,
        "expected 1 settings + 1 skill + 1 agent = 3, got: {}",
        line2(&lines)
    );
}

#[test]
fn counts_user_scope_skill_hooks() {
    // Skills in ~/.claude/skills with frontmatter hooks should also be counted.
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");
    fs::create_dir_all(home.path().join(".claude/skills/user-skill")).unwrap();
    fs::write(
        home.path().join(".claude/skills/user-skill/SKILL.md"),
        "---\nname: user-skill\nhooks:\n  Stop:\n    - type: command\n      command: x\n---\n",
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(hooks_count(line2(&lines)), 1);
}

#[test]
fn skill_without_hooks_key_contributes_zero() {
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");
    fs::create_dir_all(ws.path().join(".claude/skills/plain")).unwrap();
    fs::write(
        ws.path().join(".claude/skills/plain/SKILL.md"),
        "---\nname: plain\ndescription: no hooks here\n---\n\nbody.\n",
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(hooks_count(line2(&lines)), 0);
}

#[test]
fn malformed_frontmatter_is_safely_ignored() {
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");
    fs::create_dir_all(ws.path().join(".claude/skills/broken")).unwrap();
    // File with no leading ---
    fs::write(
        ws.path().join(".claude/skills/broken/SKILL.md"),
        "This skill has no frontmatter.\nhooks: but mentioned in body\n",
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(hooks_count(line2(&lines)), 0, "no frontmatter → zero hooks");
}

#[test]
fn nested_skills_directory_is_walked() {
    let ws = setup_workspace();
    let home = TempDir::new().expect("home");
    // CC 2.1.6 added nested `.claude/skills` discovery in subdirectories.
    fs::create_dir_all(ws.path().join(".claude/skills/category/deep-skill")).unwrap();
    fs::write(
        ws.path()
            .join(".claude/skills/category/deep-skill/SKILL.md"),
        "---\nname: deep\nhooks:\n  Stop:\n    - type: command\n      command: x\n---\n",
    )
    .unwrap();

    let lines = render(ws.path(), home.path());
    assert_eq!(hooks_count(line2(&lines)), 1);
}
