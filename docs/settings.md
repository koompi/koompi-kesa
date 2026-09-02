# Settings

KESA reads JSON settings and applies them with clear precedence rules.

## Locations

KESA loads settings from (up to) two files:

| Location | Scope |
|----------|-------|
| `~/.kesa/agent/settings.json` | Global (all projects) |
| `.kesa/settings.json` | Project (current directory) |

You can override the path entirely with `KESA_CONFIG_PATH` (see below).

Run `kesa config` to open the settings UI, which shows the effective paths.

## Precedence (highest → lowest)

1. CLI flags
2. Environment variables
3. Project settings (`.kesa/settings.json`)
4. Global settings (`~/.kesa/agent/settings.json`)
5. Built-in defaults

## `KESA_CONFIG_PATH` (single-file mode)

If `KESA_CONFIG_PATH` is set, KESA loads *only* that file and skips the global/project merge.

## Merge behavior (global vs project)

Project settings override global settings key by key, including inside nested
objects. A project that sets one key of `compaction` leaves the rest of the
global `compaction` object standing.

Example:

```json
// ~/.kesa/agent/settings.json (global)
{ "compaction": { "enabled": false, "reserve_tokens": 16384 } }
```

```json
// .kesa/settings.json (project)
{ "compaction": { "reserve_tokens": 8192 } }
```

Resulting behavior:
- `compaction.reserve_tokens` becomes `8192`
- `compaction.enabled` stays `false`, inherited from global

The same per-key merge applies to `retry`, `images`, `markdown`, `terminal`,
`branch_summary`, `thinking_budgets`, `formatters`, `extension_policy`,
`repair_policy` and `extension_risk`.

Three shapes behave differently:

- `permissions.allow`, `permissions.deny` and `permissions.ask` accumulate: the
  project's rules are appended to the global ones, skipping duplicates. So does
  `additional_directories`.
- `hooks` accumulate per event. A project that adds a `PreToolUse` hook runs it
  alongside the global one rather than in place of it — and only when the global
  file has set `hooks.trustProjectHooks`.
- Arrays of resource names (`packages`, `extensions`, `skills`, `prompts`,
  `themes`) are replaced wholesale by the project's array.

A key missing from both files falls back to its built-in default when read.

## Supported settings (snake_case JSON keys)

### Appearance

- `theme` (string): Theme name to apply. Defaults to `dark` if unset.
- `hide_thinking_block` (bool): Hide thinking blocks in interactive output. Default `false`.
- `show_hardware_cursor` (bool): Show terminal hardware cursor. Default `false` unless
  `KESA_HARDWARE_CURSOR=1`.

### Model selection

- `default_provider` (string)
- `default_model` (string)
- `default_thinking_level` (string)
- `enabled_models` (array of model patterns)

Example:

```json
{
  "default_provider": "anthropic",
  "default_model": "claude-sonnet-4-20250514",
  "default_thinking_level": "medium",
  "enabled_models": ["claude-*", "gpt-*"]
}
```

### Message delivery (queue modes)

- `steering_mode` (string): `one-at-a-time` or `all` (default `one-at-a-time`).
- `follow_up_mode` (string): `one-at-a-time` or `all` (default `one-at-a-time`).

Legacy aliases: `steeringMode`, `followUpMode`.

```json
{
  "steering_mode": "one-at-a-time",
  "follow_up_mode": "one-at-a-time"
}
```

### Interactive UX / editor

- `double_escape_action` (string): `tree`, `fork`, or `none` (default `tree`).
  Alias: `doubleEscapeAction`. Use `none` to disable the double-escape shortcut.
- `editor_padding_x` (u32): Horizontal editor padding (clamped to 0–3). Default `0`.
- `autocomplete_max_visible` (u32): Max autocomplete rows (clamped 3–20). Default `5`.
- `session_picker_input` (u32): Non-interactive session picker selection (1-based).
  Alias: `sessionPickerInput`.
- `quiet_startup` (bool): Suppress the startup header.
- `collapse_changelog` (bool): Condense “What’s New” output when present.

### Compaction (defaults)

Accessor defaults:
- `compaction.enabled`: `true`
- `compaction.reserve_tokens`: `16384`
- `compaction.keep_recent_tokens`: `20000`

```json
{
  "compaction": {
    "enabled": true,
    "reserve_tokens": 16384,
    "keep_recent_tokens": 20000
  }
}
```

### Branch summary

- `branch_summary.reserve_tokens` (u32): Defaults to `compaction.reserve_tokens`.

### Retry (defaults)

Accessor defaults:
- `retry.enabled`: `true`
- `retry.max_retries`: `3`
- `retry.base_delay_ms`: `2000`
- `retry.max_delay_ms`: `60000`

```json
{
  "retry": {
    "enabled": true,
    "max_retries": 3,
    "base_delay_ms": 2000,
    "max_delay_ms": 60000
  }
}
```

### Shell

- `shell_path` (string): Shell binary for the `bash` tool. Unset, KESA takes the
  first of `/bin/bash`, `/usr/bin/bash`, `/usr/local/bin/bash` that exists, and
  falls back to `sh`.
- `shell_command_prefix` (string): Prepended to every `bash` command. Unset by
  default, meaning no prefix.
- `gh_path` (string): Override path to `gh` for `/share`. Alias: `ghPath`.

```json
{
  "shell_path": "/bin/bash",
  "shell_command_prefix": "set -euo pipefail"
}
```

### Images

- `images.auto_resize` (bool): Default `true`.
- `images.block_images` (bool): Default `false`.

```json
{
  "images": {
    "auto_resize": true,
    "block_images": false
  }
}
```

### Terminal display

- `terminal.show_images` (bool): Default `true`. When `false`, image blocks are hidden in terminal tool output (they are still stored in sessions and exports).
- `terminal.clear_on_shrink` (bool): Default `false`. When `true`, scrollback is purged on terminal shrink so stale rows do not reappear after a resize.

### Thinking budgets (tokens)

- `thinking_budgets.minimal`: default `1024`
- `thinking_budgets.low`: default `2048`
- `thinking_budgets.medium`: default `8192`
- `thinking_budgets.high`: default `16384`
- `thinking_budgets.xhigh`: default `32768`
- `thinking_budgets.max`: default `65536`

### Formatters

`formatters` pipes every file a tool writes through a shell command before the
result is persisted. It is empty by default.

- Key: a glob matched against the path relative to the working directory. The
  longest matching key wins.
- Value: a command run with `/bin/sh -c` that reads the file on **stdin** and
  writes the formatted file to **stdout**. The file is left exactly as the tool
  wrote it unless the formatter exits `0` with non-empty, valid UTF-8 stdout, so
  a formatter that fails cannot empty or truncate your file. It is killed after
  30 seconds, and the first line of its stderr is reported to the model.

```json
{
  "formatters": {
    "*.rs": "rustfmt --edition 2024",
    "*.py": "ruff format -"
  }
}
```

A formatter is an arbitrary shell command running with your privileges on every
write, so it is treated exactly like a hook: formatters in a project's
`.kesa/settings.json` are ignored unless the **global** settings file sets
`hooks.trustProjectHooks`.

Formatters go through the same sandbox as the `bash` tool. The sandbox is off
unless you turn it on, but once it is on it hides `$HOME`, and a formatter
binary living in `~/.cargo/bin`, `~/.local/bin` or `~/.bun/bin` stops being
reachable. Name an absolute path outside `$HOME`, or start KESA with `--add-dir`
for the directory holding it.

Two more things to know before turning one on: a formatter rewrites lines the
model never touched, which makes a diff harder to review, and the binary it
names is not guaranteed to be installed.

### Prompt caching

- `cache_retention` (string): `none`, `short` or `long`. Default `short`.
  Alias: `cacheRetention`. An unrecognized value fails to load.

Only first-party Anthropic reads this today; other providers, including
Anthropic-compatible gateways, ignore it. The default `short` means KESA asks
Anthropic to write prompt cache entries, which are billed at a higher rate than
plain input tokens and read back at a lower one. Set `none` to send nothing.

```json
{
  "cache_retention": "long"
}
```

### Packages and resources

- `packages` (array): package sources (string or `{ source, local, kind }`).
- `extensions`, `skills`, `prompts`, `themes` (arrays): resource filters.
- `enable_skill_commands` (bool): default `true`.

## Full reference

`src/config.rs` is the authoritative list of supported fields and defaulting behavior.
