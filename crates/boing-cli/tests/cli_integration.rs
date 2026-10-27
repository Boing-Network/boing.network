//! Integration tests for the boing CLI.
//! Run with: cargo test -p boing-cli

use std::fs;
use std::process::Command;

fn boing_bin() -> String {
    std::env::var("CARGO_BIN_EXE_boing").unwrap_or_else(|_| "boing".to_string())
}

#[test]
fn test_boing_version() {
    let out = Command::new(boing_bin())
        .arg("--version")
        .output()
        .expect("failed to run boing --version");
    assert!(out.status.success(), "boing --version should succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("boing"),
        "version output should contain 'boing'"
    );
}

#[test]
fn test_boing_help() {
    let out = Command::new(boing_bin())
        .arg("--help")
        .output()
        .expect("failed to run boing --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("init"));
    assert!(stdout.contains("dev"));
    assert!(stdout.contains("deploy"));
    assert!(stdout.contains("completions"));
}

#[test]
fn test_boing_init_creates_project() {
    let dir = std::env::temp_dir().join("boing-cli-test-init");
    let _ = fs::remove_dir_all(&dir);
    let out = Command::new(boing_bin())
        .args(["init", "test-dapp", "--output", dir.to_str().unwrap()])
        .output()
        .expect("failed to run boing init");
    assert!(
        out.status.success(),
        "boing init should succeed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("Cargo.toml").exists());
    assert!(dir.join("README.md").exists());
    assert!(dir.join("boing.json").exists());
    assert!(dir.join("src").join("lib.rs").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_boing_init_rejects_invalid_name() {
    let dir = std::env::temp_dir().join("boing-cli-test-invalid");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let out = Command::new(boing_bin())
        .args([
            "init",
            "bad name",
            "--output",
            dir.join("x").to_str().unwrap(),
        ])
        .output()
        .expect("failed to run boing init");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Project name")
            || stderr.contains("only contain")
            || stderr.contains("invalid"),
        "expected validation error: {}",
        stderr
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_boing_completions_bash() {
    let out = Command::new(boing_bin())
        .args(["completions", "bash"])
        .output()
        .expect("failed to run boing completions bash");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("boing") || stdout.contains("complete"));
}
