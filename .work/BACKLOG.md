---
updated: 2026-08-20
---

# Backlog — run 5, TUI UX/UI

Frozen at first dispatch. What actually happened lives in `LEAD.md`; when they disagree, `LEAD.md`
wins.

## Baseline

Branch `fix-tui-audit-bugs`, at `1cc9eb3e` plus uncommitted work in `src/subagents.rs` owned by J63.

Gate on **name sets**, never on counts — every job here adds tests.

| check | command | state at dispatch |
|---|---|---|
| lib | `cargo test --lib` | **2 known failures**, names only: `v2_healthy_open_accepts…`, `tool_output_cache_reuses…`. Gate on that exact name set |
| tui snapshots | `cargo test --test tui_snapshot` | 145/0 |
| sdk | `cargo test --test sdk_unit` | 168/0 |
| fmt | `cargo fmt --check` | clean |
| clippy | `cargo clippy --all-targets` | clean |

Snapshot note that shapes the partition: `normalize_snapshot` strips ANSI
(`tests/tui_snapshot.rs:203-207`), so **a palette change does not move a single snapshot**. Only
layout and glyph changes do. That is why J61 and J63 can run concurrently.

## Jobs

| id | title | deltas | owns | state | pairs with |
|----|-------|--------|------|-------|------------|
| J61 | KESA palette, one source of truth, contrast gate | D36 D37 D38 D40 | `src/theme.rs`, `themes/*.json`, `tests/theme_contrast.rs` (new), `Cargo.toml` | ready | — |
| J62 | Workflow polish outside the view layer | D45 D47 D48 D49 D50 D51 D52 | `src/interactive/state.rs`, `src/model_selector.rs`, `src/interactive/model_selector_ui.rs`, `src/autocomplete.rs`, `src/interactive/commands.rs`, `src/main.rs`, `src/auth.rs`, `src/error.rs`, `src/interactive/share.rs` | ready | J63 (D45 needs D46) |
| J63 | The orchestration surface, and the frame it sits in | D30 D31 D32 D33 D34 D35 D39 D41 D42 D43 D44 D46 | `src/subagents.rs`, `src/interactive/agent_roster.rs` (new), `src/interactive.rs`, `src/interactive/agent.rs`, `src/interactive/tool_render.rs`, `src/interactive/view.rs`, `src/interactive/tests.rs`, `src/tools.rs`, `tests/tui_snapshot.rs`, `tests/snapshots/tui_snapshot__*` | live, lead's pane | J62 (D46 needs D45) |

### Contended files, and how they are sequenced

- **`src/interactive/view.rs`** is the single most contended file in the tree. **J63 owns it
  outright.** J61 and J62 stop and report rather than opening it. Every delta that touches it —
  D39, D41, D42, D43, D44, D46 — is in J63 for that reason alone.
- **`src/interactive/agent.rs`** is J63's. The one branding string in it (`:723`, D52) is J63's to
  fix, not J62's, despite belonging to D52.
- **`src/tools.rs`** is J63's. The one branding string in it (`:5333`, D52) is J63's, same reason.
- **`tests/snapshots/`** moves only under J63. J61 cannot move it (ANSI is stripped) and J62 should
  not; if J62 finds a snapshot moving, that is a report, not a fix.
- **`Cargo.toml`** is J61's alone.

### D45 / D46 pair

D45 (preselect row 0, `state.rs`, J62) and D46 (fixed description row, `view.rs`, J63) are one
user-visible behaviour split across two owners. Either alone is an improvement; both together are
the fix. Neither blocks the other. The lead verifies them as one demonstration after both return.
