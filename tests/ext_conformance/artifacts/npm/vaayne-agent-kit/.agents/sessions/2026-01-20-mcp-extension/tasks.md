# Tasks: MCP Extension for Pi

## Status Legend

- [ ] Pending
- [x] Completed

**Task States:** `PENDING` | `IMPLEMENTING` | `VALIDATING` | `REVIEWING` | `APPROVED`

## Phase 1: Foundation

- [x] Task 1: Create types module
  - **Files:** `pi/extensions/mcp/types.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - Define MCPTool type for tool list results
    - Define MCPToolDetail type for inspect results
    - Define MCPInvokeResult type for invoke results
    - Define MCPExecResult type for exec results
  - **Approach:** Created types matching mh CLI JSON output format
  - **Gotchas:** None
  - **Commit:** Included in initial implementation

- [x] Task 2: Create MCP client wrapper
  - **Files:** `pi/extensions/mcp/client.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - MCPClient class with constructor taking configPath and exec function
    - list() method that runs `mh -c config list --json`
    - inspect(toolName) method that runs `mh -c config inspect <tool> --json`
    - invoke(toolName, params) method that runs `mh -c config invoke <tool> '<params>' --json`
    - exec(code) method that runs `mh -c config exec '<code>' --json`
    - Proper error handling for CLI failures
  - **Approach:** Dependency injection via exec function for testability
  - **Gotchas:** 30s timeout on all commands
  - **Commit:** Included in initial implementation

## Phase 2: Extension Core

- [x] Task 3: Create extension entry with session_start handler
  - **Files:** `pi/extensions/mcp/index.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - Export default function receiving ExtensionAPI
    - On session_start, check if ~/.pi/agent/mcp.json exists
    - If exists, create MCPClient and fetch tool list
    - Store tool list for later use
    - Notify user about loaded tools count
  - **Approach:** Check config file existence, then mh CLI availability, then fetch tools
  - **Gotchas:** Gracefully handle missing config or missing CLI
  - **Commit:** Included in initial implementation

- [x] Task 4: Add system prompt injection
  - **Files:** `pi/extensions/mcp/index.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - On before_agent_start, if tools loaded, append to system prompt
    - List available MCP tools with names and descriptions
    - Include usage instructions for mcp_invoke, mcp_inspect, mcp_exec
  - **Approach:** Followed claude-rules.ts pattern for system prompt injection
  - **Gotchas:** None
  - **Commit:** Included in initial implementation

## Phase 3: Tool Registration

- [x] Task 5: Register mcp_inspect tool
  - **Files:** `pi/extensions/mcp/index.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - Tool name: mcp_inspect
    - Parameter: toolName (string, required)
    - Returns detailed tool information including input schema
    - Proper error handling
  - **Approach:** TypeBox schema, returns JSON-formatted MCPToolDetail
  - **Gotchas:** Check client availability before use
  - **Commit:** Included in initial implementation

- [x] Task 6: Register mcp_invoke tool
  - **Files:** `pi/extensions/mcp/index.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - Tool name: mcp_invoke
    - Parameters: toolName (string), params (object, optional)
    - Invokes the MCP tool and returns result
    - Handles errors gracefully
  - **Approach:** Extract text content from result, fall back to JSON
  - **Gotchas:** params is optional, defaults to empty object
  - **Commit:** Included in initial implementation

- [x] Task 7: Register mcp_exec tool
  - **Files:** `pi/extensions/mcp/index.ts`
  - **State:** APPROVED
  - **Iterations:** 1
  - **Acceptance Criteria:**
    - Tool name: mcp_exec
    - Parameter: code (string, required)
    - Executes JS orchestration code via mh exec
    - Returns output or error
  - **Approach:** Pass code directly, handle error field in result
  - **Gotchas:** None
  - **Commit:** Included in initial implementation

## Completion Summary

**Total Tasks:** 7
**Completed:** 7
**Remaining:** 0

### Final Notes

All tasks completed in a single implementation session. The extension follows established patterns:

- `claude-rules.ts` for system prompt injection via `before_agent_start`
- `notify.ts` for process execution via `pi.exec()`
- `tools.ts` for tool registration pattern

Files created:

- `pi/extensions/mcp/types.ts` - TypeScript types for MCP data
- `pi/extensions/mcp/client.ts` - MCPClient class wrapping mh CLI
- `pi/extensions/mcp/index.ts` - Main extension with handlers and tools

Key design decisions:

- Stateless CLI approach: each operation spawns `mh` with config flag
- Session-scoped tool cache: tool list fetched once at session start
- Graceful degradation: no-op when config missing or mh unavailable
- Dependency injection: MCPClient takes exec function for testability
