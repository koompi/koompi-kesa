#!/bin/bash
# STEP-01 Integration Test: Parallel Agents Zero Lock Conflicts
set -e

echo "=== STEP-01 INTEGRATION TEST: PARALLEL AGENTS ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents"/{a1,a2,a3}

echo "Spawning 3 agents in parallel..."

for agent in a1 a2 a3; do
  PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="$agent" \
    timeout 60 pi --max-turns 3 --no-input -p \
    -e "$EXT" \
    "Write 'hello from $agent' to output/greeting.txt" 2>&1 > /tmp/agent-$agent.log &
done

echo "Waiting for agents to complete..."
wait

# Count lock errors
LOCK_ERRORS=$(grep -r "index.lock\|Unable to create.*lock" "$TEST_WS"/agents/*/audit.jsonl 2>/dev/null | wc -l | tr -d ' ' || echo 0)

echo "  Lock errors found: $LOCK_ERRORS"

if [ "$LOCK_ERRORS" -eq 0 ]; then
  echo "  ✓ ZERO lock conflicts"
else
  echo "  ✗ FAIL - $LOCK_ERRORS lock conflicts detected"
  echo "  Error samples:"
  grep -r "index.lock" "$TEST_WS"/agents/*/audit.jsonl 2>/dev/null | head -3
  rm -rf "$TEST_WS"
  exit 1
fi

# Verify each agent has isolated .git
echo -n "Verifying isolated .git directories... "
ALL_ISOLATED=1
for agent in a1 a2 a3; do
  if [ ! -d "$TEST_WS/agents/$agent/.git" ]; then
    echo ""
    echo "  FAIL - $agent missing .git"
    ALL_ISOLATED=0
  fi
done

if [ "$ALL_ISOLATED" -eq 1 ]; then
  echo "PASS"
else
  rm -rf "$TEST_WS"
  exit 1
fi

rm -rf "$TEST_WS"
echo "=== STEP-01 INTEGRATION TEST COMPLETE ==="
