---
updated: 2026-08-20
---

# Audit — run 5, TUI UX/UI

Measured against Claude Code's TUI and against KESA's own capabilities. The recurring shape:
**KESA does the work and does not show it.** The orchestration engine, the resolved provider per
child, and the theme's own syntax colours all exist and none reach the screen.

Source: the live UX audit at `c96af163`
(`~/.claude/projects/-home-userx-workspace/memory/worklog/archive/koompi-kesa-tui-ux-AUDIT.md`)
plus two code traces run 2026-08-20. Every row cites both sides.

| id | title | ours | target | effort | why |
|----|-------|------|--------|--------|-----|
| D30 | Subagent tool is off in a default install | `src/tools.rs:5331` `default_enabled: false`; `DEFAULT_TOOL_NAMES` built only from true entries at `:5372-5384` | Claude Code ships `Task` enabled | S | The orchestration engine is unreachable without `--tools ...,subagent`. Everything below it is dead UI until this flips |
| D31 | N parallel children collapse into one line | `current_tool: Option<String>` `src/interactive.rs:2544-2546`; single `writeln!` at `src/interactive/view.rs:1039-1045` | per-agent rows | L | `RequestMode::Parallel` (`src/subagents.rs:88-111`) fans out up to 8 children; the screen shows `⣾ Running subagent ...` |
| D32 | Child updates are dropped before they render | `Batcher::push_tool_update` keeps one un-keyed `pending_tool_update` slot, `src/interactive/agent.rs:245` | coalesce per agent | M | All N children share one `on_update` callback (`src/subagents.rs:93-104`), so the last writer wins and the panel would flicker between agents |
| D33 | A child's provider is never known | `SubagentResult.model` is raw frontmatter, `src/subagents.rs:765-788`; no provider field; resolution happens in the child at `src/app.rs:443-470` | show `anthropic/opus-5` | M | The child's `message_end` already carries `AssistantMessage{provider, model, usage}` (`src/model.rs:59-77`) and `ingest_child_event` (`src/subagents.rs:962-1000`) throws it away |
| D34 | Finished delegation renders as a markdown dump | `render_results` emits `## {agent}` headings, `src/subagents.rs:867-885` | per-agent summary rows | M | Scrollback loses the shape the live view had. `format_tool_result_body` (`src/interactive/tool_render.rs:42-72`) already has the schema-dispatch hook |
| D35 | Subagent call header degrades to a bare name | `tool_primary_arg` maps `"subagent"` to `["agent","name","prompt"]`, `src/interactive/tool_render.rs:133` | `subagent · 3 agents · parallel` | S | Schema field is `task`, not `prompt` (`src/subagents.rs:167-168`), and `tasks`/`chain` mode has no top-level `agent` |
| D36 | Default border is invisible | `#3c3c3c` on `#1e1e1e` = **1.51:1**, `src/theme.rs:339`; solarized `#073642` on `#002b36` = **1.28:1** | ≥3:1, WCAG non-text | M | Every box outline in the shipped default theme is at the edge of visibility |
| D37 | Footer and hints fail AA | `muted` `#6a6a6a` on `#1e1e1e` = **3.08:1**, `src/theme.rs:338` | ≥4.5:1 | S | `muted` carries the footer, every hint and every collapsed-detail line |
| D38 | `themes/*.json` is dead weight | `Theme::resolve_spec` short-circuits `dark`/`light`/`solarized` to hardcoded constructors, `src/theme.rs:127-153`, `:333-419`; `build.rs:13-37` embeds four unrelated assets and no themes | one source | S | Two copies of every palette, already drifting; editing the JSON changes nothing at runtime |
| D39 | Idle input box renders as a failure | `render_input` maps `ThinkingLevel::XHigh\|Max` → `error_bold`, `src/interactive/view.rs:1272-1280`; Max is the default | border = permission mode | M | A red box around an empty prompt reads as an error. Permission mode, the higher-stakes signal, gets no colour at all (`src/tool_policy.rs:13-50`) |
| D40 | Fenced code looks like inline code | both get one colour and nothing else, `src/theme.rs:236-239` | background + margin | S | `syntax.keyword/number/comment/function` are loaded from the theme and never used by the renderer |
| D41 | Speakers do not agree | user `›` (`view.rs:6`), tool `⎿` (`:7`), assistant a bare six-space indent (`view.rs:1526-1529`) | one convention | S | Nothing marks where the model starts speaking |
| D42 | `97% ctx` reads backwards | `short: format!("{percent}% ctx")`, `src/interactive/view.rs:1469` | `97% ctx left` | S | The short form drops the word carrying the meaning; a nearly empty context looks nearly full |
| D43 | Footer permanently spends a slot on a fsync policy | `Persist: balanced` always shown, `src/interactive/view.rs:1474-1477`, from `SESSION_DURABILITY_MODE` (`src/session.rs:2341-2377`) | show only when abnormal | S | An internal autosave knob with no in-app explanation and no way to change it from the TUI |
| D44 | Header advertises a panel that does not exist | `format!("{tools_key}: tools")`, `src/interactive/view.rs:1178` | `ctrl+o: detail` | S | `AppAction::ExpandTools` toggles verbosity of tool output **and** thinking blocks (`src/interactive/keybindings.rs:1132-1156`) |
| D45 | Autocomplete opens with nothing selected | `selected = None` in `AutocompleteState::new`, `src/interactive/state.rs:150`, and on every re-open at `:163-187` | row 0 preselected | S | No highlight and no description until the first arrow key, though all 25 builtins carry one |
| D46 | Autocomplete list shifts as you arrow | description pushed inside the item loop, `src/interactive/view.rs:1859-1862`, while `visible_count` counts items only (`:1821-1825`) | fixed description row | S | The box grows by a row and every entry below the cursor moves down one |
| D47 | Three slash commands never autocomplete | `/rewind`, `/context`, `/template` in `/help` but absent from `builtin_slash_commands`, `src/autocomplete.rs:254-800` | all commands complete | S | Documented and undiscoverable |
| D48 | Model picker ignores the model you are on | `selected: 0` at `src/model_selector.rs:62` and `:220`; `open_model_selector` never seeks it, `src/interactive/model_selector_ui.rs:54-69` | open scrolled to current | M | 111 models sorted provider-then-id (`model_selector.rs:57-58`), so amazon-bedrock fills the first four pages |
| D49 | Model picker disagrees with itself and with the rest of the UI | two counters, `(1-30 of 111)` at `model_selector_ui.rs:271-278` vs `(111/111)` at `:283-291`; `"─".repeat(50)` at `:217` where every other overlay uses `bordered_box` (`view.rs:113-131`) | one counter, rounded box | M | Plus a pointer to a README section that does not exist (`:195-198`; `README.md` has no such text) |
| D50 | `/help`'s dash column breaks on its longest entries | hand-aligned to column 21, `src/interactive/commands.rs:91-130`; overruns at `:97` (col 30), `:98` (24), `:99` (34), `:117` (26) | generated column | S | Four rows out of column, and `textwrap` at `view.rs:1579-1584` destroys the rest on a narrow pane |
| D51 | Three different answers for "how do I insert a newline" | box says `\+Enter` (`view.rs:1288-1292`), `/help` says `Shift+Enter (Ctrl+Enter on Windows)` (`commands.rs:126`), a fixture says `Shift+Enter \| Alt+Enter` (`view.rs:2692`) | one answer, stated twice | S | Two of the three are on the same screen |
| D52 | User-visible strings still say "Pi" | `commands.rs:120`, `:2459`, `:2466`, `:2470`, `:2978-2982`; `autocomplete.rs:731`; `main.rs:4658`; `interactive/agent.rs:723`; `share.rs:114-115`; `auth.rs:4113,4116,4225,4228`; `error.rs:811,892,923`; `tools.rs:5333` | KESA | S | `APP_LABEL` is already `"KESA"` (`view.rs:9`); these are leftovers from the fork |

## ALREADY BUILT

Do not rebuild these.

- **The orchestration engine.** Single, parallel (`buffer_unordered`, concurrency 1-8) and chain
  modes with `{previous}` substitution, agent discovery from user and project scopes, per-agent
  model / reasoning / tools / skills, depth guard at 3, output cap at 256KB — all of
  `src/subagents.rs`. D30-D35 are about **surfacing** it, not building it.
- **Cancellation.** `ChildProcessGuard` (`src/subagents.rs:660-711`) kills children on drop and the
  loop sets `SubagentStatus::Cancelled` on `cx.checkpoint()` failure (`:621-631`). Esc already
  works. The panel renders that state; it adds no cancellation machinery.
- **Schema dispatch in the renderer.** `todos_from_details` / `todo_checkbox_block`
  (`src/interactive/tool_render.rs:76-95`) already dispatch on `details["schema"]` and win over the
  generic text path at `:47-49`. The subagent renderers copy this; they do not invent it.
- **Live/replay parity.** `format_tool_output` is what replays a resumed session
  (`src/interactive/conversation.rs:206`), so any renderer added there lands in `--continue` free.
- **Per-message and per-turn collapse.** `tools_expanded` plus per-message `collapsed`
  (`view.rs:1522-1576`) already exist; the audit's "tool results not collapsed by default" finding
  is closed — `tui_tool_auto_collapsed.snap` proves it.
- **Transcript wrapping.** Closed on this branch at `1cc9eb3e`.
- **Error text and hints.** Closed on this branch at `0990408a`.
- **Rounded box primitive.** `bordered_box` (`view.rs:113-131`) exists and is used by the
  autocomplete dropdown, the tool-approval overlay and the input box. D49 adopts it; it is not new.

## Not closing in this run

- Multiple *model-issued* `subagent` calls still serialise. `ToolEffects::process()`
  (`src/subagents.rs:221`) is in `ToolEffects::BARRIER` (`src/tools.rs:56`), so
  `plan_tool_effect_batches` (`src/agent.rs:3068-3080`) never overlaps two of them. Only the
  intra-call fan-out is concurrent. Relaxing it is a scheduler change, not a UI one.
- `tool_policy.rs:120-128` returns no match target for `subagent`, so pattern rules cannot scope it
  by argument. D30 widens the default reach knowing this; the panel plus Esc is the control surface.
  A `Subagent(agent-name:*)` target kind is its own job.
