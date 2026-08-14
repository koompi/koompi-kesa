#!/bin/bash
# TDD-02-2: Commit message should include "turn"
# Depends on TDD-01 completing first
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 2 --no-input -p \
  -e "$EXT" "Write 'test' to output/test.txt" 2>&1 >/dev/null || true

AGENT_DIR="$TEST_WS/agents/test1"

if [ -d "$AGENT_DIR/.git" ]; then
  if cd "$AGENT_DIR" && git log --oneline 2>/dev/null | grep -qi "turn"; then
    echo "PASS: commit message includes 'turn'"
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 0
  else
    echo "FAIL: no commit message with 'turn'"
    cd "$AGENT_DIR" && git log --oneline 2>/dev/null || true
    rm -rf "$TEST_WS" 2>/dev/null || true
    exit 1
  fi
else
  echo "FAIL: no .git in agent directory (TDD-01 not complete)"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
