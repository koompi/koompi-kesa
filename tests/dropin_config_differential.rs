//! Differential evidence for G07 config/environment parity against pi-mono
//! settings semantics.

use kode::config::Config;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;

fn write_settings(path: &Path, value: &serde_json::Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create settings parent");
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&value).expect("serialize settings"),
    )
    .expect("write settings");
}

fn load_from_values(global: &serde_json::Value, project: &serde_json::Value) -> Config {
    let temp = TempDir::new().expect("create tempdir");
    let global_dir = temp.path().join("global");
    let cwd = temp.path().join("workspace");
    write_settings(&global_dir.join("settings.json"), global);
    write_settings(&cwd.join(".pi/settings.json"), project);
    Config::load_with_roots(None, &global_dir, &cwd).expect("load merged config")
}

#[test]
#[allow(clippy::too_many_lines)]
fn g07_legacy_settings_schema_fields_are_honored_with_project_precedence() {
    let config = load_from_values(
        &json!({
            "lastChangelogVersion": "global-version",
            "defaultProvider": "anthropic",
            "defaultModel": "claude-global",
            "defaultThinkingLevel": "low",
            "steeringMode": "all",
            "followUpMode": "all",
            "theme": "global-theme",
            "compaction": {
                "enabled": true,
                "reserveTokens": 111,
                "keepRecentTokens": 222
            },
            "branchSummary": {
                "reserveTokens": 333
            },
            "retry": {
                "enabled": true,
                "maxRetries": 4,
                "baseDelayMs": 500,
                "maxDelayMs": 9000
            },
            "hideThinkingBlock": false,
            "shellPath": "/bin/bash",
            "quietStartup": false,
            "shellCommandPrefix": "set -e",
            "collapseChangelog": false,
            "packages": ["global-package"],
            "extensions": ["global-extension"],
            "skills": ["global-skill"],
            "prompts": ["global-prompt"],
            "themes": ["global-theme-package"],
            "enableSkillCommands": true,
            "terminal": {
                "showImages": true,
                "clearOnShrink": false
            },
            "images": {
                "autoResize": true,
                "blockImages": false
            },
            "enabledModels": ["global-model-*"],
            "doubleEscapeAction": "tree",
            "thinkingBudgets": {
                "minimal": 101,
                "low": 202,
                "medium": 303,
                "high": 404
            },
            "editorPaddingX": 1,
            "autocompleteMaxVisible": 6,
            "showHardwareCursor": false,
            "markdown": {
                "codeBlockIndent": "  "
            }
        }),
        &json!({
            "lastChangelogVersion": "project-version",
            "defaultProvider": "openai",
            "defaultModel": "gpt-project",
            "defaultThinkingLevel": "xhigh",
            "steeringMode": "one-at-a-time",
            "followUpMode": "one-at-a-time",
            "theme": "project-theme",
            "compaction": {
                "enabled": false,
                "reserveTokens": 444,
                "keepRecentTokens": 555
            },
            "branchSummary": {
                "reserveTokens": 666
            },
            "retry": {
                "enabled": false,
                "maxRetries": 7,
                "baseDelayMs": 800,
                "maxDelayMs": 900
            },
            "hideThinkingBlock": true,
            "shellPath": "/bin/zsh",
            "quietStartup": true,
            "shellCommandPrefix": "set -eux",
            "collapseChangelog": true,
            "packages": ["project-package"],
            "extensions": ["project-extension"],
            "skills": ["project-skill"],
            "prompts": ["project-prompt"],
            "themes": ["project-theme-package"],
            "enableSkillCommands": false,
            "terminal": {
                "showImages": false,
                "clearOnShrink": true
            },
            "images": {
                "autoResize": false,
                "blockImages": true
            },
            "enabledModels": ["project-model-*"],
            "doubleEscapeAction": "cancel",
            "thinkingBudgets": {
                "minimal": 1001,
                "low": 2002,
                "medium": 3003,
                "high": 4004,
                "xhigh": 5005
            },
            "editorPaddingX": 2,
            "autocompleteMaxVisible": 9,
            "showHardwareCursor": true,
            "markdown": {
                "codeBlockIndent": "    "
            }
        }),
    );

    let mut scenarios = 0_u32;
    macro_rules! scenario {
        ($assertion:expr) => {{
            assert!($assertion);
            scenarios += 1;
        }};
    }

    scenario!(config.last_changelog_version.as_deref() == Some("project-version"));
    scenario!(config.default_provider.as_deref() == Some("openai"));
    scenario!(config.default_model.as_deref() == Some("gpt-project"));
    scenario!(config.default_thinking_level.as_deref() == Some("xhigh"));
    scenario!(config.steering_mode.as_deref() == Some("one-at-a-time"));
    scenario!(config.follow_up_mode.as_deref() == Some("one-at-a-time"));
    scenario!(config.theme.as_deref() == Some("project-theme"));
    scenario!(!config.compaction_enabled());
    scenario!(config.compaction_reserve_tokens() == 444);
    scenario!(config.compaction_keep_recent_tokens() == 555);
    scenario!(config.branch_summary_reserve_tokens() == 666);
    scenario!(!config.retry_enabled());
    scenario!(config.retry_max_retries() == 7);
    scenario!(config.retry_base_delay_ms() == 800);
    scenario!(config.retry_max_delay_ms() == 900);
    scenario!(config.hide_thinking_block == Some(true));
    scenario!(config.shell_path.as_deref() == Some("/bin/zsh"));
    scenario!(config.quiet_startup == Some(true));
    scenario!(config.shell_command_prefix.as_deref() == Some("set -eux"));
    scenario!(config.collapse_changelog == Some(true));
    scenario!(
        config
            .packages
            .as_ref()
            .is_some_and(|packages| packages.len() == 1)
    );
    scenario!(config.extensions.as_deref() == Some(["project-extension".to_string()].as_slice()));
    scenario!(config.skills.as_deref() == Some(["project-skill".to_string()].as_slice()));
    scenario!(config.prompts.as_deref() == Some(["project-prompt".to_string()].as_slice()));
    scenario!(config.themes.as_deref() == Some(["project-theme-package".to_string()].as_slice()));
    scenario!(!config.enable_skill_commands());
    scenario!(!config.terminal_show_images());
    scenario!(config.terminal_clear_on_shrink());
    scenario!(!config.image_auto_resize());
    scenario!(config.image_block_images());
    scenario!(config.enabled_models.as_deref() == Some(["project-model-*".to_string()].as_slice()));
    scenario!(config.double_escape_action.as_deref() == Some("cancel"));
    scenario!(config.thinking_budget("minimal") == 1001);
    scenario!(config.thinking_budget("low") == 2002);
    scenario!(config.thinking_budget("medium") == 3003);
    scenario!(config.thinking_budget("high") == 4004);
    scenario!(config.thinking_budget("xhigh") == 5005);
    scenario!(config.editor_padding_x == Some(2));
    scenario!(config.autocomplete_max_visible == Some(9));
    scenario!(config.show_hardware_cursor == Some(true));
    scenario!(config.markdown_code_block_indent() == 4);

    assert!(
        scenarios >= 20,
        "G07 differential evidence must cover at least 20 config scenarios; saw {scenarios}"
    );
}

#[test]
fn g07_config_path_override_skips_global_and_project_merge() {
    let temp = TempDir::new().expect("create tempdir");
    let global_dir = temp.path().join("global");
    let cwd = temp.path().join("workspace");
    let override_path = temp.path().join("override-settings.json");

    write_settings(
        &global_dir.join("settings.json"),
        &json!({ "theme": "global", "defaultProvider": "anthropic" }),
    );
    write_settings(
        &cwd.join(".pi/settings.json"),
        &json!({ "theme": "project", "defaultProvider": "gemini" }),
    );
    write_settings(
        &override_path,
        &json!({ "theme": "override", "defaultProvider": "openai" }),
    );

    let config =
        Config::load_with_roots(Some(&override_path), &global_dir, &cwd).expect("load override");
    assert_eq!(config.theme.as_deref(), Some("override"));
    assert_eq!(config.default_provider.as_deref(), Some("openai"));
}

#[test]
fn g07_legacy_markdown_indent_string_and_xhigh_defaults_match_documented_parity() {
    let string_indent: Config =
        serde_json::from_value(json!({ "markdown": { "codeBlockIndent": "   " } }))
            .expect("parse legacy string indent");
    assert_eq!(string_indent.markdown_code_block_indent(), 3);

    let numeric_indent: Config =
        serde_json::from_value(json!({ "markdown": { "codeBlockIndent": 5 } }))
            .expect("parse numeric indent");
    assert_eq!(numeric_indent.markdown_code_block_indent(), 5);

    let defaults = Config::default();
    assert_eq!(defaults.markdown_code_block_indent(), 2);
    assert_eq!(defaults.thinking_budget("xhigh"), 32768);
}
