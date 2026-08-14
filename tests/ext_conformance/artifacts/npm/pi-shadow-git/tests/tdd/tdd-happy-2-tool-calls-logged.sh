#!/bin/bash
# TDD-HAPPY-2: Tool calls must be logged
# Behavior: audit.jsonl contains tool_call events
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'hello' to output/hello.txt" 2>&1 >/dev/null || true

AUDIT="$TEST_WS/agents/test1/audit.jsonl"

# ASSERTION: tool_call event exists
if grep -q '"event":"tool_call"' "$AUDIT" 2>/dev/null; then
  echo "PASS: tool_call logged"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: no tool_call in audit log"
  cat "$AUDIT" 2>/dev/null || true
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
