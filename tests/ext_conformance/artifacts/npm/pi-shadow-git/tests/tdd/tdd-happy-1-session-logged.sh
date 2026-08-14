#!/bin/bash
# TDD-HAPPY-1: Session start must be logged
# Behavior: audit.jsonl contains session_start event
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

AUDIT="$TEST_WS/agents/test1/audit.jsonl"

# ASSERTION: session_start event exists
if grep -q '"event":"session_start"' "$AUDIT" 2>/dev/null; then
  echo "PASS: session_start logged"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: no session_start in audit log"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
