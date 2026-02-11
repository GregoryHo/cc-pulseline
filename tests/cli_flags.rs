use std::process::Command;

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
    assert!(stdout.contains("1.0.1"), "should contain version number");
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
        stdout.contains("cc-pulseline 1.0.1"),
        "should show name and version"
    );
}
