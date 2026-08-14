# @benvargas/pi-cut-stack

Cut-stack editor shortcuts for pi.

- `cut` takes the entire current editor content, pushes it onto a stack, and clears the editor.
- `pop` pops the latest entry from the stack and appends it to the current editor content.
- The stack is in-memory only (cleared on restart).

## Install

```bash
pi install npm:@benvargas/pi-cut-stack
```

Or try without installing:

```bash
pi -e npm:@benvargas/pi-cut-stack
```

## Default Keybindings

- Cut: `alt+x`
- Pop: `alt+p`

## Configuration

`keybindings.json` is optional. Only add it if you want to override the defaults.

Add entries to `~/.pi/agent/keybindings.json`:

```json
{
  "ext.pi-cut-stack.cut": "alt+x",
  "ext.pi-cut-stack.pop": "alt+p"
}
```

Each key can be a single string or an array of strings. If the file is missing or invalid, defaults are used.

## Notes

- Pop with an empty stack displays a `Cut stack is empty` status notification.
- If the editor is empty, cut is a no-op.

## Ghostty (macOS) Tip

macOS terminal apps do not receive `cmd` shortcuts directly. If you want `cmd+x` / `cmd+p` to behave like `alt+x` / `alt+p`, map them in Ghostty:

```ini
keybind = cmd+x=esc:x
keybind = cmd+p=esc:p
```

This sends `esc+x` / `esc+p`. Because pi uses Kitty keyboard protocol outside of tmux, these mappings only behave like `alt+x` / `alt+p` inside tmux. Outside tmux, use the actual Option/Alt keys (ensure Ghostty is configured to treat Option as Alt/Meta).

## Uninstall

```bash
pi remove npm:@benvargas/pi-cut-stack
```

## License

MIT
