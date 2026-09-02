use super::commands::{model_entry_matches, resolve_model_key_from_default_auth};
use super::*;
use crate::models::model_requires_configured_credential;

impl PiApp {
    fn normalize_model_key(entry: &ModelEntry) -> (String, String) {
        let canonical_provider =
            crate::provider_metadata::canonical_provider_id(entry.model.provider.as_str())
                .unwrap_or(entry.model.provider.as_str());
        (
            canonical_provider.to_ascii_lowercase(),
            entry.model.id.to_ascii_lowercase(),
        )
    }

    fn unique_model_count(models: &[ModelEntry]) -> usize {
        models
            .iter()
            .map(Self::normalize_model_key)
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    fn available_models_with_credentials(&self) -> Vec<ModelEntry> {
        let auth = crate::auth::AuthStorage::load(crate::config::Config::auth_path()).ok();
        let mut provider_has_credential: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        let mut filtered = Vec::new();
        for entry in &self.available_models {
            let provider = entry.model.provider.as_str();
            let canonical = crate::provider_metadata::canonical_provider_id(provider)
                .unwrap_or(provider)
                .to_ascii_lowercase();
            let requires_configured_credential = model_requires_configured_credential(entry);
            let has_inline_key = entry
                .api_key
                .as_ref()
                .is_some_and(|key| !key.trim().is_empty());
            let has_auth_key = auth.as_ref().is_some_and(|storage| {
                *provider_has_credential
                    .entry(canonical.clone())
                    .or_insert_with(|| storage.resolve_api_key(&canonical, None).is_some())
            });
            if !requires_configured_credential || has_inline_key || has_auth_key {
                filtered.push(entry.clone());
            }
        }

        filtered.sort_by_key(Self::normalize_model_key);
        filtered.dedup_by(|left, right| model_entry_matches(left, right));
        filtered
    }

    /// Open the model selector overlay.
    pub fn open_model_selector(&mut self) {
        if self.agent_state != AgentState::Idle {
            self.status_message = Some("Cannot switch models while KESA is working".to_string());
            return;
        }

        if self.available_models.is_empty() {
            self.status_message = Some("No models available".to_string());
            return;
        }

        let current = crate::model_selector::ModelKey::from_entry(&self.model_entry);
        let mut overlay = crate::model_selector::ModelSelectorOverlay::new(
            &self.available_models,
            Some(&current),
        );
        overlay.set_max_visible(super::overlay_max_visible(self.term_height));
        self.model_selector = Some(overlay);
    }

    pub(super) fn open_model_selector_configured_only(&mut self) {
        if self.agent_state != AgentState::Idle {
            self.status_message = Some("Cannot switch models while KESA is working".to_string());
            return;
        }

        if self.available_models.is_empty() {
            self.status_message = Some("No models available".to_string());
            return;
        }

        let filtered = self.available_models_with_credentials();
        if filtered.is_empty() {
            self.status_message = Some(
                "No model has credentials configured. Run /login <provider> to add one."
                    .to_string(),
            );
            return;
        }

        let current = crate::model_selector::ModelKey::from_entry(&self.model_entry);
        let mut overlay =
            crate::model_selector::ModelSelectorOverlay::new(&filtered, Some(&current));
        overlay.set_configured_only_scope(Self::unique_model_count(&self.available_models));
        overlay.set_max_visible(super::overlay_max_visible(self.term_height));
        self.model_selector = Some(overlay);
    }

    /// Handle keyboard input while the model selector is open.
    pub fn handle_model_selector_key(&mut self, key: &KeyMsg) -> Option<Cmd> {
        let selector = self.model_selector.as_mut()?;

        match key.key_type {
            KeyType::Up => selector.select_prev(),
            KeyType::Down => selector.select_next(),
            KeyType::Runes if key.runes == ['k'] => selector.select_prev(),
            KeyType::Runes if key.runes == ['j'] => selector.select_next(),
            KeyType::PgDown => selector.select_page_down(),
            KeyType::PgUp => selector.select_page_up(),
            KeyType::Backspace => selector.pop_char(),
            KeyType::Runes => selector.push_chars(key.runes.iter().copied()),
            KeyType::Enter => {
                let selected = selector.selected_item().cloned();
                self.model_selector = None;
                if let Some(selected) = selected {
                    self.apply_model_selection(&selected);
                } else {
                    self.status_message = Some("No model selected".to_string());
                }
                return None;
            }
            KeyType::Esc | KeyType::CtrlC => {
                self.model_selector = None;
                self.status_message = Some("Model selector cancelled".to_string());
            }
            _ => {} // consume all other input while selector is open
        }

        None
    }

    /// Apply a model selection from the model selector overlay.
    fn apply_model_selection(&mut self, selected: &crate::model_selector::ModelKey) {
        // Find the matching ModelEntry from available_models
        let entry = self
            .available_models
            .iter()
            .find(|e| {
                e.model.provider.eq_ignore_ascii_case(&selected.provider)
                    && e.model.id.eq_ignore_ascii_case(&selected.id)
            })
            .cloned();

        let Some(next) = entry else {
            self.status_message = Some(format!("Model {} not found", selected.full_id()));
            return;
        };

        if model_entry_matches(&next, &self.model_entry) {
            self.status_message = Some(format!("Already using {}", selected.full_id()));
            return;
        }

        let resolved_key_opt = resolve_model_key_from_default_auth(&next);
        if model_requires_configured_credential(&next) && resolved_key_opt.is_none() {
            self.status_message = Some(format!(
                "Missing credentials for provider {}. Run /login {}.",
                next.model.provider, next.model.provider
            ));
            return;
        }

        let provider_impl = match providers::create_provider(&next, self.extensions.as_ref()) {
            Ok(p) => p,
            Err(err) => {
                self.status_message = Some(err.to_string());
                return;
            }
        };

        if let Err(message) = self.switch_active_model(
            &next,
            provider_impl,
            resolved_key_opt.as_deref(),
            "selector",
        ) {
            self.status_message = Some(message);
            return;
        }
        self.status_message = Some(format!("Switched model: {}", self.model));
    }

    /// Render the model selector overlay.
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_model_selector(
        &self,
        selector: &crate::model_selector::ModelSelectorOverlay,
    ) -> String {
        use std::fmt::Write;

        let mut rows = vec![self.styles.title.render("Select a model")];
        if selector.configured_only() {
            rows.push(self.styles.muted.render(
                "Showing models with credentials configured. Run /login <provider> to add more.",
            ));
        }
        rows.push(String::new());

        let query = selector.query();
        let search_line = if query.is_empty() {
            if selector.configured_only() {
                ">".to_string()
            } else {
                "> (type to filter)".to_string()
            }
        } else {
            format!("> {query}")
        };
        rows.push(self.styles.muted.render(&search_line));
        rows.push(String::new());

        if selector.filtered_len() == 0 {
            rows.push(self.styles.muted_italic.render("No matching models"));
        } else {
            let offset = selector.scroll_offset();
            let visible_count = selector.max_visible().min(selector.filtered_len());
            let end = (offset + visible_count).min(selector.filtered_len());

            let current_full = format!(
                "{}/{}",
                self.model_entry.model.provider, self.model_entry.model.id
            );

            for idx in offset..end {
                let Some(key) = selector.item_at(idx) else {
                    continue;
                };

                let starts_new_provider = idx == 0
                    || selector
                        .item_at(idx - 1)
                        .is_none_or(|prev| !prev.provider.eq_ignore_ascii_case(&key.provider));
                if starts_new_provider {
                    rows.push(self.styles.muted_bold.render(&key.provider));
                }

                let is_selected = idx == selector.selected_index();
                let prefix = if is_selected { ">" } else { " " };
                let full = key.full_id();
                let is_current = full.eq_ignore_ascii_case(&current_full);
                let marker = if is_current { " *" } else { "" };
                let mut row = format!("{prefix} {full}{marker}");
                if crate::models::provider_credentials_are_unchecked(&key.provider) {
                    row.push_str(" (credentials not checked)");
                }
                if let Some(badge) = selector
                    .routing_evidence_for(key)
                    .and_then(crate::model_routing::ModelRoutingEvidence::row_badge)
                {
                    row.push(' ');
                    row.push_str(&badge);
                }
                let rendered = if is_selected {
                    self.styles.accent_bold.render(&row)
                } else if is_current {
                    self.styles.accent.render(&row)
                } else {
                    self.styles.muted.render(&row)
                };
                rows.push(rendered);
            }

            let mut counter_parts = Vec::new();
            if selector.filtered_len() > visible_count {
                counter_parts.push(format!(
                    "{}-{} of {}",
                    offset + 1,
                    end,
                    selector.filtered_len()
                ));
            }
            if selector.configured_only() {
                counter_parts.push(format!(
                    "{} of {} shown",
                    selector.filtered_len(),
                    selector.source_total()
                ));
            }
            if !counter_parts.is_empty() {
                rows.push(String::new());
                rows.push(
                    self.styles
                        .muted
                        .render(&format!("({})", counter_parts.join(" \u{b7} "))),
                );
            }

            if let Some(selected) = selector.selected_item()
                && let Some(entry) = self.available_models.iter().find(|entry| {
                    entry
                        .model
                        .provider
                        .eq_ignore_ascii_case(&selected.provider)
                        && entry.model.id.eq_ignore_ascii_case(&selected.id)
                })
            {
                rows.push(String::new());
                rows.push(
                    self.styles
                        .muted
                        .render(&format!("Model name: {}", entry.model.name)),
                );

                if let Some(evidence) = selector.routing_evidence_for(selected) {
                    let summary = evidence
                        .row_badge()
                        .unwrap_or_else(|| format!("[{}]", evidence.decision.short_label()));
                    rows.push(self.styles.muted.render(&format!("Routing: {summary}")));
                }
            }
        }

        let width = box_width(self.term_width);
        let mut output = String::from("\n");
        for line in bordered_box(rows.iter().map(String::as_str), width, &self.styles.border) {
            let _ = writeln!(output, "  {line}");
        }
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("↑/↓/j/k/PgUp/PgDn: navigate  Enter: select  Esc: cancel  * = current")
        );
        output
    }
}

// copy of interactive::view's bordered_box/fit_to_width/box_width; that module is J63's, don't re-export
const MIN_BOX_WIDTH: usize = 16;

fn box_width(term_width: usize) -> usize {
    let width = term_width.saturating_sub(4);
    if width < MIN_BOX_WIDTH {
        MIN_BOX_WIDTH
    } else {
        width
    }
}

fn fit_to_width(line: &str, width: usize) -> String {
    let visible = lipgloss::width(line);
    if visible == width {
        return line.to_string();
    }
    if visible < width {
        let mut out = line.to_string();
        out.push_str(&" ".repeat(width - visible));
        return out;
    }

    let mut out = String::with_capacity(line.len());
    let mut taken = 0usize;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            out.push(ch);
            for esc in chars.by_ref() {
                out.push(esc);
                if esc.is_ascii_alphabetic() || esc == '\u{7}' {
                    break;
                }
            }
            continue;
        }
        let cell = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if taken + cell > width {
            break;
        }
        out.push(ch);
        taken += cell;
    }
    if out.contains('\u{1b}') {
        out.push_str("\u{1b}[0m");
    }
    if taken < width {
        out.push_str(&" ".repeat(width - taken));
    }
    out
}

fn bordered_box<'a>(
    content: impl IntoIterator<Item = &'a str>,
    width: usize,
    style: &lipgloss::Style,
) -> Vec<String> {
    let width = width.max(4);
    let inner = width - 4;
    let rule = "\u{2500}".repeat(width - 2);

    let mut lines = vec![style.render(&format!("\u{256d}{rule}\u{256e}"))];
    let left = style.render("\u{2502}");
    let right = style.render("\u{2502}");
    for row in content {
        let row = fit_to_width(row, inner);
        lines.push(format!("{left} {row} {right}"));
    }
    lines.push(style.render(&format!("\u{2570}{rule}\u{256f}")));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentConfig};
    use crate::model::{StreamEvent, Usage};
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

    fn runtime_handle() -> asupersync::runtime::RuntimeHandle {
        static RT: OnceLock<asupersync::runtime::Runtime> = OnceLock::new();
        RT.get_or_init(|| {
            RuntimeBuilder::multi_thread()
                .blocking_threads(1, 8)
                .build()
                .expect("build runtime")
        })
        .handle()
    }

    fn model_entry(provider: &str, id: &str, api_key: Option<&str>) -> ModelEntry {
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
            headers: HashMap::new(),
            auth_header: true,
            compat: None,
            oauth_config: None,
        }
    }

    fn build_test_app(current: ModelEntry, available: Vec<ModelEntry>) -> PiApp {
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
        let (event_tx, _event_rx) = mpsc::channel(64);
        let config = Config {
            last_changelog_version: Some(crate::platform::VERSION.to_string()),
            ..Config::default()
        };
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
        )
    }

    #[test]
    fn apply_model_selection_replaces_stream_options_api_key_and_headers() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"));
        let mut next = model_entry("openrouter", "openai/gpt-4o-mini", Some("next-key"));
        next.headers
            .insert("x-provider-header".to_string(), "next".to_string());

        let mut app = build_test_app(current.clone(), vec![current, next.clone()]);

        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().api_key = Some("stale-token".to_string());
            guard
                .stream_options_mut()
                .headers
                .insert("x-stale".to_string(), "stale".to_string());
        }

        app.apply_model_selection(&crate::model_selector::ModelKey {
            provider: next.model.provider.clone(),
            id: next.model.id,
        });

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
            "switching models must replace stale provider headers"
        );
    }

    #[test]
    fn apply_model_selection_clears_stale_api_key_when_next_model_has_no_key() {
        let current = model_entry("openai", "gpt-4o-mini", Some("old-key"));
        let mut next = model_entry("ollama", "llama3.2", None);
        next.auth_header = false;
        let mut app = build_test_app(current.clone(), vec![current, next.clone()]);

        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().api_key = Some("stale-token".to_string());
        }

        app.apply_model_selection(&crate::model_selector::ModelKey {
            provider: next.model.provider.clone(),
            id: next.model.id,
        });

        let mut guard = app.agent.try_lock().expect("agent lock");
        assert!(
            guard.stream_options_mut().api_key.is_none(),
            "switching to a keyless model must clear stale API key"
        );
    }

    #[test]
    fn apply_model_selection_clamps_thinking_level_for_non_reasoning_targets() {
        let current = model_entry("openai", "gpt-5.2", Some("old-key"));
        let mut next = model_entry("ollama", "llama3.2", None);
        next.auth_header = false;
        next.model.reasoning = false;
        let mut app = build_test_app(current.clone(), vec![current, next.clone()]);

        {
            let mut guard = app.agent.try_lock().expect("agent lock");
            guard.stream_options_mut().thinking_level = Some(crate::model::ThinkingLevel::High);
        }
        {
            let mut guard = app.session.try_lock().expect("session lock");
            guard.header.thinking_level = Some(crate::model::ThinkingLevel::High.to_string());
        }

        app.apply_model_selection(&crate::model_selector::ModelKey {
            provider: next.model.provider.clone(),
            id: next.model.id,
        });

        let mut agent_guard = app.agent.try_lock().expect("agent lock");
        assert_eq!(
            agent_guard.stream_options_mut().thinking_level,
            Some(crate::model::ThinkingLevel::Off)
        );
        drop(agent_guard);

        let session_guard = app.session.try_lock().expect("session lock");
        assert_eq!(
            session_guard.header.thinking_level.as_deref(),
            Some("high"),
            "only the runtime clamps; the header keeps the level to restore"
        );
    }

    #[test]
    fn configured_only_selector_includes_keyless_ready_models() {
        let mut keyless = model_entry("ollama", "llama3.2", None);
        keyless.auth_header = false;

        let mut requires_creds = model_entry("acme-remote", "cloud-model", None);
        requires_creds.auth_header = true;

        let mut app = build_test_app(keyless.clone(), vec![keyless, requires_creds]);
        app.open_model_selector_configured_only();

        let selector = app
            .model_selector
            .as_ref()
            .expect("configured-only selector should open when keyless models are ready");
        let mut ids = Vec::new();
        for idx in 0..selector.filtered_len() {
            if let Some(item) = selector.item_at(idx) {
                ids.push(item.full_id());
            }
        }

        assert!(
            ids.iter().any(|id| id == "ollama/llama3.2"),
            "keyless local model must be considered ready"
        );
        assert!(
            !ids.iter().any(|id| id == "acme-remote/cloud-model"),
            "credentialed providers without configured auth should not appear"
        );
    }

    #[test]
    fn configured_only_selector_keeps_unknown_keyless_provider_models() {
        let mut unknown_keyless = model_entry("acme-local", "dev-model", None);
        unknown_keyless.auth_header = false;
        let mut unknown_requires = model_entry("acme-remote", "cloud-model", None);
        unknown_requires.auth_header = true;

        let mut app = build_test_app(
            unknown_keyless.clone(),
            vec![unknown_keyless, unknown_requires],
        );
        app.open_model_selector_configured_only();

        let selector = app
            .model_selector
            .as_ref()
            .expect("unknown keyless model should keep selector available");
        let mut ids = Vec::new();
        for idx in 0..selector.filtered_len() {
            if let Some(item) = selector.item_at(idx) {
                ids.push(item.full_id());
            }
        }

        assert!(ids.iter().any(|id| id == "acme-local/dev-model"));
        assert!(!ids.iter().any(|id| id == "acme-remote/cloud-model"));
    }

    #[test]
    fn configured_only_selector_renders_degraded_routing_evidence() {
        let mut keyless = model_entry("ollama", "llama3.2", None);
        keyless.auth_header = false;

        let mut requires_creds = model_entry("acme-remote", "cloud-model", None);
        requires_creds.auth_header = true;

        let mut app = build_test_app(keyless.clone(), vec![keyless.clone(), requires_creds]);
        app.open_model_selector_configured_only();

        let metrics = crate::model_routing::ProviderRoutingMetrics::new("ollama", 9_500, 12)
            .for_model("llama3.2")
            .with_latency_p95_ms(3_000)
            .with_error_rate(0.01);
        let evidence =
            crate::model_routing::evaluate_model_routing(crate::model_routing::RoutingEvaluation {
                model: &keyless,
                metrics: Some(&metrics),
                now_ms: 10_000,
                configured_only_scope: true,
                user_override: false,
                thresholds: crate::model_routing::ProviderRoutingThresholds {
                    degraded_latency_ms: 2_000,
                    ..crate::model_routing::ProviderRoutingThresholds::default()
                },
            });
        app.model_selector
            .as_mut()
            .expect("configured-only selector should open")
            .set_routing_evidence([evidence]);

        let selector = app.model_selector.as_ref().expect("selector");
        let rendered = app.render_model_selector(selector);
        assert!(rendered.contains("ollama/llama3.2 * [degraded: latency]"));
        assert!(rendered.contains("Routing: [degraded: latency]"));
        assert!(!rendered.contains("acme-remote/cloud-model"));
    }

    #[test]
    fn opens_scrolled_to_the_current_model_grouped_by_provider() {
        let mut available: Vec<ModelEntry> = (0..109)
            .map(|i| model_entry("amazon-bedrock", &format!("model-{i:03}"), None))
            .collect();
        available.push(model_entry("openai", "gpt-4o", None));
        let current = model_entry("zzz-provider", "zzz-model", None);
        available.push(current.clone());
        assert_eq!(available.len(), 111);

        let mut app = build_test_app(current, available);
        app.open_model_selector();

        let selector = app
            .model_selector
            .as_ref()
            .expect("selector should open with 111 models");
        assert_eq!(
            selector.selected_item().unwrap().full_id(),
            "zzz-provider/zzz-model"
        );
        assert_eq!(selector.scroll_offset(), 0);

        let rendered = app.render_model_selector(selector);
        println!("{rendered}");
        assert!(rendered.contains("> zzz-provider/zzz-model *"));
        assert!(rendered.contains('\u{256d}'));
        assert!(rendered.contains('\u{2570}'));
        assert!(rendered.contains("amazon-bedrock"));
    }
}
