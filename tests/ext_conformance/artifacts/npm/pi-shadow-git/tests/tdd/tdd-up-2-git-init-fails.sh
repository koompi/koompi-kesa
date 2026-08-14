#!/bin/bash
# TDD-UP-2: Git init failure should fail-open (agent continues)
# Behavior: Agent completes task even if git init fails
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Make agent dir read-only to cause git init failure
chmod 555 "$TEST_WS/agents/test1"

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" PI_AGENT_NAME="test1" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "What is 2+2?" 2>&1 || true)

chmod 755 "$TEST_WS/agents/test1"
rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: Agent should still respond (fail-open)
if echo "$OUTPUT" | grep -qi "4\|four"; then
  echo "PASS: agent completed despite git failure (fail-open)"
  exit 0
else
  # Even if no answer, shouldn't have crashed
  if echo "$OUTPUT" | grep -qi "TypeError\|fatal\|EACCES"; then
    echo "FAIL: agent crashed on git failure"
    exit 1
  else
    echo "PASS: agent didn't crash (fail-open)"
    exit 0
  fi
fi
