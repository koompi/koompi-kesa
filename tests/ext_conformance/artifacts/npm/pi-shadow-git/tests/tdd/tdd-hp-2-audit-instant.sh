#!/bin/bash
# TDD-HP-2: Audit log writes must be instant (append-only, no locks)
# Behavior: audit.jsonl is created and has entries after agent runs
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

AUDIT="$TEST_WS/agents/test1/audit.jsonl"

# ASSERTION: audit.jsonl exists and has content
if [ -f "$AUDIT" ] && [ -s "$AUDIT" ]; then
  LINES=$(wc -l < "$AUDIT" | tr -d ' ')
  echo "PASS: audit.jsonl has $LINES entries"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 0
else
  echo "FAIL: audit.jsonl missing or empty"
  rm -rf "$TEST_WS" 2>/dev/null || true
  exit 1
fi
