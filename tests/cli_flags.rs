use std::process::Command;

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
    assert!(stdout.contains("1.0.2"), "should contain version number");
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
        stdout.contains("cc-pulseline 1.0.2"),
        "should show name and version"
    );
}
