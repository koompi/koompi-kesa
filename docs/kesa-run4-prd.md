# KESA Run 4: Undo and Awareness

Status: draft for paneout
Baseline: `main` at `53e1424c`, clean, pushed
Author: lead

## Why this run

Runs 1 through 3 made KESA shippable and gave it a terminal that looks right.
v0.3.0 installs from a one-liner and from crates.io, the tool loop is proven against a real provider, and the chrome no longer copies another product's marks.

What is missing now is not features in the catalogue sense.
KESA already has read, write, edit, bash, ls, grep, glob, web_fetch, web_search, subagents, todo, background bash, image paste, ACP for Zed, an extension runtime with MCP, and four permission modes.
Measured against Claude Code, Codex and Kimi, the gaps that remain are all the same shape: **the user cannot see what the agent is doing to their context, and cannot undo what it did to their files.**

That is the run. It ends when a user can rewind a bad turn, see what is eating the context window, and trust that the sandbox is either on or loudly off.

## Non-goals

Named so nobody builds them by accident.

- Output styles, vim mode, a persistent shell pane, and cloud task handoff. Real gaps, none of them on the path to trust.
- Feature-for-feature parity with pi_agent_rust. It is not on this machine, so it cannot be diffed. That work starts with a clone, not with a job file.
- The history rewrite and the crates.io publish. Both are done or blocked on the user, and neither is engineering.
- Rewriting the upstream program-management corpus under `docs/planning/`, `docs/evidence/` and `docs/contracts/`. Run 3 scoped this and rejected it: four test suites and a dozen scripts read those files by path, so removal is a test-rewrite job wearing a cleanup costume.

## W1: Checkpoint and rewind

The single highest-value gap. It is the safety net that makes every other permission decision cheaper, because a user who can undo will accept edits without reading each one.

### Model

A checkpoint is not a snapshot of the tree.
Snapshotting a repo before every turn is too slow and too large, and most turns touch three files.

Capture on write instead.
Before any of `write`, `edit` or `apply_patch` mutates a path, if that path has no pre-image recorded for the current turn, copy the pre-image into the checkpoint store.
A file created by the turn records a tombstone rather than a blob, so rewind deletes it.

The unit is the turn, not the tool call.
A turn is one user message and everything the agent did in response, which is the granularity a user thinks in when they say "undo that".

### Store

`~/.kesa/agent/checkpoints/<session-id>/`, with a per-turn manifest mapping workspace-relative path to blob hash, and blobs content-addressed so an unchanged file re-touched across five turns costs one copy.

Retention is bounded on two axes, turn count and total bytes, whichever binds first.
Garbage collection runs on session close and on store open, because sessions do not always close cleanly.

### Restore

Three restore scopes, and the UI must make the choice explicit rather than picking for the user:

- files only, leaving the conversation intact so the agent knows what it tried
- conversation only, rolling the transcript back without touching the tree
- both

Restore is a write, so it goes through the same `allowed_roots` enforcement as any other write.
A manifest entry that resolves outside the workspace roots is refused, not restored. That is the trust boundary, and it is the reason restore does not simply shell out to `cp`.

### The hole, and saying so

Bash is not covered and cannot be.
A command can delete a directory, push a branch or drop a table, and no pre-image copy will bring that back.

The design decision is to cover the file tools honestly rather than to cover bash badly.
The rewind overlay states which turns contained bash calls, so a user rewinding past one knows the file restore is partial.
Claiming otherwise is worse than the gap.

### Surface

Esc Esc opens a rewind overlay listing turns newest first: the user message truncated to a line, the count of files the turn changed, and a marker for turns that ran bash.
The overlay machinery already exists. `BranchPicker` and `ToolApprovalOverlay` are the pattern to copy.

`/rewind` reaches the same overlay for anyone who does not find the chord.

## W2: Context breakdown

The footer shows a context percentage. That number tells a user they are in trouble without telling them why, which is the least actionable form of a warning.

`/context` renders the window as its parts: system prompt, tool schemas, project memory files, conversation history, file reads, tool outputs.

The design content is where the numbers come from.
Do not build a second accounting path beside the one that already computes the percentage at `view.rs:197`.
Tag each contribution with its origin where it enters the context, then aggregate at render time from the session that is already in memory.
One source of truth, two renderings.

The same aggregation is what compaction should be choosing on, so the tagging is worth more than the overlay it ships with.

## W3: Workspace roots, unified

`allowed_roots` is enforced throughout `tools.rs` and there is no way for a user to add one.
Multi-repo work is out, which for a monorepo user is most work.

The gap is not the flag. It is that KESA has **two** enforcement layers fed from separate code: the `allowed_roots` checks in the file tools, and the landlock ruleset that constrains bash.
Adding `--add-dir` to only the first gives bash a different view of the filesystem than `read` has, in the permissive direction, which is the wrong direction for a security boundary to disagree in.

Build one `WorkspaceRoots` value, resolved once at startup, symlinks canonicalised, and have both layers consume it.
Then `--add-dir` is a repeatable flag, a config key and a `/add-dir` command that all feed the same value.

Root resolution must canonicalise before comparison, or a symlink inside the workspace pointing out of it is an escape.

## W4: Sandbox status honesty

`src/sandbox.rs` is landlock and nothing else: `#[cfg(target_os = "linux")]` with a stub at line 135 for every other platform.
On macOS and Windows the sandbox is inert, and nothing in the product says so.

Silent no-op security is worse than declared absence, because a user who believes they are sandboxed grants permissions they would otherwise withhold.

Model the backend as a status with a reason, not a boolean, and surface it in three places: `doctor`, the statusline, and the tool-approval overlay at the moment the user is deciding.
A platform with no backend reports degraded, visibly and persistently, and `doctor` fails rather than warns.

A config opt-in makes degraded a hard refusal for users who would rather not run at all than run unconfined.

A seatbelt backend for macOS is the real fix and is a separate job, because it cannot be verified without a Mac.
If no Mac is available the honesty layer still ships and still helps, which is why it is scoped first and separately.

## W5: Hook coverage and turn-end notification

`HookEvent` has four variants: PreToolUse, PostToolUse, UserPromptSubmit, Stop.
Missing: SessionStart, SessionEnd, PreCompact, SubagentStop, Notification.

Four of the five are mechanical.
SessionEnd is not, because a hook that only fires on the clean path is a hook nobody can rely on.
Fire it on normal exit, on SIGINT and on panic unwind, and document the case it still misses rather than implying it misses none.

Notification carries turn-end alerting.
Long runs currently finish in silence: no bell, no OSC 9, nothing.
Emit OSC 9 where the terminal supports it with a bell fallback, and only when the terminal is unfocused if focus reporting is available. A notification that fires while the user is watching the screen is noise, and noise gets muted.

## W6: Statusline as a command

The footer segments are hardcoded. The priority-ranked segment system that would host a user's own line already exists at `view.rs:1327`.

Config takes a command and a refresh interval.
The command receives session state as JSON on stdin and its stdout becomes the line.

Two constraints carry the design.
It runs off the render thread against a cached last value, because a user's shell script must never be able to stall a frame.
It is trusted the way hooks are trusted, since it comes from the user's own config, so it does not go through tool permissions.

On failure or timeout, fall back to the built-in segments rather than rendering an empty bar. A broken statusline should degrade to the old one, not to nothing.

## W7: Wheel scroll without breaking selection

Run 3 turned mouse capture off by default, deliberately, and it was right to.
D39 measured it: the default build was emitting `?1000h ?1002h ?1003h ?1006h ?1015h`, and that capture is why drag-select did nothing. J27 removed it and terminal selection works again.

The cost is that the wheel does not scroll, and PageUp-only scrolling reads as broken in 2026.

Both are reachable.
The conflict is with motion tracking, `?1002` and `?1003`, not with wheel reporting.
Enable SGR wheel reporting alone and leave motion off, so the wheel scrolls the viewport while drag-select keeps working in terminals that pass selection through.

The gate is the check D39 established, not an opinion: emit the escape bytes, grep them, and assert that the motion modes are absent while the wheel modes are present.
This one carries genuine risk that some terminals still swallow selection, so it ships behind a setting and gets verified in a real terminal before the default moves.

## W8: Conversation render cache

`build_conversation_content()` runs on **every** PageUp and PageDown keystroke, at `keybindings.rs:966` and again at `:979`.
It is the same function `benches/tui_perf.rs` was written to measure, which suggests somebody already suspected it.

Cache it against a revision counter on the message list plus the render width.
Invalidate on new message, stream chunk and resize.

The bench exists, so this job has its verification already written: it lands when the scroll path stops rebuilding and the benchmark shows it.

## Carry-forward

Open from run 3, still real:

- **J21 exit_plan_mode.** Plan mode is a permission mode with no way for the agent to propose exiting it.
- **J22, eight tool-name lists.** Adding a tool means editing eight places, and B04 already shipped a bug caused by exactly that.
- **`tests/installer_regression.sh` is 48/48 red and always has been.** Upstream's suite against a rewritten `install.sh` with no `--yes`. It gates nothing and it is the first thing that will look like a regression to whoever runs it next. Baseline it or delete it.
- **Two conformance defects J31 surfaced and did not fix.** A whole extension batch fails when any sibling entry is invalid, where pi-mono reports per entry. And a registration made inside an unresolvable bare import is silently lost, which is worse than a load error because nothing is raised.

## Cross-cutting

**Gate on names, not counts.** The two baseline lib failures are `session::tests::v2_healthy_open_accepts_read_only_owner_class_without_mutation` and `tools::tests::tool_output_cache_reuses_and_invalidates_read_only_tool_outputs`. CI sees only the first. Do not rediscover this.

**Worktree per worker, disjoint file ownership, lead fast-forwards.** J29 and J30 ran that way off a shared base, touched no file in common, and neither merge could conflict. Two workers in one checkout share a git index and commit over each other.

**`.work/` is gitignored**, so a worktree does not carry the contracts. Every worker gets the absolute path to its job file in the main tree.

**File contention to partition around.** `view.rs` is wanted by W2, W6 and W8. `tools.rs` by W1 and W3. `sandbox.rs` by W3 and W4. Those pairs cannot run concurrently against the same file without a merge fight, and the partition is the job-sizing problem, not an afterthought.

**Every workstream leaves one runnable check behind.** W1 restores a file and asserts its content. W3 asserts bash and `read` agree on a path outside the roots. W7 greps the escape bytes. W8 uses the bench that already exists.

## What needs the user

- **A Mac**, or the seatbelt backend in W4 stays scoped and unbuilt while the honesty layer ships without it.
- **J24 option 1** still wants somebody to run the aarch64 tarball on a clean box.
- **D29** wants a real-provider run. The context percentage is implemented correctly but every capture so far came from a mock returning constant usage, so nobody has watched the number actually fall.
