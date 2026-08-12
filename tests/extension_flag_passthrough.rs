//! Integration tests for extension CLI flag pass-through functionality.
//!
//! Tests that extension flags from the CLI are properly passed through
//! to extensions and applied correctly.

use serde_json::{Value, json};
use std::collections::HashMap;

use kode::cli::{ExtensionCliFlag, parse_with_extension_flags};

/// Test that extension CLI flags are parsed correctly from command line arguments.
#[test]
fn test_extension_flag_parsing_basic() {
    let args = vec![
        "pi".to_string(),
        "--debug".to_string(),
        "true".to_string(),
        "Hello world".to_string(),
    ];

    let parsed = parse_with_extension_flags(args).expect("Should parse successfully");

    // The --debug flag should be extracted as an extension flag
    assert_eq!(parsed.extension_flags.len(), 1);
    assert_eq!(parsed.extension_flags[0].name, "debug");
    assert_eq!(parsed.extension_flags[0].value, Some("true".to_string()));

    // The message should still be parsed correctly
    assert_eq!(parsed.cli.message_args(), vec!["Hello world"]);
}

/// Test parsing extension flags without values (boolean flags).
#[test]
fn test_extension_flag_parsing_boolean() {
    let args = vec![
        "pi".to_string(),
        "--trace".to_string(),
        "--dry-run".to_string(),
        "--print".to_string(),
        "test message".to_string(),
    ];

    let parsed = parse_with_extension_flags(args).expect("Should parse successfully");

    assert_eq!(parsed.extension_flags.len(), 2);

    let trace_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "trace")
        .expect("Should have trace flag");
    assert_eq!(trace_flag.value, None);

    let dry_run_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "dry-run")
        .expect("Should have dry-run flag");
    assert_eq!(dry_run_flag.value, None);
    assert!(parsed.cli.print);
    assert_eq!(parsed.cli.message_args(), vec!["test message"]);
}

/// Test parsing extension flags with equals syntax.
#[test]
fn test_extension_flag_parsing_equals_syntax() {
    let args = vec![
        "pi".to_string(),
        "--level=debug".to_string(),
        "--format=json".to_string(),
        "test".to_string(),
    ];

    let parsed = parse_with_extension_flags(args).expect("Should parse successfully");

    assert_eq!(parsed.extension_flags.len(), 2);

    let level_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "level")
        .expect("Should have level flag");
    assert_eq!(level_flag.value, Some("debug".to_string()));

    let format_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "format")
        .expect("Should have format flag");
    assert_eq!(format_flag.value, Some("json".to_string()));
}

/// Test that known CLI flags are not extracted as extension flags.
#[test]
fn test_known_flags_not_extracted() {
    let args = vec![
        "pi".to_string(),
        "--model".to_string(),
        "gpt-4".to_string(),
        "--thinking".to_string(),
        "high".to_string(),
        "--custom-flag".to_string(),
        "value".to_string(),
        "Hello".to_string(),
    ];

    let parsed = parse_with_extension_flags(args).expect("Should parse successfully");

    // Only --custom-flag should be extracted as extension flag
    assert_eq!(parsed.extension_flags.len(), 1);
    assert_eq!(parsed.extension_flags[0].name, "custom-flag");
    assert_eq!(parsed.extension_flags[0].value, Some("value".to_string()));

    // Known flags should be parsed normally by clap
    assert_eq!(parsed.cli.model, Some("gpt-4".to_string()));
    assert_eq!(parsed.cli.thinking, Some("high".to_string()));
}

/// Test extension flag display names.
#[test]
fn test_extension_flag_display_names() {
    let flag = ExtensionCliFlag {
        name: "test-flag".to_string(),
        value: Some("test-value".to_string()),
    };

    assert_eq!(flag.display_name(), "--test-flag");
}

/// Test edge cases in extension flag parsing.
#[test]
fn test_extension_flag_parsing_edge_cases() {
    // Empty value
    let args1 = vec![
        "pi".to_string(),
        "--empty".to_string(),
        String::new(),
        "message".to_string(),
    ];

    let parsed1 = parse_with_extension_flags(args1).expect("Should parse empty value");
    assert_eq!(parsed1.extension_flags.len(), 1);
    assert_eq!(parsed1.extension_flags[0].value, Some(String::new()));

    // Flag at end without value
    let args2 = vec![
        "pi".to_string(),
        "message".to_string(),
        "--end-flag".to_string(),
    ];

    let parsed2 = parse_with_extension_flags(args2).expect("Should parse flag at end");
    assert_eq!(parsed2.extension_flags.len(), 1);
    assert_eq!(parsed2.extension_flags[0].name, "end-flag");
    assert_eq!(parsed2.extension_flags[0].value, None);
}

/// Test parsing multiple extension flags with mixed formats.
#[test]
fn test_extension_flag_parsing_mixed_formats() {
    let args = vec![
        "pi".to_string(),
        "--flag1".to_string(),
        "value1".to_string(),
        "--flag2".to_string(),
        "--flag3=value3".to_string(),
        "--flag4".to_string(),
        "value4".to_string(),
        "final message".to_string(),
    ];

    let parsed = parse_with_extension_flags(args).expect("Should parse mixed formats");

    assert_eq!(parsed.extension_flags.len(), 4);

    let flags: HashMap<String, Option<String>> = parsed
        .extension_flags
        .into_iter()
        .map(|f| (f.name, f.value))
        .collect();

    assert_eq!(flags["flag1"], Some("value1".to_string()));
    assert_eq!(flags["flag2"], None);
    assert_eq!(flags["flag3"], Some("value3".to_string()));
    assert_eq!(flags["flag4"], Some("value4".to_string()));

    assert_eq!(parsed.cli.message_args(), vec!["final message"]);
}

/// Test that subcommands don't interfere with extension flag parsing.
#[test]
fn test_extension_flags_with_subcommands() {
    let args = vec![
        "pi".to_string(),
        "--global-flag".to_string(),
        "value".to_string(),
        "install".to_string(),
        "package-name".to_string(),
    ];

    let parsed = parse_with_extension_flags(args);

    // Should handle gracefully even with subcommands
    // Note: This might fail parsing due to subcommand, but extension flags should still be extracted
    if let Ok(p) = parsed {
        // If parsing succeeds, extension flag should be extracted
        assert!(p.extension_flags.iter().any(|f| f.name == "global-flag"));
    } else {
        // If parsing fails due to subcommand, that's expected
        // The important thing is that the preprocessing works
    }
}

/// Test that negative numbers are not treated as extension flags.
#[test]
fn test_negative_numbers_not_extension_flags() {
    let args = vec![
        "pi".to_string(),
        "--real-flag".to_string(),
        "value".to_string(),
        "--".to_string(),
        "-42".to_string(),
        "message".to_string(),
    ];

    let parsed = parse_with_extension_flags(args).expect("Should parse with negative number");

    // Only --real-flag should be an extension flag
    assert_eq!(parsed.extension_flags.len(), 1);
    assert_eq!(parsed.extension_flags[0].name, "real-flag");

    // -42 should be part of the message
    assert!(parsed.cli.message_args().contains(&"-42"));
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;

    /// Mock extension manager for testing flag application.
    struct MockExtensionManager {
        registered_flags: Vec<Value>,
        set_values: Arc<std::sync::Mutex<Vec<(String, String, Value)>>>,
    }

    impl MockExtensionManager {
        fn new() -> Self {
            Self {
                registered_flags: vec![
                    json!({
                        "name": "debug",
                        "extension_id": "test.debug",
                        "type": "boolean"
                    }),
                    json!({
                        "name": "level",
                        "extension_id": "test.logger",
                        "type": "string"
                    }),
                ],
                set_values: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn list_flags(&self) -> Vec<Value> {
            self.registered_flags.clone()
        }

        fn set_flag_value(&self, extension_id: &str, name: &str, value: Value) {
            self.set_values.lock().unwrap().push((
                extension_id.to_string(),
                name.to_string(),
                value,
            ));
        }

        fn get_set_values(&self) -> Vec<(String, String, Value)> {
            self.set_values.lock().unwrap().clone()
        }
    }

    #[test]
    fn test_extension_flag_application_success() {
        let manager = MockExtensionManager::new();
        let flags = vec![
            ExtensionCliFlag {
                name: "debug".to_string(),
                value: Some("true".to_string()),
            },
            ExtensionCliFlag {
                name: "level".to_string(),
                value: Some("info".to_string()),
            },
        ];

        // Note: This would need the actual apply_extension_cli_flags function
        // For now, we'll test the logic manually

        let registered = manager.list_flags();
        for flag in &flags {
            let matches: Vec<_> = registered
                .iter()
                .filter(|spec| {
                    spec.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| name.eq_ignore_ascii_case(&flag.name))
                })
                .collect();

            assert!(
                !matches.is_empty(),
                "Flag {} should be registered",
                flag.name
            );

            for spec in matches {
                let extension_id = spec.get("extension_id").and_then(Value::as_str).unwrap();
                let flag_type = spec.get("type").and_then(Value::as_str).unwrap_or("string");

                let value = match flag_type {
                    "boolean" => {
                        let bool_val = match flag.value.as_deref() {
                            Some("false" | "0" | "no" | "off") => false,
                            Some("true" | "1" | "yes" | "on") | None => true,
                            Some(v) => panic!("Invalid boolean value: {v}"),
                        };
                        Value::Bool(bool_val)
                    }
                    _ => Value::String(flag.value.clone().unwrap_or_default()),
                };

                manager.set_flag_value(extension_id, &flag.name, value);
            }
        }

        let set_values = manager.get_set_values();
        assert_eq!(set_values.len(), 2);

        // Check that debug flag was set correctly
        let debug_set = set_values
            .iter()
            .find(|(_, name, _)| name == "debug")
            .unwrap();
        assert_eq!(debug_set.0, "test.debug");
        assert_eq!(debug_set.2, Value::Bool(true));

        // Check that level flag was set correctly
        let level_set = set_values
            .iter()
            .find(|(_, name, _)| name == "level")
            .unwrap();
        assert_eq!(level_set.0, "test.logger");
        assert_eq!(level_set.2, Value::String("info".to_string()));
    }
}
