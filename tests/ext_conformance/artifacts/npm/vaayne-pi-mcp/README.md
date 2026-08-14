# MCP Extension

A Pi extension that connects to MCP (Model Context Protocol) servers and provides tools to interact with them.

## Tools

### `mcp_list`

List all available MCP tools with names and descriptions.

### `mcp_inspect`

Show full tool signature as a JSDoc function stub.

**Parameters:**

| Name   | Type   | Required | Description                     |
| ------ | ------ | -------- | ------------------------------- |
| `tool` | string | Yes      | Name of the MCP tool to inspect |

### `mcp_invoke`

Call a single MCP tool with JSON parameters.

**Parameters:**

| Name     | Type   | Required | Description                    |
| -------- | ------ | -------- | ------------------------------ |
| `tool`   | string | Yes      | Name of the MCP tool to invoke |
| `params` | object | No       | Parameters to pass to the tool |

### `mcp_exec`

Execute JavaScript code to orchestrate multiple MCP tool calls with logic.

**Parameters:**

| Name   | Type   | Required | Description                                        |
| ------ | ------ | -------- | -------------------------------------------------- |
| `code` | string | Yes      | JavaScript code using `mcp.callTool(name, params)` |

## Configuration

Create `~/.pi/agent/mcp.json` to configure MCP servers.

## Requirements

This extension depends on the `mh` CLI from [MCP Hub](https://github.com/vaayne/mcphub).

### Automatic Installation

The `mh` binary is automatically installed during `npm install` via the postinstall script. It will be installed to `~/.local/bin/` (or `$XDG_BIN_HOME` if set).

### Manual Installation

If automatic installation fails, you can install `mh` manually:

```bash
# Using the install script
curl -fsSL https://raw.githubusercontent.com/vaayne/mcphub/main/scripts/install.sh | sh

# Or build from source
git clone https://github.com/vaayne/mcphub.git
cd mcphub
go build -o ~/.local/bin/mh .
```

Ensure `~/.local/bin` is in your PATH:

```bash
export PATH="$PATH:$HOME/.local/bin"
```
