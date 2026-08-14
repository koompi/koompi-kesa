# pi OTEL Telemetry Extension

Emits OpenTelemetry spans for pi agent lifecycle + tool usage to an OTLP/HTTP collector (Jaeger).

## Install / enable

Install as a **pi package** (recommended):

```bash
# Global install (writes ~/.pi/agent/settings.json)
pi install npm:pi-telemetry-otel

# Project-local install (writes .pi/settings.json)
pi install -l npm:pi-telemetry-otel
```

Try without installing:

```bash
pi -e npm:pi-telemetry-otel "List files"
```

Pi loads the extension from the package’s `pi.extensions` manifest automatically once installed (unless disabled via `pi config`).

## Configuration

All settings accept `PI_` overrides (e.g. `PI_OTEL_SERVICE_NAME`) and fall back to standard OTEL env vars.

| Env var                       | Default                           | Purpose                                                                                         |
| ----------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318/v1/traces` | OTLP/HTTP endpoint                                                                              |
| `OTEL_EXPORTER_OTLP_HEADERS`  | _(none)_                          | Comma-separated `k=v` headers                                                                   |
| `OTEL_SERVICE_NAME`           | `pi-agent`                        | Service name in Jaeger (defaults to agent name from `PI_AGENT_CHAIN` if present)                |
| `OTEL_RESOURCE_ATTRIBUTES`    | _(none)_                          | Comma-separated `k=v` attributes                                                                |
| `PI_AGENT_TRACE_ID`           | _(generated)_                     | Parent trace ID to reuse; set once and kept for subprocess trace linking                        |
| `PI_AGENT_SPAN_ID`            | _(generated)_                     | Parent span ID for subprocess linking; updated to current active span (session/agent/turn/tool) |

## Extension interoperability (child spans)

Other pi extensions can attach spans to the *current* pi trace.

### Recommended: use the helper APIs

```ts
import { withPiSpan } from "pi-telemetry-otel/helpers";

pi.on("tool_call", async (_event, ctx) => {
  await withPiSpan(ctx, "myext.do_work", async (span) => {
    span?.setAttribute("myext.foo", "bar");
    // ... your work
  });
});
```

The helper functions:
- reuse the tracer/export pipeline registered by this extension
- parent spans under the **active** pi span (session/agent/turn/tool)
- prefer a per-session context registry (avoids env-var races in concurrent in-process sub-sessions)

### Advanced: global registries (no hard dependency)

If you don’t want to depend on the package, you can read the registries directly:

- Runtime registry symbol: `Symbol.for("pi.telemetry-otel.runtimeRegistry.v1")`
- Active span context registry symbol: `Symbol.for("pi.telemetry-otel.activeSpanContextRegistry.v1")`
- Map key: `ctx.sessionManager.getSessionId()`

(See `flow-machine` for a reference integration that emits `pi.flow.*` spans.)

## Open the current trace

Use `/open-jaeger-trace` to show the current trace URL and optionally open it in your browser.
If Tailscale is available, the command also surfaces a Tailscale IP URL for remote access.

## Jaeger smoke check

```bash
# Run pi (interactive preferred so spans flush)
PI_OTEL_SERVICE_NAME=pi-agent-dev pi

# Query traces
curl -sSf "http://localhost:16686/api/traces?service=pi-agent-dev&limit=5" | jq -r '.data[].traceID'
curl -sSf "http://localhost:16686/api/traces/<trace-id>" | jq -r '.data[0].spans[].operationName'
```

Expected operations: `pi.session`, `pi.agent`, `pi.turn`, `pi.tool: <tool-name>` (bash adds command preview).

## Tests

```bash
cd telemetry-otel
bun install
bun test
```
