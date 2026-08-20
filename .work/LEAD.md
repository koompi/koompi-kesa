---
updated: 2026-08-20
---

# Lead — run 5, TUI UX/UI

## Goal

Runs 1-4 made KESA shippable and made its context and its file edits visible. Run 5 is the next
gap, and it is one shape: **KESA already runs multi-agent orchestration and the screen does not
show it.** `src/subagents.rs` fans work out to child processes in single, parallel and chain modes,
each child can carry its own model and provider, and each streams progress back — and the TUI
collapses all of it into `⣾ Running subagent ...`, on a frame whose default border is invisible at
1.51:1 and whose idle input box is red.

The user's stated direction: KESA as smart multi-agent orchestration with a good workflow, like
Claude Code but with different AI providers working together. The orchestration is the product; this
run makes it the thing you watch.

**Gate.** The run ends when all four hold, each demonstrated in a real terminal, not a snapshot:

1. Three agents on three different providers run in parallel from one prompt, and the panel shows
   three rows updating independently, each naming the provider it actually reached.
2. Esc cancels them, the rows go to cancelled, and no child process survives.
3. The finished delegation reads the same in scrollback and after `--continue` as it did live.
4. Every built-in theme passes the contrast gate, and the gate fails when a palette regresses.

Clause 4 is J61's. Clauses 1-3 are J63's. J62 is not in the gate — it is friction, and it lands or
it does not.

Plan: `~/.claude/plans/modular-wiggling-stallman.md`. Audit: `.work/AUDIT.md`, 23 deltas, every one
landing in exactly one job. Run 4's closed artifacts are archived in `.work/.trash-run4/`.

## Ledger

| id | runner | state | verdict |
|----|--------|-------|---------|
| J61 | herdr, worktree | ready | — |
| J62 | herdr, worktree | ready | — |
| J63 | lead's pane, no worktree | live | — |

## Live now

- **J63** — lead's pane `w1:p1`, no worktree. Owns `subagents.rs`, `interactive.rs`,
  `interactive/{agent,tool_render,view,tests}.rs`, `tools.rs`, a new `interactive/agent_roster.rs`,
  and `tests/snapshots/tui_snapshot__*`. In progress: `SubagentResult` carries
  `provider`/`resolved_model`/`tokens`/`cost`/`elapsed_ms`/`started`, all three constructors
  updated, `cargo check --lib` clean. Next is `ingest_child_event`.
- J61 and J62 not yet dispatched.

## Decided

- **Both a live panel and inline history**, not one or the other. The panel is the safety surface
  while children run; the inline block is what scrollback and `--continue` keep. User's choice among
  three options.
- **`subagent` goes on by default** (D30). A panel for a tool nobody has enabled is dead UI. User
  chose this over the two safer alternatives knowing it widens the model's default reach: children
  spawn with read/bash/edit/write and `tool_policy.rs:120-128` cannot scope the call by argument.
  The panel plus Esc is the control surface. Depth 3 and 8-way fan-out still bound it.
- **KESA gets its own palette**, not a contrast patch on VS Code's. User's choice. Anchored on the
  example already in `docs/themes.md:56-79`, which reads as KESA and mostly clears the floors —
  except its `ui.border` of `#1f2937`, which fails at about 1.4:1 and must not be copied.
- **Provider shown as text, never as a colour tint.** The provider set is open-ended; a colour per
  provider cannot be kept accessible. Rejected before J61 was written so it does not get invented.
- **The TUI reads the subagent wire schema through its own reader type**, not by making
  `SubagentResult` public and `Deserialize`. `pi.subagent.progress.v1` is a versioned contract and
  the consumer picks the fields it renders; `session_isolation: &'static str` also cannot round-trip
  through `Deserialize`. Cost is a small duplicated struct in `agent_roster.rs`.
- **`src/interactive/view.rs` is J63's alone.** It is the most contended file in the tree, so every
  delta touching it — D39, D41, D42, D43, D44, D46 — went to J63 regardless of theme. That is why
  D45 and D46 are split across two jobs, and why J62 copies `bordered_box`'s output rather than
  re-exporting it.
- **J61 and J62 can run concurrently with J63** because `normalize_snapshot` strips ANSI
  (`tests/tui_snapshot.rs:203-207`): a palette change cannot move a snapshot. Only J63 moves
  `tests/snapshots/`. If J61 or J62 sees one move, that is a stop condition, not a fix.
- **The contrast gate applies to built-in themes only.** Enforcing it on user themes in
  `~/.kesa/agent/themes/` would break a working setup on upgrade. Written into J61's stop
  conditions.
- **Scheduler stays as it is.** Two model-issued `subagent` calls still serialise, because
  `ToolEffects::process()` is a barrier (`src/tools.rs:56`). Only the intra-call fan-out is
  concurrent, and that is the case the panel is for. Relaxing the barrier is a scheduler change with
  its own risk surface; not this run.
- **Run 4's `.work/` archived, not deleted**, to `.work/.trash-run4/`. Its gate closed and all 19
  jobs landed; the history is in the worklog and in git.

## Next action

Commit the contracts, then dispatch J61 and J62 into herdr panes with worktrees, then continue J63
at `ingest_child_event` in `src/subagents.rs`.
