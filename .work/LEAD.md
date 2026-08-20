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
| J61 | herdr `j61`, worktree `.wt/j61` | verified, landed | `6c0c071a`, merged `fefd67e0`. **Verified by me in its own worktree**: `theme_contrast` 4/0 + 2 report-only ignored; tightest margin solarized `ui.border` at 3.57:1 against a 3.0 floor. Diff is exactly its 7 owned files. `tui_snapshot` did not move, as predicted. **Its report's prose says the old dark border measured 1.74:1; it is 1.51:1** — the table is machine-generated and correct, the narrative number is not |
| J62 | herdr `j62`, worktree `.wt/j62` | verified, landed | `7b2415ee`, merged `9345d418`. Diff is exactly its 9 owned files; `view.rs` untouched. Its clippy finding reproduced here: **clippy cannot run on this repo at all** — see Decided |
| J63 | lead's pane, no worktree | verified, landed | `3b1442eb` + `dcce0d31`. lib 7159/2, the two baseline names; `tui_snapshot` 147/0; `theme_contrast` 4/0. **Demonstrated live** in pane `w1:pB` — see Decided |

## Live now

Empty. All three jobs returned and are merged into `fix-tui-audit-bugs`.

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
- **The live run found a provider-layer bug and I fixed it rather than filing it.** KESA sent
  `entry.model.max_tokens` as the output budget, and for any model reporting `max_tokens ==
  context_window` (e.g. `openrouter/openai/gpt-oss-20b:free`, both 131072) every request failed
  HTTP 400 before generating a token. That is what "KESA is going in circles" looked like from
  outside: children died, the orchestrator retried, nothing progressed. `Model::usable_max_tokens`
  now clamps to half the window. Three tests in `provider.rs`; the previously-dead model answers.
- **Two findings left open, both provider-layer, neither this run's scope.** The orchestrator
  retries a failing delegation without a stop condition, which is agent-loop policy. And the
  tool-approval overlay shows a fan-out as truncated raw JSON (`view.rs:2418`) where it should say
  "3 agents: counter, greeter, namer"; `summarize_tool_arguments` (`state.rs:634`) is J62's file.
- **clippy cannot run on this repo.** `rust-toolchain.toml` pins `nightly-2026-07-05`, which has no
  clippy component, and falling back to stable fails because `.cargo/config.toml` sets nightly-only
  `-Z` rustflags unconditionally. Found independently by J62 and by me. `cargo fmt` and
  `cargo check --all-targets` are the substitutes. Pre-existing; not introduced here.
- **Run 4's `.work/` archived, not deleted**, to `.work/.trash-run4/`. Its gate closed and all 19
  jobs landed; the history is in the worklog and in git.

## Gate

1. **Three agents in parallel, each naming the provider it reached** — **met, live**. Pane `w1:pB`,
   three OpenRouter models, three independent rows updating separately. Provider is one gateway,
   not three, because that is the credential held; the per-child field is read the same way either
   way and the snapshot covers three distinct providers.
2. **Esc cancels, rows go to cancelled, no child survives** — **not demonstrated live.** The drop
   guard is pre-existing (`subagents.rs:660-711`) and the panel renders `Cancelled`, but no live
   Esc was driven. Unclosed.
3. **Scrollback and `--continue` read as the run looked** — **met for scrollback**, live: the
   `Subagent(3 agents, parallel)` block with per-agent model, elapsed and outcome.
   `--continue` follows from `format_tool_output` being the replay path
   (`conversation.rs:206`) but was not driven.
4. **Every built-in theme passes the contrast gate, and the gate fails on regression** — **met**.

## Next action

Close gate clause 2: drive Esc against a live fan-out and confirm no child process survives. Then
decide whether the two provider-layer findings below get their own jobs.
