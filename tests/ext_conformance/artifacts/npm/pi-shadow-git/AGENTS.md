# Agent Instructions

This repository contains a pi extension for git-based orchestration logging. Read this file first.

## Quick Start

To use the shadow-git extension in your agent workflow:

```bash
PI_WORKSPACE_ROOT="/path/to/workspace" \
PI_AGENT_NAME="your-agent-name" \
  pi -e /path/to/src/shadow-git.ts "your prompt"
```

## What This Extension Does

When enabled, the extension:

1. **Creates per-agent git repo** — `agents/{name}/.git` (isolated from other agents)
2. **Commits at turn boundaries** — `[agent:turn-3] 2 tools` (not per-tool)
3. **Writes checkpoint state** — `state.json` with agent status
4. **Writes structured audit logs** — `audit.jsonl` (real-time, not in git)
5. **Updates workspace manifest** — `manifest.json` for orchestration
6. **Shows status in footer** — `📝 agent-name T3` during execution
7. **Provides runtime killswitch** — `/shadow-git disable` to stop logging

This enables the orchestrator to:
- Run parallel agents without lock conflicts
- View agent history with `cd agents/{name} && git log`
- Rollback to any turn with `/shadow-git rollback <turn>`
- Branch from any turn with `/shadow-git branch <name> [turn]`
- Query events with `jq` on the audit file
- Disable logging during incidents without restarting

## Failure Behavior

**The extension fails open.** If git operations fail:
- Errors are logged to `audit.jsonl` with `event: "commit_error"`
- Agent execution continues uninterrupted
- Use `/shadow-git stats` to check for errors

This design ensures git issues never block agent work.

## Required Environment Variables

| Variable | Purpose |
|----------|---------|
| `PI_WORKSPACE_ROOT` | Absolute path to the shadow git workspace (must be git-initialized) |
| `PI_AGENT_NAME` | Name of this agent (used in commit messages and file paths) |

## Optional Environment Variables

| Variable | Purpose |
|----------|---------|
| `PI_TARGET_REPOS` | Comma-separated paths to target repos (for patch capture) |
| `PI_TARGET_BRANCH` | Branch name to include in commits (for linkage) |
| `PI_SHADOW_GIT_DISABLED` | Set to `1` or `true` to disable (killswitch) |

## Runtime Commands

Use these commands during agent execution:

| Command | Purpose |
|---------|---------|
| `/shadow-git` | Show current status |
| `/shadow-git enable` | Enable logging |
| `/shadow-git disable` | Disable logging (killswitch) |
| `/shadow-git history` | Show last 20 commits |
| `/shadow-git stats` | Show commit/error counts |

## Workspace Structure (v2.0+)

The extension now uses **per-agent git repos** for isolation:

```
{PI_WORKSPACE_ROOT}/
├── manifest.json                 # Agent registry (auto-created)
└── agents/
    └── {PI_AGENT_NAME}/
        ├── .git/                 # Agent's OWN git repo (auto-created)
        ├── .gitignore            # Excludes audit.jsonl from git
        ├── audit.jsonl           # Real-time event log (NOT in git)
        ├── state.json            # Checkpoint state (IN git)
        ├── plan.md               # Your agent's plan (optional)
        ├── log.md                # Your agent's execution log (optional)
        └── output/               # Your agent's outputs (IN git)
```

**Key Changes:**
- Each agent has `agents/{name}/.git` — completely isolated from other agents
- No workspace root `.git` required anymore (agents create their own)
- `audit.jsonl` is NOT tracked by git (for real-time observability)
- `state.json` IS tracked by git (for checkpoints/rollback)

**Benefits:**
- **Zero lock conflicts** when running parallel agents
- **Turn-level commits** (~10x fewer commits)
- **Clean separation**: Real-time data vs checkpoints

## Spawning Pattern

For orchestrators spawning subagents:

```bash
# Create workspace
mkdir -p workspace/agents/scout1/{workspace,output}
cd workspace && git init

# Option 1: Use the spawn script (recommended)
./examples/spawn-with-logging.sh "$(pwd)" scout1 "Read plan.md and execute."

# Option 2: Set env vars before tmux to avoid quoting issues
WORKSPACE="$(pwd)"
PI_WORKSPACE_ROOT="$WORKSPACE" \
PI_AGENT_NAME="scout1" \
  tmux new-session -d -s scout1 \
    "cd $WORKSPACE/agents/scout1 && \
     pi -e /path/to/src/shadow-git.ts \
        --model claude-haiku-4-5 \
        --no-input \
        \"Read plan.md and execute.\" \
        2>&1 | tee output/run.log"
```

**Warning**: Complex shell quoting in tmux commands can cause arguments to be split incorrectly. Use the spawn script or write a temp script file.

## Emergency Killswitch

If an agent's logging is causing problems:

**Immediate (no restart):**
```
/shadow-git disable
```

**Via environment (for new agents):**
```bash
PI_SHADOW_GIT_DISABLED=1 pi -e shadow-git.ts ...
```

The agent continues running; only logging stops.

## Querying Audit Logs

```bash
# All events for an agent
cat agents/scout1/audit.jsonl

# Tool calls only
jq 'select(.event == "tool_call")' agents/scout1/audit.jsonl

# Errors only (tool errors + commit failures)
jq 'select(.error == true or .event == "commit_error")' agents/scout1/audit.jsonl

# Event timeline
jq -c '{ts, event, tool}' agents/scout1/audit.jsonl
```

## Branching Workflow

```bash
# See agent history
git log --oneline

# Branch from specific commit
git checkout -b alternative abc1234

# Continue with new agent from that point
PI_WORKSPACE_ROOT="$(pwd)" PI_AGENT_NAME="scout1-v2" \
  pi -e /path/to/src/shadow-git.ts ...
```

## Integration with pi Subagent Orchestration

This extension complements the pi subagent orchestration skill:

| Layer | Scope | This Extension's Role |
|-------|-------|----------------------|
| Workspace | Plans, logs, outputs | Tracks via shadow git commits |
| Target repo | Code being modified | Captures patches only |

The skill doc's git worktrees/branches handle target repo isolation. This extension handles workspace state tracking.
