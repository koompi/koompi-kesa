# Themes

Pi’s interactive TUI supports **JSON theme files** plus a few built-in themes.

If something described here doesn’t match what you see, check `src/theme.rs` and the theme workstream (`bd-22p`) — the theme UX is still evolving.

## Built-in themes

- `dark`
- `light`
- `solarized`

## Theme discovery (custom themes)

Pi discovers custom themes by scanning these directories for `*.json` files:

- Global: `~/.kode/agent/themes/`
- Project: `<cwd>/.kode/themes/`

Discovery is by file extension only; Pi loads each JSON file and uses the `name` field inside it.

## Selecting a theme

### Interactive command

- ` /theme ` (no args): list discovered themes
- ` /theme <name> `: switch themes

Note: `/settings` includes a Theme entry that opens the picker. `/theme` remains available for quick switching, and editing `settings.json` works as well.

### Settings file

Set `theme` in your settings JSON:

- Global: `~/.kode/agent/settings.json`
- Project: `<cwd>/.kode/settings.json`

Example:

```json
{
  "theme": "solarized"
}
```

If a configured theme can’t be loaded, Pi falls back to `dark` and logs a warning.

## Theme file format (JSON)

Theme JSON files are validated on load. All colors are **hex strings** in `#RRGGBB` format.

Minimal example:

```json
{
  "name": "my-theme",
  "version": "1.0",
  "colors": {
    "foreground": "#e6e6e6",
    "background": "#0b0f14",
    "accent": "#38bdf8",
    "success": "#22c55e",
    "warning": "#f59e0b",
    "error": "#ef4444",
    "muted": "#94a3b8"
  },
  "syntax": {
    "keyword": "#38bdf8",
    "string": "#22c55e",
    "number": "#a78bfa",
    "comment": "#94a3b8",
    "function": "#f59e0b"
  },
  "ui": {
    "border": "#1f2937",
    "selection": "#111827",
    "cursor": "#e6e6e6"
  }
}
```

### Field meanings (high level)

- `colors.*`: primary UI colors (text/background + semantic colors)
- `syntax.*`: colors used for code/markup rendering
- `ui.*`: frame/selection/cursor colors

## Current gaps vs legacy pi-mono

Legacy pi-mono supports additional theme discovery mechanisms (packages, `themes[]` settings paths, CLI `--theme`, hot reload, many more tokens). The Rust port is intentionally smaller right now.

Track progress in `bd-22p`.
