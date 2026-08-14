#!/bin/bash
# TDD-UP-5: Killswitch must disable all logging
# Behavior: PI_SHADOW_GIT_DISABLED=1 prevents audit writes
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# First run WITHOUT killswitch
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "hi" 2>&1 >/dev/null || true

AUDIT="$TEST_WS/agents/test1/audit.jsonl"
BEFORE=$(wc -l < "$AUDIT" 2>/dev/null | tr -d ' ' || echo 0)

# Second run WITH killswitch
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" PI_SHADOW_GIT_DISABLED=1 \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "bye" 2>&1 >/dev/null || true

AFTER=$(wc -l < "$AUDIT" 2>/dev/null | tr -d ' ' || echo 0)
rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: No new lines added when killswitch active
if [ "$AFTER" -eq "$BEFORE" ]; then
  echo "PASS: killswitch prevented logging ($BEFORE lines before and after)"
  exit 0
else
  echo "FAIL: logging continued despite killswitch ($BEFORE -> $AFTER)"
  exit 1
fi
