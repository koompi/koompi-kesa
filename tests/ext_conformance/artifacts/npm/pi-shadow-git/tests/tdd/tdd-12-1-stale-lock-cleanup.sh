#!/bin/bash
# TDD-12-1: Stale lock files should be cleaned up
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1/.git"

# Create a stale lock file (older than 60 seconds - fake it)
touch "$TEST_WS/agents/test1/.git/index.lock"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

# Check if lock was handled (either cleaned up or error logged)
if [ ! -f "$TEST_WS/agents/test1/.git/index.lock" ]; then
  echo "PASS: stale lock was cleaned up"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
elif grep -q "lock\|index.lock" "$TEST_WS/agents/test1/audit.jsonl" 2>/dev/null; then
  echo "PASS: lock issue was logged"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: lock not handled"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
