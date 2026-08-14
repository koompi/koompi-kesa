#![forbid(unsafe_code)]

use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn test_error(message: impl Into<String>) -> Box<dyn Error> {
    io::Error::other(message.into()).into()
}

fn require(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(test_error(message))
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn output_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn run_output(mut command: Command, label: &str) -> TestResult<Output> {
    let output = command.output()?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(test_error(format!(
            "{label} failed\nstdout:\n{}\nstderr:\n{}",
            output_text(&output.stdout),
            output_text(&output.stderr)
        )))
    }
}

#[test]
fn swarm_runpack_freshness_script_self_test_passes() -> TestResult {
    let output = run_output(
        {
            let mut command = Command::new("python3");
            command
                .current_dir(repo_root())
                .args(["scripts/check_swarm_runpack_freshness.py", "--self-test"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
        },
        "check_swarm_runpack_freshness_self_test",
    )?;
    require(
        output_text(&output.stdout).contains("SELF-TEST PASS"),
        "freshness script self-test should report PASS",
    )?;
    Ok(())
}

#[test]
fn swarm_runpack_freshness_runpack_smoke_passes() -> TestResult {
    let output = run_output(
        {
            let mut command = Command::new("python3");
            command
                .current_dir(repo_root())
                .args([
                    "scripts/check_swarm_runpack_freshness.py",
                    "--run-runpack-smoke",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
        },
        "check_swarm_runpack_freshness_runpack_smoke",
    )?;
    require(
        output_text(&output.stdout).contains("RUNPACK-SMOKE PASS"),
        "freshness runpack smoke should rebuild and verify the runpack",
    )?;
    Ok(())
}

#[test]
fn extension_conformance_triage_script_self_test_passes() -> TestResult {
    let output = run_output(
        {
            let mut command = Command::new("python3");
            command
                .current_dir(repo_root())
                .args([
                    "scripts/summarize_ext_conformance_failures.py",
                    "--self-test",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
        },
        "summarize_ext_conformance_failures_self_test",
    )?;
    require(
        output_text(&output.stdout).contains("SELF-TEST PASS"),
        "extension conformance triage script self-test should report PASS",
    )?;
    Ok(())
}
