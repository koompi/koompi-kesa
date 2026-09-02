# Themes

The interactive TUI reads **JSON theme files** and ships three built-in themes.

## Built-in themes

- `dark`
- `light`
- `solarized`

## Theme discovery (custom themes)

KESA discovers custom themes by scanning these directories for `*.json` files:

- Global: `~/.kesa/agent/themes/`
- Project: `<cwd>/.kesa/themes/`

Discovery is by file extension only; each JSON file is loaded and the `name` field inside it is used.

## Selecting a theme

### Interactive command

- ` /theme ` (no args): list discovered themes
- ` /theme <name> `: switch themes

Note: `/settings` includes a Theme entry that opens the picker. `/theme` remains available for quick switching, and editing `settings.json` works as well.

### Command line

```bash
kesa --theme solarized              # built-in name, discovered name, or a path
kesa --theme ./my-theme.json
kesa --theme-path ~/themes          # add a file or directory to discovery
kesa --no-themes                    # skip discovery, built-ins only
```

`--theme-path` may be repeated.

### Settings file

Set `theme` in your settings JSON:

- Global: `~/.kesa/agent/settings.json`
- Project: `<cwd>/.kesa/settings.json`

Example:

```json
{
  "theme": "solarized"
}
```

If a configured theme can’t be loaded, KESA falls back to `dark` and logs a warning.

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
    "border": "#64748b",
    "selection": "#1e293b",
    "cursor": "#e6e6e6"
  }
}
```

`ui.border` must clear a 3:1 contrast ratio against `colors.background`, and every other color above must clear 4.5:1 — built-in themes are held to this by `tests/theme_contrast.rs`.

### Field meanings (high level)

- `colors.*`: primary UI colors (text/background + semantic colors)
- `syntax.*`: colors used for code/markup rendering
- `ui.*`: frame/selection/cursor colors

## What is not supported

Themes are read once at startup. Editing a theme file while a session runs has
no effect until the next launch; `/theme` re-selects among the themes that were
discovered at startup.

The token set is the one listed above. A theme file may carry extra keys, and
they are ignored rather than rejected.
