# pi-shadow-git

Git-based orchestration logging for pi subagents with **Mission Control** dashboard.

Enables branching, rewinding, and forking agent execution paths through automatic git commits and structured audit logging.

## Installation

### Via pi (Recommended)

```bash
# Install globally
pi install git:github.com/EmZod/pi-subagent-with-logging

# Or install for project only
pi install -l git:github.com/EmZod/pi-subagent-with-logging

# Or try without installing
pi -e git:github.com/EmZod/pi-subagent-with-logging
```

Once installed, the extension and skill are automatically loaded. No manual setup required.

### Manual Installation (Alternative)

```bash
# Clone
git clone https://github.com/EmZod/pi-subagent-with-logging.git ~/.pi/packages/shadow-git

# Add to settings.json
# In ~/.pi/agent/settings.json, add to "packages" array:
# "~/.pi/packages/shadow-git"
```

## What's Included

This package provides:

| Resource | Description |
|----------|-------------|
| **Extension: shadow-git** | Commits workspace state after every turn, captures audit logs |
| **Extension: mission-control** | Real-time TUI dashboard for monitoring agents |
| **Skill: pi-subagent-orchestration** | Complete guide for orchestrating subagents with git logging |

## Features

- **Shadow Git Logging** - Commits workspace state after every turn and agent completion
- **Mission Control** - Real-time TUI dashboard for monitoring 100s of agents
- **Audit Trail** - Structured JSONL logs for querying with jq
- **Patch Capture** - Captures diffs when agents modify external repos
- **Killswitch** - Runtime toggle to disable logging during incidents
- **Fail-Open** - Git errors are logged but don't block the agent

## Quick Start

```bash
# 1. Create workspace
WORKSPACE="$HOME/workspaces/$(date +%Y%m%d)-task"
mkdir -p "$WORKSPACE/agents/scout1"/{workspace,output}
cd "$WORKSPACE"

# 2. Write agent plan
cat > agents/scout1/plan.md << 'EOF'
# Plan: Research Task

## Objective
Research X and produce findings.

## Steps
1. Read the codebase
2. Document findings in output/findings.md

## Output
- output/findings.md
EOF

# 3. Spawn agent with shadow-git
PI_WORKSPACE_ROOT="$WORKSPACE" \
PI_AGENT_NAME="scout1" \
pi --max-turns 20 'Read plan.md and execute it.'

# 4. Open Mission Control to monitor
# Type: /mc
```

## Mission Control Dashboard

Monitor multiple agents in real-time with the Mission Control TUI:

```
/mc
# or
/mission-control
```

**Dashboard Features:**
- Real-time status for all agents (running, done, error, pending)
- Turn count, tool calls, error count per agent
- Auto-refresh every 2 seconds
- Scrollable list for 100s of agents
- Sort by status, activity, or name
- Detail panel for selected agent

**Keyboard Controls:**
| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate agents |
| `Enter` | Toggle detail panel |
| `s` | Cycle sort mode |
| `r` | Manual refresh |
| `q` or `Esc` | Close dashboard |

## Commands

| Command | Description |
|---------|-------------|
| `/mission-control` | Open full Mission Control dashboard |
| `/mc` | Alias for mission-control |
| `/shadow-git` | Show logging status |
| `/shadow-git enable` | Enable logging |
| `/shadow-git disable` | Disable logging (killswitch) |
| `/shadow-git history` | Show last 20 commits |

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `PI_WORKSPACE_ROOT` | Yes* | Root of the shadow git workspace |
| `PI_AGENT_NAME` | For logging | Agent name for commits and paths |
| `PI_TARGET_REPOS` | No | Comma-separated target repo paths |
| `PI_TARGET_BRANCH` | No | Branch name for commit linkage |
| `PI_SHADOW_GIT_DISABLED` | No | Set to `1` to disable (killswitch) |

*Required for both Mission Control and logging

## Spawning Subagents

For detailed orchestration patterns, see the included skill documentation. Key patterns:

### Blocking (Sequential)
```bash
PI_WORKSPACE_ROOT="$WORKSPACE" PI_AGENT_NAME="scout" \
pi --max-turns 20 --print 'Read plan.md and execute.'
```

### Non-blocking (Parallel with tmux)
```bash
tmux new-session -d -s scout1 \
  "PI_WORKSPACE_ROOT='$WORKSPACE' PI_AGENT_NAME='scout1' \
   pi --max-turns 30 'Read plan.md and execute.'"
```

### Non-blocking (Headless)
```bash
(PI_WORKSPACE_ROOT="$WORKSPACE" PI_AGENT_NAME="scout1" \
 pi --max-turns 20 --print 'Read plan.md and execute.') &
```

## Architecture

### Per-Agent Git Repos

Each agent has its own isolated git repository, eliminating lock conflicts:

```
workspace/
├── manifest.json                 ← Agent registry
└── agents/
    ├── scout1/
    │   ├── .git/                 ← Agent's OWN repo (isolated)
    │   ├── audit.jsonl           ← Real-time log (NOT in git)
    │   ├── state.json            ← Checkpoint state (IN git)
    │   └── output/               ← Work output (IN git)
    └── scout2/
        ├── .git/                 ← Completely isolated
        └── ...
```

**Benefits:**
- **Zero lock conflicts**: Parallel agents never compete for `.git/index.lock`
- **Turn-level commits**: ~10x fewer commits (per turn, not per tool)
- **Clean separation**: `audit.jsonl` for observability, git for checkpoints

## Audit Log

Events are appended to `agents/{name}/audit.jsonl`:

```json
{"ts":1704567890123,"event":"tool_call","agent":"scout1","turn":3,"tool":"write"}
{"ts":1704567890456,"event":"tool_result","agent":"scout1","turn":3,"tool":"write"}
{"ts":1704567890789,"event":"turn_end","agent":"scout1","turn":3,"toolResultCount":1}
```

Query with jq:

```bash
# Tool calls only
jq 'select(.event == "tool_call")' agents/scout1/audit.jsonl

# Errors
jq 'select(.error == true)' agents/scout1/audit.jsonl
```

## Failure Handling

The extension **fails open**:

| Failure | Behavior |
|---------|----------|
| Git commit fails | Error logged to audit.jsonl, agent continues |
| Audit file write fails | Error to stderr, agent continues |
| Patch capture fails | Error logged, agent continues |

## Killswitch

During an incident, disable logging instantly:

**Runtime:**
```
/shadow-git disable
```

**Environment variable:**
```bash
PI_SHADOW_GIT_DISABLED=1 pi ...
```

## License

MIT
