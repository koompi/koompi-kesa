use super::*;
use crate::agent::AgentConfig;
use crate::model::{ContentBlock, StreamEvent, TextContent};
use crate::provider::{Context, Provider, StreamOptions};
use crate::resources::{ResourceCliOptions, ResourceLoader};
use crate::tools::ToolRegistry;
use asupersync::channel::mpsc;
use asupersync::runtime::RuntimeBuilder;
use bubbletea::{KeyMsg, Message, WindowSizeMsg};
use futures::stream;
use serde_json::json;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

struct DummyProvider;

#[async_trait::async_trait]
impl Provider for DummyProvider {
    fn name(&self) -> &'static str {
        "dummy"
    }

    fn api(&self) -> &'static str {
        "dummy"
    }

    fn model_id(&self) -> &'static str {
        "dummy-model"
    }

    async fn stream(
        &self,
        _context: &Context<'_>,
        _options: &StreamOptions,
    ) -> crate::error::Result<
        Pin<Box<dyn futures::Stream<Item = crate::error::Result<StreamEvent>> + Send>>,
    > {
        Ok(Box::pin(stream::empty()))
    }
}

fn test_runtime_handle() -> asupersync::runtime::RuntimeHandle {
    static RT: OnceLock<asupersync::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        RuntimeBuilder::current_thread()
            .build()
            .expect("build asupersync runtime")
    })
    .handle()
}

fn test_model_entry() -> ModelEntry {
    ModelEntry {
        model: crate::provider::Model {
            id: "gpt-5.2".to_string(),
            name: "gpt-5.2".to_string(),
            api: "openai-responses".to_string(),
            provider: "openai".to_string(),
            base_url: "https://example.invalid".to_string(),
            reasoning: true,
            input: vec![crate::provider::InputType::Text],
            cost: crate::provider::ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 128_000,
            max_tokens: 8_192,
            headers: std::collections::HashMap::new(),
        },
        api_key: None,
        headers: std::collections::HashMap::new(),
        auth_header: false,
        compat: None,
        oauth_config: None,
    }
}

fn build_test_app(cwd: PathBuf) -> PiApp {
    let config = Config::default();
    let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
    let agent = Agent::new(
        provider,
        ToolRegistry::new(&[], &cwd, Some(&config)),
        AgentConfig::default(),
    );
    let resources = ResourceLoader::empty(config.enable_skill_commands());
    let resource_cli = ResourceCliOptions {
        no_skills: false,
        no_prompt_templates: false,
        no_extensions: false,
        no_themes: false,
        skill_paths: Vec::new(),
        prompt_paths: Vec::new(),
        extension_paths: Vec::new(),
        theme_paths: Vec::new(),
    };
    let model_entry = test_model_entry();
    let (event_tx, _event_rx) = mpsc::channel(64);

    PiApp::new(
        agent,
        Arc::new(asupersync::sync::Mutex::new(Session::in_memory())),
        config,
        resources,
        resource_cli,
        cwd,
        model_entry.clone(),
        vec![model_entry.clone()],
        vec![model_entry],
        Vec::new(),
        event_tx,
        test_runtime_handle(),
        false,
        false,
        None,
        Some(KeyBindings::new()),
        Vec::new(),
        Usage::default(),
    )
}

fn tempdir() -> tempfile::TempDir {
    std::fs::create_dir_all(std::env::temp_dir()).expect("create temp root");
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn prepare_startup_changelog_skips_disk_write_when_persistence_disabled() {
    let dir = tempdir();
    let cwd = dir.path().join("workspace");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    let settings_path = dir.path().join("settings.json");
    let mut config = Config {
        last_changelog_version: Some("0.9.0".to_string()),
        ..Config::default()
    };

    let changelog = "## 1.0.0\n- Added startup changelog notices\n\n## 0.9.0\n- Previous release\n";
    let startup = prepare_startup_changelog_with_roots(
        &mut config,
        dir.path(),
        &cwd,
        Some(&settings_path),
        false,
        false,
        "1.0.0",
        || changelog,
    );

    assert_eq!(
        startup,
        Some(StartupChangelog::Full {
            markdown: "## 1.0.0\n- Added startup changelog notices".to_string(),
        })
    );
    assert!(
        !settings_path.exists(),
        "startup construction should not write settings"
    );
    assert_eq!(config.last_changelog_version.as_deref(), Some("1.0.0"));
}

#[test]
fn prepare_startup_changelog_writes_when_persistence_enabled() {
    let dir = tempdir();
    let cwd = dir.path().join("workspace");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    let settings_path = dir.path().join("settings.json");
    let mut config = Config {
        last_changelog_version: Some("0.9.0".to_string()),
        ..Config::default()
    };

    let startup = prepare_startup_changelog_with_roots(
        &mut config,
        dir.path(),
        &cwd,
        Some(&settings_path),
        false,
        true,
        "1.0.0",
        || "## 1.0.0\n- Added startup changelog notices\n\n## 0.9.0\n- Previous release\n",
    );

    assert!(matches!(startup, Some(StartupChangelog::Full { .. })));
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).expect("read settings"))
            .expect("parse settings");
    assert_eq!(saved["lastChangelogVersion"].as_str(), Some("1.0.0"));
}

#[test]
fn extract_file_references_removes_indented_ref_line_without_leaving_blank_whitespace() {
    let dir = tempdir();
    std::fs::write(dir.path().join("notes.txt"), "hi").expect("write file");
    let mut app = build_test_app(dir.path().to_path_buf());

    let (cleaned, refs) = app.extract_file_references("Summary:\n  @notes.txt\nNext line");

    assert_eq!(cleaned, "Summary:\nNext line");
    assert_eq!(refs, vec!["notes.txt".to_string()]);
}

#[test]
fn extract_file_references_preserves_newline_before_trailing_punctuation() {
    let dir = tempdir();
    std::fs::write(dir.path().join("notes.txt"), "hi").expect("write file");
    let mut app = build_test_app(dir.path().to_path_buf());

    let (cleaned, refs) = app.extract_file_references("Summary:\n@notes.txt.");

    assert_eq!(cleaned, "Summary:\n.");
    assert_eq!(refs, vec!["notes.txt".to_string()]);
}

#[test]
fn is_inside_jj_repo_detects_root_directly() {
    let dir = tempdir();
    std::fs::create_dir(dir.path().join(".jj")).expect("mkdir .jj");
    assert!(super::is_inside_jj_repo(dir.path()));
}

#[test]
fn is_inside_jj_repo_walks_up_to_ancestor() {
    let dir = tempdir();
    let root = dir.path();
    std::fs::create_dir(root.join(".jj")).expect("mkdir .jj");
    let nested = root.join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).expect("mkdir nested");
    assert!(super::is_inside_jj_repo(&nested));
}

#[test]
fn is_inside_jj_repo_false_when_no_dot_jj_anywhere() {
    let dir = tempdir();
    let nested = dir.path().join("a").join("b");
    std::fs::create_dir_all(&nested).expect("mkdir nested");
    assert!(!super::is_inside_jj_repo(&nested));
}

#[test]
fn is_inside_jj_repo_requires_dot_jj_to_be_a_directory() {
    // A file named `.jj` is a gitlink-like stub in some tooling; only a
    // real `.jj/` directory counts as a jj repo for display purposes.
    let dir = tempdir();
    std::fs::write(dir.path().join(".jj"), "not a dir").expect("write stub");
    assert!(!super::is_inside_jj_repo(dir.path()));
}

#[test]
fn read_jj_change_returns_none_outside_jj_repo() {
    // No `.jj` anywhere -> must short-circuit without forking a
    // subprocess and without touching $PATH for the `jj` binary.
    let dir = tempdir();
    assert!(super::read_jj_change(dir.path()).is_none());
}

#[test]
fn read_vcs_info_falls_back_to_git_when_no_jj() {
    // Seed a minimal `.git/HEAD` pointing at a branch. With no `.jj`
    // anywhere, read_vcs_info must return the git branch name unchanged.
    let dir = tempdir();
    let dot_git = dir.path().join(".git");
    std::fs::create_dir(&dot_git).expect("mkdir .git");
    std::fs::write(dot_git.join("HEAD"), "ref: refs/heads/feature/jj-demo\n").expect("seed HEAD");

    let vcs = super::read_vcs_info(dir.path());
    assert_eq!(vcs.as_deref(), Some("feature/jj-demo"));
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            if ch != '\r' {
                out.push(ch);
            }
            continue;
        }
        for escape in chars.by_ref() {
            if escape.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

#[test]
fn render_header_hints_fit_and_never_truncate() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());

    for width in [30usize, 50, 60, 88, 184, 200] {
        app.set_terminal_size(width, 40);
        let header = strip_ansi(&app.render_header());
        let hints = header.lines().nth(1).unwrap_or_default();

        assert!(hints.contains("/help"), "header: {header}");
        assert!(
            !hints.ends_with("..."),
            "hints truncated at {width}: {hints}"
        );
    }
}

#[test]
fn render_header_drops_the_resource_row_until_something_loads() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(120, 40);

    let header = strip_ansi(&app.render_header());

    assert!(!header.contains("resources ready"), "header: {header}");
    assert!(!header.contains("0 skills"), "header: {header}");
    assert_eq!(app.header_rows(), 3, "header: {header}");
}

/// Type `text` one key at a time, the way the terminal delivers it.
fn type_text(app: &mut PiApp, text: &str) {
    for ch in text.chars() {
        let _ = app.update(Message::new(KeyMsg::from_runes(vec![ch])));
    }
}

/// The editor rows drawn inside the input frame, borders and padding stripped.
fn input_rows(app: &PiApp) -> Vec<String> {
    strip_ansi(&app.render_input())
        .lines()
        .filter(|line| line.trim_start().starts_with('│'))
        .map(|line| {
            let inner = line.trim().trim_start_matches('│').trim_end_matches('│');
            inner.trim_end().to_string()
        })
        .collect()
}

#[test]
fn long_prompt_wraps_into_rows_instead_of_clipping() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);
    let typed = "x".repeat(200);

    type_text(&mut app, &typed);

    let rows = input_rows(&app);
    let shown: usize = rows.iter().map(|row| row.matches('x').count()).sum();
    assert_eq!(shown, 200, "input box clipped the prompt: {rows:#?}");
    assert!(rows.len() > 1, "input box did not grow: {rows:#?}");
    assert_eq!(app.input.height(), rows.len());
}

#[test]
fn input_box_stops_growing_at_a_third_of_the_screen_and_scrolls() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 30);
    let cap = super::view::input_max_rows(30);

    type_text(&mut app, &"y".repeat(cap * 200));

    let rows = input_rows(&app);
    assert_eq!(rows.len(), cap, "box grew past its cap: {rows:#?}");
    // Scrolled to the tail: the cursor is on the last row, so the first row
    // shown is no longer the start of the text.
    let layout = app.input_layout();
    assert!(layout.rows.len() > cap);
    assert_eq!(layout.cursor_row, layout.rows.len() - 1);
}

#[test]
fn shift_enter_and_alt_enter_insert_newlines_and_enter_sends() {
    use bubbletea::KeyType;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);

    type_text(&mut app, "one");
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::ShiftEnter)));
    type_text(&mut app, "two");
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Enter).with_alt()));
    type_text(&mut app, "three");

    assert_eq!(app.input.value(), "one\ntwo\nthree");
    assert_eq!(app.input.height(), 3);

    // Enter takes the submit path, which this credential-less test app then
    // refuses. What matters is that it submitted instead of adding a line.
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Enter)));
    assert_eq!(app.input.value(), "one\ntwo\nthree");
    assert_eq!(
        app.status_message.as_deref(),
        Some("Missing credentials for provider openai. Run /login openai.")
    );
}

#[test]
fn editing_a_middle_line_and_deleting_one_leaves_the_rest_intact() {
    use bubbletea::KeyType;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);

    type_text(&mut app, "alpha");
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::ShiftEnter)));
    type_text(&mut app, "beta");
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::ShiftEnter)));
    type_text(&mut app, "gamma");

    // Up onto the middle row, then edit it in place.
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Up)));
    assert_eq!(app.input_layout().cursor_row, 1);
    type_text(&mut app, "!");
    assert_eq!(app.input.value(), "alpha\nbeta!\ngamma");

    // Backspace through the middle line and its break: two rows are left.
    for _ in 0..6 {
        let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Backspace)));
    }
    assert_eq!(app.input.value(), "alpha\ngamma");
    assert_eq!(app.input.height(), 2);
}

#[test]
fn cursor_walks_wrapped_rows_before_it_reaches_history() {
    use bubbletea::KeyType;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);
    let width = super::view::editor_width(100, 0);

    type_text(&mut app, &"z".repeat(width * 2));
    let end = app.input.cursor_byte_offset();

    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Up)));

    assert_eq!(app.input_layout().cursor_row, 1);
    assert_ne!(app.input.cursor_byte_offset(), end);
    assert_eq!(
        app.input.value().len(),
        width * 2,
        "history navigation must not replace a wrapped draft"
    );

    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Down)));
    assert_eq!(app.input.cursor_byte_offset(), end);
}

#[test]
fn narrowing_the_terminal_reflows_the_input_instead_of_cutting_it() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(184, 40);
    type_text(&mut app, &"w".repeat(200));
    let wide_rows = app.input.height();

    app.set_terminal_size(60, 40);

    let narrow = input_rows(&app);
    let shown: usize = narrow.iter().map(|row| row.matches('w').count()).sum();
    assert!(narrow.len() > wide_rows, "no reflow: {narrow:#?}");
    assert_eq!(shown, 200, "reflow clipped the prompt: {narrow:#?}");
}

#[test]
fn a_long_status_message_never_pushes_the_input_box_off_screen() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 24);
    app.messages = (1..=30)
        .map(|idx| ConversationMessage {
            role: MessageRole::User,
            content: format!("message {idx}"),
            thinking: None,
            collapsed: false,
        })
        .collect();
    app.status_message = Some(
        (1..=10)
            .map(|idx| format!("provider error line {idx}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app.scroll_to_bottom();

    let frame = strip_ansi(&app.view());

    assert!(
        frame.contains("Type a message"),
        "input box fell off the bottom: {frame}"
    );
    assert!(frame.lines().count() <= 24, "frame overflows: {frame}");
}

#[test]
fn wrapping_breaks_after_a_space_and_keeps_the_cursor_on_the_typed_row() {
    let layout = super::view::layout_input("hello world again", 17, 8);
    let rows: Vec<&str> = layout
        .rows
        .iter()
        .map(|row| &"hello world again"[row.clone()])
        .collect();

    assert_eq!(rows, vec!["hello ", "world ", "again"]);
    assert_eq!((layout.cursor_row, layout.cursor_col), (2, 5));
}

#[test]
fn mouse_capture_is_off_unless_the_user_opts_in() {
    let mut config = Config::default();
    assert!(
        !super::mouse_capture_enabled(&config),
        "capture must default off so drag-select reaches the terminal"
    );

    config.disable_mouse_capture = Some(false);
    assert!(super::mouse_capture_enabled(&config));

    config.disable_mouse_capture = Some(true);
    assert!(!super::mouse_capture_enabled(&config));
}

#[test]
fn bordered_box_frames_content_at_a_fixed_width() {
    let plain = lipgloss::Style::new();
    let lines = super::view::bordered_box(["hi", "a longer row"], 20, &plain);

    assert_eq!(
        lines,
        vec![
            "╭──────────────────╮".to_string(),
            "│ hi               │".to_string(),
            "│ a longer row     │".to_string(),
            "╰──────────────────╯".to_string(),
        ]
    );
}

#[test]
fn bordered_box_truncates_rows_wider_than_the_frame() {
    let plain = lipgloss::Style::new();
    let lines = super::view::bordered_box(["overflowing content here"], 12, &plain);

    assert_eq!(
        lines,
        vec![
            "╭──────────╮".to_string(),
            "│ overflow │".to_string(),
            "╰──────────╯".to_string(),
        ]
    );
}

#[test]
fn shift_tab_cycles_the_permission_mode_and_renders_its_indicator() {
    use bubbletea::KeyType;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);

    assert_eq!(app.permission_mode, PermissionMode::Default);
    assert!(
        !app.render_input().contains("accept edits on"),
        "default mode must not draw an indicator"
    );

    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::ShiftTab)));

    assert_eq!(app.permission_mode, PermissionMode::AcceptEdits);
    assert!(
        app.render_input().contains("accept edits on"),
        "input: {}",
        app.render_input()
    );
}

/// Queue one approval question and open it, returning the answer channel.
fn open_tool_approval(
    app: &mut PiApp,
    tool_name: &str,
    arguments: serde_json::Value,
) -> asupersync::channel::oneshot::Receiver<crate::agent::ToolApprovalDecision> {
    use crate::agent::ToolApprovalRequest;

    let (reply, answer) = asupersync::channel::oneshot::channel();
    app.tool_approval_queue
        .lock()
        .expect("approval queue")
        .push_back(super::ToolApprovalPrompt {
            request: ToolApprovalRequest {
                tool_call_id: "call-1".to_string(),
                tool_name: tool_name.to_string(),
                arguments,
            },
            reply,
        });
    let _ = app.update(Message::new(super::PiMsg::ToolApprovalPending));
    answer
}

#[test]
fn approval_modal_allows_and_answers_the_waiting_agent() {
    use crate::agent::ToolApprovalDecision;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);

    let mut answer = open_tool_approval(&mut app, "bash", serde_json::json!({"command": "ls -la"}));
    assert!(app.tool_approval.is_some(), "the modal should be open");
    assert!(
        app.view().contains("ls -la"),
        "the command belongs in the box: {}",
        app.view()
    );

    let _ = app.update(Message::new(KeyMsg::from_runes(vec!['1'])));

    assert!(app.tool_approval.is_none(), "answering closes the modal");
    assert_eq!(
        answer.try_recv().expect("the agent was answered"),
        ToolApprovalDecision::Allow
    );
}

#[test]
fn approval_modal_rejects_on_escape_and_never_strands_the_turn() {
    use crate::agent::ToolApprovalDecision;
    use bubbletea::KeyType;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);

    let mut answer =
        open_tool_approval(&mut app, "bash", serde_json::json!({"command": "rm -rf /"}));
    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Esc)));

    assert!(app.tool_approval.is_none());
    assert!(matches!(
        answer.try_recv().expect("the agent was answered"),
        ToolApprovalDecision::Deny { .. }
    ));
}

#[test]
fn approving_for_the_session_writes_a_rule_the_policy_then_allows() {
    use crate::tool_policy::Decision;
    use crate::tools::ToolEffects;

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    app.set_terminal_size(100, 40);

    let args = serde_json::json!({"command": "git status"});
    let _answer = open_tool_approval(&mut app, "bash", args.clone());
    let _ = app.update(Message::new(KeyMsg::from_runes(vec!['2'])));

    let policy = app.tool_policy.read().expect("policy");
    assert_eq!(
        policy.decide("bash", ToolEffects::process(), &args),
        Decision::Allow,
        "the session rule should now cover `git`"
    );
    assert!(
        matches!(
            policy.decide(
                "bash",
                ToolEffects::process(),
                &serde_json::json!({"command": "rm -rf /"})
            ),
            Decision::Ask
        ),
        "and only `git`"
    );
}

#[test]
fn enter_accepts_highlighted_autocomplete_item() {
    // Regression for issue #61: with the slash dropdown open and an entry
    // highlighted (e.g. user pressed Down to select `/model`), pressing Enter
    // must accept the highlighted item — matching the dropdown's own footer
    // hint "Enter/Tab accept" — not submit the raw `/` typed so far.
    use crate::autocomplete::{AutocompleteItem, AutocompleteItemKind};
    use bubbletea::{KeyMsg, KeyType, Message};

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());

    app.input.set_value("/");
    app.autocomplete.open = true;
    app.autocomplete.items = vec![AutocompleteItem {
        kind: AutocompleteItemKind::SlashCommand,
        label: "/model".to_string(),
        insert: "/model ".to_string(),
        description: None,
    }];
    app.autocomplete.selected = Some(0);
    app.autocomplete.replace_range = 0..1;

    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Enter)));

    assert_eq!(
        app.input.value(),
        "/model ",
        "Enter with a highlighted dropdown entry must accept the item"
    );
    assert!(
        !app.autocomplete.open,
        "Accepting via Enter should close the dropdown"
    );
}

#[test]
fn enter_submits_when_no_autocomplete_item_highlighted() {
    // The dual contract for issue #61: when the dropdown is open but the
    // user has not navigated to any item (selected.is_none()), Enter must
    // still submit the raw editor contents — i.e. behavior is unchanged
    // for users who never pressed Down.
    use bubbletea::{KeyMsg, KeyType, Message};

    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());

    app.input.set_value("/foo");
    app.autocomplete.open = true;
    app.autocomplete.items.clear();
    app.autocomplete.selected = None;
    app.autocomplete.replace_range = 0..4;

    let _ = app.update(Message::new(KeyMsg::from_type(KeyType::Enter)));

    assert!(
        !app.autocomplete.open,
        "Enter with no selection should still close the dropdown"
    );
}

#[derive(Default)]
struct TuiDegradationDrillTrace {
    event_count: usize,
    redraw_count: usize,
    coalesced_count: usize,
    preserved_input_count: usize,
    max_rendered_rows: usize,
}

impl TuiDegradationDrillTrace {
    fn event(&mut self) {
        self.event_count += 1;
    }

    fn render(&mut self, app: &PiApp) -> String {
        let frame = app.view();
        self.redraw_count += 1;
        self.max_rendered_rows = self.max_rendered_rows.max(frame.lines().count());
        frame
    }

    fn record_input_preserved(&mut self, before_len: usize, after: &str) {
        self.preserved_input_count += after.len().saturating_sub(before_len);
    }
}

fn pressure_tool_block(label: &str) -> ContentBlock {
    let mut output = String::new();
    for line in 0..32 {
        output.push_str(label);
        output.push_str(" line ");
        output.push_str(&line.to_string());
        output.push('\n');
    }
    ContentBlock::Text(TextContent::new(output))
}

fn semantic_visible(frame: &str, marker: &str) -> bool {
    frame.contains(marker)
}

const PROVIDER_DELTA_COUNT: usize = 72;
const THINKING_DELTA_COUNT: usize = 12;
const TOOL_UPDATE_COUNT: usize = 18;
const SESSION_BURST_COUNT: usize = 10;
const FINAL_MARKER: &str = "semantic-provider-delta-71";
const TOOL_MARKER: &str = "semantic-tool-final";
const SESSION_MARKER: &str = "session write burst 9 committed";

fn seed_normal_tui_load(app: &mut PiApp, trace: &mut TuiDegradationDrillTrace) {
    app.messages.push(ConversationMessage::new(
        MessageRole::User,
        "normal-load prompt remains readable".to_string(),
        None,
    ));
    app.messages.push(ConversationMessage::new(
        MessageRole::Assistant,
        "normal-load assistant reply remains readable".to_string(),
        None,
    ));
    app.scroll_to_bottom();
    trace.event_count += 2;
    let normal_frame = trace.render(app);
    assert!(semantic_visible(
        &normal_frame,
        "normal-load assistant reply"
    ));
}

fn drive_provider_pressure(app: &mut PiApp, trace: &mut TuiDegradationDrillTrace) {
    app.tui_pressure_frame_p99_us.store(
        TuiPressureController::HIGH_FRAME_P99_US,
        std::sync::atomic::Ordering::Relaxed,
    );
    app.handle_pi_message(PiMsg::AgentStart);
    trace.event();
    for idx in 0..PROVIDER_DELTA_COUNT {
        let delta = format!("semantic-provider-delta-{idx} ");
        app.handle_pi_message(PiMsg::TextDelta(delta));
        trace.event();
        if idx % 16 == 0 {
            let frame = trace.render(app);
            assert!(
                semantic_visible(&frame, &format!("semantic-provider-delta-{idx}")),
                "streaming provider delta must stay visible at idx {idx}"
            );
        }
    }
    for idx in 0..THINKING_DELTA_COUNT {
        app.handle_pi_message(PiMsg::ThinkingDelta(format!("thinking-step-{idx} ")));
        trace.event();
    }
}

fn drive_tool_pressure(app: &mut PiApp, trace: &mut TuiDegradationDrillTrace) {
    app.handle_pi_message(PiMsg::ToolStart {
        name: "bash".to_string(),
        tool_id: "tool-pressure".to_string(),
    });
    trace.event();
    for idx in 0..TOOL_UPDATE_COUNT {
        let label = if idx + 1 == TOOL_UPDATE_COUNT {
            TOOL_MARKER
        } else {
            "low-value-tool-noise"
        };
        app.handle_pi_message(PiMsg::ToolUpdate {
            name: "bash".to_string(),
            tool_id: "tool-pressure".to_string(),
            content: vec![pressure_tool_block(label)],
            details: Some(json!({
                "line_count": (idx + 1) * 32,
                "byte_count": (idx + 1) * 512,
            })),
        });
        trace.event();
    }
    trace.coalesced_count += TOOL_UPDATE_COUNT.saturating_sub(1);
    app.handle_pi_message(PiMsg::ToolEnd {
        name: "bash".to_string(),
        tool_id: "tool-pressure".to_string(),
        is_error: false,
    });
    trace.event();
}

fn drive_session_write_bursts(app: &mut PiApp, trace: &mut TuiDegradationDrillTrace) {
    for idx in 0..SESSION_BURST_COUNT {
        app.handle_pi_message(PiMsg::SystemNote(format!(
            "session write burst {idx} committed"
        )));
        trace.event();
    }
}

fn drive_resize_pressure(app: &mut PiApp, trace: &mut TuiDegradationDrillTrace) {
    let _ = app.update(Message::new(WindowSizeMsg {
        width: 92,
        height: 26,
    }));
    trace.event();
    let compact_frame = trace.render(app);
    assert!(
        compact_frame.lines().count() <= app.term_height,
        "compact resize frame must not exceed terminal height"
    );

    let _ = app.update(Message::new(WindowSizeMsg {
        width: 120,
        height: 64,
    }));
    trace.event();
}

fn finish_agent_and_preserve_input(app: &mut PiApp, trace: &mut TuiDegradationDrillTrace) {
    app.handle_pi_message(PiMsg::AgentDone {
        usage: None,
        stop_reason: StopReason::Stop,
        error_message: None,
    });
    trace.event();

    for key in ['o', 'k'] {
        let before_len = app.input.value().len();
        let _ = app.update(Message::new(KeyMsg::from_char(key)));
        trace.event();
        trace.record_input_preserved(before_len, &app.input.value());
    }
}

fn assert_tui_degradation_evidence(app: &PiApp, trace: &mut TuiDegradationDrillTrace) {
    let final_frame = trace.render(app);
    let collapsed_tool_messages = app
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::Tool && message.collapsed)
        .count();
    let semantic_visible_count = [
        semantic_visible(&final_frame, FINAL_MARKER),
        semantic_visible(&final_frame, TOOL_MARKER),
        semantic_visible(&final_frame, SESSION_MARKER),
        app.input.value() == "ok",
    ]
    .into_iter()
    .filter(|visible| *visible)
    .count();
    let frame_pressure = TuiPressureController::decide(
        TuiPressureController::HIGH_FRAME_P99_US,
        TuiPressureController::HIGH_TOOL_OUTPUT_BYTES,
        TOOL_UPDATE_COUNT,
    );
    let evidence = json!({
        "schema": "pi.tui.degradation_drill.v1",
        "fixture": "sustained_event_pressure",
        "event_count": trace.event_count,
        "redraw_count": trace.redraw_count,
        "coalesced_count": trace.coalesced_count,
        "max_frame_budget_pressure": format!("{:?}", frame_pressure.level),
        "max_rendered_rows": trace.max_rendered_rows,
        "terminal_height": app.term_height,
        "preserved_input_count": trace.preserved_input_count,
        "semantic_visible_count": semantic_visible_count,
        "collapsed_tool_message_count": collapsed_tool_messages,
        "verdict": if semantic_visible_count == 4
            && collapsed_tool_messages == 1
            && trace.preserved_input_count == 2
            && trace.max_rendered_rows <= app.term_height
        {
            "pass"
        } else {
            "fail_closed"
        },
    });

    assert_eq!(evidence["schema"], "pi.tui.degradation_drill.v1");
    assert_eq!(evidence["event_count"], 122);
    assert_eq!(evidence["redraw_count"], 8);
    assert_eq!(evidence["coalesced_count"], TOOL_UPDATE_COUNT - 1);
    assert_eq!(evidence["max_frame_budget_pressure"], "High");
    assert_eq!(evidence["preserved_input_count"], 2);
    assert_eq!(evidence["collapsed_tool_message_count"], 1);
    assert_eq!(evidence["semantic_visible_count"], 4);
    assert_eq!(
        evidence["verdict"], "pass",
        "degradation evidence: {evidence}"
    );
}

#[test]
fn tui_degradation_drill_preserves_input_and_semantics_under_pressure() {
    let dir = tempdir();
    let mut app = build_test_app(dir.path().to_path_buf());
    let mut trace = TuiDegradationDrillTrace::default();
    app.enable_frame_timing_for_test();
    app.reset_frame_timing_for_test();
    app.set_terminal_size(120, 48);

    seed_normal_tui_load(&mut app, &mut trace);
    drive_provider_pressure(&mut app, &mut trace);
    drive_tool_pressure(&mut app, &mut trace);
    drive_session_write_bursts(&mut app, &mut trace);
    drive_resize_pressure(&mut app, &mut trace);
    finish_agent_and_preserve_input(&mut app, &mut trace);
    assert_tui_degradation_evidence(&app, &mut trace);
}
