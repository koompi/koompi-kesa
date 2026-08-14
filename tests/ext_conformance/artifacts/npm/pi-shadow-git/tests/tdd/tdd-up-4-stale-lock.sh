#!/bin/bash
# TDD-UP-4: Stale .git/index.lock should not block agent
# Behavior: Agent continues despite stale lock file
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1/.git"

# Create stale lock file
touch "$TEST_WS/agents/test1/.git/index.lock"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/x.txt" 2>&1 >/dev/null || true

# ASSERTION: Agent should have created output or at least logged
AUDIT="$TEST_WS/agents/test1/audit.jsonl"
if [ -f "$TEST_WS/agents/test1/output/x.txt" ] || [ -f "$AUDIT" ]; then
  echo "PASS: agent continued despite stale lock"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: agent blocked by stale lock"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
