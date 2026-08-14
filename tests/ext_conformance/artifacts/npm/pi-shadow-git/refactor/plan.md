# Shadow-Git 10x Refactor: Implementation Plan

**Goal:** Achieve 10x performance improvement while maintaining auditability, recoverability, and observability.

**Core Insight (from Goedecke):** "State is the entire problem. One owner, one writer."

---

## Executive Summary

### Current Architecture (Broken)
```
workspace/
├── .git/                    ← SHARED by all agents (lock conflicts!)
└── agents/
    ├── scout1/
    │   └── audit.jsonl
    ├── scout2/
    │   └── audit.jsonl      ← All agents commit to same .git
    └── ...
```

**Problems:**
| Issue | Impact |
|-------|--------|
| Shared .git repo | Lock conflicts when agents run in parallel |
| Per-tool commits | 100s of noisy commits, slow |
| Synchronous commits | Blocks agent execution |
| Mixed concerns | Real-time observability conflated with rollback capability |

### Target Architecture (10x)
```
workspace/
├── manifest.json            ← Orchestrator's view of all agents
└── agents/
    ├── scout1/
    │   ├── .git/            ← Agent's OWN repo (no lock conflicts!)
    │   ├── audit.jsonl      ← Real-time log (NOT in git)
    │   ├── state.json       ← Checkpoint state (IN git)
    │   └── output/          ← Deliverables (IN git)
    ├── scout2/
    │   ├── .git/            ← Completely isolated
    │   └── ...
    └── ...
```

**Improvements:**
| Aspect | Before | After | Gain |
|--------|--------|-------|------|
| Lock conflicts | Constant | Zero | ∞ |
| Commits per agent | ~100 (per tool) | ~10 (per turn) | 10x |
| Commit timing | Synchronous | Async | Non-blocking |
| Real-time data | In git | audit.jsonl | Instant reads |

---

## Design Principles (Goedecke-Aligned)

### 1. Separation of Concerns

| Need | Solution | Rationale |
|------|----------|-----------|
| **Real-time observability** | `audit.jsonl` | Append-only, no locks, instant writes |
| **Rollback/branching** | Git commits | Meaningful turn-level snapshots |
| **Tool-level audit** | `audit.jsonl` queries | Full granularity preserved |
| **Cross-agent coordination** | `manifest.json` | Single source of truth for agent registry |

### 2. One Owner, One Writer

| State | Owner | Writers |
|-------|-------|---------|
| `agents/X/.git` | Agent X | Agent X only |
| `agents/X/audit.jsonl` | Agent X | Agent X only |
| `manifest.json` | Orchestrator | Orchestrator only |

### 3. Fail-Open by Default

| Failure | Behavior |
|---------|----------|
| `git init` fails | Log error, continue without git |

---

## ⚠️ IMPLEMENTATION PROTOCOL (TDD + Backpressure)

**READ THIS FIRST** — This section defines mandatory procedures for ANY implementation agent.

### Test-First Development (TDD)

Every step follows this exact sequence:

```
1. RUN baseline tests      → Must PASS before touching code
2. RUN regression tests    → Capture current behavior
3. IMPLEMENT the step      → Make changes
4. RUN unit tests         → Verify step-specific behavior
5. RUN integration tests  → Verify system behavior
6. RUN regression tests   → Verify nothing broke
7. RUN hot path tests     → Verify performance acceptable
8. LOG results            → Update log.md with PASS/FAIL
```

### ⛔ STOP CONDITIONS (Non-Negotiable)

**If ANY of these occur, STOP IMMEDIATELY:**

| Condition | Action |
|-----------|--------|
| Baseline tests fail | Do NOT proceed. Fix environment first. |
| Unit tests fail after implementation | REVERT changes. Diagnose. |
| Regression tests fail | REVERT immediately. You broke something. |
| Lock conflict in parallel test | Architecture is WRONG. Escalate. |
| Hot path >50% slower | Investigate before proceeding. |

### ✅ PROCEED CONDITIONS

**Only proceed to next step when ALL are true:**

- [ ] All unit tests for current step PASS
- [ ] All integration tests PASS  
- [ ] All regression tests PASS (nothing broke)
- [ ] Hot path benchmarks acceptable
- [ ] Step logged in `log.md` with PASS status

### Test Commands

```bash
# Before ANY implementation
cd /tmp/pi-hook-logging-shitty-state
./tests/run-all.sh

# After EACH step
./tests/run-all.sh

# Run specific test
./tests/unit/step01-per-agent-repos.sh
./tests/integration/step01-parallel-agents.sh
```

### Test Files Reference

| Test Type | Location | When to Run |
|-----------|----------|-------------|
| Baseline | `tests/baseline/` | Before implementation |
| Unit | `tests/unit/step*.sh` | After implementing step |
| Integration | `tests/integration/step*.sh` | After implementing step |
| Regression | `tests/regression/` | After EVERY change |
| Hot Path | `tests/hotpath/` | After EVERY change |
| Unhappy Path | `tests/unhappy/` | After EVERY change |

### Detailed Test Harness

See `TEST-HARNESS.md` for:
- Complete test scripts with code
- Per-step test requirements matrix
- Backpressure decision tree
- Expected baseline metrics

---

## Implementation Tasks

### Phase 1: Core Refactor (shadow-git.ts)

#### STEP-01: Refactor git initialization to per-agent repos

**Context:**
Currently, git operations target `PI_WORKSPACE_ROOT/.git`. After refactor, each agent initializes its own repo at `agents/{name}/.git`.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Remove `isGitRepo()` check at workspace root
2. Add `initAgentRepo(agentDir: string)` function:
   - Check if `agentDir/.git` exists
   - If not, run `git init` in agentDir
   - Create `.gitignore` with `audit.jsonl` (real-time log shouldn't be in git)
   - Initial commit: "agent initialized"
3. Call `initAgentRepo` in `session_start` hook
4. Update all git operations to use `agentDir` as cwd, not workspace root

**Acceptance Criteria:**
- [ ] Each agent has its own `.git` directory
- [ ] No shared git state between agents
- [ ] `audit.jsonl` is gitignored

**🧪 TESTS (Must Pass Before Proceeding):**

| Test | File | Assertion |
|------|------|-----------|
| UT-01-01 | `tests/unit/step01-per-agent-repos.sh` | `.git` exists in agent dir |
| UT-01-02 | `tests/unit/step01-per-agent-repos.sh` | `audit.jsonl` in `.gitignore` |
| UT-01-03 | `tests/unit/step01-per-agent-repos.sh` | Workspace root `.git` unchanged |
| IT-01-01 | `tests/integration/step01-parallel-agents.sh` | **ZERO** lock conflicts with 3 parallel agents |
| RT-* | `tests/regression/core-functionality.sh` | All regression tests still pass |

**⛔ STOP IF:**
- Lock conflicts detected in IT-01-01 (architecture fundamentally broken)
- Workspace root `.git` modified (wrong target)
- Regression tests fail (broke existing behavior)

**Code Sketch:**
```typescript
async function initAgentRepo(agentDir: string): Promise<boolean> {
  const gitDir = join(agentDir, '.git');
  if (existsSync(gitDir)) return true;
  
  try {
    execSync('git init', { cwd: agentDir, stdio: 'pipe' });
    writeFileSync(join(agentDir, '.gitignore'), 'audit.jsonl\n');
    execSync('git add .gitignore', { cwd: agentDir, stdio: 'pipe' });
    execSync('git commit -m "agent initialized"', { cwd: agentDir, stdio: 'pipe' });
    return true;
  } catch (err) {
    logAuditEvent({ event: 'git_init_error', error: String(err) });
    return false;  // Fail open
  }
}
```

---

#### STEP-02: Change commit strategy from per-tool to per-turn

**Context:**
Currently we commit after every `tool_result`. This creates ~10 commits per turn. After refactor, we commit only at `turn_end`, reducing to 1 commit per turn.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Remove git commit from `PostToolUse` handler
2. Add git commit to turn tracking logic (on `turn_end`)
3. Commit message format: `[{agent}:turn-{N}] {summary}`
4. Summary should include: tool count, files changed

**Acceptance Criteria:**
- [ ] Zero commits during tool calls
- [ ] Exactly 1 commit at turn end
- [ ] Commit message includes turn number and tool count

**🧪 TESTS (Must Pass Before Proceeding):**

| Test | File | Assertion |
|------|------|-----------|
| UT-02-01 | `tests/unit/step02-turn-commits.sh` | Commits < tool calls (turn-level) |
| UT-02-02 | `tests/unit/step02-turn-commits.sh` | Commit message contains "turn" |
| IT-02-01 | `tests/integration/step02-commit-reduction.sh` | Commit count reduced by ≥5x |
| RT-* | `tests/regression/core-functionality.sh` | All regression tests still pass |

**⛔ STOP IF:**
- Commits ≥ tool calls (no reduction achieved)
- Regression tests fail
- audit.jsonl events missing (broke logging)

**Code Sketch:**
```typescript
// In turn_end handler:
if (turnStats.toolCalls > 0) {
  const summary = `${turnStats.toolCalls} tools, ${turnStats.filesChanged} files`;
  await gitCommit(`[${agentName}:turn-${turnNumber}] ${summary}`, agentDir);
}
```

---

#### STEP-03: Add state.json checkpoint file

**Context:**
Instead of relying on git history to reconstruct agent state, we maintain an explicit `state.json` that captures checkpoint data. This is what gets committed to git.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Define `AgentState` interface:
   ```typescript
   interface AgentState {
     agent: string;
     turn: number;
     status: 'running' | 'done' | 'error';
     toolCalls: number;
     lastTool: string | null;
     lastActivity: number;
     errors: number;
   }
   ```
2. Create `writeStateCheckpoint(state: AgentState, agentDir: string)` function
3. Call at turn_end before git commit
4. Include state.json in git commits

**Acceptance Criteria:**
- [ ] `state.json` exists in each agent directory
- [ ] Updated at every turn end
- [ ] Committed to git

**🧪 TESTS (Must Pass Before Proceeding):**

| Test | Assertion |
|------|-----------|
| Unit | `state.json` created in agent directory |
| Unit | `state.json` contains valid JSON with required fields |
| Unit | `state.json` included in git commits |
| Regression | All RT-* tests pass |

**⛔ STOP IF:**
- state.json not created
- state.json has invalid/incomplete structure
- Regression tests fail

---

#### STEP-04: Remove commit queue (no longer needed)

**Context:**
The commit queue was added to serialize git commits to a shared repo. With per-agent repos, there are no lock conflicts, so the queue is unnecessary complexity.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Remove `commitQueue` variable and promise chaining
2. Replace `queueCommit()` with direct `gitCommit()` calls
3. Keep git operations async but don't serialize them

**Acceptance Criteria:**
- [ ] No commit queue code remains
- [ ] Git commits still async (non-blocking)
- [ ] Simpler code path

**🧪 TESTS (Must Pass Before Proceeding):**

| Test | Assertion |
|------|-----------|
| Unit | No `commitQueue` variable in source |
| Unit | No promise chaining for commits |
| Integration | Parallel agents still work (IT-01-01) |
| Regression | All RT-* tests pass |

**⛔ STOP IF:**
- `commitQueue` still in code
- Parallel agent test fails (we broke isolation)
- Regression tests fail

---

#### STEP-05: Update audit.jsonl to NOT be git-tracked

**Context:**
`audit.jsonl` is for real-time observability. It grows constantly and shouldn't be in git. Mission Control reads it directly.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Ensure `.gitignore` includes `audit.jsonl`
2. Verify Mission Control reads audit.jsonl correctly (no changes needed)
3. Document: "audit.jsonl is append-only, not in git"

**Acceptance Criteria:**
- [ ] `audit.jsonl` not tracked by git
- [ ] Mission Control still works
- [ ] No git bloat from audit files

**🧪 TESTS (Must Pass Before Proceeding):**

| Test | Assertion |
|------|-----------|
| Unit | `audit.jsonl` listed in `.gitignore` |
| Unit | `git ls-files` does not show `audit.jsonl` |
| Integration | Mission Control reads audit correctly (RT-05) |
| Regression | All RT-* tests pass |

**⛔ STOP IF:**
- audit.jsonl tracked by git
- Mission Control fails to discover agents
- Regression tests fail

---

### Phase 2: Manifest & Orchestration

#### STEP-06: Add workspace manifest.json

**Context:**
The orchestrator needs a single source of truth for which agents exist and their status. This replaces scanning the filesystem.

**Files:** `src/shadow-git.ts` (or new `src/manifest.ts`)

**Tasks:**
1. Define manifest schema:
   ```typescript
   interface Manifest {
     version: 1;
     created: number;
     agents: {
       [name: string]: {
         status: 'pending' | 'running' | 'done' | 'error';
         spawnedAt: number | null;
         completedAt: number | null;
         pid: number | null;
       }
     }
   }
   ```
2. Create `updateManifest(agentName, updates)` function
3. Call on session_start, agent_end
4. Make manifest writes atomic (write to temp, rename)

**Acceptance Criteria:**
- [ ] `manifest.json` at workspace root
- [ ] Updated when agents start/stop
- [ ] Mission Control can use it for faster discovery

---

#### STEP-07: Update Mission Control to use manifest

**Context:**
Currently Mission Control scans `agents/*/audit.jsonl`. With manifest, it can get agent list faster and fall back to audit.jsonl for details.

**Files:** `src/mission-control.ts`

**Tasks:**
1. Read `manifest.json` for agent list (if exists)
2. Fall back to filesystem scan if no manifest
3. Use manifest status as primary, audit.jsonl for details
4. Show manifest vs scan mode in debug

**Acceptance Criteria:**
- [ ] Faster agent discovery with manifest
- [ ] Graceful fallback without manifest
- [ ] Same UI behavior

---

### Phase 3: Rollback & Branching

#### STEP-08: Add `/rollback` command

**Context:**
The whole point of git is rollback capability. We should expose this via a command.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Add `/shadow-git rollback <agent> <turn>` command
2. Implementation:
   - cd to agent directory
   - Find commit for target turn: `git log --oneline | grep "turn-{N}"`
   - `git checkout <commit>` or `git reset --hard <commit>`
3. Update state.json after rollback
4. Log rollback event to audit.jsonl

**Acceptance Criteria:**
- [ ] Can rollback any agent to any turn
- [ ] State.json reflects rolled-back state
- [ ] Audit log shows rollback happened

**Code Sketch:**
```typescript
pi.registerCommand("shadow-git rollback", {
  description: "Rollback agent to a previous turn",
  args: [
    { name: "agent", description: "Agent name" },
    { name: "turn", description: "Turn number to rollback to" }
  ],
  handler: async (args, ctx) => {
    const [agent, turn] = args.split(/\s+/);
    const agentDir = join(workspaceRoot, 'agents', agent);
    // Find commit, reset, update state
  }
});
```

---

#### STEP-09: Add `/branch` command for forking execution

**Context:**
Sometimes you want to try a different approach from a checkpoint without losing the current state. Git branches enable this.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Add `/shadow-git branch <agent> <branch-name> [from-turn]` command
2. Implementation:
   - cd to agent directory
   - If from-turn specified, checkout that turn's commit first
   - `git checkout -b <branch-name>`
3. Track current branch in state.json

**Acceptance Criteria:**
- [ ] Can create branch from any turn
- [ ] Agent continues on new branch
- [ ] Can list branches with `/shadow-git branches <agent>`

---

### Phase 4: Performance & Reliability

#### STEP-10: Make git operations fully async

**Context:**
Even with per-agent repos, git operations should not block the agent. Use fire-and-forget with error logging.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Wrap all `execSync` in async wrapper
2. Use `spawn` or `exec` (not `execSync`)
3. Handle errors in callback, log to audit
4. Don't await git operations in hooks

**Acceptance Criteria:**
- [ ] No `execSync` for git commands
- [ ] Agent never blocks on git
- [ ] Errors still logged

**Code Sketch:**
```typescript
function gitCommitAsync(message: string, cwd: string): void {
  exec(`git add -A && git commit -m "${message}"`, { cwd }, (err) => {
    if (err) logAuditEvent({ event: 'commit_error', error: String(err) });
  });
  // Don't await - fire and forget
}
```

---

#### STEP-11: Add commit batching with debounce

**Context:**
If multiple turn_end events happen rapidly (unlikely but possible), debounce to avoid rapid-fire commits.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Add debounce timer per agent (500ms)
2. If another turn ends within debounce window, batch into single commit
3. Commit message indicates batched turns

**Acceptance Criteria:**
- [ ] Rapid events don't cause rapid commits
- [ ] Debounce is per-agent (not global)
- [ ] Final state still captured

---

#### STEP-12: Add health check and recovery

**Context:**
Git repos can get corrupted. We should detect and recover gracefully.

**Files:** `src/shadow-git.ts`

**Tasks:**
1. Add `isRepoHealthy(agentDir)` check:
   - `git status` succeeds
   - No lock files present
2. On unhealthy repo:
   - Log warning
   - Remove stale lock files
   - If still broken, disable git for this agent (fail open)
3. Check health at session_start

**Acceptance Criteria:**
- [ ] Stale lock files auto-removed
- [ ] Corrupt repos don't crash agent
- [ ] Health issues logged

---

### Phase 5: Documentation & Testing

#### STEP-13: Update README with new architecture

**Files:** `README.md`

**Tasks:**
1. Document per-agent repo structure
2. Document turn-level commit strategy
3. Document new commands (rollback, branch)
4. Add architecture diagram
5. Update troubleshooting section

---

#### STEP-14: Update AGENTS.md for AI agents

**Files:** `AGENTS.md`

**Tasks:**
1. Explain new workspace structure
2. Document manifest.json usage
3. Explain rollback/branch capabilities
4. Update examples

---

#### STEP-15: Add integration tests

**Files:** `tests/` (new directory)

**Tasks:**
1. Test: Multiple agents run in parallel without lock conflicts
2. Test: Turn-level commits work correctly
3. Test: Rollback restores state
4. Test: Branch creates new timeline
5. Test: Fail-open behavior on git errors

---

### Phase 6: Migration

#### STEP-16: Add migration for existing workspaces

**Context:**
Existing workspaces have shared `.git` at root. Need migration path.

**Files:** `src/shadow-git.ts` or `src/migrate.ts`

**Tasks:**
1. Detect old-style workspace (`.git` at root with agents/)
2. Offer migration command: `/shadow-git migrate`
3. Migration:
   - For each agent, extract relevant commits to new per-agent repo
   - Or: just initialize fresh per-agent repos (simpler)
4. Preserve old workspace `.git` as backup

**Acceptance Criteria:**
- [ ] Old workspaces still work (backward compatible)
- [ ] Migration command available
- [ ] No data loss on migration

---

## Operational Concerns (Goedecke Checklist)

### How is it deployed?
- Copy `shadow-git.ts` and `mission-control.ts` to `~/.pi/agent/extensions/`
- Or symlink from cloned repo
- No build step required (TypeScript runs directly in pi)

### How is it rolled back?
- Keep backup of old extension files
- `PI_SHADOW_GIT_DISABLED=1` killswitch stops all logging immediately
- Per-agent repos mean one broken agent doesn't affect others

### How do you know it's broken?
- Status bar shows error count (`⚠️N`)
- Mission Control shows agent errors
- `audit.jsonl` contains all errors with context
- `/shadow-git stats` shows commit success/fail ratio

### How do you debug it?
- `audit.jsonl` has complete event history
- Git log shows checkpoint history
- State.json shows current agent state
- All errors include stack traces and context

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Lock conflicts | ~30% of parallel runs | 0% |
| Commits per agent | ~100 | ~10 |
| Commit latency | Blocking | Non-blocking |
| Recovery from git error | Manual | Automatic |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking existing workflows | Backward-compatible detection, migration command |
| Data loss during migration | Preserve old `.git` as backup |
| Performance regression | Benchmarks before/after |
| Mission Control breakage | Graceful fallback to filesystem scan |

---

## Implementation Order

**Critical Path:**
1. STEP-01 (per-agent repos) ← Eliminates lock conflicts
2. STEP-02 (turn-level commits) ← 10x fewer commits
3. STEP-05 (audit.jsonl not in git) ← Correct separation
4. STEP-04 (remove queue) ← Simplify

**Nice to Have:**
5. STEP-03, STEP-06, STEP-07 (state.json, manifest)
6. STEP-08, STEP-09 (rollback, branch commands)
7. STEP-10, STEP-11, STEP-12 (performance, reliability)
8. STEP-13, STEP-14, STEP-15 (docs, tests)
9. STEP-16 (migration)

---

## Appendix: File Changes Summary

| File | Changes |
|------|---------|
| `src/shadow-git.ts` | Major refactor: per-agent repos, turn-level commits |
| `src/mission-control.ts` | Use manifest.json, minor updates |
| `src/manifest.ts` | New file (optional, can be in shadow-git.ts) |
| `README.md` | Architecture docs, new commands |
| `AGENTS.md` | AI agent instructions |
| `tests/*.ts` | New integration tests |
