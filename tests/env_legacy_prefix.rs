//! The `KODE_` fallback lives at the process-environment boundary, and Rust
//! 2024 makes `set_var` unsafe in a crate that forbids unsafe, so the only
//! honest way to assert it is to spawn the binary with the variable set.

use std::path::PathBuf;
use std::process::Command;

fn doctor(env: &[(&str, &str)]) -> (String, String) {
    let home = tempfile::tempdir().expect("temp home");
    let mut cmd = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_kesa")));
    cmd.arg("doctor")
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", home.path());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let out = cmd.output().expect("run kesa doctor");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const DEPRECATION: &str = "KODE_CODING_AGENT_DIR is deprecated";

#[test]
fn legacy_name_is_read_and_warns_exactly_once() {
    let (stdout, stderr) = doctor(&[("KODE_CODING_AGENT_DIR", "/tmp/kesa-legacy-only")]);
    assert!(
        stdout.contains("/tmp/kesa-legacy-only"),
        "legacy name was ignored: {stdout}"
    );
    assert_eq!(
        stderr.matches(DEPRECATION).count(),
        1,
        "expected one deprecation line per process: {stderr}"
    );
}

#[test]
fn current_name_is_read_silently() {
    let (stdout, stderr) = doctor(&[("KESA_CODING_AGENT_DIR", "/tmp/kesa-current-only")]);
    assert!(
        stdout.contains("/tmp/kesa-current-only"),
        "current name was ignored: {stdout}"
    );
    assert!(
        !stderr.contains("deprecated"),
        "current name should not warn: {stderr}"
    );
}

#[test]
fn current_name_wins_over_legacy() {
    let (stdout, stderr) = doctor(&[
        ("KODE_CODING_AGENT_DIR", "/tmp/kesa-loser"),
        ("KESA_CODING_AGENT_DIR", "/tmp/kesa-winner"),
    ]);
    assert!(
        stdout.contains("/tmp/kesa-winner") && !stdout.contains("/tmp/kesa-loser"),
        "legacy name won: {stdout}"
    );
    assert!(
        !stderr.contains("deprecated"),
        "unread legacy name should not warn: {stderr}"
    );
}

#[test]
fn harness_variables_have_no_legacy_fallback() {
    let (stdout, stderr) = doctor(&[("KODE_DOCTOR_LOGICAL_CPU_CORES", "97")]);
    assert!(
        !stdout.contains("97 logical") && !stderr.contains("deprecated"),
        "an internal variable was given a deprecation shim: {stdout}{stderr}"
    );
}
