# agentsbox

Tool-search facade for MCP servers (OpenCode + pi integrations).

> **agentsbox** implements the "tool search tool" pattern for MCP: instead of loading every MCP tool schema into the agent context up-front, it exposes a small set of `agentsbox_*` tools that search and execute MCP server tools on-demand.

[![npm version](https://badge.fury.io/js/agentsbox.svg)](https://www.npmjs.com/package/agentsbox)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## Installation

### NPM Package

```bash
npm install -g agentsbox
```

After installation via npm, initialize the configuration:

```bash
agentsbox init
```

This creates the config directory under `$XDG_CONFIG_HOME/agentsbox` (usually `~/.config/agentsbox`).

## Setup Integration

#### OpenCode

```bash
agentsbox setup opencode
```

Installs the plugin by copying `dist/opencode.js` to `~/.config/opencode/plugins/agentsbox.js` (auto-loaded by OpenCode).

#### pi

```bash
agentsbox setup pi
```

Installs extension by symlinking to `~/.pi/agent/extensions/agentsbox` (auto-loaded by pi).

### What Gets Created

Initialization creates under `$XDG_CONFIG_HOME/agentsbox` (usually `~/.config/agentsbox`):
- `config.jsonc` – minimal config scaffold
- `agentsbox.schema.json` – JSON schema for validation
- `skill/agentsbox/` – bundled skill for agents

---

### With OpenCode

After `agentsbox setup opencode`, the plugin is automatically loaded by OpenCode. The plugin exposes:
- `agentsbox_search_bm25`
- `agentsbox_search_regex`
- `agentsbox_execute`
- `agentsbox_status`
- `agentsbox_perf`
- `agentsbox_test`

### With pi

After `agentsbox setup pi`, the extension is available in the pi agent system. The bundled skill at `skill/agentsbox/` provides guidelines for agents using these tools.

---

## What It Does

Instead of exposing **all** MCP server tools to the LLM context, agentsbox provides only 6 stable tools:

| Tool | Description |
|------|-------------|
| `agentsbox_search_bm25` | Search tools by natural language query |
| `agentsbox_search_regex` | Search tools by regex pattern on tool names |
| `agentsbox_execute` | Execute a discovered tool by toolId |
| `agentsbox_status` | Get status (servers, tools, health) |
| `agentsbox_perf` | Get performance metrics |
| `agentsbox_test` | Test all tools with minimal inputs |

### The Problem: Context Bloat

Without agentsbox, LLMs see **all** MCP tools with full schemas:
```json
{
  "tools": [
    { "name": "time_get_current_time", "description": "...", "parameters": {...} },
    { "name": "brave_web_search", "description": "...", "parameters": {...} },
    { "name": "tavily_search", "description": "...", "parameters": {...} },
    ... (dozens more)
  ]
}
```

### The Solution: Search-First Pattern

With agentsbox, the LLM sees only 6 tools and discovers others on-demand:

```
User: "Search the web for AI news"
    ↓
LLM calls: agentsbox_search_bm25({ text: "web search" })
    ↓
Returns: [ "brave_web_search", "tavily_search" ] + schemas
    ↓
LLM calls: agentsbox_execute({ toolId: "brave_web_search", arguments: "{...}" })
```

---

## Configuration

### Config Path

- Default: `~/.config/agentsbox/config.jsonc`
- Override: `export AGENTSBOX_CONFIG=/path/to/config.jsonc`

### Config Format

Full reference in [CONFIG.md](./CONFIG.md). Minimal example:

```jsonc
{
  "$schema": "./agentsbox.schema.json",
  "mcp": {
    "time": {
      "type": "local",
      "command": ["uvx", "mcp-server-time"]
    },
    "tavily": {
      "type": "remote",
      "url": "https://mcp.tavily.com/mcp/",
      "headers": {
        "Authorization": "Bearer {env:TAVILY_API_KEY}"
      }
    }
  },
  "settings": {
    "defaultLimit": 5,
    "initMode": "eager",
    "connection": {
      "connectTimeout": 5000,
      "requestTimeout": 30000,
      "retryAttempts": 2,
      "retryDelay": 1000
    }
  }
}
```

### Environment Variable Interpolation

Use `{env:VAR_NAME}` anywhere in config:

```jsonc
{
  "mcp": {
    "my-server": {
      "type": "remote",
      "url": "{env:MCP_SERVER_URL}",
      "headers": {
        "Authorization": "Bearer {env:API_TOKEN}"
      }
    }
  }
}
```

---

## Development

```bash
# Lint and format
bun run check
bun run check:fix

# Type checking
bun run typecheck

# Run tests
bun test
bun test --coverage

# Benchmarks
bun run bench
```

---

## Project Structure

```
agentsbox/
├── src/
│   ├── catalog/       # Tool catalog + types
│   ├── config/        # Config loading + validation
│   ├── mcp-client/    # MCP client manager (local/remote)
│   ├── search/        # BM25 + regex search
│   ├── profiler/      # Performance metrics
│   ├── runtime.ts     # Core runtime (tool registration)
│   ├── plugin.ts      # OpenCode plugin implementation
│   ├── pi.ts          # pi extension implementation
│   └── cli.ts         # CLI (init/setup commands)
├── docs/
│   └── ARCHITECTURE.md    # Architecture deep-dive
├── DEVELOPMENT.md     # Contributor guide
├── QUICKSTART.md      # Detailed usage guide
├── skill/agentsbox/   # Bundled skill for agents
└── dist/              # Built output
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [README.md](./README.md) | This file – project overview |
| [QUICKSTART.md](./QUICKSTART.md) | Detailed usage guide with examples |
| [DEVELOPMENT.md](./DEVELOPMENT.md) | Contributor guide |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Architecture deep-dive |
| [CONFIG.md](./CONFIG.md) | Configuration reference |
| [AGENTS.md](./AGENTS.md) | Guidelines for coding agents using agentsbox |
| [llms.txt](./llms.txt) | Compressed context for LLMs |
| [TESTING.md](./TESTING.md) | Testing guide |
| [RELEASE.md](./RELEASE.md) | Release process |

---

## License

MIT

---

## Related

- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)
- [OpenCode](https://opencode.ai/)
- [pi](https://github.com/mariozechner/pi-coding-agent)
