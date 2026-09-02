# Interactive Interface (TUI)

KESA's interactive mode provides a full-screen terminal UI for chatting, streaming
responses, and managing sessions.

## Layout

### Header
Shows high-level session context (current model, status, hints). Exact contents
may vary as the UI evolves.

### Conversation View
The main area shows the conversation history.
- **User messages**: Highlighted in accent color.
- **Assistant messages**: Rendered as Markdown.
- **Thinking blocks**: Muted and italicized.
- **Tool calls/results**: Structured blocks showing tool execution and output.

### Editor
The input area at the bottom.
- **Single-line + multi-line editing** (see shortcuts below).
- **Autocomplete** for `@file` references, `/commands`, and resource names.
- Paste and editing behaviors follow the configured keybindings.

### Footer
Displays session statistics and status.
- Token usage (input/output) and estimated cost.
- Editor mode hints (Single-line vs Multi-line).
- Current status messages.

## Display Controls

| Action | Shortcut | Description |
|--------|----------|-------------|
| **Toggle Thinking** | `Ctrl+T` | Hide/show thinking blocks to reduce noise. |
| **Scroll History** | `PageUp` / `PageDown` | Scroll conversation view. |

## Operator Telemetry

Setting `KESA_PERF_TELEMETRY=1` enables bounded in-process timing samples for
operator diagnosis during long swarm runs. The samples are timing-only: they do
not include prompts, tool arguments, provider payloads, transcript text, or
credentials. Runtime summaries use the `pi.operator_tail_latency.v1` schema and
include p95, p99, and p999 windows for provider streaming, local tools,
extension hostcalls, session append/index work, and TUI render phases.
Frame-budget snapshots use the `pi.tui.frame_budget.v1` schema for large
conversation, tool preview, model selector, branch picker, and tree selector
surfaces. The deterministic regression evidence for those snapshots is recorded
in `docs/evidence/large-session-tui-frame-budget.json`.

This telemetry is an operator handoff aid, not release performance evidence.
Release-facing speed claims still require the measured artifacts and freshness
gates under `tests/perf/reports/`, `docs/evidence/`, and the perf SLI contract.

## Navigation & Overlays

### Keyboard shortcuts (`/hotkeys`)
Use `/hotkeys` to see the current shortcut list (including any user overrides
from `~/.kesa/agent/keybindings.json`).

## Slash commands

Type a slash command into the editor (prefix with `/`) and press Enter.

`/help` is the authoritative, in-app list. The table below is the same set, from
`SLASH_COMMANDS` in `src/interactive/commands.rs`.

| Command | Description |
|---------|-------------|
| `/help` (`/h`, `/?`) | Show this help message. |
| `/login [provider]` | Sign in to a provider; without one, show the status table. |
| `/logout [provider]` | Remove stored credentials. |
| `/clear` (`/cls`) | Clear the conversation history. |
| `/model [id\|provider/id]` (`/m`) | Open the model selector, or switch directly. |
| `/thinking [level]` (`/think`, `/t`) | Set the thinking level (off/minimal/low/medium/high/xhigh/max). |
| `/scoped-models [patterns\|clear]` (`/scoped`) | Show or set the models Ctrl+P cycles through. |
| `/history` (`/hist`) | Show the input history. |
| `/export [path]` | Export the session to an HTML file on this machine. |
| `/session` (`/info`) | Show the session path, token use and cost. |
| `/settings` | Open the settings selector. |
| `/theme [name]` | List or switch themes. |
| `/resume` (`/r`) | Pick and resume a previous session. |
| `/new` | Start a new session. |
| `/copy` (`/cp`) | Copy the last model message to the clipboard. |
| `/name <name>` | Set the session display name. |
| `/hotkeys` (`/keys`, `/keybindings`) | Show the keyboard shortcuts. |
| `/changelog` | Show the changelog entries. |
| `/tree` | Show the session branch tree. |
| `/fork [id\|index]` | Branch from an earlier message of yours (default: the last one). |
| `/rewind` (`/undo`) | Undo a turn's file edits, its transcript, or both (also Esc Esc). |
| `/context` (`/ctx`) | Break the context window down by what is filling it. |
| `/compact [notes]` | Compact older context, with optional instructions. |
| `/reload` | Reload skills, prompts, themes and extensions from disk. |
| `/resources` | List the skills, templates, themes and extensions that loaded. |
| `/template <name> [args]` | Expand a prompt template by name. |
| `/share` | Upload the session to a secret GitHub gist and show the URL. |
| `/mcp` | List the MCP servers extensions registered (KESA does not connect to them). |
| `/exit` (`/quit`, `/q`) | Exit KESA. |

### Model selection
- Use `/model` to switch models (by `provider/id` or fuzzy match).
- `Ctrl+L` opens the model selector. `Ctrl+P` cycles forward through the models
  `/scoped-models` selects, `Ctrl+Shift+P` backward.

### Session Picker (`/resume`)
Browse and resume previous sessions without restarting KESA.
- `Enter`: Select session
- `Ctrl+D`: Delete session (with confirmation)

### Tree Navigator (`/tree`)
Visualize the conversation branching structure.
- `Up` / `Down`: Navigate nodes
- `Enter`: Switch to selected node (forks if not a leaf)
- `Ctrl+U`: Toggle user-only view (hides assistant/tool noise)

### Settings (`/settings`)
Change configuration on the fly (Thinking levels, themes, message delivery mode).

## Message Queue

When KESA is busy generating a response or running tools, you can still type.

- **Queue Steering (`Enter`)**: Sends your message as a steering interrupt after
  the current step completes.
- **Queue Follow-up (`Alt+Enter`)**: Adds your message to the follow-up queue to
  be processed when the agent becomes idle.
- **Restore queued messages (`Alt+Up`)**: Pull queued messages back into the
  editor (useful if you queued something by mistake).

The queue is visible above the editor when not empty.
