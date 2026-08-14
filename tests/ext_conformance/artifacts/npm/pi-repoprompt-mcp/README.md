# RepoPrompt MCP for Pi (`pi-repoprompt-mcp`)

A token-efficient RepoPrompt MCP integration for Pi.

Exposes a single `rp` tool (RepoPrompt MCP proxy) plus `/rp …` commands, with:
- window/tab binding (auto-detect by `cwd`, optional persistence)
- output rendering (syntax + diff highlighting)
- safety guardrails (deletes blocked unless explicitly allowed; optional edit confirmation)

## Install

From npm:

```bash
pi install npm:pi-repoprompt-mcp
```

From the dot314 git bundle (filtered install):

Add to `~/.pi/agent/settings.json` (or replace an existing unfiltered `git:github.com/w-winter/dot314` entry):

```json
{
  "packages": [
    {
      "source": "git:github.com/w-winter/dot314",
      "extensions": ["extensions/repoprompt-mcp/src/index.ts"],
      "skills": [],
      "themes": [],
      "prompts": []
    }
  ]
}
```

## Requirements

- RepoPrompt installed
- RepoPrompt MCP server reachable (stdio transport)
  - If the server is not configured/auto-detected, the package will still load, but `rp(...)` will error until you configure it
- `rp-cli` on `PATH` is recommended (used as a fallback for window discovery)

## Usage

Commands:
- `/rp status`
- `/rp windows`
- `/rp bind`
- `/rp reconnect`

Tool:
```ts
rp({ windows: true })
rp({ bind: { window: 3 } })
rp({ call: "read_file", args: { path: "src/main.ts" } })
```

## Configuration

Create `~/.pi/agent/extensions/repoprompt-mcp.json`:

```json
{
  "autoBindOnStart": true,
  "persistBinding": true,
  "confirmDeletes": true,
  "confirmEdits": false
}
```

If the MCP server is not auto-detected, set `command` explicitly:

```json
{
  "command": "/Applications/Repo Prompt.app/Contents/MacOS/repoprompt-mcp",
  "args": []
}
```

(Alternatively, configure RepoPrompt in `~/.pi/agent/mcp.json`)

For more detail, see: `extensions/repoprompt-mcp/README.md` in the dot314 repo.
