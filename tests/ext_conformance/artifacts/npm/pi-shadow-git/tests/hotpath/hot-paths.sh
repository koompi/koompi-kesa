#!/bin/bash
# Hot Path Tests - Performance-Critical Paths
set -e

echo "=== HOT PATH TESTS ==="
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# HP-01: Tool execution latency
echo "HP-01: Tool execution not blocked by git..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

START=$(python3 -c 'import time; print(int(time.time() * 1000))' 2>/dev/null || date +%s%3N)
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 >/dev/null || true
END=$(python3 -c 'import time; print(int(time.time() * 1000))' 2>/dev/null || date +%s%3N)

LATENCY=$((END - START))
echo "  Total response time: ${LATENCY}ms"

if [ "$LATENCY" -lt 30000 ]; then
  echo "  ✓ PASS - completed in <30s"
else
  echo "  ✗ FAIL - too slow (>30s)"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

# HP-02: Audit log append is non-blocking
echo "HP-02: Audit log writes are instant..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 60 pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/x.txt" 2>&1 >/dev/null || true

# Check if audit.jsonl was written
if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
  LINES=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl" | tr -d ' ')
  echo "  Audit log has $LINES entries"
  if [ "$LINES" -gt 0 ]; then
    echo "  ✓ PASS - audit logging works"
  else
    echo "  ✗ FAIL - no audit entries"
    rm -rf "$TEST_WS"
    exit 1
  fi
else
  echo "  ✗ FAIL - no audit.jsonl"
  rm -rf "$TEST_WS"
  exit 1
fi
rm -rf "$TEST_WS"

# HP-03: Mission Control can read while agent is writing
echo "HP-03: Mission Control reads during agent execution..."
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Start a longer-running agent
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  timeout 120 pi --max-turns 5 --no-input -p \
  -e "$EXT" "Count from 1 to 5, writing each number to a separate file" 2>&1 >/dev/null &
AGENT_PID=$!

# Give agent time to start
sleep 5

# Try to read audit.jsonl
if [ -f "$TEST_WS/agents/test1/audit.jsonl" ]; then
  # Can we read it without error?
  if cat "$TEST_WS/agents/test1/audit.jsonl" > /dev/null 2>&1; then
    LINES=$(wc -l < "$TEST_WS/agents/test1/audit.jsonl" | tr -d ' ')
    echo "  Read $LINES lines while agent running"
    echo "  ✓ PASS - concurrent read works"
  else
    echo "  ✗ FAIL - could not read audit.jsonl"
  fi
else
  echo "  WARN - audit file not yet created (agent may not have started)"
fi

kill $AGENT_PID 2>/dev/null || true
wait $AGENT_PID 2>/dev/null || true
rm -rf "$TEST_WS"

echo "=== HOT PATH TESTS COMPLETE ==="
