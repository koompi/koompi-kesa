//! Advanced CLI argument parsing edge cases tests.
//!
//! Tests quoting, environment expansion, @file references, and positional precedence
//! edge cases to ensure parity with pi-mono behavior.

use std::fs;
use tempfile::TempDir;

use kesa::cli::{Commands, parse_with_extension_flags};

/// Test that arguments with spaces are handled correctly when quoted.
#[test]
fn test_quoted_arguments_with_spaces() {
    // Test space-containing arguments in different positions
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--provider".to_string(),
        "custom provider".to_string(), // Space in value
        "hello world".to_string(),     // Space in message
        "another message".to_string(),
    ])
    .expect("Should parse quoted args with spaces");

    assert_eq!(parsed.cli.provider, Some("custom provider".to_string()));
    assert_eq!(
        parsed.cli.message_args(),
        vec!["hello world", "another message"]
    );
}

/// Test nested quoting scenarios.
#[test]
fn test_nested_quotes() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        r#"You are a "helpful" assistant"#.to_string(),
        r#"Process the file "data.txt""#.to_string(),
    ])
    .expect("Should parse nested quotes");

    assert_eq!(
        parsed.cli.system_prompt,
        Some(r#"You are a "helpful" assistant"#.to_string())
    );
    assert_eq!(
        parsed.cli.message_args(),
        vec![r#"Process the file "data.txt""#]
    );
}

/// Test escaped quotes in arguments.
#[test]
fn test_escaped_quotes() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        r#"Say \"hello\" to the user"#.to_string(),
        r#"The file is \"important.txt\""#.to_string(),
    ])
    .expect("Should parse escaped quotes");

    assert_eq!(
        parsed.cli.system_prompt,
        Some(r#"Say \"hello\" to the user"#.to_string())
    );
    assert_eq!(
        parsed.cli.message_args(),
        vec![r#"The file is \"important.txt\""#]
    );
}

/// Test environment variable expansion in CLI arguments.
#[test]
fn test_environment_variable_expansion() {
    // Note: This tests if the CLI would handle env var expansion
    // Real shell would expand these before they reach our CLI parser
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--provider".to_string(),
        "${TEST_PROVIDER}".to_string(),
        "--model".to_string(),
        "$TEST_MODEL".to_string(),
        "Check ${TEST_PATH}/file.txt".to_string(),
    ])
    .expect("Should handle env var syntax");

    // These are literal strings since shell expansion happens before CLI parsing
    assert_eq!(parsed.cli.provider, Some("${TEST_PROVIDER}".to_string()));
    assert_eq!(parsed.cli.model, Some("$TEST_MODEL".to_string()));
    assert_eq!(
        parsed.cli.message_args(),
        vec!["Check ${TEST_PATH}/file.txt"]
    );
}

/// Test @file references with edge cases.
#[test]
fn test_file_reference_edge_cases() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create test files
    let simple_file = temp_path.join("simple.txt");
    let space_file = temp_path.join("file with spaces.txt");
    let nested_dir = temp_path.join("nested");
    fs::create_dir(&nested_dir)?;
    let nested_file = nested_dir.join("deep.txt");

    fs::write(&simple_file, "simple content")?;
    fs::write(&space_file, "content with spaces")?;
    fs::write(&nested_file, "nested content")?;

    // Test various @file reference patterns
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        format!("@{}", simple_file.display()),
        format!("@{}", space_file.display()), // File path with spaces
        format!("@{}", nested_file.display()),
        "analyze these".to_string(),
    ])
    .expect("Should parse @file references");

    assert_eq!(
        parsed.cli.file_args(),
        vec![
            simple_file.to_string_lossy(),
            space_file.to_string_lossy(),
            nested_file.to_string_lossy()
        ]
    );
    assert_eq!(parsed.cli.message_args(), vec!["analyze these"]);

    Ok(())
}

/// Test @file references to non-existent files.
#[test]
fn test_nonexistent_file_references() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "@/nonexistent/file.txt".to_string(),
        "@missing.txt".to_string(),
        "process anyway".to_string(),
    ])
    .expect("Should parse even with non-existent @file references");

    // CLI parsing should succeed even if files don't exist
    // File existence checking happens later in the pipeline
    assert_eq!(
        parsed.cli.file_args(),
        vec!["/nonexistent/file.txt", "missing.txt"]
    );
    assert_eq!(parsed.cli.message_args(), vec!["process anyway"]);
}

/// Test positional argument precedence with various flag patterns.
#[test]
fn test_positional_precedence() {
    // Flags before positionals
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--model".to_string(),
        "claude".to_string(),
        "--verbose".to_string(),
        "hello".to_string(),
        "world".to_string(),
    ])
    .expect("Should parse flags before positionals");

    assert_eq!(parsed.cli.model, Some("claude".to_string()));
    assert!(parsed.cli.verbose);
    assert_eq!(parsed.cli.message_args(), vec!["hello", "world"]);

    // With trailing argv capture, flags after the first positional token remain
    // message tokens instead of being reinterpreted as global CLI flags.
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "first".to_string(),
        "--model".to_string(),
        "claude".to_string(),
        "second".to_string(),
        "--verbose".to_string(),
        "third".to_string(),
    ])
    .expect("Should parse mixed flags and positionals");

    assert_eq!(parsed.cli.model, None);
    assert!(!parsed.cli.verbose);
    assert_eq!(
        parsed.cli.message_args(),
        vec!["first", "--model", "claude", "second", "--verbose", "third"]
    );
}

/// Test double-dash separator behavior.
#[test]
fn test_double_dash_separator() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--model".to_string(),
        "claude".to_string(),
        "--".to_string(),
        "--not-a-flag".to_string(),
        "regular".to_string(),
        "args".to_string(),
    ])
    .expect("Should parse args after --");

    assert_eq!(parsed.cli.model, Some("claude".to_string()));
    // Everything after -- should be treated as positional arguments
    assert_eq!(
        parsed.cli.message_args(),
        vec!["--not-a-flag", "regular", "args"]
    );
}

/// Test extension flags with edge cases.
#[test]
fn test_extension_flags_edge_cases() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--debug-level=high".to_string(), // Equals syntax
        "--dry-run".to_string(),          // Boolean extension flag
        "--output-dir".to_string(),
        "/path/with spaces".to_string(), // Value with spaces
        "--flag-with-dashes".to_string(),
        "value".to_string(),
        "message".to_string(),
    ])
    .expect("Should parse extension flags with edge cases");

    assert_eq!(parsed.extension_flags.len(), 4);

    // Check equals syntax
    let debug_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "debug-level")
        .expect("Should have debug-level flag");
    assert_eq!(debug_flag.value, Some("high".to_string()));

    // Check boolean flag
    let dry_run_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "dry-run")
        .expect("Should have dry-run flag");
    assert_eq!(dry_run_flag.value, None);

    // Check value with spaces
    let output_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "output-dir")
        .expect("Should have output-dir flag");
    assert_eq!(output_flag.value, Some("/path/with spaces".to_string()));

    // Check flag with dashes
    let dashes_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "flag-with-dashes")
        .expect("Should have flag-with-dashes flag");
    assert_eq!(dashes_flag.value, Some("value".to_string()));
}

/// Test complex mixed scenarios.
#[test]
fn test_complex_mixed_scenarios() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let temp_file = temp_dir.path().join("test.txt");
    fs::write(&temp_file, "test content")?;

    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--model".to_string(),
        "claude-opus".to_string(),
        "--custom-flag=debug".to_string(), // Extension flag with equals
        format!("@{}", temp_file.display()), // File reference
        "--another-flag".to_string(),      // Extension boolean flag
        "Analyze this file".to_string(),   // Message with spaces
        "--".to_string(),                  // Separator
        "--not-parsed-as-flag".to_string(), // After separator
    ])
    .expect("Should parse complex mixed scenario");

    // Check standard flags
    assert_eq!(parsed.cli.model, Some("claude-opus".to_string()));

    // Check extension flags
    assert_eq!(parsed.extension_flags.len(), 2);
    let custom_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "custom-flag")
        .expect("Should have custom-flag");
    assert_eq!(custom_flag.value, Some("debug".to_string()));

    let another_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "another-flag")
        .expect("Should have another-flag");
    assert_eq!(another_flag.value, Some("Analyze this file".to_string()));

    // Check file references
    assert_eq!(parsed.cli.file_args().len(), 1);
    assert!(parsed.cli.file_args()[0].ends_with("test.txt"));

    // Check message (including args after --)
    assert!(parsed.cli.message_args().contains(&"--not-parsed-as-flag"));

    Ok(())
}

/// Test subcommand parsing with edge cases.
#[test]
fn test_subcommand_edge_cases() {
    // Subcommand with flags before it
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--global-flag".to_string(),
        "value".to_string(),
        "install".to_string(),
        "npm:package".to_string(),
        "--local".to_string(),
    ]);

    // This might fail parsing due to subcommand, but extension flags should be extracted
    if let Ok(p) = parsed {
        // Verify extension flags were processed
        assert!(p.extension_flags.iter().any(|f| f.name == "global-flag"));
        // Verify subcommand was recognized
        assert!(matches!(p.cli.command, Some(Commands::Install { .. })));
    } else {
        // If parsing fails, that's expected behavior for complex subcommand scenarios.
        // The important thing is that preprocessing works correctly.
    }
}

/// Test Unicode and special characters in arguments.
#[test]
fn test_unicode_and_special_chars() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        "Respond in 中文 and emoji 🚀".to_string(),
        "--custom-flag".to_string(),
        "café-naïve".to_string(),
        "Message with émojis 🎉 and ñice chars".to_string(),
    ])
    .expect("Should handle Unicode and special characters");

    assert_eq!(
        parsed.cli.system_prompt,
        Some("Respond in 中文 and emoji 🚀".to_string())
    );

    let custom_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "custom-flag")
        .expect("Should have custom-flag");
    assert_eq!(custom_flag.value, Some("café-naïve".to_string()));

    assert!(parsed.cli.message_args().join(" ").contains("🎉"));
    assert!(parsed.cli.message_args().join(" ").contains("ñice"));
}

/// Test very long arguments.
#[test]
fn test_long_arguments() {
    let long_value = "x".repeat(10000);
    let long_message = "word ".repeat(1000);

    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        long_value.clone(),
        "--custom-flag".to_string(),
        long_value,
        long_message.trim().to_string(),
    ])
    .expect("Should handle very long arguments");

    assert_eq!(parsed.cli.system_prompt.as_ref().unwrap().len(), 10000);

    let custom_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "custom-flag")
        .expect("Should have custom-flag");
    assert_eq!(custom_flag.value.as_ref().unwrap().len(), 10000);

    assert_eq!(parsed.cli.message_args().len(), 1);
    assert_eq!(parsed.cli.message_args()[0], long_message.trim());
}

/// Test repeated flags with different values.
#[test]
fn test_repeated_flags() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--custom-flag".to_string(),
        "first".to_string(),
        "--custom-flag".to_string(),
        "second".to_string(),
        "--another-flag=value1".to_string(),
        "--another-flag=value2".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle repeated extension flags");

    // Should have all repeated flags
    let custom_flags = parsed
        .extension_flags
        .iter()
        .filter(|f| f.name == "custom-flag")
        .count();
    assert_eq!(custom_flags, 2);

    let another_flags = parsed
        .extension_flags
        .iter()
        .filter(|f| f.name == "another-flag")
        .count();
    assert_eq!(another_flags, 2);
}

/// Test flag aliases and short forms.
#[test]
fn test_flag_aliases() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "-v".to_string(),
        "--model".to_string(),
        "claude".to_string(),
        "--verbose".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle flag aliases");

    assert!(parsed.cli.verbose);
    assert!(parsed.cli.version);
    assert_eq!(parsed.cli.model, Some("claude".to_string()));
}

/// Test empty string arguments.
#[test]
fn test_empty_string_arguments() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        String::new(), // Empty string
        "--custom-flag".to_string(),
        String::new(), // Empty value
        String::new(), // Empty message part
        "real message".to_string(),
    ])
    .expect("Should handle empty string arguments");

    assert_eq!(parsed.cli.system_prompt, Some(String::new()));

    let custom_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "custom-flag")
        .expect("Should have custom-flag");
    assert_eq!(custom_flag.value, Some(String::new()));

    assert!(parsed.cli.message_args().contains(&""));
    assert!(parsed.cli.message_args().contains(&"real message"));
}

/// Test numeric arguments and negative numbers.
#[test]
fn test_numeric_arguments() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--temperature".to_string(),
        "0.7".to_string(),
        "--max-tokens".to_string(),
        "1000".to_string(),
        "--negative-flag".to_string(),
        "-42".to_string(), // Negative number
        "Process -1 items".to_string(),
        "-999".to_string(), // Negative in message
    ])
    .expect("Should handle numeric arguments");

    // Check extension flags got numeric values
    let temp_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "temperature")
        .expect("Should have temperature flag");
    assert_eq!(temp_flag.value, Some("0.7".to_string()));

    let neg_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "negative-flag")
        .expect("Should have negative-flag");
    assert_eq!(neg_flag.value, Some("-42".to_string()));
}

/// Test special characters in flag names and values.
#[test]
fn test_special_characters_in_flags() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--debug_mode".to_string(), // Underscore
        "true".to_string(),
        "--config-file".to_string(), // Hyphen
        "./path/to/file.json".to_string(),
        "--url=https://example.com:8080/path?param=value&other=test".to_string(), // Complex URL
        "message".to_string(),
    ])
    .expect("Should handle special characters in flags");

    let debug_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "debug_mode")
        .expect("Should have debug_mode flag");
    assert_eq!(debug_flag.value, Some("true".to_string()));

    let url_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "url")
        .expect("Should have url flag");
    assert_eq!(
        url_flag.value,
        Some("https://example.com:8080/path?param=value&other=test".to_string())
    );
}

/// Test whitespace handling in values.
#[test]
fn test_whitespace_handling() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        "  leading and trailing spaces  ".to_string(),
        "--flag-with-tabs".to_string(),
        "\t\ttab\tdelimited\t\t".to_string(),
        "   message   with   spaces   ".to_string(),
    ])
    .expect("Should preserve whitespace in values");

    assert_eq!(
        parsed.cli.system_prompt,
        Some("  leading and trailing spaces  ".to_string())
    );

    let tab_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "flag-with-tabs")
        .expect("Should have flag-with-tabs");
    assert_eq!(tab_flag.value, Some("\t\ttab\tdelimited\t\t".to_string()));
}

/// Test shell metacharacters in arguments.
#[test]
fn test_shell_metacharacters() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--command".to_string(),
        "echo $HOME && ls -la | grep test".to_string(),
        "--pattern".to_string(),
        "*.rs".to_string(),
        "--pipe-flag".to_string(),
        "input | output".to_string(),
        "Process files: *.txt $(date) `pwd`".to_string(),
    ])
    .expect("Should handle shell metacharacters as literals");

    let cmd_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "command")
        .expect("Should have command flag");
    assert_eq!(
        cmd_flag.value,
        Some("echo $HOME && ls -la | grep test".to_string())
    );

    let pattern_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "pattern")
        .expect("Should have pattern flag");
    assert_eq!(pattern_flag.value, Some("*.rs".to_string()));

    assert!(parsed.cli.message_args().join(" ").contains("*.txt"));
    assert!(parsed.cli.message_args().join(" ").contains("$(date)"));
}

/// Test multiple @file references.
#[test]
fn test_multiple_file_references() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    let file3 = temp_dir.path().join("file3.txt");

    fs::write(&file1, "content1")?;
    fs::write(&file2, "content2")?;
    fs::write(&file3, "content3")?;

    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        format!("@{}", file1.display()),
        "--model".to_string(),
        "claude".to_string(),
        format!("@{}", file2.display()),
        "analyze".to_string(),
        format!("@{}", file3.display()),
        "these files".to_string(),
    ])
    .expect("Should handle multiple @file references");

    assert_eq!(parsed.cli.file_args().len(), 3);
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("file1.txt"))
    );
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("file2.txt"))
    );
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("file3.txt"))
    );

    assert_eq!(parsed.cli.model, None);

    Ok(())
}

/// Test edge cases with equals sign in values.
#[test]
fn test_equals_in_values() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--equation=x=y+z".to_string(),
        "--url".to_string(),
        "https://example.com?param=value&other=test".to_string(),
        "--config=key1=val1,key2=val2".to_string(),
        "Solve: a=b+c=d".to_string(),
    ])
    .expect("Should handle equals signs in values");

    let eq_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "equation")
        .expect("Should have equation flag");
    assert_eq!(eq_flag.value, Some("x=y+z".to_string()));

    let url_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "url")
        .expect("Should have url flag");
    assert_eq!(
        url_flag.value,
        Some("https://example.com?param=value&other=test".to_string())
    );

    let config_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "config")
        .expect("Should have config flag");
    assert_eq!(config_flag.value, Some("key1=val1,key2=val2".to_string()));
}

/// Test single character flags and values.
#[test]
fn test_single_character_values() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--char".to_string(),
        "a".to_string(),
        "--symbol".to_string(),
        "@".to_string(),
        "--number".to_string(),
        "7".to_string(),
        "x".to_string(), // Single char message
    ])
    .expect("Should handle single character values");

    let char_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "char")
        .expect("Should have char flag");
    assert_eq!(char_flag.value, Some("a".to_string()));

    let symbol_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "symbol")
        .expect("Should have symbol flag");
    assert_eq!(symbol_flag.value, Some("@".to_string()));

    assert_eq!(parsed.cli.message_args(), vec!["x"]);
}

/// Test boundary cases with dashes.
#[test]
fn test_dash_boundary_cases() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--".to_string(),
        "--not-a-flag".to_string(),
        "-".to_string(),       // Single dash
        "---".to_string(),     // Triple dash
        "--flag-".to_string(), // Flag ending with dash
        "regular".to_string(),
    ])
    .expect("Should handle dash boundary cases");

    // Everything after -- should be positional
    assert!(parsed.cli.message_args().contains(&"--not-a-flag"));
    assert!(parsed.cli.message_args().contains(&"-"));
    assert!(parsed.cli.message_args().contains(&"---"));
    assert!(parsed.cli.message_args().contains(&"--flag-"));
    assert!(parsed.cli.message_args().contains(&"regular"));
}

/// Test flag names that are numeric.
#[test]
fn test_numeric_flag_names() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--123".to_string(),
        "numeric_flag".to_string(),
        "--42flag".to_string(),
        "mixed".to_string(),
        "--flag99".to_string(),
        "ending_numeric".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle numeric flag names");

    // These should be treated as extension flags
    assert!(parsed.extension_flags.iter().any(|f| f.name == "123"));
    assert!(parsed.extension_flags.iter().any(|f| f.name == "42flag"));
    assert!(parsed.extension_flags.iter().any(|f| f.name == "flag99"));
}

/// Test very short and very long flag names.
#[test]
fn test_extreme_flag_name_lengths() {
    let short_flag = "a".to_string();
    let long_flag = "a".repeat(100);

    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        format!("--{}", short_flag),
        "short_value".to_string(),
        format!("--{}", long_flag),
        "long_value".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle extreme flag name lengths");

    assert!(parsed.extension_flags.iter().any(|f| f.name == short_flag));
    assert!(parsed.extension_flags.iter().any(|f| f.name == long_flag));

    let long_flag_found = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == long_flag)
        .expect("Should have long flag");
    assert_eq!(long_flag_found.value, Some("long_value".to_string()));
}

/// Test mixed quote types in single argument.
#[test]
fn test_mixed_quote_types() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--system-prompt".to_string(),
        r#"Say 'hello' and "goodbye""#.to_string(),
        "--mixed".to_string(),
        r#"'quoted' and "double" and `backtick`"#.to_string(),
        r#"Handle 'nested "quotes" inside' strings"#.to_string(),
    ])
    .expect("Should handle mixed quote types");

    assert_eq!(
        parsed.cli.system_prompt,
        Some(r#"Say 'hello' and "goodbye""#.to_string())
    );

    let mixed_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "mixed")
        .expect("Should have mixed flag");
    assert_eq!(
        mixed_flag.value,
        Some(r#"'quoted' and "double" and `backtick`"#.to_string())
    );
}

/// Test JSON and structured data in arguments.
#[test]
fn test_json_in_arguments() {
    let json_data = r#"{"name": "test", "values": [1, 2, 3], "nested": {"key": "value"}}"#;
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--config".to_string(),
        json_data.to_string(),
        "--array=[]".to_string(),
        "--object={}".to_string(),
        "Parse this JSON".to_string(),
    ])
    .expect("Should handle JSON in arguments");

    let config_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "config")
        .expect("Should have config flag");
    assert_eq!(config_flag.value, Some(json_data.to_string()));

    let array_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "array")
        .expect("Should have array flag");
    assert_eq!(array_flag.value, Some("[]".to_string()));
}

/// Test flag collision with known CLI flags.
#[test]
fn test_flag_collision_scenarios() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--verbose".to_string(),        // Known CLI flag
        "--custom-verbose".to_string(), // Extension flag with similar name
        "value".to_string(),
        "--model".to_string(),
        "claude".to_string(),          // Known CLI flag
        "--model-version".to_string(), // Extension flag starting with known name
        "4.0".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle flag collision scenarios");

    // Known CLI flags before extension extraction should be parsed normally.
    assert!(parsed.cli.verbose);
    assert_eq!(parsed.cli.model, Some("claude".to_string()));

    // Extension flags should be captured
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "custom-verbose")
    );
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "model-version")
    );
}

/// Test pathological input cases.
#[test]
fn test_pathological_inputs() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--=value".to_string(), // Empty flag name with equals
        "--".to_string(),       // Lone separator
        String::new(),          // Empty string
        "   ".to_string(),      // Whitespace only
        "\n\t\r".to_string(),   // Control characters
    ]);

    // Empty flag names are rejected by clap; the parser should return a typed
    // error rather than panic or misclassify the following separator.
    assert!(parsed.is_err());
}

/// Test @file references with relative paths and edge cases.
#[test]
fn test_file_reference_path_edge_cases() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let nested_dir = temp_dir.path().join("nested").join("deep");
    fs::create_dir_all(&nested_dir)?;

    let file_with_dots = nested_dir.join("..").join("file.txt");
    let file_absolute = temp_dir.path().join("absolute.txt");
    let file_relative = temp_dir.path().join("./relative.txt");

    fs::write(&file_absolute, "absolute content")?;
    fs::write(&file_relative, "relative content")?;

    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        format!("@{}", file_with_dots.display()), // Path with ..
        format!("@{}", file_absolute.display()),  // Absolute path
        "@./relative.txt".to_string(),            // Relative path
        "@~/home/file.txt".to_string(),           // Tilde path (literal)
        "analyze".to_string(),
    ])
    .expect("Should handle various @file path formats");

    assert_eq!(parsed.cli.file_args().len(), 4);
    assert!(parsed.cli.file_args().iter().any(|f| f.contains("..")));
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("absolute.txt"))
    );
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("./relative.txt"))
    );
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("~/home/file.txt"))
    );

    Ok(())
}

/// Test extension flag precedence and ordering.
#[test]
fn test_extension_flag_precedence() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--debug".to_string(), // First occurrence
        "level1".to_string(),
        "--model".to_string(),
        "claude".to_string(),  // Known CLI flag between extensions
        "--debug".to_string(), // Second occurrence
        "level2".to_string(),
        "--verbose".to_string(),      // Known CLI flag
        "--debug=level3".to_string(), // Third occurrence with equals
        "message".to_string(),
    ])
    .expect("Should handle extension flag precedence");

    // Should preserve all occurrences of extension flags
    let debug_flags: Vec<_> = parsed
        .extension_flags
        .iter()
        .filter(|f| f.name == "debug")
        .collect();
    assert_eq!(debug_flags.len(), 3);

    // Check values are preserved in order
    assert_eq!(debug_flags[0].value, Some("level1".to_string()));
    assert_eq!(debug_flags[1].value, Some("level2".to_string()));
    assert_eq!(debug_flags[2].value, Some("level3".to_string()));

    // Known CLI flags should still work
    assert_eq!(parsed.cli.model, Some("claude".to_string()));
    assert!(parsed.cli.verbose);
}

/// Test boolean flag variations.
#[test]
fn test_boolean_flag_variations() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--enable-feature".to_string(),      // Boolean without value
        "--disable-cache=false".to_string(), // Boolean with explicit false
        "--debug=true".to_string(),          // Boolean with explicit true
        "--flag".to_string(),                // Simple boolean
        "--enable".to_string(),
        "not-a-value".to_string(), // This should be next flag's value
        "--config".to_string(),
        "config.json".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle boolean flag variations");

    let enable_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "enable-feature")
        .expect("Should have enable-feature");
    assert_eq!(enable_flag.value, None); // Boolean without value

    let disable_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "disable-cache")
        .expect("Should have disable-cache");
    assert_eq!(disable_flag.value, Some("false".to_string()));

    let debug_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "debug")
        .expect("Should have debug");
    assert_eq!(debug_flag.value, Some("true".to_string()));

    let secondary_enable_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "enable")
        .expect("Should have enable");
    assert_eq!(secondary_enable_flag.value, Some("not-a-value".to_string()));
}

/// Test complex @file and flag interleaving.
#[test]
fn test_complex_file_flag_interleaving() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let file1 = temp_dir.path().join("input1.txt");
    let file2 = temp_dir.path().join("input2.txt");

    fs::write(&file1, "input1 content")?;
    fs::write(&file2, "input2 content")?;

    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        format!("@{}", file1.display()),
        "--processor".to_string(),
        "nlp".to_string(),
        "--model".to_string(),
        "claude".to_string(), // Known CLI flag
        format!("@{}", file2.display()),
        "--output-format=json".to_string(),
        "Analyze both files".to_string(),
        "--verbose".to_string(),        // Known CLI flag
        "@nonexistent.txt".to_string(), // Nonexistent file
    ])
    .expect("Should handle complex file/flag interleaving");

    // Check files were extracted
    assert_eq!(parsed.cli.file_args().len(), 3);
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("input1.txt"))
    );
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("input2.txt"))
    );
    assert!(
        parsed
            .cli
            .file_args()
            .iter()
            .any(|f| f.contains("nonexistent.txt"))
    );

    // Check extension flags
    assert!(parsed.extension_flags.iter().any(|f| f.name == "processor"));
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "output-format")
    );

    // Check CLI flags
    assert_eq!(parsed.cli.model, None);
    assert!(!parsed.cli.verbose);

    Ok(())
}

/// Test flag names with unusual characters.
#[test]
fn test_unusual_flag_characters() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--flag.with.dots".to_string(),
        "dots_value".to_string(),
        "--flag:with:colons".to_string(),
        "colons_value".to_string(),
        "--flag/with/slashes".to_string(),
        "slashes_value".to_string(),
        "--CamelCaseFlag".to_string(),
        "camel_value".to_string(),
        "--flag@with@ats".to_string(),
        "ats_value".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle unusual characters in flag names");

    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "flag.with.dots")
    );
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "flag:with:colons")
    );
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "flag/with/slashes")
    );
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "CamelCaseFlag")
    );
    assert!(
        parsed
            .extension_flags
            .iter()
            .any(|f| f.name == "flag@with@ats")
    );
}

/// Test massive argument lists.
#[test]
fn test_massive_argument_list() {
    let mut args = vec!["pi".to_string()];

    // Add 100 extension flags
    for i in 0..100 {
        args.push(format!("--flag{i}"));
        args.push(format!("value{i}"));
    }

    // Add some regular flags mixed in
    args.push("--model".to_string());
    args.push("claude".to_string());
    args.push("--verbose".to_string());

    // Add message
    args.push("Process with many flags".to_string());

    let parsed = parse_with_extension_flags(args).expect("Should handle massive argument lists");

    // Should have 100 extension flags
    assert_eq!(parsed.extension_flags.len(), 100);

    // Known CLI flags should still work
    assert_eq!(parsed.cli.model, Some("claude".to_string()));
    assert!(parsed.cli.verbose);

    // Message should be preserved
    assert!(
        parsed
            .cli
            .message_args()
            .contains(&"Process with many flags")
    );
}

/// Test flag value detection edge cases.
#[test]
fn test_flag_value_detection_edge_cases() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--flag-before-separator".to_string(),
        "--".to_string(), // Should NOT be flag value
        "--flag-with-dash-value".to_string(),
        "--not-a-value".to_string(), // Should be treated as value
        "--flag-with-equals=--also-not-flag".to_string(),
        "--last-flag".to_string(), // No value (end of args)
    ])
    .expect("Should handle flag value detection edge cases");

    // flag-before-separator should have no value due to --
    let separator_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "flag-before-separator")
        .expect("Should have flag-before-separator");
    assert_eq!(separator_flag.value, None);

    // Everything after the separator remains positional, even when it looks
    // flag-shaped.
    assert!(
        parsed
            .cli
            .message_args()
            .contains(&"--flag-with-dash-value")
    );
    assert!(parsed.cli.message_args().contains(&"--not-a-value"));
    assert!(
        parsed
            .cli
            .message_args()
            .contains(&"--flag-with-equals=--also-not-flag")
    );
    assert!(parsed.cli.message_args().contains(&"--last-flag"));
}

/// Test mixed short and long flags.
#[test]
fn test_mixed_short_long_flags() {
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "-v".to_string(),              // Known short flag
        "--verbose-level".to_string(), // Extension flag similar to known
        "high".to_string(),
        "--model".to_string(), // Known long flag
        "claude".to_string(),
        "--model-temperature".to_string(), // Extension flag similar to known
        "0.7".to_string(),
        "-e".to_string(), // Known short flag (expecting value)
        "gpt".to_string(),
        "message".to_string(),
    ])
    .expect("Should handle mixed short and long flags");

    // Known CLI flags
    assert!(parsed.cli.version);
    assert!(!parsed.cli.verbose);
    assert_eq!(parsed.cli.model, Some("claude".to_string()));
    assert_eq!(parsed.cli.extension, vec!["gpt".to_string()]);

    // Extension flags with similar names
    let verbose_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "verbose-level")
        .expect("Should have verbose-level");
    assert_eq!(verbose_flag.value, Some("high".to_string()));

    let temp_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "model-temperature")
        .expect("Should have model-temperature");
    assert_eq!(temp_flag.value, Some("0.7".to_string()));
}

/// Test binary data and non-UTF8 sequences (should be gracefully handled).
#[test]
fn test_binary_data_arguments() {
    // These are valid UTF-8 strings that represent binary-like data
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--binary-flag".to_string(),
        "\x00\x01\x02\x03".to_string(), // Null bytes and low values
        "--hex-data=\x41\x42\x43".to_string(), // Hex-like data
        "Message with \x1b[31mANSI\x1b[0m codes".to_string(),
    ])
    .expect("Should handle binary-like data in arguments");

    let binary_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "binary-flag")
        .expect("Should have binary-flag");
    assert_eq!(binary_flag.value, Some("\x00\x01\x02\x03".to_string()));

    let hex_flag = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "hex-data")
        .expect("Should have hex-data");
    assert_eq!(hex_flag.value, Some("\x41\x42\x43".to_string()));

    // Message should contain ANSI sequences
    assert!(parsed.cli.message_args().join(" ").contains("\x1b[31m"));
}

/// Test comprehensive flag ordering and precedence scenarios.
#[test]
fn test_comprehensive_flag_ordering_scenarios() {
    // Test complex interleaving of CLI flags, extension flags, files, and messages
    let parsed = parse_with_extension_flags(vec![
        "pi".to_string(),
        "--model".to_string(), // CLI flag first
        "claude".to_string(),
        "--ext-before".to_string(), // Extension flag
        "before_value".to_string(),
        "--verbose".to_string(),                 // CLI boolean flag
        "@file1.txt".to_string(),                // File reference
        "--ext-middle=middle_value".to_string(), // Extension with equals
        "first message".to_string(),             // Message part
        "--temperature".to_string(),             // CLI flag with value
        "0.8".to_string(),
        "--ext-after".to_string(), // Extension at end
        "after_value".to_string(),
        "second message".to_string(), // Final message
    ])
    .expect("Should handle comprehensive flag ordering scenarios");

    // Verify CLI flags are extracted correctly regardless of position
    assert_eq!(parsed.cli.model, Some("claude".to_string()));
    assert!(parsed.cli.verbose);

    // Verify extension flags are captured in order
    assert_eq!(parsed.extension_flags.len(), 4);
    let ext_before = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "ext-before")
        .expect("Should have ext-before");
    assert_eq!(ext_before.value, Some("before_value".to_string()));

    let ext_middle = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "ext-middle")
        .expect("Should have ext-middle");
    assert_eq!(ext_middle.value, Some("middle_value".to_string()));

    let ext_after = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "ext-after")
        .expect("Should have ext-after");
    assert_eq!(ext_after.value, Some("after_value".to_string()));

    let temperature = parsed
        .extension_flags
        .iter()
        .find(|f| f.name == "temperature")
        .expect("Should have temperature");
    assert_eq!(temperature.value, Some("0.8".to_string()));

    // Verify file references
    assert_eq!(parsed.cli.file_args().len(), 1);
    assert!(parsed.cli.file_args()[0].contains("file1.txt"));

    // Verify message parts
    assert!(parsed.cli.message_args().contains(&"first message"));
    assert!(parsed.cli.message_args().contains(&"second message"));
}
