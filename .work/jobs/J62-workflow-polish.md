---
id: J62
title: Workflow polish outside the view layer
deltas: D45 D47 D48 D49 D50 D51 D52
state: ready
---

# J62 — Workflow polish outside the view layer

Seven independent frictions, each small, each in a file J63 does not own. None of them are
architectural; all of them are things a user hits in the first ten minutes.

Work them in the order listed. Each is independently landable, so if one turns out to be larger than
it looks, finish the others and report the one you left.

## Files you own

- `src/interactive/state.rs`
- `src/model_selector.rs`
- `src/interactive/model_selector_ui.rs`
- `src/autocomplete.rs`
- `src/interactive/commands.rs`
- `src/main.rs`
- `src/auth.rs`
- `src/error.rs`
- `src/interactive/share.rs`

**`src/interactive/view.rs`, `src/interactive/agent.rs`, `src/interactive/tool_render.rs`,
`src/interactive.rs`, `src/tools.rs` and `tests/snapshots/` belong to J63.** Do not open them. If a
change needs one, that is a line in your report.

## Do

1. **D45 — the autocomplete opens with nothing selected.** `AutocompleteState::selected` is
   `Option<usize>`, initialized `None` in `new()` (`src/interactive/state.rs:150`) and reset to
   `None` on every re-open in `open_with` (`:163-187`) unless the replace range is unchanged. So
   there is no highlight and no description row until the user presses an arrow key, even though all
   25 builtins carry a description.

   Preselect index 0 whenever the list is non-empty. Keep the existing "preserve the previous
   selection when the replace range is unchanged" behaviour — that is what stops the highlight
   jumping while you keep typing; only the `None` fallback changes.

   Check the consumers before you change the type: `src/interactive.rs:3538-3576` (J63's file, read
   only) branches on `selected.is_none()` for Tab and for Enter. Leaving `selected` as an `Option`
   and just never yielding `None` for a non-empty list is the smaller diff and keeps those branches
   valid. Do not flatten it to `usize` unless you can show every consumer still reads correctly.

2. **D47 — three documented commands never autocomplete.** `/rewind`, `/context` and `/template`
   appear in `/help` (`src/interactive/commands.rs:91-130`) but are absent from
   `builtin_slash_commands()` (`src/autocomplete.rs:254-800`). Add them with descriptions matching
   their `/help` text, including any aliases `/help` documents.

   While you are in there, derive the check rather than eyeballing it: add a test asserting that
   every command `/help` documents has an autocomplete entry. That is the runnable check that keeps
   this closed.

3. **D48, D49 — the model picker.** Four separate defects in
   `src/interactive/model_selector_ui.rs` and `src/model_selector.rs`:

   - **It ignores the model you are on.** `selected` is `0` at `src/model_selector.rs:62` and reset
     to `0` in `refresh_filtered` (`:220`); `open_model_selector`
     (`src/interactive/model_selector_ui.rs:54-69`) never seeks the current entry, and
     `scroll_offset` derives purely from `selected` (`model_selector.rs:201-207`). With 111 models
     the model you are using is usually off-screen. Open with the current model selected and
     scrolled into view. Resetting to 0 on a *filter change* is correct; keep that.
   - **111 models in one flat alphabetical list.** Sorted provider-then-id at
     `model_selector.rs:57-58`, so amazon-bedrock fills the first four pages. Group by provider with
     a header row per provider. Put the current model's provider first.
   - **Two counters that disagree.** `"({}-{} of {})"` at `model_selector_ui.rs:271-278` and
     `"({}/{})"` at `:283-291`. Keep one format. If both facts matter — window position and
     credential filtering — say both in one string that a user can parse.
   - **It looks like nothing else in the app.** `"─".repeat(50)` at `model_selector_ui.rs:217`.
     Every other overlay uses the rounded `bordered_box` helper. That helper lives in
     `src/interactive/view.rs:113-131`, which you do not own — it is `pub(super)`. **Read it, match
     its output exactly in your own file, and note in your report that the two should be unified
     once J63 lands.** Do not edit view.rs to re-export it.

   Also delete the pointer to a README section that does not exist: `model_selector_ui.rs:195-198`
   says "see README for details" and `README.md` has no text on model readiness or credentials.
   Either say the thing in the hint or drop the sentence.

4. **D50, D51 — `/help`.** The dash column in `SlashCommand::help_text()`
   (`src/interactive/commands.rs:91-130`) is hand-aligned to column 21 and four entries overrun it:
   `:97` at column 30, `:98` at 24, `:99` at 34, `:117` at 26. Replace the hand-aligned string
   literal with a `(command, description)` table padded to the widest entry at render time.

   Then settle D51. There are three different answers on screen for how to insert a newline: the
   input box says `\+Enter` (`view.rs:1288-1292`, J63's file), `/help` says
   `Shift+Enter (Ctrl+Enter on Windows)` (`commands.rs:126`), and a fixture says a third thing.
   `src/interactive/state.rs:115` documents it as "Shift+Enter or `\`", and
   `src/interactive/keybindings.rs:961` records that Shift+Enter does not survive every terminal —
   which is why the backslash form exists. Both are real. Say both, in one phrasing, in `/help`.
   J63 makes the input box agree.

5. **D52 — branding.** Replace user-visible "Pi" with "KESA". `APP_LABEL` is already `"KESA"`, so
   these are fork leftovers:

   - `src/interactive/commands.rs:120` — `Exit Pi`
   - `src/interactive/commands.rs:2459`, `:2466`, `:2470` — OAuth guidance
   - `src/interactive/commands.rs:2978-2982` — the `/mcp` note, three occurrences in one string
   - `src/autocomplete.rs:731` — `Exit Pi`
   - `src/main.rs:4658` — `Pi will continue automatically`, **and the em dash on that line**
   - `src/interactive/share.rs:114-115` — gist descriptions, visible on the published gist
   - `src/auth.rs:4113`, `:4116`, `:4225`, `:4228` — the OAuth success page title and body
   - `src/error.rs:811`, `:892`, `:923` — remediation hints

   Two more live in J63's files (`src/interactive/agent.rs:723`, `src/tools.rs:5333`) and are J63's.

   Grep your owned files for remaining user-visible occurrences before you finish; the list above
   came from one pass and may not be complete. Do **not** touch `pi_agent_rust#NN` issue references
   in comments, the `pi.*` schema strings, or `KESA_SUBAGENT_PI_BINARY` — those are identifiers and
   wire contracts, not copy.

## Acceptance

Paste real output for each.

1. **D45**: paste a test showing a freshly opened autocomplete has `selected == Some(0)` and its
   description available, and a second showing that typing another character while the replace range
   is unchanged does not reset the selection.
2. **D47**: paste the new test's name and output, and the failure it produces when you temporarily
   remove `/rewind` from `builtin_slash_commands` again.
3. **D48**: with a registry of 111 models and a current model that sorts late alphabetically,
   demonstrate the picker opens with that model selected and on-screen. Paste the rendered first
   page as text, with the provider group headers visible.
4. **D49**: paste the rendered picker header and footer showing exactly one counter format, and the
   rendered frame showing rounded corners. State the README sentence you removed or the text you
   replaced it with.
5. **D50**: paste the full rendered `/help` output at 80 columns and again at 60 columns. Every dash
   must be in the same column at 80. State what happens at 60 and whether it is acceptable.
6. **D51**: paste the one `/help` line covering newlines, and quote `keybindings.rs:961` in your
   report as the reason both forms are documented.
7. **D52**: paste `grep -rn 'Pi ' <your owned files>` output showing what remains and asserting each
   remaining hit is an identifier, a schema string or a code comment.
8. `cargo test --lib` — failing set is exactly the two baseline names,
   `v2_healthy_open_accepts…` and `tool_output_cache_reuses…`. Gate on names, never counts.
9. `cargo test --test tui_snapshot` — 145/0. **If a snapshot moves, stop and report it**; that means
   you changed something J63 owns the rendering of.
10. `cargo fmt --check` and `cargo clippy --all-targets` clean.

## Out of scope

- Anything in `src/interactive/view.rs`. The autocomplete *dropdown rendering* (D46, the description
  row that makes the list shift) is J63's half of the same fix. You do the state; J63 does the draw.
- Redesigning what the model picker filters. Credential-gated filtering stays as it is; only its
  presentation changes.
- The `/help` content itself. Fix the column and the newline line; do not rewrite the command
  descriptions or add commands to the list.
- Renaming `KESA_SUBAGENT_PI_BINARY`, the `pi.*` schema constants, or the `pi_agent_rust` upstream
  attribution in `Cargo.toml` and `NOTICE.md`. Those are contracts and credit, not branding.

## Stop conditions

- A snapshot in `tests/snapshots/` moves. Stop and report; do not accept it.
- The model picker's provider grouping turning out to need a change to `ModelEntry` or the registry
  in `src/models.rs`. You do not own that file. Report the shape you need.
- `bordered_box` turning out not to be reachable from your files without editing `view.rs`. Copy its
  output, note the duplication in your report, and move on. Do not export it.
- Any `/help` change that would require reflowing text in `view.rs`.
