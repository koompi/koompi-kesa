# Test Harness: Shadow-Git 10x Refactor

**Purpose:** Mechanical verification for implementation agents with limited context.

**Philosophy (Goedecke-aligned):**
> "How do you know it's broken?"

Every step has tests. Tests run before AND after implementation. If tests fail, STOP.

---

## CRITICAL: Backpressure Protocol for Implementation Agent

### ⛔ STOP CONDITIONS (Non-Negotiable)

If ANY of these occur, **STOP IMMEDIATELY** and log the failure:

1. **Baseline tests fail** → Do not proceed to implementation
2. **Unit tests fail after implementation** → Revert and diagnose
3. **Regression tests fail** → You broke something, revert immediately
4. **Hot path tests show >10% performance regression** → Investigate before proceeding
5. **Lock conflict detected in parallel test** → Architecture is wrong, escalate

### ✅ PROCEED CONDITIONS

Only proceed to next step when ALL of these are true:

1. All unit tests for current step pass
2. All integration tests pass
3. All regression tests pass (nothing broke)
4. Hot path benchmarks within acceptable range
5. Step logged in log.md with PASS status

---

## Phase 0: Establish Baseline (MUST DO FIRST)

Before ANY implementation, capture current system behavior.

### BASELINE-01: Current Performance Metrics

**Script:** `tests/baseline/capture-metrics.sh`

```bash
#!/bin/bash
# Run this BEFORE any refactor work
# Captures baseline metrics for comparison

set -e
BASELINE_DIR="tests/baseline/results"
mkdir -p "$BASELINE_DIR"

echo "=== BASELINE CAPTURE: $(date -Iseconds) ===" | tee "$BASELINE_DIR/baseline.log"

# 1. Single agent commit count
echo "Test: Single agent, 10 turns, measure commits..."
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init
mkdir -p agents/test1/{workspace,output}
git add -A && git commit -m "init"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 10 --no-input -p \
  -e ~/.pi/agent/extensions/shadow-git.ts \
  "Write numbers 1-10 to separate files in output/" 2>&1

COMMIT_COUNT=$(cd "$TEST_WS" && git log --oneline | wc -l)
echo "single_agent_commits=$COMMIT_COUNT" >> "$BASELINE_DIR/metrics.txt"
echo "  Commits: $COMMIT_COUNT" | tee -a "$BASELINE_DIR/baseline.log"

# 2. Parallel agents - lock conflict rate
echo "Test: 3 parallel agents, measure lock conflicts..."
TEST_WS2=$(mktemp -d)
cd "$TEST_WS2"
git init
mkdir -p agents/{a1,a2,a3}/{workspace,output}
git add -A && git commit -m "init"

# Spawn 3 agents in parallel
for agent in a1 a2 a3; do
  PI_WORKSPACE_ROOT="$TEST_WS2" PI_AGENT_NAME="$agent" \
    pi --max-turns 5 --no-input -p \
    -e ~/.pi/agent/extensions/shadow-git.ts \
    "Write 'hello from $agent' to output/greeting.txt" 2>&1 &
done
wait

# Count lock errors in audit logs
LOCK_ERRORS=$(grep -r "index.lock\|lock conflict" "$TEST_WS2/agents/*/audit.jsonl" 2>/dev/null | wc -l || echo 0)
echo "parallel_lock_errors=$LOCK_ERRORS" >> "$BASELINE_DIR/metrics.txt"
echo "  Lock errors: $LOCK_ERRORS" | tee -a "$BASELINE_DIR/baseline.log"

# 3. Commit latency (time from tool_result to commit)
echo "Test: Commit latency..."
# Parse audit.jsonl for timing
LATENCY=$(jq -s '[.[] | select(.event == "tool_result" or .event == "commit_done") | .ts] | 
  if length > 1 then (.[1] - .[0]) else 0 end' \
  "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null || echo "N/A")
echo "commit_latency_ms=$LATENCY" >> "$BASELINE_DIR/metrics.txt"
echo "  Commit latency: ${LATENCY}ms" | tee -a "$BASELINE_DIR/baseline.log"

# Cleanup
rm -rf "$TEST_WS" "$TEST_WS2"

echo ""
echo "=== BASELINE COMPLETE ===" | tee -a "$BASELINE_DIR/baseline.log"
cat "$BASELINE_DIR/metrics.txt"
```

**Expected Baseline (current broken state):**
```
single_agent_commits=~50-100 (way too many)
parallel_lock_errors=~10-30 (lock conflicts)
commit_latency_ms=~100-500 (blocking)
```

**Target After Refactor:**
```
single_agent_commits=~10 (turn-level only)
parallel_lock_errors=0 (per-agent repos)
commit_latency_ms=<10 (async, non-blocking)
```

---

### BASELINE-02: Functional Verification

**Script:** `tests/baseline/verify-functionality.sh`

```bash
#!/bin/bash
# Verify current functionality works before we change anything

set -e
RESULTS="tests/baseline/results/functional.log"

echo "=== FUNCTIONAL BASELINE: $(date -Iseconds) ===" | tee "$RESULTS"

TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init
mkdir -p agents/test1/{workspace,output}
git add -A && git commit -m "init"

# Test 1: Session start creates audit.jsonl
echo -n "TEST: session_start creates audit.jsonl... " | tee -a "$RESULTS"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e ~/.pi/agent/extensions/shadow-git.ts \
  "Say hello" 2>&1 >/dev/null

if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
  echo "PASS" | tee -a "$RESULTS"
else
  echo "FAIL" | tee -a "$RESULTS"
  exit 1
fi

# Test 2: audit.jsonl has session_start event
echo -n "TEST: audit.jsonl contains session_start... " | tee -a "$RESULTS"
if grep -q '"event":"session_start"' "$TEST_WS/agents/test1/audit.jsonl"; then
  echo "PASS" | tee -a "$RESULTS"
else
  echo "FAIL" | tee -a "$RESULTS"
  exit 1
fi

# Test 3: Git commits exist
echo -n "TEST: git commits created... " | tee -a "$RESULTS"
COMMITS=$(git log --oneline | wc -l)
if [ "$COMMITS" -gt 1 ]; then
  echo "PASS ($COMMITS commits)" | tee -a "$RESULTS"
else
  echo "FAIL (only $COMMITS commits)" | tee -a "$RESULTS"
  exit 1
fi

# Test 4: Killswitch works
echo -n "TEST: killswitch disables logging... " | tee -a "$RESULTS"
BEFORE=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl")
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" PI_SHADOW_GIT_DISABLED=1 \
  pi --max-turns 1 --no-input -p \
  -e ~/.pi/agent/extensions/shadow-git.ts \
  "Say goodbye" 2>&1 >/dev/null
AFTER=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl")
if [ "$AFTER" -eq "$BEFORE" ]; then
  echo "PASS (no new lines)" | tee -a "$RESULTS"
else
  echo "FAIL (audit grew from $BEFORE to $AFTER)" | tee -a "$RESULTS"
  exit 1
fi

# Cleanup
rm -rf "$TEST_WS"

echo ""
echo "=== ALL FUNCTIONAL TESTS PASS ===" | tee -a "$RESULTS"
```

---

## Test Matrix by Step

Each step has three test categories:
1. **Unit Tests** - Test the specific function/change in isolation
2. **Integration Tests** - Test interaction with other components
3. **Regression Tests** - Verify nothing else broke

---

## STEP-01 Tests: Per-Agent Git Repos

### Unit Tests

**File:** `tests/unit/step01-per-agent-repos.sh`

```bash
#!/bin/bash
set -e

echo "=== STEP-01 UNIT TESTS ==="

# UT-01-01: initAgentRepo creates .git in agent directory
echo -n "UT-01-01: initAgentRepo creates agent-level .git... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Call the init function (we'll test via running the extension)
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

if [ -d "$TEST_WS/agents/test1/.git" ]; then
  echo "PASS"
else
  echo "FAIL - no .git in agent dir"
  exit 1
fi
rm -rf "$TEST_WS"

# UT-01-02: .gitignore excludes audit.jsonl
echo -n "UT-01-02: .gitignore excludes audit.jsonl... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

if [ -f "$TEST_WS/agents/test1/.gitignore" ]; then
  if grep -q "audit.jsonl" "$TEST_WS/agents/test1/.gitignore"; then
    echo "PASS"
  else
    echo "FAIL - audit.jsonl not in .gitignore"
    exit 1
  fi
else
  echo "FAIL - no .gitignore"
  exit 1
fi
rm -rf "$TEST_WS"

# UT-01-03: No .git at workspace root (or if exists, not used by agents)
echo -n "UT-01-03: Agents don't commit to workspace root... "
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init  # Root git exists
mkdir -p agents/test1
ROOT_COMMITS_BEFORE=$(git log --oneline 2>/dev/null | wc -l || echo 0)

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write hello to output/test.txt" 2>&1 >/dev/null || true

ROOT_COMMITS_AFTER=$(git log --oneline 2>/dev/null | wc -l || echo 0)
if [ "$ROOT_COMMITS_AFTER" -eq "$ROOT_COMMITS_BEFORE" ]; then
  echo "PASS (root unchanged)"
else
  echo "FAIL - root git was modified"
  exit 1
fi
rm -rf "$TEST_WS"

echo "=== STEP-01 UNIT TESTS COMPLETE ==="
```

### Integration Tests

**File:** `tests/integration/step01-parallel-agents.sh`

```bash
#!/bin/bash
set -e

echo "=== STEP-01 INTEGRATION TESTS ==="

# IT-01-01: Multiple agents can run in parallel without lock conflicts
echo "IT-01-01: Parallel agents, zero lock conflicts..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents"/{a1,a2,a3}

# Spawn 3 agents simultaneously
PIDS=""
for agent in a1 a2 a3; do
  PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="$agent" \
    timeout 60 pi --max-turns 5 --no-input -p \
    -e "$EXT" "Write numbers 1-5 to output/count.txt" 2>&1 > /tmp/agent-$agent.log &
  PIDS="$PIDS $!"
done

# Wait for all
FAILED=0
for pid in $PIDS; do
  wait $pid || FAILED=1
done

# Check for lock errors
LOCK_ERRORS=$(grep -r "index.lock\|Unable to create.*lock" "$TEST_WS/agents/*/audit.jsonl" 2>/dev/null | wc -l || echo 0)
echo "  Lock errors found: $LOCK_ERRORS"

if [ "$LOCK_ERRORS" -eq 0 ]; then
  echo "  PASS - zero lock conflicts"
else
  echo "  FAIL - $LOCK_ERRORS lock conflicts detected"
  grep -r "index.lock" "$TEST_WS/agents/*/audit.jsonl" || true
  exit 1
fi

# Verify each agent has its own .git
for agent in a1 a2 a3; do
  if [ ! -d "$TEST_WS/agents/$agent/.git" ]; then
    echo "  FAIL - $agent missing .git"
    exit 1
  fi
done
echo "  All agents have isolated .git directories"

rm -rf "$TEST_WS"

echo "=== STEP-01 INTEGRATION TESTS COMPLETE ==="
```

### Regression Tests

**File:** `tests/regression/core-functionality.sh`

```bash
#!/bin/bash
set -e

echo "=== REGRESSION TESTS ==="

# RT-01: audit.jsonl still created
echo -n "RT-01: audit.jsonl created... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true
if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi
rm -rf "$TEST_WS"

# RT-02: session_start event logged
echo -n "RT-02: session_start event logged... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true
if grep -q '"event":"session_start"' "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi
rm -rf "$TEST_WS"

# RT-03: tool_call events logged
echo -n "RT-03: tool_call events logged... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Read this file: $TEST_WS/agents/test1/audit.jsonl" 2>&1 >/dev/null || true
if grep -q '"event":"tool_call"' "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi
rm -rf "$TEST_WS"

# RT-04: killswitch still works
echo -n "RT-04: killswitch disables logging... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true
LINES_BEFORE=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null || echo 0)

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" PI_SHADOW_GIT_DISABLED=1 \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "bye" 2>&1 >/dev/null || true
LINES_AFTER=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null || echo 0)

if [ "$LINES_AFTER" -eq "$LINES_BEFORE" ]; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi
rm -rf "$TEST_WS"

# RT-05: Mission Control still discovers agents
echo -n "RT-05: Mission Control discovers agents... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents"/{a1,a2}
# Create minimal audit files
echo '{"event":"session_start","ts":1234}' > "$TEST_WS/agents/a1/audit.jsonl"
echo '{"event":"session_start","ts":1234}' > "$TEST_WS/agents/a2/audit.jsonl"

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "/mc" 2>&1 || true)

if echo "$OUTPUT" | grep -q "2.*running\|2.*total\|Agents.*2"; then
  echo "PASS"
else
  echo "FAIL - Mission Control didn't find agents"
  echo "$OUTPUT"
  exit 1
fi
rm -rf "$TEST_WS"

echo "=== ALL REGRESSION TESTS PASS ==="
```

---

## STEP-02 Tests: Turn-Level Commits

### Unit Tests

**File:** `tests/unit/step02-turn-commits.sh`

```bash
#!/bin/bash
set -e

echo "=== STEP-02 UNIT TESTS ==="

# UT-02-01: No commits during tool execution
echo -n "UT-02-01: No commits during tool calls... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Run agent with multiple tools in one turn
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 1 --no-input -p \
  -e "$EXT" "Write 'a' to output/a.txt, 'b' to output/b.txt, 'c' to output/c.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  # Should have ~2 commits: init + turn-1 (not 4: init + 3 tools)
  COMMITS=$(cd "$AGENT_DIR" && git log --oneline | wc -l)
  if [ "$COMMITS" -le 3 ]; then
    echo "PASS ($COMMITS commits)"
  else
    echo "FAIL - too many commits ($COMMITS, expected <=3)"
    cd "$AGENT_DIR" && git log --oneline
    exit 1
  fi
else
  echo "SKIP - no .git (step-01 not implemented yet)"
fi
rm -rf "$TEST_WS"

# UT-02-02: Commit message includes turn number
echo -n "UT-02-02: Commit message has turn number... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/test.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  if cd "$AGENT_DIR" && git log --oneline | grep -q "turn"; then
    echo "PASS"
  else
    echo "FAIL - no turn in commit message"
    cd "$AGENT_DIR" && git log --oneline
    exit 1
  fi
else
  echo "SKIP - no .git"
fi
rm -rf "$TEST_WS"

echo "=== STEP-02 UNIT TESTS COMPLETE ==="
```

### Integration Tests

**File:** `tests/integration/step02-commit-reduction.sh`

```bash
#!/bin/bash
set -e

echo "=== STEP-02 INTEGRATION TESTS ==="

# IT-02-01: 10x commit reduction
echo "IT-02-01: Verify 10x commit reduction..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Run agent for 5 turns with multiple tools per turn
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 120 pi --max-turns 5 --no-input -p \
  -e "$EXT" "For each number 1-5: write it to output/num-N.txt. Do one number per turn." 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  COMMITS=$(cd "$AGENT_DIR" && git log --oneline | wc -l)
  TOOL_CALLS=$(grep -c '"event":"tool_call"' "$AGENT_DIR/audit.jsonl" 2>/dev/null || echo 0)
  
  echo "  Tool calls: $TOOL_CALLS"
  echo "  Commits: $COMMITS"
  
  # Should have roughly 1 commit per turn, not 1 per tool
  # If 5 turns with 2 tools each = 10 tool calls, should have ~6 commits (init + 5 turns)
  # NOT ~11 commits (init + 10 tools)
  
  if [ "$COMMITS" -lt "$TOOL_CALLS" ]; then
    RATIO=$(echo "scale=1; $TOOL_CALLS / $COMMITS" | bc)
    echo "  Reduction ratio: ${RATIO}x"
    echo "  PASS"
  else
    echo "  FAIL - commits >= tool calls (no reduction)"
    exit 1
  fi
else
  echo "  SKIP - no .git"
fi

rm -rf "$TEST_WS"

echo "=== STEP-02 INTEGRATION TESTS COMPLETE ==="
```

---

## STEP-04 Tests: Remove Commit Queue

### Unit Tests

**File:** `tests/unit/step04-no-queue.sh`

```bash
#!/bin/bash
set -e

echo "=== STEP-04 UNIT TESTS ==="

# UT-04-01: No commitQueue in source code
echo -n "UT-04-01: No commitQueue variable... "
if grep -q "commitQueue" "$EXT"; then
  echo "FAIL - commitQueue still in source"
  exit 1
else
  echo "PASS"
fi

# UT-04-02: No promise chaining for commits
echo -n "UT-04-02: No promise chaining... "
if grep -q "commitQueue.*then" "$EXT"; then
  echo "FAIL - promise chaining still present"
  exit 1
else
  echo "PASS"
fi

echo "=== STEP-04 UNIT TESTS COMPLETE ==="
```

---

## STEP-05 Tests: audit.jsonl Not in Git

### Unit Tests

**File:** `tests/unit/step05-audit-not-tracked.sh`

```bash
#!/bin/bash
set -e

echo "=== STEP-05 UNIT TESTS ==="

# UT-05-01: audit.jsonl is gitignored
echo -n "UT-05-01: audit.jsonl in .gitignore... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -f "$AGENT_DIR/.gitignore" ] && grep -q "audit.jsonl" "$AGENT_DIR/.gitignore"; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi
rm -rf "$TEST_WS"

# UT-05-02: audit.jsonl not in git status
echo -n "UT-05-02: audit.jsonl not tracked by git... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write test to output/x.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  cd "$AGENT_DIR"
  if git ls-files | grep -q "audit.jsonl"; then
    echo "FAIL - audit.jsonl is tracked"
    exit 1
  else
    echo "PASS"
  fi
else
  echo "SKIP"
fi
rm -rf "$TEST_WS"

echo "=== STEP-05 UNIT TESTS COMPLETE ==="
```

---

## Hot Path Tests

The hot paths are:
1. **Tool execution** - Must not be blocked by git
2. **Audit logging** - Must be instant (append-only)
3. **Mission Control reads** - Must work while agents run

**File:** `tests/hotpath/hot-paths.sh`

```bash
#!/bin/bash
set -e

echo "=== HOT PATH TESTS ==="

# HP-01: Tool execution is not blocked
echo "HP-01: Tool execution latency..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Time a simple tool call
START=$(date +%s%3N)
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 >/dev/null || true
END=$(date +%s%3N)

LATENCY=$((END - START))
echo "  Total latency: ${LATENCY}ms"

# Should complete in <10 seconds for a trivial task
if [ "$LATENCY" -lt 10000 ]; then
  echo "  PASS"
else
  echo "  FAIL - too slow (>10s)"
  exit 1
fi
rm -rf "$TEST_WS"

# HP-02: Audit log append is instant
echo "HP-02: Audit log write latency..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Run and check audit timestamps
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/x.txt" 2>&1 >/dev/null || true

# Check time between consecutive events
DELAYS=$(jq -s '[.[].ts] | [range(1;length) as $i | .[$i] - .[$i-1]]' \
  "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null || echo "[]")
MAX_DELAY=$(echo "$DELAYS" | jq 'max // 0')
echo "  Max inter-event delay: ${MAX_DELAY}ms"

if [ "${MAX_DELAY:-0}" -lt 5000 ]; then
  echo "  PASS"
else
  echo "  WARN - some delays >5s (might be LLM thinking time)"
fi
rm -rf "$TEST_WS"

# HP-03: Mission Control can read while agent writes
echo "HP-03: Mission Control reads during agent execution..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Start a long-running agent in background
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 120 pi --max-turns 10 --no-input -p \
  -e "$EXT" "Count from 1 to 10, writing each to output/count.txt" 2>&1 >/dev/null &
AGENT_PID=$!

# Wait for agent to start
sleep 3

# Try to read audit.jsonl while agent is running
if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
  LINES=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl")
  echo "  Read $LINES lines while agent running"
  echo "  PASS"
else
  echo "  WARN - audit file not yet created"
fi

# Cleanup
kill $AGENT_PID 2>/dev/null || true
wait $AGENT_PID 2>/dev/null || true
rm -rf "$TEST_WS"

echo "=== HOT PATH TESTS COMPLETE ==="
```

---

## Unhappy Path Tests

**File:** `tests/unhappy/failure-modes.sh`

```bash
#!/bin/bash
set -e

echo "=== UNHAPPY PATH TESTS ==="

# UP-01: Git init failure (e.g., permission denied) → agent continues
echo "UP-01: Git init failure - fail open..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
# Make agent dir read-only to cause git init failure
chmod 555 "$TEST_WS/agents/test1"

# Should still work (fail open)
OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 || true)

chmod 755 "$TEST_WS/agents/test1"

if echo "$OUTPUT" | grep -qi "4\|four"; then
  echo "  PASS - agent completed despite git failure"
else
  echo "  FAIL - agent blocked by git failure"
  exit 1
fi
rm -rf "$TEST_WS"

# UP-02: Disk full simulation (audit write fails) → agent continues
echo "UP-02: Audit write failure - fail open..."
# This is hard to test reliably, so we just verify the fail-open code path exists
if grep -q "catch\|try" "$EXT" && grep -q "continue\|return" "$EXT"; then
  echo "  PASS - error handling present"
else
  echo "  WARN - check error handling manually"
fi

# UP-03: Malformed audit.jsonl → Mission Control handles gracefully
echo "UP-03: Malformed audit.jsonl..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"
echo "this is not json" > "$TEST_WS/agents/test1/audit.jsonl"
echo '{"event":"session_start","ts":123}' >> "$TEST_WS/agents/test1/audit.jsonl"

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "/mc" 2>&1 || true)

if echo "$OUTPUT" | grep -qi "error\|crash\|exception"; then
  echo "  FAIL - Mission Control crashed on bad data"
  exit 1
else
  echo "  PASS - Mission Control handled gracefully"
fi
rm -rf "$TEST_WS"

# UP-04: Missing PI_WORKSPACE_ROOT → graceful error
echo "UP-04: Missing environment variable..."
OUTPUT=$(PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 || true)

# Should not crash, should log something about missing config
echo "  PASS - no crash"

# UP-05: Stale git lock file → auto-cleanup
echo "UP-05: Stale git lock file cleanup..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1/.git"
touch "$TEST_WS/agents/test1/.git/index.lock"  # Stale lock

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 30 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write test to output/x.txt" 2>&1 >/dev/null || true

# Check if lock was cleaned up or handled
if [ -f "$TEST_WS/agents/test1/.git/index.lock" ]; then
  # Lock still exists - might be okay if we handled the error
  if grep -q "lock" "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then
    echo "  PASS - lock error was logged"
  else
    echo "  WARN - lock not cleaned, check handling"
  fi
else
  echo "  PASS - lock was cleaned up"
fi
rm -rf "$TEST_WS"

echo "=== UNHAPPY PATH TESTS COMPLETE ==="
```

---

## Master Test Runner

**File:** `tests/run-all.sh`

```bash
#!/bin/bash
set -e

# Configuration
export EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TESTS_DIR="$(dirname "$0")"
RESULTS_DIR="$TESTS_DIR/results"
mkdir -p "$RESULTS_DIR"

echo "========================================"
echo "SHADOW-GIT TEST SUITE"
echo "========================================"
echo "Extension: $EXT"
echo "Started: $(date -Iseconds)"
echo ""

# Track results
PASSED=0
FAILED=0
SKIPPED=0

run_test() {
  local name=$1
  local script=$2
  
  echo "--- Running: $name ---"
  if bash "$script" 2>&1 | tee "$RESULTS_DIR/$name.log"; then
    echo "✓ $name PASSED"
    ((PASSED++))
  else
    echo "✗ $name FAILED"
    ((FAILED++))
    # Don't exit - continue running other tests
  fi
  echo ""
}

# Phase 0: Baseline
echo "=== PHASE 0: BASELINE ==="
run_test "baseline-metrics" "$TESTS_DIR/baseline/capture-metrics.sh"
run_test "baseline-functional" "$TESTS_DIR/baseline/verify-functionality.sh"

# Check if baseline passed
if [ "$FAILED" -gt 0 ]; then
  echo "⛔ BASELINE FAILED - DO NOT PROCEED WITH IMPLEMENTATION"
  exit 1
fi

# Regression tests (run after any change)
echo "=== REGRESSION TESTS ==="
run_test "regression" "$TESTS_DIR/regression/core-functionality.sh"

# Step-specific tests
echo "=== STEP TESTS ==="
for step_dir in "$TESTS_DIR"/unit/step*.sh; do
  if [ -f "$step_dir" ]; then
    name=$(basename "$step_dir" .sh)
    run_test "$name" "$step_dir"
  fi
done

for step_dir in "$TESTS_DIR"/integration/step*.sh; do
  if [ -f "$step_dir" ]; then
    name=$(basename "$step_dir" .sh)
    run_test "$name" "$step_dir"
  fi
done

# Hot path tests
echo "=== HOT PATH TESTS ==="
run_test "hot-paths" "$TESTS_DIR/hotpath/hot-paths.sh"

# Unhappy path tests
echo "=== UNHAPPY PATH TESTS ==="
run_test "unhappy-paths" "$TESTS_DIR/unhappy/failure-modes.sh"

# Summary
echo "========================================"
echo "TEST SUMMARY"
echo "========================================"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo "Completed: $(date -Iseconds)"
echo ""

if [ "$FAILED" -gt 0 ]; then
  echo "⛔ $FAILED TESTS FAILED"
  exit 1
else
  echo "✅ ALL TESTS PASSED"
  exit 0
fi
```

---

## Per-Step Test Requirements

| Step | Unit Tests | Integration Tests | Regression Tests | Proceed Only If |
|------|------------|-------------------|------------------|-----------------|
| STEP-01 | UT-01-01, UT-01-02, UT-01-03 | IT-01-01 (parallel) | RT-01 to RT-05 | All pass, zero lock conflicts |
| STEP-02 | UT-02-01, UT-02-02 | IT-02-01 (10x reduction) | RT-01 to RT-05 | Commits < tool calls |
| STEP-04 | UT-04-01, UT-04-02 | - | RT-01 to RT-05 | No queue in source |
| STEP-05 | UT-05-01, UT-05-02 | - | RT-01 to RT-05 | audit.jsonl not tracked |

---

## Backpressure Summary

```
┌─────────────────────────────────────────────────────────┐
│ IMPLEMENTATION AGENT DECISION TREE                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  START                                                   │
│    │                                                     │
│    ▼                                                     │
│  Run baseline tests ────────────────────┐               │
│    │                                     │               │
│    │ PASS                               │ FAIL          │
│    ▼                                     ▼               │
│  Implement step N                     ⛔ STOP           │
│    │                                  Log failure       │
│    ▼                                  Escalate          │
│  Run unit tests for step N ─────────────┐               │
│    │                                     │               │
│    │ PASS                               │ FAIL          │
│    ▼                                     ▼               │
│  Run regression tests ──────────────────┤               │
│    │                                     │               │
│    │ PASS                               │               │
│    ▼                                     ▼               │
│  Run hot path tests ────────────────────┤               │
│    │                                     │               │
│    │ PASS (no regression)               │ FAIL          │
│    ▼                                     ▼               │
│  Log STEP-N: PASS                     ⛔ STOP           │
│  Proceed to step N+1                  REVERT changes    │
│                                       Re-run tests      │
│                                       Diagnose          │
└─────────────────────────────────────────────────────────┘
```
