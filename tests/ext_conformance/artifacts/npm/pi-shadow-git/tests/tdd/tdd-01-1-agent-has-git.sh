#!/bin/bash
# TDD-01-1: Agent directory gets its own .git
# RED: Should FAIL on current code (no per-agent .git)
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

if [ -d "$TEST_WS/agents/test1/.git" ]; then
  echo "PASS: agents/test1/.git exists"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: agents/test1/.git does NOT exist"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
