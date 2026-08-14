#!/bin/bash
# TDD-03-2: state.json should contain valid JSON with required fields
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

STATE_FILE="$TEST_WS/agents/test1/state.json"
if [ -f "$STATE_FILE" ]; then
  # Check required fields: agent, turn, status
  if jq -e '.agent and .turn != null and .status' "$STATE_FILE" >/dev/null 2>&1; then
    echo "PASS: state.json has required fields"
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 0
  else
    echo "FAIL: state.json missing required fields"
    cat "$STATE_FILE"
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 1
  fi
else
  echo "FAIL: state.json does not exist"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
