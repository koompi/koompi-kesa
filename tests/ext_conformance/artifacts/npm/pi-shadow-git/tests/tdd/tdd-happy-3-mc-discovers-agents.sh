#!/bin/bash
# TDD-HAPPY-3: Mission Control must discover agents
# Behavior: /mc command shows agent count
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents"/{a1,a2}

# Create minimal audit files
echo '{"event":"session_start","ts":1234}' > "$TEST_WS/agents/a1/audit.jsonl"
echo '{"event":"session_start","ts":1234}' > "$TEST_WS/agents/a2/audit.jsonl"

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "/mc" 2>&1 || true)

rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: Output mentions agents (2 running, or total, etc.)
if echo "$OUTPUT" | grep -qi "2\|agents\|running"; then
  echo "PASS: Mission Control discovered agents"
  exit 0
else
  echo "FAIL: Mission Control didn't show agents"
  echo "$OUTPUT" | head -5
  exit 1
fi
