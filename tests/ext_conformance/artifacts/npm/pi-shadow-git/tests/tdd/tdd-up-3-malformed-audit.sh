#!/bin/bash
# TDD-UP-3: Malformed audit.jsonl should not crash Mission Control
# Behavior: Mission Control handles bad JSON gracefully
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/agents/test1"

# Create malformed audit.jsonl
echo "this is not json at all" > "$TEST_WS/agents/test1/audit.jsonl"
echo '{"event":"session_start","ts":12345}' >> "$TEST_WS/agents/test1/audit.jsonl"

OUTPUT=$(PI_WORKSPACE_ROOT="$TEST_WS" \
  pi --max-turns 1 --no-input -p \
  -e "$EXT" "/mc" 2>&1 || true)

rm -rf "$TEST_WS" 2>/dev/null || true

# ASSERTION: No JSON parse crash
if echo "$OUTPUT" | grep -qi "SyntaxError\|Unexpected token\|JSON"; then
  echo "FAIL: Mission Control crashed on bad JSON"
  exit 1
else
  echo "PASS: handled malformed audit.jsonl gracefully"
  exit 0
fi
