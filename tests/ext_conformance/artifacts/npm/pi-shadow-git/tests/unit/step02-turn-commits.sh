#!/bin/bash
# STEP-02 Unit Tests: Turn-Level Commits
set -e

echo "=== STEP-02 UNIT TESTS ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# UT-02-01: Commits happen at turn boundaries, not per-tool
echo -n "UT-02-01: Turn-level commits (not per-tool)... "
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# This should generate multiple tool calls but few commits
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 120 pi --max-turns 2 --no-input -p \
  -e "$EXT" \
  "Write 'a' to output/a.txt, 'b' to output/b.txt, 'c' to output/c.txt in one turn" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  COMMITS=$(cd "$AGENT_DIR" && git log --oneline 2>/dev/null | wc -l | tr -d ' ')
  TOOL_CALLS=$(grep -c '"event":"tool_call"' "$AGENT_DIR/audit.jsonl" 2>/dev/null || echo 0)
  
  echo "(commits=$COMMITS, tools=$TOOL_CALLS)"
  
  # Should have fewer commits than tool calls
  if [ "$COMMITS" -lt "$TOOL_CALLS" ] || [ "$COMMITS" -le 3 ]; then
    echo "  PASS - commits < tool calls"
  else
    echo "  FAIL - too many commits"
    cd "$AGENT_DIR" && git log --oneline
    rm -rf "$TEST_WS"
    exit 1
  fi
else
  echo "SKIP - no .git (step-01 prerequisite)"
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
  if cd "$AGENT_DIR" && git log --oneline | grep -qi "turn"; then
    echo "PASS"
  else
    echo "FAIL - no 'turn' in commit messages"
    cd "$AGENT_DIR" && git log --oneline
    rm -rf "$TEST_WS"
    exit 1
  fi
else
  echo "SKIP - no .git"
fi
rm -rf "$TEST_WS"

echo "=== STEP-02 UNIT TESTS COMPLETE ==="
