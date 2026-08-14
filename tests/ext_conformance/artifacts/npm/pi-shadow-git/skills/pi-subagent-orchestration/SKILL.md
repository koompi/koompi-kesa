---
name: pi-subagent-orchestration-git-only
description: Orchestrate subagents in pi with git-based logging and Mission Control. Use when spawning multiple agents that need audit trails, rollback, branching, and real-time monitoring. Covers per-agent git repos, turn-level commits, workspace setup, and the shadow-git extension.
---

# Pi Subagent Orchestration

**YOU ARE AN ORCHESTRATING AGENT.** Your job is to decompose complex tasks, spawn subagents, coordinate their work, and synthesize results. You never execute leaf-level work yourself when subagents are more appropriate.

---

## ⚠️ THE RESOURCE COMMANDMENTS ⚠️

**SUBAGENTS CONSUME REAL RESOURCES. EVERY AGENT YOU SPAWN:**
- Uses CPU and memory on the user's machine
- Costs money (API calls to LLM providers)
- Runs until killed or max-turns reached
- Can accumulate silently if forgotten

### The 7 Commandments

1. **THOU SHALT KILL WHAT THOU SPAWNS**
   - Every spawn must have a corresponding cleanup plan
   - Never assume agents will terminate themselves

2. **THOU SHALT TRACK ALL PROCESSES**
   - Record PIDs or tmux session names
   - Maintain a list of what you spawned

3. **THOU SHALT SET MAX-TURNS**
   - Always use `--max-turns` to prevent runaway agents
   - 20-30 turns for research, 50 max for complex tasks

4. **THOU SHALT CLEAN UP ON EXIT**
   - Before ending your session, kill all spawned agents
   - `tmux kill-server` or `kill $(cat agents/*/pid)`

5. **THOU SHALT CHECK BEFORE SPAWNING**
   - Run `tmux ls` to see existing sessions
   - Run `ps aux | grep pi` to find orphaned agents
   - Kill stale processes before starting new work

6. **THOU SHALT NOT SPAWN OPUS LIGHTLY**
   - `claude-opus-4-5` costs 15x more than haiku
   - Use haiku for research, sonnet for synthesis
   - Opus only for genuinely hard problems

7. **THOU SHALT MONITOR RESOURCE USAGE**
   - Check `top` or Activity Monitor periodically
   - Watch for runaway CPU/memory consumption
   - Kill stuck agents immediately

### Cleanup Commands (Memorize These)

```bash
# See all tmux sessions
tmux ls

# Kill ALL tmux sessions (nuclear option)
tmux kill-server

# Kill specific session
tmux kill-session -t agent-name

# Find orphaned pi processes
ps aux | grep -E "[p]i --model"

# Kill by PID
kill <pid>

# Check system load
uptime
```

### Before You Start ANY Orchestration

```bash
# 1. Check for existing agents
tmux ls 2>/dev/null || echo "No tmux sessions"
ps aux | grep -E "[p]i " | grep -v grep

# 2. Kill any stale agents from previous work
tmux kill-server 2>/dev/null

# 3. Check system resources
uptime  # Load should be < number of CPU cores

# 4. ⚠️ CRITICAL: Force correct model in settings (CLI flags are IGNORED!)
cat > ~/.pi/agent/settings.json << 'EOF'
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-haiku-4-5",
  "defaultThinkingLevel": "none"
}
EOF
echo "Model set to: $(jq -r .defaultModel ~/.pi/agent/settings.json)"
```

**⚠️ WARNING: `--model` CLI flag is IGNORED! Pi uses `~/.pi/agent/settings.json` instead.**
**If you skip step 4, you WILL spawn Opus agents at 15x the cost of Haiku.**

**IF YOU SPAWN IT, YOU OWN IT. CLEAN UP AFTER YOURSELF.**

---

## Quick Start: Spawn a Single Agent

If you just need to spawn ONE agent quickly, here's the minimum viable setup:

```bash
# 1. Create workspace
WORKSPACE="$HOME/workspaces/$(date +%Y%m%d)-task"
mkdir -p "$WORKSPACE/agents/scout1"/{workspace,output}
cd "$WORKSPACE"

# 2. Write the plan
cat > agents/scout1/plan.md << 'EOF'
# Plan: Research Task

## Objective
Research X and produce findings.

## Steps
1. Read the codebase/docs
2. Document findings in output/findings.md

## Output
- output/findings.md
EOF

# 3. Spawn (using --print for non-interactive mode)
(cd $WORKSPACE/agents/scout1 && \
 pi \
    --model claude-haiku-4-5 \
    --max-turns 20 \
    --print \
    'Read plan.md and execute it.' \
    2>&1 | tee output/run.log) &
echo $! > agents/scout1/pid

# 4. Check status
ps -p $(cat agents/scout1/pid) >/dev/null 2>&1 && echo "Running" || echo "Done"

# 5. View output when done
cat agents/scout1/output/findings.md
```

**Alternative: Use tmux for observability** (can attach to see progress):
```bash
tmux new-session -d -s scout1 \
  "cd $WORKSPACE/agents/scout1 && \
   pi --model claude-haiku-4-5 --max-turns 20 \
   'Read plan.md and execute it.' \
   2>&1 | tee output/run.log"
```

**That's it.** For more complex orchestration (multiple agents, audit trails, coordination), read on.

---

## Table of Contents

0. [THE RESOURCE COMMANDMENTS](#️-the-resource-commandments-️) ← **READ THIS FIRST**
1. [Critical Concepts](#critical-concepts)
2. [Pre-Flight Decisions](#pre-flight-decisions)
3. [Workspace Structure](#workspace-structure)
4. [Spawning Methods](#spawning-methods)
5. [Shadow Git Extension (Audit Trail)](#shadow-git-extension-audit-trail)
6. [Mission Control Dashboard](#mission-control-dashboard)
7. [Output Aggregation Patterns](#output-aggregation-patterns)
8. [Git-Based Coordination](#git-based-coordination)
9. [Logging Protocol](#logging-protocol)
10. [Error Handling and Recovery](#error-handling-and-recovery)
11. [Quick Reference](#quick-reference)
12. [Gotchas and Hard-Won Lessons](#gotchas-and-hard-won-lessons) ← **LEARN FROM THESE**

---

## Critical Concepts

### Why Subagents?

| Problem | Solution |
|---------|----------|
| Task too large for one context window | Decompose into focused subtasks |
| Need parallelism | Spawn multiple agents simultaneously |
| Need isolation | Each agent has clean context |
| Need audit trail | Shadow git tracks everything |
| Task may exceed max turns | Handoff to continuation agent |

### Pi Sessions vs Shadow Git

**Pi has built-in session management** — it stores conversation history in `~/.pi/agent/sessions/`. This is great for single-agent work but has limitations for orchestration:

| Feature | Pi Sessions | Shadow Git |
|---------|-------------|------------|
| Scope | Per-agent, per-cwd | Per-agent repos, unified workspace |
| Branching | Tree structure, UI-driven | Git branches, CLI-driven |
| Multi-agent | Separate session files | Per-agent repos, shared manifest |
| Audit queries | Limited | `git log`, `jq` on JSONL |
| Rollback | Limited | `/shadow-git rollback <turn>` |
| Target repo changes | Not tracked | Captured as patches |

**Use both**: Pi sessions for individual agent state, shadow git for orchestration-level audit trail.

### The TTY Problem (CRITICAL)

**Pi has a Terminal User Interface (TUI)** that shows progress, token counts, and costs. This TUI needs a real terminal (TTY) to render.

**The problem:** When you run `pi` in the background with `&` or `nohup`, there's no terminal. The TUI tries to render, fails, and the process crashes or hangs.

**Simple decision tree:**

```
Do you need to OBSERVE the agent while it runs?
│
├── YES → Use tmux (provides virtual terminal)
│         tmux new-session -d -s name "pi ..."
│
└── NO → Use --print flag (disables TUI, non-interactive mode)
          (pi --print ...) &
```

**NEVER do this:**
```bash
pi ... &              # WRONG: No TTY, no --print → crash
nohup pi ... &        # WRONG: nohup doesn't provide TTY
```

---

## Pre-Flight Decisions

**Before spawning ANY subagent, decide and document:**

### 1. Execution Mode

| Mode | When | How |
|------|------|-----|
| **Blocking** | Need result before continuing | `pi --print ...` (no `&`) |
| **Non-blocking** | Fire and forget, check later | tmux or `(pi --print ...) &` |
| **Parallel + Join** | Multiple independent tasks, wait for all | Spawn all, then `wait` |

### 2. Environment Strategy

| Strategy | When | Setup |
|----------|------|-------|
| **Shared workspace** | Read-only operations | All agents in same dir |
| **Separate directories** | Writes, no git | `agents/{name}/workspace/` |
| **Git worktrees** | Writes, need isolation + merge | `git worktree add ...` |
| **Git branches** | Sequential handoffs | Branch per stage |

### 3. Output Aggregation

| Strategy | When |
|----------|------|
| **Orchestrator reads all** | Few agents (3-5), small outputs |
| **Aggregator subagent** | Many agents, need synthesis |
| **Programmatic** | JSON/CSV, deterministic combine |
| **User explores** | Uncertain value, user guides |

**Record these decisions** in `orchestrator/decisions.md` before spawning.

---

## Workspace Structure

### Per-Agent Git Repos (v2.0+)

Each agent has its **own isolated git repository**, eliminating lock conflicts:

```
{workspace_root}/
├── manifest.json                  # Agent registry (auto-created by extension)
├── orchestrator/
│   ├── decisions.md               # Pre-flight decisions
│   ├── log.md                     # Orchestrator's execution log
│   └── synthesis/                 # Final outputs
│
└── agents/
    ├── {agent_name}/
    │   ├── .git/                  # Agent's OWN repo (auto-created)
    │   ├── .gitignore             # Excludes audit.jsonl
    │   ├── audit.jsonl            # Real-time event log (NOT in git)
    │   ├── state.json             # Checkpoint state (IN git)
    │   ├── plan.md                # REQUIRED: What to do
    │   ├── log.md                 # Agent's execution log
    │   └── output/                # Agent's deliverables (IN git)
    │
    └── {agent_name_2}/
        ├── .git/                  # Completely isolated from other agents
        └── ...
```

**Key Benefits:**
- **Zero lock conflicts** — Parallel agents never compete for `.git/index.lock`
- **Turn-level commits** — ~10x fewer commits (per turn, not per tool)
- **Clean separation** — `audit.jsonl` for real-time observability, git for checkpoints
- **Rollback/branch** — Per-agent history with `/shadow-git rollback` and `/shadow-git branch`

### Creating a Workspace

```bash
# Create workspace (no git init needed at root - agents create their own)
WORKSPACE="$HOME/workspaces/$(date +%Y%m%d)-$TASK_NAME"
mkdir -p "$WORKSPACE"/{orchestrator,agents}
cd "$WORKSPACE"

# Create orchestrator files
cat > orchestrator/decisions.md << 'EOF'
# Pre-Flight Decisions

## Task
{description}

## Execution Mode
{blocking|non-blocking|parallel-join}

## Environment
{shared|directories|worktrees|branches}

## Output Aggregation
{orchestrator|aggregator|programmatic|user-explores}

## Agents
| Name | Role | Model | Tools |
|------|------|-------|-------|
| scout1 | research | claude-haiku-4-5 | read,bash,browser |
| worker1 | implement | claude-sonnet-4-5 | read,write,edit,bash |
EOF

echo "# Orchestrator Log" > orchestrator/log.md
```

### Creating an Agent Directory

```bash
AGENT="scout1"
mkdir -p "agents/$AGENT"/{workspace,output}

# Write plan.md (REQUIRED)
cat > "agents/$AGENT/plan.md" << 'EOF'
# Plan: {Task Name}

## Objective
{One sentence: what this agent must accomplish}

## Scope
- IN: {what IS in scope}
- OUT: {what is NOT in scope}

## Steps

### STEP-01: {Title}
- Do: {action}
- Output: {file or result}

### STEP-02: {Title}
...

### FINAL: Compile Output
- Write findings to output/
- Ensure log.md is complete

## Boundaries
- Do NOT exceed scope
- Log ambiguity, choose simplest path
- All deliverables → output/
EOF
```

---

## Spawning Methods

### Method 1: tmux (RECOMMENDED for observability)

**Why tmux?** It provides a virtual terminal, so pi's TUI works. You can attach to observe progress, costs, and debug in real-time.

```bash
WORKSPACE="$(pwd)"
AGENT="scout1"
EXT="$HOME/.pi/agent/extensions/shadow-git.ts"

# Single agent with shadow-git logging
tmux new-session -d -s "$AGENT" \
  "cd $WORKSPACE/agents/$AGENT && \
   PI_WORKSPACE_ROOT='$WORKSPACE' \
   PI_AGENT_NAME='$AGENT' \
   pi \
      --model claude-haiku-4-5 \
      --tools read,bash,browser \
      --max-turns 30 \
      -e '$EXT' \
      'Read plan.md and execute it.' \
      2>&1 | tee output/run.log"

# Check if running
tmux has-session -t "$AGENT" 2>/dev/null && echo "Running" || echo "Done"

# Attach to observe (Ctrl+B then D to detach)
tmux attach -t "$AGENT"

# Kill if stuck
tmux kill-session -t "$AGENT"
```

**Multiple agents in parallel:**

```bash
WORKSPACE="$(pwd)"
EXT="$HOME/.pi/agent/extensions/shadow-git.ts"
AGENTS="scout1 scout2 scout3"

for agent in $AGENTS; do
  tmux new-session -d -s "$agent" \
    "cd $WORKSPACE/agents/$agent && \
     PI_WORKSPACE_ROOT='$WORKSPACE' \
     PI_AGENT_NAME='$agent' \
     pi --model claude-haiku-4-5 --max-turns 30 \
     -e '$EXT' 'Read plan.md and execute.' 2>&1 | tee output/run.log"
  echo "Spawned: $agent"
done

# List all sessions
tmux ls

# Wait for all to complete (poll)
while tmux ls 2>/dev/null | grep -qE "scout[123]"; do
  echo "Waiting... $(date)"
  sleep 30
done
echo "All done"
```

### Method 2: Bash Background with --print (RECOMMENDED for headless)

**When to use:** Simple tasks where you don't need to observe progress. Fire-and-forget style.

**CRITICAL:** The `--print` flag (or `-p`) is MANDATORY. It disables the TUI and runs non-interactively.

```bash
WORKSPACE="$(pwd)"
AGENT="scout1"
EXT="$HOME/.pi/agent/extensions/shadow-git.ts"

# Single agent
(cd $WORKSPACE/agents/$AGENT && \
 PI_WORKSPACE_ROOT="$WORKSPACE" \
 PI_AGENT_NAME="$AGENT" \
 pi \
    --model claude-haiku-4-5 \
    --max-turns 20 \
    --print \
    -e "$EXT" \
    'Read plan.md and execute.' \
    2>&1 | tee output/run.log) &
echo $! > agents/$AGENT/pid

# Check if running
ps -p $(cat agents/$AGENT/pid) >/dev/null 2>&1 && echo "Running" || echo "Done"

# Wait for completion
wait $(cat agents/$AGENT/pid)
echo "Exit code: $?"
```

### Method 3: Blocking (Sequential)

**When to use:** Agent B needs Agent A's output.

```bash
WORKSPACE="$(pwd)"
EXT="$HOME/.pi/agent/extensions/shadow-git.ts"

# Agent 1 (blocking - wait for result)
cd $WORKSPACE/agents/scout
PI_WORKSPACE_ROOT="$WORKSPACE" PI_AGENT_NAME="scout" \
pi --model claude-haiku-4-5 --max-turns 20 --print \
   -e "$EXT" 'Read plan.md and execute.' > output/result.txt 2>&1

# Agent 2 (uses Agent 1's output)
FINDINGS=$(cat $WORKSPACE/agents/scout/output/result.txt)
cd $WORKSPACE/agents/planner
PI_WORKSPACE_ROOT="$WORKSPACE" PI_AGENT_NAME="planner" \
pi --model claude-sonnet-4-5 --max-turns 20 --print \
   -e "$EXT" "Based on these findings: $FINDINGS - create implementation plan." \
   > output/result.txt 2>&1
```

---

## Shadow Git Extension (Audit Trail)

The shadow-git extension creates a **per-agent git-based audit trail**. Every turn completion is committed to the agent's isolated git repo.

### Why Use It?

| Benefit | How |
|---------|-----|
| **Audit trail** | `git log` shows all agent turns |
| **Rollback** | `/shadow-git rollback <turn>` to go back |
| **Branching** | `/shadow-git branch <name>` to fork execution |
| **Zero lock conflicts** | Each agent has own `.git` |
| **Structured queries** | `jq` on audit.jsonl for analytics |
| **Killswitch** | `/shadow-git disable` or env var to stop |
| **Fail-open** | Git errors logged but don't block agent |

### Installation

**Option A: Pi Package (Recommended)**

```bash
# Install globally - includes extension AND this skill
pi install git:github.com/EmZod/pi-subagent-with-logging

# Or install for project only
pi install -l git:github.com/EmZod/pi-subagent-with-logging

# Or try without installing
pi -e git:github.com/EmZod/pi-subagent-with-logging
```

Once installed, the extension is automatically loaded. No `-e` flag needed.

**Option B: Manual Installation**

```bash
# Clone the repo
git clone https://github.com/EmZod/pi-subagent-with-logging.git

# Copy to global extensions
mkdir -p ~/.pi/agent/extensions
cp pi-subagent-with-logging/extensions/shadow-git.ts ~/.pi/agent/extensions/
cp pi-subagent-with-logging/extensions/lib/mission-control.ts ~/.pi/agent/extensions/
```

> **Note on `-e` flag:** Examples below show `-e "$EXT"` for manual installation. 
> If you installed via `pi install`, the extension auto-loads and you can omit the `-e` flag entirely.

### Usage

**Environment variables:**

| Variable | Required | Description |
|----------|----------|-------------|
| `PI_WORKSPACE_ROOT` | Yes | Absolute path to workspace |
| `PI_AGENT_NAME` | Yes | Agent name (used in commits and paths) |
| `PI_TARGET_REPOS` | No | Comma-separated paths to track external repos |
| `PI_TARGET_BRANCH` | No | Branch name to include in commits |
| `PI_SHADOW_GIT_DISABLED` | No | Set to `1` to disable (killswitch) |

**Spawning with shadow-git:**

```bash
WORKSPACE="$(pwd)"
AGENT="scout1"
EXT="$HOME/.pi/agent/extensions/shadow-git.ts"

# Option A: tmux (for observability)
tmux new-session -d -s "$AGENT" \
  "cd $WORKSPACE/agents/$AGENT && \
   PI_WORKSPACE_ROOT='$WORKSPACE' \
   PI_AGENT_NAME='$AGENT' \
   pi \
      --model claude-haiku-4-5 \
      --max-turns 30 \
      -e '$EXT' \
      'Read plan.md and execute.' \
      2>&1 | tee output/run.log"

# Option B: Background with --print (headless)
(cd $WORKSPACE/agents/$AGENT && \
 PI_WORKSPACE_ROOT="$WORKSPACE" \
 PI_AGENT_NAME="$AGENT" \
 pi \
    --model claude-haiku-4-5 \
    --max-turns 30 \
    --print \
    -e "$EXT" \
    'Read plan.md and execute.' \
    2>&1 | tee output/run.log) &
```

### Commands

| Command | Description |
|---------|-------------|
| `/shadow-git` | Show current status |
| `/shadow-git enable` | Enable logging |
| `/shadow-git disable` | Disable logging (killswitch) |
| `/shadow-git history` | Show last 20 commits |
| `/shadow-git stats` | Show commit/error counts |
| `/shadow-git rollback <turn>` | Reset to previous turn checkpoint |
| `/shadow-git branch <name> [turn]` | Create branch, optionally from turn |
| `/shadow-git branches` | List all branches |

### Killswitch (Emergency Disable)

If logging is causing problems during an incident:

**Runtime (no restart):**
```
/shadow-git disable
```

**Environment (for new agents):**
```bash
PI_SHADOW_GIT_DISABLED=1 pi -e shadow-git.ts ...
```

The extension **fails open**: git errors are logged but don't block the agent.

### What Gets Logged

**Git commits (turn-level):**
```
9187798 [scout1:turn-3] no tools
8e0685e [scout1:turn-2] 1 tools
b705c6a [scout1:turn-1] 2 tools
95b964d [scout1:start] session began
6780e53 agent initialized
```

**Audit JSONL** (`agents/scout1/audit.jsonl`):
```json
{"ts":1704567890123,"event":"tool_call","agent":"scout1","turn":3,"tool":"write","input":{...}}
{"ts":1704567890456,"event":"tool_result","agent":"scout1","turn":3,"tool":"write","error":false}
{"ts":1704567890789,"event":"turn_end","agent":"scout1","turn":3,"toolResultCount":1}
```

### Querying the Audit Trail

```bash
# View agent history (in agent directory)
cd agents/scout1 && git log --oneline

# Query audit events
jq 'select(.event == "tool_call")' agents/scout1/audit.jsonl

# Find errors
jq 'select(.error == true)' agents/scout1/audit.jsonl

# Event timeline
jq -c '{ts: .ts, event: .event, tool: .tool}' agents/scout1/audit.jsonl
```

### Rollback and Branching

```bash
# During execution, use commands:
/shadow-git rollback 2    # Go back to turn 2
/shadow-git branch fix    # Create branch from current state
/shadow-git branch alt 1  # Create branch from turn 1
/shadow-git branches      # List all branches

# Or via git directly (in agent directory):
cd agents/scout1
git log --oneline
git checkout -b alt-approach abc1234
```

---

## Mission Control Dashboard

A real-time TUI dashboard for monitoring multiple agents.

### Quick Start

```bash
# Set workspace root and start pi with the extension
PI_WORKSPACE_ROOT="/path/to/workspace" pi -e ~/.pi/agent/extensions/shadow-git.ts

# Open dashboard
/mc
# or
/mission-control
```

### Features

- Real-time status for all agents (running, done, error, pending)
- Turn count, tool calls, error count per agent
- Auto-refresh every 2 seconds
- Scrollable list for 100s of agents
- Sort by status, activity, or name
- Detail panel for selected agent

### Keyboard Controls

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate agents |
| `Enter` | Toggle detail panel |
| `s` | Cycle sort mode |
| `r` | Manual refresh |
| `q` or `Esc` | Close dashboard |

### Persistent Widget

Shows compact status above the editor while you work:

```
🚀 Mission Control: ● 2 running │ ○ 1 pending │ ✓ 1 done
```

Toggle with `/mc-widget` or `Ctrl+Shift+M`.

### Data Sources

Mission Control reads:
1. `manifest.json` — Agent registry (status, spawn time, PID)
2. `agents/*/audit.jsonl` — Event logs for turn/tool counts
3. `agents/*/state.json` — Checkpoint state

---

## Output Aggregation Patterns

### Pattern A: Orchestrator Reads All (Simple)

**Use when:** 3-5 agents, small outputs.

```bash
# After all agents complete, orchestrator reads outputs
for agent in scout1 scout2 scout3; do
  echo "=== $agent ==="
  cat agents/$agent/output/findings.md
  echo ""
done
```

### Pattern B: Aggregator Subagent

**Use when:** Many agents, complex synthesis needed.

```bash
# Create aggregator after scouts complete
mkdir -p agents/aggregator/{workspace,output}

cat > agents/aggregator/plan.md << 'EOF'
# Plan: Synthesize Findings

## Objective
Read all scout outputs, produce unified synthesis.

## Input
- ../scout1/output/findings.md
- ../scout2/output/findings.md
- ../scout3/output/findings.md

## Steps
1. Read all findings
2. Identify themes, conflicts, gaps
3. Write output/synthesis.md

## Output
- output/synthesis.md (executive summary + details)
EOF

# Spawn aggregator (blocking - we need the result)
cd agents/aggregator
PI_WORKSPACE_ROOT="$(pwd)/../.." PI_AGENT_NAME="aggregator" \
pi --model claude-sonnet-4-5 --max-turns 15 --print \
   -e ~/.pi/agent/extensions/shadow-git.ts \
   'Read plan.md and execute.' 2>&1 | tee output/run.log
```

### Pattern C: Programmatic Combination

**Use when:** Structured outputs (JSON, CSV), deterministic merge.

```bash
# Combine JSON
jq -s 'add' agents/*/output/data.json > results/combined.json

# Combine markdown
for agent in scout1 scout2 scout3; do
  echo "# $agent"
  cat agents/$agent/output/findings.md
  echo "---"
done > results/all-findings.md
```

---

## Git-Based Coordination

### Git Worktrees (Parallel Isolation)

**Use when:** Multiple agents editing same repo, need isolation + merge.

```bash
# Setup
git worktree add agents/worker1/workspace feature/worker1
git worktree add agents/worker2/workspace feature/worker2

# Spawn agents (they work in isolated copies)
for worker in worker1 worker2; do
  tmux new-session -d -s $worker \
    "cd agents/$worker/workspace && pi ... 'Make changes per plan.md'"
done

# After completion, merge
git checkout main
git merge feature/worker1 --no-ff -m "Worker 1 changes"
git merge feature/worker2 --no-ff -m "Worker 2 changes"

# Cleanup
git worktree remove agents/worker1/workspace
git worktree remove agents/worker2/workspace
```

### Git Branches (Sequential Handoffs)

**Use when:** Review gates, audit trail needed.

```bash
git checkout -b task/implement-feature

# Stage 1: Research
cd agents/scout && pi ... 'Research and commit findings'
git add -A && git commit -m "Scout: research complete"

# Stage 2: Plan (builds on scout's commit)
cd agents/planner && pi ... 'Review commits, create plan'
git add -A && git commit -m "Planner: implementation plan"

# Stage 3: Implement
cd agents/worker && pi ... 'Execute plan'
git add -A && git commit -m "Worker: implementation complete"
```

---

## Logging Protocol

**Every subagent MUST maintain an append-only log.**

### Why?

1. **Resumability** — If agent dies, new agent reads log, continues
2. **Debugging** — See exactly what happened
3. **Audit** — Record of all decisions

### Log Format

```markdown
# Agent Log: {agent_name}

## Current
step_id: STEP-01
status: IN_PROGRESS
objective: {what we're doing}

---

## STEP-01: {Title}

### Pre-Execution
**Objective**: {what to accomplish}
**Target**: {files/resources}
**Assumptions**: {what we believe to be true}

### Execution
- Did X
- Found Y
- Wrote Z

### Post-Execution
**Outcome**: PASS | PARTIAL | FAIL
**Notes**: {anything important}

**STEP-01 COMPLETE**

---

## STEP-02: ...
```

### Rules

1. **APPEND-ONLY** — Never edit previous entries
2. **FORWARD-ONLY FIXES** — Bug in STEP-03? Create STEP-04 to fix. Never go back.
3. **ATOMIC STEPS** — Pre → Execution → Post, then sealed
4. **LOG UNCERTAINTY** — Don't hide ambiguity, document it

---

## Error Handling and Recovery

### Agent Crashes

```bash
# Check if agent is running
tmux has-session -t scout1 2>/dev/null || echo "Crashed or completed"

# Check exit in log
tail -20 agents/scout1/output/run.log

# Check last step in log.md
grep -A5 "STEP-" agents/scout1/log.md | tail -20
```

### Resuming Failed Agent

```bash
# Create continuation agent
cat > agents/scout1-resume/plan.md << 'EOF'
# Plan: Continue scout1 Work

## Context
Previous agent crashed. Read ../scout1/log.md to find last completed step.

## Steps
1. Read ../scout1/log.md — find last COMPLETE step
2. Continue from next step
3. Complete remaining work
4. Output to ../scout1/output/ (same location)
EOF

tmux new-session -d -s scout1-resume \
  "cd agents/scout1-resume && pi ... 'Read plan.md and continue work.'"
```

### Using Rollback for Recovery

```bash
# If agent made bad changes, rollback to earlier turn
# First, attach to agent or use commands file:
/shadow-git rollback 5  # Go back to turn 5

# Or via git:
cd agents/scout1
git log --oneline
git reset --hard abc1234  # Reset to good commit
```

---

## Quick Reference

### Spawning Commands

| Task | Command |
|------|---------|
| Spawn with tmux | `tmux new-session -d -s NAME "cd DIR && PI_WORKSPACE_ROOT=... PI_AGENT_NAME=... pi --model MODEL -e shadow-git.ts ..."` |
| Spawn headless | `(cd DIR && PI_WORKSPACE_ROOT=... PI_AGENT_NAME=... pi --model MODEL --print -e shadow-git.ts ...) &` |
| Spawn blocking | `cd DIR && PI_WORKSPACE_ROOT=... PI_AGENT_NAME=... pi --model MODEL --print -e shadow-git.ts ...` |

### Process Management

| Task | tmux | bash background |
|------|------|-----------------|
| Check running | `tmux has-session -t NAME` | `ps -p $(cat pid)` |
| Wait | Poll with `tmux ls` | `wait $(cat pid)` |
| Kill | `tmux kill-session -t NAME` | `kill $(cat pid)` |
| Observe | `tmux attach -t NAME` | `tail -f output/run.log` |
| List all | `tmux ls` | `ps aux \| grep pi` |

### Shadow Git Commands

| Command | Description |
|---------|-------------|
| `/shadow-git` | Show status |
| `/shadow-git enable/disable` | Toggle logging |
| `/shadow-git history` | Show commits |
| `/shadow-git stats` | Show counts |
| `/shadow-git rollback <turn>` | Reset to turn |
| `/shadow-git branch <name> [turn]` | Create branch |
| `/shadow-git branches` | List branches |
| `/mc` | Open Mission Control |
| `/mc-widget` | Toggle status widget |

### Models

| Model | Use For | Cost | Relative |
|-------|---------|------|----------|
| `claude-haiku-4-5` | Fast research, simple tasks | ~$0.001/turn | 1x |
| `claude-sonnet-4-5` | Complex reasoning, implementation | ~$0.01/turn | 10x |
| `claude-opus-4-5` | Hardest problems, synthesis | ~$0.015/turn | 15x |

**⚠️ WARNING: CLI `--model` flag is IGNORED!** Pi uses `~/.pi/agent/settings.json`.

---

## Gotchas and Hard-Won Lessons

### 1. Use `--print` Not `--print-last`

**Wrong:** `pi --print-last ...` (flag doesn't exist!)
**Right:** `pi --print ...` or `pi -p ...`

The `--print` flag runs pi in non-interactive mode: it processes the prompt and exits without the TUI.

### 2. Use `-e` Not `--hook`

**Wrong:** `pi --hook shadow-git.ts ...` (deprecated)
**Right:** `pi -e shadow-git.ts ...`

### 3. Set PI_WORKSPACE_ROOT for Mission Control

```bash
# Mission Control needs this to find agents
PI_WORKSPACE_ROOT=/path/to/workspace pi -e shadow-git.ts
# Then: /mc
```

### 4. Extension Path Must Be Absolute in tmux

```bash
# WRONG: Relative path may fail
-e ./shadow-git.ts

# RIGHT: Absolute path
-e /full/path/to/shadow-git.ts
# or
-e ~/.pi/agent/extensions/shadow-git.ts
```

### 5. Env Vars Must Be Set Inside tmux Command

```bash
# WRONG: Vars not passed to tmux subshell
PI_WORKSPACE_ROOT="$PWD" tmux new-session -d -s agent "pi ..."

# RIGHT: Set vars inside the command string
tmux new-session -d -s agent "PI_WORKSPACE_ROOT='$PWD' PI_AGENT_NAME='agent' pi ..."
```

### 6. Check manifest.json for Agent Status

```bash
# Quick status check
cat workspace/manifest.json | jq '.agents | to_entries[] | {name: .key, status: .value.status}'
```

---

## Checklist Before Spawning

```
[ ] Pre-flight decisions documented
    [ ] Execution mode (blocking/non-blocking/parallel)
    [ ] Environment (shared/directories/worktrees)
    [ ] Aggregation strategy

[ ] Workspace setup
    [ ] agents/{name}/ directories created
    [ ] orchestrator/decisions.md written

[ ] Per-agent setup
    [ ] plan.md with clear steps
    [ ] output/ directory exists

[ ] Spawn command ready
    [ ] TTY decision: tmux OR --print
    [ ] settings.json configured with correct model
    [ ] PI_WORKSPACE_ROOT and PI_AGENT_NAME set (for shadow-git)
    [ ] -e with absolute path to shadow-git.ts
    [ ] --max-turns set

[ ] Monitoring plan
    [ ] Mission Control ready (/mc) if using shadow-git
    [ ] Know how to check status
    [ ] Know how to kill if stuck
```

---

## Final Reminder: CLEANUP

**Before you finish ANY orchestration session:**

```bash
# 1. List what's running
tmux ls
ps aux | grep -E "[p]i --model" | grep -v grep

# 2. Kill everything you spawned
tmux kill-server  # or kill specific sessions

# 3. Verify cleanup
tmux ls  # Should show "no server running"
```

**You spawned it. You kill it. No exceptions.**
