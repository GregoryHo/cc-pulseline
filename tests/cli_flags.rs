use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn print_flag_shows_new_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--print")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();

    // New fields from feature/adjust-metrics should appear
    assert!(
        stdout.contains("show_git_stats"),
        "should show show_git_stats field"
    );
    assert!(
        stdout.contains("show_speed"),
        "should show show_speed field"
    );
    assert!(
        stdout.contains("[segments.quota]"),
        "should show quota section"
    );
    assert!(
        stdout.contains("show_five_hour"),
        "should show show_five_hour field"
    );
    assert!(
        stdout.contains("show_seven_day"),
        "should show show_seven_day field"
    );
}

#[test]
fn help_flag_exits_zero_with_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--help")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("USAGE:"), "should contain USAGE section");
    assert!(stdout.contains("--init"), "should mention --init");
    assert!(stdout.contains("NO_COLOR"), "should mention NO_COLOR");
}

#[test]
fn short_help_flag_works() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("-h")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("USAGE:"), "should contain USAGE section");
}

#[test]
fn version_flag_shows_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--version")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("cc-pulseline"),
        "should contain binary name"
    );
    assert!(stdout.contains("1.1.0"), "should contain version number");
}

#[test]
fn short_version_flag_works() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("-V")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("cc-pulseline 1.1.0"),
        "should show name and version"
    );
}

#[test]
fn select_theme_shows_list_and_handles_invalid() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--select-theme")
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    // Send invalid selection to trigger exit
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"0\n")
        .expect("failed to write stdin");

    let output = child.wait_with_output().expect("failed to wait");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        stdout.contains("Available themes"),
        "should show theme list header"
    );
    assert!(stdout.contains("tokyo-night"), "should list tokyo-night");
    assert!(
        stdout.contains("echo-sub-zero"),
        "should list echo-sub-zero"
    );
    assert!(
        stderr.contains("Invalid selection"),
        "should reject 0 as invalid"
    );
}

#[test]
fn select_theme_writes_config() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let fake_home = tmp.path().to_str().unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--select-theme")
        .env("HOME", fake_home)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    // Select theme #2 (echo-sub-zero)
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"2\n")
        .expect("failed to write stdin");

    let output = child.wait_with_output().expect("failed to wait");
    assert!(output.status.success(), "should exit 0");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("echo-sub-zero"),
        "should show selected theme name"
    );

    // Verify config was written
    let config_path = tmp.path().join(".claude/pulseline/config.toml");
    assert!(config_path.exists(), "config file should be created");
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        contents.contains("theme = \"echo-sub-zero\""),
        "config should contain selected theme, got: {contents}"
    );
}

#[test]
fn help_mentions_new_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--help")
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("--select-theme"),
        "help should mention --select-theme"
    );
    assert!(
        stdout.contains("--palette-map"),
        "help should mention --palette-map"
    );
}

#[test]
fn palette_map_shows_field_info() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--palette-map")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("Anatomy"), "should contain Anatomy header");
    assert!(
        stdout.contains("Field Reference"),
        "should contain Field Reference section"
    );
    assert!(
        stdout.contains("emphasis_primary"),
        "should list emphasis_primary"
    );
    assert!(stdout.contains("alert_red"), "should list alert_red");
    assert!(
        stdout.contains("completed_check"),
        "should list completed_check"
    );
    assert!(
        stdout.contains("L1: Identity"),
        "should have L1 anatomy section"
    );
    assert!(
        stdout.contains("stable_blue"),
        "should annotate stable_blue"
    );
}

#[test]
fn palette_map_with_theme_arg() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .args(["--palette-map", "echo-sub-zero"])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("echo-sub-zero"),
        "should show requested theme name"
    );
    assert!(stdout.contains("emphasis_primary"), "should list fields");
}

#[test]
fn palette_map_no_color_has_no_ansi() {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .arg("--palette-map")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "should exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("\x1b["),
        "NO_COLOR output should not contain ANSI escapes"
    );
}
