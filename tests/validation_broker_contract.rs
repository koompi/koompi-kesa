#![allow(clippy::too_many_lines)]
#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

const CONTRACT_PATH: &str = "docs/contracts/validation-broker-contract.json";
const CLOSEOUT_CONTRACT_PATH: &str = "docs/contracts/validation-broker-closeout-gate-contract.json";
const CLOSEOUT_EVIDENCE_PATH: &str = "docs/evidence/validation-broker-closeout-gate.json";
const README_PATH: &str = "README.md";
const SWARM_RUNBOOK_PATH: &str = "docs/swarm-operations-runbook.md";
const EXPECTED_SCHEMA: &str = "pi.validation_broker.contract.v1";
const EXPECTED_CLOSEOUT_CONTRACT_SCHEMA: &str = "pi.validation_broker.closeout_gate_contract.v1";
const EXPECTED_CLOSEOUT_SCHEMA: &str = "pi.validation_broker.closeout_gate.v1";
const EXPECTED_REQUEST_SCHEMA: &str = "pi.validation_broker.request.v1";
const EXPECTED_SLOT_SCHEMA: &str = "pi.validation_broker.slot.v1";
const EXPECTED_DECISION_SCHEMA: &str = "pi.validation_broker.decision.v1";
const EXPECTED_FAULT_CORPUS_SCHEMA: &str = "pi.validation_broker.fault_corpus.v1";
const EXPECTED_FAULT_EVENT_SCHEMA: &str = "pi.validation_broker.fault_event.v1";
const EXPECTED_STRESS_PROFILE_CORPUS_SCHEMA: &str = "pi.validation_broker.stress_profile_corpus.v1";
const EXPECTED_STRESS_BUDGET_REPORT_SCHEMA: &str = "pi.validation_broker.stress_budget_report.v1";
const EXPECTED_STRESS_EVIDENCE_SCHEMA: &str = "pi.validation_broker.stress_evidence.v1";
const EXPECTED_BEAD_ID: &str = "bd-gusp4.1";
const EXPECTED_PARENT_BEAD_ID: &str = "bd-gusp4";

const REQUIRED_SOURCE_IDS: &[&str] = &[
    "beads_jsonl",
    "beads_db",
    "agent_mail_reservations",
    "rch_status",
    "rch_queue",
    "cargo_headroom_preflight",
    "doctor_swarm_preflight",
    "git_status",
    "validation_artifact_manifest",
    "cargo_command_request",
    "agent_identity",
];

type TestResult = Result<(), String>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_contract() -> Result<Value, String> {
    load_json(CONTRACT_PATH)
}

fn load_json(relative_path: &str) -> Result<Value, String> {
    let path = repo_root().join(relative_path);
    let raw = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse {} as JSON: {err}", path.display()))
}

fn load_text(relative_path: &str) -> Result<String, String> {
    let path = repo_root().join(relative_path);
    std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))
}

fn require(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn pointer<'a>(value: &'a Value, path: &str) -> Result<&'a Value, String> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing JSON pointer {path}"))
}

fn pointer_str<'a>(value: &'a Value, path: &str) -> Result<&'a str, String> {
    pointer(value, path)?
        .as_str()
        .ok_or_else(|| format!("{path} must be a string"))
}

fn pointer_bool(value: &Value, path: &str) -> Result<bool, String> {
    pointer(value, path)?
        .as_bool()
        .ok_or_else(|| format!("{path} must be a bool"))
}

fn pointer_array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], String> {
    pointer(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{path} must be an array"))
}

fn string_set<'a>(value: &'a Value, path: &str) -> Result<HashSet<&'a str>, String> {
    let mut entries = HashSet::new();
    let non_string_message = format!("{path} entries must be strings");
    let blank_message = format!("{path} has blank entry");
    for entry in pointer_array(value, path)? {
        let Some(raw) = entry.as_str() else {
            return Err(non_string_message);
        };
        if raw.trim().is_empty() {
            return Err(blank_message);
        }
        entries.insert(raw);
    }
    Ok(entries)
}

fn require_set(value: &Value, path: &str, expected: &[&str], label: &str) -> TestResult {
    let observed = string_set(value, path)?;
    if let Some(missing) = expected.iter().find(|item| !observed.contains(**item)) {
        return Err(format!("missing {label}: {missing}"));
    }
    Ok(())
}

fn require_object_keys(value: &Value, keys: &[&str], label: &str) -> TestResult {
    for key in keys {
        require(
            value.get(*key).is_some(),
            format!("{label} missing key {key}"),
        )?;
    }
    Ok(())
}

fn require_array_contains_fragment(value: &Value, path: &str, fragment: &str) -> TestResult {
    let entries = pointer_array(value, path)?;
    require(
        entries
            .iter()
            .any(|entry| entry.as_str().is_some_and(|text| text.contains(fragment))),
        format!("{path} must contain fragment {fragment:?}"),
    )
}

#[test]
fn validation_broker_contract_has_identity_and_advisory_purpose() -> TestResult {
    let contract = load_contract()?;

    require(
        pointer_str(&contract, "/schema")? == EXPECTED_SCHEMA,
        "schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/request_schema")? == EXPECTED_REQUEST_SCHEMA,
        "request schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/slot_schema")? == EXPECTED_SLOT_SCHEMA,
        "slot schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/decision_schema")? == EXPECTED_DECISION_SCHEMA,
        "decision schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/fault_corpus_schema")? == EXPECTED_FAULT_CORPUS_SCHEMA,
        "fault corpus schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/fault_event_schema")? == EXPECTED_FAULT_EVENT_SCHEMA,
        "fault event schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/bead_id")? == EXPECTED_BEAD_ID,
        "bead linkage mismatch",
    )?;
    require(
        pointer_str(&contract, "/parent_bead_id")? == EXPECTED_PARENT_BEAD_ID,
        "parent bead linkage mismatch",
    )?;
    require(
        pointer_str(&contract, "/purpose")?
            == "live_validation_admission_advisory_not_ci_or_rch_replacement",
        "purpose must keep broker advisory",
    )?;
    require_array_contains_fragment(&contract, "/non_goals", "replace_rch")?;
    require_array_contains_fragment(&contract, "/non_goals", "suppress_required")?;

    Ok(())
}

#[test]
fn validation_broker_contract_declares_source_inventory_and_boundaries() -> TestResult {
    let contract = load_contract()?;

    require_set(
        &contract,
        "/required_source_ids",
        REQUIRED_SOURCE_IDS,
        "source id",
    )?;
    require_set(
        &contract,
        "/source_status_contract/required_fields",
        &[
            "source_id",
            "source_kind",
            "availability",
            "freshness_state",
            "source_hash",
            "authoritative_for",
            "redaction_state",
            "degraded_reasons",
            "suppressed_claims",
        ],
        "source status field",
    )?;

    let boundaries = pointer_array(&contract, "/authoritative_source_boundaries")?;
    require(
        boundaries.len() >= 7,
        "source boundary list must cover all major input surfaces",
    )?;
    let boundary_ids: HashSet<&str> = boundaries
        .iter()
        .filter_map(|entry| entry.get("source_id").and_then(Value::as_str))
        .collect();
    let required_boundaries = [
        "beads_jsonl",
        "agent_mail_reservations",
        "rch_status",
        "cargo_headroom_preflight",
        "doctor_swarm_preflight",
        "git_status",
        "validation_artifact_manifest",
    ];
    if let Some(missing) = required_boundaries
        .iter()
        .find(|required| !boundary_ids.contains(**required))
    {
        return Err(format!("missing authoritative boundary for {missing}"));
    }

    Ok(())
}

#[test]
fn validation_broker_contract_fails_closed_for_missing_sources() -> TestResult {
    let contract = load_contract()?;

    require_set(
        &contract,
        "/source_status_contract/allowed_availability",
        &[
            "unavailable",
            "partial",
            "malformed",
            "stale",
            "not_configured",
        ],
        "availability state",
    )?;
    require_set(
        &contract,
        "/source_status_contract/allowed_freshness_states",
        &["missing", "malformed", "freshness_unknown"],
        "freshness state",
    )?;

    let policy = pointer_str(&contract, "/source_status_contract/missing_source_policy")?;
    require(
        policy.contains("must not infer or invent facts"),
        "missing source policy must forbid invented facts",
    )?;
    require(
        policy.contains("must not become allow or coalesce"),
        "missing source policy must block allow/coalesce",
    )?;
    require_set(
        &contract,
        "/decision_contract/allowed_decisions",
        &["degraded_block", "wait", "narrow", "stale_recover"],
        "safe degraded decision",
    )
}

#[test]
fn validation_broker_contract_covers_request_slot_and_decision_shapes() -> TestResult {
    let contract = load_contract()?;

    require_set(
        &contract,
        "/request_contract/required_top_level_keys",
        &[
            "schema",
            "request_id",
            "agent_name",
            "bead_id",
            "cwd",
            "git_head",
            "command",
            "command_class",
            "requested_scope",
            "target_dir",
            "tmpdir",
            "runner_requirement",
            "dirty_worktree_policy",
            "evidence_requirements",
        ],
        "request key",
    )?;
    require_set(
        &contract,
        "/slot_lease_contract/required_top_level_keys",
        &[
            "schema",
            "slot_id",
            "state",
            "owner_agent",
            "bead_id",
            "command_fingerprint",
            "environment_fingerprint",
            "git_head",
            "target_dir",
            "tmpdir",
            "runner",
            "heartbeat_at_utc",
            "expires_at_utc",
            "artifacts",
        ],
        "slot key",
    )?;
    require_set(
        &contract,
        "/decision_contract/required_top_level_keys",
        &[
            "schema",
            "decision_id",
            "request_id",
            "decision",
            "confidence",
            "reasons",
            "source_statuses",
            "required_actions",
            "coalesced_artifacts",
            "rejected_reusable_slots",
            "suppressed_claims",
            "no_claims",
        ],
        "decision key",
    )?;
    require_set(
        &contract,
        "/decision_contract/coalescing_equivalence_fields",
        &[
            "command_fingerprint",
            "cwd",
            "git_head",
            "feature_flags",
            "target_dir",
            "tmpdir",
            "runner",
            "rust_toolchain",
            "environment_fingerprint",
            "artifact_schema",
            "artifact_hash",
        ],
        "coalescing equivalence field",
    )
}

#[test]
fn validation_broker_contract_declares_fault_corpus() -> TestResult {
    let contract = load_contract()?;

    require(
        pointer_str(&contract, "/fault_corpus_contract/schema")? == EXPECTED_FAULT_CORPUS_SCHEMA,
        "fault corpus contract schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/fault_corpus_contract/event_schema")?
            == EXPECTED_FAULT_EVENT_SCHEMA,
        "fault corpus event schema mismatch",
    )?;
    require_set(
        &contract,
        "/fault_corpus_contract/required_faults",
        &[
            "agent_mail_unavailable",
            "rch_queue_saturated",
            "rch_fail_open_local_fallback",
            "stale_pre_commit_ubs",
            "stuck_cargo_clippy",
            "insufficient_tmpdir",
            "target_dir_collision",
            "reusable_provenance_mismatch",
            "duplicate_broad_gate_request",
            "equivalent_reusable_artifact",
        ],
        "fault corpus fault",
    )?;
    require_set(
        &contract,
        "/fault_corpus_contract/required_decisions",
        &[
            "allow",
            "wait",
            "coalesce",
            "narrow",
            "deny_local_fallback",
            "stale_recover",
            "degraded_block",
        ],
        "fault corpus decision",
    )?;
    require_set(
        &contract,
        "/fault_corpus_contract/required_rejected_reusable_slot_reasons",
        &[
            "command_fingerprint_mismatch",
            "environment_fingerprint_mismatch",
            "target_dir_mismatch",
            "artifact_hash_mismatch",
        ],
        "rejected reusable slot reason",
    )?;

    let corpus_path = pointer_str(&contract, "/fault_corpus_contract/corpus_path")?;
    let event_log_path = pointer_str(&contract, "/fault_corpus_contract/event_log_path")?;
    let corpus_abs = repo_root().join(corpus_path);
    let event_log_abs = repo_root().join(event_log_path);
    require(corpus_abs.exists(), "fault corpus artifact exists")?;
    require(event_log_abs.exists(), "fault event log artifact exists")?;

    let corpus_raw = std::fs::read_to_string(&corpus_abs)
        .map_err(|err| format!("failed to read {}: {err}", corpus_abs.display()))?;
    let corpus: Value = serde_json::from_str(&corpus_raw)
        .map_err(|err| format!("failed to parse {}: {err}", corpus_abs.display()))?;
    require(
        pointer_str(&corpus, "/schema")? == EXPECTED_FAULT_CORPUS_SCHEMA,
        "fault corpus artifact schema mismatch",
    )?;
    require(
        pointer_str(&corpus, "/event_log_path")? == event_log_path,
        "fault corpus event log linkage mismatch",
    )?;

    let mut observed_faults = HashSet::new();
    let mut observed_decisions = HashSet::new();
    let scenarios = pointer_array(&corpus, "/scenarios")?;
    require(
        scenarios.len() >= 9,
        "fault corpus must cover all required scenarios",
    )?;
    for scenario in scenarios {
        let faults = scenario
            .get("faults")
            .and_then(Value::as_array)
            .ok_or("scenario faults must be an array")?;
        for fault in faults {
            let fault_name = fault.as_str().ok_or("scenario fault must be a string")?;
            observed_faults.insert(fault_name);
        }
        let decision = scenario
            .pointer("/expected/decision")
            .and_then(Value::as_str)
            .ok_or("scenario expected decision must be a string")?;
        observed_decisions.insert(decision);
    }
    for required_fault in string_set(&contract, "/fault_corpus_contract/required_faults")? {
        if !observed_faults.contains(required_fault) {
            return Err(missing_fault_message(required_fault));
        }
    }
    for required_decision in string_set(&contract, "/fault_corpus_contract/required_decisions")? {
        if !observed_decisions.contains(required_decision) {
            return Err(missing_decision_message(required_decision));
        }
    }

    let event_log_raw = std::fs::read_to_string(&event_log_abs)
        .map_err(|err| format!("failed to read {}: {err}", event_log_abs.display()))?;
    let mut event_count = 0_usize;
    for (line_index, line) in event_log_raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        validate_fault_event_line(line, line_index + 1)?;
        event_count += 1;
    }
    require(
        event_count >= scenarios.len(),
        "fault event log must cover every corpus scenario",
    )
}

#[test]
fn validation_broker_contract_declares_large_host_stress_budgets() -> TestResult {
    let contract = load_contract()?;

    require(
        pointer_str(&contract, "/stress_budget_report_schema")?
            == EXPECTED_STRESS_BUDGET_REPORT_SCHEMA,
        "stress budget report schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/stress_evidence_schema")? == EXPECTED_STRESS_EVIDENCE_SCHEMA,
        "stress evidence schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/stress_profile_corpus_schema")?
            == EXPECTED_STRESS_PROFILE_CORPUS_SCHEMA,
        "stress profile corpus schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/stress_budget_contract/report_schema")?
            == EXPECTED_STRESS_BUDGET_REPORT_SCHEMA,
        "stress budget contract report schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/stress_budget_contract/entry_schema")?
            == EXPECTED_STRESS_EVIDENCE_SCHEMA,
        "stress budget contract entry schema mismatch",
    )?;
    require_set(
        &contract,
        "/stress_budget_contract/required_profiles",
        &[
            "synthetic_64c_256gb_nominal",
            "synthetic_64c_256gb_saturated",
            "synthetic_64c_256gb_missing_store_bytes",
        ],
        "stress profile",
    )?;
    require_set(
        &contract,
        "/stress_budget_contract/required_verdicts",
        &["pass", "fail", "blocked"],
        "stress verdict",
    )?;
    require_set(
        &contract,
        "/stress_budget_contract/required_guards",
        &[
            "synthetic_only",
            "not_release_performance_evidence",
            "missing_data_blocks_measurements",
        ],
        "stress guard",
    )?;
    require_set(
        &contract,
        "/stress_budget_contract/required_cache_fields",
        &[
            "cache_key",
            "cache_status",
            "ttl_seconds",
            "input_fingerprint",
        ],
        "stress cache field",
    )?;
    require_set(
        &contract,
        "/stress_budget_contract/required_scenario_guards",
        &[
            "synthetic_data",
            "no_live_rch_mutation",
            "provider_calls",
            "live_mutations",
            "release_claim_allowed",
        ],
        "stress scenario guard",
    )?;

    let corpus_path = pointer_str(&contract, "/stress_budget_contract/profile_corpus_path")?;
    let corpus_abs = repo_root().join(corpus_path);
    require(corpus_abs.exists(), "stress profile corpus exists")?;
    let corpus_raw = std::fs::read_to_string(&corpus_abs)
        .map_err(|err| format!("failed to read {}: {err}", corpus_abs.display()))?;
    let corpus: Value = serde_json::from_str(&corpus_raw)
        .map_err(|err| format!("failed to parse {}: {err}", corpus_abs.display()))?;
    require(
        pointer_str(&corpus, "/schema")? == EXPECTED_STRESS_PROFILE_CORPUS_SCHEMA,
        "stress profile corpus schema mismatch",
    )?;
    require_stress_budget_fields(
        &corpus,
        &contract,
        "/stress_budget_contract/required_budget_fields",
        "/budgets",
    )?;

    let report_path = pointer_str(&contract, "/stress_budget_contract/report_path")?;
    let report_abs = repo_root().join(report_path);
    require(
        report_abs.exists(),
        "stress budget evidence artifact exists",
    )?;
    let report_raw = std::fs::read_to_string(&report_abs)
        .map_err(|err| format!("failed to read {}: {err}", report_abs.display()))?;
    let report: Value = serde_json::from_str(&report_raw)
        .map_err(|err| format!("failed to parse {}: {err}", report_abs.display()))?;

    require(
        pointer_str(&report, "/schema")? == EXPECTED_STRESS_BUDGET_REPORT_SCHEMA,
        "stress report artifact schema mismatch",
    )?;
    require(
        pointer_str(&report, "/entry_schema")? == EXPECTED_STRESS_EVIDENCE_SCHEMA,
        "stress report entry schema mismatch",
    )?;
    require_set(
        &report,
        "/guards",
        &[
            "synthetic_only",
            "not_release_performance_evidence",
            "missing_data_blocks_measurements",
        ],
        "stress report guard",
    )?;

    let scenarios = pointer_array(&report, "/scenarios")?;
    require(scenarios.len() >= 3, "stress report scenario count")?;
    let mut observed_profiles = HashSet::new();
    let mut observed_verdicts = HashSet::new();
    for scenario in scenarios {
        require(
            scenario.get("schema").and_then(Value::as_str) == Some(EXPECTED_STRESS_EVIDENCE_SCHEMA),
            "stress scenario entry schema",
        )?;
        let profile_id = scenario
            .pointer("/profile/profile_id")
            .and_then(Value::as_str)
            .ok_or("stress scenario missing profile_id")?;
        observed_profiles.insert(profile_id);
        let verdict = scenario
            .get("verdict")
            .and_then(Value::as_str)
            .ok_or("stress scenario missing verdict")?;
        observed_verdicts.insert(verdict);
        require_stress_budget_fields(
            scenario,
            &contract,
            "/stress_budget_contract/required_budget_fields",
            "/budgets",
        )?;
        require_stress_budget_fields(
            scenario,
            &contract,
            "/stress_budget_contract/required_measurement_fields",
            "/measurements",
        )?;
        require(
            scenario
                .pointer("/cache/input_fingerprint")
                .and_then(Value::as_str)
                .is_some(),
            "stress scenario cache fingerprint",
        )?;
        require_stress_budget_fields(
            scenario,
            &contract,
            "/stress_budget_contract/required_cache_fields",
            "/cache",
        )?;
        require_stress_budget_fields(
            scenario,
            &contract,
            "/stress_budget_contract/required_scenario_guards",
            "/guards",
        )?;
        require(
            scenario
                .pointer("/guards/no_live_rch_mutation")
                .and_then(Value::as_bool)
                == Some(true)
                && scenario
                    .pointer("/guards/provider_calls")
                    .and_then(Value::as_u64)
                    == Some(0)
                && scenario
                    .pointer("/guards/live_mutations")
                    .and_then(Value::as_u64)
                    == Some(0)
                && scenario
                    .pointer("/guards/release_claim_allowed")
                    .and_then(Value::as_bool)
                    == Some(false),
            "stress scenario guards block live mutation and release claims",
        )?;
        require(
            scenario
                .pointer("/provenance/artifact_path")
                .and_then(Value::as_str)
                == Some(report_path),
            "stress scenario provenance points at report artifact",
        )?;
        require_set(
            scenario,
            "/no_claims",
            &[
                "not_ci_success",
                "not_release_performance_evidence",
                "not_permission_to_skip_required_gates",
            ],
            "stress no-claim",
        )?;
        if verdict == "blocked" {
            require(
                !pointer_array(scenario, "/missing_data")?.is_empty(),
                "blocked stress scenario names missing data",
            )?;
            require(
                scenario
                    .pointer("/measurements/plan_latency_ms")
                    .is_some_and(Value::is_null),
                "blocked stress scenario does not invent plan latency",
            )?;
        }
    }

    for required_profile in string_set(&contract, "/stress_budget_contract/required_profiles")? {
        if !observed_profiles.contains(required_profile) {
            return Err(format!("stress report missing profile {required_profile}"));
        }
    }
    for required_verdict in string_set(&contract, "/stress_budget_contract/required_verdicts")? {
        if !observed_verdicts.contains(required_verdict) {
            return Err(format!("stress report missing verdict {required_verdict}"));
        }
    }

    Ok(())
}

fn require_stress_budget_fields(
    scenario: &Value,
    contract: &Value,
    contract_path: &str,
    scenario_path: &str,
) -> TestResult {
    for field in string_set(contract, contract_path)? {
        require(
            scenario
                .pointer(&format!("{scenario_path}/{field}"))
                .is_some(),
            format!("stress scenario missing {scenario_path}/{field}"),
        )?;
    }
    Ok(())
}

fn missing_fault_message(required_fault: &str) -> String {
    format!("fault corpus missing scenario fault {required_fault}")
}

fn missing_decision_message(required_decision: &str) -> String {
    format!("fault corpus missing decision {required_decision}")
}

fn validate_fault_event_line(line: &str, line_number: usize) -> TestResult {
    let event: Value = serde_json::from_str(line)
        .map_err(|err| format!("failed to parse event line {line_number}: {err}"))?;
    require(
        pointer_str(&event, "/schema")? == EXPECTED_FAULT_EVENT_SCHEMA,
        format!("fault event line {line_number} schema mismatch"),
    )
}

#[test]
fn validation_broker_contract_is_read_only_in_plan_mode() -> TestResult {
    let contract = load_contract()?;

    require(
        !pointer_bool(
            &contract,
            "/mutation_policy/plan_mode_live_mutation_allowed",
        )?,
        "plan mode must be read-only",
    )?;
    require_set(
        &contract,
        "/mutation_policy/forbidden_mutations",
        &[
            "git_reset",
            "git_clean",
            "file_delete",
            "kill_other_agent_process",
            "stash_or_checkout_other_agent_work",
            "rewrite_mail_archive",
            "rewrite_beads_without_br",
        ],
        "forbidden mutation",
    )?;
    require_set(
        &contract,
        "/slot_lease_contract/stale_policy/safe_next_actions",
        &[
            "wait_for_owner",
            "request_owner_update",
            "open_new_non_overlapping_slot",
            "surface_blocker",
            "rerun_after_provenance_mismatch",
        ],
        "safe stale action",
    )
}

#[test]
fn validation_broker_contract_declares_cli_surface() -> TestResult {
    let contract = load_contract()?;

    require(
        pointer_str(&contract, "/cli_status_schema")? == "pi.validation_broker.cli_status.v1",
        "CLI status schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/cli_plan_schema")? == "pi.validation_broker.cli_plan.v1",
        "CLI plan schema mismatch",
    )?;
    require(
        pointer_bool(&contract, "/cli_surface_contract/plan_mode/read_only")?,
        "CLI plan mode must be read-only",
    )?;
    require_set(
        &contract,
        "/cli_surface_contract/actions",
        &["status", "plan", "acquire", "renew", "release"],
        "CLI action",
    )?;
    require_set(
        &contract,
        "/cli_surface_contract/plan_mode/required_next_actions",
        &[
            "run_now",
            "wait",
            "coalesce_with_reusable_slot",
            "narrow_scope",
            "surface_blocker",
            "recover_stale_slot_or_bead",
        ],
        "CLI next action",
    )
}

#[test]
fn validation_broker_contract_declares_doctor_runpack_projection() -> TestResult {
    let contract = load_contract()?;

    require(
        pointer_str(
            &contract,
            "/doctor_runpack_projection_contract/doctor_schema",
        )? == "pi.doctor.validation_broker_posture.v1",
        "Doctor projection schema mismatch",
    )?;
    require(
        pointer_str(
            &contract,
            "/doctor_runpack_projection_contract/runpack_optional_source_id",
        )? == "validation_broker",
        "runpack projection source id mismatch",
    )?;
    require(
        pointer_str(
            &contract,
            "/doctor_runpack_projection_contract/autopilot_optional_source_id",
        )? == "validation_broker",
        "autopilot projection source id mismatch",
    )?;
    require_set(
        &contract,
        "/doctor_runpack_projection_contract/required_projection_fields",
        &[
            "source_status",
            "current_slots",
            "degraded_reasons",
            "duplicate_gate_opportunities",
            "stale_build_warnings",
            "recommended_next_actions",
            "guards",
        ],
        "projection field",
    )?;
    require_set(
        &contract,
        "/doctor_runpack_projection_contract/required_guards",
        &[
            "advisory_only",
            "no_live_mutation",
            "not_ci_success",
            "not_release_claim_evidence",
            "does_not_replace_rch_doctor_beads_agent_mail",
        ],
        "projection guard",
    )?;
    let boundary = pointer_str(
        &contract,
        "/doctor_runpack_projection_contract/authority_boundary",
    )?;
    require(
        boundary.contains("must not replace RCH"),
        "projection boundary must keep RCH authoritative",
    )?;
    require(
        boundary.contains("release-claim gates"),
        "projection boundary must block release-claim promotion",
    )
}

#[test]
fn validation_broker_operator_docs_cover_workflow_privacy_and_cli_flags() -> TestResult {
    let readme = load_text(README_PATH)?;
    let runbook = load_text(SWARM_RUNBOOK_PATH)?;

    require(
        readme.contains("docs/swarm-operations-runbook.md#validation-broker-operator-workflow"),
        "README must link to validation broker operator workflow docs",
    )?;
    require(
        runbook.contains("### Validation Broker Operator Workflow"),
        "runbook must include validation broker workflow heading",
    )?;

    for fragment in [
        "pi validation-broker status",
        "pi validation-broker plan",
        "pi validation-broker acquire",
        "pi validation-broker renew",
        "pi validation-broker release",
        "--store \"$KODE_VALIDATION_BROKER_STORE\"",
        "--request \"$capture_dir/validation-request.json\"",
        "--inputs \"$capture_dir/validation-inputs.json\"",
        "--slot-id \"$slot_id\"",
        "--owner \"$AGENT_NAME\"",
        "--reason \"gate completed and artifacts recorded\"",
    ] {
        require(
            runbook.contains(fragment),
            format!("operator docs missing CLI fragment {fragment:?}"),
        )?;
    }

    for required_boundary in [
        "does not claim beads",
        "does not replace RCH",
        "Agent Mail",
        "use the Beads assignee as the soft lock",
        "do not infer that nobody owns a file",
        "not release performance evidence",
        "strict drop-in claims",
        "Validation broker troubleshooting",
        "Agent Mail schema-corrupt",
        "deny_local_fallback",
        "Scratch-space, target-dir, or TMPDIR headroom is low",
        "Slot store is missing, malformed, or unavailable",
        "Reusable artifact provenance does not match",
        "These commands remain mandatory before commit",
        "rch exec -- cargo check --all-targets",
        "rch exec -- cargo clippy --all-targets -- -D warnings",
        "ubs --staged --only=rust .",
        "./scripts/reconcile_beads_ledger.sh",
    ] {
        require(
            runbook.contains(required_boundary),
            format!("operator docs missing boundary {required_boundary:?}"),
        )?;
    }

    Ok(())
}

#[test]
fn validation_broker_contract_preserves_redaction_and_no_claims() -> TestResult {
    let contract = load_contract()?;

    require_set(
        &contract,
        "/redaction_contract/required_redacted_classes",
        &[
            "api_key",
            "bearer_token",
            "oauth_token",
            "mail_body",
            "private_prompt",
            "absolute_home_path_when_not_needed",
            "large_command_output_body",
        ],
        "redacted class",
    )?;
    require_set(
        &contract,
        "/decision_contract/required_no_claims",
        &[
            "not_ci_success",
            "not_release_performance_evidence",
            "not_dropin_certification_evidence",
            "not_permission_to_skip_required_gates",
            "not_permission_to_modify_other_agents_files",
        ],
        "no-claim marker",
    )
}

#[test]
fn validation_broker_contract_links_downstream_beads_and_requirements() -> TestResult {
    let contract = load_contract()?;

    require_set(
        &contract,
        "/downstream_dependencies/unblocked_by_this_contract",
        &["bd-gusp4.2", "bd-gusp4.3", "bd-gusp4.6"],
        "downstream bead",
    )?;
    require(
        pointer_str(&contract, "/downstream_dependencies/fault_corpus_bead")? == "bd-gusp4.6",
        "fault corpus bead mismatch",
    )?;
    require(
        pointer_str(
            &contract,
            "/downstream_dependencies/doctor_runpack_projection_bead",
        )? == "bd-gusp4.7",
        "Doctor/runpack projection bead mismatch",
    )?;
    require(
        pointer_str(&contract, "/downstream_dependencies/final_closeout_bead")? == "bd-gusp4.11",
        "final closeout bead mismatch",
    )?;

    let requirements = pointer_array(&contract, "/must_requirements")?;
    require(
        requirements.len() >= 8,
        "contract must define enough must-requirements for closeout",
    )?;
    for requirement in requirements {
        let id = requirement
            .get("id")
            .and_then(Value::as_str)
            .ok_or("requirement id must be a string")?;
        require(
            id.starts_with("VALIDBROKER-MUST-"),
            "requirement id must use VALIDBROKER-MUST- prefix",
        )?;
        require(
            requirement
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| !description.trim().is_empty()),
            "requirement must have a description",
        )?;
        require(
            requirement
                .get("validated_by")
                .and_then(Value::as_array)
                .is_some_and(|validated_by| !validated_by.is_empty()),
            "requirement must name validation hooks",
        )?;
    }

    Ok(())
}

#[test]
fn validation_broker_closeout_gate_artifact_satisfies_contract() -> TestResult {
    let contract = load_json(CLOSEOUT_CONTRACT_PATH)?;
    let evidence = load_json(CLOSEOUT_EVIDENCE_PATH)?;
    let readme = load_text(README_PATH)?;
    let runbook = load_text(SWARM_RUNBOOK_PATH)?;

    require(
        pointer_str(&contract, "/schema")? == EXPECTED_CLOSEOUT_CONTRACT_SCHEMA,
        "closeout contract schema mismatch",
    )?;
    require(
        pointer_str(&contract, "/decision_gate_schema")? == EXPECTED_CLOSEOUT_SCHEMA,
        "closeout decision schema mismatch",
    )?;
    require(
        pointer_str(&evidence, "/schema")? == EXPECTED_CLOSEOUT_SCHEMA,
        "closeout evidence schema mismatch",
    )?;
    require(
        pointer_str(&evidence, "/purpose")?
            == "prompt_to_artifact_validation_broker_closeout_gate_not_source_of_truth",
        "closeout evidence purpose mismatch",
    )?;
    require(
        pointer_str(&evidence, "/status")? == "pass",
        "closeout evidence must pass",
    )?;
    require(
        pointer_str(&evidence, "/decision")? == "close_final_gate_and_parent_epic",
        "closeout decision mismatch",
    )?;
    require(
        pointer_bool(&evidence, "/epic_can_close_after_this_commit")?,
        "closeout evidence must authorize closing the epic after this commit",
    )?;
    require(
        !pointer_bool(&evidence, "/follow_up_required")?,
        "passing closeout evidence must not require follow-up beads",
    )?;
    require(
        pointer_array(&evidence, "/missing_checks")?.is_empty(),
        "passing closeout evidence must not have missing checks",
    )?;
    require(
        pointer_array(&evidence, "/follow_up_beads")?.is_empty(),
        "passing closeout evidence must not create follow-up beads",
    )?;

    let top_level_keys = string_set(&contract, "/required_top_level_keys")?;
    let top_level_key_list = top_level_keys.iter().copied().collect::<Vec<_>>();
    require_object_keys(&evidence, &top_level_key_list, "closeout evidence")?;
    require_required_strings_present(
        &contract,
        "/required_check_ids",
        &evidence,
        "/required_checks",
        "required closeout check",
    )?;

    let checklist = pointer_array(&evidence, "/checklist")?;
    for required_check in string_set(&contract, "/required_check_ids")? {
        let check = checklist
            .iter()
            .find(|entry| entry.get("id").and_then(Value::as_str) == Some(required_check))
            .ok_or_else(|| format!("closeout checklist missing {required_check}"))?;
        require(
            check.get("status").and_then(Value::as_str) == Some("pass"),
            format!("closeout checklist item {required_check} must pass"),
        )?;
        require(
            check
                .get("evidence")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()),
            format!("closeout checklist item {required_check} must name evidence"),
        )?;
    }

    let child_map = pointer_array(&evidence, "/child_artifact_map")?;
    for required_child in string_set(&contract, "/required_child_bead_ids")? {
        let child = child_map
            .iter()
            .find(|entry| entry.get("bead_id").and_then(Value::as_str) == Some(required_child))
            .ok_or_else(|| format!("closeout child map missing {required_child}"))?;
        require(
            child.get("status").and_then(Value::as_str) == Some("closed"),
            format!("closeout child {required_child} must be closed"),
        )?;
        for key in [
            "title",
            "close_reason",
            "commit",
            "code_paths",
            "test_paths",
            "docs_or_evidence_paths",
            "validation_commands",
        ] {
            require(
                child.get(key).is_some(),
                format!("child {required_child} missing {key}"),
            )?;
        }
        require(
            child
                .get("commit")
                .and_then(Value::as_str)
                .is_some_and(|commit| commit.len() >= 9),
            format!("child {required_child} must name a commit"),
        )?;
        for array_key in [
            "code_paths",
            "test_paths",
            "docs_or_evidence_paths",
            "validation_commands",
        ] {
            require(
                child
                    .get(array_key)
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty()),
                format!("child {required_child} must have non-empty {array_key}"),
            )?;
        }
        require(
            child
                .get("validation_commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| {
                    commands.iter().any(|command| {
                        command
                            .as_str()
                            .is_some_and(|text| text.contains("rch exec --"))
                    })
                }),
            format!("child {required_child} validation command must include RCH proof"),
        )?;
    }

    let quality_gates = pointer_array(&evidence, "/quality_gate_results")?;
    for required_gate in string_set(&contract, "/required_quality_gate_ids")? {
        let gate = quality_gates
            .iter()
            .find(|entry| entry.get("id").and_then(Value::as_str) == Some(required_gate))
            .ok_or_else(|| format!("closeout quality gate missing {required_gate}"))?;
        require(
            gate.get("status").and_then(Value::as_str) == Some("pass"),
            format!("quality gate {required_gate} must pass"),
        )?;
        let command = gate
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("quality gate {required_gate} must name command"))?;
        if required_gate.ends_with("_rch")
            || required_gate.contains("cargo_check")
            || required_gate.contains("cargo_clippy")
        {
            require(
                command.contains("rch exec --"),
                format!("heavy quality gate {required_gate} must use RCH"),
            )?;
        }
    }

    require(
        readme.contains("docs/contracts/validation-broker-closeout-gate-contract.json"),
        "README must link validation broker closeout contract",
    )?;
    require(
        readme.contains("docs/evidence/validation-broker-closeout-gate.json"),
        "README must link validation broker closeout evidence",
    )?;
    require(
        runbook.contains("pi.validation_broker.closeout_gate.v1"),
        "runbook must document validation broker closeout schema",
    )?;
    require(
        runbook.contains("docs/contracts/validation-broker-closeout-gate-contract.json"),
        "runbook must link validation broker closeout contract",
    )
}

fn require_required_strings_present(
    contract: &Value,
    required_path: &str,
    evidence: &Value,
    evidence_path: &str,
    label: &str,
) -> TestResult {
    let observed = string_set(evidence, evidence_path)?;
    for required in string_set(contract, required_path)? {
        require(
            observed.contains(required),
            format!("missing {label}: {required}"),
        )?;
    }
    Ok(())
}
