use super::commands::model_entry_matches;
use super::*;

/// Outcome of a ctrl+V image paste. `Empty` and `Unsupported` are different
/// failures and used to be the same silent `None`.
pub(super) enum ImagePaste {
    Attached(PathBuf),
    Empty,
    Unsupported,
}

impl PiApp {
    pub(super) fn handle_custom_extension_key(&mut self, key: &KeyMsg) -> bool {
        if !self.custom_overlay_input_is_available() {
            return false;
        }
        if key.key_type == KeyType::CtrlC {
            return false;
        }

        if let Some(encoded) = encode_custom_ui_key(key) {
            const MAX_CUSTOM_KEY_QUEUE: usize = 256;
            if self.extension_custom_key_queue.len() >= MAX_CUSTOM_KEY_QUEUE {
                let _ = self.extension_custom_key_queue.pop_front();
            }
            self.extension_custom_key_queue.push_back(encoded);
        }

        true
    }

    /// Format keyboard shortcuts for /hotkeys display.
    ///
    /// Groups actions by category and shows their key bindings.
    pub(super) fn format_hotkeys(&self) -> String {
        use crate::keybindings::ActionCategory;
        use std::fmt::Write;

        let mut output = String::new();
        let _ = writeln!(output, "Keyboard Shortcuts");
        let _ = writeln!(output, "==================");
        let _ = writeln!(output);
        let _ = writeln!(
            output,
            "Config: {}",
            KeyBindings::user_config_path().display()
        );
        let _ = writeln!(output);

        for category in ActionCategory::all() {
            let actions: Vec<_> = self.keybindings.iter_category(*category).collect();

            // Skip empty categories
            if actions.iter().all(|(_, bindings)| bindings.is_empty()) {
                continue;
            }

            let _ = writeln!(output, "## {}", category.display_name());
            let _ = writeln!(output);

            for (action, bindings) in actions {
                if bindings.is_empty() {
                    continue;
                }

                // Format bindings as comma-separated list
                let keys: Vec<_> = bindings
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                let keys_str = keys.join(", ");

                let _ = writeln!(output, "  {:20} {}", keys_str, action.display_name());
            }
            let _ = writeln!(output);
        }

        output
    }

    pub(super) fn resolve_action(&self, candidates: &[AppAction]) -> Option<AppAction> {
        let &first = candidates.first()?;

        // Some bindings are ambiguous and depend on UI state.
        // Example: `ctrl+d` can mean "delete forward" while editing, but "exit" when the editor
        // is empty (legacy behavior).
        if candidates.contains(&AppAction::Exit)
            && self.agent_state == AgentState::Idle
            && self.input.value().is_empty()
        {
            return Some(AppAction::Exit);
        }

        Some(first)
    }

    pub(super) fn handle_capability_prompt_key(&mut self, key: &KeyMsg) -> Option<Cmd> {
        let prompt = self.capability_prompt.as_mut()?;

        // The options stack vertically like the tool-approval modal, so the
        // same keys drive both; the older horizontal keys still work.
        match key.key_type {
            KeyType::Down | KeyType::Right | KeyType::Tab => prompt.focus_next(),
            KeyType::Up | KeyType::Left => prompt.focus_prev(),
            KeyType::Runes if key.runes == ['j'] || key.runes == ['l'] => prompt.focus_next(),
            KeyType::Runes if key.runes == ['k'] || key.runes == ['h'] => prompt.focus_prev(),
            KeyType::Runes
                if key.runes.len() == 1
                    && let Some(index) = key.runes[0]
                        .to_digit(10)
                        .map(|digit| digit as usize)
                        .filter(|digit| (1..=CapabilityAction::ALL.len()).contains(digit)) =>
            {
                self.answer_capability_prompt(CapabilityAction::ALL[index - 1]);
            }
            KeyType::Enter => {
                let action = prompt.selected_action();
                self.answer_capability_prompt(action);
            }
            // Escape = deny once.
            KeyType::Esc => {
                let response = ExtensionUiResponse {
                    id: prompt.request.id.clone(),
                    value: Some(Value::Bool(false)),
                    cancelled: true,
                };
                self.capability_prompt = None;
                self.send_extension_ui_response(response);
            }
            _ => {}
        }

        None
    }

    /// Answer the open capability prompt. The two "always" answers are
    /// recorded on disk and survive restarts, unlike a tool approval's
    /// session rule.
    fn answer_capability_prompt(&mut self, action: CapabilityAction) {
        let Some(prompt) = self.capability_prompt.take() else {
            return;
        };
        let response = ExtensionUiResponse {
            id: prompt.request.id.clone(),
            value: Some(Value::Bool(action.is_allow())),
            cancelled: false,
        };
        if action.is_persistent()
            && let Ok(mut store) = crate::permissions::PermissionStore::open_default()
        {
            let _ = store.record(&prompt.extension_id, &prompt.capability, action.is_allow());
        }
        self.send_extension_ui_response(response);
    }

    pub(super) fn handle_tool_approval_key(&mut self, key: &KeyMsg) -> Option<Cmd> {
        let prompt = self.tool_approval.as_mut()?;

        match key.key_type {
            KeyType::Down | KeyType::Tab => prompt.focus_next(),
            KeyType::Up => prompt.focus_prev(),
            KeyType::Runes if key.runes == ['j'] => prompt.focus_next(),
            KeyType::Runes if key.runes == ['k'] => prompt.focus_prev(),
            KeyType::Runes if key.runes == ['1'] => self.answer_tool_approval(ApprovalAction::Once),
            KeyType::Runes if key.runes == ['2'] => {
                self.answer_tool_approval(ApprovalAction::Session);
            }
            KeyType::Runes if key.runes == ['3'] => {
                self.answer_tool_approval(ApprovalAction::Reject);
            }
            KeyType::Enter => {
                let action = prompt.selected_action();
                self.answer_tool_approval(action);
            }
            KeyType::Esc => self.answer_tool_approval(ApprovalAction::Reject),
            _ => {}
        }

        None
    }

    /// Answer the open approval and surface the next queued one.
    fn answer_tool_approval(&mut self, action: ApprovalAction) {
        let Some(mut prompt) = self.tool_approval.take() else {
            return;
        };

        if action == ApprovalAction::Session
            && let Err(err) = self
                .tool_policy
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .allow_for_session(&prompt.session_rule)
        {
            self.status_message = Some(format!("Could not remember that choice: {err}"));
        }

        let decision = if action == ApprovalAction::Reject {
            ToolApprovalDecision::deny("the user rejected this tool call")
        } else {
            ToolApprovalDecision::Allow
        };
        if let Some(reply) = prompt.reply.take() {
            let cx = Cx::current().unwrap_or_else(Cx::for_request);
            let _ = reply.send(&cx, decision);
        }

        self.messages.push(ConversationMessage {
            role: MessageRole::System,
            content: match action {
                ApprovalAction::Once => format!("Approved `{}`.", prompt.tool_name),
                ApprovalAction::Session => format!(
                    "Approved `{}`, and will not ask again for `{}` this session.",
                    prompt.tool_name, prompt.session_rule
                ),
                ApprovalAction::Reject => format!("Rejected `{}`.", prompt.tool_name),
            },
            thinking: None,
            collapsed: false,
        });
        self.scroll_to_bottom();

        self.show_next_tool_approval();
    }

    /// Open the next queued approval, if the modal is free and one is waiting.
    pub(super) fn show_next_tool_approval(&mut self) {
        if self.tool_approval.is_some() {
            return;
        }
        let next = self
            .tool_approval_queue
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front());
        self.tool_approval = next.map(|prompt| ToolApprovalOverlay::new(prompt, &self.cwd));
    }

    pub(super) fn handle_rewind_overlay_key(&mut self, key: &KeyMsg) -> Option<Cmd> {
        let rune = (key.key_type == KeyType::Runes && key.runes.len() == 1).then(|| key.runes[0]);
        let overlay = self.rewind_overlay.as_mut()?;
        let mut close = false;
        let mut commit = false;

        if overlay.picking_scope() {
            match (key.key_type, rune) {
                (KeyType::Up, _) | (_, Some('k')) => overlay.scope_prev(),
                (KeyType::Down, _) | (_, Some('j')) => overlay.scope_next(),
                (_, Some(digit @ '1'..='3')) => {
                    overlay.scope = Some(digit as usize - '1' as usize);
                    commit = true;
                }
                (KeyType::Enter, _) => commit = true,
                (KeyType::Esc, _) => overlay.scope = None,
                _ => {}
            }
        } else {
            match (key.key_type, rune) {
                (KeyType::Up, _) | (_, Some('k')) => overlay.select_prev(),
                (KeyType::Down, _) | (_, Some('j')) => overlay.select_next(),
                (KeyType::PgUp, _) => overlay.select_page_up(),
                (KeyType::PgDown, _) => overlay.select_page_down(),
                (KeyType::Enter, _) => {
                    if overlay.turns.is_empty() {
                        close = true;
                    } else {
                        overlay.scope = Some(0);
                    }
                }
                (KeyType::Esc, _) => close = true,
                _ => {}
            }
        }

        if close {
            self.rewind_overlay = None;
            return None;
        }
        if !commit {
            return None;
        }

        let overlay = self.rewind_overlay.take()?;
        let target = overlay.selected_turn()?.turn;
        let scope = overlay.selected_scope()?;
        self.apply_rewind(target, scope)
    }

    pub(super) fn handle_paste_event(&mut self, key: &KeyMsg) -> bool {
        if key.key_type != KeyType::Runes || key.runes.is_empty() {
            return false;
        }

        // Terminals send pasted line breaks as CR (tmux rewrites LF to CR
        // outright), so count and store lines on \n or nothing matches.
        let pasted = key
            .runes
            .iter()
            .collect::<String>()
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let Some((insert, count)) = self.normalize_pasted_paths(&pasted) else {
            return self.collapse_large_paste(&pasted);
        };

        self.input.insert_string(&insert);
        if count > 0 {
            self.status_message = Some(format!(
                "Attached {} file{}",
                count,
                if count == 1 { "" } else { "s" }
            ));
        }
        true
    }

    /// Insert a `[pasted N lines]` placeholder for a paste too big to read in
    /// the box, keeping the real text for submission.
    fn collapse_large_paste(&mut self, pasted: &str) -> bool {
        let lines = pasted.lines().count();
        if lines < PASTE_COLLAPSE_MIN_LINES {
            return false;
        }

        let placeholder = format!("[pasted {lines} lines]");
        self.input.insert_string(&placeholder);
        self.pasted_blocks
            .push((placeholder, pasted.trim_end_matches('\n').to_string()));
        self.status_message = Some(format!("Collapsed paste of {lines} lines"));
        true
    }

    /// Swap every collapsed-paste placeholder back for its real text.
    ///
    /// Each block is consumed on first match, so a placeholder the user deleted
    /// is simply dropped.
    fn expand_pasted_blocks(&mut self, text: &str) -> String {
        let mut expanded = text.to_string();
        for (placeholder, body) in self.pasted_blocks.drain(..) {
            expanded = expanded.replacen(&placeholder, &body, 1);
        }
        expanded
    }

    /// Expand collapsed pastes in place, for paths that read the editor
    /// themselves instead of taking the submitted text.
    fn expand_pasted_blocks_in_editor(&mut self) {
        if self.pasted_blocks.is_empty() {
            return;
        }
        let expanded = self.expand_pasted_blocks(&self.input.value());
        self.input.set_value(&expanded);
    }

    fn normalize_pasted_paths(&self, pasted: &str) -> Option<(String, usize)> {
        let mut refs = Vec::new();
        for line in pasted.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = self.normalize_pasted_path(trimmed)?;
            refs.push(path);
        }

        if refs.is_empty() {
            return None;
        }

        let mut insert = refs
            .iter()
            .map(|path| format_file_ref(path))
            .collect::<Vec<_>>()
            .join(" ");
        if !insert.ends_with(' ') {
            insert.push(' ');
        }

        Some((insert, refs.len()))
    }

    fn normalize_pasted_path(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('@') {
            return None;
        }

        let unquoted = strip_wrapping_quotes(trimmed);
        let unescaped = unescape_dragged_path(unquoted);
        let path = file_url_to_path(&unescaped).unwrap_or_else(|| PathBuf::from(&unescaped));
        let resolved = resolve_read_path(path.to_string_lossy().as_ref(), &self.cwd);
        if !resolved.exists() {
            return None;
        }

        Some(path_for_display(&resolved, &self.cwd))
    }

    pub(super) fn insert_file_ref_path(&mut self, path: &Path) {
        let display = path_for_display(path, &self.cwd);
        let mut insert_text = format_file_ref(&display);
        if !insert_text.ends_with(' ') {
            insert_text.push(' ');
        }
        self.input.insert_string(&insert_text);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub(super) fn paste_image_from_clipboard() -> ImagePaste {
        #[cfg(all(feature = "clipboard", feature = "image-resize"))]
        {
            let decode = || -> Option<PathBuf> {
                use image::ImageEncoder;

                let mut clipboard = ArboardClipboard::new().ok()?;
                let image = clipboard.get_image().ok()?;

                let width = u32::try_from(image.width).ok()?;
                let height = u32::try_from(image.height).ok()?;
                let bytes = image.bytes.into_owned();
                let width_usize = usize::try_from(width).ok()?;
                let height_usize = usize::try_from(height).ok()?;
                let expected = width_usize.checked_mul(height_usize)?.checked_mul(4)?;
                if bytes.len() != expected {
                    return None;
                }

                // Under the agent dir, not the system temp dir: the read tools
                // refuse any path outside cwd or the agent dir, so a /tmp paste
                // attaches and then fails validation on the very next send.
                let pastes = crate::config::Config::global_dir().join("pastes");
                std::fs::create_dir_all(&pastes).ok()?;
                let mut temp_file = tempfile::Builder::new()
                    .prefix("kesa-paste-")
                    .suffix(".png")
                    .tempfile_in(&pastes)
                    .ok()?;
                let encoder = image::codecs::png::PngEncoder::new(&mut temp_file);
                if encoder
                    .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
                    .is_err()
                {
                    return None;
                }
                let (_file, path) = temp_file.keep().ok()?;
                Some(path)
            };
            decode().map_or(ImagePaste::Empty, ImagePaste::Attached)
        }

        #[cfg(not(all(feature = "clipboard", feature = "image-resize")))]
        {
            ImagePaste::Unsupported
        }
    }

    /// Open external editor with current input text.
    ///
    /// Uses $VISUAL if set, otherwise $EDITOR, otherwise "vi".
    /// Supports editors with arguments like "code --wait" or "vim -u NONE".
    pub(super) fn open_external_editor(&self) -> std::io::Result<String> {
        use std::io::Write;

        // Determine editor command
        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());

        // Create temp file with current editor content
        let mut temp_file = tempfile::NamedTempFile::new()?;
        let current_text = self.input.value();
        temp_file.write_all(current_text.as_bytes())?;
        temp_file.flush()?;

        let temp_path = temp_file.path().to_path_buf();

        // Pause terminal UI so the external editor can use the terminal correctly
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

        // Spawn editor via shell to handle EDITOR with arguments (e.g., "code --wait")
        // The shell properly handles quoting, arguments, and PATH lookup
        #[cfg(unix)]
        let status = std::process::Command::new("sh")
            .args(["-c", &format!("{editor} \"$1\"")])
            .arg("--") // separator for positional args
            .arg(&temp_path)
            .status();

        #[cfg(not(unix))]
        let status = std::process::Command::new("cmd")
            .args(["/c", &format!("{} \"{}\"", editor, temp_path.display())])
            .status();

        // Resume terminal UI
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        );

        let status = status?;

        if !status.success() {
            return Err(std::io::Error::other(format!(
                "Editor exited with status: {status}"
            )));
        }

        // Read back the edited content
        let new_text = std::fs::read_to_string(&temp_path)?;
        Ok(new_text)
    }

    /// Navigate to previous history entry.
    fn navigate_history_back(&mut self) {
        if !self.history.has_entries() {
            return;
        }

        self.history.cursor_up();
        self.apply_history_selection();
    }

    /// Navigate to next history entry.
    fn navigate_history_forward(&mut self) {
        // Avoid clearing the editor when the user hasn't entered history navigation.
        if self.history.cursor_is_empty() {
            return;
        }

        self.history.cursor_down();
        self.apply_history_selection();
    }

    fn apply_history_selection(&mut self) {
        let selected = self.history.selected_value();
        if selected.is_empty() {
            self.input.reset();
        } else {
            self.input.set_value(selected);
        }
    }

    /// The configured value wins. Unconfigured, the chord opens rewind when a
    /// checkpoint store is running, because undo is what Esc Esc is for; with
    /// rewind off there is nothing to open, so the old default stands.
    pub(super) fn double_escape_action(&self) -> &str {
        match self.config.double_escape_action.as_deref().map(str::trim) {
            Some(configured) if !configured.is_empty() => configured,
            _ if crate::rewind::is_active() => "rewind",
            _ => "tree",
        }
    }

    fn handle_double_escape_action(&mut self) -> (bool, Option<Cmd>) {
        let action = self.double_escape_action();
        if action.eq_ignore_ascii_case("none") {
            self.last_escape_time = None;
            return (false, None);
        }
        let now = std::time::Instant::now();
        if let Some(last_time) = self.last_escape_time
            && now.duration_since(last_time) < std::time::Duration::from_millis(500)
        {
            self.last_escape_time = None;
            return (true, self.trigger_double_escape_action());
        }
        self.last_escape_time = Some(now);
        (false, None)
    }

    fn trigger_double_escape_action(&mut self) -> Option<Cmd> {
        let raw_action = self.double_escape_action().to_string();
        match raw_action.to_ascii_lowercase().as_str() {
            "none" => None,
            "tree" => self.handle_slash_command(SlashCommand::Tree, ""),
            "fork" => self.handle_slash_command(SlashCommand::Fork, ""),
            "rewind" | "undo" => self.handle_slash_command(SlashCommand::Rewind, ""),
            _ => {
                self.status_message = Some(format!(
                    "Unknown doubleEscapeAction: {raw_action} (expected rewind, tree, fork, or none)"
                ));
                self.handle_slash_command(SlashCommand::Tree, "")
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn cycle_model(&mut self, delta: i32) {
        if self.agent_state != AgentState::Idle {
            self.status_message = Some("Cannot switch models while processing".to_string());
            return;
        }

        let scope_configured = self
            .config
            .enabled_models
            .as_ref()
            .is_some_and(|patterns| !patterns.is_empty());
        let use_scope = scope_configured || !self.model_scope.is_empty();
        let mut fell_back_to_available = false;
        let mut candidates = if use_scope {
            self.model_scope.clone()
        } else {
            self.available_models.clone()
        };
        if use_scope && candidates.is_empty() {
            candidates.clone_from(&self.available_models);
            fell_back_to_available = true;
        }

        candidates.sort_by(|a, b| {
            let left = format!("{}/{}", a.model.provider, a.model.id);
            let right = format!("{}/{}", b.model.provider, b.model.id);
            left.cmp(&right)
        });
        candidates.dedup_by(|left, right| model_entry_matches(left, right));

        if candidates.is_empty() {
            self.status_message = Some("No models available".to_string());
            return;
        }

        let current_index = candidates
            .iter()
            .position(|entry| model_entry_matches(entry, &self.model_entry));

        let next_index = current_index.map_or_else(
            || {
                if delta >= 0 { 0 } else { candidates.len() - 1 }
            },
            |idx| {
                if delta >= 0 {
                    (idx + 1) % candidates.len()
                } else {
                    idx.checked_sub(1).unwrap_or(candidates.len() - 1)
                }
            },
        );

        let next = candidates[next_index].clone();

        if model_entry_matches(&next, &self.model_entry) {
            self.status_message = Some(if use_scope && !fell_back_to_available {
                "Only one model in scope".to_string()
            } else {
                "Only one model available".to_string()
            });
            return;
        }

        let provider_impl = match providers::create_provider(&next, self.extensions.as_ref()) {
            Ok(provider_impl) => provider_impl,
            Err(err) => {
                self.status_message = Some(err.to_string());
                return;
            }
        };
        let resolved_key_opt = super::commands::resolve_model_key_from_default_auth(&next);
        if crate::models::model_requires_configured_credential(&next) && resolved_key_opt.is_none()
        {
            self.status_message = Some(format!(
                "Missing credentials for provider {}. Run /login {}.",
                next.model.provider, next.model.provider
            ));
            return;
        }

        if let Err(message) =
            self.switch_active_model(&next, provider_impl, resolved_key_opt.as_deref(), "cycle")
        {
            self.status_message = Some(message);
            return;
        }
        self.status_message = Some(if fell_back_to_available {
            format!(
                "No scoped models matched; cycling all available models. Switched model: {}",
                self.model
            )
        } else {
            format!("Switched model: {}", self.model)
        });
    }

    pub(super) fn cycle_thinking_level(&mut self) {
        let levels = self.model_entry.available_thinking_levels();
        if levels.len() <= 1 {
            self.status_message = Some("Current model does not support thinking".to_string());
            return;
        }

        let Ok(mut agent_guard) = self.agent.try_lock() else {
            self.status_message = Some("Agent busy; try again".to_string());
            return;
        };
        let Ok(mut session_guard) = self.session.try_lock() else {
            self.status_message = Some("Session busy; try again".to_string());
            return;
        };

        let current = session_guard
            .effective_thinking_level_for_current_path()
            .as_deref()
            .and_then(|value| value.parse::<crate::model::ThinkingLevel>().ok())
            .or_else(|| agent_guard.stream_options().thinking_level)
            .unwrap_or_default();

        let current_index = levels
            .iter()
            .position(|level| *level == current)
            .unwrap_or(0);
        let next = levels[(current_index + 1) % levels.len()];

        let previous_level = session_guard
            .effective_thinking_level_for_current_path()
            .as_deref()
            .and_then(|value| value.parse::<crate::model::ThinkingLevel>().ok());
        session_guard.header.thinking_level = Some(next.to_string());
        let changed = previous_level != Some(next);
        if changed {
            session_guard.append_thinking_level_change(next.to_string());
        }

        agent_guard.stream_options_mut().thinking_level = Some(next);
        drop(session_guard);
        drop(agent_guard);

        if changed {
            self.spawn_save_session();
        }

        self.status_message = Some(format!("Thinking level: {next}"));
    }

    pub(super) fn quit_cmd(&mut self) -> Cmd {
        if let Some(manager) = &self.extensions {
            manager.clear_ui_sender();
        }

        // Schedule a guaranteed bridge shutdown instead of a lossy try_send so quit
        // still unwinds when the bounded event queue is already saturated.
        let shutdown_tx = self.event_tx.clone();
        self.runtime_handle.spawn(async move {
            let shutdown_cx = Cx::for_request();
            super::enqueue_ui_shutdown(&shutdown_tx, &shutdown_cx).await;
        });

        // Drop the async → bubbletea bridge sender so bubbletea can shut down cleanly.
        // Without this, bubbletea's external forwarder thread can block on `recv()` during quit.
        let (tx, _rx) = mpsc::channel::<PiMsg>(1);
        drop(std::mem::replace(&mut self.event_tx, tx));
        quit()
    }

    /// Swap a backslash the cursor sits behind for a newline, so Enter
    /// continues the message instead of sending it.
    fn take_line_continuation(&mut self) -> bool {
        let mut value = self.input.value();
        if self.input.cursor_byte_offset() != value.len() || !value.ends_with('\\') {
            return false;
        }
        value.pop();
        value.push('\n');
        self.input.set_value(&value);
        self.input.set_cursor_byte_offset(value.len());
        true
    }

    /// Whether the editor wraps or breaks onto more than one visual row, which
    /// is what decides whether Up and Down move the cursor or walk history.
    fn input_is_multi_row(&self) -> bool {
        self.input_layout().rows.len() > 1
    }

    /// Move the cursor one visual row up or down, wrapped rows included.
    fn move_cursor_by_row(&mut self, up: bool) {
        let value = self.input.value();
        let layout = self.input_layout();
        let target = if up {
            layout.cursor_row.checked_sub(1)
        } else {
            (layout.cursor_row + 1 < layout.rows.len()).then_some(layout.cursor_row + 1)
        };
        let Some(target) = target else {
            return;
        };
        let offset = view::input_byte_at_column(&value, &layout.rows[target], layout.cursor_col);
        self.input.set_cursor_byte_offset(offset);
    }

    /// Handle an action dispatched from the keybindings layer.
    ///
    /// Returns `Some(Cmd)` if a command should be executed,
    /// `None` if the action was handled without a command.
    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_action(&mut self, action: AppAction, key: &KeyMsg) -> Option<Cmd> {
        match action {
            // =========================================================
            // Application actions
            // =========================================================
            AppAction::Interrupt => {
                // Escape: Abort if processing, otherwise context-dependent
                if self.agent_state != AgentState::Idle {
                    self.last_escape_time = None;
                    let restored = self.restore_queued_messages_to_editor(true);
                    if restored > 0 {
                        self.status_message = Some(format!(
                            "Restored {restored} queued message{}",
                            if restored == 1 { "" } else { "s" }
                        ));
                    } else {
                        self.status_message = Some("Aborting request...".to_string());
                    }
                    return None;
                }
                if key.key_type == KeyType::Esc {
                    let (triggered, cmd) = self.handle_double_escape_action();
                    if triggered {
                        return cmd;
                    }
                }
                // Legacy behavior: Escape when idle does nothing (no quit)
                None
            }
            AppAction::Clear | AppAction::Copy => {
                // Ctrl+C: abort if processing, clear editor if has text, or quit on double-tap
                // Note: Copy and Clear both bound to Ctrl+C - Copy takes precedence in lookup
                // When selection is implemented, Copy should only trigger with active selection
                let now = std::time::Instant::now();
                let double_tap = self
                    .last_ctrlc_time
                    .is_some_and(|last| now.duration_since(last) < CTRLC_QUIT_WINDOW);

                if self.agent_state != AgentState::Idle {
                    if double_tap {
                        return Some(self.quit_cmd());
                    }
                    if let Some(handle) = &self.abort_handle {
                        handle.abort();
                    }
                    self.last_ctrlc_time = Some(now);
                    self.status_message =
                        Some("Aborting request... press Ctrl+C again to quit".to_string());
                    return None;
                }

                // If editor has text, clear it
                let editor_text = self.input.value();
                if !editor_text.is_empty() {
                    self.input.reset();
                    self.pasted_blocks.clear();
                    self.last_ctrlc_time = Some(now);
                    self.status_message = Some("Input cleared".to_string());
                    return None;
                }

                // Editor is empty - check for double-tap to quit
                if double_tap {
                    return Some(self.quit_cmd());
                }
                // Record this Ctrl+C and show hint
                self.last_ctrlc_time = Some(now);
                self.status_message = Some("Press Ctrl+C again to quit".to_string());
                None
            }
            AppAction::PasteImage => {
                match Self::paste_image_from_clipboard() {
                    ImagePaste::Attached(path) => {
                        self.insert_file_ref_path(&path);
                        self.status_message = Some("Image attached".to_string());
                    }
                    ImagePaste::Empty => {
                        self.status_message = Some("No image in the clipboard".to_string());
                    }
                    ImagePaste::Unsupported => {
                        self.status_message = Some(
                            "This build has no clipboard support. Attach the file instead: @path/to/image.png"
                                .to_string(),
                        );
                    }
                }
                None
            }
            AppAction::Exit => {
                // Ctrl+D: Exit only when editor is empty (legacy behavior)
                if self.agent_state == AgentState::Idle && self.input.value().is_empty() {
                    return Some(self.quit_cmd());
                }
                // Editor has text - don't consume, let TextArea handle as delete char forward
                None
            }
            AppAction::Suspend => {
                // Ctrl+Z: Suspend to background (Unix only)
                #[cfg(unix)]
                {
                    use std::process::Command;
                    // Send SIGTSTP to our process. When resumed via `fg`, status() returns
                    // and we show the resumed message.
                    let pid = std::process::id().to_string();
                    let _ = Command::new("kill").args(["-TSTP", &pid]).status();
                    self.status_message = Some("Resumed from background".to_string());
                }
                #[cfg(not(unix))]
                {
                    self.status_message =
                        Some("Suspend not supported on this platform".to_string());
                }
                None
            }
            AppAction::ExternalEditor => {
                // Ctrl+G: Open external editor with current input
                if self.agent_state != AgentState::Idle {
                    self.status_message = Some("Cannot open editor while processing".to_string());
                    return None;
                }
                match self.open_external_editor() {
                    Ok(new_text) => {
                        self.input.set_value(&new_text);
                        self.status_message = Some("Editor content loaded".to_string());
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Editor error: {e}"));
                    }
                }
                None
            }
            AppAction::Help => self.handle_slash_command(SlashCommand::Help, ""),
            AppAction::OpenSettings => self.handle_slash_command(SlashCommand::Settings, ""),

            // =========================================================
            // Models & thinking
            // =========================================================
            AppAction::CycleModelForward => {
                self.cycle_model(1);
                None
            }
            AppAction::CycleModelBackward => {
                self.cycle_model(-1);
                None
            }
            AppAction::CycleThinkingLevel => {
                self.cycle_thinking_level();
                None
            }
            AppAction::CyclePermissionMode => {
                // exit_plan_mode moves the policy behind the cached field's back
                let next = self
                    .tool_policy
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .mode()
                    .next();
                self.tool_policy
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_mode(next);
                self.permission_mode = next;
                self.status_message = Some(format!("Permission mode: {}", self.permission_mode));
                self.resize_conversation_viewport();
                None
            }
            AppAction::SelectModel => {
                self.open_model_selector_configured_only();
                None
            }

            // =========================================================
            // Text input actions
            // =========================================================
            AppAction::Submit => {
                // A trailing backslash continues the line instead of sending.
                // Shift+Enter and Alt+Enter do not survive every terminal, so
                // this is the one way to write a second line that always works.
                if self.take_line_continuation() {
                    return None;
                }
                // Enter: Submit when idle, queue steering when busy
                if self.agent_state != AgentState::Idle {
                    self.expand_pasted_blocks_in_editor();
                    self.queue_input(QueuedMessageKind::Steering);
                    return None;
                }
                let value = self.input.value();
                if !value.trim().is_empty() {
                    let message = self.expand_pasted_blocks(value.trim());
                    return self.submit_message(&message);
                }
                // Don't consume - let TextArea handle Enter if needed
                None
            }
            AppAction::FollowUp => {
                // Alt+Enter: queue a follow-up when busy, insert a newline when
                // idle. Enter is the only key that sends.
                if self.agent_state != AgentState::Idle {
                    self.expand_pasted_blocks_in_editor();
                    self.queue_input(QueuedMessageKind::FollowUp);
                    return None;
                }
                self.input.insert_rune('\n');
                None
            }
            AppAction::NewLine => {
                self.input.insert_rune('\n');
                None
            }

            // =========================================================
            // Cursor movement (history when the box holds a single row)
            // =========================================================
            AppAction::CursorUp => {
                if self.input_is_multi_row() {
                    self.move_cursor_by_row(true);
                } else if self.agent_state == AgentState::Idle {
                    self.navigate_history_back();
                }
                None
            }
            AppAction::CursorDown => {
                if self.input_is_multi_row() {
                    self.move_cursor_by_row(false);
                } else if self.agent_state == AgentState::Idle {
                    self.navigate_history_forward();
                }
                None
            }

            // =========================================================
            // Viewport scrolling
            // =========================================================
            AppAction::PageUp => {
                self.sync_conversation_viewport_for_paging();
                self.conversation_viewport.page_up();
                self.follow_stream_tail = false;
                None
            }
            AppAction::PageDown => {
                // Following the tail already means the bottom, so there is
                // nowhere to page to and nothing to rebuild.
                if self.follow_stream_tail {
                    return None;
                }
                self.sync_conversation_viewport_for_paging();
                self.conversation_viewport.page_down();
                if self.conversation_viewport.at_bottom() {
                    self.follow_stream_tail = true;
                }
                None
            }
            AppAction::ScrollToBottom => {
                self.follow_stream_tail = true;
                self.scroll_to_bottom();
                None
            }

            // =========================================================
            // Autocomplete
            // =========================================================
            AppAction::Tab => {
                if self.agent_state != AgentState::Idle || self.session_picker.is_some() {
                    return None;
                }

                let text = self.input.value();
                if text.trim().is_empty() {
                    self.autocomplete.close();
                    return None;
                }

                let cursor = self.input.cursor_byte_offset();
                let response = self.autocomplete.provider.suggest(&text, cursor);

                if response.items.is_empty() {
                    self.autocomplete.close();
                    return None;
                }

                if response.items.len() == 1
                    && response
                        .items
                        .first()
                        .is_some_and(|item| item.kind == AutocompleteItemKind::Path)
                {
                    let item = response.items[0].clone();
                    self.autocomplete.replace_range = response.replace;
                    self.accept_autocomplete(&item);
                    self.autocomplete.close();
                    return None;
                }

                self.autocomplete.open_with(response);
                None
            }

            // =========================================================
            // Message queue actions
            // =========================================================
            AppAction::Dequeue => {
                let restored = self.restore_queued_messages_to_editor(false);
                if restored == 0 {
                    self.status_message = Some("No queued messages to restore".to_string());
                } else {
                    self.status_message = Some(format!(
                        "Restored {restored} queued message{}",
                        if restored == 1 { "" } else { "s" }
                    ));
                }
                None
            }

            // =========================================================
            // Display actions
            // =========================================================
            AppAction::ToggleThinking => {
                self.thinking_visible = !self.thinking_visible;
                self.message_render_cache.invalidate_all();
                let content = self.build_conversation_content();
                let effective = self.view_effective_conversation_height().max(1);
                self.conversation_viewport.height = effective;
                self.conversation_viewport.set_content(content.trim_end());
                self.status_message = Some(if self.thinking_visible {
                    "Thinking shown".to_string()
                } else {
                    "Thinking hidden".to_string()
                });
                None
            }
            AppAction::ToggleTodoPanel => {
                self.todo_panel_expanded = !self.todo_panel_expanded;
                // The panel takes rows from the conversation, so the viewport
                // has to be re-measured or paging drifts by the panel's height.
                let saved_offset = self.conversation_viewport.y_offset();
                let content = self.build_conversation_content();
                let effective = self.view_effective_conversation_height().max(1);
                self.conversation_viewport.height = effective;
                self.conversation_viewport.set_content(content.trim_end());
                if self.follow_stream_tail {
                    self.conversation_viewport.goto_bottom();
                } else {
                    self.conversation_viewport.set_y_offset(saved_offset);
                }
                None
            }
            AppAction::ExpandTools => {
                let has_collapsed = self
                    .messages
                    .iter()
                    .any(|msg| msg.role == MessageRole::Tool && msg.collapsed);
                self.tools_expanded = has_collapsed || !self.tools_expanded;
                if self.tools_expanded {
                    for msg in &mut self.messages {
                        if msg.role == MessageRole::Tool {
                            msg.collapsed = false;
                        }
                    }
                }
                self.message_render_cache.invalidate_all();
                let content = self.build_conversation_content();
                let effective = self.view_effective_conversation_height().max(1);
                self.conversation_viewport.height = effective;
                self.conversation_viewport.set_content(content.trim_end());
                self.status_message = Some(if self.tools_expanded {
                    "Tool output expanded".to_string()
                } else {
                    "Tool output collapsed".to_string()
                });
                None
            }

            // =========================================================
            // Branch navigation
            // =========================================================
            AppAction::BranchPicker => {
                self.open_branch_picker();
                None
            }
            AppAction::BranchNextSibling => {
                self.cycle_sibling_branch(true);
                None
            }
            AppAction::BranchPrevSibling => {
                self.cycle_sibling_branch(false);
                None
            }

            // Editor-native and picker-specific actions fall through to the
            // focused component when PiApp does not need to intercept them.
            _ => None,
        }
    }

    /// Determine if an action should be consumed (not forwarded to TextArea).
    ///
    /// Some actions need to be consumed even when `handle_action` returns `None`,
    /// to prevent the TextArea from also handling the key.
    pub(super) fn should_consume_action(&self, action: AppAction) -> bool {
        match action {
            // Up/Down move by visual row or walk history; either way the
            // TextArea must not also move by logical line.
            AppAction::CursorUp | AppAction::CursorDown => true,

            // Exit (Ctrl+D) only consumed when editor is empty (otherwise deleteCharForward)
            AppAction::Exit => {
                self.agent_state == AgentState::Idle && self.input.value().is_empty()
            }

            // Viewport scrolling should always be consumed.
            // FollowUp (Alt+Enter) should be consumed so TextArea doesn't insert text.
            // NewLine is handled directly (Shift+Enter / Ctrl+Enter).
            // Interrupt/Clear/Copy are always consumed.
            // Suspend/ExternalEditor are always consumed.
            // Tab is consumed (autocomplete).
            AppAction::PageUp
            | AppAction::PageDown
            | AppAction::ScrollToBottom
            | AppAction::CycleModelForward
            | AppAction::CycleModelBackward
            | AppAction::CycleThinkingLevel
            | AppAction::CyclePermissionMode
            | AppAction::ToggleThinking
            | AppAction::ToggleTodoPanel
            | AppAction::ExpandTools
            | AppAction::FollowUp
            | AppAction::NewLine
            | AppAction::Submit
            | AppAction::Dequeue
            | AppAction::Interrupt
            | AppAction::Clear
            | AppAction::Copy
            | AppAction::PasteImage
            | AppAction::Suspend
            | AppAction::ExternalEditor
            | AppAction::Help
            | AppAction::OpenSettings
            | AppAction::Tab
            | AppAction::BranchPicker
            | AppAction::BranchNextSibling
            | AppAction::BranchPrevSibling
            | AppAction::SelectModel => true,

            // Other actions pass through to TextArea
            _ => false,
        }
    }

    /// Give the viewport the line count and page size paging measures against.
    ///
    /// Stale only while following the tail: `view()` slices freshly built
    /// content there, so `TextDelta` skips the viewport. Once the user has
    /// scrolled away every writer calls `refresh_conversation_viewport()`.
    fn sync_conversation_viewport_for_paging(&mut self) {
        self.conversation_viewport.height = self.view_effective_conversation_height().max(1);
        if self.follow_stream_tail {
            let content = self.build_conversation_content();
            self.conversation_viewport.set_content(content.trim_end());
            // offset drifted behind the streamed lines
            self.conversation_viewport.goto_bottom();
        } else {
            // re-clamp against the new page size
            self.conversation_viewport
                .set_y_offset(self.conversation_viewport.y_offset());
        }
    }
}

fn encode_custom_ui_key(key: &KeyMsg) -> Option<String> {
    let control = |byte: u8| Some(char::from(byte).to_string());
    match key.key_type {
        KeyType::Runes => {
            if key.runes.is_empty() {
                None
            } else {
                let text: String = key.runes.iter().collect();
                if key.alt {
                    Some(format!("\u{1b}{text}"))
                } else {
                    Some(text)
                }
            }
        }
        KeyType::Space => Some(" ".to_string()),
        KeyType::Enter | KeyType::ShiftEnter | KeyType::CtrlEnter | KeyType::CtrlShiftEnter => {
            Some("\r".to_string())
        }
        KeyType::Tab => Some("\t".to_string()),
        KeyType::ShiftTab => Some("\u{1b}[Z".to_string()),
        KeyType::Esc => Some("\u{1b}".to_string()),
        KeyType::Backspace | KeyType::CtrlH => Some("\u{7f}".to_string()),
        KeyType::Up => Some("\u{1b}[A".to_string()),
        KeyType::Down => Some("\u{1b}[B".to_string()),
        KeyType::Right => Some("\u{1b}[C".to_string()),
        KeyType::Left => Some("\u{1b}[D".to_string()),
        KeyType::Home => Some("\u{1b}[H".to_string()),
        KeyType::End => Some("\u{1b}[F".to_string()),
        KeyType::PgUp => Some("\u{1b}[5~".to_string()),
        KeyType::PgDown => Some("\u{1b}[6~".to_string()),
        KeyType::Delete => Some("\u{1b}[3~".to_string()),
        KeyType::Insert => Some("\u{1b}[2~".to_string()),
        KeyType::CtrlA => control(0x01),
        KeyType::CtrlB => control(0x02),
        KeyType::CtrlD => control(0x04),
        KeyType::CtrlE => control(0x05),
        KeyType::CtrlF => control(0x06),
        KeyType::CtrlG => control(0x07),
        KeyType::CtrlJ => control(0x0a),
        KeyType::CtrlK => control(0x0b),
        KeyType::CtrlL => control(0x0c),
        KeyType::CtrlN => control(0x0e),
        KeyType::CtrlO => control(0x0f),
        KeyType::CtrlP => control(0x10),
        KeyType::CtrlQ => control(0x11),
        KeyType::CtrlR => control(0x12),
        KeyType::CtrlS => control(0x13),
        KeyType::CtrlT => control(0x14),
        KeyType::CtrlU => control(0x15),
        KeyType::CtrlV => control(0x16),
        KeyType::CtrlW => control(0x17),
        KeyType::CtrlX => control(0x18),
        KeyType::CtrlY => control(0x19),
        KeyType::CtrlZ => control(0x1a),
        KeyType::Null => control(0x00),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentConfig};
    use crate::config::Config;
    use crate::model::{StreamEvent, Usage};
    use crate::models::ModelEntry;
    use crate::provider::{Context, InputType, Model, ModelCost, Provider, StreamOptions};
    use crate::resources::{ResourceCliOptions, ResourceLoader};
    use crate::session::Session;
    use crate::tools::ToolRegistry;
    use asupersync::channel::mpsc;
    use asupersync::runtime::RuntimeBuilder;
    use futures::stream;
    use std::collections::HashMap;
    use std::path::Path;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::OnceLock;

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

    fn runtime() -> &'static asupersync::runtime::Runtime {
        static RT: OnceLock<asupersync::runtime::Runtime> = OnceLock::new();
        RT.get_or_init(|| {
            RuntimeBuilder::multi_thread()
                .blocking_threads(1, 8)
                .build()
                .expect("build runtime")
        })
    }

    fn runtime_handle() -> asupersync::runtime::RuntimeHandle {
        runtime().handle()
    }

    fn model_entry(
        provider: &str,
        id: &str,
        api_key: Option<&str>,
        headers: HashMap<String, String>,
    ) -> ModelEntry {
        ModelEntry {
            model: Model {
                id: id.to_string(),
                name: id.to_string(),
                api: "openai-completions".to_string(),
                provider: provider.to_string(),
                base_url: "https://example.invalid".to_string(),
                reasoning: true,
                input: vec![InputType::Text],
                cost: ModelCost {
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                },
                context_window: 128_000,
                max_tokens: 8_192,
                headers: HashMap::new(),
            },
            api_key: api_key.map(str::to_string),
            headers,
            auth_header: true,
            compat: None,
            oauth_config: None,
        }
    }

    fn build_test_app_with_event_rx(
        current: ModelEntry,
        available: Vec<ModelEntry>,
    ) -> (PiApp, mpsc::Receiver<PiMsg>) {
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let agent = Agent::new(
            provider,
            ToolRegistry::new(&[], Path::new("."), None),
            AgentConfig::default(),
        );
        let session = Arc::new(asupersync::sync::Mutex::new(Session::in_memory()));
        let resources = ResourceLoader::empty(false);
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
        let (event_tx, event_rx) = mpsc::channel(64);
        let config = Config {
            last_changelog_version: Some(crate::platform::VERSION.to_string()),
            ..Config::default()
        };
        (
            PiApp::new(
                agent,
                session,
                config,
                resources,
                resource_cli,
                Path::new(".").to_path_buf(),
                current,
                Vec::new(),
                available,
                Vec::new(),
                event_tx,
                runtime_handle(),
                false,
                false,
                None,
                Some(KeyBindings::new()),
                Vec::new(),
                Usage::default(),
            ),
            event_rx,
        )
    }

    fn build_test_app(current: ModelEntry, available: Vec<ModelEntry>) -> PiApp {
        let (app, _event_rx) = build_test_app_with_event_rx(current, available);
        app
    }

    #[test]
    fn cycle_model_replaces_stream_options_api_key_and_headers() {
        let mut current_headers = HashMap::new();
        current_headers.insert("x-stale".to_string(), "old".to_string());
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), current_headers);

        let mut next_headers = HashMap::new();
        next_headers.insert("x-provider-header".to_string(), "next".to_string());
        let next = model_entry(
            "openrouter",
            "openai/gpt-4o-mini",
            Some("next-key"),
            next_headers,
        );

        let mut app = build_test_app(current.clone(), vec![current, next]);
        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().api_key = Some("stale-key".to_string());
            guard
                .stream_options_mut()
                .headers
                .insert("x-stale".to_string(), "stale".to_string());
        }

        app.cycle_model(1);

        let mut guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(
            guard.stream_options_mut().api_key.as_deref(),
            Some("next-key")
        );
        assert_eq!(
            guard
                .stream_options_mut()
                .headers
                .get("x-provider-header")
                .map(String::as_str),
            Some("next")
        );
        assert!(
            !guard.stream_options_mut().headers.contains_key("x-stale"),
            "cycling models must replace stale provider headers"
        );
    }

    #[test]
    fn cycle_model_clears_stale_api_key_when_next_model_has_no_key() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut next = model_entry("ollama", "llama3.2", None, HashMap::new());
        next.auth_header = false;
        let mut app = build_test_app(current.clone(), vec![current, next]);
        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().api_key = Some("stale-key".to_string());
            guard
                .stream_options_mut()
                .headers
                .insert("x-stale".to_string(), "stale".to_string());
        }

        app.cycle_model(1);

        let mut guard = app.agent.try_lock().expect("agent lock");
        assert!(
            guard.stream_options_mut().api_key.is_none(),
            "cycling to a keyless model must clear stale API key"
        );
        assert!(
            guard.stream_options_mut().headers.is_empty(),
            "cycling to keyless model with no headers must clear stale headers"
        );
    }

    #[test]
    fn cycle_model_clamps_thinking_level_for_non_reasoning_targets() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"), HashMap::new());
        let mut next = model_entry("ollama", "llama3.2", None, HashMap::new());
        next.auth_header = false;
        next.model.reasoning = false;
        let mut app = build_test_app(current.clone(), vec![current, next]);

        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().thinking_level = Some(crate::model::ThinkingLevel::High);
        }
        {
            let mut guard = app.session.try_lock().expect("session lock");
            guard.header.thinking_level = Some(crate::model::ThinkingLevel::High.to_string());
        }

        app.cycle_model(1);

        let mut agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(
            agent_guard.stream_options_mut().thinking_level,
            Some(crate::model::ThinkingLevel::Off)
        );
        drop(agent_guard);

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(
            session_guard.header.thinking_level.as_deref(),
            Some("off"),
            "session thinking level should clamp alongside the active model"
        );
    }

    #[test]
    fn slash_model_allows_switch_to_keyless_provider_without_api_key() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut keyless = model_entry("ollama", "llama3.2", None, HashMap::new());
        keyless.auth_header = false;
        let mut app = build_test_app(current.clone(), vec![current, keyless]);

        let _ = app.handle_slash_command(SlashCommand::Model, "ollama/llama3.2");

        assert_eq!(app.model, "ollama/llama3.2");
        let mut guard = app.agent.try_lock().expect("agent lock");
        assert!(
            guard.stream_options_mut().api_key.is_none(),
            "keyless model switch must not keep stale API key"
        );
    }

    #[test]
    fn slash_model_rejects_missing_credentials_for_required_provider() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut requires_creds = model_entry("acme-remote", "cloud-model", None, HashMap::new());
        requires_creds.auth_header = true;
        let mut app = build_test_app(current.clone(), vec![current, requires_creds]);

        let _ = app.handle_slash_command(SlashCommand::Model, "acme-remote/cloud-model");

        assert_eq!(app.model, "openai/gpt-4o-mini");
        assert!(
            app.status_message
                .as_deref()
                .is_some_and(|msg| msg.contains("Missing credentials for provider acme-remote")),
            "switch should fail fast when selected provider still lacks credentials"
        );
    }

    #[test]
    fn slash_model_treats_blank_inline_key_as_missing_credentials() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut blank_key = model_entry("acme-remote", "cloud-model", Some("   "), HashMap::new());
        blank_key.auth_header = true;
        let mut app = build_test_app(current.clone(), vec![current, blank_key]);

        let _ = app.handle_slash_command(SlashCommand::Model, "acme-remote/cloud-model");

        assert_eq!(app.model, "openai/gpt-4o-mini");
        assert!(
            app.status_message
                .as_deref()
                .is_some_and(|msg| msg.contains("Missing credentials for provider acme-remote")),
            "blank inline keys must not bypass credential checks"
        );
    }

    #[test]
    fn slash_thinking_clamps_and_avoids_duplicate_history_for_non_reasoning_models() {
        let mut current = model_entry("ollama", "llama3.2", None, HashMap::new());
        current.auth_header = false;
        current.model.reasoning = false;
        let mut app = build_test_app(current.clone(), vec![current]);

        let _ = app.handle_slash_command(SlashCommand::Thinking, "high");
        let _ = app.handle_slash_command(SlashCommand::Thinking, "high");

        let agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(
            agent_guard.stream_options().thinking_level,
            Some(crate::model::ThinkingLevel::Off)
        );
        drop(agent_guard);

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(session_guard.header.thinking_level.as_deref(), Some("off"));
        let thinking_changes = session_guard
            .entries_for_current_path()
            .iter()
            .filter(|entry| matches!(entry, crate::session::SessionEntry::ThinkingLevelChange(_)))
            .count();
        assert_eq!(
            thinking_changes, 1,
            "reapplying the same effective thinking level should not add duplicate history"
        );
    }

    #[test]
    fn cycle_thinking_level_action_updates_runtime_and_session_state() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);

        app.handle_action(
            AppAction::CycleThinkingLevel,
            &KeyMsg::from_type(KeyType::ShiftTab),
        );

        let agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(
            agent_guard.stream_options().thinking_level,
            Some(crate::model::ThinkingLevel::Minimal)
        );
        drop(agent_guard);

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(
            session_guard.header.thinking_level.as_deref(),
            Some("minimal")
        );
        let thinking_changes = session_guard
            .entries_for_current_path()
            .iter()
            .filter(|entry| matches!(entry, crate::session::SessionEntry::ThinkingLevelChange(_)))
            .count();
        assert_eq!(thinking_changes, 1);
        drop(session_guard);

        assert_eq!(
            app.status_message.as_deref(),
            Some("Thinking level: minimal")
        );
    }

    #[test]
    fn cycle_thinking_level_action_reports_unsupported_models() {
        let mut current = model_entry("ollama", "llama3.2", None, HashMap::new());
        current.auth_header = false;
        current.model.reasoning = false;
        let mut app = build_test_app(current.clone(), vec![current]);

        app.handle_action(
            AppAction::CycleThinkingLevel,
            &KeyMsg::from_type(KeyType::ShiftTab),
        );

        let agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(agent_guard.stream_options().thinking_level, None);
        drop(agent_guard);

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(session_guard.header.thinking_level, None);
        let thinking_changes = session_guard
            .entries_for_current_path()
            .iter()
            .filter(|entry| matches!(entry, crate::session::SessionEntry::ThinkingLevelChange(_)))
            .count();
        assert_eq!(thinking_changes, 0);
        drop(session_guard);

        assert_eq!(
            app.status_message.as_deref(),
            Some("Current model does not support thinking")
        );
    }

    #[test]
    fn cycle_thinking_level_action_does_not_persist_without_agent_lock() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        let agent = Arc::clone(&app.agent);
        let _agent_guard = agent.try_lock().expect("agent lock");

        app.handle_action(
            AppAction::CycleThinkingLevel,
            &KeyMsg::from_type(KeyType::ShiftTab),
        );

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(session_guard.header.thinking_level, None);
        let thinking_changes = session_guard
            .entries_for_current_path()
            .iter()
            .filter(|entry| matches!(entry, crate::session::SessionEntry::ThinkingLevelChange(_)))
            .count();
        assert_eq!(thinking_changes, 0);
        drop(session_guard);

        assert_eq!(app.status_message.as_deref(), Some("Agent busy; try again"));
    }

    #[test]
    fn cycle_thinking_level_action_is_consumed_by_app() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"), HashMap::new());
        let app = build_test_app(current.clone(), vec![current]);

        assert!(app.should_consume_action(AppAction::CycleThinkingLevel));
    }

    fn load_one_skill(app: &mut PiApp) {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join("skills").join("my-skill");
        std::fs::create_dir_all(&skill_dir).expect("mkdir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: Answers questions\n---\nBody",
        )
        .expect("write skill");
        app.resources
            .extend_with_paths(
                Path::new("."),
                &crate::resources::ExtensionResourcePaths {
                    skill_paths: vec![temp.path().join("skills")],
                    ..Default::default()
                },
            )
            .expect("load skill");
        assert_eq!(app.resources.skills().len(), 1);
        // keep the fixture alive for the app's lifetime
        std::mem::forget(temp);
    }

    fn header_lines(app: &PiApp) -> Vec<String> {
        strip_ansi(&app.render_header())
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The onboarding rows leave with the first message; one hint row stays
    /// for the session and rotates with what the user just did.
    #[test]
    fn session_hint_row_survives_the_first_message_and_rotates() {
        let current = model_entry("openai", "gpt-5.2", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.set_terminal_size(120, 40);
        assert_eq!(
            app.header_rows(),
            3,
            "onboarding rows before the first message"
        );

        app.messages.push(ConversationMessage::new(
            MessageRole::User,
            "hello".to_string(),
            None,
        ));
        assert_eq!(
            app.session_hint(),
            None,
            "nothing has happened, nothing to hint"
        );
        assert_eq!(app.header_rows(), 2);
        assert_eq!(
            header_lines(&app).len(),
            1,
            "title only: {:?}",
            header_lines(&app)
        );

        app.last_hint_trigger = Some(HintTrigger::Tool);
        assert_eq!(app.session_hint().as_deref(), Some("ctrl+o: detail"));
        assert_eq!(app.header_rows(), 3);
        let lines = header_lines(&app);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[1].contains("ctrl+o: detail"), "{lines:?}");

        app.last_hint_trigger = Some(HintTrigger::Edit);
        app.config.double_escape_action = Some("rewind".to_string());
        assert_eq!(app.session_hint().as_deref(), Some("Esc Esc: rewind"));
        app.config.double_escape_action = Some("tree".to_string());
        assert_eq!(app.session_hint().as_deref(), Some("Esc Esc: tree"));
        app.config.double_escape_action = Some("none".to_string());
        assert_eq!(
            app.session_hint(),
            None,
            "a chord that does nothing gets no hint"
        );
        assert_eq!(app.header_rows(), 2);

        load_one_skill(&mut app);
        assert_eq!(
            app.session_hint().as_deref(),
            Some("/skill:my-skill: run a skill"),
            "with no edit hint to show, a loaded skill is the resting hint"
        );
        app.last_hint_trigger = None;
        assert_eq!(
            app.session_hint().as_deref(),
            Some("/skill:my-skill: run a skill")
        );
        app.last_hint_trigger = Some(HintTrigger::Tool);
        assert_eq!(app.session_hint().as_deref(), Some("ctrl+o: detail"));
    }

    /// `/resources` says what loaded, and names where it looked for what did not.
    #[test]
    fn slash_resources_lists_a_loaded_skill_and_names_the_searched_dirs() {
        let current = model_entry("openai", "gpt-5.2", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        let project_dir = Config::project_dir_in(&app.cwd);

        assert!(app.handle_slash_resources().is_none());
        let status = app.status_message.clone().expect("status names the gap");
        assert!(
            status.starts_with("No skills, prompt templates, themes or extensions loaded"),
            "{status}"
        );
        assert!(
            status.contains(&project_dir.display().to_string()),
            "{status}"
        );
        assert!(app.messages.is_empty());

        load_one_skill(&mut app);
        app.status_message = None;
        assert!(app.handle_slash_resources().is_none());
        let listing = app.messages.last().expect("listing row").content.clone();
        assert!(listing.contains("Skills (1):"), "{listing}");
        assert!(
            listing.contains("/skill:my-skill - Answers questions"),
            "{listing}"
        );
        let prompts_dir = project_dir.join("prompts").display().to_string();
        assert!(
            listing.contains(&format!("Prompt templates: none (looked in"))
                && listing.contains(&prompts_dir),
            "{listing}"
        );
        assert!(
            listing.contains("Agents: discovered by the subagent tool"),
            "{listing}"
        );
        assert!(SlashCommand::parse("/resources").is_some());
        assert!(SlashCommand::help_text().contains("/resources"));
    }

    fn open_capability_prompt(app: &mut PiApp) {
        let request = ExtensionUiRequest::new(
            "req-cap",
            "confirm",
            serde_json::json!({
                "extension_id": "snake",
                "capability": "exec",
                "message": "Needs shell access",
            }),
        )
        .with_extension_id(Some("snake".to_string()));
        app.capability_prompt = Some(CapabilityPromptOverlay::from_request(request));
    }

    fn open_tool_approval(app: &mut PiApp, tool_name: &str, arguments: Value) {
        let (reply, _answer) = oneshot::channel();
        let prompt = ToolApprovalPrompt {
            request: crate::agent::ToolApprovalRequest {
                tool_call_id: "call-1".to_string(),
                tool_name: tool_name.to_string(),
                arguments,
            },
            reply,
        };
        app.tool_approval = Some(ToolApprovalOverlay::new(prompt, &app.cwd));
    }

    fn box_lines(view: &str) -> Vec<String> {
        let plain = strip_ansi(view);
        let start = plain.rfind('\u{256d}').expect("box opens");
        let mut lines = Vec::new();
        for line in plain[start..].lines() {
            lines.push(line.trim_end().to_string());
            if line.trim_start().starts_with('\u{2570}') {
                return lines;
            }
        }
        panic!("box never closes: {plain}")
    }

    /// The most-repeated interaction in the product: approving an edit must
    /// show the edit, and the two approval boxes must look and answer alike.
    #[test]
    fn approval_boxes_share_a_border_number_keys_and_esc() {
        let current = model_entry("openai", "gpt-5.2", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.set_terminal_size(100, 40);

        open_tool_approval(
            &mut app,
            "edit",
            serde_json::json!({
                "path": "src/main.rs",
                "oldText": "let x = 1;",
                "newText": "let x = 10;"
            }),
        );
        let view = app.view();
        let lines = box_lines(&view);
        println!("{}", lines.join("\n"));
        assert!(
            lines.iter().any(|line| line.contains("│ -let x = 1;")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("│ +let x = 10;")),
            "{lines:?}"
        );
        assert!(!view.contains("oldText"), "raw JSON leaked into the modal");
        assert!(
            lines.iter().any(|line| line.contains("1. Yes")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("3. No,")),
            "{lines:?}"
        );
        assert!(strip_ansi(&view).contains("1-3 choose"), "{view}");
        assert_eq!(app.tool_approval_rows(), 1 + 2 + 4 + 2 + 1 + 3 + 1);

        app.update(Message::new(KeyMsg::from_runes(vec!['1'])));
        assert!(app.tool_approval.is_none(), "1 answers the approval");
        open_tool_approval(&mut app, "read", serde_json::json!({"path": "a"}));
        app.update(Message::new(KeyMsg::from_type(KeyType::Esc)));
        assert!(app.tool_approval.is_none(), "Esc answers the approval");

        open_capability_prompt(&mut app);
        let view = app.view();
        let lines = box_lines(&view);
        println!("{}", lines.join("\n"));
        assert!(
            lines[0].starts_with('\u{256d}'),
            "same bordered box: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("snake requests exec")),
            "{lines:?}"
        );
        for (idx, action) in CapabilityAction::ALL.iter().enumerate() {
            let expected = format!("{}. {}", idx + 1, action.label());
            assert!(
                lines.iter().any(|line| line.contains(&expected)),
                "choice {expected} missing: {lines:?}"
            );
        }
        assert!(
            lines
                .iter()
                .any(|line| line.contains("\u{276f} 1. Yes, allow it this once")),
            "{lines:?}"
        );
        assert!(strip_ansi(&view).contains("1-4 choose"), "{view}");
        assert!(strip_ansi(&view).contains("Esc deny once"), "{view}");
        assert_eq!(app.capability_prompt_rows(), 1 + 2 + 2 + 2 + 4 + 2 + 1);

        app.update(Message::new(KeyMsg::from_type(KeyType::Down)));
        assert_eq!(app.capability_prompt.as_ref().map(|p| p.focused), Some(1));
        app.update(Message::new(KeyMsg::from_runes(vec!['3'])));
        assert!(app.capability_prompt.is_none(), "3 answers the prompt");
        open_capability_prompt(&mut app);
        app.update(Message::new(KeyMsg::from_runes(vec!['9'])));
        assert!(app.capability_prompt.is_some(), "9 is not a choice");
        app.update(Message::new(KeyMsg::from_type(KeyType::Esc)));
        assert!(app.capability_prompt.is_none(), "Esc answers the prompt");
    }

    /// A provider failure must not look like /help output.
    #[test]
    fn error_rows_and_notice_rows_render_in_different_styles() {
        let current = model_entry("openai", "gpt-5.2", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.set_terminal_size(100, 40);
        app.messages.push(ConversationMessage::new(
            MessageRole::System,
            "Something happened".to_string(),
            None,
        ));
        app.messages.push(ConversationMessage::new(
            MessageRole::System,
            "\u{2717} Something failed\nthe detail\nNext: /login openai".to_string(),
            None,
        ));
        let view = app.view();
        let notice = app.styles.warning.render("Something happened");
        let error_head = app.styles.error_bold.render("\u{2717} Something failed");
        let error_next = app.styles.error_bold.render("Next: /login openai");
        let detail = app.styles.muted.render("the detail");
        assert_ne!(
            app.styles.warning.render("x"),
            app.styles.error_bold.render("x"),
            "the two styles must differ or the test proves nothing"
        );
        assert!(view.contains(&notice), "notice row in warning style");
        assert!(view.contains(&error_head), "error headline in error_bold");
        assert!(view.contains(&error_next), "next step in error_bold");
        assert!(view.contains(&detail), "detail in muted");
        assert!(
            !view.contains(&app.styles.warning.render("\u{2717} Something failed")),
            "error must not be painted as a notice"
        );
    }

    /// The clock and "esc to interrupt" used to vanish with the first token,
    /// exactly when a turn got long enough to want them.
    #[test]
    fn the_progress_row_keeps_its_clock_after_the_first_token() {
        let current = model_entry("openai", "gpt-5.2", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.set_terminal_size(100, 40);
        app.agent_state = AgentState::Processing;
        app.busy_since = Some(std::time::Instant::now() - std::time::Duration::from_secs(12));

        let waiting_height = app.view_effective_conversation_height();
        assert!(
            app.show_processing_status_spinner(),
            "glyph spins before any token"
        );

        app.current_response.push_str("first token");
        assert!(app.progress_row_visible(), "the row stays");
        assert!(!app.show_processing_status_spinner(), "the glyph stops");
        assert!(
            app.spinner_visible(),
            "ticks keep flowing so the clock advances"
        );
        assert_eq!(
            app.view_effective_conversation_height(),
            waiting_height,
            "the viewport budget must count the row it still draws"
        );

        let view = strip_ansi(&app.view());
        let row = view
            .lines()
            .find(|line| line.contains("Working"))
            .expect("progress row after the first token");
        assert!(row.contains("12s"), "{row}");
        assert!(row.contains("esc to interrupt"), "{row}");
        let frames: Vec<char> =
            "\u{280b}\u{2819}\u{2839}\u{2838}\u{283c}\u{2834}\u{2826}\u{2827}\u{2807}\u{280f}"
                .chars()
                .collect();
        assert!(
            !row.chars().any(|ch| frames.contains(&ch)),
            "spinner glyph must be absent once tokens stream: {row}"
        );
        assert!(row.trim_start().starts_with("Working"), "{row}");
    }

    #[test]
    fn double_escape_action_none_does_not_arm_or_trigger() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.config.double_escape_action = Some("none".to_string());

        let (triggered, cmd) = app.handle_double_escape_action();
        assert!(!triggered);
        assert!(cmd.is_none());
        assert!(app.last_escape_time.is_none());

        let (triggered_again, cmd_again) = app.handle_double_escape_action();
        assert!(!triggered_again);
        assert!(cmd_again.is_none());
        assert!(app.last_escape_time.is_none());
    }

    #[test]
    fn session_header_sync_updates_runtime_model_and_clamps_thinking() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"), HashMap::new());
        let mut next_headers = HashMap::new();
        next_headers.insert("x-provider-header".to_string(), "next".to_string());
        let mut next = model_entry("acme-local", "plain-model", None, next_headers.clone());
        next.auth_header = false;
        next.model.reasoning = false;
        let mut app = build_test_app(current.clone(), vec![current, next.clone()]);

        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().api_key = Some("stale-key".to_string());
            let _ = guard
                .stream_options_mut()
                .headers
                .insert("x-stale".to_string(), "old".to_string());
            guard.stream_options_mut().thinking_level = Some(crate::model::ThinkingLevel::High);
        }
        {
            let mut guard = app.session.try_lock().expect("session lock");
            guard.header.provider = Some(next.model.provider.clone());
            guard.header.model_id = Some(next.model.id);
            guard.header.thinking_level = Some(crate::model::ThinkingLevel::High.to_string());
        }

        app.sync_runtime_selection_from_session_header()
            .expect("sync runtime selection");

        let agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(agent_guard.provider().name(), "acme-local");
        assert_eq!(agent_guard.provider().model_id(), "plain-model");
        assert_eq!(agent_guard.stream_options().api_key, None);
        assert_eq!(agent_guard.stream_options().headers, next_headers);
        assert_eq!(
            agent_guard.stream_options().thinking_level,
            Some(crate::model::ThinkingLevel::Off)
        );
        drop(agent_guard);

        assert_eq!(app.model, "acme-local/plain-model");
        assert_eq!(app.model_entry.model.provider, "acme-local");
        assert_eq!(app.model_entry.model.id, "plain-model");
        let shared_guard = app.model_entry_shared.lock().expect("shared model lock");
        assert_eq!(shared_guard.model.provider, "acme-local");
        assert_eq!(shared_guard.model.id, "plain-model");
        drop(shared_guard);

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(session_guard.header.thinking_level.as_deref(), Some("off"));
        let thinking_changes = session_guard
            .entries_for_current_path()
            .iter()
            .filter(|entry| matches!(entry, crate::session::SessionEntry::ThinkingLevelChange(_)))
            .count();
        assert_eq!(thinking_changes, 1);
    }

    #[test]
    fn session_header_sync_rejects_missing_credentials_without_switching() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut requires_creds = model_entry("acme-remote", "cloud-model", None, HashMap::new());
        requires_creds.auth_header = true;
        let mut app = build_test_app(current.clone(), vec![current, requires_creds]);

        {
            let mut guard = app.session.try_lock().expect("session lock");
            guard.header.provider = Some("acme-remote".to_string());
            guard.header.model_id = Some("cloud-model".to_string());
        }

        let err = app
            .sync_runtime_selection_from_session_header()
            .expect_err("missing credentials should fail closed");
        assert_eq!(
            err,
            "Missing credentials for provider acme-remote. Run /login acme-remote."
        );
        assert_eq!(app.model, "openai/gpt-4o-mini");
        assert_eq!(app.model_entry.model.provider, "openai");
        assert_eq!(app.model_entry.model.id, "gpt-4o-mini");
    }

    #[test]
    fn session_header_sync_ignores_incomplete_model_header_and_keeps_current_runtime() {
        let mut current = model_entry("acme-local", "plain-model", None, HashMap::new());
        current.auth_header = false;
        current.model.reasoning = false;
        let mut app = build_test_app(current.clone(), vec![current]);

        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().thinking_level = Some(crate::model::ThinkingLevel::High);
        }
        {
            let mut guard = app.session.try_lock().expect("session lock");
            guard.header.provider = Some("partial-provider".to_string());
            guard.header.model_id = None;
            guard.header.thinking_level = Some(crate::model::ThinkingLevel::High.to_string());
        }

        app.sync_runtime_selection_from_session_header()
            .expect("incomplete headers should not block runtime sync");

        let agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(agent_guard.provider().name(), "acme-local");
        assert_eq!(agent_guard.provider().model_id(), "plain-model");
        assert_eq!(
            agent_guard.stream_options().thinking_level,
            Some(crate::model::ThinkingLevel::Off)
        );
        drop(agent_guard);

        assert_eq!(app.model, "acme-local/plain-model");
        assert_eq!(app.model_entry.model.provider, "acme-local");
        assert_eq!(app.model_entry.model.id, "plain-model");

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(
            session_guard.header.provider.as_deref(),
            Some("partial-provider")
        );
        assert_eq!(session_guard.header.model_id, None);
        assert_eq!(session_guard.header.thinking_level.as_deref(), Some("off"));
    }

    #[test]
    fn custom_extension_key_handler_queues_rune_input_when_active() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.extension_custom_active = true;

        let consumed = app.handle_custom_extension_key(&KeyMsg::from_char('w'));
        assert!(consumed, "custom overlay should consume key input");
        assert_eq!(
            app.extension_custom_key_queue.pop_front().as_deref(),
            Some("w")
        );
    }

    #[test]
    fn custom_extension_key_handler_preserves_ctrl_c_for_global_exit() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.extension_custom_active = true;

        let consumed = app.handle_custom_extension_key(&KeyMsg::from_type(KeyType::CtrlC));
        assert!(
            !consumed,
            "Ctrl+C should remain available for normal global handling"
        );
        assert!(app.extension_custom_key_queue.is_empty());
    }

    #[test]
    fn quit_cmd_schedules_shutdown_when_event_queue_is_full() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"), HashMap::new());
        let (mut app, mut event_rx) = build_test_app_with_event_rx(current.clone(), vec![current]);
        app.event_tx
            .try_send(PiMsg::System("busy".to_string()))
            .expect("fill bounded event channel");

        let _ = app.quit_cmd();

        let (first, second) = runtime().block_on(async {
            let cx = asupersync::Cx::for_request();
            let first = event_rx.recv(&cx).await.expect("first queued message");
            let second = event_rx.recv(&cx).await.expect("shutdown message");
            (first, second)
        });

        assert!(matches!(first, PiMsg::System(text) if text == "busy"));
        assert!(matches!(second, PiMsg::UiShutdown));
    }

    fn busy_app() -> PiApp {
        let current = model_entry("openai", "gpt-4o-mini", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.set_terminal_size(80, 24);
        app.agent_state = AgentState::Processing;
        app
    }

    #[test]
    fn typing_during_a_turn_reaches_the_editor_and_enter_queues_steering() {
        let mut app = busy_app();

        for key in "steer me".chars() {
            let _ = BubbleteaModel::update(&mut app, Message::new(KeyMsg::from_char(key)));
        }
        assert_eq!(app.input.value(), "steer me");
        assert!(
            BubbleteaModel::view(&app).contains("steer me"),
            "the input box must stay drawn while the agent works"
        );

        let _ = BubbleteaModel::update(&mut app, Message::new(KeyMsg::from_type(KeyType::Enter)));
        let queue = app.message_queue.lock().expect("queue lock");
        assert_eq!(queue.steering_len(), 1);
        assert_eq!(queue.steering_front().map(String::as_str), Some("steer me"));
        drop(queue);
        assert_eq!(app.input.value(), "");
    }

    #[test]
    fn interrupt_returns_queued_text_and_keeps_half_typed_input() {
        let mut app = busy_app();
        app.input.set_value("queued");
        app.handle_action(AppAction::Submit, &KeyMsg::from_type(KeyType::Enter));
        app.input.set_value("half typed");

        app.handle_action(AppAction::Interrupt, &KeyMsg::from_type(KeyType::Esc));

        assert_eq!(app.input.value(), "queued\n\nhalf typed");
        assert_eq!(
            app.message_queue.lock().expect("queue lock").steering_len(),
            0
        );
    }

    #[test]
    fn large_paste_collapses_to_a_placeholder_and_submits_in_full() {
        let mut app = busy_app();
        app.agent_state = AgentState::Idle;
        let pasted = (0..200)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");

        let handled = app.handle_paste_event(
            &KeyMsg::from_runes(pasted.chars().collect::<Vec<_>>()).with_paste(),
        );

        assert!(handled);
        assert_eq!(app.input.value(), "[pasted 200 lines]");
        let submitted = app.expand_pasted_blocks(&app.input.value());
        assert_eq!(submitted, pasted);
        assert!(app.pasted_blocks.is_empty());
    }

    #[test]
    fn carriage_return_pastes_still_collapse() {
        let mut app = busy_app();
        app.agent_state = AgentState::Idle;
        let pasted = (0..200)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\r");

        assert!(app.handle_paste_event(
            &KeyMsg::from_runes(pasted.chars().collect::<Vec<_>>()).with_paste()
        ));

        assert_eq!(app.input.value(), "[pasted 200 lines]");
        assert_eq!(
            app.expand_pasted_blocks(&app.input.value()),
            pasted.replace('\r', "\n")
        );
    }

    #[test]
    fn small_paste_is_left_alone() {
        let mut app = busy_app();
        app.agent_state = AgentState::Idle;
        let pasted = "not a path\nsecond line";

        assert!(
            !app.handle_paste_event(
                &KeyMsg::from_runes(pasted.chars().collect::<Vec<_>>()).with_paste()
            ),
            "short pastes must fall through to the textarea"
        );
        assert!(app.pasted_blocks.is_empty());
    }

    #[test]
    fn scroll_to_bottom_rearms_stream_follow() {
        let mut app = busy_app();
        app.handle_action(AppAction::PageUp, &KeyMsg::from_type(KeyType::PgUp));
        assert!(!app.follow_stream_tail);

        app.handle_action(
            AppAction::ScrollToBottom,
            &KeyMsg::from_type(KeyType::Down).with_alt(),
        );

        assert!(app.follow_stream_tail);
        assert!(app.is_at_bottom());
    }

    fn pgup() -> KeyMsg {
        KeyMsg::from_type(KeyType::PgUp)
    }

    fn pgdn() -> KeyMsg {
        KeyMsg::from_type(KeyType::PgDown)
    }

    fn scrolled_transcript_app(line: &str, count: usize) -> PiApp {
        let mut app = busy_app();
        for i in 0..count {
            app.messages.push(ConversationMessage {
                role: MessageRole::Assistant,
                content: format!("{line} {i}"),
                thinking: None,
                collapsed: false,
            });
        }
        app.scroll_to_bottom();
        app
    }

    #[test]
    fn paging_up_and_back_down_lands_on_the_offset_it_started_from() {
        let mut app = scrolled_transcript_app("reply", 120);
        let bottom = app.conversation_viewport.y_offset();
        assert!(bottom > 0, "the transcript must be taller than one page");

        app.handle_action(AppAction::PageUp, &pgup());
        let one_page_up = app.conversation_viewport.y_offset();
        app.handle_action(AppAction::PageUp, &pgup());
        let two_pages_up = app.conversation_viewport.y_offset();
        assert!(two_pages_up < one_page_up && one_page_up < bottom);

        app.handle_action(AppAction::PageDown, &pgdn());
        assert_eq!(app.conversation_viewport.y_offset(), one_page_up);
        app.handle_action(AppAction::PageDown, &pgdn());
        assert_eq!(app.conversation_viewport.y_offset(), bottom);
        assert!(app.follow_stream_tail, "the tail re-arms at the bottom");
    }

    #[test]
    fn paging_reuses_the_synced_transcript_instead_of_rebuilding_it() {
        let mut app = scrolled_transcript_app("reply", 120);
        app.handle_action(AppAction::PageUp, &pgup());
        let lines = app.conversation_viewport.total_line_count();

        // no viewport sync, which every real writer does
        app.messages.push(ConversationMessage {
            role: MessageRole::Assistant,
            content: "never synced".to_string(),
            thinking: None,
            collapsed: false,
        });
        app.handle_action(AppAction::PageUp, &pgup());

        assert_eq!(
            app.conversation_viewport.total_line_count(),
            lines,
            "paging must not rebuild the transcript"
        );
    }

    #[test]
    fn a_chunk_streamed_while_scrolled_up_is_in_the_page_that_follows() {
        let mut app = scrolled_transcript_app("reply", 120);
        app.handle_action(AppAction::PageUp, &pgup());
        let before = app.conversation_viewport.total_line_count();

        let _ = BubbleteaModel::update(
            &mut app,
            Message::new(PiMsg::TextDelta("streamed tail\n".repeat(3))),
        );

        assert!(
            app.conversation_viewport.total_line_count() > before,
            "a chunk arriving while scrolled away must reach the pager"
        );
        app.conversation_viewport.goto_bottom();
        assert!(strip_ansi(&app.conversation_viewport.view()).contains("streamed tail"));
    }

    #[test]
    fn a_resize_mid_scroll_rewraps_the_page() {
        let mut app = scrolled_transcript_app(&"wide ".repeat(12), 40);
        app.handle_action(AppAction::PageUp, &pgup());
        let before = app.conversation_viewport.total_line_count();

        app.set_terminal_size(40, 24);
        app.handle_action(AppAction::PageUp, &pgup());

        assert!(
            app.conversation_viewport.total_line_count() > before,
            "a narrower terminal must rewrap the transcript the pager walks"
        );
    }

    #[test]
    fn long_thinking_wraps_instead_of_cutting_at_one_hundred_characters() {
        let mut app = busy_app();
        let thinking = "reasoning ".repeat(40);
        app.messages.push(ConversationMessage {
            role: MessageRole::Assistant,
            content: "answer".to_string(),
            thinking: Some(thinking.clone()),
            collapsed: false,
        });

        let frame = strip_ansi(&BubbleteaModel::view(&app));
        let rendered: String = frame
            .lines()
            .filter(|line| line.contains("reasoning"))
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");

        assert!(rendered.len() > 100, "thinking must not be cut to a clause");
        assert!(rendered.starts_with("Thinking: reasoning"));
        for line in frame.lines() {
            assert!(
                line.chars().count() <= app.term_width,
                "thinking must wrap to the viewport: {line:?}"
            );
        }
    }

    fn is_quit(cmd: Option<Cmd>) -> bool {
        cmd.is_some_and(|cmd| {
            cmd.execute()
                .is_some_and(|msg| msg.is::<bubbletea::QuitMsg>())
        })
    }

    /// A real terminal only reaches `update()` through `terminal_event_message`,
    /// so the regression is only visible when the raw crossterm event drives it.
    fn ctrl_c_event() -> KeyMsg {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let msg = super::super::terminal_event_message(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )))
        .expect("ctrl+c must reach the model as a key message, not an interrupt");
        msg.downcast_ref::<KeyMsg>()
            .expect("ctrl+c must stay a KeyMsg")
            .clone()
    }

    #[test]
    fn ctrl_c_mid_turn_aborts_and_keeps_the_session() {
        let mut app = busy_app();
        app.input.set_value("half typed");

        let key = ctrl_c_event();
        assert_eq!(key.key_type, KeyType::CtrlC);
        assert!(
            !is_quit(BubbleteaModel::update(&mut app, Message::new(key))),
            "the first Ctrl+C during a turn must abort, not quit"
        );
        assert_eq!(app.input.value(), "half typed");
        assert!(
            app.status_message
                .as_deref()
                .is_some_and(|msg| msg.contains("Ctrl+C again")),
            "the abort must say how to quit: {:?}",
            app.status_message
        );
    }

    #[test]
    fn second_ctrl_c_mid_turn_quits() {
        let mut app = busy_app();
        let _ = BubbleteaModel::update(&mut app, Message::new(ctrl_c_event()));

        assert!(
            is_quit(BubbleteaModel::update(
                &mut app,
                Message::new(ctrl_c_event())
            )),
            "a second Ctrl+C inside the window must quit"
        );
    }

    #[test]
    fn ctrl_c_mid_turn_after_the_window_aborts_again() {
        let mut app = busy_app();
        let _ = BubbleteaModel::update(&mut app, Message::new(ctrl_c_event()));
        app.last_ctrlc_time = Some(std::time::Instant::now() - CTRLC_QUIT_WINDOW * 2);

        assert!(
            !is_quit(BubbleteaModel::update(
                &mut app,
                Message::new(ctrl_c_event())
            )),
            "a late second Ctrl+C must abort again rather than quit"
        );
    }

    #[test]
    fn ctrl_c_when_idle_still_quits_on_the_second_press() {
        let mut app = busy_app();
        app.agent_state = AgentState::Idle;

        assert!(!is_quit(BubbleteaModel::update(
            &mut app,
            Message::new(ctrl_c_event())
        )));
        assert!(
            is_quit(BubbleteaModel::update(
                &mut app,
                Message::new(ctrl_c_event())
            )),
            "idle Ctrl+C must keep its double-tap quit"
        );
    }

    #[test]
    fn shift_tab_after_exit_plan_mode_advances_from_the_policy_not_the_stale_field() {
        let policy: SharedToolPolicy = {
            let mut policy = crate::tool_policy::ToolPolicy::default();
            policy.set_mode(PermissionMode::Plan);
            Arc::new(std::sync::RwLock::new(policy))
        };

        let current = model_entry("openai", "gpt-4o-mini", Some("key"), HashMap::new());
        let mut app = build_test_app(current.clone(), vec![current]);
        app.set_terminal_size(80, 24);
        app.attach_tool_policy(Arc::clone(&policy), Arc::default());
        assert_eq!(app.permission_mode, PermissionMode::Plan);

        runtime().block_on({
            let policy = Arc::clone(&policy);
            async move {
                let registry = ToolRegistry::new(&["exit_plan_mode"], Path::new("."), None);
                registry.bind_permission_mode(policy);
                registry
                    .get("exit_plan_mode")
                    .expect("exit_plan_mode is registered")
                    .execute("call-1", serde_json::json!({ "plan": "ship it" }), None)
                    .await
                    .expect("approved exit leaves plan mode");
            }
        });
        assert_eq!(
            policy
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .mode(),
            PermissionMode::Default
        );

        let _ =
            BubbleteaModel::update(&mut app, Message::new(KeyMsg::from_type(KeyType::ShiftTab)));

        assert_eq!(
            app.permission_mode,
            PermissionMode::AcceptEdits,
            "shift+tab must step off the mode the policy holds, not the cached one"
        );
        assert_eq!(
            policy
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .mode(),
            PermissionMode::AcceptEdits,
            "the cycle must not write a tighter mode into the policy that gates tool calls"
        );
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
}
