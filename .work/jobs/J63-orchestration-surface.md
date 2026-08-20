---
id: J63
title: The orchestration surface, and the frame it sits in
deltas: D30 D31 D32 D33 D34 D35 D39 D41 D42 D43 D44 D46
state: live — lead's pane, no worktree
---

# J63 — The orchestration surface, and the frame it sits in

Run in the lead's own pane. No worktree: the lead verifies here anyway, and this job owns the files
the other two are forbidden from opening.

## Files owned

`src/subagents.rs`, `src/interactive/agent_roster.rs` (new), `src/interactive.rs`,
`src/interactive/agent.rs`, `src/interactive/tool_render.rs`, `src/interactive/view.rs`,
`src/interactive/tests.rs`, `src/tools.rs`, `tests/tui_snapshot.rs`,
`tests/snapshots/tui_snapshot__*`.

## Do

1. **D33** — capture the child's resolved provider, model, tokens and cost in `SubagentResult`, from
   the `message_end` event `ingest_child_event` already parses. *(fields landed; ingest pending)*
2. **D32** — key the batcher's pending tool-update slot per agent instead of one global slot.
3. **D31** — `AgentRoster` state and `render_agent_panel` above the input box.
4. **D34** — `pi.subagent.result.v1` renders as per-agent rows in the transcript, not a markdown dump.
5. **D35** — fix `tool_primary_arg`'s subagent field names and emit the mode and agent count.
6. **D30** — flip `default_enabled`, reword the `prompt_description` (closes its half of D52).
7. **D39** — input border encodes permission mode; thinking level keeps its label text only.
8. **D41** — one speaker convention across user, assistant and tool rows.
9. **D42, D43, D44** — footer context wording, persistence segment only when abnormal, header hint.
10. **D46** — autocomplete description moves to a fixed row so the list stops shifting.

## Acceptance

Held to the same standard as J61 and J62; the lead states it in `LEAD.md` at the seam rather than
here, since this job's verification is the run's gate.

## Stop conditions

- Reaching into `src/theme.rs`, `themes/`, `src/interactive/state.rs`, `src/autocomplete.rs`,
  `src/interactive/commands.rs`, `src/model_selector.rs` or `src/interactive/model_selector_ui.rs`.
  Those are J61's and J62's.
- Changing `ToolEffects::process()` or the scheduler to overlap two model-issued subagent calls.
  Out of scope for the run; see `AUDIT.md`.
