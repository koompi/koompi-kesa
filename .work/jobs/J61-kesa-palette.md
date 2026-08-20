---
id: J61
title: KESA palette, one source of truth, contrast gate
deltas: D36 D37 D38 D40
state: ready
---

# J61 — KESA palette, one source of truth, contrast gate

KESA's three built-in themes are VS Code's palette. The default border sits at **1.51:1** against
the background and solarized's at **1.28:1**, so every box outline in the shipped default is at the
edge of visibility; `muted`, which carries the footer and every hint, is at **3.08:1**, under AA.
Worse, each palette exists twice — once hardcoded in `src/theme.rs` and once in `themes/*.json`,
which nothing loads.

Give KESA its own visual identity, collapse the duplication, and leave behind a test that fails if
a future edit regresses the contrast.

## Files you own

- `src/theme.rs`
- `themes/dark.json`
- `themes/light.json`
- `themes/solarized.json`
- `tests/theme_contrast.rs` — new
- `Cargo.toml`
- `docs/themes.md`

Anything else, stop and report. **`src/interactive/view.rs` belongs to J63 — do not open it.**
If a change you want requires it, that is a line in your report, not an edit.

## Do

1. **D38 — one source of truth.** `Theme::resolve_spec` (`src/theme.rs:127-153`) short-circuits the
   names `dark`, `light` and `solarized` to hardcoded constructors at `src/theme.rs:333-419`, before
   any file lookup. `build.rs:13-37` embeds four assets and none of them are themes, and there is no
   `include_str!` of `themes/` anywhere in `src/`. So the JSONs are dead.

   Make `Theme::dark()`, `Theme::light()` and `Theme::solarized()` parse
   `include_str!("../themes/<name>.json")` instead of constructing struct literals. Add
   `/themes/**` to the `include` list in `Cargo.toml` (the list starting at `Cargo.toml:20`) or
   `cargo publish` will produce a tarball that cannot build.

   Parsing at startup must not be able to panic the binary on a malformed edit: if you use
   `expect`, the message must name the file. A compile-time-checked alternative is fine if you find
   one that does not add a dependency.

2. **D36, D37 — the KESA palette.** Redesign all three themes. This is not a contrast patch on
   VS Code's hues; it is KESA's own identity, and the two other themes are `light` and `solarized`
   variants of the same identity, not untouched leftovers.

   `docs/themes.md:56-79` already documents an example palette that reads as KESA rather than as
   VS Code. Use it as the anchor for `dark`, and derive `light` and `solarized` from the same role
   assignments:

   | role | anchor | carries |
   |---|---|---|
   | `background` | `#0b0f14` | the frame |
   | `foreground` | `#e6e6e6` | body text |
   | `accent` | `#38bdf8` | the app label, the prompt glyph, a running agent, plan mode |
   | `success` | `#22c55e` | a finished agent, an applied edit |
   | `warning` | `#f59e0b` | bash mode, accept-edits mode |
   | `error` | `#ef4444` | a failed agent, a denied write |
   | `muted` | `#94a3b8` | footer, hints, collapsed detail |
   | `ui.border` | **must change** | every box outline |

   `docs/themes.md`'s own `ui.border` of `#1f2937` fails the gate at roughly 1.4:1 — do not copy it.
   `#64748b` measures about 4.0:1 against `#0b0f14`; verify rather than trust that number, and pick
   what actually passes.

   `syntax.*` must be five distinguishable hues that all clear the text floor, because step 4 starts
   using them. Today they are loaded and never rendered.

   Update the example in `docs/themes.md` so the docs stop shipping a palette that fails the gate
   the code now enforces.

3. **D36, D37 — the gate.** New `tests/theme_contrast.rs`. For each of `Theme::dark()`,
   `Theme::light()`, `Theme::solarized()`, compute the WCAG 2.1 contrast ratio of every colour
   against that theme's own `colors.background` and assert:

   - `ui.border` ≥ **3.0** — the non-text UI floor
   - `foreground`, `muted`, `accent`, `success`, `warning`, `error` ≥ **4.5** — AA for text
   - every `syntax.*` ≥ **4.5**

   Implement the ratio from the spec: sRGB channel to linear via the 0.03928 / 12.92 / 2.4 piecewise
   transfer function, luminance `0.2126R + 0.7152G + 0.0722B`, ratio `(L1+0.05)/(L2+0.05)`. Do not
   add a crate for this; it is about fifteen lines.

   The failure message must name the theme, the token, the measured ratio and the floor. A test that
   says `assertion failed: ratio >= 3.0` is worth less than one that says
   `solarized ui.border #073642 on #002b36 is 1.28:1, floor 3.0:1`.

   Assert the current values would have failed: add one test that runs the ratio function over the
   *old* dark border `#3c3c3c` on `#1e1e1e` and asserts it is under 3.0, so the gate is proven to
   have teeth rather than merely passing.

4. **D40 — fenced code stops looking like inline code.** `glamour_style_config`
   (`src/theme.rs:209-252`) gives inline `code` and fenced `code_block` the same single colour and
   nothing else, so a block is indistinguishable from a span. Give the block a background and a left
   margin, and route the theme's `syntax.keyword`, `syntax.number`, `syntax.comment` and
   `syntax.function` into the block's style config — they are loaded from every theme and currently
   reach nothing.

   Check what `GlamourStyleConfig` actually exposes before designing this; if a field you want does
   not exist, say so in the report rather than faking it with padding hacks.

## Acceptance

Paste real output for each. A claim without output is not acceptance.

1. `cargo test --test theme_contrast` — paste the full list of passing test names. Then temporarily
   set dark's `ui.border` back to `#3c3c3c`, re-run, and paste the failure message to demonstrate
   the gate bites and that the message names theme, token, ratio and floor. Restore.
2. For all three themes, paste a table of every token with its measured ratio, sorted ascending, so
   the tightest margin in the design is visible. State which token is closest to its floor.
3. `cargo test --lib theme` — paste the name set. Then `cargo test --lib` and confirm the failing
   set is exactly `v2_healthy_open_accepts…` and `tool_output_cache_reuses…`, the two baseline
   names. Gate on the names, never the count.
4. `cargo test --test tui_snapshot` — must be **145/0 with zero snapshot changes**.
   `normalize_snapshot` strips ANSI (`tests/tui_snapshot.rs:203-207`), so a palette change cannot
   move a snapshot. If one moves, you changed layout, not colour — stop and report it.
5. Demonstrate D38 is actually closed: edit `themes/dark.json`'s accent to something obviously
   wrong, run `cargo test --test theme_contrast`, and show it fails. Before this job, editing that
   file changed nothing at all. Restore.
6. `cargo fmt --check` and `cargo clippy --all-targets` clean.
7. For D40, paste a before/after of the rendered output of a markdown string containing both an
   inline span and a fenced block, with `--format ansi` or an escape-visible dump, so the difference
   is evidence rather than assertion.

## Out of scope

- **`src/interactive/view.rs`.** J63 owns it. The input-border semantics (D39), the speaker glyphs
  (D41) and the footer (D42, D43) are J63's, even though they are colour-adjacent.
- Theme discovery, hot reload, `--theme`, per-package themes. `docs/themes.md:84` lists these as
  known gaps against legacy pi-mono; they stay gaps.
- Adding theme tokens. The schema is `ThemeColors` / `SyntaxColors` / `UiColors`
  (`src/theme.rs:46-73`); a new token is a user-facing schema change and needs its own job.
- Per-provider colour tints. Decided against: the provider set is open-ended and cannot be kept
  accessible. Provider is shown as text.

## Stop conditions

- A user theme in `~/.kesa/agent/themes/` or `<cwd>/.kesa/themes/` fails the new floors. **Do not
  enforce the gate on user themes** — that would break someone's working setup on upgrade. The gate
  is for built-ins only. If you find yourself validating user themes, stop.
- `include_str!` of the JSONs turning out to need a new dependency, or `Cargo.toml`'s `include` list
  interacting badly with `cargo publish --dry-run`. Report; do not add a build script.
- Any need to touch a file you do not own.
- If a palette you like cannot clear the floors, do not lower the floors. Report the conflict with
  the measured numbers and your two best alternatives.
