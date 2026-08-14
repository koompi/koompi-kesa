#!/bin/bash
# TDD-03-1: state.json should exist in agent directory after turn
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

STATE_FILE="$TEST_WS/agents/test1/state.json"
if [ -f "$STATE_FILE" ]; then
  echo "PASS: state.json exists"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: state.json does not exist"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
