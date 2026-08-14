# Formal TDD Protocol

**This is the ONLY development protocol for this refactor.**

---

## The TDD Cycle (Kent Beck)

```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│    ┌─────────┐     ┌─────────┐     ┌──────────┐        │
│    │   RED   │────▶│  GREEN  │────▶│ REFACTOR │───┐    │
│    └─────────┘     └─────────┘     └──────────┘   │    │
│         ▲                                         │    │
│         └─────────────────────────────────────────┘    │
│                                                         │
└─────────────────────────────────────────────────────────┘

RED:      Write ONE failing test for behavior that doesn't exist
GREEN:    Write MINIMUM code to make that ONE test pass
REFACTOR: Clean up code, all tests still pass
REPEAT:   Next behavior
```

---

## Rules (Non-Negotiable)

### Rule 1: No Production Code Without a Failing Test
You may not write any production code until you have written a failing test that requires that code.

### Rule 2: Only Enough Test to Fail
Write only enough of a test to demonstrate a failure. Compilation failures count as failures.

### Rule 3: Only Enough Code to Pass
Write only enough production code to make the currently failing test pass. No more.

### Rule 4: Refactor Only When Green
Only refactor when all tests pass. Never refactor during RED or GREEN phases.

---

## TDD Cycle Format for log.md

Each cycle MUST be logged in this exact format:

```markdown
## TDD-{STEP}-{N}: {Behavior Description}

### RED
**Test:** `{test file or inline test}`
**Expected:** FAIL
**Actual:** FAIL ✓ (or PASS ✗ - test is wrong!)
**What we're testing:** {one specific behavior}

### GREEN
**Code changed:** `{file}:{lines}`
**Minimum change:** {description}
**Test result:** PASS ✓

### REFACTOR
**Changes:** {none | description}
**All tests:** PASS ✓

---
```

---

## TDD Cycles for STEP-01: Per-Agent Git Repos

The current code commits to `PI_WORKSPACE_ROOT/.git` (shared).
We need it to commit to `agents/{name}/.git` (per-agent).

### TDD-01-1: Agent directory gets its own .git

**Behavior:** When an agent starts, `agents/{name}/.git` should be created.

**Test (must fail first):**
```bash
#!/bin/bash
# tests/tdd/tdd-01-1-agent-has-git.sh
# RED: This should FAIL on current code (no per-agent .git exists)

set -e
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

# ASSERTION: Agent should have its own .git
if [ -d "$TEST_WS/agents/test1/.git" ]; then
  echo "PASS: agents/test1/.git exists"
  rm -rf "$TEST_WS"
  exit 0
else
  echo "FAIL: agents/test1/.git does NOT exist"
  rm -rf "$TEST_WS"
  exit 1
fi
```

**Current behavior:** FAIL (git is at workspace root, not agent level)

---

### TDD-01-2: Agent .git is independent of workspace root .git

**Behavior:** Agent commits should NOT modify workspace root `.git`.

**Test (must fail first):**
```bash
#!/bin/bash
# tests/tdd/tdd-01-2-root-git-unchanged.sh
# RED: This should FAIL on current code (agents modify root .git)

set -e
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init >/dev/null 2>&1
mkdir -p agents/test1
git add -A && git commit -m "init" >/dev/null 2>&1
ROOT_BEFORE=$(git rev-parse HEAD)

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'hello' to output/test.txt" 2>&1 >/dev/null || true

ROOT_AFTER=$(git rev-parse HEAD)

# ASSERTION: Root .git should be UNCHANGED
if [ "$ROOT_BEFORE" = "$ROOT_AFTER" ]; then
  echo "PASS: workspace root .git unchanged"
  rm -rf "$TEST_WS"
  exit 0
else
  echo "FAIL: workspace root .git was modified"
  echo "  Before: $ROOT_BEFORE"
  echo "  After:  $ROOT_AFTER"
  rm -rf "$TEST_WS"
  exit 1
fi
```

**Current behavior:** FAIL (agents commit to root .git)

---

### TDD-01-3: audit.jsonl is gitignored in agent repo

**Behavior:** Each agent's `.gitignore` should exclude `audit.jsonl`.

**Test (must fail first):**
```bash
#!/bin/bash
# tests/tdd/tdd-01-3-audit-gitignored.sh
# RED: This should FAIL on current code (no per-agent .gitignore)

set -e
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

# ASSERTION: .gitignore exists and contains audit.jsonl
GITIGNORE="$TEST_WS/agents/test1/.gitignore"
if [ -f "$GITIGNORE" ] && grep -q "audit.jsonl" "$GITIGNORE"; then
  echo "PASS: audit.jsonl is gitignored"
  rm -rf "$TEST_WS"
  exit 0
else
  echo "FAIL: audit.jsonl is NOT gitignored"
  echo "  .gitignore exists: $([ -f "$GITIGNORE" ] && echo yes || echo no)"
  rm -rf "$TEST_WS"
  exit 1
fi
```

**Current behavior:** FAIL (no per-agent .gitignore)

---

### TDD-01-4: Parallel agents have zero lock conflicts

**Behavior:** 3 agents running in parallel should have ZERO git lock conflicts.

**Test (must fail first):**
```bash
#!/bin/bash
# tests/tdd/tdd-01-4-no-lock-conflicts.sh
# RED: This should FAIL on current code (shared .git causes locks)

set -e
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init >/dev/null 2>&1
mkdir -p agents/{a1,a2,a3}
git add -A && git commit -m "init" >/dev/null 2>&1

# Spawn 3 agents in parallel
for agent in a1 a2 a3; do
  PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="$agent" \
    timeout 60 pi --max-turns 3 --no-input -p \
    -e "$EXT" "Write 'hello from $agent' to output/greeting.txt" 2>&1 >/dev/null &
done
wait

# ASSERTION: Zero lock conflicts in any audit log
LOCK_ERRORS=$(grep -r "index.lock\|Unable to create.*lock" "$TEST_WS"/agents/*/audit.jsonl 2>/dev/null | wc -l | tr -d ' ' || echo 0)

if [ "$LOCK_ERRORS" -eq 0 ]; then
  echo "PASS: zero lock conflicts"
  rm -rf "$TEST_WS"
  exit 0
else
  echo "FAIL: $LOCK_ERRORS lock conflicts detected"
  grep -r "index.lock" "$TEST_WS"/agents/*/audit.jsonl 2>/dev/null | head -3
  rm -rf "$TEST_WS"
  exit 1
fi
```

**Current behavior:** FAIL (shared .git causes lock conflicts)

---

## TDD Cycles for STEP-02: Turn-Level Commits

### TDD-02-1: No commits during tool execution

**Behavior:** Git commits should NOT happen after each tool call.

**Test (must fail first):**
```bash
#!/bin/bash
# tests/tdd/tdd-02-1-no-per-tool-commits.sh
# RED: This should FAIL on current code (commits after every tool)

set -e
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Run agent with multiple tools
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 120 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'a' to output/a.txt, then 'b' to output/b.txt, then 'c' to output/c.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
TOOL_CALLS=$(grep -c '"event":"tool_call"' "$AGENT_DIR/audit.jsonl" 2>/dev/null || echo 0)

if [ -d "$AGENT_DIR/.git" ]; then
  COMMITS=$(cd "$AGENT_DIR" && git log --oneline 2>/dev/null | wc -l | tr -d ' ')
else
  # Fallback: check root .git
  COMMITS=$(cd "$TEST_WS" && git log --oneline 2>/dev/null | wc -l | tr -d ' ')
fi

echo "Tool calls: $TOOL_CALLS"
echo "Commits: $COMMITS"

# ASSERTION: Commits should be LESS than tool calls
# (If we have 3+ tool calls, we should NOT have 3+ commits)
if [ "$TOOL_CALLS" -ge 3 ] && [ "$COMMITS" -lt "$TOOL_CALLS" ]; then
  echo "PASS: commits ($COMMITS) < tool calls ($TOOL_CALLS)"
  rm -rf "$TEST_WS"
  exit 0
else
  echo "FAIL: too many commits (commits=$COMMITS, tools=$TOOL_CALLS)"
  rm -rf "$TEST_WS"
  exit 1
fi
```

**Current behavior:** FAIL (commits after every tool call)

---

### TDD-02-2: Commit happens at turn end

**Behavior:** A commit should happen at turn boundaries.

**Test (must fail first):**
```bash
#!/bin/bash
# tests/tdd/tdd-02-2-commit-at-turn-end.sh
# This test verifies commits happen and include "turn" in message

set -e
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/test.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"

if [ -d "$AGENT_DIR/.git" ]; then
  # ASSERTION: At least one commit message should contain "turn"
  if cd "$AGENT_DIR" && git log --oneline 2>/dev/null | grep -qi "turn"; then
    echo "PASS: commit message includes 'turn'"
    rm -rf "$TEST_WS"
    exit 0
  else
    echo "FAIL: no commit message with 'turn'"
    cd "$AGENT_DIR" && git log --oneline 2>/dev/null
    rm -rf "$TEST_WS"
    exit 1
  fi
else
  echo "FAIL: no .git in agent directory (TDD-01 not complete?)"
  rm -rf "$TEST_WS"
  exit 1
fi
```

**Current behavior:** FAIL (commits say "[agent:tool]" not "[agent:turn]")

---

## TDD Cycles for STEP-04: Remove Commit Queue

### TDD-04-1: No commitQueue in source

**Behavior:** The `commitQueue` variable and promise chaining should not exist.

**Test:**
```bash
#!/bin/bash
# tests/tdd/tdd-04-1-no-commit-queue.sh
# This is a static analysis test

set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# ASSERTION: No commitQueue variable
if grep -q "commitQueue" "$EXT"; then
  echo "FAIL: commitQueue still exists in source"
  grep -n "commitQueue" "$EXT" | head -3
  exit 1
else
  echo "PASS: no commitQueue in source"
  exit 0
fi
```

**Current behavior:** FAIL (commitQueue exists)

---

## Implementation Order (Strict TDD)

```
FOR each TDD cycle:
    1. Create test file
    2. Run test → MUST FAIL (RED)
       - If test passes, the test is WRONG (behavior already exists)
       - Rewrite test to target missing behavior
    3. Write MINIMUM code to pass (GREEN)
    4. Run test → MUST PASS
       - If test fails, fix code (not test)
    5. Run ALL tests → MUST PASS (no regressions)
    6. REFACTOR if needed
    7. Log cycle in log.md
    8. Commit: "TDD-{step}-{n}: {behavior}"
```

---

## Verification: How to Know Tests Are Correct

A test is correct if and only if:

1. **It FAILS on current code** (before implementation)
2. **It PASSES after implementation** (minimum code added)
3. **It FAILS again if you revert the implementation**

If a test passes before you write any code, the test is testing the wrong thing.

---

## File Structure

```
tests/
├── tdd/                          # TDD cycle tests (one per behavior)
│   ├── tdd-01-1-agent-has-git.sh
│   ├── tdd-01-2-root-git-unchanged.sh
│   ├── tdd-01-3-audit-gitignored.sh
│   ├── tdd-01-4-no-lock-conflicts.sh
│   ├── tdd-02-1-no-per-tool-commits.sh
│   ├── tdd-02-2-commit-at-turn-end.sh
│   └── tdd-04-1-no-commit-queue.sh
├── regression/                   # Run after EVERY change
│   └── core-functionality.sh
└── run-tdd.sh                    # TDD cycle runner
```

---

## TDD Runner Script

```bash
#!/bin/bash
# tests/run-tdd.sh - Run a single TDD cycle

CYCLE=$1  # e.g., "tdd-01-1"
TEST="tests/tdd/${CYCLE}-*.sh"

if [ -z "$CYCLE" ]; then
  echo "Usage: ./tests/run-tdd.sh <cycle>"
  echo "Example: ./tests/run-tdd.sh tdd-01-1"
  exit 1
fi

TEST_FILE=$(ls $TEST 2>/dev/null | head -1)
if [ -z "$TEST_FILE" ]; then
  echo "No test found for cycle: $CYCLE"
  exit 1
fi

echo "═══════════════════════════════════════"
echo "TDD CYCLE: $CYCLE"
echo "═══════════════════════════════════════"
echo ""

# Run the test
bash "$TEST_FILE"
STATUS=$?

echo ""
if [ $STATUS -eq 0 ]; then
  echo "✅ TEST PASSED"
else
  echo "❌ TEST FAILED"
fi

exit $STATUS
```
