# Themes Template

This file documents how to add themes to this package.

## Theme Structure

Themes are JSON files that define colors and styling for pi's TUI.

```
pi-themes/
├── my-theme.json         # Dark theme
├── my-theme-light.json   # Light theme variant
└── README.md             # Theme descriptions
```

## Basic Theme Template

```json
{
  "name": "my-theme",
  "description": "A beautiful dark theme for pi",
  "palette": {
    "background": "#1e1e2e",
    "foreground": "#cdd6f4",
    "primary": "#89b4fa",
    "secondary": "#f5c2e7",
    "accent": "#f38ba8",
    "error": "#f38ba8",
    "warning": "#f9e2af",
    "success": "#a6e3a1",
    "muted": "#6c7086",
    "dim": "#45475a"
  },
  "syntax": {
    "keyword": "#cba6f7",
    "string": "#a6e3a1",
    "number": "#fab387",
    "comment": "#6c7086",
    "function": "#89b4fa",
    "variable": "#cdd6f4"
  }
}
```

## Theme Colors

| Key | Description |
|-----|-------------|
| `background` | Terminal background |
| `foreground` | Default text color |
| `primary` | Primary accent color |
| `secondary` | Secondary accent color |
| `accent` | Highlight color |
| `error` | Error messages |
| `warning` | Warning messages |
| `success` | Success messages |
| `muted` | Dimmed text |
| `dim` | Very dim text |

## Using Themes

After installing the package, you can switch themes:

```
/theme:my-theme
```

Or set as default in settings:

```json
{
  "theme": "my-theme"
}
```

## Theme Discovery

Themes are auto-discovered from:
- Global: `~/.pi/agent/themes/`
- Project: `.pi/themes/`
- Packages: `pi-themes/` directories

## Documentation

See [pi themes documentation](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent/docs/themes.md) for full specification.
