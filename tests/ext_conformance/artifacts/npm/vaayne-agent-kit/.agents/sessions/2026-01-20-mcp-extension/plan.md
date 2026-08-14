# Plan: MCP Extension for Pi

## Overview

Create a pi extension that integrates MCPHub's `mh` CLI to provide Model Context Protocol (MCP) support. The extension will automatically start an MCP hub on session start, inject available tools into the system prompt, and register pi tools for invoking MCP operations.

### Goals

- Seamless MCP integration via `mh` CLI
- Automatic tool discovery and system prompt injection
- Support for `invoke`, `inspect`, and `exec` operations
- Clean modular structure in `pi/extensions/mcp/`

### Success Criteria

- [x] Extension loads and checks for `~/.pi/agent/mcp.json` on session start
- [x] Available MCP tools are listed in the system prompt
- [x] `mcp_invoke` tool can invoke MCP tools with JSON parameters
- [x] `mcp_inspect` tool can retrieve detailed tool information
- [x] `mcp_exec` tool can execute JS orchestration code
- [x] Graceful handling when config file doesn't exist

### Out of Scope

- MCP config file management UI
- Real-time tool discovery (only at session start)
- SSE/HTTP transport modes (stdio only)
- Tool enable/disable persistence

## Technical Approach

### Architecture

```
pi/extensions/mcp/
├── index.ts        # Main extension entry point
├── client.ts       # MCP client wrapper for mh CLI
└── types.ts        # TypeScript types for MCP data
```

The extension follows existing patterns:

- `claude-rules.ts` pattern for system prompt injection
- `tools.ts` pattern for tool registration
- `notify.ts` pattern for process execution

### Components

- **MCPClient**: Wrapper class for `mh` CLI commands (list, inspect, invoke, exec)
- **Extension Entry**: Event handlers for session_start and before_agent_start
- **Tools**: Three registered tools (mcp_invoke, mcp_inspect, mcp_exec)

### Data Models

```typescript
// types.ts
type MCPTool = {
  name: string;
  description: string;
};

type MCPToolDetail = {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
};

type MCPInvokeResult = {
  content: Array<{ type: string; text?: string }>;
  isError?: boolean;
};

type MCPExecResult = {
  output: string;
  error?: string;
};
```

### APIs / Interfaces

```typescript
// client.ts
class MCPClient {
  constructor(configPath: string, exec: ExecFn);

  async list(): Promise<MCPTool[]>;
  async inspect(toolName: string): Promise<MCPToolDetail>;
  async invoke(
    toolName: string,
    params: Record<string, unknown>,
  ): Promise<MCPInvokeResult>;
  async exec(code: string): Promise<MCPExecResult>;
}
```

## Implementation Steps

### Phase 1: Foundation

1. Create types module (files: `types.ts`)
2. Create MCP client wrapper (files: `client.ts`)

### Phase 2: Extension Core

3. Create extension entry with session_start handler (files: `index.ts`)
4. Add system prompt injection via before_agent_start (files: `index.ts`)

### Phase 3: Tool Registration

5. Register mcp_inspect tool (files: `index.ts`)
6. Register mcp_invoke tool (files: `index.ts`)
7. Register mcp_exec tool (files: `index.ts`)

## Testing Strategy

### Manual Tests

- Start pi with `~/.pi/agent/mcp.json` present → tools should appear in prompt
- Start pi without config file → no MCP section in prompt
- Use mcp_inspect to get tool details
- Use mcp_invoke to call an MCP tool
- Use mcp_exec to run orchestration code

### Edge Cases

- Config file missing → graceful skip, notify user
- mh command not found → error notification
- MCP server connection failure → handle and report
- Invalid JSON parameters → return error to LLM

## Considerations

### Security

- Tool parameters are passed directly to mh CLI - JSON escaping is handled by mh

### Performance

- Tool list is fetched once at session start, cached for the session
- Each tool invocation spawns a new mh process (stateless)

### Risks & Mitigations

| Risk                 | Likelihood | Impact | Mitigation                      |
| -------------------- | ---------- | ------ | ------------------------------- |
| mh CLI not installed | Medium     | High   | Check existence and notify user |
| Slow tool listing    | Low        | Medium | Show status during loading      |
| Large tool output    | Medium     | Medium | Truncate output if needed       |

### Open Questions

- [x] Should we support tool enable/disable like tools.ts? → No, keep simple for v1

## Review Feedback

### Round 1

(Pending review)

## Implementation Progress

(Updated during implementation phase)
