#!/bin/bash
# TDD-HP-3: Mission Control can read audit.jsonl while agent writes
# Behavior: Reading audit.jsonl doesn't fail during agent execution
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Start agent in background
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 3 --no-input -p \
  -e "$EXT" "Count 1, 2, 3 writing each to output/count.txt" 2>&1 >/dev/null &
PID=$!

sleep 3  # Let agent start

AUDIT="$TEST_WS/agents/test1/audit.jsonl"
READ_OK=0

# Try to read while agent is running
if [ -f "$AUDIT" ]; then
  if cat "$AUDIT" >/dev/null 2>&1; then
    READ_OK=1
  fi
fi

kill $PID 2>/dev/null || true
wait $PID 2>/dev/null || true
rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: Could read file during agent execution
if [ "$READ_OK" -eq 1 ]; then
  echo "PASS: concurrent read succeeded"
  exit 0
else
  echo "FAIL: could not read audit.jsonl during agent execution"
  exit 1
fi
