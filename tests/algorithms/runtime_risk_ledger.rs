use kesa::extensions::{
    RUNTIME_RISK_CALIBRATION_SCHEMA_VERSION, RUNTIME_RISK_LEDGER_SCHEMA_VERSION,
    RUNTIME_RISK_REPLAY_SCHEMA_VERSION, RuntimeRiskActionValue, RuntimeRiskCalibrationObjective,
    RuntimeRiskCalibrationReport, RuntimeRiskExpectedLossEvidence,
    RuntimeRiskExplanationBudgetState, RuntimeRiskExplanationContributor,
    RuntimeRiskExplanationLevelValue, RuntimeRiskLedgerArtifact, RuntimeRiskLedgerArtifactEntry,
    RuntimeRiskLedgerVerificationReport, RuntimeRiskPosteriorEvidence, RuntimeRiskReplayArtifact,
    RuntimeRiskStateLabelValue, runtime_risk_compute_ledger_hash_artifact,
    runtime_risk_ledger_data_hash,
};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

struct LedgerCase {
    call_id: &'static str,
    capability: &'static str,
    method: &'static str,
    risk_score: f64,
    selected_action: RuntimeRiskActionValue,
    derived_state: RuntimeRiskStateLabelValue,
    outcome_error_code: Option<&'static str>,
}

#[test]
#[allow(clippy::too_many_lines)]
fn runtime_risk_ledger_cli_verifies_replays_and_calibrates_round_trip() {
    let temp = tempdir().expect("create tempdir");
    let artifact = deterministic_ledger_artifact();
    let ledger_path = temp.path().join("runtime-risk-ledger.json");
    write_json(&ledger_path, &artifact);

    let verify_path = temp.path().join("verify.json");
    let verify_output = run_ledger_command()
        .args(["verify", "--input"])
        .arg(&ledger_path)
        .args(["--output"])
        .arg(&verify_path)
        .output()
        .expect("run verify command");
    assert_success(&verify_output, "verify valid ledger");

    let verify_report: RuntimeRiskLedgerVerificationReport = read_json(&verify_path);
    assert!(verify_report.valid);
    assert_eq!(verify_report.entry_count, artifact.entries.len());
    assert_eq!(verify_report.artifact_data_hash, artifact.data_hash);
    assert_eq!(verify_report.computed_data_hash, artifact.data_hash);

    let tampered_path = temp.path().join("runtime-risk-ledger-tampered.json");
    let mut tampered = artifact.clone();
    tampered.entries[1].risk_score = 0.97;
    write_json(&tampered_path, &tampered);

    let tampered_verify_path = temp.path().join("verify-tampered.json");
    let tampered_output = run_ledger_command()
        .args(["verify", "--input"])
        .arg(&tampered_path)
        .args(["--output"])
        .arg(&tampered_verify_path)
        .output()
        .expect("run tampered verify command");
    assert_failure(&tampered_output, "verify tampered ledger");

    let tampered_report: RuntimeRiskLedgerVerificationReport = read_json(&tampered_verify_path);
    assert!(!tampered_report.valid);
    assert!(
        tampered_report
            .errors
            .iter()
            .any(|error| error.code == "hash_mismatch"),
        "tampered ledger must report a hash mismatch: {:?}",
        tampered_report.errors
    );

    let replay_path = temp.path().join("replay.json");
    let replay_output = run_ledger_command()
        .args(["replay", "--input"])
        .arg(&ledger_path)
        .args(["--output"])
        .arg(&replay_path)
        .output()
        .expect("run replay command");
    assert_success(&replay_output, "replay valid ledger");

    let replay: RuntimeRiskReplayArtifact = read_json(&replay_path);
    assert_eq!(replay.schema, RUNTIME_RISK_REPLAY_SCHEMA_VERSION);
    assert_eq!(replay.source_schema, RUNTIME_RISK_LEDGER_SCHEMA_VERSION);
    assert_eq!(replay.source_data_hash, artifact.data_hash);
    assert_eq!(replay.entry_count, artifact.entries.len());
    for (step, entry) in replay.steps.iter().zip(&artifact.entries) {
        assert_eq!(step.call_id, entry.call_id);
        assert_eq!(step.selected_action, entry.selected_action);
        assert_eq!(step.derived_state, entry.derived_state);
        assert_eq!(step.ledger_hash, entry.ledger_hash);
    }

    let calibration_path = temp.path().join("calibration.json");
    let calibration_output = run_ledger_command()
        .args([
            "calibrate",
            "--objective",
            "balanced_accuracy",
            "--baseline-threshold",
            "0.95",
            "--min-threshold",
            "0.05",
            "--max-threshold",
            "0.95",
            "--step",
            "0.05",
            "--input",
        ])
        .arg(&ledger_path)
        .args(["--output"])
        .arg(&calibration_path)
        .output()
        .expect("run calibrate command");
    assert_success(&calibration_output, "calibrate valid ledger");

    let calibration: RuntimeRiskCalibrationReport = read_json(&calibration_path);
    assert_eq!(calibration.schema, RUNTIME_RISK_CALIBRATION_SCHEMA_VERSION);
    assert_eq!(
        calibration.source_schema,
        RUNTIME_RISK_LEDGER_SCHEMA_VERSION
    );
    assert_eq!(calibration.source_data_hash, artifact.data_hash);
    assert_eq!(
        calibration.objective,
        RuntimeRiskCalibrationObjective::BalancedAccuracy
    );
    assert!(
        calibration.recommended_threshold < calibration.baseline_threshold,
        "expected calibration to lower the over-strict baseline threshold: {calibration:?}"
    );
    assert!(
        calibration.recommended.objective_score <= calibration.baseline.objective_score,
        "recommended threshold must not regress balanced-accuracy objective: {calibration:?}"
    );
    assert!(
        calibration.recommended.false_positive_rate + calibration.recommended.false_negative_rate
            <= calibration.baseline.false_positive_rate + calibration.baseline.false_negative_rate,
        "recommended threshold should reduce or preserve FPR+FNR: {calibration:?}"
    );
}

fn deterministic_ledger_artifact() -> RuntimeRiskLedgerArtifact {
    let entries = [
        LedgerCase {
            call_id: "safe-log-1",
            capability: "log",
            method: "log",
            risk_score: 0.10,
            selected_action: RuntimeRiskActionValue::Allow,
            derived_state: RuntimeRiskStateLabelValue::SafeFast,
            outcome_error_code: None,
        },
        LedgerCase {
            call_id: "safe-log-2",
            capability: "log",
            method: "log",
            risk_score: 0.20,
            selected_action: RuntimeRiskActionValue::Allow,
            derived_state: RuntimeRiskStateLabelValue::SafeFast,
            outcome_error_code: None,
        },
        LedgerCase {
            call_id: "suspicious-http",
            capability: "http",
            method: "fetch",
            risk_score: 0.65,
            selected_action: RuntimeRiskActionValue::Harden,
            derived_state: RuntimeRiskStateLabelValue::Suspicious,
            outcome_error_code: None,
        },
        LedgerCase {
            call_id: "unsafe-exec-deny",
            capability: "exec",
            method: "exec",
            risk_score: 0.80,
            selected_action: RuntimeRiskActionValue::Deny,
            derived_state: RuntimeRiskStateLabelValue::Unsafe,
            outcome_error_code: Some("denied"),
        },
        LedgerCase {
            call_id: "unsafe-exec-terminate",
            capability: "exec",
            method: "exec",
            risk_score: 0.90,
            selected_action: RuntimeRiskActionValue::Terminate,
            derived_state: RuntimeRiskStateLabelValue::Unsafe,
            outcome_error_code: Some("terminated"),
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, case)| ledger_entry(index, &case))
    .collect::<Vec<_>>();

    seal_ledger(entries)
}

fn ledger_entry(index: usize, case: &LedgerCase) -> RuntimeRiskLedgerArtifactEntry {
    RuntimeRiskLedgerArtifactEntry {
        ts_ms: 1_775_000_000_000 + i64::try_from(index).expect("index fits i64"),
        extension_id: "ext.runtime-risk-roundtrip".to_string(),
        call_id: case.call_id.to_string(),
        capability: case.capability.to_string(),
        method: case.method.to_string(),
        params_hash: format!("params-hash-{index}"),
        policy_reason: "synthetic policy decision".to_string(),
        risk_score: case.risk_score,
        posterior: RuntimeRiskPosteriorEvidence {
            safe_fast: 1.0 - case.risk_score,
            suspicious: case.risk_score / 2.0,
            unsafe_: case.risk_score / 2.0,
        },
        expected_loss: RuntimeRiskExpectedLossEvidence {
            allow: case.risk_score * 8.0,
            harden: case.risk_score * 4.0,
            deny: 1.0 - case.risk_score,
            terminate: (1.0 - case.risk_score) / 2.0,
        },
        selected_action: case.selected_action,
        derived_state: case.derived_state,
        triggers: vec![format!("trigger-{index}")],
        fallback_reason: None,
        e_process: case.risk_score,
        e_threshold: 1.0,
        conformal_residual: case.risk_score / 10.0,
        conformal_quantile: 0.95,
        drift_detected: false,
        outcome_error_code: case.outcome_error_code.map(ToString::to_string),
        explanation_schema: "pi.ext.runtime_risk_explanation.v1".to_string(),
        explanation_level: RuntimeRiskExplanationLevelValue::Standard,
        explanation_summary: format!("synthetic explanation for {}", case.call_id),
        top_contributors: vec![RuntimeRiskExplanationContributor {
            code: "synthetic_risk_score".to_string(),
            signed_impact: case.risk_score,
            magnitude: case.risk_score,
            rationale: "deterministic fixture contributor".to_string(),
        }],
        budget_state: RuntimeRiskExplanationBudgetState::default(),
        ledger_hash: String::new(),
        prev_ledger_hash: None,
    }
}

fn seal_ledger(mut entries: Vec<RuntimeRiskLedgerArtifactEntry>) -> RuntimeRiskLedgerArtifact {
    let mut previous_hash = None;
    for entry in &mut entries {
        entry.prev_ledger_hash.clone_from(&previous_hash);
        let hash = runtime_risk_compute_ledger_hash_artifact(entry, previous_hash.as_deref());
        entry.ledger_hash.clone_from(&hash);
        previous_hash = Some(hash);
    }

    RuntimeRiskLedgerArtifact {
        schema: RUNTIME_RISK_LEDGER_SCHEMA_VERSION.to_string(),
        generated_at_ms: 1_775_000_000_999,
        entry_count: entries.len(),
        head_ledger_hash: entries.first().map(|entry| entry.ledger_hash.clone()),
        tail_ledger_hash: entries.last().map(|entry| entry.ledger_hash.clone()),
        data_hash: runtime_risk_ledger_data_hash(&entries),
        entries,
    }
}

fn run_ledger_command() -> Command {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "run",
            "--quiet",
            "--example",
            "ext_runtime_risk_ledger",
            "--",
        ]);
    command
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let payload = serde_json::to_string_pretty(value).expect("serialize json");
    fs::write(path, payload).expect("write json");
}

fn read_json<T: DeserializeOwned>(path: &Path) -> T {
    let payload = fs::read_to_string(path).expect("read json");
    serde_json::from_str(&payload).expect("parse json")
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output, context: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
