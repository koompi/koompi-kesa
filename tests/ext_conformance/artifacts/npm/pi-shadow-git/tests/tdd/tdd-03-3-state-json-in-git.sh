#!/bin/bash
# TDD-03-3: state.json should be tracked by git (committed)
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"
if [ -d "$AGENT_DIR/.git" ]; then
  cd "$AGENT_DIR"
  if git ls-files | grep -q "state.json"; then
    echo "PASS: state.json is tracked by git"
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 0
  else
    echo "FAIL: state.json not tracked by git"
    git ls-files
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 1
  fi
else
  echo "FAIL: no .git directory"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
