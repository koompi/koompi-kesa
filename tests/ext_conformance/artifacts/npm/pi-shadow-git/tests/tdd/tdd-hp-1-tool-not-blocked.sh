#!/bin/bash
# TDD-HP-1: Tool execution must not be blocked by git operations
# Behavior: Agent completes simple task in <30s (git shouldn't add latency)
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

START=$(date +%s)
PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2? Reply with just the number." 2>&1 >/dev/null || true
END=$(date +%s)

DURATION=$((END - START))
rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: Should complete in <30s
if [ "$DURATION" -lt 30 ]; then
  echo "PASS: completed in ${DURATION}s (< 30s)"
  exit 0
else
  echo "FAIL: took ${DURATION}s (>= 30s) - git may be blocking"
  exit 1
fi
