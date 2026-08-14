#!/bin/bash
# TDD-01-2: Agent commits should NOT modify workspace root .git
# RED: Should FAIL on current code (agents modify root .git)
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
cd "$TEST_WS"
git init >/dev/null 2>&1
mkdir -p agents/test1
echo "init" > .marker
git add -A && git commit -m "init" >/dev/null 2>&1
ROOT_BEFORE=$(git rev-parse HEAD 2>/dev/null || echo "NONE")

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

ROOT_AFTER=$(git rev-parse HEAD 2>/dev/null || echo "NONE")

if [ "$ROOT_BEFORE" = "$ROOT_AFTER" ]; then
  echo "PASS: workspace root .git unchanged"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: workspace root .git was modified"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
