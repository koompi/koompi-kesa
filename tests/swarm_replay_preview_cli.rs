#![forbid(unsafe_code)]

use jsonschema::Validator;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const TRACE_PATH: &str = "tests/golden_corpus/swarm_replay_trace/normalized_trace.json";
const SCHEMA_PATH: &str = "docs/schema/swarm_replay_preview.json";
const GOLDEN_TEXT_PATH: &str = "tests/golden_corpus/swarm_replay_trace/preview_text.txt";
const GENERATED_AT: &str = "2026-05-14T00:00:00Z";

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_kode"))
}

fn read_json_value(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(path).map_err(|err| {
        std::io::Error::new(
            err.kind(),
            format!("failed to read JSON {}: {err}", path.display()),
        )
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to parse JSON {}: {err}", path.display()),
        )
        .into()
    })
}

fn compiled_preview_schema() -> Result<Validator, Box<dyn std::error::Error>> {
    let path = repo_root().join(SCHEMA_PATH);
    let schema = read_json_value(&path)?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to compile schema {}: {err}", path.display()),
            )
            .into()
        })
}

fn validate_preview(preview: &Value) -> TestResult {
    if let Err(err) = compiled_preview_schema()?.validate(preview) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("preview JSON does not validate: {err}"),
        )
        .into());
    }
    Ok(())
}

fn preview_command() -> Command {
    let mut command = Command::new(binary_path());
    command.current_dir(repo_root());
    command.args([
        "swarm-replay-preview",
        "--trace",
        TRACE_PATH,
        "--policy",
        "existing_autopilot",
        "--policy",
        "rch_fanout_limited",
        "--generated-at",
        GENERATED_AT,
    ]);
    command
}

fn output_text(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
}

#[test]
fn swarm_replay_preview_json_validates_and_writes_outputs() -> TestResult {
    let temp = TempDir::new()?;
    let json_path = temp.path().join("preview.json");
    let text_path = temp.path().join("preview.txt");
    let output = preview_command()
        .arg("--out-json")
        .arg(&json_path)
        .arg("--out-text")
        .arg(&text_path)
        .output()?;

    assert!(
        output.status.success(),
        "preview command failed\nstdout:\n{}\nstderr:\n{}",
        output_text(&output.stdout),
        output_text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "output-path mode should not print stdout: {}",
        output_text(&output.stdout)
    );
    let preview = read_json_value(&json_path)?;
    validate_preview(&preview)?;
    assert_eq!(
        preview.pointer("/schema").and_then(Value::as_str),
        Some("pi.swarm.replay_preview.v1")
    );
    assert_eq!(
        preview
            .pointer("/command/output_writes")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        preview.pointer("/trace/trace_id").and_then(Value::as_str),
        Some("golden-swarm-replay-normalized")
    );
    assert!(
        text_path.exists(),
        "text output was not written to {}",
        text_path.display()
    );
    Ok(())
}

#[test]
fn swarm_replay_preview_text_matches_golden() -> TestResult {
    let output = preview_command().args(["--format", "text"]).output()?;
    assert!(
        output.status.success(),
        "preview text command failed\nstdout:\n{}\nstderr:\n{}",
        output_text(&output.stdout),
        output_text(&output.stderr)
    );
    let expected = fs::read_to_string(repo_root().join(GOLDEN_TEXT_PATH))?;
    assert_eq!(output_text(&output.stdout), expected);
    Ok(())
}

#[test]
fn swarm_replay_preview_json_stdout_validates() -> TestResult {
    let output = preview_command().args(["--format", "json"]).output()?;
    assert!(
        output.status.success(),
        "preview JSON stdout failed\nstdout:\n{}\nstderr:\n{}",
        output_text(&output.stdout),
        output_text(&output.stderr)
    );
    let preview: Value = serde_json::from_slice(&output.stdout)?;
    validate_preview(&preview)?;
    Ok(())
}

#[test]
fn swarm_replay_preview_refuses_to_overwrite_json() -> TestResult {
    let temp = TempDir::new()?;
    let json_path = temp.path().join("preview.json");
    fs::write(&json_path, "{}")?;
    let output = preview_command()
        .arg("--out-json")
        .arg(&json_path)
        .output()?;

    assert!(
        !output.status.success(),
        "overwrite command unexpectedly succeeded"
    );
    assert!(
        output_text(&output.stderr).contains("refusing to overwrite existing JSON preview"),
        "stderr did not explain overwrite refusal:\n{}",
        output_text(&output.stderr)
    );
    Ok(())
}

#[test]
fn swarm_replay_preview_rejects_unknown_policy() -> TestResult {
    let output = Command::new(binary_path())
        .current_dir(repo_root())
        .args([
            "swarm-replay-preview",
            "--trace",
            TRACE_PATH,
            "--policy",
            "optimistic-local-builds",
        ])
        .output()?;

    assert!(
        !output.status.success(),
        "unknown policy command unexpectedly succeeded"
    );
    assert!(
        output_text(&output.stderr).contains("unsupported swarm-replay-preview policy"),
        "stderr did not explain policy rejection:\n{}",
        output_text(&output.stderr)
    );
    Ok(())
}
