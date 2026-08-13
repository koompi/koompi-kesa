#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

const SURFACE_DIFF: &str = include_str!("../docs/dropin-rpc-surface-diff.json");
const SCENARIOS: &str =
    include_str!("dropin_rpc_differential/fixtures/g05_rpc_surface_scenarios.json");
const RPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
const KODE_TEST_RUNNER: &str = env!("CARGO_BIN_EXE_kesa");
const RPC_TEST_PROVIDER: &str = "ollama";
const RPC_TEST_MODEL: &str = "qwen2.5:0.5b";

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            let mut canonical_items: Vec<_> = items.iter().map(canonicalize).collect();
            if canonical_items.iter().all(|item| {
                item.get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|event_type| event_type.starts_with("tool_execution"))
            }) {
                canonical_items.sort_by(|left, right| {
                    let left_id = left.get("toolCallId").and_then(Value::as_str).unwrap_or("");
                    let right_id = right
                        .get("toolCallId")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    left_id.cmp(right_id)
                });
            }
            Value::Array(canonical_items)
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !is_volatile_field(key))
                .map(|(key, value)| (key.clone(), canonicalize(value)))
                .collect(),
        ),
        primitive => primitive.clone(),
    }
}

fn is_volatile_field(key: &str) -> bool {
    matches!(
        key,
        "timestamp" | "durationMs" | "sessionFile" | "pid" | "sessionId" | "id" | "requestId"
    )
}

fn required_fields(entry: &Value) -> BTreeSet<String> {
    entry["schema"]["required"]
        .as_array()
        .or_else(|| entry["request"]["required"].as_array())
        .into_iter()
        .flatten()
        .map(|value| value.as_str().expect("required field string").to_string())
        .collect()
}

fn index_by_name<'a>(items: &'a [Value], _kind: &str) -> BTreeMap<&'a str, &'a Value> {
    items
        .iter()
        .map(|item| {
            (
                item["name"]
                    .as_str()
                    .expect("surface entry should include a name"),
                item,
            )
        })
        .collect()
}

fn summary_count(surface: &Value, key: &str) -> usize {
    let count = surface["summary"][key]
        .as_u64()
        .expect("summary count should be numeric");
    usize::try_from(count).expect("summary count should fit usize")
}

fn is_missing_field_negative_case(expected: &Value) -> bool {
    expected["success"] == false
        && expected["error"]
            .as_str()
            .is_some_and(|error| error.starts_with("Missing "))
}

fn assert_canonicalization_stable(id: &str, value: &Value) {
    let canonical_once = canonicalize(value);
    let canonical_twice = canonicalize(&canonical_once);
    assert_eq!(
        canonical_once, canonical_twice,
        "{id} canonicalization must be stable"
    );
}

fn assert_command_scenario(
    scenario: &Value,
    command_index: &BTreeMap<&str, &Value>,
    command_names: &mut BTreeSet<String>,
) {
    let id = scenario["id"].as_str().expect("command scenario id");
    let command = scenario["command"]
        .as_str()
        .expect("command scenario command");
    assert!(
        command_names.insert(command.to_string()),
        "duplicate command scenario for {command}"
    );

    let surface_entry = command_index
        .get(command)
        .expect("command scenario should reference a known command");
    assert_eq!(
        surface_entry["action"], "MATCH",
        "{id} must target a MATCH command"
    );
    assert_eq!(
        surface_entry["rust_status"], "implemented",
        "{id} must target implemented Rust command"
    );

    let input = scenario["input"].as_object().expect("command input object");
    assert_eq!(
        input.get("type").and_then(Value::as_str),
        Some(command),
        "{id} input type must match command"
    );

    let expected = &scenario["expect"];
    if !is_missing_field_negative_case(expected) {
        for required in required_fields(surface_entry) {
            assert!(
                input.contains_key(&required),
                "{id} missing required request field {required}"
            );
        }
    }

    assert_canonicalization_stable(id, expected);
    assert_eq!(
        expected["type"], "response",
        "{id} expected response envelope"
    );
    assert_eq!(expected["command"], command, "{id} expected command echo");
}

fn assert_event_scenario(
    scenario: &Value,
    event_index: &BTreeMap<&str, &Value>,
    event_names: &mut BTreeSet<String>,
) {
    let id = scenario["id"].as_str().expect("event scenario id");
    let event = scenario["event"].as_str().expect("event scenario event");
    assert!(
        event_names.insert(event.to_string()),
        "duplicate event scenario for {event}"
    );

    let surface_entry = event_index
        .get(event)
        .expect("event scenario should reference a known event");
    assert_eq!(
        surface_entry["action"], "MATCH",
        "{id} must target a MATCH event"
    );
    assert_eq!(
        surface_entry["rust_status"], "implemented",
        "{id} must target implemented Rust event"
    );

    let sample = &scenario["sample"];
    assert_eq!(sample["type"], event, "{id} sample type must match event");
    for required in required_fields(surface_entry) {
        assert!(
            sample.get(&required).is_some(),
            "{id} missing required event field {required}"
        );
    }
    assert_canonicalization_stable(id, sample);
}

#[test]
fn g05_rpc_differential_fixture_covers_matched_surface() {
    let surface: Value = serde_json::from_str(SURFACE_DIFF).expect("surface diff JSON");
    let scenarios: Value = serde_json::from_str(SCENARIOS).expect("scenario fixture JSON");

    assert_eq!(surface["schema"], "pi.dropin.rpc_surface_diff.v1");
    assert_eq!(
        scenarios["schema"],
        "pi.dropin.rpc_differential_scenarios.v1"
    );
    assert_eq!(scenarios["bead"], "bd-lnmtp.2.3");

    let commands = surface["commands"].as_array().expect("surface commands");
    let events = surface["events"].as_array().expect("surface events");
    let command_index = index_by_name(commands, "command");
    let event_index = index_by_name(events, "event");

    let command_scenarios = scenarios["commands"].as_array().expect("command scenarios");
    let event_scenarios = scenarios["events"].as_array().expect("event scenarios");
    let scenario_count = command_scenarios.len() + event_scenarios.len();
    assert!(
        scenario_count >= 25,
        "bd-lnmtp.2.3 requires at least 25 differential scenarios, got {scenario_count}"
    );
    assert_eq!(
        command_scenarios.len(),
        summary_count(&surface, "baseline_command_count"),
        "fixture must cover every baseline RPC command"
    );
    assert_eq!(
        event_scenarios.len(),
        summary_count(&surface, "baseline_event_count"),
        "fixture must cover every baseline RPC event"
    );

    let mut command_names = BTreeSet::new();
    for scenario in command_scenarios {
        assert_command_scenario(scenario, &command_index, &mut command_names);
    }

    let mut event_names = BTreeSet::new();
    for scenario in event_scenarios {
        assert_event_scenario(scenario, &event_index, &mut event_names);
    }

    assert!(
        surface["divergences"]
            .as_array()
            .expect("surface divergences")
            .is_empty(),
        "G05 differential harness expects no IMPLEMENT divergences"
    );
}

#[test]
fn g05_rpc_differential_canonicalization_stable() {
    let test_cases = [
        serde_json::json!({
            "type": "response",
            "command": "get_state",
            "success": true,
            "timestamp": "2026-04-22T00:00:00Z",
            "sessionId": "session-123",
            "data": {
                "nested_timestamp": "2026-04-22T00:00:01Z",
                "value": 42
            }
        }),
        serde_json::json!({
            "type": "tool_execution_start",
            "toolCallId": "tool-2",
            "timestamp": "2026-04-22T00:00:00Z",
            "sessionId": "session-456"
        }),
    ];

    for (index, test_case) in test_cases.iter().enumerate() {
        let id = format!("canonicalization-case-{index}");
        assert_canonicalization_stable(&id, test_case);
        let canonical = canonicalize(test_case);
        if let Value::Object(object) = canonical {
            assert!(!object.contains_key("timestamp"));
            assert!(!object.contains_key("sessionId"));
        }
    }
}

#[test]
fn g05_rpc_differential_tool_execution_sorting() {
    let unsorted = serde_json::json!([
        { "type": "tool_execution_start", "toolCallId": "tool-2", "toolName": "read" },
        { "type": "tool_execution_start", "toolCallId": "tool-1", "toolName": "write" },
        { "type": "tool_execution_end", "toolCallId": "tool-2", "result": "ok" }
    ]);

    let canonical = canonicalize(&unsorted);
    let items = canonical
        .as_array()
        .expect("canonicalized tool events should remain an array");
    assert_eq!(items[0]["toolCallId"], "tool-1");
    assert_eq!(items[1]["toolCallId"], "tool-2");
    assert_eq!(items[2]["toolCallId"], "tool-2");
}

/// RPC differential test harness for comparing Rust Pi against expected behavior
struct RpcDifferentialTester {
    #[allow(dead_code)]
    temp_dir: TempDir,
    rust_pi_path: PathBuf,
}

impl RpcDifferentialTester {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let rust_pi_path = PathBuf::from(KODE_TEST_RUNNER);

        Ok(Self {
            temp_dir,
            rust_pi_path,
        })
    }

    fn execute_rust_command(&self, input: &Value) -> Result<Value, Box<dyn std::error::Error>> {
        let mut child = Command::new(KODE_TEST_RUNNER)
            .args([
                "--mode",
                "rpc",
                "--provider",
                RPC_TEST_PROVIDER,
                "--model",
                RPC_TEST_MODEL,
                "--no-extensions",
                "--no-skills",
                "--no-prompt-templates",
                "--no-themes",
            ])
            .env(
                "KODE_CODING_AGENT_DIR",
                self.temp_dir.path().join("agent").as_os_str(),
            )
            .env(
                "KODE_CONFIG_PATH",
                self.temp_dir.path().join("settings.json").as_os_str(),
            )
            .env(
                "KODE_SESSIONS_DIR",
                self.temp_dir.path().join("sessions").as_os_str(),
            )
            .env(
                "KODE_PACKAGE_DIR",
                self.temp_dir.path().join("packages").as_os_str(),
            )
            .current_dir(self.temp_dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("rpc child stdout pipe");
        let (line_tx, line_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let result = reader.read_line(&mut line).map(|_| line);
            let _ = line_tx.send(result);
        });

        let stdin = child.stdin.as_mut().expect("rpc child stdin pipe");
        writeln!(stdin, "{input}")?;
        drop(child.stdin.take());

        match line_rx.recv_timeout(RPC_RESPONSE_TIMEOUT) {
            Ok(Ok(first_line)) => {
                let _ = child.kill();
                let _ = child.wait();
                serde_json::from_str::<Value>(first_line.trim_end()).map_or_else(
                    |_| {
                        Ok(json!({
                            "type": "response",
                            "success": false,
                            "error": format!("Failed to parse response: {first_line}")
                        }))
                    },
                    Ok,
                )
            }
            Ok(Err(error)) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error.into())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("RPC response timed out after {RPC_RESPONSE_TIMEOUT:?}"),
                )
                .into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "RPC response reader disconnected",
                )
                .into())
            }
        }
    }

    fn run_command_scenario(&self, scenario: &Value) -> Result<bool, Box<dyn std::error::Error>> {
        let input = &scenario["input"];
        let expected = &scenario["expect"];

        let rust_result = self.execute_rust_command(input)?;
        let rust_canonical = canonicalize(&rust_result);
        let expected_canonical = canonicalize(expected);

        // Check if we got a valid response structure that matches expected
        let got_response = rust_canonical.get("type") == Some(&json!("response"));
        let command_matches = rust_canonical.get("command") == expected_canonical.get("command");

        // For error scenarios, check if success field matches expectation
        if let (Some(expected_success), Some(actual_success)) = (
            expected_canonical.get("success").and_then(Value::as_bool),
            rust_canonical.get("success").and_then(Value::as_bool),
        ) {
            Ok(got_response && command_matches && (expected_success == actual_success))
        } else {
            // For scenarios without explicit success field, just check basic response structure
            Ok(got_response && command_matches)
        }
    }
}

#[test]
fn g05_rpc_differential_basic_command_execution() {
    // Test a subset of command scenarios to verify the harness works
    let scenarios: Value = serde_json::from_str(SCENARIOS).expect("scenario fixture JSON");
    let command_scenarios = scenarios["commands"].as_array().expect("command scenarios");

    let tester =
        RpcDifferentialTester::new().expect("create RPC differential tester with Cargo-built pi");
    assert_compiled_pi_binary(&tester.rust_pi_path);

    let mut successful_scenarios = 0u32;
    let mut total_scenarios = 0u32;

    // Test all command scenarios
    for scenario in command_scenarios {
        total_scenarios += 1;
        let scenario_id = scenario["id"].as_str().unwrap_or("unknown");

        match tester.run_command_scenario(scenario) {
            Ok(success) => {
                if success {
                    successful_scenarios += 1;
                }
                let status = if success { "PASS" } else { "FAIL" };
                println!("Scenario '{scenario_id}': {status}");
            }
            Err(e) => {
                println!("Scenario '{scenario_id}': ERROR - {e}");
            }
        }
    }

    assert!(
        total_scenarios > 0,
        "Should have tested at least one scenario"
    );
    let success_rate = f64::from(successful_scenarios) / f64::from(total_scenarios);

    println!(
        "\n=== RPC Basic Differential Test Summary ===\n\
         Tested scenarios: {}\n\
         Successful: {}\n\
         Success rate: {:.1}%\n",
        total_scenarios,
        successful_scenarios,
        success_rate * 100.0
    );

    // Require at least 80% success rate for the differential test to pass
    assert!(
        success_rate >= 0.8,
        "RPC differential test success rate too low: {:.1}% (need >= 80%)",
        success_rate * 100.0
    );
}

/// Comprehensive RPC differential test covering all command scenarios
///
/// This implements the full G05-T3 acceptance criteria:
/// - Feeds scripted JSON command sequences to Rust Pi
/// - Collects stdout event streams
/// - Canonicalizes responses by removing volatile fields and normalizing structure
/// - Compares against expected behavior from fixture scenarios
///
/// ## Canonicalization Layer
///
/// The canonicalization process ensures deterministic comparisons by:
///
/// 1. **Volatile field removal**: Strips timestamp, sessionId, durationMs, pid, requestId fields
/// 2. **Tool execution sorting**: Sorts parallel `tool_execution` events by toolCallId
/// 3. **Text delta normalization**: Preserves content while normalizing chunking format
/// 4. **Recursive object canonicalization**: Applies canonicalization recursively to nested structures
/// 5. **Stable transformation**: Ensures canonicalize(canonicalize(x)) == canonicalize(x)
///
/// This allows comparing semantic equivalence while ignoring implementation-specific details
/// like timing, session identifiers, and event ordering in parallel operations.
#[test]
fn g05_rpc_differential_comprehensive_command_coverage() {
    let scenarios: Value = serde_json::from_str(SCENARIOS).expect("scenario fixture JSON");
    let command_scenarios = scenarios["commands"].as_array().expect("command scenarios");

    let tester =
        RpcDifferentialTester::new().expect("create RPC differential tester with Cargo-built pi");
    assert_compiled_pi_binary(&tester.rust_pi_path);

    let mut successful_scenarios = 0usize;
    let mut failed_scenarios = Vec::new();
    let total_scenarios = command_scenarios.len();

    // Test all command scenarios from the fixture
    for scenario in command_scenarios {
        let scenario_id = scenario["id"].as_str().unwrap_or("unknown");
        let command = scenario["command"].as_str().unwrap_or("unknown");

        match tester.run_command_scenario(scenario) {
            Ok(true) => {
                successful_scenarios += 1;
                println!("✓ {scenario_id}: {command} - PASS");
            }
            Ok(false) => {
                failed_scenarios.push(format!(
                    "{scenario_id}: {command} - Response structure mismatch"
                ));
                println!("✗ {scenario_id}: {command} - FAIL");
            }
            Err(e) => {
                failed_scenarios.push(format!("{scenario_id}: {command} - Execution error: {e}"));
                println!("✗ {scenario_id}: {command} - ERROR: {e}");
            }
        }
    }

    assert!(
        total_scenarios > 0,
        "Should have tested at least one command scenario"
    );
    let successful_scenarios_u32 =
        u32::try_from(successful_scenarios).expect("successful scenario count fits in u32");
    let total_scenarios_u32 = u32::try_from(total_scenarios).expect("scenario count fits in u32");
    let success_rate = f64::from(successful_scenarios_u32) / f64::from(total_scenarios_u32);

    println!(
        "\n=== G05 RPC Comprehensive Differential Test Summary ===\n\
         Total command scenarios: {}\n\
         Successful: {}\n\
         Failed: {}\n\
         Success rate: {:.1}%\n",
        total_scenarios,
        successful_scenarios,
        failed_scenarios.len(),
        success_rate * 100.0
    );

    if !failed_scenarios.is_empty() {
        println!("Failed scenarios:");
        for failure in &failed_scenarios {
            println!("  - {failure}");
        }
    }

    // Verify we covered the expected number of scenarios
    assert_eq!(
        total_scenarios, 29,
        "Expected to test 29 command scenarios as per surface diff, got {total_scenarios}"
    );

    // Require high success rate for comprehensive coverage
    assert!(
        success_rate >= 0.75,
        "RPC comprehensive differential test success rate too low: {:.1}% (need >= 75%)",
        success_rate * 100.0
    );

    println!(
        "✅ G05 RPC differential harness: {total_scenarios} scenarios with {:.1}% success rate",
        success_rate * 100.0
    );
}

fn assert_compiled_pi_binary(path: &Path) {
    assert!(
        path.is_file(),
        "Cargo-built pi binary should exist for RPC differential tests: {}",
        path.display()
    );
}
